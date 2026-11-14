//! Components, typed table columns, form fields, and inline charts.

use super::*;

/// `component "name" { ... }`.
pub(super) fn component_decl<'src>(
) -> impl Parser<'src, Input<'src>, ComponentDecl, Extra<'src>> + Clone {
    word("component")
        .ignore_then(string_lit())
        .then(component_body())
        .map_with(|(name, body), e| ComponentDecl {
            name: name.0,
            name_span: name.1,
            body,
            span: e.span(),
        })
}

/// The component body's exact PEG phases:
/// `property* (column|field|chart|condition)* (event|condition)*`.
fn component_body<'src>() -> impl Parser<'src, Input<'src>, ComponentBody, Extra<'src>> + Clone {
    let typed = choice((
        column_decl().map(ComponentMember::Column),
        field_decl().map(ComponentMember::Field),
        chart_decl().map(ComponentMember::Chart),
        condition().map(ComponentMember::Condition),
    ));
    let tail = choice((
        events::event_handler().map(ComponentMember::Event),
        condition().map(ComponentMember::Condition),
    ));

    tok(Token::LBrace)
        .ignore_then(
            property()
                .repeated()
                .collect::<Vec<_>>()
                .then(typed.repeated().collect::<Vec<_>>())
                .then(tail.repeated().collect::<Vec<_>>()),
        )
        .then_ignore(tok(Token::RBrace))
        .map_with(|((properties, typed), tail), e| ComponentBody {
            properties,
            members: typed.into_iter().chain(tail).collect(),
            span: e.span(),
        })
}

/// `column "Label" -> lookup Entity.Prop via op | field Entity.Prop | expr e ;`
fn column_decl<'src>() -> impl Parser<'src, Input<'src>, ColumnDecl, Extra<'src>> + Clone {
    let lookup = word("lookup")
        .ignore_then(expr::property_ref())
        .then_ignore(word("via"))
        .then(ident())
        .map(|(field, lookup)| ColumnKind::Lookup {
            field,
            lookup: lookup.0,
        });
    let field = word("field")
        .ignore_then(expr::property_ref())
        .map(|field| ColumnKind::Field { field });
    let expression = word("expr")
        .ignore_then(expr::expr_parser())
        .map(|expr| ColumnKind::Expression { expr });
    let kind = choice((lookup, field, expression));

    word("column")
        .ignore_then(string_lit())
        .then_ignore(tok(Token::Arrow))
        .then(kind)
        .then_ignore(tok(Token::Semi))
        .map_with(|(label, kind), e| ColumnDecl {
            label: label.0,
            kind,
            span: e.span(),
        })
}

/// `field name -> input <type> { ... };`
fn field_decl<'src>() -> impl Parser<'src, Input<'src>, FieldDecl, Extra<'src>> + Clone {
    let body = tok(Token::LBrace)
        .ignore_then(property().repeated().collect::<Vec<_>>())
        .then_ignore(tok(Token::RBrace));
    let tail = choice((body, tok(Token::Semi).to(Vec::new())));

    word("field")
        .ignore_then(ident())
        .then_ignore(tok(Token::Arrow))
        .then_ignore(word("input"))
        .then(ident())
        .then(tail)
        .map_with(|(((name, _), input), properties), e| FieldDecl {
            name,
            input: input.0,
            properties,
            span: e.span(),
        })
}

/// `chart <kind> { ... };` — the kind vocabulary is closed (no identifier
/// fallback; unknown kinds are parse errors, as in the PEG grammar).
fn chart_kind<'src>() -> impl Parser<'src, Input<'src>, String, Extra<'src>> + Clone {
    select! {
        Token::Ident(text) = e
            if matches!(text, "bar" | "line" | "pie" | "radar" | "metric" | "org") =>
            text.to_string()
    }
}

fn chart_decl<'src>() -> impl Parser<'src, Input<'src>, ChartDecl, Extra<'src>> + Clone {
    let body = tok(Token::LBrace)
        .ignore_then(property().repeated().collect::<Vec<_>>())
        .then_ignore(tok(Token::RBrace));
    let tail = choice((body, tok(Token::Semi).to(Vec::new())));

    word("chart")
        .ignore_then(chart_kind())
        .then(tail)
        .map_with(|(kind, properties), e| ChartDecl {
            kind,
            properties,
            span: e.span(),
        })
}
