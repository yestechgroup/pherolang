//! Module declarations (`module "M" { input/output ... }`) and `use`
//! statements.

use super::*;

/// `use "Module" as alias? { overrides } ;?` or `use "Module";`.
pub(crate) fn module_use<'src>() -> impl Parser<'src, Input<'src>, ModuleUse, Extra<'src>> + Clone {
    let body = tok(Token::LBrace)
        .ignore_then(property().repeated().collect::<Vec<_>>())
        .then_ignore(tok(Token::RBrace))
        .then_ignore(tok(Token::Semi).or_not());
    let bare = tok(Token::Semi).to(Vec::new());

    word("use")
        .ignore_then(string_lit())
        .then(word("as").ignore_then(ident()).or_not())
        .then(choice((body, bare)))
        .map_with(|((module, alias), properties), e| ModuleUse {
            module: module.0,
            target_span: module.1,
            alias: alias.map(|(alias, _)| alias),
            properties,
            span: e.span(),
        })
}

/// `module "name" { input { ... } output { ... } property* (container |
/// component | event | use)* }`.
pub(crate) fn module_decl<'src>() -> impl Parser<'src, Input<'src>, ModuleDecl, Extra<'src>> + Clone
{
    let item = choice((
        views::container_decl().map(|container| ModuleItem::Container(Box::new(container))),
        components::component_decl().map(|component| ModuleItem::Component(Box::new(component))),
        events::event_handler().map(ModuleItem::Event),
        module_use().map(ModuleItem::Use),
    ));

    word("module")
        .ignore_then(string_lit())
        .then_ignore(tok(Token::LBrace))
        .then_ignore(word("input"))
        .then(braced_params())
        .then_ignore(word("output"))
        .then(braced_params())
        .then(property().repeated().collect::<Vec<_>>())
        .then(item.repeated().collect::<Vec<_>>())
        .then_ignore(tok(Token::RBrace))
        .map_with(
            |((((name, input), output), properties), items), e| ModuleDecl {
                name: name.0,
                name_span: name.1,
                input,
                output,
                properties,
                items,
                span: e.span(),
            },
        )
}
