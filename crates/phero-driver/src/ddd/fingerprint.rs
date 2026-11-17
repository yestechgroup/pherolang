//! The domain pairing fingerprint (DF1, issue pherolang#324).
//!
//! A design artifact names the domain shape it was compiled against so a
//! consumer can refuse a stale `.ddd`/`.mox` pair before generating. The
//! compiler computes the fingerprint here — the driver owns domain lowering,
//! and `phero-ir` carries only the value (no hashing in the IR crate).
//!
//! # What is hashed (normative)
//!
//! The **shape** each imported domain [`Model`] contributes, not provenance:
//!
//! - every imported domain model's canonical JSON, with `pheroVersion`
//!   stripped (a compiler bump must not invalidate a pair);
//! - `formatVersion` and every package/class/enum/datatype/interface/
//!   vocabulary/feature/type/multiplicity/constraint/opposite value kept;
//! - models ordered by their sorted package names (canonical bytes as the
//!   deterministic tie-break), each prefixed with its length as a big-endian
//!   `u64` so concatenation is unambiguous;
//! - SHA-256 over that byte stream, lowercase hex.
//!
//! Not hashed: the `.ddd` source, the `.mox` source text, `import
//! schema`/`import sigil` raw content (the lowered model already carries the
//! resolved types), `.actor`/`.dmn` artifacts (their absence is legal), and
//! the design's own `pheroVersion`.
//!
//! The fingerprint is a **staleness check**, not a signature or attestation.

use sha2::{Digest, Sha256};

use phero_ir::ddd::DomainFingerprint;
use phero_ir::Model;

/// Computes the pairing fingerprint over every imported domain model.
///
/// The empty slice is the no-imports case: `packages: []` and the SHA-256 of
/// the empty byte stream, so every compiler-written design carries a stable
/// value.
pub fn domain_fingerprint(domains: &[Model]) -> DomainFingerprint {
    let refs: Vec<&Model> = domains.iter().collect();
    domain_fingerprint_refs(&refs)
}

/// The borrow-taking twin of [`domain_fingerprint`], used by the compile
/// pipeline (which holds [`DomainUnit`](crate::lower::DomainUnit) models by
/// reference).
pub(crate) fn domain_fingerprint_refs(domains: &[&Model]) -> DomainFingerprint {
    let mut packages: Vec<String> = domains
        .iter()
        .flat_map(|model| model.packages.iter().map(|package| package.name.clone()))
        .collect();
    packages.sort();
    packages.dedup();

    // Order by the model's sorted package names, then by canonical bytes so
    // two models in one package still hash deterministically.
    let mut entries: Vec<(String, String)> = domains
        .iter()
        .map(|model| {
            let mut names: Vec<&str> = model
                .packages
                .iter()
                .map(|package| package.name.as_str())
                .collect();
            names.sort_unstable();
            (names.join("\0"), canonical_domain_json(model))
        })
        .collect();
    entries.sort();

    let mut hasher = Sha256::new();
    for (_, json) in &entries {
        hasher.update((json.len() as u64).to_be_bytes());
        hasher.update(json.as_bytes());
    }
    let digest = hasher.finalize();
    let mut value = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(value, "{byte:02x}");
    }

    DomainFingerprint::sha256(value, packages)
}

/// Canonical JSON for one domain model: `pheroVersion` removed and object
/// keys sorted recursively.
fn canonical_domain_json(model: &Model) -> String {
    let mut value = serde_json::to_value(model).expect("a domain model serializes");
    if let Some(object) = value.as_object_mut() {
        object.remove("pheroVersion");
    }
    serde_json::to_string(&canonical_key_order(value)).expect("canonical JSON serializes")
}

/// Recursively sorts object keys so the digest is independent of map
/// insertion order (serde_json's `preserve_order` feature flips `Map` to
/// insertion order workspace-wide).
fn canonical_key_order(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();
            let mut ordered = serde_json::Map::new();
            for key in keys {
                let entry = map.get(&key).cloned().expect("the key came from the map");
                ordered.insert(key, canonical_key_order(entry));
            }
            serde_json::Value::Object(ordered)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(canonical_key_order).collect())
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phero_ir::{Package, PrimitiveType};

    fn model(package: &str, version: &str) -> Model {
        let mut model = Model::new();
        model.phero_version = Some(version.to_string());
        let mut pkg = Package::new(package);
        pkg.classes.push(phero_ir::ClassDef::new(
            "Order",
            vec![],
            vec![phero_ir::Feature::new(
                "total",
                phero_ir::FeatureKind::Attribute,
                phero_ir::TypeRef::Primitive(PrimitiveType::Int),
                phero_ir::Multiplicity::REQUIRED,
            )],
        ));
        model.packages.push(pkg);
        model
    }

    #[test]
    fn empty_domains_are_stable_and_shaped() {
        let fingerprint = domain_fingerprint(&[]);
        assert_eq!(fingerprint.algorithm, "sha256");
        assert_eq!(
            fingerprint.value,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert!(fingerprint.packages.is_empty());
        assert!(fingerprint.value_is_sha256_hex());
    }

    #[test]
    fn compiler_version_is_not_hashed() {
        let first = domain_fingerprint(&[model("demo", "0.4.0")]);
        let second = domain_fingerprint(&[model("demo", "9.9.9")]);
        assert_eq!(first, second);
    }

    #[test]
    fn package_order_does_not_change_the_hash() {
        let first = domain_fingerprint(&[model("a", "0.4.0"), model("b", "0.4.0")]);
        let second = domain_fingerprint(&[model("b", "0.4.0"), model("a", "0.4.0")]);
        assert_eq!(first, second);
    }

    #[test]
    fn a_shape_change_changes_the_hash() {
        let mut changed = model("demo", "0.4.0");
        changed.packages[0].classes[0].features[0].type_ =
            phero_ir::TypeRef::Primitive(PrimitiveType::String);
        assert_ne!(
            domain_fingerprint(&[model("demo", "0.4.0")]).value,
            domain_fingerprint(&[changed]).value
        );
    }
}
