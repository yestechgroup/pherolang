//! Engine behaviour against the repository's real corpus: every surface
//! example and every documentation fence must highlight without panicking,
//! respect the span invariants, and stay inside a coarse performance budget.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use phero_highlight::{
    highlight, HighlightClass, HighlightModifier, HighlightOptions, HighlightSpan, Surface,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root")
}

fn assert_invariants(source: &str, spans: &[HighlightSpan]) {
    let mut previous_end = 0;
    for span in spans {
        assert!(span.start < span.end, "empty span in {source:?}");
        assert!(span.end <= source.len(), "out-of-bounds span");
        assert!(
            span.start >= previous_end,
            "overlapping or unsorted spans at {}..{}",
            span.start,
            span.end
        );
        assert!(source.is_char_boundary(span.start));
        assert!(source.is_char_boundary(span.end));
        previous_end = span.end;
    }
}

fn example_files() -> Vec<(Surface, PathBuf)> {
    let directory = repository_root().join("examples/support");
    let mut files = Vec::new();
    for entry in fs::read_dir(&directory).expect("examples/support") {
        let path = entry.expect("directory entry").path();
        if !path.is_file() {
            continue;
        }
        if let Some(surface) = Surface::from_path(&path) {
            files.push((surface, path));
        }
    }
    files.sort_by(|left, right| left.1.cmp(&right.1));
    files
}

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
            "examples/support has no `{surface}` file"
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

/// Collects every fenced block in `docs/**/*.md` whose info string is a
/// surface id.
fn documentation_fences() -> Vec<(Surface, String, String)> {
    let root = repository_root();
    let mut fences = Vec::new();
    let mut stack = vec![root.join("docs")];
    while let Some(directory) = stack.pop() {
        let mut entries: Vec<PathBuf> = fs::read_dir(&directory)
            .expect("docs directory")
            .map(|entry| entry.expect("directory entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
                continue;
            }
            let text = fs::read_to_string(&path).expect("read markdown");
            let label = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .display()
                .to_string();
            let mut current: Option<(Surface, String)> = None;
            for line in text.lines() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("```") {
                    match current.take() {
                        Some((surface, content)) => {
                            fences.push((surface, content, label.clone()));
                        }
                        None => {
                            let info = trimmed.trim_start_matches('`').trim();
                            let word = info.split_whitespace().next().unwrap_or("");
                            if let Some(surface) = Surface::ALL
                                .into_iter()
                                .find(|surface| surface.id() == word)
                            {
                                current = Some((surface, String::new()));
                            }
                        }
                    }
                    continue;
                }
                if let Some((_, content)) = current.as_mut() {
                    content.push_str(line);
                    content.push('\n');
                }
            }
        }
    }
    fences
}

#[test]
fn documentation_fences_highlight_within_the_performance_budget() {
    let started = Instant::now();
    let fences = documentation_fences();
    assert!(!fences.is_empty(), "docs should contain surface fences");

    for (surface, content, label) in &fences {
        let result = highlight(content, *surface, HighlightOptions::default());
        assert_invariants(content, &result.spans);
        assert!(result.diagnostics.is_empty());
        // A fence may be a deliberately partial snippet; degraded is allowed,
        // but it must never be silent corruption: spans stay in bounds. The
        // epic's success measure is a fence with more than one semantic class.
        if content.trim().len() > 20 {
            let classes: std::collections::BTreeSet<_> =
                result.spans.iter().map(|span| span.class).collect();
            assert!(
                classes.len() > 1,
                "{label} fence has only one highlight class ({classes:?})"
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
