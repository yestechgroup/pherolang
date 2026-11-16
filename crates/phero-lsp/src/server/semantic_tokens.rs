//! Semantic tokens: the LSP projection of the compiler-backed highlighter.
//!
//! `docs/HIGHLIGHTING.md` owns the taxonomy and the wire contract; this module
//! only projects it onto the standard LSP semantic-token vocabulary.
//!
//! # A standard-only legend
//!
//! The legend is derived from the registry, never hand-written: token types
//! are [`phero_highlight::HighlightClass::ALL`] filtered through
//! [`phero_highlight::lsp_token_type`] and deduped in wire order, so a client
//! with no custom-token capability still renders every class the mapping
//! knows. [`HighlightClass::Punctuation`] and [`HighlightClass::Error`] have
//! no standard LSP type and are **omitted** (punctuation is the editor
//! grammar's job; lexical errors are published as diagnostics), and
//! [`HighlightClass::Escape`] shares the `string` type. Modifiers are
//! [`phero_highlight::HighlightModifier::ALL`] in wire order — the LSP names
//! are the highlighter's own (camelCase `defaultLibrary`).
//!
//! # UTF-16, single-line, delta-encoded
//!
//! Compiler spans are byte offsets; [`encode`] converts them through the one
//! boundary ([`phero_highlight::to_utf16`]) and never re-derives them. LSP
//! positions count UTF-16 code units, and the protocol's tuple carries no end
//! line, so every emitted token is **single-line**: a multi-line block
//! comment is split at its lines' content ends (`\r`/`\n` excluded). The
//! wire form is the protocol's delta encoding — `deltaLine`, then
//! `deltaStart` relative to the previous token on the same line, then
//! `length`, the legend index, and the modifier bitset.
//!
//! # Never panic
//!
//! The editor may send anything, so encoding is defensive: tokens are sorted,
//! zero-length and out-of-bounds segments are dropped, an overlap (never
//! expected — the engine emits sorted non-overlapping spans) is discarded
//! rather than emitted, and nothing in this module indexes unchecked.

use std::collections::HashSet;
use std::path::Path;

use phero_highlight::{
    HighlightClass, HighlightModifier, HighlightModifiers, HighlightSpan, Surface,
};
use tower_lsp_server::ls_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
    Uri,
};

/// The standard LSP token types, in legend order.
///
/// This is the single derivation [`legend`] and [`encode`] share, so a token's
/// declared type index can never drift from the advertised legend.
fn legend_token_types() -> Vec<&'static str> {
    let mut types = Vec::with_capacity(HighlightClass::ALL.len());
    let mut seen: HashSet<&'static str> = HashSet::new();
    for class in HighlightClass::ALL {
        if let Some(name) = phero_highlight::lsp_token_type(class) {
            if seen.insert(name) {
                types.push(name);
            }
        }
    }
    types
}

/// The semantic-tokens legend: standard types from the registry, every
/// modifier in wire order.
pub(super) fn legend() -> SemanticTokensLegend {
    let token_types = legend_token_types()
        .into_iter()
        .map(SemanticTokenType::new)
        .collect();
    let token_modifiers = HighlightModifier::ALL
        .into_iter()
        .map(|modifier| SemanticTokenModifier::new(phero_highlight::lsp_modifier(modifier)))
        .collect();
    SemanticTokensLegend {
        token_types,
        token_modifiers,
    }
}

/// The modifier bitset of one span: bit `i` for `HighlightModifier::ALL[i]`.
fn modifier_bits(modifiers: HighlightModifiers) -> u32 {
    HighlightModifier::ALL
        .into_iter()
        .enumerate()
        .fold(0u32, |bits, (index, modifier)| {
            if modifiers.contains(modifier) {
                bits | (1u32 << index)
            } else {
                bits
            }
        })
}

/// One line's content range in UTF-16 code units (terminators excluded).
#[derive(Debug, Clone, Copy)]
struct Line {
    start: usize,
    content_end: usize,
}

/// The UTF-16 length of a string slice.
fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

/// The line model (same shape as [`crate::position::PositionMap`]'s): one
/// entry per line, its content excluding the `\r`/`\n` terminator.
fn lines(text: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut start = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.strip_suffix('\n').unwrap_or(line);
        let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);
        let content_end = start + utf16_len(trimmed);
        lines.push(Line { start, content_end });
        start += utf16_len(line);
    }
    // A trailing fragment without a terminator is still a line; empty input
    // gets a single empty line so line 0 always exists.
    let total = utf16_len(text);
    if start < total || lines.is_empty() {
        lines.push(Line {
            start,
            content_end: total,
        });
    }
    lines
}

/// The line index containing `offset`, clamped into the document.
fn line_of(lines: &[Line], offset: usize) -> usize {
    match lines.binary_search_by(|line| line.start.cmp(&offset)) {
        Ok(index) => index,
        Err(insertion) => insertion.saturating_sub(1),
    }
}

/// One emitted token before delta encoding: line-relative UTF-16 start.
#[derive(Debug, Clone, Copy)]
struct Run {
    line: usize,
    start: usize,
    length: usize,
    token_type: u32,
    modifiers: u32,
}

/// Encodes `spans` (byte offsets) as LSP semantic tokens over `text`.
pub(super) fn encode(text: &str, spans: &[HighlightSpan]) -> SemanticTokens {
    let lines = lines(text);
    let types = legend_token_types();
    let mut runs: Vec<Run> = Vec::with_capacity(spans.len());
    for span in phero_highlight::to_utf16(text, spans) {
        // Punctuation and lexical errors have no standard type: omitted.
        let Some(class_type) = phero_highlight::lsp_token_type(span.class) else {
            continue;
        };
        let Some(token_type) = types.iter().position(|name| *name == class_type) else {
            continue;
        };
        let token_type = token_type as u32;
        let modifiers = modifier_bits(span.modifiers);
        // Split at line content ends: the LSP tuple has no end line, so one
        // token never spans lines (a block comment becomes one token per line).
        let mut line = line_of(&lines, span.start).min(lines.len().saturating_sub(1));
        let mut cursor = span.start;
        while cursor < span.end && line < lines.len() {
            let current = lines[line];
            let segment_start = cursor.max(current.start);
            let segment_end = span.end.min(current.content_end);
            if segment_end > segment_start {
                runs.push(Run {
                    line,
                    start: segment_start - current.start,
                    length: segment_end - segment_start,
                    token_type,
                    modifiers,
                });
            }
            if span.end <= current.content_end {
                break;
            }
            cursor = current.content_end.max(cursor);
            line += 1;
        }
    }

    // Defensive normalization: sort, clamp to the document, drop empties and
    // any overlap (the engine never emits one; a hostile span must not break
    // the client).
    runs.sort_by_key(|run| (run.line, run.start));
    let mut accepted: Vec<Run> = Vec::with_capacity(runs.len());
    for run in runs {
        let Some(line) = lines.get(run.line) else {
            continue;
        };
        let width = line.content_end.saturating_sub(line.start);
        let start = run.start.min(width);
        let length = run.length.min(width.saturating_sub(start));
        if length == 0 {
            continue;
        }
        if let Some(previous) = accepted.last() {
            if previous.line == run.line && previous.start + previous.length > start {
                continue;
            }
        }
        accepted.push(Run {
            line: run.line,
            start,
            length,
            ..run
        });
    }

    // Delta encoding: line deltas are absolute when the line moves, start
    // deltas are relative to the previous token on the same line.
    let mut data = Vec::with_capacity(accepted.len());
    let mut previous_line = 0u32;
    let mut previous_start = 0u32;
    for run in accepted {
        let line = u32::try_from(run.line).unwrap_or(u32::MAX);
        let start = u32::try_from(run.start).unwrap_or(u32::MAX);
        let length = u32::try_from(run.length).unwrap_or(u32::MAX);
        let delta_line = line.saturating_sub(previous_line);
        let delta_start = if delta_line == 0 {
            start.saturating_sub(previous_start)
        } else {
            start
        };
        data.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type: run.token_type,
            token_modifiers_bitset: run.modifiers,
        });
        previous_line = line;
        previous_start = start;
    }
    SemanticTokens {
        result_id: None,
        data,
    }
}

/// The highlight surface a document URI names, by its extension.
///
/// Mirrors [`super::surface_for`]'s decode (a `file:` URI's path may be
/// percent-encoded); `None` for a document the highlighter does not know.
pub(super) fn surface_of_uri(uri: &Uri) -> Option<Surface> {
    let path = uri.path().decode().to_string_lossy().into_owned();
    Surface::from_path(Path::new(&path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class_of(spans: &[HighlightSpan], source: &str, needle: &str) -> HighlightClass {
        let start = source.find(needle).expect("the needle occurs");
        spans
            .iter()
            .find(|span| span.start <= start && start < span.end)
            .map(|span| span.class)
            .expect("the needle is highlighted")
    }

    #[test]
    fn legend_is_standard_only_and_deduped() {
        let legend = legend();
        let types: Vec<&str> = legend.token_types.iter().map(|t| t.as_str()).collect();
        assert_eq!(
            types,
            [
                "keyword",
                "type",
                "function",
                "namespace",
                "variable",
                "parameter",
                "property",
                "string",
                "number",
                "comment",
                "operator",
                "decorator",
                "enumMember",
            ]
        );
        assert_eq!(types.len(), 13);
        let modifiers: Vec<&str> = legend.token_modifiers.iter().map(|m| m.as_str()).collect();
        assert_eq!(
            modifiers,
            [
                "declaration",
                "definition",
                "readonly",
                "deprecated",
                "documentation",
                "defaultLibrary",
            ]
        );
    }

    #[test]
    fn encode_drops_punctuation_and_keeps_modifiers() {
        // A hand-built span list keeps this independent of the engine.
        let source = "class Book {}";
        let mut declaration = HighlightModifiers::new();
        declaration.insert(HighlightModifier::Declaration);
        let spans = [
            HighlightSpan::new(0, 5, HighlightClass::Keyword),
            HighlightSpan::with_modifiers(6, 10, HighlightClass::Type, declaration),
            HighlightSpan::new(10, 12, HighlightClass::Punctuation),
            HighlightSpan::new(13, 13, HighlightClass::Type),
        ];
        let tokens = encode(source, &spans);
        assert_eq!(tokens.data.len(), 2, "{:?}", tokens.data);
        let first = tokens.data[0];
        assert_eq!(
            (first.delta_line, first.delta_start, first.length),
            (0, 0, 5)
        );
        let second = tokens.data[1];
        assert_eq!((second.delta_line, second.delta_start), (0, 6));
        assert_eq!(second.length, 4);
        assert_eq!(second.token_modifiers_bitset, 1);
        assert_eq!(class_of(&spans, source, "Book"), HighlightClass::Type);
    }

    #[test]
    fn block_comments_split_at_line_content_ends() {
        let source = "/* first\r\nsecond */\r\nclass Book {}\r\n";
        let spans = [HighlightSpan::new(0, 19, HighlightClass::Comment)];
        let tokens = encode(source, &spans);
        assert_eq!(tokens.data.len(), 2);
        assert_eq!(tokens.data[0].delta_line, 0);
        assert_eq!(tokens.data[0].length, 8);
        assert_eq!(tokens.data[1].delta_line, 1);
        assert_eq!(tokens.data[1].delta_start, 0);
        assert_eq!(tokens.data[1].length, 9);
    }

    #[test]
    fn offsets_are_utf16_and_out_of_bounds_spans_are_clamped() {
        let source = "// 😀\nclass Book {}";
        let spans = [
            HighlightSpan::new(0, 6, HighlightClass::Comment),
            HighlightSpan::new(8, 13, HighlightClass::Keyword),
            HighlightSpan::new(999, 1000, HighlightClass::Type),
        ];
        let tokens = encode(source, &spans);
        assert_eq!(tokens.data.len(), 2);
        assert_eq!(tokens.data[1].delta_line, 1);
        assert_eq!(tokens.data[1].delta_start, 0);
        assert_eq!(tokens.data[1].length, 5);
    }

    #[test]
    fn empty_document_yields_no_tokens() {
        assert!(encode("", &[]).data.is_empty());
        assert!(
            encode("", &[HighlightSpan::new(0, 1, HighlightClass::Type)])
                .data
                .is_empty()
        );
    }
}
