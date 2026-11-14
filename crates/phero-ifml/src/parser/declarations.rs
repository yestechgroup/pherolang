//! Top-level import/domain/action/actor declarations.

use super::*;

/// `import "path";` — the trailing `;` is optional.
pub(crate) fn import_decl<'src>() -> impl Parser<'src, Input<'src>, ImportDecl, Extra<'src>> + Clone
{
    word("import")
        .ignore_then(string_lit())
        .then_ignore(tok(Token::Semi).or_not())
        .map_with(|(path, _), e| ImportDecl {
            path,
            span: e.span(),
        })
}

/// `domain "name" { schema "path"; }`.
pub(crate) fn domain_decl<'src>() -> impl Parser<'src, Input<'src>, DomainDecl, Extra<'src>> + Clone
{
    word("domain")
        .ignore_then(string_lit())
        .then_ignore(tok(Token::LBrace))
        .then_ignore(word("schema"))
        .then(string_lit())
        .then_ignore(tok(Token::Semi))
        .then_ignore(tok(Token::RBrace))
        .map_with(|((name, _), (schema, _)), e| DomainDecl {
            name,
            schema,
            span: e.span(),
        })
}

fn stop_at_action_member(token: &Token<'_>) -> bool {
    matches!(token, Token::RBrace)
        || is_top_introducer(token)
        || matches!(token, Token::Ident(text) if *text == "on")
}

fn stop_at_actor_member(token: &Token<'_>) -> bool {
    matches!(token, Token::RBrace) || is_top_introducer(token)
}

enum ActionItem {
    Property(Property),
    Event(EventHandler),
    Junk,
}

/// `action "name" { property* event* }`. The header commits once the name
/// matched; the body recovers.
pub(crate) fn action_decl<'src>() -> impl Parser<'src, Input<'src>, ActionDecl, Extra<'src>> + Clone
{
    word("action")
        .ignore_then(string_lit())
        .then(action_body().or_not())
        .validate(|(name, body), e, emitter| {
            let (properties, events) = body.unwrap_or_else(|| {
                emitter.emit(Rich::custom(
                    (name.1.end..name.1.end).into(),
                    "expected '{' after the action name",
                ));
                (Vec::new(), Vec::new())
            });
            ActionDecl {
                name: name.0,
                name_span: name.1,
                properties,
                events,
                span: e.span(),
            }
        })
}

fn action_body<'src>(
) -> impl Parser<'src, Input<'src>, (Vec<Property>, Vec<EventHandler>), Extra<'src>> + Clone {
    let item = choice((
        property().map(ActionItem::Property),
        events::event_handler().map(ActionItem::Event),
        junk_member(stop_at_action_member, "expected an action body member")
            .map(|()| ActionItem::Junk),
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
                    ActionItem::Property(property) => {
                        if seen_event {
                            emitter.emit(Rich::custom(
                                property.span,
                                "property assignments must precede the action's event handlers",
                            ));
                        }
                        properties.push(property);
                    }
                    ActionItem::Event(event) => {
                        seen_event = true;
                        events.push(event);
                    }
                    ActionItem::Junk => {}
                }
            }
            if close.is_none() {
                emitter.emit(Rich::custom(
                    (e.span().end..e.span().end).into(),
                    "expected '}' to close the action body",
                ));
            }
            (properties, events)
        })
}

/// `actor "name" { property* }`. The header commits once the name matched.
pub(crate) fn actor_decl<'src>() -> impl Parser<'src, Input<'src>, ActorDecl, Extra<'src>> + Clone {
    word("actor")
        .ignore_then(string_lit())
        .then(actor_body().or_not())
        .validate(|(name, properties), e, emitter| {
            let properties = properties.unwrap_or_else(|| {
                emitter.emit(Rich::custom(
                    (name.1.end..name.1.end).into(),
                    "expected '{' after the actor name",
                ));
                Vec::new()
            });
            ActorDecl {
                name: name.0,
                name_span: name.1,
                properties,
                span: e.span(),
            }
        })
}

fn actor_body<'src>() -> impl Parser<'src, Input<'src>, Vec<Property>, Extra<'src>> + Clone {
    let item = choice((
        property().map(Some),
        junk_member(stop_at_actor_member, "expected an actor body property").map(|()| None),
    ));
    tok(Token::LBrace)
        .ignore_then(item.repeated().collect::<Vec<_>>())
        .then(tok(Token::RBrace).or_not())
        .validate(|(items, close), e, emitter| {
            if close.is_none() {
                emitter.emit(Rich::custom(
                    (e.span().end..e.span().end).into(),
                    "expected '}' to close the actor body",
                ));
            }
            items.into_iter().flatten().collect()
        })
}
