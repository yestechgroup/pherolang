//! LSP server tests: `.ddd` machine references (issue pherolang#388).
//!
//! The behavior tests: in-process via `LspService` and a client socket
//! (`tower-lsp` 0.20). The client channel has capacity one — tests must
//! interleave reads (`next_publish`/`drain_socket`) with opens or the
//! server's publish send deadlocks the handler. A referenced `.ts` file is
//! not a phero surface: it reads from disk only, so these tests need real
//! files (the sibling-join scratch-dir pattern).

use super::*;

use tower_lsp_server::LspService;

/// A scratch directory the machine files read from disk.
fn scratch_dir(tag: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("phero-lsp-388-tests-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn url_for(path: &std::path::Path) -> Uri {
    Uri::from_file_path(path).unwrap()
}

/// The domain: `PayRun.status: PayRunStatus`.
const REF_MOX: &str = "package nz.example.payroll\n\nenum PayRunStatus {\n    Draft as \"D\" = 0\n    Posted as \"O\" = 1\n}\n\nclass PayRun {\n    PayRunStatus status\n}\n";

/// The design referencing the machine file.
const REF_DDD: &str = "import \"payrun.mox\"\n\napplication Payroll {\n    base nz.example.payroll\n    module payroll {\n        entity PayRun\n        machine status: PayRunStatus from \"machines/payrun.machine.ts\";\n    }\n}\n";

/// The referenced machine: a minimal valid lifecycle.
const GOOD_TS: &str = "import { createMachine } from 'xstate';\n\nexport const payrunMachine = createMachine({\n  id: 'payrun',\n  initial: 'Draft',\n  states: {\n    Draft: { on: { POST: 'Posted' } },\n    Posted: { type: 'final' },\n  },\n});\n";

/// The referenced machine with an inline guard — the fail-closed parser's
/// typed rejection.
const BROKEN_TS: &str = "import { createMachine } from 'xstate';\n\nexport const payrunMachine = createMachine({\n  id: 'payrun',\n  initial: 'Draft',\n  states: {\n    Draft: { on: { POST: { target: 'Posted', guard: ({ context }) => context.ok } } },\n    Posted: { type: 'final' },\n  },\n});\n";

async fn open_design(service: &mut LspService<PheroBackend>, uri: &Uri, text: &str) {
    open_at(service, uri, text).await;
}

#[tokio::test]
async fn a_machine_reference_compiles_clean_in_the_editor() {
    let dir = scratch_dir("clean");
    std::fs::write(dir.join("payrun.mox"), REF_MOX).expect("write mox");
    std::fs::create_dir_all(dir.join("machines")).expect("write machines dir");
    std::fs::write(dir.join("machines/payrun.machine.ts"), GOOD_TS).expect("write ts");
    let design_uri = url_for(&dir.join("payrun.ddd"));

    let (mut service, mut socket) = initialized_service().await;
    open_design(&mut service, &design_uri, REF_DDD).await;
    let publish = next_publish(&mut socket, &design_uri)
        .await
        .expect("publishDiagnostics for the design");
    assert!(
        publish.diagnostics.is_empty(),
        "the referenced machine resolves: {publish:?}"
    );
}

#[tokio::test]
async fn a_missing_machine_file_publishes_the_not_provided_error() {
    let dir = scratch_dir("missing");
    std::fs::write(dir.join("payrun.mox"), REF_MOX).expect("write mox");
    let design_uri = url_for(&dir.join("payrun.ddd"));

    let (mut service, mut socket) = initialized_service().await;
    open_design(&mut service, &design_uri, REF_DDD).await;
    let publish = next_publish(&mut socket, &design_uri)
        .await
        .expect("publishDiagnostics for the design");
    let diagnostic = publish
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("E0403"))
        .expect("the unprovided reference publishes");
    assert!(
        diagnostic
            .message
            .contains("referenced machine file 'machines/payrun.machine.ts' was not provided"),
        "the diagnostic names the written path: {diagnostic:?}"
    );
}

#[tokio::test]
async fn an_edited_machine_file_recompiles_the_design() {
    let dir = scratch_dir("edited");
    std::fs::write(dir.join("payrun.mox"), REF_MOX).expect("write mox");
    std::fs::create_dir_all(dir.join("machines")).expect("write machines dir");
    std::fs::write(dir.join("machines/payrun.machine.ts"), GOOD_TS).expect("write ts");
    let design_uri = url_for(&dir.join("payrun.ddd"));

    let (mut service, mut socket) = initialized_service().await;
    open_design(&mut service, &design_uri, REF_DDD).await;
    let publish = next_publish(&mut socket, &design_uri)
        .await
        .expect("first publish");
    assert!(
        publish.diagnostics.is_empty(),
        "the clean machine compiles clean: {publish:?}"
    );

    // The `.ts` breaks on disk; a `.ts` file is not a phero surface, so
    // the editor learns about it when the design recompiles — the
    // next `didChange` on the design must see the fresh file content and
    // publish the fail-closed rejection.
    std::fs::write(dir.join("machines/payrun.machine.ts"), BROKEN_TS).expect("break ts");
    open_design(&mut service, &design_uri, REF_DDD).await;
    let publish = next_publish(&mut socket, &design_uri)
        .await
        .expect("publishDiagnostics after the edit");
    let diagnostic = publish
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("E0404"))
        .expect("the broken machine publishes");
    assert!(
        diagnostic
            .message
            .contains("referenced machine file 'machines/payrun.machine.ts' did not parse"),
        "the rejection carries the file: {diagnostic:?}"
    );
}
