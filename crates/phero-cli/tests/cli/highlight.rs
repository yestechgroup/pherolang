//! `pherolang highlight` black-box tests: the JSON/HTML encodings, stdin,
//! directory output, and the usage/exit-code contract.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};

use crate::support::*;

fn run(args: &[&str]) -> Output {
    pherolang()
        .arg("highlight")
        .args(args)
        .output()
        .expect("run pherolang highlight")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn json_is_the_default_and_carries_the_contract() {
    let path = write_source("highlight_basic.mox", GOOD);
    let output = run(&[path.to_str().expect("utf-8 path")]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("stdout is JSON");
    assert_eq!(json["formatVersion"], 1);
    assert_eq!(json["surface"], "mox");
    assert_eq!(json["degraded"], false);
    assert!(json["sourceHash"]
        .as_str()
        .expect("sourceHash is a string")
        .starts_with("sha256:"));
    let spans = json["spans"].as_array().expect("spans is an array");
    assert!(!spans.is_empty());
    assert!(spans.iter().any(|span| span["class"] == "keyword"));
}

#[test]
fn html_is_a_deterministic_classed_fragment() {
    let path = write_source("highlight_html.mox", GOOD);
    let output = run(&[path.to_str().expect("utf-8 path"), "--format", "html"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let html = stdout(&output);
    assert!(html.contains("<pre class=\"pherolang-highlight\">"));
    assert!(html.contains("<code class=\"language-mox\">"));
    assert!(html.contains("hl-keyword"));
    assert!(!html.contains("<script>"));
}

#[test]
fn stdin_requires_a_surface() {
    let mut child = pherolang()
        .args(["highlight", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn highlight");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(GOOD.as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait");
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("--surface"));
}

#[test]
fn stdin_is_lenient_by_default_and_never_fails() {
    let mut child = pherolang()
        .args(["highlight", "-", "--surface", "mox"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn highlight");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(b"class Book {")
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait");
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("stdout is JSON");
    assert_eq!(json["degraded"], true);
    assert!(!json["spans"].as_array().expect("spans").is_empty());
}

#[test]
fn several_inputs_require_an_output_directory() {
    let one = write_source("highlight_multi_one.mox", GOOD);
    let two = write_source("highlight_multi_two.mox", GOOD);
    let output = run(&[
        one.to_str().expect("utf-8 path"),
        two.to_str().expect("utf-8 path"),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("--output"));
}

#[test]
fn directory_input_writes_one_artifact_per_file() {
    let dir = scratch_dir().join("highlight_tree");
    std::fs::create_dir_all(&dir).expect("create tree");
    std::fs::write(dir.join("a.mox"), GOOD).expect("write a");
    std::fs::write(dir.join("b.mox"), GOOD).expect("write b");
    let out = scratch_dir().join("highlight_out");
    let output = run(&[
        dir.to_str().expect("utf-8 path"),
        "--output",
        out.to_str().expect("utf-8 path"),
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    for input in [dir.join("a.mox"), dir.join("b.mox")] {
        let expected = mirrored_output(&out, &input, "json");
        assert!(expected.is_file(), "missing {}", expected.display());
    }
}

fn mirrored_output(out: &Path, input: &Path, extension: &str) -> PathBuf {
    let suffixed = format!("{}.{extension}", input.display());
    let mut path = out.to_path_buf();
    for component in Path::new(&suffixed).components() {
        match component {
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {}
            component => path.push(component.as_os_str()),
        }
    }
    path
}

// --- goldens -----------------------------------------------------------------

const EXAMPLES: &[(&str, &str)] = &[
    ("mox", "support.mox"),
    ("ifml", "support.ifml"),
    ("ddd", "support.ddd"),
    ("actor", "support.actor"),
    ("evt", "support.evt"),
    ("deploy", "support.deploy"),
    ("uc", "support.uc"),
    ("cmmn", "support.cmmn"),
    ("dmn", "support.dmn"),
];

fn golden_dir() -> PathBuf {
    workspace_root().join("tests/highlight")
}

fn golden_or_update(path: &Path, generated: &str) -> String {
    if std::env::var("PHERO_UPDATE_FIXTURES").is_ok() {
        std::fs::write(path, generated).unwrap_or_else(|error| {
            panic!("write golden {}: {error}", path.display());
        });
        eprintln!("updated {}", path.display());
        return generated.to_string();
    }
    std::fs::read_to_string(path).unwrap_or_else(|error| {
        panic!(
            "read golden {}: {error} — regenerate with PHERO_UPDATE_FIXTURES=1 \
             cargo test -p phero-cli --test cli highlight",
            path.display()
        );
    })
}

fn render_file(input: &Path, format: &str) -> String {
    let out = scratch_dir().join(format!(
        "{}-{}.{format}",
        input
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("input"),
        format
    ));
    let output = run(&[
        input.to_str().expect("utf-8 path"),
        "--format",
        format,
        "--output",
        out.to_str().expect("utf-8 output path"),
    ]);
    assert!(
        output.status.success(),
        "highlight {} failed: {}",
        input.display(),
        stderr(&output)
    );
    std::fs::read_to_string(&out).expect("read highlight output")
}

#[test]
fn json_goldens_for_every_support_example() {
    for (surface, file) in EXAMPLES {
        let input = workspace_root().join("examples/support").join(file);
        let generated = render_file(&input, "json");
        let golden = golden_dir()
            .join("json")
            .join(format!("support.{surface}.json"));
        assert_eq!(golden_or_update(&golden, &generated), generated);
    }
}

#[test]
fn html_goldens_for_one_snippet_per_surface() {
    for (surface, _) in EXAMPLES {
        let input = golden_dir()
            .join("snippets")
            .join(format!("{surface}.{surface}"));
        let generated = render_file(&input, "html");
        let golden = golden_dir().join("html").join(format!("{surface}.html"));
        assert_eq!(golden_or_update(&golden, &generated), generated);
    }
}

#[test]
fn a_docs_fence_through_stdin_is_a_golden() {
    let source = "package demo\nclass Book { String title }\n";
    let out = scratch_dir().join("highlight-fence.json");
    let mut child = pherolang()
        .args([
            "highlight",
            "-",
            "--surface",
            "mox",
            "--format",
            "json",
            "--output",
            out.to_str().expect("utf-8 output path"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn highlight");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(source.as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait");
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let generated = std::fs::read_to_string(&out).expect("read fence output");
    let golden = golden_dir().join("json/fence.mox.json");
    assert_eq!(golden_or_update(&golden, &generated), generated);
}

#[test]
fn highlighting_is_deterministic() {
    let input = workspace_root().join("examples/support/support.mox");
    let first = render_file(&input, "json");
    let second = render_file(&input, "json");
    assert_eq!(first, second);
}

#[test]
fn json_never_carries_a_machine_path() {
    let input = workspace_root().join("examples/support/support.mox");
    let json = render_file(&input, "json");
    let root = workspace_root().display().to_string();
    assert!(!json.contains(&root), "artifact leaked a machine path");
    assert!(
        !json.contains("\"/home/"),
        "artifact leaked an absolute path"
    );
}

#[test]
fn unknown_surface_and_extension_are_clean_errors() {
    let path = write_source("highlight_unknown.mox", GOOD);
    let unknown = run(&[path.to_str().expect("utf-8 path"), "--surface", "nope"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(stderr(&unknown).contains("unknown surface"));

    let text = write_source("highlight_unknown.txt", GOOD);
    let inferred = run(&[text.to_str().expect("utf-8 path")]);
    assert_eq!(inferred.status.code(), Some(1));
    assert!(stderr(&inferred).contains("cannot infer"));
}
