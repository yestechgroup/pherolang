//! The `driver.intent` band: `E0450`–`E0499`, the phero-driver `.intent`
//! intent-specification checks (issue pherolang#196 —
//! `crates/phero-driver/src/intents.rs`). Every check prefixes its code
//! onto the diagnostic message (`E0450 malformed intent id …`), the
//! `driver.ddd` band's precedent. E0450–E0452 are the stable-id and
//! statement checks; E0453–E0454 are the link checks. The cross-surface
//! coverage pass over compiled artifacts lives in the `driver.integrity`
//! band (`X0009`+), not here — this band is single-surface. Claim a new
//! code by adding a documented constant plus an `Entry` to `ENTRIES`.

use crate::{Entry, Severity};

/// The band's registry id ([`crate::all`] entries reference it).
pub const INTENT_BAND: &str = "driver.intent";

/// `malformed intent id '<id>'` — the stable requirement id is not the
/// `LOAN-001` shape: one or more hyphen-separated segments, the first
/// starting with an uppercase ASCII letter and carrying only uppercase
/// letters and digits, and at least one later segment all digits. The id is
/// the traceability key (#196), so a loose spelling would fork identities.
pub const E_MALFORMED_INTENT_ID: &str = "E0450";
/// `duplicate intent id '<id>'` — two intents of one file declare the same
/// stable id. Within one file the id must already be unique; across files
/// the coverage pass reports it (`X0014`).
pub const E_DUPLICATE_INTENT_ID: &str = "E0451";
/// `intent '<id>' carries an empty statement` — the statement string is
/// empty or whitespace. An intent without a statement is a claim nobody
/// can read, so it is refused rather than silently kept.
pub const E_EMPTY_INTENT_STATEMENT: &str = "E0452";
/// `unknown <slot> kind '<kind>' in intent '<id>'` — the link's kind word
/// is not in the slot's closed vocabulary (`class`, `capability`, `event`,
/// `view`, `command`, `op` for an `element` link; `invariant`, `decision`
/// for a `check` link). The vocabulary is closed so a misspelled kind can
/// never silently degrade into an unresolvable link.
pub const E_UNKNOWN_LINK_KIND: &str = "E0453";
/// `duplicate <slot> link '<kind>:<name>' in intent '<id>'` — the same
/// kind and name are linked twice from one intent (per slot). The
/// coverage matrix counts links, so a duplicate would double-count.
pub const E_DUPLICATE_LINK: &str = "E0454";

pub(crate) const ENTRIES: &[Entry] = &[
    Entry {
        code: E_MALFORMED_INTENT_ID,
        band: INTENT_BAND,
        severity: Severity::Error,
        title: "Malformed intent id",
        meaning: "`malformed intent id '<id>'` — a stable id is one or more hyphen-separated segments, the first starting with an uppercase ASCII letter and carrying only uppercase letters and digits, and at least one later segment all digits (`LOAN-001`, `REFUND-DESK-007`). The id is the traceability key: requirements, artifacts, and (later, #137) receipts key on it, so a loose spelling would fork identities.",
        fix: "Write the id in the stable shape — uppercase lead segment, at least one all-digits segment (`LOAN-001`).",
    },
    Entry {
        code: E_DUPLICATE_INTENT_ID,
        band: INTENT_BAND,
        severity: Severity::Error,
        title: "Duplicate intent id",
        meaning: "`duplicate intent id '<id>'` — two intents of this file declare the same stable id. The id is the traceability key; within one file it must already be unique. (Across files of one application the coverage pass reports the collision as `X0014`.)",
        fix: "Renumber one of the intents so every id is unique (`LOAN-001`, `LOAN-002`, ...).",
    },
    Entry {
        code: E_EMPTY_INTENT_STATEMENT,
        band: INTENT_BAND,
        severity: Severity::Error,
        title: "Empty intent statement",
        meaning: "`intent '<id>' carries an empty statement` — the statement string is empty or whitespace. The statement is the requirement itself; an intent without one is a claim nobody can read, so it is refused rather than silently kept as an id with no meaning.",
        fix: "Write the requirement as the intent's quoted statement.",
    },
    Entry {
        code: E_UNKNOWN_LINK_KIND,
        band: INTENT_BAND,
        severity: Severity::Error,
        title: "Unknown link kind",
        meaning: "`unknown <slot> kind '<kind>' in intent '<id>'` — the link's kind word is not in the slot's closed vocabulary. An `element` link names `class`, `capability`, `event`, `view`, `command`, or `op`; a `check` link names `invariant` (a designed class carrying invariants) or `decision` (a `.dmn` decision). The vocabulary is closed so a misspelled kind can never silently degrade into an unresolvable link.",
        fix: "Write one of the slot's kinds (`element class:RefundRequest`, `check decision:RefundAuthority`).",
    },
    Entry {
        code: E_DUPLICATE_LINK,
        band: INTENT_BAND,
        severity: Severity::Error,
        title: "Duplicate link",
        meaning: "`duplicate <slot> link '<kind>:<name>' in intent '<id>'` — the same kind and name are linked twice from one intent. The coverage matrix counts links, so a duplicate would double-count the same element or check.",
        fix: "Remove the repeated link line — one link per element or check per intent.",
    },
];
