//! The lexical pass: token spans, comment spans, string segmentation, and the
//! token metadata the contextual-keyword pass needs.
//!
//! The lexical pass is the *only* place raw characters are classified. The
//! semantic pass then overlays exact compiler spans on top. Each surface's own
//! lexer is used directly — `phero_syntax::lexer::lex_with_comments_lossy` or
//! `phero_ifml::lexer::lex_with_trivia` — so there is no second grammar here,
//! only a classification of the tokens the compiler already produced.

use phero_syntax::lexer::Token;

use crate::class::{HighlightClass, HighlightModifiers, HighlightSpan};
use crate::metadata::{is_builtin_type, keyword_class, metadata_for, LexerKind};
use crate::surface::Surface;

/// One classified lexer token. Comments are deliberately absent: they are
/// spans and a protected region, but "the previous non-trivia token" must skip
/// them.
#[derive(Debug, Clone)]
pub(crate) struct LexToken<'a> {
    /// Start byte offset.
    pub start: usize,
    /// End byte offset (exclusive).
    pub end: usize,
    /// The token's classification.
    pub kind: LexTokenKind<'a>,
}

/// The coarse kind of a lexer token, used by the contextual-keyword pass.
#[derive(Debug, Clone)]
pub(crate) enum LexTokenKind<'a> {
    /// An identifier; `escaped` is the `^`-prefixed form (never a keyword).
    Ident { text: &'a str, escaped: bool },
    /// A real lexer keyword.
    Keyword(&'static str),
    /// A string literal.
    Str,
    /// A numeric literal.
    Int,
    /// An opening brace.
    OpenBrace,
    /// A closing brace.
    CloseBrace,
    /// A semicolon.
    Semi,
    /// A closing parenthesis.
    RParen,
    /// Structural punctuation.
    Punct,
    /// An operator.
    Operator,
    /// A lexical error region.
    Error,
}

impl LexToken<'_> {
    /// The token's word, when it has one.
    pub(crate) fn word(&self) -> Option<&str> {
        match &self.kind {
            LexTokenKind::Ident { text, .. } => Some(text),
            LexTokenKind::Keyword(word) => Some(word),
            LexTokenKind::Str
            | LexTokenKind::Int
            | LexTokenKind::OpenBrace
            | LexTokenKind::CloseBrace
            | LexTokenKind::Semi
            | LexTokenKind::RParen
            | LexTokenKind::Punct
            | LexTokenKind::Operator
            | LexTokenKind::Error => None,
        }
    }
}

/// The output of the lexical pass.
#[derive(Debug, Default)]
pub(crate) struct Lexed<'a> {
    /// Base-class spans (non-overlapping by construction).
    pub spans: Vec<HighlightSpan>,
    /// Comment and string-token byte ranges: semantic overlays may not cut
    /// through them (an exact match is allowed).
    pub protected: Vec<(usize, usize)>,
    /// Classified tokens for the contextual and semantic passes.
    pub tokens: Vec<LexToken<'a>>,
}

/// Lexes `source` for `surface`.
pub(crate) fn lex_surface<'a>(surface: Surface, source: &'a str) -> Lexed<'a> {
    match metadata_for(surface).lexer {
        LexerKind::MoxFamily => lex_mox_family(surface, source),
        LexerKind::Ifml => lex_ifml(surface, source),
        LexerKind::Rune => lex_rune(surface, source),
    }
}

fn lex_mox_family<'a>(surface: Surface, source: &'a str) -> Lexed<'a> {
    let (tokens, comments, _errors) = phero_syntax::lexer::lex_with_comments_lossy(source);
    let mut lexed = Lexed::default();

    for comment in comments {
        let start = comment.span.start;
        let end = comment.span.end;
        if start >= end {
            continue;
        }
        let mut modifiers = HighlightModifiers::new();
        if comment.doc_content().is_some() {
            modifiers.insert(crate::class::HighlightModifier::Documentation);
        }
        lexed.spans.push(HighlightSpan::with_modifiers(
            start,
            end,
            HighlightClass::Comment,
            modifiers,
        ));
        lexed.protected.push((start, end));
    }

    for (token, span) in tokens {
        let (start, end) = (span.start, span.end);
        if start >= end {
            continue;
        }
        let Some(kind) = syntax_kind(&token) else {
            continue;
        };
        let mut spans = classify_syntax_token(surface, source, &token, start, end);
        lexed.spans.append(&mut spans);
        if matches!(kind, LexTokenKind::Str) {
            lexed.protected.push((start, end));
        }
        lexed.tokens.push(LexToken { start, end, kind });
    }

    lexed.protected.sort_unstable();
    lexed
}

/// The coarse kind of a `phero-syntax` token, or `None` for skipped trivia.
fn syntax_kind<'src>(token: &Token<'src>) -> Option<LexTokenKind<'src>> {
    Some(match token {
        Token::Ident(text) => LexTokenKind::Ident {
            text,
            escaped: false,
        },
        Token::IdentEscaped(text) => LexTokenKind::Ident {
            text,
            escaped: true,
        },
        Token::Str(_) => LexTokenKind::Str,
        Token::Int(_) => LexTokenKind::Int,
        Token::LBrace => LexTokenKind::OpenBrace,
        Token::RBrace => LexTokenKind::CloseBrace,
        Token::RParen => LexTokenKind::RParen,
        Token::Dot | Token::Comma | Token::LParen | Token::LBracket | Token::RBracket => {
            LexTokenKind::Punct
        }
        Token::Eq | Token::FatArrow | Token::Star | Token::Arrow => LexTokenKind::Operator,
        Token::Other(char) => match char {
            ';' => LexTokenKind::Semi,
            '+' | '-' | '/' | '%' | '<' | '>' | '!' | '&' | '|' | '^' | '~' | '?' => {
                LexTokenKind::Operator
            }
            _ => LexTokenKind::Punct,
        },
        Token::Error => LexTokenKind::Error,
        Token::Whitespace | Token::LineComment(_) | Token::BlockComment(_) => return None,
        Token::Package
        | Token::Annotation
        | Token::As
        | Token::Class
        | Token::Extends
        | Token::Interface
        | Token::Enum
        | Token::Type
        | Token::Wraps
        | Token::Opaque
        | Token::Contains
        | Token::Refers
        | Token::Container
        | Token::Opposite
        | Token::Op
        | Token::Derived
        | Token::True
        | Token::False
        | Token::Vocabulary
        | Token::From
        | Token::Version
        | Token::Key
        | Token::Facet
        | Token::Actors
        | Token::Actor
        | Token::Agent
        | Token::Capability
        | Token::Grant
        | Token::Permit
        | Token::Forbid
        | Token::When
        | Token::Obligation
        | Token::On
        | Token::NeverBoth
        | Token::Delegation
        | Token::Purpose
        | Token::Cedar
        | Token::Event
        | Token::Channel
        | Token::Publishes
        | Token::Subscription
        | Token::Import => {
            LexTokenKind::Keyword(token.keyword().expect("keyword token has a spelling"))
        }
    })
}

/// Classifies one `phero-syntax` token into base spans.
fn classify_syntax_token(
    surface: Surface,
    source: &str,
    token: &Token<'_>,
    start: usize,
    end: usize,
) -> Vec<HighlightSpan> {
    match token {
        Token::Ident(text) => vec![ident_span(surface, text, start, end)],
        Token::IdentEscaped(text) => vec![escaped_ident_span(text, start, end)],
        Token::Str(_) => string_spans(source, start, end),
        Token::Int(_) => vec![HighlightSpan::new(start, end, HighlightClass::Number)],
        Token::LBrace
        | Token::RBrace
        | Token::RParen
        | Token::Dot
        | Token::Comma
        | Token::LParen
        | Token::LBracket
        | Token::RBracket => {
            vec![HighlightSpan::new(start, end, HighlightClass::Punctuation)]
        }
        Token::Eq | Token::FatArrow | Token::Star | Token::Arrow => {
            vec![HighlightSpan::new(start, end, HighlightClass::Operator)]
        }
        Token::Other(char) => vec![HighlightSpan::new(start, end, other_class(*char))],
        Token::Error => vec![HighlightSpan::new(start, end, HighlightClass::Error)],
        Token::Whitespace | Token::LineComment(_) | Token::BlockComment(_) => Vec::new(),
        _ => {
            let word = token.keyword().expect("keyword token has a spelling");
            let class =
                keyword_class(surface, word).unwrap_or_else(|| ident_class(surface, word).0);
            vec![HighlightSpan::new(start, end, class)]
        }
    }
}

fn lex_ifml<'a>(surface: Surface, source: &'a str) -> Lexed<'a> {
    use phero_ifml::lexer::Token as IfmlToken;

    let tokens = phero_ifml::lexer::lex_with_trivia(source);
    let mut lexed = Lexed::default();

    for (token, span) in tokens {
        let (start, end) = (span.start, span.end);
        if start >= end {
            continue;
        }
        match token {
            IfmlToken::LineComment(text) => {
                let mut modifiers = HighlightModifiers::new();
                if text.starts_with("///") {
                    modifiers.insert(crate::class::HighlightModifier::Documentation);
                }
                lexed.spans.push(HighlightSpan::with_modifiers(
                    start,
                    end,
                    HighlightClass::Comment,
                    modifiers,
                ));
                lexed.protected.push((start, end));
            }
            IfmlToken::Ident(text) => {
                lexed.spans.push(ident_span(surface, text, start, end));
                lexed.tokens.push(LexToken {
                    start,
                    end,
                    kind: LexTokenKind::Ident {
                        text,
                        escaped: false,
                    },
                });
            }
            IfmlToken::Number(_) => {
                lexed
                    .spans
                    .push(HighlightSpan::new(start, end, HighlightClass::Number));
                lexed.tokens.push(LexToken {
                    start,
                    end,
                    kind: LexTokenKind::Int,
                });
            }
            IfmlToken::Str(_) => {
                lexed.spans.append(&mut string_spans(source, start, end));
                lexed.protected.push((start, end));
                lexed.tokens.push(LexToken {
                    start,
                    end,
                    kind: LexTokenKind::Str,
                });
            }
            IfmlToken::LBrace => lexed.push_punct_token(start, end, LexTokenKind::OpenBrace),
            IfmlToken::RBrace => lexed.push_punct_token(start, end, LexTokenKind::CloseBrace),
            IfmlToken::Semi => lexed.push_punct_token(start, end, LexTokenKind::Semi),
            IfmlToken::RParen => lexed.push_punct_token(start, end, LexTokenKind::RParen),
            IfmlToken::Colon
            | IfmlToken::Comma
            | IfmlToken::Dot
            | IfmlToken::LBracket
            | IfmlToken::RBracket
            | IfmlToken::LParen => lexed.push_punct_token(start, end, LexTokenKind::Punct),
            IfmlToken::Arrow
            | IfmlToken::OrOr
            | IfmlToken::AndAnd
            | IfmlToken::EqEq
            | IfmlToken::NotEq
            | IfmlToken::RegexMatch
            | IfmlToken::NotRegex
            | IfmlToken::Le
            | IfmlToken::Ge
            | IfmlToken::Lt
            | IfmlToken::Gt
            | IfmlToken::Plus
            | IfmlToken::Minus
            | IfmlToken::Star
            | IfmlToken::Slash
            | IfmlToken::Percent
            | IfmlToken::Bang
            | IfmlToken::Eq => {
                lexed
                    .spans
                    .push(HighlightSpan::new(start, end, HighlightClass::Operator));
                lexed.tokens.push(LexToken {
                    start,
                    end,
                    kind: LexTokenKind::Operator,
                });
            }
            IfmlToken::Error => {
                lexed
                    .spans
                    .push(HighlightSpan::new(start, end, HighlightClass::Error));
                lexed.tokens.push(LexToken {
                    start,
                    end,
                    kind: LexTokenKind::Error,
                });
            }
        }
    }

    lexed.protected.sort_unstable();
    lexed
}

/// Lexes a `.rosetta` source with the Rune lexer (ADR 0013).
///
/// Every word is an identifier; the hyphenated words are the only keyword
/// tokens. A string written as a definition (`<"…">`) is documentation, not a
/// literal, so it takes the documentation comment class.
fn lex_rune<'a>(surface: Surface, source: &'a str) -> Lexed<'a> {
    use phero_syntax::rune::lexer::RuneToken;

    let tokens = phero_syntax::rune::lexer::lex_with_comments(source);
    let code: Vec<usize> = (0..tokens.len())
        .filter(|&index| !tokens[index].0.is_comment())
        .collect();
    let mut lexed = Lexed::default();

    let mut position_of = vec![usize::MAX; tokens.len()];
    for (position, &index) in code.iter().enumerate() {
        position_of[index] = position;
    }
    // The nearest code token before (`-1`) or after (`1`) token `index`.
    let neighbour = |index: usize, offset: isize| -> Option<RuneToken<'a>> {
        let target = position_of[index].checked_add_signed(offset)?;
        code.get(target).map(|&at| tokens[at].0)
    };

    for (index, (token, span)) in tokens.iter().enumerate() {
        let (start, end) = (span.start, span.end);
        if start >= end {
            continue;
        }
        match *token {
            RuneToken::LineComment(_) | RuneToken::BlockComment(_) => {
                lexed
                    .spans
                    .push(HighlightSpan::new(start, end, HighlightClass::Comment));
                lexed.protected.push((start, end));
            }
            RuneToken::Ident((text, escaped)) => {
                lexed.spans.push(if escaped {
                    escaped_ident_span(text, start, end)
                } else {
                    ident_span(surface, text, start, end)
                });
                lexed.tokens.push(LexToken {
                    start,
                    end,
                    kind: LexTokenKind::Ident { text, escaped },
                });
            }
            RuneToken::Word(word) => {
                let keyword = phero_syntax::words::RUNE_HYPHEN_KEYWORDS
                    .iter()
                    .copied()
                    .find(|candidate| *candidate == word)
                    .unwrap_or("");
                lexed
                    .spans
                    .push(HighlightSpan::new(start, end, HighlightClass::Keyword));
                lexed.tokens.push(LexToken {
                    start,
                    end,
                    kind: LexTokenKind::Keyword(keyword),
                });
            }
            RuneToken::Int(_) | RuneToken::Decimal(_) => {
                lexed
                    .spans
                    .push(HighlightSpan::new(start, end, HighlightClass::Number));
                lexed.tokens.push(LexToken {
                    start,
                    end,
                    kind: LexTokenKind::Int,
                });
            }
            RuneToken::Str(_) => {
                let definition = matches!(neighbour(index, -1), Some(RuneToken::Punct("<")))
                    && matches!(neighbour(index, 1), Some(RuneToken::Punct(">")));
                if definition {
                    lexed.spans.push(HighlightSpan::with_modifiers(
                        start,
                        end,
                        HighlightClass::Comment,
                        HighlightModifiers::new()
                            .with(crate::class::HighlightModifier::Documentation),
                    ));
                } else {
                    lexed.spans.append(&mut string_spans(source, start, end));
                }
                lexed.protected.push((start, end));
                lexed.tokens.push(LexToken {
                    start,
                    end,
                    kind: LexTokenKind::Str,
                });
            }
            RuneToken::Punct(text) => {
                let kind = match text {
                    "{" => LexTokenKind::OpenBrace,
                    "}" => LexTokenKind::CloseBrace,
                    ";" => LexTokenKind::Semi,
                    ")" => LexTokenKind::RParen,
                    "->" | "->>" | "*" | "+" | "-" | "/" | "=" | "<=" | "<>" | "<" | ">=" | ">" => {
                        LexTokenKind::Operator
                    }
                    _ => LexTokenKind::Punct,
                };
                if matches!(kind, LexTokenKind::Operator) {
                    lexed
                        .spans
                        .push(HighlightSpan::new(start, end, HighlightClass::Operator));
                    lexed.tokens.push(LexToken { start, end, kind });
                } else {
                    lexed.push_punct_token(start, end, kind);
                }
            }
            RuneToken::Error => {
                lexed
                    .spans
                    .push(HighlightSpan::new(start, end, HighlightClass::Error));
                lexed.tokens.push(LexToken {
                    start,
                    end,
                    kind: LexTokenKind::Error,
                });
            }
        }
    }

    lexed.protected.sort_unstable();
    lexed
}

impl Lexed<'_> {
    fn push_punct_token(&mut self, start: usize, end: usize, kind: LexTokenKind<'static>) {
        self.spans
            .push(HighlightSpan::new(start, end, HighlightClass::Punctuation));
        self.tokens.push(LexToken { start, end, kind });
    }
}

/// The class of an ordinary identifier: literal, builtin type, or a
/// capitalization heuristic.
fn ident_class(surface: Surface, text: &str) -> (HighlightClass, HighlightModifiers) {
    let metadata = metadata_for(surface);
    if metadata.literals.contains(&text) {
        return (HighlightClass::Constant, HighlightModifiers::new());
    }
    if is_builtin_type(surface, text) {
        return (
            HighlightClass::Type,
            HighlightModifiers::new().with(crate::class::HighlightModifier::DefaultLibrary),
        );
    }
    (case_class(text), HighlightModifiers::new())
}

/// The capitalization heuristic: uppercase-initial names are types, everything
/// else is a variable. Escaped identifiers never take the keyword/literal
/// tables (the `^` is an explicit "this is a name" marker).
fn case_class(text: &str) -> HighlightClass {
    match text.chars().next() {
        Some(first) if first.is_uppercase() => HighlightClass::Type,
        _ => HighlightClass::Variable,
    }
}

fn ident_span(surface: Surface, text: &str, start: usize, end: usize) -> HighlightSpan {
    let (class, modifiers) = match metadata_for(surface).lexer {
        LexerKind::MoxFamily | LexerKind::Rune => ident_class(surface, text),
        LexerKind::Ifml => match keyword_class(surface, text) {
            Some(class) => {
                let modifiers = if is_builtin_type(surface, text) {
                    HighlightModifiers::new().with(crate::class::HighlightModifier::DefaultLibrary)
                } else {
                    HighlightModifiers::new()
                };
                (class, modifiers)
            }
            None => ident_class(surface, text),
        },
    };
    HighlightSpan::with_modifiers(start, end, class, modifiers)
}

fn escaped_ident_span(text: &str, start: usize, end: usize) -> HighlightSpan {
    HighlightSpan::new(start, end, case_class(text))
}

/// The class of a raw character outside the dedicated punctuation tokens.
fn other_class(char: char) -> HighlightClass {
    if "+-/%<>!=&|^~?*".contains(char) {
        HighlightClass::Operator
    } else {
        HighlightClass::Punctuation
    }
}

/// Segments a string token into `String` pieces and `Escape` sub-spans.
///
/// `start..end` is the token's full span (quotes included). The escape scan
/// only looks for `\\` inside the quoted region; `\uXXXX` is one six-byte
/// escape, everything else is a two-byte `\\.` escape.
pub(crate) fn string_spans(source: &str, start: usize, end: usize) -> Vec<HighlightSpan> {
    if start >= end || end > source.len() {
        return Vec::new();
    }
    let bytes = source.as_bytes();
    if end - start < 2 || bytes[start] != b'"' || bytes[end - 1] != b'"' {
        return vec![HighlightSpan::new(start, end, HighlightClass::String)];
    }

    let inner_end = end - 1;
    let mut out = Vec::new();
    let mut cursor = start;
    let mut index = start + 1;
    while index < inner_end {
        if bytes[index] == b'\\' {
            if cursor < index {
                out.push(HighlightSpan::new(cursor, index, HighlightClass::String));
            }
            let available = inner_end - index;
            let length = if available >= 6
                && bytes.get(index + 1) == Some(&b'u')
                && bytes[index + 2..index + 6]
                    .iter()
                    .all(u8::is_ascii_hexdigit)
            {
                6
            } else {
                2.min(available)
            };
            out.push(HighlightSpan::new(
                index,
                index + length,
                HighlightClass::Escape,
            ));
            index += length;
            cursor = index;
        } else {
            index += 1;
        }
    }
    if cursor < end {
        out.push(HighlightSpan::new(cursor, end, HighlightClass::String));
    }
    if out.is_empty() {
        out.push(HighlightSpan::new(start, end, HighlightClass::String));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classes(source: &str, surface: Surface) -> Vec<(usize, usize, HighlightClass)> {
        lex_surface(surface, source)
            .spans
            .into_iter()
            .map(|span| (span.start, span.end, span.class))
            .collect()
    }

    #[test]
    fn whitespace_is_skipped_and_gaps_are_implicit() {
        let spans = classes("class A", Surface::Mox);
        assert_eq!(
            spans,
            vec![
                (0, 5, HighlightClass::Keyword),
                (6, 7, HighlightClass::Type)
            ]
        );
    }

    #[test]
    fn container_is_a_keyword_in_mox_and_a_variable_in_deploy() {
        assert_eq!(
            classes("container", Surface::Mox),
            vec![(0, 9, HighlightClass::Keyword)]
        );
        assert_eq!(
            classes("container", Surface::Deploy),
            vec![(0, 9, HighlightClass::Variable)]
        );
    }

    #[test]
    fn strings_segment_escapes() {
        let source = r#"a "x\ny" b"#;
        let spans = lex_surface(Surface::Mox, source).spans;
        let string: Vec<_> = spans
            .iter()
            .filter(|span| span.class == HighlightClass::String)
            .collect();
        let escape: Vec<_> = spans
            .iter()
            .filter(|span| span.class == HighlightClass::Escape)
            .collect();
        assert_eq!(
            escape,
            vec![&HighlightSpan::new(4, 6, HighlightClass::Escape)]
        );
        assert_eq!(string.len(), 2);
        assert_eq!(string[0].start, 2);
        assert_eq!(string[1].end, 8);
    }

    #[test]
    fn unicode_escape_is_one_escape_span() {
        let source = r#""a\u00e9b""#;
        let spans = lex_surface(Surface::Ifml, source).spans;
        let escape: Vec<_> = spans
            .into_iter()
            .filter(|span| span.class == HighlightClass::Escape)
            .collect();
        assert_eq!(
            escape,
            vec![HighlightSpan::new(2, 8, HighlightClass::Escape)]
        );
    }

    #[test]
    fn doc_comments_get_the_documentation_modifier() {
        let lexed = lex_surface(Surface::Mox, "/// docs\n// plain\nclass A");
        let comments: Vec<_> = lexed
            .spans
            .iter()
            .filter(|span| span.class == HighlightClass::Comment)
            .collect();
        assert_eq!(comments.len(), 2);
        assert!(comments[0]
            .modifiers
            .contains(crate::class::HighlightModifier::Documentation));
        assert!(!comments[1]
            .modifiers
            .contains(crate::class::HighlightModifier::Documentation));
        assert_eq!(lexed.protected.len(), 2);
    }

    #[test]
    fn identifiers_follow_literal_builtin_and_case_rules() {
        assert_eq!(
            classes("true String Ticket ticket", Surface::Mox),
            vec![
                (0, 4, HighlightClass::Constant),
                (5, 11, HighlightClass::Type),
                (12, 18, HighlightClass::Type),
                (19, 25, HighlightClass::Variable),
            ]
        );
        let spans = lex_surface(Surface::Mox, "String").spans;
        assert!(spans[0]
            .modifiers
            .contains(crate::class::HighlightModifier::DefaultLibrary));
    }

    #[test]
    fn escaped_identifiers_are_never_keywords() {
        assert_eq!(
            classes("^class", Surface::Mox),
            vec![(0, 6, HighlightClass::Variable)]
        );
        assert_eq!(
            classes("^true", Surface::Mox),
            vec![(0, 5, HighlightClass::Variable)]
        );
        assert_eq!(
            classes("^Ticket", Surface::Mox),
            vec![(0, 7, HighlightClass::Type)]
        );
    }

    #[test]
    fn ifml_structural_words_are_keywords() {
        assert_eq!(
            classes("view", Surface::Ifml),
            vec![(0, 4, HighlightClass::Keyword)]
        );
        assert_eq!(
            classes("Int", Surface::Ifml),
            vec![(0, 3, HighlightClass::Type)]
        );
    }

    #[test]
    fn comments_are_protected_and_tokens_exclude_them() {
        let lexed = lex_surface(Surface::Mox, "class /* c */ A");
        assert_eq!(lexed.tokens.len(), 2);
        assert_eq!(lexed.protected, vec![(6, 13)]);
    }
}
