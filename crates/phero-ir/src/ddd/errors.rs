//! The declared error vocabulary of a DDD design artifact (DD7, issue
//! pherolang#304): the `error` declarations at application and module scope
//! and the resolved `throws` references of the operations.
//!
//! The vocabulary is **sparse**: a design with no declaration emits no
//! `errors` key, and an operation with no `throws` clause emits none, so a
//! vocabulary-less artifact stays byte-identical (the wire contract's
//! additive-field discipline).
//!
//! The standard prelude — `NotFound(resource: string, key: string)` and
//! `Conflict(detail: string)` — is **implicit**: it is never emitted in the
//! artifact's `errors` lists, and a `throws` reference names it with
//! [`ErrorScope::Prelude`]. Consumers provide its fixed shapes; the
//! artifact-only contract refuses a declaration that shadows one.

use serde::{Deserialize, Serialize};

use crate::ddd::DddParam;

/// One declared error of the design's vocabulary: the name and its typed
/// payload fields, in declaration order.
///
/// ```json
/// {"name":"PaymentDeclined",
///  "fields":[{"name":"reason","type":{"type":"class","value":{...}}},
///            {"name":"reference","type":{"type":"primitive","value":"string"}}]}
/// ```
///
/// A declaration with an empty payload serializes as `{"name":"Empty"}` —
/// the `description` and `fields` keys are omitted. An application-level
/// declaration rides [`crate::ddd::DddModel::errors`]; a module-level one rides
/// [`Module::errors`](crate::ddd::Module::errors) and is private to that
/// module. Error names are application-wide unique and may not shadow a
/// prelude error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDecl {
    /// The error name (application-wide unique, never a prelude name).
    pub name: String,
    /// Human-readable description, from the declaration's doc comment.
    /// Additive; omitted when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Payload fields in declaration order. Additive; omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<DddParam>,
}

impl ErrorDecl {
    /// Creates a payload-less error declaration.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            fields: Vec::new(),
        }
    }

    /// Chainable setter for the description (the declaration's doc comment).
    pub fn with_description(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }

    /// Chainable setter for the payload fields.
    pub fn with_fields(mut self, fields: Vec<DddParam>) -> Self {
        self.fields = fields;
        self
    }
}

/// The visibility scope of a declared error (DD7).
///
/// Serializes as a bare lowercase string: `"application"`, `"module"`, or
/// `"prelude"`. Application-scoped errors are visible everywhere in the
/// application; a module-scoped error is visible only inside its module
/// (and a cross-module reference is a compile error naming the fix); the
/// prelude is always available and never declared in the artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorScope {
    /// Declared at application scope: visible to every module.
    Application,
    /// Declared at module scope: private to that module.
    Module,
    /// A standard prelude error (`NotFound`, `Conflict`), always available
    /// and never emitted in the artifact's declarations.
    Prelude,
}

/// One resolved `throws` reference of an operation: the error name and the
/// scope the compiler resolved it in.
///
/// Serialized shape:
/// `{"name":"PaymentDeclined","scope":"application"}` — with `module` set
/// (`"module":"billing"`) exactly when the scope is
/// [`ErrorScope::Module`] (the declaring module), and omitted for the
/// application and prelude scopes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorRef {
    /// The error name exactly as declared (or the prelude's).
    pub name: String,
    /// The scope the name resolved in.
    pub scope: ErrorScope,
    /// The declaring module, exactly when [`Self::scope`] is
    /// [`ErrorScope::Module`]. Additive; omitted otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
}

impl ErrorRef {
    /// References an application-scoped error.
    pub fn application(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            scope: ErrorScope::Application,
            module: None,
        }
    }

    /// References an error private to `module`.
    pub fn module(name: impl Into<String>, module: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            scope: ErrorScope::Module,
            module: Some(module.into()),
        }
    }

    /// References a standard prelude error.
    pub fn prelude(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            scope: ErrorScope::Prelude,
            module: None,
        }
    }
}
