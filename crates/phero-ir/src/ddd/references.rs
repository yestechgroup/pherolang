use serde::{Deserialize, Serialize};

/// The kind of a resolved [`TargetRef`]: the declaration space the target
/// lives in.
///
/// Serializes as a bare lowercase string: `"service"`, `"repository"`,
/// `"search"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TargetKind {
    /// An application [`Service`](crate::ddd::Service).
    Service,
    /// A designed [`Repository`](crate::ddd::Repository).
    Repository,
    /// A [`SearchDef`](crate::ddd::SearchDef).
    Search,
}

/// A fully resolved target identity: what a loose name reference
/// ([`Service::dependencies`](crate::ddd::Service::dependencies),
/// [`Delegation::target`](crate::ddd::Delegation::target)) actually resolved to
/// at compile time (DD1, pherolang#298).
///
/// The authored name stays on the referring struct; this type is the
/// resolution beside it, so consumers need not re-derive the binding from
/// the artifact's module structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetRef {
    /// The declaration space the target lives in.
    pub kind: TargetKind,
    /// The target's declaration identity — `<Application>.<module>.<Name>`
    /// (or `<module>.<Name>` when the design declares no application), the
    /// same name the semantic graph gives the declaration
    /// (`Library.catalogue.LoanService`).
    pub qualified_name: String,
    /// The module that declares the target.
    pub module: String,
}

impl TargetRef {
    /// Creates a resolved target reference from its kind, declaration
    /// identity, and declaring module.
    pub fn new(
        kind: TargetKind,
        qualified_name: impl Into<String>,
        module: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            qualified_name: qualified_name.into(),
            module: module.into(),
        }
    }
}

/// A resolved actor-capability reference: the identity behind one
/// [`ServiceOperation::capabilities`](crate::ddd::ServiceOperation::capabilities)
/// name (DD1, pherolang#298).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityRef {
    /// The capability name, as the resolution universe spells it.
    pub name: String,
    /// The name of the actor block that declares the capability, when an
    /// `.actor` artifact was supplied at compile time (the first declaring
    /// block when several do).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_in: Option<String>,
}

impl CapabilityRef {
    /// Creates a capability reference with no known declaring policy.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            declared_in: None,
        }
    }

    /// Chainable setter for the declaring actor block.
    pub fn with_declared_in(mut self, declared_in: Option<String>) -> Self {
        self.declared_in = declared_in;
        self
    }
}

/// A resolved event reference: the identity behind one
/// [`ServiceOperation::emits`](crate::ddd::ServiceOperation::emits) name
/// (DD1, pherolang#298).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRef {
    /// The event name, as the resolution universe spells it.
    pub name: String,
    /// The identity of the `.evt` contract that declares the event, when
    /// the contract was supplied at compile time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
}

impl EventRef {
    /// Creates an event reference with no known declaring contract.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            contract: None,
        }
    }

    /// Chainable setter for the declaring contract's identity.
    pub fn with_contract(mut self, contract: Option<String>) -> Self {
        self.contract = contract;
        self
    }
}

/// A resolved decision reference: the identity behind one
/// [`ServiceOperation::invokes`](crate::ddd::ServiceOperation::invokes) /
/// [`Service::decision_dependencies`](crate::ddd::Service::decision_dependencies)
/// name (DD1, pherolang#298).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionRef {
    /// The decision service's name, as the resolution universe spells it
    /// (the same last-segment spelling the name lists lower).
    pub service: String,
    /// The identity of the `.dmn` artifact that declares the service, when
    /// the decision model was supplied at compile time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
    /// The resolved version of the invoked service, when the reference
    /// joined a supplied decision model carrying one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl DecisionRef {
    /// Creates a decision reference with no known declaring contract or
    /// resolved version.
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
            contract: None,
            version: None,
        }
    }

    /// Chainable setter for the declaring artifact's identity.
    pub fn with_contract(mut self, contract: Option<String>) -> Self {
        self.contract = contract;
        self
    }

    /// Chainable setter for the resolved version.
    pub fn with_version(mut self, version: Option<String>) -> Self {
        self.version = version;
        self
    }
}
