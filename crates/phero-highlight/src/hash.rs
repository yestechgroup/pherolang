//! Source hashing for highlight artifacts.
//!
//! An artifact records the SHA-256 of the exact source bytes it describes so a
//! consumer can detect a stale artifact without embedding the source (and
//! without ever putting an absolute path on the wire).

use sha2::{Digest as _, Sha256};

/// The SHA-256 digest of `source` formatted `sha256:<64 lowercase hex>`.
#[must_use]
pub fn sha256_source_hash(source: &str) -> String {
    let digest = Sha256::digest(source.as_bytes());
    let mut out = String::with_capacity(7 + digest.len() * 2);
    out.push_str("sha256:");
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_digests_are_stable() {
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
    fn the_spelling_is_lowercase_hex_with_a_prefix() {
        let hash = sha256_source_hash("class A");
        assert!(hash.starts_with("sha256:"));
        let hex = &hash["sha256:".len()..];
        assert_eq!(hex.len(), 64);
        assert!(hex
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
}
