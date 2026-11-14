//! Shared wire-format machinery for the versioned artifact roots.
//!
//! Every artifact root ([`Model`], [`ActorModel`], [`ddd::DddModel`],
//! [`events::EventModel`], [`deploy::DeployModel`],
//! [`usecases::UseCaseModel`], [`ifml::IfmlModel`],
//! [`ifml_manifest::IfmlManifest`], [`cmmn::CmmnModel`], [`dmn::DmnModel`],
//! [`graph::GraphModel`])
//! carries a `format_version` field and declares its wire plumbing with one
//! [`versioned_artifact!`] invocation next to its struct:
//!
//! ```text
//! crate::wire::versioned_artifact!(GraphModel, GRAPH_MODEL_FORMAT_VERSION, "the graph");
//! ```
//!
//! The serialize/version-gate semantics live once here, as
//! [`VersionedArtifact`]'s default methods. That one invocation expands to
//!
//! - the root's crate-private [`VersionedArtifact`] impl, naming its marker
//!   const (the const is a required argument — a root cannot forget it),
//! - the root's `Default`, delegating to its constructor so the marker is
//!   always stamped, and
//! - the public inherent `to_json` / `to_json_pretty` / `from_json` — the
//!   API external consumers call — each a one-line delegation whose
//!   generated doc names the gate.
//!
//! A new artifact surface is therefore the explicit struct (plus its marker
//! const) and one `versioned_artifact!` line: the gate semantics cannot
//! drift between roots because every root routes through the same defaults.
//!
//! The module also owns the two small pieces of vocabulary-level
//! machinery every artifact file shares: the [`is_false`]
//! `skip_serializing_if` predicate and the [`total_order_float!`]
//! newtype generator.
//!
//! # Version 2: the Phero Lang rename
//!
//! The artifacts gated here now write and accept **`formatVersion` 2**. The
//! bump is the rename's provenance change: the domain root's provenance key
//! and the canonical-instance discriminator were renamed (see
//! `docs/NAMING.md`), so a 0.2-era document and a 0.3 document differ in shape
//! while both would claim version 1 — exactly what the version gate exists
//! to prevent. Every root
//! whose bytes carry one of the renamed markers moved with it; a document
//! still carrying an old marker is rejected by `pherolang artifact check`
//! with the migration message, never read as a current artifact.
//!
//! The markers that do **not** depend on this module's roots are independent
//! and deliberately stayed at 1: [`crate::graph::GRAPH_MODEL_FORMAT_VERSION`]
//! (the graph carries no provenance key), the `graph query` envelope
//! version, and `phero-evidence`'s receipt and proof-register format
//! versions. Their shapes did not change, so bumping them would invalidate
//! committed artifacts for a rename that never touched them.

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::IrError;

/// A versioned artifact root: the top-level type of a serialized
/// `.phero.json` artifact, stamped with and gated on a `formatVersion`.
///
/// The default methods below are the one implementation of the wire
/// plumbing; each root keeps a public inherent method of the same name
/// (external consumers — the driver, the backends, the CLI — cannot see
/// this `pub(crate)` trait) whose body delegates here.
pub(crate) trait VersionedArtifact {
    /// The format version this root writes and accepts.
    const FORMAT_VERSION: u32;

    /// Serializes the artifact to pretty-printed (2-space indent) JSON.
    fn to_json_pretty(&self) -> Result<String, IrError>
    where
        Self: Serialize,
    {
        to_json_pretty(self)
    }

    /// Serializes the artifact to compact JSON.
    fn to_json(&self) -> Result<String, IrError>
    where
        Self: Serialize,
    {
        to_json(self)
    }

    /// Deserializes a versioned artifact from JSON, rejecting artifacts
    /// whose `formatVersion` is absent or not
    /// [`VersionedArtifact::FORMAT_VERSION`] for `Self`.
    fn from_json(json: &str) -> Result<Self, IrError>
    where
        Self: DeserializeOwned + Sized,
    {
        from_json(json)
    }
}

/// Declares one artifact root's complete wire plumbing: the crate-private
/// [`VersionedArtifact`] impl, the `Default`, and the three public inherent
/// delegation methods. The struct definition stays explicit at the call
/// site — only the plumbing is generated.
///
/// The marker const is a **required argument**, so a root cannot retain the
/// trait/gate while forgetting its version marker: omitting it (or naming a
/// non-const) is a compile error.
///
/// The usual invocation, for a root whose `new()` takes no arguments:
///
/// ```text
/// crate::wire::versioned_artifact!(Model, FORMAT_VERSION, "the model");
/// ```
///
/// A root whose constructor needs arguments passes `default = <call>`; the
/// expression is expanded inside `fn default() -> Self`, so `Self::new(…)`
/// works:
///
/// ```text
/// crate::wire::versioned_artifact!(
///     IfmlModel,
///     IFML_MODEL_FORMAT_VERSION,
///     "the IFML artifact",
///     default = Self::new(Vec::new(), Vec::new(), Vec::new(), Vec::new(),
///                        Vec::new(), Vec::new(), Vec::new())
/// );
/// ```
///
/// A root with root-specific forward-compatibility wording replaces the
/// final `from_json` doc line with `unknown = "<text>"`.
///
/// Every generated method is documented: the docs are `concat!`-built so
/// each root's methods still name their own marker const and version gate.
///
/// Two compile-time guards make forgetting impossible: the marker const is
/// a required argument of the trait impl, and an expansion-level
/// `const _: fn(&Root) -> u32` requires the root's `format_version` field.
macro_rules! versioned_artifact {
    ($root:ident, $version:ident, $noun:literal $(,)?) => {
        $crate::wire::versioned_artifact!(
            @plumbing $root,
            $version,
            $noun,
            "Unknown fields are ignored for forward compatibility.",
            $root::new()
        );
    };
    ($root:ident, $version:ident, $noun:literal, unknown = $unknown:expr $(,)?) => {
        $crate::wire::versioned_artifact!(
            @plumbing $root,
            $version,
            $noun,
            $unknown,
            $root::new()
        );
    };
    ($root:ident, $version:ident, $noun:literal, default = $constructor:expr $(,)?) => {
        $crate::wire::versioned_artifact!(
            @plumbing $root,
            $version,
            $noun,
            "Unknown fields are ignored for forward compatibility.",
            $constructor
        );
    };
    (@plumbing $root:ident, $version:ident, $noun:literal, $unknown:expr, $constructor:expr) => {
        impl $root {
            #[doc = ::core::concat!(
                "Serializes ",
                $noun,
                " to pretty-printed (2-space indent) JSON."
            )]
            pub fn to_json_pretty(&self) -> ::core::result::Result<::std::string::String, $crate::IrError> {
                $crate::wire::VersionedArtifact::to_json_pretty(self)
            }

            #[doc = ::core::concat!("Serializes ", $noun, " to compact JSON.")]
            pub fn to_json(&self) -> ::core::result::Result<::std::string::String, $crate::IrError> {
                $crate::wire::VersionedArtifact::to_json(self)
            }

            #[doc = ::core::concat!(
                "Deserializes ",
                $noun,
                " from JSON, rejecting artifacts whose"
            )]
            #[doc = ::core::concat!(
                "`formatVersion` is not [`",
                ::core::stringify!($version),
                "`]."
            )]
            #[doc = ""]
            #[doc = $unknown]
            pub fn from_json(json: &str) -> ::core::result::Result<Self, $crate::IrError> {
                <Self as $crate::wire::VersionedArtifact>::from_json(json)
            }
        }

        impl $crate::wire::VersionedArtifact for $root {
            const FORMAT_VERSION: u32 = $version;
        }

        impl ::core::default::Default for $root {
            fn default() -> Self {
                $constructor
            }
        }

        // Compile-time guard: the root must actually carry the stamped
        // `format_version` field the `Default` above and the version gate
        // rely on. A root that renames or drops it fails to compile here
        // rather than silently emitting an ungated artifact.
        const _: fn(&$root) -> u32 = |artifact| artifact.format_version;
    };
}

pub(crate) use versioned_artifact;

/// Serializes an artifact to pretty-printed (2-space indent) JSON.
pub(crate) fn to_json_pretty<T: Serialize + ?Sized>(value: &T) -> Result<String, IrError> {
    Ok(serde_json::to_string_pretty(value)?)
}

/// Serializes an artifact to compact JSON.
pub(crate) fn to_json<T: Serialize + ?Sized>(value: &T) -> Result<String, IrError> {
    Ok(serde_json::to_string(value)?)
}

/// Deserializes a versioned artifact from JSON, rejecting artifacts whose
/// `formatVersion` is absent or not [`VersionedArtifact::FORMAT_VERSION`]
/// for `T`.
pub(crate) fn from_json<T: VersionedArtifact + DeserializeOwned>(json: &str) -> Result<T, IrError> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    let found = value
        .get("formatVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default() as u32;
    if found != T::FORMAT_VERSION {
        return Err(IrError::UnsupportedFormatVersion {
            found,
            expected: T::FORMAT_VERSION,
        });
    }
    Ok(serde_json::from_value(value)?)
}

/// The `skip_serializing_if` predicate for `false`-defaulted bool fields:
/// `false` is the absence of the flag on the wire.
pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}

/// Generates a `total_cmp`-ordered float newtype: `#[serde(transparent)]`
/// on the wire, bit-pattern equality and hashing, and a genuine total
/// order (`-0.0 < 0.0`, every `NaN` above infinity), so values can be
/// sorted and compared like any other key. The type-level doc contract
/// stays at the call site.
macro_rules! total_order_float {
    (
        $(#[$doc:meta])*
        $name:ident($inner:ty)
    ) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name($inner);

        impl $name {
            /// Wraps a raw value.
            pub fn new(value: $inner) -> Self {
                Self(value)
            }

            /// The raw value.
            pub fn value(self) -> $inner {
                self.0
            }
        }

        impl From<$inner> for $name {
            fn from(value: $inner) -> Self {
                Self(value)
            }
        }

        impl From<$name> for $inner {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.0.total_cmp(&other.0) == ::std::cmp::Ordering::Equal
            }
        }

        impl Eq for $name {}

        impl ::std::hash::Hash for $name {
            fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
                ::std::hash::Hash::hash(&self.0.to_bits(), state);
            }
        }

        impl ::std::cmp::Ord for $name {
            fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
                self.0.total_cmp(&other.0)
            }
        }

        impl ::std::cmp::PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<::std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
    };
}

pub(crate) use total_order_float;

#[cfg(test)]
pub(crate) mod tests {
    use serde::de::DeserializeOwned;
    use serde::Serialize;
    use std::fmt::Debug;

    use super::{from_json, to_json, to_json_pretty, VersionedArtifact};
    use crate::cmmn::CmmnModel;
    use crate::cmmn::CMMN_MODEL_FORMAT_VERSION;
    use crate::ddd::{DddModel, DDD_MODEL_FORMAT_VERSION};
    use crate::deploy::{DeployModel, DEPLOY_MODEL_FORMAT_VERSION};
    use crate::dmn::{DmnModel, DMN_MODEL_FORMAT_VERSION};
    use crate::events::{EventModel, EVENT_MODEL_FORMAT_VERSION};
    use crate::graph::{GraphModel, GRAPH_MODEL_FORMAT_VERSION};
    use crate::ifml::{IfmlModel, IFML_MODEL_FORMAT_VERSION};
    use crate::ifml_manifest::{IfmlManifest, IFML_MANIFEST_FORMAT_VERSION};
    use crate::usecases::{UseCaseModel, USE_CASE_MODEL_FORMAT_VERSION};
    use crate::{ActorModel, IrError, Model, ACTOR_MODEL_FORMAT_VERSION, FORMAT_VERSION};

    /// One line per artifact root: asserts that its macro-generated plumbing
    /// is complete and agrees with its marker — the crate-private
    /// [`super::VersionedArtifact`] const, the `Default` stamp, and the three
    /// inherent delegations (`from_json`, `to_json`, `to_json_pretty`). A new
    /// root is added to this list (the only per-root list left in this
    /// module).
    macro_rules! assert_root_wired {
        ($root:ident, $version:ident) => {{
            assert_eq!(
                <$root as VersionedArtifact>::FORMAT_VERSION,
                $version,
                concat!(stringify!($root), " must gate on ", stringify!($version))
            );
            let default = $root::default();
            assert_eq!(
                default.format_version, $version,
                concat!(
                    stringify!($root),
                    "::default() must stamp ",
                    stringify!($version)
                )
            );
            let accepted = $root::from_json(&format!("{{\"formatVersion\":{}}}", $version)).expect(
                concat!(stringify!($root), " must accept its own format version"),
            );
            assert_eq!(accepted.format_version, $version);
            let compact = accepted.to_json().expect(concat!(
                stringify!($root),
                " must serialize to compact JSON"
            ));
            let through_compact: $root =
                $root::from_json(&compact).expect("compact output must round-trip");
            assert_eq!(through_compact, accepted);
            let pretty = accepted
                .to_json_pretty()
                .expect(concat!(stringify!($root), " must serialize to pretty JSON"));
            let through_pretty: $root =
                $root::from_json(&pretty).expect("pretty output must round-trip");
            assert_eq!(through_pretty, accepted);
        }};
    }

    /// The version gate is one function, so every root must accept
    /// exactly its declared version and reject absent/wrong versions
    /// with the same error shape. The uniformity this pins is the reason
    /// [`super::VersionedArtifact`] exists; [`assert_root_wired!`] covers
    /// the per-root plumbing, and this test adds the shared rejection
    /// matrix.
    #[test]
    fn every_artifact_root_gates_format_version_identically() {
        assert_root_wired!(Model, FORMAT_VERSION);
        assert_root_wired!(ActorModel, ACTOR_MODEL_FORMAT_VERSION);
        assert_root_wired!(DddModel, DDD_MODEL_FORMAT_VERSION);
        assert_root_wired!(EventModel, EVENT_MODEL_FORMAT_VERSION);
        assert_root_wired!(DeployModel, DEPLOY_MODEL_FORMAT_VERSION);
        assert_root_wired!(UseCaseModel, USE_CASE_MODEL_FORMAT_VERSION);
        assert_root_wired!(IfmlModel, IFML_MODEL_FORMAT_VERSION);
        assert_root_wired!(IfmlManifest, IFML_MANIFEST_FORMAT_VERSION);
        assert_root_wired!(CmmnModel, CMMN_MODEL_FORMAT_VERSION);
        assert_root_wired!(DmnModel, DMN_MODEL_FORMAT_VERSION);
        assert_root_wired!(GraphModel, GRAPH_MODEL_FORMAT_VERSION);

        for json in ["{}", "{\"formatVersion\":3}"] {
            rejects_version::<Model>(json);
            rejects_version::<ActorModel>(json);
            rejects_version::<DddModel>(json);
            rejects_version::<EventModel>(json);
            rejects_version::<DeployModel>(json);
            rejects_version::<UseCaseModel>(json);
            rejects_version::<IfmlModel>(json);
            rejects_version::<IfmlManifest>(json);
            rejects_version::<CmmnModel>(json);
            rejects_version::<DmnModel>(json);
            rejects_version::<GraphModel>(json);
        }
    }

    fn rejects_version<T: VersionedArtifact + DeserializeOwned + Debug>(json: &str) {
        let error = from_json::<T>(json)
            .expect_err(&format!("`{json}` must be rejected by the version gate"));
        assert!(
            matches!(error, IrError::UnsupportedFormatVersion { .. }),
            "unexpected error for `{json}`: {error}"
        );
    }

    /// The shared wire-contract characterization for one artifact root,
    /// covering the contract points every root shares (each caller keeps
    /// its own canonical-bytes expectation, because root-specific always-on
    /// keys live there):
    ///
    /// 1. `T::default()` (== `new()`) serializes to **exactly**
    ///    `empty_canonical_json` — the Default stamp and the write-side
    ///    minimal-artifact discipline in one byte-identity.
    /// 2. Those canonical bytes deserialize back to the same empty model.
    /// 3. The version gate rejects a wrong and an absent `formatVersion`
    ///    with [`IrError::UnsupportedFormatVersion`], reporting the found
    ///    and expected versions — on payloads that also carry unknown
    ///    fields, so the gate demonstrably fires before decoding.
    /// 4. Unknown fields are tolerated: the canonical bytes plus a
    ///    `nobodyKnowsMe` object deserialize to the plain empty model.
    /// 5. A populated `sample` round-trips through both `to_json` and
    ///    `to_json_pretty`, re-serializing byte-stably.
    pub(crate) fn assert_wire_contract<T>(empty_canonical_json: &str, sample: &T)
    where
        T: VersionedArtifact + Serialize + DeserializeOwned + Default + PartialEq + Debug,
    {
        let name = std::any::type_name::<T>();

        let default = T::default();
        assert_eq!(
            to_json(&default).expect("serialize empty"),
            empty_canonical_json,
            "{name}'s empty artifact must serialize to exactly the canonical bytes"
        );

        let parsed: T = from_json(empty_canonical_json).expect("canonical bytes must deserialize");
        assert_eq!(parsed, default, "{name}'s canonical bytes must round-trip");

        let with_version = |version: Option<u64>| {
            let mut value: serde_json::Value =
                serde_json::from_str(empty_canonical_json).expect("canonical bytes are JSON");
            let object = value
                .as_object_mut()
                .expect("canonical bytes are an object");
            match version {
                Some(version) => {
                    object.insert("formatVersion".to_string(), version.into());
                }
                None => {
                    object.remove("formatVersion");
                }
            }
            object.insert(
                "nobodyKnowsMe".to_string(),
                serde_json::json!({ "nested": [1, 2, 3] }),
            );
            value.to_string()
        };
        for (json, found) in [(with_version(Some(0)), 0u32), (with_version(Some(3)), 3)] {
            let error = from_json::<T>(&json).expect_err("wrong version must be rejected");
            assert!(
                matches!(error, IrError::UnsupportedFormatVersion { found: f, expected } if f == found && expected == T::FORMAT_VERSION),
                "unexpected gate error for `{json}`: {error:?}"
            );
        }
        let error =
            from_json::<T>(&with_version(None)).expect_err("absent version must be rejected");
        assert!(
            matches!(error, IrError::UnsupportedFormatVersion { found: 0, expected } if expected == T::FORMAT_VERSION),
            "unexpected gate error for the absent version: {error:?}"
        );

        let tolerant = with_version(Some(T::FORMAT_VERSION as u64));
        let parsed: T = from_json(&tolerant).expect("unknown fields are ignored");
        assert_eq!(parsed, default, "unknown fields are dropped on write");

        let compact = to_json(sample).expect("serialize sample");
        let parsed: T = from_json(&compact).expect("sample must round-trip through to_json");
        assert_eq!(&parsed, sample);
        assert_eq!(to_json(&parsed).expect("re-serialize"), compact);

        let pretty = to_json_pretty(sample).expect("serialize sample pretty");
        let parsed: T = from_json(&pretty).expect("sample must round-trip through to_json_pretty");
        assert_eq!(&parsed, sample);
        assert_eq!(to_json_pretty(&parsed).expect("re-serialize"), pretty);
    }
}
