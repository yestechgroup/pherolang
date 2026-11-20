//! Span index for `.intent` intent-specification sources (issue
//! pherolang#363).
//!
//! Definitions are the graph's `intent` nodes — the stable id as written
//! (`REFUND-001`), which is path-independent, so unlike [`super::evt_index`]
//! and [`super::dmn_index`] the builder needs no document path.
//!
//! References are the links: an `element <kind>:<name>;` line is an
//! [`ReferenceContext::IntentElement`] mention and a `check …` line an
//! [`ReferenceContext::IntentCheck`] one, each carrying the node kind its
//! written tag targets. The tag→kind table is the graph's
//! ([`crate::graph::ids::intent_link_target`]), so the span index and the
//! edge emitter cannot disagree about what a link points at. Binding stays
//! the graph index's job: an unbindable reference yields no location, never
//! an error.

use phero_ir::graph::kinds;
use phero_ir::intents::IntentLinkKind;

use super::surface::{ReferenceContext, SurfaceIndex};
use crate::graph::ids;

/// Builds the span index for one `.intent` source. A source that does not
/// parse yields an empty index; this never panics.
pub fn intents_index(source: &str) -> SurfaceIndex {
    let mut index = SurfaceIndex::default();
    let Some(ast) = phero_syntax::parse_intents(source).ast else {
        return index;
    };

    for intent in &ast.intents {
        index.push_definition(
            kinds::INTENT,
            intent.id.text.clone(),
            intent.id.span,
            intent.span,
        );
        for link in &intent.links {
            // A tag outside the closed vocabulary is a compile error (E0453),
            // so a recovered AST can carry one; it names no node kind and is
            // simply not indexed.
            let Some(kind) = IntentLinkKind::from_word(link.kind.text.as_str()) else {
                continue;
            };
            let context = match link.slot {
                phero_syntax::ast::IntentLinkSlot::Element => ReferenceContext::IntentElement,
                phero_syntax::ast::IntentLinkSlot::Check => ReferenceContext::IntentCheck,
            };
            index.push_reference(
                ids::intent_link_targets(kind),
                link.name.clone(),
                link.name_span,
                context,
            );
        }
    }

    index.sort();
    index
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::navigation::test_support;
    use phero_testkit::fixtures;

    const APP: &str = fixtures::APP_INTENT;

    #[test]
    fn intent_definitions_are_the_graph_nodes() {
        let source = test_support::fixture(APP);
        let index = intents_index(&source);
        test_support::assert_definitions_match_graph(&index, test_support::fixture_graph(), APP);
    }

    #[test]
    fn links_are_references_with_their_target_kinds() {
        let source = test_support::fixture(APP);
        let index = intents_index(&source);
        let elements: Vec<_> = index
            .references
            .iter()
            .filter(|reference| reference.context == ReferenceContext::IntentElement)
            .collect();
        assert_eq!(elements.len(), 4, "class, capability, event, view");
        assert!(elements.iter().any(
            |reference| reference.target_kinds == [kinds::CLASS] && reference.name == "Ticket"
        ));
        assert!(elements
            .iter()
            .any(|reference| reference.target_kinds == [kinds::VIEW]));
        let checks: Vec<_> = index
            .references
            .iter()
            .filter(|reference| reference.context == ReferenceContext::IntentCheck)
            .collect();
        assert_eq!(checks.len(), 1, "the single decision check");
        assert_eq!(checks[0].target_kinds, &[kinds::DECISION]);
        assert_eq!(
            &source[checks[0].span.start..checks[0].span.end],
            "RefundAssessment"
        );
    }

    #[test]
    fn the_intent_id_span_slices_the_written_id() {
        let source = test_support::fixture(APP);
        let index = intents_index(&source);
        let definition = index.definitions_of(kinds::INTENT, "GRAPH-001").remove(0);
        assert_eq!(
            &source[definition.name_span.start..definition.name_span.end],
            "GRAPH-001"
        );
    }

    #[test]
    fn broken_source_yields_an_empty_index_not_a_panic() {
        assert!(intents_index("%%% not an intent file %%%").is_empty());
        assert!(intents_index("").is_empty());
    }
}
