//! The versioned highlight artifact wire format.
//!
//! The shape is a hand-written struct (not a macro) because this crate is not
//! an artifact root of `phero-ir`; the semantics are the same house contract:
//!
//! - camelCase fields, `formatVersion` first, adjacent tagging;
//! - [`HIGHLIGHT_FORMAT_VERSION`] is the independent marker and is gated on
//!   read (absent reports as `0`);
//! - unknown JSON fields are ignored for forward compatibility;
//! - pretty JSON is deterministic (struct field order, two-space indent);
//! - the source is represented by its `sha256:` hash, never by a path.
//!
//! `docs/HIGHLIGHTING.md` owns the normative wire contract; this module is its
//! implementation.

use serde::{Deserialize, Serialize};

use crate::class::{HighlightClass, HighlightModifier};
use crate::engine::HighlightResult;
use crate::hash::sha256_source_hash;
use crate::surface::Surface;

/// The highlight artifact format version this crate writes and reads.
pub const HIGHLIGHT_FORMAT_VERSION: u32 = 1;

/// The generator name stamped on every artifact.
pub const GENERATOR_NAME: &str = "pherolang";

/// The generator version stamped on every artifact.
pub const GENERATOR_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The wire shape of one highlighted span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireSpan {
    /// Start byte offset (inclusive).
    pub start: usize,
    /// End byte offset (exclusive).
    pub end: usize,
    /// The semantic class (wire spelling).
    pub class: HighlightClass,
    /// The modifier array, in canonical order.
    pub modifiers: Vec<HighlightModifier>,
}

impl WireSpan {
    /// Builds the wire span for a core span.
    #[must_use]
    pub fn from_span(span: &crate::class::HighlightSpan) -> Self {
        Self {
            start: span.start,
            end: span.end,
            class: span.class,
            modifiers: span.modifiers.to_vec(),
        }
    }
}

/// The generator stamp: the tool that produced the artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Generator {
    /// The generator name (`pherolang`).
    pub name: String,
    /// The generating crate's version.
    pub version: String,
}

impl Default for Generator {
    fn default() -> Self {
        Self {
            name: GENERATOR_NAME.to_string(),
            version: GENERATOR_VERSION.to_string(),
        }
    }
}

/// A versioned highlight artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HighlightArtifact {
    /// The format version marker (`formatVersion`).
    #[serde(default)]
    pub format_version: u32,
    /// The surface the source belongs to.
    pub surface: Surface,
    /// The SHA-256 digest of the source, `sha256:<hex>`.
    pub source_hash: String,
    /// The generator stamp.
    pub generator: Generator,
    /// Whether highlighting degraded to a best-effort result.
    pub degraded: bool,
    /// The highlighted spans, byte offsets, sorted and non-overlapping.
    pub spans: Vec<WireSpan>,
}

impl HighlightArtifact {
    /// Builds an artifact from a highlight result.
    #[must_use]
    pub fn new(source: &str, surface: Surface, result: HighlightResult) -> Self {
        Self {
            format_version: HIGHLIGHT_FORMAT_VERSION,
            surface,
            source_hash: sha256_source_hash(source),
            generator: Generator::default(),
            degraded: result.degraded,
            spans: result.spans.iter().map(WireSpan::from_span).collect(),
        }
    }

    /// Builds an artifact from a borrowed highlight result.
    #[must_use]
    pub fn from_result(source: &str, surface: Surface, result: &HighlightResult) -> Self {
        Self::new(source, surface, result.clone())
    }

    /// Serializes to compact JSON.
    pub fn to_json(&self) -> Result<String, HighlightError> {
        Ok(serde_json::to_string(self)?)
    }

    /// Serializes to pretty-printed (two-space indent) JSON.
    pub fn to_json_pretty(&self) -> Result<String, HighlightError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Deserializes and gates on [`HIGHLIGHT_FORMAT_VERSION`].
    ///
    /// Unknown fields are ignored; an absent `formatVersion` reports as `0`.
    pub fn from_json(json: &str) -> Result<Self, HighlightError> {
        let artifact: Self = serde_json::from_str(json)?;
        if artifact.format_version != HIGHLIGHT_FORMAT_VERSION {
            return Err(HighlightError::UnsupportedFormatVersion {
                found: artifact.format_version,
                expected: HIGHLIGHT_FORMAT_VERSION,
            });
        }
        Ok(artifact)
    }
}

/// A highlight artifact error.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum HighlightError {
    /// The artifact's `formatVersion` is absent or not the version this crate
    /// reads.
    #[error("unsupported highlight artifact formatVersion: found {found}, expected {expected}")]
    UnsupportedFormatVersion {
        /// The version found (0 when the field was absent).
        found: u32,
        /// The version this crate supports.
        expected: u32,
    },
    /// The artifact is not valid JSON, or does not match the schema.
    #[error("invalid highlight artifact JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{highlight, HighlightOptions};

    fn artifact(source: &str) -> HighlightArtifact {
        let result = highlight(source, Surface::Mox, HighlightOptions::default());
        HighlightArtifact::new(source, Surface::Mox, result)
    }

    #[test]
    fn docs_contract_example_matches_the_wire_contract() {
        // The `.mox` attribute grammar is type-first (`String name`), so the
        // contract example uses the compiler-correct spelling.
        let source = "package shop\n\nclass Order {\n    String name\n}\n";
        let result = highlight(source, Surface::Mox, HighlightOptions::default());
        assert!(!result.degraded);
        let artifact = HighlightArtifact::new(source, Surface::Mox, result);
        assert_eq!(
            artifact.source_hash,
            "sha256:44d3914d8c335d4824a9ceabb439c82aa544896343870ba260ad17c92eee613b"
        );
        let spans: Vec<(usize, usize, HighlightClass, Vec<HighlightModifier>)> = artifact
            .spans
            .iter()
            .map(|span| (span.start, span.end, span.class, span.modifiers.clone()))
            .collect();
        assert_eq!(
            spans,
            vec![
                (0, 7, HighlightClass::Keyword, vec![]),
                (
                    8,
                    12,
                    HighlightClass::Namespace,
                    vec![HighlightModifier::Declaration]
                ),
                (14, 19, HighlightClass::Keyword, vec![]),
                (
                    20,
                    25,
                    HighlightClass::Type,
                    vec![HighlightModifier::Declaration]
                ),
                (26, 27, HighlightClass::Punctuation, vec![]),
                (
                    32,
                    38,
                    HighlightClass::Type,
                    vec![HighlightModifier::DefaultLibrary]
                ),
                (
                    39,
                    43,
                    HighlightClass::Property,
                    vec![HighlightModifier::Declaration]
                ),
                (44, 45, HighlightClass::Punctuation, vec![]),
            ]
        );
    }

    #[test]
    fn wire_shape_puts_format_version_first() {
        let json = artifact("class A { }").to_json_pretty().expect("serialize");
        assert!(json.starts_with("{\n  \"formatVersion\": 1,"), "{json}");
        assert!(json.contains("\"surface\": \"mox\""));
        assert!(json.contains("\"sourceHash\": \"sha256:"));
        assert!(json.contains("\"generator\": {"));
        assert!(json.contains("\"name\": \"pherolang\""));
        assert!(json.contains("\"version\": \""));
        assert!(json.contains("\"degraded\": false"));
        assert!(json.contains("\"spans\": ["));
    }

    #[test]
    fn round_trip_is_lossless_and_deterministic() {
        let artifact = artifact("class A { int x }");
        let json = artifact.to_json_pretty().expect("serialize");
        let parsed = HighlightArtifact::from_json(&json).expect("parse");
        assert_eq!(parsed, artifact);
        assert_eq!(parsed.to_json_pretty().expect("reserialize"), json);
        assert_eq!(
            parsed.to_json().expect("compact"),
            artifact.to_json().expect("compact")
        );
    }

    #[test]
    fn version_gate_rejects_absent_zero_and_other_versions() {
        for version in ["0", "2", "999"] {
            let json = format!(
                r#"{{"formatVersion": {version}, "surface": "mox", "sourceHash": "sha256:00", "generator": {{"name": "pherolang", "version": "0"}}, "degraded": false, "spans": []}}"#
            );
            match HighlightArtifact::from_json(&json) {
                Err(HighlightError::UnsupportedFormatVersion { found, expected }) => {
                    assert_eq!(found, version.parse::<u32>().expect("number"));
                    assert_eq!(expected, HIGHLIGHT_FORMAT_VERSION);
                }
                other => panic!("expected a version rejection, got {other:?}"),
            }
        }
        let absent = r#"{"surface": "mox", "sourceHash": "sha256:00", "generator": {"name": "pherolang", "version": "0"}, "degraded": false, "spans": []}"#;
        assert!(matches!(
            HighlightArtifact::from_json(absent),
            Err(HighlightError::UnsupportedFormatVersion { found: 0, .. })
        ));
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let json = artifact("class A").to_json_pretty().expect("serialize");
        let at = json.find("\"spans\"").expect("spans key");
        let mut with_extra = json.clone();
        with_extra.insert_str(at, "\"someFutureField\": {\"nested\": true},\n  ");
        let parsed = HighlightArtifact::from_json(&with_extra).expect("unknown fields ignored");
        assert_eq!(parsed, artifact("class A"));
    }

    #[test]
    fn the_artifact_never_carries_a_path() {
        let json = artifact("class A").to_json_pretty().expect("serialize");
        assert!(!json.contains("examples/"));
        assert!(!json.contains("/home/"));
        assert!(!json.contains("\\\\"));
    }

    #[test]
    fn modifiers_serialize_as_ordered_arrays() {
        let result = highlight(
            "/// docs\nclass A",
            Surface::Mox,
            HighlightOptions::default(),
        );
        let artifact = HighlightArtifact::new("/// docs\nclass A", Surface::Mox, result);
        let json = artifact.to_json_pretty().expect("serialize");
        assert!(json.contains("\"modifiers\": [\n"));
        assert!(json.contains("\"documentation\""));
    }
}
