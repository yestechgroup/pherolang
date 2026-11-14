use chumsky::prelude::*;
use phero_parser::{chain, junk_until, kw, list_separated, Span, Tokens};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    A,
    B,
    C,
    Comma,
    Plus,
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

fn kw_a<'src>() -> impl Parser<'src, Tokens<'src, Tok>, Span, Extra<'src>> + Clone {
    kw(Tok::A)
}

fn kw_b<'src>() -> impl Parser<'src, Tokens<'src, Tok>, Span, Extra<'src>> + Clone {
    kw(Tok::B)
}

fn kw_comma<'src>() -> impl Parser<'src, Tokens<'src, Tok>, Span, Extra<'src>> + Clone {
    kw(Tok::Comma)
}

fn kw_plus<'src>() -> impl Parser<'src, Tokens<'src, Tok>, Span, Extra<'src>> + Clone {
    kw(Tok::Plus)
}

fn kw_c<'src>() -> impl Parser<'src, Tokens<'src, Tok>, Span, Extra<'src>> + Clone {
    kw(Tok::C)
}

fn junk_before_c<'src>() -> impl Parser<'src, Tokens<'src, Tok>, Span, Extra<'src>> + Clone {
    junk_until(|t: &Tok| *t == Tok::C)
}

/// `kw` matches the exact token and yields its byte span.
#[test]
fn kw_matches_and_captures_the_span() {
    let tokens = vec![(Tok::A, (0..1).into()), (Tok::B, (2..4).into())];
    let (out, errors) = kw_a()
        .then(kw_b())
        .parse(input(&tokens))
        .into_output_errors();
    assert_eq!(out, Some(((0..1).into(), (2..4).into())));
    assert!(errors.is_empty());
}

/// `kw` fails without consuming when the token differs.
#[test]
fn kw_rejects_a_different_token() {
    let tokens = vec![(Tok::A, (0..1).into())];
    let (out, errors) = kw_b().parse(input(&tokens)).into_output_errors();
    assert_eq!(out, None);
    assert!(!errors.is_empty());
}

/// `junk_until` consumes everything up to the stop predicate and reports the
/// skipped region's span.
#[test]
fn junk_until_consumes_up_to_the_stop_token() {
    let tokens = vec![
        (Tok::A, (0..1).into()),
        (Tok::B, (2..3).into()),
        (Tok::C, (4..5).into()),
    ];
    let (out, errors) = junk_before_c()
        .then(kw_c())
        .parse(input(&tokens))
        .into_output_errors();
    assert_eq!(out, Some(((0..3).into(), (4..5).into())));
    assert!(errors.is_empty());
}

/// `junk_until` never succeeds on a stop token: that is what guarantees the
/// enclosing recovery path makes progress (the enclosing body reports the
/// error instead).
#[test]
fn junk_until_fails_at_a_stop_token() {
    let tokens = vec![(Tok::C, (0..1).into())];
    let (out, errors) = junk_before_c().parse(input(&tokens)).into_output_errors();
    assert_eq!(out, None);
    assert!(!errors.is_empty());
}

/// `list_separated` collects items in source order.
#[test]
fn list_separated_collects_in_order() {
    let tokens = vec![
        (Tok::A, (0..1).into()),
        (Tok::Comma, (1..2).into()),
        (Tok::A, (3..4).into()),
    ];
    let (out, errors) = list_separated(kw_a().to(7u8), kw_comma().to(()))
        .parse(input(&tokens))
        .into_output_errors();
    assert_eq!(out, Some(vec![7, 7]));
    assert!(errors.is_empty());
}

/// `chain` folds left-associatively.
#[test]
fn chain_folds_left() {
    let tokens = vec![
        (Tok::A, (0..1).into()),
        (Tok::Plus, (1..2).into()),
        (Tok::A, (2..3).into()),
        (Tok::Plus, (3..4).into()),
        (Tok::A, (4..5).into()),
    ];
    let (out, errors) = chain(kw_a().to(1i32), kw_plus().to(()), |lhs, (), rhs| lhs + rhs)
        .parse(input(&tokens))
        .into_output_errors();
    assert_eq!(out, Some(3));
    assert!(errors.is_empty());
}
