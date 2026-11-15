//! The resolved-identity post-pass (DD1, pherolang#298): fills every
//! artifact companion that needs application-wide knowledge.
//!
//! Each loose reference in the design surface keeps its authored spelling
//! and gains a **resolved companion** beside it — `Design::type_ref`,
//! `Delegation::target_ref`, `Service::dependency_refs` /
//! `decision_dependency_refs`, `ServiceOperation::capability_refs` /
//! `emitted_refs` / `decision_refs`, `SearchDef::entity_ref` /
//! `capability_refs`, and the search members' `type_ref`. The companions
//! whose resolution is local to a lowering stage fill inline (designs,
//! searches); the ones below need the **completed** registries (a service
//! may inject a repository declared later in the file) or one of the
//! entry point's artifact buckets (`.actor` capabilities, `.dmn` decision
//! services), so they fill here — after rule 17, before `engine_finish`.
//!
//! The wire contract is `docs/DDD.md` (resolved companions): every authored
//! reference a consumer acts on carries its companion, the parallel lists
//! align index-for-index, and an optional provenance member stays absent
//! exactly when its counterpart artifact was not supplied. A companion is
//! left absent only when the lookup genuinely failed, which happens only on
//! an error-severity compile — and those artifacts are dropped by
//! `engine_finish`, so a published artifact is complete.

use std::collections::BTreeMap;

use phero_ir::ddd::{CapabilityRef, DecisionRef, EventRef, TargetKind, TargetRef};
use phero_ir::dmn::VersionPin;
use phero_syntax::ast as dsl;

use super::services::{declared_version, resolve_decision_service, DecisionResolution};
use super::Registries;

/// Fills every resolved companion of `model` from the completed
/// application registries and the entry point's artifact buckets.
pub(crate) fn fill_resolved_links(
    model: &mut phero_ir::ddd::DddModel,
    registries: &Registries<'_>,
    actors: Option<&[phero_ir::ActorModel]>,
    decisions: &[(String, phero_ir::dmn::DmnModel)],
) {
    let application = model
        .application
        .as_ref()
        .map(|application| application.name.clone());
    let targets = TargetTable::new(registries, application);
    let declared = declared_capabilities(actors);

    for module in &mut model.modules {
        for service in &mut module.services {
            service.dependency_refs = service
                .dependencies
                .iter()
                .filter_map(|dependency| targets.lookup(dependency))
                .collect();
            service.decision_dependency_refs = decision_refs(
                &service.decision_dependencies,
                &service.decision_dependency_pins,
                decisions,
            );
            for operation in &mut service.operations {
                fill_operation(operation, &targets, &declared, decisions);
            }
        }
        for design in &mut module.designs {
            // Child-scoped operations carry no capability/event/decision
            // clauses and no delegation, so this only fills what exists.
            for operation in &mut design.child_operations {
                fill_operation(operation, &targets, &declared, decisions);
            }
            // DD9: a repository query's `capability` clause is the read
            // authorization; its companion resolves like a service op's.
            if let Some(repository) = &mut design.repository {
                for operation in &mut repository.operations {
                    if operation.capabilities.is_empty() {
                        continue;
                    }
                    let names: Vec<String> = operation
                        .capabilities
                        .iter()
                        .map(|capability| capability.name.clone())
                        .collect();
                    operation.capabilities = capability_refs(&names, &declared);
                }
            }
        }
        for search in &mut module.searches {
            search.capability_refs = capability_refs(&search.capabilities, &declared);
        }
    }
}

/// Fills one service operation's companions: the delegation target, the
/// guarding capabilities, the emitted events, and the invoked decision
/// services.
fn fill_operation(
    operation: &mut phero_ir::ddd::ServiceOperation,
    targets: &TargetTable<'_>,
    declared: &BTreeMap<String, String>,
    decisions: &[(String, phero_ir::dmn::DmnModel)],
) {
    if let Some(delegation) = &mut operation.delegation {
        delegation.target_ref = targets.lookup(&delegation.target);
    }
    operation.capability_refs = capability_refs(&operation.capabilities, declared);
    // The `.ddd` compile has no `.evt` bucket, so event references are
    // unresolved by design: the name rides along, `contract` stays absent.
    operation.emitted_refs = operation
        .emits
        .iter()
        .map(|name| EventRef::new(name.as_str()))
        .collect();
    operation.decision_refs =
        decision_refs(&operation.invokes, &operation.decision_pins, decisions);
}

/// The `{kind, qualifiedName, module}` table the delegation and dependency
/// companions resolve through: every service, repository, and search
/// declared anywhere in the application, in the compiler's resolution
/// precedence (services, then repositories, then searches) — the order
/// rules 8 and 9 use.
struct TargetTable<'a> {
    application: Option<String>,
    services: &'a [(String, super::ServiceEntry)],
    repositories: &'a [(String, super::RepositoryEntry)],
    searches: &'a [(String, super::SearchEntry)],
}

impl<'a> TargetTable<'a> {
    fn new(registries: &'a Registries<'_>, application: Option<String>) -> Self {
        Self {
            application,
            services: &registries.services,
            repositories: &registries.repositories,
            searches: &registries.searches,
        }
    }

    /// The resolved identity of `name`, or `None` when nothing declares it
    /// (which only happens on an error-severity compile).
    fn lookup(&self, name: &str) -> Option<TargetRef> {
        let (kind, module) =
            if let Some((_, entry)) = self.services.iter().find(|(declared, _)| declared == name) {
                (TargetKind::Service, entry.module.as_str())
            } else if let Some((_, entry)) = self
                .repositories
                .iter()
                .find(|(declared, _)| declared == name)
            {
                (TargetKind::Repository, entry.module.as_str())
            } else {
                let (_, entry) = self
                    .searches
                    .iter()
                    .find(|(declared, _)| declared == name)?;
                (TargetKind::Search, entry.module.as_str())
            };
        // `<Application>.<module>.<Name>`, or `<module>.<Name>` when the
        // artifact declares no application (docs/DDD.md owns the contract).
        let qualified_name = match &self.application {
            Some(application) => format!("{application}.{module}.{name}"),
            None => format!("{module}.{name}"),
        };
        Some(TargetRef::new(kind, qualified_name, module))
    }
}

/// The declared capability names, each mapped to the first `.actor` block
/// (in model order) that declares it. An empty actor slice means no actor
/// artifacts were supplied at all — the plain compile's behavior, so every
/// `declaredIn` stays absent.
fn declared_capabilities(actors: Option<&[phero_ir::ActorModel]>) -> BTreeMap<String, String> {
    let mut declared: BTreeMap<String, String> = BTreeMap::new();
    let Some(actors) = actors.filter(|actors| !actors.is_empty()) else {
        return declared;
    };
    for model in actors {
        for block in &model.blocks {
            for capability in &block.capabilities {
                declared
                    .entry(capability.name.clone())
                    .or_insert_with(|| block.name.clone());
            }
        }
    }
    declared
}

/// The capability companions of `names`, index-aligned: `declaredIn` names
/// the declaring `.actor` block when one was supplied, else stays absent.
fn capability_refs(names: &[String], declared: &BTreeMap<String, String>) -> Vec<CapabilityRef> {
    names
        .iter()
        .map(|name| CapabilityRef::new(name.as_str()).with_declared_in(declared.get(name).cloned()))
        .collect()
}

/// The decision-service companions of `names`, index-aligned: `contract`
/// names the provided `.dmn` artifact and `version` the version it
/// declares, each absent when no decision model was supplied (or the
/// reference did not resolve, which only happens on an error compile).
///
/// `pins` holds the authored **version pins** of the subset of `names` that
/// carried one, in the same order — the artifact's additive-pin discipline
/// — so the pairing walks both lists with one cursor: a pin applies to the
/// name it was written on.
fn decision_refs(
    names: &[String],
    pins: &[phero_ir::ddd::DecisionPin],
    decisions: &[(String, phero_ir::dmn::DmnModel)],
) -> Vec<DecisionRef> {
    let mut cursor = pins.iter().peekable();
    names
        .iter()
        .map(|name| {
            let pin = cursor
                .next_if(|pin| pin.name == *name)
                .map(|pin| authored_pin(&pin.pin));
            decision_ref(name, pin.as_ref(), decisions)
        })
        .collect()
}

/// One decision-service companion: name-only without a decision bucket, else
/// the resolved model's identity and declared version.
fn decision_ref(
    name: &str,
    pin: Option<&dsl::DddVersionPin>,
    decisions: &[(String, phero_ir::dmn::DmnModel)],
) -> DecisionRef {
    let mut reference = DecisionRef::new(name);
    if decisions.is_empty() {
        return reference;
    }
    if let DecisionResolution::Resolved { model, path, .. } =
        resolve_decision_service(name, pin, decisions)
    {
        reference.contract = Some(path.to_string());
        reference.version = declared_version(model).map(str::to_string);
    }
    reference
}

/// The wire version pin back as the surface spelling the shared resolver
/// takes (the inverse of the lowering's `lower_version_pin`).
fn authored_pin(pin: &VersionPin) -> dsl::DddVersionPin {
    match pin {
        VersionPin::Compatible => dsl::DddVersionPin::Compatible,
        VersionPin::Exact(value) => dsl::DddVersionPin::Exact(value.clone()),
    }
}
