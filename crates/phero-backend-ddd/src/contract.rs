//! The artifact-only `.ddd` contract.
//!
//! The compiler is the source of truth for these rules; this module repeats
//! them over a serialized [`DddModel`] so a consumer can prove a published
//! artifact is contract-complete without the `.mox` sources. Every
//! violation carries the JSON path of the offending value, so a hand-edited
//! artifact is diagnosable without a compiler session.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use phero_ir::ddd::{
    DddModel, DecisionRef, Design, EventRef, Repository, RepositoryOperation, SearchDef, Service,
    ServiceOperation, Stereotype, TargetKind, TargetRef, DDD_MODEL_FORMAT_VERSION,
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
    validate_modules(model, application_name, &registry, &mut violations);
    violations
}

/// Every declared service, repository, and search, by name, with the module
/// that declares it. The compiler guarantees the names are unique within
/// their kind, so a plain map is enough for resolution.
#[derive(Default)]
struct Registry<'a> {
    services: BTreeMap<&'a str, &'a str>,
    repositories: BTreeMap<&'a str, &'a str>,
    searches: BTreeMap<&'a str, &'a str>,
}

impl<'a> Registry<'a> {
    fn build(model: &'a DddModel) -> Self {
        let mut registry = Self::default();
        for module in &model.modules {
            for service in &module.services {
                registry
                    .services
                    .insert(service.name.as_str(), module.name.as_str());
            }
            for design in &module.designs {
                if let Some(repository) = &design.repository {
                    registry
                        .repositories
                        .insert(repository.name.as_str(), module.name.as_str());
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
            validate_service(service, &service_path, registry, application, violations);
        }

        for (design_index, design) in module.designs.iter().enumerate() {
            let design_path = format!("{module_path}.designs[{design_index}]");
            if let Some(TypeRef::Class { package, name }) = &design.type_ref {
                if !design_keys.insert((package.clone(), name.clone())) {
                    violations.push(Violation::new(
                        format!("{design_path}.typeRef"),
                        format!("duplicate design for class '{package}.{name}'"),
                    ));
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
            validate_design(design, &design_path, registry, application, violations);
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
            registry,
            application,
            false,
            violations,
        );
    }
}

fn validate_design(
    design: &Design,
    path: &str,
    registry: &Registry<'_>,
    application: Option<&str>,
    violations: &mut Vec<Violation>,
) {
    check_class_ref(
        violations,
        &format!("{path}.typeRef"),
        &design.class,
        design.type_ref.as_ref(),
        "design",
    );

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
        validate_repository(repository, &format!("{path}.repository"), violations);
    }

    for (index, operation) in design.child_operations.iter().enumerate() {
        validate_operation(
            operation,
            &format!("{path}.childOperations[{index}]"),
            registry,
            application,
            true,
            violations,
        );
    }
}

fn validate_repository(repository: &Repository, path: &str, violations: &mut Vec<Violation>) {
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
        validate_repository_operation(operation, &operation_path, violations);
    }
}

fn validate_repository_operation(
    operation: &RepositoryOperation,
    path: &str,
    violations: &mut Vec<Violation>,
) {
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
}

fn validate_operation(
    operation: &ServiceOperation,
    path: &str,
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
