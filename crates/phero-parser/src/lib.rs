//! Shared parser substrate for every pherolang surface.
//!
//! This crate owns the parser infrastructure that is genuinely agnostic of
//! any one surface: the byte-offset [`Span`] type, [`Spanned`], the
//! token-slice [`Tokens`] input, the [`ParseError`] shape, token-agnostic
//! recovery/combination helpers, the grammar-doc test harness, and the
//! bracket-depth scanner.
//!
//! **No surface grammars live here.** Surface lexemes, AST declarations,
//! keyword tables, and parser entry points stay in their own crates
//! (`phero-syntax`, `phero-expr`, `phero-ifml`). If a helper needs to know a
//! surface's token variants, it belongs to that surface.
//!
//! # Byte spans and empty ranges
//!
//! [`Span`] is always a byte range into the original source text. The
//! [`Tokens`] input exists because chumsky's built-in `MappedInput` maps an
//! *empty* cursor range to the end-of-input offset; parsers that report
//! zero-width positions (missing tokens, empty declarations) need the true
//! cursor location instead. [`Tokens`] therefore reports an empty range as an
//! empty span at the start token's byte offset, and the EOF offset only when
//! the cursor really is at the end of the input.
//!
//! # The unsafe exception
//!
//! chumsky 0.10's [`chumsky::input::Input`] trait declares required `unsafe fn`s;
//! [`input`] implements them exactly as documented (cursors come from
//! [`chumsky::input::Input::begin`], are never shared across inputs, and the token
//! slice is immutable). There are no other unsafe blocks in this crate, and
//! every other workspace member keeps the workspace-level
//! `unsafe_code = "forbid"` lint.

use chumsky::span::SimpleSpan;

pub mod combinators;
pub mod grammar_doc;
pub mod input;
pub mod nesting;

pub use combinators::{chain, junk_until, kw, list_separated};
pub use input::Tokens;

/// Byte-offset span into a source text.
pub type Span = SimpleSpan<usize>;

/// A value paired with the span it was written at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    /// The value.
    pub value: T,
    /// Span of the value in the source.
    pub span: Span,
}

/// A parse error with a span and a rendered message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message} at byte offsets {span}")]
pub struct ParseError {
    /// Human-readable description of the error.
    pub message: String,
    /// Byte range the error refers to.
    pub span: Span,
}
