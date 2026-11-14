//! Views, containers, and the shared body shape.

use super::*;

/// Tokens that can begin the next view/container member (and therefore end a
/// junk region), plus the enclosing `}` and the next top-level declaration.
fn stop_at_view_member(token: &Token<'_>) -> bool {
    matches!(token, Token::RBrace)
        || is_top_introducer(token)
        || matches!(
            token,
            Token::Ident(text)
                if matches!(
                    *text,
                    "params" | "label" | "container" | "component" | "on" | "use" | "if"
                )
        )
}

fn empty_body(position: usize) -> Body {
    Body {
        params: None,
        label: None,
        properties: Vec::new(),
        items: Vec::new(),
        span: (position..position).into(),
    }
}

/// One item in a view/container body, before phase validation.
enum BodyMember {
    Params(Params),
    Label(Label),
    Property(Property),
    Item(BodyItem),
    Junk,
}

/// `view "name" { ... }`. The header commits once the name matched: a
/// missing body recovers a view node plus one error.
pub(crate) fn view_decl<'src>() -> impl Parser<'src, Input<'src>, ViewDecl, Extra<'src>> + Clone {
    word("view")
        .ignore_then(string_lit())
        .then(body_parser().or_not())
        .validate(|(name, body), e, emitter| {
            let body = body.unwrap_or_else(|| {
                emitter.emit(Rich::custom(
                    (name.1.end..name.1.end).into(),
                    "expected '{' after the view name",
                ));
                empty_body(name.1.end)
            });
            ViewDecl {
                name: name.0,
                name_span: name.1,
                body,
                span: e.span(),
            }
        })
}

/// `container "name" { ... }` — the module-facing entry.
pub(crate) fn container_decl<'src>() -> BoxedP<'src, ContainerDecl> {
    container_from(body_parser()).boxed()
}

/// A container whose body is the supplied (possibly recursive) body parser.
fn container_from<'src, P>(
    body: P,
) -> impl Parser<'src, Input<'src>, ContainerDecl, Extra<'src>> + Clone
where
    P: Parser<'src, Input<'src>, Body, Extra<'src>> + Clone + 'src,
{
    word("container")
        .ignore_then(string_lit())
        .then(body.or_not())
        .validate(|(name, body), e, emitter| {
            let body = body.unwrap_or_else(|| {
                emitter.emit(Rich::custom(
                    (name.1.end..name.1.end).into(),
                    "expected '{' after the container name",
                ));
                empty_body(name.1.end)
            });
            ContainerDecl {
                name: name.0,
                name_span: name.1,
                body,
                span: e.span(),
            }
        })
}

/// A view/container body. Members are parsed in any order and the PEG's
/// phases (`params? label? property* (container|component|event|use|
/// condition)*`) are validated afterwards, so an out-of-order member is
/// recovered into the right list and reported instead of discarding the rest.
pub(crate) fn body_parser<'src>() -> BoxedP<'src, Body> {
    recursive(|body| {
        let container = container_from(body)
            .map(|container| BodyMember::Item(BodyItem::Container(Box::new(container))));
        let member = choice((
            container,
            components::component_decl()
                .map(|component| BodyMember::Item(BodyItem::Component(Box::new(component)))),
            events::event_handler().map(|event| BodyMember::Item(BodyItem::Event(event))),
            modules::module_use().map(|use_decl| BodyMember::Item(BodyItem::Use(use_decl))),
            condition().map(|condition| BodyMember::Item(BodyItem::Condition(condition))),
        ));
        let item = choice((
            params_block().map(BodyMember::Params),
            label_decl().map(BodyMember::Label),
            property().map(BodyMember::Property),
            member,
            junk_member(stop_at_view_member, "expected a view body member")
                .map(|()| BodyMember::Junk),
        ));

        tok(Token::LBrace)
            .ignore_then(item.repeated().collect::<Vec<_>>())
            .then(tok(Token::RBrace).or_not())
            .validate(|(items, close), e, emitter| {
                let mut params = None;
                let mut label = None;
                let mut properties = Vec::new();
                let mut body_items = Vec::new();
                let mut seen_params = false;
                let mut seen_label = false;
                let mut seen_property = false;
                let mut seen_item = false;

                for item in items {
                    match item {
                        BodyMember::Params(value) => {
                            if seen_params {
                                emitter.emit(Rich::custom(
                                    value.span,
                                    "duplicate `params` block in the body",
                                ));
                            } else if seen_label || seen_property || seen_item {
                                emitter.emit(Rich::custom(
                                    value.span,
                                    "`params` must precede the body's other members",
                                ));
                            }
                            seen_params = true;
                            params = Some(value);
                        }
                        BodyMember::Label(value) => {
                            if seen_label {
                                emitter.emit(Rich::custom(
                                    value.span,
                                    "duplicate `label` declaration in the body",
                                ));
                            } else if seen_property || seen_item {
                                emitter.emit(Rich::custom(
                                    value.span,
                                    "`label` must precede the body's properties and members",
                                ));
                            }
                            seen_label = true;
                            label = Some(value);
                        }
                        BodyMember::Property(value) => {
                            if seen_item {
                                emitter.emit(Rich::custom(
                                    value.span,
                                    "property assignments must precede the body's members",
                                ));
                            }
                            seen_property = true;
                            properties.push(value);
                        }
                        BodyMember::Item(value) => {
                            seen_item = true;
                            body_items.push(value);
                        }
                        BodyMember::Junk => {}
                    }
                }

                if close.is_none() {
                    emitter.emit(Rich::custom(
                        (e.span().end..e.span().end).into(),
                        "expected '}' to close the body",
                    ));
                }
                Body {
                    params,
                    label,
                    properties,
                    items: body_items,
                    span: e.span(),
                }
            })
    })
    .boxed()
}
