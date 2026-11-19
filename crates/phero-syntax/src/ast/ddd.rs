//! The `.ddd` design DSL AST.

use super::*;

// --- .ddd design DSL ---------------------------------------------------------

/// The root node of a parsed `.ddd` source. A **root** file declares imports
/// followed by one `application` declaration; a **fragment** declares imports
/// followed by top-level `module` blocks and no application (DD8, issue
/// pherolang#305). The two forms are mutually exclusive. An `import` after
/// the application or after a top-level module is a syntax error and is
/// dropped from the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct DddFile {
    /// The `import` declarations in source order.
    pub imports: Vec<Import>,
    /// The `application` declaration of a root file, when one was parsed.
    pub application: Option<DddApplication>,
    /// The top-level module blocks of a **fragment** file, in source order.
    /// Empty for a root file.
    pub fragments: Vec<DddModule>,
}

/// The `application <name> { ... }` declaration of a `.ddd` file: the
/// designed application with its optional `base` package and its modules.
#[derive(Debug, Clone, PartialEq)]
pub struct DddApplication {
    /// The application name.
    pub name: Name,
    /// The `base` package declaration, when present. The grammar pins it to
    /// the first member position; a misplaced or repeated `base` is a syntax
    /// error reported by the parser.
    pub base: Option<DddBase>,
    /// Application-scoped `error` declarations (DD7, issue pherolang#304), in
    /// source order. The grammar pins them between `base` and the modules.
    pub errors: Vec<DddErrorDecl>,
    /// The modules in source order.
    pub modules: Vec<DddModule>,
    /// Span of the whole declaration, `application` keyword included.
    pub span: Span,
}

/// The `base <qualified-name>` member of a [`DddApplication`]: the default
/// domain package for unqualified references.
#[derive(Debug, Clone, PartialEq)]
pub struct DddBase {
    /// The declared package name.
    pub package: QualifiedName,
    /// Span of the whole declaration, `base` keyword included.
    pub span: Span,
}

/// A `module <name> { ... }` of a [`DddApplication`]: a cohesive slice of
/// application services, designed classes, and search projections.
#[derive(Debug, Clone, PartialEq)]
pub struct DddModule {
    /// The module name.
    pub name: Name,
    /// Module-scoped `error` declarations (DD7, issue pherolang#304), in
    /// source order. Module errors are private: visible only inside this
    /// module. The grammar pins them to the leading member position.
    pub errors: Vec<DddErrorDecl>,
    /// Application services in source order.
    pub services: Vec<DddService>,
    /// Class designs in source order.
    pub designs: Vec<DddDesign>,
    /// Search projections in source order.
    pub searches: Vec<DddSearch>,
    /// Module-level `extend <Class> { ... }` child-operation blocks in
    /// source order (issue pherolang#179). Several blocks may target one
    /// class; the driver merges them onto that class's design.
    pub extends: Vec<DddExtend>,
    /// Span of the whole declaration, `module` keyword included.
    pub span: Span,
}

/// A `service <name> { ... }` of a [`DddModule`]: a stateless application
/// service. The interleaved order of operations and `inject` lines is not
/// preserved; operations and dependencies are collected in their own source
/// orders (the same split the wire artifact makes).
#[derive(Debug, Clone, PartialEq)]
pub struct DddService {
    /// The service name.
    pub name: Name,
    /// Description from the doc comment directly above the declaration
    /// (contiguous `///` lines, joined).
    pub doc: Option<String>,
    /// Declared and delegated operations, in source order.
    pub operations: Vec<DddServiceOp>,
    /// Injected dependency names (`inject <name>;`), in source order.
    pub dependencies: Vec<Name>,
    /// Injected decision-service references (`inject decision
    /// <qualified-name> [version <pin>];`), in source order — the `.dmn`
    /// dimension, loosely coupled to the decision surface.
    pub decision_dependencies: Vec<DddDecisionRef>,
    /// Span of the whole declaration, `service` keyword included.
    pub span: Span,
}

/// One operation of a [`DddService`]: either a declared signature or a
/// delegation to an injected dependency's operation.
#[derive(Debug, Clone, PartialEq)]
pub struct DddServiceOp {
    /// The operation name. Contextual keywords are legal here: a delegated
    /// op may be named `save`.
    pub name: Name,
    /// The declared return type; `None` for delegation operations.
    pub return_type: Option<TypeRef>,
    /// Multiplicity annotation on the return type, if any.
    pub multiplicity: Option<Multiplicity>,
    /// Parameters in source order.
    pub params: Vec<DddParam>,
    /// The delegation target, when this operation forwards instead of
    /// declaring a signature.
    pub delegation: Option<DddDelegation>,
    /// `true` for the `protected` modifier: the operation stays off the
    /// public interface (Sculptor's visibility).
    pub is_protected: bool,
    /// Actor capability names from the `capability` clause, in source order.
    pub capabilities: Vec<Name>,
    /// Decision-service references from the `invokes` clause, in source
    /// order — the decision services the operation invokes (the `.dmn`
    /// dimension, loosely coupled to the decision surface), each with its
    /// optional `version` pin.
    pub invokes: Vec<DddDecisionRef>,
    /// Event names from the `emits` clause, in source order — the events
    /// the operation produces (loosely coupled to the `.evt` layer).
    pub emits: Vec<Name>,
    /// The `transition <Trigger>` binding (issue pherolang#176): the
    /// machine-bound transition a delegated operation fires — legal only
    /// on a delegation to a repository `save`, which the driver validates.
    pub transition: Option<DddTransitionRef>,
    /// Error names from the `throws` clause (DD7, issue pherolang#304), in
    /// source order: the declared error vocabulary the operation can raise.
    /// Names are bare; the driver resolves them against the application's
    /// error declarations and the standard prelude.
    pub throws: Vec<Name>,
    /// The `when ( ... )` guard condition: a `raw_parens` span (parens
    /// included, expression inside deliberately unparsed — slicing and
    /// type-checking it is the driver's job).
    pub condition: Option<Span>,
    /// The executable statement body, when the operation is written as a
    /// body form (issue pherolang#178): `Type? name(params) clauses? =>
    /// <Repository> { ... }`.
    pub body: Option<DddBody>,
    /// The `=> <Repository>` receiver bound as `self` in a service body
    /// (issue pherolang#178). `None` for declared/delegated operations and for
    /// repository bodies (which bind their own design's repository).
    pub body_receiver: Option<Name>,
    /// Span of the whole operation, `;` included (or the closing body `}`).
    pub span: Span,
}

/// The executable statement body of a `.ddd` service or repository operation
/// (issue pherolang#178). The subset is closed: `let`, `let … else error …`,
/// assignment, `guard … else error …`, `if`/`else`, and `return`. Every
/// expression slot is a raw span parsed and typed by the driver against the
/// operation's scope.
#[derive(Debug, Clone, PartialEq)]
pub struct DddBody {
    /// The statements in execution order.
    pub stmts: Vec<DddStmt>,
}

/// One statement of a [`DddBody`] (issue pherolang#178).
#[derive(Debug, Clone, PartialEq)]
pub enum DddStmt {
    /// `let <name> = <expr>;` or `let <name> = <expr> else error
    /// <Variant>(<args>);`.
    Let {
        /// The bound name.
        name: Name,
        /// The initializer's raw expression span.
        expr: Span,
        /// The `else error <Variant>(...)` raise, when written.
        raise: Option<DddRaise>,
    },
    /// `<path> = <expr>;` — an assignment to a local root's feature path.
    Assign {
        /// The target path; the first segment is a local binding.
        path: Vec<Name>,
        /// The assigned expression's raw span.
        expr: Span,
    },
    /// `guard <expr> else error <Variant>(<args>);`.
    Guard {
        /// The guard condition's raw span.
        expr: Span,
        /// The raise taken when the condition is false.
        raise: DddRaise,
    },
    /// `if <cond> { <statements> } else { <statements> }`.
    If {
        /// The condition's raw span.
        cond: Span,
        /// The then-branch statements.
        then: Vec<DddStmt>,
        /// The else-branch statements (empty when the source omitted it).
        else_: Vec<DddStmt>,
    },
    /// `return <expr>?;`.
    Return {
        /// The returned expression's raw span; `None` for a void return.
        expr: Option<Span>,
    },
}

/// One `error <Variant>(<args>)` raise of a [`DddBody`] (DD7): the error name
/// as written plus the raw argument spans. Resolution against the declared
/// error vocabulary (application/module declarations and the standard
/// prelude) is a driver concern.
#[derive(Debug, Clone, PartialEq)]
pub struct DddRaise {
    /// The error variant name as written.
    pub variant: Name,
    /// The argument expressions' raw spans, in written order.
    pub args: Vec<Span>,
    /// Span covering `error <Variant>(<args>)`.
    pub span: Span,
}

/// The `transition <Trigger>` (or `transition <EnumType>.<Trigger>`)
/// binding of a [`DddServiceOp`] (issue pherolang#176): the declared
/// transition a delegated operation fires on the machine bound to the
/// delegation target's repository design. The qualifier — when written —
/// is the machine's **enum type name**, the only machine-qualification the
/// surface has (machine declarations are field-named).
#[derive(Debug, Clone, PartialEq)]
pub struct DddTransitionRef {
    /// The trigger name as written (`Submit`); an explicit `via` trigger
    /// or a bare arrow's derived name.
    pub trigger: Name,
    /// The machine qualifier's enum type name (`PayRunStatus` in
    /// `transition PayRunStatus.Reverse`); `None` for the unqualified
    /// spelling.
    pub machine: Option<Name>,
    /// Span of the whole clause, `transition` included.
    pub span: Span,
}

/// The `=> <target>.<operation>` delegation of a [`DddServiceOp`].
#[derive(Debug, Clone, PartialEq)]
pub struct DddDelegation {
    /// The injected dependency receiving the call.
    pub target: QualifiedName,
    /// The operation invoked on the target.
    pub operation: Name,
    /// Span covering `target.operation`.
    pub span: Span,
}

/// One decision-service reference of a `.ddd` service: an `invokes` entry
/// or an `inject decision` line — the qualified name plus its optional
/// `version` pin (pherolang#154). The pin is the reference's declared
/// compatibility contract; entries without one keep the pin-less shape.
#[derive(Debug, Clone, PartialEq)]
pub struct DddDecisionRef {
    /// The referenced decision service's qualified name. The driver
    /// resolves it by its **last segment** (the rule-17 convention); the
    /// AST keeps it as written.
    pub name: QualifiedName,
    /// The `version compatible` / `version "<X.Y.Z>"` pin, when written.
    pub pin: Option<DddVersionPin>,
}

/// The `version <pin>` clause of a [`DddDecisionRef`] (pherolang#154).
///
/// `version` is the existing lexer token (the `.mox` vocabulary
/// keyword); `compatible` is a contextual word — an ordinary identifier
/// everywhere else, so a service may still be named `compatible`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DddVersionPin {
    /// `version compatible` — the highest version in the highest major
    /// present among the candidates resolves.
    Compatible,
    /// `version "<X.Y.Z>"` — exactly this version resolves.
    Exact(String),
}

/// One parameter of a declared operation: `type_ref multiplicity? name`.
#[derive(Debug, Clone, PartialEq)]
pub struct DddParam {
    /// The parameter type.
    pub type_ref: TypeRef,
    /// Multiplicity annotation, if any.
    pub multiplicity: Option<Multiplicity>,
    /// The parameter name.
    pub name: Name,
    /// Span covering the whole parameter.
    pub span: Span,
}

/// One `error <Name>(<field> (, <field>)*);` declaration of an application
/// or module (DD7, issue pherolang#304): the declared error vocabulary and
/// its typed payload. The fields reuse the operation-parameter production
/// ([`DddParam`]) — an error payload is data, typed against the domain union
/// and the DTO/value types.
///
/// Application-level errors are visible everywhere in the application;
/// module-level errors are private to their module; the standard prelude
/// ([`PRELUDE_ERRORS`]) is always available and cannot be shadowed. `error`
/// is a contextual word: at declaration position it introduces this
/// declaration, inside a body it introduces a raise.
#[derive(Debug, Clone, PartialEq)]
pub struct DddErrorDecl {
    /// The error name.
    pub name: Name,
    /// Payload fields in source order.
    pub fields: Vec<DddParam>,
    /// Description from the doc comment directly above the declaration
    /// (contiguous `///` lines, joined).
    pub doc: Option<String>,
    /// Span of the whole declaration, `error` keyword included.
    pub span: Span,
}

/// The standard prelude's error declarations (DD7): always available, never
/// emitted in the artifact's `errors`, and not shadowable by a user
/// declaration. `NotFound(resource: string, key: string)` and
/// `Conflict(detail: string)`; a body raises them like any declared error,
/// but a prelude raise is still declared in the operation's `throws`.
pub const PRELUDE_ERRORS: [(&str, &[(&str, &str)]); 2] = [
    ("NotFound", &[("resource", "string"), ("key", "string")]),
    ("Conflict", &[("detail", "string")]),
];

/// `true` when the prelude declares the given error name (DD7).
pub fn is_prelude_error(name: &str) -> bool {
    PRELUDE_ERRORS.iter().any(|(prelude, _)| *prelude == name)
}

/// A module-level `children <Class> { ... }` block (issue pherolang#179):
/// child-scoped operations over an aggregate root's contained children —
/// `boolean addLine(PayRunLineDraft line) => lines.add;` means "reach the
/// `lines` child collection through the root and `add` to it". A class may
/// carry several blocks; the driver merges them in source order onto the
/// class's design. (The 0.4 `extend` alias is gone from the grammar in 0.5 —
/// DF4, issue pherolang#327; the formatter rewrites it.)
#[derive(Debug, Clone, PartialEq)]
pub struct DddExtend {
    /// The aggregate-root class being extended.
    pub class: Name,
    /// The child operations in source order.
    pub operations: Vec<DddChildOp>,
    /// Span of the whole block, the block keyword included.
    pub span: Span,
}

/// One `("protected")? signature => feature.action guard_clause? ";"` child
/// operation of a [`DddExtend`]. The signature reuses the service-operation
/// production; the dotted pair names the root's child feature and the
/// action to perform on it (`add`, `remove`, `save` — a closed set the
/// driver validates against the feature's multiplicity).
#[derive(Debug, Clone, PartialEq)]
pub struct DddChildOp {
    /// The operation name.
    pub name: Name,
    /// The declared return type (the child-op signature always declares
    /// one, like a declared service operation).
    pub return_type: TypeRef,
    /// Multiplicity annotation on the return type, if any.
    pub multiplicity: Option<Multiplicity>,
    /// Parameters in source order.
    pub params: Vec<DddParam>,
    /// The root class's child feature the operation acts on (`lines`).
    pub feature: Name,
    /// The action word (`add`, `remove`, `save`), validated by the driver.
    pub action: Name,
    /// `true` for the `protected` modifier: the operation stays off the
    /// public interface (Sculptor's visibility).
    pub is_protected: bool,
    /// The `when ( ... )` guard condition: a `raw_parens` span (parens
    /// included, expression inside deliberately unparsed — slicing and
    /// type-checking it is the driver's job, against the declared
    /// parameters and the child class as `self`).
    pub condition: Option<Span>,
    /// Span of the whole operation, `;` included.
    pub span: Span,
}

/// The DDD stereotype of a [`DddDesign`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DddStereotype {
    /// `entity` — a class with identity.
    Entity,
    /// `value` — an immutable value object.
    Value,
    /// `dto` — a data transfer object.
    Dto,
}

/// Which design flag a [`DddFlags::push`] recorded. The flags are contextual
/// keywords, so — like [`ModifierKind`] — the parser needs the kind to fill
/// the right slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DddFlagKind {
    /// `scaffold` — generate scaffolding.
    Scaffold,
    /// `auditable` — record an audit trail.
    Auditable,
    /// `optimisticLocking` — optimistic locking on persistent state.
    OptimisticLocking,
    /// `nonPersistent` — the class is never persisted.
    NonPersistent,
    /// `cache` — cache instances.
    Cache,
}

impl DddFlagKind {
    /// The keyword text as written in the source (from the `ddd` module's
    /// design-flag table, the single source of truth).
    pub fn keyword(self) -> &'static str {
        crate::ddd::flag_keyword(self)
    }
}

/// One occurrence of a design flag: the flag keyword with its span and
/// whether it was written negated (`!auditable` — the Sculptor opt-out from
/// a flag that defaults on).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DddFlagUse {
    /// `true` for the `!flag` form.
    pub negated: bool,
    /// Span of the occurrence, `!` included when negated.
    pub span: Span,
}

/// The design flags of a [`DddDesign`], each carrying the span of the flag
/// occurrence as written. Absence means the flag is unset (which — for the
/// flags that default on — the driver lowers to the stereotype's default).
/// Repeating a flag is idempotent (the first occurrence wins).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DddFlags {
    /// The `scaffold` occurrence, if present.
    pub scaffold: Option<DddFlagUse>,
    /// The `auditable` occurrence, if present.
    pub auditable: Option<DddFlagUse>,
    /// The `optimisticLocking` occurrence, if present.
    pub optimistic_locking: Option<DddFlagUse>,
    /// The `nonPersistent` occurrence, if present.
    pub non_persistent: Option<DddFlagUse>,
    /// The `cache` occurrence, if present.
    pub cache: Option<DddFlagUse>,
}

impl DddFlags {
    /// Records one flag occurrence; repeats are idempotent (the first wins).
    pub fn push(&mut self, kind: DddFlagKind, occurrence: DddFlagUse) {
        match kind {
            DddFlagKind::Scaffold => self.scaffold = self.scaffold.or(Some(occurrence)),
            DddFlagKind::Auditable => self.auditable = self.auditable.or(Some(occurrence)),
            DddFlagKind::OptimisticLocking => {
                self.optimistic_locking = self.optimistic_locking.or(Some(occurrence))
            }
            DddFlagKind::NonPersistent => {
                self.non_persistent = self.non_persistent.or(Some(occurrence))
            }
            DddFlagKind::Cache => self.cache = self.cache.or(Some(occurrence)),
        }
    }

    /// `true` when no flag is present.
    pub fn is_empty(&self) -> bool {
        self.scaffold.is_none()
            && self.auditable.is_none()
            && self.optimistic_locking.is_none()
            && self.non_persistent.is_none()
            && self.cache.is_none()
    }
}

/// The authored aggregate-boundary declaration of a design (DD4, issue
/// pherolang#301): `root` asserts the class is an aggregate root,
/// `aggregate <Root>` places it in the named root's boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum DddBoundary {
    /// `root` — an authored aggregate root.
    Root,
    /// `aggregate <Root>` — an authored membership in another root.
    Aggregate(QualifiedName),
}

/// One authored boundary clause with its span, so the driver can diagnose a
/// contradictory or illegal declaration at the exact word.
#[derive(Debug, Clone, PartialEq)]
pub struct DddBoundaryDecl {
    /// The authored boundary kind.
    pub kind: DddBoundary,
    /// Span of the whole clause (`root` or `aggregate <Root>`).
    pub span: Span,
}

/// A design declaration of a [`DddModule`]: the DDD decisions for one
/// referenced `.mox` class — `("abstract")? stereotype name flag*
/// boundary_decl? invariant_decl* dto_suffix? ("repository" ...)?
/// ("machine" ...)?` (the members are ordered by the grammar: the boundary
/// and invariants first, then the DTO suffix, the repository, the machine).
///
/// [`DddDesign::class`] is the **declaration name**. For an entity, value,
/// or mirror (`dto` with neither `from` nor a member block) it is the
/// designed `.mox` class. For a DTO with members it is the DTO's own name —
/// `BookSummary` in `dto BookSummary from Book { ... }`, `CreateBookRequest`
/// in `dto CreateBookRequest { String title; }` — while the design's
/// projection source lives in [`DddDesign::dto_source`] and its members in
/// [`DddDesign::dto_members`].
#[derive(Debug, Clone, PartialEq)]
pub struct DddDesign {
    /// The declaration name: the designed `.mox` class for entity/value/
    /// mirror designs, or the DTO's own name for a projection/standalone
    /// `dto`.
    pub class: Name,
    /// The DDD stereotype.
    pub stereotype: DddStereotype,
    /// Whether the `abstract` modifier was written before the stereotype.
    pub is_abstract: bool,
    /// The design flags, in canonical field order regardless of source order.
    pub flags: DddFlags,
    /// The authored aggregate boundary (DD4, issue pherolang#301), when
    /// written.
    pub boundary: Option<DddBoundaryDecl>,
    /// The `invariant <raw_expr>;` clauses (DD5, issue pherolang#302), in
    /// declaration order. Each span covers the raw expression between
    /// `invariant` and `;`; the driver slices and types it.
    pub invariants: Vec<Span>,
    /// The `from <source>` class of a **projection** DTO — the designed
    /// class the projection reads — or `None` for a standalone DTO, a
    /// mirror DTO, and every non-DTO design.
    pub dto_source: Option<QualifiedName>,
    /// The DTO's `{ ... }` member block, when written: a projection (only
    /// for a `from` DTO) or a standalone declaration. `None` for a mirror
    /// DTO and every non-DTO design.
    pub dto_members: Option<DddDtoMembers>,
    /// The class's repository, when designed.
    pub repository: Option<DddRepository>,
    /// The class's lifecycle state machine, when designed.
    pub machine: Option<DddMachine>,
    /// Span of the whole declaration (`abstract` included, when present).
    pub span: Span,
}

/// The `{ ... }` member block of a DTO design (issue pherolang#300): either
/// a projection over a `from <source>` class or a standalone declaration of
/// typed members. The block's shape follows the DTO suffix form — a `from`
/// DTO always carries projection members, a standalone DTO always carries
/// declared members.
#[derive(Debug, Clone, PartialEq)]
pub enum DddDtoMembers {
    /// The members of a `dto Name from Source { ... }` projection.
    Projection(Vec<DddProjectionMember>),
    /// The members of a standalone `dto Name { ... }` declaration.
    Declared(Vec<DddDeclaredMember>),
}

/// One member of a DTO projection block (or of a search's `document`
/// clause — both share this AST): a selected source property or an assigned
/// expression producing a DTO field.
#[derive(Debug, Clone, PartialEq)]
pub enum DddProjectionMember {
    /// `name;` — select the source class's `name` property unchanged.
    Select {
        /// The selected (and produced) field name.
        name: Name,
        /// Span covering the whole member, `;` included.
        span: Span,
    },
    /// `name = <expression>;` — produce `name` from the raw expression; the
    /// expression is captured as a span (the `raw_expr` convention) and
    /// parsed/typed by the driver.
    Assign {
        /// The produced field name.
        name: Name,
        /// Span of the raw expression source (the `;` excluded).
        expr: Span,
        /// Span covering the whole member, `;` included.
        span: Span,
    },
}

/// One `type_ref multiplicity? name;` member of a standalone DTO block
/// (issue pherolang#300): a field the DTO declares on its own, shaped like a
/// `.mox` feature.
#[derive(Debug, Clone, PartialEq)]
pub struct DddDeclaredMember {
    /// The declared member's type.
    pub type_ref: TypeRef,
    /// Multiplicity annotation on the type, if any.
    pub multiplicity: Option<Multiplicity>,
    /// The declared member name.
    pub name: Name,
    /// Span covering the whole member, `;` included.
    pub span: Span,
}

/// The `machine <field>: <Enum> { ... }` block of a [`DddDesign`]: the
/// designed class's lifecycle, a state machine over one enum-typed feature.
///
/// The interleaved source order of the members is not preserved; `initial`,
/// the `terminal` declarations, and the transitions are collected in their
/// own source orders (the same split the wire artifact makes, and the
/// canonical order the formatter emits).
#[derive(Debug, Clone, PartialEq)]
pub struct DddMachine {
    /// The designed class's feature carrying the state; the driver binds the
    /// machine to it (the feature must exist and be enum-typed).
    pub field: Name,
    /// The enum the states are drawn from.
    pub type_: TypeRef,
    /// The `initial <State>;` line, when written; the driver requires exactly
    /// one.
    pub initial: Option<Name>,
    /// The `terminal <State>;` lines in source order.
    pub terminals: Vec<Name>,
    /// The transitions in source order.
    pub transitions: Vec<DddMachineTransition>,
    /// Span of the whole declaration, `machine` keyword included.
    pub span: Span,
}

/// One `from -> to` transition of a [`DddMachine`].
#[derive(Debug, Clone, PartialEq)]
pub struct DddMachineTransition {
    /// The source state name.
    pub from: Name,
    /// The target state name.
    pub to: Name,
    /// The `via <Trigger>` name — the workflow action the transition is
    /// performed through; `None` for a bare arrow (the driver derives the
    /// action name).
    pub trigger: Option<Name>,
    /// The `when ( ... )` guard condition: a `raw_parens` span (parens
    /// included, expression inside deliberately unparsed — slicing and
    /// type-checking it is the driver's job).
    pub guard: Option<Span>,
    /// Span of the whole transition, `;` included.
    pub span: Span,
}

/// The `repository <name> { ... }` block of a [`DddDesign`].
#[derive(Debug, Clone, PartialEq)]
pub struct DddRepository {
    /// The repository name.
    pub name: Name,
    /// Repository operations in source order: built-ins and declared
    /// signatures interleaved as written.
    pub operations: Vec<DddRepositoryOp>,
    /// Span of the whole declaration, `repository` keyword included.
    pub span: Span,
}

/// One operation of a [`DddRepository`]: either a built-in (whose signature
/// the consumer knows), a declared operation with an explicit signature, or a
/// declared **query** operation (DD9, typed repository queries) — a read-only
/// `where`/`sort`/`page` projection over the designed class.
#[derive(Debug, Clone, PartialEq)]
pub struct DddRepositoryOp {
    /// The operation name; for built-ins this is the written keyword.
    pub name: Name,
    /// The built-in op, when this is one; a built-in has no signature of its
    /// own.
    pub builtin: Option<DddBuiltinOp>,
    /// The declared return type; always `Some` for declared operations (the
    /// grammar requires it) and `None` for built-ins.
    pub return_type: Option<TypeRef>,
    /// Multiplicity annotation on the return type, if any.
    pub multiplicity: Option<Multiplicity>,
    /// Declared parameters in source order.
    pub params: Vec<DddParam>,
    /// `true` for the `protected` modifier: the operation stays off the
    /// public interface (Sculptor's visibility).
    pub is_protected: bool,
    /// Error names from the `throws` clause (DD7, issue pherolang#304), in
    /// source order. Declared repository operations take the clause; the
    /// built-in operations take none (a named syntax error).
    pub throws: Vec<Name>,
    /// The `when ( ... )` guard condition of a declared operation: a
    /// `raw_parens` span (parens included, expression inside deliberately
    /// unparsed — slicing and type-checking it is the driver's job).
    /// Built-ins take no condition. A **query** operation takes no guard: its
    /// predicate is [`DddRepositoryOp::query`]'s `where` clause.
    pub condition: Option<Span>,
    /// The executable statement body of a declared operation (issue
    /// pherolang#178): `Type? name(params) when (...)? { ... }`. Built-ins
    /// never carry one.
    pub body: Option<DddBody>,
    /// The read-only query form of a declared operation (DD9, typed
    /// repository queries), when written: the `where`/`sort`/`page` clause
    /// sequence. `None` for built-ins, delegated/plain declared operations,
    /// and body operations.
    pub query: Option<DddQuery>,
    /// Actor capability names from the `capability` clause, in source order.
    /// Non-empty only for the query form: a query may be guarded by the same
    /// `capability <name> (, <name>)*` list a service operation takes, while
    /// a non-query repository operation takes no clause (the wire artifact's
    /// repository operations carry none, so accepting one would silently drop
    /// data).
    pub capabilities: Vec<Name>,
    /// Span of the whole operation, `;` included (or the closing body `}`).
    pub span: Span,
}

/// The read-only query form of a [`DddRepositoryOp`] (DD9, typed repository
/// queries): a `where` predicate, an ordered `sort` key list, and an optional
/// `page` window, canonicalized by the formatter into the fixed clause order
/// `where? sort? page?`.
///
/// The predicate is a `raw_parens` span exactly like the `when` guard
/// convention: the parentheses are included and the expression inside is
/// deliberately unparsed — slicing and type-checking it (against the
/// operation's parameters, with the designed class as `self`) is the driver's
/// job.
#[derive(Debug, Clone, PartialEq)]
pub struct DddQuery {
    /// The `where ( ... )` predicate: a `raw_parens` span (parens included,
    /// expression inside deliberately unparsed).
    pub predicate: Span,
    /// The `sort` keys in written order; empty when the clause is absent.
    pub sort: Vec<DddSortKey>,
    /// The `page` window, when written.
    pub page: Option<DddPage>,
    /// Span of the clause sequence, from the `where` keyword through the
    /// last query clause (the `capability` clause is the operation's, not the
    /// query's).
    pub span: Span,
}

/// The sort direction of a [`DddSortKey`] as authored.
///
/// `asc` is the default: the **formatter omits it** in canonical output
/// (`sort createdDate desc, id asc` formats to `sort createdDate desc, id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DddSortDirection {
    /// `asc` — the default, re-emitted only as its absence.
    Asc,
    /// `desc`.
    Desc,
}

/// One `field ("asc" | "desc")?` key of a [`DddQuery`]'s `sort` clause.
#[derive(Debug, Clone, PartialEq)]
pub struct DddSortKey {
    /// The sorted feature name as written.
    pub field: Name,
    /// The authored direction; `None` when the key omitted one.
    pub direction: Option<DddSortDirection>,
    /// Span covering the key and its direction.
    pub span: Span,
}

/// The `page` window of a [`DddQuery`]: `page (limit <int>)? (max <int>)?
/// cursor?`. Every member is optional and defaults per the consumer's
/// profile; the AST is order-independent (the members' written order is not
/// preserved).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DddPage {
    /// The `limit <int>` page size.
    pub limit: Option<i64>,
    /// The `max <int>` page-size ceiling.
    pub max: Option<i64>,
    /// Whether `cursor` pagination was declared.
    pub cursor: bool,
    /// Span of the whole clause, the `page` keyword included.
    pub span: Span,
}

/// A built-in repository operation, recognized by its contextual keyword in
/// repository-member position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DddBuiltinOp {
    /// `findById` — fetch one aggregate by identity.
    FindById,
    /// `findAll` — fetch all aggregates.
    FindAll,
    /// `findByExample` — fetch the aggregates matching an example instance.
    FindByExample,
    /// `findByKeys` — fetch the aggregates by their natural keys.
    FindByKeys,
    /// `save` — insert or update an aggregate.
    Save,
    /// `delete` — delete an aggregate.
    Delete,
}

impl DddBuiltinOp {
    /// The built-in introduced by the given contextual keyword, or `None`
    /// for any other identifier (from the `ddd` module's repository-builtin
    /// table, the single source of truth).
    pub fn from_keyword(text: &str) -> Option<Self> {
        crate::ddd::builtin_from_keyword(text)
    }

    /// The keyword text as written in the source (from the same table).
    pub fn keyword(self) -> &'static str {
        crate::ddd::builtin_keyword(self)
    }
}

/// A `search <name> { ... }` projection of a [`DddModule`]: the declarative
/// search intent over one designed entity — indexed text fields, filter and
/// sort facets, computed document entries, ranking, pagination, and the
/// actor capabilities guarding it.
///
/// The interleaved source order of the members is not preserved; each
/// member kind is collected in its own source order (the same split the
/// wire artifact makes, and the canonical order the formatter emits).
#[derive(Debug, Clone, PartialEq)]
pub struct DddSearch {
    /// The search projection name.
    pub name: Name,
    /// Description from the doc comment directly above the declaration
    /// (contiguous `///` lines, joined).
    pub doc: Option<String>,
    /// The designed entity the projection is over; `None` when the source
    /// omitted the `entity` line (a driver-side validation error, not a
    /// syntax one).
    pub entity: Option<QualifiedName>,
    /// Indexed text fields in source order.
    pub text: Vec<DddSearchField>,
    /// Filter facet names in source order.
    pub filters: Vec<QualifiedName>,
    /// Sort facet names in source order.
    pub sort: Vec<QualifiedName>,
    /// Document projection entries in source order. The entries share the
    /// DTO projection AST ([`DddProjectionMember`]); search syntax is the
    /// assignment form only (`name = <expression>;`).
    pub document: Vec<DddProjectionMember>,
    /// The named document boundary (`document DtoName;`, DF3, issue
    /// pherolang#326), when the search names a same-module DTO instead of an
    /// inline block. Mutually exclusive with [`Self::document`].
    pub document_ref: Option<QualifiedName>,
    /// The declared ranking strategy, when present.
    pub ranking: Option<DddRanking>,
    /// The search-wide default analyzer, when present.
    pub analyzer: Option<String>,
    /// The declared pagination, when present.
    pub pagination: Option<DddPagination>,
    /// Actor capability names guarding the search, in source order.
    pub capabilities: Vec<Name>,
    /// Span of the whole declaration, `search` keyword included.
    pub span: Span,
}

/// One entry of a search's `text { ... }` clause: an indexed property with
/// its optional boost and per-field analyzer.
#[derive(Debug, Clone, PartialEq)]
pub struct DddSearchField {
    /// The indexed feature of the search's entity.
    pub property: QualifiedName,
    /// The `boost <int>` weight, when present.
    pub boost: Option<i64>,
    /// The `analyzer "<name>"` override, when present.
    pub analyzer: Option<String>,
    /// Span covering the whole field.
    pub span: Span,
}

/// The `ranking <strategy>` line of a [`DddSearch`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DddRanking {
    /// `ranking bm25`.
    Bm25,
    /// `ranking tfIdf`.
    TfIdf,
    /// `ranking exact`.
    Exact,
    /// `ranking custom "<name>"`.
    Custom(String),
}

/// The `pagination { ... }` block of a [`DddSearch`]: every member is
/// optional and defaults per the consumer's profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DddPagination {
    /// The `limit <int>` default page size.
    pub limit: Option<i64>,
    /// The `max <int>` page-size ceiling.
    pub max: Option<i64>,
    /// Whether `cursor` pagination was declared.
    pub cursor: bool,
    /// Span of the whole block, `pagination` keyword included.
    pub span: Span,
}
