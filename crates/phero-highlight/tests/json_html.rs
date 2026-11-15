//! Wire and rendering contract tests: JSON round-trips, the version gate, HTML
//! escaping, source hashing, and UTF-16 boundary conversion.

use phero_highlight::{
    highlight, sha256_source_hash, to_html, to_utf16, HighlightArtifact, HighlightError,
    HighlightOptions, Surface, Utf16HighlightSpan, HIGHLIGHT_FORMAT_VERSION,
};

fn highlight_surface(source: &str, surface: Surface) -> phero_highlight::HighlightResult {
    highlight(source, surface, HighlightOptions::default())
}

#[test]
fn pretty_json_is_deterministic_and_version_first() {
    let source = "class Book { String title }";
    let result = highlight_surface(source, Surface::Mox);
    let first = HighlightArtifact::new(source, Surface::Mox, result.clone())
        .to_json_pretty()
        .expect("serialize");
    let second = HighlightArtifact::new(source, Surface::Mox, result.clone())
        .to_json_pretty()
        .expect("serialize");
    assert_eq!(first, second);
    assert!(first.starts_with("{\n  \"formatVersion\": 1,"));
    assert!(first.contains("\"surface\": \"mox\""));
    assert!(first.contains("\"degraded\": false"));
}

#[test]
fn every_surface_round_trips() {
    for surface in Surface::ALL {
        let result = highlight_surface("", surface);
        let artifact = HighlightArtifact::new("", surface, result);
        let json = artifact.to_json_pretty().expect("serialize");
        let parsed = HighlightArtifact::from_json(&json).expect("parse");
        assert_eq!(parsed, artifact);
        assert_eq!(parsed.surface, surface);
        assert_eq!(parsed.format_version, HIGHLIGHT_FORMAT_VERSION);
    }
}

#[test]
fn version_gate_rejects_absent_and_other_versions() {
    let json = r#"{"formatVersion": 2, "surface": "mox", "sourceHash": "sha256:00", "generator": {"name": "pherolang", "version": "0"}, "degraded": false, "spans": []}"#;
    assert!(matches!(
        HighlightArtifact::from_json(json),
        Err(HighlightError::UnsupportedFormatVersion {
            found: 2,
            expected: 1
        })
    ));
    let absent = r#"{"surface": "mox", "sourceHash": "sha256:00", "generator": {"name": "pherolang", "version": "0"}, "degraded": false, "spans": []}"#;
    assert!(matches!(
        HighlightArtifact::from_json(absent),
        Err(HighlightError::UnsupportedFormatVersion { found: 0, .. })
    ));
}

#[test]
fn unknown_fields_are_ignored_and_invalid_json_errors() {
    let source = "class A { }";
    let result = highlight_surface(source, Surface::Mox);
    let artifact = HighlightArtifact::new(source, Surface::Mox, result.clone());
    let json = artifact.to_json_pretty().expect("serialize");
    let with_extra = json.replace("{\n", "{\n  \"futureField\": {\"nested\": [1, 2, 3]},\n");
    assert_eq!(
        HighlightArtifact::from_json(&with_extra).expect("unknown fields ignored"),
        artifact
    );
    assert!(matches!(
        HighlightArtifact::from_json("not json"),
        Err(HighlightError::Json(_))
    ));
}

#[test]
fn source_hash_goldens() {
    assert_eq!(
        sha256_source_hash("abc"),
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_source_hash(""),
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn json_never_contains_an_absolute_path() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("manifest dir");
    let source = "class A { }";
    let result = highlight_surface(source, Surface::Mox);
    let json = HighlightArtifact::new(source, Surface::Mox, result.clone())
        .to_json_pretty()
        .expect("serialize");
    assert!(!json.contains(&root.display().to_string()));
    assert!(!json.contains("/home/"));
    assert!(!json.contains("examples/"));
}

#[test]
fn html_output_escapes_untrusted_text() {
    let source = "// <script>alert(\"x & y\")</script>\nclass A { }";
    let result = highlight_surface(source, Surface::Mox);
    let html = to_html(source, &result.spans, Surface::Mox);
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(html.contains("&amp;"));
    assert!(html.contains("&quot;"));
    assert!(html.starts_with("<pre class=\"pherolang-highlight\">"));
    assert!(html.ends_with("</code></pre>"));
    assert!(!html.ends_with('\n'));
}

#[test]
fn html_is_deterministic_and_validates_spans() {
    let source = "class A { }";
    let result = highlight_surface(source, Surface::Mox);
    let first = to_html(source, &result.spans, Surface::Mox);
    let second = to_html(source, &result.spans, Surface::Mox);
    assert_eq!(first, second);
    assert!(first.contains("hl-keyword"));
    assert!(first.contains("hl-type hl-declaration"));
}

#[test]
fn utf16_conversion_handles_unicode_emoji_combining_and_crlf() {
    // "a😀e\u{301}\r\nZ"
    let source = "a😀e\u{301}\r\nZ";
    assert_eq!(source.len(), 11);
    let spans: Vec<phero_highlight::HighlightSpan> = vec![
        phero_highlight::HighlightSpan::new(0, 1, phero_highlight::HighlightClass::Variable),
        phero_highlight::HighlightSpan::new(1, 5, phero_highlight::HighlightClass::Type),
        phero_highlight::HighlightSpan::new(5, 9, phero_highlight::HighlightClass::Property),
    ];
    let converted = to_utf16(source, &spans);
    assert_eq!(
        converted,
        vec![
            Utf16HighlightSpan {
                start: 0,
                end: 1,
                class: phero_highlight::HighlightClass::Variable,
                modifiers: phero_highlight::HighlightModifiers::new(),
            },
            Utf16HighlightSpan {
                start: 1,
                end: 3,
                class: phero_highlight::HighlightClass::Type,
                modifiers: phero_highlight::HighlightModifiers::new(),
            },
            Utf16HighlightSpan {
                start: 3,
                end: 6,
                class: phero_highlight::HighlightClass::Property,
                modifiers: phero_highlight::HighlightModifiers::new(),
            },
        ]
    );
}

#[test]
fn utf16_out_of_bounds_offsets_never_panic() {
    let source = "a😀b";
    let spans = vec![
        phero_highlight::HighlightSpan::new(0, 999, phero_highlight::HighlightClass::Type),
        phero_highlight::HighlightSpan::new(999, 1000, phero_highlight::HighlightClass::Error),
        phero_highlight::HighlightSpan::new(2, 3, phero_highlight::HighlightClass::String),
    ];
    let converted = to_utf16(source, &spans);
    assert!(converted.iter().all(|span| span.end <= 4));
    assert!(converted.iter().all(|span| span.start <= span.end));
}
