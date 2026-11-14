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

/// `action "name" { property* event* }`.
pub(crate) fn action_decl<'src>() -> impl Parser<'src, Input<'src>, ActionDecl, Extra<'src>> + Clone
{
    word("action")
        .ignore_then(string_lit())
        .then_ignore(tok(Token::LBrace))
        .then(property().repeated().collect::<Vec<_>>())
        .then(events::event_handler().repeated().collect::<Vec<_>>())
        .then_ignore(tok(Token::RBrace))
        .map_with(|((name, properties), events), e| ActionDecl {
            name: name.0,
            name_span: name.1,
            properties,
            events,
            span: e.span(),
        })
}

/// `actor "name" { property* }`.
pub(crate) fn actor_decl<'src>() -> impl Parser<'src, Input<'src>, ActorDecl, Extra<'src>> + Clone {
    word("actor")
        .ignore_then(string_lit())
        .then_ignore(tok(Token::LBrace))
        .then(property().repeated().collect::<Vec<_>>())
        .then_ignore(tok(Token::RBrace))
        .map_with(|(name, properties), e| ActorDecl {
            name: name.0,
            name_span: name.1,
            properties,
            span: e.span(),
        })
}
