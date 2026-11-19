//! The `.intent` entry points: intent compilation against the imported
//! domain union (issue pherolang#196).

use salsa::Database as Db;

use crate::compilation::IntentCompilation;
use crate::entry::finish_surface;
use crate::inputs::CompileInputs;
use crate::session::{parse_intent_query, Database, SourceFile};
use crate::union::{domain_files, lower_domains};
use crate::intents;

/// Compiles a `.intent` file against its imported domain models to the
/// standalone [`phero_ir::intents::IntentModel`] (memoized by salsa): parse
/// the file, lower every named domain through the ordinary domain pipeline
/// (the same import discipline [`compile_actors_str`](crate::compile_actors_str)
/// uses), then validate the intents and lower the artifact.
///
/// The tracked surface carries **no cross-surface artifacts**: link names
/// are never resolved here — the project-level coverage pass (the
/// `driver.integrity` band's `X0009`+ codes) resolves them against the
/// application's compiled artifacts. Use this query for parse and
/// stable-id editing feedback.
#[salsa::tracked(returns(clone))]
pub fn compile_intent(
    db: &dyn Db,
    file: SourceFile,
    domains: Vec<SourceFile>,
) -> IntentCompilation {
    compile_intent_with(db, file, domains, &CompileInputs::default())
}

/// The body of [`compile_intent`], parameterized over the compile-input
/// bundle: its `imports` provider for the domain files (see
/// [`compile_file`](crate::compile_file) for the import shape).
fn compile_intent_with(
    db: &dyn Db,
    file: SourceFile,
    domains: Vec<SourceFile>,
    inputs: &CompileInputs,
) -> IntentCompilation {
    let path = file.path(db);
    let parsed = parse_intent_query(db, file);
    let domain_imports: Vec<phero_syntax::Import> = parsed
        .ast
        .as_ref()
        .map(|ast| ast.imports.clone())
        .unwrap_or_default();
    let domains = lower_domains(db, &path, &domain_imports, &domains, inputs.imports());

    let (model, diagnostics, spans) = intents::compile_intent_file(
        &path,
        parsed.ast.as_ref(),
        &parsed.diagnostics,
        &domains.domain_units,
    );
    finish_surface(domains, model, diagnostics, |model, domains_model, diagnostics| {
        IntentCompilation::assemble(model, domains_model, diagnostics, spans)
    })
}

/// Compiles a `.intent` file against its imported domain models in one call.
///
/// `inputs` is the one [`CompileInputs`] bundle the host builds once from
/// the gathered import closure: its `domains` bucket holds the raw `.mox`
/// texts keyed by the path the host resolved (the
/// [`compile_actors_str`](crate::compile_actors_str) shape — an `.intent`
/// file imports domains only; links are not imports, they are resolved by
/// the coverage pass). Every import matches a provided entry by exact path
/// or by lexical resolution relative to the `.intent` file's directory.
/// Extra entries no import names are ignored; an import no provided entry
/// names is the error ``imported file "<path>" was not provided``. The
/// result carries diagnostics from the `.intent` file and each imported
/// domain, each tagged with its path — the same shape
/// [`compile_evt_str`](crate::compile_evt_str) returns, so a host renders
/// both with ariadne identically.
///
/// ```
/// let domains = [(
///     "delivery.mox".to_string(),
///     "package nz.example.delivery\n\nclass Drone {}".to_string(),
/// )];
/// let source = concat!(
///     "import \"delivery.mox\"\n",
///     "\n",
///     "intent DRONE-001 \"A drone below 10 kg flies without a waiver.\" {\n",
///     "    element class:Drone;\n",
///     "    check invariant:Drone;\n",
///     "}\n",
/// );
/// let compilation = phero_driver::compile_intent_str(
///     "delivery.intent",
///     source,
///     &phero_driver::CompileInputs::from_domains(&domains),
/// );
/// assert!(compilation.diagnostics.is_empty());
/// assert_eq!(compilation.model.unwrap().intents.len(), 1);
/// ```
///
/// # Normative contract
///
/// ## Entry points
///
/// - [`compile_intent_str`] — parse, domain lowering, intent validation,
///   and artifact lowering in one call, against caller-provided domain
///   texts.
/// - [`compile_intent`] / [`parse_intent_query`] — the salsa-tracked
///   queries the one-shot wrapper runs on a fresh [`Database`].
/// - `compile_intent_file` — the salsa-free engine the entry point calls.
///
/// ## Validation rules
///
/// Any error-severity diagnostic anywhere (the `.intent` file or an
/// imported domain) drops the artifact; warnings do not. Per intent, in
/// source order: the stable-id shape (`E0450`), in-file id uniqueness
/// (`E0451`), the non-empty statement (`E0452`), and — per link in source
/// order — the slot's kind vocabulary (`E0453`) and the slot's kind+name
/// uniqueness (`E0454`). Link names are carried as authored; resolution is
/// the coverage pass's job, never the compile's.
pub fn compile_intent_str(path: &str, source: &str, inputs: &CompileInputs) -> IntentCompilation {
    let db = Database::new();
    let file = SourceFile::new(&db, path.to_string(), source.to_string());
    let files = domain_files(&db, inputs.domains());
    compile_intent_with(&db, file, files, inputs)
}
