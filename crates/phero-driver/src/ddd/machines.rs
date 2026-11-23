//! State-machine lowering: machine scope collection, transition
//! lowering, machine-reference resolution (issue pherolang#388), and the
//! machine-local helpers.

use phero_diagnostics::ddd::{
    E_MACHINE_REF_EMPTY, E_MACHINE_REF_NO_ID, E_MACHINE_REF_NOT_PROVIDED, E_MACHINE_REF_PARSE,
    E_MACHINE_REF_SEVERAL, E_MACHINE_REF_SHAPE,
};
use phero_ir::ddd::{StateMachine, Transition};
use phero_syntax::ast as dsl;

use crate::diagnostic::Diagnostic;
use crate::imports::MachineSources;
use crate::lower::{self, DomainPackage};

use super::designs::stereotype_label;
use super::*;

/// Stage 2 — one module's machine scopes, collected before its services
/// lower so their `when` conditions can bind machine states (the design
/// stage validates the machines themselves). Every machine-bearing design
/// resolves its class and enum here; ambiguous bindings (two machines
/// declaring the same state in one module) drop the name and say so once.
/// Alongside the machines, the scope carries the module-level tables the
/// condition scoping reads (issue pherolang#180): the ambiguous state names
/// and one row per machine field (the bound feature with its enum, the
/// designed class, and whether the field name is unique across the
/// module's machines).
pub(crate) fn collect_machine_scopes(
    module: &dsl::DddModule,
    packages: &[DomainPackage],
    base: Option<&str>,
    externals: &[MachineReference],
    local: &mut Vec<Diagnostic>,
) -> MachineScopes {
    let mut machines: Vec<ResolvedMachine> = Vec::new();
    let mut fields: Vec<MachineField> = Vec::new();
    let mut ambiguous_states: Vec<String> = Vec::new();
    for design in &module.designs {
        // A DTO carries no lifecycle (the design stage reports the dto-machine
        // error); collecting a scope for one would invent a machine.
        if dto_form(design).is_some() {
            continue;
        }
        let Some(machine_decl) = &design.machine else {
            continue;
        };
        // A referenced machine (issue pherolang#388) contributes its
        // *projection*: the resolved IR takes the body's place, so the
        // scope sees the authored states and events.
        let projected;
        let machine = match external_machine_of(
            externals,
            &module.name.text,
            &machine_decl.field.text,
        ) {
            Some(reference) => {
                projected = project_external_machine(reference, machine_decl);
                &projected
            }
            None => machine_decl,
        };
        let help = format!(
            "the machine '{}: {}' of module '{}' must target a class declared \
             in the imported domains",
            machine.field.text,
            machine.type_.name.full_name(),
            module.name.text
        );
        let Some(class) =
            lower::resolve_ddd_class(&class_type_ref(&design.class), packages, base, &help, local)
        else {
            continue;
        };
        let enum_type = lower::resolve_ddd_type(&machine.type_, packages, base, local);
        let phero_ir::TypeRef::Enum {
            package: enum_package,
            name: enum_name,
        } = &enum_type
        else {
            // The design-stage validation reports this properly; the
            // binding here stays nameless.
            continue;
        };
        fields.push(MachineField {
            field: machine.field.text.clone(),
            alias: lower_camel(enum_name),
            enum_type: (enum_package.clone(), enum_name.clone()),
            // Filled once every machine is collected.
            unique: true,
        });
        let resolved = ResolvedMachine {
            class: class.clone(),
            field: machine.field.text.clone(),
            enum_type: (enum_package.clone(), enum_name.clone()),
            states: machine
                .initial
                .iter()
                .map(|name| name.text.clone())
                .chain(machine.terminals.iter().map(|name| name.text.clone()))
                .chain(
                    machine
                        .transitions
                        .iter()
                        .flat_map(|transition| [&transition.from.text, &transition.to.text])
                        .cloned(),
                )
                .collect::<Vec<_>>(),
            explicit_triggers: machine
                .transitions
                .iter()
                .filter_map(|transition| transition.trigger.as_ref().map(|name| name.text.clone()))
                .collect(),
            derived_triggers: machine
                .transitions
                .iter()
                .filter(|transition| transition.trigger.is_none())
                .map(|transition| lower_camel(&transition.to.text))
                .collect(),
        };
        let mut seen: Vec<String> = Vec::new();
        let mut ambiguous: Vec<String> = Vec::new();
        for state in &resolved.states {
            if seen.contains(state) {
                continue;
            }
            if machines
                .iter()
                .any(|machine| machine.states.iter().any(|declared| declared == state))
            {
                ambiguous.push(state.clone());
            }
            seen.push(state.clone());
        }
        let ambiguous_refs: Vec<&str> = ambiguous.iter().map(String::as_str).collect();
        for state in &ambiguous {
            if !ambiguous_states.contains(state) {
                ambiguous_states.push(state.clone());
            }
            local.push(Diagnostic::error(
                format!(
                    "ambiguous state name '{state}' in module '{}': it is declared \
                     by more than one machine, so conditions cannot bind it",
                    module.name.text
                ),
                Some(machine.span),
            ));
        }
        let mut filtered = resolved;
        filtered
            .states
            .retain(|state| !ambiguous_refs.contains(&state.as_str()));
        machines.push(filtered);
    }
    // A field name two machines bind never binds bare (issue pherolang#180):
    // the enum-derived spelling is its qualified form.
    let mut field_counts: Vec<(String, usize)> = Vec::new();
    for field in &fields {
        match field_counts
            .iter_mut()
            .find(|(name, _)| *name == field.field)
        {
            Some((_, count)) => *count += 1,
            None => field_counts.push((field.field.clone(), 1)),
        }
    }
    for field in &mut fields {
        field.unique = field_counts
            .iter()
            .find(|(name, _)| *name == field.field)
            .is_some_and(|(_, count)| *count == 1);
    }
    MachineScopes {
        machines,
        fields,
        ambiguous_states,
    }
}

/// One resolved `machine … from "<path>";` reference (issue pherolang#388):
/// the file was provided, parsed, and shape-checked at compile time. The
/// module+field key matches the design the declaration head binds.
pub(crate) struct MachineReference {
    /// The declaring module's name.
    pub module: String,
    /// The declaration head's field — the machine's binding key.
    pub field: String,
    /// The referenced file path exactly as written (the provenance's
    /// `path`).
    pub written_path: String,
    /// The authored id (the provenance's `machineId`).
    pub id: String,
    /// The parsed machine IR.
    pub machine: xstate_ir::Machine,
    /// The IR's canonical JSON **value** (keys sorted recursively —
    /// [`phero_ir::canonical_json_value`]): the exact form the artifact
    /// embeds, so its serialization is deterministic.
    pub canonical_value: serde_json::Value,
    /// SHA-256 (lowercase hex) over the canonical JSON **string** — the
    /// provenance's `contentSha256`, recomputable from the artifact alone.
    pub content_sha256: String,
}

/// The resolved machine reference of one `(module, field)` pair, if the
/// design declared the reference form and it resolved.
pub(crate) fn external_machine_of<'a>(
    references: &'a [MachineReference],
    module: &str,
    field: &str,
) -> Option<&'a MachineReference> {
    references
        .iter()
        .find(|reference| reference.module == module && reference.field == field)
}

/// Stage 1.5 — the machine references of one file's modules (issue
/// pherolang#388): every `machine … from "<path>";` declaration's file is
/// looked up in the provided [`MachineSources`] under **the declaring
/// file's path**, parsed with restingstate's fail-closed `xstate-parser`,
/// and checked against the flat single-target subset the design mirror can
/// carry. Roots and fragments call it with their own path, the file that
/// declares the reference (`compile_ddd_app_str` concatenates the
/// per-file results before the merged compile). An unresolved or rejected
/// reference contributes no machine — its diagnostic is the one report.
pub(crate) fn resolve_machine_references(
    path: &str,
    modules: &[dsl::DddModule],
    sources: &MachineSources,
    local: &mut Vec<Diagnostic>,
) -> Vec<MachineReference> {
    let mut references = Vec::new();
    for module in modules {
        for design in &module.designs {
            let Some(machine) = &design.machine else {
                continue;
            };
            let Some(reference) = &machine.source else {
                continue;
            };
            let Some(source) = sources.get(path, &reference.path) else {
                local.push(
                    Diagnostic::error(
                        format!(
                            "{E_MACHINE_REF_NOT_PROVIDED} referenced machine file '{}' \
                             was not provided",
                            reference.path
                        ),
                        Some(reference.span),
                    )
                    .with_help(format!(
                        "the path resolves relative to the declaring design file; \
                         create the file or fix the spelling"
                    )),
                );
                continue;
            };
            let machines = match xstate_parser::parse_all(source) {
                Ok(machines) => machines,
                Err(error) => {
                    // The fail-closed parser's typed rejection surfaces
                    // verbatim — it names the construct and teaches the
                    // convention.
                    local.push(Diagnostic::error(
                        format!(
                            "{E_MACHINE_REF_PARSE} referenced machine file '{}' did \
                             not parse: {error}",
                            reference.path
                        ),
                        Some(reference.span),
                    ));
                    continue;
                }
            };
            let [parsed] = machines.as_slice() else {
                if machines.is_empty() {
                    local.push(Diagnostic::error(
                        format!(
                            "{E_MACHINE_REF_EMPTY} referenced machine file '{}' \
                             declares no machine",
                            reference.path
                        ),
                        Some(reference.span),
                    ));
                } else {
                    let ids = machines
                        .iter()
                        .map(|machine| format!("'{}'", machine.id))
                        .collect::<Vec<_>>()
                        .join(", ");
                    local.push(Diagnostic::error(
                        format!(
                            "{E_MACHINE_REF_SEVERAL} referenced machine file '{}' \
                             declares several machines: {ids}",
                            reference.path
                        ),
                        Some(reference.span),
                    ));
                }
                continue;
            };
            // A stable id: the parser synthesizes `machine-<index>` for an
            // anonymous `createMachine` — an index-derived placeholder, not
            // an authored identity. With exactly one machine the
            // placeholder is always `machine-0`; a machine genuinely given
            // that id is refused too (the collision is the author's to
            // avoid).
            if parsed.id.is_empty() || parsed.id == "machine-0" {
                local.push(Diagnostic::error(
                    format!(
                        "{E_MACHINE_REF_NO_ID} referenced machine file '{}' carries \
                         no stable machine id; the authored createMachine must \
                         declare one",
                        reference.path
                    ),
                    Some(reference.span),
                ));
                continue;
            }
            if let Err(detail) = check_machine_shape(parsed) {
                local.push(Diagnostic::error(
                    format!(
                        "{E_MACHINE_REF_SHAPE} referenced machine '{}' declares {detail}",
                        parsed.id
                    ),
                    Some(reference.span),
                ));
                continue;
            }
            // The embed and its hash both come from the canonical form (the
            // determinism contract): the IR's `HashMap`-borne structures
            // serialize in varying iteration order, so the canonical
            // projection — keys sorted recursively — is the form the
            // artifact embeds and the hash is taken over.
            let parsed_value =
                serde_json::to_value(parsed).expect("the parsed machine IR is JSON-serializable");
            let canonical_value = phero_ir::canonical_json_value(&parsed_value);
            let canonical = phero_ir::canonical_json(&parsed_value);
            let content_sha256 = phero_evidence::sha256_hex(canonical.as_bytes());
            references.push(MachineReference {
                module: module.name.text.clone(),
                field: machine.field.text.clone(),
                written_path: reference.path.clone(),
                id: parsed.id.clone(),
                canonical_value: canonical_value,
                content_sha256: content_sha256,
                machine: parsed.clone(),
            });
        }
    }
    references
}

/// The flat single-target subset the design mirror can carry (issue
/// pherolang#388): every state atomic or final, every transition
/// event-driven with exactly one target. Nested (compound/parallel)
/// states, automatic transitions (`after`/`always`/`onDone` — the parser
/// surfaces them with an empty event name — and `invoke`), timed
/// transitions, and multi-target transitions are each rejected with the
/// construct named.
fn check_machine_shape(machine: &xstate_ir::Machine) -> Result<(), String> {
    for state in &machine.states {
        if matches!(
            state.node_type,
            xstate_ir::StateType::Compound | xstate_ir::StateType::Parallel
        ) || !state.children.is_empty()
        {
            return Err(format!("nested state '{}'", state.name));
        }
        if state.invoke.is_some() {
            return Err(format!(
                "an invoked actor on state '{}' — the mirror is event-driven only",
                state.name
            ));
        }
        for transition in state.on_done.iter().chain(state.transitions.iter()) {
            if transition.event.is_empty() {
                return Err(format!(
                    "an automatic transition on state '{}'",
                    state.name
                ));
            }
            if transition.delay_ms.is_some() {
                return Err(format!(
                    "a timed transition on state '{}'",
                    state.name
                ));
            }
            if transition.targets.len() > 1 {
                return Err(format!(
                    "a transition on state '{}' targeting several states",
                    state.name
                ));
            }
            if transition.target.is_none() {
                return Err(format!(
                    "a transition on state '{}' with no target",
                    state.name
                ));
            }
        }
    }
    Ok(())
}

/// Projects a resolved machine reference onto the DSL inline form (issue
/// pherolang#388): the declaration head keeps field + enum, and the
/// initial/terminals/transitions are projected from the parsed IR, so the
/// *existing* validations (enum closure, reachability, trigger
/// determinism, terminal rules) and every downstream consumer run
/// unchanged. The projection's names carry the reference clause's span —
/// a projected state has no authored span of its own. Guard typing is
/// skipped naturally: the projection carries no `when` clauses.
pub(crate) fn project_external_machine(
    reference: &MachineReference,
    decl: &dsl::DddMachine,
) -> dsl::DddMachine {
    let span = decl
        .source
        .as_ref()
        .map(|source| source.span)
        .unwrap_or(decl.span);
    let name = |text: &str| dsl::Name {
        text: text.to_string(),
        span,
        escaped: false,
    };
    let mut projected = dsl::DddMachine {
        field: decl.field.clone(),
        type_: decl.type_.clone(),
        initial: Some(name(&reference.machine.initial)),
        terminals: Vec::new(),
        transitions: Vec::new(),
        source: None,
        span: decl.span,
    };
    for state in &reference.machine.states {
        if state.node_type == xstate_ir::StateType::Final {
            projected.terminals.push(name(&state.name));
        }
        for transition in &state.transitions {
            projected.transitions.push(dsl::DddMachineTransition {
                from: name(&state.name),
                to: name(transition.target.as_deref().unwrap_or_default()),
                trigger: Some(name(&transition.event)),
                guard: None,
                span,
            });
        }
    }
    projected
}

/// The lowerCamel of a name (`PayRunStatus` → `payRunStatus`) — the
/// enum-derived spelling that qualifies a machine field in a multi-machine
/// module (issue pherolang#180), the same derivation a bare machine arrow's
/// trigger uses.
pub(crate) fn lower_camel(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
/// Validates and lowers one `machine` block against the designed class:
/// the design must be an entity bound to an enum-typed feature, the states
/// must form a deterministic, reachable machine anchored at exactly one
/// initial state, terminal states take no outgoing transitions, non-terminal
/// states take at least one, and every guard type-checks with the designed
/// class as `self` and the machine's states bound (states bind as enum
/// values, so guards read `status == AwaitingApproval` — never magic
/// strings). Returns `None` on a broken machine only when an error was
/// reported; an empty artifact is still preferred over a wrong one.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_design_machine(
    machine: &dsl::DddMachine,
    design: &dsl::DddDesign,
    resolved: Option<&(String, String)>,
    union: Option<&phero_ir::Model>,
    packages: &[DomainPackage],
    base: Option<&str>,
    machines: &[ResolvedMachine],
    context: Option<&phero_expr::TypeContext>,
    source: &str,
    local: &mut Vec<Diagnostic>,
) -> Option<StateMachine> {
    // A referenced machine (issue pherolang#388) arrives here as its
    // *projection* — the caller resolved the reference and projected the
    // IR onto the inline form — so the existing rules (enum closure,
    // reachability, trigger determinism, terminal rules) run unchanged.
    // Guard typing is skipped naturally: the projection carries no `when`
    // clauses.
    let mut ok = true;

    // A lifecycle is entity semantics (the repository-placement precedent):
    // a value object is immutable and a dto is a transfer record — neither
    // carries states to move through.
    if design.stereotype != dsl::DddStereotype::Entity {
        local.push(Diagnostic::error(
            format!(
                "machine '{}: {}' requires an entity design; '{}' is designed as a {}",
                machine.field.text,
                machine.type_.name.full_name(),
                design.class.text,
                stereotype_label(design.stereotype)
            ),
            Some(machine.span),
        ));
        ok = false;
    }

    // The machine's type must resolve to an enum.
    let enum_type = lower::resolve_ddd_type(&machine.type_, packages, base, local);
    if !matches!(&enum_type, phero_ir::TypeRef::Enum { .. }) {
        local.push(Diagnostic::error(
            format!(
                "machine '{}: {}' must bind an enum; '{}' is not one",
                machine.field.text,
                machine.type_.name.full_name(),
                machine.type_.name.full_name()
            ),
            Some(machine.type_.span),
        ));
        ok = false;
    }

    // The bound feature must exist on the class and carry exactly the
    // machine's enum type.
    if let (Some((package, class)), Some(union)) = (resolved, union) {
        match class_features(union, package, class).and_then(|features| {
            features
                .iter()
                .find(|feature| feature.name == machine.field.text)
                .cloned()
        }) {
            None => {
                local.push(Diagnostic::error(
                    format!(
                        "machine '{}' binds feature '{}' of '{}', which does not exist",
                        machine.field.text, machine.field.text, design.class.text
                    ),
                    Some(machine.field.span),
                ));
                ok = false;
            }
            Some(feature) => {
                if feature.type_ != enum_type {
                    local.push(Diagnostic::error(
                        format!(
                            "machine '{}: {}' binds feature '{}' of '{}', which carries a \
                             different type",
                            machine.field.text,
                            machine.type_.name.full_name(),
                            machine.field.text,
                            design.class.text
                        ),
                        Some(machine.field.span),
                    ));
                    ok = false;
                }
            }
        }
    }

    // Exactly one initial state.
    let initial: Option<String> = match &machine.initial {
        Some(name) => Some(name.text.clone()),
        None => {
            local.push(
                Diagnostic::error(
                    format!(
                        "machine '{}: {}' declares no initial state",
                        machine.field.text,
                        machine.type_.name.full_name()
                    ),
                    Some(machine.span),
                )
                .with_help("add `initial <State>;` to the machine body"),
            );
            ok = false;
            // Recovery: the first terminal, else the first transition
            // endpoint, keeps the rest of the machine checkable.
            machine
                .terminals
                .first()
                .map(|name| name.text.clone())
                .or_else(|| {
                    machine
                        .transitions
                        .first()
                        .map(|transition| transition.from.text.clone())
                })
        }
    };

    // Duplicate terminals.
    let mut terminals: Vec<String> = Vec::new();
    for terminal in &machine.terminals {
        if terminals.contains(&terminal.text) {
            local.push(
                Diagnostic::error(
                    format!(
                        "duplicate terminal state '{}' in machine '{}'",
                        terminal.text, machine.field.text
                    ),
                    Some(terminal.span),
                )
                .with_help("a state is declared terminal once per machine"),
            );
        } else {
            terminals.push(terminal.text.clone());
        }
    }

    // Duplicate transitions, and determinism: one trigger name leaves a
    // state for at most one target (the statechart event rule).
    let mut transitions: Vec<(String, String, Option<String>)> = Vec::new();
    for transition in &machine.transitions {
        let trigger = transition.trigger.as_ref().map(|name| name.text.clone());
        let row = (
            transition.from.text.clone(),
            transition.to.text.clone(),
            trigger.clone(),
        );
        if transitions.contains(&row) {
            local.push(
                Diagnostic::error(
                    format!(
                        "duplicate transition '{} -> {}' in machine '{}'",
                        transition.from.text, transition.to.text, machine.field.text
                    ),
                    Some(transition.span),
                )
                .with_help("an edge is declared once per machine"),
            );
            continue;
        }
        if let Some(trigger) = &trigger {
            let clashing = transitions.iter().any(|(from, _, other)| {
                from == &transition.from.text && other.as_deref() == Some(trigger.as_str())
            });
            if clashing {
                local.push(
                    Diagnostic::error(
                        format!(
                            "trigger '{trigger}' leaves state '{}' more than once in machine '{}'",
                            transition.from.text, machine.field.text
                        ),
                        Some(transition.span),
                    )
                    .with_help("a trigger deterministically names one transition per state"),
                );
                ok = false;
            }
        }
        transitions.push(row);
    }

    // Terminal states take no outgoing transitions.
    for transition in &machine.transitions {
        if terminals.contains(&transition.from.text) {
            local.push(
                Diagnostic::error(
                    format!(
                        "terminal state '{}' takes a transition to '{}' in machine '{}'",
                        transition.from.text, transition.to.text, machine.field.text
                    ),
                    Some(transition.span),
                )
                .with_help("a terminal state ends the lifecycle; remove the outgoing edge"),
            );
            ok = false;
        }
    }

    // Reachability from the initial state; every non-terminal state takes at
    // least one outgoing transition.
    let states: Vec<String> = {
        let mut states = initial
            .clone()
            .into_iter()
            .chain(terminals.iter().cloned())
            .collect::<Vec<_>>();
        for (from, to, _) in &transitions {
            if !states.contains(from) {
                states.push(from.clone());
            }
            if !states.contains(to) {
                states.push(to.clone());
            }
        }
        states
    };

    // Machines are closed over their enum: every state name — the initial,
    // a terminal, and every transition endpoint — must be a literal of the
    // machine's enum type. A typo'd endpoint is an error, never a new
    // state.
    if let Some(literals) = enum_literals(union, &enum_type) {
        for state in &states {
            if !literals.contains(state) {
                local.push(
                    Diagnostic::error(
                        format!(
                            "state '{state}' of machine '{}' is not a literal of enum '{}'",
                            machine.field.text,
                            machine.type_.name.full_name()
                        ),
                        Some(machine.span),
                    )
                    .with_help(format!("the enum declares: {}", literals.join(", "))),
                );
                ok = false;
            }
        }
    }

    if let Some(initial) = &initial {
        let mut reachable = vec![initial.clone()];
        let mut frontier = vec![initial.clone()];
        while let Some(state) = frontier.pop() {
            for (from, to, _) in transitions.iter().rev() {
                if from == &state && !reachable.contains(to) {
                    reachable.push(to.clone());
                    frontier.push(to.clone());
                }
            }
        }
        for state in &states {
            if !reachable.contains(state) {
                local.push(Diagnostic::error(
                    format!(
                        "state '{state}' is unreachable from the initial state in machine '{}'",
                        machine.field.text
                    ),
                    Some(machine.span),
                ));
                ok = false;
            }
        }
        let mut outgoing: Vec<&str> = transitions
            .iter()
            .map(|(from, _, _)| from.as_str())
            .collect();
        outgoing.sort_unstable();
        outgoing.dedup();
        for state in &states {
            if !terminals.contains(state) && !outgoing.contains(&state.as_str()) {
                local.push(
                    Diagnostic::error(
                        format!(
                        "state '{state}' takes no transition and is not terminal in machine '{}'",
                        machine.field.text
                    ),
                        Some(machine.span),
                    )
                    .with_help("declare it `terminal <State>;` or add an outgoing edge"),
                );
                ok = false;
            }
        }
    }

    // Edge guards: the designed class scopes them; the machine's own states
    // bind (a foreign machine's states are unbound here — cross-entity state
    // references are a type error). The scope table carries only the own
    // machine, so the shared check binds no foreign state and no machine
    // fields (the own-record names bind through `self`).
    let own_machine = machines
        .iter()
        .find(|candidate| candidate.class == resolved.cloned().unwrap_or_default());
    let edge_machines: Vec<ResolvedMachine> = own_machine.cloned().into_iter().collect();
    let edge_scopes = MachineScopes {
        machines: edge_machines,
        fields: Vec::new(),
        ambiguous_states: Vec::new(),
    };
    for transition in &machine.transitions {
        if let Some(guard) = transition.guard {
            check_condition(
                source,
                guard,
                ConditionCheck {
                    label: format!(
                        "transition '{} -> {}' of machine '{}'",
                        transition.from.text, transition.to.text, machine.field.text
                    ),
                    // The module name feeds the path walker's E0354 only;
                    // edge guards run no walker.
                    module: "",
                    self_class: resolved,
                    scopes: &edge_scopes,
                    injections: &[],
                    dependencies: &[],
                    union: None,
                    context,
                    params: &[],
                },
                local,
            );
        }
    }

    if !ok {
        return None;
    }
    let mut machine_out = StateMachine::new(machine.field.text.clone(), enum_type, initial?);
    machine_out.terminals = terminals;
    machine_out.transitions = transitions
        .into_iter()
        .map(|(from, to, trigger)| Transition {
            from,
            to,
            trigger,
            guard: None,
        })
        .collect();
    // The guard text fills in from the AST (the dedup pass above drops
    // duplicate rows, so indexes stay aligned).
    for (out, ast) in machine_out.transitions.iter_mut().zip(&machine.transitions) {
        out.guard = ast
            .guard
            .map(|span| crate::conditions::when_interior(source, span).to_string());
    }
    Some(machine_out)
}
/// One designed machine resolved against its domain class, collected before
/// a module's services lower so their conditions can bind machine states.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedMachine {
    /// The designed class, package-qualified (the condition scope of the
    /// module's service operations).
    pub(crate) class: (String, String),
    /// The machine's bound field name as authored (`status`).
    pub(crate) field: String,
    /// The enum the states are drawn from.
    pub(crate) enum_type: (String, String),
    /// Every state the machine declares (initial, terminals, endpoints),
    /// minus the module's ambiguous names (those never bind).
    pub(crate) states: Vec<String>,
    /// The explicit `via` trigger names (issue pherolang#176: legal binding
    /// targets of a service operation's `transition` clause).
    pub(crate) explicit_triggers: Vec<String>,
    /// The derived trigger names — the lowerCamel of the target state for
    /// every bare arrow (`Posted` → `posted`). Derived names are legal
    /// binding targets too; a name that is both explicit and derived is
    /// ambiguous and binds nothing (the E0350 error at the binding site).
    pub(crate) derived_triggers: Vec<String>,
}

impl ResolvedMachine {
    /// Every trigger the machine declares, explicit and derived, in
    /// declaration order (duplicates removed) — the vocabulary a
    /// transition binding resolves against and the did-you-mean pool.
    pub(crate) fn trigger_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for name in self
            .explicit_triggers
            .iter()
            .chain(self.derived_triggers.iter())
        {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        names
    }
}

/// One module's machine scopes: the per-machine resolutions plus the
/// module-level tables the condition scoping reads (issue pherolang#180).
#[derive(Debug, Clone, Default)]
pub(crate) struct MachineScopes {
    /// The resolved machines, in design order, ambiguous state names
    /// removed.
    pub(crate) machines: Vec<ResolvedMachine>,
    /// One row per machine-bearing design whose class and enum resolved.
    pub(crate) fields: Vec<MachineField>,
    /// State names declared by more than one machine of the module —
    /// unbound everywhere; a condition referencing one is the `E0354`
    /// error with the `<EnumType>.<State>` qualification as help.
    pub(crate) ambiguous_states: Vec<String>,
}

impl MachineScopes {
    /// `true` when the module carries more than one machine-bearing design —
    /// the multi-machine scoping regime (no implicit self; machine fields
    /// bind by uniqueness plus their enum-derived spellings).
    pub(crate) fn is_multi_machine(&self) -> bool {
        self.machines.len() > 1
    }

    /// The machine whose designed class is `class`, if any.
    pub(crate) fn machine_of_class(&self, class: &(String, String)) -> Option<&ResolvedMachine> {
        self.machines.iter().find(|machine| &machine.class == class)
    }
}

/// One machine-bearing design's bound field, as the condition scoping
/// addresses it (issue pherolang#180): by the authored field name when it is
/// unique across the module's machines, and always by the enum-derived
/// spelling ([`MachineField::alias`], the lowerCamel of the machine's enum
/// type name).
#[derive(Debug, Clone)]
pub(crate) struct MachineField {
    /// The authored machine field name (`status`).
    pub(crate) field: String,
    /// The enum-derived spelling (`PayRunStatus` → `payRunStatus`).
    pub(crate) alias: String,
    /// The machine's enum type, package-qualified.
    pub(crate) enum_type: (String, String),
    /// Whether [`MachineField::field`] is unique across the module's
    /// machines; an ambiguous field name never binds bare (the `E0354`
    /// error at reference sites).
    pub(crate) unique: bool,
}
/// The literal names of the referenced enum, in declaration order, when the
/// type resolves to an enum of the domain union.
fn enum_literals(
    union: Option<&phero_ir::Model>,
    enum_type: &phero_ir::TypeRef,
) -> Option<Vec<String>> {
    let phero_ir::TypeRef::Enum { package, name } = enum_type else {
        return None;
    };
    let package = union?
        .packages
        .iter()
        .find(|candidate| &candidate.name == package)?;
    let enum_def = package
        .enums
        .iter()
        .find(|candidate| &candidate.name == name)?;
    Some(
        enum_def
            .literals
            .iter()
            .map(|literal| literal.name.clone())
            .collect(),
    )
}
/// The operations of one resolved class (own only — the dto-with-ops
/// warning, rule 15, is about the designed class's declared behavior).
pub(crate) fn class_operations<'a>(
    union: &'a phero_ir::Model,
    package: &str,
    class: &str,
) -> Option<&'a [phero_ir::Operation]> {
    union
        .packages
        .iter()
        .find(|candidate| candidate.name == package)
        .and_then(|candidate| candidate.classes.iter().find(|c| c.name == class))
        .map(|def| def.operations.as_slice())
}
