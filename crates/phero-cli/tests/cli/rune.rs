//! `pherolang` CLI tests: `.rosetta` as a surface (RN10, ADR 0012 D7).

use std::path::PathBuf;

use crate::support::*;

/// A two-file Rune project in a fresh scratch directory: `main.rosetta`
/// imports `dep.rosetta`'s namespace, and an unrelated `other.rosetta` sits
/// next to them.
fn rune_project() -> PathBuf {
    let dir = scratch_dir();
    std::fs::write(
        dir.join("main.rosetta"),
        "namespace demo.main\nimport demo.dep.*\n\ntype Trade:\n\tleg Leg (1..1)\n",
    )
    .expect("write main");
    std::fs::write(
        dir.join("dep.rosetta"),
        "namespace demo.dep\n\ntype Leg:\n\tnotional number (1..1)\n",
    )
    .expect("write dep");
    std::fs::write(
        dir.join("other.rosetta"),
        "namespace demo.other\n\ntype Other:\n\tx int (1..1)\n",
    )
    .expect("write other");
    dir
}

fn model_files(stdout: &[u8]) -> Vec<String> {
    let model: serde_json::Value = serde_json::from_slice(stdout).expect("model JSON");
    assert_eq!(model["format"], "sigil-model/1");
    model["files"]
        .as_array()
        .expect("files")
        .iter()
        .map(|file| file["name"].as_str().expect("name").to_string())
        .collect()
}

#[test]
fn check_and_ir_compile_a_rosetta_file_with_its_namespace_closure() {
    let dir = rune_project();
    let main = dir.join("main.rosetta");
    let check = pherolang()
        .args(["check", main.to_str().unwrap()])
        .output()
        .expect("run check");
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&check.stdout),
        format!("OK {}\n", main.display())
    );

    let ir = pherolang()
        .args(["ir", main.to_str().unwrap()])
        .output()
        .expect("run ir");
    assert!(ir.status.success());
    let files = model_files(&ir.stdout);
    assert!(files.contains(&main.display().to_string()), "{files:?}");
    assert!(
        files.contains(&dir.join("dep.rosetta").display().to_string()),
        "the imported namespace's file resolves: {files:?}"
    );
    assert!(
        !files.contains(&dir.join("other.rosetta").display().to_string()),
        "an unrelated sibling is not resolved: {files:?}"
    );
}

#[test]
fn ir_over_several_rosetta_files_resolves_exactly_those() {
    let dir = rune_project();
    let other = dir.join("other.rosetta");
    let dep = dir.join("dep.rosetta");
    let ir = pherolang()
        .args(["ir", other.to_str().unwrap(), dep.to_str().unwrap()])
        .output()
        .expect("run ir");
    assert!(
        ir.status.success(),
        "{}",
        String::from_utf8_lossy(&ir.stderr)
    );
    let files = model_files(&ir.stdout);
    assert!(files.contains(&other.display().to_string()));
    assert!(files.contains(&dep.display().to_string()));
    assert!(!files.contains(&dir.join("main.rosetta").display().to_string()));
}

#[test]
fn a_resolution_error_fails_check_with_the_rune_code() {
    let path = write_source(
        "broken_ref.rosetta",
        "namespace demo.broken\n\ntype A:\n\tx Missing (1..1)\n",
    );
    let output = pherolang()
        .args(["check", path.to_str().unwrap()])
        .output()
        .expect("run check");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(phero_diagnostics::E_RUNE_UNKNOWN_SYMBOL),
        "{stderr}"
    );
}

#[test]
fn a_directory_check_includes_rosetta_files() {
    let dir = rune_project();
    let output = pherolang()
        .args(["check", dir.to_str().unwrap()])
        .output()
        .expect("run check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for name in ["main.rosetta", "dep.rosetta", "other.rosetta"] {
        assert!(stdout.contains(name), "{name} was not checked: {stdout}");
    }
}

#[test]
fn fmt_refuses_a_named_rosetta_file_and_skips_one_in_a_directory() {
    let dir = rune_project();
    let main = dir.join("main.rosetta");
    let named = pherolang()
        .args(["fmt", "--check", main.to_str().unwrap()])
        .output()
        .expect("run fmt");
    assert!(!named.status.success());
    assert!(
        String::from_utf8_lossy(&named.stderr).contains("no Rune (.rosetta) formatter yet"),
        "{}",
        String::from_utf8_lossy(&named.stderr)
    );

    let before = std::fs::read_to_string(&main).unwrap();
    let scanned = pherolang()
        .args(["fmt", dir.to_str().unwrap()])
        .output()
        .expect("run fmt");
    assert!(
        scanned.status.success(),
        "{}",
        String::from_utf8_lossy(&scanned.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(&main).unwrap(),
        before,
        "the .mox formatter never touches a .rosetta file"
    );
}

#[test]
fn verify_refuses_a_rosetta_input_by_name() {
    let dir = rune_project();
    let main = dir.join("main.rosetta");
    let output = pherolang()
        .args(["verify", main.to_str().unwrap(), "-o"])
        .arg(dir.join("receipt.json"))
        .output()
        .expect("run verify");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("does not support .rosetta inputs yet"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
