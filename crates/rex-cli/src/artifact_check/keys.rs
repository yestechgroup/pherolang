//! The structural-key scan: the snake_case shadow detector, run against the
//! camelCase vocabulary shared from [`rex_backend_common::STRUCTURAL_KEYS`]
//! (the corpus has exactly one home — the sortedness invariant travels with
//! it).

use rex_backend_common::{camel_case, STRUCTURAL_KEYS};
use serde_json::Value;

/// `packages`/`blocks` sanity: an array of objects each carrying a string
/// `name`.
pub(super) fn check_named_array(value: &Value, key: &str, errors: &mut Vec<String>) {
    let Some(items) = value.as_array() else {
        errors.push(format!("{key:?} must be an array, found {value}"));
        return;
    };
    for (index, item) in items.iter().enumerate() {
        match item.as_object() {
            None => errors.push(format!("{key}[{index}] must be an object, found {item}")),
            Some(object) => match object.get("name") {
                None => errors.push(format!("{key}[{index}] is missing \"name\"")),
                Some(name) if name.as_str().is_none_or(str::is_empty) => errors.push(format!(
                    "{key}[{index}] \"name\" must be a non-empty string, found {name}"
                )),
                _ => {}
            },
        }
    }
}

/// Reports every key whose spelling is snake_case but whose camelCase form
/// is a known structural key — the readers ignore unknown fields (forward
/// compatibility), so such artifacts load but silently drop the value.
///
/// Runs on IR, actor, DDD design, event, deployment, and use-case artifacts
/// only: their keys are entirely structural. Instance objects' feature keys
/// are model-defined names, so instance documents are exempt.
pub(super) fn scan_snake_case_keys(value: &Value, errors: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key.contains('_') {
                    let camel_case = camel_case(key);
                    if STRUCTURAL_KEYS.binary_search(&camel_case.as_str()).is_ok() {
                        errors.push(format!(
                            "key {key:?} is not wire format: the camelCase key is \
                             {camel_case:?}"
                        ));
                    }
                }
                scan_snake_case_keys(child, errors);
            }
        }
        Value::Array(items) => {
            for item in items {
                scan_snake_case_keys(item, errors);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The consumer-side half of the shared-corpus contract: the detector
    /// actually fires on a snake_case spelling of a corpus key (here a real
    /// IR field), naming both the violation and its camelCase spelling. The
    /// corpus's sortedness invariant is pinned once, in
    /// `rex_backend_common`; this test is what a drift there would break.
    #[test]
    fn detector_fires_on_snake_case_structural_key() {
        let value = serde_json::json!({
            "classes": [{ "name": "Writer", "is_read_only": false }],
        });
        let mut errors = Vec::new();
        scan_snake_case_keys(&value, &mut errors);
        assert_eq!(errors.len(), 1, "errors: {errors:?}");
        assert!(
            errors[0].contains("is_read_only") && errors[0].contains("isReadOnly"),
            "the violation and its camelCase spelling must both be named: {errors:?}"
        );
    }

    /// A corpus that is not sorted defeats the detector's binary search —
    /// recorded here so nobody mistakes a stale corpus for "no violations".
    /// The production corpus is not made mutable for this: the check runs
    /// over a local unsorted copy and the real list's order is pinned once,
    /// in `rex_backend_common`.
    #[test]
    fn unsorted_corpus_defeats_the_binary_search() {
        // The corpus is sorted, so "attributes" belongs between
        // "annotations" and "when"; appending it is the drift the invariant
        // forbids, and the binary search then never looks at it.
        let appended_at_the_end = ["annotations", "when", "attributes"];
        assert!(
            appended_at_the_end.binary_search(&"attributes").is_err(),
            "an unsorted corpus must miss a key the detector should flag"
        );
    }
}
