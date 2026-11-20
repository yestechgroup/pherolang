//! The intent artifact: requirement statements with stable ids, each linked
//! to the model elements that express it (`element` links) and the
//! model-native checks that verify it (`check` links), produced by the
//! `.intent` intent-specification surface (issue pherolang#196) and consumed
//! by the project-level coverage pass exactly like
//! [`crate::ddd::DddModel`], [`crate::events::EventModel`], and
//! [`crate::usecases::UseCaseModel`].
//!
//! This is a standalone, versioned wire artifact in the [pherolang] family,
//! versioned independently of [`crate::FORMAT_VERSION`] via
//! [`INTENT_MODEL_FORMAT_VERSION`] — the [`crate::usecases::UseCaseModel`]
//! precedent (wire contract rule 10). The [wire format contract](crate#wire-format-contract)
//! applies unchanged, under the UseCaseModel empty-model discipline:
//!
//! - **Casing.** Struct fields serialize as `camelCase` (`formatVersion`);
//!   enum variant names serialize as lowercase tags (`"class"`,
//!   `"decision"`).
//! - **Verbatim names.** A link's [`IntentLink::name`] is the dotted path
//!   exactly as authored; the artifact never resolves it — the coverage
//!   pass resolves, the #137 receipt table (a follow-up) will record the
//!   resolutions.
//! - **Version gate.** [`IntentModel::from_json`] rejects any other
//!   `formatVersion` (absent reports as 0) with
//!   [`crate::IrError::UnsupportedFormatVersion`] rather than guessing.
//! - **Empty = minimal.** Every collection field is `#[serde(default)]` and
//!   omitted when empty (`skip_serializing_if`) — an intent-free artifact
//!   serializes as exactly `{"formatVersion":1}`, byte-identical no matter
//!   what is added later.
//! - **Forward compatibility.** Unknown JSON fields are ignored on
//!   deserialize; every field added after v1 must follow the same
//!   default + skip discipline.
//! - **Equality policy.** Like every artifact root, [`IntentModel`] derives
//!   [`Eq`] (wire contract rule 14); this artifact carries no
//!   floating-point values.
//!
//! # Semantics consumers can rely on
//!
//! - [`IntentDef::id`] is **stable across publications**: it is the
//!   traceability key (#196), unlike the Core IR's publication-ordinal
//!   feature ids.
//! - The slot split ([`IntentDef::elements`] / [`IntentDef::checks`]) is
//!   what the coverage pass requires: at least one *resolved* link per slot
//!   maps the intent; a missing slot is the unmapped-intent warning, never
//!   an error.
//! - [`IntentLink::kind`] values are a closed, driver-validated vocabulary
//!   (the element kinds `class`, `capability`, `event`, `view`, `command`,
//!   `op`; the check kinds `invariant`, `decision`), but the wire keeps
//!   them as authored strings-with-tags so a consumer can render the
//!   authoring surface faithfully.
//!
//! [pherolang]: https://github.com/yestechgroup/pherolang

use serde::{Deserialize, Serialize};

/// The artifact format version this crate writes and accepts for intent
/// artifacts ([`IntentModel`]).
///
/// Bump whenever the intent wire format changes incompatibly; this marker
/// is versioned independently of every other artifact marker — see the
/// [version-marker inventory](crate#version-markers).
pub const INTENT_MODEL_FORMAT_VERSION: u32 = 1;

/// The root of an intent artifact: the declared intents, in declaration
/// order.
///
/// See the [wire format contract](crate#wire-format-contract) and the
/// [module docs](self).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentModel {
    /// Artifact format version. Always [`INTENT_MODEL_FORMAT_VERSION`] for
    /// artifacts this crate writes; [`IntentModel::from_json`] rejects
    /// anything else.
    pub format_version: u32,
    /// Declared intents, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intents: Vec<IntentDef>,
}

impl IntentModel {
    /// Creates an empty intent artifact with
    /// [`INTENT_MODEL_FORMAT_VERSION`] stamped.
    pub fn new() -> Self {
        Self {
            format_version: INTENT_MODEL_FORMAT_VERSION,
            intents: Vec::new(),
        }
    }
}

crate::wire::versioned_artifact!(
    IntentModel,
    INTENT_MODEL_FORMAT_VERSION,
    "the intent artifact"
);

/// A declared intent (issue pherolang#196): one requirement statement with
/// a stable id, the model elements that express it, and the checks that
/// verify it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentDef {
    /// The stable requirement id (`LOAN-001`). Unique across the
    /// application's intent artifacts (a coverage-pass concern, never a
    /// wire concern); the shape is driver-validated at compile time.
    pub id: String,
    /// The requirement statement: the string literal's text as written.
    pub statement: String,
    /// The `element` links, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<IntentLink>,
    /// The `check` links, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<IntentLink>,
}

/// One link of an [`IntentDef`]: a kind-tagged name pointing at a model
/// element or a check. The name stays exactly as authored — resolution is
/// the coverage pass's job, never the wire's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentLink {
    /// The link's kind tag (`class`, `capability`, `event`, `view`,
    /// `command`, `op`, `invariant`, `decision`).
    pub kind: IntentLinkKind,
    /// The dotted link name as written (`RefundRequest`,
    /// `nz.example.support.RefundRequest`).
    pub name: String,
}

/// The closed vocabulary of [`IntentLink::kind`] tags. The element kinds
/// express an intent; the check kinds verify one. A kind used in the wrong
/// slot is a compile-time error (the driver's E0453), so a wire artifact
/// never carries one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IntentLinkKind {
    /// A domain class of the imported `.mox` union.
    Class,
    /// A capability of the application's `.actor` policies.
    Capability,
    /// An event of the application's `.evt` contracts.
    Event,
    /// A view of the application's `.ifml` flows.
    View,
    /// A CLI command of the application's `.ifml` flows.
    Command,
    /// An operation of the application's `.ddd` designs (a service or
    /// declared repository operation; the bare name must be unique across
    /// the application).
    Op,
    /// The invariants of a designed class (the class carries at least one
    /// `.ddd` invariant) — a check.
    Invariant,
    /// A decision of the application's `.dmn` decision models — a check.
    Decision,
}

impl IntentLinkKind {
    /// The element-slot kinds, in diagnostic-listing order.
    pub const ELEMENT_KINDS: [IntentLinkKind; 6] = [
        IntentLinkKind::Class,
        IntentLinkKind::Capability,
        IntentLinkKind::Event,
        IntentLinkKind::View,
        IntentLinkKind::Command,
        IntentLinkKind::Op,
    ];

    /// The check-slot kinds, in diagnostic-listing order.
    pub const CHECK_KINDS: [IntentLinkKind; 2] =
        [IntentLinkKind::Invariant, IntentLinkKind::Decision];

    /// The tag as written on the authoring surface (`element class:Name`).
    pub const fn as_str(self) -> &'static str {
        match self {
            IntentLinkKind::Class => "class",
            IntentLinkKind::Capability => "capability",
            IntentLinkKind::Event => "event",
            IntentLinkKind::View => "view",
            IntentLinkKind::Command => "command",
            IntentLinkKind::Op => "op",
            IntentLinkKind::Invariant => "invariant",
            IntentLinkKind::Decision => "decision",
        }
    }

    /// The inverse of [`IntentLinkKind::as_str`]: the kind a written tag
    /// denotes, or `None` for a word outside the closed vocabulary (the
    /// driver reports that as E0453 at compile time, so a consumer of a
    /// compiled artifact never sees one).
    pub fn from_word(word: &str) -> Option<Self> {
        [
            IntentLinkKind::Class,
            IntentLinkKind::Capability,
            IntentLinkKind::Event,
            IntentLinkKind::View,
            IntentLinkKind::Command,
            IntentLinkKind::Op,
            IntentLinkKind::Invariant,
            IntentLinkKind::Decision,
        ]
        .into_iter()
        .find(|kind| kind.as_str() == word)
    }
}
