//! The `.ifml` logos lexer.
//!
//! The surface has **no keyword tokens**: every word is [`Token::Ident`], and
//! words are special only in the grammar position the parser dispatches on.
//! That is what keeps `if:`/`on:` usable as property names and lets `true`,
//! `view`, `use`, ... be ordinary identifiers everywhere else.
//!
//! Lexical parity with the historical PEG grammar:
//!
//! * identifiers are exactly `[A-Za-z][A-Za-z0-9_]*` (no leading `_`);
//! * numbers are unsigned `[0-9]+(\.[0-9]+)?`; `-` is a separate token;
//! * strings keep their raw payload (quotes stripped, escapes intact) and may
//!   contain newlines; the JSON escape set is validated by the string token,
//!   and decoding happens in the parser/lexeme layer;
//! * only `//` line comments exist (`/* */` is not a comment — pinning the
//!   historical parser behaviour, which differs from the formatter);
//! * `->`, `~=`, and `!~` are single tokens; longest-match wins.
//!
//! Lexing is **total**: an unrecognized region becomes [`Token::Error`] and
//! scanning continues, so the recovering parser can report it and recover.

use logos::Logos;
use phero_parser::Span;

/// One `.ifml` token. Words are identifiers; there are no keyword variants.
#[derive(Logos, Debug, Clone, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]+")]
pub enum Token<'src> {
    /// A `// ...` line comment. Captured as a token for tooling (the syntax
    /// highlighter keeps comments); [`lex`] filters it out, so the parser's
    /// input is unchanged.
    #[regex(r"//[^\n]*", priority = 3)]
    LineComment(&'src str),

    /// An identifier: `[A-Za-z][A-Za-z0-9_]*`.
    #[regex(r"[A-Za-z][A-Za-z0-9_]*")]
    Ident(&'src str),

    /// An unsigned number: `[0-9]+(\.[0-9]+)?`.
    #[regex(r"[0-9]+(\.[0-9]+)?")]
    Number(&'src str),

    /// A quoted string; the payload is raw (quotes stripped, escapes intact).
    #[regex(r#""(\\(["\\/bfnrt]|u[0-9A-Fa-f]{4})|[^"\\])*""#, string_payload)]
    Str(&'src str),

    /// `->`
    #[token("->")]
    Arrow,
    /// `||`
    #[token("||")]
    OrOr,
    /// `&&`
    #[token("&&")]
    AndAnd,
    /// `==`
    #[token("==")]
    EqEq,
    /// `!=`
    #[token("!=")]
    NotEq,
    /// `~=`
    #[token("~=")]
    RegexMatch,
    /// `!~`
    #[token("!~")]
    NotRegex,
    /// `<=`
    #[token("<=")]
    Le,
    /// `>=`
    #[token(">=")]
    Ge,
    /// `<`
    #[token("<")]
    Lt,
    /// `>`
    #[token(">")]
    Gt,
    /// `+`
    #[token("+")]
    Plus,
    /// `-`
    #[token("-")]
    Minus,
    /// `*`
    #[token("*")]
    Star,
    /// `/`
    #[token("/")]
    Slash,
    /// `%`
    #[token("%")]
    Percent,
    /// `!`
    #[token("!")]
    Bang,
    /// `=`
    #[token("=")]
    Eq,
    /// `:`
    #[token(":")]
    Colon,
    /// `;`
    #[token(";")]
    Semi,
    /// `,`
    #[token(",")]
    Comma,
    /// `.`
    #[token(".")]
    Dot,
    /// `{`
    #[token("{")]
    LBrace,
    /// `}`
    #[token("}")]
    RBrace,
    /// `[`
    #[token("[")]
    LBracket,
    /// `]`
    #[token("]")]
    RBracket,
    /// `(`
    #[token("(")]
    LParen,
    /// `)`
    #[token(")")]
    RParen,

    /// An unrecognized region. The lexer is total: this token keeps the
    /// parser and the recovering paths running instead of aborting the scan.
    Error,
}

fn string_payload<'src>(lex: &mut logos::Lexer<'src, Token<'src>>) -> &'src str {
    let slice = lex.slice();
    &slice[1..slice.len() - 1]
}

/// Lex `source` into tokens and byte spans. Never fails: invalid regions
/// become [`Token::Error`]. Line comments are filtered out — the parser uses
/// this entry and its input is unchanged.
pub fn lex(source: &str) -> Vec<(Token<'_>, Span)> {
    let mut tokens = Vec::new();
    let mut lexer = Token::lexer(source);
    while let Some(result) = lexer.next() {
        let span: Span = lexer.span().into();
        match result.unwrap_or(Token::Error) {
            Token::LineComment(_) => {}
            token => tokens.push((token, span)),
        }
    }
    tokens
}

/// Lex `source` into tokens and byte spans **including** line comments.
/// Never fails: invalid regions become [`Token::Error`]. The highlighter
/// uses this entry; the parser uses [`lex`].
pub fn lex_with_trivia(source: &str) -> Vec<(Token<'_>, Span)> {
    let mut tokens = Vec::new();
    let mut lexer = Token::lexer(source);
    while let Some(result) = lexer.next() {
        let span: Span = lexer.span().into();
        tokens.push((result.unwrap_or(Token::Error), span));
    }
    tokens
}

impl std::fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Ident(text) => write!(f, "`{text}`"),
            Token::Number(text) => write!(f, "`{text}`"),
            Token::Str(_) => write!(f, "a string literal"),
            Token::LineComment(_) => write!(f, "a comment"),
            Token::Error => write!(f, "an invalid token"),
            other => write!(f, "`{}`", other.symbol()),
        }
    }
}

impl Token<'_> {
    /// The punctuation/keyword spelling for diagnostics (empty for literals).
    fn symbol(&self) -> &'static str {
        match self {
            Token::Arrow => "->",
            Token::OrOr => "||",
            Token::AndAnd => "&&",
            Token::EqEq => "==",
            Token::NotEq => "!=",
            Token::RegexMatch => "~=",
            Token::NotRegex => "!~",
            Token::Le => "<=",
            Token::Ge => ">=",
            Token::Lt => "<",
            Token::Gt => ">",
            Token::Plus => "+",
            Token::Minus => "-",
            Token::Star => "*",
            Token::Slash => "/",
            Token::Percent => "%",
            Token::Bang => "!",
            Token::Eq => "=",
            Token::Colon => ":",
            Token::Semi => ";",
            Token::Comma => ",",
            Token::Dot => ".",
            Token::LBrace => "{",
            Token::RBrace => "}",
            Token::LBracket => "[",
            Token::RBracket => "]",
            Token::LParen => "(",
            Token::RParen => ")",
            Token::Ident(_)
            | Token::Number(_)
            | Token::Str(_)
            | Token::LineComment(_)
            | Token::Error => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{lex, Token};

    fn kinds(source: &str) -> Vec<Token<'_>> {
        lex(source).into_iter().map(|(token, _)| token).collect()
    }

    #[test]
    fn a_leading_underscore_is_not_an_identifier() {
        let tokens = kinds("_foo");
        assert_eq!(tokens, vec![Token::Error, Token::Ident("foo")]);
    }

    #[test]
    fn minus_is_a_separate_token() {
        assert_eq!(kinds("-5"), vec![Token::Minus, Token::Number("5")]);
        assert_eq!(
            kinds("a-1"),
            vec![Token::Ident("a"), Token::Minus, Token::Number("1")]
        );
    }

    #[test]
    fn longest_match_wins_for_operator_pairs() {
        assert_eq!(kinds("->"), vec![Token::Arrow]);
        assert_eq!(kinds("~="), vec![Token::RegexMatch]);
        assert_eq!(kinds("!~"), vec![Token::NotRegex]);
        assert_eq!(kinds("<="), vec![Token::Le]);
        assert_eq!(kinds(">="), vec![Token::Ge]);
        assert_eq!(kinds("=="), vec![Token::EqEq]);
        assert_eq!(kinds("!="), vec![Token::NotEq]);
        assert_eq!(kinds("||"), vec![Token::OrOr]);
        assert_eq!(kinds("&&"), vec![Token::AndAnd]);
    }

    #[test]
    fn string_payloads_are_raw_and_may_contain_newlines() {
        assert_eq!(kinds(r#""a\u00e9""#), vec![Token::Str(r"a\u00e9")]);
        assert_eq!(kinds("\"name\""), vec![Token::Str("name")]);
        assert_eq!(kinds("\"a\nb\""), vec![Token::Str("a\nb")]);
        assert_eq!(kinds(r#""a\"b""#), vec![Token::Str(r#"a\"b"#)]);
    }

    #[test]
    fn line_comments_are_skipped_and_block_comments_are_not() {
        assert_eq!(kinds("// hidden\nview"), vec![Token::Ident("view")]);
        // `/* */` is not a comment: the slash and star are ordinary tokens.
        assert_eq!(
            kinds("/* x */"),
            vec![
                Token::Slash,
                Token::Star,
                Token::Ident("x"),
                Token::Star,
                Token::Slash
            ]
        );
    }

    #[test]
    fn words_are_identifiers() {
        for word in ["true", "false", "if", "view", "component", "on", "use"] {
            assert_eq!(kinds(word), vec![Token::Ident(word)]);
        }
    }

    #[test]
    fn numbers_are_unsigned_with_an_optional_fraction() {
        assert_eq!(kinds("12"), vec![Token::Number("12")]);
        assert_eq!(kinds("1.5"), vec![Token::Number("1.5")]);
        assert_eq!(kinds("1."), vec![Token::Number("1"), Token::Dot]);
    }

    #[test]
    fn an_unterminated_string_is_an_error_region() {
        // The failed string token consumes the run it attempted; the lexer
        // still returns a total token stream (recovery quality is the
        // parser's phase-2 concern). Preceding declarations always survive.
        let tokens = kinds("\"oops");
        assert_eq!(tokens.first(), Some(&Token::Error));
    }
}
