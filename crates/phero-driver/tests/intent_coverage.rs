//! The intent coverage pass (issue pherolang#196): pinned behavior over
//! hand-built application artifacts — every intent maps to ≥ 1 resolved
//! element and ≥ 1 resolved check, unmapped intents **warn** (honesty over
//! reach), unknown links error with the did-you-mean candidate, ambiguous
//! bare names error, and cross-file id collisions error. The register row
//! for this classification is `V18` (`tests/mutants/V18.patch` attacks the
//! mapped/unmapped classification directly; these tests are its kill).

use std::collections::BTreeMap;

use phero_driver::coverage::check_coverage;
use phero_driver::compile_intent_str;
use phero_ir::ddd::{Design, DddModel, Invariant, Module, Service, ServiceOperation, Stereotype};
use phero_ir::dmn::{Decision, DmnModel};
use phero_ir::intents::IntentModel;
use phero_ir::{ClassDef, Model, Package, TypeRef};

/// The domain union: two packages, each with a `Widget` (the ambiguity
/// case) plus a unique `Gadget`.
fn domains() -> Vec<Model> {
    let package = |name: &str| Package {
        name: name.to_string(),
        description: None,
        annotations: Vec::new(),
        enums: Vec::new(),
        datatypes: Vec::new(),
        interfaces: Vec::new(),
        classes: vec![
            ClassDef::new("Widget", Vec::new(), Vec::new()),
            ClassDef::new("Gadget", Vec::new(), Vec::new()),
        ],
        functions: Vec::new(),
        actors: Vec::new(),
        vocabularies: Vec::new(),
    };
    vec![Model {
        format_version: phero_ir::FORMAT_VERSION,
        phero_version: None,
        packages: vec![package("nz.example.shop"), package("nz.example.other")],
    }]
}

/// A design artifact: `Widget` designed as an entity carrying one invariant
/// (its resolved identity in the first package), plus a service declaring
/// `confirm`.
fn designs() -> Vec<(String, DddModel)> {
    let mut design = Design::new("Widget", Stereotype::Entity);
    design.type_ref = Some(TypeRef::Class {
        package: "nz.example.shop".to_string(),
        name: "Widget".to_string(),
    });
    design.invariants.push(Invariant::new("count > 0"));
    let mut service = Service::new("ShopService");
    service.operations.push(ServiceOperation::new("confirm"));
    let mut module = Module::new("shop");
    module.services.push(service);
    module.designs.push(design);
    let mut model = DddModel::new();
    model.modules.push(module);
    vec![("design.ddd".to_string(), model)]
}

/// A decision artifact declaring `ReleaseAssessment` and `ReleaseApproval`.
fn decisions() -> Vec<(String, DmnModel)> {
    let decision = |name: &str| Decision {
        name: name.to_string(),
        description: None,
        outputs: Vec::new(),
        logic: None,
        emits: Vec::new(),
    };
    let mut model = DmnModel::new();
    model.name = Some("ShopDecisions".to_string());
    model.decisions = vec![decision("ReleaseAssessment"), decision("ReleaseApproval")];
    vec![("shop.dmn".to_string(), model)]
}

/// Compiles one `.intent` source (the real compile path, spans included).
fn compile_intents(
    path: &str,
    source: &str,
) -> (
    String,
    IntentModel,
    Vec<phero_driver::IntentLinkSite>,
) {
    let domain = [(
        "domain.mox".to_string(),
        "package nz.example.shop\n\nclass Widget {}\n\nclass Gadget {}".to_string(),
    )];
    let compilation = compile_intent_str(
        path,
        source,
        &phero_driver::CompileInputs::from_domains(&domain),
    );
    assert!(
        compilation.diagnostics.is_empty(),
        "{path} must compile clean: {:?}",
        compilation.diagnostics
    );
    (
        path.to_string(),
        compilation.model.expect("intent artifact"),
        compilation.spans,
    )
}

fn spans_map(
    entries: &[(String, IntentModel, Vec<phero_driver::IntentLinkSite>)],
) -> BTreeMap<String, Vec<phero_driver::IntentLinkSite>> {
    entries
        .iter()
        .map(|(path, _, spans)| (path.clone(), spans.clone()))
        .collect()
}

fn messages(diagnostics: &[(String, phero_driver::Diagnostic)]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|(_, diagnostic)| diagnostic.message.clone())
        .collect()
}

#[test]
fn full_coverage_with_every_vocabulary_is_clean() {
    let entry = compile_intents(
        "good.intent",
        concat!(
            "intent SHOP-001 \"every widget confirms\" {\n",
            "    element class:nz.example.shop.Widget;\n",
            "    element class:nz.example.shop.Gadget;\n",
            "    element op:ShopService.confirm;\n",
            "    check invariant:Widget;\n",
            "    check decision:ReleaseAssessment;\n",
            "}\n",
        ),
    );
    let diagnostics = check_coverage(
        &[(entry.0.clone(), entry.1.clone())],
        &spans_map(&[entry]),
        &domains(),
        &[],
        &[],
        &[],
        &designs(),
        &decisions(),
        "Shop",
    );
    assert!(
        diagnostics.is_empty(),
        "every link resolves and both slots are satisfied: {diagnostics:?}"
    );
}

#[test]
fn an_intent_without_checks_warns_x0010() {
    let entry = compile_intents(
        "orphan.intent",
        "intent SHOP-002 \"an orphaned claim\" {\n    element class:Widget;\n}\n",
    );
    let diagnostics = check_coverage(
        &[(entry.0.clone(), entry.1.clone())],
        &spans_map(&[entry]),
        &domains(),
        &[],
        &[],
        &[],
        &designs(),
        &decisions(),
        "Shop",
    );
    let (path, diagnostic) = diagnostics
        .iter()
        .find(|(_, diagnostic)| diagnostic.message.starts_with("X0010"))
        .expect("the unmapped-check warning");
    assert_eq!(path, "orphan.intent");
    assert_eq!(
        diagnostic.message,
        "X0010 intent 'SHOP-002' maps to no check"
    );
    assert!(
        !diagnostic.is_error(),
        "the warning never fails the pass on its own"
    );
}

#[test]
fn an_intent_without_elements_warns_x0009() {
    let entry = compile_intents(
        "orphan.intent",
        "intent SHOP-003 \"a claim nothing expresses\" {\n    check decision:ReleaseAssessment;\n}\n",
    );
    let diagnostics = check_coverage(
        &[(entry.0.clone(), entry.1.clone())],
        &spans_map(&[entry]),
        &domains(),
        &[],
        &[],
        &[],
        &designs(),
        &decisions(),
        "Shop",
    );
    assert!(
        messages(&diagnostics)
            .iter()
            .any(|message| message == "X0009 intent 'SHOP-003' maps to no model element"),
        "{diagnostics:?}"
    );
}

#[test]
fn an_unknown_element_link_errors_with_the_did_you_mean_candidate() {
    let entry = compile_intents(
        "bad.intent",
        "intent SHOP-004 \"x\" {\n    element class:Widgetz;\n    check decision:ReleaseAssessment;\n}\n",
    );
    let diagnostics = check_coverage(
        &[(entry.0.clone(), entry.1.clone())],
        &spans_map(&[entry]),
        &domains(),
        &[],
        &[],
        &[],
        &designs(),
        &decisions(),
        "Shop",
    );
    let diagnostic = diagnostics
        .iter()
        .find(|(_, diagnostic)| diagnostic.message.starts_with("X0011"))
        .map(|(_, diagnostic)| diagnostic)
        .expect("the unknown element error");
    assert_eq!(
        diagnostic.message,
        "X0011 unknown element class 'Widgetz' in intent 'SHOP-004'"
    );
    assert_eq!(
        diagnostic.help.as_deref(),
        Some("did you mean 'Widget'?"),
        "the close misspelling is named"
    );
}

#[test]
fn an_unknown_check_link_errors_x0012() {
    let entry = compile_intents(
        "bad.intent",
        "intent SHOP-005 \"x\" {\n    element class:Widget;\n    check decision:ReleaseAssesment;\n}\n",
    );
    let diagnostics = check_coverage(
        &[(entry.0.clone(), entry.1.clone())],
        &spans_map(&[entry]),
        &domains(),
        &[],
        &[],
        &[],
        &designs(),
        &decisions(),
        "Shop",
    );
    let diagnostic = diagnostics
        .iter()
        .find(|(_, diagnostic)| diagnostic.message.starts_with("X0012"))
        .map(|(_, diagnostic)| diagnostic)
        .expect("the unknown check error");
    assert_eq!(
        diagnostic.message,
        "X0012 unknown check decision 'ReleaseAssesment' in intent 'SHOP-005'"
    );
    assert!(
        diagnostic.help.as_deref() == Some("did you mean 'ReleaseAssessment'?"),
        "{diagnostic:?}"
    );
}

#[test]
fn an_ambiguous_bare_name_errors_x0013() {
    // Two packages carry a `Widget` each, so the bare name cannot bind.
    let entry = compile_intents(
        "bad.intent",
        "intent SHOP-006 \"x\" {\n    element class:Widget;\n    check decision:ReleaseAssessment;\n}\n",
    );
    let diagnostics = check_coverage(
        &[(entry.0.clone(), entry.1.clone())],
        &spans_map(&[entry]),
        &domains(),
        &[],
        &[],
        &[],
        &designs(),
        &decisions(),
        "Shop",
    );
    let diagnostic = diagnostics
        .iter()
        .find(|(_, diagnostic)| diagnostic.message.starts_with("X0013"))
        .map(|(_, diagnostic)| diagnostic)
        .expect("the ambiguity error");
    assert_eq!(
        diagnostic.message,
        "X0013 'Widget' matches several 'class' targets in application 'Shop': nz.example.shop.Widget, nz.example.other.Widget"
    );
}

#[test]
fn a_cross_file_duplicate_id_errors_x0014() {
    let first = compile_intents(
        "a.intent",
        "intent SHOP-007 \"x\" {\n    element class:Widget;\n    check invariant:Widget;\n}\n",
    );
    let second = compile_intents(
        "b.intent",
        "intent SHOP-007 \"y\" {\n    element class:Gadget;\n    check decision:ReleaseApproval;\n}\n",
    );
    let entries = vec![first, second];
    let intents: Vec<(String, IntentModel)> = entries
        .iter()
        .map(|(path, model, _)| (path.clone(), model.clone()))
        .collect();
    let diagnostics = check_coverage(
        &intents,
        &spans_map(&entries),
        &domains(),
        &[],
        &[],
        &[],
        &designs(),
        &decisions(),
        "Shop",
    );
    let diagnostic = diagnostics
        .iter()
        .find(|(_, diagnostic)| diagnostic.message.starts_with("X0014"))
        .map(|(_, diagnostic)| diagnostic)
        .expect("the collision error");
    assert_eq!(
        diagnostic.message,
        "X0014 duplicate intent id 'SHOP-007' across 'a.intent' and 'b.intent'"
    );
}

#[test]
fn the_unmapped_warning_is_spanned_at_the_intent_id() {
    let entry = compile_intents(
        "orphan.intent",
        concat!(
            "\n\n",
            "intent SHOP-008 \"an orphaned claim, indented deep\" {\n",
            "    element class:Widget;\n",
            "}\n",
        ),
    );
    let site = entry
        .2
        .iter()
        .find(|site| site.intent == "SHOP-008")
        .expect("the id span site")
        .clone();
    let diagnostics = check_coverage(
        &[(entry.0.clone(), entry.1.clone())],
        &spans_map(&[entry]),
        &domains(),
        &[],
        &[],
        &[],
        &[],
        &[],
        "Shop",
    );
    let diagnostic = diagnostics
        .iter()
        .find(|(_, diagnostic)| diagnostic.message.starts_with("X0010"))
        .map(|(_, diagnostic)| diagnostic)
        .expect("the unmapped-check warning");
    assert_eq!(
        diagnostic.span.as_ref(),
        Some(&site.intent_span),
        "the warning anchors at the intent's id span"
    );
}
