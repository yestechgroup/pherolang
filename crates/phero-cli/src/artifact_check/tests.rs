use super::legacy::{LEGACY_INSTANCE_TYPE, LEGACY_MESSAGE, LEGACY_PROVENANCE_KEY};
use super::*;
use serde_json::{json, Value};

/// One row of the classification table: a document and the outcome
/// `check` must produce. `Err` rows carry a fragment every error line
/// must contain (the kind whose check claimed the document).
struct ClassificationCase {
    name: &'static str,
    json: String,
    expected: Result<ArtifactKind, &'static str>,
}

#[test]
fn classification_table_over_all_artifact_kinds() {
    // Round-trip rows serialize the real struct kinds (phero_ir types),
    // so the table pins the classification of actual emitter output,
    // not hand-written approximations.
    let block_less_actor = phero_ir::ActorModel::new();
    let block_less_actor_json =
        serde_json::to_string(&block_less_actor).expect("serialize ActorModel");
    let empty_ifml = phero_ir::ifml::IfmlModel::default();
    let empty_ifml_json = serde_json::to_string(&empty_ifml).expect("serialize IfmlModel");
    let empty_event = phero_ir::events::EventModel::new();
    let empty_event_json = serde_json::to_string(&empty_event).expect("serialize EventModel");
    let empty_usecase = phero_ir::usecases::UseCaseModel::new();
    let empty_usecase_json = serde_json::to_string(&empty_usecase).expect("serialize UseCaseModel");
    let empty_dmn = phero_ir::dmn::DmnModel::new();
    let empty_dmn_json = serde_json::to_string(&empty_dmn).expect("serialize DmnModel");

    let cases = vec![
        ClassificationCase {
            name: "domain IR artifact (serialized phero_ir::Model)",
            json: serde_json::to_string(&phero_ir::Model::new()).expect("serialize Model"),
            expected: Ok(ArtifactKind::IrModel),
        },
        ClassificationCase {
            name: "actor-policy artifact with a block (serialized ActorModel)",
            json: serde_json::to_string(
                &phero_ir::ActorModel::new().block(phero_ir::ActorsDef::new("Support")),
            )
            .expect("serialize ActorModel"),
            expected: Ok(ArtifactKind::ActorModel),
        },
        ClassificationCase {
            name: "minimal-empty artifact is the block-less ActorModel",
            json: block_less_actor_json.clone(),
            expected: Ok(ArtifactKind::ActorModel),
        },
        ClassificationCase {
            name: "DDD design artifact (serialized DddModel)",
            json: serde_json::to_string(
                &phero_ir::ddd::DddModel::new()
                    .with_domain_fingerprint(phero_ir::ddd::DomainFingerprint::sha256(
                        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                        Vec::new(),
                    ))
                    .application(phero_ir::ddd::Application::new("Library"))
                    .module(phero_ir::ddd::Module::new("core")),
            )
            .expect("serialize DddModel"),
            expected: Ok(ArtifactKind::DddModel),
        },
        ClassificationCase {
            name: "event-contract artifact (serialized EventModel)",
            json: serde_json::to_string(
                &phero_ir::events::EventModel::new()
                    .event(phero_ir::events::EventDef::new("Ping"))
                    .channel(phero_ir::events::ChannelDef::new("pings").publishes("Ping"))
                    .subscription(
                        phero_ir::events::SubscriptionDef::new("watch")
                            .event("Ping")
                            .consumer("watcher"),
                    ),
            )
            .expect("serialize EventModel"),
            expected: Ok(ArtifactKind::EventModel),
        },
        ClassificationCase {
            name: "deployment artifact (serialized DeployModel)",
            json: serde_json::to_string(
                &phero_ir::deploy::DeployModel::new()
                    .application(phero_ir::deploy::DeployApplication::new("Svc").component(
                        phero_ir::deploy::DeployComponent::new(
                            "db",
                            phero_ir::deploy::ComponentKind::Database,
                        ),
                    ))
                    .profile(phero_ir::deploy::DeployProfile::new(
                        "edge",
                        phero_ir::deploy::DeployTarget::CloudflareWorkers,
                    ))
                    .deployment(phero_ir::deploy::DeployDeployment::new(
                        "prod", "Svc", "edge",
                    )),
            )
            .expect("serialize DeployModel"),
            expected: Ok(ArtifactKind::DeployModel),
        },
        ClassificationCase {
            name: "use-case artifact (serialized UseCaseModel)",
            json: serde_json::to_string(&{
                let mut model = phero_ir::usecases::UseCaseModel::new();
                model.use_cases.push(phero_ir::usecases::UseCaseDef {
                    name: "HandOver".to_string(),
                    description: None,
                    subject: None,
                    actors: vec![phero_ir::usecases::Participant {
                        name: "Courier".to_string(),
                        type_name: None,
                        occurrences: Vec::new(),
                        steps: Vec::new(),
                    }],
                    messages: Vec::new(),
                    successions: Vec::new(),
                    realizes: Vec::new(),
                });
                model
            })
            .expect("serialize UseCaseModel"),
            expected: Ok(ArtifactKind::UseCaseModel),
        },
        // Misclassification guard: a malformed use-case-shaped document
        // must be reported by the UseCaseModel check, never fall through
        // to the actor fallback.
        ClassificationCase {
            name: "malformed use-case shape is reported as a UseCaseModel, not an ActorModel",
            json: r#"{"formatVersion": 2, "useCases": [{"oops": true}]}"#.to_string(),
            expected: Err("useCases[0]"),
        },
        // Misclassification guard: a malformed deployment-shaped
        // document must be reported by the DeployModel check, never
        // fall through to the actor fallback.
        ClassificationCase {
            name: "malformed deployment shape is reported as a DeployModel, not an ActorModel",
            json: r#"{"formatVersion": 2, "applications": [{"oops": true}]}"#.to_string(),
            expected: Err("applications[0]"),
        },
        // Misclassification guard: a malformed event-shaped document
        // must be reported by the EventModel check, never fall through
        // to the actor fallback (whose "missing formatVersion"-style
        // errors would misattribute the artifact kind).
        ClassificationCase {
            name: "malformed event shape is reported as an EventModel, not an ActorModel",
            json: r#"{"formatVersion": 2, "events": [{"oops": true}]}"#.to_string(),
            expected: Err("events[0]"),
        },
        ClassificationCase {
            name: "decision-model artifact (serialized DmnModel)",
            json: serde_json::to_string(&{
                let mut model = phero_ir::dmn::DmnModel::new();
                model.decisions.push(phero_ir::dmn::Decision {
                    name: "Assess".to_string(),
                    description: None,
                    outputs: Vec::new(),
                    logic: Some(phero_ir::dmn::DecisionLogic::Expression {
                        text: "1".to_string(),
                    }),
                    emits: Vec::new(),
                });
                model
            })
            .expect("serialize DmnModel"),
            expected: Ok(ArtifactKind::DmnModel),
        },
        // Precedence case: a decision model's root commonly carries a
        // `views` array too — the IFML rule's key — but the unique
        // `decisions` key claims the document first.
        ClassificationCase {
            name: "a decisions-and-views DmnModel classifies as a DmnModel, \
                       never the IFML shape",
            json: serde_json::to_string(&{
                let mut model = phero_ir::dmn::DmnModel::new();
                model.decisions.push(phero_ir::dmn::Decision {
                    name: "Assess".to_string(),
                    description: None,
                    outputs: Vec::new(),
                    logic: None,
                    emits: Vec::new(),
                });
                model.views.push(phero_ir::dmn::DecisionView {
                    name: "overview".to_string(),
                    description: None,
                    show: vec!["Assess".to_string()],
                    direction: None,
                });
                model
            })
            .expect("serialize DmnModel"),
            expected: Ok(ArtifactKind::DmnModel),
        },
        // Misclassification guard: a malformed decision-shaped document
        // must be reported by the DmnModel check, never fall through
        // to the actor fallback.
        ClassificationCase {
            name: "malformed decision shape is reported as a DmnModel, not an ActorModel",
            json: r#"{"formatVersion": 2, "decisions": [{"oops": true}]}"#.to_string(),
            expected: Err("decisions[0]"),
        },
        ClassificationCase {
            name: "unknown root shape is unrecognized",
            json: r#"{"formatVersion": 2, "nobodyKnowsMe": true}"#.to_string(),
            expected: Err("unrecognized artifact"),
        },
        // Empty-form limitation, shared with the empty IfmlModel: an
        // empty EventModel serializes as exactly `{"formatVersion":2}`,
        // so shape-sniffing classifies it as the block-less ActorModel.
        // Pinned here so any future $type discriminator must update the
        // three kinds consciously.
        ClassificationCase {
            name: "empty event artifact serializes as the same minimal-empty bytes",
            json: {
                assert_eq!(
                    empty_event_json, block_less_actor_json,
                    "an empty EventModel and an empty ActorModel are \
                         byte-identical on the wire"
                );
                empty_event_json.clone()
            },
            expected: Ok(ArtifactKind::ActorModel),
        },
        // Empty-form limitation, shared with the empty EventModel and
        // IfmlModel: an empty DeployModel serializes as exactly
        // `{"formatVersion":2}`, so shape-sniffing classifies it as the
        // block-less ActorModel. Pinned here so any future $type
        // discriminator must update the four kinds consciously.
        ClassificationCase {
            name: "empty deployment artifact serializes as the same minimal-empty bytes",
            json: {
                let empty_deploy_json =
                    serde_json::to_string(&phero_ir::deploy::DeployModel::new())
                        .expect("serialize DeployModel");
                assert_eq!(
                    empty_deploy_json, block_less_actor_json,
                    "an empty DeployModel and an empty ActorModel are \
                         byte-identical on the wire"
                );
                empty_deploy_json
            },
            expected: Ok(ArtifactKind::ActorModel),
        },
        // Empty-form limitation, shared with the empty EventModel,
        // DeployModel, and IfmlModel: an empty UseCaseModel serializes
        // as exactly `{"formatVersion":2}`, so shape-sniffing classifies
        // it as the block-less ActorModel. Pinned here so any future
        // $type discriminator must update the five kinds consciously.
        ClassificationCase {
            name: "empty use-case artifact serializes as the same minimal-empty bytes",
            json: {
                assert_eq!(
                    empty_usecase_json, block_less_actor_json,
                    "an empty UseCaseModel and an empty ActorModel are \
                         byte-identical on the wire"
                );
                empty_usecase_json
            },
            expected: Ok(ArtifactKind::ActorModel),
        },
        // Empty-form limitation, shared with the empty EventModel,
        // DeployModel, UseCaseModel, and IfmlModel: an empty DmnModel
        // serializes as exactly `{"formatVersion":2}`, so shape-sniffing
        // classifies it as the block-less ActorModel. Pinned here so any
        // future $type discriminator must update the six kinds
        // consciously.
        ClassificationCase {
            name: "empty decision artifact serializes as the same minimal-empty bytes",
            json: {
                assert_eq!(
                    empty_dmn_json, block_less_actor_json,
                    "an empty DmnModel and an empty ActorModel are \
                         byte-identical on the wire"
                );
                empty_dmn_json
            },
            expected: Ok(ArtifactKind::ActorModel),
        },
        // IfmlModel is a classified kind; the two unfixable shape
        // collisions are pinned here so any future $type discriminator
        // must update them consciously.
        ClassificationCase {
            name: "empty IFML artifact serializes as the same minimal-empty bytes",
            json: {
                assert_eq!(
                    empty_ifml_json, block_less_actor_json,
                    "an empty IfmlModel and an empty ActorModel are \
                         byte-identical on the wire"
                );
                empty_ifml_json.clone()
            },
            expected: Ok(ArtifactKind::ActorModel),
        },
        ClassificationCase {
            name: "module-only IFML artifact shape-matches the DDD design keys, \
                       is claimed by the DDD check, and fails its contract \
                       (the documented limitation)",
            json: serde_json::to_string(&phero_ir::ifml::IfmlModel {
                modules: vec![phero_ir::ifml::ModuleDeclaration {
                    name: "Pagination".to_string(),
                    input_params: Vec::new(),
                    output_params: Vec::new(),
                    properties: Vec::new(),
                    containers: Vec::new(),
                    components: Vec::new(),
                    events: Vec::new(),
                    module_uses: Vec::new(),
                }],
                ..phero_ir::ifml::IfmlModel::default()
            })
            .expect("serialize IfmlModel"),
            expected: Err("pheroVersion"),
        },
        ClassificationCase {
            name: "view-bearing IFML artifact classifies as an IfmlModel",
            json: serde_json::to_string(&phero_ir::ifml::IfmlModel {
                views: vec![phero_ir::ifml::ViewDeclaration {
                    name: "Home".to_string(),
                    label: None,
                    is_landmark: false,
                    is_xor: false,
                    is_modal: false,
                    params: Vec::new(),
                    properties: Vec::new(),
                    containers: Vec::new(),
                    components: Vec::new(),
                    events: Vec::new(),
                    module_uses: Vec::new(),
                    roles: Vec::new(),
                    requires: Vec::new(),
                    condition: None,
                    position: None,
                }],
                ..phero_ir::ifml::IfmlModel::default()
            })
            .expect("serialize IfmlModel"),
            expected: Ok(ArtifactKind::IfmlModel),
        },
        ClassificationCase {
            name: "a view-and-module IFML artifact classifies as an IfmlModel, \
                       never the DDD shape",
            json: serde_json::to_string(&phero_ir::ifml::IfmlModel {
                views: vec![phero_ir::ifml::ViewDeclaration {
                    name: "Home".to_string(),
                    label: None,
                    is_landmark: false,
                    is_xor: false,
                    is_modal: false,
                    params: Vec::new(),
                    properties: Vec::new(),
                    containers: Vec::new(),
                    components: Vec::new(),
                    events: Vec::new(),
                    module_uses: Vec::new(),
                    roles: Vec::new(),
                    requires: Vec::new(),
                    condition: None,
                    position: None,
                }],
                modules: vec![phero_ir::ifml::ModuleDeclaration {
                    name: "Pagination".to_string(),
                    input_params: Vec::new(),
                    output_params: Vec::new(),
                    properties: Vec::new(),
                    containers: Vec::new(),
                    components: Vec::new(),
                    events: Vec::new(),
                    module_uses: Vec::new(),
                }],
                ..phero_ir::ifml::IfmlModel::default()
            })
            .expect("serialize IfmlModel"),
            expected: Ok(ArtifactKind::IfmlModel),
        },
        // Misclassification guard: a malformed IFML-shaped document is
        // reported by the IfmlModel check, never the actor fallback.
        ClassificationCase {
            name: "malformed IFML shape is reported as an IfmlModel, not an ActorModel",
            json: r#"{"formatVersion": 2, "views": [{"oops": true}]}"#.to_string(),
            expected: Err("views[0]"),
        },
        ClassificationCase {
            name: "semantic graph artifact (serialized GraphModel)",
            json: serde_json::to_string(&{
                let mut model = phero_ir::graph::GraphModel::new();
                model.nodes.push(phero_ir::graph::GraphNode::new(
                    "class:support.mox#nz.example.support.Ticket",
                    phero_ir::graph::kinds::CLASS,
                    "nz.example.support.Ticket",
                    "support.mox",
                    "support.mox",
                ));
                model
            })
            .expect("serialize GraphModel"),
            expected: Ok(ArtifactKind::GraphModel),
        },
        // Precedence case: a graph artifact also carries an
        // `applications` array — the deployment rule's key — but the
        // graph's unique `nodes` array claims the document first. This row
        // is the ordering guard: swap the two rules and it fails.
        ClassificationCase {
            name: "a nodes-and-applications GraphModel classifies as a GraphModel, \
                       never the deployment shape",
            json: serde_json::to_string(&{
                let mut model = phero_ir::graph::GraphModel::new();
                model.applications.push(phero_ir::graph::GraphApplication {
                    id: "support.mox".to_string(),
                    files: vec!["support.mox".to_string()],
                    packages: Vec::new(),
                });
                model.nodes.push(phero_ir::graph::GraphNode::new(
                    "class:support.mox#nz.example.support.Ticket",
                    phero_ir::graph::kinds::CLASS,
                    "nz.example.support.Ticket",
                    "support.mox",
                    "support.mox",
                ));
                model
            })
            .expect("serialize GraphModel"),
            expected: Ok(ArtifactKind::GraphModel),
        },
        // Misclassification guard: a malformed graph-shaped document is
        // reported by the GraphModel check, never the deployment or actor
        // fallback.
        ClassificationCase {
            name: "malformed graph shape is reported as a GraphModel, not a DeployModel",
            json: r#"{"formatVersion": 2, "applications": [], "nodes": [{"oops": true}]}"#
                .to_string(),
            expected: Err("nodes[0]"),
        },
        // The graph marker deliberately stays at 1 while every other kind
        // moved to 2, so the empty graph is **no longer** byte-identical to
        // the empty ActorModel: it serializes as `{"formatVersion":1}` and
        // reaches the minimal-empty actor fallback, where the actor's own
        // (now 2) version gate rejects it. A non-empty graph is unambiguous
        // (`nodes`, rule 7), and a legacy marker on any graph still trips
        // rule 0 — so this is a version mismatch, never a legacy artifact.
        ClassificationCase {
            name: "empty graph artifact is a version mismatch at the actor fallback",
            json: {
                let empty_graph_json = serde_json::to_string(&phero_ir::graph::GraphModel::new())
                    .expect("serialize GraphModel");
                assert_ne!(
                    empty_graph_json, block_less_actor_json,
                    "the graph marker stayed at 1 while the actor marker moved to 2"
                );
                assert_eq!(empty_graph_json, r#"{"formatVersion":1}"#);
                empty_graph_json
            },
            expected: Err("actor artifact formatVersion 1"),
        },
    ];

    for case in &cases {
        match (&case.expected, check(&case.json)) {
            (Ok(kind), Ok(found)) => assert_eq!(
                found, *kind,
                "{}: classified as {found:?}, expected {kind:?}",
                case.name
            ),
            (Err(fragment), Err(errors)) => assert!(
                errors.iter().any(|error| error.contains(fragment)),
                "{}: errors {errors:?} must mention {fragment:?}",
                case.name
            ),
            (expected, found) => {
                panic!("{}: expected {expected:?}, found {found:?}", case.name)
            }
        }
    }
}

#[test]
fn bare_format_version_classifies_as_block_less_actor_model() {
    // A Model always emits `packages`; the only artifact whose canonical
    // form is just the version marker is the empty ActorModel.
    assert_eq!(
        check(r#"{"formatVersion": 2}"#),
        Ok(ArtifactKind::ActorModel)
    );
}

#[test]
fn packages_key_classifies_as_ir_model() {
    assert_eq!(
        check(r#"{"formatVersion": 2, "pheroVersion": "0.1.0", "packages": []}"#),
        Ok(ArtifactKind::IrModel)
    );
}

#[test]
fn format_version_gate_names_both_versions() {
    let errors = check(r#"{"formatVersion": 3, "packages": []}"#).unwrap_err();
    assert!(
        errors[0].contains("formatVersion 3") && errors[0].contains("formatVersion 2"),
        "errors: {errors:?}"
    );
}

#[test]
fn snake_case_keys_are_reported_with_the_camel_case_spelling() {
    let errors = check(r#"{"formatVersion": 2, "format_version": 1, "packages": []}"#).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.contains("format_version") && error.contains("formatVersion")),
        "errors: {errors:?}"
    );
}

#[test]
fn instance_ids_must_be_well_formed_unique_and_resolved() {
    let errors = check(
        r#"{
                "$type": "phero.instance", "formatVersion": 2, "model": "m",
                "objects": [
                    {"$id": "book/0", "$type": "Book", "other": {"$ref": "writer/0"}}
                ]
            }"#,
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.contains("unresolved $ref \"writer/0\"")),
        "errors: {errors:?}"
    );

    let errors = check(
        r#"{
                "$type": "phero.instance", "formatVersion": 2, "model": "m",
                "objects": [
                    {"$id": "book/0", "$type": "Book"},
                    {"$id": "book/0", "$type": "Book"}
                ]
            }"#,
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.contains("duplicate $id \"book/0\"")),
        "errors: {errors:?}"
    );

    let errors = check(
        r#"{
                "$type": "phero.instance", "formatVersion": 2, "model": "m",
                "objects": [{"$id": "book", "$type": "Book"}]
            }"#,
    )
    .unwrap_err();
    assert!(
        errors.iter().any(|error| error.contains("malformed")),
        "errors: {errors:?}"
    );
}

#[test]
fn unknown_root_type_is_rejected() {
    let errors = check(r#"{"$type": "something.else", "formatVersion": 2}"#).unwrap_err();
    assert!(
        errors[0].contains("something.else") && errors[0].contains("phero.instance"),
        "errors: {errors:?}"
    );
}

#[test]
fn application_or_modules_keys_classify_as_ddd_model() {
    let fingerprint = r#""domainFingerprint": {"algorithm": "sha256", "value": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", "packages": []}"#;
    assert_eq!(
        check(&format!(
            r#"{{"formatVersion": 2, "pheroVersion": "0.3.0", {fingerprint}, "application": {{"name": "Library"}}, "modules": []}}"#
        )),
        Ok(ArtifactKind::DddModel)
    );
    assert_eq!(
        check(&format!(
            r#"{{"formatVersion": 2, "pheroVersion": "0.3.0", {fingerprint}, "application": {{"name": "Library"}}}}"#
        )),
        Ok(ArtifactKind::DddModel)
    );
    assert_eq!(
        check(&format!(
            r#"{{"formatVersion": 2, "pheroVersion": "0.3.0", {fingerprint}, "application": {{"name": "Library"}}, "modules": [{{"name": "core"}}]}}"#
        )),
        Ok(ArtifactKind::DddModel)
    );
}

#[test]
fn ddd_provenance_is_not_an_ir_classifier() {
    // `pheroVersion` appears on the DDD design root too, so only `packages`
    // may claim the IR reading. Before the rule change this document was
    // misclassified as an IrModel and failed on the missing `packages`.
    let golden = include_str!("../../../../tests/conformance/ddd/library.ddd.json");
    assert_eq!(check(golden), Ok(ArtifactKind::DddModel));

    // Provenance alone is not an artifact: no known root shape claims it.
    let errors = check(r#"{"formatVersion": 2, "pheroVersion": "0.3.0"}"#).unwrap_err();
    assert!(
        errors.iter().any(|error| error.contains("unrecognized")),
        "{errors:?}"
    );
}

#[test]
fn the_ddd_contract_is_enforced_by_artifact_check() {
    let golden = include_str!("../../../../tests/conformance/ddd/library.ddd.json");

    let mut missing_provenance: Value = serde_json::from_str(golden).expect("golden JSON");
    missing_provenance
        .as_object_mut()
        .expect("artifact root")
        .remove("pheroVersion");
    let errors = check(&missing_provenance.to_string()).unwrap_err();
    assert!(
        errors.iter().any(|error| error.contains("pheroVersion")),
        "{errors:?}"
    );

    let mut missing_type_ref: Value = serde_json::from_str(golden).expect("golden JSON");
    missing_type_ref["modules"][0]["designs"][0]
        .as_object_mut()
        .expect("first design")
        .remove("typeRef");
    let errors = check(&missing_type_ref.to_string()).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.contains("designs[0].typeRef")),
        "{errors:?}"
    );

    let mut dto_with_repository: Value = serde_json::from_str(golden).expect("golden JSON");
    dto_with_repository["modules"][0]["designs"][0]["stereotype"] =
        Value::String("dto".to_string());
    let errors = check(&dto_with_repository.to_string()).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.contains("only an entity design may declare a repository")),
        "{errors:?}"
    );
}

#[test]
fn a_ddd_artifact_does_not_fall_through_to_the_actor_fallback() {
    // Regression for the fallback order: before the DddModel check
    // existed, the actor fallback claimed any versioned document
    // without `packages`/`$type` — a malformed DDD design was reported
    // as a (block-less) ActorModel missing its formatVersion.
    let errors =
        check(r#"{"formatVersion": 2, "application": {"name": "X"}, "modules": [{"oops": true}]}"#)
            .unwrap_err();
    assert!(
        errors.iter().any(|error| error.contains("modules[0]")),
        "must validate as a DddModel, not an ActorModel: {errors:?}"
    );
}

#[test]
fn the_dddd_golden_artifact_classifies_as_ddd_model() {
    // The real emitter output (`pherolang ir` on the canonical fixture):
    // it carries no `packages`/`blocks`/`$type`, so only the DddModel
    // check may claim it.
    let golden = include_str!("../../../../tests/conformance/ddd/library.ddd.json");
    assert_eq!(check(golden), Ok(ArtifactKind::DddModel));
}

#[test]
fn the_ifml_golden_artifact_classifies_as_ifml_model() {
    // The real emitter output (`pherolang ir` on the canonical fixture):
    // view-bearing, so the IFML rule claims it before the DDD shape
    // (the fixture carries `modules` too — the precedence case).
    let golden = include_str!("../../../../tests/conformance/ifml/app.ifml.json");
    assert_eq!(check(golden), Ok(ArtifactKind::IfmlModel));
}

#[test]
fn an_ifml_artifact_does_not_fall_through_to_the_actor_fallback() {
    // Regression for the fallback order: before the IfmlModel check
    // existed, only a view-bearing artifact was rejected outright and
    // a module-bearing one was claimed by the DDD shape. Every
    // IFML-exclusive root key claims the document.
    assert_eq!(
        check(r#"{"formatVersion": 2, "views": [{"name": "Home"}]}"#),
        Ok(ArtifactKind::IfmlModel)
    );
    assert_eq!(
        check(r#"{"formatVersion": 2, "domains": [{"name": "sales"}]}"#),
        Ok(ArtifactKind::IfmlModel)
    );
    assert_eq!(
        check(r#"{"formatVersion": 2, "actions": [{"name": "Update"}]}"#),
        Ok(ArtifactKind::IfmlModel)
    );
    assert_eq!(
        check(r#"{"formatVersion": 2, "commands": [{"name": "deploy"}]}"#),
        Ok(ArtifactKind::IfmlModel)
    );
    assert_eq!(
        check(r#"{"formatVersion": 2, "imports": ["./patterns.ifml"]}"#),
        Ok(ArtifactKind::IfmlModel)
    );
}

#[test]
fn ifml_format_version_gate_names_both_versions() {
    let errors = check(r#"{"formatVersion": 3, "views": [{"name": "Home"}]}"#).unwrap_err();
    assert!(
        errors[0].contains("formatVersion 3") && errors[0].contains("formatVersion 2"),
        "errors: {errors:?}"
    );
}

#[test]
fn ddd_format_version_gate_names_both_versions() {
    let errors = check(r#"{"formatVersion": 3, "application": {"name": "X"}}"#).unwrap_err();
    assert!(
        errors[0].contains("formatVersion 3") && errors[0].contains("formatVersion 2"),
        "errors: {errors:?}"
    );
}

#[test]
fn an_event_artifact_does_not_fall_through_to_the_actor_fallback() {
    // Regression for the fallback order: before the EventModel check
    // existed, the actor fallback claimed any versioned document
    // without `packages`/`application`/`modules`/`blocks`.
    assert_eq!(
        check(r#"{"formatVersion": 2, "events": [{"name": "Ping"}]}"#),
        Ok(ArtifactKind::EventModel)
    );
    assert_eq!(
        check(r#"{"formatVersion": 2, "channels": [{"name": "pings"}]}"#),
        Ok(ArtifactKind::EventModel)
    );
    assert_eq!(
        check(r#"{"formatVersion": 2, "subscriptions": [{"name": "w", "consumer": "c"}]}"#,),
        Ok(ArtifactKind::EventModel)
    );
}

#[test]
fn event_format_version_gate_names_both_versions() {
    let errors = check(r#"{"formatVersion": 3, "events": []}"#).unwrap_err();
    assert!(
        errors[0].contains("formatVersion 3") && errors[0].contains("formatVersion 2"),
        "errors: {errors:?}"
    );
}

#[test]
fn event_arrays_must_be_arrays_of_named_objects() {
    let errors =
        check(r#"{"formatVersion": 2, "events": [42], "channels": [{}], "subscriptions": [7]}"#)
            .unwrap_err();
    assert!(
        errors.iter().any(|error| error.contains("events[0]")),
        "errors: {errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("channels[0] is missing \"name\"")),
        "errors: {errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("subscriptions[0]")),
        "errors: {errors:?}"
    );
}

#[test]
fn snake_case_event_keys_are_reported_with_the_camel_case_spelling() {
    let errors =
        check(r#"{"formatVersion": 2, "events": [{"name": "Ping", "format_version": 1}]}"#)
            .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.contains("format_version") && error.contains("formatVersion")),
        "errors: {errors:?}"
    );
}

#[test]
fn ddd_application_must_be_a_named_object_and_modules_named_objects() {
    let errors =
        check(r#"{"formatVersion": 2, "application": "Library", "modules": [42]}"#).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.contains("\"application\" must be an object")),
        "errors: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| error.contains("modules[0]")),
        "errors: {errors:?}"
    );

    let errors = check(r#"{"formatVersion": 2, "application": {"name": ""}}"#).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.contains("non-empty string")),
        "errors: {errors:?}"
    );
}

#[test]
fn unknown_root_shape_is_reported_as_unrecognized() {
    // The tightened fallback: a truly unknown v1 artifact is no longer
    // misclassified as a block-less ActorModel.
    let errors = check(r#"{"formatVersion": 2, "nobodyKnowsMe": true}"#).unwrap_err();
    assert!(
        errors[0].contains("unrecognized artifact"),
        "errors: {errors:?}"
    );
}

#[test]
fn a_usecase_artifact_does_not_fall_through_to_the_actor_fallback() {
    // Regression for the fallback order: before the UseCaseModel check
    // existed, the actor fallback claimed any versioned document
    // without `packages`/`application`/`modules`/`events`/`blocks`.
    assert_eq!(
        check(r#"{"formatVersion": 2, "useCases": [{"name": "HandOver"}]}"#),
        Ok(ArtifactKind::UseCaseModel)
    );
}

#[test]
fn usecase_format_version_gate_names_both_versions() {
    let errors = check(r#"{"formatVersion": 3, "useCases": []}"#).unwrap_err();
    assert!(
        errors[0].contains("formatVersion 3") && errors[0].contains("formatVersion 2"),
        "errors: {errors:?}"
    );
}

#[test]
fn usecase_arrays_must_be_arrays_of_named_objects() {
    let errors = check(r#"{"formatVersion": 2, "useCases": [42]}"#).unwrap_err();
    assert!(
        errors.iter().any(|error| error.contains("useCases[0]")),
        "errors: {errors:?}"
    );
}

#[test]
fn snake_case_usecase_keys_are_reported_with_the_camel_case_spelling() {
    let errors = check(
        r#"{"formatVersion": 2, "useCases": [{"name": "HandOver", "type_name": "demo.Drone"}]}"#,
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.contains("type_name") && error.contains("typeName")),
        "errors: {errors:?}"
    );
}

#[test]
fn the_usecase_golden_artifact_classifies_as_usecase_model() {
    // The real emitter output (`pherolang ir` on the canonical fixture):
    // it carries no `packages`/`blocks`/`$type`, so only the UseCaseModel
    // check may claim it.
    let golden = include_str!("../../../../tests/conformance/usecases/delivery.uc.json");
    assert_eq!(check(golden), Ok(ArtifactKind::UseCaseModel));
}

#[test]
fn a_cmmn_artifact_does_not_fall_through_to_the_actor_fallback() {
    assert_eq!(
        check(r#"{"formatVersion": 2, "cases": [{"name": "Loan"}]}"#),
        Ok(ArtifactKind::CmmnModel)
    );
}

#[test]
fn cmmn_format_version_gate_names_both_versions() {
    let errors = check(r#"{"formatVersion": 3, "cases": []}"#).unwrap_err();
    assert!(
        errors[0].contains("formatVersion 3") && errors[0].contains("formatVersion 2"),
        "errors: {errors:?}"
    );
}

#[test]
fn cmmn_arrays_must_be_arrays_of_named_objects() {
    let errors = check(r#"{"formatVersion": 2, "cases": [42]}"#).unwrap_err();
    assert!(
        errors.iter().any(|error| error.contains("cases[0]")),
        "errors: {errors:?}"
    );
}

#[test]
fn snake_case_cmmn_keys_are_reported_with_the_camel_case_spelling() {
    let errors =
        check(r#"{"formatVersion": 2, "cases": [{"name": "Loan", "case_file": []}]}"#).unwrap_err();
    assert!(
        errors.iter().any(|error| error.contains("caseFile")),
        "errors: {errors:?}"
    );
}

#[test]
fn the_cmmn_golden_artifact_classifies_as_cmmn_model() {
    // The real emitter output (`pherolang ir` on the canonical fixture).
    let golden = include_str!("../../../../tests/conformance/cmmn/lending.cmmn.json");
    assert_eq!(check(golden), Ok(ArtifactKind::CmmnModel));
}

#[test]
fn the_cmmn_engine_golden_does_not_classify_as_a_pherolang_artifact() {
    // The cmmn-rs CaseDefinition JSON is a DIFFERENT wire format
    // (snake_case, no formatVersion): the checker must reject it rather
    // than guess.
    let golden = include_str!("../../../../tests/conformance/cmmn/lending.case.json");
    assert!(check(golden).is_err());
}

#[test]
fn a_dmn_artifact_does_not_fall_through_to_the_actor_fallback() {
    // Regression for the fallback order: before the DmnModel check
    // existed, the actor fallback claimed any versioned document
    // without `packages`/`application`/`modules`/`events`/`useCases`/
    // `cases`/`blocks`.
    assert_eq!(
        check(r#"{"formatVersion": 2, "decisions": [{"name": "Assess"}]}"#),
        Ok(ArtifactKind::DmnModel)
    );
}

#[test]
fn dmn_format_version_gate_names_both_versions() {
    let errors = check(r#"{"formatVersion": 3, "decisions": []}"#).unwrap_err();
    assert!(
        errors[0].contains("formatVersion 3") && errors[0].contains("formatVersion 2"),
        "errors: {errors:?}"
    );
}

#[test]
fn dmn_arrays_must_be_arrays_of_named_objects() {
    let errors = check(r#"{"formatVersion": 2, "decisions": [42]}"#).unwrap_err();
    assert!(
        errors.iter().any(|error| error.contains("decisions[0]")),
        "errors: {errors:?}"
    );
}

#[test]
fn snake_case_dmn_keys_are_reported_with_the_camel_case_spelling() {
    let errors =
        check(r#"{"formatVersion": 2, "decisions": [{"name": "Assess", "format_version": 1}]}"#)
            .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.contains("format_version") && error.contains("formatVersion")),
        "errors: {errors:?}"
    );
}

#[test]
fn the_graph_golden_artifact_classifies_as_graph_model() {
    // The real emitter output (`pherolang graph build examples/`): it carries
    // `applications`, `nodes`, and `edges` — the deploy rule's
    // `applications` lookalike included — so only the graph rule (which
    // runs first) may claim it.
    let golden = include_str!("../../../../examples/artifacts/examples.graph.json");
    assert_eq!(check(golden), Ok(ArtifactKind::GraphModel));
}

#[test]
fn the_dmn_golden_artifact_classifies_as_dmn_model() {
    // The real emitter output (`pherolang ir` on the canonical fixture):
    // it carries no `packages`/`blocks`/`$type`, so only the DmnModel
    // check may claim it.
    let golden = include_str!("../../../../tests/conformance/dmn/kiwisaver.dmn.json");
    assert_eq!(check(golden), Ok(ArtifactKind::DmnModel));
}

/// How an artifact kind's *actual* 0.2-era form behaves under `check`.
///
/// Only the domain IR artifact carried the old provenance key and only
/// canonical instances carried the old `$type` discriminator, so only those
/// two can match the named legacy message. Every other root never serialized
/// a marker: its 0.2 form differs from the current one solely by
/// `formatVersion`, so the version gate reports the mismatch. The semantic
/// graph is the third case: its marker deliberately stayed at 1, so its
/// current bytes *are* its 0.2-era bytes.
enum Downgrade {
    /// The 0.2 form carries the old provenance key: the legacy message.
    LegacyProvenance,
    /// The 0.2 form carries the old instance discriminator: the legacy message.
    LegacyDiscriminator,
    /// The marker moved 1 → 2 and carried no legacy marker: a plain version
    /// mismatch, never the legacy message.
    VersionMismatch,
    /// The marker deliberately stayed at 1: the document is unchanged and
    /// still accepted.
    UnchangedAtOne,
}

struct LegacyMatrixCase {
    name: &'static str,
    current: Value,
    kind: ArtifactKind,
    downgrade: Downgrade,
}

/// Asserts that a document is rejected by rule 0 with the exact message and
/// nothing else.
fn assert_legacy(case: &str, document: &Value) {
    let errors = check(&document.to_string()).expect_err(&format!(
        "{case}: a legacy-marked document must be rejected"
    ));
    assert_eq!(
        errors,
        vec![LEGACY_MESSAGE.to_string()],
        "{case}: a legacy artifact must get exactly the migration message"
    );
}

/// R2-09: every classified artifact kind accepts its own current document and
/// rejects a document carrying a legacy marker with the named message. The
/// second half encodes each kind's *real* 0.2 form rather than pretending
/// every root carried a mark it never serialized.
#[test]
fn legacy_detection_matrix_covers_every_classified_artifact_kind() {
    let cases = vec![
        LegacyMatrixCase {
            name: "IR artifact",
            current: json!({
                "formatVersion": 2,
                "pheroVersion": "0.3.0",
                "packages": [],
            }),
            kind: ArtifactKind::IrModel,
            downgrade: Downgrade::LegacyProvenance,
        },
        LegacyMatrixCase {
            name: "actor artifact",
            current: json!({"formatVersion": 2, "blocks": [{"name": "Support"}]}),
            kind: ArtifactKind::ActorModel,
            downgrade: Downgrade::VersionMismatch,
        },
        LegacyMatrixCase {
            name: "DDD design artifact",
            current: json!({
                "formatVersion": 2,
                "pheroVersion": "0.3.0",
                "domainFingerprint": {
                    "algorithm": "sha256",
                    "value": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                    "packages": [],
                },
                "application": {"name": "Library"},
                "modules": [{"name": "core"}],
            }),
            kind: ArtifactKind::DddModel,
            downgrade: Downgrade::VersionMismatch,
        },
        LegacyMatrixCase {
            name: "event artifact",
            current: json!({"formatVersion": 2, "events": [{"name": "Ping"}]}),
            kind: ArtifactKind::EventModel,
            downgrade: Downgrade::VersionMismatch,
        },
        LegacyMatrixCase {
            name: "deployment artifact",
            current: json!({"formatVersion": 2, "applications": [{"name": "Svc"}]}),
            kind: ArtifactKind::DeployModel,
            downgrade: Downgrade::VersionMismatch,
        },
        LegacyMatrixCase {
            name: "use-case artifact",
            current: json!({"formatVersion": 2, "useCases": [{"name": "HandOver"}]}),
            kind: ArtifactKind::UseCaseModel,
            downgrade: Downgrade::VersionMismatch,
        },
        LegacyMatrixCase {
            name: "case artifact",
            current: json!({"formatVersion": 2, "cases": [{"name": "Loan"}]}),
            kind: ArtifactKind::CmmnModel,
            downgrade: Downgrade::VersionMismatch,
        },
        LegacyMatrixCase {
            name: "decision artifact",
            current: json!({"formatVersion": 2, "decisions": [{"name": "Assess"}]}),
            kind: ArtifactKind::DmnModel,
            downgrade: Downgrade::VersionMismatch,
        },
        LegacyMatrixCase {
            name: "interaction-flow artifact",
            current: json!({"formatVersion": 2, "views": [{"name": "Home"}]}),
            kind: ArtifactKind::IfmlModel,
            downgrade: Downgrade::VersionMismatch,
        },
        LegacyMatrixCase {
            name: "control/route manifest artifact",
            // The manifest is deliberately not a classified kind: it shares
            // the IFML root shape (`views`), so the IFML rule claims it. The
            // row still belongs here because the manifest is a versioned
            // artifact root that moved to `formatVersion` 2.
            current: json!({
                "formatVersion": 2,
                "views": [{"name": "CustomerList"}],
                "controls": [],
                "edges": [],
                "actions": [],
                "commands": [],
            }),
            kind: ArtifactKind::IfmlModel,
            downgrade: Downgrade::VersionMismatch,
        },
        LegacyMatrixCase {
            name: "semantic graph artifact",
            current: json!({
                "formatVersion": 1,
                "applications": [],
                "nodes": [{
                    "id": "class:support.mox#nz.example.support.Ticket",
                    "kind": "class",
                    "name": "nz.example.support.Ticket",
                    "file": "support.mox",
                    "application": "support.mox",
                }],
                "edges": [],
            }),
            kind: ArtifactKind::GraphModel,
            downgrade: Downgrade::UnchangedAtOne,
        },
        LegacyMatrixCase {
            name: "canonical instance",
            current: json!({
                "$type": "phero.instance",
                "formatVersion": 2,
                "model": "demo",
                "objects": [],
            }),
            kind: ArtifactKind::Instance,
            downgrade: Downgrade::LegacyDiscriminator,
        },
    ];

    for case in &cases {
        assert_eq!(
            check(&case.current.to_string()),
            Ok(case.kind),
            "{}: the current document must be accepted",
            case.name
        );

        // Every kind is caught by either old marker before shape-sniffing:
        // the provenance marker names the document a 0.2 artifact, and the
        // discriminator marker proves rule 0 runs before the `$type` branch.
        let mut marked = case.current.clone();
        marked
            .as_object_mut()
            .expect("matrix documents are objects")
            .insert(LEGACY_PROVENANCE_KEY.to_string(), json!("0.2.0"));
        assert_legacy(case.name, &marked);

        let mut marked_instance = case.current.clone();
        marked_instance
            .as_object_mut()
            .expect("matrix documents are objects")
            .insert("$type".to_string(), json!(LEGACY_INSTANCE_TYPE));
        assert_legacy(case.name, &marked_instance);

        // The document's actual 0.2-era form.
        let mut downgraded = case.current.clone();
        let object = downgraded
            .as_object_mut()
            .expect("matrix documents are objects");
        match case.downgrade {
            Downgrade::LegacyProvenance => {
                object.insert("formatVersion".to_string(), json!(1));
                object.remove("pheroVersion");
                object.insert(LEGACY_PROVENANCE_KEY.to_string(), json!("0.2.0"));
                assert_legacy(case.name, &downgraded);
            }
            Downgrade::LegacyDiscriminator => {
                object.insert("formatVersion".to_string(), json!(1));
                object.insert("$type".to_string(), json!(LEGACY_INSTANCE_TYPE));
                assert_legacy(case.name, &downgraded);
            }
            Downgrade::VersionMismatch => {
                object.insert("formatVersion".to_string(), json!(1));
                let errors = check(&downgraded.to_string()).expect_err(
                    "a marker-less 0.2 artifact must still be rejected by the version gate",
                );
                assert!(
                    errors.iter().all(|error| !error.contains(LEGACY_MESSAGE)),
                    "{}: a marker-less 0.2 artifact is a version mismatch, never a \
                     legacy artifact: {errors:?}",
                    case.name
                );
                assert!(
                    errors.iter().any(|error| error.contains("formatVersion 1")
                        && error.contains("formatVersion 2")),
                    "{}: the gate must name both versions: {errors:?}",
                    case.name
                );
            }
            Downgrade::UnchangedAtOne => {
                // The graph marker deliberately stayed at 1: the document is
                // its own 0.2 form, so it is accepted — and it is never
                // reported as a legacy artifact.
                assert_eq!(
                    object.get("formatVersion"),
                    Some(&json!(1)),
                    "{}: the graph marker stays at 1 by design",
                    case.name
                );
                assert_eq!(
                    check(&downgraded.to_string()),
                    Ok(case.kind),
                    "{}: an unmoved marker is not a downgrade",
                    case.name
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// DTO boundaries (DD3, pherolang#300)
// ---------------------------------------------------------------------------

/// A DTO-bearing design artifact: an entity design, a projection DTO over
/// it (input-capable, so it may fill a parameter), a standalone DTO, and a
/// service whose signature uses both boundaries.
const GOOD_DTO_ARTIFACT: &str = r#"{
  "formatVersion": 2,
  "pheroVersion": "0.3.0",
  "domainFingerprint": {
    "algorithm": "sha256",
    "value": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "packages": []
  },
  "application": { "name": "Library" },
  "modules": [
    {
      "name": "media",
      "services": [
        {
          "name": "MediaService",
          "operations": [
            {
              "name": "describe",
              "returnType": { "dto": "BookSummary" },
              "params": [
                { "name": "request", "type": { "dto": "CreateBookRequest" } }
              ]
            }
          ]
        }
      ],
      "designs": [
        {
          "class": "Book",
          "typeRef": {
            "type": "class",
            "value": { "package": "nz.example.library", "name": "Book" }
          },
          "stereotype": "entity",
          "isAggregateRoot": true
        },
        {
          "class": "Book",
          "typeRef": {
            "type": "class",
            "value": { "package": "nz.example.library", "name": "Book" }
          },
          "stereotype": "dto",
          "dtoShape": {
            "name": "BookSummary",
            "source": {
              "type": "class",
              "value": { "package": "nz.example.library", "name": "Book" }
            },
            "direction": "input",
            "members": [
              {
                "name": "title",
                "kind": "select",
                "type": { "type": "primitive", "value": "string" },
                "path": ["title"]
              }
            ]
          }
        },
        {
          "stereotype": "dto",
          "dtoShape": {
            "name": "CreateBookRequest",
            "direction": "input",
            "members": [
              {
                "name": "title",
                "kind": "declared",
                "type": { "type": "primitive", "value": "string" }
              }
            ]
          }
        }
      ]
    }
  ]
}"#;

fn dto_artifact() -> Value {
    serde_json::from_str(GOOD_DTO_ARTIFACT).expect("the DTO fixture is valid JSON")
}

fn dto_errors(artifact: &Value) -> Vec<String> {
    check(&artifact.to_string()).expect_err("the artifact must violate the DTO contract")
}

fn has_error(errors: &[String], needle: &str) -> bool {
    errors.iter().any(|error| error.contains(needle))
}

#[test]
fn a_dto_bearing_artifact_satisfies_the_ddd_contract() {
    assert_eq!(check(GOOD_DTO_ARTIFACT), Ok(ArtifactKind::DddModel));
}

#[test]
fn a_dto_design_without_its_shape_is_rejected() {
    let mut artifact = dto_artifact();
    artifact["modules"][0]["designs"][1]
        .as_object_mut()
        .expect("the projection design")
        .remove("dtoShape");
    let errors = dto_errors(&artifact);
    assert!(
        has_error(&errors, "designs[1].dtoShape")
            && has_error(&errors, "must carry its boundary shape"),
        "{errors:?}"
    );
}

#[test]
fn a_non_dto_design_carrying_a_dto_shape_is_rejected() {
    let mut artifact = dto_artifact();
    let shape = artifact["modules"][0]["designs"][1]
        .as_object_mut()
        .expect("the projection design")
        .remove("dtoShape")
        .expect("the projection carries a shape");
    artifact["modules"][0]["designs"][0]["dtoShape"] = shape;
    let errors = dto_errors(&artifact);
    assert!(
        has_error(&errors, "designs[0].dtoShape") && has_error(&errors, "only a dto design"),
        "{errors:?}"
    );
}

#[test]
fn a_standalone_dto_must_not_carry_a_class() {
    let mut artifact = dto_artifact();
    artifact["modules"][0]["designs"][2]["class"] = json!("Book");
    artifact["modules"][0]["designs"][2]["typeRef"] = json!({
        "type": "class",
        "value": { "package": "nz.example.library", "name": "Book" }
    });
    let errors = dto_errors(&artifact);
    assert!(has_error(&errors, "designs[2].class"), "{errors:?}");
    assert!(has_error(&errors, "designs[2].typeRef"), "{errors:?}");
}

#[test]
fn a_projection_dto_must_agree_with_its_source() {
    let mut artifact = dto_artifact();
    artifact["modules"][0]["designs"][1]["typeRef"] = json!({
        "type": "class",
        "value": { "package": "nz.example.library", "name": "Movie" }
    });
    let errors = dto_errors(&artifact);
    assert!(
        has_error(&errors, "designs[1].typeRef") && has_error(&errors, "dto boundary's source"),
        "{errors:?}"
    );
}

#[test]
fn a_dto_boundary_name_must_not_shadow_a_designed_class() {
    let mut artifact = dto_artifact();
    artifact["modules"][0]["designs"][2]["dtoShape"]["name"] = json!("Book");
    let errors = dto_errors(&artifact);
    assert!(
        has_error(&errors, "designs[2].dtoShape.name")
            && has_error(&errors, "shadows a designed class"),
        "{errors:?}"
    );
}

#[test]
fn dto_member_payloads_must_match_their_kind() {
    let mut artifact = dto_artifact();
    artifact["modules"][0]["designs"][1]["dtoShape"]["members"][0]["expression"] = json!("title");
    artifact["modules"][0]["designs"][2]["dtoShape"]["members"] = json!([
        {
            "name": "title",
            "kind": "derive",
            "type": { "type": "primitive", "value": "string" },
            "path": ["title"]
        }
    ]);
    let errors = dto_errors(&artifact);
    assert!(
        has_error(
            &errors,
            "select member must not carry a derivation expression"
        ),
        "{errors:?}"
    );
    assert!(
        has_error(
            &errors,
            "derive member must carry the derivation's source text"
        ),
        "{errors:?}"
    );
    assert!(
        has_error(
            &errors,
            "derive member must not carry a selected feature path"
        ),
        "{errors:?}"
    );
}

#[test]
fn a_duplicate_dto_member_is_rejected() {
    let mut artifact = dto_artifact();
    artifact["modules"][0]["designs"][1]["dtoShape"]["members"]
        .as_array_mut()
        .expect("the members array")
        .push(json!({
            "name": "title",
            "kind": "select",
            "type": { "type": "primitive", "value": "string" },
            "path": ["title"]
        }));
    let errors = dto_errors(&artifact);
    assert!(
        has_error(&errors, "designs[1].dtoShape.members[1].name")
            && has_error(&errors, "duplicate dto member 'title'"),
        "{errors:?}"
    );
}

#[test]
fn a_dto_reference_must_resolve() {
    let mut artifact = dto_artifact();
    artifact["modules"][0]["services"][0]["operations"][0]["returnType"] =
        json!({ "dto": "Ghost" });
    artifact["modules"][0]["services"][0]["operations"][0]["params"][0]["type"] =
        json!({ "dto": "Ghost" });
    let errors = dto_errors(&artifact);
    assert!(
        has_error(&errors, "operations[0].returnType") && has_error(&errors, "dto 'Ghost'"),
        "{errors:?}"
    );
    assert!(
        has_error(&errors, "operations[0].params[0].type"),
        "{errors:?}"
    );
}

#[test]
fn an_output_dto_cannot_fill_a_parameter() {
    let mut artifact = dto_artifact();
    artifact["modules"][0]["designs"][1]["dtoShape"]["direction"] = json!("output");
    artifact["modules"][0]["designs"][1]["dtoShape"]["members"]
        .as_array_mut()
        .expect("the members array")
        .push(json!({
            "name": "label",
            "kind": "derive",
            "type": { "type": "primitive", "value": "string" },
            "expression": "title"
        }));
    artifact["modules"][0]["services"][0]["operations"][0]["params"][0]["type"] =
        json!({ "dto": "BookSummary" });
    let errors = dto_errors(&artifact);
    assert!(
        has_error(&errors, "operations[0].params[0].type")
            && has_error(&errors, "output-only")
            && has_error(&errors, "label"),
        "{errors:?}"
    );
}

#[test]
fn a_dto_cannot_carry_a_repository_or_a_machine() {
    let mut artifact = dto_artifact();
    artifact["modules"][0]["designs"][1]["repository"] = json!({
        "name": "BookRepository",
        "operations": []
    });
    artifact["modules"][0]["designs"][1]["machine"] = json!({
        "field": "state",
        "enumType": {
            "type": "enum",
            "value": { "package": "nz.example.library", "name": "MediaStatus" }
        },
        "initial": "available"
    });
    let errors = dto_errors(&artifact);
    assert!(
        has_error(&errors, "designs[1].repository")
            && has_error(&errors, "only an entity design may declare a repository"),
        "{errors:?}"
    );
    assert!(
        has_error(&errors, "designs[1].machine")
            && has_error(&errors, "only an entity design may declare a state machine"),
        "{errors:?}"
    );
}

#[test]
fn a_dto_direction_is_checked_against_its_members() {
    let mut artifact = dto_artifact();
    artifact["modules"][0]["designs"][1]["dtoShape"]["direction"] = json!("output");
    let errors = dto_errors(&artifact);
    assert!(
        has_error(&errors, "designs[1].dtoShape.direction")
            && has_error(&errors, "has no derive member"),
        "{errors:?}"
    );
}
