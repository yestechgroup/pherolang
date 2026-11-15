//! Collection-kind parsing (DD6, issue #303): the value-less `set` constraint,
//! its `unique` sibling, and the four feature kinds that admit a constraint
//! block — attributes, `contains`, `refers`, and `derived`.

use phero_syntax::ast::*;
use phero_syntax::parse;

fn class(source: &str) -> Result<ClassDecl, Vec<String>> {
    let result = parse(source);
    let errors = result
        .errors
        .iter()
        .map(|error| error.message.clone())
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        return Err(errors);
    }
    let model = result.ast.expect("an AST on success");
    match model.declarations.into_iter().next() {
        Some(Decl::Class(class)) => Ok(class),
        other => panic!("expected a class declaration, found {other:?}"),
    }
}

fn constraints(feature: &FeatureDecl) -> &[Constraint] {
    match feature {
        FeatureDecl::Attribute { constraints, .. } => constraints,
        FeatureDecl::Containment { constraints, .. } => constraints,
        FeatureDecl::Reference { constraints, .. } => constraints,
        FeatureDecl::Derived { constraints, .. } => constraints,
        other => panic!("`{}` carries no constraint block", other.kind()),
    }
}

fn assert_flags(feature: &FeatureDecl, expected: &[&str]) {
    let names = constraints(feature)
        .iter()
        .map(|constraint| {
            assert!(
                matches!(constraint.value, ConstraintValue::Flag { .. }),
                "`{}` must be value-less",
                constraint.name.text
            );
            constraint.name.text.as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        expected,
        "constraint flags of `{}`",
        feature.name().text
    );
}

#[test]
fn set_parses_on_a_many_valued_attribute() {
    let class = class("class P { String[] tags { set } }").expect("parses");
    assert_flags(&class.features[0], &["set"]);
}

#[test]
fn set_and_unique_parse_together() {
    let class = class("class P { String[] tags { unique set } }").expect("parses");
    assert_flags(&class.features[0], &["unique", "set"]);
}

#[test]
fn set_parses_on_a_reference() {
    let class = class("class P { refers Person[] playedBy { set } }").expect("parses");
    let FeatureDecl::Reference {
        type_ref,
        multiplicity,
        opposite,
        ..
    } = &class.features[0]
    else {
        panic!("expected a reference");
    };
    assert_eq!(type_ref.name.full_name(), "Person");
    assert!(matches!(
        multiplicity.as_ref().map(|m| &m.kind),
        Some(MultiplicityKind::Unbounded)
    ));
    assert!(opposite.is_none());
    assert_flags(&class.features[0], &["set"]);
}

#[test]
fn set_parses_after_the_opposite_clause() {
    let class = class("class P { contains Line[] lines opposite run { set } }").expect("parses");
    let FeatureDecl::Containment { opposite, .. } = &class.features[0] else {
        panic!("expected a containment");
    };
    assert_eq!(
        opposite.as_ref().map(|name| name.text.as_str()),
        Some("run")
    );
    assert_flags(&class.features[0], &["set"]);
}

#[test]
fn unique_parses_on_containment_and_reference() {
    let class =
        class("class P {\n  contains Line[] lines { unique }\n  refers Tag[] tags { unique }\n}")
            .expect("parses");
    assert_flags(&class.features[0], &["unique"]);
    assert_flags(&class.features[1], &["unique"]);
}

#[test]
fn derived_takes_a_constraint_block_without_a_body() {
    let class = class("class P { derived Person[] active { set } }").expect("parses");
    let FeatureDecl::Derived { body, bodies, .. } = &class.features[0] else {
        panic!("expected a derived feature");
    };
    assert!(body.is_none(), "no body was written");
    assert!(bodies.is_empty(), "no target bodies were written");
    assert_flags(&class.features[0], &["set"]);
}

#[test]
fn derived_body_is_followed_by_its_constraint_block() {
    let class = class(
        "class P { derived Person[] active { expr { members.filter(m => m.active) } } { set } }",
    )
    .expect("parses");
    let FeatureDecl::Derived { body, bodies, .. } = &class.features[0] else {
        panic!("expected a derived feature");
    };
    assert!(body.is_some(), "the tagged body span is retained");
    assert_eq!(bodies.len(), 1);
    assert_eq!(bodies[0].target.text, "expr");
    assert_flags(&class.features[0], &["set"]);
}

#[test]
fn derived_body_only_parses() {
    let class = class("class P { derived String label { expr { \"x\" } } }").expect("parses");
    assert!(constraints(&class.features[0]).is_empty());
}

#[test]
fn derived_constraint_block_does_not_swallow_an_expression() {
    // `{ expr { ... } }` begins with a non-constraint identifier: it is a
    // tagged body, not a constraint block (the tagged-first resolution).
    let class = class("class P { derived String label { expr { title } } }").expect("parses");
    let FeatureDecl::Derived { bodies, .. } = &class.features[0] else {
        panic!("expected a derived feature");
    };
    assert_eq!(bodies.len(), 1, "{bodies:?}");
}

#[test]
fn attribute_named_set_parses_and_can_carry_the_set_constraint() {
    let class = class("class P { String set\n  String[] tags { set }\n}").expect("parses");
    assert_eq!(class.features[0].name().text, "set");
    assert_flags(&class.features[1], &["set"]);
}

#[test]
fn escaped_set_is_a_plain_identifier() {
    // `^set` is never the constraint: the escaped token does not enter a
    // constraint block, so the block rejects it.
    assert!(class("class P { String ^set }").is_ok());
    assert!(class("class P { String[] tags { ^set } }").is_err());
}

#[test]
fn set_takes_no_value() {
    let errors = class("class P { String[] tags { set 3 } }").expect_err("must not parse");
    assert!(
        errors
            .iter()
            .any(|error| error.contains("`set` takes no value")),
        "{errors:?}"
    );
}

#[test]
fn duplicate_set_is_rejected() {
    let errors = class("class P { String[] tags { set set } }").expect_err("must not parse");
    assert!(
        errors
            .iter()
            .any(|error| error.contains("duplicate constraint `set`")),
        "{errors:?}"
    );
}

#[test]
fn containers_do_not_take_a_constraint_block() {
    // A container is the single-valued owner slot: the grammar admits no
    // constraint block, so the block is recovered as junk.
    assert!(class("class P { container Box box { set } }").is_err());
}

#[test]
fn set_stays_an_ordinary_identifier_elsewhere() {
    // Contextual word: `set` names a class, a feature, an enum literal, and a
    // parameter without escaping.
    let source = concat!(
        "class set {\n",
        "  String set\n",
        "  op set get(set set)\n",
        "}\n",
        "enum set { set = 0 }\n"
    );
    let result = parse(source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}
