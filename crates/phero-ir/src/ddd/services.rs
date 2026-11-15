use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ddd::{CapabilityRef, DecisionRef, ErrorRef, EventRef, TargetRef};
use crate::wire::is_false;
use crate::{Multiplicity, TypeRef};

/// A signature type reference of a DDD operation (DD3, pherolang#300):
/// either a **resolved domain type** or a **DTO boundary name**.
///
/// **Untagged**, so the wire shape is the reference's own rather than a
/// wrapper's:
///
/// - [`DddTypeRef::Domain`] is exactly the shared [`TypeRef`] object
///   (`{"type":"class","value":{...}}`), which keeps every domain-only DDD
///   signature byte-identical to what it was before DD3.
/// - [`DddTypeRef::Dto`] is `{"dto":"BookSummary"}`.
///
/// A DTO reference is deliberately *not* a [`TypeRef`]: a DTO boundary need
/// not exist in the domain model at all (a standalone DTO has no `.mox`
/// source), so there is no package-qualified identity to resolve. The
/// boundary name matches the [`DtoShape::name`](crate::ddd::DtoShape::name)
/// of the design that defines it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DddTypeRef {
    /// A fully resolved domain type, exactly as the domain IR spells it.
    Domain(TypeRef),
    /// A DTO boundary, by the boundary name.
    Dto {
        /// The DTO boundary name.
        dto: String,
    },
}

impl DddTypeRef {
    /// Wraps a resolved domain type.
    pub fn domain(type_ref: TypeRef) -> Self {
        Self::Domain(type_ref)
    }

    /// References a DTO boundary by its boundary name.
    pub fn dto(name: impl Into<String>) -> Self {
        Self::Dto { dto: name.into() }
    }

    /// The wrapped domain type, or `None` for a DTO boundary.
    pub fn as_domain(&self) -> Option<&TypeRef> {
        match self {
            Self::Domain(type_ref) => Some(type_ref),
            Self::Dto { .. } => None,
        }
    }

    /// The DTO boundary name, or `None` for a domain type.
    pub fn dto_name(&self) -> Option<&str> {
        match self {
            Self::Domain(_) => None,
            Self::Dto { dto } => Some(dto),
        }
    }
}

/// A parameter of a DDD operation signature ([`ServiceOperation`] or
/// [`RepositoryOperation`](crate::ddd::RepositoryOperation)) (DD3,
/// pherolang#300).
///
/// The wire shape matches the shared [`crate::OperationParam`] for a
/// domain-typed parameter (`{"name": ..., "type": {...}}`), but the type
/// slot is a [`DddTypeRef`] so a parameter may also be DTO-typed
/// (`{"name": ..., "type": {"dto": "BookSummary"}}`). The domain IR's own
/// [`crate::OperationParam`] stays unchanged on purpose: a `.mox` operation
/// never references a DTO, so widening it would put a DDD-only concept into
/// the domain artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DddParam {
    /// Parameter name.
    pub name: String,
    /// The resolved parameter type, domain or DTO.
    #[serde(rename = "type")]
    pub type_: DddTypeRef,
    /// Declared cardinality. `None` is single-valued.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiplicity: Option<Multiplicity>,
}

impl DddParam {
    /// Creates a single-valued parameter of the given type.
    pub fn new(name: impl Into<String>, type_: DddTypeRef) -> Self {
        Self {
            name: name.into(),
            type_,
            multiplicity: None,
        }
    }

    /// Chainable setter for the declared cardinality.
    pub fn with_multiplicity(mut self, multiplicity: Option<Multiplicity>) -> Self {
        self.multiplicity = multiplicity;
        self
    }
}

/// An application service: stateless use-case orchestration over
/// repositories and other services.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    /// Service name.
    pub name: String,
    /// Human-readable description of the service's responsibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Service operations, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operations: Vec<ServiceOperation>,
    /// Declared `inject` dependencies: names of repositories and other
    /// services this service delegates to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    /// The resolved identities of [`Self::dependencies`] (DD1,
    /// pherolang#298), aligning index-for-index with it: entry `i` is the
    /// [`TargetRef`] the compiler resolved `dependencies[i]` to. Additive;
    /// omitted when the dependency list is empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependency_refs: Vec<TargetRef>,
    /// Injected decision-service references (`inject decision <name>;`;
    /// the [`crate::dmn::DmnModel`] dimension); loosely coupled, not
    /// validated at the wire layer — the `.ddd` decisions entry point
    /// joins them against the provided decision models.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decision_dependencies: Vec<String>,
    /// The resolved identities of [`Self::decision_dependencies`] (DD1,
    /// pherolang#298), aligning index-for-index with it: entry `i` is the
    /// [`DecisionRef`] the compiler resolved `decision_dependencies[i]` to.
    /// [`DecisionRef::contract`] and [`DecisionRef::version`] are `None`
    /// when the decision artifact was not supplied at compile time. Additive;
    /// omitted when the dependency list is empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decision_dependency_refs: Vec<DecisionRef>,
    /// The **version pins** of [`Self::decision_dependencies`] entries
    /// that carry one (pherolang#154): each names the service (the same
    /// last-segment spelling `decision_dependencies` lowers) and the
    /// [`crate::dmn::VersionPin`] the `inject decision` line wrote.
    /// Pin-less lines lower no entry — the additive-field discipline
    /// keeps every pin-less artifact byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decision_dependency_pins: Vec<DecisionPin>,
}

impl Service {
    /// Creates a service with no description, operations, or dependencies.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            operations: Vec::new(),
            dependencies: Vec::new(),
            dependency_refs: Vec::new(),
            decision_dependencies: Vec::new(),
            decision_dependency_refs: Vec::new(),
            decision_dependency_pins: Vec::new(),
        }
    }
}

/// One **version pin** a decision-link reference carries (pherolang#154):
/// the invoked service's name (the same last-segment spelling the
/// `invokes`/`decision_dependencies` lists lower) paired with the
/// [`crate::dmn::VersionPin`] the reference wrote. Only pinned
/// references serialize — the pin-less name lists carry them already.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionPin {
    /// The pinned service's name, as the resolution universe spells it.
    pub name: String,
    /// The declared compatibility contract.
    pub pin: crate::dmn::VersionPin,
}

/// An operation of a [`Service`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceOperation {
    /// Operation name, unique within its service.
    pub name: String,
    /// The resolved return type (DD3: a domain type or a DTO boundary).
    /// `None` for delegation operations whose signature is copied from the
    /// delegated target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_type: Option<DddTypeRef>,
    /// The declared return cardinality; `None` is single-valued. Paired
    /// with [`ServiceOperation::return_type`] — never set without it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_multiplicity: Option<Multiplicity>,
    /// Parameters in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<DddParam>,
    /// Delegation target, when this operation forwards to a dependency's
    /// operation instead of declaring its own body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delegation: Option<Delegation>,
    /// `true` for a `protected` operation: it stays off the public service
    /// interface (Sculptor's visibility). Additive; omitted when `false`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_protected: bool,
    /// Actor capability names guarding this operation (the
    /// [`crate::ActorModel`] dimension); loosely coupled, not validated at
    /// the wire layer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    /// The resolved identities of [`Self::capabilities`] (DD1,
    /// pherolang#298), aligning index-for-index with it: entry `i` is the
    /// [`CapabilityRef`] the compiler resolved `capabilities[i]` to.
    /// [`CapabilityRef::declared_in`] is `None` when the `.actor` policy was
    /// not supplied at compile time. Additive; omitted when the capability
    /// list is empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_refs: Vec<CapabilityRef>,
    /// Event names this operation emits (the `emits` clause; the
    /// [`crate::events::EventModel`] dimension); loosely coupled, not
    /// validated at the wire layer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub emits: Vec<String>,
    /// The resolved identities of [`Self::emits`] (DD1, pherolang#298),
    /// aligning index-for-index with it: entry `i` is the [`EventRef`] the
    /// compiler resolved `emits[i]` to. [`EventRef::contract`] is `None` when
    /// the `.evt` contract was not supplied at compile time. Additive;
    /// omitted when the emitted-event list is empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub emitted_refs: Vec<EventRef>,
    /// Decision-service names this operation invokes (the `invokes`
    /// clause; the [`crate::dmn::DmnModel`] dimension); loosely coupled,
    /// not validated at the wire layer — the `.ddd` decisions entry point
    /// joins them against the provided decision models.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invokes: Vec<String>,
    /// The resolved identities of [`Self::invokes`] (DD1, pherolang#298),
    /// aligning index-for-index with it: entry `i` is the [`DecisionRef`]
    /// the compiler resolved `invokes[i]` to. [`DecisionRef::contract`] and
    /// [`DecisionRef::version`] are `None` when the decision artifact was not
    /// supplied at compile time. Additive; omitted when the invoked-service
    /// list is empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decision_refs: Vec<DecisionRef>,
    /// The **version pins** of [`Self::invokes`] entries that carry one
    /// (pherolang#154): each names the invoked service (the same
    /// last-segment spelling `invokes` lowers) and the
    /// [`crate::dmn::VersionPin`] the entry wrote. Pin-less entries lower
    /// no entry — the additive-field discipline keeps every pin-less
    /// artifact byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decision_pins: Vec<DecisionPin>,
    /// The `when (...)` guard condition's source text, verbatim between the
    /// parentheses (the `when`-condition rule); typing is a driver concern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// The resolved field paths of the `when` condition (issue pherolang#180):
    /// every path whose resolution a consumer cannot re-derive from the
    /// artifact alone — injected-scope paths always (the repository→class
    /// binding is design knowledge), own-record paths only in a module with
    /// several machine-bearing designs (where the owning machine is not a
    /// priori derivable). A single-machine module's own-record guard lowers
    /// none. Resolved form, so consumers never re-parse [`Self::condition`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub condition_paths: Vec<ConditionPath>,
    /// The declared transition binding of a delegated operation (issue
    /// pherolang#176): `name => Repository.save transition <Trigger>` — the
    /// machine-bound transition the delegation fires, validated by the
    /// driver against the target repository's designed machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<TransitionRef>,
    /// The child-scoped target of a module-level `extend <Class>` operation
    /// (issue pherolang#179): `boolean addLine(PayRunLineDraft line) =>
    /// lines.add` lowers with the root's child feature (`lines`) and the
    /// resolved child class. `None` for every ordinary service operation
    /// (the additive-field discipline: absent from body-less artifacts, so
    /// existing goldens stay byte-identical).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_scope: Option<ChildRef>,
    /// The resolved error vocabulary the operation declares it can raise
    /// (DD7, issue pherolang#304): the `throws` clause's entries, resolved
    /// to their declaration scope. Explicit and complete — a body raise must
    /// be declared here, and a delegation's set is a superset of its
    /// target's. Additive; an operation with no clause omits the key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub throws: Vec<ErrorRef>,
    /// Checked executable bodies, keyed by target name (the
    /// [`crate::Operation::bodies`] discipline; issue pherolang#178). The
    /// `"stmt"` target holds the canonical JSON of the operation's
    /// `phero_expr::stmt::StmtBody` — phero-ir deliberately does not depend on
    /// phero-expr, so the value is opaque here. Additive; a body-less
    /// operation omits the key.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bodies: BTreeMap<String, String>,
    /// For a statement-body operation written `name(...) => Repository
    /// { ... }`: the `=>` repository bound as `self` in the body. Additive;
    /// absent from every operation without a body receiver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_receiver: Option<String>,
}

impl ServiceOperation {
    /// Creates an operation with no return type, parameters, delegation,
    /// capabilities, invoked decision services, emitted events, condition
    /// paths, transition binding, child scope, or body.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            return_type: None,
            return_multiplicity: None,
            params: Vec::new(),
            delegation: None,
            is_protected: false,
            capabilities: Vec::new(),
            capability_refs: Vec::new(),
            emits: Vec::new(),
            emitted_refs: Vec::new(),
            invokes: Vec::new(),
            decision_refs: Vec::new(),
            decision_pins: Vec::new(),
            condition: None,
            condition_paths: Vec::new(),
            transition: None,
            child_scope: None,
            throws: Vec::new(),
            bodies: BTreeMap::new(),
            body_receiver: None,
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

    /// Chainable setter for the emitted event names.
    pub fn with_emits(mut self, emits: Vec<String>) -> Self {
        self.emits = emits;
        self
    }

    /// Chainable setter for the invoked decision-service names.
    pub fn with_invokes(mut self, invokes: Vec<String>) -> Self {
        self.invokes = invokes;
        self
    }

    /// Chainable setter for the resolved condition paths (issue pherolang#180).
    pub fn with_condition_paths(mut self, paths: Vec<ConditionPath>) -> Self {
        self.condition_paths = paths;
        self
    }

    /// Chainable setter for the declared transition binding (issue
    /// pherolang#176).
    pub fn with_transition(mut self, transition: Option<TransitionRef>) -> Self {
        self.transition = transition;
        self
    }

    /// Chainable setter for the child-scoped target (issue pherolang#179).
    pub fn with_child_scope(mut self, child_scope: Option<ChildRef>) -> Self {
        self.child_scope = child_scope;
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

    /// Chainable setter for the `=>` body receiver bound as `self` (issue
    /// pherolang#178).
    pub fn with_body_receiver(mut self, body_receiver: Option<String>) -> Self {
        self.body_receiver = body_receiver;
        self
    }
}

/// The explicit transition binding of a delegated [`ServiceOperation`]
/// (issue pherolang#176): `submit => PayRunRepository.save transition Submit`
/// — or, in a multi-machine module, `transition PayRunStatus.Reverse`,
/// qualified by the machine's **enum type name** (pherolang machine
/// declarations are field-named — `machine status: PayRunStatus` — so a
/// machine-name qualifier does not exist as authored). The driver validates
/// the trigger against the machine bound to the delegation target's
/// repository design; the artifact keeps the names exactly as authored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionRef {
    /// The trigger name as written — an explicit `via` trigger or a bare
    /// arrow's derived name (the lowerCamel of the target state).
    pub trigger: String,
    /// The machine qualifier — the enum type name when the source wrote the
    /// qualified spelling (`PayRunStatus.Reverse`), `None` when unqualified
    /// (`Submit`). Required in a multi-machine module.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
}

impl TransitionRef {
    /// Creates an unqualified transition reference.
    pub fn new(trigger: impl Into<String>) -> Self {
        Self {
            trigger: trigger.into(),
            machine: None,
        }
    }

    /// Chainable setter for the machine qualifier.
    pub fn with_machine(mut self, machine: Option<String>) -> Self {
        self.machine = machine;
        self
    }
}

/// The child-scoped target of a module-level `extend <Class>` operation
/// (issue pherolang#179).
///
/// An `extend PayRun { boolean addLine(PayRunLineDraft line) => lines.add; }`
/// block declares root-side operations over the aggregate's contained
/// children; the lowered operation carries the root's child feature
/// ([`ChildRef::feature`], as authored), the child class that feature
/// resolves to ([`ChildRef::class`], package-qualified — the
/// [`Delegation::target`] convention), and the validated action
/// ([`ChildRef::action`] — `add`, `remove`, or `save`) so consumers can
/// distinguish the commands without re-reading the source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildRef {
    /// The root class's child feature name, exactly as declared in the
    /// domain model (`lines`).
    pub feature: String,
    /// The child class the feature resolves to, package-qualified
    /// (`nz.example.payroll.PayRunLine`).
    pub class: String,
    /// The validated child action: `"add"`, `"remove"`, or `"save"`.
    pub action: String,
}

impl ChildRef {
    /// Creates a child reference from the feature name, its resolved
    /// class's full name, and the validated action.
    pub fn new(
        feature: impl Into<String>,
        class: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            feature: feature.into(),
            class: class.into(),
            action: action.into(),
        }
    }
}

/// The resolved scope of one [`ConditionPath`] (issue pherolang#180).
///
/// Adjacently tagged: `{"type":"ownRecord"}` for a field of the op's bound
/// entity, `{"type":"injected","value":{"repository":...,"class":...}}` for
/// a field reached through an injected repository (the path's first segment
/// is the injection name as declared; `class` is the repository design's
/// class).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum ConditionScope {
    /// A field of the op's own record (the bound entity).
    OwnRecord,
    /// A field reached through an injected repository.
    #[serde(rename_all = "camelCase")]
    Injected {
        /// The injection name exactly as declared (`inject TimecardRepository;`).
        repository: String,
        /// The class the repository design binds.
        class: String,
    },
}

/// One resolved field path of a [`ServiceOperation`]'s `when` condition
/// (issue pherolang#180). The scope records where the path resolves, the
/// field the dotted remainder (single segments in the common case), and the
/// field's declared type — the lowering contract consumers build on, so
/// they never re-parse the condition source string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionPath {
    /// Where the path resolves.
    pub scope: ConditionScope,
    /// The field path — one segment (`status`) or a dotted chain
    /// (`holder.name`) under the scope's root.
    pub field: String,
    /// The field's declared type.
    pub field_type: TypeRef,
}

impl ConditionPath {
    /// Creates a condition path.
    pub fn new(scope: ConditionScope, field: impl Into<String>, field_type: TypeRef) -> Self {
        Self {
            scope,
            field: field.into(),
            field_type,
        }
    }
}

/// Delegation of a [`ServiceOperation`] to a dependency's operation: the
/// service copies the target's signature rather than declaring its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Delegation {
    /// Name of the injected dependency (repository or service) receiving
    /// the call.
    pub target: String,
    /// The resolved identity of [`Self::target`] (DD1, pherolang#298): what
    /// the injected name actually resolves to, when the compiler could bind
    /// it. Additive; absent when unresolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<TargetRef>,
    /// Name of the operation invoked on the target.
    pub operation: String,
}
