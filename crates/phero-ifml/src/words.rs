//! The `.ifml` word tables.
//!
//! The surface has **no keyword tokens**: every word is an identifier and
//! is special only in the grammar position the parser dispatches on. These
//! tables are the curated grammar vocabulary, shared by the parser tests
//! and by tooling (the syntax highlighter, editor grammars). The closed
//! data vocabularies (`button`, `list`, `text`, ...) remain owned by
//! `phero-ir`; they are values in a property position, not grammar words.

/// Declaration/body/action words (grammar positions, not data vocabularies).
pub const STRUCTURAL_WORDS: &[&str] = &[
    "import",
    "domain",
    "schema",
    "view",
    "action",
    "module",
    "actor",
    "command",
    "params",
    "label",
    "container",
    "component",
    "column",
    "field",
    "chart",
    "lookup",
    "via",
    "expr",
    "input",
    "output",
    "argument",
    "option",
    "prompt",
    "on",
    "use",
    "as",
    "if",
    "requires",
    "navigate",
    "refresh",
    "stay",
];

/// The primitive type names the language accepts as `type_ref`s.
pub const TYPE_REFS: &[&str] = &["Uuid", "String", "Int", "Float", "Boolean", "DateTime"];

/// The boolean literal words.
pub const BOOLEANS: &[&str] = &["true", "false"];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn tables_are_duplicate_free() {
        for (name, words) in [
            ("STRUCTURAL_WORDS", STRUCTURAL_WORDS),
            ("TYPE_REFS", TYPE_REFS),
            ("BOOLEANS", BOOLEANS),
        ] {
            let set: BTreeSet<&str> = words.iter().copied().collect();
            assert_eq!(set.len(), words.len(), "{name} contains a duplicate");
        }
    }

    #[test]
    fn structural_words_are_identifier_shaped() {
        for word in STRUCTURAL_WORDS {
            assert!(
                word.chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'),
                "`{word}` is not an identifier-shaped word"
            );
        }
    }
}
