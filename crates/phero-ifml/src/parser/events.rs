//! Event handlers and their actions: `on <type>(params)? requires: [...]?
//! if <expr>? -> navigate/refresh/action/stay ;`.

use super::*;

/// `on <type> ... -> <action>;`.
///
/// Event handlers and inline action bodies are mutually recursive
/// (`action("X", { on ... -> ...; })`), so one `recursive` group builds both
/// and the handler is threaded into the action-body parser.
pub(super) fn event_handler<'src>() -> BoxedP<'src, EventHandler> {
    recursive(|handler| {
        let body = action_body(handler);
        let action = event_action(body);
        word("on")
            .ignore_then(ident())
            .then(event_param().or_not())
            .then(event_requires().or_not())
            .then(word("if").ignore_then(expr::expr_parser()).or_not())
            .then_ignore(tok(Token::Arrow))
            .then(action)
            .then(tok(Token::Semi).or_not())
            .validate(
                |(((((event, params), requires), condition), action), semi), e, emitter| {
                    if semi.is_none() {
                        emitter.emit(Rich::custom(
                            event.1,
                            format!("expected ';' after the '{}' event handler", event.0),
                        ));
                    }
                    EventHandler {
                        event_type: event.0,
                        type_span: event.1,
                        params: params.unwrap_or_default(),
                        requires: requires.unwrap_or_default(),
                        condition,
                        action,
                        span: e.span(),
                    }
                },
            )
    })
    .boxed()
}

/// `( name (, name)* )` — at least one parameter.
fn event_param<'src>() -> impl Parser<'src, Input<'src>, Vec<String>, Extra<'src>> + Clone {
    let name = ident().map(|(name, _)| name);
    tok(Token::LParen)
        .ignore_then(
            name.clone()
                .then(
                    tok(Token::Comma)
                        .ignore_then(name)
                        .repeated()
                        .collect::<Vec<_>>(),
                )
                .map(|(first, rest)| std::iter::once(first).chain(rest).collect::<Vec<_>>()),
        )
        .then_ignore(tok(Token::RParen))
}

/// `requires: [Cap, ...]` — at least one capability, with per-name spans.
fn event_requires<'src>() -> impl Parser<'src, Input<'src>, Vec<AuthName>, Extra<'src>> + Clone {
    let name = ident().map(|(name, span)| AuthName { name, span });
    word("requires")
        .ignore_then(tok(Token::Colon))
        .ignore_then(tok(Token::LBracket))
        .ignore_then(
            name.clone()
                .then(
                    tok(Token::Comma)
                        .ignore_then(name)
                        .repeated()
                        .collect::<Vec<_>>(),
                )
                .map(|(first, rest)| std::iter::once(first).chain(rest).collect::<Vec<_>>()),
        )
        .then_ignore(tok(Token::RBracket))
}

fn event_action<'src, P>(
    action_body: P,
) -> impl Parser<'src, Input<'src>, EventAction, Extra<'src>> + Clone
where
    P: Parser<'src, Input<'src>, ActionBody, Extra<'src>> + Clone + 'src,
{
    let navigate = word("navigate")
        .ignore_then(tok(Token::LParen))
        .ignore_then(string_lit())
        .then(tok(Token::Comma).ignore_then(parameter_binding()).or_not())
        .then_ignore(tok(Token::RParen))
        .map(|((target, target_span), binding)| EventAction::Navigate {
            target,
            target_span,
            binding,
        });
    let refresh = word("refresh")
        .ignore_then(tok(Token::LParen))
        .ignore_then(string_lit())
        .then(tok(Token::Comma).ignore_then(parameter_binding()).or_not())
        .then_ignore(tok(Token::RParen))
        .map(|((target, target_span), binding)| EventAction::Refresh {
            target,
            target_span,
            binding,
        });
    let invocation = word("action")
        .ignore_then(tok(Token::LParen))
        .ignore_then(string_lit())
        .then(tok(Token::Comma).ignore_then(action_body).or_not())
        .then_ignore(tok(Token::RParen))
        .map(|((name, name_span), body)| EventAction::ActionInvocation {
            name,
            name_span,
            body,
        });
    let stay = word("stay").to(EventAction::Stay);
    choice((navigate, refresh, invocation, stay))
}

/// `{ key: <expression> (, key: <expression>)* }`.
fn parameter_binding<'src>() -> impl Parser<'src, Input<'src>, ParameterBinding, Extra<'src>> + Clone
{
    let pair = ident()
        .then_ignore(tok(Token::Colon))
        .then(expr::expr_parser())
        .map(|(key, value)| (key.0, value));
    tok(Token::LBrace)
        .ignore_then(
            pair.clone()
                .then(
                    tok(Token::Comma)
                        .ignore_then(pair)
                        .repeated()
                        .collect::<Vec<_>>(),
                )
                .or_not()
                .map(|pairs| {
                    pairs
                        .map(|(first, rest)| std::iter::once(first).chain(rest).collect::<Vec<_>>())
                        .unwrap_or_default()
                }),
        )
        .then_ignore(tok(Token::RBrace))
        .map_with(|pairs, e| ParameterBinding {
            pairs,
            span: e.span(),
        })
}

/// `{ property* event* }` — an inline action body.
fn stop_at_action_body_member(token: &Token<'_>) -> bool {
    matches!(token, Token::RBrace)
        || is_top_introducer(token)
        || matches!(token, Token::Ident(text) if *text == "on")
}

enum ActionBodyItem {
    Property(Property),
    Event(EventHandler),
    Junk,
}

/// `{ property* event* }` — an inline action body, recovered in any order.
fn action_body<'src, P>(
    handler: P,
) -> impl Parser<'src, Input<'src>, ActionBody, Extra<'src>> + Clone
where
    P: Parser<'src, Input<'src>, EventHandler, Extra<'src>> + Clone + 'src,
{
    let item = choice((
        property().map(ActionBodyItem::Property),
        handler.map(ActionBodyItem::Event),
        junk_member(stop_at_action_body_member, "expected an action body member")
            .map(|()| ActionBodyItem::Junk),
    ));
    tok(Token::LBrace)
        .ignore_then(item.repeated().collect::<Vec<_>>())
        .then(tok(Token::RBrace).or_not())
        .validate(|(items, close), e, emitter| {
            let mut properties = Vec::new();
            let mut events = Vec::new();
            let mut seen_event = false;
            for item in items {
                match item {
                    ActionBodyItem::Property(property) => {
                        if seen_event {
                            emitter.emit(Rich::custom(
                                property.span,
                                "property assignments must precede the action body's event handlers",
                            ));
                        }
                        properties.push(property);
                    }
                    ActionBodyItem::Event(event) => {
                        seen_event = true;
                        events.push(event);
                    }
                    ActionBodyItem::Junk => {}
                }
            }
            if close.is_none() {
                emitter.emit(Rich::custom(
                    (e.span().end..e.span().end).into(),
                    "expected '}' to close the action body",
                ));
            }
            ActionBody {
                properties,
                events,
                span: e.span(),
            }
        })
}
