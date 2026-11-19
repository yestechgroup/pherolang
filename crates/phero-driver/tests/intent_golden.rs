//! Golden-artifact conformance test for the `.intent` surface (issue
//! pherolang#196): compiling the canonical intent file against its imported
//! domain must produce a [`phero_ir::intents::IntentModel`] whose pretty
//! JSON byte-matches the committed golden artifact, and the artifact must
//! round-trip through `from_json`. Negative cases pin the stable-id and
//! link validation (`E0450`–`E0454`).
//!
//! Regenerate with `PHERO_UPDATE_FIXTURES=1 cargo test -p phero-driver --test
//! intent_golden`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use phero_driver::compile_intent_str;

fn workspace_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../")
            .canonicalize()
            .expect("workspace root")
    })
}

fn fixture_path(relative: &str) -> PathBuf {
    workspace_root().join(relative)
}

const INTENT_PATH: &str = "tests/conformance/intents/delivery.intent";
const ARTIFACT_PATH: &str = "tests/conformance/intents/delivery.intent.json";
const DOMAIN_PATH: &str = "tests/conformance/usecases/delivery.mox";

/// Compiles the conformance intent file against its imported domain,
/// resolving the import exactly the way the CLI does (relative to the
/// `.intent` file).
fn compile_conformance() -> phero_driver::IntentCompilation {
    let absolute = fixture_path(INTENT_PATH);
    let source = std::fs::read_to_string(&absolute).expect("read conformance intent file");
    let domain_source =
        std::fs::read_to_string(fixture_path(DOMAIN_PATH)).expect("read the imported domain");
    let domain = [(fixture_path(DOMAIN_PATH).display().to_string(), domain_source)];
    compile_intent_str(
        absolute.to_str().expect("utf-8 path"),
        &source,
        &phero_driver::CompileInputs::from_domains(&domain),
    )
}

#[test]
fn conformance_intent_file_matches_golden_artifact() {
    let compilation = compile_conformance();
    assert!(
        compilation.diagnostics.is_empty(),
        "conformance intent file must compile clean: {:?}",
        compilation.diagnostics
    );
    let model = compilation.model.expect("intent artifact lowered");
    assert_eq!(
        model.format_version,
        phero_ir::intents::INTENT_MODEL_FORMAT_VERSION
    );
    assert_eq!(
        model.intents.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        ["DELIVERY-001", "DELIVERY-002"]
    );
    let first = &model.intents[0];
    assert_eq!(first.elements.len(), 3);
    assert_eq!(first.checks.len(), 1);
    // The qualified class spelling survives as authored (resolution is the
    // coverage pass's job, never the wire's).
    assert_eq!(first.elements[1].name, "nz.example.delivery.Recipient");
    assert_eq!(first.elements[2].kind, phero_ir::intents::IntentLinkKind::Op);
    assert_eq!(first.checks[0].name, "DeliveryDrone");
    // The compile carries the span table beside the artifact (never
    // serialized): one site per link (4 + 2).
    assert_eq!(compilation.spans.len(), 6);

    let json = model.to_json_pretty().expect("serialize IR");

    let artifact = fixture_path(ARTIFACT_PATH);
    if std::env::var("PHERO_UPDATE_FIXTURES").is_ok() {
        std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        std::fs::write(&artifact, &json).unwrap();
        eprintln!("updated {}", artifact.display());
        return;
    }
    let expected = std::fs::read_to_string(&artifact).unwrap_or_else(|_| {
        panic!(
            "missing golden artifact {}; regenerate with PHERO_UPDATE_FIXTURES=1",
            artifact.display()
        )
    });
    assert_eq!(
        json, expected,
        "intent artifact for {INTENT_PATH} drifted from the golden file"
    );
}

#[test]
fn golden_artifact_round_trips_through_from_json() {
    let expected = std::fs::read_to_string(fixture_path(ARTIFACT_PATH))
        .unwrap_or_else(|error| panic!("read golden artifact {ARTIFACT_PATH}: {error}"));
    let model = phero_ir::intents::IntentModel::from_json(&expected)
        .expect("golden artifact must deserialize");
    assert_eq!(
        model.format_version,
        phero_ir::intents::INTENT_MODEL_FORMAT_VERSION
    );
    let reserialized = model.to_json_pretty().expect("re-serialize IR");
    assert_eq!(reserialized, expected, "artifact round trip must be stable");
}

#[test]
fn fixture_is_fmt_stable() {
    let source = std::fs::read_to_string(fixture_path(INTENT_PATH)).expect("read conformance file");
    let once = phero_syntax::format_intents(&source).expect("format succeeds");
    assert_eq!(once, source, "the conformance file must be canonical");
    let twice = phero_syntax::format_intents(&once).expect("format succeeds");
    assert_eq!(twice, once, "formatting must be a fixpoint");
    let parsed = phero_syntax::parse_intents(&once);
    assert!(
        parsed.errors.is_empty(),
        "formatted output must parse cleanly: {:?}",
        parsed.errors
    );
}

// --- negative cases (E0450–E0454) ---------------------------------------------

fn compile(source: &str) -> phero_driver::IntentCompilation {
    let domain = [(
        "domain.mox".to_string(),
        "package nz.example\n\nclass Widget {}".to_string(),
    )];
    compile_intent_str(
        "bad.intent",
        source,
        &phero_driver::CompileInputs::from_domains(&domain),
    )
}

/// Asserts the compilation is blocked and carries exactly one diagnostic on
/// the `.intent` file whose message starts with `expected`.
fn expect_diagnostic(compilation: &phero_driver::IntentCompilation, expected: &str) {
    assert!(
        compilation.model.is_none(),
        "the negative case must block the artifact"
    );
    let matches: Vec<_> = compilation
        .diagnostics
        .iter()
        .filter(|(path, diagnostic)| path == "bad.intent" && diagnostic.message == expected)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one {expected:?} diagnostic, got {:?}",
        compilation.diagnostics
    );
}

#[test]
fn an_unprovided_import_is_an_error() {
    let source = "import \"nowhere.mox\"\n\nintent LOAN-001 \"x\" {\n    element class:Widget;\n}\n";
    let compilation = compile(source);
    expect_diagnostic(
        &compilation,
        "imported file \"nowhere.mox\" was not provided",
    );
}

#[test]
fn a_malformed_id_is_e0450() {
    // Lowercase lead segment.
    let source = "intent loan-001 \"x\" {\n    element class:Widget;\n}\n";
    let compilation = compile(source);
    assert!(compilation.model.is_none());
    let matches: Vec<_> = compilation
        .diagnostics
        .iter()
        .filter(|(path, diagnostic)| path == "bad.intent" && diagnostic.message.starts_with("E0450"))
        .collect();
    assert_eq!(matches.len(), 1, "got {:?}", compilation.diagnostics);
}

#[test]
fn a_duplicate_in_file_id_is_e0451() {
    let source = concat!(
        "intent LOAN-001 \"x\" {\n",
        "    element class:Widget;\n",
        "}\n",
        "\n",
        "intent LOAN-001 \"y\" {\n",
        "    element class:Widget;\n",
        "}\n",
    );
    let compilation = compile(source);
    expect_diagnostic(&compilation, "E0451 duplicate intent id 'LOAN-001'");
}

#[test]
fn an_empty_statement_is_e0452() {
    let source = "intent LOAN-001 \"   \" {\n    element class:Widget;\n}\n";
    let compilation = compile(source);
    expect_diagnostic(
        &compilation,
        "E0452 intent 'LOAN-001' carries an empty statement",
    );
}

#[test]
fn an_unknown_link_kind_is_e0453() {
    let source = "intent LOAN-001 \"x\" {\n    element widget:Thing;\n}\n";
    let compilation = compile(source);
    expect_diagnostic(
        &compilation,
        "E0453 unknown element kind 'widget' in intent 'LOAN-001'",
    );
}

#[test]
fn a_kind_in_the_wrong_slot_is_e0453() {
    let source = "intent LOAN-001 \"x\" {\n    check class:Widget;\n}\n";
    let compilation = compile(source);
    expect_diagnostic(
        &compilation,
        "E0453 unknown check kind 'class' in intent 'LOAN-001'",
    );
}

#[test]
fn a_duplicate_link_is_e0454() {
    let source = concat!(
        "intent LOAN-001 \"x\" {\n",
        "    element class:Widget;\n",
        "    element class:Widget;\n",
        "}\n",
    );
    let compilation = compile(source);
    expect_diagnostic(
        &compilation,
        "E0454 duplicate element link 'class:Widget' in intent 'LOAN-001'",
    );
}
