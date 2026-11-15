//! The `.ddd` entry points: design compilation against the domain union,
//! with optional capability validation (actor models) and decision-link
//! validation (decision models).

use salsa::Database as Db;

use crate::compilation::DddCompilation;
use crate::ddd;
use crate::diagnostic::Diagnostic;
use crate::entry::finish_surface;
use crate::imports::DomainImports;
use crate::session::{parse_ddd_query, Database, SourceFile};
use crate::union::{domain_files, lower_domains, lower_domains_from, DomainOrigin};

/// One imported `.ddd` design fragment (DD8, issue pherolang#305): the
/// host-resolved path and its source text. Fragments are merged into the
/// root application at compile time; they never emit their own artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DddFragment {
    /// Host-resolved path (the diagnostic tag and import identity).
    pub path: String,
    /// The fragment's source text.
    pub source: String,
}

/// Compiles a `.ddd` design file against its imported domain models to the
/// standalone [`phero_ir::ddd::DddModel`] (memoized by salsa): parse the
/// design, lower every named domain through the ordinary domain pipeline
/// (the same union-namespace rule [`compile_actors_str`] uses), then lower
/// and validate the design against the resolved domain model.
///
/// See [`compile_ddd_str`] for the normative contract.
#[salsa::tracked(returns(clone))]
pub fn compile_ddd(db: &dyn Db, design: SourceFile, domains: Vec<SourceFile>) -> DddCompilation {
    compile_ddd_with(db, design, domains, None, &DomainImports::default(), &[])
}

/// The body of [`compile_ddd`], parameterized over the actor models the
/// `_with_actors` entry point validates capabilities against, the decision
/// models the `_with_decisions` entry point validates decision links
/// against, and the provided import content for the domain files (see
/// [`compile_file`] for the import shape). Both buckets are
/// empty-by-default: tracked surfaces carry no cross-surface artifacts.
fn compile_ddd_with(
    db: &dyn Db,
    design: SourceFile,
    domains: Vec<SourceFile>,
    actors: Option<&[phero_ir::ActorModel]>,
    imports: &DomainImports,
    decisions: &[(String, phero_ir::dmn::DmnModel)],
) -> DddCompilation {
    let design_path = design.path(db);
    let parsed = parse_ddd_query(db, design);
    let domains = lower_domains(
        db,
        &design_path,
        parsed
            .ast
            .as_ref()
            .map(|ast| ast.imports.as_slice())
            .unwrap_or_default(),
        &domains,
        imports,
    );

    let (model, diagnostics) = ddd::compile_ddd_file(
        &design_path,
        &design.text(db),
        parsed.ast.as_ref(),
        &parsed.diagnostics,
        &domains.domain_units,
        &domains.provided_imports,
        actors,
        decisions,
    );
    finish_surface(domains, model, diagnostics, DddCompilation::assemble)
}
/// Compiles a `.ddd` design file against its imported domain models in one
/// call.
///
/// Each domain is a `(path, source)` pair; a design import resolves by
/// exact path match with a lexical resolution relative to the design file's
/// directory as the fallback (the same rule
/// [`compile_actors_str`](crate::compile_actors_str) applies). Extra pairs
/// no import names are ignored; an import no provided pair names is the
/// error `imported file "<path>" was not provided`. The result carries
/// diagnostics from every file involved, each tagged with its path — the
/// same shape [`compile_actors_str`](crate::compile_actors_str) returns, so
/// a host renders both with ariadne identically.
///
/// ```
/// let domains = [(
///     "library.mox".to_string(),
///     "package nz.example.library\n\nclass Book { String title }".to_string(),
/// )];
/// let source = concat!(
///     "import \"library.mox\"\n",
///     "\n",
///     "application Library {\n",
///     "    base nz.example.library\n",
///     "\n",
///     "    module catalogue {\n",
///     "        entity Book repository BookRepository {\n",
///     "            findById;\n",
///     "        }\n",
///     "    }\n",
///     "}\n",
/// );
/// let compilation = phero_driver::compile_ddd_str(
///     "library.ddd",
///     source,
///     &domains,
///     &phero_driver::DomainImports::default(),
/// );
/// assert!(compilation.diagnostics.is_empty());
/// assert_eq!(compilation.model.unwrap().modules.len(), 1);
/// ```
///
/// # Normative contract
///
/// ## Entry points
///
/// - [`compile_ddd_str`] — parse, domain lowering, design validation; the
///   capabilities declared on operations are recorded as authored and are
///   **not** validated.
/// - [`compile_ddd_str_with_actors`] — the same, plus capability
///   validation against a caller-provided [`phero_ir::ActorModel`].
/// - [`compile_ddd_str_with_decisions`] — the same, plus decision-link
///   validation (rule 17) against caller-provided
///   [`phero_ir::dmn::DmnModel`]s: every operation `invokes` name and every
///   `inject decision` name must resolve to a decision service the
///   provided models declare.
/// - [`compile_ddd`] / [`parse_ddd_query`] — the salsa-tracked queries the
///   one-shot wrappers run on a fresh [`Database`].
///
/// ## Resolution semantics
///
/// Names resolve with the `.mox` cross-package rules against the union
/// namespace of the imported domains (the same rule multi-file `.mox`
/// compiles follow). A bare single-segment name must be unique across all
/// domain packages; when several packages declare it the error is
/// ``ambiguous type `X`; qualify as `p.X``` with the matching packages as
/// help. A qualified `pkg.Name` must match exactly one package. When the
/// design declares `base`, that package becomes the preferred resolution
/// scope: a bare name the base declares resolves there before the
/// unique-across-packages lookup runs. `base` itself must name an existing
/// domain package.
///
/// Resolution is **additive, never rewriting**: the artifact keeps every
/// name exactly as authored (unqualified where written unqualified,
/// qualified where written qualified), and every reference a consumer acts
/// on also carries its resolved companion (DD1, pherolang#298) —
/// `Design::type_ref`, `Delegation::target_ref`, `Service::dependency_refs`
/// / `decision_dependency_refs`, `ServiceOperation::capability_refs` /
/// `emitted_refs` / `decision_refs`, `SearchDef::entity_ref` /
/// `capability_refs`, and each search member's `type_ref`. Consumers act on
/// the companion and never re-implement resolution; the authored spelling
/// stays for round-tripping and diagnostics.
///
/// ## Validation rules
///
/// 1. **base** — an unknown base package is an error naming it.
/// 2. **Uniqueness** (application-wide unless noted) — module names;
///    service names; design class references (one design per class per
///    application, keyed by the resolved class; a projection/standalone
///    DTO introduces its own boundary name instead — rule 21); repository
///    names; operation names within one service; operation names within one
///    repository.
/// 3. **Stereotype targets** — every design's `class` must resolve to a
///    class in the domain model (entity and value designs, a mirror DTO,
///    and a projection DTO's `from` source all target classes; a
///    standalone DTO has no source class at all — rule 21).
/// 4. **Flag/stereotype compatibility** — `scaffold`, `auditable`, and
///    `optimisticLocking` are entity-only; `nonPersistent` is
///    value/dto-only; `cache` is legal on any design. A `!`-negated flag is
///    only meaningful where the flag defaults on: `!auditable` and
///    `!optimisticLocking` are the entity opt-outs, while `!scaffold`,
///    `!nonPersistent`, and `!cache` negate flags that default off and are
///    rejected wherever written.
/// 5. **Repository placement** — only entity designs may declare a
///    repository.
/// 6. **Aggregate boundary** — see the derivation rule below; an entity
///    design derived as a non-root may not declare a repository.
/// 7. **Signature type-check** — declared service and repository operation
///    signatures type-check against the domain model: the
///    `phero-syntax` primitives resolve everywhere, every other reference
///    resolves through the bare/qualified rules above (any declared kind —
///    class, enum, datatype, interface, vocabulary). A multiplicity
///    annotation on a return type or parameter lowers into the
///    artifact's cardinality slots (`returnMultiplicity` / parameter
///    `multiplicity`).
/// 8. **Delegations** — the target must resolve to a service, a repository,
///    or a search declared anywhere in the application, and the operation
///    must name an operation on it (repository built-ins included; a search
///    exposes only the virtual operation `search`). Service → repository
///    and service → search delegation across different modules is the
///    module-coupling error ("interaction between a Service in one Module
///    and a Repository/Search in another Module is not allowed; go via a
///    Service"); service → service across modules is allowed.
/// 9. **inject** — every dependency name must resolve to a service, a
///    repository, or a search of the application.
/// 10. **Capabilities** — only [`compile_ddd_str_with_actors`] validates:
///     each declared capability name must exist in the union of the actor
///     model's blocks' capabilities. Plain compiles record the names
///     unvalidated — the design dimension is loosely coupled to the policy
///     dimension by design.
/// 11. **Searches** — a search's name is unique application-wide. Its
///     `entity` line is required, resolves like a design target, and must
///     bind to an entity design **of the same module** (the repository
///     coupling rule; non-root entities may be searched — a projection over
///     a contained entity is legitimate). Text fields must name a direct
///     feature of the entity that is text-like (the string primitives,
///     enums, datatypes — the string-family default the constraint rules
///     use — and vocabularies whose key facet is a string primitive), with
///     a boost of at least 1 and a non-empty analyzer; filters and sorts
///     must name direct features that are not class references; document
///     field names are unique per search and every document expression is
///     parsed and type-checked with the entity as `self` (any value type is
///     legal — the consumer decides the rendered form). Pagination bounds
///     must satisfy `1 ≤ limit ≤ max`.
/// 12. **Value contract** — a `value` design is an immutable value object
///     (Sculptor's ValueObject default, with no opt-out): every stored
///     feature of the resolved class (own and inherited, derived features
///     excluded) must carry the domain model's `readonly` modifier. A
///     changeable class designs as `entity` or `dto` instead. Rule 20 adds
///     the strict value-semantics rules (R-V1–R-V3) the target class must
///     also satisfy when the application designs the other end.
/// 13. **Aggregate reference constraint** — an outside class may hold a
///     cross reference only to an aggregate's root: for every
///     cross-reference (`refers`) feature whose target sits under a
///     containment chain (a non-root aggregate member), the referring
///     class must share the target's containment root. Computed over the
///     whole union like the aggregate-derivation rule, and skipped when
///     the verdict would be a guess (the target or source sits under two
///     distinct roots, or under a containment cycle).
/// 14. **scaffold** — a scaffolded entity design must declare a repository:
///     `scaffold` stands for Sculptor's generated CRUD operations (which
///     also imply the service), and the artifact stays validation-only —
///     nothing is synthesized — so the author declares the repository the
///     scaffolding would hang off. The service remains the author's to
///     declare.
/// 15. **dto-with-ops error** — a `dto` design whose class carries `ops`
///     errors once per op, naming the design and the op (`E0365`, DD3): a
///     dto is a behavior-free transfer record. A `derived` feature stays
///     legal — it is a computed wire field, not behavior. The rule covers
///     the mirror form (whose boundary *is* the class's shape); a
///     projection reads a source class that may legitimately carry behavior.
/// 16. **Typed status fields** — an entity design whose class carries a
///     feature named `status` or `state` typed as a bare `String` is an
///     error: a status-like field is a lifecycle, and the bare-`String`
///     shape is the silent no-machine case. Declare an enum in the domain
///     model and a `machine '<field>: <Enum>'` on the design. Value objects
///     and dtos are exempt (the check is entity semantics, the machine
///     rule's scope), and enum-typed fields stay legal — an enum without a
///     machine is a legitimate modeling choice.
/// 17. **Decision links** — only [`compile_ddd_str_with_decisions`]
///     validates: every decision-service reference (an operation's
///     `invokes` name, a service's `inject decision` name) must resolve by
///     its **last segment** to a decision service declared in the union of
///     the provided decision models' services (E0250/E0251, the
///     `phero_diagnostics::dmn` codes — the code prefixes the message text,
///     the `driver.integrity` band precedent). Plain compiles record the
///     names unvalidated — the design dimension is loosely coupled to the
///     decision dimension by design. Every `.dmn` import the design
///     declares must be provided under its path (exact match, or the
///     lexical resolution relative to the design file's directory) when a
///     non-empty decisions bucket is supplied, else
///     `imported decision model "<path>" was not provided`; an empty
///     bucket is the plain compile's behavior — `.dmn` imports are not
///     domains and lower nothing.
/// 18. **Transition bindings** (E0350–E0352, the `phero_diagnostics::ddd`
///     codes, issue pherolang#176) — a service operation's `transition`
///     clause is legal only on a delegation to a repository `save` of the
///     same module (a declared operation, or a delegation rule 8 already
///     rejects, is refused or silently skipped — no cascade). The machine
///     is the one bound to the target repository's designed class; the
///     trigger must be one that machine declares — an explicit `via` name
///     or a bare arrow's derived name (the lowerCamel of the target
///     state), never a name that is both explicit and derived (ambiguous,
///     so it binds nothing). A module carrying more than one machine
///     -bearing design requires the qualifier, spelled with the machine's
///     **enum type name** (`transition PayRunStatus.Reverse` — machine
///     declarations are field-named, so a machine-name qualifier does not
///     exist as authored). The binding lowers as authored (`TransitionRef`
///     on the operation), and a binding that resolves pins the machine's
///     class as the operation's own-record condition scope — rule 19's
///     joint shape. Public service operations lower to capability-gated
///     routes downstream (issue pherolang#143, aligned with codegraph#590):
///     the binding and the resolved condition paths are the lowering
///     contract consumers build on, and until an in-tree `.ddd` lowering
///     exists compiles add no "lowers to nothing" diagnostics —
///     `protected` carriers stay legal-but-dead under the existing
///     warning-severity coverage lint.
/// 19. **Condition scoping** (E0353/E0354, issue pherolang#180) — a service
///     operation's `when` guard is scoped by the module's machine count:
///     one machine-bearing design scopes the guard on that class (the
///     machine-bound rule); **no** machine-bearing design refuses an
///     op-level guard outright (unless a transition binding pins the
///     scope); **several** admit a guard whose every referenced name
///     resolves uniquely — machine states bind when exactly one machine
///     declares the name (else the `E0354` ambiguous-machine-scope error,
///     qualified as `<EnumType>.<State>`), each machine's bound field
///     binds bare when its name is unique and always under the
///     enum-derived spelling (lowerCamel of the machine's enum type name),
///     and a qualified path (`TimecardRepository.status`) resolves against
///     the service's injected repositories — first segment the injection
///     name exactly as declared, remaining segments fields of the
///     repository's designed class (`E0353` otherwise; a resolved path
///     that mistypes is the ordinary same-type error). The resolution
///     lowers as `conditionPaths` on the operation — injected paths
///     always, own-record paths only in a multi-machine module (where the
///     owning machine is not derivable from the artifact alone); the
///     `condition` source string stays verbatim.
/// 20. **Value semantics** (E0361–E0364, issue pherolang#299) — a `value`
///     design means *strict* value semantics with no opt-out, so the
///     compiler lowers the fixed `semantics` block (identity `none`,
///     `immutable`, `deepImmutable`, structural equality) onto every value
///     design — and only values — and refuses the shapes that contradict
///     it. The rules read the **application's designs**, not the `.mox`
///     file alone (one class may be a value in one application and an
///     entity in another), and they are deferred until every module
///     lowered, so a value design may precede the entity design it is
///     judged against:
///     **R-V1** — every `id` feature of the value's class (own or
///     inherited) is an error (`E0361`), naming the design and the feature;
///     **R-V2** — a `contains` feature whose class target is an `entity`
///     design of the same application is an error (`E0362`): a value owns
///     no mutable state, so it contains values only;
///     **R-V3** — a `refers` feature whose class target is a designed
///     entity of the same application is an error (`E0363`): refer to the
///     entity by id (`refers String personId`) instead;
///     **`abstract value`** is an error (`E0364`) — a value object is
///     concrete. Every diagnostic spans the value design's class name;
///     feature-level rules run only against a fully lowered union (the
///     rule-6 deferral discipline).
/// 21. **DTO boundaries** (DD3, issue pherolang#300; `E0365`–`E0372`) —
///     a `dto` design is a behavior-free transport shape with a lowered
///     `dtoShape`:
///     a **mirror** (`dto User`) copies every stored feature of its class
///     (derived features excluded) as a `select` in declaration order;
///     a **projection** (`dto BookSummary from Book { … }`) selects
///     inherited features too, lowers a plain feature path
///     (`authorName = author.fullName`) as a `select` carrying the path,
///     and lowers any other assignment as a typed `derive` carrying the
///     verbatim source text; a **standalone** DTO
///     (`dto CreateBookRequest { … }`) has **no** source class — its
///     `class` and `typeRef` are absent and its declared members resolve
///     against the domain union or another DTO boundary. Member output
///     names are unique (`E0366`), projection features/paths must resolve
///     (`E0367`), expressions and declared types must type-check
///     (`E0368`), and a member may not embed a class designed as an
///     `entity` (`E0369`; project its fields instead). The direction is
///     derived — any `derive` member makes the boundary `output`, else
///     `input` — and an output-only DTO cannot fill a signature's
///     parameter slot (`E0370`, naming the derived members). DTO names are
///     application-wide unique and must not shadow a domain type
///     (`E0371`; a mirror's name is its source class and is exempt). A DTO
///     carries no repository, machine, or entity-only flag (`E0372`; rule
///     5's repository error and rule 4's flag errors carry the same code
///     on a DTO). The pre-pass runs before services lower, so a signature
///     in an earlier module may name a DTO declared later.
/// 22. **Error vocabulary and `throws`** (DD7, issue pherolang#304;
///     `E0373`–`E0378`) — `error <Name>(<payload>);` declarations lead their
///     enclosing body (`base? error* module*` at application scope, `error*
///     module_member*` at module scope) and lower to the root/module
///     `errors` lists with typed payloads. Error names are application-wide
///     unique (`E0374`), never shadow the implicit prelude
///     (`NotFound(resource, key)`, `Conflict(detail)` — `E0374`), carry
///     unique payload field names (`E0378`), and a payload field may not
///     embed a class designed as an `entity` (`E0376`, deferred until the
///     designs are known — an error payload is serializable data).
///     Application-scoped errors are visible everywhere; a module-scoped
///     error is private (`E0375`, naming the owning module and the
///     application-scope fix). A declared, delegated, or body service
///     operation and a declared repository operation may carry `throws`
///     (service order: after `transition`, before `when`; repository order:
///     after the signature); each bare name resolves against the prelude,
///     the application declarations, and the operation's module (`E0373`;
///     a foreign module's private error is `E0375`) and lowers to a
///     resolved `ErrorRef { name, scope, module? }`. `throws` is explicit
///     and complete — the body's raises must be a subset (an undeclared
///     raise is `E0360` naming the declared set; a declared-but-unraised
///     entry stays legal, never inferred), and a delegation's `throws` must
///     be a **superset** of its target's (`E0377`, naming each missing
///     error). Built-in repository operations take no `throws` (the parser
///     refuses it). The body checker (`phero_expr::stmt::check_stmt_body`)
///     receives the module's visible raise signatures and the operation's
///     declared set; the statement subset stays orchestration-only (see
///     `docs/DDD.md`).
/// ## Aggregate derivation
///
/// The aggregate boundary derives from the domain model's containment
/// graph, standing in for Sculptor's `belongsTo`/`!aggregateRoot` markers:
/// a class B is *contained* when some class A owns a containment feature
/// (`contains`) whose type resolves to B, transitively. An entity design
/// whose class is contained by another stereotyped entity's class is a
/// non-root — it may not declare a repository. The closure is computed
/// only when every imported domain lowered (a failed domain may hide
/// containments, and a wrong non-root verdict is worse than none).
pub fn compile_ddd_str(
    path: &str,
    source: &str,
    domains: &[(String, String)],
    imports: &DomainImports,
) -> DddCompilation {
    compile_ddd_str_inner(path, source, domains, None, imports, &[])
}

/// Like [`compile_ddd_str`], but the capabilities declared on service
/// operations are validated against `actors`: every name must exist in the
/// union of the actor models' blocks' capabilities, else the operation's
/// error names the operation, its service, and the unknown capability
/// (rule 10) — with the
/// [`did_you_mean_adaptive`](phero_diagnostics::did_you_mean_adaptive) closest
/// declared name when one is a plausible typo, and the `unknownCapability`
/// code carrying it for quick fixes. Capabilities pool across every
/// provided model, so a host passes all of the application's `.actor`
/// artifacts. The plain [`compile_ddd_str`] records capabilities
/// unvalidated.
pub fn compile_ddd_str_with_actors(
    path: &str,
    source: &str,
    domains: &[(String, String)],
    imports: &DomainImports,
    actors: &[phero_ir::ActorModel],
) -> DddCompilation {
    compile_ddd_str_inner(path, source, domains, Some(actors), imports, &[])
}

/// Like [`compile_ddd_str`], but the design's **decision links** are
/// validated against `decisions` (rule 17): every operation `invokes`
/// name and every `inject decision` name must resolve by its last segment
/// to a decision service declared in the union of the provided
/// [`phero_ir::dmn::DmnModel`]s' services, else the reference errors
/// `E0250 unknown decision service '<name>' invoked by operation '<op>'
/// of service '<service>'` (or the `inject decision` variant, E0251),
/// spanned on the reference, with the
/// [`did_you_mean_adaptive`](phero_diagnostics::did_you_mean_adaptive)
/// closest declared service when one is a plausible typo. Decision
/// services pool across every provided model, so a host passes all of the
/// application's `.dmn` artifacts as `(path, model)` pairs. An **empty**
/// slice is the plain [`compile_ddd_str`] behavior: the names lower into
/// the artifact unvalidated. Every `import "<path>.dmn"` the design
/// declares must be provided under that path when a non-empty bucket is
/// supplied, else `imported decision model "<path>" was not provided` —
/// `.dmn` imports are never domains and never lower as `.mox` sources
/// (a plain compile ignores them outright).
pub fn compile_ddd_str_with_decisions(
    path: &str,
    source: &str,
    domains: &[(String, String)],
    imports: &DomainImports,
    actors: &[phero_ir::ActorModel],
    decisions: &[(String, phero_ir::dmn::DmnModel)],
) -> DddCompilation {
    compile_ddd_str_inner(path, source, domains, Some(actors), imports, decisions)
}

/// The body of the one-shot `.ddd` entry points.
fn compile_ddd_str_inner(
    path: &str,
    source: &str,
    domains: &[(String, String)],
    actors: Option<&[phero_ir::ActorModel]>,
    imports: &DomainImports,
    decisions: &[(String, phero_ir::dmn::DmnModel)],
) -> DddCompilation {
    let db = Database::new();
    let design = SourceFile::new(&db, path.to_string(), source.to_string());
    let files = domain_files(&db, domains);
    compile_ddd_with(&db, design, files, actors, imports, decisions)
}

/// Compiles a `.ddd` **root** with its imported fragments (DD8, issue
/// pherolang#305): the fragments' top-level modules merge into the root
/// application in import order, and the result is one [`DddCompilation`]
/// and one artifact. A source without an `application` declaration is a
/// fragment and is refused here — compile its root instead.
pub fn compile_ddd_app_str(
    path: &str,
    source: &str,
    fragments: &[DddFragment],
    domains: &[(String, String)],
    imports: &DomainImports,
) -> DddCompilation {
    compile_ddd_app_str_inner(path, source, fragments, domains, None, imports, &[])
}

/// [`compile_ddd_app_str`] with capability and decision-link validation.
pub fn compile_ddd_app_str_with_decisions(
    path: &str,
    source: &str,
    fragments: &[DddFragment],
    domains: &[(String, String)],
    imports: &DomainImports,
    actors: &[phero_ir::ActorModel],
    decisions: &[(String, phero_ir::dmn::DmnModel)],
) -> DddCompilation {
    compile_ddd_app_str_inner(
        path,
        source,
        fragments,
        domains,
        Some(actors),
        imports,
        decisions,
    )
}

fn compile_ddd_app_str_inner(
    path: &str,
    source: &str,
    fragments: &[DddFragment],
    domains: &[(String, String)],
    actors: Option<&[phero_ir::ActorModel]>,
    imports: &DomainImports,
    decisions: &[(String, phero_ir::dmn::DmnModel)],
) -> DddCompilation {
    let root = phero_syntax::parse_ddd(source);
    let mut diagnostics: Vec<(String, Diagnostic)> = root
        .errors
        .iter()
        .map(|error| {
            (
                path.to_string(),
                Diagnostic::error(error.message.clone(), Some(error.span)),
            )
        })
        .collect();
    let Some(mut ast) = root.ast else {
        return DddCompilation::assemble(None, None, diagnostics);
    };
    let Some(mut application) = ast.application.take() else {
        diagnostics.push((
            path.to_string(),
            Diagnostic::error(
                "this file is a design fragment (no `application` declaration); compile \
                 the root that imports it",
                None,
            ),
        ));
        return DddCompilation::assemble(None, None, diagnostics);
    };

    // The combined source keeps root spans at their authored offsets and
    // pads each fragment before parsing, so every AST span is already a
    // combined-source span. Ranges map diagnostics back to their file.
    let mut combined = source.to_string();
    let mut ranges: Vec<(String, usize, usize)> = vec![(path.to_string(), 0, source.len())];
    let mut fragment_modules = Vec::new();
    // The root's own import list, kept separate from the merged one: an
    // import resolves relative to the file that declares it (DD8).
    let root_imports = ast.imports.clone();
    let mut fragment_origins: Vec<(String, Vec<phero_syntax::Import>)> = Vec::new();
    let mut merged_imports = ast.imports.clone();
    for fragment in fragments {
        let base = combined.len();
        let padded = format!("{}{}", " ".repeat(base), fragment.source);
        let parsed = phero_syntax::parse_ddd(&padded);
        for error in &parsed.errors {
            let local =
                (error.span.start.saturating_sub(base)..error.span.end.saturating_sub(base)).into();
            diagnostics.push((
                fragment.path.clone(),
                Diagnostic::error(error.message.clone(), Some(local)),
            ));
        }
        if let Some(fragment_ast) = parsed.ast {
            if fragment_ast.application.is_some() {
                diagnostics.push((
                    fragment.path.clone(),
                    Diagnostic::error(
                        "a design application root cannot be imported as a fragment; \
                         remove its `application` declaration or import its root instead",
                        None,
                    ),
                ));
            } else {
                fragment_origins.push((fragment.path.clone(), fragment_ast.imports.clone()));
                fragment_modules.extend(fragment_ast.fragments);
                merged_imports.extend(fragment_ast.imports);
            }
        }
        combined.push_str(&fragment.source);
        ranges.push((fragment.path.clone(), base, base + fragment.source.len()));
    }
    application.modules.extend(fragment_modules);
    application.modules = merge_modules(application.modules);
    ast.imports = merged_imports;
    ast.application = Some(application);
    ast.fragments.clear();

    let db = Database::new();
    let files = domain_files(&db, domains);
    let mut origins = vec![DomainOrigin {
        path,
        imports: &root_imports,
    }];
    for (fragment_path, fragment_imports) in &fragment_origins {
        origins.push(DomainOrigin {
            path: fragment_path,
            imports: fragment_imports,
        });
    }
    let lowered = lower_domains_from(&db, &origins, &files, imports);
    let (model, local_diagnostics) = ddd::compile_ddd_file(
        path,
        &combined,
        Some(&ast),
        &[],
        &lowered.domain_units,
        &lowered.provided_imports,
        actors,
        decisions,
    );
    diagnostics.extend(local_diagnostics);
    let diagnostics = remap_diagnostics(diagnostics, path, &ranges);
    let model = if diagnostics
        .iter()
        .any(|(_, diagnostic)| diagnostic.is_error())
    {
        None
    } else {
        model
    };
    finish_surface(lowered, model, diagnostics, DddCompilation::assemble)
}

/// Reopens same-named modules across fragments: members concatenate in
/// first-appearance order, and the artifact carries one module per name.
fn merge_modules(modules: Vec<phero_syntax::ast::DddModule>) -> Vec<phero_syntax::ast::DddModule> {
    let mut merged: Vec<phero_syntax::ast::DddModule> = Vec::new();
    for module in modules {
        if let Some(existing) = merged
            .iter_mut()
            .find(|existing| existing.name.text == module.name.text)
        {
            existing.errors.extend(module.errors);
            existing.services.extend(module.services);
            existing.designs.extend(module.designs);
            existing.searches.extend(module.searches);
            existing.extends.extend(module.extends);
        } else {
            merged.push(module);
        }
    }
    merged
}

/// Re-tags the combined-source diagnostics with their originating file and
/// rebases their spans to that file's local coordinates.
fn remap_diagnostics(
    diagnostics: Vec<(String, Diagnostic)>,
    root_path: &str,
    ranges: &[(String, usize, usize)],
) -> Vec<(String, Diagnostic)> {
    diagnostics
        .into_iter()
        .map(|(file, mut diagnostic)| {
            if file != root_path {
                return (file, diagnostic);
            }
            let Some(span) = diagnostic.span else {
                return (file, diagnostic);
            };
            for (path, start, end) in ranges {
                if span.start >= *start && span.end <= *end {
                    diagnostic.span = Some((span.start - start..span.end - start).into());
                    return (path.clone(), diagnostic);
                }
            }
            (file, diagnostic)
        })
        .collect()
}
