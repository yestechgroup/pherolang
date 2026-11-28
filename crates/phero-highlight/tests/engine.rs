//! Engine behaviour against the repository's real corpus: every surface
//! example and every documentation fence must highlight without panicking,
//! respect the span invariants, and stay inside a coarse performance budget.

use std::fs;
use std::time::Instant;

use phero_highlight::{
    highlight, HighlightClass, HighlightModifier, HighlightOptions, HighlightSpan, Surface,
};

mod common;

use common::{assert_invariants, documentation_fences, example_files, repository_root};

#[test]
fn every_surface_example_highlights_and_holds_invariants() {
    let files = example_files();
    let mut surfaces = std::collections::BTreeSet::new();
    for (surface, path) in &files {
        let source = fs::read_to_string(path).expect("read example");
        let result = highlight(&source, *surface, HighlightOptions::default());
        assert!(
            !result.spans.is_empty(),
            "{} produced no spans",
            path.display()
        );
        assert!(
            !result.degraded,
            "{} should parse cleanly: {:?}",
            path.display(),
            result.diagnostics
        );
        assert_invariants(&source, &result.spans);
        surfaces.insert(*surface);

        // Idempotence: the same bytes must produce the same spans.
        let again = highlight(&source, *surface, HighlightOptions::default());
        assert_eq!(result, again, "{} is not deterministic", path.display());
    }
    for surface in Surface::ALL {
        assert!(
            surfaces.contains(&surface),
            "examples has no `{surface}` example file"
        );
    }
}

/// One coarse semantic assertion per surface: the declaration walker is live.
#[test]
fn every_surface_example_has_semantic_declaration_spans() {
    let expectations: [(Surface, &str, HighlightClass); 9] = [
        (Surface::Mox, "Ticket", HighlightClass::Type),
        (Surface::Ifml, "\"Inbox\"", HighlightClass::Type),
        (Surface::Ddd, "SupportDesk", HighlightClass::Namespace),
        (Surface::Actor, "Support", HighlightClass::Namespace),
        (Surface::Evt, "TicketRaised", HighlightClass::Type),
        (Surface::Deploy, "SupportDesk", HighlightClass::Namespace),
        (Surface::Uc, "TrackRefund", HighlightClass::Function),
        (Surface::Cmmn, "RefundDesk", HighlightClass::Namespace),
        (Surface::Dmn, "RefundDecisions", HighlightClass::Type),
    ];
    for (surface, name, expected) in expectations {
        let path = repository_root()
            .join("examples/support")
            .join(format!("support.{}", surface.extension()));
        assert!(path.is_file(), "{} is missing", path.display());
        let source = fs::read_to_string(&path).expect("read example");
        let result = highlight(&source, surface, HighlightOptions::default());
        let matches: Vec<&HighlightSpan> = result
            .spans
            .iter()
            .filter(|span| source.get(span.start..span.end) == Some(name))
            .collect();
        assert!(
            !matches.is_empty(),
            "`{name}` is not highlighted in {}",
            path.display()
        );
        assert!(
            matches.iter().any(|span| span.class == expected
                && span.modifiers.contains(HighlightModifier::Declaration)),
            "`{name}` is not a declaration {expected:?} in {}: {matches:?}",
            path.display()
        );
    }
}

#[test]
fn documentation_fences_highlight_within_the_performance_budget() {
    let started = Instant::now();
    let fences = documentation_fences();
    assert!(!fences.is_empty(), "docs should contain surface fences");

    for fence in &fences {
        let result = highlight(&fence.source, fence.surface, HighlightOptions::default());
        assert_invariants(&fence.source, &result.spans);
        assert!(result.diagnostics.is_empty());
        // A fence may be a deliberately partial snippet; degraded is allowed,
        // but it must never be silent corruption: spans stay in bounds. The
        // epic's success measure is a fence with more than one semantic class.
        if fence.source.trim().len() > 20 {
            let classes: std::collections::BTreeSet<_> =
                result.spans.iter().map(|span| span.class).collect();
            assert!(
                classes.len() > 1,
                "{}:{} fence has only one highlight class ({classes:?})",
                fence.file,
                fence.line
            );
        }
    }

    for (surface, path) in example_files() {
        let source = fs::read_to_string(path).expect("read example");
        let result = highlight(&source, surface, HighlightOptions::default());
        assert_invariants(&source, &result.spans);
    }

    let elapsed = started.elapsed();
    assert!(
        elapsed.as_secs() < 30,
        "examples + docs fences took {elapsed:?} (budget: 30s in debug)"
    );
}
