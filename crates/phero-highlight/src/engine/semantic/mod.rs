//! The semantic pass: exact compiler spans overlaid on the lexical base.
//!
//! Each surface parses with its **own compiler parser** and walks the spanned
//! AST. No grammar is re-implemented here and no source is re-read: a name's
//! span is the span the parser recorded, which is why the overlays line up with
//! what the compiler sees even in the presence of edits the lexical pass cannot
//! know about.
//!
//! Coverage is deliberately prioritized: declaration names, type references,
//! operation/function names, parameters, fields/properties, namespace/package
//! segments, and references. Everything else falls back to the lexical pass,
//! which is still compiler-tokenized.

use phero_syntax::ast::{Name, QualifiedName, Span, TypeRef};

use crate::class::{HighlightClass, HighlightModifier, HighlightModifiers, HighlightSpan};
use crate::engine::lexical::{LexToken, LexTokenKind};
use crate::metadata::{is_builtin_type, metadata_for};
use crate::surface::Surface;

pub(crate) mod actor;
pub(crate) mod cmmn;
pub(crate) mod ddd;
pub(crate) mod deploy;
pub(crate) mod dmn;
pub(crate) mod evt;
pub(crate) mod ifml;
pub(crate) mod intents;
pub(crate) mod mox;
pub(crate) mod rune;
pub(crate) mod uc;

/// Walk context: the source, the token table for span recovery, and the
/// semantic overlay sink.
pub(crate) struct Ctx<'a> {
    /// The source text (for slicing spans that carry more than one name).
    pub source: &'a str,
    /// The surface being highlighted.
    pub surface: Surface,
    /// The lexical token table (for recovering spans the AST does not carry).
    pub tokens: &'a [LexToken<'a>],
    /// The overlay sink, in insertion order.
    pub out: Vec<HighlightSpan>,
}

impl<'a> Ctx<'a> {
    /// A new context with an empty overlay sink.
    pub(crate) fn new(source: &'a str, surface: Surface, tokens: &'a [LexToken<'a>]) -> Ctx<'a> {
        Ctx {
            source,
            surface,
            tokens,
            out: Vec::new(),
        }
    }

    /// Pushes a span overlay with no modifiers, ignoring empty ranges.
    pub(crate) fn push(&mut self, span: Span, class: HighlightClass) {
        self.push_with(span, class, HighlightModifiers::new());
    }

    /// Pushes a span overlay with explicit modifiers.
    pub(crate) fn push_with(
        &mut self,
        span: Span,
        class: HighlightClass,
        modifiers: HighlightModifiers,
    ) {
        if span.end > span.start {
            self.out.push(HighlightSpan::with_modifiers(
                span.start, span.end, class, modifiers,
            ));
        }
    }

    /// Pushes a declaration name with the `declaration` modifier.
    pub(crate) fn decl_name(&mut self, name: &Name, class: HighlightClass) {
        self.push_with(
            name.span,
            class,
            HighlightModifiers::new().with(HighlightModifier::Declaration),
        );
    }

    /// Pushes a qualified name: namespace segments, then the final segment as
    /// `last`.
    pub(crate) fn qualified(&mut self, name: &QualifiedName, last: HighlightClass) {
        let count = name.segments.len();
        for (index, segment) in name.segments.iter().enumerate() {
            let class = if index + 1 == count {
                last
            } else {
                HighlightClass::Namespace
            };
            self.push(segment.span, class);
        }
    }

    /// Pushes a dotted member path: every leading segment is a `variable`
    /// reference, the final one is `last`.
    pub(crate) fn member_path(&mut self, name: &QualifiedName, last: HighlightClass) {
        let count = name.segments.len();
        for (index, segment) in name.segments.iter().enumerate() {
            let class = if index + 1 == count {
                last
            } else {
                HighlightClass::Variable
            };
            self.push(segment.span, class);
        }
    }

    /// Pushes a type reference: namespace segments, then the type name (with
    /// the `defaultLibrary` modifier for builtin primitives).
    pub(crate) fn type_ref(&mut self, type_ref: &TypeRef) {
        let count = type_ref.name.segments.len();
        if count == 0 {
            return;
        }
        let builtin = type_ref.name.segments.last().is_some_and(|segment| {
            !segment.escaped && is_builtin_type(self.surface, &segment.text)
        });
        for (index, segment) in type_ref.name.segments.iter().enumerate() {
            if index + 1 < count {
                self.push(segment.span, HighlightClass::Namespace);
            } else {
                let mut modifiers = HighlightModifiers::new();
                if builtin {
                    modifiers.insert(HighlightModifier::DefaultLibrary);
                }
                self.push_with(segment.span, HighlightClass::Type, modifiers);
            }
        }
    }

    /// Pushes a reference name, classifying it the way the lexical pass would:
    /// literal, builtin type, uppercase initial, else variable. An escaped name
    /// is never a keyword/literal (the `^` says "this is a name").
    pub(crate) fn reference(&mut self, name: &Name) {
        let class = if name.escaped {
            match name.text.chars().next() {
                Some(first) if first.is_uppercase() => HighlightClass::Type,
                _ => HighlightClass::Variable,
            }
        } else {
            self.reference_class(&name.text)
        };
        self.push(name.span, class);
    }

    /// The lexical class of a bare reference word.
    pub(crate) fn reference_class(&self, text: &str) -> HighlightClass {
        if metadata_for(self.surface).literals.contains(&text) {
            return HighlightClass::Constant;
        }
        if is_builtin_type(self.surface, text) {
            return HighlightClass::Type;
        }
        match text.chars().next() {
            Some(first) if first.is_uppercase() => HighlightClass::Type,
            _ => HighlightClass::Variable,
        }
    }

    /// Expands `span` to the whole enclosing string token when it lies inside
    /// one, then overlays `class`. `.ifml` names live inside string tokens and
    /// strings stay atomic, so this is the one place an overlay replaces a
    /// lexical `string` span.
    pub(crate) fn string_in(&mut self, span: Span, class: HighlightClass) {
        self.string_with(span, class, HighlightModifiers::new());
    }

    /// [`Ctx::string_in`] with explicit modifiers.
    pub(crate) fn string_with(
        &mut self,
        span: Span,
        class: HighlightClass,
        modifiers: HighlightModifiers,
    ) {
        match self.enclosing_string(span) {
            Some((start, end)) => {
                self.out
                    .push(HighlightSpan::with_modifiers(start, end, class, modifiers));
            }
            None => self.push_with(span, class, modifiers),
        }
    }

    /// A `.ifml` declaration name: the enclosing string token with the
    /// `declaration` modifier.
    pub(crate) fn decl_string(&mut self, span: Span, class: HighlightClass) {
        self.string_with(
            span,
            class,
            HighlightModifiers::new().with(HighlightModifier::Declaration),
        );
    }

    fn enclosing_string(&self, span: Span) -> Option<(usize, usize)> {
        self.tokens.iter().find_map(|token| {
            if matches!(token.kind, LexTokenKind::Str)
                && token.start <= span.start
                && span.end <= token.end
            {
                Some((token.start, token.end))
            } else {
                None
            }
        })
    }

    /// Finds the first identifier/keyword token with `text` fully inside
    /// `within` and overlays it.
    pub(crate) fn word_in(&mut self, within: Span, text: &str, class: HighlightClass) {
        if let Some((start, end)) = self.find_word(within, text, false) {
            self.out.push(HighlightSpan::new(start, end, class));
        }
    }

    /// [`Ctx::word_in`] with the `declaration` modifier.
    pub(crate) fn decl_word_in(&mut self, within: Span, text: &str, class: HighlightClass) {
        if let Some((start, end)) = self.find_word(within, text, false) {
            self.out.push(HighlightSpan::with_modifiers(
                start,
                end,
                class,
                HighlightModifiers::new().with(HighlightModifier::Declaration),
            ));
        }
    }

    /// Like [`Ctx::word_in`], but takes the **last** matching token (used for
    /// `.ifml`'s `lookup ... via lookup` where the same word plays two roles).
    pub(crate) fn last_word_in(&mut self, within: Span, text: &str, class: HighlightClass) {
        if let Some((start, end)) = self.find_word(within, text, true) {
            self.out.push(HighlightSpan::new(start, end, class));
        }
    }

    fn find_word(&self, within: Span, text: &str, last: bool) -> Option<(usize, usize)> {
        let matches = |token: &&LexToken<'a>| {
            token.start >= within.start
                && token.end <= within.end
                && token.word() == Some(text)
                && !matches!(token.kind, LexTokenKind::Str)
        };
        if last {
            self.tokens
                .iter()
                .rfind(matches)
                .map(|token| (token.start, token.end))
        } else {
            self.tokens
                .iter()
                .find(matches)
                .map(|token| (token.start, token.end))
        }
    }

    /// Splits a span's source text on `.` and overlays the segments: leading
    /// segments as namespaces, the final one as `last`. Falls back to one span
    /// when the text has no separator or the bounds are not usable.
    pub(crate) fn qualified_text(&mut self, span: Span, last: HighlightClass) {
        let (Some(text), true) = (
            self.source.get(span.start..span.end),
            span.start < span.end && span.end <= self.source.len(),
        ) else {
            return;
        };
        let bytes = text.as_bytes();
        let mut segments = Vec::new();
        let mut start = 0;
        for (index, byte) in bytes.iter().enumerate() {
            if *byte == b'.' {
                segments.push((start, index));
                start = index + 1;
            }
        }
        segments.push((start, bytes.len()));
        let count = segments.len();
        for (index, (offset_start, offset_end)) in segments.into_iter().enumerate() {
            let class = if index + 1 == count {
                last
            } else {
                HighlightClass::Namespace
            };
            if offset_start < offset_end {
                self.out.push(HighlightSpan::new(
                    span.start + offset_start,
                    span.start + offset_end,
                    class,
                ));
            }
        }
    }

    /// Overlays a raw expression span as a set of reference words: every
    /// identifier token inside the span is classified (uppercase → type, else
    /// variable), leaving operators/punctuation to the lexical pass.
    pub(crate) fn expression(&mut self, span: Span) {
        let mut found = Vec::new();
        for token in self.tokens {
            if token.start < span.start || token.end > span.end {
                continue;
            }
            if let LexTokenKind::Ident { text, escaped } = &token.kind {
                let class = if *escaped {
                    match text.chars().next() {
                        Some(first) if first.is_uppercase() => HighlightClass::Type,
                        _ => HighlightClass::Variable,
                    }
                } else {
                    self.reference_class(text)
                };
                found.push((token.start, token.end, class));
            }
        }
        for (start, end, class) in found {
            self.out.push(HighlightSpan::new(start, end, class));
        }
    }
}
