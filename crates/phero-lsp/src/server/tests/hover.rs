//! LSP server tests: hover.

//! The behavior tests: in-process via `LspService` and a client
//! socket (`tower-lsp` 0.20). The client channel has capacity one —
//! tests must interleave reads (`next_publish`/`drain_socket`) with
//! opens or the server's publish send deadlocks the handler.

use super::*;

use serde_json::json;
use std::str::FromStr;
use tower::Service;
use tower_lsp_server::jsonrpc;
use tower_lsp_server::LspService;

// --- hover ---------------------------------------------------------------

const HOVER: &str = "package demo\n\n\
        enum Mood { Happy = 0 }\n\n\
        type Date wraps opaque\n\n\
        class Library {\n\
        \x20   contains Book[] books opposite library\n\
        }\n\n\
        class Book {\n\
        \x20   container Library library opposite books\n\
        }\n";

/// The hover response at `line:character`, or `None` for null.
async fn hover_at(
    service: &mut LspService<PheroBackend>,
    line: u32,
    character: u32,
) -> Option<String> {
    let response = service
        .call(
            jsonrpc::Request::build("textDocument/hover")
                .params(json!({
                    "textDocument": {"uri": uri()},
                    "position": {"line": line, "character": character},
                }))
                .id(200)
                .finish(),
        )
        .await
        .unwrap()
        .unwrap();
    let (_, body) = response.into_parts();
    let value = body.expect("hover must not error");
    if value.is_null() {
        return None;
    }
    let hover: Hover = serde_json::from_value(value).expect("a Hover");
    match hover.contents {
        HoverContents::Markup(markup) => Some(markup.value),
        other => panic!("expected markup contents, got {other:?}"),
    }
}

/// Position of the start of the `nth` occurrence of `needle` in `HOVER`.
fn hover_pos(needle: &str, nth: usize) -> (u32, u32) {
    let map = PositionMap::new(HOVER);
    let (offset, _) = HOVER
        .match_indices(needle)
        .nth(nth)
        .unwrap_or_else(|| panic!("no occurrence {nth} of '{needle}'"));
    let position = map.position_for(offset);
    (position.line, position.character)
}

/// The provenance footer every `.mox` hover appends for the test URI.
const MOX_FOOTER: &str = "\n\n`.mox` · `shelf.mox` · [Domain model](https://github.com/yestechgroup/pherolang/blob/main/docs/LANGUAGE.md)";

#[tokio::test]
async fn hover_renders_definitions_as_markdown() {
    let (mut service, _socket) = initialized_service().await;
    open(&mut service, HOVER).await;

    // The class name `Library`.
    let (line, character) = hover_pos("Library", 0);
    assert_eq!(
        hover_at(&mut service, line, character).await,
        Some(format!("**class Library**{MOX_FOOTER}"))
    );

    // The containment feature `books`.
    let (line, character) = hover_pos("books", 0);
    assert_eq!(
        hover_at(&mut service, line, character).await,
        Some(format!(
            "**books**: Book[]\ncontains — opposite: `library`{MOX_FOOTER}"
        ))
    );

    // The opposite mention `library` is a *reference*; hover resolves
    // nothing on references.
    let (line, character) = hover_pos("opposite library", 0);
    let (line, character) = (line, character + "opposite ".len() as u32);
    assert_eq!(hover_at(&mut service, line, character).await, None);

    // The enum literal `Happy`.
    let (line, character) = hover_pos("Happy", 0);
    assert_eq!(
        hover_at(&mut service, line, character).await,
        Some(format!("**Happy** = 0 (Mood){MOX_FOOTER}"))
    );
}

#[tokio::test]
async fn hover_on_an_inline_actors_member_uses_the_graph() {
    const INLINE: &str = "package demo.inline\n\n\
        class Doc {\n\
        \x20   String title\n\
        }\n\n\
        actors Inline {\n\
        \x20   actor Reader\n\
        \x20   capability ReadDoc on Doc\n\n\
        \x20   grant Reader {\n\
        \x20       permit ReadDoc\n\
        \x20   }\n\
        }\n";
    let (mut service, _socket) = initialized_service().await;
    open(&mut service, INLINE).await;

    let (line, character) = hover_pos_in(INLINE, "capability ReadDoc");
    let (line, character) = (line, character + "capability ".len() as u32);
    let markdown = hover_at(&mut service, line, character)
        .await
        .expect("the inline capability hovers");
    assert!(
        markdown.contains("capability Inline.ReadDoc"),
        "the graph node kind and name: {markdown}"
    );
    assert!(markdown.contains("on `"), "its granted class: {markdown}");
    assert!(markdown.contains("permitted to"), "its holders: {markdown}");
    assert!(markdown.contains(".mox"), "provenance: {markdown}");
}

/// Position of `needle`'s start in an arbitrary source text.
fn hover_pos_in(source: &str, needle: &str) -> (u32, u32) {
    let map = PositionMap::new(source);
    let (offset, _) = source
        .match_indices(needle)
        .next()
        .unwrap_or_else(|| panic!("no occurrence of '{needle}'"));
    let position = map.position_for(offset);
    (position.line, position.character)
}

#[tokio::test]
async fn hover_on_an_unlisted_extension_is_null() {
    let (mut service, mut socket) = initialized_service().await;
    let other =
        tower_lsp_server::ls_types::Uri::from_str("file:///workspace/notes.txt").expect("a uri");
    open_at(&mut service, &other, "shelf\n").await;
    drain_socket(&mut socket).await;

    let response = service
        .call(
            jsonrpc::Request::build("textDocument/hover")
                .params(json!({
                    "textDocument": {"uri": other.as_str()},
                    "position": {"line": 0, "character": 1},
                }))
                .id(201)
                .finish(),
        )
        .await
        .unwrap()
        .unwrap();
    let (_, body) = response.into_parts();
    assert!(
        body.expect("hover must not error").is_null(),
        "an unlisted extension has no hover"
    );
}
