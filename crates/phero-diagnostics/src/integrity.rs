//! The `driver.integrity` band: `X0001`–`X0099`, the phero-driver's
//! cross-surface integrity pass (`check_integrity`, issue pherolang#114 —
//! `crates/phero-driver/src/integrity.rs`). Every check prefixes its code
//! onto the diagnostic message (`X0001 unknown capability …`), the way
//! the CLI's `.ifml` converter prefixes codes; the registry entry is the
//! documentation `pherolang explain` and `docs/DIAGNOSTICS.md` render. Claim
//! a new code by adding a documented constant plus an `Entry` to
//! `ENTRIES`.

use crate::{Entry, Severity};

/// The band's registry id ([`crate::all`] entries reference it).
pub const INTEGRITY_BAND: &str = "driver.integrity";

/// `unknown capability '<capability>' on operation '<op>' of service
/// '<service>'` (or `on search '<search>'`) — the warning lint channel of
/// the compile-time capability join.
pub const X_UNKNOWN_OP_CAPABILITY: &str = "X0001";
/// `capability '<capability>' is granted but no operation or search
/// carries it` — the warning lint channel of the compile-time grant
/// coverage join.
pub const X_GRANTED_CAPABILITY_UNCARRIED: &str = "X0002";
/// `operation '<op>' of service '<service>' emits unknown event '<event>'`.
pub const X_UNKNOWN_EMISSION: &str = "X0003";
/// `event '<event>' is declared but no operation emits it`.
pub const X_ORPHAN_EVENT: &str = "X0004";
/// `action '<action>' does not bind to any service operation`.
pub const X_UNBOUND_ACTION: &str = "X0005";
/// `action '<action>' binds ambiguously to '<service.op>' and
/// '<service.op>'` — the case-insensitive mirror names several operations.
pub const X_AMBIGUOUS_ACTION: &str = "X0006";
/// `subscription '<subscription>' consumer '<consumer>' does not resolve
/// to a declared component`.
pub const X_UNKNOWN_CONSUMER: &str = "X0007";
/// `unknown capability or actor '<name>' in requires of …` / `unknown
/// actor '<name>' in roles of view '<name>'`.
pub const X_UNKNOWN_FLOW_REFERENCE: &str = "X0008";
/// `intent '<id>' maps to no model element` — the intent carries no
/// `element` link that resolves (or none at all). A warning: an intent no
/// model element expresses is a claim nobody can honor, and honesty beats
/// reach (#196).
pub const X_INTENT_WITHOUT_ELEMENT: &str = "X0009";
/// `intent '<id>' maps to no check` — the intent carries no `check` link
/// that resolves (or none at all). A warning, for the same honesty rule:
/// the claim exists but nothing in the model verifies it.
pub const X_INTENT_WITHOUT_CHECK: &str = "X0010";
/// `unknown element <kind> '<name>' in intent '<id>'` — the intent's
/// `element` link names no element of that kind in the application's
/// artifacts (spelled with the did-you-mean candidate when one is close).
pub const X_UNKNOWN_INTENT_ELEMENT: &str = "X0011";
/// `unknown check <kind> '<name>' in intent '<id>'` — the intent's `check`
/// link names no check of that kind in the application's artifacts
/// (spelled with the did-you-mean candidate when one is close).
pub const X_UNKNOWN_INTENT_CHECK: &str = "X0012";
/// `'<name>' matches several <kind>s in application '<app>': <a>, <b>` — a
/// bare link name is ambiguous across the application's artifacts; the
/// coverage pass refuses to guess.
pub const X_AMBIGUOUS_INTENT_LINK: &str = "X0013";
/// `duplicate intent id '<id>' across '<file-a>' and '<file-b>'` — two
/// intent artifacts of one application declare the same stable id; the id
/// is the traceability key, so the collision is an error (the in-file
/// duplicate is `E0451`).
pub const X_DUPLICATE_INTENT_ID: &str = "X0014";

pub(crate) const ENTRIES: &[Entry] = &[
    Entry {
        code: X_UNKNOWN_OP_CAPABILITY,
        band: INTEGRITY_BAND,
        severity: Severity::Warning,
        title: "Unknown operation capability",
        meaning: "`unknown capability '<capability>' on operation '<op>' of service '<service>'` (or `on search '<search>'`) — the capability clause names no capability declared in any provided actor block. Since pherolang#128 the compile-time join is the language error (code `unknownCapability`); this union-pass finding is the lint channel for hosts that gather artifacts without compiling.",
        fix: "Declare the capability (`capability X on <Class>;` in an actors block), or rename the reference to a declared capability.",
    },
    Entry {
        code: X_GRANTED_CAPABILITY_UNCARRIED,
        band: INTEGRITY_BAND,
        severity: Severity::Warning,
        title: "Granted capability uncarried",
        meaning: "`capability '<capability>' is granted but no operation or search carries it` — the granted (or delegated) capability guards a transition that cannot happen. Since pherolang#128 the compile-time join is the language error (code `grantedCapabilityUncarried`); this union-pass finding is the lint channel.",
        fix: "Carry the capability — add it to an operation or search across the designs — or drop the grant entry.",
    },
    Entry {
        code: X_UNKNOWN_EMISSION,
        band: INTEGRITY_BAND,
        severity: Severity::Error,
        title: "Unknown emitted event",
        meaning: "`operation '<op>' of service '<service>' emits unknown event '<event>'` — the `emits` clause is loosely coupled by design (the design compile has no event bucket), so the name resolves only across the artifact union.",
        fix: "Declare the event in an `.evt` contract, or fix the spelling.",
    },
    Entry {
        code: X_ORPHAN_EVENT,
        band: INTEGRITY_BAND,
        severity: Severity::Error,
        title: "Orphan declared event",
        meaning: "`event '<event>' is declared but no operation emits it` — nothing produces the declared event, so it is dead weight. Channel `publishes` entries and subscription `events [...]` lists are consumption-side wiring and never satisfy production.",
        fix: "Emit the event from a service operation's `emits` clause, or drop the declaration.",
    },
    Entry {
        code: X_UNBOUND_ACTION,
        band: INTEGRITY_BAND,
        severity: Severity::Error,
        title: "Unbound action",
        meaning: "`action '<action>' does not bind to any service operation` — binding is ASCII-case-insensitive name equality (the PascalCase action ↔ camelCase operation mirror), and no operation carries the name. An unbound action is a button whose backend does not exist.",
        fix: "Name the action after the operation it drives (or the reverse), or add the operation to a design.",
    },
    Entry {
        code: X_AMBIGUOUS_ACTION,
        band: INTEGRITY_BAND,
        severity: Severity::Error,
        title: "Ambiguous action binding",
        meaning: "`action '<action>' binds ambiguously to '<service.op>' and '<service.op>'` — several operations across the designs share the action's case-insensitive mirror name, so the binding names no unique backend (listing every carrier).",
        fix: "Rename the colliding operations so each action's case-insensitive mirror names exactly one service operation.",
    },
    Entry {
        code: X_UNKNOWN_CONSUMER,
        band: INTEGRITY_BAND,
        severity: Severity::Error,
        title: "Unknown subscription consumer",
        meaning: "`subscription '<subscription>' consumer '<consumer>' does not resolve to a declared component` — a dead inbox: the events are published, but no deploy component receives them.",
        fix: "Declare the component in a deploy model (any application), or fix the consumer's spelling.",
    },
    Entry {
        code: X_UNKNOWN_FLOW_REFERENCE,
        band: INTEGRITY_BAND,
        severity: Severity::Error,
        title: "Unknown flow actor reference",
        meaning: "`unknown capability or actor '<name>' in requires of view/command '<name>'`, or `unknown actor '<name>' in roles of view '<name>'` — the flow clause names neither a capability nor an actor the actor layer declares (`roles:` is strictly actors).",
        fix: "Declare the capability or actor in an actors block, or fix the spelling.",
    },
    Entry {
        code: X_INTENT_WITHOUT_ELEMENT,
        band: INTEGRITY_BAND,
        severity: Severity::Warning,
        title: "Intent maps to no element",
        meaning: "`intent '<id>' maps to no model element` — the intent declares no `element` link, or none of them resolves in the application's artifacts. An intent no model element expresses is a claim nobody can honor: it warns rather than errors because the honest state is visible, not hidden (#196 — honesty over reach).",
        fix: "Add an `element` link to the capability, class, event, view, command, or operation that expresses the intent — or withdraw the intent.",
    },
    Entry {
        code: X_INTENT_WITHOUT_CHECK,
        band: INTEGRITY_BAND,
        severity: Severity::Warning,
        title: "Intent maps to no check",
        meaning: "`intent '<id>' maps to no check` — the intent declares no `check` link, or none of them resolves in the application's artifacts. The claim exists but nothing model-native verifies it; it warns rather than errors for the same honesty rule (#196).",
        fix: "Add a `check` link to a designed class's invariants (`check invariant:Class`) or a decision (`check decision:Name`) that verifies the intent.",
    },
    Entry {
        code: X_UNKNOWN_INTENT_ELEMENT,
        band: INTEGRITY_BAND,
        severity: Severity::Error,
        title: "Unknown intent element",
        meaning: "`unknown element <kind> '<name>' in intent '<id>'` — the intent's `element` link names no element of that kind in the application's compiled artifacts: `class` resolves against the imported domain union, `capability` against the actor layer, `event` against the event contracts, `view`/`command` against the flows, and `op` against the designs' service and repository operations. A close misspelling is named in the help.",
        fix: "Fix the spelling (the help names the nearest candidate), declare the element, or withdraw the link.",
    },
    Entry {
        code: X_UNKNOWN_INTENT_CHECK,
        band: INTEGRITY_BAND,
        severity: Severity::Error,
        title: "Unknown intent check",
        meaning: "`unknown check <kind> '<name>' in intent '<id>'` — the intent's `check` link names no check of that kind: `invariant` resolves to a designed class carrying at least one invariant, `decision` to a `.dmn` decision of the application. A close misspelling is named in the help.",
        fix: "Fix the spelling (the help names the nearest candidate), declare the check, or withdraw the link.",
    },
    Entry {
        code: X_AMBIGUOUS_INTENT_LINK,
        band: INTEGRITY_BAND,
        severity: Severity::Error,
        title: "Ambiguous intent link",
        meaning: "`'<name>' matches several '<kind>' targets in application '<app>': <a>, <b>` — a bare link name binds to more than one element or check of the requested kind across the application's artifacts. The coverage pass refuses to guess a target, so the traceability edge would be a lie.",
        fix: "Qualify the name (a dotted `package.Class` path for a class, or rename one of the colliding elements).",
    },
    Entry {
        code: X_DUPLICATE_INTENT_ID,
        band: INTEGRITY_BAND,
        severity: Severity::Error,
        title: "Duplicate intent id across files",
        meaning: "`duplicate intent id '<id>' across '<file-a>' and '<file-b>'` — two intent artifacts of one application declare the same stable id. The id is the traceability key (#196); within one file the surface check already refuses it (`E0451`), this is the cross-file collision.",
        fix: "Renumber one file's intent so ids stay unique across the application.",
    },
];
