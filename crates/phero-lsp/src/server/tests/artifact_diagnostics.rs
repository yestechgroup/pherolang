//! LSP server tests: diagnostics for the artifact surfaces (pherolang#255).
//!
//! `.evt`, `.deploy`, `.uc` and `.cmmn` compile **only** inside the graph
//! snapshot's per-file memo (pherolang#254), so the publish pipeline reads the
//! diagnostics that memoized compile already stored — it never runs a second
//! compile. The tests assert exactly that: each broken document publishes its
//! compiler diagnostic with the offending token's range, a fix clears the
//! stale set, a same-text change adds **zero** compiles (the #254 counter),
//! and a document with no directory (a non-file URI) publishes nothing rather
//! than a bogus empty set.
//!
//! The in-process harness has a capacity-one client socket: every open and
//! change is joined with the publish read it provokes, the existing
//! `open_at`/`next_publish` discipline.

use super::*;

use serde_json::json;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tower::{Service, ServiceExt};
use tower_lsp_server::jsonrpc;
use tower_lsp_server::LspService;

// --- the fixtures ----------------------------------------------------------

/// A broken `.evt`: the channel publishes an event the file never declares.
const BROKEN_EVT: &str = r#"event RefundRequested {
    ticketNo: String;
}

channel refunds {
    publishes RefundApproved;
}
"#;

/// The same contract, fixed: the published event is declared.
const CLEAN_EVT: &str = r#"event RefundRequested {
    ticketNo: String;
}

channel refunds {
    publishes RefundRequested;
}
"#;

/// A broken `.deploy`: the connection names a component the application does
/// not declare.
const BROKEN_DEPLOY: &str = r#"application Demo {
    component api: api {}
    connects api -> web
}
"#;

/// A broken `.uc`: a succession names a node the use case never declares.
const BROKEN_UC: &str = r#"usecase Demo {
    first start then missing;
}
"#;

/// A broken `.cmmn`: the role binds an actor no provided policy declares.
const BROKEN_CMMN: &str = r#"case Demo {
    roles {
        role triage as Nobody;
    }
}
"#;

// --- the scratch directory ------------------------------------------------

/// A temp directory holding one fixture; removed on drop.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "phero-lsp-artifact-diagnostics-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("a clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("mkdir");
        Scratch { dir }
    }

    fn write(&self, name: &str, text: &str) {
        std::fs::write(self.dir.join(name), text)
            .unwrap_or_else(|error| panic!("write {name}: {error}"));
    }

    fn uri(&self, name: &str) -> Uri {
        Uri::from_file_path(self.dir.join(name)).expect("a file uri")
    }

    fn text(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.join(name))
            .unwrap_or_else(|error| panic!("read {name}: {error}"))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

// --- request helpers -------------------------------------------------------

/// Opens one scratch file and returns the publish it provokes (the open and
/// the read are joined: the client socket holds one message).
async fn open_file(
    service: &mut LspService<PheroBackend>,
    socket: &mut tower_lsp_server::ClientSocket,
    scratch: &Scratch,
    name: &str,
) -> PublishDiagnosticsParams {
    let uri = scratch.uri(name);
    let text = scratch.text(name);
    let open = open_at(service, &uri, &text);
    let publish = next_publish(socket, &uri);
    let ((), publish) = tokio::join!(open, publish);
    drain_socket(socket).await;
    publish.unwrap_or_else(|| panic!("the open of {name} publishes diagnostics"))
}

/// Replaces one open document's text (a full-sync `didChange`) and returns
/// the publish it provokes.
async fn change_file(
    service: &mut LspService<PheroBackend>,
    socket: &mut tower_lsp_server::ClientSocket,
    uri: &Uri,
    text: &str,
) -> PublishDiagnosticsParams {
    let change = service.ready().await.unwrap().call(
        jsonrpc::Request::build("textDocument/didChange")
            .params(json!({
                "textDocument": {"uri": uri.as_str(), "version": 2},
                "contentChanges": [{"text": text}],
            }))
            .finish(),
    );
    let publish = next_publish(socket, uri);
    let ((), publish) = tokio::join!(
        async {
            change.await.unwrap();
        },
        publish
    );
    drain_socket(socket).await;
    publish.expect("the change publishes diagnostics")
}

/// The published diagnostic whose message contains `fragment`, asserted to
/// carry a non-empty range (a real span, not the whole-file fallback).
fn diagnostic_with<'a>(publish: &'a PublishDiagnosticsParams, fragment: &str) -> &'a Diagnostic {
    let found = publish
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains(fragment))
        .unwrap_or_else(|| {
            panic!(
                "no diagnostic containing {fragment:?}: {:?}",
                publish.diagnostics
            )
        });
    assert_ne!(
        found.range.start, found.range.end,
        "the diagnostic carries the offending token's range: {found:?}"
    );
    assert_eq!(
        found.source.as_deref(),
        Some("pherolang"),
        "diagnostics are stamped with the server's source"
    );
    assert_eq!(found.severity, Some(DiagnosticSeverity::ERROR));
    found
}

/// The compile count observed so far.
fn compiled(counter: &AtomicUsize) -> usize {
    counter.load(Ordering::SeqCst)
}

// --- one broken document per surface ---------------------------------------

#[tokio::test]
async fn a_broken_event_contract_publishes_its_diagnostics() {
    let scratch = Scratch::new("evt");
    scratch.write("contract.evt", BROKEN_EVT);

    let (mut service, mut socket) = initialized_service().await;
    let publish = open_file(&mut service, &mut socket, &scratch, "contract.evt").await;

    let diagnostic = diagnostic_with(&publish, "unknown event 'RefundApproved'");
    // `publishes RefundApproved;` is line 5 (0-based), the event name at
    // columns 14..28.
    assert_eq!(diagnostic.range.start.line, 5);
    assert_eq!(
        (
            diagnostic.range.start.character,
            diagnostic.range.end.character
        ),
        (14, 28)
    );
}

#[tokio::test]
async fn a_broken_deployment_publishes_its_diagnostics() {
    let scratch = Scratch::new("deploy");
    scratch.write("app.deploy", BROKEN_DEPLOY);

    let (mut service, mut socket) = initialized_service().await;
    let publish = open_file(&mut service, &mut socket, &scratch, "app.deploy").await;

    let diagnostic = diagnostic_with(
        &publish,
        "unknown component 'web' in connection from 'api' to 'web'",
    );
    // `connects api -> web` is line 2 (0-based), the target at columns
    // 20..23.
    assert_eq!(diagnostic.range.start.line, 2);
    assert_eq!(
        (
            diagnostic.range.start.character,
            diagnostic.range.end.character
        ),
        (20, 23)
    );
}

#[tokio::test]
async fn a_broken_use_case_publishes_its_diagnostics() {
    let scratch = Scratch::new("uc");
    scratch.write("story.uc", BROKEN_UC);

    let (mut service, mut socket) = initialized_service().await;
    let publish = open_file(&mut service, &mut socket, &scratch, "story.uc").await;

    let diagnostic = diagnostic_with(&publish, "unknown node 'missing'");
    // `first start then missing;` is line 1 (0-based).
    assert_eq!(diagnostic.range.start.line, 1);
}

#[tokio::test]
async fn a_broken_case_publishes_its_diagnostics() {
    let scratch = Scratch::new("cmmn");
    scratch.write("desk.cmmn", BROKEN_CMMN);

    let (mut service, mut socket) = initialized_service().await;
    let publish = open_file(&mut service, &mut socket, &scratch, "desk.cmmn").await;

    let diagnostic = diagnostic_with(&publish, "unknown actor 'Nobody'");
    // `role triage as Nobody;` is line 2 (0-based).
    assert_eq!(diagnostic.range.start.line, 2);
}

// --- stale clearing --------------------------------------------------------

#[tokio::test]
async fn fixing_a_document_clears_its_stale_diagnostics() {
    let scratch = Scratch::new("stale");
    scratch.write("contract.evt", BROKEN_EVT);

    let (mut service, mut socket) = initialized_service().await;
    let uri = scratch.uri("contract.evt");
    let broken = open_file(&mut service, &mut socket, &scratch, "contract.evt").await;
    diagnostic_with(&broken, "unknown event 'RefundApproved'");

    // The fix moves the document's content key, so the memoized compile runs
    // again and the clean file publishes an empty set — the editor clears
    // its stale squiggles.
    let fixed = change_file(&mut service, &mut socket, &uri, CLEAN_EVT).await;
    assert!(
        fixed.diagnostics.is_empty(),
        "a clean document publishes no diagnostics: {fixed:?}"
    );
}

// --- the memo is the only compile ------------------------------------------

#[tokio::test]
async fn a_same_text_change_adds_no_compile_and_keeps_the_diagnostics() {
    let scratch = Scratch::new("counter");
    scratch.write("contract.evt", BROKEN_EVT);

    let (mut service, mut socket, compile_count) = initialized_service_counted().await;
    let uri = scratch.uri("contract.evt");
    let first = open_file(&mut service, &mut socket, &scratch, "contract.evt").await;
    diagnostic_with(&first, "unknown event 'RefundApproved'");
    let after_open = compiled(&compile_count);
    assert!(
        after_open > 0,
        "the first publish builds the snapshot and compiles its candidates"
    );

    // A version-only `didChange`: the snapshot cache is cleared, but the
    // content key holds, so the rebuild is all memo hits.
    let second = change_file(&mut service, &mut socket, &uri, BROKEN_EVT).await;
    diagnostic_with(&second, "unknown event 'RefundApproved'");
    assert_eq!(
        compiled(&compile_count),
        after_open,
        "publishing the memoized diagnostics compiles nothing"
    );
}

// --- the unavailable snapshot ----------------------------------------------

#[tokio::test]
async fn a_document_uri_without_a_directory_publishes_nothing() {
    // An opaque URI with a known extension is dispatched as that surface but
    // has no file path, hence no directory snapshot: it must publish nothing
    // rather than a bogus empty set.
    let uri = Uri::from_str("untitled:contract.evt").expect("an opaque uri with an extension");
    let (mut service, mut socket) = initialized_service().await;

    let open = open_at(&mut service, &uri, BROKEN_EVT);
    let publish = next_publish(&mut socket, &uri);
    let ((), publish) = tokio::join!(open, publish);
    drain_socket(&mut socket).await;

    let publish = publish.expect("an open always publishes (possibly an empty set)");
    assert!(
        publish.diagnostics.is_empty(),
        "a document with no directory snapshot publishes nothing: {publish:?}"
    );
}
