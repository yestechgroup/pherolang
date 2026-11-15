//! Declared error vocabulary and `throws` (DD7, issue pherolang#304): the
//! declarations, their visibility and uniqueness rules, the resolved
//! `throws` references, the delegation superset contract, the body raise
//! subset, and the orchestration-only posture.

use phero_ir::ddd::{DddTypeRef, ErrorScope};

use crate::support::{assert_clean, compile, single, span_of};

/// A small payments domain: an aggregate root with a repository, an
/// entity-payload candidate, and an enum used by an error payload.
const PAYMENTS: &str = r#"
package test.payments

enum DeclineReason {
    InsufficientFunds as "I" = 0
}

class Order {
    String id
    String label
}

class Customer {
    String label
}
"#;

/// The clean fixture: application errors (one with a doc comment and a
/// typed payload, one empty), module-private errors (one carrying a DTO
/// payload), and `throws` on a declared body operation, a delegation, and a
/// declared repository operation.
const DESIGN: &str = r#"
import "payments.mox";

application Payments {
    base test.payments

    /// The payment was refused by the provider.
    error PaymentDeclined(DeclineReason reason, String reference);
    error OutOfStock(String sku);

    module billing {
        error InvoiceAlreadyIssued(String invoiceId);
        error DeclinedDetail(DeclineDetail detail);
        dto DeclineDetail { String reference; String reason; }
        service PaymentService {
            inject OrderRepository;
            boolean pay(String id) throws PaymentDeclined, OutOfStock, NotFound, DeclinedDetail => OrderRepository {
                let order = self.findById(id) else error NotFound("Order", id);
                guard order.label != "" else error PaymentDeclined(DeclineReason.InsufficientFunds, id);
                let saved = self.save(order);
                return saved.label != "";
            }
        }
        service DelegatingService {
            inject PaymentService;
            pay => PaymentService.pay throws PaymentDeclined, OutOfStock, NotFound, DeclinedDetail;
        }
        entity Order repository OrderRepository {
            findById;
            boolean capture(String id) throws NotFound, PaymentDeclined when (label != "") {
                let order = self.findById(id) else error NotFound("Order", id);
                guard order.label != "" else error PaymentDeclined(DeclineReason.InsufficientFunds, id);
                return true;
            }
            save;
        }
        entity Customer
    }
}
"#;

/// `DeclinedDetail` is a declared-but-never-raised entry in the fixture (the
/// DTO payload is not constructible from a body), which is legal: `throws`
/// is an interface promise, never inferred from the body.
fn _declined_detail_is_an_interface_promise() {}

fn payments(source: &str) -> phero_driver::DddCompilation {
    compile(source, &[("payments.mox", PAYMENTS)])
}

fn errors_of(source: &str) -> Vec<String> {
    payments(source)
        .diagnostics
        .into_iter()
        .filter(|(path, diagnostic)| {
            path.ends_with(".ddd")
                && diagnostic.severity == phero_driver::diagnostic::Severity::Error
        })
        .map(|(_, diagnostic)| diagnostic.message)
        .collect()
}

fn assert_error(source: &str, expected: &str) {
    let errors = errors_of(source);
    assert!(
        errors.iter().any(|message| message.contains(expected)),
        "expected an error containing {expected:?}, got: {errors:?}"
    );
}

/// The declarations lower to the artifact's sparse `errors` lists: root
/// scope at the model, module scope on the module, payloads typed (a DTO
/// payload included), descriptions from the doc comment.
#[test]
fn error_declarations_lower_with_scope_and_payload() {
    let compilation = payments(DESIGN);
    assert_clean(&compilation);
    let model = compilation.model.expect("the design lowered");
    assert_eq!(
        model
            .errors
            .iter()
            .map(|error| error.name.as_str())
            .collect::<Vec<_>>(),
        ["PaymentDeclined", "OutOfStock"]
    );
    let declined = &model.errors[0];
    assert_eq!(
        declined.description.as_deref(),
        Some("The payment was refused by the provider.")
    );
    assert_eq!(declined.fields.len(), 2);
    assert!(matches!(
        declined.fields[0].type_,
        DddTypeRef::Domain(phero_ir::TypeRef::Enum { .. })
    ));
    assert_eq!(model.errors[1].fields.len(), 1);
    assert_eq!(model.errors[1].fields[0].name, "sku");

    let module = &model.modules[0];
    assert_eq!(
        module
            .errors
            .iter()
            .map(|error| error.name.as_str())
            .collect::<Vec<_>>(),
        ["InvoiceAlreadyIssued", "DeclinedDetail"]
    );
    assert!(matches!(
        module.errors[1].fields[0].type_,
        DddTypeRef::Dto { ref dto } if dto == "DeclineDetail"
    ));
}

/// The resolved `throws` references carry their scope: an application-level
/// error, the module's own private error, and the prelude.
#[test]
fn throws_resolves_application_module_and_prelude_scopes() {
    let compilation = payments(DESIGN);
    assert_clean(&compilation);
    let model = compilation.model.expect("the design lowered");
    let service = &model.modules[0].services[0];
    let pay = service
        .operations
        .iter()
        .find(|operation| operation.name == "pay")
        .expect("pay");
    let resolved: Vec<(String, ErrorScope, Option<String>)> = pay
        .throws
        .iter()
        .map(|reference| {
            (
                reference.name.clone(),
                reference.scope,
                reference.module.clone(),
            )
        })
        .collect();
    assert_eq!(
        resolved,
        vec![
            ("PaymentDeclined".to_string(), ErrorScope::Application, None),
            ("OutOfStock".to_string(), ErrorScope::Application, None),
            ("NotFound".to_string(), ErrorScope::Prelude, None),
            (
                "DeclinedDetail".to_string(),
                ErrorScope::Module,
                Some("billing".to_string())
            ),
        ]
    );

    let repository = model.modules[0]
        .designs
        .iter()
        .find(|design| design.class.as_deref() == Some("Order"))
        .and_then(|design| design.repository.as_ref())
        .expect("repository");
    let capture = repository
        .operations
        .iter()
        .find(|operation| operation.name == "capture")
        .expect("capture");
    assert_eq!(capture.throws.len(), 2);
    assert_eq!(capture.throws[0].scope, ErrorScope::Prelude);
    assert_eq!(capture.throws[1].scope, ErrorScope::Application);
    // A builtin takes no `throws`.
    assert!(repository.operations[0].throws.is_empty());
}

/// A delegation whose `throws` is a superset of its target's is legal.
#[test]
fn delegation_throws_superset_is_legal() {
    let compilation = payments(DESIGN);
    assert_clean(&compilation);
}

/// An unknown `throws` name is E0373, with the in-scope vocabulary.
#[test]
fn unknown_thrown_error_is_e0373() {
    let source = DESIGN.replace(
        "boolean pay(String id) throws PaymentDeclined, OutOfStock, NotFound, DeclinedDetail =>",
        "boolean pay(String id) throws PaymentDeclined, Nope =>",
    );
    assert_error(
        &source,
        "E0373 unknown error 'Nope' in throws of operation 'pay'",
    );
}

/// Error names are application-wide unique, and the prelude cannot be
/// shadowed (E0374).
#[test]
fn duplicate_and_prelude_shadowed_errors_are_e0374() {
    let duplicate = DESIGN.replace(
        "    error OutOfStock(String sku);",
        "    error OutOfStock(String sku);\n    error PaymentDeclined(String reference);",
    );
    assert_error(&duplicate, "E0374 duplicate error 'PaymentDeclined'");

    let shadow = DESIGN.replace(
        "    error OutOfStock(String sku);",
        "    error OutOfStock(String sku);\n    error NotFound(String resource, String id);",
    );
    assert_error(&shadow, "E0374 error 'NotFound' shadows a prelude error");
}

/// A module-private error cannot be named from another module (E0375); the
/// diagnostic names the owning module and the application-scope fix.
#[test]
fn module_private_error_is_e0375() {
    let source = DESIGN.replace(
        "        entity Customer\n",
        "        entity Customer\n    }\n    module shipping {\n        service ShippingService {\n            boolean ship() throws InvoiceAlreadyIssued;\n        }\n",
    );
    assert_error(
        &source,
        "E0375 error 'InvoiceAlreadyIssued' is private to module",
    );
}

/// An error payload may not embed a class designed as an entity (E0376).
#[test]
fn entity_error_payload_is_e0376() {
    let source = DESIGN.replace(
        "    error OutOfStock(String sku);",
        "    error OutOfStock(String sku);\n    error BadOrder(Order order);",
    );
    assert_error(
        &source,
        "E0376 error 'BadOrder' field 'order' carries entity",
    );
    assert_error(&source, "an error payload is data");
}

/// A delegation may not narrow its target's `throws` (E0377), naming each
/// missing error.
#[test]
fn narrowed_delegation_throws_is_e0377() {
    let source = DESIGN.replace(
        "pay => PaymentService.pay throws PaymentDeclined, OutOfStock, NotFound, DeclinedDetail;",
        "pay => PaymentService.pay throws PaymentDeclined;",
    );
    let errors = errors_of(&source);
    for missing in ["OutOfStock", "NotFound", "DeclinedDetail"] {
        assert!(
            errors
                .iter()
                .any(|message| message.contains("E0377") && message.contains(missing)),
            "expected a narrowed-throws error naming {missing}, got: {errors:?}"
        );
    }
    assert!(
        errors
            .iter()
            .any(|message| message.contains("narrows the throws of 'PaymentService.pay'")),
        "{errors:?}"
    );
}

/// A duplicate payload field is E0378; the first field wins the artifact.
#[test]
fn duplicate_error_field_is_e0378() {
    let source = DESIGN.replace(
        "    error PaymentDeclined(DeclineReason reason, String reference);",
        "    error PaymentDeclined(DeclineReason reason, String reference, String reason);",
    );
    assert_error(
        &source,
        "E0378 duplicate field 'reason' in error 'PaymentDeclined'",
    );
}

/// A body raise must be declared in the operation's `throws` — explicit and
/// complete, never inferred (E0360 naming the declared set).
#[test]
fn undeclared_body_raise_is_e0360() {
    let source = DESIGN.replace(
        "boolean pay(String id) throws PaymentDeclined, OutOfStock, NotFound, DeclinedDetail => OrderRepository {",
        "boolean pay(String id) throws PaymentDeclined, OutOfStock, DeclinedDetail => OrderRepository {",
    );
    assert_error(&source, "E0360");
    assert_error(
        &source,
        "error `NotFound` is not declared in the operation's `throws`",
    );
    assert_error(
        &source,
        "declared: PaymentDeclined, OutOfStock, DeclinedDetail",
    );

    let none = DESIGN.replace(
        "boolean pay(String id) throws PaymentDeclined, OutOfStock, NotFound, DeclinedDetail => OrderRepository {",
        "boolean pay(String id) => OrderRepository {",
    );
    assert_error(&none, "declares no `throws` clause");
}

/// A declared raise types exactly against the declared payload: arity and
/// field type, naming the offending field.
#[test]
fn body_raise_payload_mismatch_is_e0360() {
    let arity = DESIGN.replace(
        "guard order.label != \"\" else error PaymentDeclined(DeclineReason.InsufficientFunds, id);",
        "guard order.label != \"\" else error PaymentDeclined(DeclineReason.InsufficientFunds);",
    );
    assert_error(
        &arity,
        "error `PaymentDeclined` expects 2 arguments, found 1",
    );
    assert_error(
        &arity,
        "PaymentDeclined(reason: enum `DeclineReason`, reference: string)",
    );

    let wrong_type = DESIGN.replace(
        "guard order.label != \"\" else error PaymentDeclined(DeclineReason.InsufficientFunds, id);",
        "guard order.label != \"\" else error PaymentDeclined(id, id);",
    );
    assert_error(&wrong_type, "error argument `reason` types string");
    assert_error(
        &wrong_type,
        "declared payload field is enum `DeclineReason`",
    );
}

/// The orchestration-only posture is enforced in a body: a whole-local
/// reassignment and a cross-aggregate load are E0360.
#[test]
fn orchestration_only_body_rules_are_e0360() {
    let reassign = DESIGN.replace(
        "                let saved = self.save(order);",
        "                let other = order;\n                other = order;\n                let saved = self.save(order);",
    );
    assert_error(&reassign, "E0360");
    assert_error(&reassign, "cannot reassign the local 'other'");

    // A second injected repository may not load another aggregate.
    let cross = DESIGN.replace(
        "            inject OrderRepository;\n            boolean pay(String id) throws PaymentDeclined, OutOfStock, NotFound, DeclinedDetail => OrderRepository {\n                let order = self.findById(id) else error NotFound(\"Order\", id);",
        "            inject OrderRepository;\n            inject CustomerRepository;\n            boolean pay(String id) throws PaymentDeclined, OutOfStock, NotFound, DeclinedDetail => OrderRepository {\n                let order = self.findById(id) else error NotFound(\"Order\", id);\n                let customer = CustomerRepository.findById(id);",
    );
    let cross = cross.replace(
        "        entity Customer\n",
        "        entity Customer repository CustomerRepository { findById; }\n",
    );
    assert_error(&cross, "E0360");
    assert_error(
        &cross,
        "`CustomerRepository.findById` is a cross-aggregate load",
    );
}

/// The declared bodies stay attached to the artifact when the vocabulary is
/// complete: the service body and the repository body both lower, and the
/// service records its `throws`.
#[test]
fn declared_error_vocabulary_lowers_with_the_bodies() {
    let compilation = payments(DESIGN);
    assert_clean(&compilation);
    let model = compilation.model.expect("the design lowered");
    let pay = &model.modules[0].services[0].operations[0];
    assert_eq!(pay.bodies.len(), 1, "the checked body is attached");
    assert!(pay.body_receiver.is_some());
    let repository = model.modules[0]
        .designs
        .iter()
        .find(|design| design.class.as_deref() == Some("Order"))
        .and_then(|design| design.repository.as_ref())
        .expect("repository");
    let capture = repository
        .operations
        .iter()
        .find(|operation| operation.name == "capture")
        .expect("capture");
    assert_eq!(capture.bodies.len(), 1);
}

/// The application error declaration's name span is the diagnostic site of
/// the entity-payload rule.
#[test]
fn entity_payload_diagnostic_spans_the_declaration() {
    let source = DESIGN.replace(
        "    error OutOfStock(String sku);",
        "    error OutOfStock(String sku);\n    error BadOrder(Order order);",
    );
    let compilation = payments(&source);
    let declaration = span_of(&source, "BadOrder", 0);
    let diagnostic = single(&compilation, "E0376");
    assert_eq!(diagnostic.span, Some(declaration));
}
