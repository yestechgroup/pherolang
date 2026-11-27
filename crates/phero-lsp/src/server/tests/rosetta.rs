//! LSP server tests: `import sigil` diagnostics (pherolang#398, RN7 of epic
//! pherolang#391).
//!
//! Before this, the editor dropped a `.mox` model whose imported rosetta
//! was broken (or missing) without saying why: the compile's
//! `sigil_diagnostics` were never published, and the graph snapshot's
//! compile key hashed only the *named* rosetta file, not the discovered
//! pool sigil resolution also reads.
//!
//! Rosetta files are read from disk only, so these tests use real scratch
//! directories. The client socket has capacity one: every open/change/close
//! is joined with a concurrent read of the publishes it produces.

use super::*;

use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use tower::{Service, ServiceExt};
use tower_lsp_server::jsonrpc;
use tower_lsp_server::LspService;

/// A `.mox` domain importing one rosetta file.
const DOMAIN: &str = "package rune.demo\n\nimport sigil \"main.rosetta\"\n\nclass Holder {\n    refers rune.main.Order order\n}\n";

/// A clean rosetta file the domain imports by name.
const MAIN: &str = "namespace rune.main\nversion \"1.0.0\"\n\ntype Order:\n\tid string (1..1)\n";

/// A sibling the import does not name — part of the discovered pool.
const SIBLING: &str =
    "namespace rune.sibling\nversion \"1.0.0\"\n\ntype Item:\n\tcode string (1..1)\n";

/// The named file, broken.
const BROKEN_MAIN: &str =
    "namespace rune.main\nversion \"1.0.0\"\n\ntype Order\n\tid string (1..1)\n";

/// A design in the same directory, so a definition request builds the
/// directory's graph snapshot (the compile key's consumer).
const DESIGN: &str = "import \"domain.mox\";\n\napplication RuneDesk {\n    base rune.demo\n    module desk {\n        entity Holder {\n            findById;\n        }\n    }\n}\n";

struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "phero-lsp-398-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("a clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("mkdir");
        // Canonical, so the URIs the server derives (it canonicalizes
        // rosetta paths) match the ones the tests compare against.
        Scratch {
            dir: std::fs::canonicalize(&dir).expect("canonical scratch dir"),
        }
    }

    fn write(&self, name: &str, text: &str) {
        std::fs::write(self.dir.join(name), text)
            .unwrap_or_else(|error| panic!("write {name}: {error}"));
    }

    fn uri(&self, name: &str) -> Uri {
        Uri::from_file_path(self.dir.join(name)).expect("a file uri")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Every `publishDiagnostics` queued until the socket goes quiet.
async fn publishes(socket: &mut tower_lsp_server::ClientSocket) -> Vec<PublishDiagnosticsParams> {
    let mut out = Vec::new();
    while let Ok(Some(message)) =
        tokio::time::timeout(Duration::from_millis(300), socket.next()).await
    {
        if message.method() == "textDocument/publishDiagnostics" {
            out.push(serde_json::from_value(message.params().cloned().unwrap()).unwrap());
        }
    }
    out
}

/// The **last** publish for `uri` among `all` (a later publish supersedes).
fn last_for<'a>(
    all: &'a [PublishDiagnosticsParams],
    uri: &Uri,
) -> Option<&'a PublishDiagnosticsParams> {
    all.iter().rev().find(|publish| &publish.uri == uri)
}

async fn open_collect(
    service: &mut LspService<PheroBackend>,
    socket: &mut tower_lsp_server::ClientSocket,
    uri: &Uri,
    text: &str,
) -> Vec<PublishDiagnosticsParams> {
    let open = open_at(service, uri, text);
    let (_, all) = tokio::join!(open, publishes(socket));
    all
}

async fn change_collect(
    service: &mut LspService<PheroBackend>,
    socket: &mut tower_lsp_server::ClientSocket,
    uri: &Uri,
    version: i32,
    text: &str,
) -> Vec<PublishDiagnosticsParams> {
    let change = service.ready().await.unwrap().call(
        jsonrpc::Request::build("textDocument/didChange")
            .params(json!({
                "textDocument": {"uri": uri.as_str(), "version": version},
                "contentChanges": [{"text": text}],
            }))
            .finish(),
    );
    let (result, all) = tokio::join!(change, publishes(socket));
    result.unwrap();
    all
}

async fn close_collect(
    service: &mut LspService<PheroBackend>,
    socket: &mut tower_lsp_server::ClientSocket,
    uri: &Uri,
) -> Vec<PublishDiagnosticsParams> {
    let close = service.ready().await.unwrap().call(
        jsonrpc::Request::build("textDocument/didClose")
            .params(json!({"textDocument": {"uri": uri.as_str()}}))
            .finish(),
    );
    let (result, all) = tokio::join!(close, publishes(socket));
    result.unwrap();
    all
}

#[tokio::test]
async fn a_clean_rosetta_import_publishes_nothing() {
    let scratch = Scratch::new("clean");
    scratch.write("domain.mox", DOMAIN);
    scratch.write("main.rosetta", MAIN);

    let (mut service, mut socket) = initialized_service().await;
    let mox = scratch.uri("domain.mox");
    let all = open_collect(&mut service, &mut socket, &mox, DOMAIN).await;
    let own = last_for(&all, &mox).expect("the .mox publishes");
    assert!(own.diagnostics.is_empty(), "clean import: {own:?}");
    if let Some(rosetta) = last_for(&all, &scratch.uri("main.rosetta")) {
        assert!(rosetta.diagnostics.is_empty(), "clean rosetta: {rosetta:?}");
    }
}

#[tokio::test]
async fn a_broken_rosetta_publishes_on_its_file_and_anchors_on_the_import() {
    let scratch = Scratch::new("broken");
    scratch.write("domain.mox", DOMAIN);
    scratch.write("main.rosetta", BROKEN_MAIN);

    let (mut service, mut socket) = initialized_service().await;
    let mox = scratch.uri("domain.mox");
    let rosetta = scratch.uri("main.rosetta");
    let all = open_collect(&mut service, &mut socket, &mox, DOMAIN).await;

    let on_rosetta = last_for(&all, &rosetta).expect("the rosetta file gets a publish");
    assert!(
        on_rosetta
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Some(DiagnosticSeverity::ERROR)),
        "the parse error publishes on the rosetta file: {on_rosetta:?}"
    );
    // The rosetta diagnostic is in the rosetta file's coordinates: the
    // missing `:` is on line 3 (0-based), past the start of the file.
    assert!(
        on_rosetta.diagnostics[0].range.start.line >= 3,
        "rosetta coordinates: {on_rosetta:?}"
    );

    let own = last_for(&all, &mox).expect("the .mox publishes");
    let anchor = own
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("imported rosetta file 'main.rosetta' has 1 error")
        })
        .unwrap_or_else(|| panic!("the import carries an anchor: {own:?}"));
    let (line, _) = pos_of(DOMAIN, "import sigil", 0);
    assert_eq!(anchor.range.start.line, line, "anchored at the import");
    let related = anchor
        .related_information
        .as_ref()
        .expect("the anchor links to the rosetta error");
    assert_eq!(related[0].location.uri, rosetta);
}

#[tokio::test]
async fn a_missing_rosetta_publishes_the_not_provided_error_on_the_mox() {
    let scratch = Scratch::new("missing");
    scratch.write("domain.mox", DOMAIN);

    let (mut service, mut socket) = initialized_service().await;
    let mox = scratch.uri("domain.mox");
    let all = open_collect(&mut service, &mut socket, &mox, DOMAIN).await;
    let own = last_for(&all, &mox).expect("the .mox publishes");
    assert!(
        own.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("imported sigil 'main.rosetta' was not provided")),
        "the import-level error is on the .mox: {own:?}"
    );
}

#[tokio::test]
async fn fixing_the_rosetta_clears_its_diagnostics_and_the_anchor() {
    let scratch = Scratch::new("fixed");
    scratch.write("domain.mox", DOMAIN);
    scratch.write("main.rosetta", BROKEN_MAIN);

    let (mut service, mut socket) = initialized_service().await;
    let mox = scratch.uri("domain.mox");
    let rosetta = scratch.uri("main.rosetta");
    let all = open_collect(&mut service, &mut socket, &mox, DOMAIN).await;
    assert!(!last_for(&all, &rosetta).unwrap().diagnostics.is_empty());

    // Rosetta content is read from disk: fix it there, then the next
    // `.mox` compile sees it.
    scratch.write("main.rosetta", MAIN);
    let all = change_collect(&mut service, &mut socket, &mox, 2, DOMAIN).await;
    let on_rosetta = last_for(&all, &rosetta).expect("the fixed file is republished");
    assert!(
        on_rosetta.diagnostics.is_empty(),
        "stale rosetta errors clear: {on_rosetta:?}"
    );
    let own = last_for(&all, &mox).expect("the .mox publishes");
    assert!(own.diagnostics.is_empty(), "the anchor clears: {own:?}");
}

#[tokio::test]
async fn closing_the_mox_clears_the_rosetta_diagnostics_it_owned() {
    let scratch = Scratch::new("close");
    scratch.write("domain.mox", DOMAIN);
    scratch.write("main.rosetta", BROKEN_MAIN);

    let (mut service, mut socket) = initialized_service().await;
    let mox = scratch.uri("domain.mox");
    let rosetta = scratch.uri("main.rosetta");
    open_collect(&mut service, &mut socket, &mox, DOMAIN).await;

    let all = close_collect(&mut service, &mut socket, &mox).await;
    let on_rosetta = last_for(&all, &rosetta).expect("the rosetta file is republished");
    assert!(on_rosetta.diagnostics.is_empty(), "{on_rosetta:?}");
}

#[tokio::test]
async fn an_open_rosetta_keeps_the_diagnostics_routed_to_it() {
    let scratch = Scratch::new("open-rosetta");
    scratch.write("domain.mox", DOMAIN);
    scratch.write("main.rosetta", BROKEN_MAIN);

    let (mut service, mut socket) = initialized_service().await;
    let mox = scratch.uri("domain.mox");
    let rosetta = scratch.uri("main.rosetta");
    open_collect(&mut service, &mut socket, &mox, DOMAIN).await;

    // Opening the rosetta buffer must not publish an empty set over the
    // routed diagnostics (it is not a phero surface).
    let all = open_collect(&mut service, &mut socket, &rosetta, BROKEN_MAIN).await;
    let on_rosetta = last_for(&all, &rosetta).expect("the open publishes");
    assert!(!on_rosetta.diagnostics.is_empty(), "{on_rosetta:?}");
}

// --- the graph snapshot's compile key --------------------------------------

async fn definition(service: &mut LspService<PheroBackend>, uri: &Uri, position: Position) {
    let response = service
        .call(
            jsonrpc::Request::build("textDocument/definition")
                .params(json!({
                    "textDocument": {"uri": uri.as_str()},
                    "position": {"line": position.line, "character": position.character},
                }))
                .id(398)
                .finish(),
        )
        .await
        .unwrap()
        .unwrap();
    let (_, body) = response.into_parts();
    body.expect("gotoDefinition must not error");
}

fn compiled(counter: &AtomicUsize) -> usize {
    counter.load(Ordering::SeqCst)
}

#[tokio::test]
async fn an_edit_to_a_discovered_rosetta_sibling_recompiles_the_importing_mox() {
    let scratch = Scratch::new("pool");
    scratch.write("domain.mox", DOMAIN);
    scratch.write("main.rosetta", MAIN);
    scratch.write("sibling.rosetta", SIBLING);
    scratch.write("design.ddd", DESIGN);

    let (mut service, mut socket, compile_count) = initialized_service_counted().await;
    let design = scratch.uri("design.ddd");
    open_collect(&mut service, &mut socket, &design, DESIGN).await;
    let (line, character) = pos_of(DESIGN, "entity Holder", 0);
    let holder = Position {
        line,
        character: character + 7,
    };
    definition(&mut service, &design, holder).await;
    let baseline = compiled(&compile_count);

    // A no-op change rebuilds the snapshot with every key unchanged.
    change_collect(&mut service, &mut socket, &design, 2, DESIGN).await;
    definition(&mut service, &design, holder).await;
    assert_eq!(
        compiled(&compile_count),
        baseline,
        "nothing moved, nothing recompiles"
    );

    // The sibling is never named by an import, but sigil resolution reads
    // it: editing it on disk must invalidate the importing `.mox`.
    scratch.write(
        "sibling.rosetta",
        &SIBLING.replace("code string", "label string"),
    );
    change_collect(&mut service, &mut socket, &design, 3, DESIGN).await;
    definition(&mut service, &design, holder).await;
    assert_eq!(
        compiled(&compile_count),
        baseline + 1,
        "exactly the importing .mox recompiles"
    );
}
