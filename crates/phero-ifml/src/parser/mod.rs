//! The chumsky `.ifml` parser and the surface's normative grammar authority.
//!
//! Source text is lexed by logos ([`crate::lexer`]), parsed here into the
//! spanned [`crate::ast`] tree, and lowered to `phero_ir::ifml` by
//! [`crate::lower`]. This module is the **grammar authority** for `.ifml`:
//! the EBNF below is the normative grammar, and the spot-check tests at the
//! bottom fail when a word is added to a table without updating it (and vice
//! versa). `docs/IFML.md` links here instead of restating the grammar.
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
//!
//! # The normative grammar
//!
//! This EBNF is the grammar authority for `.ifml`. When any prose and these
//! productions disagree, the parser in this file wins; change both in the
//! same commit. The spot-check tests at the bottom of this module fail when
//! a quoted word is missing from the word tables below (or vice versa), so
//! adding a word without documenting it is a test failure.
//!
//! There are **no reserved words**: every word lexes as an identifier and is
//! special only in the grammar position named here (that is why `if:`,
//! `on:`, and `use:` remain valid property names). `true`/`false`/`if` are
//! literals only in their positions.
//!
//! ## File and declarations
//!
//! ```text
//! file             := prologue_item* declaration*
//! prologue_item    := import_decl | domain_decl
//! declaration      := import_decl | view_decl | action_decl | module_decl
//!                   | actor_decl | command_decl
//! import_decl      := "import" string ";"?
//! domain_decl      := "domain" string "{" "schema" string ";" "}"
//!
//! view_decl        := "view" string body
//! container_decl   := "container" string body
//! body             := "{" params_block? label_decl? property*
//!                     ( container_decl | component_decl | event_handler
//!                     | module_use | condition )* "}"
//! params_block     := "params" parameter_block ";"
//! parameter_block  := "{" parameter ("," parameter)* "}"
//! parameter        := name ":" type_ref ("=" param_default)?
//! param_default    := string | number | boolean
//! label_decl       := "label" string ";"
//! property         := name ":" value_expression ";"
//! condition        := "if" expression ";"
//!
//! component_decl   := "component" string component_body
//! component_body   := "{" property*
//!                     ( column_decl | field_decl | chart_decl | condition )*
//!                     ( event_handler | condition )* "}"
//! column_decl      := "column" string "->" column_kind ";"
//! column_kind      := "lookup" property_ref "via" name
//!                   | "field" property_ref
//!                   | "expr" expression
//! property_ref     := name "." name
//! field_decl       := "field" name "->" "input" input_type
//!                     ( "{" property* "}" | ";" )
//! chart_decl       := "chart" chart_kind ( "{" property* "}" | ";" )
//!
//! event_handler    := "on" event_type event_param? event_requires?
//!                     event_condition? "->" event_action ";"
//! event_type       := "select" | "submit" | "click" | "change" | "load"
//!                   | "save" | "cancel" | "delete" | "confirm" | "back"
//!                   | name
//! event_param      := "(" name ("," name)* ")"
//! event_requires   := "requires" ":" "[" name ("," name)* "]"
//! event_condition  := "if" expression
//! event_action     := "navigate" "(" string ("," parameter_binding)? ")"
//!                   | "refresh" "(" string ("," parameter_binding)? ")"
//!                   | "action" "(" string ("," action_body)? ")"
//!                   | "stay"
//! parameter_binding := "{" (binding_pair ("," binding_pair)*)? "}"
//! binding_pair     := name ":" expression
//! action_body      := "{" property* event_handler* "}"
//!
//! action_decl      := "action" string "{" property* event_handler* "}"
//! actor_decl       := "actor" string "{" property* "}"
//! module_decl      := "module" string "{" "input" parameter_block
//!                     "output" parameter_block property*
//!                     ( container_decl | component_decl | event_handler
//!                     | module_use )* "}"
//! module_use       := "use" string ("as" name)? ( "{" property* "}" ";"? | ";" )
//!
//! command_decl     := "command" string "{" command_member* "}"
//! command_member   := property | argument_decl | option_decl | prompt_decl
//!                   | event_handler | condition | module_use | command_decl
//! argument_decl    := "argument" name ":" type_ref ";"
//! option_decl      := "option" name ":" type_ref ("=" param_default)? ";"
//! prompt_decl      := "prompt" name ":" prompt_kind ("=" param_default)? ";"
//!
//! input_type       := "text" | "textarea" | "password" | "email" | "number"
//!                   | "date" | "time" | "datetime" | "dropdown" | "radio"
//!                   | "checkbox" | "toggle" | "file" | "hidden" | name
//! chart_kind       := "bar" | "line" | "pie" | "radar" | "metric" | "org"
//! prompt_kind      := "text" | "secret" | "number" | "confirm" | "select"
//!                   | "multiselect" | "date" | name
//! type_ref         := "Uuid" | "String" | "Int" | "Float" | "Boolean"
//!                   | "DateTime" | name
//! boolean          := "true" | "false"
//! ```
//!
//! `chart_aggregate` is a closed property-value vocabulary rather than a
//! parse position: the parser accepts any value expression after
//! `aggregate:`, and lowering rejects anything outside the set.
//!
//! ```text
//! chart_aggregate  := "sum" | "avg" | "count" | "min" | "max"
//! ```
//!
//! ## Expressions
//!
//! The C-like ladder is left-associative at every binary level. Value
//! positions (property values, array elements, object members) add arrays
//! and objects; a parenthesized value contains the *expression* surface, so
//! parentheses cannot smuggle an array/object into expression position.
//!
//! ```text
//! expression       := logical_or
//! logical_or       := logical_and ("||" logical_and)*
//! logical_and      := comparison ("&&" comparison)*
//! comparison       := addition (comparison_op addition)*
//! comparison_op    := "<=" | ">=" | "==" | "!=" | "~=" | "!~" | "<" | ">"
//! addition         := multiplication (("+" | "-") multiplication)*
//! multiplication   := unary (("*" | "/" | "%") unary)*
//! unary            := "!" unary | "-" unary | primary
//! primary          := string | number | boolean | call_expr | field_expr
//!                  | name | group_expr
//! call_expr        := name "(" (expression ("," expression)*)? ")"
//! field_expr       := name ("." name)+
//! group_expr       := "(" expression ")"
//!
//! value_expression := logical_or_value | array_literal | object_literal
//! logical_or_value := logical_and_value ("||" logical_and_value)*
//! logical_and_value := comparison_value ("&&" comparison_value)*
//! comparison_value := addition_value (comparison_op addition_value)*
//! addition_value   := multiplication_value (("+" | "-") multiplication_value)*
//! multiplication_value := unary_value (("*" | "/" | "%") unary_value)*
//! unary_value      := "!" unary_value | "-" unary_value | value_primary
//! value_primary    := string | number | boolean | call_expr | field_expr
//!                   | name | group_value
//! group_value      := "(" expression ")"
//! array_literal    := "[" (value_expression ("," value_expression)*)? "]"
//! object_literal   := "{" (object_member (";" object_member)*)? ";"? "}"
//! object_member    := name ":" object_member_value
//! object_member_value := logical_or_value | array_literal
//! ```
//!
//! ## Lexical rules
//!
//! ```text
//! name             := /[A-Za-z][A-Za-z0-9_]*/
//! number           := /[0-9]+(\.[0-9]+)?/
//! string           := '"' (escape | /[^"\\]/)* '"'
//! escape           := /\\("|\\|\/|b|f|n|r|t|u[0-9A-Fa-f]{4})/
//! ```
//!
//! Whitespace is insignificant; `//` starts a line comment and `/* */` is
//! not a comment (the formatter's comment support is separate). A `-`
//! directly before digits is part of a `param_default` number only when
//! adjacent; in expression position it is the unary operator.
//!
//! ## Ordering and recovery
//!
//! The body phases above are enforced: view/container members are
//! `params? label? property* (declaration|use|condition)*`, component members
//! are `property* (column|field|chart|condition)* (event|condition)*`, and
//! modules are `input output property* (...)`. Strict parsing rejects an
//! out-of-order member; lenient parsing recovers it into the right list and
//! reports one error. Recovery syncs at the enclosing `}` and at the next
//! member/declaration introducer, always consumes at least one token, and
//! never swallows the enclosing brace. Brackets nested deeper than
//! [`MAX_NESTING_DEPTH`] are rejected before recursive descent.

mod commands;
mod components;
mod declarations;
mod events;
mod expr;
mod modules;
mod views;

use chumsky::prelude::*;
use phero_ir::ifml::{BinOp, IfmlModel, UnaryOp};

use crate::ast::*;
use crate::lexer::{lex, Token};
use phero_parser::Span;

/// Errors produced while parsing IFML DSL sources.
#[derive(Debug, thiserror::Error)]
pub enum IfmlParseError {
    /// A syntax error at a byte position (strict entries only).
    #[error("Parse error at {position}: {message}")]
    Parse {
        /// The byte position (`start..end`) the error refers to.
        position: String,
        /// The construct-specific description.
        message: String,
    },
    /// Reading the source file failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Parse an IFML DSL string into a model.
pub fn parse_ifml(input: &str) -> Result<IfmlModel, IfmlParseError> {
    parse_model_with_path("", input).map(|(model, _)| model)
}

/// Parse an IFML DSL file into a model.
pub fn parse_ifml_file(path: &std::path::Path) -> Result<IfmlModel, IfmlParseError> {
    let input = std::fs::read_to_string(path)?;
    parse_ifml(&input)
}

pub(crate) use phero_parser::chain;

/// The token type of this surface.
pub(crate) type Tok<'src> = Token<'src>;
/// The byte-span token input.
pub(crate) type Input<'src> = phero_parser::Tokens<'src, Tok<'src>>;
/// The chumsky extra: `Rich` errors carrying byte spans.
pub(crate) type Extra<'src> = extra::Err<Rich<'src, Tok<'src>, Span>>;
/// A boxed parser: the recursion-breaking alias used by the recursive
/// expression/body/command groups.
pub(crate) type BoxedP<'src, O> = Boxed<'src, 'src, Input<'src>, O, Extra<'src>>;

/// One exact punctuation/keyword token, yielding its span.
pub(crate) fn tok<'src>(
    token: Token<'src>,
) -> impl Parser<'src, Input<'src>, Span, Extra<'src>> + Clone {
    phero_parser::kw(token)
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
    select! { Token::Str(raw) = e => (phero_strings::unescape_json_string(raw), e.span()) }
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
        .ignore_then(phero_parser::junk_until(is_top_introducer).or_not())
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
        .ignore_then(phero_parser::junk_until(stop).or_not())
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

fn classify_bracket(token: &Token<'_>) -> Option<phero_parser::nesting::Bracket> {
    use phero_parser::nesting::Bracket;
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
pub(crate) fn parse_file_recovering(
    text: &str,
) -> (Option<IfmlFile>, Vec<phero_parser::ParseError>) {
    let tokens = lex(text);
    if let Some(span) =
        phero_parser::nesting::limit_exceeded(&tokens, classify_bracket, MAX_NESTING_DEPTH)
    {
        return (
            None,
            vec![phero_parser::ParseError {
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
        .map(|error| phero_parser::ParseError {
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
) -> Result<(phero_ir::ifml::IfmlModel, crate::index::IfmlIndex), IfmlParseError> {
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
    (phero_ir::ifml::IfmlModel, crate::index::IfmlIndex),
    Vec<phero_parser::ParseError>,
) {
    let (ast, mut issues) = parse_file_recovering(text);
    let Some(file) = ast else {
        return (
            (
                phero_ir::ifml::IfmlModel::default(),
                crate::index::IfmlIndex::single_file(path, text),
            ),
            issues,
        );
    };
    let (model, index, errors) = crate::lower::lower(&file, path, text);
    issues.extend(errors.into_iter().map(|error| phero_parser::ParseError {
        message: error.message,
        span: error.span,
    }));
    ((model, index), issues)
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

#[cfg(test)]
mod grammar_doc_tests {
    use std::collections::BTreeSet;

    /// This file's source; the leading `//!` lines carry the grammar blocks.
    const PARSER_SOURCE: &str = include_str!("mod.rs");

    /// Declaration/body/action words (grammar positions, not data vocabularies).
    const STRUCTURAL_WORDS: &[&str] = &[
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

    const TYPE_REFS: &[&str] = &["Uuid", "String", "Int", "Float", "Boolean", "DateTime"];

    const BOOLEANS: &[&str] = &["true", "false"];

    /// The closed data vocabularies come from the `phero-ir` enums — the same
    /// tables the parser and lowering consume, so the EBNF tripwire cannot
    /// drift from the code.
    fn table_words() -> BTreeSet<String> {
        let mut words: BTreeSet<String> = STRUCTURAL_WORDS
            .iter()
            .map(|word| word.to_string())
            .collect();
        words.extend(TYPE_REFS.iter().map(|word| word.to_string()));
        words.extend(BOOLEANS.iter().map(|word| word.to_string()));
        for value in phero_ir::ifml::InputFieldType::ALL {
            words.insert(value.as_str().to_string());
        }
        for value in phero_ir::ifml::PromptKind::ALL {
            words.insert(value.as_str().to_string());
        }
        for value in phero_ir::ifml::EventType::ALL {
            words.insert(value.as_str().to_string());
        }
        for value in phero_ir::ifml::ChartKind::ALL {
            words.insert(value.as_str().to_string());
        }
        for value in phero_ir::ifml::ChartAggregate::ALL {
            words.insert(value.as_str().to_string());
        }
        words
    }

    fn grammar_words() -> BTreeSet<String> {
        phero_parser::grammar_doc::words_in(PARSER_SOURCE)
            .into_iter()
            .collect()
    }

    /// Every quoted EBNF terminal resolves to a word table.
    #[test]
    fn every_grammar_word_has_a_table_entry() {
        let table = table_words();
        for word in grammar_words() {
            assert!(
                table.contains(&word),
                "EBNF word `{word}` has no word-table entry"
            );
        }
    }

    /// Every table word appears in the EBNF.
    #[test]
    fn every_table_word_appears_in_the_grammar() {
        let grammar = grammar_words();
        for word in table_words() {
            assert!(
                grammar.contains(&word),
                "word-table entry `{word}` is missing from the EBNF"
            );
        }
    }
}
