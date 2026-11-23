//! Validation-rule tests for the `.ddd` machine *reference* form (issue
//! pherolang#388): `machine d: K from "path.ts";` — the lifecycle authored
//! in a TypeScript/XState file, resolved and validated at compile time.
//! One failing compile per rule (fail-closed, the `import schema`
//! posture), plus the happy-path projection: the parsed IR is projected
//! onto the same `StateMachine` mirror the inline form lowers, so every
//! downstream consumer (transition bindings, condition scoping, the
//! xstate bridge, the graph) keeps working unchanged.

use phero_driver::{compile_ddd_str, CompileInputs, Diagnostic};

/// The canonical fixture's domain: `PayRun.status: PayRunStatus`,
/// `PayRun.total: int`.
const PAYRUN_DOMAIN: &str = include_str!("../../../tests/conformance/ddd/payrun.mox");

/// The referenced-machine twin of the canonical inline machine: the same
/// lifecycle, authored in TypeScript/XState.
const PAYRUN_TS: &str = r#"
import { createMachine } from 'xstate';

export const payrunMachine = createMachine({
  id: 'payrun',
  initial: 'Draft',
  states: {
    Draft: {
      on: {
        CALCULATE: 'Calculating',
        REVERSE: 'Reversed',
      },
    },
    Calculating: { on: { SUBMIT: 'AwaitingApproval' } },
    AwaitingApproval: { on: { APPROVE: 'Approved' } },
    Approved: { on: { SCHEDULE: 'Processing' } },
    Processing: { on: { POST: 'Posted' } },
    Posted: { type: 'final' },
    Reversed: { type: 'final' },
  },
});
"#;

const DESIGN_PATH: &str = "tests/conformance/ddd/test.ddd";
const MACHINE_PATH: &str = "machines/payrun.machine.ts";

/// Compiles a reference-form design against the canonical domain, with the
/// caller-provided machine source provided under the fixture-relative key.
fn diagnose_with_machine(design: &str, ts: Option<&str>) -> Vec<Diagnostic> {
    let mut inputs = CompileInputs::from_domains(&[(
        "tests/conformance/ddd/payrun.mox".to_string(),
        PAYRUN_DOMAIN.to_string(),
    )]);
    if let Some(ts) = ts {
        inputs = inputs.with_machines([(
            DESIGN_PATH.to_string(),
            MACHINE_PATH.to_string(),
            ts.to_string(),
        )]);
    }
    let compilation = compile_ddd_str(DESIGN_PATH, design, &inputs);
    compilation
        .diagnostics
        .into_iter()
        .filter(|(path, _)| path.ends_with(".ddd"))
        .map(|(_, diagnostic)| diagnostic)
        .collect()
}

fn assert_error(design: &str, ts: Option<&str>, expected: &str) {
    let diagnostics = diagnose_with_machine(design, ts);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "expected a diagnostic containing {expected:?}, got: {diagnostics:?}"
    );
}

fn assert_clean(design: &str, ts: Option<&str>) {
    let diagnostics = diagnose_with_machine(design, ts);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != phero_driver::diagnostic::Severity::Error),
        "expected no errors, got: {diagnostics:?}"
    );
}

const HEAD: &str = r#"
import "payrun.mox";

application Payroll {
    base nz.example.payroll
    module payroll {
"#;

const TAIL: &str = r#"
    }
}
"#;

/// The reference-form twin of the canonical machine design.
fn reference_design(body: &str) -> String {
    format!("{HEAD}{body}{TAIL}")
}

#[test]
fn a_referenced_machine_compiles_clean() {
    assert_clean(
        &reference_design(
            r#"
        entity PayRun repository PayRunRepository {
            findById;
            save;
        }
        machine status: PayRunStatus from "machines/payrun.machine.ts";
"#,
        ),
        Some(PAYRUN_TS),
    );
}

#[test]
fn the_projection_is_the_inline_mirror() {
    // The projected mirror equals the inline form's lowering: same field,
    // same enum, same initial, same terminal set, same transition set (the
    // events are the authored TS event names).
    let compilation = compile_ddd_str(
        DESIGN_PATH,
        &reference_design(
            r#"
        entity PayRun repository PayRunRepository {
            findById;
            save;
        }
        machine status: PayRunStatus from "machines/payrun.machine.ts";
"#,
        ),
        &CompileInputs::from_domains(&[(
            "tests/conformance/ddd/payrun.mox".to_string(),
            PAYRUN_DOMAIN.to_string(),
        )])
        .with_machines([(
            DESIGN_PATH.to_string(),
            MACHINE_PATH.to_string(),
            PAYRUN_TS.to_string(),
        )]),
    );
    let model = compilation.model.expect("the compilation lowered");
    let design = model.modules[0].designs
        .iter()
        .find(|design| design.class.as_deref() == Some("PayRun"))
        .expect("the PayRun design");
    let machine = design.machine.as_ref().expect("the machine");
    assert_eq!(machine.field, "status");
    assert_eq!(machine.initial, "Draft");
    let mut terminals = machine.terminals.clone();
    terminals.sort();
    assert_eq!(terminals, vec!["Posted", "Reversed"]);
    assert_eq!(machine.transitions.len(), 6);
    let events: Vec<&str> = machine
        .transitions
        .iter()
        .map(|transition| transition.trigger.as_deref().expect("every referenced transition carries its event"))
        .collect();
    let mut sorted = events.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        vec!["APPROVE", "CALCULATE", "POST", "REVERSE", "SCHEDULE", "SUBMIT"],
        "the authored TS event names are the mirror's trigger names, got {events:?}"
    );
}

#[test]
fn a_missing_machine_file_is_the_not_provided_error() {
    // The host provided nothing for the written path (E0403) — the
    // `import schema '<path>' was not provided` posture.
    assert_error(
        &reference_design(
            r#"
        entity PayRun
        machine status: PayRunStatus from "machines/payrun.machine.ts";
"#,
        ),
        None,
        "E0403 referenced machine file 'machines/payrun.machine.ts' was not provided",
    );
}

#[test]
fn an_unparseable_machine_file_carries_the_typed_rejection() {
    // The fail-closed parser's typed rejection surfaces verbatim (E0404):
    // an inline arrow guard is `ParseError::UnsupportedFeature`.
    let inline_guard_ts = r#"
import { createMachine } from 'xstate';

export const m = createMachine({
  id: 'm',
  initial: 'a',
  states: {
    a: { on: { GO: { target: 'b', guard: ({ context }) => context.ok } } },
    b: { type: 'final' },
  },
});
"#;
    assert_error(
        &reference_design(
            r#"
        entity PayRun
        machine status: PayRunStatus from "machines/payrun.machine.ts";
"#,
        ),
        Some(inline_guard_ts),
        "E0404 referenced machine file 'machines/payrun.machine.ts' did not parse",
    );
}

#[test]
fn a_file_without_machines_is_an_error() {
    let empty_ts = "export const nothing = 1;\n";
    assert_error(
        &reference_design(
            r#"
        entity PayRun
        machine status: PayRunStatus from "machines/payrun.machine.ts";
"#,
        ),
        Some(empty_ts),
        "E0405 referenced machine file 'machines/payrun.machine.ts' declares no machine",
    );
}

#[test]
fn a_file_with_several_machines_names_the_ids() {
    let two_ts = r#"
import { createMachine } from 'xstate';

export const first = createMachine({
  id: 'first',
  initial: 'a',
  states: { a: { type: 'final' } },
});

export const second = createMachine({
  id: 'second',
  initial: 'z',
  states: { z: { type: 'final' } },
});
"#;
    assert_error(
        &reference_design(
            r#"
        entity PayRun
        machine status: PayRunStatus from "machines/payrun.machine.ts";
"#,
        ),
        Some(two_ts),
        "E0406 referenced machine file 'machines/payrun.machine.ts' declares several machines: 'first', 'second'",
    );
}

#[test]
fn an_anonymous_machine_has_no_stable_id() {
    // No authored `id` — the parser synthesizes `machine-<index>`, which is
    // not a stable identity (E0407): the id joins the machine's provenance.
    let anonymous_ts = r#"
import { createMachine } from 'xstate';

export const m = createMachine({
  initial: 'a',
  states: { a: { type: 'final' } },
});
"#;
    assert_error(
        &reference_design(
            r#"
        entity PayRun
        machine status: PayRunStatus from "machines/payrun.machine.ts";
"#,
        ),
        Some(anonymous_ts),
        "E0407 referenced machine file 'machines/payrun.machine.ts' carries no stable machine id",
    );
}

#[test]
fn a_nested_machine_is_outside_the_flat_subset() {
    // The design mirror is flat; compound states are a shape the mirror
    // cannot carry (E0408).
    let nested_ts = r#"
import { createMachine } from 'xstate';

export const nested = createMachine({
  id: 'nested',
  initial: 'outer',
  states: {
    outer: {
      initial: 'inner',
      states: { inner: { type: 'final' } },
      on: { NEXT: 'done' },
    },
    done: { type: 'final' },
  },
});
"#;
    assert_error(
        &reference_design(
            r#"
        entity PayRun
        machine status: PayRunStatus from "machines/payrun.machine.ts";
"#,
        ),
        Some(nested_ts),
        "E0408 referenced machine 'nested' declares nested state 'outer'",
    );
}

#[test]
fn referenced_states_must_be_enum_literals() {
    // A state name that is no literal of the bound enum is the existing
    // endpoint rule, fired on the projection.
    let wrong_states_ts = r#"
import { createMachine } from 'xstate';

export const payrunMachine = createMachine({
  id: 'payrun',
  initial: 'Draft',
  states: {
    Draft: { on: { DONE: 'Shelved' } },
    Shelved: { type: 'final' },
  },
});
"#;
    assert_error(
        &reference_design(
            r#"
        entity PayRun
        machine status: PayRunStatus from "machines/payrun.machine.ts";
"#,
        ),
        Some(wrong_states_ts),
        "state 'Shelved' of machine 'status' is not a literal of enum 'PayRunStatus'",
    );
}

#[test]
fn an_unreachable_referenced_state_is_the_reachability_error() {
    let unreachable_ts = r#"
import { createMachine } from 'xstate';

export const payrunMachine = createMachine({
  id: 'payrun',
  initial: 'Draft',
  states: {
    Draft: { type: 'final' },
    Posted: { type: 'final' },
  },
});
"#;
    assert_error(
        &reference_design(
            r#"
        entity PayRun
        machine status: PayRunStatus from "machines/payrun.machine.ts";
"#,
        ),
        Some(unreachable_ts),
        "state 'Posted' is unreachable",
    );
}

#[test]
fn transition_bindings_validate_against_referenced_events() {
    // pherolang#176 keeps working: the binding's trigger is validated
    // against the referenced machine's declared event names.
    let with_repo = r#"
        entity PayRun repository PayRunRepository {
            findById;
            save;
        }
        machine status: PayRunStatus from "machines/payrun.machine.ts";
"#;
    assert_clean(
        &reference_design(&format!(
            r#"
        {with_repo}
        service PayRunService {{
            inject PayRunRepository;
            advance => PayRunRepository.save transition PayRunStatus.CALCULATE;
        }}
"#
        )),
        Some(PAYRUN_TS),
    );
    assert_error(
        &reference_design(&format!(
            r#"
        {with_repo}
        service PayRunService {{
            inject PayRunRepository;
            advance => PayRunRepository.save transition PayRunStatus.NOPE;
        }}
"#
        )),
        Some(PAYRUN_TS),
        "unknown transition trigger 'NOPE'",
    );
}

#[test]
fn service_conditions_bind_referenced_machine_states() {
    // The module's condition scoping sees the referenced machine: its
    // states bind as enum values in `when` conditions.
    assert_clean(
        &reference_design(&format!(
            r#"
        entity PayRun repository PayRunRepository {{
            findById;
            save;
            PayRun[] findByState(String state) when (status == Posted);
        }}
        machine status: PayRunStatus from "machines/payrun.machine.ts";
        service PayRunService {{
            boolean approve() capability Approve when (status == AwaitingApproval);
        }}
"#
        )),
        Some(PAYRUN_TS),
    );
}

// --- the artifact embed (issue pherolang#388) --------------------------------

use phero_driver::machines::lower_design_machines;

/// Compiles the happy-path reference design and returns the model.
fn compile_reference() -> phero_ir::ddd::DddModel {
    let compilation = compile_ddd_str(
        DESIGN_PATH,
        &reference_design(
            r#"
        entity PayRun repository PayRunRepository {
            findById;
            save;
        }
        machine status: PayRunStatus from "machines/payrun.machine.ts";
"#,
        ),
        &CompileInputs::from_domains(&[(
            "tests/conformance/ddd/payrun.mox".to_string(),
            PAYRUN_DOMAIN.to_string(),
        )])
        .with_machines([(
            DESIGN_PATH.to_string(),
            MACHINE_PATH.to_string(),
            PAYRUN_TS.to_string(),
        )]),
    );
    compilation.model.expect("the compilation lowered")
}

#[test]
fn the_embed_and_provenance_ride_the_artifact() {
    let model = compile_reference();
    let design = model.modules[0]
        .designs
        .iter()
        .find(|design| design.class.as_deref() == Some("PayRun"))
        .expect("the PayRun design");
    let provenance = design
        .machine_provenance
        .as_ref()
        .expect("the provenance rides the artifact");
    assert_eq!(provenance.path, "machines/payrun.machine.ts");
    assert_eq!(provenance.machine_id, "payrun");
    // The hash is over the canonical IR JSON — recomputable from the
    // artifact alone.
    let canonical = phero_ir::canonical_json(design.machine_ir.as_ref().expect("the embed"));
    assert_eq!(
        provenance.content_sha256,
        phero_evidence::sha256_hex(canonical.as_bytes()),
        "the provenance hash is over the canonical embedded IR"
    );
    // The embedded IR is canonical: re-canonicalizing changes nothing.
    assert_eq!(
        design.machine_ir.as_ref().expect("the embed").to_string(),
        canonical,
    );
    // The authored context: the machine's field, the lifecycle the TS
    // declared.
    let machine_ir = design.machine_ir.as_ref().expect("the embed");
    assert_eq!(machine_ir["id"], "payrun");
    assert_eq!(machine_ir["initial"], "Draft");
}

#[test]
fn the_same_compile_is_byte_identical() {
    // HashMap-borne IR structures serialize in varying order; the
    // canonical embed pins the bytes.
    let first = compile_reference().to_json().expect("serialize");
    let second = compile_reference().to_json().expect("serialize");
    assert_eq!(first, second, "two compiles of the same file agree byte for byte");
}

#[test]
fn the_bridge_passes_the_embedded_ir_through() {
    // lower_design_machines emits the authored form verbatim for referenced
    // machines — no re-derivation from the mirror.
    let model = compile_reference();
    let lowered = lower_design_machines(&model);
    assert_eq!(lowered.len(), 1);
    let authored = model.modules[0].designs[0]
        .machine_ir
        .as_ref()
        .expect("the embed");
    assert_eq!(
        serde_json::to_value(&lowered[0]).expect("serialize"),
        *authored,
        "the bridge emits the authored IR, not a mirror re-derivation"
    );
}
