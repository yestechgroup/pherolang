use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ddd::ServiceOperation;
use crate::wire::is_false;
use crate::{Multiplicity, OperationParam, TypeRef};

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

/// The design decision for one referenced `.mox` class: its stereotype,
/// flags, and optionally its repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Design {
    /// The referenced `.mox` class name (unqualified or package-qualified
    /// at this layer; resolution is a driver/consumer concern).
    pub class: String,
    /// The resolved identity of [`Self::class`] (DD1, pherolang#298): the
    /// class the name binds to, when the domain model was supplied at compile
    /// time. Additive; absent when unresolved.
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
    /// Creates a concrete design with default flags, no resolved class
    /// reference, no value semantics, no repository, no machine, and no
    /// child operations.
    pub fn new(class: impl Into<String>, stereotype: Stereotype) -> Self {
        Self {
            class: class.into(),
            type_ref: None,
            stereotype,
            is_abstract: false,
            flags: DesignFlags::default(),
            semantics: None,
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

/// An operation of a [`Repository`]: either a built-in (whose signature the
/// consumer knows) or a declared operation with an explicit signature.
///
/// **Invariant:** exactly one of [`RepositoryOperation::builtin`] or a
/// declared signature ([`RepositoryOperation::return_type`] and/or
/// [`RepositoryOperation::params`]) is present. Not validated on
/// deserialize (consistent with the other artifacts); use the
/// [`RepositoryOperation::builtin`] / [`RepositoryOperation::declared`]
/// constructors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryOperation {
    /// Operation name, unique within its repository.
    pub name: String,
    /// The built-in op, when this is one; a built-in has no signature of
    /// its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builtin: Option<BuiltinRepositoryOp>,
    /// The declared return type, when this is a declared operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TypeRef>,
    /// The declared return cardinality; `None` is single-valued. Paired
    /// with [`RepositoryOperation::return_type`] — never set without it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_multiplicity: Option<Multiplicity>,
    /// Declared parameters in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<OperationParam>,
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
    /// Checked executable bodies, keyed by target name (the
    /// [`crate::Operation::bodies`] discipline; issue pherolang#178). The
    /// `"stmt"` target holds the canonical JSON of the operation's
    /// `phero_expr::stmt::StmtBody` — opaque here. Additive; a body-less
    /// operation omits the key. A repository body binds `self` to the
    /// operation's own repository design, so no receiver is recorded.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bodies: BTreeMap<String, String>,
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
            bodies: BTreeMap::new(),
        }
    }

    /// Creates a declared repository operation with its explicit signature
    /// (at least one of a return type or a parameter).
    pub fn declared(
        name: impl Into<String>,
        return_type: Option<TypeRef>,
        params: Vec<OperationParam>,
    ) -> Self {
        Self {
            name: name.into(),
            builtin: None,
            return_type,
            return_multiplicity: None,
            params,
            is_protected: false,
            condition: None,
            bodies: BTreeMap::new(),
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

    /// Chainable setter for the checked executable bodies (issue
    /// pherolang#178).
    pub fn with_bodies(mut self, bodies: BTreeMap<String, String>) -> Self {
        self.bodies = bodies;
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
