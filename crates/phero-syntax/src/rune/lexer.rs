//! The Rune (`.rosetta`) logos lexer (ADR 0013).
//!
//! Rune has its own token enum, not rows in the shared `.mox`-family
//! [`crate::Token`]: its words and punctuation do not fit that table. It has
//! hyphenated keywords (`to-date-time`, `post-condition`), `->>`, decimals
//! next to `0..1` ranges, and words such as `type`, `enum` or `version` that
//! Rune uses as ordinary names while the shared lexer reserves them. The
//! tokens still feed the shared [`phero_parser::Tokens`] input.
//!
//! Lexical rules:
//!
//! * **Every word is an identifier** ([`RuneToken::Ident`]), with its `^`
//!   escape recorded. Rune keywords are matched by text in the grammar
//!   position that needs them, so CDM attributes named `condition`, `source`
//!   or `version` stay names. This is the shape the scannerless predecessor
//!   had.
//! * **The hyphenated keywords** ([`crate::words::RUNE_HYPHEN_KEYWORDS`]) are single tokens, as in
//!   Xtext, whose lexer makes them global keywords. Longest match wins, so
//!   `to-date-time` beats `to-date` beats `to`.
//! * **Numbers**:
//!   - [`RuneToken::Int`] is `[0-9]+`.
//!   - [`RuneToken::Decimal`] needs digits on both sides of the point, or a
//!     leading point (`.5`), plus an optional exponent.
//!   - `0..1` therefore lexes as `0`, `..`, `1`.
//!   - The `1.` form and signs are assembled by the parser from adjacent
//!     tokens. A sign must touch its digits, so `- 1` is not a literal.
//! * **`T+1` … `T+5`** lex as `T`, `+`, `1` and are matched by adjacency in
//!   report timing, so `T + 1` in an expression stays arithmetic.
//! * **Strings**: `"…"` or `'…'` with backslash escapes, newlines allowed.
//!   The payload is raw and the parser unescapes it.
//! * **Comments**: `//` lines and non-nesting `/* … */` blocks. [`lex`] drops
//!   them; [`lex_with_comments`] keeps them for the formatter (RN17).
//!
//! Lexing is **total**: an unrecognized region becomes
//! [`RuneToken::Error`], and the parser reports it.

use logos::Logos;
use phero_parser::Span;

fn slice<'src>(lex: &logos::Lexer<'src, RuneToken<'src>>) -> &'src str {
    lex.slice()
}

/// Scans a block comment to its first `*/` (comments do not nest).
fn block_comment<'src>(lex: &mut logos::Lexer<'src, RuneToken<'src>>) -> Option<&'src str> {
    let end = lex.remainder().find("*/")?;
    lex.bump(end + 2);
    Some(lex.slice())
}

fn ident<'src>(lex: &logos::Lexer<'src, RuneToken<'src>>) -> (&'src str, bool) {
    let text = lex.slice();
    match text.strip_prefix('^') {
        Some(name) => (name, true),
        None => (text, false),
    }
}

/// One Rune token.
// Several `#[token("…", slice)]` attributes share the `slice` callback
// argument, which clippy's attribute-duplication lint mistakes for a
// repeated attribute: each attribute names a different token.
#[allow(clippy::duplicated_attributes)]
#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
#[logos(skip r"\s+")]
pub enum RuneToken<'src> {
    /// `// …` to the end of the line.
    #[regex(r"//[^\n]*", slice, priority = 5)]
    LineComment(&'src str),
    /// `/* … */`, non-nesting. An unterminated block is an
    /// [`RuneToken::Error`].
    #[token("/*", block_comment)]
    BlockComment(&'src str),

    /// An identifier (`^?[A-Za-z_][A-Za-z0-9_]*`): the name without its
    /// caret, and whether it was `^`-escaped.
    #[regex(r"\^?[A-Za-z_][A-Za-z0-9_]*", ident)]
    Ident((&'src str, bool)),

    /// One of [`crate::words::RUNE_HYPHEN_KEYWORDS`].
    #[token("to-zoned-date-time", slice)]
    #[token("to-date-time", slice)]
    #[token("to-date", slice)]
    #[token("to-string", slice)]
    #[token("to-number", slice)]
    #[token("to-int", slice)]
    #[token("to-time", slice)]
    #[token("to-enum", slice)]
    #[token("only-element", slice)]
    #[token("one-of", slice)]
    #[token("with-meta", slice)]
    #[token("as-key", slice)]
    #[token("post-condition", slice)]
    #[token("real-time", slice)]
    Word(&'src str),

    /// An unsigned integer, digits verbatim.
    #[regex(r"[0-9]+", slice)]
    Int(&'src str),
    /// A decimal with a fraction (`1.5`, `.5`) and an optional exponent,
    /// verbatim.
    #[regex(r"[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?", slice)]
    #[regex(r"\.[0-9]+([eE][+-]?[0-9]+)?", slice)]
    Decimal(&'src str),

    /// A string literal with its quotes; the payload is raw.
    #[regex(r#""([^"\\]|\\(.|\n))*""#, slice)]
    #[regex(r#"'([^'\\]|\\(.|\n))*'"#, slice)]
    Str(&'src str),

    /// Punctuation and operators, by spelling.
    #[token("->>", slice)]
    #[token("->", slice)]
    #[token("...", slice)]
    #[token("..", slice)]
    #[token(".", slice)]
    #[token("(", slice)]
    #[token(")", slice)]
    #[token("[", slice)]
    #[token("]", slice)]
    #[token("{", slice)]
    #[token("}", slice)]
    #[token(",", slice)]
    #[token(":", slice)]
    #[token(";", slice)]
    #[token("*", slice)]
    #[token("+", slice)]
    #[token("-", slice)]
    #[token("/", slice)]
    #[token("=", slice)]
    #[token("<=", slice)]
    #[token("<>", slice)]
    #[token("<", slice)]
    #[token(">=", slice)]
    #[token(">", slice)]
    Punct(&'src str),

    /// An unrecognized region.
    Error,
}

/// Lex `source` into tokens and byte spans, dropping comments. Never fails:
/// invalid regions become [`RuneToken::Error`].
pub fn lex(source: &str) -> Vec<(RuneToken<'_>, Span)> {
    lex_with_comments(source)
        .into_iter()
        .filter(|(token, _)| !token.is_comment())
        .collect()
}

/// Lex `source` into tokens and byte spans, comments included.
pub fn lex_with_comments(source: &str) -> Vec<(RuneToken<'_>, Span)> {
    let mut tokens = Vec::new();
    let mut lexer = RuneToken::lexer(source);
    while let Some(result) = lexer.next() {
        let span: Span = lexer.span().into();
        tokens.push((result.unwrap_or(RuneToken::Error), span));
    }
    tokens
}

impl RuneToken<'_> {
    /// Whether this is a comment token.
    pub fn is_comment(&self) -> bool {
        matches!(self, RuneToken::LineComment(_) | RuneToken::BlockComment(_))
    }
}

impl std::fmt::Display for RuneToken<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuneToken::Ident((text, true)) => write!(f, "'^{text}'"),
            RuneToken::Ident((text, false))
            | RuneToken::Word(text)
            | RuneToken::Int(text)
            | RuneToken::Decimal(text)
            | RuneToken::Punct(text) => write!(f, "'{text}'"),
            RuneToken::Str(_) => write!(f, "a string literal"),
            RuneToken::LineComment(_) | RuneToken::BlockComment(_) => write!(f, "a comment"),
            RuneToken::Error => write!(f, "an invalid character"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<String> {
        lex(source)
            .into_iter()
            .map(|(token, _)| token.to_string())
            .collect()
    }

    #[test]
    fn ranges_are_not_decimals() {
        assert_eq!(kinds("(0..1)"), ["'('", "'0'", "'..'", "'1'", "')'"]);
        assert_eq!(kinds("(1..*)"), ["'('", "'1'", "'..'", "'*'", "')'"]);
    }

    #[test]
    fn decimals_keep_their_text() {
        assert_eq!(lex("1.5")[0].0, RuneToken::Decimal("1.5"));
        assert_eq!(lex(".5")[0].0, RuneToken::Decimal(".5"));
        assert_eq!(lex("1.5e-10")[0].0, RuneToken::Decimal("1.5e-10"));
        assert_eq!(lex("0.01")[0].0, RuneToken::Decimal("0.01"));
        // `1.` is an int then a point: the parser joins adjacent tokens.
        assert_eq!(kinds("1."), ["'1'", "'.'"]);
        // `2.0->>x`: the arrow is never swallowed.
        assert_eq!(kinds("2.0->>x"), ["'2.0'", "'->>'", "'x'"]);
    }

    #[test]
    fn hyphen_words_take_the_longest_match() {
        assert_eq!(lex("to-date-time")[0].0, RuneToken::Word("to-date-time"));
        assert_eq!(lex("to-date")[0].0, RuneToken::Word("to-date"));
        assert_eq!(
            lex("to-zoned-date-time")[0].0,
            RuneToken::Word("to-zoned-date-time")
        );
        assert_eq!(
            lex("post-condition")[0].0,
            RuneToken::Word("post-condition")
        );
        assert_eq!(kinds("to - date"), ["'to'", "'-'", "'date'"]);
    }

    #[test]
    fn every_hyphen_keyword_is_one_word_token() {
        for word in crate::words::RUNE_HYPHEN_KEYWORDS {
            assert_eq!(
                lex(word),
                [(RuneToken::Word(word), Span::from(0..word.len()))],
                "{word}"
            );
        }
    }

    #[test]
    fn identifiers_record_their_escape() {
        assert_eq!(lex("^type")[0].0, RuneToken::Ident(("type", true)));
        assert_eq!(lex("type")[0].0, RuneToken::Ident(("type", false)));
        assert_eq!(lex("_x1")[0].0, RuneToken::Ident(("_x1", false)));
    }

    #[test]
    fn timing_words_are_separate_tokens() {
        assert_eq!(kinds("T+1"), ["'T'", "'+'", "'1'"]);
    }

    #[test]
    fn strings_and_doc_strings() {
        assert_eq!(lex(r#""a\"b""#)[0].0, RuneToken::Str(r#""a\"b""#));
        assert_eq!(lex("'it''")[0].0, RuneToken::Str("'it'"));
        assert_eq!(kinds(r#"<"doc">"#), ["'<'", "a string literal", "'>'"]);
        assert_eq!(lex("\"two\nlines\"")[0].0, RuneToken::Str("\"two\nlines\""));
    }

    #[test]
    fn operators_take_the_longest_match() {
        assert_eq!(
            kinds("-> ->> ... .. . <= <> >= < > = -"),
            [
                "'->'", "'->>'", "'...'", "'..'", "'.'", "'<='", "'<>'", "'>='", "'<'", "'>'",
                "'='", "'-'"
            ]
        );
    }

    #[test]
    fn comments_are_dropped_by_lex_and_kept_by_lex_with_comments() {
        let source = "a // line\n/* block * with / stars **/ b";
        assert_eq!(kinds(source), ["'a'", "'b'"]);
        assert_eq!(lex_with_comments(source).len(), 4);
    }

    #[test]
    fn an_unterminated_block_comment_is_an_error() {
        // The parser then fails on the error token; lexing itself goes on.
        assert_eq!(
            kinds("a /* never closed")[..2],
            ["'a'", "an invalid character"]
        );
    }

    #[test]
    fn lexing_is_total() {
        assert_eq!(kinds("a # b"), ["'a'", "an invalid character", "'b'"]);
    }
}
