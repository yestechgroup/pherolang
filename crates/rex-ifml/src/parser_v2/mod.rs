//! The chumsky `.ifml` parser (the v2 path).
//!
//! This module is the surface's grammar authority once phase 3 removes the
//! Pest grammar. It produces the spanned [`crate::ast`] tree; lowering to
//! `rex_ir::ifml` lives in [`crate::lower`]. The Pest parser remains the live
//! entry until the cutover, so both trees coexist during the migration.
//!
//! Design rules (parity with the historical PEG grammar):
//! * there are **no keyword tokens** — every word is an identifier and is
//!   special only in the position the parser dispatches on;
//! * property assignments are `Ident ":" value ";"`, so `on:`/`if:`/`use:`
//!   stay valid property names, and `use "M"` vs `use: "M"` is decided by the
//!   colon lookahead;
//! * body phases are exactly the PEG's: view/container `params? label?
//!   property* (container|component|event|use|condition)*`; component
//!   `property* (column|field|chart|condition)* (event|condition)*`; module
//!   `input output property* (container|component|event|use)*`; action
//!   `property* event*`; command members in any order;
//! * a `param_default` is `string | number | boolean` (negative numbers need
//!   the adjacent minus), while expression-position negative numbers are
//!   unary nodes.

mod commands;
mod components;
mod declarations;
mod events;
mod expr;
mod modules;
mod views;

use chumsky::prelude::*;
use rex_ir::ifml::{BinOp, UnaryOp};

use crate::ast::*;
use crate::lexer::{lex, Token};
use crate::IfmlParseError;
use rex_parser::Span;

pub(crate) use rex_parser::chain;

/// The token type of this surface.
pub(crate) type Tok<'src> = Token<'src>;
/// The byte-span token input.
pub(crate) type Input<'src> = rex_parser::Tokens<'src, Tok<'src>>;
/// The chumsky extra: `Rich` errors carrying byte spans.
pub(crate) type Extra<'src> = extra::Err<Rich<'src, Tok<'src>, Span>>;
/// A boxed parser: the recursion-breaking alias used by the recursive
/// expression/body/command groups.
pub(crate) type BoxedP<'src, O> = Boxed<'src, 'src, Input<'src>, O, Extra<'src>>;

/// One exact punctuation/keyword token, yielding its span.
pub(crate) fn tok<'src>(
    token: Token<'src>,
) -> impl Parser<'src, Input<'src>, Span, Extra<'src>> + Clone {
    rex_parser::kw(token)
}

/// One contextual word (an identifier whose text is exactly `word`), yielding
/// its span.
pub(crate) fn word<'src>(
    word: &'static str,
) -> impl Parser<'src, Input<'src>, Span, Extra<'src>> + Clone {
    select! { Token::Ident(text) = e if text == word => e.span() }
}

/// One identifier as `(text, span)`.
pub(crate) fn ident<'src>() -> impl Parser<'src, Input<'src>, (String, Span), Extra<'src>> + Clone {
    select! { Token::Ident(text) = e => (text.to_string(), e.span()) }
}

/// One quoted string as `(unescaped text, token span)`.
pub(crate) fn string_lit<'src>(
) -> impl Parser<'src, Input<'src>, (String, Span), Extra<'src>> + Clone {
    select! { Token::Str(raw) = e => (unescape_json(raw), e.span()) }
}

/// One unsigned number as `(value, token span)`.
pub(crate) fn number_lit<'src>() -> impl Parser<'src, Input<'src>, (f64, Span), Extra<'src>> + Clone
{
    select! { Token::Number(raw) = e => (raw.parse::<f64>().unwrap_or(0.0), e.span()) }
}

/// One boolean literal as `(value, token span)`.
pub(crate) fn boolean_lit<'src>(
) -> impl Parser<'src, Input<'src>, (bool, Span), Extra<'src>> + Clone {
    select! {
        Token::Ident("true") = e => (true, e.span()),
        Token::Ident("false") = e => (false, e.span()),
    }
}

/// A `key: <value>;` property assignment; the span covers key through `;`.
pub(crate) fn property<'src>() -> impl Parser<'src, Input<'src>, Property, Extra<'src>> + Clone {
    ident()
        .then_ignore(tok(Token::Colon))
        .then(expr::value_parser())
        .then(tok(Token::Semi).or_not())
        .validate(|((key, value), semi), e, emitter| {
            if semi.is_none() {
                emitter.emit(Rich::custom(
                    value.span,
                    format!("expected ';' after the '{}' property", key.0),
                ));
            }
            Property {
                key: key.0,
                value,
                span: e.span(),
            }
        })
}

/// A `params { ... };` block.
pub(crate) fn params_block<'src>() -> impl Parser<'src, Input<'src>, Params, Extra<'src>> + Clone {
    word("params")
        .ignore_then(braced_params())
        .then_ignore(tok(Token::Semi))
        .map_with(|params, e| Params {
            decls: params.decls,
            span: e.span(),
        })
}

/// The braced parameter block a module's `input`/`output` carries.
pub(crate) fn braced_params<'src>() -> impl Parser<'src, Input<'src>, Params, Extra<'src>> + Clone {
    tok(Token::LBrace)
        .ignore_then(parameter_block_contents())
        .then_ignore(tok(Token::RBrace))
        .map_with(|decls, e| Params {
            decls,
            span: e.span(),
        })
}

/// A `name: Type (= default)?` parameter declaration.
pub(crate) fn parameter_decl<'src>(
) -> impl Parser<'src, Input<'src>, ParamDecl, Extra<'src>> + Clone {
    ident()
        .then_ignore(tok(Token::Colon))
        .then(expr::type_ref())
        .then(tok(Token::Eq).ignore_then(expr::param_default()).or_not())
        .map_with(|((name, type_ref), default), e| ParamDecl {
            name: name.0,
            type_ref: type_ref.0,
            default: default.map(|(value, _)| value),
            span: e.span(),
        })
}

/// The non-empty comma-separated parameter list inside `{ ... }`.
fn parameter_block_contents<'src>(
) -> impl Parser<'src, Input<'src>, Vec<ParamDecl>, Extra<'src>> + Clone {
    parameter_decl()
        .then(
            tok(Token::Comma)
                .ignore_then(parameter_decl())
                .repeated()
                .collect::<Vec<_>>(),
        )
        .map(|(first, rest)| std::iter::once(first).chain(rest).collect::<Vec<_>>())
}

/// A `label "...";` declaration.
pub(crate) fn label_decl<'src>() -> impl Parser<'src, Input<'src>, Label, Extra<'src>> + Clone {
    word("label")
        .ignore_then(string_lit())
        .then_ignore(tok(Token::Semi))
        .map_with(|(text, _), e| Label {
            text,
            span: e.span(),
        })
}

/// An `if <expression>;` condition statement.
pub(crate) fn condition<'src>() -> impl Parser<'src, Input<'src>, Condition, Extra<'src>> + Clone {
    word("if")
        .ignore_then(expr::expr_parser())
        .then_ignore(tok(Token::Semi))
        .map_with(|expr, e| Condition {
            expr,
            span: e.span(),
        })
}

/// A word that can introduce a top-level declaration.
pub(crate) fn is_top_introducer(token: &Token<'_>) -> bool {
    matches!(
        token,
        Token::Ident(text)
            if matches!(
                *text,
                "import" | "domain" | "view" | "action" | "module" | "actor" | "command"
            )
    )
}

/// Consumes a junk region that cannot be a declaration, stopping at the next
/// top-level introducer. Always consumes at least one token (the caller
/// reached junk because no declaration matched), so recovery terminates.
fn junk_top<'src>() -> impl Parser<'src, Input<'src>, (), Extra<'src>> + Clone {
    any()
        .ignore_then(rex_parser::junk_until(is_top_introducer).or_not())
        .validate(|_, e, emitter| {
            emitter.emit(Rich::custom(
                e.span(),
                "expected a declaration (import, domain, view, action, module, actor, or command)",
            ));
        })
        .ignored()
}

/// Consumes one or more junk tokens inside a body, stopping before the
/// enclosing `}` and before the next member/top-level introducer. The first
/// token is consumed even when it is a member introducer (a malformed member
/// was committed), but never the enclosing `}` or the next declaration.
pub(crate) fn junk_member<'src>(
    stop: fn(&Token<'src>) -> bool,
    message: &'static str,
) -> impl Parser<'src, Input<'src>, (), Extra<'src>> + Clone {
    let first = select! {
        token if !matches!(token, Token::RBrace) && !is_top_introducer(&token) => ()
    };
    first
        .ignore_then(rex_parser::junk_until(stop).or_not())
        .validate(move |_, e, emitter| {
            emitter.emit(Rich::custom(e.span(), message));
        })
        .ignored()
}

/// One top-level item.
enum Top {
    Import(ImportDecl),
    Domain(DomainDecl),
    Decl(Declaration),
    Junk,
}

/// The whole file: `(import|domain)* (import|view|action|module|actor|command)*`.
pub(crate) fn file_parser<'src>() -> impl Parser<'src, Input<'src>, IfmlFile, Extra<'src>> + Clone {
    let import = declarations::import_decl();
    let domain = declarations::domain_decl();
    // The prologue stops at the first declaration introducer; junk before a
    // declaration is recovered by the declaration phase (so a leading `view`
    // is never eaten by prologue recovery).
    let prologue = choice((import.clone().map(Top::Import), domain.map(Top::Domain)));
    let declaration = choice((
        import.map(Top::Import),
        views::view_decl().map(|view| Top::Decl(Declaration::View(Box::new(view)))),
        declarations::action_decl().map(|action| Top::Decl(Declaration::Action(Box::new(action)))),
        modules::module_decl().map(|module| Top::Decl(Declaration::Module(Box::new(module)))),
        declarations::actor_decl().map(|actor| Top::Decl(Declaration::Actor(Box::new(actor)))),
        commands::command_decl().map(|command| Top::Decl(Declaration::Command(Box::new(command)))),
        junk_top().map(|()| Top::Junk),
    ));

    prologue
        .repeated()
        .collect::<Vec<_>>()
        .then(declaration.repeated().collect::<Vec<_>>())
        .map_with(|(prologue, declarations), e| {
            let mut imports = Vec::new();
            let mut domains = Vec::new();
            let mut decls = Vec::new();
            for item in prologue.into_iter().chain(declarations) {
                match item {
                    Top::Import(import) => imports.push(import),
                    Top::Domain(domain) => domains.push(domain),
                    Top::Decl(declaration) => decls.push(declaration),
                    Top::Junk => {}
                }
            }
            IfmlFile {
                imports,
                domains,
                declarations: decls,
                span: e.span(),
            }
        })
}

/// The maximum bracket nesting accepted from untrusted editor input.
///
/// The corpus is shallow; this bounds recursive descent before the parser
/// can overflow the LSP thread's stack. Exceeding it is one diagnostic.
pub(crate) const MAX_NESTING_DEPTH: usize = 256;

fn classify_bracket(token: &Token<'_>) -> Option<rex_parser::nesting::Bracket> {
    use rex_parser::nesting::Bracket;
    match token {
        Token::LBrace | Token::LBracket | Token::LParen => Some(Bracket::Open),
        Token::RBrace | Token::RBracket | Token::RParen => Some(Bracket::Close),
        _ => None,
    }
}

/// Parse `text`, recovering from broken regions.
///
/// Returns the partial AST (when a file node could be recovered) plus one
/// issue per recovered error. Parsing never panics and always terminates;
/// the junk paths consume at least one token.
pub(crate) fn parse_file_recovering(text: &str) -> (Option<IfmlFile>, Vec<rex_parser::ParseError>) {
    let tokens = lex(text);
    if let Some(span) =
        rex_parser::nesting::limit_exceeded(&tokens, classify_bracket, MAX_NESTING_DEPTH)
    {
        return (
            None,
            vec![rex_parser::ParseError {
                message: format!("nesting depth exceeds {MAX_NESTING_DEPTH}"),
                span,
            }],
        );
    }
    let (ast, errors) = file_parser()
        .parse(Input::new(&tokens))
        .into_output_errors();
    let issues = errors
        .into_iter()
        .map(|error| rex_parser::ParseError {
            message: error.to_string(),
            span: *error.span(),
        })
        .collect();
    (ast, issues)
}

/// Parse `text` into the spanned AST, rejecting any recovered error (the
/// strict contract the public entries keep).
pub(crate) fn parse_file(text: &str) -> Result<IfmlFile, IfmlParseError> {
    let (ast, issues) = parse_file_recovering(text);
    match (ast, issues.into_iter().next()) {
        (Some(ast), None) => Ok(ast),
        (_, Some(issue)) => Err(parse_error(Some(issue.span), &issue.message)),
        (None, None) => Err(parse_error(None, "expected a file declaration")),
    }
}

/// Build the crate's parse error from an optional span and a message.
pub(crate) fn parse_error(span: Option<Span>, message: &str) -> IfmlParseError {
    IfmlParseError::Parse {
        position: span
            .map(|span| format!("{}..{}", span.start, span.end))
            .unwrap_or_else(|| "unknown".to_string()),
        message: message.to_string(),
    }
}

/// Parse `text` and lower it into the model + index, recording the file under
/// `path`. Lowering errors (chart authoring) are collected on the lenient
/// path; the strict entry rejects them.
pub(crate) fn parse_model_with_path(
    path: &str,
    text: &str,
) -> Result<(rex_ir::ifml::IfmlModel, crate::index::IfmlIndex), IfmlParseError> {
    let file = parse_file(text)?;
    let (model, index, errors) = crate::lower::lower(&file, path, text);
    match errors.into_iter().next() {
        Some(error) => Err(parse_error(Some(error.span), &error.message)),
        None => Ok((model, index)),
    }
}

/// The lenient entry: a partial model + index and every parse/lowering issue.
pub(crate) fn parse_model_recovering_with_path(
    path: &str,
    text: &str,
) -> (
    (rex_ir::ifml::IfmlModel, crate::index::IfmlIndex),
    Vec<rex_parser::ParseError>,
) {
    let (ast, mut issues) = parse_file_recovering(text);
    let Some(file) = ast else {
        return (
            (
                rex_ir::ifml::IfmlModel::default(),
                crate::index::IfmlIndex::single_file(path, text),
            ),
            issues,
        );
    };
    let (model, index, errors) = crate::lower::lower(&file, path, text);
    issues.extend(errors.into_iter().map(|error| rex_parser::ParseError {
        message: error.message,
        span: error.span,
    }));
    ((model, index), issues)
}

/// Decode a JSON-style string payload (quotes already stripped, escapes
/// validated by the lexer). Mirrors the historical `.ifml` decoder exactly:
/// the JSON escape set decodes; an unreachable malformed escape is dropped.
pub(crate) fn unescape_json(raw: &str) -> String {
    let mut result = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some('/') => result.push('/'),
                Some('b') => result.push('\u{0008}'),
                Some('f') => result.push('\u{000C}'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if let Ok(code) = u32::from_str_radix(&hex, 16) {
                        if let Some(ch) = char::from_u32(code) {
                            result.push(ch);
                        }
                    }
                }
                _ => {}
            }
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod smoke {
    use super::parse_file;

    #[test]
    fn parses_empty_input() {
        let file = parse_file("").expect("empty file");
        assert!(file.declarations.is_empty());
    }

    #[test]
    fn parses_a_minimal_view() {
        let file = parse_file("view \"X\" { }").expect("minimal view");
        assert_eq!(file.declarations.len(), 1);
    }

    #[test]
    fn parses_a_module_with_a_string_default() {
        let file = parse_file(
            r#"module "M" { input { items: Item, fields: String = "name" } output { selected: Uuid } }"#,
        )
        .expect("module with defaults");
        assert_eq!(file.declarations.len(), 1);
    }
}
