//! Token-agnostic parser combinators shared by every surface.
//!
//! Only helpers whose behaviour needs no surface vocabulary live here. The
//! keyword *tokens*, lexemes, and sync sets stay with their surface.

use chumsky::extra::ParserExtra;
use chumsky::input::ValueInput;
use chumsky::prelude::*;

use crate::Span;

/// A parser for one exact token, yielding its byte [`Span`].
///
/// Matches by equality; the caller supplies the token type and error extra.
pub fn kw<'src, T, I, E>(token: T) -> impl Parser<'src, I, Span, E> + Clone
where
    T: Clone + PartialEq + 'src,
    I: ValueInput<'src, Token = T, Span = Span>,
    E: ParserExtra<'src, I>,
{
    select! { t = e if t == token => e.span() }
}

/// Consumes a run of tokens that are **not** stops, yielding the skipped
/// region's byte [`Span`].
///
/// Recovery combinators call this only after the construct parser failed, so
/// the first token is normally a token no construct can start. The parser
/// nevertheless requires at least one non-stop token: if the current token is
/// a stop it fails rather than looping, which keeps every caller's
/// recovery path structurally terminating (the caller's `choice` then fails
/// the enclosing body and reports one error).
///
/// The caller attaches its own construct-specific error message to the
/// returned span; this helper deliberately knows no vocabulary.
pub fn junk_until<'src, T, I, E, F>(is_stop: F) -> impl Parser<'src, I, Span, E> + Clone
where
    T: Clone + 'src,
    I: ValueInput<'src, Token = T, Span = Span>,
    E: ParserExtra<'src, I>,
    F: Fn(&T) -> bool + Clone + 'src,
{
    let first = {
        let is_stop = is_stop.clone();
        select! { t if !is_stop(&t) => () }
    };
    let rest = select! { t if !is_stop(&t) => () };
    first
        .ignore_then(rest.repeated().ignored())
        .map_with(|(), e| e.span())
}

/// A list of `item`s separated by `separator`, collecting them in source
/// order. A trailing separator is not accepted (the surfaces that allow one
/// express it explicitly).
pub fn list_separated<'src, I, E, O, P, S>(
    item: P,
    separator: S,
) -> impl Parser<'src, I, Vec<O>, E> + Clone
where
    I: Input<'src>,
    E: ParserExtra<'src, I>,
    P: Parser<'src, I, O, E> + Clone,
    S: Parser<'src, I, (), E> + Clone,
{
    item.separated_by(separator).collect::<Vec<_>>()
}

/// A left-associative operator chain: one `operand`, then zero or more
/// `operator operand` pairs, folded left-to-right through `join`.
///
/// The operator parser yields whatever the fold needs (a `BinOp` in the
/// expression grammars); this helper stays free of surface types.
pub fn chain<'src, I, E, O, B, P, Q, F>(
    operand: P,
    operator: Q,
    join: F,
) -> impl Parser<'src, I, O, E> + Clone
where
    I: Input<'src>,
    E: ParserExtra<'src, I>,
    P: Parser<'src, I, O, E> + Clone,
    Q: Parser<'src, I, B, E> + Clone,
    F: Fn(O, B, O) -> O + Clone,
{
    operand
        .clone()
        .foldl(operator.then(operand).repeated(), move |lhs, (op, rhs)| {
            join(lhs, op, rhs)
        })
}
