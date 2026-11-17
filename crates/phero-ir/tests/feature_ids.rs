//! Wire rule 3 (register V10): a [`Feature`]'s stable id equals its
//! 0-based declaration index within the owning class, and
//! [`ClassDef::assign_feature_ids`] establishes exactly that. The
//! Creusot contract proves it for all inputs; these tests pin the
//! behavior executably.

use phero_ir::{ClassDef, Feature, FeatureKind, Multiplicity, TypeRef};

fn feature(name: &str) -> Feature {
    Feature::new(
        name,
        FeatureKind::Attribute,
        TypeRef::Primitive(phero_ir::PrimitiveType::String),
        Multiplicity::REQUIRED,
    )
}

#[test]
fn ids_equal_declaration_indices() {
    let class = ClassDef::new(
        "Account",
        vec![],
        vec![feature("bank"), feature("number"), feature("balance")],
    );
    for (index, f) in class.features.iter().enumerate() {
        assert_eq!(f.id, index as u32, "feature '{}'", f.name);
    }
}

#[test]
fn reassignment_follows_reordering() {
    let mut class = ClassDef::new(
        "Account",
        vec![],
        vec![feature("bank"), feature("number"), feature("balance")],
    );
    // Authoring-time reorder: ids must follow the new indices.
    class.features.swap(0, 2);
    class.assign_feature_ids();
    for (index, f) in class.features.iter().enumerate() {
        assert_eq!(f.id, index as u32, "feature '{}'", f.name);
    }
}

#[test]
fn assignment_is_idempotent() {
    let mut class = ClassDef::new("Account", vec![], vec![feature("bank"), feature("number")]);
    let before: Vec<(String, u32)> = class
        .features
        .iter()
        .map(|f| (f.name.clone(), f.id))
        .collect();
    class.assign_feature_ids();
    let after: Vec<(String, u32)> = class
        .features
        .iter()
        .map(|f| (f.name.clone(), f.id))
        .collect();
    assert_eq!(before, after);
}

#[test]
fn inserting_a_feature_renumbers_later_ids() {
    // DF2 (issue pherolang#325): the id is an intra-publication ordinal.
    // Inserting a feature renumbers every later slot — pin the behaviour as
    // intended, and use (package, class, feature name) for identity across
    // publications instead.
    let mut class = ClassDef::new("Account", vec![], vec![feature("bank"), feature("balance")]);
    let before: Vec<(String, u32)> = class
        .features
        .iter()
        .map(|f| (f.name.clone(), f.id))
        .collect();
    assert_eq!(
        before,
        vec![("bank".to_string(), 0), ("balance".to_string(), 1)]
    );

    class.features.insert(1, feature("number"));
    class.assign_feature_ids();
    let after: Vec<(String, u32)> = class
        .features
        .iter()
        .map(|f| (f.name.clone(), f.id))
        .collect();
    assert_eq!(
        after,
        vec![
            ("bank".to_string(), 0),
            ("number".to_string(), 1),
            ("balance".to_string(), 2),
        ],
        "balance's id moved from 1 to 2 — a persisted id would re-point"
    );
}

#[test]
fn feature_lookup_resolves_by_id() {
    let class = ClassDef::new(
        "Account",
        vec![],
        vec![feature("bank"), feature("number"), feature("balance")],
    );
    assert_eq!(class.feature(0).map(|f| f.name.as_str()), Some("bank"));
    assert_eq!(class.feature(2).map(|f| f.name.as_str()), Some("balance"));
    assert!(class.feature(3).is_none(), "no feature past the last index");
}
