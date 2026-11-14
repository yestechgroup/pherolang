use chumsky::input::ExactSizeInput;
use chumsky::prelude::*;
use phero_parser::{Span, Tokens};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    A,
    B,
}

impl std::fmt::Display for Tok {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

type Extra<'src> = extra::Err<Rich<'src, Tok, Span>>;

fn input<'src>(tokens: &'src [(Tok, Span)]) -> Tokens<'src, Tok> {
    Tokens::new(tokens)
}

fn one_token_span<'src>() -> impl Parser<'src, Tokens<'src, Tok>, Span, Extra<'src>> + Clone {
    any().map_with(|_, e| e.span())
}

fn empty_span<'src>() -> impl Parser<'src, Tokens<'src, Tok>, Span, Extra<'src>> + Clone {
    empty().map_with(|(), e| e.span())
}

/// A normal token range reports the exact byte span.
#[test]
fn a_normal_token_range_returns_the_exact_byte_span() {
    let tokens = vec![(Tok::A, (1..4).into()), (Tok::B, (7..9).into())];
    let (out, errors) = one_token_span()
        .then(any())
        .parse(input(&tokens))
        .into_output_errors();
    assert_eq!(out.map(|(span, _)| span), Some((1..4).into()));
    assert!(errors.is_empty());
}

/// An empty range maps to the start of the current token, **not** to the end
/// of input. This is the `MappedInput` quirk the custom input exists to
/// avoid.
#[test]
fn an_empty_range_returns_an_empty_span_at_the_start_offset() {
    let tokens = vec![(Tok::A, (1..4).into())];
    let (out, errors) = empty_span()
        .then(any())
        .parse(input(&tokens))
        .into_output_errors();
    assert_eq!(out.map(|(span, _)| span), Some((1..1).into()));
    assert!(errors.is_empty());
}

/// The same empty range after consuming a token points at that token's start,
/// not at the previous token's end.
#[test]
fn an_empty_range_after_a_token_points_at_the_next_offset() {
    let tokens = vec![(Tok::A, (1..4).into()), (Tok::B, (7..9).into())];
    let (out, errors) = any()
        .ignore_then(empty_span())
        .then(any())
        .parse(input(&tokens))
        .into_output_errors();
    assert_eq!(out.map(|(span, _)| span), Some((7..7).into()));
    assert!(errors.is_empty());
}

/// At EOF the empty range really is the EOF offset.
#[test]
fn an_empty_range_at_eof_returns_the_eof_offset() {
    let tokens = vec![(Tok::A, (1..4).into()), (Tok::B, (7..9).into())];
    let (out, errors) = any()
        .repeated()
        .at_least(2)
        .ignore_then(empty_span())
        .parse(input(&tokens))
        .into_output_errors();
    assert_eq!(out, Some((9..9).into()));
    assert!(errors.is_empty());
}

/// Empty input has EOF offset zero and a zero-width span there.
#[test]
fn empty_input_reports_offset_zero() {
    let tokens: Vec<(Tok, Span)> = Vec::new();
    let (out, errors) = empty_span().parse(input(&tokens)).into_output_errors();
    assert_eq!(out, Some((0..0).into()));
    assert!(errors.is_empty());
}

/// `span_from` runs from the start token's byte offset to EOF.
#[test]
fn span_from_returns_the_start_offset_to_eof() {
    let tokens = vec![(Tok::A, (1..4).into()), (Tok::B, (7..9).into())];
    let mut cache = input(&tokens);
    let cursor = 1usize;
    let span = unsafe { <Tokens<'_, Tok> as ExactSizeInput>::span_from(&mut cache, &cursor..) };
    assert_eq!(span, (7..9).into());
}

/// `span_from` at EOF is a zero-width span at the end.
#[test]
fn span_from_at_eof_is_empty_at_the_end() {
    let tokens = vec![(Tok::A, (1..4).into())];
    let mut cache = input(&tokens);
    let cursor = 1usize;
    let span = unsafe { <Tokens<'_, Tok> as ExactSizeInput>::span_from(&mut cache, &cursor..) };
    assert_eq!(span, (4..4).into());
}
