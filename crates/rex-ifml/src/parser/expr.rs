//! The `.ifml` expression surfaces.
//!
//! One precedence ladder produces the [`Expr`] node (bound conditions, call
//! arguments, bindings) and another produces [`ValueExpr`] (property values,
//! which additionally admit arrays and objects). The two share the primary
//! shapes and the exact PEG operator order:
//! `||` → `&&` → comparisons → `+ -` → `* / %` → unary → primary.
//!
//! Parity details that are easy to lose:
//! * a negative literal is a `unary` node in expression position
//!   (`-1` → `UnaryOp(Neg, 1)`) but a negative number in `param_default`
//!   (`= -1` → `Number(-1)`), and the minus must be adjacent there;
//! * object-literal member values may not be object literals (arrays are
//!   fine); build that in the parser, not as a post-hoc check;
//! * `f(a, b)` is a call, `a.b.c` folds left, and `a.b(c)` is not a call.

use super::*;

/// A binary expression node spanning left through right.
pub(super) fn bin(left: Expr, op: BinOp, right: Expr) -> Expr {
    Expr {
        span: (left.span.start..right.span.end).into(),
        kind: ExprKind::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
        },
    }
}

/// A binary value-expression node spanning left through right.
pub(super) fn value_bin(left: ValueExpr, op: BinOp, right: ValueExpr) -> ValueExpr {
    ValueExpr {
        span: (left.span.start..right.span.end).into(),
        kind: ValueExprKind::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
        },
    }
}

fn mul_op<'src>() -> impl Parser<'src, Input<'src>, BinOp, Extra<'src>> + Clone {
    select! {
        Token::Star => BinOp::Mul,
        Token::Slash => BinOp::Div,
        Token::Percent => BinOp::Mod,
    }
}

fn add_op<'src>() -> impl Parser<'src, Input<'src>, BinOp, Extra<'src>> + Clone {
    select! {
        Token::Plus => BinOp::Add,
        Token::Minus => BinOp::Sub,
    }
}

fn cmp_op<'src>() -> impl Parser<'src, Input<'src>, BinOp, Extra<'src>> + Clone {
    select! {
        Token::EqEq => BinOp::Eq,
        Token::NotEq => BinOp::Ne,
        Token::Lt => BinOp::Lt,
        Token::Le => BinOp::Le,
        Token::Gt => BinOp::Gt,
        Token::Ge => BinOp::Ge,
        Token::RegexMatch => BinOp::RegexMatch,
        Token::NotRegex => BinOp::NegRegex,
    }
}

fn and_op<'src>() -> impl Parser<'src, Input<'src>, BinOp, Extra<'src>> + Clone {
    tok(Token::AndAnd).to(BinOp::And)
}

fn or_op<'src>() -> impl Parser<'src, Input<'src>, BinOp, Extra<'src>> + Clone {
    tok(Token::OrOr).to(BinOp::Or)
}

/// The expression surface: string/number/boolean/call/field/identifier/group
/// primaries, prefix `!`/`-`, and the C-like operator ladder.
pub(crate) fn expr_parser<'src>() -> BoxedP<'src, Expr> {
    recursive(|expr| {
        let string = string_lit().map(|(text, span)| Expr {
            kind: ExprKind::StringLit(text),
            span,
        });
        let number = number_lit().map(|(value, span)| Expr {
            kind: ExprKind::NumLit(value),
            span,
        });
        let boolean = boolean_lit().map(|(value, span)| Expr {
            kind: ExprKind::BoolLit(value),
            span,
        });
        let call = call_expr(expr.clone()).map(|(name, name_span, args, span)| Expr {
            kind: ExprKind::Call {
                name,
                name_span,
                args,
            },
            span,
        });
        let field = expr_field();
        let identifier = ident().map(|(text, span)| Expr {
            kind: ExprKind::Ident(text),
            span,
        });
        let group = tok(Token::LParen)
            .ignore_then(expr)
            .then_ignore(tok(Token::RParen))
            .map_with(|inner, e| Expr {
                kind: ExprKind::Group(Box::new(inner)),
                span: e.span(),
            });

        let primary = choice((string, number, boolean, call, field, identifier, group));

        let unary = recursive(|unary| {
            let not = tok(Token::Bang)
                .then(unary.clone())
                .map_with(|(_, operand), e| Expr {
                    kind: ExprKind::Unary {
                        op: UnaryOp::Not,
                        operand: Box::new(operand),
                    },
                    span: e.span(),
                });
            let neg = tok(Token::Minus)
                .then(unary)
                .map_with(|(_, operand), e| Expr {
                    kind: ExprKind::Unary {
                        op: UnaryOp::Neg,
                        operand: Box::new(operand),
                    },
                    span: e.span(),
                });
            choice((not, neg, primary))
        })
        .boxed();

        let mul = chain(unary, mul_op(), bin).boxed();
        let add = chain(mul, add_op(), bin).boxed();
        let cmp = chain(add, cmp_op(), bin).boxed();
        let and = chain(cmp, and_op(), bin).boxed();
        chain(and, or_op(), bin).boxed()
    })
    .boxed()
}

/// The value-expression surface: the expression primaries plus arrays and
/// objects.
pub(crate) fn value_parser<'src>() -> BoxedP<'src, ValueExpr> {
    recursive(|value| {
        let string = string_lit().map(|(text, span)| ValueExpr {
            kind: ValueExprKind::String(text),
            span,
        });
        let number = number_lit().map(|(value, span)| ValueExpr {
            kind: ValueExprKind::Number(value),
            span,
        });
        let boolean = boolean_lit().map(|(value, span)| ValueExpr {
            kind: ValueExprKind::Bool(value),
            span,
        });
        let call = call_expr(expr_parser()).map(|(name, name_span, args, span)| ValueExpr {
            kind: ValueExprKind::Call {
                name,
                name_span,
                args,
            },
            span,
        });
        let field = value_field_expr();
        let identifier = ident().map(|(text, span)| ValueExpr {
            kind: ValueExprKind::Ident(text),
            span,
        });
        // The PEG's `group_expr` contains an *expression*, not a value:
        // parentheses cannot smuggle an array/object into expression position.
        let group = tok(Token::LParen)
            .ignore_then(expr_parser())
            .then_ignore(tok(Token::RParen))
            .map_with(|inner, e| ValueExpr {
                kind: ValueExprKind::Group(Box::new(expr_to_value(inner))),
                span: e.span(),
            });

        let primary = choice((string, number, boolean, call, field, identifier, group));

        let unary = recursive(|unary| {
            let not = tok(Token::Bang)
                .then(unary.clone())
                .map_with(|(_, operand), e| ValueExpr {
                    kind: ValueExprKind::Unary {
                        op: UnaryOp::Not,
                        operand: Box::new(operand),
                    },
                    span: e.span(),
                });
            let neg = tok(Token::Minus)
                .then(unary)
                .map_with(|(_, operand), e| ValueExpr {
                    kind: ValueExprKind::Unary {
                        op: UnaryOp::Neg,
                        operand: Box::new(operand),
                    },
                    span: e.span(),
                });
            choice((not, neg, primary))
        })
        .boxed();

        let mul = chain(unary, mul_op(), value_bin).boxed();
        let add = chain(mul, add_op(), value_bin).boxed();
        let cmp = chain(add, cmp_op(), value_bin).boxed();
        let and = chain(cmp, and_op(), value_bin).boxed();
        let logical_or = chain(and, or_op(), value_bin).boxed();

        let array = tok(Token::LBracket)
            .ignore_then(
                value
                    .clone()
                    .separated_by(tok(Token::Comma))
                    .collect::<Vec<_>>(),
            )
            .then_ignore(tok(Token::RBracket))
            .map_with(|items, e| ValueExpr {
                kind: ValueExprKind::Array(items),
                span: e.span(),
            })
            .boxed();

        // `object_member_value = logical_or | array_literal` — nested object
        // literals are not part of the grammar.
        let member_value = choice((logical_or.clone(), array.clone())).boxed();
        let member = ident()
            .then_ignore(tok(Token::Colon))
            .then(member_value)
            .map_with(|(key, value), e| ObjectMember {
                key: key.0,
                value,
                span: e.span(),
            })
            .boxed();
        let object = tok(Token::LBrace)
            .ignore_then(
                member
                    .clone()
                    .or_not()
                    .then(
                        tok(Token::Semi)
                            .ignore_then(member)
                            .repeated()
                            .collect::<Vec<_>>(),
                    )
                    .then(tok(Token::Semi).or_not())
                    .map(|((first, rest), _)| first.into_iter().chain(rest).collect::<Vec<_>>()),
            )
            .then_ignore(tok(Token::RBrace))
            .map_with(|members, e| ValueExpr {
                kind: ValueExprKind::Object(members),
                span: e.span(),
            })
            .boxed();

        choice((logical_or, array, object)).boxed()
    })
    .boxed()
}

/// Convert an expression node to the value surface (never arrays/objects).
fn expr_to_value(expr: Expr) -> ValueExpr {
    let span = expr.span;
    let kind = match expr.kind {
        ExprKind::Ident(text) => ValueExprKind::Ident(text),
        ExprKind::StringLit(text) => ValueExprKind::String(text),
        ExprKind::NumLit(number) => ValueExprKind::Number(number),
        ExprKind::BoolLit(value) => ValueExprKind::Bool(value),
        ExprKind::Field {
            object,
            field,
            field_span,
        } => ValueExprKind::Field {
            object: Box::new(expr_to_value(*object)),
            field,
            field_span,
        },
        ExprKind::Binary { left, op, right } => ValueExprKind::Binary {
            left: Box::new(expr_to_value(*left)),
            op,
            right: Box::new(expr_to_value(*right)),
        },
        ExprKind::Unary { op, operand } => ValueExprKind::Unary {
            op,
            operand: Box::new(expr_to_value(*operand)),
        },
        ExprKind::Group(inner) => ValueExprKind::Group(Box::new(expr_to_value(*inner))),
        ExprKind::Call {
            name,
            name_span,
            args,
        } => ValueExprKind::Call {
            name,
            name_span,
            args,
        },
    };
    ValueExpr { kind, span }
}

/// `identifier ("." identifier)+` on the value surface, folded left.
fn value_field_expr<'src>() -> impl Parser<'src, Input<'src>, ValueExpr, Extra<'src>> + Clone {
    ident()
        .then(
            tok(Token::Dot)
                .ignore_then(ident())
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .map(|(first, rest)| {
            let mut object = ValueExpr {
                kind: ValueExprKind::Ident(first.0),
                span: first.1,
            };
            for (field, field_span) in rest {
                object = ValueExpr {
                    span: (object.span.start..field_span.end).into(),
                    kind: ValueExprKind::Field {
                        object: Box::new(object),
                        field,
                        field_span,
                    },
                };
            }
            object
        })
}

/// `identifier ("." identifier)+` on the expression surface, folded left.
fn expr_field<'src>() -> impl Parser<'src, Input<'src>, Expr, Extra<'src>> + Clone {
    ident()
        .then(
            tok(Token::Dot)
                .ignore_then(ident())
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .map(|(first, rest)| {
            let mut object = Expr {
                kind: ExprKind::Ident(first.0),
                span: first.1,
            };
            for (field, field_span) in rest {
                object = Expr {
                    span: (object.span.start..field_span.end).into(),
                    kind: ExprKind::Field {
                        object: Box::new(object),
                        field,
                        field_span,
                    },
                };
            }
            object
        })
}

/// `identifier "(" (expression ("," expression)*)? ")"`.
fn call_expr<'src, P>(
    expr: P,
) -> impl Parser<'src, Input<'src>, (String, Span, Vec<Expr>, Span), Extra<'src>> + Clone
where
    P: Parser<'src, Input<'src>, Expr, Extra<'src>> + Clone + 'src,
{
    ident()
        .then(
            expr.separated_by(tok(Token::Comma))
                .collect::<Vec<_>>()
                .delimited_by(tok(Token::LParen), tok(Token::RParen)),
        )
        .map_with(|(name, args), e| (name.0, name.1, args, e.span()))
}

/// A `param_default` literal: string, number, or boolean. A negative number
/// requires the minus to be adjacent to the digits (the PEG atomic rule).
pub(crate) fn param_default<'src>(
) -> impl Parser<'src, Input<'src>, (DefaultValue, Span), Extra<'src>> + Clone {
    let negative = tok(Token::Minus)
        .then(number_lit())
        .filter(|(minus, (_, number))| minus.end == number.start)
        .map(|(minus, (value, span))| {
            (DefaultValue::Number(-value), (minus.start..span.end).into())
        });
    choice((
        string_lit().map(|(text, span)| (DefaultValue::String(text), span)),
        negative,
        number_lit().map(|(value, span)| (DefaultValue::Number(value), span)),
        boolean_lit().map(|(value, span)| (DefaultValue::Bool(value), span)),
    ))
}

/// A `type_ref` as written (builtins and identifiers are both identifier
/// tokens in this surface).
pub(crate) fn type_ref<'src>() -> impl Parser<'src, Input<'src>, (String, Span), Extra<'src>> + Clone
{
    ident()
}

/// An `Entity.Property` reference.
pub(crate) fn property_ref<'src>(
) -> impl Parser<'src, Input<'src>, PropertyRef, Extra<'src>> + Clone {
    ident()
        .then_ignore(tok(Token::Dot))
        .then(ident())
        .map(|(entity, property)| PropertyRef {
            entity: entity.0,
            property: property.0,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parse_expr(source: &str) -> Expr {
        let tokens = lex(source);
        let (out, errors) = expr_parser()
            .parse(Input::new(&tokens))
            .into_output_errors();
        assert!(errors.is_empty(), "{errors:?}");
        out.expect("expression parses")
    }

    #[test]
    fn precedence_and_associativity_match_the_peg_ladder() {
        // a || b && c
        let expr = parse_expr("a || b && c");
        let ExprKind::Binary { op, right, .. } = expr.kind else {
            panic!("expected || at the root");
        };
        assert_eq!(op, BinOp::Or);
        assert!(matches!(
            right.kind,
            ExprKind::Binary { op: BinOp::And, .. }
        ));

        // a - b - c folds left
        let expr = parse_expr("a - b - c");
        let ExprKind::Binary { left, op, .. } = expr.kind else {
            panic!("expected a subtraction at the root");
        };
        assert_eq!(op, BinOp::Sub);
        assert!(matches!(left.kind, ExprKind::Binary { op: BinOp::Sub, .. }));
    }

    #[test]
    fn grouping_is_preserved() {
        let expr = parse_expr("(a)");
        assert!(matches!(expr.kind, ExprKind::Group(_)));
    }

    #[test]
    fn field_access_folds_left_and_bare_identifiers_stay_identifiers() {
        let expr = parse_expr("a.b.c");
        let ExprKind::Field { object, field, .. } = expr.kind else {
            panic!("expected field access");
        };
        assert_eq!(field, "c");
        assert!(matches!(object.kind, ExprKind::Field { .. }));

        assert!(matches!(parse_expr("a").kind, ExprKind::Ident(_)));
    }

    #[test]
    fn calls_parse_and_field_calls_do_not() {
        assert!(matches!(parse_expr("f(a, b)").kind, ExprKind::Call { .. }));
        let tokens = lex("a.b(c)");
        let (out, errors) = expr_parser()
            .parse(Input::new(&tokens))
            .into_output_errors();
        assert!(out.is_none());
        assert!(!errors.is_empty());
    }

    #[test]
    fn negative_numbers_are_unary_in_expression_position() {
        let expr = parse_expr("-1");
        let ExprKind::Unary { op, .. } = expr.kind else {
            panic!("expected a unary node");
        };
        assert_eq!(op, UnaryOp::Neg);
    }

    fn parse_value(source: &str) -> ValueExpr {
        let tokens = lex(source);
        let (out, errors) = value_parser()
            .parse(Input::new(&tokens))
            .into_output_errors();
        assert!(errors.is_empty(), "{errors:?}");
        out.expect("value parses")
    }

    #[test]
    fn arrays_and_objects_parse_but_nested_objects_do_not() {
        assert!(matches!(
            parse_value("[1, \"a\", [2]]").kind,
            ValueExprKind::Array(_)
        ));
        assert!(matches!(
            parse_value("{ x: 1; y: [2]; }").kind,
            ValueExprKind::Object(_)
        ));
        let tokens = lex("{ x: { y: 1 } }");
        let (out, errors) = value_parser()
            .parse(Input::new(&tokens))
            .into_output_errors();
        assert!(out.is_none());
        assert!(!errors.is_empty());

        // Parentheses carry the expression surface, which has no arrays.
        let tokens = lex("([1])");
        let (out, errors) = value_parser()
            .parse(Input::new(&tokens))
            .into_output_errors();
        assert!(out.is_none());
        assert!(!errors.is_empty());
    }
}
