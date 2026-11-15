//! UTF-16 conversion at the wire boundary.
//!
//! The core engine speaks **byte** offsets (the compiler's span unit). Every
//! editor protocol — LSP most prominently — speaks UTF-16 code units. This
//! module is the one conversion point; the compiler spans are never re-derived
//! in UTF-16 space.
//!
//! Conversion is total and panic-free: offsets are clamped to the source
//! length, a start offset floors to the previous char boundary, and an end
//! offset ceils to the next one, so a span never splits a surrogate pair.

use crate::class::{HighlightClass, HighlightModifiers, HighlightSpan};

/// A highlighted range in UTF-16 code units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Utf16HighlightSpan {
    /// Start offset in UTF-16 code units.
    pub start: usize,
    /// End offset in UTF-16 code units.
    pub end: usize,
    /// The semantic class.
    pub class: HighlightClass,
    /// Orthogonal modifiers.
    pub modifiers: HighlightModifiers,
}

/// Converts byte-offset spans into UTF-16 code-unit spans.
///
/// Invalid offsets are clamped/snapped rather than rejected; the result never
/// panics and never splits a surrogate pair.
#[must_use]
pub fn to_utf16(source: &str, spans: &[HighlightSpan]) -> Vec<Utf16HighlightSpan> {
    let map = boundary_map(source);
    spans
        .iter()
        .filter_map(|span| {
            let start = floor_offset(&map, span.start, source.len());
            let end = ceil_offset(&map, span.end, source.len());
            if end <= start {
                return None;
            }
            Some(Utf16HighlightSpan {
                start,
                end,
                class: span.class,
                modifiers: span.modifiers,
            })
        })
        .collect()
}

/// A (byte offset, UTF-16 offset) pair at every char boundary, plus the end.
fn boundary_map(source: &str) -> Vec<(usize, usize)> {
    let mut map = Vec::with_capacity(source.len() + 1);
    let mut utf16 = 0;
    for (byte, character) in source.char_indices() {
        map.push((byte, utf16));
        utf16 += character.len_utf16();
    }
    map.push((source.len(), utf16));
    map
}

fn floor_offset(map: &[(usize, usize)], offset: usize, len: usize) -> usize {
    let offset = offset.min(len);
    match map.binary_search_by_key(&offset, |(byte, _)| *byte) {
        Ok(index) => map[index].1,
        Err(index) => map
            .get(index.saturating_sub(1))
            .map_or(0, |(_, utf16)| *utf16),
    }
}

fn ceil_offset(map: &[(usize, usize)], offset: usize, len: usize) -> usize {
    let offset = offset.min(len);
    match map.binary_search_by_key(&offset, |(byte, _)| *byte) {
        Ok(index) => map[index].1,
        Err(index) => map.get(index).map_or_else(
            || map.last().map_or(0, |(_, utf16)| *utf16),
            |(_, utf16)| *utf16,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans_for(_source: &str, ranges: &[(usize, usize)]) -> Vec<HighlightSpan> {
        ranges
            .iter()
            .map(|(start, end)| HighlightSpan::new(*start, *end, HighlightClass::Type))
            .collect()
    }

    #[test]
    fn ascii_offsets_are_identical() {
        let source = "class Ticket";
        let spans = spans_for(source, &[(6, 12)]);
        assert_eq!(
            to_utf16(source, &spans),
            vec![Utf16HighlightSpan {
                start: 6,
                end: 12,
                class: HighlightClass::Type,
                modifiers: HighlightModifiers::new(),
            }]
        );
    }

    #[test]
    fn non_bmp_characters_are_two_code_units() {
        let source = "😀X";
        let spans = spans_for(source, &[(4, 5)]);
        assert_eq!(to_utf16(source, &spans)[0].start, 2);
        assert_eq!(to_utf16(source, &spans)[0].end, 3);
    }

    #[test]
    fn combining_marks_advance_by_their_own_units() {
        let source = "e\u{301}X";
        let spans = spans_for(source, &[(3, 4)]);
        assert_eq!(to_utf16(source, &spans)[0].start, 2);
        assert_eq!(to_utf16(source, &spans)[0].end, 3);
    }

    #[test]
    fn crlf_is_two_units_and_offsets_stay_exact() {
        let source = "a\r\nb";
        let spans = spans_for(source, &[(3, 4)]);
        assert_eq!(to_utf16(source, &spans)[0].start, 3);
    }

    #[test]
    fn invalid_and_mid_char_offsets_are_clamped() {
        let source = "😀";
        let spans = spans_for(source, &[(0, 1), (1, 4), (0, 999), (999, 1000)]);
        let converted = to_utf16(source, &spans);
        assert_eq!(converted[0].start, 0);
        assert_eq!(converted[0].end, 2);
        assert!(converted
            .iter()
            .all(|span| span.end <= 2 && span.start <= span.end));
    }

    #[test]
    fn empty_spans_are_dropped() {
        let source = "abc";
        assert!(to_utf16(source, &spans_for(source, &[(1, 1), (3, 2)])).is_empty());
    }
}
