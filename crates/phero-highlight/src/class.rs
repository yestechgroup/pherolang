//! The highlight taxonomy: classes, modifiers, and the byte-span core shape.
//!
//! A [`HighlightClass`] is the *semantic* category of a source range — what the
//! compiler knows the range is, not what a regular expression guessed. The
//! lowercase wire spelling ([`HighlightClass::as_str`]) is the stable contract
//! shared by the JSON artifact, the HTML renderer, the TextMate scopes, and the
//! LSP token types.
//!
//! [`HighlightModifiers`] is a compact bitset: modifiers are orthogonal to the
//! class (`readonly` is a property flag, `declaration` marks a definition
//! site), so a caller can test membership without allocating. Its wire shape is
//! an ordered array of the [`HighlightModifier::ALL`] spellings, which keeps
//! artifacts deterministic.

use std::fmt;
use std::ops::{BitOr, BitOrAssign};

use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The semantic class of a highlighted source range.
///
/// The vocabulary mirrors the standard editor token taxonomy: a language
/// server maps each class onto one LSP semantic-token type
/// ([`crate::lsp_token_type`]) and each class onto a TextMate scope
/// ([`crate::scope_for`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum HighlightClass {
    /// A language keyword (`class`, `when`, `module`, ...).
    Keyword,
    /// A type name, including a builtin primitive (`String`, `int`, ...).
    Type,
    /// A function, operation, service operation, or capability name.
    Function,
    /// A namespace or package-name segment.
    Namespace,
    /// An ordinary value binding or reference.
    Variable,
    /// A declared parameter.
    Parameter,
    /// A feature, field, property, or setting path member.
    Property,
    /// A string literal.
    String,
    /// A numeric literal.
    Number,
    /// A line or block comment.
    Comment,
    /// An operator symbol.
    Operator,
    /// A structural punctuation character.
    Punctuation,
    /// An annotation/decorator word or a design-flag occurrence.
    Annotation,
    /// A language literal (`true`, `false`, enum members, vocabulary words).
    Constant,
    /// An escape sequence inside a string literal.
    Escape,
    /// A lexical error region.
    Error,
}

impl HighlightClass {
    /// Every class, in the canonical wire order.
    pub const ALL: [Self; 16] = [
        Self::Keyword,
        Self::Type,
        Self::Function,
        Self::Namespace,
        Self::Variable,
        Self::Parameter,
        Self::Property,
        Self::String,
        Self::Number,
        Self::Comment,
        Self::Operator,
        Self::Punctuation,
        Self::Annotation,
        Self::Constant,
        Self::Escape,
        Self::Error,
    ];

    /// The stable lowercase wire spelling of this class.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Keyword => "keyword",
            Self::Type => "type",
            Self::Function => "function",
            Self::Namespace => "namespace",
            Self::Variable => "variable",
            Self::Parameter => "parameter",
            Self::Property => "property",
            Self::String => "string",
            Self::Number => "number",
            Self::Comment => "comment",
            Self::Operator => "operator",
            Self::Punctuation => "punctuation",
            Self::Annotation => "annotation",
            Self::Constant => "constant",
            Self::Escape => "escape",
            Self::Error => "error",
        }
    }
}

impl fmt::Display for HighlightClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An orthogonal modifier attached to a highlighted range.
///
/// The wire spelling is camelCase for the multi-word member
/// (`defaultLibrary`), matching the LSP semantic-token modifier names
/// verbatim — [`crate::lsp_modifier`] is the identity mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum HighlightModifier {
    /// The range declares the name (a definition site).
    Declaration,
    /// The range is a definition rather than a reference.
    Definition,
    /// The binding cannot be reassigned (`readonly`).
    Readonly,
    /// The range refers to a deprecated construct.
    Deprecated,
    /// The comment carries documentation text (`///`, `/**`).
    Documentation,
    /// The name resolves to a standard-library/builtin construct.
    DefaultLibrary,
}

impl HighlightModifier {
    /// Every modifier, in the canonical wire order.
    pub const ALL: [Self; 6] = [
        Self::Declaration,
        Self::Definition,
        Self::Readonly,
        Self::Deprecated,
        Self::Documentation,
        Self::DefaultLibrary,
    ];

    /// The stable wire spelling of this modifier (also its LSP spelling).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Declaration => "declaration",
            Self::Definition => "definition",
            Self::Readonly => "readonly",
            Self::Deprecated => "deprecated",
            Self::Documentation => "documentation",
            Self::DefaultLibrary => "defaultLibrary",
        }
    }

    /// The bit this modifier occupies in [`HighlightModifiers`].
    const fn bit(self) -> u8 {
        match self {
            Self::Declaration => 1 << 0,
            Self::Definition => 1 << 1,
            Self::Readonly => 1 << 2,
            Self::Deprecated => 1 << 3,
            Self::Documentation => 1 << 4,
            Self::DefaultLibrary => 1 << 5,
        }
    }
}

impl fmt::Display for HighlightModifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A compact set of [`HighlightModifier`]s.
///
/// Iteration order is always [`HighlightModifier::ALL`] order, which makes both
/// the wire array and the LSP modifier array deterministic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct HighlightModifiers(u8);

impl HighlightModifiers {
    /// An empty set.
    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    /// An empty set (alias of [`HighlightModifiers::new`]).
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Builds a set from raw bits, discarding bits outside the known
    /// vocabulary.
    #[must_use]
    pub const fn from_bits_truncate(bits: u8) -> Self {
        Self(bits & Self::ALL_BITS)
    }

    const ALL_BITS: u8 = (1 << HighlightModifier::ALL.len()) - 1;

    /// The raw bits.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Inserts one modifier.
    pub fn insert(&mut self, modifier: HighlightModifier) {
        self.0 |= modifier.bit();
    }

    /// Removes one modifier.
    pub fn remove(&mut self, modifier: HighlightModifier) {
        self.0 &= !modifier.bit();
    }

    /// `true` when the set holds `modifier`.
    #[must_use]
    pub const fn contains(self, modifier: HighlightModifier) -> bool {
        self.0 & modifier.bit() != 0
    }

    /// `true` when the set is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Iterates the modifiers in [`HighlightModifier::ALL`] order.
    pub fn iter(self) -> impl Iterator<Item = HighlightModifier> {
        HighlightModifier::ALL
            .into_iter()
            .filter(move |modifier| self.contains(*modifier))
    }

    /// Returns a set with `modifier` inserted (builder form).
    #[must_use]
    pub fn with(mut self, modifier: HighlightModifier) -> Self {
        self.insert(modifier);
        self
    }
}

impl BitOr for HighlightModifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for HighlightModifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl FromIterator<HighlightModifier> for HighlightModifiers {
    fn from_iter<T: IntoIterator<Item = HighlightModifier>>(iter: T) -> Self {
        let mut set = Self::new();
        for modifier in iter {
            set.insert(modifier);
        }
        set
    }
}

impl Serialize for HighlightModifiers {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.iter().count()))?;
        for modifier in self.iter() {
            seq.serialize_element(&modifier)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for HighlightModifiers {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ModifiersVisitor;

        impl<'de> Visitor<'de> for ModifiersVisitor {
            type Value = HighlightModifiers;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an array of highlight modifier strings")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut set = HighlightModifiers::new();
                while let Some(modifier) = seq.next_element::<HighlightModifier>()? {
                    set.insert(modifier);
                }
                Ok(set)
            }
        }

        deserializer.deserialize_seq(ModifiersVisitor)
    }
}

/// One highlighted byte range: half-open, in source byte offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HighlightSpan {
    /// Start offset (inclusive).
    pub start: usize,
    /// End offset (exclusive).
    pub end: usize,
    /// The semantic class.
    pub class: HighlightClass,
    /// Orthogonal modifiers.
    pub modifiers: HighlightModifiers,
}

impl HighlightSpan {
    /// A span with no modifiers.
    #[must_use]
    pub const fn new(start: usize, end: usize, class: HighlightClass) -> Self {
        Self {
            start,
            end,
            class,
            modifiers: HighlightModifiers::new(),
        }
    }

    /// A span with an explicit modifier set.
    #[must_use]
    pub const fn with_modifiers(
        start: usize,
        end: usize,
        class: HighlightClass,
        modifiers: HighlightModifiers,
    ) -> Self {
        Self {
            start,
            end,
            class,
            modifiers,
        }
    }

    /// The number of bytes covered.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// `true` when the span covers no bytes.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// `true` when `offset` falls inside the half-open range.
    #[must_use]
    pub const fn contains(&self, offset: usize) -> bool {
        self.start <= offset && offset < self.end
    }

    /// `true` when this span and `other` overlap.
    #[must_use]
    pub const fn intersects(&self, other: &Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

// A `ModifiersVisitor` that only accepts a sequence would reject a lone
// string; the wire contract is always an array, so that is deliberate.
impl HighlightModifiers {
    /// Deserializes a set from its wire array — the same contract as serde.
    pub fn from_vec(modifiers: Vec<HighlightModifier>) -> Self {
        modifiers.into_iter().collect()
    }

    /// The wire array spelling, in canonical order.
    #[must_use]
    pub fn to_vec(self) -> Vec<HighlightModifier> {
        self.iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_wire_spellings_are_lowercase_and_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for class in HighlightClass::ALL {
            let spelling = class.as_str();
            assert_eq!(spelling, spelling.to_lowercase());
            assert!(
                seen.insert(spelling),
                "duplicate class spelling `{spelling}`"
            );
        }
        assert_eq!(seen.len(), 16);
    }

    #[test]
    fn class_round_trips_through_serde_with_as_str() {
        for class in HighlightClass::ALL {
            let json = serde_json::to_string(&class).expect("serialize class");
            assert_eq!(json, format!("\"{}\"", class.as_str()));
            let parsed: HighlightClass = serde_json::from_str(&json).expect("deserialize class");
            assert_eq!(parsed, class);
        }
    }

    #[test]
    fn modifier_wire_spellings_match_the_lsp_names() {
        let expected = [
            "declaration",
            "definition",
            "readonly",
            "deprecated",
            "documentation",
            "defaultLibrary",
        ];
        for (modifier, spelling) in HighlightModifier::ALL.into_iter().zip(expected) {
            assert_eq!(modifier.as_str(), spelling);
            let json = serde_json::to_string(&modifier).expect("serialize modifier");
            assert_eq!(json, format!("\"{spelling}\""));
        }
    }

    #[test]
    fn modifier_set_serializes_in_all_order_regardless_of_insertion() {
        let mut set = HighlightModifiers::new();
        set.insert(HighlightModifier::DefaultLibrary);
        set.insert(HighlightModifier::Declaration);
        set.insert(HighlightModifier::Readonly);
        let json = serde_json::to_string(&set).expect("serialize set");
        assert_eq!(json, r#"["declaration","readonly","defaultLibrary"]"#);
        let parsed: HighlightModifiers = serde_json::from_str(&json).expect("deserialize set");
        assert_eq!(parsed, set);
    }

    #[test]
    fn modifier_set_bit_operations_and_iteration() {
        let a = HighlightModifiers::new().with(HighlightModifier::Declaration);
        let b = HighlightModifiers::new().with(HighlightModifier::Readonly);
        let both = a | b;
        assert!(both.contains(HighlightModifier::Declaration));
        assert!(both.contains(HighlightModifier::Readonly));
        assert!(!both.contains(HighlightModifier::Deprecated));
        assert_eq!(
            both.iter().collect::<Vec<_>>(),
            vec![HighlightModifier::Declaration, HighlightModifier::Readonly]
        );
        let mut removed = both;
        removed.remove(HighlightModifier::Declaration);
        assert_eq!(removed.to_vec(), vec![HighlightModifier::Readonly]);
        assert_eq!(
            HighlightModifiers::from_vec(vec![
                HighlightModifier::DefaultLibrary,
                HighlightModifier::Declaration
            ]),
            a | HighlightModifiers::from_vec(vec![HighlightModifier::DefaultLibrary])
        );
    }

    #[test]
    fn span_basics() {
        let span = HighlightSpan::new(2, 6, HighlightClass::Type);
        assert_eq!(span.len(), 4);
        assert!(!span.is_empty());
        assert!(span.contains(2));
        assert!(span.contains(5));
        assert!(!span.contains(6));
        assert!(span.intersects(&HighlightSpan::new(5, 9, HighlightClass::Type)));
        assert!(!span.intersects(&HighlightSpan::new(6, 9, HighlightClass::Type)));
        assert!(HighlightSpan::new(2, 2, HighlightClass::Type).is_empty());
    }
}
