//! The checked highlight corpus.
//!
//! Every case under `tests/highlight/corpus/` is highlighted in strict mode,
//! held to the span invariants, and folded into the compact summary at
//! `tests/highlight/summary/corpus.json`. A representative subset is also
//! pinned as a full exact-span artifact under `tests/highlight/json/corpus.*`.
//! The docs-fence summary (`tests/highlight/summary/docs-fences.json`) covers
//! every Phero Lang fence in `docs/**/*.md`, so a new fence is a corpus change
//! even when no corpus file moves.
//!
//! Nothing here is auto-blessed in CI: regenerate deliberately and read the
//! diff.
//!
//! ```sh
//! PHERO_UPDATE_FIXTURES=1 cargo test -p phero-highlight --test corpus
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use phero_highlight::{
    highlight, HighlightArtifact, HighlightClass, HighlightError, HighlightModifier,
    HighlightOptions, Surface, HIGHLIGHT_FORMAT_VERSION,
};
use serde_json::json;

mod common;

use common::{assert_invariants, documentation_fences, repository_root};

const CORPUS_FORMAT_VERSION: u32 = 1;

/// One corpus input file.
struct Case {
    /// `/`-separated path relative to `tests/highlight/corpus/`, without the
    /// extension (for example `mox/valid`).
    id: String,
    surface: Surface,
    source: String,
}

fn corpus_root() -> PathBuf {
    repository_root().join("tests/highlight/corpus")
}

fn summary_dir() -> PathBuf {
    repository_root().join("tests/highlight/summary")
}

fn json_dir() -> PathBuf {
    repository_root().join("tests/highlight/json")
}

fn walk(directory: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            walk(&path, out);
        } else if path.is_file() {
            out.push(path);
        }
    }
}

fn corpus_cases() -> Vec<Case> {
    let root = corpus_root();
    let mut files = Vec::new();
    walk(&root, &mut files);

    let mut cases = Vec::new();
    for path in files {
        let relative = path
            .strip_prefix(&root)
            .expect("corpus file under the corpus root");
        if relative == Path::new("README.md") {
            continue;
        }
        let surface = Surface::from_path(&path).unwrap_or_else(|| {
            panic!(
                "{}: every corpus file needs a Phero Lang extension",
                relative.display()
            )
        });
        let id = relative
            .with_extension("")
            .to_string_lossy()
            .replace('\\', "/");
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        cases.push(Case {
            id,
            surface,
            source,
        });
    }
    cases.sort_by(|left, right| left.id.cmp(&right.id));
    cases
}

/// A corpus case is deliberately broken when its name says so.
fn expects_degradation(id: &str) -> bool {
    id.contains("truncated")
        || id.contains("parse-error")
        || id.contains("unicode-identifier-error")
}

fn golden(path: &Path, generated: &str) -> String {
    if std::env::var_os("PHERO_UPDATE_FIXTURES").is_some() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create golden directory");
        }
        fs::write(path, generated)
            .unwrap_or_else(|error| panic!("write golden {}: {error}", path.display()));
        eprintln!("updated {}", path.display());
        return generated.to_string();
    }
    fs::read_to_string(path).unwrap_or_else(|error| {
        panic!(
            "read golden {}: {error} — regenerate with PHERO_UPDATE_FIXTURES=1 \
             cargo test -p phero-highlight --test corpus",
            path.display()
        )
    })
}

fn class_counts(spans: &[phero_highlight::HighlightSpan]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for span in spans {
        *counts.entry(span.class.as_str()).or_insert(0) += 1;
    }
    counts
}

fn modifier_counts(spans: &[phero_highlight::HighlightSpan]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for span in spans {
        for modifier in span.modifiers.iter() {
            *counts.entry(modifier.as_str()).or_insert(0) += 1;
        }
    }
    counts
}

/// Every span whose source slice is exactly `text`, in document order.
fn spans_for<'a>(
    source: &str,
    spans: &'a [phero_highlight::HighlightSpan],
    text: &str,
) -> Vec<&'a phero_highlight::HighlightSpan> {
    spans
        .iter()
        .filter(|span| source.get(span.start..span.end) == Some(text))
        .collect()
}

/// Representative cases that carry a full byte-pinned artifact, not just a
/// class summary. One per surface family plus every shared byte case.
const EXACT_CASES: &[&str] = &[
    "ddd/contextual-ordinary",
    "ifml/lookalikes",
    "mox/escapes",
    "mox/valid",
    "shared/comments-doc",
    "shared/crlf",
    "shared/escapes",
    "shared/unicode-strings",
];

#[test]
fn corpus_cases_hold_the_invariants_and_expectations() {
    let cases = corpus_cases();
    assert!(!cases.is_empty(), "the highlight corpus is empty");

    for case in &cases {
        let result = highlight(&case.source, case.surface, HighlightOptions::strict());
        assert_invariants(&case.source, &result.spans);
        assert_eq!(
            result.degraded,
            expects_degradation(&case.id),
            "{}: degraded should be {} (diagnostics: {:?})",
            case.id,
            expects_degradation(&case.id),
            result.diagnostics
        );
        assert!(
            !result.spans.is_empty(),
            "{}: strict mode produced no spans",
            case.id
        );

        let again = highlight(&case.source, case.surface, HighlightOptions::strict());
        assert_eq!(result, again, "{} is not deterministic", case.id);
    }
}

#[test]
fn every_surface_has_a_corpus_case() {
    let cases = corpus_cases();
    for surface in Surface::ALL {
        assert!(
            cases.iter().any(|case| case.surface == surface),
            "surface `{}` has no corpus case under tests/highlight/corpus/",
            surface.id()
        );
    }
}

#[test]
fn exact_cases_are_registered_and_byte_pinned() {
    let cases = corpus_cases();
    for id in EXACT_CASES {
        let case = cases
            .iter()
            .find(|case| case.id == *id)
            .unwrap_or_else(|| panic!("exact case `{id}` is not a corpus file"));
        let result = highlight(&case.source, case.surface, HighlightOptions::strict());
        let artifact = HighlightArtifact::new(&case.source, case.surface, result);
        let generated = artifact.to_json_pretty().expect("serialize the artifact");
        let name = id.replace('/', "-");
        let path = json_dir().join(format!("corpus.{name}.json"));
        assert_eq!(golden(&path, &generated), generated, "exact case `{id}`");
    }
}

#[test]
fn corpus_class_summary_is_a_golden() {
    let mut entries = Vec::new();
    for case in corpus_cases() {
        let result = highlight(&case.source, case.surface, HighlightOptions::strict());
        entries.push(json!({
            "id": case.id,
            "surface": case.surface.id(),
            "degraded": result.degraded,
            "spans": result.spans.len(),
            "classes": class_counts(&result.spans),
            "modifiers": modifier_counts(&result.spans),
        }));
    }
    let generated = format!(
        "{}\n",
        serde_json::to_string_pretty(&json!({
            "formatVersion": CORPUS_FORMAT_VERSION,
            "cases": entries,
        }))
        .expect("serialize the corpus summary")
    );
    let path = summary_dir().join("corpus.json");
    assert_eq!(golden(&path, &generated), generated);
}

#[test]
fn docs_fence_summary_covers_every_phero_fence() {
    let fences = documentation_fences();
    assert!(!fences.is_empty(), "docs should contain surface fences");

    let mut entries = Vec::new();
    let mut surfaces = BTreeSet::new();
    for fence in &fences {
        let result = highlight(
            fence.source.as_str(),
            fence.surface,
            HighlightOptions::strict(),
        );
        assert_invariants(&fence.source, &result.spans);
        surfaces.insert(fence.surface);
        entries.push(json!({
            "file": fence.file,
            "line": fence.line,
            "surface": fence.surface.id(),
            "degraded": result.degraded,
            "spans": result.spans.len(),
            "classes": class_counts(&result.spans),
        }));
    }

    let generated = format!(
        "{}\n",
        serde_json::to_string_pretty(&json!({
            "formatVersion": CORPUS_FORMAT_VERSION,
            "fences": entries,
        }))
        .expect("serialize the docs-fence summary")
    );
    let path = summary_dir().join("docs-fences.json");
    assert_eq!(golden(&path, &generated), generated);

    for surface in Surface::ALL {
        assert!(
            surfaces.contains(&surface),
            "docs/**/*.md has no `{}` fence — every surface needs a documentation example",
            surface.id()
        );
    }
}

#[test]
fn doc_comments_carry_the_documentation_modifier() {
    let case = corpus_cases()
        .into_iter()
        .find(|case| case.id == "shared/comments-doc")
        .expect("shared/comments-doc case");
    let result = highlight(&case.source, case.surface, HighlightOptions::strict());
    let comments: Vec<_> = result
        .spans
        .iter()
        .filter(|span| span.class == HighlightClass::Comment)
        .collect();
    assert!(
        comments
            .iter()
            .any(|span| span.modifiers.contains(HighlightModifier::Documentation)),
        "the `///` comments must carry `documentation`: {comments:?}"
    );
}

#[test]
fn escapes_are_separate_escape_spans() {
    let case = corpus_cases()
        .into_iter()
        .find(|case| case.id == "shared/escapes")
        .expect("shared/escapes case");
    let result = highlight(&case.source, case.surface, HighlightOptions::strict());
    let escapes: Vec<&str> = result
        .spans
        .iter()
        .filter(|span| span.class == HighlightClass::Escape)
        .map(|span| &case.source[span.start..span.end])
        .collect();
    assert!(
        escapes.contains(&"\\t"),
        "the tab escape is one escape span: {escapes:?}"
    );
    assert!(
        escapes.iter().any(|escape| escape.starts_with("\\u")),
        "the unicode escape is one escape span: {escapes:?}"
    );
}

#[test]
fn crlf_is_preserved_in_the_source_hash_and_spans() {
    let case = corpus_cases()
        .into_iter()
        .find(|case| case.id == "shared/crlf")
        .expect("shared/crlf case");
    assert!(
        case.source.contains("\r\n"),
        "the CRLF fixture must keep CRLF line endings in the checkout"
    );
    let result = highlight(&case.source, case.surface, HighlightOptions::strict());
    assert_invariants(&case.source, &result.spans);
    for span in &result.spans {
        assert!(
            !case.source[span.start..span.end].contains('\r'),
            "a span must not swallow the carriage return: {span:?}"
        );
    }
    let artifact = HighlightArtifact::new(&case.source, case.surface, result);
    assert_eq!(
        artifact.source_hash,
        phero_highlight::sha256_source_hash(&case.source),
        "the hash covers the exact CRLF bytes"
    );
}

#[test]
fn contextual_words_stay_ordinary_outside_their_grammar_positions() {
    let case = corpus_cases()
        .into_iter()
        .find(|case| case.id == "ddd/contextual-ordinary")
        .expect("ddd/contextual-ordinary case");
    let result = highlight(&case.source, case.surface, HighlightOptions::strict());
    for word in ["service", "decision"] {
        let spans = spans_for(&case.source, &result.spans, word);
        assert!(
            !spans.is_empty(),
            "`{word}` should still be highlighted as an identifier"
        );
        // The declaration position may be a keyword; the parameter position is
        // an ordinary identifier with the parameter class.
        assert!(
            spans
                .iter()
                .any(|span| span.class == HighlightClass::Parameter),
            "`{word}` keeps an ordinary parameter span: {spans:?}"
        );
        assert!(
            spans
                .iter()
                .any(|span| span.class != HighlightClass::Keyword),
            "`{word}` is not a keyword everywhere: {spans:?}"
        );
    }
}

#[test]
fn ifml_lookalikes_are_not_keywords() {
    let case = corpus_cases()
        .into_iter()
        .find(|case| case.id == "ifml/lookalikes")
        .expect("ifml/lookalikes case");
    let result = highlight(&case.source, case.surface, HighlightOptions::strict());

    for word in [
        "class",
        "service",
        "deployment",
        "event",
        "decision",
        "case",
    ] {
        let spans = spans_for(&case.source, &result.spans, word);
        assert!(!spans.is_empty(), "`{word}` should be an identifier span");
        assert!(
            spans
                .iter()
                .all(|span| span.class != HighlightClass::Keyword),
            "`{word}` is a keyword in another surface, never in .ifml: {spans:?}"
        );
    }

    for word in ["view", "params", "label", "component"] {
        let spans = spans_for(&case.source, &result.spans, word);
        assert!(
            spans
                .iter()
                .any(|span| span.class == HighlightClass::Keyword),
            "`{word}` is a structural .ifml word: {spans:?}"
        );
    }
}

#[test]
fn invalid_format_versions_are_rejected_from_committed_fixtures() {
    let root = repository_root().join("tests/highlight/invalid");
    let two = fs::read_to_string(root.join("format-version-2.json")).expect("version 2 fixture");
    assert!(matches!(
        HighlightArtifact::from_json(&two),
        Err(HighlightError::UnsupportedFormatVersion {
            found: 2,
            expected: HIGHLIGHT_FORMAT_VERSION
        })
    ));

    let absent =
        fs::read_to_string(root.join("format-version-absent.json")).expect("absent fixture");
    assert!(matches!(
        HighlightArtifact::from_json(&absent),
        Err(HighlightError::UnsupportedFormatVersion {
            found: 0,
            expected: HIGHLIGHT_FORMAT_VERSION
        })
    ));
}
