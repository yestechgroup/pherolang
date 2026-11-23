//! The `driver.ddd` band: `E0350`–`E0449`, the phero-driver `.ddd` lifecycle
//! and aggregate-operation checks (issues pherolang#176, pherolang#179, and
//! pherolang#180 — the service-operation transition bindings, the
//! child-scoped `children` operations, and the `when`-condition scoping over
//! `crates/phero-driver/src/ddd/`). Every check prefixes its code onto the
//! diagnostic message (`E0350 unknown transition trigger …`), the
//! `dmn.check` band's E0250/E0251 precedent. E0350–E0352 are the
//! transition-binding checks; E0353–E0354 are the condition-scope checks;
//! E0355–E0358 are the child-operation (`children` block) checks; E0359–E0360 are the
//! executable operation-body checks (issue pherolang#178); E0361–E0364 are
//! the strict value-semantics checks (issue pherolang#299: R-V1, R-V2, R-V3,
//! and the `abstract value` rejection); E0365–E0372 are the DTO-shape checks
//! (issue pherolang#300: the dto-op error, member duplication/unknown/type,
//! entity members, non-input parameters, name collisions, and entity-only
//! constructs on a DTO); E0373–E0378 are the declared error-vocabulary and
//! `throws` checks (DD7, issue pherolang#304: unknown throws name,
//! duplicate/shadowed error, module-private visibility, entity payload
//! field, narrowed delegation throws, and duplicate error field);
//! E0390–E0395 are the multi-file application tree checks (DD8, issue
//! pherolang#305: orphan fragment, fragment under two roots, duplicate
//! application name, import cycle, root imported as a fragment, and a
//! missing fragment); E0396–E0399 are the typed repository-query checks
//! (DD9, issue pherolang#306: read-only shape, predicate typing, sort keys,
//! and pagination); E0400–E0402 are the named search-document checks (DF3,
//! issue pherolang#326: an invalid `document DtoName;` reference, a source
//! mismatch, and an inline-plus-named conflict); E0403–E0408 are the
//! machine-reference checks (issue pherolang#388: an unprovided file, an
//! unparseable one, a file with no machines or several, an anonymous
//! machine, and a machine outside the flat single-target subset).
//! Claim a new code by adding a documented constant plus an `Entry` to
//! `ENTRIES`.

use crate::{Entry, Severity};

/// The band's registry id ([`crate::all`] entries reference it).
pub const DDD_BAND: &str = "driver.ddd";

/// `unknown transition trigger '<trigger>' on operation '<op>' of service
/// '<service>'` — the transition binding names a trigger the machine bound
/// to the delegation target's repository design does not declare (an
/// unknown name, a qualifier that names a foreign machine's enum, or a
/// name both declared `via` and derived from a bare arrow — ambiguous, so
/// it binds nothing).
pub const E_UNKNOWN_TRANSITION_TRIGGER: &str = "E0350";
/// `transition on operation '<op>' of service '<service>', which …` — the
/// transition binding is legal only on a delegation to a repository `save`;
/// a declared operation, or a delegation to anything else, carries none.
pub const E_TRANSITION_NOT_SAVE: &str = "E0351";
/// `transition on operation '<op>' of service '<service>' has no machine
/// qualifier` — a multi-machine module's bindings must name the machine by
/// its enum type (`transition <EnumType>.<Trigger>`); the unqualified
/// spelling cannot say which machine it advances.
pub const E_TRANSITION_QUALIFIER_REQUIRED: &str = "E0352";
/// `unknown condition path '<path>' in <context>` — the condition path's
/// first segment names no scope binding of the operation (and no declared
/// injection it could be a typo of), or a qualified path names no field of
/// the injected repository's class.
pub const E_UNKNOWN_CONDITION_PATH: &str = "E0353";
/// `ambiguous machine scope: '<name>' is declared by more than one machine
/// in module '<module>'` — a condition references a state name (or a
/// state-typed field name) that more than one machine of the module
/// declares, so the unqualified spelling never binds; the qualified
/// spellings (`<EnumType>.<State>`, and lowerCamel(`<EnumType>`) for a
/// field) resolve it.
pub const E_AMBIGUOUS_MACHINE_SCOPE: &str = "E0354";
/// `unknown child feature '<feature>' on '<class>'` — a `children` block's
/// child operation names a feature the extended class does not carry (or
/// one that is not a containment/reference, so it has no child aggregate
/// to act on).
pub const E_UNKNOWN_CHILD_FEATURE: &str = "E0355";
/// `child operation '<op>' in children '<class>': action '<action>'
/// requires a collection` — `add` and `remove` operate on the elements of a
/// many-valued child feature; a single-valued containment takes only
/// `save`.
pub const E_CHILD_ACTION_NEEDS_COLLECTION: &str = "E0356";
/// `unknown child action '<action>' in child operation '<op>'` — the
/// child-operation action is one of the closed set `add`, `remove`,
/// `save`.
pub const E_UNKNOWN_CHILD_ACTION: &str = "E0357";
/// `unknown children target '<class>'` — the `children` block's class has
/// no entity design that is an aggregate root (a class declared in the
/// imported domains, designed in the same module, with a repository).
pub const E_UNKNOWN_EXTEND_TARGET: &str = "E0358";
/// `unknown body receiver '<name>' on operation '<op>' of service
/// '<service>'` — a statement-body operation's `=> <Repository>` receiver
/// names no repository designed in the operation's module that the service
/// injects (the primary repository bound as `self`), or the receiver is not
/// a single repository name.
pub const E_UNKNOWN_BODY_RECEIVER: &str = "E0359";
/// `invalid service body in operation '<op>': <slot>: <detail>` (or the
/// repository-body wording) — the closed statement subset's typing and flow
/// contract: an unknown feature/operation, a type mismatch, a non-boolean
/// guard, an unknown raise variant/arity, or a value-returning path that
/// does not return. Unlike the sigil statement bodies, every failure here is
/// a hard compile error.
pub const E_INVALID_OPERATION_BODY: &str = "E0360";
/// `value design '<design>' in module '<module>' of application
/// '<application>': feature '<feature>' carries the 'id' modifier; a value
/// object has no identity` — rule R-V1 of the strict value semantics (DD2,
/// pherolang#299): a `value` design targets a class with no identity, so
/// every `id` feature of the class (including inherited ones) is refused.
pub const E_VALUE_HAS_IDENTITY: &str = "E0361";
/// `value design '<design>' in module '<module>' of application
/// '<application>': feature '<feature>' contains entity '<entity>'; a value
/// object owns no mutable state` — rule R-V2: a value may contain values,
/// never a class designed as an `entity` in the same application.
pub const E_VALUE_CONTAINS_ENTITY: &str = "E0362";
/// `value design '<design>' in module '<module>' of application
/// '<application>': feature '<feature>' refers to entity '<entity>'; a value
/// object carries no entity references` — rule R-V3: a value refers to an
/// entity by id, never by object reference.
pub const E_VALUE_REFERENCES_ENTITY: &str = "E0363";
/// `value design '<design>' in module '<module>' of application
/// '<application>' is abstract; a value object is concrete` — strict value
/// semantics have no abstract values (an abstract value would leave its
/// identity/structure undefined).
pub const E_ABSTRACT_VALUE: &str = "E0364";
/// `dto design '<design>': class carries op '<op>'; a dto is a transfer
/// record and carries no behavior` — rule 15 (DD3, pherolang#300): the
/// dto-with-ops warning is an error, since a DTO is behavior-free.
pub const E_DTO_OPERATION: &str = "E0365";
/// `dto '<design>': duplicate member '<name>'` — two members of one DTO
/// produce the same output field name.
pub const E_DTO_DUPLICATE_MEMBER: &str = "E0366";
/// `unknown dto member '<name>' on '<class>'` (or an unknown path segment)
/// — a projection selects a feature the source class does not carry.
pub const E_DTO_UNKNOWN_MEMBER: &str = "E0367";
/// `invalid dto member '<name>' of '<design>': <detail>` — a projection
/// expression does not parse, does not type-check, or produces a type a DTO
/// boundary cannot carry; a declared member's type does not resolve.
pub const E_DTO_MEMBER_TYPE: &str = "E0368";
/// `dto member '<name>' of '<design>' carries entity '<entity>'` — a
/// transport shape may not embed a mutable entity; project its fields
/// instead.
pub const E_DTO_ENTITY_MEMBER: &str = "E0369";
/// `dto '<design>' is output-only: … parameter '<param>' of <context> is
/// not input-capable` — a DTO with derived members cannot be supplied by a
/// caller; the diagnostic names the derived members.
pub const E_DTO_PARAM_NOT_INPUT: &str = "E0370";
/// `dto '<name>' collides with <kind> '<name>'` (or `duplicate dto
/// '<name>'`) — a DTO introduces an application-wide boundary name that
/// must be unique and must not shadow a domain type (a mirror DTO is its
/// source class and is exempt).
pub const E_DTO_NAME_COLLISION: &str = "E0371";
/// `dto design '<design>' carries <construct>; a dto is a transport shape`
/// — a DTO may not carry a repository, a machine, or an entity-only flag
/// (`scaffold`, `auditable`, `optimisticLocking`).
pub const E_DTO_ENTITY_ONLY: &str = "E0372";
/// `unknown error '<name>' in throws of <context>` — a `throws` entry names
/// no error declared at application scope, none private to the operation's
/// module, and none of the standard prelude.
pub const E_UNKNOWN_THROWN_ERROR: &str = "E0373";
/// `duplicate error '<name>'` or `error '<name>' shadows a prelude error` —
/// error names are application-wide unique, and the standard prelude
/// (`NotFound`, `Conflict`) cannot be shadowed.
pub const E_DUPLICATE_ERROR: &str = "E0374";
/// `error '<name>' is private to module '<module>'` — a module-level error is
/// visible only inside its module; another module cannot name it in
/// `throws`, so the cross-module contract must be application-scoped.
pub const E_MODULE_PRIVATE_ERROR: &str = "E0375";
/// `error '<name>' field '<field>' carries entity '<entity>'` — an error
/// payload crosses boundaries and must be serializable data, so it may not
/// embed a class designed as an entity.
pub const E_ERROR_PAYLOAD_ENTITY: &str = "E0376";
/// `delegation on <context> narrows the throws of '<target>.<operation>':
/// missing '<error>'` — a delegation is contract-preserving: its `throws`
/// must be a superset of the delegated operation's.
pub const E_NARROWED_DELEGATION_THROWS: &str = "E0377";
/// `duplicate field '<field>' in error '<name>'` — the payload field names
/// of one error declaration are unique.
pub const E_DUPLICATE_ERROR_FIELD: &str = "E0378";
/// `entity '<class>' declares both 'root' and 'aggregate <root>'; the two
/// boundary decisions contradict each other` (DD4, issue pherolang#301).
pub const E_AGGREGATE_CONTRADICTION: &str = "E0379";
/// `entity '<class>' is contained by '<owner>'; an owned class cannot be an
/// independent aggregate root` — `root` on a contained class (DD4).
pub const E_AGGREGATE_CONTAINED_ROOT: &str = "E0380";
/// `entity '<class>' is contained by '<owner>', but declares aggregate
/// '<root>'; containment fixes its root` — a conflicting authored membership
/// (DD4).
pub const E_AGGREGATE_CONTAINED_TARGET: &str = "E0381";
/// `aggregate root '<root>' has no entity design in this module` (DD4).
pub const E_AGGREGATE_UNKNOWN_ROOT: &str = "E0382";
/// `aggregate '<class>' belongs to module '<root module>', not '<module>'`
/// — aggregate membership cannot cross modules (DD4).
pub const E_AGGREGATE_CROSS_MODULE: &str = "E0383";
/// `root`/`aggregate` on a value or dto design — only entities have an
/// aggregate boundary (DD4).
pub const E_AGGREGATE_NON_ENTITY: &str = "E0384";
/// `aggregate boundary of '<class>' is unknown (...)` — the union or the
/// containment graph cannot decide it; the artifact omits
/// `isAggregateRoot` (DD4).
pub const E_AGGREGATE_BOUNDARY_UNKNOWN: &str = "E0385";
/// `aggregate root '<root>' is itself a member of '<outer>'; name the
/// outermost root` (DD4).
pub const E_AGGREGATE_ROOT_NOT_ROOT: &str = "E0386";
// E0387 is retired (issue pherolang#359, DF4 #327): it was
// `E_DEPRECATED_EXTEND`, the `extend`-alias deprecation warning. The alias
// was removed from the grammar in 0.5, so the code burned with it —
// never reuse E0387.
/// `invalid invariant in design '<design>': <detail>` — the invariant does
/// not parse, does not type-check, is not boolean, or reads beyond the
/// designed class's own state (DD5, issue pherolang#302).
pub const E_INVALID_INVARIANT: &str = "E0388";
/// `design '<design>' is a dto; a transport shape carries no invariants` —
/// invariants are entity/value semantics (DD5).
pub const E_DTO_INVARIANT: &str = "E0389";
/// `design fragment '<path>' has no root: no .ddd application imports it` —
/// a file of top-level `module` blocks is legal only as an imported fragment
/// of exactly one design root (DD8, issue pherolang#305).
pub const E_DESIGN_FRAGMENT_ORPHAN: &str = "E0390";
/// `design fragment '<path>' is imported by more than one root: <roots>` — a
/// design file belongs to exactly one application (DD8).
pub const E_DESIGN_FRAGMENT_MULTI_ROOT: &str = "E0391";
/// `duplicate design application name '<name>' in <roots>` — the authored
/// `application <Name>` is the graph's application id, so two roots in one
/// checked tree may not share it (DD8).
pub const E_DESIGN_APPLICATION_NAME_DUPLICATE: &str = "E0392";
/// `design import cycle through <path>` — `.ddd` imports form a cycle, so no
/// root can merge the fragments (DD8).
pub const E_DESIGN_IMPORT_CYCLE: &str = "E0393";
/// `a design application root cannot be imported as a fragment; remove its
/// application declaration or import its root instead` — a `.ddd` file with
/// an `application` declaration is a root by definition (DD8).
pub const E_DESIGN_ROOT_AS_FRAGMENT: &str = "E0394";
/// `imported design fragment '<path>' was not provided` — a declared `.ddd`
/// import the host could not read (DD8).
pub const E_DESIGN_FRAGMENT_MISSING: &str = "E0395";
/// `query operation '<op>' of repository '<repo>' …` — a repository query is
/// read-only by construction: no statement body, no `when` guard (the
/// predicate is `where`), no `throws` (DD9, issue pherolang#306).
pub const E_QUERY_READ_ONLY: &str = "E0396";
/// `invalid query predicate in operation '<op>' of repository '<repo>': …` —
/// the `where` predicate does not parse, does not type-check, is not
/// boolean, or a parameter collides with a feature of the designed class
/// (DD9).
pub const E_QUERY_PREDICATE: &str = "E0397";
/// `invalid sort key '<field>' in operation '<op>' of repository '<repo>':
/// …` — a sort key must be a direct stored scalar feature; a class
/// reference, a collection, and a derived feature are each refused with the
/// reason (DD9).
pub const E_QUERY_SORT: &str = "E0398";
/// `invalid pagination in operation '<op>' of repository '<repo>': …` —
/// `limit`/`max` must be positive and `limit` may not exceed `max` (DD9).
pub const E_QUERY_PAGE: &str = "E0399";
/// `invalid document reference '<name>' in search '<search>': <detail>` —
/// the named document is not a same-module dto boundary with a source class
/// (an unknown name, another module, or a standalone dto) (DF3).
pub const E_DOCUMENT_REF: &str = "E0400";
/// `document reference '<name>' of search '<search>' projects '<source>', but
/// the search targets '<entity>'` — a named document must be a mirror or
/// projection of the search's own entity (DF3).
pub const E_DOCUMENT_SOURCE_MISMATCH: &str = "E0401";
/// `search '<search>' declares both an inline document block and a named
/// document reference` — exactly one document shape per search (DF3).
pub const E_DOCUMENT_FORM_CONFLICT: &str = "E0402";
/// `referenced machine file '<path>' was not provided` — the host (CLI/LSP)
/// did not supply content for the path a `machine … from "<path>";`
/// reference names: the file does not exist relative to the declaring
/// `.ddd` file, or the host could not read it (issue pherolang#388).
pub const E_MACHINE_REF_NOT_PROVIDED: &str = "E0403";
/// `referenced machine file '<path>' did not parse: <detail>` — the file
/// exists but is not a representable XState machine: a TypeScript syntax
/// error, or one of the fail-closed parser's typed rejections (an inline
/// guard, a non-literal scalar, …) whose message is carried verbatim
/// (issue pherolang#388).
pub const E_MACHINE_REF_PARSE: &str = "E0404";
/// `referenced machine file '<path>' declares no machine` — the file parsed
/// as TypeScript but contains no `createMachine` call, so it binds nothing
/// (issue pherolang#388).
pub const E_MACHINE_REF_EMPTY: &str = "E0405";
/// `referenced machine file '<path>' declares several machines: '<id>',
/// …` — v1 binds exactly one machine per referenced file; a file exporting
/// several must name one (the deferred `as` clause) or be split
/// (issue pherolang#388).
pub const E_MACHINE_REF_SEVERAL: &str = "E0406";
/// `referenced machine file '<path>' carries no stable machine id` — the
/// authored `createMachine` declared no `id`, so the parser synthesized an
/// index-derived one; the id joins the machine's provenance and must be
/// authored (issue pherolang#388).
pub const E_MACHINE_REF_NO_ID: &str = "E0407";
/// `referenced machine '<id>' declares <detail>` — the machine uses a
/// construct the design mirror cannot carry: a nested (compound or
/// parallel) state, a transition targeting several states, a transition
/// with no target, or an automatic (`onDone`/`invoke`/`after`) transition.
/// The mirror is flat and event-driven (issue pherolang#388).
pub const E_MACHINE_REF_SHAPE: &str = "E0408";

pub(crate) const ENTRIES: &[Entry] = &[
    Entry {
        code: E_UNKNOWN_TRANSITION_TRIGGER,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Unknown transition trigger",
        meaning: "`unknown transition trigger '<trigger>' on operation '<op>' of service '<service>'` — the operation's `transition` binding names no trigger of the machine bound to the delegation target's repository design: the spelling is a typo (a did-you-mean names the closest declared trigger when one is plausible), the qualifier names another machine's enum type, the target's design carries no machine, or the name is both declared `via` and derived from a bare arrow — ambiguous, so it binds nothing.",
        fix: "Name a trigger the machine declares — an explicit `via` name or a bare arrow's derived name (the lowerCamel of the target state) — and qualify it with the machine's enum type in a multi-machine module.",
    },
    Entry {
        code: E_TRANSITION_NOT_SAVE,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Transition on a non-save delegation",
        meaning: "`transition on operation '<op>' of service '<service>', which declares its own signature` or `… which delegates to <target>`.`<operation>`; only a `save` delegation carries a transition` — the transition binding fires the machine-bound transition of the aggregate a repository `save` persists, so a declared operation or a delegation to anything else has no machine to bind.",
        fix: "Delegate with `name => <Repository>.save` and bind the transition there, or drop the clause.",
    },
    Entry {
        code: E_TRANSITION_QUALIFIER_REQUIRED,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Transition qualifier required",
        meaning: "`transition on operation '<op>' of service '<service>' has no machine qualifier; module '<module>' carries <n> machine-bearing designs` — in a multi-machine module the unqualified spelling cannot say which machine the operation advances, so the binding is refused.",
        fix: "Qualify the trigger by the machine's enum type: `transition PayRunStatus.Submit`.",
    },
    Entry {
        code: E_UNKNOWN_CONDITION_PATH,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Unknown condition path",
        meaning: "`unknown condition path '<path>' in <context>` — the `when` condition's qualified path names no scope binding of the operation: the first segment is neither an injected repository of the service nor any other bound name, or a later segment names no field of the injected repository's designed class. A path that resolves but mistypes is not this error — the shared expression language's same-type rule reports it.",
        fix: "Declare the injection (`inject <Repository>;`), fix the spelling, or name a field the repository's class carries.",
    },
    Entry {
        code: E_AMBIGUOUS_MACHINE_SCOPE,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Ambiguous machine scope",
        meaning: "`ambiguous machine scope: '<name>' is declared by more than one machine in module '<module>'` — the condition references a state name or a state-typed field name that several machines of the module declare, so the unqualified spelling never binds (multi-machine modules admit op-level `when` guards only when every referenced name resolves uniquely). The qualified spellings resolve it: `<EnumType>.<State>` for a state, lowerCamel(`<EnumType>`) for a field.",
        fix: "Qualify the reference by its machine's enum type — `PayRunStatus.Draft` for a state, `payRunStatus` for a field.",
    },
    Entry {
        code: E_UNKNOWN_CHILD_FEATURE,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Unknown child feature",
        meaning: "`unknown child feature '<feature>' on '<class>'` (or `child feature '<feature>' of '<class>' is an attribute`) — a `children` block's child operation names a feature the extended class does not carry, or one that is not a containment or cross reference (an attribute has no child aggregate to act on). `children` operates on the aggregate's contained children, so only `contains` and `refers` features are eligible.",
        fix: "Name a containment or reference feature of the extended class (the help lists the eligible ones), or extend the class that actually holds the child.",
    },
    Entry {
        code: E_CHILD_ACTION_NEEDS_COLLECTION,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Child action needs a collection",
        meaning: "`child operation '<op>' in children '<class>': action '<action>' requires a collection; feature '<feature>' is single-valued` — `add` and `remove` act on the elements of a many-valued child feature, so a single-valued containment (a scalar child object) admits only `save`.",
        fix: "Use `save` for a single-valued child, or declare the feature as a many-valued containment (`contains Child[] children`).",
    },
    Entry {
        code: E_UNKNOWN_CHILD_ACTION,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Unknown child action",
        meaning: "`unknown child action '<action>' in child operation '<op>' of children '<class>'` — the action after the child feature's dot is one of the closed vocabulary `add`, `remove`, `save`: add a new child, remove an existing one, or persist (insert/replace) it. The action selects the aggregate-internal operation the child-scoped operation performs, so an unknown word is refused rather than silently ignored (the lowered artifact carries the operation's signature and its `childScope`).",
        fix: "Write `add`, `remove`, or `save` as the action.",
    },
    Entry {
        code: E_UNKNOWN_EXTEND_TARGET,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Unknown children target",
        meaning: "`unknown children target '<class>'` — the `children` block's class must be a domain class with an entity design in the same module that is an aggregate root (a repository-bearing entity, validation rule 6). The message names which part failed: the class resolves against the imported domains, the design must live in the block's module, and the design must carry a repository (a contained, repository-less child is not a root).",
        fix: "Extend an aggregate root designed in the same module (add its `repository` block if it is missing), or move the `children` block next to the root's design.",
    },
    Entry {
        code: E_UNKNOWN_BODY_RECEIVER,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Unknown body receiver",
        meaning: "`unknown body receiver '<name>' on operation '<op>' of service '<service>'` — a statement-body operation written `name(...) => <Repository> { ... }` binds the named repository as `self`; the name must match a repository designed in the operation's module that the service `inject`s. A qualified receiver is refused at the syntax layer (the body binds one repository as `self`), and a repository another module designs is not addressable from this service (the module-coupling rule).",
        fix: "Name a repository the service injects (`inject <Repository>;`), or add the missing `inject` line.",
    },
    Entry {
        code: E_INVALID_OPERATION_BODY,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Invalid operation body",
        meaning: "`invalid service body in operation '<op>': <slot>: <detail>` (repository bodies use the same shape) — the `.ddd` statement body is a closed procedural subset (`let`, `let … else error …`, assignment, `guard … else error …`, `if`/`else`, `return`), typed against the operation's parameters, the designed aggregate, and the synthetic repository classes. Unknown features/operations, type mismatches, non-boolean guards, unknown raise variants or arities, and value-returning paths that do not return (`missing return`) are all hard compile errors — unlike the sigil Rune bodies' generate-time deferral, the `.ddd` compiler never attaches an unchecked body.",
        fix: "Fix the named slot: use a declared feature or operation, match the target type exactly, make guards boolean, raise `NotFound` (2 arguments) or `Conflict` (1 argument), and return on every successful path of a value-returning operation.",
    },
    Entry {
        code: E_VALUE_HAS_IDENTITY,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Value design has an identity feature",
        meaning: "`value design '<design>' in module '<module>' of application '<application>': feature '<feature>' carries the 'id' modifier; a value object has no identity` — rule R-V1 of the strict value semantics (DD2). A `value` design is strictly structural: it has no identity, so every `id` feature of its class (own or inherited) is refused.",
        fix: "Remove the `id` modifier from the named feature (a stored surrogate id may stay a plain `readonly` attribute, which is not identity), or design the class as an `entity`.",
    },
    Entry {
        code: E_VALUE_CONTAINS_ENTITY,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Value design contains an entity",
        meaning: "`value design '<design>' in module '<module>' of application '<application>': feature '<feature>' contains entity '<entity>'; a value object owns no mutable state` — rule R-V2 of the strict value semantics (DD2). Deep immutability means a value may contain other values, never a class designed as an `entity` in the same application (whose state could mutate after construction).",
        fix: "Contain another value object, refer to the entity by id (`refers String <name>Id`), or move the containment onto an entity design.",
    },
    Entry {
        code: E_VALUE_REFERENCES_ENTITY,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Value design references an entity",
        meaning: "`value design '<design>' in module '<module>' of application '<application>': feature '<feature>' refers to entity '<entity>'; a value object carries no entity references` — rule R-V3 of the strict value semantics (DD2). A cross-reference to a designed entity would let a mutable object hide inside an immutable value; a value refers to an entity by id instead.",
        fix: "Refer to the entity by id (`refers String <name>Id`), or model the relationship on the entity that owns it.",
    },
    Entry {
        code: E_ABSTRACT_VALUE,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Abstract value design",
        meaning: "`value design '<design>' in module '<module>' of application '<application>' is abstract; a value object is concrete` — the strict value semantics (DD2) admit no abstract values: a value's identity, structure, and equality are exactly its stored features, so an abstract value would leave its instances undefined.",
        fix: "Remove `abstract` from the design, or design the class as an `entity` (abstract entity designs are the sanctioned polymorphism shape).",
    },
    Entry {
        code: E_DTO_OPERATION,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "DTO carries an operation",
        meaning: "`dto design '<design>': class carries op '<op>'; a dto is a transfer record and carries no behavior` — rule 15 (DD3): a DTO is a behavior-free transport shape, so an `op` on a class designed `dto` is a modeling error rather than the pre-DD3 warning. A `derived` member stays legal — it is a computed wire field, not behavior.",
        fix: "Move the behavior to an aggregate-owned class `op`, or declare a service operation that delegates to it.",
    },
    Entry {
        code: E_DTO_DUPLICATE_MEMBER,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Duplicate DTO member",
        meaning: "`dto '<design>': duplicate member '<name>'` — two members of one DTO produce the same output field name, so the wire shape would carry an ambiguous field. A rename that lands on another member's name is the same collision: output names, not source paths, must be unique.",
        fix: "Rename one of the members so every output field name is distinct.",
    },
    Entry {
        code: E_DTO_UNKNOWN_MEMBER,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Unknown DTO member",
        meaning: "`unknown dto member '<name>' on '<class>'` or an unknown path segment — a projection (`dto Name from Source { … }`) selects a feature or traverses a path the source class does not carry (inherited features included).",
        fix: "Select a feature the source class declares (the help lists them), or fix the path's spelling.",
    },
    Entry {
        code: E_DTO_MEMBER_TYPE,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Invalid DTO member",
        meaning: "`invalid dto member '<name>' of '<design>': <detail>` — a projection's `name = expr` assignment does not parse, does not type-check with the source class as `self`, or yields a type a transport boundary cannot carry; a standalone member's declared type does not resolve against the domain union or the application's other DTOs.",
        fix: "Fix the expression or the declared type, or project a value of a type the boundary can carry.",
    },
    Entry {
        code: E_DTO_ENTITY_MEMBER,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "DTO carries an entity",
        meaning: "`dto member '<name>' of '<design>' carries entity '<entity>'` — a transport shape may not embed a class designed as an `entity` in the same application: it would re-create the mutable, lazy-loaded coupling DTOs exist to avoid.",
        fix: "Project the entity's fields as plain members, or refer to it by id.",
    },
    Entry {
        code: E_DTO_PARAM_NOT_INPUT,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Output-only DTO as a parameter",
        meaning: "`dto '<design>' is output-only: … parameter '<param>' of <context> is not input-capable` — a DTO with derived members cannot be constructed by a caller, so it may be returned but not accepted as an operation parameter; the diagnostic names the derived members that cannot be supplied.",
        fix: "Use an input-capable DTO (no derived members) for the parameter, or make the derived member a select.",
    },
    Entry {
        code: E_DTO_NAME_COLLISION,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "DTO name collision",
        meaning: "`dto '<name>' collides with <kind> '<name>'` or `duplicate dto '<name>'` — a projection or standalone DTO introduces an application-wide boundary name; it must be unique and must not shadow a domain class, enum, datatype, interface, or vocabulary. A mirror DTO's name is its source class and is exempt.",
        fix: "Rename the DTO so its boundary name is unique and does not shadow a domain type.",
    },
    Entry {
        code: E_DTO_ENTITY_ONLY,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Entity-only construct on a DTO",
        meaning: "`dto design '<design>' carries <construct>; a dto is a transport shape` — a DTO has no persistence, lifecycle, or domain identity, so a `repository`, a `machine`, or an entity-only flag (`scaffold`, `auditable`, `optimisticLocking`) is refused. `nonPersistent` and `cache` stay legal.",
        fix: "Remove the construct from the DTO design, or design the class as an `entity` if it really owns state.",
    },
    Entry {
        code: E_UNKNOWN_THROWN_ERROR,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Unknown thrown error",
        meaning: "`unknown error '<name>' in throws of <context>` — a `throws` entry names no error of the application's vocabulary: application-scoped declarations are visible everywhere, module-scoped ones only inside their module (the message names the declaring module when that is the reason), and the standard prelude (`NotFound`, `Conflict`) is always available. The help lists the declared set in scope.",
        fix: "Declare the error with `error <Name>(...)` at application (or the operation's module) scope, or fix the spelling — a did-you-mean names the closest declared error when one is plausible.",
    },
    Entry {
        code: E_DUPLICATE_ERROR,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Duplicate or shadowed error",
        meaning: "`duplicate error '<name>'` or `error '<name>' shadows a prelude error` — error names are application-wide unique (like service names), and the standard prelude's `NotFound` and `Conflict` cannot be shadowed by a user declaration. The first declaration keeps resolving; the later one is reported.",
        fix: "Rename the error, or reference the existing application-wide declaration instead of redeclaring it. The prelude's shapes are fixed; choose another name for a domain error.",
    },
    Entry {
        code: E_MODULE_PRIVATE_ERROR,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Module-private error referenced",
        meaning: "`error '<name>' is private to module '<module>'` — a module-level `error` declaration is visible only inside its module, so an operation in another module cannot name it in `throws`. Module errors are the private detail; application-scoped errors are the cross-module contract.",
        fix: "Declare the error at application scope (move the declaration out of the module body), or keep the operation inside the declaring module.",
    },
    Entry {
        code: E_ERROR_PAYLOAD_ENTITY,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Entity in an error payload",
        meaning: "`error '<name>' field '<field>' carries entity '<entity>'; an error payload is data` — an error crosses a boundary with its payload unchanged, so the payload must be serializable data. Embedding a class designed as an `entity` in the same application would put mutable state on the wire.",
        fix: "Refer to the entity by id (a string/primitive field), embed a `value` object, or carry a DTO boundary.",
    },
    Entry {
        code: E_NARROWED_DELEGATION_THROWS,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Delegation narrows throws",
        meaning: "`delegation on <context> narrows the throws of '<target>.<operation>': missing '<error>'` — a delegation is contract-preserving: the delegating operation's `throws` must be a superset of the delegated operation's, so a caller can rely on the declared set. A missing entry names the error (and, when the target's error is module-private, the application-scope fix).",
        fix: "Add every missing error to the delegation's `throws` clause; if the target's error is module-private, declare it at application scope first.",
    },
    Entry {
        code: E_DUPLICATE_ERROR_FIELD,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Duplicate error payload field",
        meaning: "`duplicate field '<field>' in error '<name>'` — the payload field names of one `error` declaration are unique, since a consumer builds one field per name in declaration order.",
        fix: "Rename or remove the duplicate field.",
    },
    Entry {
        code: E_AGGREGATE_CONTRADICTION,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Contradictory aggregate boundary",
        meaning: "`entity '<class>' declares both 'root' and 'aggregate <root>'` — the two boundary decisions cannot both hold; a class is either its own aggregate root or a member of another root.",
        fix: "Keep one clause: `root` for a root, `aggregate <Root>` for a member.",
    },
    Entry {
        code: E_AGGREGATE_CONTAINED_ROOT,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Contained class declared a root",
        meaning: "`entity '<class>' is contained by '<owner>'; an owned class cannot be an independent aggregate root` — containment already places the class inside the owner's boundary, so `root` contradicts the domain model.",
        fix: "Remove `root`, or remove the `contains` feature that owns the class and model the relationship explicitly.",
    },
    Entry {
        code: E_AGGREGATE_CONTAINED_TARGET,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Contained class declares a foreign root",
        meaning: "`entity '<class>' is contained by '<owner>', but declares aggregate '<root>'; containment fixes its root` — a contained class's aggregate membership is derived from the containment chain and cannot be reassigned.",
        fix: "Remove the `aggregate` clause, or change the domain containment so the class is not owned.",
    },
    Entry {
        code: E_AGGREGATE_UNKNOWN_ROOT,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Unknown aggregate root",
        meaning: "`aggregate root '<root>' has no entity design in this module` — `aggregate <Root>` must name a class with an entity design in the same module.",
        fix: "Declare the root's entity design in the same module, or fix the reference.",
    },
    Entry {
        code: E_AGGREGATE_CROSS_MODULE,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Cross-module aggregate membership",
        meaning: "`aggregate '<class>' belongs to module '<root module>', not '<module>'` — an aggregate is a module-local consistency boundary; membership cannot cross modules.",
        fix: "Move the member into the root's module, or model a separate aggregate.",
    },
    Entry {
        code: E_AGGREGATE_NON_ENTITY,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Aggregate boundary on a non-entity",
        meaning: "`design '<class>' declares an aggregate boundary, but is a <stereotype>` — only an entity has identity and an aggregate boundary; a value is immutable and a dto is a transport shape.",
        fix: "Remove the boundary clause, or design the class as an `entity`.",
    },
    Entry {
        code: E_AGGREGATE_BOUNDARY_UNKNOWN,
        band: DDD_BAND,
        severity: Severity::Warning,
        title: "Unknown aggregate boundary",
        meaning: "`aggregate boundary of '<class>' is unknown (...)` — the imported domain union or the containment graph cannot decide the boundary (a domain failed to lower, the class sits under several roots, or a containment cycle exists). The artifact omits `isAggregateRoot` rather than guessing.",
        fix: "Fix the imported domain, or author the boundary explicitly with `root`/`aggregate <Root>`.",
    },
    Entry {
        code: E_AGGREGATE_ROOT_NOT_ROOT,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Aggregate root is itself a member",
        meaning: "`aggregate root '<root>' is itself a member of '<outer>'; name the outermost root` — an aggregate root cannot belong to another aggregate, so `aggregate <Root>` must name the boundary's outermost class.",
        fix: "Name the outermost root, or remove the contradictory membership.",
    },
    Entry {
        code: E_INVALID_INVARIANT,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Invalid invariant",
        meaning: "`invalid invariant in design '<design>': <detail>` — the expression does not parse, does not type-check, is not boolean, or reads a repository/global name. An invariant is a state predicate over the designed class; cross-aggregate consistency belongs in a process manager or an operation guard.",
        fix: "Use a boolean expression over the class's own stored and derived features; handle optional navigation explicitly.",
    },
    Entry {
        code: E_DTO_INVARIANT,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Invariant on a DTO",
        meaning: "`design '<design>' is a dto; a transport shape carries no invariants` — invariants are consistency rules on a value or an aggregate, not on a transfer boundary.",
        fix: "Remove the invariant, or design the class that owns the state as an `entity`/`value`.",
    },
    Entry {
        code: E_DESIGN_FRAGMENT_ORPHAN,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Design fragment without a root",
        meaning: "`design fragment '<path>' has no root: no .ddd application imports it` — a `.ddd` file of top-level `module` blocks is a fragment, legal only when imported (directly or transitively) by exactly one `application` root.",
        fix: "Import the fragment from a root's `import \"<path>.ddd\";`, or give it its own `application` declaration to make it a root.",
    },
    Entry {
        code: E_DESIGN_FRAGMENT_MULTI_ROOT,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Design fragment under two roots",
        meaning: "`design fragment '<path>' is imported by more than one root: <roots>` — a design file belongs to exactly one application; sharing it would merge its modules into two artifacts.",
        fix: "Remove one root's import, or split the shared members into a fragment only one root imports.",
    },
    Entry {
        code: E_DESIGN_APPLICATION_NAME_DUPLICATE,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Duplicate design application name",
        meaning: "`duplicate design application name '<name>' in <roots>` — `application <Name>` is the graph's application id for a design application, so two roots in one checked tree may not share it.",
        fix: "Rename one of the applications; the authored name, not the file path, identifies the application.",
    },
    Entry {
        code: E_DESIGN_IMPORT_CYCLE,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Design import cycle",
        meaning: "`design import cycle through <path>` — `.ddd` imports form a cycle, so no root can merge the fragments into one application.",
        fix: "Break the cycle: a fragment may import only fragments that do not (transitively) import it.",
    },
    Entry {
        code: E_DESIGN_ROOT_AS_FRAGMENT,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Application root imported as a fragment",
        meaning: "`a design application root cannot be imported as a fragment; remove its application declaration or import its root instead` — a `.ddd` file with an `application` declaration is a root by definition, and roots do not compose.",
        fix: "Remove the imported file's `application` block (making it a fragment), or import the root's own entry file.",
    },
    Entry {
        code: E_DESIGN_FRAGMENT_MISSING,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Imported design fragment not provided",
        meaning: "`imported design fragment '<path>' was not provided` — a declared `.ddd` import the host could not read at the resolved path.",
        fix: "Fix the import path (it resolves relative to the importing file) or add the missing fragment file.",
    },
    Entry {
        code: E_QUERY_READ_ONLY,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Query operation is not read-only",
        meaning: "`query operation '<op>' of repository '<repo>' …` — a repository query carries a statement body, a `when` guard, or a `throws` clause. A query is a read surface: its predicate is `where`, it never raises a declared error, and it has no body to execute.",
        fix: "Remove the body/`when`/`throws`; express the selection in `where`, and use a declared repository operation (with a body) for a write.",
    },
    Entry {
        code: E_QUERY_PREDICATE,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Invalid query predicate",
        meaning: "`invalid query predicate in operation '<op>' of repository '<repo>': <detail>` — the `where` predicate does not parse, does not type-check as boolean over the designed class and the operation's parameters, or a parameter name collides with a feature of the designed class.",
        fix: "Write a boolean expression over the designed class's features (bare or `self.<feature>`) and the operation's parameters; rename a colliding parameter or read the feature as `self.<name>`.",
    },
    Entry {
        code: E_QUERY_SORT,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Invalid query sort key",
        meaning: "`invalid sort key '<field>' in operation '<op>' of repository '<repo>': <reason>` — a sort key must be a direct stored scalar feature (primitive, enum, datatype, vocabulary). A class reference would need a join, a collection has no stored scalar order, and a derived feature is not stored so it cannot be indexed.",
        fix: "Sort on a stored scalar feature of the designed class; project the referenced class's field into a DTO instead.",
    },
    Entry {
        code: E_QUERY_PAGE,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Invalid query pagination",
        meaning: "`invalid pagination in operation '<op>' of repository '<repo>': <detail>` — `limit` and `max` must be positive page sizes, and a `limit` above the declared `max` cap is refused.",
        fix: "Declare a positive `limit` no greater than the `max` cap (or drop `max` to let `limit` stand alone).",
    },
    Entry {
        code: E_DOCUMENT_REF,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Invalid document reference",
        meaning: "`invalid document reference '<name>' in search '<search>': <detail>` — `document DtoName;` must name a dto design declared in the search's own module whose boundary is a mirror or projection of a source class. An unknown name, a cross-module boundary, and a standalone (source-less) dto are each refused with the reason (DF3).",
        fix: "Declare the dto in the same module as a mirror (`dto Book`) or projection (`dto BookSummary from Book { ... }`), or use the inline `document { ... }` block.",
    },
    Entry {
        code: E_DOCUMENT_SOURCE_MISMATCH,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Document boundary projects another entity",
        meaning: "`document reference '<name>' of search '<search>' projects '<source>', but the search targets '<entity>'` — a named document shares the DTO boundary's shape, so its source class must be the search's entity (DF3).",
        fix: "Name a boundary projected from the search's entity, or use an inline `document { ... }` block.",
    },
    Entry {
        code: E_DOCUMENT_FORM_CONFLICT,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Conflicting search document forms",
        meaning: "`search '<search>' declares both an inline document block and a named document reference` — a search carries exactly one document shape (DF3).",
        fix: "Keep the reusable `document DtoName;` reference or the inline `document { ... }` block, not both.",
    },
    Entry {
        code: E_MACHINE_REF_NOT_PROVIDED,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Referenced machine file was not provided",
        meaning: "`referenced machine file '<path>' was not provided` — the `machine … from \"<path>\";` reference names a file the host did not supply: it does not exist relative to the declaring `.ddd` file, or the host could not read it (issue pherolang#388).",
        fix: "Create the file at the referenced path (relative to the `.ddd` file), or fix the written spelling.",
    },
    Entry {
        code: E_MACHINE_REF_PARSE,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Referenced machine file did not parse",
        meaning: "`referenced machine file '<path>' did not parse: <detail>` — the file is not a representable XState machine. The detail carries the parser's typed message verbatim: a TypeScript syntax error, or a fail-closed rejection (an inline guard function, a non-literal scalar, …) — the recommended convention is serialized guards, string or `{ type, params }` objects (issue pherolang#388).",
        fix: "Author the machine in the serialized XState v5 forms the parser represents; the detail names the construct to rewrite.",
    },
    Entry {
        code: E_MACHINE_REF_EMPTY,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Referenced machine file declares no machine",
        meaning: "`referenced machine file '<path>' declares no machine` — the file parsed as TypeScript but contains no `createMachine` call, so the reference binds nothing (issue pherolang#388).",
        fix: "Add a `createMachine({ id: …, initial: …, states: … })` to the file, or point the reference at the file that carries it.",
    },
    Entry {
        code: E_MACHINE_REF_SEVERAL,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Referenced machine file declares several machines",
        meaning: "`referenced machine file '<path>' declares several machines: '<id>', …` — v1 binds exactly one machine per referenced file, so a multi-machine file cannot say which machine the design advances (issue pherolang#388).",
        fix: "Split the machines into one file each (or defer to the future `as` clause once it exists).",
    },
    Entry {
        code: E_MACHINE_REF_NO_ID,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Referenced machine carries no stable id",
        meaning: "`referenced machine file '<path>' carries no stable machine id` — the authored `createMachine` declared no `id`, so the parser synthesized an index-derived placeholder; the id joins the machine's provenance (`machineId` in the artifact) and must be authored (issue pherolang#388).",
        fix: "Add a stable string `id` to the `createMachine` call.",
    },
    Entry {
        code: E_MACHINE_REF_SHAPE,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Referenced machine is outside the flat subset",
        meaning: "`referenced machine '<id>' declares <detail>` — the machine uses a construct the design mirror cannot carry: a nested (compound or parallel) state, a transition targeting several states, a transition with no target, or an automatic (`onDone`/`invoke`/`after`) transition. The mirror is flat and event-driven, closed over the bound enum (issue pherolang#388).",
        fix: "Flatten the machine's states, give every transition exactly one target, and drive it by events only.",
    },
];
