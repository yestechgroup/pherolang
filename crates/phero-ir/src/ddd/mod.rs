//! The DDD design artifact: application, modules, application services,
//! per-class design decisions (Sculptor-style), and search definitions,
//! produced by the `.ddd` design DSL and ingested by downstream consumers
//! exactly like [`crate::ifml::IfmlModel`].
//!
//! This is a standalone, versioned wire artifact in the [pherolang] family,
//! versioned independently of [`crate::FORMAT_VERSION`] via
//! [`DDD_MODEL_FORMAT_VERSION`] — the [`crate::ActorModel`] precedent (wire
//! contract rule 10). The [wire format contract](crate#wire-format-contract)
//! applies unchanged, under the ActorModel (not the IFML) empty-model
//! discipline:
//!
//! - **Casing.** Struct fields serialize as `camelCase` (`formatVersion`,
//!   `returnType`, `optimisticLocking`, `deepImmutable`, ...). Unit-only
//!   enums serialize as bare strings: [`Stereotype`] lowercase
//!   (`"entity" | "value" | "dto"`), [`IdentityPolicy`] / [`EqualityPolicy`]
//!   lowercase (`"none"`, `"structural"`), [`BuiltinRepositoryOp`] camelCase
//!   (`"findById"`, `"findAll"`, `"findByExample"`, `"findByKeys"`,
//!   `"save"`, `"delete"`). The one payload-bearing enum,
//!   [`RankingStrategy`], is adjacently tagged instead (`{"type":
//!   "tfIdf"}`, `{"type": "custom", "value": "..."}`).
//! - **Version gate.** [`DddModel::from_json`] rejects any other
//!   `formatVersion` (absent reports as 0) with
//!   [`crate::IrError::UnsupportedFormatVersion`] rather than guessing.
//! - **Provenance, not a gate.** [`DddModel::phero_version`] records the
//!   phero-ir version that wrote the artifact — the [`crate::Model`]
//!   precedent. Readers gate on `formatVersion` alone: an artifact without
//!   `pheroVersion` (written before the field existed) deserializes
//!   unchanged.
//! - **Empty = minimal.** Every optional field is `#[serde(default)]` and
//!   omitted when absent/empty (`skip_serializing_if`), so an empty artifact
//!   serializes as exactly
//!   `{"formatVersion":2,"pheroVersion":"<crate version>"}` — the version
//!   marker plus the provenance line, byte-identical no matter what is added
//!   later (beyond a workspace version bump).
//! - **Forward compatibility.** Unknown JSON fields are ignored on
//!   deserialize; every field added after v1 must follow the same
//!   default + skip discipline.
//! - **Resolved identities.** The resolved companion of each loose name
//!   reference — [`Design::type_ref`], [`Delegation::target_ref`],
//!   [`Service::dependency_refs`] / [`Service::decision_dependency_refs`],
//!   [`ServiceOperation::capability_refs`] / [`ServiceOperation::emitted_refs`]
//!   / [`ServiceOperation::decision_refs`], [`SearchDef::entity_ref`] /
//!   [`SearchDef::capability_refs`], and the search members' `type_ref` —
//!   records the identity the compiler actually
//!   resolved, without replacing the authored name (DD1, pherolang#298).
//!   Each list aligns index-for-index with the name list it accompanies, and
//!   its optional provenance members (`declared_in`, `contract`, `version`)
//!   stay `None` when the counterpart artifact was not supplied at compile
//!   time.
//! - **Shared IR types.** Signatures reuse the domain IR's [`crate::TypeRef`]
//!   (adjacent tagging: `{"type": "class", "value": {...}}`) and
//!   [`crate::OperationParam`] (`{"name": ..., "type": ...}`); there are no
//!   parallel wire-level signature types. A [`StateMachine`]'s enum is the
//!   same [`crate::TypeRef::Enum`].
//! - **State machines.** A [`Design`] may carry a [`StateMachine`] (`machine`
//!   block). The state set is derived — `initial` + `terminals` + transition
//!   endpoints — never serialized as a separate list; terminalness is
//!   membership in `terminals`. A [`Transition`]'s `trigger` is the `via`
//!   workflow action (`None` = bare arrow, action name derived downstream),
//!   and its `guard`/an operation's `condition` are the `when`-condition
//!   rule: verbatim source text between the parentheses, typed by the
//!   driver.
//! - **Value semantics.** A `value`-stereotyped [`Design`] carries the fixed
//!   [`ValueSemantics`] block in [`Design::semantics`] — identity `"none"`,
//!   `immutable`, `deepImmutable`, structural equality — with **no opt-out**
//!   (DD2, pherolang#299). Entity and dto designs omit the key entirely. The
//!   strict rules (no `id` feature, no contained entity, no entity reference,
//!   no `abstract value`) are driver errors, so an artifact that exists
//!   states the semantics its consumer must honor.
//! - **Child-scoped operations.** A [`Design`] may carry child operations
//!   ([`Design::child_operations`]) declared by module-level `extend
//!   <Class>` blocks (issue pherolang#179): operations over the aggregate's
//!   contained children, each naming the child feature, its resolved
//!   class, and the validated action through
//!   [`ServiceOperation::child_scope`] ([`ChildRef`]).
//! - **Executable bodies.** An operation may carry a checked statement body
//!   (issue pherolang#178): the [`ServiceOperation::bodies`] /
//!   [`RepositoryOperation::bodies`] map's `"stmt"` target holds the
//!   canonical JSON of the `phero_expr::stmt::StmtBody` (opaque here — the
//!   crate does not depend on phero-expr, the [`crate::Operation::bodies`]
//!   precedent), and a service body written `name(...) => Repository { ... }`
//!   also records that repository as [`ServiceOperation::body_receiver`]
//!   (the binding `self` names in the body).
//! - **Equality policy.** Like every artifact root, [`DddModel`] derives
//!   [`Eq`] (wire contract rule 14): the one floating-point field in any
//!   artifact — [`SearchField::boost`] — is wrapped in the ordered
//!   [`Boost`] newtype, transparent on the wire and total in equality,
//!   hashing, and ordering.
//!
//! [`Design::class`] names the referenced `.mox` class (unqualified or
//! package-qualified at this layer); resolution against the domain
//! [`crate::Model`] is a driver/consumer concern, never a wire concern.
//! Likewise [`ServiceOperation::capabilities`] loosely references actor
//! capability names (the [`crate::ActorModel`] dimension),
//! [`ServiceOperation::emits`] loosely references event names (the
//! [`crate::events::EventModel`] dimension), and
//! [`ServiceOperation::invokes`] / [`Service::decision_dependencies`]
//! loosely reference decision-service names (the [`crate::dmn::DmnModel`]
//! dimension); none are validated at the wire layer — the `.ddd`
//! decisions entry point joins the design against the provided decision
//! models at compile time. A reference may carry a **version pin**
//! (pherolang#154, [`crate::dmn::VersionPin`]): pinned references lower
//! into [`ServiceOperation::decision_pins`] /
//! [`Service::decision_dependency_pins`], each a [`DecisionPin`] pairing
//! the name with its compatibility contract.
//!
//! A [`RepositoryOperation`] carries **exactly one** of a built-in op
//! ([`RepositoryOperation::builtin`] — the consumer knows its signature) or
//! a declared signature ([`RepositoryOperation::declared`] — a return type
//! and/or parameters). Consistent with the other artifacts this invariant is
//! not validated on deserialize; the constructors make the invalid states
//! hard to author.
//!
//! [pherolang]: https://github.com/yestechgroup/pherolang

use serde::{Deserialize, Serialize};

mod application;
mod designs;
mod references;
mod searches;
mod services;

pub use application::{Application, Module};
pub use designs::{
    BuiltinRepositoryOp, Design, DesignFlags, EqualityPolicy, IdentityPolicy, Repository,
    RepositoryOperation, StateMachine, Stereotype, Transition, ValueSemantics,
};
pub use references::{CapabilityRef, DecisionRef, EventRef, TargetKind, TargetRef};
pub use searches::{
    Boost, DocumentField, Pagination, RankingStrategy, SearchDef, SearchField, SearchFilter,
    SearchSort,
};
pub use services::{
    ChildRef, ConditionPath, ConditionScope, DecisionPin, Delegation, Service, ServiceOperation,
    TransitionRef,
};

/// The artifact format version this crate writes and accepts for DDD design
/// artifacts ([`DddModel`]).
///
/// Bump whenever the DDD wire format changes incompatibly; this marker is
/// versioned independently of every other artifact marker — see the
/// [version-marker inventory](crate#version-markers).
pub const DDD_MODEL_FORMAT_VERSION: u32 = 2;

/// The root of a DDD design artifact: one application with its modules of
/// application services, class designs, and search definitions.
///
/// See the [wire format contract](crate#wire-format-contract) and the
/// [module docs](self).
///
/// Every artifact type derives [`Eq`] (wire contract rule 14): this one's
/// only floating-point field — [`SearchField::boost`] — is wrapped in the
/// ordered [`Boost`] newtype, so value equality, hashing, and ordering are
/// total and deterministic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DddModel {
    /// Artifact format version. Always [`DDD_MODEL_FORMAT_VERSION`] for
    /// artifacts this crate writes; [`DddModel::from_json`] rejects
    /// anything else.
    pub format_version: u32,
    /// Version of the phero-ir crate that produced this artifact (provenance
    /// only; not part of equality semantics for consumers that must not
    /// care). The version gate reads `formatVersion` alone, so an artifact
    /// without this field still deserializes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phero_version: Option<String>,
    /// The designed application, when the artifact declares one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application: Option<Application>,
    /// Modules in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<Module>,
}

impl DddModel {
    /// Creates an empty design artifact with [`DDD_MODEL_FORMAT_VERSION`]
    /// stamped and the current crate version recorded as
    /// [`DddModel::phero_version`].
    pub fn new() -> Self {
        Self {
            format_version: DDD_MODEL_FORMAT_VERSION,
            phero_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            application: None,
            modules: Vec::new(),
        }
    }

    /// Chainable setter installing the application.
    pub fn application(mut self, application: Application) -> Self {
        self.application = Some(application);
        self
    }

    /// Chainable setter appending a module.
    pub fn module(mut self, module: Module) -> Self {
        self.modules.push(module);
        self
    }
}

crate::wire::versioned_artifact!(DddModel, DDD_MODEL_FORMAT_VERSION, "the design artifact");

#[cfg(test)]
mod tests;
