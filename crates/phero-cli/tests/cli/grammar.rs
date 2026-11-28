//! `pherolang grammar` black-box tests: generation, byte-checking, and the
//! usage/exit-code contract.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use crate::support::*;

fn run(args: &[&str]) -> Output {
    pherolang()
        .arg("grammar")
        .args(args)
        .output()
        .expect("run pherolang grammar")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn path_arg(path: &Path) -> &str {
    path.to_str().expect("utf-8 path")
}

fn fresh_dir(name: &str) -> PathBuf {
    let dir = scratch_dir().join(name);
    let _ = fs::remove_dir_all(&dir);
    dir
}

#[test]
fn out_writes_every_surface_grammar_and_configuration() {
    let dir = fresh_dir("grammar_out");
    let output = run(&["--out", path_arg(&dir)]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("wrote 23 generated files"));

    assert!(dir.join("package.json").is_file());
    for surface in phero_highlight::Surface::ALL {
        let grammar = dir
            .join("grammars")
            .join(format!("{}.tmLanguage.json", surface.id()));
        let configuration = dir
            .join("language-configurations")
            .join(format!("{}.json", surface.id()));
        assert!(grammar.is_file(), "missing {}", grammar.display());
        assert!(
            configuration.is_file(),
            "missing {}",
            configuration.display()
        );

        let grammar_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&grammar).expect("read grammar"))
                .expect("grammar is JSON");
        assert_eq!(grammar_json["name"], surface.id());
        assert_eq!(
            grammar_json["scopeName"],
            format!("source.{}", surface.id())
        );
        assert!(grammar_json["comment"]
            .as_str()
            .expect("comment")
            .contains("do not edit"));

        let configuration_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&configuration).expect("read configuration"))
                .expect("configuration is JSON");
        assert_eq!(configuration_json["comments"]["lineComment"], "//");
    }
}

#[test]
fn check_accepts_a_freshly_written_package() {
    let dir = fresh_dir("grammar_check_clean");
    assert!(run(&["--out", path_arg(&dir)]).status.success());
    let output = run(&["--check", path_arg(&dir)]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("up to date"));
}

#[test]
fn check_rejects_a_hand_edit() {
    let dir = fresh_dir("grammar_check_stale");
    assert!(run(&["--out", path_arg(&dir)]).status.success());
    let grammar = dir.join("grammars/mox.tmLanguage.json");
    let original = fs::read_to_string(&grammar).expect("read grammar");
    fs::write(&grammar, format!("{original} ")).expect("hand edit grammar");

    let output = run(&["--check", path_arg(&dir)]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr(&output);
    assert!(stderr.contains("stale generated file"), "stderr: {stderr}");
    assert!(stderr.contains("mox.tmLanguage.json"), "stderr: {stderr}");
}

#[test]
fn check_rejects_a_missing_and_an_extra_file() {
    let dir = fresh_dir("grammar_check_missing");
    assert!(run(&["--out", path_arg(&dir)]).status.success());
    fs::remove_file(dir.join("language-configurations/cmmn.json")).expect("remove config");
    let output = run(&["--check", path_arg(&dir)]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("missing generated file"));

    assert!(run(&["--out", path_arg(&dir)]).status.success());
    fs::write(
        dir.join("grammars/legacy.tmLanguage.json"),
        "{\"name\":\"legacy\"}\n",
    )
    .expect("add extra grammar");
    let output = run(&["--check", path_arg(&dir)]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("unexpected generated file"));
}

#[test]
fn out_and_check_are_mutually_exclusive_and_one_is_required() {
    let output = run(&[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("--out"));

    let dir = scratch_dir().join("grammar_both");
    let output = run(&["--out", path_arg(&dir), "--check", path_arg(&dir)]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("exactly one"));
}
