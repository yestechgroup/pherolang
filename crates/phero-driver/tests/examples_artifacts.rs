//! Examples conformance harness (artifact surfaces): every canonical
//! `.evt`, `.ddd`, `.uc`, `.deploy`, and `.cmmn` example under `examples/`
//! must compile with zero diagnostics against its imports — resolved from
//! disk exactly the way the CLI resolves them — and byte-match its
//! committed golden artifact under `examples/artifacts/`, which must
//! round-trip through the artifact's `from_json`.
//!
//! The lists are hardcoded (the house convention: a missing file fails
//! loudly instead of being skipped) and a sync test pins each list against
//! the directory tree, so a new example can never be added unlisted. Each
//! entry is `(name, app-relative stem)`: `name` is the file stem the golden
//! artifact is named after (`examples/artifacts/<name>.<extension>.json`),
//! `stem` is the example's path under `examples/` — one directory per
//! application since pherolang#258. `support.cmmn` compiles against all four
//! buckets (the `.uc` rule), and `support.ddd` is additionally compiled
//! through `compile_ddd_str` with the actor bucket, proving its service-operation
//! capabilities validate against the real `support.actor` policy.
//!
//! Regenerate the goldens with `PHERO_UPDATE_FIXTURES=1 cargo test -p
//! phero-driver --test examples_artifacts`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The canonical `.evt` event-contract examples.
const EVT_EXAMPLES: [(&str, &str); 2] = [
    ("ecommerce", "shop/ecommerce"),
    ("support", "support/support"),
];

/// The canonical `.dmn` decision-model examples.
const DMN_EXAMPLES: [(&str, &str); 2] = [
    ("lending", "lending/lending"),
    ("support", "support/support"),
];

/// The canonical `.ddd` design examples.
const DDD_EXAMPLES: [(&str, &str); 2] = [("orders", "shop/orders"), ("support", "support/support")];

/// The canonical `.uc` use-case examples.
const UC_EXAMPLES: [(&str, &str); 1] = [("support", "support/support")];

/// The canonical `.deploy` deployment examples.
const DEPLOY_EXAMPLES: [(&str, &str); 1] = [("support", "support/support")];

/// The canonical `.cmmn` case-management examples.
const CMMN_EXAMPLES: [(&str, &str); 1] = [("support", "support/support")];

fn workspace_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../")
            .canonicalize()
            .expect("workspace root")
    })
}

fn examples_dir() -> PathBuf {
    workspace_root().join("examples")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("missing canonical example {}: {error}", path.display()))
}

/// The example's path and source: `examples/<stem>.<extension>`, where
/// `stem` is the app-relative stem (`support/support`, `shop/orders`).
fn example(stem: &str, extension: &str) -> (PathBuf, String) {
    let path = examples_dir().join(format!("{stem}.{extension}"));
    let source = read(&path);
    (path, source)
}

/// An `.evt`/`.ddd` example's imported domain stem (the `.mox` each file
/// declares), resolved next to the example the way the CLI resolves
/// import paths (the driver matches the resolved path, so the key is the
/// joined absolute path).
fn domain_of(example_stem: &str) -> (String, String) {
    let dir = match example_stem.rsplit_once('/') {
        Some((dir, _)) => dir,
        None => "",
    };
    let stem = match example_stem {
        // orders.ddd designs the shop domain.
        "shop/orders" => "ecommerce",
        other => other.rsplit('/').next().unwrap_or(other),
    };
    let path = examples_dir().join(dir).join(format!("{stem}.mox"));
    (path.display().to_string(), read(&path))
}

fn assert_clean<D: std::fmt::Debug>(where_: &str, diagnostics: &[D]) {
    assert!(
        diagnostics.is_empty(),
        "{where_} must compile with zero diagnostics: {:?}",
        diagnostics
    );
}

/// Byte-compares `json` against the committed golden artifact for
/// `name.extension`, regenerating it under `PHERO_UPDATE_FIXTURES`.
fn assert_matches_golden(name: &str, extension: &str, json: &str) {
    let golden = examples_dir()
        .join("artifacts")
        .join(format!("{name}.{extension}.json"));
    if std::env::var("PHERO_UPDATE_FIXTURES").is_ok() {
        std::fs::create_dir_all(golden.parent().unwrap()).expect("create artifacts dir");
        std::fs::write(&golden, json).expect("write golden artifact");
        eprintln!("updated {}", golden.display());
        return;
    }
    let expected = std::fs::read_to_string(&golden).unwrap_or_else(|error| {
        panic!(
            "missing golden artifact {}: {error} — regenerate with PHERO_UPDATE_FIXTURES=1",
            golden.display()
        )
    });
    assert_eq!(
        json, expected,
        "artifact for {name}.{extension} drifted from the golden file"
    );
}

fn compile_evt(stem: &str) -> phero_ir::events::EventModel {
    let (path, source) = example(stem, "evt");
    let (domain_path, domain) = domain_of(stem);
    let compilation = phero_driver::compile_evt_str(
        path.to_str().expect("utf-8 example path"),
        &source,
        &phero_driver::CompileInputs::from_domains(&[(domain_path, domain)]),
    );
    assert_clean(&format!("{stem}.evt"), &compilation.diagnostics);
    compilation
        .model
        .unwrap_or_else(|| panic!("{stem}.evt must lower to an event artifact"))
}

fn compile_ddd(stem: &str) -> phero_ir::ddd::DddModel {
    let (path, source) = example(stem, "ddd");
    let (domain_path, domain) = domain_of(stem);
    let compilation = phero_driver::compile_ddd_str(
        path.to_str().expect("utf-8 example path"),
        &source,
        &phero_driver::CompileInputs::from_domains(&[(domain_path, domain)]),
    );
    assert_clean(&format!("{stem}.ddd"), &compilation.diagnostics);
    compilation
        .model
        .unwrap_or_else(|| panic!("{stem}.ddd must lower to a design artifact"))
}

/// The `support.actor` policy, compiled as a pair against the domain it
/// imports. The `.actor` import matching is by exact import string (the
/// driver receives texts only), so the key is the path as written.
fn support_actor_model() -> phero_ir::ActorModel {
    let (path, source) = example("support/support", "actor");
    let domain = read(&examples_dir().join("support/support.mox"));
    let compilation = phero_driver::compile_actors_str(
        path.to_str().expect("utf-8 example path"),
        &source,
        &phero_driver::CompileInputs::from_domains(&[("support.mox".to_string(), domain)]),
    );
    assert_clean("support.actor", &compilation.diagnostics);
    compilation
        .model
        .unwrap_or_else(|| panic!("support.actor must lower to an actor model"))
}

/// The `support.ifml` flow, compiled with its `.ifml` imports (the
/// pattern library) gathered from disk through the shared traversal.
fn support_ifml_model() -> phero_ir::ifml::IfmlModel {
    let (path, source) = example("support/support", "ifml");
    let path_string = path.display().to_string();
    let mut imports = phero_ifml::IfmlImports::default();
    let mut disk_paths = BTreeMap::new();
    disk_paths.insert(path_string.clone(), path.clone());
    phero_ifml::walk_ifml_imports(&path_string, &source, &mut |importer, import| {
        let importer_disk = disk_paths.get(importer)?;
        let resolved = importer_disk.parent().unwrap().join(import);
        if import.ends_with(".ifml") {
            let imported = read(&resolved);
            let resolved_path = resolved.display().to_string();
            disk_paths.insert(import.to_string(), resolved);
            imports.provide(importer, import, imported.clone());
            Some((resolved_path, imported))
        } else {
            None
        }
    });
    let compilation = phero_ifml::compile_ifml_str(&path_string, &source, &imports)
        .expect("support.ifml must compile with zero diagnostics");
    compilation.model
}

#[test]
fn evt_examples_match_golden_artifacts_and_round_trip() {
    for (name, stem) in EVT_EXAMPLES {
        let model = compile_evt(stem);
        assert_eq!(
            model.format_version,
            phero_ir::events::EVENT_MODEL_FORMAT_VERSION
        );
        let json = model.to_json_pretty().expect("serialize event artifact");
        assert_matches_golden(name, "evt", &json);
        let reloaded = phero_ir::events::EventModel::from_json(&json)
            .unwrap_or_else(|error| panic!("{name}.evt artifact must round-trip: {error}"));
        assert_eq!(
            reloaded.to_json_pretty().expect("re-serialize"),
            json,
            "{name}.evt artifact round trip must be stable"
        );
    }
}

#[test]
fn ddd_examples_match_golden_artifacts_and_round_trip() {
    for (name, stem) in DDD_EXAMPLES {
        let model = compile_ddd(stem);
        assert_eq!(
            model.format_version,
            phero_ir::ddd::DDD_MODEL_FORMAT_VERSION
        );
        let json = model.to_json_pretty().expect("serialize design artifact");
        assert_matches_golden(name, "ddd", &json);
        let reloaded = phero_ir::ddd::DddModel::from_json(&json)
            .unwrap_or_else(|error| panic!("{name}.ddd artifact must round-trip: {error}"));
        assert_eq!(
            reloaded.to_json_pretty().expect("re-serialize"),
            json,
            "{name}.ddd artifact round trip must be stable"
        );
    }
}

/// The loose design↛policy coupling, proven tight for the one example
/// that ships both dimensions: `support.ddd`'s capability clauses name
/// real `support.actor` capabilities.
#[test]
fn support_ddd_capabilities_validate_against_the_actor_model() {
    let (path, source) = example("support/support", "ddd");
    let (domain_path, domain) = domain_of("support/support");
    let actors = support_actor_model();
    let compilation = phero_driver::compile_ddd_str(
        path.to_str().expect("utf-8 example path"),
        &source,
        &phero_driver::CompileInputs::from_domains(&[(domain_path, domain)])
            .with_actors([("support/support.actor".to_string(), actors)]),
    );
    assert_clean("support.ddd (with actors)", &compilation.diagnostics);
    assert!(
        compilation.model.is_some(),
        "support.ddd must lower to a design artifact under actor validation"
    );
}

#[test]
fn uc_examples_match_golden_artifacts_and_round_trip() {
    for (name, stem) in UC_EXAMPLES {
        let (path, source) = example(stem, "uc");
        let dir = examples_dir().join("support");
        let domains = vec![domain_of(stem)];
        let designs = vec![(
            dir.join("support.ddd").display().to_string(),
            compile_ddd("support/support"),
        )];
        let actors = vec![(
            dir.join("support.actor").display().to_string(),
            support_actor_model(),
        )];
        let events = vec![(
            dir.join("support.evt").display().to_string(),
            compile_evt("support/support"),
        )];
        let compilation = phero_driver::compile_usecase_str(
            path.to_str().expect("utf-8 example path"),
            &source,
            &phero_driver::CompileInputs::from_domains(&domains)
                .with_designs(designs)
                .with_actors(actors)
                .with_events(events),
        );
        assert_clean(&format!("{name}.uc"), &compilation.diagnostics);
        let model = compilation
            .model
            .unwrap_or_else(|| panic!("{name}.uc must lower to a use-case artifact"));
        assert_eq!(
            model.format_version,
            phero_ir::usecases::USE_CASE_MODEL_FORMAT_VERSION
        );
        let json = model.to_json_pretty().expect("serialize use-case artifact");
        assert_matches_golden(name, "uc", &json);
        let reloaded = phero_ir::usecases::UseCaseModel::from_json(&json)
            .unwrap_or_else(|error| panic!("{name}.uc artifact must round-trip: {error}"));
        assert_eq!(
            reloaded.to_json_pretty().expect("re-serialize"),
            json,
            "{name}.uc artifact round trip must be stable"
        );
    }
}

#[test]
fn dmn_examples_match_golden_artifacts_and_round_trip() {
    for (name, stem) in DMN_EXAMPLES {
        let (path, source) = example(stem, "dmn");
        let dir = path.parent().expect("example directory").to_path_buf();
        // The example's domain imports resolve next to the file (the CLI
        // rule); `lending.dmn` imports nothing.
        let domains: Vec<(String, String)> = if source.contains("import \"") {
            vec![domain_of(stem)]
        } else {
            Vec::new()
        };
        // The example's `.evt` imports compile through the ordinary event
        // pipeline and join rule 14's bucket under the import strings the
        // model writes (the CLI rule); `support.evt` rides next to it.
        let mut events: Vec<(String, phero_ir::events::EventModel)> = Vec::new();
        for import in phero_driver::project::dmn_import_paths(&source) {
            if !import.ends_with(".evt") || events.iter().any(|(key, _)| *key == import) {
                continue;
            }
            events.push((import, compile_evt("support/support")));
        }
        // The external markdown decision tables resolve relative to the
        // example file and are keyed by the paths the `logic table`
        // clauses write.
        let mut tables: Vec<(String, String)> = Vec::new();
        for relative in ["./tables/refund-authority.md", "./tables/lvr.md"] {
            let absolute = dir.join(relative.trim_start_matches("./"));
            if absolute.exists() {
                tables.push((relative.to_string(), read(&absolute)));
            }
        }
        let compilation = phero_driver::compile_dmn_str(
            path.to_str().expect("utf-8 example path"),
            &source,
            &phero_driver::CompileInputs::from_domains(&domains)
                .with_tables(tables.to_vec())
                .with_events(events),
        );
        assert_clean(&format!("{name}.dmn"), &compilation.diagnostics);
        let model = compilation
            .model
            .unwrap_or_else(|| panic!("{name}.dmn must lower to a decision artifact"));
        assert_eq!(
            model.format_version,
            phero_ir::dmn::DMN_MODEL_FORMAT_VERSION
        );
        let json = model.to_json_pretty().expect("serialize decision artifact");
        assert_matches_golden(name, "dmn", &json);
        let reloaded = phero_ir::dmn::DmnModel::from_json(&json)
            .unwrap_or_else(|error| panic!("{name}.dmn artifact must round-trip: {error}"));
        assert_eq!(
            reloaded.to_json_pretty().expect("re-serialize"),
            json,
            "{name}.dmn artifact round trip must be stable"
        );
    }
}

#[test]
fn deploy_examples_match_golden_artifacts_and_round_trip() {
    for (name, stem) in DEPLOY_EXAMPLES {
        let (path, source) = example(stem, "deploy");
        let dir = examples_dir().join("support");
        let designs = vec![(
            dir.join("support.ddd").display().to_string(),
            compile_ddd("support/support"),
        )];
        let flows = vec![(
            dir.join("support.ifml").display().to_string(),
            support_ifml_model(),
        )];
        let compilation = phero_driver::compile_deploy_str(
            path.to_str().expect("utf-8 example path"),
            &source,
            &phero_driver::CompileInputs::new()
                .with_designs(designs)
                .with_flows(flows),
        );
        assert_clean(&format!("{name}.deploy"), &compilation.diagnostics);
        let model = compilation
            .model
            .unwrap_or_else(|| panic!("{name}.deploy must lower to a deployment artifact"));
        assert_eq!(
            model.format_version,
            phero_ir::deploy::DEPLOY_MODEL_FORMAT_VERSION
        );
        let json = model
            .to_json_pretty()
            .expect("serialize deployment artifact");
        assert_matches_golden(name, "deploy", &json);
        let reloaded = phero_ir::deploy::DeployModel::from_json(&json)
            .unwrap_or_else(|error| panic!("{name}.deploy artifact must round-trip: {error}"));
        assert_eq!(
            reloaded.to_json_pretty().expect("re-serialize"),
            json,
            "{name}.deploy artifact round trip must be stable"
        );
    }
}

/// Compiles one `.cmmn` case example against its four buckets, resolved
/// the way the CLI resolves them: absolute paths for the raw domain (the
/// `.evt` rule) and for the pre-compiled event/actor/design artifacts (the
/// four-bucket rule, `.uc` matching).
fn compile_cmmn(stem: &str) -> phero_ir::cmmn::CmmnModel {
    let (path, source) = example(stem, "cmmn");
    let dir = examples_dir().join("support");
    let domains = vec![domain_of(stem)];
    let designs = vec![(
        dir.join("support.ddd").display().to_string(),
        compile_ddd("support/support"),
    )];
    let actors = vec![(
        dir.join("support.actor").display().to_string(),
        support_actor_model(),
    )];
    let events = vec![(
        dir.join("support.evt").display().to_string(),
        compile_evt("support/support"),
    )];
    // The sibling `.dmn` decision model, compiled the way the CLI compiles
    // it (its own domain import plus the decision tables its `logic table`
    // clauses name) — the decisions bucket a `decision` task's `invokes`
    // clause resolves against.
    let decisions = if source.contains("import \"support.dmn\"") {
        let (dmn_path, dmn_source) = example("support/support", "dmn");
        let dmn_domains = vec![domain_of("support/support")];
        // The external markdown decision table resolves relative to the
        // example file and is keyed by the path the `logic table` clause
        // writes.
        let tables = [(
            "./tables/refund-authority.md".to_string(),
            read(&dir.join("tables/refund-authority.md")),
        )];
        // The example's `.evt` import resolves next to the file and joins
        // the events bucket (the emits rule joins it the same way).
        let events = [(
            "support.evt".to_string(),
            phero_driver::compile_evt_str(
                dir.join("support.evt").to_str().expect("utf-8 event path"),
                &read(&dir.join("support.evt")),
                &phero_driver::CompileInputs::from_domains(&dmn_domains),
            )
            .model
            .expect("support.evt must lower to an event artifact"),
        )];
        let compilation = phero_driver::compile_dmn_str(
            dmn_path.to_str().expect("utf-8 example path"),
            &dmn_source,
            &phero_driver::CompileInputs::from_domains(&dmn_domains)
                .with_tables(tables.to_vec())
                .with_events(events),
        );
        assert_clean("support.dmn", &compilation.diagnostics);
        vec![(
            dmn_path.display().to_string(),
            compilation
                .model
                .unwrap_or_else(|| panic!("support.dmn must lower to a decision artifact")),
        )]
    } else {
        Vec::new()
    };
    let compilation = phero_driver::compile_cmmn_str(
        path.to_str().expect("utf-8 example path"),
        &source,
        &phero_driver::CompileInputs::from_domains(&domains)
            .with_designs(designs)
            .with_actors(actors)
            .with_events(events)
            .with_decisions(decisions),
    );
    assert_clean(&format!("{stem}.cmmn"), &compilation.diagnostics);
    compilation
        .model
        .unwrap_or_else(|| panic!("{stem}.cmmn must lower to a case artifact"))
}

#[test]
fn cmmn_examples_match_golden_artifacts_and_round_trip() {
    for (name, stem) in CMMN_EXAMPLES {
        let model = compile_cmmn(stem);
        assert_eq!(
            model.format_version,
            phero_ir::cmmn::CMMN_MODEL_FORMAT_VERSION
        );
        let json = model.to_json_pretty().expect("serialize case artifact");
        assert_matches_golden(name, "cmmn", &json);
        let reloaded = phero_ir::cmmn::CmmnModel::from_json(&json)
            .unwrap_or_else(|error| panic!("{name}.cmmn artifact must round-trip: {error}"));
        assert_eq!(
            reloaded.to_json_pretty().expect("re-serialize"),
            json,
            "{name}.cmmn artifact round trip must be stable"
        );
    }
}

/// Every example file of `extension` under the application directories,
/// as app-relative stems (`support/support`, `shop/orders`) — the
/// recursive sibling of the flat `examples/*.<extension>` scan. The
/// `artifacts/` and `instances/` roots hold outputs, never examples, and
/// are skipped.
fn stems_on_disk(extension: &str) -> Vec<String> {
    fn walk(dir: &Path, base: &Path, extension: &str, found: &mut Vec<String>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .expect("readable examples directory")
            .map(|entry| entry.expect("readable entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if base == Path::new("") && (name == "artifacts" || name == "instances") {
                    continue;
                }
                walk(&path, &base.join(name.as_ref()), extension, found);
            } else if path
                .extension()
                .is_some_and(|candidate| candidate == extension)
            {
                let stem = path.file_stem().expect("named file").to_string_lossy();
                found.push(base.join(stem.as_ref()).to_string_lossy().into_owned());
            }
        }
    }
    let mut found = Vec::new();
    walk(&examples_dir(), Path::new(""), extension, &mut found);
    found.sort();
    found
}

/// The house convention from `crates/phero-ifml/tests/examples.rs`: each
/// hardcoded list must cover every file of its extension on disk, so a
/// new example can never be added unlisted.
#[test]
fn the_example_lists_cover_every_artifact_file_on_disk() {
    let assert_sync = |list: &[(&str, &str)], extension: &str| {
        let mut listed: Vec<String> = list.iter().map(|(_, stem)| (*stem).to_string()).collect();
        listed.sort();
        let on_disk = stems_on_disk(extension);
        assert_eq!(
            listed, on_disk,
            "examples/**/*.{extension} and its hardcoded list must stay in sync"
        );
    };
    assert_sync(&EVT_EXAMPLES, "evt");
    assert_sync(&DMN_EXAMPLES, "dmn");
    assert_sync(&DDD_EXAMPLES, "ddd");
    assert_sync(&UC_EXAMPLES, "uc");
    assert_sync(&DEPLOY_EXAMPLES, "deploy");
    assert_sync(&CMMN_EXAMPLES, "cmmn");
}
