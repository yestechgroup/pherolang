//! The containment graph the aggregate rules walk: containment edges,
//! reachability, cross references, aggregate-root derivation, and the
//! capability-noun classification the actor join reads (pherolang#144).

use std::collections::{HashMap, HashSet};

use phero_diagnostics::ddd::{
    E_AGGREGATE_BOUNDARY_UNKNOWN, E_AGGREGATE_CONTAINED_ROOT, E_AGGREGATE_CONTAINED_TARGET,
    E_AGGREGATE_CROSS_MODULE, E_AGGREGATE_ROOT_NOT_ROOT, E_AGGREGATE_UNKNOWN_ROOT,
};
use phero_ir::ddd::AggregateOrigin;

use crate::diagnostic::Diagnostic;
use crate::lower::DomainUnit;

use super::*;

/// The effective aggregate verdict for one entity design (DD4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BoundaryVerdict {
    /// The class is an aggregate root.
    Root,
    /// The class is a member of the named root.
    Member {
        /// The resolved root class.
        root: (String, String),
        /// Whether containment or an authored declaration decided it.
        origin: AggregateOrigin,
    },
    /// The verdict would be a guess (incomplete union, ambiguous containment,
    /// or a cycle).
    Unknown,
}

/// The effective boundary table keyed by resolved `(package, class)`.
#[derive(Debug, Default)]
pub(crate) struct BoundaryTable {
    verdicts: HashMap<(String, String), BoundaryVerdict>,
}

impl BoundaryTable {
    /// The verdict for one resolved class.
    pub(crate) fn verdict(&self, package: &str, class: &str) -> Option<&BoundaryVerdict> {
        self.verdicts.get(&(package.to_string(), class.to_string()))
    }

    fn insert(&mut self, key: (String, String), verdict: BoundaryVerdict) {
        self.verdicts.insert(key, verdict);
    }
}

/// Resolves the effective aggregate boundary of every entity design (DD4,
/// issue pherolang#301): authored `root`/`aggregate <Root>` wins over the
/// containment derivation. An incomplete domain union leaves the derived
/// verdict **unknown** (never a silent `true`/`false`) and warns per
/// affected entity design.
pub(crate) fn resolve_aggregate_boundaries(
    entity_designs: &[EntityDesign],
    union: Option<&phero_ir::Model>,
    domains: &[DomainUnit],
) -> (BoundaryTable, Vec<Diagnostic>) {
    let mut local = Vec::new();
    let mut table = BoundaryTable::default();
    let edges = union.map(containment_edges);
    let failed: Vec<&str> = domains
        .iter()
        .filter(|domain| domain.model.is_none())
        .map(|domain| domain.path.as_str())
        .collect();

    for design in entity_designs {
        let key = (design.package.clone(), design.class.clone());
        let derived = edges
            .as_ref()
            .map(|edges| derived_boundary(entity_designs, edges, &key));
        let verdict = match &design.authored {
            Some(dsl::DddBoundaryDecl {
                kind: dsl::DddBoundary::Root,
                span,
            }) => {
                if let Some(Some(container)) = derived.as_ref() {
                    if *container != key {
                        local.push(
                            Diagnostic::error(format!(
                                "{} entity '{}' is contained by '{}'; an owned class cannot be \
                                 an independent aggregate root — model it as a value or remove \
                                 the containment",
                                E_AGGREGATE_CONTAINED_ROOT,
                                design.class,
                                container.1
                            ), Some(*span))
                            .with_help(
                                "remove `root`, or remove the `contains` feature that owns the class",
                            ),
                        );
                    }
                }
                BoundaryVerdict::Root
            }
            Some(dsl::DddBoundaryDecl {
                kind: dsl::DddBoundary::Aggregate(_target),
                span,
            }) => match design.resolved_aggregate.clone() {
                // The target failed to resolve; the rule-3 diagnostic is the
                // one finding.
                None => BoundaryVerdict::Unknown,
                Some(root) => {
                    let root_module = root_design_module(entity_designs, &root);
                    if root_module
                        .as_ref()
                        .is_some_and(|module| module != &design.module)
                    {
                        local.push(
                            Diagnostic::error(format!(
                                "{} aggregate '{}' belongs to module '{}', not '{}'; an aggregate \
                                 cannot span modules",
                                E_AGGREGATE_CROSS_MODULE,
                                design.class,
                                root_module.as_deref().unwrap_or("<unknown>"),
                                design.module
                            ), Some(*span))
                            .with_help(
                                "move the member into the root's module, or model a separate aggregate",
                            ),
                        );
                    }
                    // A contained class's root is fixed by containment; only
                    // a non-contained class may choose its root. `derived`
                    // is `None` when ambiguous (the warning case).
                    match derived.as_ref() {
                        Some(Some(contained_by))
                            if *contained_by != key && *contained_by != root =>
                        {
                            local.push(
                                Diagnostic::error(format!(
                                    "{} entity '{}' is contained by '{}', but declares aggregate '{}'; \
                                     containment fixes its root",
                                    E_AGGREGATE_CONTAINED_TARGET,
                                    design.class,
                                    contained_by.1,
                                    root.1
                                ), Some(*span))
                                .with_help("remove the `aggregate` clause; a contained class's root is derived from containment"),
                            );
                        }
                        None => {
                            local.push(unknown_boundary(
                                design,
                                &failed,
                                "the containment graph is ambiguous",
                            ));
                        }
                        _ => {}
                    }
                    BoundaryVerdict::Member {
                        root,
                        origin: AggregateOrigin::Authored,
                    }
                }
            },
            None => match (union, derived) {
                (Some(_), Some(Some(container))) if container == key => BoundaryVerdict::Root,
                (Some(_), Some(Some(container))) => BoundaryVerdict::Member {
                    root: container,
                    origin: AggregateOrigin::Derived,
                },
                (Some(_), Some(None)) => {
                    local.push(unknown_boundary(
                        design,
                        &failed,
                        "the containment graph is ambiguous",
                    ));
                    BoundaryVerdict::Unknown
                }
                _ => {
                    local.push(unknown_boundary(
                        design,
                        &failed,
                        "an imported domain did not compile",
                    ));
                    BoundaryVerdict::Unknown
                }
            },
        };
        table.insert(key, verdict);
    }

    // Authored members must name an effective root, not another member.
    for design in entity_designs {
        let key = (design.package.clone(), design.class.clone());
        let Some(BoundaryVerdict::Member { root, .. }) = table.verdict(&key.0, &key.1).cloned()
        else {
            continue;
        };
        match table.verdict(&root.0, &root.1) {
            Some(BoundaryVerdict::Root | BoundaryVerdict::Unknown) => {}
            Some(BoundaryVerdict::Member { root: outer, .. }) => {
                if let Some(dsl::DddBoundaryDecl {
                    kind: dsl::DddBoundary::Aggregate(_),
                    span,
                }) = &design.authored
                {
                    local.push(
                        Diagnostic::error(
                            format!(
                                "{} aggregate root '{}' is itself a member of '{}'; name the \
                             outermost root",
                                E_AGGREGATE_ROOT_NOT_ROOT, root.1, outer.1
                            ),
                            Some(*span),
                        )
                        .with_help("an aggregate root cannot be a member of another aggregate"),
                    );
                }
            }
            None => {
                if let Some(dsl::DddBoundaryDecl {
                    kind: dsl::DddBoundary::Aggregate(_),
                    span,
                }) = &design.authored
                {
                    local.push(
                        Diagnostic::error(
                            format!(
                                "{} aggregate root '{}' has no entity design in this module",
                                E_AGGREGATE_UNKNOWN_ROOT, root.1
                            ),
                            Some(*span),
                        )
                        .with_help("declare an entity design for the root in the same module"),
                    );
                }
            }
        }
    }

    (table, local)
}

/// The warning for an entity design whose boundary cannot be derived.
fn unknown_boundary(design: &EntityDesign, failed: &[&str], why: &str) -> Diagnostic {
    let detail = if failed.is_empty() {
        why.to_string()
    } else {
        format!("{why}: {}", failed.join(", "))
    };
    Diagnostic::warning(
        format!(
            "{} aggregate boundary of '{}' is unknown ({detail})",
            E_AGGREGATE_BOUNDARY_UNKNOWN, design.class
        ),
        Some(design.span),
    )
    .with_help("fix the imported domain, or author the boundary with `root`/`aggregate <Root>`")
}

/// The module declaring the resolved root design, when it has one.
fn root_design_module(entity_designs: &[EntityDesign], root: &(String, String)) -> Option<String> {
    entity_designs
        .iter()
        .find(|design| (&design.package, &design.class) == (&root.0, &root.1))
        .map(|design| design.module.clone())
}

/// The derived boundary of a designed entity over the containment graph:
///
/// - `Some(key)` — no designed entity contains it: it is a root;
/// - `Some(root)` — the topmost designed entity that transitively contains
///   it: it is a member of that root;
/// - `None` — several unrelated designed entities contain it (ambiguous), or
///   the graph is cyclic: the verdict would be a guess.
fn derived_boundary(
    entity_designs: &[EntityDesign],
    edges: &HashMap<(String, String), Vec<(String, String)>>,
    key: &(String, String),
) -> Option<(String, String)> {
    let containers: Vec<&EntityDesign> = entity_designs
        .iter()
        .filter(|design| {
            (design.package.as_str(), design.class.as_str()) != (key.0.as_str(), key.1.as_str())
        })
        .filter(|design| reaches(edges, (&design.package, &design.class), (&key.0, &key.1)))
        .collect();
    if containers.is_empty() {
        return Some(key.clone());
    }
    let mut topmost: Vec<(String, String)> = containers
        .iter()
        .filter(|candidate| {
            !containers.iter().any(|other| {
                (&other.package, &other.class) != (&candidate.package, &candidate.class)
                    && reaches(
                        edges,
                        (&other.package, &other.class),
                        (&candidate.package, &candidate.class),
                    )
            })
        })
        .map(|design| (design.package.clone(), design.class.clone()))
        .collect();
    topmost.sort();
    topmost.dedup();
    if topmost.len() == 1 {
        topmost.pop()
    } else {
        None
    }
}

/// Rule 6 — the aggregate boundary: only an effective aggregate root may
/// declare a repository. The verdict is authored-or-derived (DD4); an
/// **unknown** boundary never convicts, its warning already fired.
pub(crate) fn rule_6_aggregate_boundary(
    root_candidates: &[RootCandidate],
    boundaries: &BoundaryTable,
) -> Vec<Diagnostic> {
    let mut local: Vec<Diagnostic> = Vec::new();
    for candidate in root_candidates {
        if let Some(BoundaryVerdict::Member { root, origin }) =
            boundaries.verdict(&candidate.package, &candidate.class)
        {
            let relation = match origin {
                AggregateOrigin::Derived => format!("is contained by '{}'", root.1),
                AggregateOrigin::Authored => {
                    format!("belongs to aggregate '{}'", root.1)
                }
            };
            local.push(
                Diagnostic::error(
                    format!(
                        "entity '{}' {relation}; only aggregate roots \
                         may declare a repository",
                        candidate.authored
                    ),
                    Some(candidate.repository_span),
                )
                .with_help(format!(
                    "remove the repository from design '{0}', or add an operation on \
                     '{1}'s repository instead",
                    candidate.authored, root.1
                )),
            );
        }
    }
    local
}
/// Rule 13 — the aggregate reference constraint (Sculptor's validation of
/// the DDD aggregate): an outside class may hold a cross reference only to
/// an aggregate's root. A reference between two classes of one aggregate
/// and a reference to a root are fine. Computed over the whole union
/// (aggregates are domain structure, not design decisions), and only when
/// every imported domain lowered — the rule-6 discipline.
pub(crate) fn rule_13_aggregate_references(
    union: Option<&phero_ir::Model>,
    boundaries: &BoundaryTable,
) -> Vec<Diagnostic> {
    let mut local: Vec<Diagnostic> = Vec::new();
    if let Some(union) = union {
        let edges = containment_edges(union);
        for (source, feature) in cross_references(union) {
            let phero_ir::TypeRef::Class {
                package: target_package,
                name: target_name,
            } = &feature.type_
            else {
                continue;
            };
            let target = (target_package.as_str(), target_name.as_str());
            // Referencing the effective root is the sanctioned outside access.
            let Some(root) = effective_root(boundaries, &edges, target) else {
                continue;
            };
            if target == (root.0.as_str(), root.1.as_str()) {
                continue;
            }
            // Inside the same aggregate is fine too.
            let Some(source_root) = effective_root(boundaries, &edges, (&source.0, &source.1))
            else {
                continue;
            };
            if source_root == root {
                continue;
            }
            local.push(
                Diagnostic::error(
                    format!(
                        "cross reference '{}.{}' to '{}' escapes its aggregate rooted \
                         at '{}'",
                        source.1, feature.name, target.1, root.1
                    ),
                    None,
                )
                .with_help(
                    "an outside class may hold references only to the aggregate root; \
                     move the reference inside the aggregate or target the root",
                ),
            );
        }
    }
    local
}

/// Writes the effective boundary verdict onto the lowered artifact (DD4):
/// entity designs carry `isAggregateRoot`, members carry the resolved root
/// and origin, and an unknown verdict stays absent.
pub(crate) fn apply_aggregate_boundaries(
    model: &mut phero_ir::ddd::DddModel,
    table: &BoundaryTable,
) {
    for module in &mut model.modules {
        for design in &mut module.designs {
            if design.stereotype != phero_ir::ddd::Stereotype::Entity {
                design.is_aggregate_root = None;
                design.aggregate = None;
                continue;
            }
            let Some(phero_ir::TypeRef::Class { package, name }) = &design.type_ref else {
                continue;
            };
            match table.verdict(package, name) {
                Some(BoundaryVerdict::Root) => {
                    design.is_aggregate_root = Some(true);
                    design.aggregate = None;
                }
                Some(BoundaryVerdict::Member { root, origin }) => {
                    design.is_aggregate_root = Some(false);
                    design.aggregate = Some(phero_ir::ddd::AggregateMembership {
                        root: phero_ir::TypeRef::Class {
                            package: root.0.clone(),
                            name: root.1.clone(),
                        },
                        origin: *origin,
                    });
                }
                Some(BoundaryVerdict::Unknown) | None => {
                    design.is_aggregate_root = None;
                    design.aggregate = None;
                }
            }
        }
    }
}

/// The effective root of a class: the design's authored/derived verdict when
/// one exists, else the containment derivation (DD4).
fn effective_root(
    boundaries: &BoundaryTable,
    edges: &HashMap<(String, String), Vec<(String, String)>>,
    class: (&str, &str),
) -> Option<(String, String)> {
    match boundaries.verdict(class.0, class.1) {
        Some(BoundaryVerdict::Root) => Some((class.0.to_string(), class.1.to_string())),
        Some(BoundaryVerdict::Member { root, .. }) => Some(root.clone()),
        Some(BoundaryVerdict::Unknown) => None,
        None => aggregate_root(edges, class),
    }
}
/// The direct containment edges of the domain model: a class to every class
/// it owns through a `contains` feature.
fn containment_edges(model: &phero_ir::Model) -> HashMap<(String, String), Vec<(String, String)>> {
    let mut edges: HashMap<(String, String), Vec<(String, String)>> = HashMap::new();
    for package in &model.packages {
        for class in &package.classes {
            for feature in &class.features {
                if feature.kind != phero_ir::FeatureKind::Containment {
                    continue;
                }
                if let phero_ir::TypeRef::Class {
                    package: target_package,
                    name: target,
                } = &feature.type_
                {
                    edges
                        .entry((package.name.clone(), class.name.clone()))
                        .or_default()
                        .push((target_package.clone(), target.clone()));
                }
            }
        }
    }
    edges
}
/// `true` when `to` is reachable from `from` over the containment edges
/// (transitively contained).
fn reaches(
    edges: &HashMap<(String, String), Vec<(String, String)>>,
    from: (&str, &str),
    to: (&str, &str),
) -> bool {
    let from = (from.0.to_string(), from.1.to_string());
    let to = (to.0.to_string(), to.1.to_string());
    let mut visited: HashSet<(String, String)> = HashSet::new();
    let mut stack: Vec<(String, String)> = vec![from];
    while let Some(current) = stack.pop() {
        for next in edges.get(&current).into_iter().flatten() {
            if *next == to {
                return true;
            }
            if visited.insert(next.clone()) {
                stack.push(next.clone());
            }
        }
    }
    false
}
/// The cross-reference (`refers`) features of the model, paired with their
/// owning class's `(package, class)` key.
fn cross_references(model: &phero_ir::Model) -> Vec<((String, String), &phero_ir::Feature)> {
    let mut references = Vec::new();
    for package in &model.packages {
        for class in &package.classes {
            for feature in &class.features {
                if feature.kind == phero_ir::FeatureKind::CrossReference {
                    references.push(((package.name.clone(), class.name.clone()), feature));
                }
            }
        }
    }
    references
}
/// The aggregate root of a class: the unique class at the top of its
/// containment chain, walking the containment edges backwards (cycle-safe).
///
/// `None` when the verdict would be a guess: the class sits under two
/// distinct roots (an ambiguous placement never convicts), or under a
/// containment cycle. A class with no container is its own root.
fn aggregate_root(
    edges: &HashMap<(String, String), Vec<(String, String)>>,
    class: (&str, &str),
) -> Option<(String, String)> {
    let class = (class.0.to_string(), class.1.to_string());
    let mut containers_of: HashMap<(String, String), Vec<(String, String)>> = HashMap::new();
    for (owner, contained) in edges {
        for child in contained {
            containers_of
                .entry(child.clone())
                .or_default()
                .push(owner.clone());
        }
    }
    let mut roots: Vec<(String, String)> = Vec::new();
    let mut visited: HashSet<(String, String)> = HashSet::new();
    let mut stack = vec![class];
    while let Some(current) = stack.pop() {
        if !visited.insert(current.clone()) {
            continue;
        }
        match containers_of.get(&current) {
            None => {
                // A top of the chain: a root. Distinct roots stay distinct.
                if !roots.contains(&current) {
                    roots.push(current);
                }
            }
            Some(containers) => {
                for container in containers {
                    stack.push(container.clone());
                }
            }
        }
    }
    if roots.len() == 1 {
        roots.pop()
    } else {
        None
    }
}
/// The aggregate verdict for one capability noun (pherolang#144): what the
/// aggregate machinery says about the class a capability is granted on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AggregateVerdict {
    /// A value design: immutable and shareable by value — no aggregate to
    /// escape.
    Value,
    /// An entity design with a repository that no stereotyped entity
    /// contains: an aggregate root.
    Root,
    /// An entity design with a repository that another stereotyped entity's
    /// class contains (exactly the rule-6 predicate): a non-root aggregate
    /// member. `root` names the containing aggregate root when the
    /// containment chain tops out at exactly one class; an ambiguous chain
    /// never names one.
    NonRoot {
        /// The containing aggregate root, when derivable.
        root: Option<(String, String)>,
    },
}

/// The aggregate boundary one design artifact carries (DD4, issue
/// pherolang#301): the **effective** verdict the compiler lowered.
enum Boundary {
    /// An effective aggregate root (`isAggregateRoot: true`).
    Root,
    /// A member of an aggregate (`isAggregateRoot: false`); `Some` names the
    /// resolved root, `None` when the artifact omitted it.
    Member(Option<(String, String)>),
    /// The compile could not decide (`isAggregateRoot` absent in a
    /// compiler-produced artifact): never re-guessed here.
    Unknown,
    /// No verdict at all — a hand-built artifact without provenance; the
    /// historical containment derivation applies.
    Absent,
}

/// The capability-noun classification (pherolang#144): an aggregate verdict
/// per resolved `(package, class)`, derived purely from the union model and
/// the provided designs. Absence is silence by construction: classes with
/// no design, dto designs, and entities without a repository get no
/// verdict, and the actor join diagnoses only what the table convicts.
pub(crate) struct NounClassification {
    verdicts: HashMap<(String, String), AggregateVerdict>,
}

impl NounClassification {
    /// Classifies every provided design's class against the union's
    /// containment graph — **preferring the artifact's effective aggregate
    /// boundary** (DD4, issue pherolang#301) and falling back to the
    /// containment derivation only for a hand-built artifact that carries no
    /// verdict at all.
    ///
    /// Designs resolve their authored class names the `.ddd` way (base
    /// package first for bare names, otherwise exactly one class across all
    /// packages); a design whose class the union does not declare contributes
    /// nothing. When several designs target one class, the first provided
    /// wins. A design whose boundary the compiler could not decide
    /// (`isAggregateRoot` absent, DD4) is **unknown**, never re-guessed: the
    /// compile already warned E0385, and the join does not convict on an
    /// artifact that says "unknown".
    pub(crate) fn classify(designs: &[phero_ir::ddd::DddModel], union: &phero_ir::Model) -> Self {
        let edges = containment_edges(union);
        // Pass 1 — the stereotyped entity pool (the rule-6 container
        // universe): every entity design resolved to its domain class,
        // repository or not, plus the design entries in provided order.
        let mut entities: Vec<(String, String)> = Vec::new();
        let mut resolved: Vec<((String, String), phero_ir::ddd::Stereotype, bool, Boundary)> =
            Vec::new();
        for model in designs {
            let base = model
                .application
                .as_ref()
                .and_then(|application| application.base.as_deref());
            for module in &model.modules {
                for design in &module.designs {
                    let key = design
                        .type_ref
                        .as_ref()
                        .and_then(|resolved| match resolved {
                            phero_ir::TypeRef::Class { package, name } => {
                                Some((package.clone(), name.clone()))
                            }
                            _ => None,
                        })
                        .or_else(|| {
                            design
                                .class
                                .as_deref()
                                .and_then(|class| resolve_design_class(union, base, class))
                        });
                    let Some(key) = key else {
                        continue;
                    };
                    if design.stereotype == phero_ir::ddd::Stereotype::Entity {
                        entities.push(key.clone());
                    }
                    let boundary = match (design.is_aggregate_root, &design.aggregate) {
                        (Some(true), _) => Boundary::Root,
                        (Some(false), Some(membership)) => {
                            let root = match &membership.root {
                                phero_ir::TypeRef::Class { package, name } => {
                                    Some((package.clone(), name.clone()))
                                }
                                _ => None,
                            };
                            Boundary::Member(root)
                        }
                        (Some(false), None) => Boundary::Member(None),
                        // A compiler-produced artifact always carries the
                        // verdict; absence means the compile could not derive
                        // it (unknown, never false). An artifact with no
                        // provenance is hand-built and keeps the historical
                        // containment derivation.
                        (None, _) if model.phero_version.is_none() => Boundary::Absent,
                        (None, _) => Boundary::Unknown,
                    };
                    resolved.push((
                        key,
                        design.stereotype,
                        design.repository.is_some(),
                        boundary,
                    ));
                }
            }
        }
        // Pass 2 — the verdicts: value designs are values; an entity design
        // with a repository takes the artifact's boundary, and only a
        // hand-built artifact without one falls back to containment (rule
        // 6's predicate, transitive over the containment closure).
        let mut verdicts: HashMap<(String, String), AggregateVerdict> = HashMap::new();
        for (key, stereotype, has_repository, boundary) in resolved {
            let verdict = match stereotype {
                phero_ir::ddd::Stereotype::Value => AggregateVerdict::Value,
                phero_ir::ddd::Stereotype::Entity if has_repository => match boundary {
                    // The artifact's authored/derived membership is
                    // authoritative: a class another design placed in an
                    // aggregate is a member even when no feature of this
                    // pool contains it (the DD4 `aggregate <Root>` case).
                    Boundary::Member(root) => AggregateVerdict::NonRoot { root },
                    // A root verdict is only as good as that file's
                    // visibility: another pooled design's containment still
                    // convicts (a design split across files cannot see its
                    // sibling's `contains`). `Absent` is the hand-built
                    // artifact's fallback and derives the same way.
                    Boundary::Root | Boundary::Absent => {
                        let contained = entities.iter().any(|entity| {
                            *entity != key
                                && reaches(&edges, (&entity.0, &entity.1), (&key.0, &key.1))
                        });
                        if contained {
                            AggregateVerdict::NonRoot {
                                root: aggregate_root(&edges, (&key.0, &key.1)),
                            }
                        } else {
                            AggregateVerdict::Root
                        }
                    }
                    Boundary::Unknown => continue,
                },
                // Repository-less entities and dtos get no verdict.
                _ => continue,
            };
            verdicts.entry(key).or_insert(verdict);
        }
        Self { verdicts }
    }

    /// The verdict for a resolved class, when the designs classify it.
    pub(crate) fn verdict(&self, package: &str, class: &str) -> Option<&AggregateVerdict> {
        self.verdicts.get(&(package.to_string(), class.to_string()))
    }
}

/// Resolves one artifact design's authored class name against the union —
/// the fallback for a hand-built artifact without the resolved `typeRef`
/// companion (DD1, pherolang#298). A compiler-produced artifact carries
/// `typeRef`, so this re-derivation never runs for one. A qualified name
/// names its package and class exactly; a bare name prefers the
/// application's base package and otherwise must match exactly one class
/// across all packages. `None` when nothing (or several classes) match.
fn resolve_design_class(
    union: &phero_ir::Model,
    base: Option<&str>,
    name: &str,
) -> Option<(String, String)> {
    let declares =
        |package: &phero_ir::Package| package.classes.iter().any(|class| class.name == name);
    if let Some((package, class)) = name.rsplit_once('.') {
        let found = union
            .packages
            .iter()
            .find(|candidate| candidate.name == package)
            .is_some_and(|candidate| {
                candidate
                    .classes
                    .iter()
                    .any(|declared| declared.name == class)
            });
        return found.then(|| (package.to_string(), class.to_string()));
    }
    if let Some(base) = base {
        if union
            .packages
            .iter()
            .any(|package| package.name == base && declares(package))
        {
            return Some((base.to_string(), name.to_string()));
        }
    }
    let matches: Vec<&str> = union
        .packages
        .iter()
        .filter(|package| declares(package))
        .map(|package| package.name.as_str())
        .collect();
    if let [package] = matches[..] {
        return Some((package.to_string(), name.to_string()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A domain whose `Library` contains `Book` (the containment derivation
    /// case).
    fn containing_union() -> phero_ir::Model {
        crate::compile_str(
            "domain.mox",
            "package demo\n\nclass Library {\n    String name\n    contains Book[] books\n}\n\nclass Book {\n    String title\n}\n",
            &crate::DomainImports::default(),
        )
        .model
        .expect("the domain compiles")
    }

    /// A domain with no containment at all (`Library` and `Book` unrelated).
    fn flat_union() -> phero_ir::Model {
        crate::compile_str(
            "domain.mox",
            "package demo\n\nclass Library {\n    String name\n}\n\nclass Book {\n    String title\n}\n",
            &crate::DomainImports::default(),
        )
        .model
        .expect("the domain compiles")
    }

    fn artifact(designs: &str, provenance: bool) -> phero_ir::ddd::DddModel {
        let provenance = if provenance {
            r#""pheroVersion": "0.3.0","#
        } else {
            ""
        };
        phero_ir::ddd::DddModel::from_json(&format!(
            r#"{{
  "formatVersion": 3,
  {provenance}
  "application": {{ "name": "App" }},
  "modules": [{{ "name": "m", "designs": [{designs}] }}]
}}"#
        ))
        .expect("the hand-built artifact reads")
    }

    fn design(class: &str, boundary: &str) -> String {
        format!(
            r#"{{
  "class": "{class}",
  "typeRef": {{ "type": "class", "value": {{ "package": "demo", "name": "{class}" }} }},
  "stereotype": "entity",
  {boundary}
  "repository": {{ "name": "{class}Repository", "operations": [] }}
}}"#
        )
    }

    #[test]
    fn an_authored_membership_verdict_wins_without_containment() {
        // The shape the compiler can only produce once rule 6 admits a
        // member-side read (or a hand-built artifact): the artifact says
        // `Book` belongs to `Library`, and no feature contains it. The
        // classification must trust the artifact, not re-derive.
        let model = artifact(
            &format!(
                "{},{}",
                design("Library", r#""isAggregateRoot": true,"#),
                design(
                    "Book",
                    r#""isAggregateRoot": false,
                    "aggregate": { "root": { "type": "class", "value": { "package": "demo", "name": "Library" } }, "origin": "authored" },"#
                )
            ),
            true,
        );
        let classification = NounClassification::classify(&[model], &flat_union());
        assert_eq!(
            classification.verdict("demo", "Book"),
            Some(&AggregateVerdict::NonRoot {
                root: Some(("demo".to_string(), "Library".to_string()))
            }),
            "the artifact's membership is authoritative"
        );
    }

    #[test]
    fn an_undecided_boundary_is_never_guessed() {
        // A compiler-produced artifact (it carries `pheroVersion`) with no
        // verdict: the compile already warned E0385, so the join stays
        // silent instead of deriving a verdict the compiler refused.
        let model = artifact(
            &format!("{},{}", design("Library", ""), design("Book", "")),
            true,
        );
        let classification = NounClassification::classify(&[model], &containing_union());
        assert_eq!(
            classification.verdict("demo", "Book"),
            None,
            "an unknown boundary never convicts"
        );
    }

    #[test]
    fn a_hand_built_artifact_falls_back_to_containment() {
        // No `pheroVersion` and no boundary fields: a hand-built artifact.
        // The historical containment derivation still applies.
        let model = artifact(
            &format!("{},{}", design("Library", ""), design("Book", "")),
            false,
        );
        let classification = NounClassification::classify(&[model], &containing_union());
        assert_eq!(
            classification.verdict("demo", "Book"),
            Some(&AggregateVerdict::NonRoot {
                root: Some(("demo".to_string(), "Library".to_string()))
            }),
            "containment derivation is the hand-built fallback"
        );
    }
}
