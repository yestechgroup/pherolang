//! `pherolang` CLI tests: the manifest command and the missing-file error.

use crate::support::*;

#[test]
fn manifest_prints_the_golden_manifest_to_stdout() {
    let path = conformance_fixture("tests/conformance/ifml/app.ifml");
    let golden = std::fs::read_to_string(conformance_fixture(
        "tests/conformance/ifml/app.manifest.json",
    ))
    .expect("read app manifest golden");
    let output = pherolang()
        .args(["manifest", path.to_str().unwrap()])
        .output()
        .expect("run pherolang manifest");
    assert!(
        output.status.success(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{golden}\n"),
        "the CLI manifest must byte-match the committed golden"
    );
}

#[test]
fn manifest_writes_the_golden_manifest_to_an_output_file() {
    let path = conformance_fixture("tests/conformance/ifml/cli.ifml");
    let golden = std::fs::read_to_string(conformance_fixture(
        "tests/conformance/ifml/cli.manifest.json",
    ))
    .expect("read cli manifest golden");
    let out = scratch_dir().join("cli.manifest.json");
    let output = pherolang()
        .args([
            "manifest",
            path.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("run pherolang manifest");
    assert!(
        output.status.success(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).is_empty());
    let json = std::fs::read_to_string(&out).expect("read manifest output");
    assert_eq!(json, golden, "the file manifest must byte-match the golden");
    let manifest = phero_ir::ifml_manifest::IfmlManifest::from_json(&json)
        .expect("the output is a versioned manifest");
    assert!(!manifest.commands.is_empty());
}

/// The `support` example is the decision dimension's regression pin (issue
/// #283): its `data: AssessRefund` names a decision service declared in
/// `support.dmn`, not a domain entity, so the command must check with the
/// decisions-aware pass or it reports E0100 and emits nothing. The emitted
/// bytes must be the committed golden artifact, exactly (the
/// `examples_artifacts` discipline).
#[test]
fn manifest_regenerates_the_support_example_golden() {
    let source = workspace_root().join("examples/support/support.ifml");
    let golden = workspace_root().join("examples/artifacts/support.manifest.json");
    let out = scratch_dir().join("support.manifest.json");
    let output = pherolang()
        .args([
            "manifest",
            source.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("run pherolang manifest");
    assert!(
        output.status.success(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).is_empty());
    let json = std::fs::read_to_string(&out).expect("read manifest output");
    assert_eq!(
        json,
        std::fs::read_to_string(&golden).expect("read support manifest golden"),
        "the support example must regenerate its committed golden byte for byte"
    );
    let manifest =
        phero_ir::ifml_manifest::IfmlManifest::from_json(&json).expect("a versioned manifest");
    assert!(
        manifest
            .controls
            .iter()
            .any(|control| control.data.as_deref() == Some("AssessRefund")),
        "the decision-service data entity rides along as written"
    );
}

/// The same pin for the CLI example, whose manifest carries the command
/// vocabulary and no decision data — the pass must stay a no-op there.
#[test]
fn manifest_regenerates_the_cli_example_golden() {
    let source = workspace_root().join("examples/support/cli.ifml");
    let golden = workspace_root().join("examples/artifacts/cli.manifest.json");
    let out = scratch_dir().join("cli.manifest.json");
    let output = pherolang()
        .args([
            "manifest",
            source.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("run pherolang manifest");
    assert!(
        output.status.success(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = std::fs::read_to_string(&out).expect("read manifest output");
    assert_eq!(
        json,
        std::fs::read_to_string(&golden).expect("read cli manifest golden"),
        "the cli example must regenerate its committed golden byte for byte"
    );
    let manifest =
        phero_ir::ifml_manifest::IfmlManifest::from_json(&json).expect("a versioned manifest");
    assert!(!manifest.commands.is_empty());
}

#[test]
fn manifest_fails_on_a_dangling_reference() {
    // The declaration-backed guarantee at the CLI boundary: the checker
    // gates emission, so the missing-button class never reaches a
    // manifest.
    let path = write_source(
        "dangling.ifml",
        r#"view "List" {
    component "grid" {
        type: list;

        on select(row) -> action("Missing");
    }
}
"#,
    );
    let output = pherolang()
        .args(["manifest", path.to_str().unwrap()])
        .output()
        .expect("run pherolang manifest");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E0104"), "{stderr}");
    assert!(stderr.contains("'Missing'"), "{stderr}");
}

#[test]
fn manifest_requires_an_ifml_file() {
    let path = write_source("not_a_flow.mox", GOOD);
    let output = pherolang()
        .args(["manifest", path.to_str().unwrap()])
        .output()
        .expect("run pherolang manifest");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(".ifml"), "{stderr}");
}

#[test]
fn missing_file_is_a_clean_error() {
    let missing = scratch_dir().join("does-not-exist.mox");
    let output = pherolang()
        .args(["check", missing.to_str().unwrap()])
        .output()
        .expect("run pherolang check");
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("error: cannot read"));
}
