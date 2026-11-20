//! `pherolang` CLI tests: examples.

use std::path::{Path, PathBuf};

use crate::support::*;

// --- the canonical examples suite --------------------------------------------

/// The seven canonical domain examples as `(name, app-relative stem)`,
/// hardcoded so a missing file fails loudly. The stem is the example's
/// path under `examples/` — one directory per application since
/// pherolang#258.
const EXAMPLES: [(&str, &str); 7] = [
    ("library", "library/library"),
    ("ecommerce", "shop/ecommerce"),
    ("org", "org/org"),
    ("iot", "iot/iot"),
    ("shapes", "shapes/shapes"),
    ("support", "support/support"),
    ("pricing", "pricing/pricing"),
];

/// Canonical `.actor` policy files: compiled against the domain they import.
const ACTOR_EXAMPLES: [(&str, &str); 1] = [("support", "support/support")];

/// Canonical artifact-surface examples: each compiled against the imports
/// it declares (and, for `.dmn`, the decision tables its `logic table`
/// clauses name), all resolved relative to the file (the CLI gathers and
/// compiles `.ddd`/`.evt`/`.dmn`/`.uc`/`.deploy`/`.cmmn` imports exactly
/// like a `pherolang check` of those surfaces). Their artifact bytes are
/// pinned by `crates/phero-driver/tests/examples_artifacts.rs` and the cmmn
/// backend's examples test.
const ARTIFACT_EXAMPLES: [(&str, &str, &str); 10] = [
    ("ecommerce", "shop/ecommerce", "evt"),
    ("support", "support/support", "evt"),
    ("lending", "lending/lending", "dmn"),
    ("support", "support/support", "dmn"),
    ("orders", "shop/orders", "ddd"),
    ("support", "support/support", "ddd"),
    ("support", "support/support", "uc"),
    ("support", "support/support", "deploy"),
    ("support", "support/support", "cmmn"),
    ("support", "support/support", "intent"),
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .canonicalize()
        .expect("workspace root")
}

#[test]
fn examples_suite_checks_clean_and_is_fmt_canonical() {
    let paths: Vec<PathBuf> = EXAMPLES
        .iter()
        .map(|(_, stem)| {
            workspace_root()
                .join("examples")
                .join(format!("{stem}.mox"))
        })
        .collect();
    let actor_paths: Vec<PathBuf> = ACTOR_EXAMPLES
        .iter()
        .map(|(_, stem)| {
            workspace_root()
                .join("examples")
                .join(format!("{stem}.actor"))
        })
        .collect();
    let artifact_paths: Vec<PathBuf> = ARTIFACT_EXAMPLES
        .iter()
        .map(|(_, stem, extension)| {
            workspace_root()
                .join("examples")
                .join(format!("{stem}.{extension}"))
        })
        .collect();
    for ((name, _), path) in EXAMPLES.iter().zip(&paths) {
        assert!(
            path.is_file(),
            "missing canonical example {}: the examples suite is part of the harness",
            path.display()
        );
        let output = pherolang()
            .args(["check", path.to_str().unwrap()])
            .output()
            .expect("run pherolang check");
        assert!(
            output.status.success(),
            "pherolang check {name}.mox failed: {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("OK {}\n", path.display()),
            "pherolang check {name}.mox stdout"
        );
    }
    for ((name, _), path) in ACTOR_EXAMPLES.iter().zip(&actor_paths) {
        assert!(
            path.is_file(),
            "missing canonical example {name}.actor: the examples suite is part of the harness"
        );
        let output = pherolang()
            .args(["check", path.to_str().unwrap()])
            .output()
            .expect("run pherolang check");
        assert!(
            output.status.success(),
            "pherolang check {name}.actor failed: {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("OK {}\n", path.display()),
            "pherolang check {name}.actor stdout"
        );
    }

    for ((name, _, extension), path) in ARTIFACT_EXAMPLES.iter().zip(&artifact_paths) {
        assert!(
            path.is_file(),
            "missing canonical example {name}.{extension}: the examples suite is part of the harness"
        );
        let output = pherolang()
            .args(["check", path.to_str().unwrap()])
            .output()
            .expect("run pherolang check");
        assert!(
            output.status.success(),
            "pherolang check {name}.{extension} failed: {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("OK {}\n", path.display()),
            "pherolang check {name}.{extension} stdout"
        );
    }

    // The examples must be in canonical format (same gate the conformance
    // models get in CI).
    let mut check = pherolang();
    check.arg("fmt").arg("--check");
    for path in paths.iter().chain(&actor_paths).chain(&artifact_paths) {
        check.arg(path.to_str().unwrap());
    }
    let output = check.output().expect("run pherolang fmt --check");
    assert!(
        output.status.success(),
        "examples must be fmt-canonical. stdout: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(String::from_utf8_lossy(&output.stdout).is_empty());
}
