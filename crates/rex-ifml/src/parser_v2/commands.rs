//! CLI `command` declarations, their typed members, and nested subcommands.

use super::*;

/// `command "name" { (property | argument | option | prompt | event |
/// condition | use | command)* }`.
pub(crate) fn command_decl<'src>() -> BoxedP<'src, CommandDecl> {
    recursive(|command| {
        let member = choice((
            property().map(CommandMember::Property),
            argument_decl().map(CommandMember::Argument),
            option_decl().map(CommandMember::Option),
            prompt_decl().map(CommandMember::Prompt),
            events::event_handler().map(CommandMember::Event),
            modules::module_use().map(CommandMember::Use),
            condition().map(CommandMember::Condition),
            command
                .clone()
                .map(|decl| CommandMember::Command(Box::new(decl))),
        ));

        word("command")
            .ignore_then(string_lit())
            .then_ignore(tok(Token::LBrace))
            .then(member.repeated().collect::<Vec<_>>())
            .then_ignore(tok(Token::RBrace))
            .map_with(|(name, members), e| CommandDecl {
                name: name.0,
                name_span: name.1,
                members,
                span: e.span(),
            })
    })
    .boxed()
}

/// `argument name: Type;`
fn argument_decl<'src>() -> impl Parser<'src, Input<'src>, CommandArgument, Extra<'src>> + Clone {
    word("argument")
        .ignore_then(ident())
        .then_ignore(tok(Token::Colon))
        .then(expr::type_ref())
        .then_ignore(tok(Token::Semi))
        .map_with(|(name, type_ref), e| CommandArgument {
            name: name.0,
            type_ref: type_ref.0,
            span: e.span(),
        })
}

/// `option name: Type (= default)?;`
fn option_decl<'src>() -> impl Parser<'src, Input<'src>, CommandOption, Extra<'src>> + Clone {
    word("option")
        .ignore_then(ident())
        .then_ignore(tok(Token::Colon))
        .then(expr::type_ref())
        .then(tok(Token::Eq).ignore_then(expr::param_default()).or_not())
        .then_ignore(tok(Token::Semi))
        .map_with(|((name, type_ref), default), e| CommandOption {
            name: name.0,
            type_ref: type_ref.0,
            default: default.map(|(value, _)| value),
            span: e.span(),
        })
}

/// `prompt name: kind (= default)?;`
fn prompt_decl<'src>() -> impl Parser<'src, Input<'src>, CommandPrompt, Extra<'src>> + Clone {
    word("prompt")
        .ignore_then(ident())
        .then_ignore(tok(Token::Colon))
        .then(ident())
        .then(tok(Token::Eq).ignore_then(expr::param_default()).or_not())
        .then_ignore(tok(Token::Semi))
        .map_with(|((name, kind), default), e| CommandPrompt {
            name: name.0,
            kind: kind.0,
            default: default.map(|(value, _)| value),
            span: e.span(),
        })
}
