use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ddd::{CapabilityRef, DddParam, DddTypeRef, ErrorRef, ServiceOperation};
use crate::wire::is_false;
use crate::{Multiplicity, TypeRef};

/// The DDD stereotype of a designed class.
///
/// Serializes as a bare lowercase string: `"entity"`, `"value"`, `"dto"`.
/// Abstractness is the separate [`Design::is_abstract`] flag, not a
/// stereotype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stereotype {
    /// An aggregate-root/characteristic class with identity.
    Entity,
    /// An immutable value object.
    Value,
    /// A data transfer object.
    Dto,
}

/// The design flags of a [`Design`], all defaulting to `false`. A
/// fully-default flags value is omitted entirely on serialize.
///
/// The bools carry the **effective** decision, not the raw spelling: the
/// design DSL defaults `auditable` and `optimisticLocking` on for entity
/// designs (Sculptor's polarity, opted out with `!auditable`), so a plain
/// `entity Book` design serializes both as `true` while a `value`/`dto`
/// design leaves them absent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignFlags {
    /// Generate scaffolding for the class.
    #[serde(default, skip_serializing_if = "is_false")]
    pub scaffold: bool,
    /// Record an audit trail for the class's mutations. Defaults to `true`
    /// for entity designs; `!auditable` opts out.
    #[serde(default, skip_serializing_if = "is_false")]
    pub auditable: bool,
    /// Optimistic locking on the class's persistent state. Defaults to
    /// `true` for entity designs; `!optimisticLocking` opts out.
    #[serde(default, skip_serializing_if = "is_false")]
    pub optimistic_locking: bool,
    /// The class is never persisted.
    #[serde(default, skip_serializing_if = "is_false")]
    pub non_persistent: bool,
    /// Cache instances of the class.
    #[serde(default, skip_serializing_if = "is_false")]
    pub cache: bool,
}

impl DesignFlags {
    /// `true` when no flag is set — the value that serializes as an omitted
    /// `flags` field.
    pub fn is_empty(&self) -> bool {
        !(self.scaffold
            || self.auditable
            || self.optimistic_locking
            || self.non_persistent
            || self.cache)
    }
}

/// The identity policy a value design carries. Strict value semantics admit
/// exactly one: [`IdentityPolicy::None`] — a value object has no identity.
///
/// Serializes as the bare lowercase string `"none"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IdentityPolicy {
    /// No identity: equality and lifetime derive from the value's structure,
    /// never from a feature (a value object carries no `id`).
    None,
}

/// The equality policy a value design carries. Strict value semantics admit
/// exactly one: [`EqualityPolicy::Structural`] — two instances are equal when
/// their stored features are equal.
///
/// Serializes as the bare lowercase string `"structural"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EqualityPolicy {
    /// Structural equality over the value's stored features.
    Structural,
}

/// The effective, non-optional value semantics of a `value`-stereotyped
/// [`Design`] (DD2, pherolang#299).
///
/// A value design is strictly immutable — there is **no opt-out** (no
/// `!immutable` spelling, no flag that relaxes it) — so every value design
/// the compiler lowers carries exactly [`ValueSemantics::strict`]:
///
/// ```json
/// {"identity":"none","immutable":true,"deepImmutable":true,
///  "equality":"structural"}
/// ```
///
/// Only `value` designs carry the block: [`Design::semantics`] is `None` on
/// every `entity`/`dto` design and the key is omitted entirely there.
/// Consumers may rely on the values being fixed — the artifact contract
/// (`phero-backend-ddd`) refuses any other block and refuses one on an
/// entity/dto design.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueSemantics {
    /// The identity policy — always [`IdentityPolicy::None`].
    pub identity: IdentityPolicy,
    /// Feature values cannot be reassigned after construction. Always
    /// `true`; a value design has no opt-out.
    pub immutable: bool,
    /// Every transitively reached value is immutable too — a value contains
    /// values, never mutable entities (wire key `deepImmutable`). Always
    /// `true`.
    pub deep_immutable: bool,
    /// The equality policy — always [`EqualityPolicy::Structural`].
    pub equality: EqualityPolicy,
}

impl ValueSemantics {
    /// The one strict value-semantics block every `value` design carries.
    pub fn strict() -> Self {
        Self {
            identity: IdentityPolicy::None,
            immutable: true,
            deep_immutable: true,
            equality: EqualityPolicy::Structural,
        }
    }
}

/// The direction of a DTO boundary (DD3, pherolang#300): whether the
/// boundary carries data **into** a use case ([`DtoDirection::Input`]) or
/// **out of** one ([`DtoDirection::Output`]).
///
/// Serializes as the bare lowercase string `"input"` / `"output"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DtoDirection {
    /// An inbound boundary: the caller supplies the data.
    Input,
    /// An outbound boundary: the use case returns the data.
    Output,
}

/// How a [`DtoMember`] is populated on the DTO boundary (DD3,
/// pherolang#300).
///
/// Serializes as the bare lowercase string `"select"` / `"derive"` /
/// `"declared"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DtoMemberKind {
    /// A selected source feature, copied verbatim — the member's
    /// [`DtoMember::path`] names it.
    Select,
    /// A derived value computed from the source — the member's
    /// [`DtoMember::expression`] is the verbatim derivation source text.
    Derive,
    /// A member declared only on the boundary, with no source counterpart.
    Declared,
}

/// The shape of a DTO boundary design ([`Design::dto_shape`]) (DD3,
/// pherolang#300): what crosses a layer boundary — no identity, no
/// behaviour, and possibly no `.mox` source class at all.
///
/// [`DtoShape::name`] is the **boundary** name (the name signatures use in
/// a [`DddTypeRef::Dto`]), while [`DtoShape::source`] is the resolved domain
/// class the boundary is built from — `None` for a standalone DTO, whose
/// [`Design::class`] and [`Design::type_ref`] are `None` too.
///
/// Serialized shape:
///
/// ```json
/// {"name":"BookSummary",
///  "source":{"type":"class","value":{"package":"nz.example.library","name":"Book"}},
///  "direction":"output",
///  "members":[{"name":"title","kind":"select",
///              "type":{"type":"primitive","value":"string"},"path":["title"]}]}
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DtoShape {
    /// The DTO boundary name — the name a [`DddTypeRef::Dto`] references.
    pub name: String,
    /// The resolved domain class the boundary is built from. `None` for a
    /// standalone DTO (one with no `.mox` source class).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<TypeRef>,
    /// The direction of the boundary.
    pub direction: DtoDirection,
    /// The boundary's members, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<DtoMember>,
}

impl DtoShape {
    /// Creates a member-less DTO shape with no resolved source class.
    pub fn new(name: impl Into<String>, direction: DtoDirection) -> Self {
        Self {
            name: name.into(),
            source: None,
            direction,
            members: Vec::new(),
        }
    }

    /// Chainable setter for the resolved source class.
    pub fn with_source(mut self, source: Option<TypeRef>) -> Self {
        self.source = source;
        self
    }

    /// Chainable setter for the direction.
    pub fn with_direction(mut self, direction: DtoDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Chainable setter for the boundary's members.
    pub fn with_members(mut self, members: Vec<DtoMember>) -> Self {
        self.members = members;
        self
    }
}

/// One member of a [`DtoShape`] (DD3, pherolang#300): a selected source
/// feature, a derived value, or a member declared only on the boundary.
///
/// The payload each kind uses is additive — [`DtoMember::path`] on a
/// selection, [`DtoMember::expression`] on a derivation, neither on a
/// declared member — and every unused payload is omitted entirely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DtoMember {
    /// Member name as it appears on the boundary.
    pub name: String,
    /// How the member is populated.
    pub kind: DtoMemberKind,
    /// The member's type — a domain type or another DTO boundary.
    #[serde(rename = "type")]
    pub type_: DddTypeRef,
    /// Declared cardinality. `None` is single-valued.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiplicity: Option<Multiplicity>,
    /// The selected source feature path, for [`DtoMemberKind::Select`]
    /// (`["title"]`); a dotted chain for a nested feature. Omitted for
    /// every other kind.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<String>,
    /// The derivation's source text, verbatim, for [`DtoMemberKind::Derive`]
    /// — the same "verbatim expression text" rule as an operation
    /// `condition`; typing is a driver concern. Omitted for every other
    /// kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
}

impl DtoMember {
    /// Creates a single-valued member with no path or expression payload.
    pub fn new(name: impl Into<String>, kind: DtoMemberKind, type_: DddTypeRef) -> Self {
        Self {
            name: name.into(),
            kind,
            type_,
            multiplicity: None,
            path: Vec::new(),
            expression: None,
        }
    }

    /// Chainable setter for the declared cardinality.
    pub fn with_multiplicity(mut self, multiplicity: Option<Multiplicity>) -> Self {
        self.multiplicity = multiplicity;
        self
    }

    /// Chainable setter for the selected source feature path.
    pub fn with_path(mut self, path: Vec<String>) -> Self {
        self.path = path;
        self
    }

    /// Chainable setter for the derivation's verbatim source text.
    pub fn with_expression(mut self, expression: Option<String>) -> Self {
        self.expression = expression;
        self
    }
}

/// How an entity design's aggregate boundary was decided (DD4, issue
/// pherolang#301): derived from the domain model's containment graph, or
/// authored with `root`/`aggregate <Root>`.
///
/// Serializes as the bare lowercase string `"derived"` / `"authored"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AggregateOrigin {
    /// The containment closure decided the boundary.
    Derived,
    /// The design authored the boundary.
    Authored,
}

/// The aggregate membership of a non-root entity design (DD4): the resolved
/// root class and how the placement was decided. A root design omits the
/// field entirely; `None` on `Design::aggregate` means this class is a root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateMembership {
    /// The resolved aggregate root class.
    pub root: TypeRef,
    /// Whether containment or an authored declaration placed the member.
    pub origin: AggregateOrigin,
}

/// One invariant of a design (DD5, issue pherolang#302): the verbatim source
/// text between `invariant` and `;`. Typing is proven at compile time; the
/// consumer re-parses with the shared expression semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Invariant {
    /// The invariant expression, verbatim.
    pub expression: String,
}

impl Invariant {
    /// Creates one invariant from its verbatim source text.
    pub fn new(expression: impl Into<String>) -> Self {
        Self {
            expression: expression.into(),
        }
    }
}

/// The design decision for one class or DTO boundary: its stereotype, flags,
/// and optionally its repository.
///
/// A design either targets a `.mox` class — [`Design::class`] names it,
/// [`Design::type_ref`] resolves it — or, for a **standalone DTO** (DD3,
/// pherolang#300), both are `None` and the boundary is described entirely by
/// [`Design::dto_shape`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Design {
    /// The referenced `.mox` class name, exactly as authored (unqualified
    /// or package-qualified at this layer); its resolved identity rides in
    /// [`Self::type_ref`], which is absent for a standalone DTO. The
    /// authored spelling stays for round-tripping and diagnostics. `None`
    /// for a standalone DTO, which has no source class; the key is omitted
    /// entirely there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    /// The resolved identity of [`Self::class`] (DD1, pherolang#298): the
    /// class the name binds to, when the domain model was supplied at compile
    /// time. Additive; absent when unresolved or for a standalone DTO (whose
    /// resolved source, if any, rides in
    /// [`DtoShape::source`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_ref: Option<TypeRef>,
    /// The DDD stereotype of the class.
    pub stereotype: Stereotype,
    /// Whether the class is abstract.
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_abstract: bool,
    /// Design flags; omitted entirely when fully default.
    #[serde(default, skip_serializing_if = "DesignFlags::is_empty")]
    pub flags: DesignFlags,
    /// The effective value semantics (DD2, pherolang#299), present on a
    /// `value` design and absent on every `entity`/`dto` design. The
    /// compiler lowers the fixed [`ValueSemantics::strict`] block; the
    /// block is a decision, never author input (there is no `.ddd`
    /// spelling for it). Additive; omitted when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics: Option<ValueSemantics>,
    /// The DTO boundary shape (DD3, pherolang#300), present on a `dto`
    /// design and absent on every `entity`/`value` design. Additive;
    /// omitted when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dto_shape: Option<DtoShape>,
    /// The effective aggregate-root verdict (DD4, pherolang#301): `true` for
    /// a root, `false` for a member, **absent when unknown** (the imported
    /// domain union did not fully lower). Entity designs only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_aggregate_root: Option<bool>,
    /// The resolved aggregate membership of a **non-root** entity design,
    /// with the decision's origin. Absent on roots and non-entities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregate: Option<AggregateMembership>,
    /// The design's invariants (DD5, pherolang#302), in declaration order.
    /// Sparse: designs without invariants stay byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invariants: Vec<Invariant>,
    /// The class's repository, when designed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<Repository>,
    /// The class's lifecycle state machine, when designed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<StateMachine>,
    /// Child-scoped operations declared by the module's `extend <Class>`
    /// blocks (issue pherolang#179), in declaration order: root-side
    /// operations over the aggregate's contained children. Each carries a
    /// [`ChildRef`](crate::ddd::ChildRef) in [`ServiceOperation::child_scope`]. Additive; an
    /// extend-less design omits the key (the byte-identical discipline).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub child_operations: Vec<ServiceOperation>,
}

impl Design {
    /// Creates a concrete design targeting the named `.mox` class, with
    /// default flags, no resolved class reference, no value semantics, no
    /// DTO shape, no repository, no machine, and no child operations.
    pub fn new(class: impl Into<String>, stereotype: Stereotype) -> Self {
        Self {
            class: Some(class.into()),
            type_ref: None,
            stereotype,
            is_abstract: false,
            flags: DesignFlags::default(),
            semantics: None,
            dto_shape: None,
            is_aggregate_root: None,
            aggregate: None,
            invariants: Vec::new(),
            repository: None,
            machine: None,
            child_operations: Vec::new(),
        }
    }

    /// Creates a **standalone DTO** design (DD3, pherolang#300): a DTO
    /// boundary with no `.mox` source class, so [`Self::class`] and
    /// [`Self::type_ref`] are `None` and the boundary is described by a
    /// member-less [`DtoShape`] carrying the boundary name.
    ///
    /// The shape starts [`DtoDirection::Output`] (the read-model default);
    /// install the real shape — members, source, direction — with
    /// [`Self::with_dto_shape`].
    pub fn standalone_dto(name: impl Into<String>) -> Self {
        Self {
            class: None,
            type_ref: None,
            stereotype: Stereotype::Dto,
            is_abstract: false,
            flags: DesignFlags::default(),
            semantics: None,
            dto_shape: Some(DtoShape::new(name, DtoDirection::Output)),
            is_aggregate_root: None,
            aggregate: None,
            invariants: Vec::new(),
            repository: None,
            machine: None,
            child_operations: Vec::new(),
        }
    }

    /// Chainable setter for the value semantics block (DD2, pherolang#299).
    /// The driver sets `Some(ValueSemantics::strict())` on value designs and
    /// leaves every other design `None`.
    pub fn with_semantics(mut self, semantics: Option<ValueSemantics>) -> Self {
        self.semantics = semantics;
        self
    }

    /// Chainable setter for the DTO boundary shape (DD3, pherolang#300).
    pub fn with_dto_shape(mut self, dto_shape: Option<DtoShape>) -> Self {
        self.dto_shape = dto_shape;
        self
    }

    /// Chainable setter for the child-scoped operations (issue pherolang#179).
    pub fn with_child_operations(mut self, child_operations: Vec<ServiceOperation>) -> Self {
        self.child_operations = child_operations;
        self
    }
}

/// The repository designed for a [`Design`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    /// Repository name.
    pub name: String,
    /// Repository operations, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operations: Vec<RepositoryOperation>,
}

impl Repository {
    /// Creates a repository with no operations.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            operations: Vec::new(),
        }
    }
}

/// The lifecycle state machine of a [`Design`]: a `machine <field>: <Enum>`
/// block over one enum-typed feature of the designed class.
///
/// The state set is the union of [`StateMachine::initial`], the
/// [`StateMachine::terminals`], and every transition endpoint; consumers
/// derive it (the artifact never repeats a bare list). Whether a state is
/// terminal is carried by the [`StateMachine::terminals`] list, not by a
/// per-state flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateMachine {
    /// The designed class's feature carrying the state. The driver binds the
    /// machine to it (the feature must exist and be enum-typed); the wire
    /// layer carries the name loosely.
    pub field: String,
    /// The enum the states are drawn from.
    #[serde(rename = "enumType")]
    pub enum_type: TypeRef,
    /// The initial state.
    pub initial: String,
    /// Terminal states, in declaration order. A terminal state takes no
    /// outgoing transitions; the driver validates the machine before
    /// lowering, so consumers may rely on it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terminals: Vec<String>,
    /// Transitions in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<Transition>,
}

impl StateMachine {
    /// Creates a state machine with no terminals or transitions.
    pub fn new(field: impl Into<String>, enum_type: TypeRef, initial: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            enum_type,
            initial: initial.into(),
            terminals: Vec::new(),
            transitions: Vec::new(),
        }
    }

    /// Every state of the machine, in canonical order: the initial state
    /// first, then the terminals in declaration order, then every remaining
    /// transition endpoint in first-appearance order (duplicates removed).
    pub fn states(&self) -> Vec<&str> {
        let mut states = vec![self.initial.as_str()];
        for terminal in &self.terminals {
            if !states.contains(&terminal.as_str()) {
                states.push(terminal);
            }
        }
        for transition in &self.transitions {
            for endpoint in [&transition.from, &transition.to] {
                if !states.contains(&endpoint.as_str()) {
                    states.push(endpoint);
                }
            }
        }
        states
    }
}

/// One transition of a [`StateMachine`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transition {
    /// The source state name.
    pub from: String,
    /// The target state name.
    pub to: String,
    /// The `via` trigger — the workflow action performed through this
    /// transition. `None` for a bare arrow, whose action name is derived
    /// (the lowerCamel of the target state; documented in `docs/DDD.md`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
    /// The guard condition's source text, verbatim between the parentheses
    /// (the `when`-condition rule). Parsing, typing, and evaluation are
    /// driver concerns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard: Option<String>,
}

impl Transition {
    /// Creates an unguarded transition.
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            trigger: None,
            guard: None,
        }
    }
}

/// The direction one [`QuerySortKey`] orders by (DD9).
///
/// Serializes as the bare lowercase string: `"asc"`, `"desc"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    /// Ascending order.
    Asc,
    /// Descending order.
    Desc,
}

/// One sort key of a [`QuerySpec`] (DD9): the order-by field, its resolved
/// domain type, and the direction.
///
/// The sort key is deliberately **typed** — [`QuerySortKey::field_type`] is
/// the resolved [`TypeRef`] of the field the driver typed the key against —
/// so a consumer can build the ordering without re-resolving the name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuerySortKey {
    /// The order-by field name.
    pub field: String,
    /// The resolved type of [`QuerySortKey::field`].
    pub field_type: TypeRef,
    /// The ordering direction.
    pub direction: SortDirection,
}

impl QuerySortKey {
    /// Creates one typed sort key.
    pub fn new(field: impl Into<String>, field_type: TypeRef, direction: SortDirection) -> Self {
        Self {
            field: field.into(),
            field_type,
            direction,
        }
    }
}

/// The pagination window of a [`QuerySpec`] (DD9): the requested page size
/// (`limit`), the caller-visible ceiling (`max`), and whether the query is
/// cursor-paged.
///
/// Every member is additive — an absent `limit`/`max` is omitted entirely —
/// except [`QueryPagination::cursor`], which is serialized whenever the
/// block itself is present (`"cursor": false` included), so a reader never
/// has to infer the paging style from an absent key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryPagination {
    /// The requested page size. Omitted when the query declares none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// The maximum page size a caller may request. Omitted when the query
    /// declares none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u32>,
    /// Whether the query is cursor-paged. Serialized whenever the block is
    /// present — `"cursor": false` included — so a reader never has to infer
    /// the paging style from an absent key.
    #[serde(default)]
    pub cursor: bool,
}

impl QueryPagination {
    /// Creates an unlimited, offset-paged window.
    pub fn new() -> Self {
        Self::default()
    }

    /// Chainable setter for the requested page size.
    pub fn with_limit(mut self, limit: Option<u32>) -> Self {
        self.limit = limit;
        self
    }

    /// Chainable setter for the maximum page size.
    pub fn with_max(mut self, max: Option<u32>) -> Self {
        self.max = max;
        self
    }

    /// Chainable setter for cursor paging.
    pub fn with_cursor(mut self, cursor: bool) -> Self {
        self.cursor = cursor;
        self
    }
}

/// The read-only query shape of a [`RepositoryOperation`] (DD9): a typed
/// predicate over the repository's aggregate with optional ordering and
/// pagination.
///
/// A query is **always** a read: [`QuerySpec::read_only`] is serialized
/// under the `readOnly` key on every query (never skipped) and the compiler
/// sets it `true`. The artifact contract refuses a query that is not
/// read-only, and refuses a query carrying an executable body, a `when`
/// condition, or a `throws` clause.
///
/// Serialized shape:
///
/// ```json
/// {"predicate":"customer == self.customer and self.status == state",
///  "sort":[{"field":"createdDate",
///           "fieldType":{"type":"primitive","value":"date"},
///           "direction":"desc"}],
///  "pagination":{"limit":20,"max":100,"cursor":false},
///  "readOnly":true}
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuerySpec {
    /// The typed predicate source text, verbatim — the same
    /// "verbatim expression text" rule as an operation `condition`; typing
    /// is a driver concern. Always present and never empty.
    pub predicate: String,
    /// The order-by keys, in declaration order. Additive; omitted when the
    /// query declares no ordering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sort: Vec<QuerySortKey>,
    /// The pagination window. Additive; omitted when the query declares
    /// none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pagination: Option<QueryPagination>,
    /// Whether the query is read-only. Always serialized (never skipped);
    /// the compiler sets `true` — a query is a read. An artifact written
    /// without the key reads as `false`, which the contract refuses.
    #[serde(default)]
    pub read_only: bool,
}

impl QuerySpec {
    /// Creates a read-only query with no ordering and no pagination — a
    /// query is a read, so [`QuerySpec::read_only`] starts `true`.
    pub fn new(predicate: impl Into<String>) -> Self {
        Self {
            predicate: predicate.into(),
            sort: Vec::new(),
            pagination: None,
            read_only: true,
        }
    }

    /// Chainable setter for the order-by keys.
    pub fn with_sort(mut self, sort: Vec<QuerySortKey>) -> Self {
        self.sort = sort;
        self
    }

    /// Chainable setter for the pagination window.
    pub fn with_pagination(mut self, pagination: Option<QueryPagination>) -> Self {
        self.pagination = pagination;
        self
    }

    /// Chainable setter for the read-only flag.
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }
}

/// An operation of a [`Repository`]: either a built-in (whose signature the
/// consumer knows) or a declared operation with an explicit signature.
///
/// **Invariant:** exactly one of [`RepositoryOperation::builtin`] or a
/// declared signature ([`RepositoryOperation::return_type`] and/or
/// [`RepositoryOperation::params`]) is present. Not validated on
/// deserialize (consistent with the other artifacts); use the
/// [`RepositoryOperation::builtin`] / [`RepositoryOperation::declared`]
/// constructors.
///
/// A declared operation may additionally carry a read-only
/// [`QuerySpec`] (DD9): `query` and its guarding `capabilities` are
/// additive and omitted entirely on every non-query operation, so existing
/// artifacts stay byte-identical.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryOperation {
    /// Operation name, unique within its repository.
    pub name: String,
    /// The built-in op, when this is one; a built-in has no signature of
    /// its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builtin: Option<BuiltinRepositoryOp>,
    /// The declared return type, when this is a declared operation (DD3: a
    /// domain type or a DTO boundary).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_type: Option<DddTypeRef>,
    /// The declared return cardinality; `None` is single-valued. Paired
    /// with [`RepositoryOperation::return_type`] — never set without it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_multiplicity: Option<Multiplicity>,
    /// Declared parameters in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<DddParam>,
    /// `true` for a `protected` operation: it stays off the public
    /// repository interface (Sculptor's visibility). Additive; omitted when
    /// `false`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_protected: bool,
    /// The `when (...)` guard condition's source text, verbatim between the
    /// parentheses (the `when`-condition rule); typing is a driver concern.
    /// A built-in never carries one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// The resolved error vocabulary the declared operation can raise (DD7,
    /// issue pherolang#304). A built-in takes no clause (its failures are
    /// structural, not declared), so the key never rides a built-in.
    /// Additive; omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub throws: Vec<ErrorRef>,
    /// Checked executable bodies, keyed by target name (the
    /// [`crate::Operation::bodies`] discipline; issue pherolang#178). The
    /// `"stmt"` target holds the canonical JSON of the operation's
    /// `phero_expr::stmt::StmtBody` — opaque here. Additive; a body-less
    /// operation omits the key. A repository body binds `self` to the
    /// operation's own repository design, so no receiver is recorded.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bodies: BTreeMap<String, String>,
    /// The read-only query shape (DD9), present on a query operation and
    /// absent on every other operation. Additive; omitted when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<QuerySpec>,
    /// The resolved actor-capability references guarding the query (DD9),
    /// aligned index-for-index with the authored capability names. Legal
    /// only on a query operation. Additive; omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<CapabilityRef>,
}

impl RepositoryOperation {
    /// Creates a built-in repository operation (no signature of its own).
    pub fn builtin(name: impl Into<String>, builtin: BuiltinRepositoryOp) -> Self {
        Self {
            name: name.into(),
            builtin: Some(builtin),
            return_type: None,
            return_multiplicity: None,
            params: Vec::new(),
            is_protected: false,
            condition: None,
            throws: Vec::new(),
            bodies: BTreeMap::new(),
            query: None,
            capabilities: Vec::new(),
        }
    }

    /// Creates a declared repository operation with its explicit signature
    /// (at least one of a return type or a parameter).
    pub fn declared(
        name: impl Into<String>,
        return_type: Option<DddTypeRef>,
        params: Vec<DddParam>,
    ) -> Self {
        Self {
            name: name.into(),
            builtin: None,
            return_type,
            return_multiplicity: None,
            params,
            is_protected: false,
            condition: None,
            throws: Vec::new(),
            bodies: BTreeMap::new(),
            query: None,
            capabilities: Vec::new(),
        }
    }

    /// Chainable setter for the declared return cardinality.
    pub fn with_return_multiplicity(mut self, multiplicity: Option<Multiplicity>) -> Self {
        self.return_multiplicity = multiplicity;
        self
    }

    /// Chainable setter marking the operation `protected`.
    pub fn with_protected(mut self, is_protected: bool) -> Self {
        self.is_protected = is_protected;
        self
    }

    /// Chainable setter installing the `when (...)` condition's source text.
    pub fn with_condition(mut self, condition: Option<String>) -> Self {
        self.condition = condition;
        self
    }

    /// Chainable setter for the resolved error vocabulary (DD7, issue
    /// pherolang#304).
    pub fn with_throws(mut self, throws: Vec<ErrorRef>) -> Self {
        self.throws = throws;
        self
    }

    /// Chainable setter for the checked executable bodies (issue
    /// pherolang#178).
    pub fn with_bodies(mut self, bodies: BTreeMap<String, String>) -> Self {
        self.bodies = bodies;
        self
    }

    /// Chainable setter for the read-only query shape (DD9).
    pub fn with_query(mut self, query: Option<QuerySpec>) -> Self {
        self.query = query;
        self
    }

    /// Chainable setter for the query's resolved capabilities (DD9).
    pub fn with_capabilities(mut self, capabilities: Vec<CapabilityRef>) -> Self {
        self.capabilities = capabilities;
        self
    }
}

/// The built-in repository operations. Serialized as bare camelCase
/// strings: `"findById"`, `"findAll"`, `"findByExample"`, `"findByKeys"`,
/// `"save"`, `"delete"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BuiltinRepositoryOp {
    /// Fetch one aggregate by identity.
    FindById,
    /// Fetch all aggregates.
    FindAll,
    /// Fetch the aggregates matching an example instance.
    FindByExample,
    /// Fetch the aggregates by their natural keys.
    FindByKeys,
    /// Insert or update an aggregate.
    Save,
    /// Delete an aggregate.
    Delete,
}
