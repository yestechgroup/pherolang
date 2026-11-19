//! The `.intent` intent-specification grammar: stable-id requirement
//! statements, their `element`/`check` links, and the file parser.

use crate::intents;
use crate::lexer::{lex, Token};

use super::*;

/// A contextual `.intent` keyword: an ordinary identifier that is special
/// only in the grammar position it is parsed here in (the `.deploy`
/// discipline). Escaped forms (`^intent`) are different token variants and
/// never match.
pub(crate) fn intent_keyword<'src>(
    keyword: &'static str,
) -> impl Parser<'src, Tokens<'src>, Span, MoxExtra<'src>> + Clone {
    select! { Token::Ident(text) = e if text == keyword => e.span() }
}

/// An `import "<path>";` declaration of an `.intent` file: the plain
/// production (an intent file imports raw domains only — links are resolved
/// by the coverage pass, never imported). A trailing `;` is tolerated when
/// missing, exactly like the `.ddd` import.
pub(crate) fn intent_import_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, Import, MoxExtra<'src>> + Clone {
    kw(Token::Import)
        .ignore_then(string_lit())
        .then_ignore(kw(Token::Other(';')).or_not())
        .map_with(|path, e| Import {
            path,
            alias: None,
            span: e.span(),
        })
}

/// One `element <kind>:<name>;` / `check <kind>:<name>;` link line. The
/// `;` is tolerated when missing (the formatter emits it canonically); the
/// kind word and the dotted name are taken as written — the driver
/// validates the kind vocabulary, and the coverage pass resolves the name.
pub(crate) fn intent_link_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, IntentLink, MoxExtra<'src>> + Clone {
    select! {
        Token::Ident(text) = e
            if text == intents::ELEMENT || text == intents::CHECK => (e.span(), text)
    }
    .then(qname_segment())
    .then_ignore(kw(Token::Other(':')))
    .then(qname().map_with(|qualified, e| (qualified.full_name(), e.span())))
    .then_ignore(kw(Token::Other(';')).or_not())
    .map_with(
        |(((_slot_span, slot_word), kind), (link_name, name_span)), e| IntentLink {
            slot: if slot_word == intents::ELEMENT {
                IntentLinkSlot::Element
            } else {
                IntentLinkSlot::Check
            },
            kind,
            name: link_name,
            name_span,
            span: e.span(),
        },
    )
}

/// Consumes a run of tokens that cannot start or continue an intent body
/// link, stopping before the body's closing `}` and before identifiers
/// (which could begin the next link). Always consumes at least one token.
pub(crate) fn junk_intent_link<'src>(
) -> impl Parser<'src, Tokens<'src>, (), MoxExtra<'src>> + Clone {
    let first = select! { t if !matches!(t, Token::RBrace) => () };
    let rest = select! {
        t if !matches!(t, Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)) => ()
    };
    first
        .ignore_then(rest.repeated().ignored())
        .validate(|(), e, emitter| {
            emitter.emit(Rich::custom(e.span(), "expected a link line"));
        })
}

/// One `intent <Id> "<statement>" { ... }` declaration: the stable-id
/// requirement statement with its link body. The statement is the string
/// literal's unquoted text, kept verbatim. The id parses token-wise
/// ([`intent_id`]) and its text is sliced from the source afterwards.
pub(crate) fn intent_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, IntentDecl, MoxExtra<'src>> + Clone {
    intent_keyword(intents::INTENT)
        .ignore_then(intent_id())
        .then(string_lit().map_with(|statement, e| (statement, e.span())))
        .then_ignore(kw(Token::LBrace))
        .then(
            intent_link_decl()
                .map(Some)
                .or(junk_intent_link().to(None))
                .repeated()
                .collect::<Vec<_>>(),
        )
        .then_ignore(kw(Token::RBrace))
        .map_with(
            |((id, (statement, statement_span)), links), e| IntentDecl {
                id,
                statement,
                statement_span,
                links: links.into_iter().flatten().collect(),
                span: e.span(),
            },
        )
}

/// One stable-id spelling: `LOAN-001`. The lexer never sees a hyphen as one
/// identifier token (`-001` lexes as a signed int, a letter hyphen as the
/// `Other` catch-all), so the id parses token-wise — a lead identifier and
/// at least one following segment, each a hyphen-led identifier or int —
/// and the written text is sliced from the whole span by
/// [`parse_intents`](crate::parse_intents) (leading zeros survive exactly).
/// A single bare word (`intent LOAN`) is not this production.
pub(crate) fn intent_id<'src>() -> impl Parser<'src, Tokens<'src>, Name, MoxExtra<'src>> + Clone {
    name()
        .then(
            kw(Token::Other('-'))
                .ignore_then(name())
                .or(int_lit().map_with(|_, e| Name {
                    text: String::new(),
                    span: e.span(),
                    escaped: false,
                }))
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .map_with(|(first, _), e| Name {
            text: String::new(),
            span: e.span(),
            escaped: first.escaped,
        })
}

/// Consumes a run of tokens up to the next `import` keyword or the
/// contextual `intent` word (or end of input). File-level recovery for
/// `.intent` files: always consumes at least one token (guaranteeing
/// progress) and emits an error for the skipped region.
pub(crate) fn junk_intent_file<'src>(
) -> impl Parser<'src, Tokens<'src>, (), MoxExtra<'src>> + Clone {
    let rest = select! {
        t if !matches!(t, Token::Import | Token::Ident(intents::INTENT)) => ()
    };
    any()
        .ignore_then(rest.repeated().ignored())
        .validate(|(), e, emitter| {
            emitter.emit(Rich::custom(
                e.span(),
                "expected an import or intent declaration",
            ));
        })
}

/// One top-level item of an `.intent` file.
#[derive(Clone)]
pub(crate) enum IntentFileItem {
    Import(Import),
    Intent(Box<IntentDecl>),
    Junk,
}

pub(crate) fn fold_intent_file(items: Vec<IntentFileItem>) -> (IntentFile, Option<Span>) {
    let mut imports = Vec::new();
    let mut intents = Vec::new();
    // Span of the first `import` that follows an intent (a syntax error
    // reported by the caller's `validate`).
    let mut late_import = None;
    for item in items {
        match item {
            IntentFileItem::Import(decl) => {
                if intents.is_empty() {
                    imports.push(decl);
                } else if late_import.is_none() {
                    late_import = Some(decl.span);
                }
            }
            IntentFileItem::Intent(decl) => intents.push(*decl),
            IntentFileItem::Junk => {}
        }
    }
    (IntentFile { imports, intents }, late_import)
}

pub(crate) fn intent_file<'src>(
) -> impl Parser<'src, Tokens<'src>, IntentFile, MoxExtra<'src>> + Clone {
    let item = choice((
        intent_import_decl().map(IntentFileItem::Import),
        intent_decl().map(|decl| IntentFileItem::Intent(Box::new(decl))),
    ))
    .or(junk_intent_file().to(IntentFileItem::Junk));

    item.repeated()
        .collect::<Vec<_>>()
        .then_ignore(end())
        .map(fold_intent_file)
        .validate(|(file, late_import), _, emitter| {
            if let Some(span) = late_import {
                emitter.emit(Rich::custom(span, "`import` after an intent declaration"));
            }
            file
        })
}

/// The outcome of parsing an `.intent` source: as much of the file as could
/// be recovered, plus all encountered errors. Parsing never panics and
/// always produces a result.
#[derive(Debug, Clone, PartialEq)]
pub struct IntentsParseResult {
    /// The recovered intent file, or `None` if no output could be produced
    /// at all.
    pub ast: Option<IntentFile>,
    /// All errors encountered during lexing and parsing.
    pub errors: Vec<ParseError>,
}

/// Lex and parse an `.intent` source text: `import` declarations followed by
/// `intent` declarations (imports must come first).
///
/// This function never panics and always recovers as much of the AST as
/// possible; check [`IntentsParseResult::errors`] for syntax problems.
pub fn parse_intents(source: &str) -> IntentsParseResult {
    let tokens = match lex(source) {
        Ok(tokens) => tokens,
        Err(error) => {
            return IntentsParseResult {
                ast: None,
                errors: vec![ParseError {
                    message: error.to_string(),
                    span: error.span,
                }],
            }
        }
    };
    let (mut ast, errors) = intent_file().parse(Tokens::new(&tokens)).into_output_errors();
    // The intent ids' written text: the grammar parses the hyphen-separated
    // segments token-wise, so the verbatim source slice is the id's text
    // (leading zeros survive exactly).
    if let Some(file) = ast.as_mut() {
        for intent in &mut file.intents {
            let span = intent.id.span;
            intent.id.text = source
                .get(span.start..span.end)
                .expect("the id span is a source slice")
                .to_string();
        }
    }
    IntentsParseResult {
        ast,
        errors: errors
            .into_iter()
            .map(|error| ParseError {
                message: error.to_string(),
                span: *error.span(),
            })
            .collect(),
    }
}
