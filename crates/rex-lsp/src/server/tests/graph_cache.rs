//! LSP server tests: the graph snapshot's per-file compile memo (rexlang#254).
//!
//! Every assertion is on the **compile counter**, never wall-clock time: the
//! memo is keyed by content (own text plus the resolved import closure's
//! text), so a keystroke must recompile exactly the edited file and the files
//! importing it — every other directory sibling stays a memo hit.
//!
//! The scratch directories are self-contained (a domain, an unrelated domain
//! sibling, a design importing the first, and — where the retry case needs a
//! policy — an actor file). The harness has a capacity-one client socket:
//! opens and changes are drained concurrently, the existing
//! `open_at`/`drain_socket` discipline.

use super::*;

use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tower::{Service, ServiceExt};
use tower_lsp_server::jsonrpc;
use tower_lsp_server::LspService;

// --- the scratch directory -------------------------------------------------

/// The domain the design imports; `Ticket` is the cross-file definition the
/// navigation requests land on.
const DOMAIN: &str = r#"package cache.demo

class Ticket {
    String title
}
"#;

/// An unrelated domain: it shares no import with the design, so it must
/// never recompile when the design or its imported domain moves.
const EXTRA: &str = r#"package cache.extra

class Archive {
    String code
}
"#;

/// The design: `entity Ticket` binds to the domain class across files, and
/// the service carries a capability declared by the sibling policy.
const DESIGN: &str = r#"import "domain.mox";

application CacheDesk {
    base cache.demo
    module desk {
        service TicketService {
            boolean readTicket(String ticketNo) capability ReadTicket;
        }
        entity Ticket {
            findById;
        }
    }
}
"#;

/// The policy declaring the capability the design references.
const POLICY: &str = r#"import "domain.mox"

actors Cache {
    actor Agent
    capability ReadTicket on Ticket

    grant Agent {
        permit ReadTicket
    }
}
"#;

/// A policy that does not compile: it contributes no capability node, and
/// the memo retains the failure until its text moves.
const BROKEN_POLICY: &str = "actors Cache {\n    capability\n";

/// A temp directory with the fixture files the test writes; removed on drop.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "rex-lsp-graph-cache-{tag}-{}-{}",
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
        std::fs::write(self.path(name), text)
            .unwrap_or_else(|error| panic!("write {name}: {error}"));
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn uri(&self, name: &str) -> Uri {
        Uri::from_file_path(self.path(name)).expect("a file uri")
    }

    fn text(&self, name: &str) -> String {
        std::fs::read_to_string(self.path(name))
            .unwrap_or_else(|error| panic!("read {name}: {error}"))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Opens one scratch file, draining the socket concurrently (capacity one).
async fn open_file(
    service: &mut LspService<RexBackend>,
    socket: &mut tower_lsp_server::ClientSocket,
    scratch: &Scratch,
    name: &str,
) {
    let uri = scratch.uri(name);
    let text = scratch.text(name);
    let open = open_at(service, &uri, &text);
    let drain = drain_socket(socket);
    tokio::join!(open, drain);
    drain_socket(socket).await;
}

/// Opens one scratch file with arbitrary text (the broken-sibling case).
async fn open_with(
    service: &mut LspService<RexBackend>,
    socket: &mut tower_lsp_server::ClientSocket,
    scratch: &Scratch,
    name: &str,
    text: &str,
) {
    let uri = scratch.uri(name);
    let open = open_at(service, &uri, text);
    let drain = drain_socket(socket);
    tokio::join!(open, drain);
    drain_socket(socket).await;
}

/// Replaces one open document's text (a `didChange` with a full sync),
/// draining the socket concurrently.
async fn change_file(
    service: &mut LspService<RexBackend>,
    socket: &mut tower_lsp_server::ClientSocket,
    scratch: &Scratch,
    name: &str,
    text: &str,
) {
    let change = service.ready().await.unwrap().call(
        jsonrpc::Request::build("textDocument/didChange")
            .params(json!({
                "textDocument": {"uri": scratch.uri(name).as_str(), "version": 2},
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

// --- request helpers -------------------------------------------------------

/// The position of `needle`'s `nth` occurrence, advanced `into` characters.
fn position(text: &str, needle: &str, nth: usize, into: usize) -> Position {
    let (line, character) = pos_of(text, needle, nth);
    Position {
        line,
        character: character + into as u32,
    }
}

/// The location of the name `len` characters long inside `needle`'s `nth`
/// occurrence of `text`.
fn location(text: &str, uri: &Uri, needle: &str, nth: usize, into: usize, len: usize) -> Location {
    let start = position(text, needle, nth, into);
    Location {
        uri: uri.clone(),
        range: Range {
            start,
            end: Position {
                line: start.line,
                character: start.character + len as u32,
            },
        },
    }
}

async fn definition(
    service: &mut LspService<RexBackend>,
    uri: &Uri,
    position: Position,
) -> Option<Location> {
    let response = service
        .call(
            jsonrpc::Request::build("textDocument/definition")
                .params(json!({
                    "textDocument": {"uri": uri.as_str()},
                    "position": {"line": position.line, "character": position.character},
                }))
                .id(400)
                .finish(),
        )
        .await
        .unwrap()
        .unwrap();
    let (_, body) = response.into_parts();
    let value = body.expect("gotoDefinition must not error");
    if value.is_null() {
        return None;
    }
    Some(serde_json::from_value(value).expect("a Location"))
}

/// The compile count observed so far.
fn compiled(counter: &AtomicUsize) -> usize {
    counter.load(Ordering::SeqCst)
}

// --- the memo --------------------------------------------------------------

#[tokio::test]
async fn a_keystroke_recompiles_only_the_changed_file() {
    let scratch = Scratch::new("keystroke");
    scratch.write("domain.mox", DOMAIN);
    scratch.write("extra.mox", EXTRA);
    scratch.write("design.ddd", DESIGN);

    let (mut service, mut socket, compile_count) = initialized_service_counted().await;
    open_file(&mut service, &mut socket, &scratch, "design.ddd").await;

    let design = scratch.text("design.ddd");
    let design_uri = scratch.uri("design.ddd");
    let ticket = position(&design, "entity Ticket", 0, 7);
    assert!(
        definition(&mut service, &design_uri, ticket)
            .await
            .is_some(),
        "the design binds its entity to the domain class"
    );
    let first = compiled(&compile_count);
    assert_eq!(
        first, 3,
        "the first request compiles every candidate (domain, extra, design)"
    );

    // One keystroke in the design: every other candidate's content key holds.
    let edited = design.replace("boolean readTicket", "boolean readTicket2");
    assert_ne!(edited, design, "the edit must change the text");
    change_file(&mut service, &mut socket, &scratch, "design.ddd", &edited).await;

    assert!(
        definition(&mut service, &design_uri, ticket)
            .await
            .is_some(),
        "navigation still answers from the memoized snapshot"
    );
    assert_eq!(
        compiled(&compile_count) - first,
        1,
        "one keystroke recompiles exactly the changed file"
    );
}

#[tokio::test]
async fn editing_an_imported_domain_recompiles_its_dependents() {
    let scratch = Scratch::new("import-closure");
    scratch.write("domain.mox", DOMAIN);
    scratch.write("extra.mox", EXTRA);
    scratch.write("design.ddd", DESIGN);

    let (mut service, mut socket, compile_count) = initialized_service_counted().await;
    open_file(&mut service, &mut socket, &scratch, "design.ddd").await;

    let design = scratch.text("design.ddd");
    let design_uri = scratch.uri("design.ddd");
    let ticket = position(&design, "entity Ticket", 0, 7);
    assert!(definition(&mut service, &design_uri, ticket)
        .await
        .is_some());
    let first = compiled(&compile_count);

    // Edit the imported domain in its open buffer (never saved): the
    // design's own text is untouched, so only the closure key can move it.
    open_file(&mut service, &mut socket, &scratch, "domain.mox").await;
    let domain = scratch.text("domain.mox");
    let edited = format!("{domain}\nclass Archive2 {{\n    String code\n}}\n");
    change_file(&mut service, &mut socket, &scratch, "domain.mox", &edited).await;

    assert!(
        definition(&mut service, &design_uri, ticket)
            .await
            .is_some(),
        "the dependant still resolves through the edited domain"
    );
    assert_eq!(
        compiled(&compile_count) - first,
        2,
        "the imported domain and its dependant recompile; the unrelated sibling does not \
         (an own-text key would recompile only the domain)"
    );
}

#[tokio::test]
async fn a_non_compiling_sibling_contributes_nothing_and_is_retried_when_it_changes() {
    let scratch = Scratch::new("retry");
    scratch.write("domain.mox", DOMAIN);
    scratch.write("design.ddd", DESIGN);
    scratch.write("policy.actor", POLICY);

    let (mut service, mut socket, compile_count) = initialized_service_counted().await;
    open_file(&mut service, &mut socket, &scratch, "design.ddd").await;
    open_with(
        &mut service,
        &mut socket,
        &scratch,
        "policy.actor",
        BROKEN_POLICY,
    )
    .await;

    let design = scratch.text("design.ddd");
    let design_uri = scratch.uri("design.ddd");
    let clause = position(&design, "capability ReadTicket", 0, 11);
    assert_eq!(
        definition(&mut service, &design_uri, clause).await,
        None,
        "a non-compiling sibling contributes no capability node"
    );
    assert!(
        service
            .inner()
            .graph_memo_diagnostics(&scratch.path("policy.actor"))
            .is_some_and(|diagnostics| !diagnostics.is_empty()),
        "the memo retains the failed compile's diagnostics"
    );
    let broken = compiled(&compile_count);

    // Fixing the policy moves its content key: the next request retries it.
    change_file(&mut service, &mut socket, &scratch, "policy.actor", POLICY).await;
    let policy = scratch.text("policy.actor");
    assert_eq!(
        definition(&mut service, &design_uri, clause).await,
        Some(location(
            &policy,
            &scratch.uri("policy.actor"),
            "capability ReadTicket",
            0,
            11,
            10
        )),
        "the fixed policy contributes its capability"
    );
    assert_eq!(
        compiled(&compile_count) - broken,
        1,
        "only the fixed sibling recompiles"
    );
}

#[tokio::test]
async fn memo_prunes_a_file_removed_from_the_directory() {
    let scratch = Scratch::new("prune");
    scratch.write("domain.mox", DOMAIN);
    scratch.write("extra.mox", EXTRA);
    scratch.write("design.ddd", DESIGN);

    let (mut service, mut socket, compile_count) = initialized_service_counted().await;
    open_file(&mut service, &mut socket, &scratch, "design.ddd").await;

    let design = scratch.text("design.ddd");
    let design_uri = scratch.uri("design.ddd");
    let ticket = position(&design, "entity Ticket", 0, 7);
    assert!(definition(&mut service, &design_uri, ticket)
        .await
        .is_some());
    assert_eq!(
        service.inner().graph_memo_len(),
        3,
        "one entry per candidate"
    );
    let extra = scratch.path("extra.mox");
    assert!(service.inner().graph_memo_contains(&extra));

    std::fs::remove_file(&extra).expect("remove extra.mox");
    let before = compiled(&compile_count);

    // A version-only change clears the snapshot cache without moving any
    // content key: the rebuild reuses every entry and prunes the removed
    // file, so memory stays bounded by the directory's candidate set.
    change_file(&mut service, &mut socket, &scratch, "design.ddd", &design).await;
    assert!(definition(&mut service, &design_uri, ticket)
        .await
        .is_some());
    assert_eq!(
        compiled(&compile_count) - before,
        0,
        "no content moved: every surviving entry is a memo hit"
    );
    assert_eq!(
        service.inner().graph_memo_len(),
        2,
        "the removed file's entry is pruned"
    );
    assert!(
        !service.inner().graph_memo_contains(&extra),
        "the memo does not retain artifacts for files gone from the directory"
    );
}
