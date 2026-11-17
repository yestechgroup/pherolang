//! Characterization tests for the shared host-side import gathering (SK3,
//! epic pherolang#328 / issue pherolang#331).
//!
//! These pin **current behaviour** before the one-walker extraction, so any
//! refactor that changes a message, a dedupe identity, an ordering, or a
//! cycle report fails here first. Every test drives the gatherers through
//! in-memory fetch closures and asserts the *fetch log* too — a duplicate
//! import gathered once and the exact order the host is asked for content
//! are both part of the contract.
//!
//! Surface coverage: `.mox` domain gathering, `.uc`, `.cmmn`, `.deploy`,
//! `.dmn` (events + decision tables), `.ddd` (fragment + application
//! closure), `.actor`/`.evt` nested model gathering, and `.ifml` (the
//! two-key project walk).

use std::collections::BTreeMap;

use phero_driver::project::{
    dmn_import_paths, gather_case_imports, gather_ddd_app_imports, gather_ddd_fragments,
    gather_deploy_imports, gather_dmn_events, gather_domain_sources, gather_ifml_project,
    gather_usecase_imports,
};

const WIDGET_MOX: &str = "package test.widgets\n\nclass Widget {\n    id readonly String widgetId\n    String label\n}\n";

const GADGET_MOX: &str = "package test.gadgets\n\nclass Gadget {\n    id readonly String gadgetId\n    String label\n}\n";

const OPS_ACTOR: &str = "import \"widgets.mox\"\n\nactors Ops {\n    actor Admin\n\n    capability Touch on Widget\n\n    grant Admin {\n        permit Touch\n    }\n}\n";

const DESIGN_DDD: &str = "import \"widgets.mox\";\n\napplication App {\n    base test.widgets\n    module core {\n        service WidgetService {\n            inject WidgetRepository;\n            Widget find(String widgetId);\n            findOne => WidgetRepository.findById;\n        }\n        entity Widget scaffold repository WidgetRepository {\n            findById;\n        }\n    }\n}\n";

const OPS_EVT: &str = "import \"widgets.mox\"\n\nevent WidgetMade version \"1.0.0\" {\n    widgetId: String;\n}\n\nchannel ops {\n    publishes WidgetMade;\n}\n";

const GUI_IFML: &str = "import \"pager.ifml\";\n\nmodule \"Pager\" {\n    input { page: Int }\n    output { total: Int }\n}\n";

const PAGER_IFML: &str = "view \"PagerPage\" {}\n";

const WIDGET_DMN: &str = "import \"widgets.mox\"\n\ndecision_model WidgetDecisions {\n    namespace test.decisions\n    inputs {\n        widget: Widget;\n    }\n    decisions {\n        decision WidgetVerdict {\n            outputs {\n                verdict: string;\n            }\n            logic table \"./tables/verdict.md\"\n        }\n    }\n    services {\n        service AssessWidget {\n            inputs { widget }\n            outputs { WidgetVerdict }\n        }\n    }\n}\n";

const VERDICT_TABLE: &str =
    "> # Widget verdict\n\n| U | verdict |\n|:-:|:-------:|\n|   |   `o`   |\n| 1 | \"ok\"   |\n";

/// A path-keyed host: `resolved path -> text`, logging every fetch so a
/// test can assert both the dedupe behaviour and the request order.
fn logging_path_fetch<'a>(
    paths: &'a BTreeMap<String, String>,
    log: &'a mut Vec<String>,
) -> impl FnMut(&str) -> Option<String> + 'a {
    move |key: &str| {
        log.push(key.to_string());
        paths.get(key).cloned()
    }
}

/// A two-key host for the `.ifml` walk, logging `(importer, import)`.
fn logging_pair_fetch<'a>(
    pairs: &'a BTreeMap<(String, String), (String, String)>,
    log: &'a mut Vec<(String, String)>,
) -> impl FnMut(&str, &str) -> Option<(String, String)> + 'a {
    move |importer: &str, import: &str| {
        log.push((importer.to_string(), import.to_string()));
        pairs
            .get(&(importer.to_string(), import.to_string()))
            .map(|(key, text)| (key.clone(), text.clone()))
    }
}

fn owned(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(key, text)| ((*key).to_string(), (*text).to_string()))
        .collect()
}

/// One `((importer, import), (resolved key, text))` pair fixture.
type PairFixture<'a> = ((&'a str, &'a str), (&'a str, &'a str));

fn owned_pairs(entries: &[PairFixture<'_>]) -> BTreeMap<(String, String), (String, String)> {
    entries
        .iter()
        .map(|((importer, import), (key, text))| {
            (
                ((*importer).to_string(), (*import).to_string()),
                ((*key).to_string(), (*text).to_string()),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// `.mox` domain gathering (`gather_domain_sources`)
// ---------------------------------------------------------------------------

#[test]
fn domain_gather_missing_is_a_silent_skip_and_order_is_source_order() {
    let paths = owned(&[
        ("lib/widgets.mox", WIDGET_MOX),
        ("lib/gadgets.mox", GADGET_MOX),
    ]);
    let mut log = Vec::new();
    let mut fetch = logging_path_fetch(&paths, &mut log);
    let domains = gather_domain_sources(
        "lib/app.mox",
        "// irrelvant",
        |_| {
            vec![
                "gadgets.mox".to_string(),
                "missing.mox".to_string(),
                "widgets.mox".to_string(),
            ]
        },
        &mut fetch,
    );
    assert_eq!(
        domains,
        vec![
            ("lib/gadgets.mox".to_string(), GADGET_MOX.to_string()),
            ("lib/widgets.mox".to_string(), WIDGET_MOX.to_string()),
        ],
        "a missing import skips silently; the rest keep source order"
    );
    drop(fetch);
    assert_eq!(
        log,
        vec![
            "lib/gadgets.mox".to_string(),
            "lib/missing.mox".to_string(),
            "lib/widgets.mox".to_string(),
        ],
        "every import is fetched exactly once, in source order"
    );
}

#[test]
fn domain_gather_dedups_identical_import_strings_by_resolved_key() {
    let paths = owned(&[("lib/widgets.mox", WIDGET_MOX)]);
    let mut log = Vec::new();
    let mut fetch = logging_path_fetch(&paths, &mut log);
    let domains = gather_domain_sources(
        "lib/app.mox",
        "",
        |_| vec!["widgets.mox".to_string(), "widgets.mox".to_string()],
        &mut fetch,
    );
    assert_eq!(domains.len(), 1);
    drop(fetch);
    assert_eq!(
        log,
        vec!["lib/widgets.mox".to_string()],
        "the duplicate is never fetched twice"
    );
}

/// The lexical resolution rule is exact and unnormalised:
/// `dir_of(importer).join(written)`, no `./` collapsing. Two spellings of
/// one file are two resolved keys (and the second is a miss here), which is
/// the behaviour the refactor must not "improve".
#[test]
fn domain_gather_resolves_lexically_without_normalising() {
    let paths = owned(&[("lib/widgets.mox", WIDGET_MOX)]);
    let mut log = Vec::new();
    let mut fetch = logging_path_fetch(&paths, &mut log);
    let domains = gather_domain_sources(
        "lib/app.mox",
        "",
        |_| vec!["widgets.mox".to_string(), "./widgets.mox".to_string()],
        &mut fetch,
    );
    assert_eq!(
        domains,
        vec![("lib/widgets.mox".to_string(), WIDGET_MOX.to_string())]
    );
    drop(fetch);
    assert_eq!(
        log,
        vec![
            "lib/widgets.mox".to_string(),
            "lib/./widgets.mox".to_string()
        ],
        "`./widgets.mox` resolves to `lib/./widgets.mox`, not `lib/widgets.mox`"
    );
}

// ---------------------------------------------------------------------------
// `.uc` (`gather_usecase_imports`)
// ---------------------------------------------------------------------------

fn path_host<'a>(
    paths: &'a BTreeMap<String, String>,
    log: &'a mut Vec<String>,
) -> impl FnMut(&str) -> Option<String> + 'a {
    logging_path_fetch(paths, log)
}

#[test]
fn uc_missing_import_for_every_bucket_is_the_pinned_message() {
    for missing in [
        "missing-domain.mox",
        "missing.ddd",
        "missing.actor",
        "missing.evt",
    ] {
        let paths = owned(&[]);
        let mut log = Vec::new();
        let mut fetch = path_host(&paths, &mut log);
        let error = gather_usecase_imports(
            "lib/app.uc",
            "// uc",
            |_| vec![missing.to_string()],
            &mut fetch,
        )
        .unwrap_err();
        assert_eq!(
            error,
            format!("cannot read imported file lib/{missing} (imported by lib/app.uc)"),
            "{missing} keeps the surface message"
        );
    }
}

#[test]
fn uc_buckets_keep_source_order_and_dedupe_per_bucket() {
    let paths = owned(&[
        ("lib/widgets.mox", WIDGET_MOX),
        ("lib/gadgets.mox", GADGET_MOX),
        ("lib/app.ddd", DESIGN_DDD),
        ("lib/ops.actor", OPS_ACTOR),
        ("lib/ops.evt", OPS_EVT),
    ]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let buckets = gather_usecase_imports(
        "lib/app.uc",
        "// uc",
        |_| {
            vec![
                "widgets.mox".to_string(),
                "app.ddd".to_string(),
                "gadgets.mox".to_string(),
                "widgets.mox".to_string(),
                "ops.evt".to_string(),
                "app.ddd".to_string(),
                "ops.actor".to_string(),
            ]
        },
        &mut fetch,
    )
    .expect("every import gathers and compiles");
    assert_eq!(
        buckets
            .domains
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["lib/widgets.mox", "lib/gadgets.mox"],
        "domains keep source order, duplicates gathered once"
    );
    assert_eq!(buckets.designs.len(), 1);
    assert_eq!(buckets.events.len(), 1);
    assert_eq!(buckets.actors.len(), 1);
    // The duplicate `app.ddd` is never fetched twice; nested domain fetches
    // follow their artifact.
    drop(fetch);
    assert_eq!(
        log,
        vec![
            "lib/widgets.mox".to_string(),
            "lib/app.ddd".to_string(),
            "lib/widgets.mox".to_string(),
            "lib/gadgets.mox".to_string(),
            "lib/ops.evt".to_string(),
            "lib/widgets.mox".to_string(),
            "lib/ops.actor".to_string(),
            "lib/widgets.mox".to_string(),
        ],
        "the fetch log pins both dedupe and request order"
    );
}

#[test]
fn uc_nested_artifact_domain_miss_names_the_artifact_as_importer() {
    // `app.ddd` imports `extra.mox`, which the host cannot provide: the
    // nested gather's strict contract names the artifact, not the `.uc`.
    const DESIGN_WITH_EXTRA: &str = "import \"widgets.mox\";\nimport \"extra.mox\";\n\napplication App {\n    base test.widgets\n    module core {\n        entity Widget;\n    }\n}\n";
    let paths = owned(&[
        ("lib/app.ddd", DESIGN_WITH_EXTRA),
        ("lib/widgets.mox", WIDGET_MOX),
    ]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let error = gather_usecase_imports(
        "lib/app.uc",
        "// uc",
        |_| vec!["app.ddd".to_string()],
        &mut fetch,
    )
    .unwrap_err();
    assert_eq!(
        error,
        "cannot read imported file lib/extra.mox (imported by lib/app.ddd)"
    );
}

#[test]
fn uc_non_compiling_artifact_messages_name_kind_and_key() {
    // `.actor` fails: unknown capability target.
    const BROKEN_ACTOR: &str = "import \"widgets.mox\"\n\nactors Ops {\n    actor Admin\n\n    capability Touch on Nope\n\n    grant Admin {\n        permit Touch\n    }\n}\n";
    let paths = owned(&[
        ("lib/ops.actor", BROKEN_ACTOR),
        ("lib/widgets.mox", WIDGET_MOX),
    ]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let error = gather_usecase_imports(
        "lib/app.uc",
        "// uc",
        |_| vec!["ops.actor".to_string()],
        &mut fetch,
    )
    .unwrap_err();
    assert!(
        error.starts_with("imported actor model lib/ops.actor does not compile"),
        "unexpected: {error}"
    );

    // `.ddd` fails: a service refers to a design that is not designed.
    const BROKEN_DDD: &str = "application App {\n    base test.widgets\n    module core {\n        service WidgetService {\n            inject WidgetRepository;\n            Widget find(String widgetId);\n            findOne => WidgetRepository.findById;\n        }\n    }\n}\n";
    let paths = owned(&[("lib/app.ddd", BROKEN_DDD)]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let error = gather_usecase_imports(
        "lib/app.uc",
        "// uc",
        |_| vec!["app.ddd".to_string()],
        &mut fetch,
    )
    .unwrap_err();
    assert!(
        error.starts_with("imported design lib/app.ddd does not compile"),
        "unexpected: {error}"
    );

    // `.evt` fails: an event with a concrete body member instead of a
    // declared field type.
    const BROKEN_EVT: &str =
        "import \"widgets.mox\"\n\nevent WidgetMade version \"1.0.0\" {\n    widgetId: Nope;\n}\n";
    let paths = owned(&[("lib/ops.evt", BROKEN_EVT), ("lib/widgets.mox", WIDGET_MOX)]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let error = gather_usecase_imports(
        "lib/app.uc",
        "// uc",
        |_| vec!["ops.evt".to_string()],
        &mut fetch,
    )
    .unwrap_err();
    assert!(
        error.starts_with("imported contract lib/ops.evt does not compile"),
        "unexpected: {error}"
    );
}

// ---------------------------------------------------------------------------
// `.cmmn` (`gather_case_imports`)
// ---------------------------------------------------------------------------

#[test]
fn cmmn_decisions_gather_in_source_order_once() {
    let paths = owned(&[
        ("lib/widgets.mox", WIDGET_MOX),
        ("lib/verdicts.dmn", WIDGET_DMN),
        ("lib/./tables/verdict.md", VERDICT_TABLE),
    ]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let buckets = gather_case_imports(
        "lib/case.cmmn",
        "// cmmn",
        &|_| {
            vec![
                "verdicts.dmn".to_string(),
                "widgets.mox".to_string(),
                "verdicts.dmn".to_string(),
            ]
        },
        &mut fetch,
    )
    .expect("the case gathers");
    assert_eq!(buckets.domains.len(), 1);
    assert_eq!(buckets.decisions.len(), 1);
    assert_eq!(buckets.decisions[0].0, "lib/verdicts.dmn");
    drop(fetch);
    assert_eq!(
        log,
        vec![
            "lib/widgets.mox".to_string(),
            "lib/verdicts.dmn".to_string(),
            "lib/widgets.mox".to_string(),
            "lib/./tables/verdict.md".to_string(),
        ],
        "the four use-case buckets gather first, then the decision pass \
         gathers the duplicate exactly once (its nested domain and table follow)"
    );
}

#[test]
fn cmmn_missing_decision_message_is_the_pinned_read_error() {
    let paths = owned(&[]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let error = gather_case_imports(
        "lib/case.cmmn",
        "// cmmn",
        &|_| vec!["missing.dmn".to_string()],
        &mut fetch,
    )
    .unwrap_err();
    assert_eq!(
        error,
        "cannot read imported file lib/missing.dmn (imported by lib/case.cmmn)"
    );
}

/// The `.cmmn` gather is **two-pass**: the four use-case buckets first, then
/// the decision pass over the same import list. A missing domain therefore
/// wins over an earlier-in-source missing decision model — the error order
/// is part of the contract.
#[test]
fn cmmn_error_order_is_the_four_bucket_pass_then_the_decision_pass() {
    let paths = owned(&[]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let error = gather_case_imports(
        "lib/case.cmmn",
        "// cmmn",
        &|_| vec!["missing.dmn".to_string(), "missing-domain.mox".to_string()],
        &mut fetch,
    )
    .unwrap_err();
    assert_eq!(
        error, "cannot read imported file lib/missing-domain.mox (imported by lib/case.cmmn)",
        "the bucket pass runs before the decision pass"
    );
}

/// A `.dmn` model's unprovided decision table is not a read error: the
/// model compiles (or, here, fails its own table rule) and the pinned
/// message names the decision model.
#[test]
fn cmmn_dmn_model_without_its_table_names_the_model_not_the_table() {
    let paths = owned(&[
        ("lib/verdicts.dmn", WIDGET_DMN),
        ("lib/widgets.mox", WIDGET_MOX),
    ]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let error = gather_case_imports(
        "lib/case.cmmn",
        "// cmmn",
        &|_| vec!["verdicts.dmn".to_string()],
        &mut fetch,
    )
    .unwrap_err();
    assert!(
        error.starts_with("imported decision model lib/verdicts.dmn does not compile"),
        "unexpected: {error}"
    );
    drop(fetch);
    assert_eq!(
        log,
        vec![
            "lib/verdicts.dmn".to_string(),
            "lib/widgets.mox".to_string(),
            "lib/./tables/verdict.md".to_string(),
        ],
        "the table is fetched under its resolved path after the domains"
    );
}

// ---------------------------------------------------------------------------
// `.deploy` (`gather_deploy_imports`)
// ---------------------------------------------------------------------------

#[test]
fn deploy_pairs_keep_source_order_and_dedupe() {
    let paths = owned(&[
        ("lib/widgets.mox", WIDGET_MOX),
        ("lib/app.ddd", DESIGN_DDD),
        ("lib/gui.ifml", GUI_IFML),
        ("lib/pager.ifml", PAGER_IFML),
    ]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let pairs = gather_deploy_imports(
        "lib/svc.deploy",
        "// deploy",
        |_| {
            vec![
                "app.ddd".to_string(),
                "gui.ifml".to_string(),
                "app.ddd".to_string(),
                "gui.ifml".to_string(),
            ]
        },
        &mut fetch,
    )
    .expect("both pairs gather and compile");
    assert_eq!(
        pairs
            .designs
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>(),
        vec!["lib/app.ddd"]
    );
    assert_eq!(
        pairs
            .flows
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>(),
        vec!["lib/gui.ifml"]
    );
    drop(fetch);
    assert_eq!(
        log,
        vec![
            "lib/app.ddd".to_string(),
            "lib/widgets.mox".to_string(),
            "lib/gui.ifml".to_string(),
            "lib/pager.ifml".to_string(),
        ],
        "duplicate deploy imports are fetched once"
    );
}

#[test]
fn deploy_missing_import_messages_are_the_pinned_read_error() {
    for missing in ["missing.ddd", "missing.ifml"] {
        let paths = owned(&[]);
        let mut log = Vec::new();
        let mut fetch = path_host(&paths, &mut log);
        let error = gather_deploy_imports(
            "lib/svc.deploy",
            "// deploy",
            |_| vec![missing.to_string()],
            &mut fetch,
        )
        .unwrap_err();
        assert_eq!(
            error,
            format!("cannot read imported file lib/{missing} (imported by lib/svc.deploy)")
        );
    }
}

#[test]
fn deploy_non_compiling_flow_names_the_flow_key() {
    // The flow imports a module the host does not provide: the walk leaves
    // it unresolved and `compile_ifml_str` fails, which the deploy gather
    // reports as the pinned "imported flow" message.
    let paths = owned(&[(
        "lib/broken.ifml",
        "import \"nope.ifml\";\n\nview \"Home\" {}\n",
    )]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let error = gather_deploy_imports(
        "lib/svc.deploy",
        "// deploy",
        |_| vec!["broken.ifml".to_string()],
        &mut fetch,
    )
    .unwrap_err();
    assert_eq!(error, "imported flow lib/broken.ifml does not compile");
}

// ---------------------------------------------------------------------------
// `.dmn` (`gather_dmn_events`, `dmn_import_paths`)
// ---------------------------------------------------------------------------

#[test]
fn dmn_import_paths_are_as_written_in_source_order() {
    const DMN: &str = "import \"widgets.mox\"\nimport \"ops.evt\"\n\ndecision_model D {\n    namespace test.d\n}\n";
    assert_eq!(
        dmn_import_paths(DMN),
        vec!["widgets.mox".to_string(), "ops.evt".to_string()]
    );
}

/// A contract with no imports of its own, so the written-key bucket is
/// observed without a nested domain gather interleaving.
const PLAIN_EVT: &str = "event WidgetMade version \"1.0.0\" {\n    widgetId: String;\n}\n";

#[test]
fn dmn_events_are_keyed_as_written_and_fetched_as_resolved() {
    const DMN: &str =
        "import \"contracts/ops.evt\"\n\ndecision_model D {\n    namespace test.d\n}\n";
    let paths = owned(&[
        ("lib/contracts/ops.evt", OPS_EVT),
        ("lib/contracts/widgets.mox", WIDGET_MOX),
    ]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let events = gather_dmn_events("lib/verdicts.dmn", DMN, &mut fetch).expect("events gather");
    assert_eq!(
        events
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["contracts/ops.evt"],
        "the bucket key is the path exactly as written"
    );
    drop(fetch);
    assert_eq!(
        log,
        vec![
            "lib/contracts/ops.evt".to_string(),
            "lib/contracts/widgets.mox".to_string()
        ],
        "the fetch sees the lexically resolved path; the contract's own \
         `.mox` import resolves against the contract"
    );
}

#[test]
fn dmn_events_dedupe_by_the_written_path_only() {
    const DMN: &str = "import \"ops.evt\"\nimport \"ops.evt\"\nimport \"./ops.evt\"\n\ndecision_model D {\n    namespace test.d\n}\n";
    let paths = owned(&[("lib/ops.evt", PLAIN_EVT), ("lib/./ops.evt", PLAIN_EVT)]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let events = gather_dmn_events("lib/verdicts.dmn", DMN, &mut fetch).expect("events gather");
    assert_eq!(
        events
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["ops.evt", "./ops.evt"],
        "two spellings are two written keys (and the same file compiles twice)"
    );
    drop(fetch);
    assert_eq!(
        log,
        vec!["lib/ops.evt".to_string(), "lib/./ops.evt".to_string()],
        "the identical second `ops.evt` is never fetched; the `./ops.evt` \
         spelling is a distinct bucket key"
    );
}

#[test]
fn dmn_event_miss_and_non_compiling_contract_messages() {
    const MISSING: &str = "import \"missing.evt\"\n\ndecision_model D {\n    namespace test.d\n}\n";
    let paths = owned(&[]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let error = gather_dmn_events("lib/verdicts.dmn", MISSING, &mut fetch).unwrap_err();
    assert_eq!(
        error,
        "cannot read imported file lib/missing.evt (imported by lib/verdicts.dmn)"
    );

    const BROKEN: &str = "import \"ops.evt\"\n\ndecision_model D {\n    namespace test.d\n}\n";
    let paths = owned(&[("lib/ops.evt", "event Bad { x: Nope; }\n")]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let error = gather_dmn_events("lib/verdicts.dmn", BROKEN, &mut fetch).unwrap_err();
    assert!(
        error.starts_with("imported contract lib/ops.evt does not compile"),
        "unexpected: {error}"
    );
}

// ---------------------------------------------------------------------------
// `.ddd` fragment + application closure
// ---------------------------------------------------------------------------

const ROOT_DDD: &str = "import \"demo.mox\";\nimport \"sub/catalogue.ddd\";\n\napplication Shop {\n    base demo\n    module orders {\n        entity Order repository OrderRepository { findById; }\n    }\n}\n";

const FRAGMENT_DDD: &str =
    "import \"product.mox\";\n\nmodule catalogue {\n    entity Product repository ProductRepository { findById; }\n}\n";

const ORDER_MOX: &str = "package demo\n\nclass Order {\n    id readonly String orderId\n}\n";

const PRODUCT_MOX: &str =
    "package demo.catalogue\n\nclass Product {\n    id readonly String productId\n}\n";

#[test]
fn ddd_fragments_are_post_order_and_dedup_once() {
    let paths = owned(&[
        ("./sub/catalogue.ddd", FRAGMENT_DDD),
        ("./sub/a.ddd", "module a { entity A; }\n"),
    ]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let fragments = gather_ddd_fragments(
        "app.ddd",
        "import \"sub/catalogue.ddd\";\nimport \"sub/a.ddd\";\nimport \"sub/catalogue.ddd\";\n",
        &mut fetch,
    );
    assert_eq!(
        fragments
            .fragments
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["./sub/catalogue.ddd", "./sub/a.ddd"],
        "fragments land in first-visit order; the duplicate is never fetched again"
    );
    drop(fetch);
    assert_eq!(
        log,
        vec!["./sub/catalogue.ddd".to_string(), "./sub/a.ddd".to_string()],
        "the nested `.mox` import of a fragment is a domain, not a fragment"
    );
    assert_eq!(
        fragments
            .sites
            .iter()
            .map(|site| (
                site.importer.as_str(),
                site.import.as_str(),
                site.resolved.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("app.ddd", "sub/catalogue.ddd", "./sub/catalogue.ddd"),
            ("app.ddd", "sub/a.ddd", "./sub/a.ddd"),
            ("app.ddd", "sub/catalogue.ddd", "./sub/catalogue.ddd"),
        ],
        "every written `.ddd` import is a site, duplicates included"
    );
}

#[test]
fn ddd_app_closure_orders_root_domains_then_fragment_domains() {
    let paths = owned(&[
        ("./demo.mox", ORDER_MOX),
        ("./sub/catalogue.ddd", FRAGMENT_DDD),
        ("./sub/product.mox", PRODUCT_MOX),
    ]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let gathered = gather_ddd_app_imports("app.ddd", ROOT_DDD, &mut fetch);
    assert_eq!(gathered.fragments.len(), 1);
    assert_eq!(
        gathered
            .domains
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["./demo.mox", "./sub/product.mox"],
        "the root's domains precede its fragments', each resolved against \
         its declaring file"
    );
    assert!(gathered.missing.is_empty());
    assert!(gathered.missing_domains.is_empty());
    drop(fetch);
    assert_eq!(
        log,
        vec![
            "./sub/catalogue.ddd".to_string(),
            "./demo.mox".to_string(),
            "./sub/product.mox".to_string(),
        ],
        "fragments are walked first, then the root's domains, then each \
         fragment's domains"
    );
}

#[test]
fn ddd_app_closure_reports_missing_fragments_domains_and_cycles() {
    // A missing fragment is recorded as written; a missing domain as
    // `(resolved, declaring file)`.
    let paths = owned(&[]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let gathered = gather_ddd_app_imports(
        "app.ddd",
        "import \"missing.ddd\";\nimport \"also-missing.mox\";\n",
        &mut fetch,
    );
    assert_eq!(gathered.missing, vec!["missing.ddd".to_string()]);
    assert_eq!(
        gathered.missing_domains,
        vec![("./also-missing.mox".to_string(), "app.ddd".to_string())]
    );

    // A cycle through the root: `a.ddd` imports the root back under its
    // resolved key, and the root re-enters `./a.ddd` — the path that
    // closed the cycle.
    let root = "import \"a.ddd\";\n";
    let a = "import \"app.ddd\";\n";
    let paths = owned(&[("./a.ddd", a), ("./app.ddd", root)]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let gathered = gather_ddd_app_imports("app.ddd", root, &mut fetch);
    assert_eq!(
        gathered.cycles,
        vec!["./a.ddd".to_string()],
        "the cycle is reported once, as the resolved path that closed it"
    );
    assert_eq!(
        gathered
            .fragments
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["./app.ddd", "./a.ddd"],
        "the re-entered root is gathered as a fragment, as it always has been"
    );
}

#[test]
fn a_use_case_reporting_a_design_cycle_keeps_the_pinned_message() {
    let root = "import \"a.ddd\";\n";
    let a = "import \"app.ddd\";\n";
    let paths = owned(&[("./a.ddd", a), ("./app.ddd", root)]);
    let mut log = Vec::new();
    let mut fetch = path_host(&paths, &mut log);
    let error = gather_usecase_imports(
        "use.uc",
        "// uc",
        |_| vec!["app.ddd".to_string()],
        &mut fetch,
    )
    .unwrap_err();
    assert_eq!(error, "design import cycle through ./app.ddd");
}

// ---------------------------------------------------------------------------
// `.ifml` (the two-key project walk)
// ---------------------------------------------------------------------------

#[test]
fn ifml_walk_keeps_entry_first_source_order_and_two_key_identities() {
    const ENTRY: &str = "import \"gui.ifml\";\nimport \"widgets.mox\";\nimport \"widgets.mox\";\n";

    let pairs = owned_pairs(&[
        (("app.ifml", "gui.ifml"), ("lib/gui.ifml", GUI_IFML)),
        (("gui.ifml", "pager.ifml"), ("lib/pager.ifml", PAGER_IFML)),
        (("app.ifml", "widgets.mox"), ("lib/widgets.mox", WIDGET_MOX)),
    ]);
    let mut log = Vec::new();
    let mut fetch = logging_pair_fetch(&pairs, &mut log);
    let project = gather_ifml_project("app.ifml", ENTRY, &mut fetch);
    assert_eq!(
        project
            .sources
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>(),
        vec!["app.ifml", "gui.ifml", "pager.ifml"],
        "the walk is depth-first in source order, entry first"
    );
    assert_eq!(
        project
            .domains
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["lib/widgets.mox"],
        "the duplicate `.mox` import is deduplicated by resolved key"
    );
    assert!(project.unresolved.is_empty());
    drop(fetch);
    assert_eq!(
        log,
        vec![
            ("app.ifml".to_string(), "gui.ifml".to_string()),
            ("gui.ifml".to_string(), "pager.ifml".to_string()),
            ("app.ifml".to_string(), "widgets.mox".to_string()),
            ("app.ifml".to_string(), "widgets.mox".to_string()),
        ],
        "the second `.mox` import is offered to the host again (bundle \
         dedupe is by resolved key, the fetch is not skipped)"
    );
}

#[test]
fn ifml_cycle_is_projected_by_the_walker_without_error() {
    // The intra-`.ifml` cycle is `phero-ifml`'s walker: it stops at the
    // active identity and the driver's projection is the sources it saw.
    // The driver never reports the cycle itself (the resolver does).
    const A: &str = "import \"b.ifml\";\nview \"A\" {}\n";
    const B: &str = "import \"app.ifml\";\nview \"B\" {}\n";
    let pairs = owned_pairs(&[
        (("app.ifml", "b.ifml"), ("lib/b.ifml", B)),
        (("b.ifml", "app.ifml"), ("lib/app.ifml", A)),
    ]);
    let mut log = Vec::new();
    let mut fetch = logging_pair_fetch(&pairs, &mut log);
    let project = gather_ifml_project("app.ifml", A, &mut fetch);
    assert_eq!(
        project
            .sources
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>(),
        vec!["app.ifml", "b.ifml"],
        "the walk stops at the active identity"
    );
    assert_eq!(
        project.imports.get("b.ifml", "app.ifml"),
        Some(A),
        "the cycle edge is still provided to the resolver"
    );
    assert!(project.unresolved.is_empty(), "{:?}", project.unresolved);
}

#[test]
fn ifml_actors_and_decisions_carry_their_nested_gathers() {
    const ENTRY: &str = "import \"ops.actor\";\nimport \"verdicts.dmn\";\nimport \"missing.actor\";\nimport \"notes.txt\";\n";
    let pairs = owned_pairs(&[
        (("app.ifml", "ops.actor"), ("lib/ops.actor", OPS_ACTOR)),
        (
            ("lib/ops.actor", "widgets.mox"),
            ("lib/widgets.mox", WIDGET_MOX),
        ),
        (
            ("app.ifml", "verdicts.dmn"),
            ("lib/verdicts.dmn", WIDGET_DMN),
        ),
        (
            ("lib/verdicts.dmn", "widgets.mox"),
            ("lib/widgets.mox", WIDGET_MOX),
        ),
        (
            ("lib/verdicts.dmn", "./tables/verdict.md"),
            ("lib/tables/verdict.md", VERDICT_TABLE),
        ),
    ]);
    let mut log = Vec::new();
    let mut fetch = logging_pair_fetch(&pairs, &mut log);
    let project = gather_ifml_project("app.ifml", ENTRY, &mut fetch);

    assert_eq!(project.actors.len(), 1);
    assert_eq!(project.actors[0].key, "lib/ops.actor");
    assert_eq!(
        project.actors[0].domains,
        vec![("lib/widgets.mox".to_string(), WIDGET_MOX.to_string())]
    );

    assert_eq!(project.decisions.len(), 1);
    assert_eq!(project.decisions[0].key, "lib/verdicts.dmn");
    assert_eq!(
        project.decisions[0].domains,
        vec![("lib/widgets.mox".to_string(), WIDGET_MOX.to_string())]
    );
    assert_eq!(
        project.decisions[0].tables,
        vec![("./tables/verdict.md".to_string(), VERDICT_TABLE.to_string())],
        "decision tables are keyed by the path as written"
    );
    assert_eq!(
        project.unresolved,
        vec![("app.ifml".to_string(), "missing.actor".to_string())],
        "unknown extensions never land in `unresolved`"
    );
    drop(fetch);
    assert_eq!(
        log,
        vec![
            ("app.ifml".to_string(), "ops.actor".to_string()),
            ("lib/ops.actor".to_string(), "widgets.mox".to_string()),
            ("app.ifml".to_string(), "verdicts.dmn".to_string()),
            ("lib/verdicts.dmn".to_string(), "widgets.mox".to_string()),
            (
                "lib/verdicts.dmn".to_string(),
                "./tables/verdict.md".to_string()
            ),
            ("app.ifml".to_string(), "missing.actor".to_string()),
            ("app.ifml".to_string(), "notes.txt".to_string()),
        ],
        "nested gathers use the resolved key as the importer identity; \
         every import is offered to the host in walk order"
    );
}

#[test]
fn ifml_unresolved_keeps_walk_order_for_surface_imports() {
    const ENTRY: &str = "import \"gone.ifml\";\nimport \"gone.mox\";\nimport \"gone.actor\";\nimport \"gone.dmn\";\nimport \"gone.txt\";\n";
    let pairs = owned_pairs(&[]);
    let mut log = Vec::new();
    let mut fetch = logging_pair_fetch(&pairs, &mut log);
    let project = gather_ifml_project("app.ifml", ENTRY, &mut fetch);
    assert_eq!(
        project.unresolved,
        vec![
            ("app.ifml".to_string(), "gone.ifml".to_string()),
            ("app.ifml".to_string(), "gone.mox".to_string()),
            ("app.ifml".to_string(), "gone.actor".to_string()),
            ("app.ifml".to_string(), "gone.dmn".to_string()),
        ],
        "surface imports land in walk order; unknown extensions are ignored"
    );
}
