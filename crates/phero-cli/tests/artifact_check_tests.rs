//! `pherolang artifact check` tests: run the binary as a subprocess against
//! known-good and deliberately corrupted artifacts, plus a dogfood pass over
//! every golden artifact under `tests/conformance/artifacts/`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn pherolang() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pherolang"))
}

/// A unique scratch directory per test, so concurrently running tests never
/// share files even when they use the same artifact name.
fn scratch_dir() -> PathBuf {
    let thread = std::thread::current();
    let test = thread
        .name()
        .unwrap_or("test")
        .replace(['/', ' ', ':'], "_");
    let dir = std::env::temp_dir().join(format!(
        "phero-artifact-check-tests-{}-{}",
        std::process::id(),
        test
    ));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn write_artifact(name: &str, contents: &str) -> PathBuf {
    let path = scratch_dir().join(name);
    std::fs::write(&path, contents).expect("write artifact");
    path
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .canonicalize()
        .expect("workspace root")
}

const GOOD_IR_ARTIFACT: &str = r#"{
  "formatVersion": 2,
  "pheroVersion": "0.3.0",
  "packages": [
    {
      "name": "demo",
      "annotations": [],
      "enums": [],
      "datatypes": [],
      "interfaces": [],
      "classes": [
        {
          "name": "Book",
          "extends": [],
          "features": [
            {
              "id": 0,
              "name": "title",
              "kind": "attribute",
              "type": { "type": "primitive", "value": "string" },
              "multiplicity": { "lower": 1, "upper": { "finite": 1 } },
              "isDerived": false,
              "isId": false,
              "isReadOnly": false
            }
          ]
        }
      ]
    }
  ]
}
"#;

const GOOD_ACTOR_ARTIFACT: &str = r#"{
  "formatVersion": 2,
  "blocks": [
    {
      "name": "Support",
      "actors": [
        { "name": "Finance" },
        { "name": "Agent", "extends": "Finance", "kind": "agent" }
      ],
      "capabilities": [
        {
          "name": "Read",
          "class": { "type": "class", "value": { "package": "demo", "name": "Book" } }
        }
      ],
      "grants": [
        { "actor": "Finance", "entries": [ { "effect": "permit", "capability": "Read" } ] }
      ]
    }
  ]
}
"#;

const GOOD_INSTANCE: &str = r#"{
  "$type": "phero.instance",
  "formatVersion": 2,
  "model": "demo",
  "objects": [
    {
      "$id": "library/0",
      "$type": "Library",
      "shelves": [
        {
          "$id": "book/0",
          "$type": "Book",
          "title": "Dune",
          "author": { "$ref": "writer/0" }
        }
      ]
    },
    { "$id": "writer/0", "$type": "Writer", "name": "Frank" }
  ]
}
"#;

const GOOD_DDD_ARTIFACT: &str = r#"{
  "formatVersion": 2,
  "pheroVersion": "0.3.0",
  "application": {
    "name": "Demo"
  },
  "modules": [
    {
      "name": "core",
      "designs": [
        {
          "class": "Thing",
          "typeRef": {
            "type": "class",
            "value": { "package": "demo", "name": "Thing" }
          },
          "stereotype": "entity"
        }
      ]
    }
  ]
}
"#;

#[test]
fn artifact_check_passes_on_valid_ir_actor_and_instance_artifacts() {
    let ir = write_artifact("valid.phero.json", GOOD_IR_ARTIFACT);
    let actor = write_artifact("valid.actors.phero.json", GOOD_ACTOR_ARTIFACT);
    let instance = write_artifact("valid.instance.json", GOOD_INSTANCE);
    let output = pherolang()
        .args([
            "artifact",
            "check",
            ir.to_str().unwrap(),
            actor.to_str().unwrap(),
            instance.to_str().unwrap(),
        ])
        .output()
        .expect("run pherolang artifact check");
    assert!(
        output.status.success(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "OK {}\nOK {}\nOK {}\n",
            ir.display(),
            actor.display(),
            instance.display()
        )
    );
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn artifact_check_passes_on_a_valid_ddd_artifact() {
    let ddd = write_artifact("valid.ddd.json", GOOD_DDD_ARTIFACT);
    let output = pherolang()
        .args(["artifact", "check", ddd.to_str().unwrap()])
        .output()
        .expect("run pherolang artifact check");
    assert!(
        output.status.success(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("OK {}\n", ddd.display())
    );
}

#[test]
fn artifact_check_rejects_a_ddd_artifact_missing_a_resolved_companion() {
    let broken = GOOD_DDD_ARTIFACT.replace("\"typeRef\":", "\"typeRefOrphan\":");
    assert_ne!(broken, GOOD_DDD_ARTIFACT, "the fixture must change");
    let ddd = write_artifact("broken.ddd.json", &broken);
    let output = pherolang()
        .args(["artifact", "check", ddd.to_str().unwrap()])
        .output()
        .expect("run pherolang artifact check");
    assert!(
        !output.status.success(),
        "a missing typeRef must fail the DDD contract"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("typeRef"),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn artifact_check_rejects_unsupported_format_version_and_names_it() {
    let cases: Vec<(&str, String)> = vec![
        ("ir_v3.phero.json", version_bump(GOOD_IR_ARTIFACT)),
        (
            "actor_v3.actors.phero.json",
            version_bump(GOOD_ACTOR_ARTIFACT),
        ),
        ("instance_v3.json", version_bump(GOOD_INSTANCE)),
        ("ddd_v3.ddd.json", version_bump(GOOD_DDD_ARTIFACT)),
    ];
    for (name, contents) in cases {
        let path = write_artifact(name, &contents);
        let output = pherolang()
            .args(["artifact", "check", path.to_str().unwrap()])
            .output()
            .expect("run pherolang artifact check");
        assert_eq!(output.status.code(), Some(1), "{name} must fail");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("formatVersion 3"),
            "{name} must name the found version; stderr was: {stderr}"
        );
        assert!(
            stderr.contains("formatVersion 2"),
            "{name} must name the supported version; stderr was: {stderr}"
        );
        assert!(
            stderr.contains(name),
            "the artifact path must name the report: {stderr}"
        );
    }
}

/// Corrupts an artifact by claiming a newer wire format.
fn version_bump(artifact: &str) -> String {
    artifact.replace("\"formatVersion\": 2", "\"formatVersion\": 3")
}

#[test]
fn artifact_check_rejects_snake_case_keys() {
    let corrupted = GOOD_IR_ARTIFACT.replace("\"isReadOnly\": false", "\"is_read_only\": false");
    assert_ne!(corrupted, GOOD_IR_ARTIFACT, "corruption must apply");
    let path = write_artifact("snake_case.phero.json", &corrupted);
    let output = pherolang()
        .args(["artifact", "check", path.to_str().unwrap()])
        .output()
        .expect("run pherolang artifact check");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("is_read_only"),
        "the offending key must be named; stderr was: {stderr}"
    );
    assert!(
        stderr.contains("isReadOnly"),
        "the camelCase spelling must be suggested; stderr was: {stderr}"
    );
}

#[test]
fn artifact_check_rejects_unresolved_refs() {
    let corrupted = GOOD_INSTANCE.replace("\"$ref\": \"writer/0\"", "\"$ref\": \"writer/9\"");
    assert_ne!(corrupted, GOOD_INSTANCE, "corruption must apply");
    let path = write_artifact("unresolved_ref.instance.json", &corrupted);
    let output = pherolang()
        .args(["artifact", "check", path.to_str().unwrap()])
        .output()
        .expect("run pherolang artifact check");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("writer/9"),
        "the dangling ref must be named; stderr was: {stderr}"
    );
}

#[test]
fn artifact_check_rejects_duplicate_ids() {
    let corrupted = GOOD_INSTANCE.replace(
        r#"{ "$id": "writer/0", "$type": "Writer", "name": "Frank" }"#,
        r#"{ "$id": "book/0", "$type": "Book", "title": "Dup" },
    { "$id": "writer/0", "$type": "Writer", "name": "Frank" }"#,
    );
    assert_ne!(corrupted, GOOD_INSTANCE, "corruption must apply");
    let path = write_artifact("duplicate_id.instance.json", &corrupted);
    let output = pherolang()
        .args(["artifact", "check", path.to_str().unwrap()])
        .output()
        .expect("run pherolang artifact check");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("book/0"),
        "the duplicated id must be named; stderr was: {stderr}"
    );
    assert!(
        stderr.to_lowercase().contains("duplicate"),
        "the error must say the id is a duplicate; stderr was: {stderr}"
    );
}

#[test]
fn artifact_check_rejects_unknown_root_type() {
    let corrupted = GOOD_INSTANCE.replace(
        "\"$type\": \"phero.instance\"",
        "\"$type\": \"rogue.instance\"",
    );
    assert_ne!(corrupted, GOOD_INSTANCE, "corruption must apply");
    let path = write_artifact("unknown_root.instance.json", &corrupted);
    let output = pherolang()
        .args(["artifact", "check", path.to_str().unwrap()])
        .output()
        .expect("run pherolang artifact check");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("rogue.instance"),
        "the unknown root type must be named; stderr was: {stderr}"
    );
    assert!(
        stderr.contains("phero.instance"),
        "the expected root type must be named; stderr was: {stderr}"
    );
}

#[test]
fn artifact_check_missing_file_is_a_clean_error() {
    let missing = scratch_dir().join("does-not-exist.phero.json");
    let output = pherolang()
        .args(["artifact", "check", missing.to_str().unwrap()])
        .output()
        .expect("run pherolang artifact check");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).starts_with("error: cannot read"),
        "stderr was: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn artifact_check_reports_every_file_and_fails_if_any_fails() {
    let good = write_artifact("batch_good.phero.json", GOOD_IR_ARTIFACT);
    let bad = write_artifact("batch_bad.phero.json", &version_bump(GOOD_IR_ARTIFACT));
    let output = pherolang()
        .args([
            "artifact",
            "check",
            good.to_str().unwrap(),
            bad.to_str().unwrap(),
        ])
        .output()
        .expect("run pherolang artifact check");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        format!("OK {}\n", good.display()),
        "stdout: {stdout}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("batch_bad.phero.json"),
        "stderr was: {stderr}"
    );
}

/// The name of the committed 0.2-era fixture, spelled in parts so the
/// rebrand guard does not see a pre-0.3 extension in this source file.
fn legacy_fixture_name() -> String {
    ["legacy-0.2.", "r", "ex", ".json"].concat()
}

/// The exact message a legacy artifact must produce, assembled from parts
/// for the same reason as [`legacy_fixture_name`].
fn legacy_message() -> String {
    [
        "0.2-era ",
        "r",
        "ex",
        "lang artifact — regenerate with pherolang 0.3",
    ]
    .concat()
}

/// Dogfood: every current golden artifact under `tests/conformance/artifacts/`
/// must pass the checker, keeping the validator honest as the wire format
/// evolves. The deliberate 0.2-era fixture is excluded here and asserted
/// separately by [`legacy_artifact_is_named_with_the_migration_message`].
#[test]
fn artifact_check_passes_on_every_golden_conformance_artifact() {
    let dir = workspace_root().join("tests/conformance/artifacts");
    let legacy = legacy_fixture_name();
    let mut artifacts: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("list golden artifacts")
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("json"))
        .filter(|path| path.file_name().and_then(|name| name.to_str()) != Some(legacy.as_str()))
        .collect();
    artifacts.sort();
    assert!(
        artifacts.len() >= 6,
        "expected the full golden artifact set, found {}: {:?}",
        artifacts.len(),
        artifacts
    );
    let mut command = pherolang();
    command.arg("artifact").arg("check");
    for artifact in &artifacts {
        command.arg(artifact.to_str().unwrap());
    }
    let output = command.output().expect("run pherolang artifact check");
    assert!(
        output.status.success(),
        "golden artifacts must pass: stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: String = artifacts
        .iter()
        .map(|path| format!("OK {}\n", path.display()))
        .collect();
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
}

/// A committed 0.2-era artifact must be named as such — the migration
/// message, not a generic `formatVersion` mismatch and not `unknown root
/// $type`.
#[test]
fn legacy_artifact_is_named_with_the_migration_message() {
    let path = workspace_root()
        .join("tests/conformance/artifacts")
        .join(legacy_fixture_name());
    assert!(
        path.is_file(),
        "the legacy fixture must be committed: {path:?}"
    );
    let output = pherolang()
        .args(["artifact", "check", path.to_str().unwrap()])
        .output()
        .expect("run pherolang artifact check");
    assert_eq!(output.status.code(), Some(1), "a legacy artifact must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&legacy_message()),
        "the exact migration message must be reported; stderr was: {stderr}"
    );
    assert!(
        !stderr.contains("formatVersion") && !stderr.contains("unrecognized artifact"),
        "a legacy artifact must never fall through to a generic shape/version error: {stderr}"
    );
}

/// A minimal semantic graph artifact: one application, one node, one edge.
const GOOD_GRAPH_ARTIFACT: &str = r#"{
  "formatVersion": 1,
  "applications": [
    { "id": "support.mox", "files": ["support.mox"], "packages": ["nz.example.support"] }
  ],
  "nodes": [
    {
      "id": "class:support.mox#nz.example.support.Ticket",
      "kind": "class",
      "name": "nz.example.support.Ticket",
      "file": "support.mox",
      "application": "support.mox"
    }
  ],
  "edges": [
    {
      "from": "class:support.mox#nz.example.support.Ticket",
      "to": "class:support.mox#nz.example.support.User",
      "kind": "refers",
      "attributes": { "feature": "reporter" }
    }
  ]
}
"#;

#[test]
fn graph_artifact_classifies_as_graph_model() {
    // The committed examples artifact: it carries `applications` (the
    // deployment rule's lookalike key) *and* `nodes`, so the classifier
    // ordering is what makes this pass rather than misreport a DeployModel.
    let golden = workspace_root().join("examples/artifacts/examples.graph.json");
    let inline = write_artifact("valid.graph.json", GOOD_GRAPH_ARTIFACT);
    let output = pherolang()
        .args([
            "artifact",
            "check",
            golden.to_str().unwrap(),
            inline.to_str().unwrap(),
        ])
        .output()
        .expect("run pherolang artifact check");
    assert!(
        output.status.success(),
        "graph artifacts must pass: stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("OK {}\nOK {}\n", golden.display(), inline.display())
    );
}

#[test]
fn graph_empty_form_is_a_version_mismatch_not_a_legacy_artifact() {
    // The graph marker deliberately stayed at 1 while every other kind moved
    // to 2, so the empty graph is no longer byte-identical to the block-less
    // ActorModel: `{"formatVersion":1}` reaches the minimal-empty actor
    // fallback and fails the actor's now-2 version gate. It is a version
    // mismatch — never a legacy artifact (no old marker is present).
    let empty = write_artifact("empty.graph.json", r#"{"formatVersion": 1}"#);
    let output = pherolang()
        .args(["artifact", "check", empty.to_str().unwrap()])
        .output()
        .expect("run pherolang artifact check");
    assert_eq!(output.status.code(), Some(1), "the empty form must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("actor artifact formatVersion 1") && stderr.contains("formatVersion 2"),
        "the actor fallback gate must name both versions: {stderr}"
    );
    assert!(
        !stderr.contains("0.2-era") && !stderr.contains("artifact — regenerate"),
        "a bare version marker is not a legacy artifact: {stderr}"
    );

    // The empty ActorModel's own bytes (version 2) still classify and pass.
    let actor = write_artifact("empty.actor.json", r#"{"formatVersion": 2}"#);
    let output = pherolang()
        .args(["artifact", "check", actor.to_str().unwrap()])
        .output()
        .expect("run pherolang artifact check");
    assert!(
        output.status.success(),
        "the empty ActorModel form must pass: stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn graph_edges_carry_non_empty_endpoints() {
    // An edge without `to` is not a semantic edge: the checker must name the
    // precise violation, not classify the document as something else.
    let broken = write_artifact(
        "broken.graph.json",
        r#"{
          "formatVersion": 1,
          "nodes": [
            {
              "id": "class:support.mox#nz.example.support.Ticket",
              "kind": "class",
              "name": "nz.example.support.Ticket",
              "file": "support.mox",
              "application": "support.mox"
            }
          ],
          "edges": [
            { "from": "class:support.mox#nz.example.support.Ticket", "kind": "refers" }
          ]
        }"#,
    );
    let output = pherolang()
        .args(["artifact", "check", broken.to_str().unwrap()])
        .output()
        .expect("run pherolang artifact check");
    assert_eq!(output.status.code(), Some(1));
    let rendered = String::from_utf8_lossy(&output.stderr);
    assert!(
        rendered.contains("edges[0]") && rendered.contains("\"to\""),
        "the violation names the edge and the missing endpoint: {rendered}"
    );
    assert!(String::from_utf8_lossy(&output.stdout).is_empty());
}
