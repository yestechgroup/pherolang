//! Shared test fixtures for the semantic graph and the spanned navigation
//! indexes (issue pherolang#260).
//!
//! The navigation tests read the same fixture applications in three places —
//! this crate's own unit tests, the `phero-driver` integration tests
//! (`tests/navigation.rs`), and the LSP server tests. Before this crate each
//! site carried its own copy of the fixture paths and the nine-surface
//! compile order, so a fixture rename or a new surface meant three edits.
//! Here they live exactly once:
//!
//! * [`fixtures`] — every fixture file's root-relative display path,
//!   including the canonical conformance application
//!   (`tests/conformance/graph/app.*`) and the support reference stack
//!   (`examples/support/support.*`).
//! * [`fixture`]/[`disk`]/[`workspace_root`] — the host-side disk reads.
//! * [`fixture_graph`] and [`support_graph`] — the two applications compiled
//!   into a graph once and shared through a `OnceLock`.
//! * [`surface_index`] — the per-surface span table, selected by extension.
//! * [`assert_definitions_match_graph`] — the one anti-drift assertion
//!   between a span table and the graph's node table. The two documented
//!   exclusions (a compiled file's `file` node and an `.ifml` flow's `flow`
//!   node — both named by host facts a source-only index cannot know) are
//!   stated in [`graph_definition_keys`], once.
//! * [`copy_fixture_tree`]/[`copy_fixture_files`] — the LSP's temp-copy of
//!   the conformance fixture directory.
//!
//! This crate is **dev-only** (`publish = false`): production crates depend
//! on it through `[dev-dependencies]` only. It may depend on `phero-driver`
//! normally because Cargo allows that dev-dependency cycle; the seam that
//! makes it sound is documented on [`assert_definition_keys`].
//!
//! The cross-policy analytics suite (`crates/phero-driver/tests/graph_analytics.rs`)
//! deliberately keeps its own loader: that suite **writes** the fixture's
//! golden artifacts when `PHERO_UPDATE_FIXTURES` is set, and this crate is the
//! read-only loader for the navigation matrix.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use phero_driver::graph::{build, GraphFile, GraphInputs};
use phero_driver::navigation::SurfaceIndex;
use phero_ir::graph::{kinds, GraphModel};

/// The fixture file paths, in one place (issue pherolang#260).
///
/// Every display path is root-relative POSIX, the same string the semantic
/// graph emits on its nodes.
pub mod fixtures {
    /// The self-contained conformance application's directory: one file per
    /// surface, all one import-connected application. It is the per-surface
    /// equality test's fixture: compiling it needs no sibling examples,
    /// tables, or pattern library.
    pub const GRAPH_DIR: &str = "tests/conformance/graph";

    /// The conformance domain.
    pub const APP_MOX: &str = "tests/conformance/graph/app.mox";
    /// The conformance actor policy.
    pub const APP_ACTOR: &str = "tests/conformance/graph/app.actor";
    /// The conformance design.
    pub const APP_DDD: &str = "tests/conformance/graph/app.ddd";
    /// The conformance event contract.
    pub const APP_EVT: &str = "tests/conformance/graph/app.evt";
    /// The conformance decision model.
    pub const APP_DMN: &str = "tests/conformance/graph/app.dmn";
    /// The conformance use cases.
    pub const APP_UC: &str = "tests/conformance/graph/app.uc";
    /// The conformance case plan.
    pub const APP_CMMN: &str = "tests/conformance/graph/app.cmmn";
    /// The conformance deployment.
    pub const APP_DEPLOY: &str = "tests/conformance/graph/app.deploy";
    /// The conformance interaction flow.
    pub const APP_IFML: &str = "tests/conformance/graph/app.ifml";
    /// The conformance application's nine surface files, in compile order.
    ///
    /// This is what [`crate::copy_fixture_tree`] copies into the LSP's temp
    /// directory.
    pub const APP_STACK: [&str; 9] = [
        APP_MOX, APP_ACTOR, APP_DDD, APP_EVT, APP_DMN, APP_UC, APP_CMMN, APP_DEPLOY, APP_IFML,
    ];

    /// The fixture's unrelated second application (an opaque `cedar`
    /// policy), for the per-application analytics rule.
    pub const OTHER_MOX: &str = "tests/conformance/graph/other.mox";
    /// The unrelated second application's policy.
    pub const OTHER_ACTOR: &str = "tests/conformance/graph/other.actor";

    /// The support reference stack's application directory.
    pub const SUPPORT_DIR: &str = "examples/support";

    /// The support domain.
    pub const SUPPORT_MOX: &str = "examples/support/support.mox";
    /// The support actor policy.
    pub const SUPPORT_ACTOR: &str = "examples/support/support.actor";
    /// The support design.
    pub const SUPPORT_DDD: &str = "examples/support/support.ddd";
    /// The support event contract.
    pub const SUPPORT_EVT: &str = "examples/support/support.evt";
    /// The support decision model.
    pub const SUPPORT_DMN: &str = "examples/support/support.dmn";
    /// The support use cases.
    pub const SUPPORT_UC: &str = "examples/support/support.uc";
    /// The support case plan.
    pub const SUPPORT_CMMN: &str = "examples/support/support.cmmn";
    /// The support deployment.
    pub const SUPPORT_DEPLOY: &str = "examples/support/support.deploy";
    /// The support interaction flow.
    pub const SUPPORT_IFML: &str = "examples/support/support.ifml";

    /// The design fixture with a state machine (its transitions).
    pub const PAYRUN_DDD: &str = "tests/conformance/ddd/payrun.ddd";
    /// The `payrun` design's domain.
    pub const PAYRUN_MOX: &str = "tests/conformance/ddd/payrun.mox";
    /// The design fixture with module-level `extend` child operations.
    pub const CHILDOPS_DDD: &str = "tests/conformance/ddd/childops.ddd";
    /// The `childops` design's domain.
    pub const CHILDOPS_MOX: &str = "tests/conformance/ddd/childops.mox";

    /// The domain carrying an inline `actors` block (the `.mox` surface's
    /// own actor/capability nodes).
    pub const ACTORS_MOX: &str = "tests/conformance/models/actors.mox";

    /// The support decision model's external decision table.
    pub const REFUND_AUTHORITY_TABLE: &str = "examples/support/tables/refund-authority.md";
}

/// The repository root (this crate's manifest is two levels down).
pub fn workspace_root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root")
    })
    .clone()
}

/// The canonical path of a root-relative display path.
pub fn disk(display: &str) -> PathBuf {
    std::fs::canonicalize(workspace_root().join(display))
        .unwrap_or_else(|error| panic!("missing fixture file {display}: {error}"))
}

/// Reads a root-relative fixture file.
pub fn fixture(display: &str) -> String {
    std::fs::read_to_string(disk(display))
        .unwrap_or_else(|error| panic!("missing fixture file {display}: {error}"))
}

// --- The LSP's temp copy -----------------------------------------------------

/// Copies the conformance application ([`fixtures::APP_STACK`]) into `dst`,
/// creating it when needed.
///
/// The LSP tests compile a *writable* directory snapshot: the committed
/// fixture stays untouched while each test adds its own files on top. The
/// nine canonical files come from here; LSP-only extras (the payrun pair,
/// an inline-actors model, a patched `.evt`/`.uc`) are added by the caller.
pub fn copy_fixture_tree(dst: &Path) {
    copy_fixture_files(dst, &fixtures::APP_STACK);
}

/// Copies root-relative fixture files into `dst`, keeping their file names.
///
/// Panics when a fixture is missing or the copy fails: fixtures are committed,
/// so a missing file is a broken test, never a skip.
pub fn copy_fixture_files(dst: &Path, displays: &[&str]) {
    std::fs::create_dir_all(dst).unwrap_or_else(|error| panic!("mkdir {}: {error}", dst.display()));
    for display in displays {
        let name = display
            .rsplit('/')
            .next()
            .unwrap_or_else(|| panic!("{display} has no file name"));
        std::fs::copy(disk(display), dst.join(name))
            .unwrap_or_else(|error| panic!("copy {display}: {error}"));
    }
}

// --- The fixture import scanner ---------------------------------------------

/// The `import "..."` strings of a source, in order.
///
/// The fixture's own scanner: graph inputs need the *resolved* keys, and
/// resolving them from disk is the host's job.
fn import_strings(source: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for line in source.lines() {
        let line = line.trim_start();
        let Some(rest) = line.strip_prefix("import ") else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('"') else {
            continue;
        };
        if let Some(end) = rest.find('"') {
            imports.push(rest[..end].to_string());
        }
    }
    imports
}

/// One graph input file with its display path, host-normalized key, and
/// disk-resolved import targets.
fn graph_file<M>(display: &str, model: M) -> GraphFile<M> {
    let path = disk(display);
    let imports = import_strings(&fixture(display))
        .into_iter()
        .filter_map(|import| std::fs::canonicalize(path.parent()?.join(import)).ok())
        .map(|path| path.display().to_string())
        .collect();
    GraphFile::new(display, path.display().to_string(), imports, model)
}

// --- Per-surface graphs (the unit tests' app fixture) ------------------------

/// One file's compiled artifact, panicking with its diagnostics when it does
/// not lower (the fixture must stay clean).
fn lowered<T, D: std::fmt::Debug>(display: &str, model: Option<T>, diagnostics: &D) -> T {
    model.unwrap_or_else(|| panic!("{display} must compile: {diagnostics:?}"))
}

/// The graph of one `.actor` example (its `import` texts keyed as written —
/// the `.actor` compile contract).
pub fn actor_graph(display: &str) -> GraphModel {
    let source = fixture(display);
    let directory = disk(display).parent().expect("a directory").to_path_buf();
    let domains: Vec<(String, String)> = import_strings(&source)
        .into_iter()
        .filter_map(|import| {
            std::fs::read_to_string(directory.join(&import))
                .ok()
                .map(|text| (import, text))
        })
        .collect();
    let compilation = phero_driver::compile_actors_str(
        &disk(display).display().to_string(),
        &source,
        &domains,
        &phero_driver::DomainImports::default(),
    );
    let model = lowered(display, compilation.model, &compilation.diagnostics);
    let mut inputs = GraphInputs::default();
    inputs.actors.push(graph_file(display, model));
    build(&inputs)
}

/// The graph of one `.ddd` example, resolved against `domain` (imports keyed
/// by canonical path — the `.ddd` compile contract).
pub fn ddd_graph(display: &str, domain: &str) -> GraphModel {
    let domains = [(disk(domain).display().to_string(), fixture(domain))];
    let compilation = phero_driver::compile_ddd_str(
        &disk(display).display().to_string(),
        &fixture(display),
        &domains,
        &phero_driver::DomainImports::default(),
    );
    let model = lowered(display, compilation.model, &compilation.diagnostics);
    let mut inputs = GraphInputs::default();
    inputs.designs.push(graph_file(display, model));
    build(&inputs)
}

/// The graph of one `.evt` example, resolved against `domain`.
pub fn evt_graph(display: &str, domain: &str) -> GraphModel {
    let domains = [(disk(domain).display().to_string(), fixture(domain))];
    let compilation = phero_driver::compile_evt_str(
        &disk(display).display().to_string(),
        &fixture(display),
        &domains,
        &phero_driver::DomainImports::default(),
    );
    let model = lowered(display, compilation.model, &compilation.diagnostics);
    let mut inputs = GraphInputs::default();
    inputs.events.push(graph_file(display, model));
    build(&inputs)
}

/// The compiled artifacts of the `app.*` fixture, one per surface, wired into
/// the graph's input buckets exactly the way `tests/graph.rs` and the CLI
/// compile them (imports resolved from disk next to the importing file).
fn fixture_inputs() -> GraphInputs {
    let domain_display = fixtures::APP_MOX;
    let domain_key = disk(domain_display).display().to_string();
    let domain_source = fixture(domain_display);
    let compilation = phero_driver::compile_str(
        &domain_key,
        &domain_source,
        &phero_driver::DomainImports::default(),
    );
    let domain = lowered(domain_display, compilation.model, &compilation.diagnostics);
    let domains = [(domain_key, domain_source.clone())];

    let actor_display = fixtures::APP_ACTOR;
    let compilation = phero_driver::compile_actors_str(
        &disk(actor_display).display().to_string(),
        &fixture(actor_display),
        // The `.actor` compile keys its imports by the string as written.
        &[("app.mox".to_string(), domain_source.clone())],
        &phero_driver::DomainImports::default(),
    );
    let actor = lowered(actor_display, compilation.model, &compilation.diagnostics);

    let ddd_display = fixtures::APP_DDD;
    let compilation = phero_driver::compile_ddd_str(
        &disk(ddd_display).display().to_string(),
        &fixture(ddd_display),
        &domains,
        &phero_driver::DomainImports::default(),
    );
    let design = lowered(ddd_display, compilation.model, &compilation.diagnostics);

    let evt_display = fixtures::APP_EVT;
    let compilation = phero_driver::compile_evt_str(
        &disk(evt_display).display().to_string(),
        &fixture(evt_display),
        &domains,
        &phero_driver::DomainImports::default(),
    );
    let events = lowered(evt_display, compilation.model, &compilation.diagnostics);

    let dmn_display = fixtures::APP_DMN;
    let compilation = phero_driver::compile_dmn_str(
        &disk(dmn_display).display().to_string(),
        &fixture(dmn_display),
        &domains,
        &phero_driver::DomainImports::default(),
        &[],
        // The decision model's import keys are the strings as written.
        &[("app.evt".to_string(), events.clone())],
    );
    let decision = lowered(dmn_display, compilation.model, &compilation.diagnostics);

    let ifml_display = fixtures::APP_IFML;
    let ifml_key = disk(ifml_display).display().to_string();
    let flow = phero_ifml::compile_ifml_str(
        &ifml_key,
        &fixture(ifml_display),
        &phero_ifml::IfmlImports::default(),
    )
    .unwrap_or_else(|diagnostics| panic!("{ifml_display} must compile: {diagnostics:#?}"))
    .model;

    let uc_display = fixtures::APP_UC;
    let compilation = phero_driver::compile_usecase_str(
        &disk(uc_display).display().to_string(),
        &fixture(uc_display),
        &domains,
        &phero_driver::DomainImports::default(),
        &[(disk(ddd_display).display().to_string(), design.clone())],
        &[(disk(actor_display).display().to_string(), actor.clone())],
        &[(disk(evt_display).display().to_string(), events.clone())],
    );
    let use_cases = lowered(uc_display, compilation.model, &compilation.diagnostics);

    let cmmn_display = fixtures::APP_CMMN;
    let compilation = phero_driver::compile_cmmn_str(
        &disk(cmmn_display).display().to_string(),
        &fixture(cmmn_display),
        &domains,
        &phero_driver::DomainImports::default(),
        &[(disk(ddd_display).display().to_string(), design.clone())],
        &[(disk(actor_display).display().to_string(), actor.clone())],
        &[(disk(evt_display).display().to_string(), events.clone())],
        &[(disk(dmn_display).display().to_string(), decision.clone())],
    );
    let case = lowered(cmmn_display, compilation.model, &compilation.diagnostics);

    let deploy_display = fixtures::APP_DEPLOY;
    let compilation = phero_driver::compile_deploy_str(
        &disk(deploy_display).display().to_string(),
        &fixture(deploy_display),
        &[(disk(ddd_display).display().to_string(), design.clone())],
        &[(ifml_key, flow.clone())],
    );
    let deployment = lowered(deploy_display, compilation.model, &compilation.diagnostics);

    let mut inputs = GraphInputs::default();
    inputs.domains.push(graph_file(domain_display, domain));
    inputs.actors.push(graph_file(actor_display, actor));
    inputs.designs.push(graph_file(ddd_display, design));
    inputs.events.push(graph_file(evt_display, events));
    inputs.decisions.push(graph_file(dmn_display, decision));
    inputs.flows.push(graph_file(ifml_display, flow));
    inputs.use_cases.push(graph_file(uc_display, use_cases));
    inputs.cases.push(graph_file(cmmn_display, case));
    inputs.deploys.push(graph_file(deploy_display, deployment));
    inputs
}

/// The graph of the whole `app.*` fixture application, built once and shared
/// by every per-surface equality test (the compile chain is the expensive
/// part; the node table per file is fixed).
pub fn fixture_graph() -> &'static GraphModel {
    static GRAPH: OnceLock<GraphModel> = OnceLock::new();
    GRAPH.get_or_init(|| build(&fixture_inputs()))
}

// --- The support stack (the integration matrix) ------------------------------

/// The pattern files `support.ifml` instantiates (its `walk_ifml_imports`
/// closure), hardcoded per the house convention: a missing file fails loudly.
const PATTERN_FLOWS: [&str; 6] = [
    "patterns/ifml/collections.ifml",
    "patterns/ifml/communication.ifml",
    "patterns/ifml/data.ifml",
    "patterns/ifml/input.ifml",
    "patterns/ifml/navigation.ifml",
    "patterns/ifml/workflow.ifml",
];

/// One `.ifml` file compiled with its `.ifml` import closure gathered from
/// disk through the shared traversal.
fn flow_model(display: &str) -> phero_ir::ifml::IfmlModel {
    let path = disk(display);
    let key = path.display().to_string();
    let source = fixture(display);
    let mut imports = phero_ifml::IfmlImports::default();
    let mut disk_paths = std::collections::BTreeMap::new();
    disk_paths.insert(key.clone(), path.clone());
    phero_ifml::walk_ifml_imports(&key, &source, &mut |importer, import| {
        if !import.ends_with(".ifml") {
            return None;
        }
        let importer_path = disk_paths.get(importer)?;
        let resolved = importer_path.parent()?.join(import);
        let imported = std::fs::read_to_string(&resolved).ok()?;
        let resolved_key = resolved.display().to_string();
        disk_paths.insert(import.to_string(), resolved);
        imports.provide(importer, import, imported.clone());
        Some((resolved_key, imported))
    });
    phero_ifml::compile_ifml_str(&key, &source, &imports)
        .unwrap_or_else(|diagnostics| panic!("{display} must compile: {diagnostics:?}"))
        .model
}

/// The whole support stack compiled into one graph: the domain, the policy,
/// the design, the events, the decision model, the flow (with its pattern
/// library), the use cases, the case, and the deployment.
pub fn support_graph() -> &'static GraphModel {
    static GRAPH: OnceLock<GraphModel> = OnceLock::new();
    GRAPH.get_or_init(|| {
        let domain_display = fixtures::SUPPORT_MOX;
        let domain_key = disk(domain_display).display().to_string();
        let domain_source = fixture(domain_display);
        let compilation = phero_driver::compile_str(
            &domain_key,
            &domain_source,
            &phero_driver::DomainImports::default(),
        );
        let domain = lowered(domain_display, compilation.model, &compilation.diagnostics);
        let domains = [(domain_key, domain_source.clone())];

        let actor_display = fixtures::SUPPORT_ACTOR;
        let compilation = phero_driver::compile_actors_str(
            &disk(actor_display).display().to_string(),
            &fixture(actor_display),
            // The `.actor` compile keys its imports by the string as written.
            &[("support.mox".to_string(), domain_source)],
            &phero_driver::DomainImports::default(),
        );
        let actor = lowered(actor_display, compilation.model, &compilation.diagnostics);

        let ddd_display = fixtures::SUPPORT_DDD;
        let compilation = phero_driver::compile_ddd_str(
            &disk(ddd_display).display().to_string(),
            &fixture(ddd_display),
            &domains,
            &phero_driver::DomainImports::default(),
        );
        let design = lowered(ddd_display, compilation.model, &compilation.diagnostics);

        let evt_display = fixtures::SUPPORT_EVT;
        let compilation = phero_driver::compile_evt_str(
            &disk(evt_display).display().to_string(),
            &fixture(evt_display),
            &domains,
            &phero_driver::DomainImports::default(),
        );
        let events = lowered(evt_display, compilation.model, &compilation.diagnostics);

        let dmn_display = fixtures::SUPPORT_DMN;
        let compilation = phero_driver::compile_dmn_str(
            &disk(dmn_display).display().to_string(),
            &fixture(dmn_display),
            &domains,
            &phero_driver::DomainImports::default(),
            &[(
                "./tables/refund-authority.md".to_string(),
                fixture(fixtures::REFUND_AUTHORITY_TABLE),
            )],
            &[("support.evt".to_string(), events.clone())],
        );
        let decision = lowered(dmn_display, compilation.model, &compilation.diagnostics);

        let ifml_display = fixtures::SUPPORT_IFML;
        let ifml_key = disk(ifml_display).display().to_string();
        let flow = flow_model(ifml_display);

        let uc_display = fixtures::SUPPORT_UC;
        let compilation = phero_driver::compile_usecase_str(
            &disk(uc_display).display().to_string(),
            &fixture(uc_display),
            &domains,
            &phero_driver::DomainImports::default(),
            &[(disk(ddd_display).display().to_string(), design.clone())],
            &[(disk(actor_display).display().to_string(), actor.clone())],
            &[(disk(evt_display).display().to_string(), events.clone())],
        );
        let use_cases = lowered(uc_display, compilation.model, &compilation.diagnostics);

        let cmmn_display = fixtures::SUPPORT_CMMN;
        let compilation = phero_driver::compile_cmmn_str(
            &disk(cmmn_display).display().to_string(),
            &fixture(cmmn_display),
            &domains,
            &phero_driver::DomainImports::default(),
            &[(disk(ddd_display).display().to_string(), design.clone())],
            &[(disk(actor_display).display().to_string(), actor.clone())],
            &[(disk(evt_display).display().to_string(), events.clone())],
            &[(disk(dmn_display).display().to_string(), decision.clone())],
        );
        let case = lowered(cmmn_display, compilation.model, &compilation.diagnostics);

        let deploy_display = fixtures::SUPPORT_DEPLOY;
        let compilation = phero_driver::compile_deploy_str(
            &disk(deploy_display).display().to_string(),
            &fixture(deploy_display),
            &[(disk(ddd_display).display().to_string(), design.clone())],
            &[(ifml_key, flow.clone())],
        );
        let deployment = lowered(deploy_display, compilation.model, &compilation.diagnostics);

        let mut inputs = GraphInputs::default();
        inputs.domains.push(graph_file(domain_display, domain));
        inputs.actors.push(graph_file(actor_display, actor));
        inputs.designs.push(graph_file(ddd_display, design));
        inputs.events.push(graph_file(evt_display, events));
        inputs.decisions.push(graph_file(dmn_display, decision));
        inputs.flows.push(graph_file(ifml_display, flow));
        inputs.use_cases.push(graph_file(uc_display, use_cases));
        inputs.cases.push(graph_file(cmmn_display, case));
        inputs.deploys.push(graph_file(deploy_display, deployment));
        for display in PATTERN_FLOWS {
            inputs.flows.push(graph_file(display, flow_model(display)));
        }
        build(&inputs)
    })
}

/// The graph over one fixture file, compiled exactly the way `tests/graph.rs`
/// compiles the examples (imports resolved from disk next to the file).
///
/// The four artifact surfaces (`deploy`, `uc`, `cmmn`, `dmn`) cannot compile
/// alone — their imports are pre-compiled artifacts — so they read the
/// support stack's single graph instead (see [`support_graph`]). The same is
/// true of the interaction flow, whose pattern library is the support stack's.
pub fn single_file_graph(display: &str, domain: &str) -> GraphModel {
    let extension = display.rsplit_once('.').map(|(_, extension)| extension);
    if matches!(extension, Some("deploy" | "uc" | "cmmn" | "dmn" | "ifml")) {
        return support_graph().clone();
    }
    let source = fixture(display);
    if display.ends_with(".mox") {
        let path = disk(display).display().to_string();
        let compilation =
            phero_driver::compile_str(&path, &source, &phero_driver::DomainImports::default());
        let model = lowered(display, compilation.model, &compilation.diagnostics);
        let mut inputs = GraphInputs::default();
        inputs.domains.push(graph_file(display, model));
        return build(&inputs);
    }
    let path = disk(display).display().to_string();
    let mut inputs = GraphInputs::default();
    if display.ends_with(".actor") {
        // The `.actor` compile keys its imports by the string as written.
        let directory = disk(display).parent().expect("a directory").to_path_buf();
        let domains: Vec<(String, String)> = import_strings(&source)
            .into_iter()
            .filter_map(|import| {
                std::fs::read_to_string(directory.join(&import))
                    .ok()
                    .map(|text| (import, text))
            })
            .collect();
        let compilation = phero_driver::compile_actors_str(
            &path,
            &source,
            &domains,
            &phero_driver::DomainImports::default(),
        );
        let model = lowered(display, compilation.model, &compilation.diagnostics);
        inputs.actors.push(graph_file(display, model));
    } else {
        // The `.ddd`/`.evt` compiles key their imports by canonical path.
        let domains = [(disk(domain).display().to_string(), fixture(domain))];
        if display.ends_with(".ddd") {
            let compilation = phero_driver::compile_ddd_str(
                &path,
                &source,
                &domains,
                &phero_driver::DomainImports::default(),
            );
            let model = lowered(display, compilation.model, &compilation.diagnostics);
            inputs.designs.push(graph_file(display, model));
        } else {
            let compilation = phero_driver::compile_evt_str(
                &path,
                &source,
                &domains,
                &phero_driver::DomainImports::default(),
            );
            let model = lowered(display, compilation.model, &compilation.diagnostics);
            inputs.events.push(graph_file(display, model));
        }
    }
    build(&inputs)
}

// --- The span tables ---------------------------------------------------------

/// The surface's spanned index, built from the fixture's source text, plus
/// the source it was built from.
///
/// The builder is selected by extension; the four artifact surfaces and
/// `.ifml` all have one.
pub fn surface_index(display: &str) -> (String, SurfaceIndex) {
    let source = fixture(display);
    let index = match display.rsplit_once('.').map(|(_, extension)| extension) {
        Some("actor") => phero_driver::navigation::actors_index(&source),
        Some("ddd") => phero_driver::navigation::ddd_index(&source),
        Some("evt") => phero_driver::navigation::evt_index(&source, display),
        Some("deploy") => phero_driver::navigation::deploy_index(&source),
        Some("uc") => phero_driver::navigation::uc_index(&source),
        Some("cmmn") => phero_driver::navigation::cmmn_index(&source),
        Some("dmn") => phero_driver::navigation::dmn_index(&source, display),
        Some("mox") => phero_driver::navigation::mox_index(&source),
        Some("ifml") => {
            let (model, index) = phero_ifml::parse_ifml_indexed(&source)
                .unwrap_or_else(|error| panic!("{display} must parse: {error:?}"));
            phero_driver::navigation::ifml_surface(&model, &index)
        }
        _ => panic!("no index builder for {display}"),
    };
    (source, index)
}

// --- The one assertion -------------------------------------------------------

/// The `(kind, name)` keys of the graph's nodes attributed to `display`.
///
/// Three node kinds have no written declaration site and are the documented
/// exclusions (stated here, once):
///
/// * the `file` node per compiled file — its name is the document's display
///   path, a host fact a source-only span index cannot know (the index keys
///   declarations, not files);
/// * an `.ifml` `flow` node — its name is the file stem, the same host fact;
/// * a `.deploy` `platformCapability` node — its name is one word of the
///   closed target-capability vocabulary in the driver's deploy tables
///   (pherolang#262), which no model file declares. The editor lands on the
///   first written `requires` mention instead (`docs/GRAPH.md`).
///
/// All three still exist in the graph (reached by `contains`/`memberOf`,
/// `bindsFlow`, and `requires`); they are simply not span-indexed.
pub fn graph_definition_keys(graph: &GraphModel, display: &str) -> BTreeSet<(String, String)> {
    graph
        .nodes
        .iter()
        .filter(|node| node.file == display && node.kind != kinds::FILE)
        .filter(|node| !(display.ends_with(".ifml") && node.kind == kinds::FLOW))
        .filter(|node| node.kind != kinds::PLATFORM_CAPABILITY)
        .map(|node| (node.kind.clone(), node.name.clone()))
        .collect()
}

/// The shared assertion body over plain `(kind, name)` pairs.
///
/// This exists as the documented seam of the dev-dependency cycle: when
/// `phero-driver`'s own `#[cfg(test)]` unit tests call the testkit, the
/// `SurfaceIndex` type they build belongs to the *test* copy of `phero-driver`,
/// while the testkit links the *normal* copy — two distinct Rust types. The
/// driver's thin adapter projects the index's definitions to these pairs and
/// calls this function, so the key-set comparison (and the exclusion list)
/// still lives here exactly once. The index-local uniqueness check stays in
/// the adapter, where the index is available.
pub fn assert_definition_keys(definitions: &[(String, String)], graph: &GraphModel, display: &str) {
    let expected = graph_definition_keys(graph, display);
    let actual: BTreeSet<(String, String)> = definitions.iter().cloned().collect();
    assert_eq!(
        actual.len(),
        definitions.len(),
        "{display}: duplicate (kind, name) definitions: {definitions:#?}"
    );
    assert_eq!(
        actual, expected,
        "{display}: the span index's definitions must be the graph's nodes, one-to-one"
    );
    assert_eq!(
        definitions.len(),
        expected.len(),
        "{display}: exactly one spanned definition per graph node"
    );
}

/// Asserts the index's definitions are exactly the graph's nodes for
/// `display`, one-to-one (see [`graph_definition_keys`] for the two
/// documented exclusions).
///
/// This is the anti-drift gate every surface's equality test runs.
pub fn assert_definitions_match_graph(index: &SurfaceIndex, graph: &GraphModel, display: &str) {
    let definitions: Vec<(String, String)> = index
        .definitions
        .iter()
        .map(|definition| (definition.kind.clone(), definition.name.clone()))
        .collect();
    assert_definition_keys(&definitions, graph, display);
    for definition in &index.definitions {
        assert_eq!(
            index
                .definitions_of(&definition.kind, &definition.name)
                .len(),
            1,
            "{display}: {} '{}' must have exactly one definition site",
            definition.kind,
            definition.name
        );
    }
}
