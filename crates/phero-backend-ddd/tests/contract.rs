//! The DDD contract checker against compiler output and hand-edited
//! artifacts: a compiled design is contract-complete; every removed or
//! lying resolved companion is reported at its JSON path.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use phero_backend_ddd::{
    canonical_json, canonical_json_string, check_contract, check_pairing, Violation,
};
use phero_driver::{compile_actors_str, compile_ddd_str, Severity};
use phero_ir::ddd::{
    BuiltinRepositoryOp, CapabilityRef, DddModel, DddTypeRef, Design, DocumentField,
    DomainFingerprint, DtoDirection, DtoMember, DtoMemberKind, DtoShape, EqualityPolicy, ErrorRef,
    EventRef, IdentityPolicy, QueryPagination, QuerySortKey, QuerySpec, RepositoryOperation,
    SortDirection, StateMachine, Stereotype, TargetKind, TargetRef, ValueSemantics,
    DDD_MODEL_FORMAT_VERSION,
};
use phero_ir::{Multiplicity, PrimitiveType, TypeRef};

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
        &phero_driver::CompileInputs::from_domains(&[(domain_path, domain)]),
    );
    assert_clean(&compilation.diagnostics);
    compilation
        .model
        .expect("library.ddd lowers to a design artifact")
}

fn compiled_queries() -> DddModel {
    let (design_path, design) = fixture("queries.ddd");
    let (domain_path, domain) = fixture("queries.mox");
    let compilation = compile_ddd_str(
        &design_path,
        &design,
        &phero_driver::CompileInputs::from_domains(&[(domain_path, domain)]),
    );
    assert_clean(&compilation.diagnostics);
    compilation
        .model
        .expect("queries.ddd lowers to a design artifact")
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
        &phero_driver::CompileInputs::from_domains(&[(domain_path.clone(), domain.clone())]),
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
    let compilation = compile_ddd_str(
        &design_path,
        design,
        &phero_driver::CompileInputs::from_domains(&[(domain_path, domain)])
            .with_actors([(actor_path, actor)]),
    );
    assert_clean(&compilation.diagnostics);
    compilation
        .model
        .expect("the focused design lowers under actor validation")
}

/// The strict value-semantics block every `value` design carries (DD2,
/// pherolang#299).
fn fixed_semantics() -> ValueSemantics {
    ValueSemantics {
        identity: IdentityPolicy::None,
        immutable: true,
        deep_immutable: true,
        equality: EqualityPolicy::Structural,
    }
}

/// The canonical value fixture: three value designs — enum/datatype
/// fields, a nested value, and a non-persistent one.
fn compiled_values() -> DddModel {
    let (design_path, design) = fixture("values.ddd");
    let (domain_path, domain) = fixture("values.mox");
    let compilation = compile_ddd_str(
        &design_path,
        &design,
        &phero_driver::CompileInputs::from_domains(&[(domain_path, domain)]),
    );
    assert_clean(&compilation.diagnostics);
    compilation
        .model
        .expect("values.ddd lowers to a design artifact")
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

// --- domain pairing (DF1, pherolang#324) -------------------------------------

fn domain_model(name: &str) -> phero_ir::Model {
    let (path, source) = fixture(name);
    let compilation =
        phero_driver::compile_str(&path, &source, &phero_driver::DomainImports::default());
    assert!(
        compilation
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "expected a clean domain compile, got: {:?}",
        compilation.diagnostics
    );
    compilation.model.expect("the domain lowers")
}

#[test]
fn a_design_without_a_domain_fingerprint_is_reported() {
    let mut model = compiled();
    model.domain_fingerprint = None;
    assert!(has(&check_contract(&model), "domainFingerprint"));
}

#[test]
fn a_malformed_domain_fingerprint_is_reported() {
    let mut model = compiled();
    model.domain_fingerprint = Some(DomainFingerprint {
        algorithm: "sha512".to_string(),
        value: "NOT-HEX".to_string(),
        packages: vec!["z".to_string(), "a".to_string()],
    });
    let violations = check_contract(&model);
    assert!(has(&violations, "domainFingerprint.algorithm"));
    assert!(has(&violations, "domainFingerprint.value"));
    assert!(has(&violations, "domainFingerprint.packages"));
}

#[test]
fn pairing_accepts_the_compiled_domain_and_names_a_mismatch() {
    let model = compiled();
    let matching = phero_driver::domain_fingerprint(&[domain_model("library.mox")]);
    assert_eq!(check_pairing(&model, &matching), Ok(()));

    let other = phero_driver::domain_fingerprint(&[domain_model("queries.mox")]);
    let violation = check_pairing(&model, &other).expect_err("the pair is stale");
    assert_eq!(violation.path, "domainFingerprint");
    assert!(
        violation.message.contains("nz.example.library"),
        "the diagnostic names the compiled package: {}",
        violation.message
    );
    assert!(
        violation.message.contains("demo.queries"),
        "the diagnostic names the supplied package: {}",
        violation.message
    );
}

// --- value semantics (DD2, pherolang#299) ------------------------------------

#[test]
fn a_compiled_value_design_is_contract_complete() {
    let model = compiled_values();
    assert_eq!(check_contract(&model), Vec::new());
    for design in &model.modules[0].designs {
        assert_eq!(
            design.semantics,
            Some(fixed_semantics()),
            "value design '{}' carries the fixed block",
            design.class.as_deref().unwrap_or_default()
        );
    }
}

#[test]
fn a_value_design_without_the_semantics_block_is_reported() {
    let mut model = compiled_values();
    model.modules[0].designs[0].semantics = None;
    let violations = check_contract(&model);
    assert!(
        has(&violations, "modules[0].designs[0].semantics"),
        "{violations:?}"
    );
    assert_eq!(
        violations.len(),
        1,
        "one violation per design: {violations:?}"
    );
    let violation = &violations[0];
    assert!(
        violation.message.contains("value design") && violation.message.contains("semantics"),
        "the message names the missing block: {}",
        violation.message
    );
}

#[test]
fn a_value_design_with_a_wrong_semantics_field_is_reported() {
    for (field, mutate) in [
        (
            "immutable",
            (|semantics: &mut ValueSemantics| semantics.immutable = false)
                as fn(&mut ValueSemantics),
        ),
        ("deepImmutable", |semantics: &mut ValueSemantics| {
            semantics.deep_immutable = false;
        }),
    ] {
        let mut model = compiled_values();
        let semantics = model.modules[0].designs[0]
            .semantics
            .as_mut()
            .expect("the compiled value design carries the block");
        mutate(semantics);
        let violations = check_contract(&model);
        assert!(
            has(&violations, "modules[0].designs[0].semantics"),
            "{field}: {violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.message.contains(field)),
            "{field}: the message names the offending field: {violations:?}"
        );
    }
}

#[test]
fn entity_and_dto_designs_may_not_carry_semantics() {
    for stereotype in [Stereotype::Entity, Stereotype::Dto] {
        let mut model = compiled_values();
        let mut design = Design::new("Summary", stereotype);
        design.type_ref = Some(phero_ir::TypeRef::Class {
            package: "nz.example.values".to_string(),
            name: "Summary".to_string(),
        });
        design.semantics = Some(fixed_semantics());
        model.modules[0].designs.push(design);
        let index = model.modules[0].designs.len() - 1;
        let violations = check_contract(&model);
        assert!(
            has(
                &violations,
                &format!("modules[0].designs[{index}].semantics")
            ),
            "{stereotype:?}: {violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.message.contains("only a value design")),
            "{stereotype:?}: the message names the rule: {violations:?}"
        );
    }
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
        .return_type = Some(phero_ir::ddd::DddTypeRef::Domain(
        phero_ir::TypeRef::Primitive(phero_ir::PrimitiveType::String),
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

/// A resolved emitted ref may carry its contract identity (pherolang#382 —
/// the compile stamps it when an `.evt` bucket was supplied) and passes
/// the artifact-alone check; an empty contract string stays a violation.
#[test]
fn resolved_emitted_refs_carry_their_contract_through_the_alone_check() {
    let mut model = compiled();
    let operation = &mut model.modules[0].services[0].operations[0];
    operation.emits.push("GhostEvent".to_string());
    operation.emitted_refs =
        vec![EventRef::new("GhostEvent").with_contract(Some("events.evt".to_string()))];
    let violations = check_contract(&model);
    assert!(
        !violations
            .iter()
            .any(|violation| violation.path.contains("emittedRefs")),
        "{violations:?}"
    );

    let mut model = compiled();
    let operation = &mut model.modules[0].services[0].operations[0];
    operation.emits.push("GhostEvent".to_string());
    operation.emitted_refs =
        vec![EventRef::new("GhostEvent").with_contract(Some("   ".to_string()))];
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.path.contains("emittedRefs")),
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

// --- DTO boundaries (DD3, pherolang#300) ------------------------------------

fn library_class(name: &str) -> TypeRef {
    TypeRef::Class {
        package: "nz.example.library".to_string(),
        name: name.to_string(),
    }
}

fn payroll_class(name: &str) -> TypeRef {
    TypeRef::Class {
        package: "nz.example.payroll".to_string(),
        name: name.to_string(),
    }
}

fn string_type() -> DddTypeRef {
    DddTypeRef::domain(TypeRef::Primitive(PrimitiveType::String))
}

fn selected(name: &str) -> DtoMember {
    DtoMember::new(name, DtoMemberKind::Select, string_type()).with_path(vec![name.to_string()])
}

fn derived(name: &str, expression: &str) -> DtoMember {
    DtoMember::new(name, DtoMemberKind::Derive, string_type())
        .with_expression(Some(expression.to_string()))
}

fn declared(name: &str) -> DtoMember {
    DtoMember::new(name, DtoMemberKind::Declared, string_type())
}

/// A projection DTO over the (entity-designed) `Book`:
/// `dto BookSummary from Book { title; label = title; }` — output-only
/// because it carries a derived member.
fn book_summary() -> DtoShape {
    DtoShape::new("BookSummary", DtoDirection::Output)
        .with_source(Some(library_class("Book")))
        .with_members(vec![selected("title"), derived("label", "title")])
}

/// An input-capable standalone DTO: no source class, declared members only.
fn create_book_request() -> DtoShape {
    DtoShape::new("CreateBookRequest", DtoDirection::Input).with_members(vec![declared("title")])
}

fn source_class_name(type_ref: &TypeRef) -> &str {
    match type_ref {
        TypeRef::Class { name, .. } => name,
        _ => "",
    }
}

/// Installs `shape` as a dto design in the first module (a mirror-shaped
/// design targets its source class; a standalone one has no class), and
/// returns the artifact plus the new design's index.
fn with_dto(mut model: DddModel, shape: DtoShape) -> (DddModel, usize) {
    let mut design = match &shape.source {
        Some(source) => {
            let mut design = Design::new(source_class_name(source).to_string(), Stereotype::Dto);
            design.type_ref = Some(source.clone());
            design
        }
        None => Design::standalone_dto(shape.name.clone()),
    };
    design.dto_shape = Some(shape);
    model.modules[0].designs.push(design);
    let index = model.modules[0].designs.len() - 1;
    (model, index)
}

fn dto_path(index: usize, suffix: &str) -> String {
    format!("modules[0].designs[{index}].{suffix}")
}

fn childops() -> DddModel {
    let (design_path, design) = fixture("childops.ddd");
    let (domain_path, domain) = fixture("childops.mox");
    let compilation = compile_ddd_str(
        &design_path,
        &design,
        &phero_driver::CompileInputs::from_domains(&[(domain_path, domain)]),
    );
    assert_clean(&compilation.diagnostics);
    compilation
        .model
        .expect("childops.ddd lowers to a design artifact")
}

#[test]
fn a_compiled_dto_design_is_contract_complete() {
    let model = childops();
    assert_eq!(check_contract(&model), Vec::new());
    let shape = model.modules[0]
        .designs
        .iter()
        .find_map(|design| design.dto_shape.as_ref())
        .expect("the mirror dto lowers a boundary shape");
    assert_eq!(shape.name, "PayRunLineDraft");
    assert_eq!(shape.direction, DtoDirection::Input);
    assert_eq!(shape.source, Some(payroll_class("PayRunLineDraft")));
}

#[test]
fn a_projection_dto_on_a_designed_source_class_is_contract_complete() {
    let (model, _) = with_dto(compiled(), book_summary());
    assert_eq!(check_contract(&model), Vec::new());
}

#[test]
fn a_standalone_input_dto_may_fill_a_parameter() {
    let (mut model, index) = with_dto(compiled(), create_book_request());
    model.modules[0].designs[index].flags.non_persistent = true;
    let operation = model.modules[0].services[0]
        .operations
        .iter_mut()
        .find(|operation| operation.name == "borrow")
        .expect("borrow lowers");
    operation.params[1].type_ = DddTypeRef::dto("CreateBookRequest");
    assert_eq!(check_contract(&model), Vec::new());
}

#[test]
fn a_mirror_dto_name_is_exempt_from_its_own_source_collision() {
    let mirror = DtoShape::new("Chapter", DtoDirection::Input)
        .with_source(Some(library_class("Chapter")))
        .with_members(vec![selected("title")]);
    let (model, index) = with_dto(compiled(), mirror);
    let violations = check_contract(&model);
    assert!(
        !has(&violations, &dto_path(index, "dtoShape.name")),
        "{violations:?}"
    );
}

#[test]
fn a_dto_design_without_a_shape_is_reported() {
    let (mut model, index) = with_dto(compiled(), book_summary());
    model.modules[0].designs[index].dto_shape = None;
    let violations = check_contract(&model);
    assert!(
        has(&violations, &dto_path(index, "dtoShape")),
        "{violations:?}"
    );
}

#[test]
fn a_non_dto_design_carrying_a_shape_is_reported() {
    let (mut model, index) = with_dto(compiled(), book_summary());
    model.modules[0].designs[index].stereotype = Stereotype::Entity;
    let violations = check_contract(&model);
    assert!(
        has(&violations, &dto_path(index, "dtoShape")),
        "{violations:?}"
    );
}

#[test]
fn a_standalone_dto_must_not_carry_a_class_or_type_ref() {
    let (mut model, index) = with_dto(compiled(), create_book_request());
    model.modules[0].designs[index].class = Some("Book".to_string());
    model.modules[0].designs[index].type_ref = Some(library_class("Book"));
    let violations = check_contract(&model);
    assert!(
        has(&violations, &dto_path(index, "class")),
        "{violations:?}"
    );
    assert!(
        has(&violations, &dto_path(index, "typeRef")),
        "{violations:?}"
    );
}

#[test]
fn a_projection_dto_must_agree_with_its_source() {
    let (mut model, index) = with_dto(compiled(), book_summary());
    model.modules[0].designs[index].class = Some("Movie".to_string());
    let violations = check_contract(&model);
    assert!(
        has(&violations, &dto_path(index, "class")),
        "the authored class must match the source: {violations:?}"
    );

    model.modules[0].designs[index].class = Some("Book".to_string());
    model.modules[0].designs[index].type_ref = Some(library_class("Movie"));
    let violations = check_contract(&model);
    assert!(
        has(&violations, &dto_path(index, "typeRef")),
        "the resolved reference must match the source: {violations:?}"
    );

    model.modules[0].designs[index].type_ref = None;
    let violations = check_contract(&model);
    assert!(
        has(&violations, &dto_path(index, "typeRef")),
        "a sourced dto requires a typeRef: {violations:?}"
    );
}

#[test]
fn a_non_dto_design_requires_a_class() {
    let mut model = compiled();
    model.modules[0].designs[1].class = None;
    let violations = check_contract(&model);
    assert!(
        has(&violations, "modules[0].designs[1].class"),
        "{violations:?}"
    );
}

#[test]
fn dto_shape_names_are_required_and_unique() {
    let (mut model, index) = with_dto(compiled(), create_book_request());
    model.modules[0].designs[index]
        .dto_shape
        .as_mut()
        .expect("the dto carries a shape")
        .name
        .clear();
    let violations = check_contract(&model);
    assert!(
        has(&violations, &dto_path(index, "dtoShape.name")),
        "{violations:?}"
    );

    let (model, first) = with_dto(compiled(), create_book_request());
    let (model, second) = with_dto(model, create_book_request());
    let violations = check_contract(&model);
    assert!(
        has(&violations, &dto_path(second, "dtoShape.name")),
        "the second boundary's name must be reported: {violations:?}"
    );
    assert!(
        !has(&violations, &dto_path(first, "dtoShape.name")),
        "the first declaration is the registered one: {violations:?}"
    );
}

#[test]
fn a_dto_name_must_not_shadow_a_designed_class() {
    let shadow = DtoShape::new("Book", DtoDirection::Input).with_members(vec![declared("title")]);
    let (model, index) = with_dto(compiled(), shadow);
    let violations = check_contract(&model);
    assert!(
        has(&violations, &dto_path(index, "dtoShape.name")),
        "{violations:?}"
    );
}

#[test]
fn dto_member_names_are_required_and_unique() {
    let shape = DtoShape::new("BookSummary", DtoDirection::Input)
        .with_source(Some(library_class("Book")))
        .with_members(vec![selected(""), selected("title"), selected("title")]);
    let (model, index) = with_dto(compiled(), shape);
    let violations = check_contract(&model);
    assert!(
        has(&violations, &dto_path(index, "dtoShape.members[0].name")),
        "{violations:?}"
    );
    assert!(
        has(&violations, &dto_path(index, "dtoShape.members[2].name")),
        "{violations:?}"
    );
}

#[test]
fn dto_member_payloads_must_match_their_kind() {
    let shape = DtoShape::new("BookSummary", DtoDirection::Input)
        .with_source(Some(library_class("Book")))
        .with_members(vec![
            DtoMember::new("emptySelect", DtoMemberKind::Select, string_type())
                .with_expression(Some("title".to_string())),
            DtoMember::new("pathlessDerive", DtoMemberKind::Derive, string_type())
                .with_path(vec!["title".to_string()]),
            DtoMember::new("carryingDeclared", DtoMemberKind::Declared, string_type())
                .with_path(vec!["title".to_string()])
                .with_expression(Some("title".to_string())),
        ]);
    let (model, index) = with_dto(compiled(), shape);
    let violations = check_contract(&model);
    for member in 0..3 {
        assert!(
            has(
                &violations,
                &dto_path(index, &format!("dtoShape.members[{member}].path"))
            ),
            "member {member}: {violations:?}"
        );
        assert!(
            has(
                &violations,
                &dto_path(index, &format!("dtoShape.members[{member}].expression"))
            ),
            "member {member}: {violations:?}"
        );
    }
}

#[test]
fn dto_direction_is_derived_from_the_members() {
    let input_with_derive = DtoShape::new("BookSummary", DtoDirection::Input)
        .with_source(Some(library_class("Book")))
        .with_members(vec![selected("title"), derived("label", "title")]);
    let (model, index) = with_dto(compiled(), input_with_derive);
    let violations = check_contract(&model);
    assert!(
        has(&violations, &dto_path(index, "dtoShape.direction")),
        "a derive member makes the boundary output-only: {violations:?}"
    );

    let output_without_derive = DtoShape::new("BookSummary", DtoDirection::Output)
        .with_source(Some(library_class("Book")))
        .with_members(vec![selected("title")]);
    let (model, index) = with_dto(compiled(), output_without_derive);
    let violations = check_contract(&model);
    assert!(
        has(&violations, &dto_path(index, "dtoShape.direction")),
        "an all-select boundary is input-capable: {violations:?}"
    );
}

#[test]
fn every_dto_reference_must_resolve() {
    let (mut model, index) = with_dto(compiled(), create_book_request());
    model.modules[0].designs[index]
        .dto_shape
        .as_mut()
        .expect("the dto carries a shape")
        .members[0]
        .type_ = DddTypeRef::dto("GhostMember");
    let operation = model.modules[0].services[0]
        .operations
        .iter_mut()
        .find(|operation| operation.name == "borrow")
        .expect("borrow lowers");
    operation.return_type = Some(DddTypeRef::dto("GhostReturn"));
    operation.params[0].type_ = DddTypeRef::dto("GhostParam");
    let repository_operation = model.modules[0].designs[0]
        .repository
        .as_mut()
        .expect("Library has a repository")
        .operations
        .iter_mut()
        .find(|operation| operation.name == "renameLibrary")
        .expect("renameLibrary is declared");
    repository_operation.return_type = Some(DddTypeRef::dto("GhostRepoReturn"));

    let violations = check_contract(&model);
    assert!(
        has(&violations, &dto_path(index, "dtoShape.members[0].type")),
        "{violations:?}"
    );
    assert!(
        has(
            &violations,
            "modules[0].services[0].operations[0].returnType"
        ),
        "{violations:?}"
    );
    assert!(
        has(
            &violations,
            "modules[0].services[0].operations[0].params[0].type"
        ),
        "{violations:?}"
    );
    assert!(
        has(
            &violations,
            "modules[0].designs[0].repository.operations[4].returnType"
        ),
        "{violations:?}"
    );
}

#[test]
fn an_output_dto_cannot_fill_a_parameter() {
    let (mut model, _) = with_dto(compiled(), book_summary());
    let operation = model.modules[0].services[0]
        .operations
        .iter_mut()
        .find(|operation| operation.name == "borrow")
        .expect("borrow lowers");
    operation.params[1].type_ = DddTypeRef::dto("BookSummary");
    let violations = check_contract(&model);
    let path = "modules[0].services[0].operations[0].params[1].type";
    assert!(has(&violations, path), "{violations:?}");
    assert!(
        violations
            .iter()
            .any(|violation| violation.path == path && violation.message.contains("label")),
        "the message names the derived member a caller cannot supply: {violations:?}"
    );
}

#[test]
fn a_dto_cannot_carry_entity_only_state() {
    let (mut model, index) = with_dto(compiled(), book_summary());
    let design = &mut model.modules[0].designs[index];
    design.machine = Some(StateMachine::new(
        "state",
        TypeRef::Enum {
            package: "nz.example.library".to_string(),
            name: "MediaStatus".to_string(),
        },
        "available",
    ));
    design.flags.scaffold = true;
    design.flags.auditable = true;
    design.flags.optimistic_locking = true;
    let violations = check_contract(&model);
    for suffix in [
        "machine",
        "flags.scaffold",
        "flags.auditable",
        "flags.optimisticLocking",
    ] {
        assert!(
            has(&violations, &dto_path(index, suffix)),
            "{suffix}: {violations:?}"
        );
    }
}

// --- declared error vocabulary and throws (DD7, pherolang#304) ---------------

/// A focused design over `library.mox` exercising the vocabulary: an
/// application error, a module-private error, a DTO payload, a body raising
/// both a declared error and the prelude, and a delegation whose `throws` is
/// a superset of its target's.
fn compiled_errors() -> DddModel {
    let (domain_path, domain) = fixture("library.mox");
    let design = r#"
import "library.mox";

application Loans {
    base nz.example.library
    error BookUnavailable(String isbn);

    module lending {
        error LoanAlreadyOpen(String loanId);
        error ReceiptError(LoanReceipt receipt);
        dto LoanReceipt { String isbn; }
        service LoanService {
            inject BookRepository;
            boolean borrow(String isbn) throws BookUnavailable, NotFound => BookRepository {
                let book = self.findById(isbn) else error NotFound("Book", isbn);
                guard book.isbn != "" else error BookUnavailable(isbn);
                return true;
            }
        }
        service FrontDesk {
            inject LoanService;
            borrow => LoanService.borrow throws BookUnavailable, NotFound, LoanAlreadyOpen;
        }
        entity Book repository BookRepository {
            findById;
            LoanReceipt receipt(String isbn);
        }
        entity Chapter
    }
}
"#;
    let design_path = Path::new(&domain_path)
        .with_file_name("errors.ddd")
        .display()
        .to_string();
    let compilation = compile_ddd_str(
        &design_path,
        design,
        &phero_driver::CompileInputs::from_domains(&[(domain_path, domain)]),
    );
    assert_clean(&compilation.diagnostics);
    compilation.model.expect("the error fixture lowers")
}

#[test]
fn a_compiled_error_vocabulary_is_contract_complete() {
    let model = compiled_errors();
    assert_eq!(check_contract(&model), Vec::new());
    assert_eq!(model.errors.len(), 1);
    assert_eq!(model.modules[0].errors.len(), 2);
    let service = &model.modules[0].services[0];
    assert_eq!(service.operations[0].throws.len(), 2);
}

#[test]
fn a_duplicate_error_name_is_reported() {
    let mut model = compiled_errors();
    let duplicate = model.errors[0].clone();
    model.modules[0].errors.push(duplicate);
    let violations = check_contract(&model);
    assert!(
        violations.iter().any(|violation| violation
            .message
            .contains("duplicate error 'BookUnavailable'")),
        "{violations:?}"
    );
}

#[test]
fn a_prelude_shadowing_error_is_reported() {
    let mut model = compiled_errors();
    let mut shadow = model.errors[0].clone();
    shadow.name = "NotFound".to_string();
    model.errors.push(shadow);
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("shadows the standard prelude")),
        "{violations:?}"
    );
}

#[test]
fn a_duplicate_error_payload_field_is_reported() {
    let mut model = compiled_errors();
    let field = model.errors[0].fields[0].clone();
    model.errors[0].fields.push(field);
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("duplicate payload field 'isbn'")),
        "{violations:?}"
    );
}

#[test]
fn an_unresolved_dto_error_payload_is_reported() {
    let mut model = compiled_errors();
    model.modules[0].errors[1].fields[0].type_ = DddTypeRef::dto("Missing");
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("references dto 'Missing'")),
        "{violations:?}"
    );
}

#[test]
fn a_throws_reference_to_no_declared_error_is_reported() {
    let mut model = compiled_errors();
    model.modules[0].services[0].operations[0].throws[0].name = "Nope".to_string();
    let violations = check_contract(&model);
    assert!(
        violations.iter().any(|violation| violation
            .message
            .contains("'Nope' resolves to no declared error")),
        "{violations:?}"
    );
}

#[test]
fn a_throws_reference_with_the_wrong_scope_is_reported() {
    // An application-scoped reference naming a module-private error.
    let mut model = compiled_errors();
    model.modules[0].services[1].operations[0].throws[2] =
        phero_ir::ddd::ErrorRef::application("LoanAlreadyOpen");
    let violations = check_contract(&model);
    assert!(
        violations.iter().any(|violation| violation
            .message
            .contains("is declared in module 'lending'")),
        "{violations:?}"
    );

    // A module-scoped reference naming an application error.
    let mut model = compiled_errors();
    model.modules[0].services[1].operations[0].throws[0] =
        phero_ir::ddd::ErrorRef::module("BookUnavailable", "lending");
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("is application-scoped")),
        "{violations:?}"
    );
}

#[test]
fn a_module_scoped_reference_must_name_its_declaring_module() {
    let mut model = compiled_errors();
    model.modules[0].services[1].operations[0].throws[2] =
        phero_ir::ddd::ErrorRef::module("LoanAlreadyOpen", "elsewhere");
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("is private to module 'lending'")),
        "{violations:?}"
    );
}

#[test]
fn a_prelude_reference_must_name_a_prelude_error() {
    let mut model = compiled_errors();
    model.modules[0].services[0].operations[0].throws[1].name = "Nope".to_string();
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("is not a prelude error")),
        "{violations:?}"
    );
}

#[test]
fn a_body_raise_outside_throws_is_reported() {
    let mut model = compiled_errors();
    let operation = &mut model.modules[0].services[0].operations[0];
    let body = operation.bodies.get("stmt").expect("stmt body").clone();
    operation.bodies.insert(
        "stmt".to_string(),
        body.replace(
            "\"variant\":\"BookUnavailable\"",
            "\"variant\":\"Conflict\"",
        ),
    );
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("the body raises 'Conflict'")),
        "{violations:?}"
    );
}

#[test]
fn a_narrowed_delegation_throws_is_reported() {
    let mut model = compiled_errors();
    model.modules[0].services[1].operations[0].throws.pop();
    model.modules[0].services[1].operations[0].throws.pop();
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("narrows the target's throws")),
        "{violations:?}"
    );
}

#[test]
fn a_builtin_repository_operation_with_throws_is_reported() {
    let mut model = compiled_errors();
    let repository = model.modules[0]
        .designs
        .iter_mut()
        .find(|design| design.class.as_deref() == Some("Book"))
        .and_then(|design| design.repository.as_mut())
        .expect("repository");
    let builtin = repository
        .operations
        .iter_mut()
        .find(|operation| operation.builtin.is_some())
        .expect("findById");
    builtin.throws = vec![phero_ir::ddd::ErrorRef::application("BookUnavailable")];
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("takes no `throws` clause")),
        "{violations:?}"
    );
}

#[test]
fn an_entity_without_an_effective_boundary_is_not_contract_complete() {
    let mut model = compiled();
    let design = model
        .modules
        .iter_mut()
        .flat_map(|module| module.designs.iter_mut())
        .find(|design| design.stereotype == Stereotype::Entity)
        .expect("an entity design");
    design.is_aggregate_root = None;
    design.aggregate = None;
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.path.contains("isAggregateRoot")),
        "{violations:?}"
    );
}

#[test]
fn a_member_must_name_a_root_entity_design() {
    let mut model = compiled();
    let member = model
        .modules
        .iter_mut()
        .flat_map(|module| module.designs.iter_mut())
        .find(|design| design.is_aggregate_root == Some(false))
        .expect("a member design");
    member.aggregate = Some(phero_ir::ddd::AggregateMembership {
        root: TypeRef::Class {
            package: "nz.example.library".to_string(),
            name: "Missing".to_string(),
        },
        origin: phero_ir::ddd::AggregateOrigin::Derived,
    });
    let violations = check_contract(&model);
    assert!(
        violations.iter().any(|violation| violation
            .message
            .contains("not an entity design marked as a root")),
        "{violations:?}"
    );
}

#[test]
fn dto_invariants_and_empty_expressions_are_refused() {
    let mut model = compiled();
    let position = model.modules.iter().enumerate().find_map(|(module, body)| {
        body.designs
            .iter()
            .position(|design| design.stereotype == Stereotype::Dto)
            .map(|design| (module, design))
    });
    let (module, design) = match position {
        Some(position) => position,
        None => {
            let design = model.modules[0].designs.len();
            model.modules[0]
                .designs
                .push(Design::standalone_dto("Boundary"));
            (0, design)
        }
    };
    model.modules[module].designs[design].invariants = vec![phero_ir::ddd::Invariant::new("")];
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("cannot carry invariants")),
        "{violations:?}"
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("must be non-empty")),
        "{violations:?}"
    );
}

// --- typed repository queries (DD9) -----------------------------------------

/// The canonical read-only query: a typed sort key and a fully-bounded
/// pagination window.
fn sample_query() -> QuerySpec {
    QuerySpec::new("customer == self.customer and self.status == state")
        .with_sort(vec![QuerySortKey::new(
            "createdDate",
            TypeRef::Primitive(PrimitiveType::Date),
            SortDirection::Desc,
        )])
        .with_pagination(Some(QueryPagination {
            limit: Some(20),
            max: Some(100),
            cursor: false,
        }))
}

/// A valid query operation: a declared signature returning the matching
/// aggregates, the query block, and the capabilities guarding it.
fn query_operation() -> RepositoryOperation {
    RepositoryOperation::declared(
        "findByCustomer",
        Some(DddTypeRef::domain(library_class("Book"))),
        Vec::new(),
    )
    .with_return_multiplicity(Some(Multiplicity::MANY))
    .with_query(Some(sample_query()))
    .with_capabilities(vec![CapabilityRef::new("SearchBooks")])
}

/// Installs the canonical query operation on the compiled library
/// repository and returns the artifact plus the operation's JSON path.
fn with_query_operation(mut model: DddModel) -> (DddModel, String) {
    let repository = model.modules[0].designs[0]
        .repository
        .as_mut()
        .expect("library.ddd designs a Book repository");
    repository.operations.push(query_operation());
    let index = repository.operations.len() - 1;
    (
        model,
        format!("modules[0].designs[0].repository.operations[{index}]"),
    )
}

fn query_operation_mut(model: &mut DddModel) -> &mut RepositoryOperation {
    model.modules[0].designs[0]
        .repository
        .as_mut()
        .expect("library.ddd designs a Book repository")
        .operations
        .last_mut()
        .expect("the query operation")
}

#[test]
fn a_clean_query_operation_is_contract_complete() {
    let (model, _) = with_query_operation(compiled());
    assert_eq!(check_contract(&model), Vec::new());
}

#[test]
fn a_query_on_a_builtin_operation_is_reported() {
    let (mut model, path) = with_query_operation(compiled());
    {
        let operation = query_operation_mut(&mut model);
        operation.builtin = Some(BuiltinRepositoryOp::FindAll);
        operation.return_type = None;
        operation.return_multiplicity = None;
    }
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.path == format!("{path}.query")
                && violation.message.contains("builtin")),
        "{violations:?}"
    );
}

#[test]
fn a_query_must_be_read_only() {
    let (mut model, path) = with_query_operation(compiled());
    query_operation_mut(&mut model)
        .query
        .as_mut()
        .expect("the query operation carries a query")
        .read_only = false;
    let violations = check_contract(&model);
    assert!(
        has(&violations, &format!("{path}.query.readOnly")),
        "{violations:?}"
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("read-only")),
        "{violations:?}"
    );
}

#[test]
fn a_query_requires_a_non_empty_predicate() {
    let (mut model, path) = with_query_operation(compiled());
    query_operation_mut(&mut model)
        .query
        .as_mut()
        .expect("the query operation carries a query")
        .predicate = "   ".to_string();
    let violations = check_contract(&model);
    assert!(
        has(&violations, &format!("{path}.query.predicate")),
        "{violations:?}"
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("non-empty predicate")),
        "{violations:?}"
    );
}

#[test]
fn a_query_cannot_carry_an_executable_body() {
    let (mut model, path) = with_query_operation(compiled());
    let mut bodies = BTreeMap::new();
    bodies.insert("stmt".to_string(), r#"{"stmts":[]}"#.to_string());
    query_operation_mut(&mut model).bodies = bodies;
    let violations = check_contract(&model);
    assert!(
        has(&violations, &format!("{path}.bodies")),
        "{violations:?}"
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("executable body")),
        "{violations:?}"
    );
}

#[test]
fn a_query_cannot_carry_a_condition() {
    let (mut model, path) = with_query_operation(compiled());
    query_operation_mut(&mut model).condition = Some("self.status == state".to_string());
    let violations = check_contract(&model);
    assert!(
        has(&violations, &format!("{path}.condition")),
        "{violations:?}"
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("cannot carry a condition")),
        "{violations:?}"
    );
}

#[test]
fn a_query_cannot_carry_a_throws_clause() {
    let (mut model, path) = with_query_operation(compiled());
    query_operation_mut(&mut model).throws = vec![ErrorRef::prelude("NotFound")];
    let violations = check_contract(&model);
    assert!(
        has(&violations, &format!("{path}.throws")),
        "{violations:?}"
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("`throws` clause")),
        "{violations:?}"
    );
}

#[test]
fn a_query_pagination_window_must_be_positive_and_ordered() {
    let (mut model, path) = with_query_operation(compiled());
    query_operation_mut(&mut model)
        .query
        .as_mut()
        .expect("the query operation carries a query")
        .pagination = Some(QueryPagination {
        limit: Some(200),
        max: Some(100),
        cursor: false,
    });
    let violations = check_contract(&model);
    assert!(
        violations.iter().any(|violation| violation.path
            == format!("{path}.query.pagination.limit")
            && violation.message.contains("must not exceed max")),
        "{violations:?}"
    );

    let (mut model, path) = with_query_operation(compiled());
    query_operation_mut(&mut model)
        .query
        .as_mut()
        .expect("the query operation carries a query")
        .pagination = Some(QueryPagination {
        limit: Some(0),
        max: Some(100),
        cursor: false,
    });
    let violations = check_contract(&model);
    assert!(
        violations.iter().any(|violation| violation.path
            == format!("{path}.query.pagination.limit")
            && violation.message.contains("greater than zero")),
        "{violations:?}"
    );

    let (mut model, path) = with_query_operation(compiled());
    query_operation_mut(&mut model)
        .query
        .as_mut()
        .expect("the query operation carries a query")
        .pagination = Some(QueryPagination {
        limit: Some(10),
        max: Some(0),
        cursor: false,
    });
    let violations = check_contract(&model);
    assert!(
        violations.iter().any(
            |violation| violation.path == format!("{path}.query.pagination.max")
                && violation.message.contains("greater than zero")
        ),
        "{violations:?}"
    );
}

#[test]
fn capabilities_require_a_query() {
    let mut model = compiled();
    model.modules[0].designs[0]
        .repository
        .as_mut()
        .expect("library.ddd designs a Book repository")
        .operations[0]
        .capabilities = vec![CapabilityRef::new("SearchBooks")];
    let violations = check_contract(&model);
    assert!(
        has(
            &violations,
            "modules[0].designs[0].repository.operations[0].capabilities"
        ),
        "{violations:?}"
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("only when it carries a query")),
        "{violations:?}"
    );
}

// --- named search documents (DF3, pherolang#326) -----------------------------

fn named_document_path() -> String {
    // queries.ddd's module 0 declares one search with `document OrderSummary;`.
    "modules[0].searches[0].documentRef".to_string()
}

#[test]
fn a_compiled_named_document_is_contract_complete() {
    let model = compiled_queries();
    assert_eq!(check_contract(&model), Vec::new());
    let search = &model.modules[0].searches[0];
    assert_eq!(
        search.document_ref,
        Some(DddTypeRef::Dto {
            dto: "OrderSummary".to_string()
        }),
        "the named form lowers the reference"
    );
    assert!(
        search.document.is_empty(),
        "the named form lowers no inline entries"
    );
}

#[test]
fn a_named_document_must_resolve_in_its_module() {
    let mut model = compiled_queries();
    model.modules[0].searches[0].document_ref = Some(DddTypeRef::Dto {
        dto: "Missing".to_string(),
    });
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.path == named_document_path()
                && violation
                    .message
                    .contains("not a dto boundary declared in module")),
        "{violations:?}"
    );
}

#[test]
fn a_named_document_source_must_match_the_search_entity() {
    let mut model = compiled_queries();
    let shape = model.modules[0]
        .designs
        .iter_mut()
        .find_map(|design| design.dto_shape.as_mut())
        .expect("OrderSummary lowers");
    shape.source = Some(TypeRef::Class {
        package: "demo.queries".to_string(),
        name: "Customer".to_string(),
    });
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.path == named_document_path()
                && violation.message.contains("but the search targets")),
        "{violations:?}"
    );
}

#[test]
fn a_named_document_cannot_carry_inline_entries() {
    let mut model = compiled_queries();
    model.modules[0].searches[0]
        .document
        .push(DocumentField::new("headline", "orderNo"));
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.path == named_document_path()
                && violation.message.contains("exactly one document shape")),
        "{violations:?}"
    );
}

// --- machine references (issue pherolang#388) -------------------------------

use phero_evidence::sha256_hex;

/// The embedded IR of a minimal two-state machine, in canonical form (keys
/// sorted recursively), and the hash over it.
fn machine_embed() -> (serde_json::Value, String, &'static str) {
    let machine_ir = serde_json::json!({
        "context": { "fields": [] },
        "id": "book",
        "initial": "Available",
        "setup": null,
        "states": [
            {
                "children": [], "description": null, "entry": [], "exit": [],
                "initial": null, "invoke": null, "name": "Available",
                "on_done": [], "output": null, "tags": [],
                "transitions": [
                    { "actions": [], "description": null, "event": "BORROW",
                      "guard": null, "target": "OnLoan", "targets": [] }
                ],
                "type": "atomic"
            },
            {
                "children": [], "description": null, "entry": [], "exit": [],
                "initial": null, "invoke": null, "name": "OnLoan",
                "on_done": [], "output": null, "tags": [],
                "transitions": [],
                "type": "final"
            }
        ],
    });
    let canonical = phero_ir::canonical_json(&machine_ir);
    let hash = sha256_hex(canonical.as_bytes());
    (machine_ir, hash, "Available")
}

/// Installs a consistent referenced-machine embed on the `Book` design:
/// the projected mirror, the canonical IR, and the matching provenance.
fn with_machine_reference(model: &DddModel) -> DddModel {
    let mut model = model.clone();
    let (machine_ir, hash, initial) = machine_embed();
    let design = model.modules[0]
        .designs
        .iter_mut()
        .find(|design| design.class.as_deref() == Some("Book"))
        .expect("the Book design");
    design.stereotype = Stereotype::Entity;
    design.machine = Some(StateMachine::new(
        "state",
        TypeRef::Enum {
            package: "nz.example.library".to_string(),
            name: "MediaStatus".to_string(),
        },
        initial,
    ));
    design
        .machine
        .as_mut()
        .expect("the machine")
        .terminals
        .push("OnLoan".to_string());
    design.machine.as_mut().expect("the machine").transitions.push({
        let mut transition =
            phero_ir::ddd::Transition::new("Available", "OnLoan");
        transition.trigger = Some("BORROW".to_string());
        transition
    });
    design.machine_ir = Some(machine_ir);
    design.machine_provenance =
        Some(phero_ir::ddd::MachineProvenance::new(
            "machines/book.machine.ts",
            "book",
            hash,
        ));
    model
}

#[test]
fn a_consistent_machine_reference_passes_the_contract() {
    let model = with_machine_reference(&compiled());
    assert!(
        check_contract(&model).is_empty(),
        "a consistent embed is contract-complete: {:?}",
        check_contract(&model)
    );
}

#[test]
fn provenance_without_the_embed_is_reported() {
    let mut model = with_machine_reference(&compiled());
    model.modules[0].designs
        .iter_mut()
        .find(|design| design.class.as_deref() == Some("Book"))
        .expect("the Book design")
        .machine_ir = None;
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.path.ends_with(".machineProvenance")),
        "provenance without the embed: {violations:?}"
    );
}

#[test]
fn the_embed_without_provenance_is_reported() {
    let mut model = with_machine_reference(&compiled());
    model.modules[0].designs
        .iter_mut()
        .find(|design| design.class.as_deref() == Some("Book"))
        .expect("the Book design")
        .machine_provenance = None;
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.path.ends_with(".machineIr")),
        "the embed without provenance: {violations:?}"
    );
}

#[test]
fn a_tampered_hash_is_reported() {
    let mut model = with_machine_reference(&compiled());
    let design = model.modules[0].designs
        .iter_mut()
        .find(|design| design.class.as_deref() == Some("Book"))
        .expect("the Book design");
    // Flip one authored byte in the embed: the hash no longer matches.
    if let Some(machine_ir) = design.machine_ir.as_mut() {
        machine_ir["initial"] = serde_json::json!("Shelved");
    }
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.path.ends_with(".machineProvenance")),
        "a lying hash is the one report: {violations:?}"
    );
}

#[test]
fn an_embed_that_is_no_machine_ir_is_reported() {
    let mut model = with_machine_reference(&compiled());
    let design = model.modules[0].designs
        .iter_mut()
        .find(|design| design.class.as_deref() == Some("Book"))
        .expect("the Book design");
    design.machine_ir = Some(serde_json::json!({ "not": "a machine" }));
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.path.ends_with(".machineIr")),
        "a non-IR embed: {violations:?}"
    );
}

#[test]
fn a_mirror_that_disagrees_with_the_embed_is_reported() {
    let mut model = with_machine_reference(&compiled());
    let design = model.modules[0].designs
        .iter_mut()
        .find(|design| design.class.as_deref() == Some("Book"))
        .expect("the Book design");
    // The mirror claims another initial than the authored machine.
    let machine = design.machine.as_mut().expect("the machine");
    machine.initial = "OnLoan".to_string();
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.path.ends_with(".machine")
                && violation.message.contains("initial")),
        "the mirror must agree with the embed: {violations:?}"
    );
}

#[test]
fn an_embed_on_a_value_design_is_reported() {
    let mut model = with_machine_reference(&compiled());
    let design = model.modules[0].designs
        .iter_mut()
        .find(|design| design.class.as_deref() == Some("Book"))
        .expect("the Book design");
    design.stereotype = Stereotype::Value;
    let violations = check_contract(&model);
    assert!(
        violations
            .iter()
            .any(|violation| violation.path.ends_with(".machine")),
        "a value carries no lifecycle: {violations:?}"
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.path.ends_with(".machineIr")),
        "and no embed: {violations:?}"
    );
}
