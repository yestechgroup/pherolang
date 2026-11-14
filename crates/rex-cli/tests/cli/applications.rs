//! `rexlang` CLI tests: the application-scoped compile-time joins
//! (rexlang#249).
//!
//! The rexlang#128 capability pool, the rexlang#78 decision bucket, and the
//! grant-coverage carrier set pool the checked file's **import-connected
//! application** (`rex_driver::applications`), never its directory: a
//! `.mox` both files import is the bridge that joins them, and two
//! applications sharing a directory without an import edge stay separate.
//! A file whose application holds no counterpart surface keeps the plain
//! `compile_*_str` behavior — the join stays silent (opt-in house style),
//! which is what makes the negative cases below succeed rather than fail.

use std::path::Path;

use crate::support::*;

/// The desk application's domain: the class its policy's capability hangs
/// on.
const DESK_MOX: &str = "package desk\n\nclass Ticket { String title }\n";

/// The shop application's domain — no import edge to desk, so the two
/// never share an application however many files share the directory.
const SHOP_MOX: &str = "package shop\n\nclass Order { String orderNo }\n";

/// A design over `domain` whose one operation carries `capability`.
fn design(domain: &str, application: &str, capability: &str) -> String {
    format!(
        "import \"{domain}\"\n\napplication {application} {{\n    module desk {{\n        service TicketService {{\n            boolean act() capability {capability};\n        }}\n    }}\n}}\n"
    )
}

/// A design over `domain` whose one operation carries no capability clause
/// — the empty carrier set.
fn uncarried_design(domain: &str, application: &str) -> String {
    format!(
        "import \"{domain}\"\n\napplication {application} {{\n    module desk {{\n        service TicketService {{\n            boolean act() ;\n        }}\n    }}\n}}\n"
    )
}

/// A design over `domain` whose one operation injects and invokes the
/// decision service `service` (rexlang#78 rule 17).
fn decision_design(domain: &str, application: &str, service: &str) -> String {
    format!(
        "import \"{domain}\";\n\napplication {application} {{\n    module desk {{\n        service TicketService {{\n            inject decision {service};\n            boolean act(String ticketNo, int cents) invokes {service};\n        }}\n    }}\n}}\n"
    )
}

/// A policy over `domain` granting `capability` to one actor.
fn policy(domain: &str, capability: &str) -> String {
    format!(
        "import \"{domain}\"\n\nactors Policy {{\n    actor Agent\n    capability {capability} on Ticket\n    grant Agent {{\n        permit {capability}\n    }}\n}}\n"
    )
}

/// A decision model over `domain` declaring one decision service
/// (`service`). The domain import is what puts it in an application.
fn decision_model(domain: &str, service: &str) -> String {
    format!(
        "import \"{domain}\"\n\ndecision_model Decisions {{\n    decisions {{\n        decision outcome {{\n            output boolean;\n        }}\n    }}\n\n    services {{\n        service {service} {{\n            outputs {{ outcome }}\n            encapsulates {{ outcome }}\n        }}\n    }}\n}}\n"
    )
}

fn write(dir: &Path, name: &str, contents: &str) {
    std::fs::write(dir.join(name), contents).expect("write test source");
}

fn check_directory(dir: &Path) -> std::process::Output {
    rexlang()
        .arg("check")
        .arg(dir)
        .output()
        .expect("run rexlang check")
}

// --- capabilities (rexlang#128's J3 compile-time join) -----------------------

/// Negative: a policy in another application never supplies the design's
/// capability pool. Directory-wide pooling made `examples/orders.ddd` fail
/// on exactly this join (the `PlaceOrder` capability rejected against
/// `support.actor`'s names); scoped, the shop application has no policy of
/// its own and the join stays silent.
#[test]
fn two_unrelated_applications_in_one_directory_do_not_share_capabilities() {
    let dir = scratch_dir();
    write(&dir, "desk.mox", DESK_MOX);
    write(&dir, "desk.actor", &policy("desk.mox", "ApproveTicket"));
    write(&dir, "shop.mox", SHOP_MOX);
    write(
        &dir,
        "orders.ddd",
        &design("shop.mox", "Shop", "PlaceOrder"),
    );

    let output = check_directory(&dir);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the desk policy must not pool into the shop design: {stderr}"
    );
    assert!(
        !stderr.contains("unknown capability"),
        "no capability pool reaches the design: {stderr}"
    );
}

/// Positive: the rexlang#128 pool still works inside one application. The
/// design and the policy import the same domain — the bridge that makes
/// them one application — so the typo resolves against the pooled name.
#[test]
fn an_imported_sibling_actor_still_pools() {
    let dir = scratch_dir();
    write(&dir, "desk.mox", DESK_MOX);
    write(
        &dir,
        "desk.ddd",
        &design("desk.mox", "Desk", "ApproveTicke"),
    );
    write(&dir, "desk.actor", &policy("desk.mox", "ApproveTicket"));

    let output = check_directory(&dir);
    assert_eq!(
        output.status.code(),
        Some(1),
        "one application: the unknown capability fails the batch"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown capability 'ApproveTicke'"),
        "the pooled capability set rejects the typo: {stderr}"
    );
    assert!(
        stderr.contains("did you mean 'ApproveTicket'?"),
        "the sibling policy's name is the suggestion: {stderr}"
    );
}

// --- `.ddd` decision links (rexlang#78 rule 17) ------------------------------

/// Negative: a decision model in another application never joins the
/// design's decisions bucket, so the unvalidated `invokes` name stays
/// silent instead of erroring against a foreign service list.
#[test]
fn two_unrelated_applications_in_one_directory_do_not_share_decision_services() {
    let dir = scratch_dir();
    write(&dir, "desk.mox", DESK_MOX);
    write(
        &dir,
        "desk.dmn",
        &decision_model("desk.mox", "AssessPayout"),
    );
    write(&dir, "shop.mox", SHOP_MOX);
    write(
        &dir,
        "orders.ddd",
        &decision_design("shop.mox", "Shop", "AssessRefund"),
    );

    let output = check_directory(&dir);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the desk decisions must not pool into the shop design: {stderr}"
    );
    assert!(
        !stderr.contains("E0250") && !stderr.contains("E0251"),
        "no decision bucket reaches the design: {stderr}"
    );
}

/// Positive: rule 17 still resolves inside one application — the design's
/// `invokes`/`inject decision` names are checked against the sibling
/// decision model's services (both import the same domain).
#[test]
fn an_imported_sibling_decision_model_still_pools() {
    let dir = scratch_dir();
    write(&dir, "desk.mox", DESK_MOX);
    write(
        &dir,
        "desk.dmn",
        &decision_model("desk.mox", "AssessRefund"),
    );
    write(
        &dir,
        "desk.ddd",
        &decision_design("desk.mox", "Desk", "AssessNothing"),
    );

    let output = check_directory(&dir);
    assert_eq!(
        output.status.code(),
        Some(1),
        "one application: the unknown decision service fails the batch"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("E0250"),
        "the invoked decision service is unresolved: {stderr}"
    );
    assert!(
        stderr.contains("the decision models declare: AssessRefund"),
        "the sibling decision model's service is the pool: {stderr}"
    );
}

// --- `.actor` grant coverage (rexlang#128's J4 compile-time join) ------------

/// Negative: a design in another application never carries the grant. With
/// a carrier only outside the policy's application, the grant dangles —
/// directory-wide pooling hid exactly this finding.
#[test]
fn two_unrelated_applications_in_one_directory_do_not_share_grant_carriers() {
    let dir = scratch_dir();
    write(&dir, "desk.mox", DESK_MOX);
    write(&dir, "desk.ddd", &uncarried_design("desk.mox", "Desk"));
    write(&dir, "desk.actor", &policy("desk.mox", "ApproveTicket"));
    write(&dir, "shop.mox", SHOP_MOX);
    write(
        &dir,
        "orders.ddd",
        &design("shop.mox", "Shop", "ApproveTicket"),
    );

    let output = check_directory(&dir);
    assert_eq!(
        output.status.code(),
        Some(1),
        "the shop design must not carry the desk policy's grant"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "capability 'ApproveTicket' is granted but no operation or search carries it"
        ),
        "the grant dangles inside its own application: {stderr}"
    );
    assert!(
        stderr.contains("desk.actor"),
        "the diagnostic renders against the policy: {stderr}"
    );
}

/// Positive: a design in the same application still carries the grant, so
/// the coverage join stays silent.
#[test]
fn an_imported_sibling_design_still_carries_grants() {
    let dir = scratch_dir();
    write(&dir, "desk.mox", DESK_MOX);
    write(
        &dir,
        "desk.ddd",
        &design("desk.mox", "Desk", "ApproveTicket"),
    );
    write(&dir, "desk.actor", &policy("desk.mox", "ApproveTicket"));

    let output = check_directory(&dir);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the sibling design carries the grant: {stderr}"
    );
    assert!(
        !stderr.contains("carries it"),
        "no dangling grant: {stderr}"
    );
}
