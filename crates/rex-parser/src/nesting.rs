//! Bracket-depth scanning for untrusted input.
//!
//! A recursive-descent parser can overflow the stack on adversarial nesting
//! before any diagnostic is produced. Surfaces that accept editor input scan
//! the token stream first and reject input whose `()[]{}` depth exceeds a
//! documented limit; this module is that scan. It is surface-agnostic because
//! it only asks the caller how to classify each token.

use crate::Span;

/// What a token does to the bracket depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bracket {
    /// An opening `(`, `[`, or `{`.
    Open,
    /// A closing `)`, `]`, or `}`.
    Close,
}

/// The maximum bracket depth reached by `tokens`.
///
/// A closing bracket at depth 0 is ignored (it cannot make the depth
/// negative); unmatched closers are the parser's concern.
pub fn max_depth<T, F>(tokens: &[(T, Span)], classify: F) -> usize
where
    F: Fn(&T) -> Option<Bracket>,
{
    let mut depth = 0usize;
    let mut max = 0usize;
    for (token, _) in tokens {
        match classify(token) {
            Some(Bracket::Open) => {
                depth += 1;
                max = max.max(depth);
            }
            Some(Bracket::Close) => depth = depth.saturating_sub(1),
            None => {}
        }
    }
    max
}

/// The span of the first opening bracket that pushes the depth past `limit`,
/// or `None` when the input stays within the limit.
pub fn limit_exceeded<T, F>(tokens: &[(T, Span)], classify: F, limit: usize) -> Option<Span>
where
    F: Fn(&T) -> Option<Bracket>,
{
    let mut depth = 0usize;
    for (token, span) in tokens {
        match classify(token) {
            Some(Bracket::Open) => {
                depth += 1;
                if depth > limit {
                    return Some(*span);
                }
            }
            Some(Bracket::Close) => depth = depth.saturating_sub(1),
            None => {}
        }
    }
    None
}
