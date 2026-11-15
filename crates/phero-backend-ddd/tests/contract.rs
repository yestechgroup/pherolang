//! The DDD contract checker against compiler output and hand-edited
//! artifacts: a compiled design is contract-complete; every removed or
//! lying resolved companion is reported at its JSON path.

use std::path::{Path, PathBuf};

use phero_backend_ddd::{canonical_json, canonical_json_string, check_contract, Violation};
use phero_driver::{
    compile_actors_str, compile_ddd_str, compile_ddd_str_with_actors, DomainImports, Severity,
};
use phero_ir::ddd::{DddModel, Stereotype, TargetKind, TargetRef, DDD_MODEL_FORMAT_VERSION};
use phero_ir::Multiplicity;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn fixture(name: &str) -> (String, String) {
    let path = workspace_root().join("tests/conformance/ddd").join(name);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    (path.display().to_string(), source)
}

fn compiled() -> DddModel {
    let (design_path, design) = fixture("library.ddd");
    let (domain_path, domain) = fixture("library.mox");
    let compilation = compile_ddd_str(
        &design_path,
        &design,
        &[(domain_path, domain)],
        &DomainImports::default(),
    );
    assert_clean(&compilation.diagnostics);
    compilation
        .model
        .expect("library.ddd lowers to a design artifact")
}

/// A focused design over `library.mox` whose capability names all exist in
/// `library.actor`. `library.ddd` itself names capabilities the actor
/// fixture does not declare, so that pair is an E-level compile.
fn compiled_with_actors() -> DddModel {
    let (design_path, _) = fixture("library.ddd");
    let design_path = Path::new(&design_path)
        .with_file_name("test.ddd")
        .display()
        .to_string();
    let (domain_path, domain) = fixture("library.mox");
    let (actor_path, actor_source) = fixture("library.actor");
    let actor = compile_actors_str(
        &actor_path,
        &actor_source,
        &[(domain_path.clone(), domain.clone())],
        &DomainImports::default(),
    );
    assert_clean(&actor.diagnostics);
    let actor = actor.model.expect("library.actor lowers");
    let design = r#"
import "library.mox";

application Library {
    base nz.example.library

    module media {
        service MediaService {
            boolean borrow() capability BorrowMedia;
        }
        entity Movie repository MovieRepository {
            findById;
            save;
        }
        search MediaSearch {
            entity Movie
            text {
                title
            }
            filters {
                genre
            }
            sort {
                title
            }
            capability SearchMedia, SearchMediaAdmin
        }
    }
}
"#;
    let compilation = compile_ddd_str_with_actors(
        &design_path,
        design,
        &[(domain_path, domain)],
        &DomainImports::default(),
        &[actor],
    );
    assert_clean(&compilation.diagnostics);
    compilation
        .model
        .expect("the focused design lowers under actor validation")
}

fn assert_clean(diagnostics: &[(String, phero_driver::Diagnostic)]) {
    assert!(
        diagnostics
            .iter()
            .all(|(_, diagnostic)| diagnostic.severity != Severity::Error),
        "expected a clean compile, got: {diagnostics:?}"
    );
}

fn paths(violations: &[Violation]) -> Vec<&str> {
    violations
        .iter()
        .map(|violation| violation.path.as_str())
        .collect()
}

fn has(violations: &[Violation], path: &str) -> bool {
    violations.iter().any(|violation| violation.path == path)
}

#[test]
fn a_compiled_design_is_contract_complete() {
    assert_eq!(check_contract(&compiled()), Vec::new());
}

#[test]
fn actor_backed_capabilities_are_declared_in_their_block() {
    let model = compiled_with_actors();
    let operation = model.modules[0].services[0]
        .operations
        .iter()
        .find(|operation| operation.name == "borrow")
        .expect("borrow lowers");
    let reference = operation
        .capability_refs
        .first()
        .expect("borrow carries a capability ref");
    assert_eq!(reference.name, "BorrowMedia");
    assert_eq!(reference.declared_in.as_deref(), Some("Library"));
    assert_eq!(check_contract(&model), Vec::new());
}

#[test]
fn missing_design_type_ref_is_reported() {
    let mut model = compiled();
    model.modules[0].designs[0].type_ref = None;
    let violations = check_contract(&model);
    assert!(
        has(&violations, "modules[0].designs[0].typeRef"),
        "{violations:?}"
    );
}

#[test]
fn missing_delegation_target_ref_is_reported() {
    let mut model = compiled();
    let operation = model.modules[0].services[0]
        .operations
        .iter_mut()
        .find(|operation| operation.name == "save")
        .expect("save lowers");
    operation
        .delegation
        .as_mut()
        .expect("save is a delegation")
        .target_ref = None;
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.path.ends_with(".delegation.targetRef")),
        "paths: {:?}",
        paths(&violations)
    );
}

#[test]
fn dependency_refs_must_align_with_dependencies() {
    let mut model = compiled();
    model.modules[0].services[0].dependency_refs.clear();
    let violations = check_contract(&model);
    assert!(
        has(&violations, "modules[0].services[0].dependencyRefs"),
        "{violations:?}"
    );
}

#[test]
fn search_capability_refs_must_align() {
    let mut model = compiled();
    model.modules[0].searches[0].capability_refs.clear();
    let violations = check_contract(&model);
    assert!(
        has(&violations, "modules[0].searches[0].capabilityRefs"),
        "{violations:?}"
    );
}

#[test]
fn search_member_types_are_required() {
    let mut model = compiled();
    model.modules[0].searches[0].text[0].type_ref = None;
    model.modules[0].searches[0].filters[0].type_ref = None;
    model.modules[0].searches[0].sort[0].type_ref = None;
    let violations = check_contract(&model);
    assert!(
        has(&violations, "modules[0].searches[0].text[0].typeRef"),
        "{violations:?}"
    );
    assert!(
        has(&violations, "modules[0].searches[0].filters[0].typeRef"),
        "{violations:?}"
    );
    assert!(
        has(&violations, "modules[0].searches[0].sort[0].typeRef"),
        "{violations:?}"
    );
}

#[test]
fn a_dto_cannot_carry_a_repository() {
    let mut model = compiled();
    model.modules[0].designs[0].stereotype = Stereotype::Dto;
    let violations = check_contract(&model);
    assert!(
        has(&violations, "modules[0].designs[0].repository"),
        "{violations:?}"
    );
}

#[test]
fn an_entity_cannot_be_non_persistent() {
    let mut model = compiled();
    model.modules[0].designs[0].flags.non_persistent = true;
    let violations = check_contract(&model);
    assert!(
        has(&violations, "modules[0].designs[0].flags.nonPersistent"),
        "{violations:?}"
    );
}

#[test]
fn a_repository_operation_cannot_be_both_builtin_and_declared() {
    let mut model = compiled();
    model.modules[0].designs[0]
        .repository
        .as_mut()
        .expect("Library has a repository")
        .operations[0]
        .return_type = Some(phero_ir::TypeRef::Primitive(
        phero_ir::PrimitiveType::String,
    ));
    let violations = check_contract(&model);
    assert!(
        has(
            &violations,
            "modules[0].designs[0].repository.operations[0]"
        ),
        "{violations:?}"
    );
}

#[test]
fn a_return_multiplicity_requires_a_return_type() {
    let mut model = compiled();
    let operation = model.modules[0].services[0]
        .operations
        .iter_mut()
        .find(|operation| operation.name == "save")
        .expect("save lowers");
    operation.return_multiplicity = Some(Multiplicity::REQUIRED);
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.path.ends_with(".returnMultiplicity")),
        "paths: {:?}",
        paths(&violations)
    );
}

#[test]
fn emitted_and_invoked_references_must_align() {
    let mut model = compiled();
    let operation = &mut model.modules[0].services[0].operations[0];
    operation.emits.push("GhostEvent".to_string());
    operation.invokes.push("GhostDecision".to_string());
    let violations = check_contract(&model);
    assert!(
        has(
            &violations,
            "modules[0].services[0].operations[0].emittedRefs"
        ),
        "{violations:?}"
    );
    assert!(
        has(
            &violations,
            "modules[0].services[0].operations[0].decisionRefs"
        ),
        "{violations:?}"
    );
}

#[test]
fn a_lying_target_ref_is_reported() {
    let mut model = compiled();
    let operation = model.modules[0].services[0]
        .operations
        .iter_mut()
        .find(|operation| operation.name == "save")
        .expect("save lowers");
    operation
        .delegation
        .as_mut()
        .expect("save is a delegation")
        .target_ref = Some(TargetRef {
        kind: TargetKind::Repository,
        qualified_name: "Library.media.NotTheRepository".to_string(),
        module: "media".to_string(),
    });
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.path.ends_with(".delegation.targetRef")),
        "paths: {:?}",
        paths(&violations)
    );
}

#[test]
fn provenance_and_application_are_required() {
    let mut model = compiled();
    model.phero_version = None;
    model.application = None;
    let violations = check_contract(&model);
    assert!(has(&violations, "pheroVersion"), "{violations:?}");
    assert!(has(&violations, "application"), "{violations:?}");
}

#[test]
fn a_wrong_format_version_is_reported() {
    let mut model = compiled();
    model.format_version = DDD_MODEL_FORMAT_VERSION + 1;
    let violations = check_contract(&model);
    assert!(has(&violations, "formatVersion"), "{violations:?}");
}

#[test]
fn canonical_json_is_sorted_and_round_trips() {
    let model = compiled();
    let value = canonical_json(&model).expect("canonical JSON");
    let keys: Vec<String> = value
        .as_object()
        .expect("artifact root is an object")
        .keys()
        .cloned()
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "root keys must be alphabetical");
    let reloaded: DddModel = serde_json::from_value(value).expect("canonical JSON reloads");
    assert_eq!(reloaded, model);

    let text = canonical_json_string(&model).expect("canonical JSON string");
    serde_json::from_str::<DddModel>(&text).expect("canonical string reloads");
}
