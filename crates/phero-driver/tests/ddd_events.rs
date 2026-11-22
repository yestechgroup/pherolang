//! Integration tests for the `.ddd` emission-link dimension
//! (`compile_ddd_str` with an `events` bucket, rule 23, pherolang#382): the
//! `emits` clauses validated against caller-provided `.evt` contracts, the
//! `.evt` import provision check, the resolved `emittedRefs[].contract`
//! companions, and the plain-compile records-unvalidated behavior. The
//! normative contract lives on `compile_ddd_str`; these tests pin it.

use phero_driver::{compile_ddd_str, DddCompilation, Severity};
use phero_ir::events::{EventDef, EventModel};

const DOMAIN: &str = r#"
package nz.example.payroll

class PayRun {
    String id
    String status
}
"#;

/// A clean design over [`DOMAIN`] with an event-contract import and one
/// emitting operation.
const DESIGN: &str = r#"
import "payroll.mox";
import "events.evt";

application Payroll {
    base nz.example.payroll

    module payment {
        service PayRunService {
            boolean approve(PayRun payRun) emits PayRunStateChanged;
        }
    }
}
"#;

/// An event contract declaring the named events.
fn event_model(names: &[&str]) -> EventModel {
    let mut model = EventModel::new();
    for name in names {
        model.events.push(EventDef::new(*name));
    }
    model
}

fn domains() -> Vec<(String, String)> {
    vec![("payroll.mox".to_string(), DOMAIN.to_string())]
}

fn compile_with_events(source: &str, events: &[(String, EventModel)]) -> DddCompilation {
    compile_ddd_str(
        "design.ddd",
        source,
        &phero_driver::CompileInputs::from_domains(&domains())
            .with_actors([])
            .with_events(events.to_vec()),
    )
}

fn assert_clean(compilation: &DddCompilation) {
    assert!(
        compilation.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        compilation.diagnostics
    );
}

fn single(compilation: &DddCompilation) -> &(String, phero_driver::Diagnostic) {
    assert_eq!(
        compilation.diagnostics.len(),
        1,
        "expected exactly one diagnostic: {:?}",
        compilation.diagnostics
    );
    &compilation.diagnostics[0]
}

/// The declared universe pools across every provided contract (a host
/// passes all of the application's `.evt` artifacts), and a resolved
/// emission stamps the providing contract's path as `contract`.
#[test]
fn emitted_names_resolve_across_contracts_and_stamp_the_contract_identity() {
    let events = vec![
        (
            "events.evt".to_string(),
            event_model(&["PayRunStateChanged"]),
        ),
        ("extra.evt".to_string(), event_model(&["PayRunSubmitted"])),
    ];
    let compilation = compile_with_events(DESIGN, &events);
    assert_clean(&compilation);

    let model = compilation.model.expect("artifact compiles");
    let service = &model.modules[0].services[0];
    assert_eq!(service.operations[0].emits, vec!["PayRunStateChanged"]);
    let refs = &service.operations[0].emitted_refs;
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name, "PayRunStateChanged");
    assert_eq!(refs[0].contract.as_deref(), Some("events.evt"));
}

/// A plausible typo errors with the code, the operation context, and the
/// did-you-mean help.
#[test]
fn an_unknown_emitted_event_errors_with_e0255_did_you_mean_and_span() {
    let source = DESIGN.replace("emits PayRunStateChanged", "emits PayRunStateChaned");
    let events = vec![(
        "events.evt".to_string(),
        event_model(&["PayRunStateChanged", "PayRunSubmitted"]),
    )];
    let compilation = compile_with_events(&source, &events);
    let (path, diagnostic) = single(&compilation);
    assert_eq!(path, "design.ddd");
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(
        diagnostic.message,
        "E0255 unknown event 'PayRunStateChaned' emitted by operation \
         'approve' of service 'PayRunService'"
    );
    assert_eq!(
        diagnostic.help.as_deref(),
        Some("did you mean 'PayRunStateChanged'?")
    );
}

/// With no plausible typo the help names the declared set.
#[test]
fn the_help_lists_the_declared_events_when_no_typo_is_close() {
    let source = DESIGN.replace("emits PayRunStateChanged", "emits GalaxyExploded");
    let events = vec![(
        "events.evt".to_string(),
        event_model(&["PayRunStateChanged", "PayRunSubmitted"]),
    )];
    let compilation = compile_with_events(&source, &events);
    let (_, diagnostic) = single(&compilation);
    assert_eq!(
        diagnostic.message,
        "E0255 unknown event 'GalaxyExploded' emitted by operation \
         'approve' of service 'PayRunService'"
    );
    assert_eq!(
        diagnostic.help.as_deref(),
        Some("the event contracts declare: PayRunStateChanged, PayRunSubmitted")
    );
}

/// An empty contract bucket lowers the names unvalidated — the plain
/// compile's behavior, never a claim that the application declares no
/// events.
#[test]
fn an_empty_events_bucket_records_the_names_unvalidated() {
    let compilation = compile_with_events(DESIGN, &[]);
    assert_clean(&compilation);

    let model = compilation.model.expect("artifact compiles");
    let refs = &model.modules[0].services[0].operations[0].emitted_refs;
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].contract, None);
}

/// A non-empty bucket with no declaring contract still validates —
/// everything is unknown.
#[test]
fn a_contract_declaring_nothing_validates_everything_unknown() {
    let events = vec![("events.evt".to_string(), event_model(&[]))];
    let compilation = compile_with_events(DESIGN, &events);
    let (_, diagnostic) = single(&compilation);
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(
        diagnostic.help.as_deref(),
        Some("the event contracts declare no events")
    );
}

/// An authored `import "x.evt"` must be satisfied by the bucket when the
/// host provided one (the rule-17 `.dmn` provision shape).
#[test]
fn an_evt_import_must_be_provided_when_the_bucket_is_non_empty() {
    let source = DESIGN.replace("import \"events.evt\";", "import \"other.evt\";");
    let events = vec![(
        "events.evt".to_string(),
        event_model(&["PayRunStateChanged"]),
    )];
    let compilation = compile_with_events(&source, &events);
    let (path, diagnostic) = single(&compilation);
    assert_eq!(path, "design.ddd");
    assert_eq!(
        diagnostic.message,
        "imported event contract \"other.evt\" was not provided"
    );
}

/// An empty events bucket stays silent on an authored `.evt` import — the
/// provision check fires only when the host actually provided contracts.
#[test]
fn an_evt_import_with_an_empty_bucket_stays_silent() {
    let compilation = compile_with_events(DESIGN, &[]);
    assert_clean(&compilation);
}

/// Import matching resolves lexically relative to the design file's
/// directory: a design at `app/design.ddd` importing `events.evt` is
/// satisfied by a bucket entry keyed `app/events.evt`.
#[test]
fn evt_import_matching_resolves_lexically() {
    let events = vec![(
        "app/events.evt".to_string(),
        event_model(&["PayRunStateChanged"]),
    )];
    let compilation = compile_ddd_str(
        "app/design.ddd",
        DESIGN,
        &phero_driver::CompileInputs::from_domains(&[(
            "app/payroll.mox".to_string(),
            DOMAIN.to_string(),
        )])
        .with_actors([])
        .with_events(events),
    );
    assert_clean(&compilation);
}

/// An event name declared by two provided contracts resolves to the first
/// in bucket order (the decision-link no-pin precedence) — documented,
/// deterministic, and never ambiguous without a pin dimension. The bucket
/// order is neither alphabetical nor the import's order, so this pins the
/// host's ordering.
#[test]
fn a_name_in_two_contracts_resolves_to_the_first_in_bucket_order() {
    let events = vec![
        (
            "secondary.evt".to_string(),
            event_model(&["PayRunStateChanged"]),
        ),
        (
            "events.evt".to_string(),
            event_model(&["PayRunStateChanged"]),
        ),
    ];
    let compilation = compile_with_events(DESIGN, &events);
    assert_clean(&compilation);

    let model = compilation.model.expect("artifact compiles");
    let refs = &model.modules[0].services[0].operations[0].emitted_refs;
    assert_eq!(refs[0].contract.as_deref(), Some("secondary.evt"));
}
