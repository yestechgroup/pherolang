//! CLI `command` declarations, their typed members, and nested subcommands.

use super::*;

fn stop_at_command_member(token: &Token<'_>) -> bool {
    matches!(token, Token::RBrace)
        || is_top_introducer(token)
        || matches!(
            token,
            Token::Ident(text)
                if matches!(
                    *text,
                    "argument" | "option" | "prompt" | "on" | "if" | "use" | "command"
                )
        )
}

/// `command "name" { ... }`. The header commits once the name matched, and
/// subcommands recurse through the same parser.
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
        let item = choice((
            member.map(Some),
            junk_member(stop_at_command_member, "expected a command body member").map(|()| None),
        ));
        let body = tok(Token::LBrace)
            .ignore_then(item.repeated().collect::<Vec<_>>())
            .then(tok(Token::RBrace).or_not())
            .validate(|(items, close), e, emitter| {
                if close.is_none() {
                    emitter.emit(Rich::custom(
                        (e.span().end..e.span().end).into(),
                        "expected '}' to close the command body",
                    ));
                }
                items.into_iter().flatten().collect::<Vec<_>>()
            });

        word("command")
            .ignore_then(string_lit())
            .then(body.or_not())
            .validate(|(name, members), e, emitter| {
                let members = members.unwrap_or_else(|| {
                    emitter.emit(Rich::custom(
                        (name.1.end..name.1.end).into(),
                        "expected '{' after the command name",
                    ));
                    Vec::new()
                });
                CommandDecl {
                    name: name.0,
                    name_span: name.1,
                    members,
                    span: e.span(),
                }
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
