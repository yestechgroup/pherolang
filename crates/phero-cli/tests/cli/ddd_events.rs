//! `pherolang` CLI tests: ddd_events.

use std::path::PathBuf;

use crate::support::*;

// --- `.ddd` emission links (pherolang#382 rule 23) --------------------------------

const JOIN_DDD: &str = r#"
import "desk.evt";

application Desk {
    module desk {
        service TicketService {
            boolean raiseRefund(String ticketNo, int cents) emits RefundRequested;
        }
    }
}
"#;

const JOIN_EVT: &str = r#"
event RefundRequested version "1.1.0" {
    ticketNo: String;
    refundCents: int;
}
"#;

#[test]
fn check_resolves_ddd_emission_links_against_a_sibling_evt_artifact() {
    let dir = scratch_dir();
    std::fs::write(dir.join("desk.ddd"), JOIN_DDD).expect("write design");
    std::fs::write(dir.join("desk.evt"), JOIN_EVT).expect("write events");
    let output = pherolang()
        .args(["check", dir.to_str().unwrap()])
        .output()
        .expect("run pherolang check");
    assert!(
        output.status.success(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn check_fails_when_an_emitted_event_is_unknown() {
    let dir = scratch_dir();
    std::fs::write(
        dir.join("desk.ddd"),
        JOIN_DDD.replace("RefundRequested", "RefundRequeste"),
    )
    .expect("write design");
    std::fs::write(dir.join("desk.evt"), JOIN_EVT).expect("write events");
    let output = pherolang()
        .args(["check", dir.to_str().unwrap()])
        .output()
        .expect("run pherolang check");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E0255"), "stderr: {stderr:?}");
    assert!(stderr.contains("RefundRequeste"), "stderr: {stderr:?}");
    assert!(
        stderr.contains("did you mean 'RefundRequested'?"),
        "stderr: {stderr:?}"
    );
}

#[test]
fn build_stamps_the_resolved_contract_on_a_sibling_evt() {
    // `pherolang ir` is deliberately project-free (a standalone compile);
    // project-mode emission is `build`, whose sibling join resolves the
    // emission links and stamps `emittedRefs[].contract`.
    let dir = scratch_dir();
    std::fs::write(dir.join("desk.ddd"), JOIN_DDD).expect("write design");
    std::fs::write(dir.join("desk.evt"), JOIN_EVT).expect("write events");
    let out = dir.join("out");
    let output = pherolang()
        .args(["build", dir.to_str().unwrap(), "-o", out.to_str().unwrap()])
        .output()
        .expect("run pherolang build");
    assert!(
        output.status.success(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    let artifact_path = PathBuf::from(format!(
        "{}/{}.json",
        out.display(),
        dir.join("desk.ddd").display()
    ));
    let artifact = std::fs::read_to_string(artifact_path).expect("design artifact");
    // The stamp is the sibling's bucket key — the path as the host resolved
    // it (absolute here, because the input dir was given absolute).
    let expected = format!("\"contract\": \"{}/desk.evt\"", dir.display());
    assert!(artifact.contains(&expected), "artifact: {artifact:?}");
}
