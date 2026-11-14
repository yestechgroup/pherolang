//! Module declarations (`module "M" { input/output ... }`) and `use`
//! statements.

use super::*;

fn stop_at_module_member(token: &Token<'_>) -> bool {
    matches!(token, Token::RBrace)
        || is_top_introducer(token)
        || matches!(
            token,
            Token::Ident(text)
                if matches!(*text, "input" | "output" | "container" | "component" | "on" | "use")
        )
}

fn empty_params(position: usize) -> Params {
    Params {
        decls: Vec::new(),
        span: (position..position).into(),
    }
}

enum ModuleAstItem {
    Input(Params),
    Output(Params),
    Property(Property),
    Item(ModuleItem),
    Junk,
}

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

/// `module "name" { ... }`. The header commits once the name matched.
pub(crate) fn module_decl<'src>() -> impl Parser<'src, Input<'src>, ModuleDecl, Extra<'src>> + Clone
{
    word("module")
        .ignore_then(string_lit())
        .then(module_body().or_not())
        .validate(|(name, body), e, emitter| {
            let (input, output, properties, items) = body.unwrap_or_else(|| {
                emitter.emit(Rich::custom(
                    (name.1.end..name.1.end).into(),
                    "expected '{' after the module name",
                ));
                (
                    empty_params(name.1.end),
                    empty_params(name.1.end),
                    Vec::new(),
                    Vec::new(),
                )
            });
            ModuleDecl {
                name: name.0,
                name_span: name.1,
                input,
                output,
                properties,
                items,
                span: e.span(),
            }
        })
}

/// The module body: `input { ... } output { ... } property* (container |
/// component | event | use)*`, recovered in any order and phase-checked.
fn module_body<'src>(
) -> impl Parser<'src, Input<'src>, (Params, Params, Vec<Property>, Vec<ModuleItem>), Extra<'src>> + Clone
{
    let item = choice((
        word("input")
            .ignore_then(braced_params())
            .map(ModuleAstItem::Input),
        word("output")
            .ignore_then(braced_params())
            .map(ModuleAstItem::Output),
        property().map(ModuleAstItem::Property),
        views::container_decl()
            .map(|container| ModuleAstItem::Item(ModuleItem::Container(Box::new(container)))),
        components::component_decl()
            .map(|component| ModuleAstItem::Item(ModuleItem::Component(Box::new(component)))),
        events::event_handler().map(|event| ModuleAstItem::Item(ModuleItem::Event(event))),
        module_use().map(|use_decl| ModuleAstItem::Item(ModuleItem::Use(use_decl))),
        junk_member(stop_at_module_member, "expected a module body member")
            .map(|()| ModuleAstItem::Junk),
    ));

    tok(Token::LBrace)
        .ignore_then(item.repeated().collect::<Vec<_>>())
        .then(tok(Token::RBrace).or_not())
        .validate(|(items, close), e, emitter| {
            let mut input = None;
            let mut output = None;
            let mut properties = Vec::new();
            let mut module_items = Vec::new();
            // 0 = input, 1 = output, 2 = properties, 3 = members.
            let mut phase = 0u8;
            for item in items {
                match item {
                    ModuleAstItem::Input(params) => {
                        if input.is_some() {
                            emitter.emit(Rich::custom(
                                params.span,
                                "duplicate `input` parameter block",
                            ));
                        } else if phase > 0 {
                            emitter.emit(Rich::custom(
                                params.span,
                                "`input` must be the first module body member",
                            ));
                        }
                        input = Some(params);
                        phase = 0;
                    }
                    ModuleAstItem::Output(params) => {
                        if output.is_some() {
                            emitter.emit(Rich::custom(
                                params.span,
                                "duplicate `output` parameter block",
                            ));
                        } else if phase > 1 {
                            emitter.emit(Rich::custom(
                                params.span,
                                "`output` must follow `input` and precede the body's other members",
                            ));
                        }
                        if input.is_none() {
                            emitter.emit(Rich::custom(
                                params.span,
                                "expected an `input` parameter block before `output`",
                            ));
                        }
                        output = Some(params);
                        phase = 1;
                    }
                    ModuleAstItem::Property(property) => {
                        if phase > 2 {
                            emitter.emit(Rich::custom(
                                property.span,
                                "property assignments must precede the module's members",
                            ));
                        }
                        phase = 2;
                        properties.push(property);
                    }
                    ModuleAstItem::Item(item) => {
                        phase = 3;
                        module_items.push(item);
                    }
                    ModuleAstItem::Junk => {}
                }
            }
            let input = match input {
                Some(input) => input,
                None => {
                    emitter.emit(Rich::custom(
                        e.span(),
                        "expected an `input` parameter block",
                    ));
                    empty_params(e.span().end)
                }
            };
            let output = match output {
                Some(output) => output,
                None => {
                    emitter.emit(Rich::custom(
                        e.span(),
                        "expected an `output` parameter block",
                    ));
                    empty_params(e.span().end)
                }
            };
            if close.is_none() {
                emitter.emit(Rich::custom(
                    (e.span().end..e.span().end).into(),
                    "expected '}' to close the module body",
                ));
            }
            (input, output, properties, module_items)
        })
}
