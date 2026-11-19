//! Integration tests for `.intent` files (issue pherolang#196): stable-id
//! requirement statements with `element`/`check` links, imports first. The
//! `intent`/`element`/`check` words are contextual (the `intents` table);
//! `import` is a real lexer keyword; a link's kind word may be a keyword
//! token (`class`, `capability`, `event`), and the id's hyphens never lex
//! as one token — the written text is sliced from the span.

use phero_syntax::ast::*;
use phero_syntax::{format_intents, parse_intents, Span};

fn span_text(source: &str, span: Span) -> &str {
    &source[span.start..span.end]
}

fn parse_ok(source: &str) -> IntentFile {
    let result = parse_intents(source);
    assert!(
        result.errors.is_empty(),
        "{source:?}: unexpected errors: {:?}",
        result.errors
    );
    result.ast.expect("expected an AST")
}

// --- parsing -----------------------------------------------------------------

#[test]
fn intents_parse_with_ids_statements_and_links() {
    let source = concat!(
        "import \"support.mox\";\n",
        "\n",
        "intent LOAN-001 \"A loan above the desk limit is decided by a human.\" {\n",
        "    element class:LoanApplication;\n",
        "    element capability:Underwrite;\n",
        "    check decision:Underwriting;\n",
        "    check invariant:LoanApplication;\n",
        "}\n",
    );
    let file = parse_ok(source);
    assert_eq!(file.imports.len(), 1);
    assert_eq!(file.intents.len(), 1);

    let intent = &file.intents[0];
    assert_eq!(intent.id.text, "LOAN-001");
    assert_eq!(span_text(source, intent.id.span), "LOAN-001");
    assert_eq!(
        intent.statement,
        "A loan above the desk limit is decided by a human."
    );
    assert_eq!(intent.links.len(), 4);
    assert_eq!(intent.links[0].slot, IntentLinkSlot::Element);
    assert_eq!(intent.links[0].kind.text, "class");
    assert_eq!(intent.links[0].name, "LoanApplication");
    assert_eq!(intent.links[2].slot, IntentLinkSlot::Check);
    assert_eq!(intent.links[2].kind.text, "decision");
    assert_eq!(intent.links[2].name, "Underwriting");
}

#[test]
fn multi_segment_and_multi_digit_ids_survive_lexing_exactly() {
    let source = "import \"a.mox\"\n\nintent REFUND-DESK-007 \"x\" {\n    element class:A;\n}\n";
    let file = parse_ok(source);
    assert_eq!(file.intents[0].id.text, "REFUND-DESK-007");
    assert_eq!(
        span_text(source, file.intents[0].id.span),
        "REFUND-DESK-007"
    );
}

#[test]
fn a_bare_single_word_id_is_a_syntax_error() {
    let result = parse_intents("import \"a.mox\"\n\nintent LOAN \"no segment\" {\n}\n");
    assert!(
        !result.errors.is_empty(),
        "the stable-id shape needs at least one hyphen segment"
    );
}

#[test]
fn kind_words_that_are_keywords_parse_as_names() {
    // `class`, `capability`, and `event` are real lexer keywords on other
    // surfaces; in the link-kind position they are names.
    let source = concat!(
        "import \"a.mox\"\n",
        "\n",
        "intent LOAN-001 \"x\" {\n",
        "    element class:A;\n",
        "    element capability:B;\n",
        "    element event:C;\n",
        "}\n",
    );
    let file = parse_ok(source);
    let kinds: Vec<&str> = file.intents[0]
        .links
        .iter()
        .map(|link| link.kind.text.as_str())
        .collect();
    assert_eq!(kinds, ["class", "capability", "event"]);
}

#[test]
fn imports_after_the_first_intent_are_a_syntax_error() {
    let result =
        parse_intents("intent LOAN-001 \"x\" {\n    element class:A;\n}\n\nimport \"a.mox\"\n");
    assert!(!result.errors.is_empty(), "an `import` after an intent");
}

// --- formatting --------------------------------------------------------------

#[test]
fn intents_format_canonically_and_are_idempotent() {
    let canonical = concat!(
        "import \"a.mox\";\n",
        "\n",
        "intent LOAN-002 \"y\" {\n",
        "    element capability:Underwrite;\n",
        "}\n",
        "\n",
        "intent LOAN-001 \"x\" {\n",
        "    element class:LoanApplication;\n",
        "    check decision:Underwriting;\n",
        "}\n",
    );
    let messy = concat!(
        "intent LOAN-002 \"y\" { element capability:Underwrite }\n",
        "import \"a.mox\"\n",
        "intent LOAN-001 \"x\" { element class : LoanApplication check decision:Underwriting ; }\n",
    );
    let formatted = format_intents(messy).expect("format");
    assert_eq!(formatted, canonical, "canonical layout drifted");
    assert_eq!(
        format_intents(&formatted).expect("format"),
        formatted,
        "formatting is a fixpoint"
    );
    assert_eq!(
        parse_intents(&formatted).errors.len(),
        0,
        "the canonical form parses"
    );
}
