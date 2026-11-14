//! Shared grammar-doc test helpers.
//!
//! Surfaces whose normative grammar is a fenced EBNF block in their parser
//! module docs (`rex-syntax`, `rex-ifml`) use these helpers to extract the
//! blocks and their quoted terminals, then cross-check them against the
//! surface's keyword/word tables. The helpers ship in the normal dependency
//! (they are a few hundred bytes); keeping one copy is what makes the
//! tripwire consistent across surfaces.

/// The module docs of `source`: the leading `//!` line run, comment markers
/// stripped. Stops at the first non-`//!` line.
pub fn module_docs(source: &str) -> String {
    let mut docs = String::new();
    for line in source.lines() {
        let Some(rest) = line.strip_prefix("//!") else {
            break;
        };
        docs.push_str(rest.strip_prefix(' ').unwrap_or(rest));
        docs.push('\n');
    }
    docs
}

/// The fenced ```text blocks of `docs`: the EBNF grammar blocks.
///
/// # Panics
///
/// Panics if a block is opened and never closed — a broken grammar doc is a
/// test failure, not a production path.
pub fn grammar_blocks(docs: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<Vec<String>> = None;
    for line in docs.lines() {
        match (current.as_mut(), line.trim()) {
            (None, "```text") => current = Some(Vec::new()),
            (Some(block), "```") => {
                blocks.push(block.join("\n"));
                current = None;
            }
            (Some(block), line) => block.push(line.to_string()),
            (None, _) => {}
        }
    }
    assert!(current.is_none(), "unterminated grammar block");
    blocks
}

/// The quoted terminals of `blocks` that are shaped like words (punctuation
/// terminals such as `"{"` are skipped).
pub fn grammar_words(blocks: &[String]) -> Vec<String> {
    let mut words = Vec::new();
    for block in blocks {
        for terminal in block.split('"').skip(1).step_by(2) {
            if !terminal.is_empty()
                && terminal
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            {
                words.push(terminal.to_string());
            }
        }
    }
    words
}

/// The quoted word terminals of `source`'s module-doc grammar blocks — the
/// one-call form the surface test modules use.
pub fn words_in(source: &str) -> Vec<String> {
    grammar_words(&grammar_blocks(&module_docs(source)))
}
