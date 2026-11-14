//! LSP server tests: capability_joins.

//! The behavior tests: in-process via `LspService` and a client
//! socket (`tower-lsp` 0.20). The client channel has capacity one —
//! tests must interleave reads (`next_publish`/`drain_socket`) with
//! opens or the server's publish send deadlocks the handler.

use super::*;

use serde_json::json;
use tower::{Service, ServiceExt};
use tower_lsp_server::jsonrpc;

// --- compile-time capability joins (rexlang#128) -----------------------------

/// A scratch directory the sibling joins read from disk: the LSP resolves
/// siblings from the open document's directory (open documents win over
/// disk), so these tests need real files.
fn scratch_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rex-lsp-128-tests-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn url_for(path: &std::path::Path) -> Uri {
    Uri::from_file_path(path).unwrap()
}

/// The design surface: one operation carrying a capability.
const JOIN_MOX: &str = "package join\n\nclass Ticket { String title }\n";

/// The policy: one actor granted the capability, on a class from the
/// domain the policy imports.
const JOIN_ACTOR: &str = "import \"join.mox\"\n\nactors Support {\n    actor Agent\n    capability ApproveTicket on Ticket\n    grant Agent {\n        permit ApproveTicket\n    }\n}\n";

/// The design importing the same domain, its one operation gated on the
/// (typo'd) capability.
fn join_ddd(capability: &str) -> String {
    format!("import \"join.mox\"\n\napplication Join {{\n    module desk {{\n        service TicketService {{\n            boolean approve() capability {capability};\n        }}\n    }}\n}}\n")
}

/// A design whose operation carries no capability clause — the carrier
/// set is empty, so the policy's grant dangles.
const JOIN_DDD_UNCARRIED: &str = "import \"join.mox\"\n\napplication Join {\n    module desk {\n        service TicketService {\n            boolean read() ;\n        }\n    }\n}\n";

/// A design whose operation carries `ApproveTicket` — the carrier the
/// coverage check pools.
const JOIN_DDD_CARRIED: &str = "import \"join.mox\"\n\napplication Join {\n    module desk {\n        service TicketService {\n            boolean approve() capability ApproveTicket;\n        }\n    }\n}\n";

#[tokio::test]
async fn ddd_open_resolves_capabilities_against_sibling_actor_files() {
    let dir = scratch_dir("ddd-sibling");
    std::fs::write(dir.join("join.mox"), JOIN_MOX).expect("write mox");
    std::fs::write(dir.join("team.actor"), JOIN_ACTOR).expect("write actor");
    let design_uri = url_for(&dir.join("desk.ddd"));

    let (mut service, mut socket) = initialized_service().await;
    open_at(&mut service, &design_uri, &join_ddd("ApproveTicke")).await;
    let publish = next_publish(&mut socket, &design_uri)
        .await
        .expect("publishDiagnostics for the design");
    let diagnostic = publish
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("unknown capability 'ApproveTicke'")
        })
        .expect("the unknown capability publishes");
    assert!(
        diagnostic.message.contains("did you mean 'ApproveTicket'?"),
        "a near-miss names its intended capability: {diagnostic:?}"
    );
    assert_eq!(
        diagnostic.code,
        Some(NumberOrString::String("unknownCapability".to_string()))
    );
    let data = diagnostic
        .data
        .as_ref()
        .expect("the code payload rides along");
    assert_eq!(data["unknownCapability"]["suggestion"], "ApproveTicket");
}

#[tokio::test]
async fn ddd_open_without_actor_siblings_publishes_no_capability_diagnostics() {
    let dir = scratch_dir("ddd-no-sibling");
    std::fs::write(dir.join("join.mox"), JOIN_MOX).expect("write mox");
    let design_uri = url_for(&dir.join("desk.ddd"));

    let (mut service, mut socket) = initialized_service().await;
    open_at(&mut service, &design_uri, &join_ddd("Anything")).await;
    let publish = next_publish(&mut socket, &design_uri)
        .await
        .expect("publishDiagnostics for the design");
    assert!(
        publish.diagnostics.is_empty(),
        "no actor artifacts: the join is silent (opt-in house style): {publish:?}"
    );
}

#[tokio::test]
async fn actor_open_publishes_grant_coverage_diagnostics() {
    let dir = scratch_dir("actor-coverage");
    std::fs::write(dir.join("join.mox"), JOIN_MOX).expect("write mox");
    std::fs::write(dir.join("desk.ddd"), JOIN_DDD_UNCARRIED).expect("write design");
    let actor_uri = url_for(&dir.join("team.actor"));

    let (mut service, mut socket) = initialized_service().await;
    open_at(&mut service, &actor_uri, JOIN_ACTOR).await;
    let publish = next_publish(&mut socket, &actor_uri)
        .await
        .expect("publishDiagnostics for the policy");
    let diagnostic = publish
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.message.contains(
                "capability 'ApproveTicket' is granted but no operation or search carries it",
            )
        })
        .expect("the dangling grant publishes");
    assert_eq!(
        diagnostic.code,
        Some(NumberOrString::String(
            "grantedCapabilityUncarried".to_string()
        ))
    );
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
}

#[tokio::test]
async fn actor_open_with_a_carrier_publishes_no_diagnostics() {
    let dir = scratch_dir("actor-carried");
    std::fs::write(dir.join("join.mox"), JOIN_MOX).expect("write mox");
    std::fs::write(dir.join("desk.ddd"), JOIN_DDD_CARRIED).expect("write design");
    let actor_uri = url_for(&dir.join("team.actor"));

    let (mut service, mut socket) = initialized_service().await;
    open_at(&mut service, &actor_uri, JOIN_ACTOR).await;
    let publish = next_publish(&mut socket, &actor_uri)
        .await
        .expect("publishDiagnostics for the policy");
    assert!(
        publish.diagnostics.is_empty(),
        "the grant has a carrier: {publish:?}"
    );
}

#[tokio::test]
async fn code_action_on_an_unknown_capability_offers_the_rename() {
    let dir = scratch_dir("ddd-quickfix");
    std::fs::write(dir.join("join.mox"), JOIN_MOX).expect("write mox");
    std::fs::write(dir.join("team.actor"), JOIN_ACTOR).expect("write actor");
    let design_uri = url_for(&dir.join("desk.ddd"));

    let (mut service, mut socket) = initialized_service().await;
    open_at(&mut service, &design_uri, &join_ddd("ApproveTicke")).await;
    let publish = next_publish(&mut socket, &design_uri)
        .await
        .expect("publishDiagnostics for the design");
    let diagnostic = publish
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("unknown capability 'ApproveTicke'")
        })
        .cloned()
        .expect("the unknown capability publishes");

    let response = service
        .ready()
        .await
        .unwrap()
        .call(
            jsonrpc::Request::build("textDocument/codeAction")
                .params(json!({
                    "textDocument": {"uri": design_uri.as_str()},
                    "range": diagnostic.range,
                    "context": {"diagnostics": [diagnostic.clone()]},
                }))
                .id(600)
                .finish(),
        )
        .await
        .unwrap()
        .unwrap();
    let (_, body) = response.into_parts();
    let value = body.expect("codeAction must not error");
    let actions: Vec<CodeActionOrCommand> =
        serde_json::from_value(value).expect("a CodeActionResponse");
    let actions: Vec<CodeAction> = actions
        .into_iter()
        .map(|action| match action {
            CodeActionOrCommand::CodeAction(action) => action,
            CodeActionOrCommand::Command(_) => panic!("expected code actions"),
        })
        .collect();
    assert_eq!(actions.len(), 1, "one rename fix: {actions:?}");
    assert_eq!(actions[0].title, "Rename to `ApproveTicket`");
    assert_eq!(actions[0].kind, Some(CodeActionKind::QUICKFIX));
    let changes = actions[0].edit.as_ref().unwrap().changes.as_ref().unwrap();
    let edit = &changes.get(&design_uri).expect("edits for the design")[0];
    assert_eq!(edit.new_text, "ApproveTicket");
    assert_eq!(
        edit.range, diagnostic.range,
        "the edit covers the reference"
    );
}

// --- application-scoped joins (rexlang#249) ----------------------------------
//
// The compile-time joins above pool the open document's **import-connected
// application** (`rex_driver::applications`), never its directory: a `.mox`
// both files import is the bridge that joins them, and two applications in
// one directory without an import edge stay separate. An application with no
// counterpart surface keeps the plain compile behavior — the join is silent
// (opt-in house style), which is what makes the negative cases publish
// nothing rather than fail.

/// The desk application's domain.
const SCOPED_DESK_MOX: &str = "package desk\n\nclass Ticket { String title }\n";

/// The shop application's domain — no import edge to desk.
const SCOPED_SHOP_MOX: &str = "package shop\n\nclass Order { String orderNo }\n";

/// A design over `domain` whose one operation carries `capability`.
fn scoped_design(domain: &str, application: &str, capability: &str) -> String {
    format!("import \"{domain}\"\n\napplication {application} {{\n    module desk {{\n        service TicketService {{\n            boolean act() capability {capability};\n        }}\n    }}\n}}\n")
}

/// A design over `domain` whose one operation carries no capability.
fn scoped_uncarried_design(domain: &str, application: &str) -> String {
    format!("import \"{domain}\"\n\napplication {application} {{\n    module desk {{\n        service TicketService {{\n            boolean act() ;\n        }}\n    }}\n}}\n")
}

/// A design over `domain` invoking the decision service `service`.
fn scoped_decision_design(domain: &str, application: &str, service: &str) -> String {
    format!("import \"{domain}\";\n\napplication {application} {{\n    module desk {{\n        service TicketService {{\n            boolean act(String ticketNo, int cents) invokes {service};\n        }}\n    }}\n}}\n")
}

/// A policy over `domain` granting `capability`.
fn scoped_policy(domain: &str, capability: &str) -> String {
    format!("import \"{domain}\"\n\nactors Policy {{\n    actor Agent\n    capability {capability} on Ticket\n    grant Agent {{\n        permit {capability}\n    }}\n}}\n")
}

/// A decision model over `domain` declaring one service.
fn scoped_decision_model(domain: &str, service: &str) -> String {
    format!("import \"{domain}\"\n\ndecision_model Decisions {{\n    decisions {{\n        decision outcome {{\n            output boolean;\n        }}\n    }}\n\n    services {{\n        service {service} {{\n            outputs {{ outcome }}\n            encapsulates {{ outcome }}\n        }}\n    }}\n}}\n")
}

/// Negative: a policy in another application never supplies the open
/// design's capability pool (the `examples/orders.ddd` failure — scoped, the
/// shop application has no policy of its own and the join is silent).
#[tokio::test]
async fn two_unrelated_applications_in_one_directory_do_not_share_capabilities() {
    let dir = scratch_dir("scoped-capabilities");
    std::fs::write(dir.join("desk.mox"), SCOPED_DESK_MOX).expect("write desk mox");
    std::fs::write(
        dir.join("desk.actor"),
        scoped_policy("desk.mox", "ApproveTicket"),
    )
    .expect("write desk policy");
    std::fs::write(dir.join("shop.mox"), SCOPED_SHOP_MOX).expect("write shop mox");
    let design_uri = url_for(&dir.join("shop.ddd"));

    let (mut service, mut socket) = initialized_service().await;
    open_at(
        &mut service,
        &design_uri,
        &scoped_design("shop.mox", "Shop", "PlaceOrder"),
    )
    .await;
    let publish = next_publish(&mut socket, &design_uri)
        .await
        .expect("publishDiagnostics for the design");
    assert!(
        publish.diagnostics.is_empty(),
        "the desk policy must not pool into the shop design: {publish:?}"
    );
}

/// Positive: the pool still works inside one application — the design and
/// the policy import the same domain (the bridge), so the typo is reported
/// against the sibling's capability with its did-you-mean.
#[tokio::test]
async fn an_imported_sibling_actor_still_pools() {
    let dir = scratch_dir("scoped-actor-pool");
    std::fs::write(dir.join("desk.mox"), SCOPED_DESK_MOX).expect("write desk mox");
    std::fs::write(
        dir.join("desk.actor"),
        scoped_policy("desk.mox", "ApproveTicket"),
    )
    .expect("write desk policy");
    let design_uri = url_for(&dir.join("desk.ddd"));

    let (mut service, mut socket) = initialized_service().await;
    open_at(
        &mut service,
        &design_uri,
        &scoped_design("desk.mox", "Desk", "ApproveTicke"),
    )
    .await;
    let publish = next_publish(&mut socket, &design_uri)
        .await
        .expect("publishDiagnostics for the design");
    let diagnostic = publish
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("unknown capability 'ApproveTicke'")
        })
        .expect("the pooled capability set rejects the typo");
    assert!(
        diagnostic.message.contains("did you mean 'ApproveTicket'?"),
        "the sibling policy's name is the suggestion: {diagnostic:?}"
    );
}

/// Negative: a decision model in another application never joins the open
/// design's decisions bucket, so the `invokes` name stays unvalidated.
#[tokio::test]
async fn two_unrelated_applications_in_one_directory_do_not_share_decision_services() {
    let dir = scratch_dir("scoped-decisions");
    std::fs::write(dir.join("desk.mox"), SCOPED_DESK_MOX).expect("write desk mox");
    std::fs::write(
        dir.join("desk.dmn"),
        scoped_decision_model("desk.mox", "AssessPayout"),
    )
    .expect("write desk decisions");
    std::fs::write(dir.join("shop.mox"), SCOPED_SHOP_MOX).expect("write shop mox");
    let design_uri = url_for(&dir.join("shop.ddd"));

    let (mut service, mut socket) = initialized_service().await;
    open_at(
        &mut service,
        &design_uri,
        &scoped_decision_design("shop.mox", "Shop", "AssessRefund"),
    )
    .await;
    let publish = next_publish(&mut socket, &design_uri)
        .await
        .expect("publishDiagnostics for the design");
    assert!(
        publish.diagnostics.is_empty(),
        "the desk decisions must not pool into the shop design: {publish:?}"
    );
}

/// Positive: rule 17 still resolves inside one application — the design's
/// `invokes` name is checked against the sibling decision model's services.
#[tokio::test]
async fn an_imported_sibling_decision_model_still_pools() {
    let dir = scratch_dir("scoped-decision-pool");
    std::fs::write(dir.join("desk.mox"), SCOPED_DESK_MOX).expect("write desk mox");
    std::fs::write(
        dir.join("desk.dmn"),
        scoped_decision_model("desk.mox", "AssessRefund"),
    )
    .expect("write desk decisions");
    let design_uri = url_for(&dir.join("desk.ddd"));

    let (mut service, mut socket) = initialized_service().await;
    open_at(
        &mut service,
        &design_uri,
        &scoped_decision_design("desk.mox", "Desk", "AssessNothing"),
    )
    .await;
    let publish = next_publish(&mut socket, &design_uri)
        .await
        .expect("publishDiagnostics for the design");
    let diagnostic = publish
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("E0250"))
        .expect("the unknown decision service publishes");
    assert!(
        diagnostic
            .message
            .contains("the decision models declare: AssessRefund"),
        "the sibling decision model's service is the pool: {diagnostic:?}"
    );
}

/// Negative: a design in another application never carries the grant, so the
/// policy's grant dangles inside its own application.
#[tokio::test]
async fn two_unrelated_applications_in_one_directory_do_not_share_grant_carriers() {
    let dir = scratch_dir("scoped-carriers");
    std::fs::write(dir.join("desk.mox"), SCOPED_DESK_MOX).expect("write desk mox");
    std::fs::write(
        dir.join("desk.ddd"),
        scoped_uncarried_design("desk.mox", "Desk"),
    )
    .expect("write desk design");
    std::fs::write(
        dir.join("desk.actor"),
        scoped_policy("desk.mox", "ApproveTicket"),
    )
    .expect("write desk policy");
    std::fs::write(dir.join("shop.mox"), SCOPED_SHOP_MOX).expect("write shop mox");
    std::fs::write(
        dir.join("shop.ddd"),
        scoped_design("shop.mox", "Shop", "ApproveTicket"),
    )
    .expect("write shop design");
    let actor_uri = url_for(&dir.join("desk.actor"));

    let (mut service, mut socket) = initialized_service().await;
    open_at(
        &mut service,
        &actor_uri,
        &scoped_policy("desk.mox", "ApproveTicket"),
    )
    .await;
    let publish = next_publish(&mut socket, &actor_uri)
        .await
        .expect("publishDiagnostics for the policy");
    assert!(
        publish
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "capability 'ApproveTicket' is granted but no operation or search carries it"
            )),
        "the shop design must not carry the desk grant: {publish:?}"
    );
}

/// Positive: a design in the same application still carries the grant.
#[tokio::test]
async fn an_imported_sibling_design_still_carries_grants() {
    let dir = scratch_dir("scoped-grant-carried");
    std::fs::write(dir.join("desk.mox"), SCOPED_DESK_MOX).expect("write desk mox");
    std::fs::write(
        dir.join("desk.ddd"),
        scoped_design("desk.mox", "Desk", "ApproveTicket"),
    )
    .expect("write desk design");
    let actor_uri = url_for(&dir.join("desk.actor"));

    let (mut service, mut socket) = initialized_service().await;
    open_at(
        &mut service,
        &actor_uri,
        &scoped_policy("desk.mox", "ApproveTicket"),
    )
    .await;
    let publish = next_publish(&mut socket, &actor_uri)
        .await
        .expect("publishDiagnostics for the policy");
    assert!(
        publish.diagnostics.is_empty(),
        "the sibling design carries the grant: {publish:?}"
    );
}
