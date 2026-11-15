//! The artifact-only `.ddd` contract.
//!
//! The compiler is the source of truth for these rules; this module repeats
//! them over a serialized [`DddModel`] so a consumer can prove a published
//! artifact is contract-complete without the `.mox` sources. Every
//! violation carries the JSON path of the offending value, so a hand-edited
//! artifact is diagnosable without a compiler session.
//!
//! The dimensions checked here are the resolved class companions (DD1), the
//! strict `value` semantics block (DD2), and the DTO boundary (DD3,
//! pherolang#300) — the `dtoShape` of every `dto` design, the
//! `class`/`typeRef` pair agreeing with its source, the derived direction,
//! member kind/payload consistency, and every `DddTypeRef::Dto` slot
//! resolving to a boundary declared in the same artifact.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use phero_ir::ddd::{
    DddModel, DddParam, DddTypeRef, DecisionRef, Delegation, Design, DtoDirection, DtoMember,
    DtoMemberKind, DtoShape, EqualityPolicy, ErrorDecl, ErrorRef, ErrorScope, EventRef,
    IdentityPolicy, Repository, RepositoryOperation, SearchDef, Service, ServiceOperation,
    Stereotype, TargetKind, TargetRef, DDD_MODEL_FORMAT_VERSION,
};
use phero_ir::TypeRef;

/// One contract violation, located by its JSON path in the artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The JSON path of the offending value (`modules[0].designs[1]`).
    pub path: String,
    /// What is wrong, phrased without the path.
    pub message: String,
}

impl Violation {
    /// Creates a violation at `path`.
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

/// Validates every decision the compiler made that the artifact must carry.
///
/// The check is pure and artifact-only: it never reads sources, the driver,
/// or another artifact. An empty result means the artifact is
/// contract-complete for the `.ddd` surface.
pub fn check_contract(model: &DddModel) -> Vec<Violation> {
    let mut violations = Vec::new();
    if model.format_version != DDD_MODEL_FORMAT_VERSION {
        violations.push(Violation::new(
            "formatVersion",
            format!(
                "is {} but this backend checks {DDD_MODEL_FORMAT_VERSION}",
                model.format_version
            ),
        ));
    }
    match model.phero_version.as_deref() {
        Some(version) if !version.trim().is_empty() => {}
        _ => violations.push(Violation::new(
            "pheroVersion",
            "the design artifact must carry a non-empty provenance string",
        )),
    }

    let application = model.application.as_ref();
    let application_name = match application {
        None => {
            violations.push(Violation::new(
                "application",
                "the design artifact must declare an application",
            ));
            None
        }
        Some(application) if application.name.trim().is_empty() => {
            violations.push(Violation::new(
                "application.name",
                "the application name must be non-empty",
            ));
            None
        }
        Some(application) => Some(application.name.as_str()),
    };

    let registry = Registry::build(model);
    validate_error_vocabulary(model, &registry, &mut violations);
    validate_modules(model, application_name, &registry, &mut violations);
    validate_aggregate_and_invariants(model, &mut violations);
    violations
}

/// The aggregate-boundary contract (DD4, pherolang#301) and the invariant
/// contract (DD5, pherolang#302), checked from the artifact alone:
///
/// - every entity design carries `isAggregateRoot`; an absent verdict means
///   the compile could not derive it and the artifact is not contract-complete;
/// - roots carry no `aggregate`; members carry `aggregate { root, origin }`
///   and the root names an entity design marked as a root in this artifact;
/// - non-entity designs carry neither key;
/// - invariants are non-empty and only entity/value designs carry them.
fn validate_aggregate_and_invariants(model: &DddModel, violations: &mut Vec<Violation>) {
    let mut roots: BTreeSet<(String, String)> = BTreeSet::new();
    for module in &model.modules {
        for design in &module.designs {
            if design.stereotype == Stereotype::Entity && design.is_aggregate_root == Some(true) {
                if let Some(TypeRef::Class { package, name }) = &design.type_ref {
                    roots.insert((package.clone(), name.clone()));
                }
            }
        }
    }
    for (module_index, module) in model.modules.iter().enumerate() {
        for (design_index, design) in module.designs.iter().enumerate() {
            let path = format!("modules[{module_index}].designs[{design_index}]");
            match design.stereotype {
                Stereotype::Entity => match design.is_aggregate_root {
                    None => violations.push(Violation::new(
                        format!("{path}.isAggregateRoot"),
                        "an entity design must carry its effective aggregate verdict; \
                         absent means the compile could not derive it (an incomplete \
                         imported domain union)",
                    )),
                    Some(true) => {
                        if design.aggregate.is_some() {
                            violations.push(Violation::new(
                                format!("{path}.aggregate"),
                                "an aggregate root must not carry an \"aggregate\" membership",
                            ));
                        }
                    }
                    Some(false) => match &design.aggregate {
                        None => violations.push(Violation::new(
                            format!("{path}.aggregate"),
                            "a non-root entity design must carry its resolved aggregate \
                             membership",
                        )),
                        Some(membership) => {
                            let TypeRef::Class { package, name } = &membership.root else {
                                violations.push(Violation::new(
                                    format!("{path}.aggregate.root"),
                                    "an aggregate root must be a resolved class reference",
                                ));
                                continue;
                            };
                            let key = (package.clone(), name.clone());
                            if design.type_ref.as_ref() == Some(&membership.root) {
                                violations.push(Violation::new(
                                    format!("{path}.aggregate.root"),
                                    "an aggregate member cannot be its own root",
                                ));
                            } else if !roots.contains(&key) {
                                violations.push(Violation::new(
                                    format!("{path}.aggregate.root"),
                                    format!(
                                        "aggregate root '{package}.{name}' is not an entity \
                                         design marked as a root in this artifact"
                                    ),
                                ));
                            }
                        }
                    },
                },
                Stereotype::Value | Stereotype::Dto => {
                    if design.is_aggregate_root.is_some() || design.aggregate.is_some() {
                        violations.push(Violation::new(
                            &path,
                            "only an entity design may carry an aggregate boundary",
                        ));
                    }
                }
            }
            if design.stereotype == Stereotype::Dto && !design.invariants.is_empty() {
                violations.push(Violation::new(
                    format!("{path}.invariants"),
                    "a dto design cannot carry invariants",
                ));
            }
            for (index, invariant) in design.invariants.iter().enumerate() {
                if invariant.expression.trim().is_empty() {
                    violations.push(Violation::new(
                        format!("{path}.invariants[{index}].expression"),
                        "an invariant expression must be non-empty",
                    ));
                }
            }
        }
    }
}

/// The standard prelude's fixed error names (DD7). The prelude is implicit
/// in the artifact: it is never emitted in an `errors` list, is always
/// available, and cannot be shadowed. Its payload shapes are part of the
/// consumer contract (`NotFound(resource, key)`,
/// `Conflict(detail)`), so the artifact-only check keys on the names.
const PRELUDE_ERROR_NAMES: [&str; 2] = ["NotFound", "Conflict"];

fn is_prelude_error(name: &str) -> bool {
    PRELUDE_ERROR_NAMES.contains(&name)
}

/// Validates the declared error vocabulary (DD7): application-wide unique
/// names, no prelude shadowing, unique payload field names, and every
/// DTO-typed payload slot resolving to a boundary declared in this artifact.
/// Duplicate names are reported at the later declaration.
fn validate_error_vocabulary(
    model: &DddModel,
    registry: &Registry<'_>,
    violations: &mut Vec<Violation>,
) {
    let mut declared: BTreeMap<String, String> = BTreeMap::new();
    for (index, error) in model.errors.iter().enumerate() {
        check_error_decl(
            error,
            &format!("errors[{index}]"),
            &mut declared,
            registry,
            violations,
        );
    }
    for (module_index, module) in model.modules.iter().enumerate() {
        for (index, error) in module.errors.iter().enumerate() {
            check_error_decl(
                error,
                &format!("modules[{module_index}].errors[{index}]"),
                &mut declared,
                registry,
                violations,
            );
        }
    }
}

/// One declaration's checks: no prelude shadow, application-wide unique
/// name, unique payload fields, and every DTO payload slot resolving.
fn check_error_decl(
    error: &ErrorDecl,
    path: &str,
    declared: &mut BTreeMap<String, String>,
    registry: &Registry<'_>,
    violations: &mut Vec<Violation>,
) {
    if is_prelude_error(&error.name) {
        violations.push(Violation::new(
            format!("{path}.name"),
            format!(
                "shadows the standard prelude error '{}'; the prelude is implicit and \
                 cannot be redeclared",
                error.name
            ),
        ));
    } else if let Some(previous) = declared.insert(error.name.clone(), path.to_string()) {
        violations.push(Violation::new(
            format!("{path}.name"),
            format!(
                "duplicate error '{}' in the application; error names are \
                 application-wide unique (already declared at {previous})",
                error.name
            ),
        ));
    }
    let mut fields: BTreeSet<&str> = BTreeSet::new();
    for (index, field) in error.fields.iter().enumerate() {
        if !fields.insert(field.name.as_str()) {
            violations.push(Violation::new(
                format!("{path}.fields[{index}].name"),
                format!(
                    "duplicate payload field '{}' in error '{}'",
                    field.name, error.name
                ),
            ));
        }
        check_ddd_type_ref(
            violations,
            &format!("{path}.fields[{index}].type"),
            &field.type_,
            registry,
        );
    }
}

/// Every declared service, repository, and search, by name, with the module
/// that declares it. The compiler guarantees the names are unique within
/// their kind, so a plain map is enough for resolution.
///
/// The registry also indexes the DD3 DTO dimensions (pherolang#300): every
/// boundary shape by name, and the class names the designs target (the
/// namespace a DTO boundary name must not shadow).
#[derive(Default)]
struct Registry<'a> {
    services: BTreeMap<&'a str, &'a str>,
    repositories: BTreeMap<&'a str, &'a str>,
    searches: BTreeMap<&'a str, &'a str>,
    dtos: BTreeMap<&'a str, &'a DtoShape>,
    designed_classes: BTreeSet<&'a str>,
    /// Every declared error, by name, with its declaring module (`None` for
    /// application scope). The prelude is implicit and never enters here.
    errors: BTreeMap<&'a str, (&'a ErrorDecl, Option<&'a str>)>,
    /// The resolved `throws` sets of every service operation and repository
    /// operation, keyed by `(owner name, operation name)` — the delegation
    /// superset check reads them.
    service_throws: BTreeMap<(&'a str, &'a str), &'a [ErrorRef]>,
    repository_throws: BTreeMap<(&'a str, &'a str), &'a [ErrorRef]>,
}

impl<'a> Registry<'a> {
    fn build(model: &'a DddModel) -> Self {
        let mut registry = Self::default();
        for error in &model.errors {
            registry.errors.insert(error.name.as_str(), (error, None));
        }
        for module in &model.modules {
            for error in &module.errors {
                registry
                    .errors
                    .entry(error.name.as_str())
                    .or_insert((error, Some(module.name.as_str())));
            }
            for service in &module.services {
                registry
                    .services
                    .insert(service.name.as_str(), module.name.as_str());
                for operation in &service.operations {
                    registry.service_throws.insert(
                        (service.name.as_str(), operation.name.as_str()),
                        &operation.throws,
                    );
                }
            }
            for design in &module.designs {
                // Every design that targets a class contributes its name to
                // the DTO shadowing namespace — including a dto design, whose
                // `class` is its source class (a mirror's boundary name is
                // that same name; the self-collision is exempted at the
                // declaration site).
                if let Some(class) = &design.class {
                    registry.designed_classes.insert(last_segment(class));
                }
                if let Some(shape) = &design.dto_shape {
                    // The first declaration owns the boundary name; a
                    // duplicate is reported at the later declaration.
                    registry.dtos.entry(shape.name.as_str()).or_insert(shape);
                }
                if let Some(repository) = &design.repository {
                    registry
                        .repositories
                        .insert(repository.name.as_str(), module.name.as_str());
                    for operation in &repository.operations {
                        registry.repository_throws.insert(
                            (repository.name.as_str(), operation.name.as_str()),
                            &operation.throws,
                        );
                    }
                }
            }
            for search in &module.searches {
                registry
                    .searches
                    .insert(search.name.as_str(), module.name.as_str());
            }
        }
        registry
    }

    /// The boundary shape declared under `name`, when the artifact carries
    /// one. A `DddTypeRef::Dto` resolves only against this map.
    fn dto(&self, name: &str) -> Option<&'a DtoShape> {
        self.dtos.get(name).copied()
    }

    /// The members of the boundary `name` that no caller can supply (the
    /// derived ones), or `None` when the name is not declared.
    fn dto_derived_members(&self, name: &str) -> Option<Vec<&'a str>> {
        self.dto(name).map(|shape| {
            shape
                .members
                .iter()
                .filter(|member| member.kind == DtoMemberKind::Derive)
                .map(|member| member.name.as_str())
                .collect()
        })
    }

    /// `true` when a design targets a class with this name.
    fn class_is_designed(&self, name: &str) -> bool {
        self.designed_classes.contains(name)
    }

    /// The declared error `name`, with its declaring module (`None` for
    /// application scope).
    fn error(&self, name: &str) -> Option<(&'a ErrorDecl, Option<&'a str>)> {
        self.errors.get(name).copied()
    }

    /// The resolved `throws` set of one operation of a service or
    /// repository (the delegation superset check).
    fn throws_of(&self, kind: TargetKind, owner: &str, operation: &str) -> Option<&'a [ErrorRef]> {
        match kind {
            TargetKind::Service => self.service_throws.get(&(owner, operation)).copied(),
            TargetKind::Repository => self.repository_throws.get(&(owner, operation)).copied(),
            TargetKind::Search => None,
        }
    }

    /// Resolves a dependency or delegation target with the compiler's
    /// precedence: services, then repositories, then searches.
    fn lookup(&self, name: &str) -> Option<(TargetKind, &'a str)> {
        if let Some(module) = self.services.get(name) {
            return Some((TargetKind::Service, *module));
        }
        if let Some(module) = self.repositories.get(name) {
            return Some((TargetKind::Repository, *module));
        }
        self.searches
            .get(name)
            .map(|module| (TargetKind::Search, *module))
    }
}

fn validate_modules(
    model: &DddModel,
    application: Option<&str>,
    registry: &Registry<'_>,
    violations: &mut Vec<Violation>,
) {
    let mut module_names: BTreeSet<&str> = BTreeSet::new();
    let mut service_names: BTreeSet<&str> = BTreeSet::new();
    let mut repository_names: BTreeSet<&str> = BTreeSet::new();
    let mut search_names: BTreeSet<&str> = BTreeSet::new();
    let mut design_keys: BTreeSet<(String, String)> = BTreeSet::new();
    // The boundary name of the first design that declares it, so a duplicate
    // is reported at the later declaration (DD3, pherolang#300).
    let mut dto_declarations: BTreeMap<String, String> = BTreeMap::new();

    for (module_index, module) in model.modules.iter().enumerate() {
        let module_path = format!("modules[{module_index}]");
        if module.name.trim().is_empty() {
            violations.push(Violation::new(
                format!("{module_path}.name"),
                "the module name must be non-empty",
            ));
        } else if !module_names.insert(module.name.as_str()) {
            violations.push(Violation::new(
                format!("{module_path}.name"),
                format!("duplicate module '{}' in the application", module.name),
            ));
        }

        for (service_index, service) in module.services.iter().enumerate() {
            let service_path = format!("{module_path}.services[{service_index}]");
            if !service_names.insert(service.name.as_str()) {
                violations.push(Violation::new(
                    format!("{service_path}.name"),
                    format!("duplicate service '{}' in the application", service.name),
                ));
            }
            validate_service(
                service,
                &service_path,
                &module.name,
                registry,
                application,
                violations,
            );
        }

        for (design_index, design) in module.designs.iter().enumerate() {
            let design_path = format!("{module_path}.designs[{design_index}]");
            // A dto design's `typeRef` is its *source* class, so a projection
            // legitimately coexists with that class's entity design; the
            // one-design-per-class rule therefore covers entity/value
            // designs only (DTO boundary uniqueness is checked by name).
            if design.stereotype != Stereotype::Dto {
                if let Some(TypeRef::Class { package, name }) = &design.type_ref {
                    if !design_keys.insert((package.clone(), name.clone())) {
                        violations.push(Violation::new(
                            format!("{design_path}.typeRef"),
                            format!("duplicate design for class '{package}.{name}'"),
                        ));
                    }
                }
            }
            if let Some(repository) = &design.repository {
                if !repository_names.insert(repository.name.as_str()) {
                    violations.push(Violation::new(
                        format!("{design_path}.repository.name"),
                        format!(
                            "duplicate repository '{}' in the application",
                            repository.name
                        ),
                    ));
                }
            }
            validate_design(
                design,
                &design_path,
                &module.name,
                registry,
                application,
                &mut dto_declarations,
                violations,
            );
        }

        for (search_index, search) in module.searches.iter().enumerate() {
            let search_path = format!("{module_path}.searches[{search_index}]");
            if !search_names.insert(search.name.as_str()) {
                violations.push(Violation::new(
                    format!("{search_path}.name"),
                    format!("duplicate search '{}' in the application", search.name),
                ));
            }
            validate_search(search, &search_path, violations);
        }
    }
}

fn validate_service(
    service: &Service,
    path: &str,
    module: &str,
    registry: &Registry<'_>,
    application: Option<&str>,
    violations: &mut Vec<Violation>,
) {
    if service.dependencies.len() != service.dependency_refs.len() {
        violations.push(Violation::new(
            format!("{path}.dependencyRefs"),
            format!(
                "has {} entries but \"dependencies\" has {}",
                service.dependency_refs.len(),
                service.dependencies.len()
            ),
        ));
    } else {
        for (index, (authored, resolved)) in service
            .dependencies
            .iter()
            .zip(&service.dependency_refs)
            .enumerate()
        {
            check_target_ref(
                violations,
                &format!("{path}.dependencyRefs[{index}]"),
                authored,
                resolved,
                registry,
                application,
            );
        }
    }

    if service.decision_dependencies.len() != service.decision_dependency_refs.len() {
        violations.push(Violation::new(
            format!("{path}.decisionDependencyRefs"),
            format!(
                "has {} entries but \"decisionDependencies\" has {}",
                service.decision_dependency_refs.len(),
                service.decision_dependencies.len()
            ),
        ));
    } else {
        for (index, (authored, resolved)) in service
            .decision_dependencies
            .iter()
            .zip(&service.decision_dependency_refs)
            .enumerate()
        {
            check_decision_ref(
                violations,
                &format!("{path}.decisionDependencyRefs[{index}]"),
                authored,
                resolved,
            );
        }
    }

    let mut operation_names: BTreeSet<&str> = BTreeSet::new();
    for (operation_index, operation) in service.operations.iter().enumerate() {
        let operation_path = format!("{path}.operations[{operation_index}]");
        if !operation_names.insert(operation.name.as_str()) {
            violations.push(Violation::new(
                format!("{operation_path}.name"),
                format!(
                    "duplicate operation '{}' in service '{}'",
                    operation.name, service.name
                ),
            ));
        }
        validate_operation(
            operation,
            &operation_path,
            module,
            registry,
            application,
            false,
            violations,
        );
    }
}

/// Validates one design against the artifact contract.
///
/// The DD1/DD2 dimensions are the resolved class reference and the value
/// semantics; DD3 (pherolang#300) adds the DTO boundary: a `dto` design
/// carries exactly one `dtoShape`, its `class`/`typeRef` agree with the
/// shape's `source` (and are absent for a standalone boundary), and its
/// members' payloads match their kinds. Every other design carries none.
fn validate_design(
    design: &Design,
    path: &str,
    module: &str,
    registry: &Registry<'_>,
    application: Option<&str>,
    dto_declarations: &mut BTreeMap<String, String>,
    violations: &mut Vec<Violation>,
) {
    match design.stereotype {
        Stereotype::Dto => match &design.dto_shape {
            Some(shape) => {
                check_dto_source_refs(design, shape, path, violations);
                validate_dto_shape(shape, path, registry, dto_declarations, violations);
            }
            None => violations.push(Violation::new(
                format!("{path}.dtoShape"),
                "a dto design must carry its boundary shape (\"dtoShape\"); a \
                 hand-edited or truncated artifact lost it",
            )),
        },
        Stereotype::Entity | Stereotype::Value => {
            match &design.class {
                None => violations.push(Violation::new(
                    format!("{path}.class"),
                    "an entity/value design must name the class it targets",
                )),
                Some(class) => check_class_ref(
                    violations,
                    &format!("{path}.typeRef"),
                    class,
                    design.type_ref.as_ref(),
                    "design",
                ),
            }
            if design.dto_shape.is_some() {
                violations.push(Violation::new(
                    format!("{path}.dtoShape"),
                    "only a dto design may carry a boundary shape",
                ));
            }
        }
    }
    validate_semantics(design, path, violations);

    match design.stereotype {
        Stereotype::Entity => {
            if design.flags.non_persistent {
                violations.push(Violation::new(
                    format!("{path}.flags.nonPersistent"),
                    "an entity design cannot be non-persistent",
                ));
            }
        }
        Stereotype::Value | Stereotype::Dto => {
            if design.repository.is_some() {
                violations.push(Violation::new(
                    format!("{path}.repository"),
                    "only an entity design may declare a repository",
                ));
            }
            if design.machine.is_some() {
                violations.push(Violation::new(
                    format!("{path}.machine"),
                    "only an entity design may declare a state machine",
                ));
            }
            if design.flags.scaffold {
                violations.push(Violation::new(
                    format!("{path}.flags.scaffold"),
                    "only an entity design may be scaffolded",
                ));
            }
            if design.flags.auditable {
                violations.push(Violation::new(
                    format!("{path}.flags.auditable"),
                    "only an entity design may be auditable",
                ));
            }
            if design.flags.optimistic_locking {
                violations.push(Violation::new(
                    format!("{path}.flags.optimisticLocking"),
                    "only an entity design may use optimistic locking",
                ));
            }
            if !design.child_operations.is_empty() {
                violations.push(Violation::new(
                    format!("{path}.childOperations"),
                    "only an entity design may carry child-scoped operations",
                ));
            }
        }
    }

    if let Some(repository) = &design.repository {
        validate_repository(
            repository,
            &format!("{path}.repository"),
            module,
            registry,
            violations,
        );
    }

    for (index, operation) in design.child_operations.iter().enumerate() {
        validate_operation(
            operation,
            &format!("{path}.childOperations[{index}]"),
            module,
            registry,
            application,
            true,
            violations,
        );
    }
}

/// The value-semantics contract (DD2, pherolang#299): a `value` design
/// carries **exactly** the fixed strict block, and an `entity`/`dto` design
/// carries none. Every violation is reported at the design's `semantics`
/// path, so a hand-edited artifact is diagnosable without the compiler.
fn validate_semantics(design: &Design, path: &str, violations: &mut Vec<Violation>) {
    let semantics_path = format!("{path}.semantics");
    match design.stereotype {
        Stereotype::Value => match &design.semantics {
            None => violations.push(Violation::new(
                semantics_path,
                "a value design must carry the strict value semantics block \
                 (identity \"none\", immutable, deepImmutable, structural equality)",
            )),
            Some(semantics) => {
                for (field, held) in [
                    (
                        "identity \"none\"",
                        semantics.identity == IdentityPolicy::None,
                    ),
                    ("immutable: true", semantics.immutable),
                    ("deepImmutable: true", semantics.deep_immutable),
                    (
                        "equality \"structural\"",
                        semantics.equality == EqualityPolicy::Structural,
                    ),
                ] {
                    if !held {
                        violations.push(Violation::new(
                            &semantics_path,
                            format!("the value semantics block must declare {field}"),
                        ));
                    }
                }
            }
        },
        Stereotype::Entity | Stereotype::Dto => {
            if design.semantics.is_some() {
                violations.push(Violation::new(
                    semantics_path,
                    "only a value design may carry value semantics",
                ));
            }
        }
    }
}

/// The `class`/`typeRef` half of the DTO contract (DD3, pherolang#300).
///
/// A DTO with a `source` names that source class in `class` and resolves it
/// in `typeRef`; a standalone DTO (no source) names no class at all, so both
/// keys are absent. The check reads the **shape's** source, which is the
/// boundary's own declaration of where it came from.
fn check_dto_source_refs(
    design: &Design,
    shape: &DtoShape,
    path: &str,
    violations: &mut Vec<Violation>,
) {
    let Some(source) = &shape.source else {
        if design.class.is_some() {
            violations.push(Violation::new(
                format!("{path}.class"),
                "a standalone dto (no boundary source) must not carry \"class\"",
            ));
        }
        if design.type_ref.is_some() {
            violations.push(Violation::new(
                format!("{path}.typeRef"),
                "a standalone dto (no boundary source) must not carry \"typeRef\"",
            ));
        }
        return;
    };
    let source_name = class_name(source);
    match &design.class {
        None => violations.push(Violation::new(
            format!("{path}.class"),
            format!("a dto sourced from '{source_name}' must carry that class as \"class\""),
        )),
        Some(class) if last_segment(class) != source_name => violations.push(Violation::new(
            format!("{path}.class"),
            format!(
                "names '{}' but the dto boundary's source is '{source_name}'",
                last_segment(class)
            ),
        )),
        Some(_) => {}
    }
    match &design.type_ref {
        None => violations.push(Violation::new(
            format!("{path}.typeRef"),
            format!("missing resolved class for the dto source '{source_name}'"),
        )),
        Some(resolved) if resolved != source => violations.push(Violation::new(
            format!("{path}.typeRef"),
            format!(
                "resolves to '{}' but the dto boundary's source is '{source_name}'",
                type_ref_label(resolved)
            ),
        )),
        Some(_) => {}
    }
}

/// The boundary half of the DTO contract (DD3, pherolang#300): the boundary
/// `name` (non-empty, unique application-wide, never shadowing a designed
/// class), the derived `direction`, the member names and their kind/payload
/// consistency, and every member's `DddTypeRef` resolving to a boundary or
/// staying in the (structurally opaque) domain namespace.
fn validate_dto_shape(
    shape: &DtoShape,
    design_path: &str,
    registry: &Registry<'_>,
    dto_declarations: &mut BTreeMap<String, String>,
    violations: &mut Vec<Violation>,
) {
    let path = format!("{design_path}.dtoShape");
    if let Some(source) = &shape.source {
        if !matches!(source, TypeRef::Class { .. }) {
            violations.push(Violation::new(
                format!("{path}.source"),
                "the dto boundary source must be a resolved class type",
            ));
        }
    }
    if shape.name.trim().is_empty() {
        violations.push(Violation::new(
            format!("{path}.name"),
            "the dto boundary name must be non-empty",
        ));
    } else {
        match dto_declarations.get(shape.name.as_str()).cloned() {
            Some(first) if first != design_path => violations.push(Violation::new(
                format!("{path}.name"),
                format!(
                    "duplicate dto boundary '{}'; it is already declared at {first}",
                    shape.name
                ),
            )),
            Some(_) => {}
            None => {
                dto_declarations.insert(shape.name.clone(), design_path.to_string());
            }
        }
        // A mirror boundary's name *is* its source class, so a collision with
        // that class is the boundary referring to itself, not a shadow.
        let self_collision = shape
            .source
            .as_ref()
            .is_some_and(|source| class_name(source) == shape.name);
        if !self_collision && registry.class_is_designed(shape.name.as_str()) {
            violations.push(Violation::new(
                format!("{path}.name"),
                format!(
                    "dto boundary '{}' shadows a designed class; a boundary name must be its \
                     own type name",
                    shape.name
                ),
            ));
        }
    }

    let derived = shape
        .members
        .iter()
        .any(|member| member.kind == DtoMemberKind::Derive);
    let expected = if derived {
        DtoDirection::Output
    } else {
        DtoDirection::Input
    };
    if shape.direction != expected {
        violations.push(Violation::new(
            format!("{path}.direction"),
            format!(
                "must be \"{}\": the direction is derived from the members, and the \
                 boundary {}",
                direction_label(expected),
                if derived {
                    "has a derive member"
                } else {
                    "has no derive member"
                }
            ),
        ));
    }

    let mut member_names: BTreeSet<String> = BTreeSet::new();
    for (index, member) in shape.members.iter().enumerate() {
        let member_path = format!("{path}.members[{index}]");
        validate_dto_member(member, &member_path, &mut member_names, violations);
        check_ddd_type_ref(
            violations,
            &format!("{member_path}.type"),
            &member.type_,
            registry,
        );
    }
}

fn validate_dto_member(
    member: &DtoMember,
    path: &str,
    member_names: &mut BTreeSet<String>,
    violations: &mut Vec<Violation>,
) {
    if member.name.trim().is_empty() {
        violations.push(Violation::new(
            format!("{path}.name"),
            "the dto member name must be non-empty",
        ));
    } else if !member_names.insert(member.name.clone()) {
        violations.push(Violation::new(
            format!("{path}.name"),
            format!("duplicate dto member '{}'", member.name),
        ));
    }
    match member.kind {
        DtoMemberKind::Select => {
            if member.path.is_empty() {
                violations.push(Violation::new(
                    format!("{path}.path"),
                    "a select member must carry the selected feature path",
                ));
            }
            if member.expression.is_some() {
                violations.push(Violation::new(
                    format!("{path}.expression"),
                    "a select member must not carry a derivation expression",
                ));
            }
        }
        DtoMemberKind::Derive => {
            match member.expression.as_deref() {
                None => violations.push(Violation::new(
                    format!("{path}.expression"),
                    "a derive member must carry the derivation's source text",
                )),
                Some(expression) if expression.trim().is_empty() => {
                    violations.push(Violation::new(
                        format!("{path}.expression"),
                        "the derivation's source text must be non-empty",
                    ));
                }
                Some(_) => {}
            }
            if !member.path.is_empty() {
                violations.push(Violation::new(
                    format!("{path}.path"),
                    "a derive member must not carry a selected feature path",
                ));
            }
        }
        DtoMemberKind::Declared => {
            if !member.path.is_empty() {
                violations.push(Violation::new(
                    format!("{path}.path"),
                    "a declared member must not carry a selected feature path",
                ));
            }
            if member.expression.is_some() {
                violations.push(Violation::new(
                    format!("{path}.expression"),
                    "a declared member must not carry a derivation expression",
                ));
            }
        }
    }
}

/// Resolves one DDD signature/member type slot (DD3, pherolang#300): a
/// domain reference is structurally opaque (the paired domain `Model` owns
/// its identity), while a DTO reference must name a boundary declared in
/// this artifact.
fn check_ddd_type_ref(
    violations: &mut Vec<Violation>,
    path: &str,
    type_: &DddTypeRef,
    registry: &Registry<'_>,
) {
    if let DddTypeRef::Dto { dto } = type_ {
        if registry.dto(dto).is_none() {
            violations.push(Violation::new(
                path,
                format!(
                    "references dto '{dto}' but no dto design in this artifact declares that \
                     boundary"
                ),
            ));
        }
    }
}

/// Checks one operation parameter's DTO slot: the boundary must resolve,
/// and it must be input-capable — a boundary with derived members has values
/// only the source class can compute, so a caller cannot supply it.
fn check_dto_param(
    violations: &mut Vec<Violation>,
    path: &str,
    param: &DddParam,
    registry: &Registry<'_>,
) {
    check_ddd_type_ref(violations, path, &param.type_, registry);
    let DddTypeRef::Dto { dto } = &param.type_ else {
        return;
    };
    let Some(derived) = registry.dto_derived_members(dto) else {
        return;
    };
    if !derived.is_empty() {
        violations.push(Violation::new(
            path,
            format!(
                "references dto '{dto}' as parameter '{}', but that boundary is output-only; \
                 derived member(s): {}",
                param.name,
                derived.join(", ")
            ),
        ));
    }
}

/// The `Derive`/`Input` wire spellings, for violation messages.
fn direction_label(direction: DtoDirection) -> &'static str {
    match direction {
        DtoDirection::Input => "input",
        DtoDirection::Output => "output",
    }
}

/// The class name of a resolved reference, or its wire spelling when the
/// reference is not a class (which [`check_class_ref`] reports separately).
fn class_name(type_ref: &TypeRef) -> &str {
    match type_ref {
        TypeRef::Class { name, .. } => name,
        _ => "",
    }
}

/// A resolved reference rendered for a message.
fn type_ref_label(type_ref: &TypeRef) -> String {
    match type_ref {
        TypeRef::Class { package, name } => format!("{package}.{name}"),
        other => format!("{other:?}"),
    }
}

/// Validates a repository's operations, including the DTO slots of each
/// declared signature (DD3, pherolang#300).
fn validate_repository(
    repository: &Repository,
    path: &str,
    module: &str,
    registry: &Registry<'_>,
    violations: &mut Vec<Violation>,
) {
    let mut operation_names: BTreeSet<&str> = BTreeSet::new();
    for (index, operation) in repository.operations.iter().enumerate() {
        let operation_path = format!("{path}.operations[{index}]");
        if !operation_names.insert(operation.name.as_str()) {
            violations.push(Violation::new(
                format!("{operation_path}.name"),
                format!(
                    "duplicate operation '{}' in repository '{}'",
                    operation.name, repository.name
                ),
            ));
        }
        validate_repository_operation(operation, &operation_path, module, registry, violations);
    }
}

fn validate_repository_operation(
    operation: &RepositoryOperation,
    path: &str,
    module: &str,
    registry: &Registry<'_>,
    violations: &mut Vec<Violation>,
) {
    if let Some(return_type) = &operation.return_type {
        check_ddd_type_ref(
            violations,
            &format!("{path}.returnType"),
            return_type,
            registry,
        );
    }
    for (index, param) in operation.params.iter().enumerate() {
        check_dto_param(
            violations,
            &format!("{path}.params[{index}].type"),
            param,
            registry,
        );
    }
    let declared = operation.return_type.is_some() || !operation.params.is_empty();
    match (operation.builtin.is_some(), declared) {
        (true, true) => violations.push(Violation::new(
            path,
            "a repository operation cannot carry both a builtin and a declared signature",
        )),
        (false, false) => violations.push(Violation::new(
            path,
            "a repository operation must carry a builtin or a declared signature",
        )),
        _ => {}
    }
    if operation.return_multiplicity.is_some() && operation.return_type.is_none() {
        violations.push(Violation::new(
            format!("{path}.returnMultiplicity"),
            "a return multiplicity requires a return type",
        ));
    }
    if operation.builtin.is_some() && operation.condition.is_some() {
        violations.push(Violation::new(
            format!("{path}.condition"),
            "a builtin repository operation cannot carry a condition",
        ));
    }
    if operation.builtin.is_some() && !operation.bodies.is_empty() {
        violations.push(Violation::new(
            format!("{path}.bodies"),
            "a builtin repository operation cannot carry a body",
        ));
    }
    if operation.builtin.is_some() && !operation.throws.is_empty() {
        violations.push(Violation::new(
            format!("{path}.throws"),
            "a builtin repository operation takes no `throws` clause; its failures are \
             structural, not declared",
        ));
    }
    validate_throws(
        &operation.throws,
        module,
        &format!("{path}.throws"),
        registry,
        violations,
    );
    validate_body_raises(
        &operation.bodies,
        &operation.throws,
        &format!("{path}.bodies.stmt"),
        violations,
    );
}

fn validate_operation(
    operation: &ServiceOperation,
    path: &str,
    module: &str,
    registry: &Registry<'_>,
    application: Option<&str>,
    is_child: bool,
    violations: &mut Vec<Violation>,
) {
    if operation.return_multiplicity.is_some() && operation.return_type.is_none() {
        violations.push(Violation::new(
            format!("{path}.returnMultiplicity"),
            "a return multiplicity requires a return type",
        ));
    }
    if let Some(return_type) = &operation.return_type {
        check_ddd_type_ref(
            violations,
            &format!("{path}.returnType"),
            return_type,
            registry,
        );
    }
    for (index, param) in operation.params.iter().enumerate() {
        check_dto_param(
            violations,
            &format!("{path}.params[{index}].type"),
            param,
            registry,
        );
    }
    if let Some(delegation) = &operation.delegation {
        match &delegation.target_ref {
            Some(target) => {
                check_target_ref(
                    violations,
                    &format!("{path}.delegation.targetRef"),
                    &delegation.target,
                    target,
                    registry,
                    application,
                );
                if operation.transition.is_some() {
                    let resolved = registry.lookup(last_segment(&delegation.target));
                    if resolved.map(|(kind, _)| kind) != Some(TargetKind::Repository)
                        || delegation.operation != "save"
                    {
                        violations.push(Violation::new(
                            format!("{path}.transition"),
                            "a transition binds only a delegation to a repository save",
                        ));
                    }
                }
            }
            None => violations.push(Violation::new(
                format!("{path}.delegation.targetRef"),
                format!(
                    "missing resolved target for the delegation to '{}'",
                    delegation.target
                ),
            )),
        }
        if operation.return_type.is_some()
            || !operation.params.is_empty()
            || operation.return_multiplicity.is_some()
        {
            violations.push(Violation::new(
                path,
                "a delegation cannot also declare a return type, multiplicity, or parameters",
            ));
        }
    }
    if operation.transition.is_some() && operation.delegation.is_none() {
        violations.push(Violation::new(
            format!("{path}.transition"),
            "a transition requires a delegation",
        ));
    }
    if let Some(transition) = &operation.transition {
        if transition
            .machine
            .as_deref()
            .is_some_and(|machine| machine.trim().is_empty())
        {
            violations.push(Violation::new(
                format!("{path}.transition.machine"),
                "the machine qualifier must be non-empty when present",
            ));
        }
    }
    match (is_child, operation.child_scope.is_some()) {
        (true, false) => violations.push(Violation::new(
            format!("{path}.childScope"),
            "a child-scoped operation must name its child scope",
        )),
        (false, true) => violations.push(Violation::new(
            format!("{path}.childScope"),
            "a service operation cannot carry a child scope",
        )),
        _ => {}
    }
    if operation.body_receiver.is_some() && operation.bodies.is_empty() {
        violations.push(Violation::new(
            format!("{path}.bodyReceiver"),
            "a body receiver requires an executable body",
        ));
    }

    validate_throws(
        &operation.throws,
        module,
        &format!("{path}.throws"),
        registry,
        violations,
    );
    validate_body_raises(
        &operation.bodies,
        &operation.throws,
        &format!("{path}.bodies.stmt"),
        violations,
    );
    if let Some(delegation) = &operation.delegation {
        validate_delegation_throws(operation, delegation, path, registry, violations);
    }

    if operation.capabilities.len() != operation.capability_refs.len() {
        violations.push(Violation::new(
            format!("{path}.capabilityRefs"),
            format!(
                "has {} entries but \"capabilities\" has {}",
                operation.capability_refs.len(),
                operation.capabilities.len()
            ),
        ));
    } else {
        for (index, (authored, resolved)) in operation
            .capabilities
            .iter()
            .zip(&operation.capability_refs)
            .enumerate()
        {
            let ref_path = format!("{path}.capabilityRefs[{index}]");
            if resolved.name.as_str() != authored {
                violations.push(Violation::new(
                    &ref_path,
                    format!(
                        "names '{}' but the capabilities entry is '{authored}'",
                        resolved.name
                    ),
                ));
            }
            if resolved
                .declared_in
                .as_deref()
                .is_some_and(|declared_in| declared_in.trim().is_empty())
            {
                violations.push(Violation::new(
                    &ref_path,
                    "the declaring block must be non-empty when present",
                ));
            }
        }
    }

    if operation.emits.len() != operation.emitted_refs.len() {
        violations.push(Violation::new(
            format!("{path}.emittedRefs"),
            format!(
                "has {} entries but \"emits\" has {}",
                operation.emitted_refs.len(),
                operation.emits.len()
            ),
        ));
    } else {
        for (index, (authored, resolved)) in operation
            .emits
            .iter()
            .zip(&operation.emitted_refs)
            .enumerate()
        {
            check_event_ref(
                violations,
                &format!("{path}.emittedRefs[{index}]"),
                authored,
                resolved,
            );
        }
    }

    if operation.invokes.len() != operation.decision_refs.len() {
        violations.push(Violation::new(
            format!("{path}.decisionRefs"),
            format!(
                "has {} entries but \"invokes\" has {}",
                operation.decision_refs.len(),
                operation.invokes.len()
            ),
        ));
    } else {
        for (index, (authored, resolved)) in operation
            .invokes
            .iter()
            .zip(&operation.decision_refs)
            .enumerate()
        {
            check_decision_ref(
                violations,
                &format!("{path}.decisionRefs[{index}]"),
                authored,
                resolved,
            );
        }
    }
}

/// Resolves every `throws` reference against the artifact's declared
/// vocabulary (DD7): an application-scoped reference names an error declared
/// at the root, a module-scoped reference names an error declared in exactly
/// the named module, and a prelude reference names a prelude error.
fn validate_throws(
    throws: &[ErrorRef],
    module: &str,
    path: &str,
    registry: &Registry<'_>,
    violations: &mut Vec<Violation>,
) {
    for (index, reference) in throws.iter().enumerate() {
        let ref_path = format!("{path}[{index}]");
        match reference.scope {
            ErrorScope::Prelude => {
                if !is_prelude_error(&reference.name) {
                    violations.push(Violation::new(
                        format!("{ref_path}.name"),
                        format!(
                            "declares the prelude scope but '{}' is not a prelude error \
                             (`NotFound`, `Conflict`)",
                            reference.name
                        ),
                    ));
                }
                if reference.module.is_some() {
                    violations.push(Violation::new(
                        format!("{ref_path}.module"),
                        "a prelude error reference carries no module",
                    ));
                }
            }
            ErrorScope::Application => match registry.error(&reference.name) {
                Some((_, None)) => {
                    if reference.module.is_some() {
                        violations.push(Violation::new(
                            format!("{ref_path}.module"),
                            "an application-scoped error reference carries no module",
                        ));
                    }
                }
                Some((_, Some(declaring))) => violations.push(Violation::new(
                    format!("{ref_path}.scope"),
                    format!(
                        "declares the application scope but '{}' is declared in module \
                         '{declaring}'",
                        reference.name
                    ),
                )),
                None => violations.push(Violation::new(
                    format!("{ref_path}.name"),
                    format!(
                        "'{}' resolves to no declared error and is not a prelude error",
                        reference.name
                    ),
                )),
            },
            ErrorScope::Module => match registry.error(&reference.name) {
                Some((_, Some(declaring))) => {
                    if reference.module.as_deref() != Some(declaring) {
                        violations.push(Violation::new(
                            format!("{ref_path}.module"),
                            format!(
                                "declares module '{}' but '{}' is private to module \
                                 '{declaring}'",
                                reference.module.as_deref().unwrap_or("<absent>"),
                                reference.name
                            ),
                        ));
                    }
                }
                Some((_, None)) => violations.push(Violation::new(
                    format!("{ref_path}.scope"),
                    format!(
                        "declares the module scope but '{}' is application-scoped",
                        reference.name
                    ),
                )),
                None => violations.push(Violation::new(
                    format!("{ref_path}.name"),
                    format!(
                        "'{}' resolves to no declared error and is not a prelude error",
                        reference.name
                    ),
                )),
            },
        }
        let _ = module;
    }
}

/// The artifact-only subset rule (DD7): every raise the checked statement
/// body carries (`bodies["stmt"]`, canonical [`crate::ddd`] statement JSON)
/// must appear in the operation's `throws`. The body is typed statements,
/// never source text — the walk reads the `variant` of every raise node.
fn validate_body_raises(
    bodies: &BTreeMap<String, String>,
    throws: &[ErrorRef],
    path: &str,
    violations: &mut Vec<Violation>,
) {
    let Some(body) = bodies.get("stmt") else {
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        violations.push(Violation::new(
            path,
            "the statement body is not canonical StmtBody JSON",
        ));
        return;
    };
    let mut raised = Vec::new();
    collect_raise_variants(&value, &mut raised);
    for name in raised {
        if !throws.iter().any(|reference| reference.name == name) {
            let declared: Vec<&str> = throws
                .iter()
                .map(|reference| reference.name.as_str())
                .collect();
            violations.push(Violation::new(
                path,
                if declared.is_empty() {
                    format!(
                        "the body raises '{name}' but the operation declares no `throws` \
                         clause"
                    )
                } else {
                    format!(
                        "the body raises '{name}' but the operation's throws declares: {}",
                        declared.join(", ")
                    )
                },
            ));
        }
    }
}

/// Collects the `error.variant` names of every raise node in one statement
/// body, recursing through `if` branches (the wire shape's `then`/`else`
/// arrays).
fn collect_raise_variants(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::Object(error)) = map.get("error") {
                if let Some(serde_json::Value::String(variant)) = error.get("variant") {
                    out.push(variant.clone());
                }
            }
            for nested in map.values() {
                collect_raise_variants(nested, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_raise_variants(item, out);
            }
        }
        _ => {}
    }
}

/// A delegation is contract-preserving (DD7): its `throws` must be a
/// superset of the delegated operation's. The target's set comes from the
/// same artifact (the registry indexes every service and repository
/// operation).
fn validate_delegation_throws(
    operation: &ServiceOperation,
    delegation: &Delegation,
    path: &str,
    registry: &Registry<'_>,
    violations: &mut Vec<Violation>,
) {
    let name = last_segment(&delegation.target);
    let Some((kind, _)) = registry.lookup(name) else {
        return;
    };
    let Some(target_throws) = registry.throws_of(kind, name, &delegation.operation) else {
        return;
    };
    for reference in target_throws {
        if !operation
            .throws
            .iter()
            .any(|declared| declared.name == reference.name)
        {
            violations.push(Violation::new(
                format!("{path}.throws"),
                format!(
                    "the delegation to '{name}.{}' narrows the target's throws: missing \
                     '{}'",
                    delegation.operation, reference.name
                ),
            ));
        }
    }
}

fn validate_search(search: &SearchDef, path: &str, violations: &mut Vec<Violation>) {
    check_class_ref(
        violations,
        &format!("{path}.entityRef"),
        &search.entity,
        search.entity_ref.as_ref(),
        "search",
    );

    if search.capabilities.len() != search.capability_refs.len() {
        violations.push(Violation::new(
            format!("{path}.capabilityRefs"),
            format!(
                "has {} entries but \"capabilities\" has {}",
                search.capability_refs.len(),
                search.capabilities.len()
            ),
        ));
    } else {
        for (index, (authored, resolved)) in search
            .capabilities
            .iter()
            .zip(&search.capability_refs)
            .enumerate()
        {
            let ref_path = format!("{path}.capabilityRefs[{index}]");
            if resolved.name.as_str() != authored {
                violations.push(Violation::new(
                    &ref_path,
                    format!(
                        "names '{}' but the capabilities entry is '{authored}'",
                        resolved.name
                    ),
                ));
            }
            if resolved
                .declared_in
                .as_deref()
                .is_some_and(|declared_in| declared_in.trim().is_empty())
            {
                violations.push(Violation::new(
                    &ref_path,
                    "the declaring block must be non-empty when present",
                ));
            }
        }
    }

    for (index, field) in search.text.iter().enumerate() {
        if field.type_ref.is_none() {
            violations.push(Violation::new(
                format!("{path}.text[{index}].typeRef"),
                format!("missing resolved type for text field '{}'", field.property),
            ));
        }
    }
    for (index, filter) in search.filters.iter().enumerate() {
        if filter.type_ref.is_none() {
            violations.push(Violation::new(
                format!("{path}.filters[{index}].typeRef"),
                format!("missing resolved type for filter '{}'", filter.property),
            ));
        }
    }
    for (index, sort) in search.sort.iter().enumerate() {
        if sort.type_ref.is_none() {
            violations.push(Violation::new(
                format!("{path}.sort[{index}].typeRef"),
                format!("missing resolved type for sort '{}'", sort.property),
            ));
        }
    }
}

fn check_class_ref(
    violations: &mut Vec<Violation>,
    path: &str,
    authored: &str,
    resolved: Option<&TypeRef>,
    owner: &str,
) {
    match resolved {
        None => violations.push(Violation::new(
            path,
            format!(
                "missing resolved class for {owner} '{}'",
                last_segment(authored)
            ),
        )),
        Some(TypeRef::Class { package, name }) => {
            if name != last_segment(authored) {
                violations.push(Violation::new(
                    path,
                    format!(
                        "resolves to '{package}.{name}' but the authored {owner} is '{}'",
                        last_segment(authored)
                    ),
                ));
            }
        }
        Some(_) => violations.push(Violation::new(
            path,
            "the resolved reference must be a class type",
        )),
    }
}

fn check_target_ref(
    violations: &mut Vec<Violation>,
    path: &str,
    authored: &str,
    target: &TargetRef,
    registry: &Registry<'_>,
    application: Option<&str>,
) {
    let name = last_segment(authored);
    let Some((kind, module)) = registry.lookup(name) else {
        violations.push(Violation::new(
            path,
            format!(
                "'{name}' does not resolve to a service, repository, or search in this artifact"
            ),
        ));
        return;
    };
    if target.kind != kind {
        violations.push(Violation::new(
            path,
            format!(
                "declares kind '{}' but '{name}' is a {}",
                kind_label(target.kind),
                kind_label(kind)
            ),
        ));
    }
    if target.module != module {
        violations.push(Violation::new(
            path,
            format!(
                "declares module '{}' but '{name}' is declared in '{module}'",
                target.module
            ),
        ));
    }
    if let Some(application) = application {
        let expected = format!("{application}.{module}.{name}");
        if target.qualified_name != expected {
            violations.push(Violation::new(
                path,
                format!(
                    "qualified name '{}' does not match '{expected}'",
                    target.qualified_name
                ),
            ));
        }
    }
}

fn check_event_ref(
    violations: &mut Vec<Violation>,
    path: &str,
    authored: &str,
    resolved: &EventRef,
) {
    if resolved.name.as_str() != authored {
        violations.push(Violation::new(
            path,
            format!(
                "names '{}' but the emitted event is '{authored}'",
                resolved.name
            ),
        ));
    }
    if resolved
        .contract
        .as_deref()
        .is_some_and(|contract| contract.trim().is_empty())
    {
        violations.push(Violation::new(
            path,
            "the event contract must be non-empty when present",
        ));
    }
}

fn check_decision_ref(
    violations: &mut Vec<Violation>,
    path: &str,
    authored: &str,
    resolved: &DecisionRef,
) {
    if resolved.service.as_str() != authored {
        violations.push(Violation::new(
            path,
            format!(
                "names '{}' but the referenced decision service is '{authored}'",
                resolved.service
            ),
        ));
    }
    for (field, value) in [
        ("contract", resolved.contract.as_deref()),
        ("version", resolved.version.as_deref()),
    ] {
        if value.is_some_and(|value| value.trim().is_empty()) {
            violations.push(Violation::new(
                format!("{path}.{field}"),
                format!("the decision {field} must be non-empty when present"),
            ));
        }
    }
}

fn kind_label(kind: TargetKind) -> &'static str {
    match kind {
        TargetKind::Service => "service",
        TargetKind::Repository => "repository",
        TargetKind::Search => "search",
    }
}

/// The last `.`/`::`-separated segment of an authored or resolved name.
fn last_segment(name: &str) -> &str {
    name.rsplit(['.', ':'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(name)
}
