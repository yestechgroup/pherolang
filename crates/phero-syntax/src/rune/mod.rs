//! The Rune (`.rosetta`) surface: the FINOS Rune DSL's lexer, spanned AST and
//! parser (RN9, ADR 0012 D2, ADR 0013).
//!
//! Rune's behaviour is pinned by the Java reference implementation
//! (`com.regnosys.rosetta` 9.58.1), the oracle that the Rune conformance
//! goldens and the CDM corpus snapshot are checked against. The grammar is a
//! transcription of its Xtext grammar (`docs/rune/reference/Rosetta.xtext`);
//! the EBNF below is the condensed, normative form of what this parser
//! accepts. Resolution and the semantic model live in `phero-rune-resolve` and
//! `phero-rune-model`; lowering this AST into that model is
//! `sigil_syntax::lower`.
//!
//! Rune has its own token enum ([`lexer::RuneToken`]) rather than rows in the
//! shared `.mox`-family token table, for the reasons ADR 0013 records. It
//! still uses the shared [`phero_parser::Tokens`] input, [`phero_parser::Span`]
//! byte spans and [`phero_parser::ParseError`].
//!
//! # Grammar
//!
//! Every quoted word is matched by text against a Rune identifier, so it is
//! special only in the position written here: Rune reserves no identifiers
//! (`crate::words::RUNE_CONTEXTUAL`). The hyphenated words are single tokens
//! (`crate::words::RUNE_HYPHEN_KEYWORDS`), and `"T+1"` … `"T+5"` are three
//! touching tokens each. `ID` is `^?[A-Za-z_][A-Za-z0-9_]*`, and `^` escapes a
//! word out of a keyword position. `qname` is `ID ("." ID)*`, and `doc` is the
//! definition string `"<" STRING ">"`.
//!
//! ```text
//! file          = "override"? "namespace" (qname | STRING) (":" doc?)?
//!                 ("scope" ID doc?)? ("version" STRING)?
//!                 import* configuration* element* ;
//! import        = "import" qname ("." "*")? ("as" ID)? ;
//! configuration = ("isEvent" | "isProduct") "root" qname ";"? ;
//! element       = func | rule | report | rule_source | data | enumeration
//!               | annotation_decl | type_alias | basic_type | record_type
//!               | library_function | schema | body | corpus | segment | meta_type ;
//!
//! data          = ("type" | "choice") ID ("extends" qname)? ":" doc? marks*
//!                 (attribute | choice_option)* condition* ;
//! attribute     = "override"? ID type_call cardinality doc? marks* ;
//! choice_option = qname doc? marks* ;
//! cardinality   = "(" INT ".." (INT | "*") ")" ;
//! type_call     = qname ("(" ID ":" expression ("," ID ":" expression)* ")")? ;
//! enumeration   = "enum" ID ("extends" qname)? ":" doc? marks*
//!                 (ID ("displayName" STRING)? doc? marks*)* ;
//! annotation_decl = "annotation" ID ":" doc? ("[" "prefix" ID "]")? attribute* ;
//! type_alias    = "typeAlias" ID params? ":" doc? type_call condition* ;
//! basic_type    = "basicType" ID params? doc? ;
//! record_type   = "recordType" ID doc? "{" attribute* "}" ;
//! library_function = "library" "function" ID "(" (ID ":" type_call ("," ID ":" type_call)*)? ")"
//!                 type_call doc? ;
//! condition     = "condition" ID? ":" doc? marks* expression ;
//!
//! marks         = "[" ( "docReference" doc_reference | "label" label
//!                     | "ruleReference" rule_reference | annotation_use ) "]" ;
//! annotation_use = qname annotation_path? ;
//! annotation_path = ("->" | "->>") ID annotation_path? | "item" ;
//! doc_reference = qname* STRING? ("structured_provision" STRING)? ("provision" STRING)?
//!                 rationale* "reportedField"? ;
//! rationale     = "rationale" STRING ("rationale_author" STRING)?
//!               | "rationale_author" STRING ("rationale" STRING)? ;
//! label         = ("for" annotation_path)? "as" STRING ;
//! rule_reference = ("for" annotation_path)? (qname | "empty") ;
//!
//! func          = "func" ID dispatch? ("extends" qname)? ":" doc?
//!                 ("[" ("ingest" | "enrich" | "projection") qname "]")* marks*
//!                 ("inputs" ":" attribute+)? ("output" ":" attribute)?
//!                 ("alias" ID ":" expression)* ("condition" ID? ":" doc? expression)*
//!                 (("set" | "add") ID ("->" ID)* ":" expression)*
//!                 ("post-condition" ID? ":" doc? expression)* ;
//! dispatch      = "(" ID ":" qname "->" ID ")" ;
//! rule          = ("reporting" | "eligibility") "rule" ID ("from" qname)? ":" doc? marks*
//!                 expression ("as" STRING)? ;
//! report        = "report" qname* STRING* "in" timing "from" qname
//!                 "when" qname ("and" qname)* "with" "type" qname ("with" "source" qname)? ;
//! timing        = "real-time" | "ASATP" | "T+1" | "T+2" | "T+3" | "T+4" | "T+5" ;
//! rule_source   = "rule" "source" ID ("extends" qname)? "{"
//!                 (qname ":" (("+" | "-") ID marks*)*)* "}" ;
//! schema        = "schema" ID qname ;
//! body          = "body" ID ID doc? ;
//! corpus        = "corpus" ID ID* (STRING | doc)* ;
//! segment       = "segment" ID ;
//! meta_type     = "metaType" ID type_call ;
//!
//! expression    = or_expr ("then" or_expr?)* ;
//! or_expr       = and_expr ("or" and_expr)* ;
//! and_expr      = equality ("and" equality)* ;
//! equality      = comparison (("=" | "<>") ("any" | "all")? comparison)* ;
//! comparison    = additive ((">=" | "<=" | ">" | "<") ("any" | "all")? additive)* ;
//! additive      = multiplicative (("+" | "-") multiplicative)* ;
//! multiplicative = binary_word (("*" | "/") binary_word)* ;
//! binary_word   = postfix (("contains" | "disjoint" | "default" | "join") postfix?)* ;
//! postfix       = primary step* ;
//! step          = ("->" | "->>") ID? | ("single" | "multiple")? "exists" | "is" "absent"
//!               | "only-element" | "count" | "flatten" | "distinct" | "reverse"
//!               | "first" | "last" | "sum" | "one-of"
//!               | ("required" | "optional") "choice" ID ("," ID)*
//!               | "to-string" | "to-number" | "to-int" | "to-time" | "to-zoned-date-time"
//!               | "to-date-time" | "to-date" | "to-enum" qname
//!               | "switch" case ("," case)*
//!               | "with-meta" "{" ID ":" expression ("," ID ":" expression)* "}"
//!               | "as" qname
//!               | ("sort" | "min" | "max") inline?
//!               | ("reduce" | "filter" | "extract") (inline | or_expr)? ;
//! case          = ("default" | literal "then" | qname "then") expression ;
//! inline        = (ID ("," ID)*)? "[" expression "]" ;
//! primary       = "only" "exists" | "(" expression ")" | "if" expression "then" or_expr
//!                 ("else" or_expr)? | literal | "empty" | "item"
//!               | qname "{" (ID ":" expression "as-key"? ",")* "..."? "}"
//!               | qname ("(" (expression ("," expression)*)? ")")?
//!               | "[" (expression ("," expression)*)? "]" ;
//! literal       = STRING | "True" | "False" | NUMBER | INT ;
//! ```
//!
//! Every binary and postfix level also has Xtext's left-parameter-less form,
//! where the operator stands alone and applies to the implicit `item`
//! (`condition C: one-of`).
//!
//! # Recovery and limits
//!
//! - **Element-level recovery.** A broken element keeps its own precise error,
//!   then the parser skips at least one token and every token up to the next
//!   element start: an identifier in `crate::words::RUNE_ELEMENT_WORDS`, or
//!   `reporting`/`eligibility`. Parsing resumes there, so every broken element
//!   is reported. Skipping always consumes a token, so recovery terminates.
//! - **Stray input.** A run of tokens that starts no element gets one error
//!   spanning the run: `expected a Rune element (type, choice, enum, …), found X`.
//! - **Partial units.** [`RuneParseResult::ast`] is the unit of every element
//!   that parsed. It is `None` only when the header fails or the nesting
//!   budget is exceeded. Callers that must not lower a broken file (the
//!   `sigil_syntax::parse` shim) drop the unit whenever there are errors.
//! - **Nesting budget.** Input nested more than [`MAX_NESTING_DEPTH`] (256)
//!   brackets deep is refused before parsing, with one error and no unit.

pub mod ast;
mod expr;
pub mod lexer;
mod parser;

pub use expr::parse_rune_expression;
pub use parser::{parse_rune, RuneParseResult, MAX_NESTING_DEPTH};

#[cfg(test)]
mod grammar_doc_tests {
    use std::collections::BTreeSet;

    use crate::words::{
        RUNE_BOOLEAN_WORDS, RUNE_CONTEXTUAL, RUNE_DOC_REFERENCE_WORDS, RUNE_ELEMENT_WORDS,
        RUNE_EXPRESSION_WORDS, RUNE_HYPHEN_KEYWORDS, RUNE_REPORT_STOP_WORDS, RUNE_TIMING_WORDS,
    };

    const MODULE_SOURCE: &str = include_str!("mod.rs");
    const PARSER_SOURCES: [&str; 2] = [include_str!("parser.rs"), include_str!("expr.rs")];

    fn contextual() -> BTreeSet<String> {
        RUNE_CONTEXTUAL
            .iter()
            .map(|word| word.to_string())
            .collect()
    }

    /// The quoted word terminals of the EBNF above.
    fn grammar_words() -> BTreeSet<String> {
        phero_parser::grammar_doc::words_in(MODULE_SOURCE)
            .into_iter()
            .collect()
    }

    /// Every word literal the parser matches: the arguments of `kw`,
    /// `word_kw` and `op_kw`, and of the `simple`/`functional_step` operator
    /// helpers, outside the test modules.
    fn parser_words() -> BTreeSet<String> {
        let mut words = BTreeSet::new();
        for source in PARSER_SOURCES {
            let code = source.split("#[cfg(test)]").next().unwrap_or(source);
            for marker in ["kw(\"", "word_kw(\"", "op_kw(\""] {
                for piece in code.split(marker).skip(1) {
                    if let Some(word) = piece.split('"').next() {
                        words.insert(word.to_string());
                    }
                }
            }
            for marker in ["simple(", "functional_step("] {
                for piece in code.split(marker).skip(1) {
                    let call = piece.split(')').next().unwrap_or("");
                    if let Some(word) = call.split('"').nth(1) {
                        words.insert(word.to_string());
                    }
                }
            }
        }
        words
    }

    #[test]
    fn every_grammar_word_is_a_rune_word() {
        let table = contextual();
        for word in grammar_words() {
            assert!(
                table.contains(&word),
                "EBNF word `{word}` is not in RUNE_CONTEXTUAL"
            );
        }
    }

    #[test]
    fn every_rune_word_appears_in_the_grammar() {
        let grammar = grammar_words();
        for word in RUNE_CONTEXTUAL {
            assert!(grammar.contains(*word), "`{word}` is missing from the EBNF");
        }
        let docs = phero_parser::grammar_doc::module_docs(MODULE_SOURCE);
        for word in RUNE_HYPHEN_KEYWORDS.iter().chain(RUNE_TIMING_WORDS) {
            assert!(
                docs.contains(&format!("\"{word}\"")),
                "`{word}` is missing from the EBNF"
            );
        }
    }

    /// The tables and the parser agree exactly: a word the parser matches is
    /// registered, and a registered word is matched somewhere.
    #[test]
    fn the_word_tables_are_the_parser_words() {
        let mut registered = contextual();
        registered.extend(RUNE_HYPHEN_KEYWORDS.iter().map(|word| word.to_string()));
        registered.extend(RUNE_TIMING_WORDS.iter().map(|word| word.to_string()));
        registered.insert("...".to_string());
        let used = parser_words();
        for word in &used {
            assert!(
                registered.contains(word),
                "parser word `{word}` has no table entry"
            );
        }
        for word in &registered {
            assert!(
                used.contains(word),
                "table word `{word}` is never matched by the parser"
            );
        }
    }

    #[test]
    fn the_stop_tables_are_rune_words() {
        let table = contextual();
        for word in RUNE_ELEMENT_WORDS
            .iter()
            .chain(RUNE_REPORT_STOP_WORDS)
            .chain(RUNE_DOC_REFERENCE_WORDS)
            .chain(RUNE_EXPRESSION_WORDS)
            .chain(RUNE_BOOLEAN_WORDS)
        {
            assert!(
                table.contains(*word),
                "stop word `{word}` is not in RUNE_CONTEXTUAL"
            );
        }
    }
}
