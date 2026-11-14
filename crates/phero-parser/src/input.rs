//! The generic token-slice parser input.
//!
//! [`Tokens`] pairs a borrowed token slice with its per-token byte [`Span`]s
//! and implements chumsky's [`Input`], [`ValueInput`], and
//! [`ExactSizeInput`] traits. It is the shared replacement for the private
//! implementations `phero-syntax` and (later) `phero-ifml` used to carry.

use std::ops::{Range, RangeFrom};

use chumsky::input::{ExactSizeInput, ValueInput};
use chumsky::prelude::*;

use crate::Span;

/// Parser input: a token slice paired with a custom [`chumsky::input::Input`]
/// implementation that reports **byte-offset** spans (chumsky's built-in slice
/// input would report token indices instead).
///
/// Generic in the token type so every surface shares one implementation.
#[derive(Debug)]
pub struct Tokens<'src, T> {
    tokens: &'src [(T, Span)],
}

// Hand-written so the markers never require `T: Copy`/`T: Clone`: the field
// is a shared slice, which is trivially copyable regardless of `T`.
impl<T> Clone for Tokens<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Tokens<'_, T> {}

impl<'src, T> Tokens<'src, T> {
    /// Build an input over `tokens`, whose spans are byte ranges into the
    /// original source text.
    pub fn new(tokens: &'src [(T, Span)]) -> Self {
        Tokens { tokens }
    }

    /// Byte offset just past the end of the last token (0 for empty input).
    pub fn eof_offset(&self) -> usize {
        self.tokens.last().map(|(_, span)| span.end).unwrap_or(0)
    }

    /// Convert a token-index range into a byte span. An empty range maps to an
    /// empty span at the start offset of the token at `start` (or the end of
    /// the input when at EOF) — never to the EOI offset through a zero-width
    /// range, which is the `MappedInput` quirk this type exists to avoid.
    pub fn byte_span(&self, start: usize, end: usize) -> Span {
        let start_byte = match self.tokens.get(start) {
            Some((_, span)) => span.start,
            None => self.eof_offset(),
        };
        let end_byte = if end > start {
            self.tokens
                .get(end - 1)
                .map(|(_, span)| span.end)
                .unwrap_or_else(|| self.eof_offset())
        } else {
            start_byte
        };
        (start_byte..end_byte).into()
    }
}

impl<'src, T: Clone + 'src> Input<'src> for Tokens<'src, T> {
    type Cursor = usize;
    type Span = Span;
    type Token = T;
    type MaybeToken = T;
    type Cache = Self;

    fn begin(self) -> (Self::Cursor, Self::Cache) {
        (0, self)
    }

    fn cursor_location(cursor: &Self::Cursor) -> usize {
        *cursor
    }

    unsafe fn next_maybe(
        this: &mut Self::Cache,
        cursor: &mut Self::Cursor,
    ) -> Option<Self::MaybeToken> {
        let (token, _) = this.tokens.get(*cursor)?;
        *cursor += 1;
        Some(token.clone())
    }

    unsafe fn span(this: &mut Self::Cache, range: Range<&Self::Cursor>) -> Self::Span {
        this.byte_span(*range.start, *range.end)
    }
}

impl<'src, T: Clone + 'src> ValueInput<'src> for Tokens<'src, T> {
    unsafe fn next(this: &mut Self::Cache, cursor: &mut Self::Cursor) -> Option<Self::Token> {
        Self::next_maybe(this, cursor)
    }
}

impl<'src, T: Clone + 'src> ExactSizeInput<'src> for Tokens<'src, T> {
    unsafe fn span_from(this: &mut Self::Cache, range: RangeFrom<&Self::Cursor>) -> Self::Span {
        let start_byte = match this.tokens.get(*range.start) {
            Some((_, span)) => span.start,
            None => this.eof_offset(),
        };
        (start_byte..this.eof_offset()).into()
    }
}
