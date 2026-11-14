//! Views, containers, and the shared body shape.

use super::*;

/// `view "name" { ... }`.
pub(crate) fn view_decl<'src>() -> impl Parser<'src, Input<'src>, ViewDecl, Extra<'src>> + Clone {
    word("view")
        .ignore_then(string_lit())
        .then(body_parser())
        .map_with(|(name, body), e| ViewDecl {
            name: name.0,
            name_span: name.1,
            body,
            span: e.span(),
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
        .then(body)
        .map_with(|(name, body), e| ContainerDecl {
            name: name.0,
            name_span: name.1,
            body,
            span: e.span(),
        })
}

/// A view/container body with the exact PEG phases:
/// `params? label? property* (container|component|event|use|condition)*`.
pub(crate) fn body_parser<'src>() -> BoxedP<'src, Body> {
    recursive(|body| {
        let container =
            container_from(body).map(|container| BodyItem::Container(Box::new(container)));
        let member = choice((
            container,
            components::component_decl().map(|component| BodyItem::Component(Box::new(component))),
            events::event_handler().map(BodyItem::Event),
            modules::module_use().map(BodyItem::Use),
            condition().map(BodyItem::Condition),
        ));

        tok(Token::LBrace)
            .ignore_then(
                params_block()
                    .or_not()
                    .then(label_decl().or_not())
                    .then(property().repeated().collect::<Vec<_>>())
                    .then(member.repeated().collect::<Vec<_>>()),
            )
            .then_ignore(tok(Token::RBrace))
            .map_with(|(((params, label), properties), items), e| Body {
                params,
                label,
                properties,
                items,
                span: e.span(),
            })
    })
    .boxed()
}
