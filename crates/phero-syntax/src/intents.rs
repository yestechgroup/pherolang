//! Single source of truth for the `.intent` specification DSL's contextual
//! keywords.
//!
//! Every `.intent` contextual keyword is an ordinary identifier that is
//! special only in a particular grammar position (the `.ddd` discipline —
//! the `id`/`readonly` precedent), so two consumers must agree on the same
//! word list: the parser (which matches them positionally) and the formatter
//! (which recognizes them to dispatch its scans). This module is the one
//! table both consume. All words are escapable with `^`.
//!
//! One more word of the grammar is *not* listed here because it is a real
//! lexer keyword: `import` ([`Token::Import`] in `crate::lexer`). The link
//! kind words after `element`/`check` (`class`, `capability`, `event`,
//! `view`, `command`, `op`, `invariant`, `decision`) are deliberately **not**
//! in this table: they are an open, driver-validated vocabulary in plain
//! name position, never grammar-special (the `.deploy` setting-value rule).

/// The `intent <Id> "<statement>" { ... }` declaration keyword.
pub(crate) const INTENT: &str = "intent";
/// The `element <kind>:<name>;` link keyword: a model element that expresses
/// the intent.
pub(crate) const ELEMENT: &str = "element";
/// The `check <kind>:<name>;` link keyword: a model-native check that
/// verifies the intent.
pub(crate) const CHECK: &str = "check";
