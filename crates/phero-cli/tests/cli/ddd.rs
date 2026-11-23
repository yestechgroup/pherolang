//! `pherolang` CLI tests: ddd.

use std::path::PathBuf;

use crate::support::*;

// --- `.ddd` design files -------------------------------------------------------

const DDD_DOMAIN: &str = "package demo\n\nclass Thing { String name }\n";

const GOOD_DDD: &str = r#"import "ddd-domain.mox"

application Demo {
    module core {
        service ThingService {
            inject ThingRepository;
            register => ThingRepository.save;
        }
        entity Thing repository ThingRepository {
            findById;
            save;
        }
    }
}
"#;

const BROKEN_DDD: &str = r#"import "ddd-domain.mox"

application Demo {
    module core {
        entity Missing repository MissingRepository { findById; }
    }
}
"#;

fn ddd() -> SurfaceCase {
    SurfaceCase {
        prereqs: &[("ddd-domain.mox", DDD_DOMAIN)],
        good_name: "good.ddd",
        good_source: GOOD_DDD,
        canonical_fixture: Some("tests/conformance/ddd/library.ddd"),
        assert_stderr_empty: true,
        broken_name: "broken.ddd",
        broken_source: BROKEN_DDD,
        broken_fragments: &[("unknown type 'Missing'", "stderr was: ")],
        broken_path_fragment: "broken.ddd",
        broken_path_noun: "design file",
        dangling: Some((
            "dangling.ddd",
            "import \"missing-domain.mox\"\n\napplication A { module m { entity T } }\n",
        )),
        // The `ir` and `fmt` steps here are the golden-comparison and
        // fmt-check shapes, not the skeleton — they stay hand-written below.
        artifact: None,
        fmt: None,
    }
}

#[test]
fn check_succeeds_on_ddd_design() {
    check_succeeds(&ddd(), InputSource::CanonicalFixture);
}

#[test]
fn check_succeeds_on_a_scratch_ddd_pair() {
    check_succeeds(&ddd(), InputSource::ScratchGood);
}

#[test]
fn check_fails_on_broken_ddd_design_with_diagnostic_on_stderr() {
    check_fails_on_broken(&ddd());
}

#[test]
fn missing_ddd_import_file_is_a_clean_error() {
    missing_import_file_is_a_clean_error(&ddd());
}

const DDD_GOLDEN: &str = include_str!("../../../../tests/conformance/ddd/library.ddd.json");

#[test]
fn ir_ddd_file_emits_the_ddd_design_artifact() {
    let path = workspace_root().join("tests/conformance/ddd/library.ddd");
    let output = pherolang()
        .args(["ir", path.to_str().unwrap()])
        .output()
        .expect("run pherolang ir");
    assert!(
        output.status.success(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    let emitted = phero_ir::ddd::DddModel::from_json(&String::from_utf8_lossy(&output.stdout))
        .expect("stdout is DDD design JSON");
    let golden = phero_ir::ddd::DddModel::from_json(DDD_GOLDEN).expect("golden is DDD design JSON");
    assert_eq!(
        emitted.format_version,
        phero_ir::ddd::DDD_MODEL_FORMAT_VERSION
    );
    assert_eq!(emitted, golden, "the emitted artifact matches the golden");
}

#[test]
fn ir_ddd_file_emits_the_machine_and_its_derivations() {
    let path = workspace_root().join("tests/conformance/ddd/payrun.ddd");
    let output = pherolang()
        .args(["ir", path.to_str().unwrap()])
        .output()
        .expect("run pherolang ir");
    assert!(
        output.status.success(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    let emitted = phero_ir::ddd::DddModel::from_json(&String::from_utf8_lossy(&output.stdout))
        .expect("stdout is DDD design JSON");
    let design = &emitted.modules[0].designs[0];
    let machine = design.machine.as_ref().expect("the machine lowers");
    assert_eq!(design.class.as_deref(), Some("PayRun"));
    assert_eq!(machine.field, "status");
    assert_eq!(machine.initial, "Draft");
    assert_eq!(machine.terminals, ["Reversed", "Posted"]);
    assert_eq!(machine.transitions.len(), 6);
    let guarded = &machine.transitions[2];
    assert_eq!(guarded.trigger.as_deref(), Some("Approve"));
    assert_eq!(guarded.guard.as_deref(), Some("total > 0"));

    // The machine derives the state_changed event contract and lowers onto
    // the restingstate xstate-ir form.
    let events = phero_driver::machines::derived_events(&emitted);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event, "state_changed");
    assert_eq!(events[0].entity, "PayRun");
    let machines = phero_driver::machines::lower_design_machines(&emitted);
    assert_eq!(machines.len(), 1);
    assert_eq!(machines[0].id, "PayRun");
    assert_eq!(machines[0].initial, "Draft");
}

#[test]
fn check_succeeds_on_the_machine_reference_fixture() {
    // Project mode: the reference resolves relative to the fixture, the
    // referenced file reads from disk, the design compiles whole.
    let path = workspace_root().join("tests/conformance/ddd/payrun-ref.ddd");
    let output = pherolang()
        .args(["check", path.to_str().unwrap()])
        .output()
        .expect("run pherolang check");
    assert!(
        output.status.success(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ir_ddd_reference_emits_the_embed_and_provenance() {
    let path = workspace_root().join("tests/conformance/ddd/payrun-ref.ddd");
    let output = pherolang()
        .args(["ir", path.to_str().unwrap()])
        .output()
        .expect("run pherolang ir");
    assert!(
        output.status.success(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    let emitted = phero_ir::ddd::DddModel::from_json(&String::from_utf8_lossy(&output.stdout))
        .expect("stdout is DDD design JSON");
    let design = &emitted.modules[0].designs[0];
    let provenance = design
        .machine_provenance
        .as_ref()
        .expect("the provenance rides the artifact");
    assert_eq!(provenance.path, "machines/payrun.machine.ts");
    assert_eq!(provenance.machine_id, "payrun");
    let machine_ir = design.machine_ir.as_ref().expect("the embed rides too");
    assert_eq!(machine_ir["id"], "payrun");
    assert_eq!(machine_ir["initial"], "Draft");
}

#[test]
fn a_missing_machine_file_is_a_clean_error() {
    // The reference names a file that does not exist: E0403 on stderr, the
    // written path named, exit 1 — the import-schema posture, no panic.
    let dir = std::env::temp_dir().join(format!(
        "pherolang-cli-388-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    std::fs::write(dir.join("domain.mox"), DDD_DOMAIN).expect("write mox");
    std::fs::write(
        dir.join("ref.ddd"),
        r#"import "domain.mox"

application Demo {
    module core {
        entity Thing
        machine state: ThingState from "machines/thing.machine.ts";
    }
}
"#,
    )
    .expect("write ddd");
    let output = pherolang()
        .args(["check", dir.join("ref.ddd").to_str().unwrap()])
        .output()
        .expect("run pherolang check");
    assert!(!output.status.success(), "the missing file must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr
            .contains("E0403 referenced machine file 'machines/thing.machine.ts' was not provided"),
        "the written path is named: {stderr}"
    );
    assert!(!stderr.contains("panicked"), "no panic: {stderr}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fmt_check_passes_on_the_machine_ddd_fixture() {
    let path = workspace_root().join("tests/conformance/ddd/payrun.ddd");
    let output = pherolang()
        .args(["fmt", "--check", path.to_str().unwrap()])
        .output()
        .expect("run pherolang fmt --check");
    assert!(
        output.status.success(),
        "stdout: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(String::from_utf8_lossy(&output.stdout).is_empty());
}

#[test]
fn fmt_check_passes_on_the_canonical_ddd_fixture() {
    let path = workspace_root().join("tests/conformance/ddd/library.ddd");
    let output = pherolang()
        .args(["fmt", "--check", path.to_str().unwrap()])
        .output()
        .expect("run pherolang fmt --check");
    assert!(
        output.status.success(),
        "stdout: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(String::from_utf8_lossy(&output.stdout).is_empty());
}

#[test]
fn fmt_check_fails_on_a_scrambled_ddd_copy() {
    let messy = "import  \"ddd-domain.mox\"\n\n\napplication   Demo { module  core { entity   Thing repository  ThingRepository { findById; } } }\n\n";
    let path = write_source("scrambled.ddd", messy);
    let output = pherolang()
        .args(["fmt", "--check", path.to_str().unwrap()])
        .output()
        .expect("run pherolang fmt --check");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        format!("would reformat: {}\n", path.display()),
        "stdout was: {stdout}"
    );
    // --check must not rewrite.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), messy);
}

#[test]
fn artifact_check_passes_on_the_ddd_golden_artifact() {
    let path = workspace_root().join("tests/conformance/ddd/library.ddd.json");
    let output = pherolang()
        .args(["artifact", "check", path.to_str().unwrap()])
        .output()
        .expect("run pherolang artifact check");
    assert!(
        output.status.success(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("OK {}\n", path.display())
    );
}

#[test]
fn artifact_check_pairs_a_design_with_the_domain_it_was_compiled_against() {
    let design = workspace_root().join("tests/conformance/ddd/library.ddd.json");
    let domain = emit_domain_artifact("library.mox");
    let output = pherolang()
        .args([
            "artifact",
            "check",
            design.to_str().unwrap(),
            "--domain",
            domain.to_str().unwrap(),
        ])
        .output()
        .expect("run pherolang artifact check");
    assert!(
        output.status.success(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );

    // A different domain is a stale pair, named by the mismatch diagnostic.
    let other = emit_domain_artifact("queries.mox");
    let output = pherolang()
        .args([
            "artifact",
            "check",
            design.to_str().unwrap(),
            "--domain",
            other.to_str().unwrap(),
        ])
        .output()
        .expect("run pherolang artifact check");
    assert_eq!(output.status.code(), Some(1), "the stale pair is refused");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("domainFingerprint") && stderr.contains("nz.example.library"),
        "stderr names the pairing mismatch, was: {stderr}"
    );
}

/// Emits a domain artifact for one conformance `.mox` file.
fn emit_domain_artifact(name: &str) -> PathBuf {
    let source = workspace_root().join("tests/conformance/ddd").join(name);
    let output = pherolang()
        .args(["ir", source.to_str().unwrap()])
        .output()
        .expect("run pherolang ir");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    write_source(
        &format!("{name}.json"),
        &String::from_utf8_lossy(&output.stdout),
    )
}

#[test]
fn artifact_check_reports_a_value_design_without_the_semantics_block() {
    // The golden value artifact, hand-edited to drop one design's fixed
    // semantics block: the contract checker must name the JSON path.
    let path = workspace_root().join("tests/conformance/ddd/values.ddd.json");
    let golden = std::fs::read_to_string(&path).expect("read the value golden artifact");
    let mut artifact: serde_json::Value =
        serde_json::from_str(&golden).expect("the golden artifact is JSON");
    let design = artifact["modules"][0]["designs"][0]
        .as_object_mut()
        .expect("the value design is an object");
    assert!(
        design.remove("semantics").is_some(),
        "the golden value design carries the semantics block"
    );
    let edited = write_source(
        "no-semantics.ddd.json",
        &serde_json::to_string_pretty(&artifact).expect("re-serialize the artifact"),
    );
    let output = pherolang()
        .args(["artifact", "check", edited.to_str().unwrap()])
        .output()
        .expect("run pherolang artifact check");
    assert_eq!(output.status.code(), Some(1), "the artifact is refused");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("modules[0].designs[0].semantics"),
        "stderr names the violation path, was: {stderr}"
    );
    assert!(
        stderr.contains("value design"),
        "stderr names the missing value semantics, was: {stderr}"
    );
}

#[test]
fn artifact_check_passes_on_the_ifml_golden_artifact() {
    // The `pherolang ir` output for the canonical interaction flow: a
    // view-and-module IFML model — the classifier's IFML rule must claim
    // it before the DDD `modules` shape.
    let path = workspace_root().join("tests/conformance/ifml/app.ifml.json");
    let output = pherolang()
        .args(["artifact", "check", path.to_str().unwrap()])
        .output()
        .expect("run pherolang artifact check");
    assert!(
        output.status.success(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("OK {}\n", path.display())
    );
}

// --- DD8: multi-file applications ---------------------------------------------

const DDD_APP_DOMAIN: &str = "package demo\n\nclass Order { String orderNo }\n";

fn ddd_app_dir(tag: &str) -> std::path::PathBuf {
    let dir = scratch_dir().join(format!("ddd-app-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create app dir");
    dir
}

fn write_app(dir: &std::path::Path, name: &str, source: &str) {
    std::fs::write(dir.join(name), source).expect("write app source");
}

#[test]
fn check_directory_compiles_a_root_and_its_fragments_once() {
    let dir = ddd_app_dir("check");
    write_app(&dir, "demo.mox", DDD_APP_DOMAIN);
    write_app(
        &dir,
        "app.ddd",
        concat!(
            "import \"demo.mox\"\n",
            "import \"sales.ddd\"\n",
            "application Shop {\n",
            "    base demo\n",
            "    module core {\n",
            "        entity Order repository OrderRepository { findById; }\n",
            "    }\n",
            "}\n",
        ),
    );
    write_app(
        &dir,
        "sales.ddd",
        "module sales {\n    service SalesService { boolean sell(String orderNo); }\n}\n",
    );
    let output = pherolang()
        .args(["check", dir.to_str().expect("utf-8")])
        .output()
        .expect("run check");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("app.ddd"), "{stdout}");
    assert!(stdout.contains("sales.ddd"), "{stdout}");
    assert!(
        String::from_utf8_lossy(&output.stderr).is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn build_emits_one_artifact_per_root_and_none_for_a_fragment() {
    let dir = ddd_app_dir("build");
    write_app(&dir, "demo.mox", DDD_APP_DOMAIN);
    write_app(
        &dir,
        "app.ddd",
        concat!(
            "import \"demo.mox\"\n",
            "import \"sales.ddd\"\n",
            "application Shop {\n",
            "    base demo\n",
            "    module core { entity Order }\n",
            "}\n",
        ),
    );
    write_app(
        &dir,
        "sales.ddd",
        "module sales { service SalesService { boolean sell(String orderNo); } }\n",
    );
    let out = ddd_app_dir("build-out");
    let output = pherolang()
        .args([
            "build",
            dir.to_str().expect("utf-8"),
            "-o",
            out.to_str().expect("utf-8"),
        ])
        .output()
        .expect("run build");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fn collect(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read out dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    collect(&out, &mut files);
    assert!(
        files.iter().any(|path| path.ends_with("app.ddd.json")),
        "{files:?}"
    );
    assert!(
        !files.iter().any(|path| path.ends_with("sales.ddd.json")),
        "{files:?}"
    );
}

#[test]
fn ir_on_a_fragment_names_the_fragment_error() {
    let dir = ddd_app_dir("ir-fragment");
    write_app(
        &dir,
        "sales.ddd",
        "module sales { service SalesService { boolean sell(String orderNo); } }\n",
    );
    let output = pherolang()
        .args(["ir", dir.join("sales.ddd").to_str().expect("utf-8")])
        .output()
        .expect("run ir");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("design fragment"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_directory_of_only_fragments_is_a_clean_error() {
    let dir = ddd_app_dir("orphan");
    write_app(
        &dir,
        "sales.ddd",
        "module sales { service SalesService { boolean sell(String orderNo); } }\n",
    );
    let output = pherolang()
        .args(["check", dir.to_str().expect("utf-8")])
        .output()
        .expect("run check");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("has no root"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_fragment_imported_by_two_roots_is_refused() {
    let dir = ddd_app_dir("two-roots");
    write_app(&dir, "demo.mox", DDD_APP_DOMAIN);
    for (name, app) in [("one.ddd", "One"), ("two.ddd", "Two")] {
        write_app(
            &dir,
            name,
            &format!(
                "import \"demo.mox\"\nimport \"shared.ddd\"\napplication {app} {{\n    base demo\n    module core {{ entity Order }}\n}}\n"
            ),
        );
    }
    write_app(
        &dir,
        "shared.ddd",
        "module shared { service SharedService { boolean touch(String id); } }\n",
    );
    let output = pherolang()
        .args(["check", dir.to_str().expect("utf-8")])
        .output()
        .expect("run check");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("more than one root"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn two_roots_with_one_application_name_are_refused() {
    let dir = ddd_app_dir("same-name");
    write_app(&dir, "demo.mox", DDD_APP_DOMAIN);
    for (file, module) in [("one.ddd", "one"), ("two.ddd", "two")] {
        write_app(
            &dir,
            file,
            &format!(
                "import \"demo.mox\"\napplication Shop {{\n    base demo\n    module {module} {{ entity Order }}\n}}\n"
            ),
        );
    }
    let output = pherolang()
        .args(["check", dir.to_str().expect("utf-8")])
        .output()
        .expect("run check");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("duplicate design application name"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// --- DD8: fragment import origins, graph membership, receipts ---------------

#[test]
fn check_directory_resolves_a_fragment_domain_from_its_own_directory() {
    // `product.mox` sits next to the fragment in `sub/`, not next to the
    // root: an import resolves against the file that declares it (DD8).
    let dir = ddd_app_dir("fragment-domain");
    write_app(&dir, "demo.mox", DDD_APP_DOMAIN);
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).expect("create sub dir");
    std::fs::write(
        sub.join("product.mox"),
        "package shop\n\nclass Product { String sku }\n",
    )
    .expect("write domain");
    std::fs::write(
        sub.join("catalogue.ddd"),
        "import \"product.mox\"\n\nmodule catalogue {\n    entity Product repository ProductRepository { findById; }\n}\n",
    )
    .expect("write fragment");
    write_app(
        &dir,
        "app.ddd",
        concat!(
            "import \"demo.mox\"\n",
            "import \"sub/catalogue.ddd\"\n",
            "application Shop {\n",
            "    base demo\n",
            "    module core { entity Order }\n",
            "}\n",
        ),
    );
    let output = pherolang()
        .args(["check", dir.to_str().expect("utf-8")])
        .output()
        .expect("run check");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn graph_build_lists_a_design_roots_fragments_as_application_files() {
    let out = scratch_dir().join("app.graph.json");
    let output = pherolang()
        .args([
            "graph",
            "build",
            "tests/conformance/ddd/app",
            "-o",
            out.to_str().expect("utf-8"),
        ])
        .current_dir(workspace_root())
        .output()
        .expect("run graph build");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let graph: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).expect("read graph"))
            .expect("graph JSON");
    let application = &graph["applications"][0];
    assert_eq!(application["id"], "Shop", "the authored application id");
    let files: Vec<&str> = application["files"]
        .as_array()
        .expect("files")
        .iter()
        .map(|file| file.as_str().expect("a path"))
        .collect();
    for expected in ["app.ddd", "sales.ddd", "billing.ddd"] {
        assert!(
            files.iter().any(|file| file.ends_with(expected)),
            "the fragment is an application file: {files:?}"
        );
    }
    let fragment_nodes: Vec<&str> = graph["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .filter(|node| node["kind"] == "file")
        .filter_map(|node| node["file"].as_str())
        .collect();
    for expected in ["sales.ddd", "billing.ddd"] {
        assert!(
            fragment_nodes.iter().any(|file| file.ends_with(expected)),
            "a fragment owns a file node: {fragment_nodes:?}"
        );
    }
}

#[test]
fn verify_hashes_a_design_roots_fragments_and_emits_one_artifact() {
    let dir = scratch_dir();
    let out = dir.join("app.receipt.json");
    let status = pherolang()
        .args([
            "verify",
            "tests/conformance/ddd/app/app.ddd",
            "-o",
            out.to_str().expect("utf-8"),
        ])
        .current_dir(workspace_root())
        .status()
        .expect("run verify");
    assert!(status.success(), "verify must succeed");
    let receipt: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).expect("read receipt"))
            .expect("receipt JSON");
    let sources: Vec<&str> = receipt["sources"]
        .as_array()
        .expect("sources")
        .iter()
        .map(|source| source["path"].as_str().expect("a path"))
        .collect();
    for expected in [
        "tests/conformance/ddd/app/app.ddd",
        "tests/conformance/ddd/app/sales.ddd",
        "tests/conformance/ddd/app/billing.ddd",
        "tests/conformance/ddd/app/app.mox",
    ] {
        assert!(
            sources.contains(&expected),
            "the closure hashes every fragment source: {sources:?}"
        );
    }
    let artifacts: Vec<&str> = receipt["artifacts"]
        .as_array()
        .expect("artifacts")
        .iter()
        .filter_map(|artifact| artifact["path"].as_str())
        .collect();
    assert_eq!(
        receipt["artifacts"].as_array().expect("artifacts").len(),
        1,
        "one application, one artifact"
    );
    assert!(
        !artifacts
            .iter()
            .any(|path| path.contains("sales.ddd") || path.contains("billing.ddd")),
        "a fragment emits no artifact: {artifacts:?}"
    );
}

#[test]
fn a_design_import_cycle_is_a_spanned_error_with_its_code() {
    let dir = ddd_app_dir("cycle");
    write_app(
        &dir,
        "app.ddd",
        concat!(
            "import \"a.ddd\"\n",
            "application Shop {\n",
            "    module core { service CoreService { boolean touch(String id); } }\n",
            "}\n",
        ),
    );
    write_app(
        &dir,
        "a.ddd",
        "import \"b.ddd\"\n\nmodule a { service AService { boolean a(String id); } }\n",
    );
    write_app(
        &dir,
        "b.ddd",
        "import \"a.ddd\"\n\nmodule b { service BService { boolean b(String id); } }\n",
    );
    let output = pherolang()
        .args(["check", dir.to_str().expect("utf-8")])
        .output()
        .expect("run check");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("E0393") && stderr.contains("design import cycle"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("b.ddd"),
        "the span anchors at the import that closes the cycle: {stderr}"
    );
}

#[test]
fn importing_a_design_application_root_as_a_fragment_is_refused() {
    let dir = ddd_app_dir("root-as-fragment");
    write_app(
        &dir,
        "app.ddd",
        concat!(
            "import \"other.ddd\"\n",
            "application Shop {\n",
            "    module core { service CoreService { boolean touch(String id); } }\n",
            "}\n",
        ),
    );
    write_app(
        &dir,
        "other.ddd",
        "application Other { module m { service MService { boolean m(String id); } } }\n",
    );
    let output = pherolang()
        .args(["check", dir.to_str().expect("utf-8")])
        .output()
        .expect("run check");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("E0394") && stderr.contains("cannot be imported as a fragment"),
        "stderr: {stderr}"
    );
}

#[test]
fn an_orphan_fragment_anchors_at_its_first_module() {
    let dir = ddd_app_dir("orphan-span");
    write_app(
        &dir,
        "sales.ddd",
        "module sales { service SalesService { boolean sell(String orderNo); } }\n",
    );
    let output = pherolang()
        .args(["check", dir.to_str().expect("utf-8")])
        .output()
        .expect("run check");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("E0390") && stderr.contains("has no root"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("module sales"),
        "the diagnostic underlines the fragment's first module: {stderr}"
    );
}

/// DD9 (issue pherolang#306): the shipped `examples/shop` finders lower the
/// typed query block the example golden carries — the behavioural assertion
/// the `.ddd` example harness can support (compile + artifact + golden).
#[test]
fn the_shop_example_emits_typed_finders() {
    let path = workspace_root().join("examples/shop/orders.ddd");
    let output = pherolang()
        .args(["ir", path.to_str().expect("utf-8")])
        .output()
        .expect("run pherolang ir");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let model = phero_ir::ddd::DddModel::from_json(&String::from_utf8_lossy(&output.stdout))
        .expect("the design artifact reads");
    let repository = model
        .modules
        .iter()
        .flat_map(|module| module.designs.iter())
        .find_map(|design| design.repository.as_ref())
        .expect("the shop example designs a repository");
    let finder = repository
        .operations
        .iter()
        .find(|operation| operation.name == "inCurrency")
        .expect("the finder is declared");
    let query = finder.query.as_ref().expect("the finder lowers a query");
    assert_eq!(query.predicate, "self.currency == code");
    assert!(query.read_only);
    let pagination = query.pagination.as_ref().expect("pagination");
    assert_eq!((pagination.limit, pagination.max), (Some(20), Some(100)));
    assert_eq!(query.sort[0].field, "orderNo");
    assert_eq!(
        finder
            .capabilities
            .iter()
            .map(|capability| capability.name.as_str())
            .collect::<Vec<_>>(),
        ["ReadOrders"]
    );
}
