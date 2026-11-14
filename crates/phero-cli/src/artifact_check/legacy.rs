//! Legacy (0.2-era) artifact detection — the pre-rename wire markers.
//!
//! This is the **only** module allowed to spell the old wire keys: it exists
//! to recognize artifacts produced before the Phero Lang rename and reject
//! them with one actionable message instead of a generic `formatVersion`
//! mismatch or an `unknown root $type` error.
//!
//! Two markers identify a 0.2-era artifact, and either one is enough:
//!
//! - the old provenance key (`rexVersion`) at the document root, and
//! - the old canonical-instance discriminator (`rex.instance`).
//!
//! The detection runs as **rule 0** of [`super::check`] — before the root
//! `$type` branch and before the `packages` branch — so both a legacy IR
//! artifact and a legacy instance are named as such, never misclassified.

use serde_json::{Map, Value};

/// The provenance key 0.2-era artifacts carried (renamed to `pheroVersion`).
pub(crate) const LEGACY_PROVENANCE_KEY: &str = "rexVersion";

/// The canonical-instance root discriminator 0.2-era artifacts carried
/// (renamed to `phero.instance`).
pub(crate) const LEGACY_INSTANCE_TYPE: &str = "rex.instance";

/// The one message a legacy artifact gets: what it is and how to fix it.
pub(crate) const LEGACY_MESSAGE: &str = "0.2-era rexlang artifact — regenerate with pherolang 0.3";

/// Reports the legacy marker a document root carries, if any.
///
/// Returns [`LEGACY_MESSAGE`] when the root carries the old provenance key
/// or declares the old instance discriminator; `None` otherwise.
pub(crate) fn detect(root: &Map<String, Value>) -> Option<&'static str> {
    let legacy_provenance = root.contains_key(LEGACY_PROVENANCE_KEY);
    let legacy_instance = root.get("$type").and_then(Value::as_str) == Some(LEGACY_INSTANCE_TYPE);
    (legacy_provenance || legacy_instance).then_some(LEGACY_MESSAGE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(json: &str) -> Map<String, Value> {
        serde_json::from_str::<Value>(json)
            .expect("test document is JSON")
            .as_object()
            .expect("test document is an object")
            .clone()
    }

    #[test]
    fn the_message_is_exact() {
        assert_eq!(
            LEGACY_MESSAGE,
            "0.2-era rexlang artifact — regenerate with pherolang 0.3"
        );
    }

    #[test]
    fn the_old_provenance_key_is_legacy() {
        let root = object(r#"{"formatVersion": 1, "rexVersion": "0.2.0", "packages": []}"#);
        assert_eq!(detect(&root), Some(LEGACY_MESSAGE));
    }

    #[test]
    fn the_old_instance_discriminator_is_legacy() {
        let root = object(r#"{"$type": "rex.instance", "formatVersion": 1, "objects": []}"#);
        assert_eq!(detect(&root), Some(LEGACY_MESSAGE));
    }

    #[test]
    fn a_current_document_is_not_legacy() {
        let ir = object(r#"{"formatVersion": 2, "pheroVersion": "0.3.0", "packages": []}"#);
        assert_eq!(detect(&ir), None, "the renamed provenance key is current");

        let instance = object(
            r#"{"$type": "phero.instance", "formatVersion": 2, "model": "m", "objects": []}"#,
        );
        assert_eq!(
            detect(&instance),
            None,
            "the renamed discriminator is current"
        );
    }

    #[test]
    fn a_format_version_two_document_with_neither_marker_is_not_legacy() {
        let root = object(r#"{"formatVersion": 2, "packages": []}"#);
        assert_eq!(detect(&root), None);
    }

    #[test]
    fn the_old_provenance_key_wins_even_on_a_version_two_document() {
        // The marker is decisive, not `formatVersion`: a document that claims
        // the new version but still carries the old key is a 0.2-era artifact
        // with a hand-edited version field, and the message must say so.
        let root = object(r#"{"formatVersion": 2, "rexVersion": "0.2.0", "packages": []}"#);
        assert_eq!(detect(&root), Some(LEGACY_MESSAGE));
    }
}
