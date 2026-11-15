//! The per-kind artifact validators: the shared version-gate/shape
//! check and the per-surface entry wrappers.

use serde_json::Value;

use super::instances::format_version_error;
use super::keys::{check_named_array, scan_snake_case_keys};
use super::ArtifactKind;

/// Validates one artifact root against its kind's spec: the version gate
/// mirrors the root's `from_json` constant, each named key must be an
/// array of named objects, any extra per-kind shape checks run, and the
/// whole document is scanned for snake_case keys.
pub(super) fn check_kind(
    root: &serde_json::Map<String, Value>,
    kind: ArtifactKind,
    version: u32,
    label: &str,
    keys: &[&str],
    extra: fn(&serde_json::Map<String, Value>, &mut Vec<String>),
) -> Result<ArtifactKind, Vec<String>> {
    let mut errors = Vec::new();
    if let Some(error) = format_version_error(root, version, label) {
        errors.push(error);
    }
    extra(root, &mut errors);
    for key in keys {
        if let Some(array) = root.get(*key) {
            check_named_array(array, key, &mut errors);
        }
    }
    scan_snake_case_keys(&Value::Object(root.clone()), &mut errors);
    finish(kind, errors)
}

/// The common shape has no checks beyond the version gate and the named
/// arrays.
fn no_extra(_: &serde_json::Map<String, Value>, _: &mut Vec<String>) {}

/// The DDD design root's `application` object must carry a non-empty
/// string `name`, and the document must satisfy the artifact-only DDD
/// contract (resolved companions, effective design decisions, aligned
/// parallel lists) through `phero-backend-ddd`.
fn check_ddd_contract(root: &serde_json::Map<String, Value>, errors: &mut Vec<String>) {
    check_ddd_application(root, errors);
    let value = Value::Object(root.clone());
    match serde_json::from_value::<phero_ir::ddd::DddModel>(value) {
        Err(error) => errors.push(format!(
            "the DDD design artifact does not match the DDD model: {error}"
        )),
        Ok(model) => errors.extend(
            phero_backend_ddd::check_contract(&model)
                .into_iter()
                .map(|violation| violation.to_string()),
        ),
    }
}

/// The DDD design root's `application` object must carry a non-empty
/// string `name`.
fn check_ddd_application(root: &serde_json::Map<String, Value>, errors: &mut Vec<String>) {
    let Some(application) = root.get("application") else {
        return;
    };
    match application.as_object() {
        None => errors.push(format!(
            "\"application\" must be an object, found {application}"
        )),
        Some(object) => match object.get("name") {
            None => errors.push("the DDD design \"application\" is missing \"name\"".to_string()),
            Some(name) if name.as_str().is_none_or(str::is_empty) => errors.push(format!(
                "the DDD design \"application\" \"name\" must be a non-empty \
                 string, found {name}"
            )),
            _ => {}
        },
    }
}

/// Validates an interaction-flow artifact: the version gate mirrors
/// [`phero_ir::ifml::IfmlModel::from_json`], and `views`, `domains`,
/// `actions`, `modules`, `actors`, and `commands` must be arrays of named
/// objects (`imports` is an array of strings — excluded from the named
/// check).
pub(super) fn check_ifml_model(
    root: &serde_json::Map<String, Value>,
) -> Result<ArtifactKind, Vec<String>> {
    check_kind(
        root,
        ArtifactKind::IfmlModel,
        phero_ir::ifml::IFML_MODEL_FORMAT_VERSION,
        "interaction-flow artifact",
        &[
            "views", "domains", "actions", "modules", "actors", "commands",
        ],
        no_extra,
    )
}

pub(super) fn check_ir_model(
    root: &serde_json::Map<String, Value>,
) -> Result<ArtifactKind, Vec<String>> {
    check_kind(
        root,
        ArtifactKind::IrModel,
        phero_ir::FORMAT_VERSION,
        "IR artifact",
        &["packages"],
        check_ir_collections,
    )
}

/// DD6 (issue #303): a `collection` marker must agree with its feature. A set
/// is duplicate-free, so `collection: "set"` requires `constraints.unique`;
/// a collection kind is only meaningful on a many-valued, non-container
/// feature. The check runs only when the version gate passes and the
/// document deserializes as the Core IR — a malformed artifact is the shape
/// gate's problem, not this contract's.
fn check_ir_collections(root: &serde_json::Map<String, Value>, errors: &mut Vec<String>) {
    if format_version_error(root, phero_ir::FORMAT_VERSION, "IR artifact").is_some() {
        return;
    }
    let Ok(model) = serde_json::from_value::<phero_ir::Model>(Value::Object(root.clone())) else {
        return;
    };
    for package in &model.packages {
        for class in &package.classes {
            for feature in &class.features {
                if feature.collection.is_none() {
                    continue;
                }
                let location = format!("class '{}' feature '{}'", class.name, feature.name);
                if feature.kind == phero_ir::FeatureKind::Container {
                    errors.push(format!(
                        "IR artifact {location}: a collection kind on a container feature; \
                         a container is the single-valued owner slot and cannot be a collection"
                    ));
                    continue;
                }
                if !feature.multiplicity.is_many() {
                    errors.push(format!(
                        "IR artifact {location}: a collection kind on a single-valued feature; \
                         collection kinds apply to many-valued features only"
                    ));
                    continue;
                }
                if !feature.constraints.unique {
                    errors.push(format!(
                        "IR artifact {location}: collection \"set\" without constraints.unique; \
                         a set is duplicate-free, so the two markers must agree"
                    ));
                }
            }
        }
    }
}

pub(super) fn check_actor_model(
    root: &serde_json::Map<String, Value>,
) -> Result<ArtifactKind, Vec<String>> {
    check_kind(
        root,
        ArtifactKind::ActorModel,
        phero_ir::ACTOR_MODEL_FORMAT_VERSION,
        "actor artifact",
        &["blocks"],
        no_extra,
    )
}

/// Validates a DDD design artifact: the version gate mirrors
/// [`phero_ir::ddd::DddModel::from_json`], the `application` object must
/// carry a non-empty string `name`, and `modules` must be an array of
/// named objects.
pub(super) fn check_ddd_model(
    root: &serde_json::Map<String, Value>,
) -> Result<ArtifactKind, Vec<String>> {
    check_kind(
        root,
        ArtifactKind::DddModel,
        phero_ir::ddd::DDD_MODEL_FORMAT_VERSION,
        "DDD design artifact",
        &["modules"],
        check_ddd_contract,
    )
}

/// Validates an event-contract artifact: the version gate mirrors
/// [`phero_ir::events::EventModel::from_json`], and `events`, `channels`,
/// and `subscriptions` must be arrays of named objects.
pub(super) fn check_event_model(
    root: &serde_json::Map<String, Value>,
) -> Result<ArtifactKind, Vec<String>> {
    check_kind(
        root,
        ArtifactKind::EventModel,
        phero_ir::events::EVENT_MODEL_FORMAT_VERSION,
        "event artifact",
        &["events", "channels", "subscriptions"],
        no_extra,
    )
}

/// Validates a deployment artifact: the version gate mirrors
/// [`phero_ir::deploy::DeployModel::from_json`], and `applications`,
/// `profiles`, and `deployments` must be arrays of named objects.
pub(super) fn check_deploy_model(
    root: &serde_json::Map<String, Value>,
) -> Result<ArtifactKind, Vec<String>> {
    check_kind(
        root,
        ArtifactKind::DeployModel,
        phero_ir::deploy::DEPLOY_MODEL_FORMAT_VERSION,
        "deployment artifact",
        &["applications", "profiles", "deployments"],
        no_extra,
    )
}

/// Validates a use-case artifact: the version gate mirrors
/// [`phero_ir::usecases::UseCaseModel::from_json`], and `useCases` must be
/// an array of named objects.
pub(super) fn check_usecase_model(
    root: &serde_json::Map<String, Value>,
) -> Result<ArtifactKind, Vec<String>> {
    check_kind(
        root,
        ArtifactKind::UseCaseModel,
        phero_ir::usecases::USE_CASE_MODEL_FORMAT_VERSION,
        "use-case artifact",
        &["useCases"],
        no_extra,
    )
}

/// Validates a case artifact: the version gate mirrors
/// [`phero_ir::cmmn::CmmnModel::from_json`], and `cases` must be an array
/// of named objects.
pub(super) fn check_cmmn_model(
    root: &serde_json::Map<String, Value>,
) -> Result<ArtifactKind, Vec<String>> {
    check_kind(
        root,
        ArtifactKind::CmmnModel,
        phero_ir::cmmn::CMMN_MODEL_FORMAT_VERSION,
        "case artifact",
        &["cases"],
        no_extra,
    )
}

/// Validates a decision-model artifact: the version gate mirrors
/// [`phero_ir::dmn::DmnModel::from_json`], and `decisions` must be an array
/// of named objects.
pub(super) fn check_dmn_model(
    root: &serde_json::Map<String, Value>,
) -> Result<ArtifactKind, Vec<String>> {
    check_kind(
        root,
        ArtifactKind::DmnModel,
        phero_ir::dmn::DMN_MODEL_FORMAT_VERSION,
        "decision artifact",
        &["decisions"],
        no_extra,
    )
}

/// Validates a semantic graph artifact: the version gate mirrors
/// [`phero_ir::graph::GraphModel::from_json`], and the three collections carry
/// their identity fields — applications an `id`, nodes a non-empty `id`
/// (plus `kind`, `name`, `file`, and `application`), edges non-empty `from`
/// and `to` (plus `kind`). Optional string arrays (`files`, `packages`) and
/// the `attributes` maps are shape-checked too.
pub(super) fn check_graph_model(
    root: &serde_json::Map<String, Value>,
) -> Result<ArtifactKind, Vec<String>> {
    let mut errors = Vec::new();
    if let Some(error) = format_version_error(
        root,
        phero_ir::graph::GRAPH_MODEL_FORMAT_VERSION,
        "semantic graph artifact",
    ) {
        errors.push(error);
    }
    check_graph_objects(
        root.get("applications"),
        "applications",
        &["id"],
        &mut errors,
    );
    check_string_arrays(
        root.get("applications"),
        "applications",
        &["files", "packages"],
        &mut errors,
    );
    check_graph_objects(
        root.get("nodes"),
        "nodes",
        &["id", "kind", "name", "file", "application"],
        &mut errors,
    );
    check_graph_attributes(root.get("nodes"), "nodes", &mut errors);
    check_graph_objects(
        root.get("edges"),
        "edges",
        &["from", "to", "kind"],
        &mut errors,
    );
    check_graph_attributes(root.get("edges"), "edges", &mut errors);
    scan_snake_case_keys(&Value::Object(root.clone()), &mut errors);
    finish(ArtifactKind::GraphModel, errors)
}

/// Every entry of an optional graph collection must be an object carrying
/// each required field as a non-empty string.
fn check_graph_objects(
    array: Option<&Value>,
    key: &str,
    required: &[&str],
    errors: &mut Vec<String>,
) {
    let Some(value) = array else {
        return;
    };
    let Some(items) = value.as_array() else {
        errors.push(format!("{key:?} must be an array, found {value}"));
        return;
    };
    for (index, item) in items.iter().enumerate() {
        let Some(object) = item.as_object() else {
            errors.push(format!("{key}[{index}] must be an object, found {item}"));
            continue;
        };
        for field in required {
            match object.get(*field) {
                None => errors.push(format!("{key}[{index}] is missing \"{field}\"")),
                Some(value) if value.as_str().is_none_or(str::is_empty) => errors.push(format!(
                    "{key}[{index}] \"{field}\" must be a non-empty string, found {value}"
                )),
                _ => {}
            }
        }
    }
}

/// The optional `files`/`packages` arrays of an application must be arrays
/// of strings.
fn check_string_arrays(
    array: Option<&Value>,
    key: &str,
    fields: &[&str],
    errors: &mut Vec<String>,
) {
    let Some(Value::Array(items)) = array else {
        // A non-array has already been reported by [`check_graph_objects`].
        return;
    };
    for (index, item) in items.iter().enumerate() {
        let Some(object) = item.as_object() else {
            continue;
        };
        for field in fields {
            let Some(value) = object.get(*field) else {
                continue;
            };
            match value.as_array() {
                None => errors.push(format!("{key}[{index}] \"{field}\" must be an array")),
                Some(entries) if entries.iter().any(|entry| entry.as_str().is_none()) => errors
                    .push(format!(
                        "{key}[{index}] \"{field}\" must be an array of strings"
                    )),
                Some(_) => {}
            }
        }
    }
}

/// An optional `attributes` map must map strings to strings.
fn check_graph_attributes(array: Option<&Value>, key: &str, errors: &mut Vec<String>) {
    let Some(Value::Array(items)) = array else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        let Some(object) = item.as_object() else {
            continue;
        };
        let Some(value) = object.get("attributes") else {
            continue;
        };
        match value.as_object() {
            None => errors.push(format!("{key}[{index}] \"attributes\" must be an object")),
            Some(entries) if entries.values().any(|entry| entry.as_str().is_none()) => errors.push(
                format!("{key}[{index}] \"attributes\" must map strings to strings"),
            ),
            Some(_) => {}
        }
    }
}

/// The shared result constructor: the kind when no violation was found,
/// every violation otherwise.
pub(super) fn finish(kind: ArtifactKind, errors: Vec<String>) -> Result<ArtifactKind, Vec<String>> {
    if errors.is_empty() {
        Ok(kind)
    } else {
        Err(errors)
    }
}
