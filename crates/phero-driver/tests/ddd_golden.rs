//! Golden-artifact conformance test: compiling the canonical `.ddd` design
//! against its imported domain model must produce a [`DddModel`] whose
//! pretty JSON byte-matches the committed golden artifact, and the artifact
//! must round-trip through `from_json`.
//!
//! Regenerate with `PHERO_UPDATE_FIXTURES=1 cargo test -p phero-driver
//! --test ddd_golden`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use phero_driver::compile_ddd_str;

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

const DESIGN_PATH: &str = "tests/conformance/ddd/library.ddd";
const DOMAIN_PATH: &str = "tests/conformance/ddd/library.mox";
const ARTIFACT_PATH: &str = "tests/conformance/ddd/library.ddd.json";

/// The pherolang#176/#180 fixture: a multi-machine module exercising the
/// transition bindings and the scoped guards; its golden carries the
/// `transition` and `conditionPaths` wire fields.
const TIMESHEET_DESIGN_PATH: &str = "tests/conformance/ddd/timesheet.ddd";
const TIMESHEET_DOMAIN_PATH: &str = "tests/conformance/ddd/timesheet.mox";
const TIMESHEET_ARTIFACT_PATH: &str = "tests/conformance/ddd/timesheet.ddd.json";

/// The pherolang#179 fixture: module-level `extend PayRun` child operations
/// over the aggregate's contained collection/scalar children; its golden
/// carries `childOperations` and each operation's `childScope`.
const CHILDOPS_DESIGN_PATH: &str = "tests/conformance/ddd/childops.ddd";
const CHILDOPS_DOMAIN_PATH: &str = "tests/conformance/ddd/childops.mox";
const CHILDOPS_ARTIFACT_PATH: &str = "tests/conformance/ddd/childops.ddd.json";

/// The pherolang#178 fixture: executable service/repository statement bodies
/// (`let`/let-else, guard, assignment, reductions, `if`/`else`, `return`)
/// alongside `capability`/`when` clauses; its golden carries the `bodies`
/// `"stmt"` targets and each service body's `bodyReceiver`.
const BODIES_DESIGN_PATH: &str = "tests/conformance/ddd/bodies.ddd";
const BODIES_DOMAIN_PATH: &str = "tests/conformance/ddd/bodies.mox";
const BODIES_ARTIFACT_PATH: &str = "tests/conformance/ddd/bodies.ddd.json";

/// The DD2 value-semantics fixture (issue pherolang#299): three value
/// designs — enum/datatype fields, a nested value (a value containing
/// values), and a `nonPersistent` one; the golden carries the fixed
/// `semantics` block on each.
const VALUES_DESIGN_PATH: &str = "tests/conformance/ddd/values.ddd";
const VALUES_DOMAIN_PATH: &str = "tests/conformance/ddd/values.mox";
const VALUES_ARTIFACT_PATH: &str = "tests/conformance/ddd/values.ddd.json";

/// The DD3 DTO-boundary fixture (issue pherolang#300): the mirror,
/// projection (select, rename, derive), and standalone forms, a nested DTO
/// member, and DTOs filling a service operation's parameter and return
/// slots; the golden carries each `dtoShape` and the `DddTypeRef` DTO
/// references.
const DTOS_DESIGN_PATH: &str = "tests/conformance/ddd/dtos.ddd";
const DTOS_DOMAIN_PATH: &str = "tests/conformance/ddd/dtos.mox";
const DTOS_ARTIFACT_PATH: &str = "tests/conformance/ddd/dtos.ddd.json";

/// The DD7 declared-error fixture (issue pherolang#304): an application
/// error with a typed payload, a module-private one, a DTO payload, the
/// prelude, a body raising a declared error, a stated domain `op` call, and
/// a delegation whose `throws` is a superset of its target's; the golden
/// carries `errors` at both scopes and the resolved `throws` references.
const ERRORS_DESIGN_PATH: &str = "tests/conformance/ddd/errors.ddd";
const ERRORS_DOMAIN_PATH: &str = "tests/conformance/ddd/errors.mox";
const ERRORS_ARTIFACT_PATH: &str = "tests/conformance/ddd/errors.ddd.json";

/// Compiles one `(design, domain)` fixture pair the way the CLI resolves
/// the import (paths relative to the design file) and returns the lowered
/// artifact, failing on any diagnostic.
fn compile_fixture(design_path: &str, domain_path: &str) -> phero_ir::ddd::DddModel {
    let design_absolute = fixture_path(design_path);
    let source = std::fs::read_to_string(&design_absolute)
        .unwrap_or_else(|error| panic!("read conformance design {design_path}: {error}"));
    let domain_absolute = fixture_path(domain_path);
    let domain = std::fs::read_to_string(&domain_absolute)
        .unwrap_or_else(|error| panic!("read conformance domain {domain_path}: {error}"));

    // The driver is filesystem-free: the harness resolves the import the
    // way the CLI does (relative to the design file) and provides the
    // domain text under its resolved absolute path.
    let compilation = compile_ddd_str(
        design_absolute.to_str().expect("utf-8 path"),
        &source,
        &[(
            domain_absolute.to_str().expect("utf-8 path").to_string(),
            domain,
        )],
        &phero_driver::DomainImports::default(),
    );
    assert!(
        compilation.diagnostics.is_empty(),
        "conformance design must compile clean: {:?}",
        compilation.diagnostics
    );
    let model = compilation.model.expect("design artifact lowered");
    assert_eq!(
        model.format_version,
        phero_ir::ddd::DDD_MODEL_FORMAT_VERSION
    );
    model
}

/// Byte-compiles the lowered artifact against the committed golden (or
/// rewrites it under `PHERO_UPDATE_FIXTURES=1`).
fn assert_golden_artifact(model: phero_ir::ddd::DddModel, artifact_path: &str, design_path: &str) {
    let json = model.to_json_pretty().expect("serialize IR");

    let artifact = fixture_path(artifact_path);
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
        "DDD artifact for {design_path} drifted from the golden file"
    );
}

#[test]
fn conformance_design_matches_golden_artifact() {
    let model = compile_fixture(DESIGN_PATH, DOMAIN_PATH);
    assert_eq!(
        model
            .application
            .as_ref()
            .map(|application| application.name.as_str()),
        Some("Library")
    );
    assert_eq!(
        model
            .modules
            .iter()
            .map(|module| module.name.as_str())
            .collect::<Vec<_>>(),
        ["media", "person"]
    );

    assert_golden_artifact(model, ARTIFACT_PATH, DESIGN_PATH);
}

#[test]
fn conformance_timesheet_matches_golden_artifact() {
    let model = compile_fixture(TIMESHEET_DESIGN_PATH, TIMESHEET_DOMAIN_PATH);
    // The pherolang#176/#180 wire fields carry the fixture's bindings.
    let submit = &model.modules[0].services[0].operations[0];
    let transition = submit.transition.as_ref().expect("the transition binding");
    assert_eq!(transition.trigger, "Submit");
    assert_eq!(transition.machine.as_deref(), Some("PayRunStatus"));
    assert_eq!(submit.condition_paths.len(), 2);
    assert_golden_artifact(model, TIMESHEET_ARTIFACT_PATH, TIMESHEET_DESIGN_PATH);
}

/// The pherolang#179 child-operation fixture: the `extend PayRun` blocks lower
/// onto the root's design as `childOperations`, each naming the child
/// feature and its resolved class; ordinary service operations stay
/// untouched.
#[test]
fn conformance_childops_matches_golden_artifact() {
    let model = compile_fixture(CHILDOPS_DESIGN_PATH, CHILDOPS_DOMAIN_PATH);
    let design = model.modules[0]
        .designs
        .iter()
        .find(|design| design.class.as_deref() == Some("PayRun"))
        .expect("the PayRun design");
    assert_eq!(
        design
            .child_operations
            .iter()
            .map(|operation| operation.name.as_str())
            .collect::<Vec<_>>(),
        [
            "addLine",
            "removeLine",
            "recalculateLine",
            "describeLine",
            "annotate"
        ],
    );
    assert_eq!(
        design.child_operations[0]
            .child_scope
            .as_ref()
            .map(|scope| (scope.feature.as_str(), scope.class.as_str())),
        Some(("lines", "nz.example.payroll.PayRunLine")),
    );
    assert_golden_artifact(model, CHILDOPS_ARTIFACT_PATH, CHILDOPS_DESIGN_PATH);
}

/// The pherolang#178 body fixture: every service body records its primary
/// repository (`bodyReceiver`) and lowers a `"stmt"` body target; the
/// repository operation's body carries no receiver.
#[test]
fn conformance_bodies_matches_golden_artifact() {
    let model = compile_fixture(BODIES_DESIGN_PATH, BODIES_DOMAIN_PATH);
    let service = &model.modules[0].services[0];
    let submit = service
        .operations
        .iter()
        .find(|operation| operation.name == "submit")
        .expect("the submit body operation");
    assert_eq!(submit.body_receiver.as_deref(), Some("PayRunRepository"));
    assert!(submit.bodies.contains_key("stmt"));
    let approve = service
        .operations
        .iter()
        .find(|operation| operation.name == "approve")
        .expect("the approve body operation");
    assert_eq!(approve.condition.as_deref(), Some("status == Draft"));
    assert_eq!(approve.capabilities, ["ApprovePayRun"]);
    assert_eq!(approve.body_receiver.as_deref(), Some("PayRunRepository"));

    let run = model.modules[0]
        .designs
        .iter()
        .find(|design| design.class.as_deref() == Some("PayRun"))
        .expect("the PayRun design");
    let total = run
        .repository
        .as_ref()
        .and_then(|repository| {
            repository
                .operations
                .iter()
                .find(|operation| operation.name == "totalQuantity")
        })
        .expect("the repository body operation");
    assert!(total.bodies.contains_key("stmt"));

    assert_golden_artifact(model, BODIES_ARTIFACT_PATH, BODIES_DESIGN_PATH);
}

/// The DD7 error-vocabulary fixture: both scopes lower their declarations,
/// the resolution carries application/module/prelude scopes, and the
/// delegation copies the target's set as a superset.
#[test]
fn conformance_errors_matches_golden_artifact() {
    let model = compile_fixture(ERRORS_DESIGN_PATH, ERRORS_DOMAIN_PATH);
    assert_eq!(
        model
            .errors
            .iter()
            .map(|error| error.name.as_str())
            .collect::<Vec<_>>(),
        ["PaymentDeclined", "OutOfStock"]
    );
    assert_eq!(
        model.errors[0].description.as_deref(),
        Some("The payment was refused by the provider.")
    );
    assert_eq!(
        model.modules[0]
            .errors
            .iter()
            .map(|error| error.name.as_str())
            .collect::<Vec<_>>(),
        ["InvoiceAlreadyIssued", "DeclinedDetail"]
    );
    assert_eq!(
        model.modules[0].errors[0].description.as_deref(),
        Some("The invoice for this order was already issued.")
    );
    assert!(matches!(
        model.modules[0].errors[1].fields[0].type_,
        phero_ir::ddd::DddTypeRef::Dto { .. }
    ));
    let pay = &model.modules[0].services[0].operations[0];
    assert_eq!(
        pay.throws
            .iter()
            .map(|reference| (reference.name.as_str(), reference.scope))
            .collect::<Vec<_>>(),
        [
            ("PaymentDeclined", phero_ir::ddd::ErrorScope::Application),
            ("OutOfStock", phero_ir::ddd::ErrorScope::Application),
            ("NotFound", phero_ir::ddd::ErrorScope::Prelude),
            ("InvoiceAlreadyIssued", phero_ir::ddd::ErrorScope::Module),
        ]
    );
    assert_golden_artifact(model, ERRORS_ARTIFACT_PATH, ERRORS_DESIGN_PATH);
}

/// The DD2 value fixture (issue pherolang#299): every value design carries
/// the fixed strict semantics block, and none of the three names an
/// identity, a contained entity, or an entity reference.
#[test]
fn conformance_values_matches_golden_artifact() {
    let model = compile_fixture(VALUES_DESIGN_PATH, VALUES_DOMAIN_PATH);
    assert_eq!(
        model.modules[0]
            .designs
            .iter()
            .map(|design| design.class.as_deref().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["Money", "PriceRange", "Snapshot"],
    );
    for design in &model.modules[0].designs {
        assert_eq!(
            design.semantics.as_ref(),
            Some(&phero_ir::ddd::ValueSemantics::strict()),
            "value design '{}' carries the fixed block",
            design.class.as_deref().unwrap_or_default()
        );
    }
    assert!(model.modules[0].designs[2].flags.non_persistent);
    assert_golden_artifact(model, VALUES_ARTIFACT_PATH, VALUES_DESIGN_PATH);
}

/// The DD3 DTO fixture (issue pherolang#300): the mirror copies the source
/// class's stored features, the projection selects/renames/derives in
/// declaration order (and is therefore output-only), the standalone
/// boundary declares its members — one of them a nested DTO — and the
/// service operation's parameter and return slots carry the DTO references.
#[test]
fn conformance_dtos_matches_golden_artifact() {
    use phero_ir::ddd::{DddTypeRef, DtoDirection, DtoMemberKind};

    let model = compile_fixture(DTOS_DESIGN_PATH, DTOS_DOMAIN_PATH);
    let designs = &model.modules[0].designs;
    let shape = |name: &str| {
        designs
            .iter()
            .filter_map(|design| design.dto_shape.as_ref())
            .find(|shape| shape.name == name)
            .unwrap_or_else(|| panic!("expected the dto shape '{name}'"))
    };
    let member_names = |name: &str| -> Vec<&str> {
        shape(name)
            .members
            .iter()
            .map(|member| member.name.as_str())
            .collect()
    };

    // Mirror: every stored feature of the source class, in declaration
    // order; `Author` has no entity design, so the mirror is its only design.
    assert_eq!(member_names("Author"), ["name", "handle"]);
    assert_eq!(shape("Author").direction, DtoDirection::Input);

    // Projection: selections, a renamed plain path, and a derivation.
    assert_eq!(
        member_names("BookSummary"),
        ["isbn", "title", "currency", "code", "pricey", "authorName"],
    );
    assert_eq!(shape("BookSummary").direction, DtoDirection::Output);
    let summary = shape("BookSummary");
    assert_eq!(summary.members[4].kind, DtoMemberKind::Derive);
    assert_eq!(
        summary.members[4].expression.as_deref(),
        Some("priceCents > 5000")
    );
    assert_eq!(summary.members[5].kind, DtoMemberKind::Select);
    assert_eq!(summary.members[5].path, ["author", "name"]);

    // Standalone: declared members, one of them another DTO boundary.
    let request = shape("OrderRequest");
    assert_eq!(request.direction, DtoDirection::Input);
    assert_eq!(member_names("OrderRequest"), ["book", "currency", "codes"]);
    assert_eq!(request.members[0].kind, DtoMemberKind::Declared);
    assert_eq!(
        request.members[0].type_,
        DddTypeRef::dto("BookDraft"),
        "a nested DTO member keeps its boundary reference"
    );

    // Service slots: a DTO return and a DTO parameter.
    let operations = &model.modules[0].services[0].operations;
    assert_eq!(
        operations[0].return_type,
        Some(DddTypeRef::dto("BookDraft"))
    );
    assert_eq!(
        operations[1].return_type,
        Some(DddTypeRef::dto("BookSummary"))
    );
    assert_eq!(
        operations[2].params[0].type_,
        DddTypeRef::dto("OrderRequest")
    );

    assert_golden_artifact(model, DTOS_ARTIFACT_PATH, DTOS_DESIGN_PATH);
}

#[test]
fn golden_artifact_round_trips_through_from_json() {
    let expected = std::fs::read_to_string(fixture_path(ARTIFACT_PATH))
        .unwrap_or_else(|error| panic!("read golden artifact {ARTIFACT_PATH}: {error}"));
    let model =
        phero_ir::ddd::DddModel::from_json(&expected).expect("golden artifact must deserialize");
    assert_eq!(
        model.format_version,
        phero_ir::ddd::DDD_MODEL_FORMAT_VERSION
    );
    let reserialized = model.to_json_pretty().expect("re-serialize IR");
    assert_eq!(reserialized, expected, "artifact round trip must be stable");
}
