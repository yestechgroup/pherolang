//! Collection-kind lowering (DD6, issue #303): `set`/`unique` on attributes,
//! `contains`, `refers`, and `derived` features; the `set ⇒ unique` lowering;
//! and the feature-kind / multiplicity restrictions.

use phero_driver::compile_str;
use phero_ir::{ClassDef, CollectionKind, Feature, Model};

fn compile(source: &str) -> phero_driver::Compilation {
    compile_str(
        "collections.mox",
        source,
        &phero_driver::DomainImports::default(),
    )
}

fn model(source: &str) -> Model {
    let compilation = compile(source);
    assert!(
        compilation.diagnostics.is_empty(),
        "expected a clean compile, got: {:?}",
        compilation.diagnostics
    );
    compilation.model.expect("a model on success")
}

fn feature<'a>(class: &'a ClassDef, name: &str) -> &'a Feature {
    class
        .features
        .iter()
        .find(|feature| feature.name == name)
        .unwrap_or_else(|| panic!("no feature `{name}` on `{}`", class.name))
}

fn errors(source: &str) -> Vec<String> {
    let compilation = compile(source);
    assert!(
        compilation.model.is_none(),
        "errors must block the model: {source}"
    );
    compilation
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.is_error())
        .map(|diagnostic| diagnostic.message.clone())
        .collect()
}

fn assert_error_contains(source: &str, needle: &str) {
    let messages = errors(source);
    assert!(
        messages.iter().any(|message| message.contains(needle)),
        "expected an error containing {needle:?}, got {messages:?}"
    );
}

const PARTIES: &str = "\
class Person { String name }
class Line { String sku }
class Play { String title }
";

#[test]
fn set_lowers_on_a_many_valued_attribute() {
    let model = model(&format!(
        "package demo\n\n{PARTIES}\nclass P {{\n    String[] tags {{ set }}\n    String set\n}}\n"
    ));
    let class = &model.packages[0].classes[3];
    let tags = feature(class, "tags");
    assert_eq!(tags.collection, Some(CollectionKind::Set));
    assert!(tags.constraints.unique, "a set implies unique");
    assert_eq!(
        feature(class, "set").collection,
        None,
        "a feature merely named `set` is a list"
    );
}

#[test]
fn unique_keeps_list_semantics() {
    let model = model(&format!(
        "package demo\n\n{PARTIES}\nclass P {{\n    String[] labels {{ unique }}\n    String[] plain\n}}\n"
    ));
    let class = &model.packages[0].classes[3];
    let labels = feature(class, "labels");
    assert_eq!(labels.collection, None, "a unique list stays a list");
    assert!(labels.constraints.unique);
    let plain = feature(class, "plain");
    assert_eq!(plain.collection, None);
    assert!(!plain.constraints.unique);
}

#[test]
fn writing_unique_and_set_together_is_legal() {
    let model = model(&format!(
        "package demo\n\n{PARTIES}\nclass P {{ String[] tags {{ unique set }} }}\n"
    ));
    let tags = feature(&model.packages[0].classes[3], "tags");
    assert_eq!(tags.collection, Some(CollectionKind::Set));
    assert!(tags.constraints.unique);
}

#[test]
fn set_lowers_on_a_reference_and_a_containment() {
    let model = model(&format!(
        "package demo\n\n{PARTIES}\nclass P {{\n    refers Person[] playedBy {{ set }}\n    contains Line[] lines {{ unique }}\n}}\n"
    ));
    let class = &model.packages[0].classes[3];
    let played_by = feature(class, "playedBy");
    assert_eq!(played_by.collection, Some(CollectionKind::Set));
    assert!(played_by.constraints.unique);
    let lines = feature(class, "lines");
    assert_eq!(lines.collection, None);
    assert!(lines.constraints.unique);
}

#[test]
fn set_lowers_on_a_derived_feature() {
    let model = model(&format!(
        "package demo\n\n{PARTIES}\nclass P {{ derived String[] tags {{ set }} }}\n"
    ));
    let tags = feature(&model.packages[0].classes[3], "tags");
    assert!(tags.is_derived);
    assert_eq!(tags.collection, Some(CollectionKind::Set));
    assert!(tags.constraints.unique);
}

#[test]
fn a_set_may_pair_with_an_opposite() {
    let model = model(
        "package demo\n\n\
         class Person { refers Play[] plays opposite playedBy }\n\
         class Play { refers Person[] playedBy opposite plays { set } }\n",
    );
    let play = &model.packages[0].classes[1];
    let played_by = feature(play, "playedBy");
    assert_eq!(played_by.collection, Some(CollectionKind::Set));
    assert!(played_by.opposite.is_some(), "the opposite survives");
}

#[test]
fn set_on_a_single_valued_attribute_is_rejected() {
    assert_error_contains(
        "package demo\n\nclass P { String tag { set } }\n",
        "constraint 'set' requires a many-valued attribute",
    );
}

#[test]
fn set_on_a_single_valued_reference_is_rejected() {
    assert_error_contains(
        "package demo\n\nclass Person { String name }\nclass P { refers Person[0..1] person { set } }\n",
        "constraint 'set' requires a many-valued reference",
    );
}

#[test]
fn unique_on_a_single_valued_containment_is_rejected() {
    assert_error_contains(
        "package demo\n\nclass Line { String sku }\nclass P { contains Line[1] line { unique } }\n",
        "constraint 'unique' requires a many-valued containment",
    );
}

#[test]
fn set_on_a_single_valued_derived_feature_is_rejected() {
    assert_error_contains(
        "package demo\n\nclass P { derived String tag { set } }\n",
        "constraint 'set' requires a many-valued derived feature",
    );
}

#[test]
fn value_bounds_stay_attribute_only() {
    // Every value-bound keyword names its family, whichever feature kind
    // carries it.
    for (kind, family) in [
        ("pattern \"x\"", "string family"),
        ("minLength 1", "string family"),
        ("maxLength 3", "string family"),
        ("minimum 0", "numeric family"),
        ("maximum 9", "numeric family"),
    ] {
        assert_error_contains(
            &format!(
                "package demo\n\nclass Person {{ String name }}\nclass P {{ refers Person[] people {{ {kind} }} }}\n"
            ),
            family,
        );
        assert_error_contains(
            &format!(
                "package demo\n\nclass Line {{ String sku }}\nclass P {{ contains Line[] lines {{ {kind} }} }}\n"
            ),
            family,
        );
        assert_error_contains(
            &format!("package demo\n\nclass P {{ derived String[] tags {{ {kind} }} }}\n"),
            family,
        );
    }
}

#[test]
fn a_value_bound_names_the_feature_kind() {
    let messages = errors(
        "package demo\n\nclass Person { String name }\nclass P { refers Person[] people { pattern \"x\" } }\n",
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("not on reference 'people'")),
        "{messages:?}"
    );
}

#[test]
fn set_and_value_bounds_coexist_on_an_attribute() {
    let model =
        model("package demo\n\nclass P { String[] tags { minLength 1 set maxLength 8 } }\n");
    let tags = feature(&model.packages[0].classes[0], "tags");
    assert_eq!(tags.collection, Some(CollectionKind::Set));
    assert_eq!(tags.constraints.min_length, Some(1));
    assert_eq!(tags.constraints.max_length, Some(8));
    assert!(tags.constraints.unique);
}

/// The generation-time fallback boundary is the Rust backend's: the driver
/// admits a set of `double` (the IR is backend-neutral), and the backend
/// rejects the element type because `f64` has no total order.
#[test]
fn a_set_of_floats_lowers() {
    let model = model("package demo\n\nclass P { double[] scores { set } }\n");
    let scores = feature(&model.packages[0].classes[0], "scores");
    assert_eq!(scores.collection, Some(CollectionKind::Set));
}

#[test]
fn set_with_a_literal_is_a_parse_error() {
    let messages = errors("package demo\n\nclass P { String[] tags { set 3 } }\n");
    assert!(
        messages
            .iter()
            .any(|message| message.contains("`set` takes no value")),
        "{messages:?}"
    );
}

#[test]
fn duplicate_set_is_a_parse_error() {
    let messages = errors("package demo\n\nclass P { String[] tags { set set } }\n");
    assert!(
        messages
            .iter()
            .any(|message| message.contains("duplicate constraint `set`")),
        "{messages:?}"
    );
}
