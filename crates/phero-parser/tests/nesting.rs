use phero_parser::nesting::{limit_exceeded, max_depth, Bracket};
use phero_parser::Span;

#[test]
fn max_depth_counts_each_bracket_kind_and_ignores_unmatched_closers() {
    let tokens = vec![
        (Bracket::Open, (0..1).into()),
        (Bracket::Open, (1..2).into()),
        (Bracket::Close, (2..3).into()),
        (Bracket::Open, (3..4).into()),
        (Bracket::Close, (4..5).into()),
        (Bracket::Close, (5..6).into()),
        (Bracket::Close, (6..7).into()),
    ];
    assert_eq!(max_depth(&tokens, |b| Some(*b)), 2);
}

#[test]
fn max_depth_of_a_flat_stream_is_zero() {
    let tokens: Vec<(Bracket, Span)> = vec![(Bracket::Close, (0..1).into())];
    assert_eq!(max_depth(&tokens, |b| Some(*b)), 0);
}

#[test]
fn limit_exceeded_reports_the_crossing_opening_span() {
    let tokens = vec![
        (Bracket::Open, (0..1).into()),
        (Bracket::Open, (2..3).into()),
        (Bracket::Open, (4..5).into()),
    ];
    assert_eq!(
        limit_exceeded(&tokens, |b| Some(*b), 2),
        Some((4..5).into())
    );
    assert_eq!(limit_exceeded(&tokens, |b| Some(*b), 3), None);
}

#[test]
fn non_bracket_tokens_are_ignored() {
    #[derive(Clone, Copy)]
    enum Tok {
        Other,
        Open,
    }
    let tokens = vec![
        (Tok::Other, (0..1).into()),
        (Tok::Open, (1..2).into()),
        (Tok::Other, (2..3).into()),
    ];
    let classify = |t: &Tok| match t {
        Tok::Open => Some(Bracket::Open),
        Tok::Other => None,
    };
    assert_eq!(max_depth(&tokens, classify), 1);
    assert_eq!(limit_exceeded(&tokens, classify, 1), None);
}
