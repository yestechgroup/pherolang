//! The `driver.ddd` band: `E0350`–`E0399`, the phero-driver `.ddd` lifecycle
//! and aggregate-operation checks (issues pherolang#176, pherolang#179, and
//! pherolang#180 — the service-operation transition bindings, the
//! child-scoped `extend` operations, and the `when`-condition scoping over
//! `crates/phero-driver/src/ddd/`). Every check prefixes its code onto the
//! diagnostic message (`E0350 unknown transition trigger …`), the
//! `dmn.check` band's E0250/E0251 precedent. E0350–E0352 are the
//! transition-binding checks; E0353–E0354 are the condition-scope checks;
//! E0355–E0358 are the `extend` child-operation checks; E0359–E0360 are the
//! executable operation-body checks (issue pherolang#178); E0361–E0364 are
//! the strict value-semantics checks (issue pherolang#299: R-V1, R-V2, R-V3,
//! and the `abstract value` rejection); E0365–E0372 are the DTO-shape checks
//! (issue pherolang#300: the dto-op error, member duplication/unknown/type,
//! entity members, non-input parameters, name collisions, and entity-only
//! constructs on a DTO).
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
/// `unknown child feature '<feature>' on '<class>'` — an `extend` block's
/// child operation names a feature the extended class does not carry (or
/// one that is not a containment/reference, so it has no child aggregate
/// to act on).
pub const E_UNKNOWN_CHILD_FEATURE: &str = "E0355";
/// `child operation '<op>' in extend '<class>': action '<action>' requires
/// a collection` — `add` and `remove` operate on the elements of a
/// many-valued child feature; a single-valued containment takes only
/// `save`.
pub const E_CHILD_ACTION_NEEDS_COLLECTION: &str = "E0356";
/// `unknown child action '<action>' in child operation '<op>'` — the
/// child-operation action is one of the closed set `add`, `remove`,
/// `save`.
pub const E_UNKNOWN_CHILD_ACTION: &str = "E0357";
/// `unknown extend target '<class>'` — the `extend` block's class has no
/// entity design that is an aggregate root (a class declared in the
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
        meaning: "`unknown child feature '<feature>' on '<class>'` (or `child feature '<feature>' of '<class>' is an attribute`) — an `extend` block's child operation names a feature the extended class does not carry, or one that is not a containment or cross reference (an attribute has no child aggregate to act on). `extend` operates on the aggregate's contained children, so only `contains` and `refers` features are eligible.",
        fix: "Name a containment or reference feature of the extended class (the help lists the eligible ones), or extend the class that actually holds the child.",
    },
    Entry {
        code: E_CHILD_ACTION_NEEDS_COLLECTION,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Child action needs a collection",
        meaning: "`child operation '<op>' in extend '<class>': action '<action>' requires a collection; feature '<feature>' is single-valued` — `add` and `remove` act on the elements of a many-valued child feature, so a single-valued containment (a scalar child object) admits only `save`.",
        fix: "Use `save` for a single-valued child, or declare the feature as a many-valued containment (`contains Child[] children`).",
    },
    Entry {
        code: E_UNKNOWN_CHILD_ACTION,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Unknown child action",
        meaning: "`unknown child action '<action>' in child operation '<op>' of extend '<class>'` — the action after the child feature's dot is one of the closed vocabulary `add`, `remove`, `save`: add a new child, remove an existing one, or persist (insert/replace) it. The action selects the aggregate-internal operation the child-scoped operation performs, so an unknown word is refused rather than silently ignored (the lowered artifact carries the operation's signature and its `childScope`).",
        fix: "Write `add`, `remove`, or `save` as the action.",
    },
    Entry {
        code: E_UNKNOWN_EXTEND_TARGET,
        band: DDD_BAND,
        severity: Severity::Error,
        title: "Unknown extend target",
        meaning: "`unknown extend target '<class>'` — the `extend` block's class must be a domain class with an entity design in the same module that is an aggregate root (a repository-bearing entity, validation rule 6). The message names which part failed: the class resolves against the imported domains, the design must live in the extending module, and the design must carry a repository (a contained, repository-less child is not a root).",
        fix: "Extend an aggregate root designed in the same module (add its `repository` block if it is missing), or move the `extend` block next to the root's design.",
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
];
