//! The strict value-object semantics (DD2, pherolang#299): rules R-V1 (no
//! identity), R-V2 (no owned mutable state), R-V3 (no entity references),
//! and the `abstract value` rejection.
//!
//! A `value` design means **strict** value semantics with no opt-out: the
//! target class has no identity, is immutable and deep-immutable, and its
//! equality is structural. The compiler lowers the fixed
//! [`ValueSemantics::strict`] block onto every value design (and only
//! values), and the rules here refuse the three shapes that would contradict
//! it:
//!
//! - **R-V1** — every `id` feature of the target class (own or inherited,
//!   through [`class_features`]) is an error: a value object has no identity.
//! - **R-V2** — a containment (`contains`) whose class target is an `entity`
//!   design **in the same application** is an error: a value owns no mutable
//!   state, so it may contain values only.
//! - **R-V3** — a cross reference (`refers`) whose class target is a designed
//!   entity in the same application is an error: refer to an entity by id.
//! - **`abstract value`** is an error: a value object is concrete.
//!
//! The rules are **deferred** — they read the completed application
//! registries after every module lowered, so a value design may precede the
//! entity design it must be judged against. They emit after the other
//! application-wide rules in a deterministic order: designs in declaration
//! order, and within one design R-V1 (one diagnostic per identity feature),
//! R-V2, R-V3, then the abstract rejection.
//!
//! Feature-level checks run only against a fully lowered union (the rule-12
//! discipline: a failed domain may hide features and its diagnostics already
//! block the artifact). Every diagnostic spans the value design's authored
//! class name.

use phero_diagnostics::ddd::{
    E_ABSTRACT_VALUE, E_VALUE_CONTAINS_ENTITY, E_VALUE_HAS_IDENTITY, E_VALUE_REFERENCES_ENTITY,
};
use phero_ir::ddd::ValueSemantics;
use phero_syntax::ast as dsl;

use crate::diagnostic::Diagnostic;

use super::*;

/// The fixed semantics block a value design lowers: `Some` for a `value`
/// stereotype, `None` for `entity`/`dto` (the block is a decision, never
/// author input).
pub(crate) fn value_semantics(design: &dsl::DddDesign) -> Option<ValueSemantics> {
    (design.stereotype == dsl::DddStereotype::Value).then(ValueSemantics::strict)
}

/// `value design '<design>' in module '<module>' of application
/// '<application>'` — the context every strict-value diagnostic opens with.
fn context(value: &ValueDesign, application: &str) -> String {
    format!(
        "value design '{}' in module '{}' of application '{}'",
        value.authored, value.module, application
    )
}

/// The deferred strict value-semantics pass: R-V1, R-V2, R-V3, and the
/// `abstract value` rejection, per value design in declaration order. Reads
/// the completed entity-design registry, so an entity designed later in the
/// source still convicts.
pub(crate) fn check_value_semantics(
    value_designs: &[ValueDesign],
    entity_designs: &[EntityDesign],
    union: Option<&phero_ir::Model>,
    application: &str,
) -> Vec<Diagnostic> {
    let mut local: Vec<Diagnostic> = Vec::new();
    for value in value_designs {
        let context = context(value, application);
        // Feature-level rules need the resolved class and the fully lowered
        // union; a failed domain (or rule 3) already reported its own error.
        if let (Some((package, class)), Some(union)) = (value.resolved.as_ref(), union) {
            if let Some(features) = class_features(union, package, class) {
                for feature in &features {
                    // R-V1 — no identity.
                    if feature.is_id {
                        local.push(
                            Diagnostic::error(
                                format!(
                                    "{E_VALUE_HAS_IDENTITY} {context}: feature '{}' carries \
                                     the 'id' modifier; a value object has no identity",
                                    feature.name
                                ),
                                Some(value.span),
                            )
                            .with_help(format!(
                                "remove the 'id' modifier from '{}', or design the class as \
                                 'entity'",
                                feature.name
                            )),
                        );
                    }
                }
                for feature in &features {
                    let phero_ir::TypeRef::Class {
                        package: target_package,
                        name: target_class,
                    } = &feature.type_
                    else {
                        continue;
                    };
                    let target = (target_package, target_class);
                    let is_entity = entity_designs
                        .iter()
                        .any(|design| (&design.package, &design.class) == target);
                    if !is_entity {
                        continue;
                    }
                    match feature.kind {
                        // R-V2 — no owned mutable state.
                        phero_ir::FeatureKind::Containment => local.push(
                            Diagnostic::error(
                                format!(
                                    "{E_VALUE_CONTAINS_ENTITY} {context}: feature '{}' \
                                     contains entity '{}'; a value object owns no mutable \
                                     state",
                                    feature.name, target_class
                                ),
                                Some(value.span),
                            )
                            .with_help(format!(
                                "contain another value object instead, or refer to '{}' by \
                                 id (e.g. 'refers String {}Id')",
                                target_class, feature.name
                            )),
                        ),
                        // R-V3 — no entity references.
                        phero_ir::FeatureKind::CrossReference => local.push(
                            Diagnostic::error(
                                format!(
                                    "{E_VALUE_REFERENCES_ENTITY} {context}: feature '{}' \
                                     refers to entity '{}'; a value object carries no entity \
                                     references",
                                    feature.name, target_class
                                ),
                                Some(value.span),
                            )
                            .with_help(format!(
                                "refer to '{}' by id (e.g. 'refers String {}Id'), or model \
                                 the relationship on the entity",
                                target_class, feature.name
                            )),
                        ),
                        _ => {}
                    }
                }
            }
        }
        // `abstract value` — a value object is concrete. Independent of the
        // union: an abstract design is refusable even when its class does
        // not resolve (rule 3 reports that separately).
        if value.is_abstract {
            local.push(
                Diagnostic::error(
                    format!(
                        "{E_ABSTRACT_VALUE} {context} is abstract; a value object is \
                         concrete"
                    ),
                    Some(value.span),
                )
                .with_help("remove 'abstract' from the design, or design the class as 'entity'"),
            );
        }
    }
    local
}
