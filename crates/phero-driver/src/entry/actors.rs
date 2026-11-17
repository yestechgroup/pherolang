//! The `.actor` entry points: standalone actor-policy compilation against
//! the imported domain union, with optional grant-coverage validation.

use salsa::Database as Db;

use crate::compilation::ActorCompilation;
use crate::entry::finish_surface;
use crate::inputs::CompileInputs;
use crate::lower;
use crate::session::{parse_actors_query, Database, SourceFile};
use crate::union::{domain_files, lower_domains};

/// Compiles an `.actor` file against its imported domain models to the
/// standalone [`phero_ir::ActorModel`] (memoized by salsa): parse the actor
/// file, lower every named domain against the **union namespace of all
/// imports** (so domains may reference types across packages — the same
/// rule multi-file `.mox` compiles follow), then resolve and validate the
/// union policy set.
///
/// Extra provided domains no import names are ignored; a domain with
/// errors contributes its diagnostics and no blocks, but the remaining
/// domains keep resolving.
///
/// The tracked query carries no compile-input bundle, so its designs bucket
/// is empty: the grant-coverage and noun-classification passes
/// [`compile_actors_str`] runs with provided designs stay silent here.
#[salsa::tracked(returns(clone))]
pub fn compile_actors(
    db: &dyn Db,
    actor: SourceFile,
    domains: Vec<SourceFile>,
) -> ActorCompilation {
    compile_actors_with(db, actor, domains, &CompileInputs::default())
}
/// The body of [`compile_actors`], parameterized over the compile-input
/// bundle: its `imports` bucket provides the domain files' `import
/// schema`/`import sigil` content (see [`compile_file`] for the shape) and
/// its `designs` bucket is the carrier set [`compile_actors_str`] validates
/// grant coverage and noun classification against.
fn compile_actors_with(
    db: &dyn Db,
    actor: SourceFile,
    domains: Vec<SourceFile>,
    inputs: &CompileInputs,
) -> ActorCompilation {
    let actor_path = actor.path(db);
    let actor_source = actor.text(db);
    let parsed = parse_actors_query(db, actor);
    let domains = lower_domains(
        db,
        &actor_path,
        parsed
            .ast
            .as_ref()
            .map(|ast| ast.imports.as_slice())
            .unwrap_or_default(),
        &domains,
        inputs.imports(),
    );
    // The coverage pass consumes design artifacts, not their paths; the
    // bundle keys them so the gatherers share one shape.
    let designs: Vec<phero_ir::ddd::DddModel> = inputs
        .designs()
        .iter()
        .map(|(_, model)| model.clone())
        .collect();

    let (model, diagnostics) = lower::compile_actor_file(
        &actor_path,
        &actor_source,
        parsed.ast.as_ref(),
        &parsed.diagnostics,
        &domains.domain_units,
        &designs,
    );
    finish_surface(domains, model, diagnostics, ActorCompilation::assemble)
}
/// Compiles an `.actor` file against its imported domain models in one call.
///
/// `inputs` is the one [`CompileInputs`] bundle the host builds once from
/// the gathered import closure; this surface consumes its `domains` and
/// `imports` buckets plus, opt-in, its `designs` bucket (see
/// [`CompileInputs::designs`]).
///
/// Each domain is a `(path, source)` pair; an actor-file import resolves by
/// exact path match. Extra pairs no import names are ignored.
///
/// ```
/// let domains = [(
///     "support.mox".to_string(),
///     "package support\n\nclass Ticket { String title }".to_string(),
/// )];
/// let source = "import \"support.mox\"\n\nactors Ops {\n    actor Agent\n}";
/// let compilation = phero_driver::compile_actors_str(
///     "ops.actor",
///     source,
///     &phero_driver::CompileInputs::from_domains(&domains),
/// );
/// assert!(compilation.diagnostics.is_empty());
/// assert_eq!(compilation.model.unwrap().blocks.len(), 1);
/// ```
///
/// # Validation rules
///
/// The provided designs join the compile two ways (both opt-in — an empty
/// `designs` bucket keeps the domain-only behavior exactly, and every check
/// stays silent unless every imported domain lowered):
///
/// * **grant coverage** (pherolang#128's J4): every capability a grant — or
///   a delegation — hands out must be carried by at least one operation or
///   search across the provided designs, else the grant entry's diagnostic
///   names the dangling capability with the span of its reference;
/// * **noun classification** (pherolang#144): each capability's resolved
///   class joins the aggregate machinery, and a class some design marks a
///   contained, repository-holding entity warns toward its aggregate root
///   (codegraph#596 hangs non-root bindings); value and root verdicts are
///   clean, no verdict stays silent, and the warning never blocks the
///   artifact.
///
/// Each design is a pre-compiled [`phero_ir::ddd::DddModel`]; capability
/// names pool across all of them.
pub fn compile_actors_str(
    actor_path: &str,
    actor_source: &str,
    inputs: &CompileInputs,
) -> ActorCompilation {
    let db = Database::new();
    let actor = SourceFile::new(&db, actor_path.to_string(), actor_source.to_string());
    let files = domain_files(&db, inputs.domains());
    compile_actors_with(&db, actor, files, inputs)
}
