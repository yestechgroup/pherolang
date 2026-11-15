//! `ddd` tests: the strict value-object semantics (DD2, pherolang#299) —
//! R-V1 (no identity), R-V2 (no owned mutable state), R-V3 (no entity
//! references), and the `abstract value` rejection.

use crate::support::*;

/// The one diagnostic carrying `code`, with an asserted message shape.
fn violation<'a>(compilation: &'a DddCompilation, code: &str) -> &'a Diagnostic {
    single(compilation, code)
}

/// Every rule's message names the design and its application/module
/// context, so one diagnostic is actionable without the source.
fn assert_names(diagnostic: &Diagnostic, design: &str) {
    for fragment in [
        &format!("value design '{design}'"),
        "module 'm'",
        "application 'A'",
    ] {
        assert!(
            diagnostic.message.contains(fragment),
            "the message names the {fragment:?} context, was: {:?}",
            diagnostic.message
        );
    }
}

/// A one-module design importing `values.mox`.
fn design(body: &str) -> String {
    format!("import \"values.mox\"\napplication A {{\n    module m {{\n{body}    }}\n}}\n")
}

/// The entity/dto shapes of the negative-context tests: the same class
/// skeletons are legal under the other stereotypes.
const PLAIN_DOMAIN: &str = r#"
package nz.example.values

class Library {
    String name
}

class Summary {
    String label
}

class Sketch {
    readonly String label
}
"#;

#[test]
fn a_value_design_lowers_the_fixed_semantics_block() {
    let domain = r#"
package nz.example.values

enum Currency {
    Nzd as "NZD" = 0
}

class Money {
    readonly int cents
    readonly Currency currency
}
"#;
    let source = design("        value Money\n");
    let compilation = compile(&source, &[("values.mox", domain)]);
    assert_clean(&compilation);
    let model = compilation.model.expect("the value design lowers");
    assert_eq!(
        model.modules[0].designs[0].semantics,
        Some(phero_ir::ddd::ValueSemantics {
            identity: phero_ir::ddd::IdentityPolicy::None,
            immutable: true,
            deep_immutable: true,
            equality: phero_ir::ddd::EqualityPolicy::Structural,
        })
    );
}

#[test]
fn entity_and_dto_designs_carry_no_semantics_block() {
    let source = design("        entity Library\n        dto Summary nonPersistent\n");
    let compilation = compile(&source, &[("values.mox", PLAIN_DOMAIN)]);
    assert_clean(&compilation);
    let model = compilation.model.expect("the designs lower");
    assert_eq!(model.modules[0].designs[0].semantics, None);
    assert_eq!(model.modules[0].designs[1].semantics, None);
}

// --- R-V1: no identity -------------------------------------------------------

#[test]
fn r_v1_a_value_design_rejects_an_identity_feature() {
    let domain = r#"
package nz.example.values

class Wallet {
    id readonly String walletId
    readonly int cents
}
"#;
    let source = design("        value Wallet\n");
    let compilation = compile(&source, &[("values.mox", domain)]);
    let diagnostic = violation(&compilation, "E0361");
    assert_names(diagnostic, "Wallet");
    assert!(
        diagnostic.message.contains("feature 'walletId'"),
        "the message names the identity feature, was: {:?}",
        diagnostic.message
    );
    assert_eq!(diagnostic.span, Some(span_of(&source, "Wallet", 0)));
    assert!(
        diagnostic
            .help
            .as_deref()
            .is_some_and(|help| help.contains("id") && help.contains("entity")),
        "the help names the fix, was: {:?}",
        diagnostic.help
    );
    assert!(compilation.model.is_none(), "the artifact drops");
}

// --- R-V2: no owned mutable state -------------------------------------------

#[test]
fn r_v2_a_value_design_rejects_a_contained_entity() {
    let domain = r#"
package nz.example.values

class Address {
    readonly String street
    readonly contains Owner owner
}

class Owner {
    String name
}
"#;
    // The rule reads the completed application registry, so the entity
    // design may follow the value design in source order.
    let source = design("        value Address\n        entity Owner\n");
    assert!(source.find("value Address") < source.find("entity Owner"));
    let compilation = compile(&source, &[("values.mox", domain)]);
    let diagnostic = violation(&compilation, "E0362");
    assert_names(diagnostic, "Address");
    assert!(
        diagnostic.message.contains("feature 'owner'"),
        "the message names the containment feature, was: {:?}",
        diagnostic.message
    );
    assert!(
        diagnostic.message.contains("Owner"),
        "the message names the contained entity, was: {:?}",
        diagnostic.message
    );
    assert_eq!(diagnostic.span, Some(span_of(&source, "Address", 0)));
    assert!(
        diagnostic
            .help
            .as_deref()
            .is_some_and(|help| help.contains("value") && help.contains("id")),
        "the help names the fixes (contain a value / refer by id), was: {:?}",
        diagnostic.help
    );
    assert!(compilation.model.is_none(), "the artifact drops");
}

// --- R-V3: no entity references ---------------------------------------------

#[test]
fn r_v3_a_value_design_rejects_an_entity_reference() {
    let domain = r#"
package nz.example.values

class Account {
    readonly String number
    readonly refers Owner owner
}

class Owner {
    String name
}
"#;
    let source = design("        value Account\n        entity Owner\n");
    let compilation = compile(&source, &[("values.mox", domain)]);
    let diagnostic = violation(&compilation, "E0363");
    assert_names(diagnostic, "Account");
    assert!(
        diagnostic.message.contains("feature 'owner'"),
        "the message names the reference feature, was: {:?}",
        diagnostic.message
    );
    assert!(
        diagnostic.message.contains("Owner"),
        "the message names the referenced entity, was: {:?}",
        diagnostic.message
    );
    assert_eq!(diagnostic.span, Some(span_of(&source, "Account", 0)));
    assert!(
        diagnostic
            .help
            .as_deref()
            .is_some_and(|help| help.contains("String") && help.contains("id")),
        "the help shows the refer-by-id spelling, was: {:?}",
        diagnostic.help
    );
    assert!(compilation.model.is_none(), "the artifact drops");
    assert!(
        !compilation
            .diagnostics
            .iter()
            .any(|(_, diagnostic)| diagnostic.message.contains("E0362")),
        "a reference is R-V3, never the containment R-V2"
    );
}

// --- abstract value ----------------------------------------------------------

#[test]
fn an_abstract_value_design_is_rejected() {
    let source = design("        abstract value Sketch\n");
    let compilation = compile(&source, &[("values.mox", PLAIN_DOMAIN)]);
    let diagnostic = violation(&compilation, "E0364");
    assert_names(diagnostic, "Sketch");
    assert_eq!(diagnostic.span, Some(span_of(&source, "Sketch", 0)));
    assert!(compilation.model.is_none(), "the artifact drops");
}

// --- happy path --------------------------------------------------------------

/// The rules are scoped to `value` designs: the same class shapes are legal
/// as an entity design, and a value design may nest values by value,
/// reference values, and carry enum/datatype fields.
#[test]
fn value_semantics_leave_entities_and_sibling_values_alone() {
    let domain = r#"
package nz.example.values

enum Currency {
    Nzd as "NZD" = 0
}

type Code wraps opaque {
    rust "std::string::String"
}

class Ledger {
    id readonly String ledgerId
    readonly contains Owner owner
}

class Owner {
    String name
}

class Money {
    readonly int cents
    readonly Currency currency
    readonly Code code
}

class PriceRange {
    readonly contains Money min
    readonly contains Money max
    readonly refers Money preferred
}
"#;
    let source = design(
        "        entity Ledger\n        entity Owner\n        value Money\n        value PriceRange nonPersistent\n",
    );
    let compilation = compile(&source, &[("values.mox", domain)]);
    assert_clean(&compilation);
    assert!(compilation.model.is_some());
}
