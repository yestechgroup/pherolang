//! Deterministic HTML rendering of a highlight result.
//!
//! The output is a self-contained fragment with **no** theme colours and no
//! trailing newline:
//!
//! ```html
//! <pre class="pherolang-highlight"><code class="language-mox">…</code></pre>
//! ```
//!
//! Every covered range becomes `<span class="hl-<class>[ hl-<modifier>]">`.
//! Text is escaped (`&`, `<`, `>`, `"`), so untrusted model text can never
//! inject markup. Overlapping or out-of-bounds spans are repaired rather than
//! trusted: the renderer only ever emits the source text once, in order.

use crate::class::{HighlightModifier, HighlightSpan};
use crate::surface::Surface;

/// Renders `source` with `spans` as a deterministic HTML fragment.
#[must_use]
pub fn to_html(source: &str, spans: &[HighlightSpan], surface: Surface) -> String {
    let mut ordered: Vec<&HighlightSpan> =
        spans.iter().filter(|span| span.start < span.end).collect();
    ordered.sort_by_key(|span| (span.start, span.end));

    let mut body = String::with_capacity(source.len() + source.len() / 2);
    let mut cursor = 0;
    for span in ordered {
        let start = floor_boundary(source, span.start.min(source.len()));
        let end = ceil_boundary(source, span.end.min(source.len()));
        let start = start.max(cursor);
        if end <= start || end > source.len() {
            continue;
        }
        if start > cursor {
            escape_into(&mut body, &source[cursor..start]);
        }
        body.push_str("<span class=\"hl-");
        body.push_str(span.class.as_str());
        for modifier in span.modifiers.iter() {
            body.push_str(" hl-");
            body.push_str(modifier_class(modifier));
        }
        body.push_str("\">");
        escape_into(&mut body, &source[start..end]);
        body.push_str("</span>");
        cursor = end;
    }
    if cursor < source.len() {
        escape_into(&mut body, &source[cursor..]);
    }

    let mut out = String::with_capacity(body.len() + 96);
    out.push_str("<pre class=\"pherolang-highlight\"><code class=\"language-");
    out.push_str(surface.id());
    out.push_str("\">");
    out.push_str(&body);
    out.push_str("</code></pre>");
    out
}

/// The kebab-case CSS suffix for a modifier (`defaultLibrary` →
/// `default-library`).
#[must_use]
pub fn modifier_class(modifier: HighlightModifier) -> &'static str {
    match modifier {
        HighlightModifier::Declaration => "declaration",
        HighlightModifier::Definition => "definition",
        HighlightModifier::Readonly => "readonly",
        HighlightModifier::Deprecated => "deprecated",
        HighlightModifier::Documentation => "documentation",
        HighlightModifier::DefaultLibrary => "default-library",
    }
}

fn floor_boundary(source: &str, mut offset: usize) -> usize {
    while offset > 0 && !source.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn ceil_boundary(source: &str, mut offset: usize) -> usize {
    while offset < source.len() && !source.is_char_boundary(offset) {
        offset += 1;
    }
    offset
}

fn escape_into(out: &mut String, text: &str) {
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::class::HighlightClass;
    use crate::engine::{highlight, HighlightOptions};
    use crate::metadata::metadata_for;

    #[test]
    fn html_wraps_the_expected_language_and_has_no_trailing_newline() {
        let source = "class A { }";
        let result = highlight(source, Surface::Mox, HighlightOptions::default());
        let html = to_html(source, &result.spans, Surface::Mox);
        assert!(
            html.starts_with("<pre class=\"pherolang-highlight\"><code class=\"language-mox\">")
        );
        assert!(html.ends_with("</code></pre>"));
        assert!(html.contains("<span class=\"hl-keyword\">class</span>"));
        assert!(html.contains("<span class=\"hl-type hl-declaration\">A</span>"));
        assert!(!html.ends_with('\n'));
    }

    #[test]
    fn html_escapes_untrusted_text() {
        let source = "// <script>alert(\"x\") & co</script>";
        let result = highlight(source, Surface::Mox, HighlightOptions::default());
        let html = to_html(source, &result.spans, Surface::Mox);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("&quot;"));
        assert!(html.contains("&amp;"));
    }

    #[test]
    fn html_repairs_overlapping_and_out_of_bounds_spans() {
        let source = "abcdef";
        let spans = vec![
            HighlightSpan::new(0, 3, HighlightClass::Type),
            HighlightSpan::new(2, 5, HighlightClass::Function),
            HighlightSpan::new(4, 999, HighlightClass::Keyword),
            HighlightSpan::new(9, 12, HighlightClass::Error),
        ];
        let html = to_html(source, &spans, Surface::Mox);
        assert!(html.contains(">abc<"));
        assert!(html.contains(">de<"));
        assert!(html.contains(">f<"));
        assert!(!html.contains("hl-error"));
    }

    #[test]
    fn html_handles_empty_source() {
        let html = to_html("", &[], Surface::Ifml);
        assert_eq!(
            html,
            "<pre class=\"pherolang-highlight\"><code class=\"language-ifml\"></code></pre>"
        );
    }

    #[test]
    fn every_surface_id_is_used_as_the_language_class() {
        for surface in Surface::ALL {
            let html = to_html("", &[], surface);
            assert!(html.contains(&format!("\"language-{}\"", metadata_for(surface).id)));
        }
    }

    #[test]
    fn modifier_classes_are_kebab_case_and_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for modifier in HighlightModifier::ALL {
            let name = modifier_class(modifier);
            assert!(!name.contains(char::is_uppercase));
            assert!(seen.insert(name));
        }
    }
}
