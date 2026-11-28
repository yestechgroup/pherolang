//! The Rune (`.rosetta`) bands (ADR 0012 D5, issue pherolang#399):
//!
//! - `rune.syntax` `E0500`–`E0519`: the Rune parser (`sigil-syntax`, which
//!   folds into `phero_syntax::rune` in RN9);
//! - `rune.resolve` `E0520`–`E0579`: name resolution and scoping
//!   (`phero-rune-resolve`);
//! - `rune.lints` `W0580`–`W0599`: resolution warnings.
//!
//! The codes replace sigil's own string literals (`E0001`, `E0101`–`E0107`,
//! `W0101`), which collided with the `ifml.*` bands. ADR 0012 records the
//! old→new map, and each constant's doc names its former number. sigil's
//! `W0001` (the unsupported-construct skip) was never emitted and is not
//! migrated. Rune semantics are pinned by the Java oracle, not here: these
//! entries describe the diagnostics, they do not specify the language.
//! Claim a new code by adding a documented constant plus an `Entry` to
//! `ENTRIES`.

use crate::{Entry, Severity};

/// The Rune parser band's registry id.
pub const RUNE_SYNTAX_BAND: &str = "rune.syntax";
/// The Rune resolution band's registry id.
pub const RUNE_RESOLVE_BAND: &str = "rune.resolve";
/// The Rune resolution-warning band's registry id.
pub const RUNE_LINTS_BAND: &str = "rune.lints";

/// A `.rosetta` file does not parse (formerly sigil `E0001`).
pub const E_RUNE_SYNTAX: &str = "E0500";
/// A type, super type, schema or serialization format, or expression symbol
/// head does not resolve (formerly sigil `E0101`).
pub const E_RUNE_UNKNOWN_SYMBOL: &str = "E0520";
/// An annotation does not resolve, or `externalConfig` is applied to a
/// non-schema element (formerly sigil `E0102`).
pub const E_RUNE_UNKNOWN_ANNOTATION: &str = "E0521";
/// An annotation use names an attribute the annotation does not declare
/// (formerly sigil `E0103`).
pub const E_RUNE_NO_SUCH_ANNOTATION_ATTRIBUTE: &str = "E0522";
/// Two definitions of the same kind share a name (formerly sigil `E0104`).
pub const E_RUNE_DUPLICATE_DEFINITION: &str = "E0523";
/// A type's `extends` chain loops back on itself (formerly sigil `E0105`).
pub const E_RUNE_INHERITANCE_CYCLE: &str = "E0524";
/// A reference resolves to an element of the wrong kind (formerly sigil
/// `E0106`).
pub const E_RUNE_WRONG_KIND: &str = "E0525";
/// An attribute or enum value does not exist on its owner (formerly sigil
/// `E0107`).
pub const E_RUNE_UNKNOWN_MEMBER: &str = "E0526";
/// A reference names a `[deprecated]` element (formerly sigil `W0101`).
pub const W_RUNE_DEPRECATED_REFERENCE: &str = "W0580";

pub(crate) const ENTRIES: &[Entry] = &[
    Entry {
        code: E_RUNE_SYNTAX,
        band: RUNE_SYNTAX_BAND,
        severity: Severity::Error,
        title: "Rune syntax error",
        meaning: "A `.rosetta` file does not parse against the Rune grammar (the FINOS Rune DSL, as accepted by the reference implementation sigil is checked against). The message names what the parser expected. Every namespace the file declares fails, so a `.mox` `import sigil` that needs it fails too.",
        fix: "Correct the syntax at the reported position. The Rune grammar is the reference in `docs/rune/` until it moves into `phero-syntax`.",
    },
    Entry {
        code: E_RUNE_UNKNOWN_SYMBOL,
        band: RUNE_RESOLVE_BAND,
        severity: Severity::Error,
        title: "Unknown Rune type or symbol",
        meaning: "A type reference, super type, schema or serialization format, or the head symbol of an expression does not resolve. Names resolve through local elements, then explicit imports (first match wins), then the file's own namespace, then the implicit `com.rosetta.model` builtins, then fully qualified names.",
        fix: "Fix the spelling, import the namespace that declares the name, or qualify it fully.",
    },
    Entry {
        code: E_RUNE_UNKNOWN_ANNOTATION,
        band: RUNE_RESOLVE_BAND,
        severity: Severity::Error,
        title: "Unknown Rune annotation",
        meaning: "An annotation use (`[name]`) does not resolve to a declared `annotation`, or `externalConfig` is applied to an element that is not a schema. Annotations resolve in their own symbol space, separate from types.",
        fix: "Declare or import the annotation, or move `externalConfig` onto a schema element.",
    },
    Entry {
        code: E_RUNE_NO_SUCH_ANNOTATION_ATTRIBUTE,
        band: RUNE_RESOLVE_BAND,
        severity: Severity::Error,
        title: "No such annotation attribute",
        meaning: "An annotation use names an attribute (`[name attr]`) that the annotation's declaration does not have.",
        fix: "Use one of the attributes the annotation declares, or add the attribute to its declaration.",
    },
    Entry {
        code: E_RUNE_DUPLICATE_DEFINITION,
        band: RUNE_RESOLVE_BAND,
        severity: Severity::Error,
        title: "Duplicate Rune definition",
        meaning: "Two definitions of the same kind share a name in one namespace. Kinds are separate symbol spaces, so a type alias and an annotation may share a name; two types may not.",
        fix: "Rename one of the definitions, or remove the duplicate.",
    },
    Entry {
        code: E_RUNE_INHERITANCE_CYCLE,
        band: RUNE_RESOLVE_BAND,
        severity: Severity::Error,
        title: "Rune inheritance cycle",
        meaning: "A type's `extends` chain reaches the type itself, so no ancestor order exists.",
        fix: "Break the cycle by removing or redirecting one `extends` clause.",
    },
    Entry {
        code: E_RUNE_WRONG_KIND,
        band: RUNE_RESOLVE_BAND,
        severity: Severity::Error,
        title: "Rune reference of the wrong kind",
        meaning: "A reference resolves, but to an element of the wrong kind for its position: for example a function where a type is required, or a transform reference that is not a schema or serialization format.",
        fix: "Point the reference at an element of the kind the position requires.",
    },
    Entry {
        code: E_RUNE_UNKNOWN_MEMBER,
        band: RUNE_RESOLVE_BAND,
        severity: Severity::Error,
        title: "Unknown Rune attribute or enum value",
        meaning: "An attribute (in an operation path, a dispatch, or a rule source) or an enum value (including a `SerializationFormat` literal) does not exist on the type or enumeration it is looked up in.",
        fix: "Use an attribute or value the owner declares, or add it to the owner.",
    },
    Entry {
        code: W_RUNE_DEPRECATED_REFERENCE,
        band: RUNE_LINTS_BAND,
        severity: Severity::Warning,
        title: "Reference to a deprecated Rune element",
        meaning: "A reference names an element annotated `[deprecated]`. This is a warning: resolution still succeeds and the model still lowers.",
        fix: "Move the reference to the element's replacement, or accept the warning while migrating.",
    },
];
