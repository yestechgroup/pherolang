//! LSP server tests: semantic tokens.
//!
//! Every request here rides the in-process `LspService` and the capacity-one
//! client socket, so each open/change/close is drained (the harness
//! discipline in `graph.rs`).
//!
//! The assertions are made on the **decoded** token stream: [`decode`]
//! reconstructs absolute positions from the wire's delta encoding, proves the
//! stream is sorted, non-overlapping and in bounds, and recovers each token's
//! source text through `String::from_utf16` — the client's own view. The
//! legend is always the advertised one, never a test-local copy.

use super::*;

use serde_json::json;
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, Instant};
use tower::{Service, ServiceExt};
use tower_lsp_server::jsonrpc;
use tower_lsp_server::ls_types::{
    SemanticToken, SemanticTokens, SemanticTokensLegend, SemanticTokensServerCapabilities,
};
use tower_lsp_server::LspService;

// --- harness helpers -------------------------------------------------------

fn doc_uri(name: &str) -> Uri {
    Uri::from_str(&format!("file:///workspace/{name}")).expect("a file uri")
}

/// Opens one document and drains the socket concurrently (capacity one).
async fn open_doc(
    service: &mut LspService<PheroBackend>,
    socket: &mut tower_lsp_server::ClientSocket,
    uri: &Uri,
    text: &str,
) {
    let open = open_at(service, uri, text);
    let drain = drain_socket(socket);
    tokio::join!(open, drain);
    drain_socket(socket).await;
}

/// Replaces one open document's text (a full-sync `didChange`), draining the
/// socket concurrently.
async fn change_doc(
    service: &mut LspService<PheroBackend>,
    socket: &mut tower_lsp_server::ClientSocket,
    uri: &Uri,
    text: &str,
    version: i32,
) {
    let change = service.ready().await.unwrap().call(
        jsonrpc::Request::build("textDocument/didChange")
            .params(json!({
                "textDocument": {"uri": uri.as_str(), "version": version},
                "contentChanges": [{"text": text}],
            }))
            .finish(),
    );
    let drain = drain_socket(socket);
    tokio::join!(
        async {
            change.await.unwrap();
        },
        drain
    );
    drain_socket(socket).await;
}

async fn close_doc(
    service: &mut LspService<PheroBackend>,
    socket: &mut tower_lsp_server::ClientSocket,
    uri: &Uri,
) {
    service
        .call(
            jsonrpc::Request::build("textDocument/didClose")
                .params(json!({"textDocument": {"uri": uri.as_str()}}))
                .finish(),
        )
        .await
        .unwrap();
    drain_socket(socket).await;
}

/// Requests the full semantic tokens, `None` when the server answered null.
async fn tokens_maybe(service: &mut LspService<PheroBackend>, uri: &Uri) -> Option<SemanticTokens> {
    let response = service
        .call(
            jsonrpc::Request::build("textDocument/semanticTokens/full")
                .params(json!({"textDocument": {"uri": uri.as_str()}}))
                .id(900)
                .finish(),
        )
        .await
        .unwrap()
        .unwrap();
    let (_, body) = response.into_parts();
    let value = body.expect("semanticTokens/full must not error");
    if value.is_null() {
        return None;
    }
    Some(serde_json::from_value(value).expect("a SemanticTokens result"))
}

/// The full semantic tokens; panics when the server answered null.
async fn tokens(service: &mut LspService<PheroBackend>, uri: &Uri) -> SemanticTokens {
    tokens_maybe(service, uri)
        .await
        .unwrap_or_else(|| panic!("semanticTokens/full returned null for {}", uri.as_str()))
}

// --- decode helpers --------------------------------------------------------

/// One token as a client sees it after decoding the delta stream.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Decoded {
    line: u32,
    start: u32,
    length: u32,
    type_name: String,
    modifiers: Vec<String>,
    text: String,
}

/// Each line's content (terminators excluded) in UTF-16 code units.
fn line_utf16(text: &str) -> Vec<Vec<u16>> {
    let mut lines = Vec::new();
    for line in text.split_inclusive('\n') {
        let trimmed = line.strip_suffix('\n').unwrap_or(line);
        let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);
        lines.push(trimmed.encode_utf16().collect());
    }
    if lines.is_empty() {
        lines.push(Vec::new());
    }
    lines
}

/// The legend the server advertises (the same one requests are encoded
/// against).
fn server_legend() -> SemanticTokensLegend {
    match crate::server::capabilities()
        .semantic_tokens_provider
        .expect("semantic tokens are advertised")
    {
        SemanticTokensServerCapabilities::SemanticTokensOptions(options) => options.legend,
        SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(_) => {
            panic!("expected the static semantic-tokens options")
        }
    }
}

/// Decodes the delta-encoded stream and proves the client-side invariants.
fn decode(text: &str, legend: &SemanticTokensLegend, data: &[SemanticToken]) -> Vec<Decoded> {
    let lines = line_utf16(text);
    let mut decoded: Vec<Decoded> = Vec::with_capacity(data.len());
    let mut line = 0u32;
    let mut start = 0u32;
    for (index, token) in data.iter().enumerate() {
        if index == 0 {
            line = token.delta_line;
            start = token.delta_start;
        } else {
            line += token.delta_line;
            start = if token.delta_line == 0 {
                start + token.delta_start
            } else {
                token.delta_start
            };
        }
        assert!(
            (line as usize) < lines.len(),
            "token {index} on line {line} but the document has {} lines",
            lines.len()
        );
        let units = &lines[line as usize];
        let end = (start + token.length) as usize;
        assert!(
            end <= units.len(),
            "token {index} overruns line {line}: {start}..{end} of {} UTF-16 units",
            units.len()
        );
        if let Some(previous) = decoded.last() {
            let ordered = previous.line < line
                || (previous.line == line && previous.start + previous.length <= start);
            assert!(
                ordered,
                "token {index} overlaps or is unsorted (previous {previous:?}, line {line} start {start})"
            );
        }
        let type_name = legend
            .token_types
            .get(token.token_type as usize)
            .unwrap_or_else(|| {
                panic!(
                    "token {index} names type {} outside the {}-entry legend",
                    token.token_type,
                    legend.token_types.len()
                )
            })
            .as_str()
            .to_string();
        let modifiers = legend
            .token_modifiers
            .iter()
            .enumerate()
            .filter(|(bit, _)| token.token_modifiers_bitset & (1u32 << bit) != 0)
            .map(|(_, modifier)| modifier.as_str().to_string())
            .collect();
        let text = String::from_utf16(&units[start as usize..end])
            .expect("a token never splits a surrogate pair");
        decoded.push(Decoded {
            line,
            start,
            length: token.length,
            type_name,
            modifiers,
            text,
        });
    }
    decoded
}

/// The token starting at the `nth` occurrence of `needle`, located through
/// the same `PositionMap` the server uses.
fn token_at(decoded: &[Decoded], text: &str, needle: &str, nth: usize) -> Decoded {
    let map = PositionMap::new(text);
    let (offset, _) = text
        .match_indices(needle)
        .nth(nth)
        .unwrap_or_else(|| panic!("no occurrence {nth} of {needle:?}"));
    let position = map.position_for(offset);
    decoded
        .iter()
        .find(|token| token.line == position.line && token.start == position.character)
        .unwrap_or_else(|| {
            panic!(
                "no token at line {} character {} ({needle:?} #{nth}); tokens: {decoded:?}",
                position.line, position.character
            )
        })
        .clone()
}

fn has_modifier(token: &Decoded, modifier: &str) -> bool {
    token.modifiers.iter().any(|name| name == modifier)
}

// --- 1. the legend ---------------------------------------------------------

#[tokio::test]
async fn legend_is_stable_and_standard() {
    let capabilities = crate::server::capabilities();
    assert!(
        capabilities.semantic_tokens_provider.is_some(),
        "the semantic-tokens provider must be advertised"
    );
    let legend = server_legend();
    let types: Vec<&str> = legend.token_types.iter().map(|t| t.as_str()).collect();
    assert_eq!(
        types,
        [
            "keyword",
            "type",
            "function",
            "namespace",
            "variable",
            "parameter",
            "property",
            "string",
            "number",
            "comment",
            "operator",
            "decorator",
            "enumMember",
        ],
        "the legend is the standard-only, deduped, wire-ordered class list"
    );
    assert_eq!(types.len(), 13);
    let modifiers: Vec<&str> = legend.token_modifiers.iter().map(|m| m.as_str()).collect();
    assert_eq!(
        modifiers,
        [
            "declaration",
            "definition",
            "readonly",
            "deprecated",
            "documentation",
            "defaultLibrary",
        ]
    );
    assert_eq!(modifiers.len(), 6);
    assert!(!types.contains(&"punctuation"));
    assert!(!types.contains(&"invalid"));
    assert!(!modifiers.contains(&"punctuation"));
    assert!(!modifiers.contains(&"invalid"));

    // The initialize answer carries exactly this legend on the wire.
    let (mut service, _socket) = LspService::new(PheroBackend::new);
    let response = service
        .ready()
        .await
        .unwrap()
        .call(
            jsonrpc::Request::build("initialize")
                .params(json!({"capabilities": {}}))
                .id(1)
                .finish(),
        )
        .await
        .unwrap()
        .unwrap();
    let result = response.result().unwrap();
    assert_eq!(
        result["capabilities"]["semanticTokensProvider"]["legend"],
        json!({
            "tokenTypes": types,
            "tokenModifiers": modifiers,
        })
    );
    assert_eq!(
        result["capabilities"]["semanticTokensProvider"]["full"],
        json!(true)
    );
}

// --- 2. `.mox` -------------------------------------------------------------

const MOX_DOC: &str = "package demo\n\n/// A book.\nclass Book { String title }\n\nenum Mood { Happy = 0 }\n\nvocabulary Currency from \"iso:4217\"\n";

#[tokio::test]
async fn mox_tokens_cover_declarations_types_strings_numbers_comments_keywords() {
    let (mut service, mut socket) = initialized_service().await;
    let uri = doc_uri("cover.mox");
    open_doc(&mut service, &mut socket, &uri, MOX_DOC).await;
    let result = tokens(&mut service, &uri).await;
    let legend = server_legend();
    let decoded = decode(MOX_DOC, &legend, &result.data);
    assert!(!decoded.is_empty());

    for keyword in ["package", "class", "enum"] {
        let token = token_at(&decoded, MOX_DOC, keyword, 0);
        assert_eq!(token.type_name, "keyword", "{keyword}: {token:?}");
    }

    let book = token_at(&decoded, MOX_DOC, "Book", 0);
    assert_eq!(book.type_name, "type");
    assert!(has_modifier(&book, "declaration"), "{book:?}");

    let string_type = token_at(&decoded, MOX_DOC, "String", 0);
    assert_eq!(string_type.type_name, "type");
    assert!(
        has_modifier(&string_type, "defaultLibrary"),
        "{string_type:?}"
    );

    let title = token_at(&decoded, MOX_DOC, "title", 0);
    assert_eq!(title.type_name, "property");
    assert_eq!(title.text, "title");

    let literal = token_at(&decoded, MOX_DOC, "\"iso:4217\"", 0);
    assert_eq!(literal.type_name, "string");
    assert_eq!(
        literal.text, "\"iso:4217\"",
        "the whole literal is one token"
    );

    let zero = token_at(&decoded, MOX_DOC, "0", 0);
    assert_eq!(zero.type_name, "number");
    assert_eq!(zero.text, "0");

    let comment = token_at(&decoded, MOX_DOC, "/// A book.", 0);
    assert_eq!(comment.type_name, "comment");
    assert!(has_modifier(&comment, "documentation"), "{comment:?}");
    assert_eq!(comment.text, "/// A book.");
}

// --- 3. `.ddd` -------------------------------------------------------------

const DDD_DOC: &str = "application Shop {\n    base demo\n    module desk {\n        service TicketService {\n            Ticket raise(NewTicket draft) capability RaiseRefund;\n        }\n    }\n}\n";

#[tokio::test]
async fn ddd_tokens_cover_application_service_and_operations() {
    let (mut service, mut socket) = initialized_service().await;
    let uri = doc_uri("shop.ddd");
    open_doc(&mut service, &mut socket, &uri, DDD_DOC).await;
    let result = tokens(&mut service, &uri).await;
    let decoded = decode(DDD_DOC, &server_legend(), &result.data);
    assert!(!decoded.is_empty());

    for keyword in ["application", "service", "capability"] {
        let token = token_at(&decoded, DDD_DOC, keyword, 0);
        assert_eq!(token.type_name, "keyword", "{keyword}: {token:?}");
    }

    let application = token_at(&decoded, DDD_DOC, "Shop", 0);
    assert_eq!(application.type_name, "namespace");
    assert!(has_modifier(&application, "declaration"), "{application:?}");

    let service = token_at(&decoded, DDD_DOC, "TicketService", 0);
    assert_eq!(service.type_name, "type");
    assert!(has_modifier(&service, "declaration"), "{service:?}");

    let operation = token_at(&decoded, DDD_DOC, "raise", 0);
    assert_eq!(operation.type_name, "function");
    assert!(has_modifier(&operation, "declaration"), "{operation:?}");

    let capability = token_at(&decoded, DDD_DOC, "RaiseRefund", 0);
    assert_eq!(capability.type_name, "function");
}

// --- 4. `.ifml` ------------------------------------------------------------

const IFML_DOC: &str = "view \"Inbox\" {\n    params { tickets: Ticket };\n    component \"queue\" {\n        type: list;\n        data: Ticket;\n        on select(row) -> navigate(\"Intake\");\n    }\n}\n";

#[tokio::test]
async fn ifml_tokens_cover_view_component_type_data_navigate() {
    let (mut service, mut socket) = initialized_service().await;
    let uri = doc_uri("inbox.ifml");
    open_doc(&mut service, &mut socket, &uri, IFML_DOC).await;
    let result = tokens(&mut service, &uri).await;
    let decoded = decode(IFML_DOC, &server_legend(), &result.data);
    assert!(!decoded.is_empty());

    for keyword in ["view", "component", "navigate"] {
        let token = token_at(&decoded, IFML_DOC, keyword, 0);
        assert_eq!(token.type_name, "keyword", "{keyword}: {token:?}");
    }
    for (name, nth) in [("\"Inbox\"", 0), ("\"queue\"", 0), ("\"Intake\"", 0)] {
        let token = token_at(&decoded, IFML_DOC, name, nth);
        assert_eq!(token.type_name, "type", "{name}: {token:?}");
        assert_eq!(token.text, name);
    }
    for property in ["type", "data"] {
        let token = token_at(&decoded, IFML_DOC, property, 0);
        assert_eq!(token.type_name, "property", "{property}: {token:?}");
    }
    // `Ticket` is a declared type in the `params` clause; the `data:` value is
    // a property expression (`variable`), which is the engine's own contract.
    let ticket = token_at(&decoded, IFML_DOC, "Ticket", 0);
    assert_eq!(ticket.type_name, "type", "{ticket:?}");
    let data_value = token_at(&decoded, IFML_DOC, "Ticket", 1);
    assert_eq!(data_value.type_name, "variable", "{data_value:?}");
}

// --- 5. UTF-16 -------------------------------------------------------------

const CRLF_DOC: &str = "package demo\r\n// 😀 cafe\u{301}\r\nclass Book { String title }\r\n";

#[tokio::test]
async fn utf16_is_correct_for_emoji_combining_marks_and_crlf() {
    let (mut service, mut socket) = initialized_service().await;
    let uri = doc_uri("utf16.mox");
    open_doc(&mut service, &mut socket, &uri, CRLF_DOC).await;
    let result = tokens(&mut service, &uri).await;
    let decoded = decode(CRLF_DOC, &server_legend(), &result.data);
    assert!(!decoded.is_empty());

    let comment = token_at(&decoded, CRLF_DOC, "//", 0);
    assert_eq!(comment.type_name, "comment");
    assert!(
        comment.text.contains('😀'),
        "the emoji survives UTF-16 reconstruction: {comment:?}"
    );
    assert!(comment.text.contains('\u{301}'), "{comment:?}");
    assert!(!comment.text.contains('\r') && !comment.text.contains('\n'));

    // The token after the multi-byte line has the exact UTF-16 position the
    // LSP `PositionMap` computes — the server and the client agree.
    let map = PositionMap::new(CRLF_DOC);
    let (offset, _) = CRLF_DOC
        .match_indices("title")
        .next()
        .expect("the title occurs");
    let expected = map.position_for(offset);
    assert_eq!((expected.line, expected.character), (2, 20));
    let title = token_at(&decoded, CRLF_DOC, "title", 0);
    assert_eq!(
        (title.line, title.start),
        (expected.line, expected.character)
    );
    assert_eq!(title.text, "title");
    assert_eq!(title.length, 5);
}

// --- 6. multi-line comments ------------------------------------------------

const BLOCK_DOC: &str = "/* first\nsecond */\nclass Book { String title }\n";

#[tokio::test]
async fn multi_line_comments_are_split_into_single_line_tokens() {
    let (mut service, mut socket) = initialized_service().await;
    let uri = doc_uri("block.mox");
    open_doc(&mut service, &mut socket, &uri, BLOCK_DOC).await;
    let result = tokens(&mut service, &uri).await;
    let decoded = decode(BLOCK_DOC, &server_legend(), &result.data);

    let comments: Vec<&Decoded> = decoded
        .iter()
        .filter(|token| token.type_name == "comment")
        .collect();
    assert_eq!(comments.len(), 2, "{comments:?}");
    assert_eq!(comments[0].text, "/* first");
    assert_eq!(comments[0].line, 0);
    assert_eq!(comments[1].text, "second */");
    assert_eq!(comments[1].line, 1);
    assert_eq!(comments[1].start, 0);
    for token in &decoded {
        assert!(
            !token.text.contains('\n') && !token.text.contains('\r'),
            "every emitted token is single-line: {token:?}"
        );
    }
}

// --- 7. staleness ----------------------------------------------------------

#[tokio::test]
async fn stale_documents_return_tokens_for_the_current_text() {
    let (mut service, mut socket) = initialized_service().await;
    let uri = doc_uri("stale.mox");
    open_doc(&mut service, &mut socket, &uri, "class Book").await;
    let first = tokens(&mut service, &uri).await;
    let decoded = decode("class Book", &server_legend(), &first.data);
    assert_eq!(
        token_at(&decoded, "class Book", "Book", 0).type_name,
        "type"
    );

    change_doc(&mut service, &mut socket, &uri, "class Shelf", 2).await;
    let second = tokens(&mut service, &uri).await;
    let decoded = decode("class Shelf", &server_legend(), &second.data);
    let shelf = token_at(&decoded, "class Shelf", "Shelf", 0);
    assert_eq!(shelf.type_name, "type");
    assert_eq!(shelf.text, "Shelf");
    assert!(
        decoded.iter().all(|token| token.text != "Book"),
        "the stale document's tokens must not survive: {decoded:?}"
    );

    // An unopened document and a closed document answer null.
    let unopened = doc_uri("never-opened.mox");
    assert!(tokens_maybe(&mut service, &unopened).await.is_none());
    close_doc(&mut service, &mut socket, &uri).await;
    assert!(tokens_maybe(&mut service, &uri).await.is_none());
}

// --- 8. malformed documents ------------------------------------------------

const BROKEN_MOX: &str = "package demo\n\nclass Book { String title\n";
const BROKEN_DDD: &str =
    "application Broken {\n    module desk {\n        service S {\n            ???\n";
const EMOJI_ONLY: &str = "😀";

#[tokio::test]
async fn malformed_documents_never_panic_and_stay_in_bounds() {
    let (mut service, mut socket) = initialized_service().await;
    let cases = [
        ("broken.mox", BROKEN_MOX),
        ("broken.ddd", BROKEN_DDD),
        ("empty.mox", ""),
        ("emoji.mox", EMOJI_ONLY),
    ];
    for (name, text) in cases {
        let uri = doc_uri(name);
        open_doc(&mut service, &mut socket, &uri, text).await;
        let result = tokens(&mut service, &uri).await;
        // `decode` asserts sorted, non-overlapping, in-bounds, and that every
        // token's UTF-16 slice reconstructs.
        let decoded = decode(text, &server_legend(), &result.data);
        for token in &decoded {
            assert!(token.length > 0, "{name}: zero-length token {token:?}");
        }
        if text.is_empty() || text == EMOJI_ONLY {
            assert!(
                decoded.is_empty(),
                "{name}: only punctuation/error classes, which have no token type"
            );
        }
    }

    // A file kind the highlighter does not know answers null, never an error.
    let other = doc_uri("notes.txt");
    open_doc(&mut service, &mut socket, &other, "class Book {}").await;
    assert!(tokens_maybe(&mut service, &other).await.is_none());
}

// --- 9. every surface ------------------------------------------------------

#[tokio::test]
async fn every_surface_yields_tokens_and_stays_in_bounds() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (mut service, mut socket) = initialized_service().await;
    let legend = server_legend();
    for extension in [
        "mox", "ifml", "ddd", "actor", "evt", "deploy", "uc", "cmmn", "dmn",
    ] {
        let name = format!("surface.{extension}");
        let path = root.join(format!("examples/support/support.{extension}"));
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let uri = doc_uri(&name);
        open_doc(&mut service, &mut socket, &uri, &text).await;
        let result = tokens(&mut service, &uri).await;
        let decoded = decode(&text, &legend, &result.data);
        assert!(!decoded.is_empty(), "`{extension}` produced no tokens");
        close_doc(&mut service, &mut socket, &uri).await;
    }
}

// --- 10. performance -------------------------------------------------------

/// A ~400-operation `.ddd` (a large but valid design document).
fn large_ddd() -> String {
    let mut text = String::from(
        "application Big {\n    base demo\n    module desk {\n        service BigService {\n",
    );
    for index in 0..400 {
        text.push_str(&format!(
            "            boolean op{index}(String arg{index}) capability Cap{index};\n"
        ));
    }
    text.push_str("        }\n    }\n}\n");
    text
}

#[tokio::test]
async fn performance_on_support_mox_and_a_large_ddd_fixture() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let support = std::fs::read_to_string(root.join("examples/support/support.mox"))
        .expect("read examples/support/support.mox");
    let large = large_ddd();
    let legend = server_legend();

    for (label, text, surface) in [
        (
            "examples/support/support.mox",
            support.as_str(),
            phero_highlight::Surface::Mox,
        ),
        ("large.ddd", large.as_str(), phero_highlight::Surface::Ddd),
    ] {
        let started = Instant::now();
        let highlight =
            phero_highlight::highlight(text, surface, phero_highlight::HighlightOptions::default());
        let encoded = crate::server::semantic_tokens::encode(text, &highlight.spans);
        let elapsed = started.elapsed();
        assert!(!highlight.spans.is_empty(), "{label}: no highlight spans");
        assert!(!encoded.data.is_empty(), "{label}: no semantic tokens");
        assert!(
            elapsed < Duration::from_secs(5),
            "{label}: highlight + encode took {elapsed:?}"
        );
        let decoded = decode(text, &legend, &encoded.data);
        assert!(!decoded.is_empty(), "{label}");
    }
}
