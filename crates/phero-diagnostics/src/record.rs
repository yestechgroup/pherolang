//! The one diagnostic record every surface emits.
//!
//! Before SK2 (epic pherolang#328) each surface family carried its own
//! shape: `phero-driver`'s record with an `Option<DiagnosticCode>` payload
//! and `.ifml`'s own record with a `&'static str` code, a `(usize, usize)`
//! span, and its own `file` string — which forced a field-by-field
//! conversion at every host (the CLI reporter, the LSP publish path).
//! [`Diagnostic`] is the single record they both use now;
//! [`Severity`] was already the registry's one severity
//! class and stays the only one in the workspace.
//!
//! # The code carrier (ADR 0005, option (a))
//!
//! The record is generic over its code payload, because the code cannot
//! move into this crate: `phero-driver` depends on `phero-diagnostics`
//! (the registry), so naming `phero_driver::DiagnosticCode` here would be
//! a cycle ([ADR 0005](https://github.com/yestechgroup/pherolang/blob/main/docs/adr/0005-shared-surface-kernel-layering.md),
//! Consequences, "SK2 — record and code carrier"). So the record owns
//! severity, message, span and help — everything every surface spells
//! identically — and `C` carries the surface's own code:
//!
//! - `phero_driver::Diagnostic` is `Diagnostic<DiagnosticCode>`: the
//!   payload-carrying camelCase codes, serializable for LSP quick fixes.
//! - `.ifml` results carry `Diagnostic<&'static str>`: the registry's
//!   stable numeric code (`E0002`), still the string the CLI prints and the
//!   LSP publishes.
//!
//! A surface that emits no code payload spells `Diagnostic<()>` (the
//! default).
//!
//! # The file tag (ADR 0005, option (c), declined)
//!
//! The file a diagnostic belongs to stays **outside** the record: every
//! multi-file surface tags its results `(path, Diagnostic<_>)` — the shape
//! the driver's compilation structs already used, now shared by `.ifml`
//! too. The record holds a byte span only, so the span's coordinate system
//! is unambiguous: it indexes the source the tag names, never a record
//! field that could disagree with it.

use phero_parser::Span;

use crate::Severity;

/// A single compiler diagnostic: a severity, a message, an optional span,
/// an optional help hint, and an optional machine-readable code.
///
/// `C` is the surface's code payload — see the module docs for why it is a
/// parameter and not a field of this crate's own vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic<C = ()> {
    /// How severe the diagnostic is.
    pub severity: Severity,
    /// Human-readable description of the problem.
    pub message: String,
    /// Byte span the diagnostic refers to, if any. The span indexes the
    /// source the result's `(path, Diagnostic)` tag names.
    pub span: Option<Span>,
    /// Optional actionable hint shown below the report.
    pub help: Option<String>,
    /// Optional machine-readable code for structured tooling (LSP quick
    /// fixes and stable printing). Absent unless explicitly attached.
    pub code: Option<C>,
}

impl<C> Diagnostic<C> {
    /// Creates an error diagnostic.
    pub fn error(message: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            span,
            help: None,
            code: None,
        }
    }

    /// Creates a warning diagnostic.
    pub fn warning(message: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            span,
            help: None,
            code: None,
        }
    }

    /// Attaches a help hint to this diagnostic.
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Attaches a machine-readable code to this diagnostic.
    pub fn with_code(mut self, code: C) -> Self {
        self.code = Some(code);
        self
    }

    /// `true` when the severity is [`Severity::Error`].
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}
