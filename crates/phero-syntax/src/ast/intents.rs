//! The `.intent` intent-specification DSL AST (issue pherolang#196).

use super::*;

// --- .intent intent specification DSL ----------------------------------------

/// The root node of a parsed `.intent` source: `import` declarations
/// followed by `intent` declarations. Imports must precede the first intent
/// (a later `import` is a syntax error); duplicate imports are a driver
/// concern, not a syntax error. An empty import or intent list is legal.
#[derive(Debug, Clone, PartialEq)]
pub struct IntentFile {
    /// The `import` declarations in source order.
    pub imports: Vec<Import>,
    /// Declared intents, in source order.
    pub intents: Vec<IntentDecl>,
}

/// An `intent <Id> "<statement>" { ... }` declaration: one requirement
/// statement with a stable id, linked to the model elements that express it
/// (`element` links) and the model-native checks that verify it (`check`
/// links). The id must match the driver's stable-id shape (`LOAN-001` — an
/// uppercase lead segment plus at least one all-digits segment); its text
/// is sliced verbatim from the source (hyphens never survive lexing as one
/// token), and the statement is the string literal's unquoted text.
#[derive(Debug, Clone, PartialEq)]
pub struct IntentDecl {
    /// The stable requirement id as written (`LOAN-001`). The text is the
    /// verbatim source slice over [`Name::span`], filled by
    /// [`parse_intents`](crate::parse_intents) (the grammar parses the
    /// hyphen-separated segments token-wise, so the written spelling —
    /// leading zeros included — survives exactly).
    pub id: Name,
    /// The requirement statement: the string literal's unquoted text.
    pub statement: String,
    /// Span of the statement string literal as written, quotes included.
    pub statement_span: Span,
    /// The declared links in source order (`element` and `check` lines
    /// interleaved as written; the driver splits them by slot).
    pub links: Vec<IntentLink>,
    /// Span of the whole declaration, `intent` keyword included.
    pub span: Span,
}

/// One `element <kind>:<name>;` / `check <kind>:<name>;` link line of an
/// [`IntentDecl`]. The slot records which keyword introduced the line; the
/// kind names the resolution vocabulary (`class`, `capability`, `event`,
/// `view`, `command`, `op` for elements; `invariant`, `decision` for
/// checks) — a closed set the driver validates, never the grammar. The name
/// is the dotted path as written; resolution happens in the project-level
/// coverage pass, never in the grammar or the artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct IntentLink {
    /// Which keyword introduced the line.
    pub slot: IntentLinkSlot,
    /// The link kind word as written (`class`, `decision`, ...). The driver
    /// validates it against the slot's closed vocabulary.
    pub kind: Name,
    /// The dotted link name as written (`RefundRequest`,
    /// `nz.example.support.RefundRequest`).
    pub name: String,
    /// Span of the dotted name as written.
    pub name_span: Span,
    /// Span of the whole link line, `;` included.
    pub span: Span,
}

/// The slot an [`IntentLink`] fills: an expressing `element` link or a
/// verifying `check` link. The coverage pass requires every intent to carry
/// at least one *resolved* link of each slot — a missing one is the
/// unmapped-intent warning, never an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentLinkSlot {
    /// `element <kind>:<name>;` — a model element that expresses the intent.
    Element,
    /// `check <kind>:<name>;` — a model-native check that verifies it.
    Check,
}
