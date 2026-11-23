use std::collections::BTreeMap;

use super::*;
use crate::{IrError, Multiplicity, PrimitiveType, TypeRef};

/// The provenance version [`DddModel::new`] stamps — derived, not
/// pinned, so a workspace version bump edits the manifest, not these
/// expected serializations (the `crate::tests::PHERO_VERSION` pattern).
const PHERO_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The canonical bytes of an empty design artifact: the version marker
/// plus the provenance line (the [`crate::Model`] precedent).
fn empty_model_json() -> String {
    format!(r#"{{"formatVersion":3,"pheroVersion":"{PHERO_VERSION}"}}"#)
}

fn class_ref(name: &str) -> TypeRef {
    TypeRef::Class {
        package: "nz.example.library".to_string(),
        name: name.to_string(),
    }
}

fn param(name: &str, type_: TypeRef) -> DddParam {
    DddParam::new(name, DddTypeRef::domain(type_))
}

/// A full-featured model: every stereotype, every flag, builtin and
/// declared repository operations, delegation, capabilities, invoked
/// decision services, injected decision dependencies, dependencies,
/// and a qualified base package.
fn sample_model() -> DddModel {
    DddModel::new()
        .application(Application {
            name: "Library".to_string(),
            base: Some("nz.example.library".to_string()),
        })
        .module(Module {
            name: "catalogue".to_string(),
            errors: Vec::new(),
            services: vec![Service {
                name: "LoanService".to_string(),
                description: Some("Manages loans".to_string()),
                operations: vec![
                    ServiceOperation {
                        name: "borrow".to_string(),
                        return_type: Some(DddTypeRef::domain(TypeRef::Primitive(
                            PrimitiveType::Boolean,
                        ))),
                        return_multiplicity: None,
                        params: vec![param("book", class_ref("Book"))],
                        delegation: None,
                        is_protected: false,
                        capabilities: vec!["BorrowBooks".to_string()],
                        capability_refs: vec![CapabilityRef::new("BorrowBooks")
                            .with_declared_in(Some("Support".to_string()))],
                        emits: vec!["LoanStateChanged".to_string()],
                        emitted_refs: vec![EventRef::new("LoanStateChanged")
                            .with_contract(Some("support.evt".to_string()))],
                        invokes: vec!["RiskScore".to_string()],
                        decision_refs: vec![DecisionRef::new("RiskScore")
                            .with_contract(Some("lending".to_string()))
                            .with_version(Some("1.4.2".to_string()))],
                        decision_pins: vec![DecisionPin {
                            name: "RiskScore".to_string(),
                            pin: crate::dmn::VersionPin::Compatible,
                        }],
                        condition: None,
                        condition_paths: Vec::new(),
                        transition: None,
                        child_scope: None,
                        throws: Vec::new(),
                        bodies: BTreeMap::new(),
                        body_receiver: None,
                    },
                    ServiceOperation {
                        name: "renew".to_string(),
                        return_type: None,
                        return_multiplicity: None,
                        params: Vec::new(),
                        delegation: Some(Delegation {
                            target: "LoanRepository".to_string(),
                            target_ref: Some(TargetRef::new(
                                TargetKind::Repository,
                                "Library.catalogue.LoanRepository",
                                "catalogue",
                            )),
                            operation: "save".to_string(),
                        }),
                        is_protected: false,
                        capabilities: Vec::new(),
                        capability_refs: Vec::new(),
                        emits: Vec::new(),
                        emitted_refs: Vec::new(),
                        invokes: Vec::new(),
                        decision_refs: Vec::new(),
                        decision_pins: Vec::new(),
                        condition: None,
                        condition_paths: Vec::new(),
                        transition: None,
                        child_scope: None,
                        throws: Vec::new(),
                        bodies: BTreeMap::new(),
                        body_receiver: None,
                    },
                ],
                dependencies: vec![
                    "LoanRepository".to_string(),
                    "NotificationService".to_string(),
                ],
                dependency_refs: vec![
                    TargetRef::new(
                        TargetKind::Repository,
                        "Library.catalogue.LoanRepository",
                        "catalogue",
                    ),
                    TargetRef::new(
                        TargetKind::Service,
                        "Library.catalogue.NotificationService",
                        "catalogue",
                    ),
                ],
                decision_dependencies: vec!["RiskScore".to_string()],
                decision_dependency_refs: vec![DecisionRef::new("RiskScore")
                    .with_contract(Some("lending".to_string()))
                    .with_version(Some("1.4.2".to_string()))],
                decision_dependency_pins: vec![DecisionPin {
                    name: "RiskScore".to_string(),
                    pin: crate::dmn::VersionPin::Exact("1.4.2".to_string()),
                }],
            }],
            designs: vec![
                Design {
                    class: Some("nz.example.library::Book".to_string()),
                    type_ref: Some(class_ref("Book")),
                    stereotype: Stereotype::Entity,
                    is_abstract: false,
                    flags: DesignFlags {
                        scaffold: true,
                        cache: true,
                        ..Default::default()
                    },
                    semantics: None,
                    dto_shape: None,
                    is_aggregate_root: None,
                    aggregate: None,
                    invariants: Vec::new(),
                    repository: Some(Repository {
                        name: "BookRepository".to_string(),
                        operations: vec![
                            RepositoryOperation::builtin("findById", BuiltinRepositoryOp::FindById),
                            RepositoryOperation::builtin("findAll", BuiltinRepositoryOp::FindAll),
                            RepositoryOperation::builtin("save", BuiltinRepositoryOp::Save),
                            RepositoryOperation::builtin("delete", BuiltinRepositoryOp::Delete),
                            RepositoryOperation::declared(
                                "findByTitle",
                                Some(DddTypeRef::domain(TypeRef::Primitive(
                                    PrimitiveType::String,
                                ))),
                                vec![param("title", TypeRef::Primitive(PrimitiveType::String))],
                            ),
                        ],
                    }),
                    machine: None,
                    machine_ir: None,
                    machine_provenance: None,
                    child_operations: Vec::new(),
                },
                Design {
                    class: Some("Money".to_string()),
                    type_ref: Some(class_ref("Money")),
                    stereotype: Stereotype::Value,
                    is_abstract: false,
                    flags: DesignFlags::default(),
                    semantics: Some(ValueSemantics::strict()),
                    dto_shape: None,
                    is_aggregate_root: None,
                    aggregate: None,
                    invariants: Vec::new(),
                    repository: None,
                    machine: None,
                    machine_ir: None,
                    machine_provenance: None,
                    child_operations: Vec::new(),
                },
                Design {
                    class: Some("LoanSummary".to_string()),
                    type_ref: Some(class_ref("LoanSummary")),
                    stereotype: Stereotype::Dto,
                    is_abstract: true,
                    flags: DesignFlags {
                        auditable: true,
                        optimistic_locking: true,
                        non_persistent: true,
                        ..Default::default()
                    },
                    semantics: None,
                    dto_shape: None,
                    is_aggregate_root: None,
                    aggregate: None,
                    invariants: Vec::new(),
                    repository: None,
                    machine: None,
                    machine_ir: None,
                    machine_provenance: None,
                    child_operations: Vec::new(),
                },
            ],
            searches: Vec::new(),
        })
}

#[test]
fn new_and_default_stamp_format_version() {
    assert_eq!(sample_model().format_version, DDD_MODEL_FORMAT_VERSION);
    assert_eq!(DddModel::default().format_version, DDD_MODEL_FORMAT_VERSION);
    assert_eq!(DDD_MODEL_FORMAT_VERSION, 3);
}

/// The shared wire-contract characterization (see
/// `wire::tests::assert_wire_contract`): an empty artifact serializes
/// as exactly the version marker plus the provenance line (as both
/// `new()` and `Default` — the helper pins `Default`), the gate rejects
/// wrong/absent versions, unknown fields are tolerated, and the
/// populated design round-trips byte-stably through both serializers.
#[test]
fn empty_model_serializes_as_exactly_format_version() {
    crate::wire::tests::assert_wire_contract(&empty_model_json(), &sample_model());
}

#[test]
fn wire_format_is_camel_case_with_no_snake_case_keys() {
    let json = sample_model().to_json().expect("serialize");
    assert!(json.contains("\"formatVersion\":3"), "{json}");
    assert!(json.contains("\"returnType\""), "{json}");
    assert!(json.contains("\"isAbstract\":true"), "{json}");
    assert!(json.contains("\"optimisticLocking\":true"), "{json}");
    assert!(json.contains("\"nonPersistent\":true"), "{json}");
    assert!(json.contains("\"base\":\"nz.example.library\""), "{json}");
    assert!(
        json.contains(
            r#""semantics":{"identity":"none","immutable":true,"deepImmutable":true,"equality":"structural"}"#
        ),
        "a value design carries the fixed semantics block: {json}"
    );
    assert!(
        json.contains(r#"{"type":"class","value":{"package":"nz.example.library","name":"Book"}}"#),
        "signatures reuse the IR TypeRef adjacent tagging: {json}"
    );
    assert!(!json.contains('_'), "no snake_case keys: {json}");
}

/// The transition binding and the resolved condition paths serialize in
/// the wire shapes of issues pherolang#176/#180: camelCase keys, the
/// unqualified `machine` omitted, and the adjacent tagging of
/// `ConditionScope` (`{"type":"ownRecord"}` /
/// `{"type":"injected","value":{...}}`).
#[test]
fn transition_refs_and_condition_paths_serialize_as_advertised() {
    let unqualified = TransitionRef::new("Submit");
    assert_eq!(
        serde_json::to_string(&unqualified).expect("serialize"),
        r#"{"trigger":"Submit"}"#,
        "an unqualified binding omits `machine`"
    );
    let qualified = TransitionRef::new("Reverse").with_machine(Some("PayRunStatus".to_string()));
    assert_eq!(
        serde_json::to_string(&qualified).expect("serialize"),
        r#"{"trigger":"Reverse","machine":"PayRunStatus"}"#
    );

    let own = ConditionPath::new(
        ConditionScope::OwnRecord,
        "status",
        TypeRef::Enum {
            package: "nz.example.payroll".to_string(),
            name: "PayRunStatus".to_string(),
        },
    );
    assert_eq!(
        serde_json::to_string(&own).expect("serialize"),
        r#"{"scope":{"type":"ownRecord"},"field":"status","fieldType":{"type":"enum","value":{"package":"nz.example.payroll","name":"PayRunStatus"}}}"#
    );
    let injected = ConditionPath::new(
        ConditionScope::Injected {
            repository: "TimecardRepository".to_string(),
            class: "Timecard".to_string(),
        },
        "status",
        TypeRef::Enum {
            package: "nz.example.payroll".to_string(),
            name: "TimecardStatus".to_string(),
        },
    );
    assert_eq!(
        serde_json::to_string(&injected).expect("serialize"),
        r#"{"scope":{"type":"injected","value":{"repository":"TimecardRepository","class":"Timecard"}},"field":"status","fieldType":{"type":"enum","value":{"package":"nz.example.payroll","name":"TimecardStatus"}}}"#
    );

    // Both round-trip: the wire forms reload as the same values.
    let round: ConditionPath =
        serde_json::from_str(&serde_json::to_string(&injected).unwrap()).expect("reload");
    assert_eq!(round, injected);

    // Absence is byte-silent: an operation with no paths and no binding
    // serializes without the new keys (the golden discipline — existing
    // artifacts stay byte-identical).
    let plain = ServiceOperation::new("approve");
    let json = serde_json::to_string(&plain).expect("serialize");
    assert!(!json.contains("conditionPaths"), "{json}");
    assert!(!json.contains("transition"), "{json}");
}

// -----------------------------------------------------------------------
// Resolved identities and provenance (DD1, issue pherolang#298)
// -----------------------------------------------------------------------

/// The resolved-identity reference types serialize in their documented
/// camelCase shapes: the bare lowercase [`TargetKind`] tag, the
/// `qualifiedName`/`module` pair, and an optional provenance member
/// omitted when the counterpart artifact was not supplied at compile
/// time. Every populated form round-trips.
#[test]
fn resolved_identity_refs_serialize_camel_case_and_round_trip() {
    for (kind, json) in [
        (TargetKind::Service, r#""service""#),
        (TargetKind::Repository, r#""repository""#),
        (TargetKind::Search, r#""search""#),
    ] {
        assert_eq!(serde_json::to_string(&kind).expect("serialize"), json);
        assert_eq!(
            serde_json::from_str::<TargetKind>(json).expect("deserialize"),
            kind
        );
    }

    let target = TargetRef::new(
        TargetKind::Repository,
        "Library.catalogue.LoanRepository",
        "catalogue",
    );
    let json = serde_json::to_string(&target).expect("serialize");
    assert_eq!(
        json,
        r#"{"kind":"repository","qualifiedName":"Library.catalogue.LoanRepository","module":"catalogue"}"#
    );
    assert_eq!(
        serde_json::from_str::<TargetRef>(&json).expect("deserialize"),
        target
    );

    assert_eq!(
        serde_json::to_string(&CapabilityRef::new("BorrowBooks")).expect("serialize"),
        r#"{"name":"BorrowBooks"}"#,
        "an unsupplied declaring policy omits `declaredIn`"
    );
    let capability =
        CapabilityRef::new("BorrowBooks").with_declared_in(Some("Support".to_string()));
    let json = serde_json::to_string(&capability).expect("serialize");
    assert_eq!(json, r#"{"name":"BorrowBooks","declaredIn":"Support"}"#);
    assert_eq!(
        serde_json::from_str::<CapabilityRef>(&json).expect("deserialize"),
        capability
    );

    assert_eq!(
        serde_json::to_string(&EventRef::new("LoanStateChanged")).expect("serialize"),
        r#"{"name":"LoanStateChanged"}"#,
        "an unsupplied event contract omits `contract`"
    );
    let event = EventRef::new("LoanStateChanged").with_contract(Some("support.evt".to_string()));
    let json = serde_json::to_string(&event).expect("serialize");
    assert_eq!(
        json,
        r#"{"name":"LoanStateChanged","contract":"support.evt"}"#
    );
    assert_eq!(
        serde_json::from_str::<EventRef>(&json).expect("deserialize"),
        event
    );

    assert_eq!(
        serde_json::to_string(&DecisionRef::new("RiskScore")).expect("serialize"),
        r#"{"service":"RiskScore"}"#,
        "an unsupplied decision contract and version stay omitted"
    );
    let decision = DecisionRef::new("RiskScore")
        .with_contract(Some("lending".to_string()))
        .with_version(Some("1.4.2".to_string()));
    let json = serde_json::to_string(&decision).expect("serialize");
    assert_eq!(
        json,
        r#"{"service":"RiskScore","contract":"lending","version":"1.4.2"}"#
    );
    assert_eq!(
        serde_json::from_str::<DecisionRef>(&json).expect("deserialize"),
        decision
    );
}

/// The resolved-identity fields land in the artifact under their
/// documented camelCase keys, on every carrier: the design's `typeRef`,
/// the delegation's `targetRef`, the service's aligned
/// `dependencyRefs`/`decisionDependencyRefs`, the operation's and the
/// search's `capabilityRefs`, the operation's
/// `emittedRefs`/`decisionRefs`, and the search's `entityRef` plus its
/// members' `typeRef`.
#[test]
fn resolved_identity_fields_serialize_as_advertised() {
    let mut model = sample_model();
    model.modules[0].searches.push(sample_search());
    let json = model.to_json().expect("serialize");
    for key in [
        r#""typeRef""#,
        r#""targetRef""#,
        r#""dependencyRefs""#,
        r#""decisionDependencyRefs""#,
        r#""capabilityRefs""#,
        r#""emittedRefs""#,
        r#""decisionRefs""#,
        r#""entityRef""#,
    ] {
        assert!(json.contains(key), "missing {key}: {json}");
    }
    assert!(
        json.contains(
            r#""typeRef":{"type":"class","value":{"package":"nz.example.library","name":"Book"}}"#
        ),
        "the design's typeRef is the shared IR TypeRef tagging: {json}"
    );
    assert!(
        json.contains(
            r#""targetRef":{"kind":"repository","qualifiedName":"Library.catalogue.LoanRepository","module":"catalogue"}"#
        ),
        "the delegation's targetRef is fully resolved: {json}"
    );
    assert!(
        json.contains(
            r#""entityRef":{"type":"class","value":{"package":"nz.example.library","name":"Book"}}"#
        ),
        "the search's entityRef resolves the searched class: {json}"
    );
    assert_eq!(
        json.matches(r#""capabilityRefs":[{"name":"SearchBooks","declaredIn":"Support"}]"#)
            .count(),
        1,
        "the search's capabilityRefs resolves its guarding capability: {json}"
    );
    assert!(!json.contains('_'), "no snake_case keys: {json}");
    assert_eq!(DddModel::from_json(&json).expect("round-trip"), model);
}

/// `pheroVersion` is provenance, never a gate: an artifact written
/// before the field existed deserializes unchanged, and re-serializing
/// it does not invent a stamp (the reader contract is `formatVersion`
/// alone).
#[test]
fn artifacts_without_phero_version_still_deserialize() {
    let model = DddModel::from_json(r#"{"formatVersion":3}"#).expect("no provenance needed");
    assert_eq!(model.format_version, DDD_MODEL_FORMAT_VERSION);
    assert!(model.phero_version.is_none());
    assert!(model.modules.is_empty());
    assert_eq!(
        model.to_json().expect("serialize"),
        r#"{"formatVersion":3}"#,
        "an unprovenanced artifact stays unprovenanced"
    );

    let full = sample_model();
    assert_eq!(
        full.phero_version.as_deref(),
        Some(PHERO_VERSION),
        "new() stamps the crate version"
    );
}

/// Issue pherolang#179's additive fields are byte-silent when absent: a
/// body-less design and an operation with no child scope serialize
/// exactly as before the fields existed, and the populated forms use
/// the documented camelCase wire shapes.
#[test]
fn child_scope_and_child_operations_are_absent_unless_declared() {
    let design = Design::new("Money", Stereotype::Value);
    assert_eq!(
        serde_json::to_string(&design).expect("serialize"),
        r#"{"class":"Money","stereotype":"value"}"#,
        "a child-operation-less design omits the key"
    );
    let op = ServiceOperation::new("approve");
    assert_eq!(
        serde_json::to_string(&op).expect("serialize"),
        r#"{"name":"approve"}"#,
        "an unscoped operation omits `childScope`"
    );

    let scoped = ServiceOperation::new("addLine").with_child_scope(Some(ChildRef::new(
        "lines",
        "nz.example.payroll.PayRunLine",
        "add",
    )));
    let json = serde_json::to_string(&scoped).expect("serialize");
    assert_eq!(
        json,
        r#"{"name":"addLine","childScope":{"feature":"lines","class":"nz.example.payroll.PayRunLine","action":"add"}}"#
    );
    let parsed: ServiceOperation = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, scoped);

    let design = Design::new("PayRun", Stereotype::Entity).with_child_operations(vec![scoped]);
    let json = serde_json::to_string(&design).expect("serialize");
    assert_eq!(
        json,
        r#"{"class":"PayRun","stereotype":"entity","childOperations":[{"name":"addLine","childScope":{"feature":"lines","class":"nz.example.payroll.PayRunLine","action":"add"}}]}"#
    );
    let parsed: Design = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, design);
}

#[test]
fn builtin_ops_serialize_as_camel_case_strings() {
    assert_eq!(
        serde_json::to_string(&BuiltinRepositoryOp::FindById).expect("serialize"),
        r#""findById""#
    );
    assert_eq!(
        serde_json::to_string(&BuiltinRepositoryOp::FindAll).expect("serialize"),
        r#""findAll""#
    );
    assert_eq!(
        serde_json::to_string(&BuiltinRepositoryOp::Save).expect("serialize"),
        r#""save""#
    );
    assert_eq!(
        serde_json::to_string(&BuiltinRepositoryOp::Delete).expect("serialize"),
        r#""delete""#
    );
    let op = RepositoryOperation::builtin("findById", BuiltinRepositoryOp::FindById);
    assert_eq!(
        serde_json::to_string(&op).expect("serialize"),
        r#"{"name":"findById","builtin":"findById"}"#,
        "a builtin op has no signature of its own"
    );
}

#[test]
fn stereotypes_serialize_as_lowercase_strings() {
    assert_eq!(
        serde_json::to_string(&Stereotype::Entity).expect("serialize"),
        r#""entity""#
    );
    assert_eq!(
        serde_json::to_string(&Stereotype::Value).expect("serialize"),
        r#""value""#
    );
    assert_eq!(
        serde_json::to_string(&Stereotype::Dto).expect("serialize"),
        r#""dto""#
    );
}

#[test]
fn default_flags_and_abstract_false_are_omitted() {
    let design = Design {
        class: Some("Money".to_string()),
        type_ref: None,
        stereotype: Stereotype::Value,
        is_abstract: false,
        flags: DesignFlags::default(),
        semantics: None,
        dto_shape: None,
        is_aggregate_root: None,
        aggregate: None,
        invariants: Vec::new(),
        repository: None,
        machine: None,
        machine_ir: None,
        machine_provenance: None,
        child_operations: Vec::new(),
    };
    assert_eq!(
        serde_json::to_string(&design).expect("serialize"),
        r#"{"class":"Money","stereotype":"value"}"#
    );
}

#[test]
fn all_flags_serialize_with_camel_case_keys() {
    let design = Design {
        class: Some("Loan".to_string()),
        type_ref: None,
        stereotype: Stereotype::Entity,
        is_abstract: true,
        flags: DesignFlags {
            scaffold: true,
            auditable: true,
            optimistic_locking: true,
            non_persistent: true,
            cache: true,
        },
        semantics: None,
        dto_shape: None,
        is_aggregate_root: None,
        aggregate: None,
        invariants: Vec::new(),
        repository: None,
        machine: None,
        machine_ir: None,
        machine_provenance: None,
        child_operations: Vec::new(),
    };
    assert_eq!(
        serde_json::to_string(&design).expect("serialize"),
        r#"{"class":"Loan","stereotype":"entity","isAbstract":true,"flags":{"scaffold":true,"auditable":true,"optimisticLocking":true,"nonPersistent":true,"cache":true}}"#
    );
    assert!(DesignFlags::default().is_empty());
    assert!(!design.flags.is_empty());
}

/// The fixed strict value-semantics block every `value` design lowers
/// (DD2, pherolang#299): no identity, immutable and deep-immutable,
/// structural equality.
fn value_semantics() -> ValueSemantics {
    ValueSemantics {
        identity: IdentityPolicy::None,
        immutable: true,
        deep_immutable: true,
        equality: EqualityPolicy::Structural,
    }
}

#[test]
fn semantics_policies_serialize_as_bare_strings() {
    assert_eq!(
        serde_json::to_string(&IdentityPolicy::None).expect("serialize"),
        r#""none""#
    );
    assert_eq!(
        serde_json::to_string(&EqualityPolicy::Structural).expect("serialize"),
        r#""structural""#
    );
}

#[test]
fn a_value_design_serializes_the_fixed_semantics_block() {
    let design = Design::new("Money", Stereotype::Value).with_semantics(Some(value_semantics()));
    assert_eq!(
        serde_json::to_string(&design).expect("serialize"),
        r#"{"class":"Money","stereotype":"value","semantics":{"identity":"none","immutable":true,"deepImmutable":true,"equality":"structural"}}"#
    );
}

#[test]
fn entity_and_dto_designs_omit_semantics() {
    for stereotype in [Stereotype::Entity, Stereotype::Dto] {
        let design = Design::new("Thing", stereotype);
        let json = serde_json::to_string(&design).expect("serialize");
        assert!(
            !json.contains("semantics"),
            "a {} design carries no semantics block: {json}",
            serde_json::to_string(&stereotype).expect("serialize")
        );
    }
}

#[test]
fn semantics_round_trips_and_stays_additive() {
    let design = Design::new("Money", Stereotype::Value).with_semantics(Some(value_semantics()));
    let json = serde_json::to_string(&design).expect("serialize");
    let reloaded: Design = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(reloaded, design);

    // A value design written before the field existed still deserializes.
    let legacy: Design = serde_json::from_str(r#"{"class":"Money","stereotype":"value"}"#)
        .expect("a semantics-less value design deserializes");
    assert_eq!(legacy.semantics, None);
}

#[test]
fn from_json_rejects_wrong_format_version() {
    let json = r#"{"formatVersion": 4, "modules": []}"#;
    match DddModel::from_json(json) {
        Err(IrError::UnsupportedFormatVersion { found, expected }) => {
            assert_eq!(found, 4);
            assert_eq!(expected, DDD_MODEL_FORMAT_VERSION);
        }
        other => panic!("expected UnsupportedFormatVersion, got {other:?}"),
    }
}

#[test]
fn from_json_rejects_zero_and_missing_format_version() {
    for json in [r#"{"formatVersion":0}"#, r#"{"modules":[]}"#] {
        match DddModel::from_json(json) {
            Err(IrError::UnsupportedFormatVersion { found, expected }) => {
                assert_eq!(found, 0, "an absent version reports as 0");
                assert_eq!(expected, DDD_MODEL_FORMAT_VERSION);
            }
            other => panic!("expected UnsupportedFormatVersion, got {other:?}"),
        }
    }
}

#[test]
fn from_json_tolerates_unknown_fields() {
    let json = r#"{
        "formatVersion": 3,
        "nobodyKnowsMe": {"nested": [1, 2, 3]},
        "application": {"name": "Library", "base": "nz.example.library"}
    }"#;
    let model = DddModel::from_json(json).expect("unknown fields are ignored");
    assert_eq!(
        model
            .application
            .map(|a| a.base.unwrap_or_default())
            .as_deref(),
        Some("nz.example.library")
    );
}

// -----------------------------------------------------------------------
// Search definitions (the `searches` feature of the artifact)
// -----------------------------------------------------------------------

/// A full-featured search definition: every collection populated, a
/// per-field boost and analyzer, `Custom` ranking, pagination,
/// capabilities, and the resolved feature types of DD1.
fn sample_search() -> SearchDef {
    let mut title = SearchField::new("title")
        .with_boost(2.0)
        .with_analyzer("standard");
    title.type_ref = Some(TypeRef::Primitive(PrimitiveType::String));
    let mut category = SearchFilter::new("category");
    category.type_ref = Some(TypeRef::Enum {
        package: "nz.example.library".to_string(),
        name: "BookCategory".to_string(),
    });
    let mut sort = SearchSort::new("title");
    sort.type_ref = Some(TypeRef::Primitive(PrimitiveType::String));
    let mut search = SearchDef::new("BookSearch", "nz.example.library::Book")
        .with_description("Full-text catalogue search")
        .with_text(vec![title, SearchField::new("synopsis")])
        .with_filters(vec![category])
        .with_sort(vec![sort])
        .with_document(vec![DocumentField::new(
            "label",
            r#"title + " - " + synopsis"#,
        )])
        .with_ranking(RankingStrategy::Custom("recency".to_string()))
        .with_analyzer("english")
        .with_pagination(Pagination {
            limit: Some(20),
            max_limit: Some(100),
            cursor: true,
        })
        .with_capabilities(vec!["SearchBooks".to_string()]);
    search.entity_ref = Some(class_ref("Book"));
    search.capability_refs =
        vec![CapabilityRef::new("SearchBooks").with_declared_in(Some("Support".to_string()))];
    search
}

/// The sample model with the search definition installed.
fn search_model() -> DddModel {
    let mut module = Module::new("catalogue");
    module.searches.push(sample_search());
    DddModel::new().module(module)
}

#[test]
fn full_search_round_trips_through_json() {
    let model = search_model();
    let json = model.to_json().expect("serialize");
    let parsed = DddModel::from_json(&json).expect("deserialize");
    assert_eq!(parsed, model);
    assert_eq!(parsed.to_json().expect("re-serialize"), json);
}

#[test]
fn search_wire_format_is_camel_case_with_adjacent_ranking_tags() {
    let json = search_model().to_json().expect("serialize");
    assert!(json.contains("\"searches\":[{"), "{json}");
    assert!(json.contains("\"maxLimit\":100"), "{json}");
    assert!(json.contains("\"boost\":2.0"), "{json}");
    assert!(
        json.contains(r#"{"type":"custom","value":"recency"}"#),
        "ranking is adjacently tagged: {json}"
    );
    assert!(
        json.contains(r#"title + \" - \" + synopsis"#),
        "the expr is verbatim source: {json}"
    );
    assert!(!json.contains('_'), "no snake_case keys: {json}");
}

#[test]
fn ranking_strategies_serialize_as_adjacent_tags() {
    for (strategy, json) in [
        (RankingStrategy::Bm25, r#"{"type":"bm25"}"#),
        (RankingStrategy::TfIdf, r#"{"type":"tfIdf"}"#),
        (RankingStrategy::Exact, r#"{"type":"exact"}"#),
    ] {
        assert_eq!(serde_json::to_string(&strategy).expect("serialize"), json);
        assert_eq!(
            serde_json::from_str::<RankingStrategy>(json).expect("deserialize"),
            strategy
        );
    }
    assert_eq!(
        serde_json::to_string(&RankingStrategy::Custom("x".to_string())).expect("serialize"),
        r#"{"type":"custom","value":"x"}"#
    );
}

#[test]
fn loose_reference_lists_are_omitted_when_empty_and_round_trip_when_set() {
    // Additive rule 9: an operation with no `emits`/`invokes` clause —
    // and a service with no `inject decision` line — serializes without
    // those keys (artifacts authored before the clauses stay
    // byte-identical), and populated lists round-trip.
    let mut op = ServiceOperation::new("approve");
    let json = serde_json::to_string(&op).expect("serialize");
    assert!(!json.contains("emits"), "empty emits are omitted: {json}");
    assert!(
        !json.contains("invokes"),
        "empty invokes are omitted: {json}"
    );
    let parsed: ServiceOperation = serde_json::from_str(&json).expect("deserialize without emits");
    assert_eq!(parsed, op, "the default is the empty list");
    let service = Service::new("LoanService");
    let json = serde_json::to_string(&service).expect("serialize");
    assert!(
        !json.contains("decisionDependencies"),
        "empty decision dependencies are omitted: {json}"
    );
    let parsed: Service = serde_json::from_str(&json).expect("deserialize without injects");
    assert_eq!(parsed, service, "the default is the empty list");

    op.emits = vec!["PayRunStateChanged".to_string()];
    let json = serde_json::to_string(&op).expect("serialize");
    assert_eq!(
        json, r#"{"name":"approve","emits":["PayRunStateChanged"]}"#,
        "camelCase key, in declaration order: {json}"
    );
    let parsed: ServiceOperation = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, op);
    assert_eq!(
        serde_json::to_string(&parsed).expect("re-serialize"),
        json,
        "re-serialization is byte-identical"
    );

    op.invokes = vec!["RiskScore".to_string()];
    let json = serde_json::to_string(&op).expect("serialize");
    assert_eq!(
        json, r#"{"name":"approve","emits":["PayRunStateChanged"],"invokes":["RiskScore"]}"#,
        "the invokes key follows emits, in declaration order: {json}"
    );
    let parsed: ServiceOperation = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, op);

    // The version pins (pherolang#154) serialize additively after the
    // name lists: camelCase keys, only pinned references appear, and
    // the shared `dmn::VersionPin` adjacent tagging holds.
    let mut service = Service::new("LoanService");
    service.decision_dependencies = vec!["RiskScore".to_string()];
    let json = serde_json::to_string(&service).expect("serialize");
    assert_eq!(
        json, r#"{"name":"LoanService","decisionDependencies":["RiskScore"]}"#,
        "a pin-less inject lowers no pin entry: {json}"
    );
    let parsed: Service = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, service);

    service.decision_dependency_pins = vec![DecisionPin {
        name: "RiskScore".to_string(),
        pin: crate::dmn::VersionPin::Compatible,
    }];
    let json = serde_json::to_string(&service).expect("serialize");
    assert_eq!(
        json,
        r#"{"name":"LoanService","decisionDependencies":["RiskScore"],"decisionDependencyPins":[{"name":"RiskScore","pin":{"type":"compatible"}}]}"#,
        "the compatible pin serializes as the bare tag: {json}"
    );
    let parsed: Service = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, service);
    assert_eq!(
        serde_json::to_string(&parsed).expect("re-serialize"),
        json,
        "re-serialization is byte-identical"
    );

    op.decision_pins = vec![DecisionPin {
        name: "RiskScore".to_string(),
        pin: crate::dmn::VersionPin::Exact("1.4.2".to_string()),
    }];
    let json = serde_json::to_string(&op).expect("serialize");
    assert!(
        json.contains(
            r#""decisionPins":[{"name":"RiskScore","pin":{"type":"exact","value":"1.4.2"}}]"#
        ),
        "the exact pin carries its value under the adjacent tag: {json}"
    );
    let parsed: ServiceOperation = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, op);

    // The chainable setters.
    let op = ServiceOperation::new("submit")
        .with_emits(vec!["PayRunSubmitted".to_string()])
        .with_invokes(vec!["CreditCheck".to_string()]);
    assert_eq!(op.emits, vec!["PayRunSubmitted".to_string()]);
    assert_eq!(op.invokes, vec!["CreditCheck".to_string()]);
    assert!(op.decision_pins.is_empty());
}

#[test]
fn a_search_less_module_omits_the_searches_key() {
    let module = Module {
        name: "catalogue".to_string(),
        errors: Vec::new(),
        services: vec![Service::new("LoanService")],
        designs: vec![Design::new("Money", Stereotype::Value)],
        searches: Vec::new(),
    };
    assert_eq!(
        serde_json::to_string(&module).expect("serialize"),
        r#"{"name":"catalogue","services":[{"name":"LoanService"}],"designs":[{"class":"Money","stereotype":"value"}]}"#
    );
}

// -----------------------------------------------------------------------
// Boost: the ordered float newtype (wire contract rule 14)
// -----------------------------------------------------------------------

#[test]
fn boost_serializes_as_the_bare_f32_and_round_trips() {
    for value in [2.5f32, 1.0, 0.1, 3.402_823_5e38, -0.0] {
        let json = serde_json::to_string(&Boost::new(value)).expect("serialize");
        assert_eq!(
            json,
            serde_json::to_string(&value).expect("plain f32"),
            "transparent layout: {json}"
        );
        let back: Boost = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, Boost::new(value));
        assert_eq!(back.value(), value);
    }
}

#[test]
fn boost_equality_is_bit_pattern_equality() {
    // Reflexive even for NaN; a NaN equals only its own bit pattern.
    assert_eq!(Boost::new(f32::NAN), Boost::new(f32::NAN));
    assert_ne!(
        Boost::new(f32::NAN),
        Boost::new(f32::from_bits(f32::NAN.to_bits() + 1))
    );
    assert_eq!(Boost::new(2.5), Boost::new(2.5));
    assert_ne!(Boost::new(2.5), Boost::new(2.75));
    // Intended: -0.0 is its own value, distinct from 0.0.
    assert_ne!(Boost::new(-0.0), Boost::new(0.0));
}

#[test]
fn boost_hash_is_consistent_with_equality() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    fn hash_of(value: f32) -> u64 {
        let mut hasher = DefaultHasher::new();
        Boost::new(value).hash(&mut hasher);
        hasher.finish()
    }
    assert_eq!(hash_of(2.5), hash_of(2.5));
    assert_ne!(hash_of(-0.0), hash_of(0.0));
    assert_ne!(
        hash_of(f32::NAN),
        hash_of(f32::from_bits(f32::NAN.to_bits() + 1))
    );
}

/// Issue #178's body fields are additive: absent from every body-less
/// operation (the byte-identical discipline) and present in the
/// documented camelCase shapes when populated. The `"stmt"` value is an
/// opaque canonical-JSON string — phero-ir never depends on phero-expr.
#[test]
fn operation_bodies_are_additive_and_round_trip() {
    const BODY: &str = r#"{"stmts":[{"kind":"return","expr":"true"}]}"#;

    let plain = ServiceOperation::new("approve");
    assert_eq!(
        serde_json::to_string(&plain).expect("serialize"),
        r#"{"name":"approve"}"#,
        "a body-less operation omits `bodies` and `bodyReceiver`"
    );
    let plain_repository = RepositoryOperation::declared(
        "total",
        Some(DddTypeRef::domain(TypeRef::Primitive(PrimitiveType::Int))),
        Vec::new(),
    );
    assert_eq!(
        serde_json::to_string(&plain_repository).expect("serialize"),
        r#"{"name":"total","returnType":{"type":"primitive","value":"int"}}"#,
        "a body-less repository operation omits `bodies`"
    );

    let mut bodies = std::collections::BTreeMap::new();
    bodies.insert("stmt".to_string(), BODY.to_string());
    let operation = ServiceOperation::new("approve")
        .with_bodies(bodies.clone())
        .with_body_receiver(Some("PayRunRepository".to_string()));
    assert_eq!(
        serde_json::to_string(&operation).expect("serialize"),
        format!(
            r#"{{"name":"approve","bodies":{{"stmt":{}}},"bodyReceiver":"PayRunRepository"}}"#,
            serde_json::to_string(BODY).expect("quote the body")
        )
    );
    let round: ServiceOperation =
        serde_json::from_str(&serde_json::to_string(&operation).unwrap()).expect("reload");
    assert_eq!(round, operation);

    let repository_operation = RepositoryOperation::declared(
        "total",
        Some(DddTypeRef::domain(TypeRef::Primitive(PrimitiveType::Int))),
        Vec::new(),
    )
    .with_bodies(bodies);
    let json = serde_json::to_string(&repository_operation).expect("serialize");
    assert!(json.contains(r#""bodies":{"stmt":"#), "{json}");
    assert!(
        !json.contains("bodyReceiver"),
        "repositories carry no receiver"
    );
    let round: RepositoryOperation = serde_json::from_str(&json).expect("reload");
    assert_eq!(round, repository_operation);

    let empty = DddModel::new();
    assert_eq!(empty.to_json().expect("serialize"), empty_model_json());
}

// -----------------------------------------------------------------------
// DDD-local type references and DTO shapes (DD3, issue pherolang#300)
// -----------------------------------------------------------------------

fn dto_ref(name: &str) -> DddTypeRef {
    DddTypeRef::dto(name)
}

/// A full-featured DTO shape: a resolved source class, both member
/// flavours that carry extra payload, a declared member with a
/// collection cardinality, and a verbatim derivation expression.
fn sample_dto_shape() -> DtoShape {
    DtoShape::new("BookSummary", DtoDirection::Output)
        .with_source(Some(class_ref("Book")))
        .with_members(vec![
            DtoMember::new(
                "title",
                DtoMemberKind::Select,
                DddTypeRef::domain(TypeRef::Primitive(PrimitiveType::String)),
            )
            .with_path(vec!["title".to_string()]),
            DtoMember::new(
                "label",
                DtoMemberKind::Derive,
                DddTypeRef::domain(TypeRef::Primitive(PrimitiveType::String)),
            )
            .with_expression(Some("title + \" - \" + synopsis".to_string())),
            DtoMember::new("tags", DtoMemberKind::Declared, dto_ref("TagSummary"))
                .with_multiplicity(Some(Multiplicity::MANY)),
        ])
}

/// A DTO-typed [`DddTypeRef::Domain`] is **byte-identical** to the shared
/// [`TypeRef`] it wraps: the enum is untagged, so the wrapper adds no
/// level to the wire shape and existing domain-only artifacts stay
/// byte-identical (DD3, pherolang#300).
#[test]
fn ddd_type_ref_domain_renders_the_shared_type_ref_bytes() {
    let type_ref = class_ref("Book");
    let domain = DddTypeRef::domain(type_ref.clone());
    let expected = r#"{"type":"class","value":{"package":"nz.example.library","name":"Book"}}"#;
    assert_eq!(
        serde_json::to_string(&type_ref).expect("serialize the shared TypeRef"),
        expected
    );
    assert_eq!(
        serde_json::to_string(&domain).expect("serialize"),
        expected,
        "the untagged Domain variant adds no wrapper"
    );
    assert_eq!(
        serde_json::from_str::<DddTypeRef>(expected).expect("deserialize"),
        domain
    );
    assert_eq!(domain.as_domain(), Some(&type_ref));
    assert_eq!(domain.dto_name(), None);
    assert_eq!(DddTypeRef::domain(type_ref), domain);
}

/// A DTO boundary reference serializes as the bare boundary name under
/// `dto` and round-trips — never as a qualified [`TypeRef`], because the
/// DTO need not exist in the domain model at all.
#[test]
fn ddd_type_ref_dto_uses_the_dto_key() {
    let dto = dto_ref("BookSummary");
    assert_eq!(
        serde_json::to_string(&dto).expect("serialize"),
        r#"{"dto":"BookSummary"}"#
    );
    assert_eq!(dto.dto_name(), Some("BookSummary"));
    assert_eq!(dto.as_domain(), None);
    assert_eq!(
        serde_json::from_str::<DddTypeRef>(r#"{"dto":"BookSummary"}"#).expect("deserialize"),
        dto
    );
}

#[test]
fn dto_directions_and_member_kinds_are_bare_lowercase_strings() {
    for (direction, json) in [
        (DtoDirection::Input, r#""input""#),
        (DtoDirection::Output, r#""output""#),
    ] {
        assert_eq!(serde_json::to_string(&direction).expect("serialize"), json);
        assert_eq!(
            serde_json::from_str::<DtoDirection>(json).expect("deserialize"),
            direction
        );
    }
    for (kind, json) in [
        (DtoMemberKind::Select, r#""select""#),
        (DtoMemberKind::Derive, r#""derive""#),
        (DtoMemberKind::Declared, r#""declared""#),
    ] {
        assert_eq!(serde_json::to_string(&kind).expect("serialize"), json);
        assert_eq!(
            serde_json::from_str::<DtoMemberKind>(json).expect("deserialize"),
            kind
        );
    }
}

/// `source`, `members`, `multiplicity`, `path`, and `expression` are all
/// additive: an absent one is omitted entirely (the byte-identical
/// discipline), and every populated form uses its documented camelCase
/// key.
#[test]
fn a_minimal_dto_shape_omits_every_optional_member() {
    let shape = DtoShape::new("BookSummary", DtoDirection::Input);
    assert_eq!(
        serde_json::to_string(&shape).expect("serialize"),
        r#"{"name":"BookSummary","direction":"input"}"#,
        "a source-less, member-less shape omits both keys"
    );

    let member = DtoMember::new(
        "id",
        DtoMemberKind::Declared,
        DddTypeRef::domain(TypeRef::Primitive(PrimitiveType::String)),
    );
    assert_eq!(
        serde_json::to_string(&member).expect("serialize"),
        r#"{"name":"id","kind":"declared","type":{"type":"primitive","value":"string"}}"#,
        "a payload-less member omits multiplicity, path, and expression"
    );
}

#[test]
fn a_full_dto_shape_round_trips() {
    let shape = sample_dto_shape();
    let json = serde_json::to_string(&shape).expect("serialize");
    assert_eq!(
        json,
        r#"{"name":"BookSummary","source":{"type":"class","value":{"package":"nz.example.library","name":"Book"}},"direction":"output","members":[{"name":"title","kind":"select","type":{"type":"primitive","value":"string"},"path":["title"]},{"name":"label","kind":"derive","type":{"type":"primitive","value":"string"},"expression":"title + \" - \" + synopsis"},{"name":"tags","kind":"declared","type":{"dto":"TagSummary"},"multiplicity":{"lower":0,"upper":"unbounded"}}]}"#
    );
    assert!(!json.contains('_'), "no snake_case keys: {json}");
    assert_eq!(
        serde_json::from_str::<DtoShape>(&json).expect("deserialize"),
        shape
    );
    assert_eq!(
        serde_json::to_string(&shape).expect("re-serialize"),
        json,
        "re-serialization is byte-identical"
    );

    // With no source class the key is absent, not null.
    let standalone =
        DtoShape::new("BookSummary", DtoDirection::Input).with_members(vec![DtoMember::new(
            "id",
            DtoMemberKind::Declared,
            dto_ref("BookId"),
        )]);
    let json = serde_json::to_string(&standalone).expect("serialize");
    assert_eq!(
        json,
        r#"{"name":"BookSummary","direction":"input","members":[{"name":"id","kind":"declared","type":{"dto":"BookId"}}]}"#
    );
    assert_eq!(
        serde_json::from_str::<DtoShape>(&json).expect("deserialize"),
        standalone
    );
}

/// A standalone DTO design has no `.mox` source class at all: `class` and
/// `typeRef` are omitted and the boundary rides in `dtoShape`. A
/// class-backed design keeps its `class` and gains the shape's resolved
/// `source` instead.
#[test]
fn a_standalone_dto_design_omits_class_and_type_ref() {
    let design = Design::standalone_dto("BookSummary");
    assert_eq!(design.class, None);
    assert_eq!(design.type_ref, None);
    assert_eq!(design.stereotype, Stereotype::Dto);
    assert_eq!(
        design.dto_shape.as_ref().map(|shape| shape.name.as_str()),
        Some("BookSummary")
    );
    assert_eq!(
        design
            .dto_shape
            .as_ref()
            .map(|shape| shape.direction)
            .expect("a standalone dto carries a shape"),
        DtoDirection::Output,
        "the constructor's documented default direction"
    );
    let json = serde_json::to_string(&design).expect("serialize");
    assert_eq!(
        json,
        r#"{"stereotype":"dto","dtoShape":{"name":"BookSummary","direction":"output"}}"#
    );
    assert!(!json.contains("class"), "no source class: {json}");
    assert!(!json.contains("typeRef"), "no resolved class: {json}");
    assert_eq!(
        serde_json::from_str::<Design>(&json).expect("deserialize"),
        design
    );

    let sourced =
        Design::new("BookSummary", Stereotype::Dto).with_dto_shape(Some(sample_dto_shape()));
    assert_eq!(sourced.class.as_deref(), Some("BookSummary"));
    let json = serde_json::to_string(&sourced).expect("serialize");
    assert!(
        json.starts_with(r#"{"class":"BookSummary","stereotype":"dto","dtoShape":{"name":"BookSummary","source":{"type":"class","value":{"package":"nz.example.library","name":"Book"}},"#),
        "a class-backed dto design keeps `class` and records its source: {json}"
    );
    assert_eq!(
        serde_json::from_str::<Design>(&json).expect("deserialize"),
        sourced
    );
}

/// DD3 changes no domain-only signature byte: a [`DddParam`] and a
/// `returnType` carrying a [`DddTypeRef::Domain`] serialize exactly as
/// the shared [`TypeRef`] slots did before the DD3 types existed.
#[test]
fn ddd_signature_slots_keep_the_domain_byte_shape() {
    let mut operation = ServiceOperation::new("borrow");
    operation.return_type = Some(DddTypeRef::domain(TypeRef::Primitive(
        PrimitiveType::Boolean,
    )));
    operation.params = vec![param("book", class_ref("Book"))];
    assert_eq!(
        serde_json::to_string(&operation).expect("serialize"),
        r#"{"name":"borrow","returnType":{"type":"primitive","value":"boolean"},"params":[{"name":"book","type":{"type":"class","value":{"package":"nz.example.library","name":"Book"}}}]}"#
    );

    let repository_operation = RepositoryOperation::declared(
        "total",
        Some(DddTypeRef::domain(TypeRef::Primitive(PrimitiveType::Int))),
        vec![param("currency", TypeRef::Primitive(PrimitiveType::String))],
    );
    assert_eq!(
        serde_json::to_string(&repository_operation).expect("serialize"),
        r#"{"name":"total","returnType":{"type":"primitive","value":"int"},"params":[{"name":"currency","type":{"type":"primitive","value":"string"}}]}"#
    );
}

/// The DTO-typed signature slots use the same `{"dto": <name>}` reference
/// on returns and parameters, and the parameter's cardinality stays
/// additive.
#[test]
fn ddd_dto_signature_slots_round_trip() {
    let mut operation = ServiceOperation::new("summarise");
    operation.return_type = Some(dto_ref("BookSummary"));
    operation.params = vec![
        DddParam::new("books", dto_ref("BookRef")).with_multiplicity(Some(Multiplicity::MANY))
    ];
    let json = serde_json::to_string(&operation).expect("serialize");
    assert_eq!(
        json,
        r#"{"name":"summarise","returnType":{"dto":"BookSummary"},"params":[{"name":"books","type":{"dto":"BookRef"},"multiplicity":{"lower":0,"upper":"unbounded"}}]}"#
    );
    assert_eq!(
        serde_json::from_str::<ServiceOperation>(&json).expect("deserialize"),
        operation
    );

    let repository_operation = RepositoryOperation::declared(
        "summary",
        Some(dto_ref("BookSummary")),
        vec![DddParam::new("book", DddTypeRef::domain(class_ref("Book")))],
    );
    assert_eq!(
        serde_json::to_string(&repository_operation).expect("serialize"),
        r#"{"name":"summary","returnType":{"dto":"BookSummary"},"params":[{"name":"book","type":{"type":"class","value":{"package":"nz.example.library","name":"Book"}}}]}"#
    );
    assert_eq!(
        serde_json::from_str::<RepositoryOperation>(
            &serde_json::to_string(&repository_operation).expect("serialize")
        )
        .expect("deserialize"),
        repository_operation
    );
}

/// The artifact root is untouched by DD3: an empty model still serializes
/// as exactly the version marker plus the provenance line.
#[test]
fn empty_model_bytes_are_unchanged_by_dd3() {
    assert_eq!(
        DddModel::new().to_json().expect("serialize"),
        empty_model_json()
    );
    assert_eq!(DDD_MODEL_FORMAT_VERSION, 3);
}

/// DF1 (issue pherolang#324): the pairing block round-trips, and a hand-built
/// model without it still deserializes (the artifact-only contract, not the
/// reader, requires the key).
#[test]
fn the_domain_fingerprint_round_trips_and_is_optional_on_read() {
    let fingerprint = DomainFingerprint::sha256(
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        Vec::new(),
    );
    let model = DddModel::new().with_domain_fingerprint(fingerprint.clone());
    let json = model.to_json().expect("serialize");
    let read = DddModel::from_json(&json).expect("deserialize");
    assert_eq!(read.domain_fingerprint, Some(fingerprint));
    assert!(read
        .domain_fingerprint
        .as_ref()
        .expect("present")
        .value_is_sha256_hex());

    assert_eq!(
        DddModel::from_json(&empty_model_json())
            .expect("a fingerprint-less artifact still reads")
            .domain_fingerprint,
        None
    );
}

#[test]
fn boost_orders_like_total_cmp() {
    let mut values = [
        Boost::new(2.5),
        Boost::new(f32::NAN),
        Boost::new(0.0),
        Boost::new(f32::NEG_INFINITY),
        Boost::new(-0.0),
        Boost::new(f32::INFINITY),
        Boost::new(-1.0),
    ];
    values.sort();
    // Bit patterns, because -0.0 == 0.0 under plain f32 equality.
    let bits: Vec<u32> = values.iter().map(|boost| boost.value().to_bits()).collect();
    assert_eq!(
        bits,
        vec![
            f32::NEG_INFINITY.to_bits(),
            (-1.0f32).to_bits(),
            (-0.0f32).to_bits(),
            0.0f32.to_bits(),
            2.5f32.to_bits(),
            f32::INFINITY.to_bits(),
            f32::NAN.to_bits(),
        ]
    );
}

// -----------------------------------------------------------------------
// Declared error vocabulary and `throws` (DD7, pherolang#304)
// -----------------------------------------------------------------------

/// An empty error declaration omits the description and fields keys; a
/// populated one serializes them in declaration order.
#[test]
fn error_declarations_serialize_sparsely_and_round_trip() {
    let empty = ErrorDecl::new("Empty");
    assert_eq!(
        serde_json::to_string(&empty).expect("serialize"),
        r#"{"name":"Empty"}"#
    );

    let declined = ErrorDecl::new("PaymentDeclined")
        .with_description(Some("The payment was refused.".to_string()))
        .with_fields(vec![
            param("reason", class_ref("DeclineReason")),
            param("reference", TypeRef::Primitive(PrimitiveType::String)),
        ]);
    let json = serde_json::to_string(&declined).expect("serialize");
    assert_eq!(
        json,
        r#"{"name":"PaymentDeclined","description":"The payment was refused.","fields":[{"name":"reason","type":{"type":"class","value":{"package":"nz.example.library","name":"DeclineReason"}}},{"name":"reference","type":{"type":"primitive","value":"string"}}]}"#
    );
    let round: ErrorDecl = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(round, declined);
}

/// A model carrying application-scoped errors emits the root `errors` key
/// after `application`; a module's errors emit after its name; both are
/// omitted entirely when empty (the additive-field discipline).
#[test]
fn error_declarations_ride_the_camel_case_wire_keys() {
    let model = DddModel::new()
        .application(Application::new("Payments"))
        .with_errors(vec![ErrorDecl::new("OutOfStock")])
        .module(Module::new("billing").with_errors(vec![ErrorDecl::new("InvoiceAlreadyIssued")]));
    let json = serde_json::to_string(&model).expect("serialize");
    assert!(
        json.contains(
            r#""application":{"name":"Payments"},"errors":[{"name":"OutOfStock"}],"modules":[{"name":"billing","errors":[{"name":"InvoiceAlreadyIssued"}]}]"#
        ),
        "{json}"
    );
    let round: DddModel = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(round, model);

    let bare = DddModel::new();
    assert_eq!(
        serde_json::to_string(&bare).expect("serialize"),
        empty_model_json()
    );
}

/// A `throws` reference serializes the scope as a bare lowercase string and
/// the declaring module only for the module scope.
#[test]
fn error_refs_serialize_their_scope() {
    assert_eq!(
        serde_json::to_string(&ErrorRef::application("PaymentDeclined")).expect("serialize"),
        r#"{"name":"PaymentDeclined","scope":"application"}"#
    );
    assert_eq!(
        serde_json::to_string(&ErrorRef::module("InvoiceAlreadyIssued", "billing"))
            .expect("serialize"),
        r#"{"name":"InvoiceAlreadyIssued","scope":"module","module":"billing"}"#
    );
    assert_eq!(
        serde_json::to_string(&ErrorRef::prelude("NotFound")).expect("serialize"),
        r#"{"name":"NotFound","scope":"prelude"}"#
    );
}

/// Service and repository operations carry their resolved `throws`
/// vocabulary additively: absent when empty, present in clause order
/// otherwise.
#[test]
fn operation_throws_are_additive_and_round_trip() {
    let service = ServiceOperation::new("pay").with_throws(vec![
        ErrorRef::application("PaymentDeclined"),
        ErrorRef::prelude("NotFound"),
    ]);
    let json = serde_json::to_string(&service).expect("serialize");
    assert_eq!(
        json,
        r#"{"name":"pay","throws":[{"name":"PaymentDeclined","scope":"application"},{"name":"NotFound","scope":"prelude"}]}"#
    );
    let round: ServiceOperation = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(round, service);

    let repository = RepositoryOperation::declared(
        "capture",
        Some(DddTypeRef::domain(TypeRef::Primitive(
            PrimitiveType::Boolean,
        ))),
        Vec::new(),
    )
    .with_throws(vec![ErrorRef::module("InvoiceAlreadyIssued", "billing")]);
    let json = serde_json::to_string(&repository).expect("serialize");
    assert!(
        json.contains(
            r#""throws":[{"name":"InvoiceAlreadyIssued","scope":"module","module":"billing"}]"#
        ),
        "{json}"
    );

    let bare = RepositoryOperation::declared("noop", None, Vec::new());
    assert!(
        !serde_json::to_string(&bare)
            .expect("serialize")
            .contains("throws"),
        "an operation with no clause omits the key"
    );
}

#[test]
fn aggregate_boundary_fields_are_sparse_and_ordered() {
    let mut design = Design::new("OrderLine", Stereotype::Entity);
    design.is_aggregate_root = Some(false);
    design.aggregate = Some(crate::ddd::AggregateMembership {
        root: class_ref("Order"),
        origin: crate::ddd::AggregateOrigin::Authored,
    });
    let json = serde_json::to_string(&design).expect("serialize");
    assert!(json.contains(r#""isAggregateRoot":false"#), "{json}");
    assert!(
        json.contains(r#""aggregate":{"root":{"type":"class""#),
        "{json}"
    );
    assert!(json.contains(r#""origin":"authored""#), "{json}");

    let mut root = Design::new("Order", Stereotype::Entity);
    root.is_aggregate_root = Some(true);
    let json = serde_json::to_string(&root).expect("serialize");
    assert!(json.contains(r#""isAggregateRoot":true"#), "{json}");
    assert!(!json.contains("aggregate"), "{json}");
}

#[test]
fn invariants_are_omitted_when_empty_and_ordered_when_present() {
    let mut design = Design::new("Order", Stereotype::Entity);
    let bare = serde_json::to_string(&design).expect("serialize");
    assert!(!bare.contains("invariants"), "{bare}");
    design.invariants = vec![
        crate::ddd::Invariant::new("total >= 0"),
        crate::ddd::Invariant::new("total <= 100"),
    ];
    let json = serde_json::to_string(&design).expect("serialize");
    assert_eq!(
        json,
        r#"{"class":"Order","stereotype":"entity","invariants":[{"expression":"total >= 0"},{"expression":"total <= 100"}]}"#
    );
}

// -----------------------------------------------------------------------
// Typed repository queries (DD9)
// -----------------------------------------------------------------------

/// The canonical query shape: one typed sort key and a fully-bounded
/// pagination window (`limit`, `max`, and an explicit `cursor`).
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

/// A declared query operation over `Book`, returning the matching
/// aggregates.
fn query_operation() -> RepositoryOperation {
    RepositoryOperation::declared(
        "findByCustomer",
        Some(DddTypeRef::domain(class_ref("Book"))),
        Vec::new(),
    )
    .with_return_multiplicity(Some(Multiplicity::MANY))
    .with_query(Some(sample_query()))
    .with_capabilities(vec![
        CapabilityRef::new("SearchBooks").with_declared_in(Some("Support".to_string()))
    ])
}

/// The query block serializes exactly as advertised, after the operation's
/// signature and before its capabilities: `predicate`, typed `sort`, the
/// `pagination` window (with `cursor` always present), and the never-skipped
/// `readOnly` key.
#[test]
fn a_query_operation_serializes_the_documented_shape() {
    let json = serde_json::to_string(&query_operation()).expect("serialize");
    assert_eq!(
        json,
        r#"{"name":"findByCustomer","returnType":{"type":"class","value":{"package":"nz.example.library","name":"Book"}},"returnMultiplicity":{"lower":0,"upper":"unbounded"},"query":{"predicate":"customer == self.customer and self.status == state","sort":[{"field":"createdDate","fieldType":{"type":"primitive","value":"date"},"direction":"desc"}],"pagination":{"limit":20,"max":100,"cursor":false},"readOnly":true},"capabilities":[{"name":"SearchBooks","declaredIn":"Support"}]}"#
    );
    assert!(!json.contains('_'), "no snake_case keys: {json}");
}

/// A non-query operation keeps the pre-DD9 bytes: neither `query` nor
/// `capabilities` rides an operation that declares none.
#[test]
fn an_operation_without_a_query_omits_the_query_and_capabilities_keys() {
    let operation = RepositoryOperation::declared(
        "findByTitle",
        Some(DddTypeRef::domain(TypeRef::Primitive(
            PrimitiveType::String,
        ))),
        vec![param("title", TypeRef::Primitive(PrimitiveType::String))],
    );
    let json = serde_json::to_string(&operation).expect("serialize");
    assert_eq!(
        json,
        r#"{"name":"findByTitle","returnType":{"type":"primitive","value":"string"},"params":[{"name":"title","type":{"type":"primitive","value":"string"}}]}"#
    );
    assert!(!json.contains("query"), "{json}");
    assert!(!json.contains("capabilities"), "{json}");
    let parsed: RepositoryOperation = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, operation, "the additive fields default to absent");
}

/// `sort` and `pagination` are themselves additive: a predicate-only query
/// serializes as exactly the predicate plus the read-only marker.
#[test]
fn a_query_without_sort_or_pagination_omits_both_keys() {
    let spec = QuerySpec::new("self.status == state");
    assert_eq!(
        serde_json::to_string(&spec).expect("serialize"),
        r#"{"predicate":"self.status == state","readOnly":true}"#
    );

    let with_sort = spec.clone().with_sort(vec![QuerySortKey::new(
        "createdDate",
        TypeRef::Primitive(PrimitiveType::Date),
        SortDirection::Asc,
    )]);
    let json = serde_json::to_string(&with_sort).expect("serialize");
    assert!(json.contains(r#""sort":[{"field":"createdDate""#), "{json}");
    assert!(!json.contains("pagination"), "{json}");
}

/// `readOnly` is never skipped: a query that is not read-only still emits
/// the key (the artifact contract refuses it), and an artifact written
/// without the key reads as `false`.
#[test]
fn a_query_always_serializes_read_only() {
    let spec = QuerySpec::new("self.status == state").with_read_only(false);
    let json = serde_json::to_string(&spec).expect("serialize");
    assert_eq!(
        json,
        r#"{"predicate":"self.status == state","readOnly":false}"#
    );

    let parsed: QuerySpec = serde_json::from_str(r#"{"predicate":"self.status == state"}"#)
        .expect("deserialize without the key");
    assert!(
        !parsed.read_only,
        "an absent readOnly key defaults to false"
    );
}

#[test]
fn sort_directions_serialize_as_bare_lowercase_strings() {
    for (direction, json) in [
        (SortDirection::Asc, r#""asc""#),
        (SortDirection::Desc, r#""desc""#),
    ] {
        assert_eq!(serde_json::to_string(&direction).expect("serialize"), json);
        assert_eq!(
            serde_json::from_str::<SortDirection>(json).expect("deserialize"),
            direction
        );
    }
}

/// The whole artifact round-trips: a model carrying a query operation
/// reloads to the same value and re-serializes byte-identically.
#[test]
fn a_query_operation_round_trips_through_the_artifact() {
    let mut design = Design::new("Book", Stereotype::Entity);
    design.repository = Some(Repository {
        name: "BookRepository".to_string(),
        operations: vec![query_operation()],
    });
    let mut module = Module::new("catalogue");
    module.designs.push(design);
    let model = DddModel::new().module(module);

    let json = model.to_json().expect("serialize");
    assert!(
        json.contains(r#""readOnly":true"#),
        "the query rides the artifact: {json}"
    );
    let parsed = DddModel::from_json(&json).expect("deserialize");
    assert_eq!(parsed, model);
    assert_eq!(parsed.to_json().expect("re-serialize"), json);
    assert_eq!(
        parsed.modules[0].designs[0]
            .repository
            .as_ref()
            .unwrap()
            .operations[0]
            .query,
        Some(sample_query()),
        "the query block survives the round trip"
    );
}

// --- machine references (issue pherolang#388) -------------------------------

/// A machine-bearing entity design with an embedded referenced machine:
/// the DSL mirror plus the compiled IR and its provenance.
fn referenced_machine_design() -> Design {
    let mut design = Design::new("PayRun", Stereotype::Entity);
    design.stereotype = Stereotype::Entity;
    design.machine = Some(StateMachine::new(
        "status",
        TypeRef::Enum {
            package: "nz.example.payroll".to_string(),
            name: "PayRunStatus".to_string(),
        },
        "Draft",
    ));
    design.machine_ir = Some(serde_json::json!({
        "id": "payrun",
        "initial": "Draft",
        "states": [{ "name": "Draft", "transitions": [] }],
    }));
    design.machine_provenance = Some(MachineProvenance::new(
        "machines/payrun.machine.ts",
        "payrun",
        "abc123",
    ));
    design
}

fn design_json(design: &Design) -> String {
    DddModel::new()
        .module(Module {
            name: "payroll".to_string(),
            errors: Vec::new(),
            services: Vec::new(),
            designs: vec![design.clone()],
            searches: Vec::new(),
        })
        .to_json()
        .expect("serialize")
}

#[test]
fn a_referenced_machine_embeds_the_ir_and_provenance() {
    let json = design_json(&referenced_machine_design());
    assert!(
        json.contains(r#""machineIr":"#),
        "the compiled IR rides the artifact: {json}"
    );
    assert!(
        json.contains(r#""machineProvenance":"#),
        "the provenance rides the artifact: {json}"
    );
    assert!(
        json.contains(r#""machineId":"payrun""#),
        "the authored id joins the provenance: {json}"
    );
    assert!(
        json.contains(r#""contentSha256":"abc123""#),
        "the provenance hash joins the provenance: {json}"
    );
    // Round trip: the embed survives deserialization.
    let parsed = DddModel::from_json(&json).expect("deserialize");
    assert_eq!(parsed, referenced_machine_model());
}

fn referenced_machine_model() -> DddModel {
    DddModel::new().module(Module {
        name: "payroll".to_string(),
        errors: Vec::new(),
        services: Vec::new(),
        designs: vec![referenced_machine_design()],
        searches: Vec::new(),
    })
}

#[test]
fn a_design_without_a_machine_reference_omits_both_keys() {
    // The byte-identical discipline: a plain inline-machine design (and a
    // machine-less design) serializes no machineIr/machineProvenance keys.
    let mut design = Design::new("AuditLog", Stereotype::Entity);
    design.stereotype = Stereotype::Entity;
    design.machine = Some(StateMachine::new(
        "state",
        TypeRef::Enum {
            package: "nz.example.payroll".to_string(),
            name: "AuditState".to_string(),
        },
        "Open",
    ));
    let json = design_json(&design);
    assert!(
        !json.contains("machineIr"),
        "an inline machine embeds nothing: {json}"
    );
    assert!(
        !json.contains("machineProvenance"),
        "an inline machine carries no provenance: {json}"
    );
    let machine_less = design_json(&Design::new("Book", Stereotype::Entity));
    assert!(!machine_less.contains("machineIr"));
    assert!(!machine_less.contains("machineProvenance"));
}

#[test]
fn machine_ir_embeds_canonically_sorted_keys() {
    // The parsed IR carries HashMap-borne structures (SetupConfig) whose
    // iteration order varies run to run: the canonical embed sorts keys
    // recursively, so the same authored file serializes byte-identically
    // on every compile.
    let insertion_a = serde_json::json!({
        "z": 1,
        "a": { "y": serde_json::json!({ "n": true, "m": 1 }), "b": 3 },
    });
    let insertion_b = serde_json::json!({
        "a": { "b": 3, "y": serde_json::json!({ "m": 1, "n": true }) },
        "z": 1,
    });
    let canonical_a = crate::canonical_json(&insertion_a);
    let canonical_b = crate::canonical_json(&insertion_b);
    assert_eq!(canonical_a, canonical_b);
    assert_eq!(
        canonical_a, r#"{"a":{"b":3,"y":{"m":1,"n":true}},"z":1}"#,
        "keys sort recursively, containers included"
    );
}
