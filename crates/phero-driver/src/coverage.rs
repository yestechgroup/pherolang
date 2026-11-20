//! The intent coverage pass (issue pherolang#196): resolves every intent's
//! links against the application's compiled artifacts and reports the
//! traceability findings — the `driver.integrity` band's `X0009`–`X0014`.
//!
//! The rules:
//!
//! - every intent must map to **at least one resolved element** and **at
//!   least one resolved check**; a missing slot **warns** (`X0009` /
//!   `X0010`) — an uncovered intent is a claim nobody can honor, and
//!   honesty beats reach;
//! - a link naming no element or check of its kind **errors**, spanned,
//!   with the did-you-mean candidate in the help when one is close
//!   (`X0011` / `X0012`);
//! - a bare link name matching several elements of its kind **errors**
//!   (`X0013`) — the pass refuses to guess a target, so the traceability
//!   edge would be a lie;
//! - a stable id declared by two intent artifacts of one application
//!   **errors** (`X0014`) — the id is the traceability key.
//!
//! Like [`crate::integrity::check_integrity`], this is a pure function over
//! path-tagged compiled artifacts — no filesystem, no re-parse — and it is
//! best-effort over the compiling subset: an application whose domains
//! failed to lower resolves nothing beyond the surviving artifacts, and the
//! domain diagnostics already block the artifact. Unlike the integrity
//! pass, the diagnostics are **spanned** where the host provides the
//! compile-time span table ([`IntentLinkSite`]): the intent artifacts are
//! span-free on the wire, so the spans travel beside them (the `.ifml`
//! auth-sites discipline), and a host without the table gets the same
//! findings span-free.

use std::collections::{BTreeMap, BTreeSet};

use phero_diagnostics::did_you_mean_adaptive;
use phero_diagnostics::integrity::{
    X_AMBIGUOUS_INTENT_LINK, X_DUPLICATE_INTENT_ID, X_INTENT_WITHOUT_CHECK,
    X_INTENT_WITHOUT_ELEMENT, X_UNKNOWN_INTENT_CHECK, X_UNKNOWN_INTENT_ELEMENT,
};
use phero_ir::ddd::DddModel;
use phero_ir::dmn::DmnModel;
use phero_ir::events::EventModel;
use phero_ir::ifml::IfmlModel;
use phero_ir::intents::{IntentLinkKind, IntentModel};
use phero_ir::ActorModel;
use phero_ir::{Model, TypeRef};

use crate::diagnostic::Diagnostic;
use crate::intents::IntentLinkSite;

/// One application's resolution vocabulary: every name a link can target,
/// per kind, collected once per application.
struct Vocabulary {
    /// Per kind: the bare (as-written) name → every target it matches
    /// (the qualified spellings for classes and operations, the name
    /// itself for the set kinds). More than one match is the ambiguity
    /// error; zero is the unknown error.
    bare: BTreeMap<IntentLinkKind, BTreeMap<String, Vec<String>>>,
    /// Per kind: the dotted spellings a qualified name matches exactly
    /// (`package.Class`, `Service.operation`). The set kinds declare none.
    qualified: BTreeMap<IntentLinkKind, BTreeSet<String>>,
    /// Every bare name of every kind — the did-you-mean candidate pool.
    bare_names: BTreeSet<String>,
}

impl Vocabulary {
    #[allow(clippy::too_many_arguments)]
    fn build(
        domains: &[Model],
        actors: &[(String, ActorModel)],
        events: &[(String, EventModel)],
        flows: &[(String, IfmlModel)],
        designs: &[(String, DddModel)],
        decisions: &[(String, DmnModel)],
    ) -> Self {
        let mut vocab = Vocabulary {
            bare: BTreeMap::new(),
            qualified: BTreeMap::new(),
            bare_names: BTreeSet::new(),
        };
        for kind in [
            IntentLinkKind::Class,
            IntentLinkKind::Capability,
            IntentLinkKind::Event,
            IntentLinkKind::View,
            IntentLinkKind::Command,
            IntentLinkKind::Op,
            IntentLinkKind::Invariant,
            IntentLinkKind::Decision,
        ] {
            vocab.bare.insert(kind, BTreeMap::new());
            vocab.qualified.insert(kind, BTreeSet::new());
        }
        for model in domains {
            for package in &model.packages {
                for class in &package.classes {
                    let qualified = format!("{}.{}", package.name, class.name);
                    vocab.register_bare(IntentLinkKind::Class, &class.name, qualified.clone());
                    vocab
                        .qualified
                        .get_mut(&IntentLinkKind::Class)
                        .expect("the kind table exists")
                        .insert(qualified);
                    vocab.bare_names.insert(class.name.clone());
                }
            }
        }
        for (_, model) in actors {
            for block in &model.blocks {
                for capability in &block.capabilities {
                    vocab.register_bare(
                        IntentLinkKind::Capability,
                        &capability.name,
                        capability.name.clone(),
                    );
                    vocab.bare_names.insert(capability.name.clone());
                }
            }
        }
        for (_, model) in events {
            for event in &model.events {
                vocab.register_bare(IntentLinkKind::Event, &event.name, event.name.clone());
                vocab.bare_names.insert(event.name.clone());
            }
        }
        for (_, model) in flows {
            for view in &model.views {
                vocab.register_bare(IntentLinkKind::View, &view.name, view.name.clone());
                vocab.bare_names.insert(view.name.clone());
            }
            for command in &model.commands {
                vocab.register_bare(IntentLinkKind::Command, &command.name, command.name.clone());
                vocab.bare_names.insert(command.name.clone());
            }
        }
        for (_, model) in designs {
            for module in &model.modules {
                for service in &module.services {
                    for operation in &service.operations {
                        let qualified = format!("{}.{}", service.name, operation.name);
                        vocab.register_bare(IntentLinkKind::Op, &operation.name, qualified.clone());
                        vocab
                            .qualified
                            .get_mut(&IntentLinkKind::Op)
                            .expect("the kind table exists")
                            .insert(qualified);
                        vocab.bare_names.insert(operation.name.clone());
                    }
                }
                for design in &module.designs {
                    if design.invariants.is_empty() {
                        continue;
                    }
                    // The designed class's bare name: the resolved identity
                    // when the domain lowered, else the authored spelling.
                    let class = match &design.type_ref {
                        Some(TypeRef::Class { name, .. }) => Some(name.clone()),
                        _ => design.class.clone(),
                    };
                    if let Some(class) = class {
                        vocab.register_bare(IntentLinkKind::Invariant, &class, class.clone());
                        vocab.bare_names.insert(class);
                    }
                }
            }
        }
        for (_, model) in decisions {
            for decision in &model.decisions {
                vocab.register_bare(
                    IntentLinkKind::Decision,
                    &decision.name,
                    decision.name.clone(),
                );
                vocab.bare_names.insert(decision.name.clone());
            }
        }
        vocab
    }

    fn register_bare(&mut self, kind: IntentLinkKind, bare: &str, target: String) {
        self.bare
            .get_mut(&kind)
            .expect("the kind table exists")
            .entry(bare.to_string())
            .or_default()
            .push(target);
    }

    /// Resolves one link name: the dotted spellings first (exact match),
    /// then the bare table (exactly one match resolves, several is the
    /// ambiguity, zero is the unknown with the did-you-mean candidate).
    fn resolve(&self, kind: IntentLinkKind, name: &str) -> Resolution {
        if name.contains('.') && self.qualified[&kind].contains(name) {
            return Resolution::Resolved;
        }
        match self.bare[&kind].get(name) {
            Some(matches) if matches.len() == 1 => Resolution::Resolved,
            Some(matches) => Resolution::Ambiguous(matches.clone()),
            None => Resolution::Unknown(did_you_mean_adaptive(name, &self.bare_names)),
        }
    }
}

/// The outcome of resolving one link name.
enum Resolution {
    /// The name matches exactly one target.
    Resolved,
    /// The name matches nothing; the closest candidate when one is close.
    Unknown(Option<String>),
    /// The bare name matches several targets; the pass refuses to guess.
    Ambiguous(Vec<String>),
}

/// Runs the coverage pass over one application's compiled artifacts:
/// `intents` are the `(path, artifact)` pairs, `spans` the compile-time
/// span tables keyed by the same paths (absent for a host without them —
/// the same findings, span-free). Returns the path-tagged diagnostics,
/// integrity-pass order (the cross-file id collisions first, then each
/// intent file in input order, per intent in source order).
#[allow(clippy::too_many_arguments)]
pub fn check_coverage(
    intents: &[(String, IntentModel)],
    spans: &BTreeMap<String, Vec<IntentLinkSite>>,
    domains: &[Model],
    actors: &[(String, ActorModel)],
    events: &[(String, EventModel)],
    flows: &[(String, IfmlModel)],
    designs: &[(String, DddModel)],
    decisions: &[(String, DmnModel)],
    application: &str,
) -> Vec<(String, Diagnostic)> {
    let vocabulary = Vocabulary::build(domains, actors, events, flows, designs, decisions);

    // Rule X0014 — stable ids are application-unique: the first file that
    // declares an id owns it; every later declaration is the collision.
    let mut diagnostics: Vec<(String, Diagnostic)> = Vec::new();
    let mut id_owner: BTreeMap<&str, &str> = BTreeMap::new();
    for (path, model) in intents {
        for intent in &model.intents {
            match id_owner.get(intent.id.as_str()) {
                Some(owner) => {
                    let span = intents
                        .iter()
                        .find(|(site_path, _)| site_path == path)
                        .and_then(|(site_path, _)| {
                            spans.get(site_path).and_then(|sites| {
                                sites
                                    .iter()
                                    .find(|site| site.intent == intent.id)
                                    .map(|site| site.intent_span)
                            })
                        });
                    diagnostics.push((
                        path.clone(),
                        Diagnostic::error(
                            format!(
                                "{X_DUPLICATE_INTENT_ID} duplicate intent id '{}' across '{}' \
                                 and '{}'",
                                intent.id, owner, path
                            ),
                            span,
                        )
                        .with_help(
                            "renumber one file's intent so ids stay unique across the application",
                        ),
                    ));
                }
                None => {
                    id_owner.insert(intent.id.as_str(), path);
                }
            }
        }
    }

    for (path, model) in intents {
        let file_sites = spans.get(path);
        let mut local: Vec<Diagnostic> = Vec::new();
        for intent in &model.intents {
            let mut resolved_elements = 0usize;
            let mut resolved_checks = 0usize;
            for (slot, links) in [("element", &intent.elements), ("check", &intent.checks)] {
                let code = match slot {
                    "element" => X_UNKNOWN_INTENT_ELEMENT,
                    _ => X_UNKNOWN_INTENT_CHECK,
                };
                for link in links {
                    let span = file_sites.and_then(|sites| {
                        sites
                            .iter()
                            .find(|site| {
                                site.intent == intent.id
                                    && site.name == link.name
                                    && site.kind.as_str() == link.kind.as_str()
                            })
                            .map(|site| site.name_span)
                    });
                    match vocabulary.resolve(link.kind, &link.name) {
                        Resolution::Resolved => match slot {
                            "element" => resolved_elements += 1,
                            _ => resolved_checks += 1,
                        },
                        Resolution::Unknown(suggestion) => {
                            let mut diagnostic = Diagnostic::error(
                                format!(
                                    "{code} unknown {slot} {} '{}' in intent '{}'",
                                    link.kind.as_str(),
                                    link.name,
                                    intent.id
                                ),
                                span,
                            );
                            diagnostic = match suggestion {
                                Some(candidate) => {
                                    diagnostic.with_help(format!("did you mean '{candidate}'?"))
                                }
                                None => diagnostic.with_help(format!(
                                    "declare the {} or fix the spelling (the {} names must \
                                     resolve in the application's artifacts)",
                                    link.kind.as_str(),
                                    slot
                                )),
                            };
                            local.push(diagnostic);
                        }
                        Resolution::Ambiguous(matches) => {
                            local.push(
                                Diagnostic::error(
                                    format!(
                                        "{X_AMBIGUOUS_INTENT_LINK} '{}' matches several '{}' \
                                         targets in application '{}': {}",
                                        link.name,
                                        link.kind.as_str(),
                                        application,
                                        matches.join(", ")
                                    ),
                                    span,
                                )
                                .with_help(
                                    "qualify the name (a dotted package.Class path for a class, \
                                     Service.operation for an operation), or rename one of the \
                                     colliding elements",
                                ),
                            );
                        }
                    }
                }
            }
            // The honesty rule — a missing slot warns, never errors.
            if resolved_elements == 0 {
                local.push(
                    Diagnostic::warning(
                        format!(
                            "{X_INTENT_WITHOUT_ELEMENT} intent '{}' maps to no model element",
                            intent.id
                        ),
                        file_sites.and_then(|sites| {
                            sites
                                .iter()
                                .find(|site| site.intent == intent.id)
                                .map(|site| site.intent_span)
                        }),
                    )
                    .with_help(
                        "an intent no model element expresses is a claim nobody can honor — \
                         add an `element` link, or withdraw the intent",
                    ),
                );
            }
            if resolved_checks == 0 {
                local.push(
                    Diagnostic::warning(
                        format!(
                            "{X_INTENT_WITHOUT_CHECK} intent '{}' maps to no check",
                            intent.id
                        ),
                        file_sites.and_then(|sites| {
                            sites
                                .iter()
                                .find(|site| site.intent == intent.id)
                                .map(|site| site.intent_span)
                        }),
                    )
                    .with_help(
                        "nothing model-native verifies this claim — add a `check` link (a \
                         designed class's invariants, or a decision)",
                    ),
                );
            }
        }
        diagnostics.extend(
            local
                .into_iter()
                .map(|diagnostic| (path.clone(), diagnostic)),
        );
    }
    diagnostics
}
