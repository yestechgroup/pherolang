//! Components, typed table columns, form fields, and inline charts.

use super::*;

/// Tokens that can begin the next component member (and therefore end a junk
/// region).
fn stop_at_component_member(token: &Token<'_>) -> bool {
    matches!(token, Token::RBrace)
        || is_top_introducer(token)
        || matches!(
            token,
            Token::Ident(text)
                if matches!(*text, "column" | "field" | "chart" | "if" | "on")
        )
}

fn empty_component_body(position: usize) -> ComponentBody {
    ComponentBody {
        properties: Vec::new(),
        members: Vec::new(),
        span: (position..position).into(),
    }
}

enum ComponentItem {
    Property(Property),
    Member(ComponentMember),
    Junk,
}

/// `component "name" { ... }`. The header commits once the name matched.
pub(super) fn component_decl<'src>(
) -> impl Parser<'src, Input<'src>, ComponentDecl, Extra<'src>> + Clone {
    word("component")
        .ignore_then(string_lit())
        .then(component_body().or_not())
        .validate(|(name, body), e, emitter| {
            let body = body.unwrap_or_else(|| {
                emitter.emit(Rich::custom(
                    (name.1.end..name.1.end).into(),
                    "expected '{' after the component name",
                ));
                empty_component_body(name.1.end)
            });
            ComponentDecl {
                name: name.0,
                name_span: name.1,
                body,
                span: e.span(),
            }
        })
}

/// The component body, recovered in any order and phase-checked afterwards:
/// `property* (column|field|chart|condition)* (event|condition)*`.
fn component_body<'src>() -> impl Parser<'src, Input<'src>, ComponentBody, Extra<'src>> + Clone {
    let member = choice((
        column_decl().map(ComponentMember::Column),
        field_decl().map(ComponentMember::Field),
        chart_decl().map(ComponentMember::Chart),
        condition().map(ComponentMember::Condition),
        events::event_handler().map(ComponentMember::Event),
    ));
    let item = choice((
        property().map(ComponentItem::Property),
        member.map(ComponentItem::Member),
        junk_member(stop_at_component_member, "expected a component body member")
            .map(|()| ComponentItem::Junk),
    ));

    tok(Token::LBrace)
        .ignore_then(item.repeated().collect::<Vec<_>>())
        .then(tok(Token::RBrace).or_not())
        .validate(|(items, close), e, emitter| {
            let mut properties = Vec::new();
            let mut members = Vec::new();
            // 0 = properties, 1 = typed members/conditions, 2 = events/conditions.
            let mut phase = 0u8;
            for item in items {
                match item {
                    ComponentItem::Property(property) => {
                        if phase > 0 {
                            emitter.emit(Rich::custom(
                                property.span,
                                "property assignments must precede the component's typed/event members",
                            ));
                        }
                        properties.push(property);
                    }
                    ComponentItem::Member(ComponentMember::Condition(condition)) => {
                        if phase == 0 {
                            phase = 1;
                        }
                        members.push(ComponentMember::Condition(condition));
                    }
                    ComponentItem::Member(member @ (ComponentMember::Column(_)
                    | ComponentMember::Field(_)
                    | ComponentMember::Chart(_))) => {
                        if phase == 2 {
                            emitter.emit(Rich::custom(
                                member_span(&member),
                                "typed component members must precede event handlers",
                            ));
                        }
                        phase = phase.max(1);
                        members.push(member);
                    }
                    ComponentItem::Member(member @ ComponentMember::Event(_)) => {
                        phase = 2;
                        members.push(member);
                    }
                    ComponentItem::Junk => {}
                }
            }
            if close.is_none() {
                emitter.emit(Rich::custom(
                    (e.span().end..e.span().end).into(),
                    "expected '}' to close the component body",
                ));
            }
            ComponentBody {
                properties,
                members,
                span: e.span(),
            }
        })
}

fn member_span(member: &ComponentMember) -> Span {
    match member {
        ComponentMember::Column(column) => column.span,
        ComponentMember::Field(field) => field.span,
        ComponentMember::Chart(chart) => chart.span,
        ComponentMember::Condition(condition) => condition.span,
        ComponentMember::Event(event) => event.span,
    }
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
            if std::str::FromStr::from_str(text)
                .map(|kind: phero_ir::ifml::ChartKind| kind)
                .is_ok() =>
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
