use serde::{Deserialize, Serialize};

/// The only [`DomainFingerprint::algorithm`] this crate writes: SHA-256.
///
/// The vocabulary is closed for now; a future algorithm is an additive value
/// and an unknown one is refused by the artifact-only contract.
pub const DOMAIN_FINGERPRINT_ALGORITHM: &str = "sha256";

/// The domain shape a design artifact was compiled against (DF1, issue
/// pherolang#324).
///
/// A [`DddModel`](crate::ddd::DddModel) is meaningful only with the domain
/// [`Model`](crate::Model) it resolved its classes against. This block is the
/// pairing key: the consumer recomputes it over the domain artifact it
/// intends to use and refuses generation on a mismatch. It is a **staleness
/// check**, not a signature or an attestation.
///
/// The field is emitted on every compiler-written design artifact, including
/// one with no domain imports (`packages: []`, the hash of the empty input).
///
/// Serialized shape:
///
/// ```json
/// {"algorithm":"sha256","value":"9f2c…","packages":["nz.example.support"]}
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainFingerprint {
    /// The hash algorithm. [`DOMAIN_FINGERPRINT_ALGORITHM`] for artifacts this
    /// crate writes; any other value is refused by the contract check.
    pub algorithm: String,
    /// The lowercase hexadecimal SHA-256 digest of the canonicalized domain
    /// shape.
    pub value: String,
    /// The sorted, de-duplicated union of every imported domain model's
    /// package names. Always serialized (possibly empty): it makes a mismatch
    /// diagnostic readable without re-hashing.
    pub packages: Vec<String>,
}

impl DomainFingerprint {
    /// Creates a SHA-256 fingerprint over `value` naming `packages`.
    pub fn sha256(value: impl Into<String>, packages: Vec<String>) -> Self {
        Self {
            algorithm: DOMAIN_FINGERPRINT_ALGORITHM.to_string(),
            value: value.into(),
            packages,
        }
    }

    /// Whether [`Self::algorithm`] is the known SHA-256 spelling.
    pub fn uses_sha256(&self) -> bool {
        self.algorithm == DOMAIN_FINGERPRINT_ALGORITHM
    }

    /// Whether [`Self::value`] is a well-formed lowercase SHA-256 hex digest
    /// (64 hex digits).
    pub fn value_is_sha256_hex(&self) -> bool {
        self.value.len() == 64
            && self
                .value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    /// Whether [`Self::packages`] is sorted and de-duplicated.
    pub fn packages_are_sorted_unique(&self) -> bool {
        self.packages.windows(2).all(|pair| pair[0] < pair[1])
    }
}
