//! Class, feature, and constraint lowering: the `.mox` feature kinds,
//! target bodies (`rust`/`expr`), relation and multiplicity lowering,
//! defaults, constraint families, and the class-graph validation passes
//! (inheritance cycles, opposites).

use std::collections::{HashMap, HashSet};

use phero_ir as ir;
use phero_syntax::ast as mox;
use phero_syntax::Span;

use crate::diagnostic::{Diagnostic, DiagnosticCode};
use crate::engine_common;

use super::compile::lower_target_bodies;
use super::*;

/// `Some(class name)` when the resolution classified the type as a class.
fn class_of(resolution: Option<&Resolution>) -> Option<String> {
    class_location(resolution).map(|(_, name)| name)
}

/// `Some((package, name))` of the target when the resolution classified the
/// type as a class.
fn class_location(resolution: Option<&Resolution>) -> Option<(String, String)> {
    match resolution {
        Some(resolution) if resolution.kind == Resolved::Class => {
            Some((resolution.package.clone(), resolution.name.clone()))
        }
        _ => None,
    }
}

/// Applies the AST `id`/`readonly` modifiers to a lowered IR feature.
/// Repeats in the source are idempotent, so the builder calls run at most once.
fn apply_modifiers(feature: ir::Feature, modifiers: &mox::Modifiers) -> ir::Feature {
    let feature = if modifiers.is_id() {
        feature.identifier()
    } else {
        feature
    };
    if modifiers.is_read_only() {
        feature.read_only()
    } else {
        feature
    }
}

/// The constraint space an attribute's resolved type admits.
#[derive(Clone, Copy)]
enum ConstraintSpace<'a> {
    /// Only the string family applies (`string` primitives, opaque
    /// datatypes, `String`-keyed vocabularies).
    String,
    /// Only the numeric family applies (numeric primitives, numeric-keyed
    /// vocabularies).
    Numeric,
    /// Neither family applies (`char`, `boolean`).
    Neither,
    /// Both families apply: string constraints bound literal names, numeric
    /// constraints bound literal values.
    Enum(&'a mox::EnumDecl),
}

impl ConstraintSpace<'_> {
    fn admits_string_family(self) -> bool {
        matches!(self, Self::String | Self::Enum(_))
    }

    fn admits_numeric_family(self) -> bool {
        matches!(self, Self::Numeric | Self::Enum(_))
    }
}

/// The diagnostic for a constraint keyword whose family the attribute's
/// type does not admit. The message names the deciding type knowledge so
/// the fix is apparent from the error alone.
fn family_mismatch(
    keyword: &str,
    family: &str,
    subject: &str,
    resolution: Option<&Resolution>,
    prep: &PrepMaps<'_>,
    span: Option<Span>,
) -> Diagnostic {
    let message = match resolution {
        Some(Resolution {
            kind: Resolved::Datatype,
            name,
            ..
        }) => format!(
            "constraint '{keyword}' requires a {family} attribute; datatype '{name}' has an opaque platform type"
        ),
        Some(Resolution {
            kind: Resolved::Vocabulary,
            name,
            package,
        }) => match prep
            .vocab_key_facet_types
            .get(&(package.as_str(), name.as_str()))
        {
            Some(primitive) => format!(
                "constraint '{keyword}' requires a {family} attribute; the key facet of vocabulary '{name}' is {primitive}"
            ),
            None => format!(
                "constraint '{keyword}' requires a {family} attribute; vocabulary '{name}' has no resolvable key facet"
            ),
        },
        _ => format!("constraint '{keyword}' requires a {family} attribute, which {subject} is not"),
    };
    Diagnostic::error(message, span)
}

/// The feature kinds that admit a constraint block (DD6, issue #303).
/// Attributes admit the value bounds plus the collection constraints; the
/// relationship kinds and `derived` admit only `unique`/`set`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConstraintTarget {
    /// An ordinary value-holding attribute.
    Attribute,
    /// A `contains` feature.
    Containment,
    /// A `refers` feature.
    Reference,
    /// The single-valued owner slot (`container`): never carries a block in
    /// the grammar, but the noun keeps diagnostics honest.
    Container,
    /// A `derived` feature.
    Derived,
}

impl ConstraintTarget {
    fn noun(self) -> &'static str {
        match self {
            Self::Attribute => "attribute",
            Self::Containment => "containment",
            Self::Reference => "reference",
            Self::Container => "container",
            Self::Derived => "derived feature",
        }
    }
}

/// The lowered constraint block: the value/duplicate constraints plus the
/// collection kind a `set` selects.
pub(crate) struct LoweredConstraints {
    /// The value bounds and the duplicate rule.
    pub constraints: ir::FeatureConstraints,
    /// `Some(Set)` when the block carried `set`; `None` is an ordered list.
    pub collection: Option<ir::CollectionKind>,
}

impl LoweredConstraints {
    fn empty() -> Self {
        Self {
            constraints: ir::FeatureConstraints::default(),
            collection: None,
        }
    }
}

/// The constraint family a value-bound keyword belongs to (named in the
/// attribute-only rejection so the fix is apparent).
fn constraint_family(keyword: &str) -> &'static str {
    match keyword {
        "pattern" | "minLength" | "maxLength" => "string family",
        "minimum" | "maximum" => "numeric family",
        _ => "value family",
    }
}

/// The non-attribute constraint path (DD6, issue #303): on a `contains`,
/// `refers`, or `derived` feature only the collection constraints apply, so
/// every value-bound keyword is rejected naming its family. Extending the
/// block to relationships therefore never admits value bounds there.
fn lower_collection_constraints_only(
    constraints: &[mox::Constraint],
    noun: &str,
    subject: &str,
    many: bool,
    lowered: &mut LoweredConstraints,
    diags: &mut Vec<Diagnostic>,
) {
    for constraint in constraints {
        let span = Some(constraint.name.span);
        let keyword = constraint.name.text.as_str();
        match (keyword, &constraint.value) {
            ("unique" | "set", mox::ConstraintValue::Flag { .. }) => {
                if !many {
                    diags.push(Diagnostic::error(
                        format!(
                            "constraint '{keyword}' requires a many-valued {noun}, which {subject} is not"
                        ),
                        span,
                    ));
                } else {
                    lowered.constraints.unique = true;
                    if keyword == "set" {
                        lowered.collection = Some(ir::CollectionKind::Set);
                    }
                }
            }
            ("unique" | "set", _) => diags.push(Diagnostic::error(
                format!("constraint '{keyword}' takes no value"),
                span,
            )),
            (keyword, mox::ConstraintValue::Flag { .. }) => diags.push(Diagnostic::error(
                format!("constraint '{keyword}' takes no value"),
                span,
            )),
            (keyword, mox::ConstraintValue::Str { .. } | mox::ConstraintValue::Int { .. }) => {
                diags.push(Diagnostic::error(
                    format!(
                        "constraint '{keyword}' ({family}) is only allowed on attributes, not on {subject}",
                        family = constraint_family(keyword)
                    ),
                    span,
                ));
            }
        }
    }
}

/// Lowers one feature's declared constraints into the IR, type-checking
/// them against the feature kind and (for attributes) its resolved type.
/// For a many-valued attribute the value bounds apply to the elements.
///
/// A constraint family is admitted by the most explicit type knowledge
/// available (violations are errors, never silently dropped):
/// - on a `contains`/`refers`/`derived` feature only the collection
///   constraints apply (`unique`, `set`); every value-bound keyword is an
///   error naming its family (DD6, issue #303);
/// - `pattern`, `minLength`, `maxLength` (the string family) require a
///   `string` attribute; `minimum`, `maximum` (the numeric family) require
///   a numeric attribute (`char` and `boolean` fall under neither rule);
/// - a datatype-typed attribute defaults to the string family — its platform
///   type is opaque — and rejects numeric bounds;
/// - a vocabulary-typed attribute follows its `key` facet's declared
///   primitive type (`String` admits the string family, a numeric key the
///   numeric family); constraints are rejected outright when the key facet
///   cannot be resolved;
/// - an enum-typed attribute carries a dual value space: all five valued
///   keywords apply, the string family bounding literal names and the
///   numeric family literal values, and both may coexist. A numeric or
///   length bound that admits zero literals is a compile error; `pattern`
///   is never statically validated;
/// - the value-less `unique` and `set` are family-agnostic: admitted on any
///   many-valued feature of each kind and rejected on a single-valued one.
///   `set` implies `unique` (a set is unordered *and* duplicate-free), so a
///   set lowers `collection: "set"` and `constraints.unique: true` together;
///   a unique list lowers the flag alone;
/// - class and interface types take no value bounds;
/// - length bounds must be non-negative and ordered; numeric bounds must be
///   ordered.
pub(crate) fn lower_constraints(
    constraints: &[mox::Constraint],
    target: ConstraintTarget,
    many: bool,
    resolution: Option<&Resolution>,
    prep: &PrepMaps<'_>,
    feature_name: &str,
    diags: &mut Vec<Diagnostic>,
) -> LoweredConstraints {
    let mut lowered = LoweredConstraints::empty();
    if constraints.is_empty() {
        return lowered;
    }
    let noun = target.noun();
    let subject = format!("{noun} '{feature_name}'");
    if target != ConstraintTarget::Attribute {
        lower_collection_constraints_only(constraints, noun, &subject, many, &mut lowered, diags);
        return lowered;
    }
    // The attribute's constraint space, or `None` when checks are skipped:
    // either resolution failed (the resolver already reported the error) or
    // a diagnostic was just raised here — both avoid cascading noise.
    let space = match resolution {
        None => None,
        Some(resolution) => match resolution.kind {
            Resolved::Primitive(ir::PrimitiveType::String) => Some(ConstraintSpace::String),
            Resolved::Primitive(primitive) if primitive.is_numeric() => {
                Some(ConstraintSpace::Numeric)
            }
            Resolved::Primitive(_) => Some(ConstraintSpace::Neither),
            Resolved::Datatype => Some(ConstraintSpace::String),
            Resolved::Enum => prep
                .enum_decls
                .get(&(resolution.package.as_str(), resolution.name.as_str()))
                .copied()
                .map(ConstraintSpace::Enum),
            Resolved::Vocabulary => {
                match prep
                    .vocab_key_facet_types
                    .get(&(resolution.package.as_str(), resolution.name.as_str()))
                {
                    Some(ir::PrimitiveType::String) => Some(ConstraintSpace::String),
                    Some(primitive) if primitive.is_numeric() => Some(ConstraintSpace::Numeric),
                    _ => {
                        diags.push(
                            Diagnostic::error(
                                format!(
                                    "constraints on {subject} cannot be checked: vocabulary '{}' has no resolvable key facet",
                                    resolution.name
                                ),
                                Some(constraints[0].name.span),
                            )
                            .with_help(
                                "declare the key facet (e.g. `facet String alpha3`) so its value space is known",
                            ),
                        );
                        None
                    }
                }
            }
            Resolved::Class | Resolved::Interface => {
                diags.push(Diagnostic::error(
                    format!(
                        "constraints are only allowed on primitive, datatype, vocabulary, and enum-typed attributes, but {subject} has type '{}'",
                        resolution.name
                    ),
                    Some(constraints[0].name.span),
                ));
                None
            }
        },
    };
    let Some(space) = space else {
        return lowered;
    };
    let mut minimum_span = None;
    let mut maximum_span = None;
    let mut min_length_span = None;
    let mut max_length_span = None;
    for constraint in constraints {
        let span = Some(constraint.name.span);
        match (constraint.name.text.as_str(), &constraint.value) {
            ("pattern", mox::ConstraintValue::Str { value, .. }) => {
                if space.admits_string_family() {
                    if lowered.constraints.pattern.is_none() {
                        lowered.constraints.pattern = Some(value.clone());
                    }
                } else {
                    diags.push(family_mismatch(
                        "pattern", "string", &subject, resolution, prep, span,
                    ));
                }
            }
            (keyword @ ("minLength" | "maxLength"), mox::ConstraintValue::Int { value, .. }) => {
                if !space.admits_string_family() {
                    diags.push(family_mismatch(
                        keyword, "string", &subject, resolution, prep, span,
                    ));
                } else if *value < 0 {
                    diags.push(Diagnostic::error(
                        format!("constraint '{keyword}' must be non-negative, found {value}"),
                        span,
                    ));
                } else if keyword == "minLength" {
                    lowered.constraints.min_length = Some(*value as u64);
                    min_length_span = span;
                } else {
                    lowered.constraints.max_length = Some(*value as u64);
                    max_length_span = span;
                }
            }
            (keyword @ ("minimum" | "maximum"), mox::ConstraintValue::Int { value, .. }) => {
                if !space.admits_numeric_family() {
                    diags.push(family_mismatch(
                        keyword, "numeric", &subject, resolution, prep, span,
                    ));
                } else if keyword == "minimum" {
                    lowered.constraints.minimum = Some(*value);
                    minimum_span = span;
                } else {
                    lowered.constraints.maximum = Some(*value);
                    maximum_span = span;
                }
            }
            ("unique", mox::ConstraintValue::Flag { .. }) => {
                // Family-agnostic: any many-valued attribute admits it.
                if many {
                    lowered.constraints.unique = true;
                } else {
                    diags.push(Diagnostic::error(
                        format!(
                            "constraint 'unique' requires a many-valued attribute, which {subject} is not"
                        ),
                        span,
                    ));
                }
            }
            ("set", mox::ConstraintValue::Flag { .. }) => {
                // A set is unordered and duplicate-free: it implies `unique`
                // and lowers both markers together.
                if many {
                    lowered.constraints.unique = true;
                    lowered.collection = Some(ir::CollectionKind::Set);
                } else {
                    diags.push(Diagnostic::error(
                        format!(
                            "constraint 'set' requires a many-valued attribute, which {subject} is not"
                        ),
                        span,
                    ));
                }
            }
            (keyword @ ("unique" | "set"), _) => {
                diags.push(Diagnostic::error(
                    format!("constraint '{keyword}' takes no value"),
                    span,
                ));
            }
            (keyword, mox::ConstraintValue::Flag { .. }) => {
                diags.push(Diagnostic::error(
                    format!("constraint '{keyword}' takes no value"),
                    span,
                ));
            }
            (keyword, mox::ConstraintValue::Str { .. } | mox::ConstraintValue::Int { .. }) => {
                diags.push(Diagnostic::error(
                    format!(
                        "constraint '{keyword}' takes a quoted string (`pattern`) or an integer (length and numeric bounds)"
                    ),
                    span,
                ));
            }
        }
    }
    if let (Some(minimum), Some(maximum)) =
        (lowered.constraints.minimum, lowered.constraints.maximum)
    {
        if minimum > maximum {
            diags.push(Diagnostic::error(
                format!(
                    "attribute '{feature_name}': minimum ({minimum}) must not exceed maximum ({maximum})"
                ),
                Some(constraints[0].name.span),
            ));
        }
    }
    if let (Some(min_length), Some(max_length)) = (
        lowered.constraints.min_length,
        lowered.constraints.max_length,
    ) {
        if min_length > max_length {
            diags.push(Diagnostic::error(
                format!(
                    "attribute '{feature_name}': minLength ({min_length}) must not exceed maxLength ({max_length})"
                ),
                Some(constraints[0].name.span),
            ));
        }
    }
    // Enums get closure checks: a bound admitting zero literals is surely a
    // typo. Inverted ranges were already reported above, so the closure
    // check stays silent for them — one clear error each.
    if let ConstraintSpace::Enum(enum_decl) = space {
        let enum_name = &enum_decl.name.text;
        if lowered.constraints.minimum.is_some() || lowered.constraints.maximum.is_some() {
            let ordered = match (lowered.constraints.minimum, lowered.constraints.maximum) {
                (Some(min), Some(max)) => min <= max,
                _ => true,
            };
            let admits = |value: i64| {
                lowered.constraints.minimum.is_none_or(|min| value >= min)
                    && lowered.constraints.maximum.is_none_or(|max| value <= max)
            };
            let any = enum_decl
                .literals
                .iter()
                .filter_map(|literal| literal.value)
                .any(admits);
            if ordered && !any {
                let (keyword, span, detail) =
                    match (lowered.constraints.minimum, lowered.constraints.maximum) {
                        (Some(min), None) => (
                            "minimum",
                            minimum_span,
                            format!("no literal value is >= {min}"),
                        ),
                        (None, Some(max)) => (
                            "maximum",
                            maximum_span,
                            format!("no literal value is <= {max}"),
                        ),
                        (Some(min), Some(max)) => (
                            "minimum",
                            minimum_span,
                            format!("no literal value lies in [{min}, {max}]"),
                        ),
                        (None, None) => unreachable!("range is bounded"),
                    };
                diags.push(Diagnostic::error(
                    format!("constraint '{keyword}' on attribute '{feature_name}' admits no literal of enum '{enum_name}': {detail}"),
                    span,
                ));
            }
        }
        if lowered.constraints.min_length.is_some() || lowered.constraints.max_length.is_some() {
            let ordered = match (
                lowered.constraints.min_length,
                lowered.constraints.max_length,
            ) {
                (Some(min), Some(max)) => min <= max,
                _ => true,
            };
            let admits = |length: usize| {
                lowered
                    .constraints
                    .min_length
                    .is_none_or(|min| length >= min as usize)
                    && lowered
                        .constraints
                        .max_length
                        .is_none_or(|max| length <= max as usize)
            };
            let any = enum_decl
                .literals
                .iter()
                .map(|literal| literal.name.text.chars().count())
                .any(admits);
            if ordered && !any {
                let (keyword, span, detail) = match (
                    lowered.constraints.min_length,
                    lowered.constraints.max_length,
                ) {
                    (Some(min), None) => (
                        "minLength",
                        min_length_span,
                        format!("no literal name has at least {min} characters"),
                    ),
                    (None, Some(max)) => (
                        "maxLength",
                        max_length_span,
                        format!("no literal name has at most {max} characters"),
                    ),
                    (Some(min), Some(max)) => (
                        "minLength",
                        min_length_span,
                        format!("no literal name has between {min} and {max} characters"),
                    ),
                    (None, None) => unreachable!("range is bounded"),
                };
                diags.push(Diagnostic::error(
                    format!("constraint '{keyword}' on attribute '{feature_name}' admits no literal of enum '{enum_name}': {detail}"),
                    span,
                ));
            }
        }
    }
    lowered
}

pub(crate) fn lower_enum(decl: &mox::EnumDecl, diags: &mut Vec<Diagnostic>) -> ir::EnumDef {
    let mut seen = HashSet::new();
    let mut literals = Vec::new();
    for literal in &decl.literals {
        if !seen.insert(literal.name.text.as_str()) {
            diags.push(Diagnostic::error(
                format!(
                    "duplicate enum literal '{}' in enum '{}'",
                    literal.name.text, decl.name.text
                ),
                Some(literal.name.span),
            ));
        }
        let value = match literal.value {
            Some(value) => value,
            None => {
                diags.push(Diagnostic::error(
                    format!(
                        "enum literal '{}' requires an explicit value (e.g. `= 0`)",
                        literal.name.text
                    ),
                    Some(literal.span),
                ));
                0
            }
        };
        let mut lowered_literal =
            ir::EnumLiteral::new(&literal.name.text, literal.label.clone(), value);
        lowered_literal.description = literal.doc.clone();
        literals.push(lowered_literal);
    }
    let mut enum_def = ir::EnumDef::new(&decl.name.text, literals);
    enum_def.description = decl.doc.clone();
    enum_def
}

pub(crate) fn lower_datatype(
    decl: &mox::DatatypeDecl,
    source: &str,
    diags: &mut Vec<Diagnostic>,
) -> ir::DatatypeDef {
    let platform = match &decl.wraps {
        Some(mox::Wraps::Named(name)) => Some(name.full_name()),
        Some(mox::Wraps::Opaque(_)) | None => None,
    };
    let mut datatype = ir::DatatypeDef::new(&decl.name.text, platform);
    datatype.description = decl.doc.clone();
    datatype.format = decl.format.clone();
    for binding in &decl.bindings {
        datatype = datatype.bind(&binding.key.text, &binding.value);
    }
    // Tier 1: create/convert bodies lower next to the binding entries.
    datatype.create = lower_target_bodies(
        &decl.create,
        source,
        &format!("datatype '{}' create", decl.name.text),
        diags,
    );
    datatype.convert = lower_target_bodies(
        &decl.convert,
        source,
        &format!("datatype '{}' convert", decl.name.text),
        diags,
    );
    datatype
}

pub(crate) fn lower_interface(decl: &mox::InterfaceDecl) -> ir::InterfaceDef {
    let mut interface = ir::InterfaceDef::new(&decl.name.text);
    interface.description = decl.doc.clone();
    for binding in &decl.bindings {
        interface = interface.bind(&binding.key.text, &binding.value);
    }
    interface
}

/// Lowers a `register` declaration (issue pherolang#181) through the exact
/// class pipeline: the grammar admits attributes only, so the declaration
/// lowers like a class with no `extends` and no table override, joining
/// every class-level rule (namespace, duplicate checks, stripped-name and
/// table uniqueness, feature ids) unchanged. The one difference is the
/// additive wire marker: the lowered `ClassDef` carries
/// `kind: ClassKind::Register` (phero-ir wire-contract rule 16).
pub(crate) fn lower_register(
    decl: &mox::RegisterDecl,
    source: &str,
    path: &str,
    package: &str,
    scope: &Scope<'_>,
    prep: &PrepMaps<'_>,
    diags: &mut Vec<Diagnostic>,
) -> ClassRecord {
    let class_decl = mox::ClassDecl {
        name: decl.name.clone(),
        doc: decl.doc.clone(),
        table: None,
        extends: Vec::new(),
        features: decl.attributes.clone(),
        span: decl.span,
    };
    let mut record = lower_class(&class_decl, source, path, package, scope, prep, diags);
    record.def.kind = ir::ClassKind::Register;
    record
}

pub(crate) fn lower_class(
    decl: &mox::ClassDecl,
    source: &str,
    path: &str,
    package: &str,
    scope: &Scope<'_>,
    prep: &PrepMaps<'_>,
    diags: &mut Vec<Diagnostic>,
) -> ClassRecord {
    let mut extends = Vec::new();
    for type_ref in &decl.extends {
        if let Some(resolution) = scope.resolve(type_ref, diags) {
            match resolution.kind {
                Resolved::Class | Resolved::Interface => {
                    extends.push(resolution.kind.to_ir(&resolution.package, &resolution.name));
                }
                _ => diags.push(
                    Diagnostic::error(
                        format!(
                            "class '{}' cannot extend '{}'",
                            decl.name.text,
                            type_ref.name.full_name()
                        ),
                        Some(type_ref.span),
                    )
                    .with_help("extends must name a class or interface"),
                ),
            }
        }
    }

    let mut features = Vec::new();
    let mut operations = Vec::new();
    let mut records = Vec::new();
    let mut seen = HashSet::new();
    for feature in &decl.features {
        let name = feature.name();
        if !seen.insert(name.text.as_str()) {
            diags.push(Diagnostic::error(
                format!(
                    "duplicate feature '{}' in class '{}'",
                    name.text, decl.name.text
                ),
                Some(name.span),
            ));
        }
        match feature {
            mox::FeatureDecl::Attribute {
                doc,
                type_ref,
                multiplicity,
                name,
                default,
                constraints,
                ..
            } => {
                let resolution = scope.resolve(type_ref, diags);
                report_class_typed_feature(name, type_ref, resolution.as_ref(), diags);
                let multiplicity = multiplicity
                    .as_ref()
                    .map(|m| lower_multiplicity(m, diags))
                    .unwrap_or(ir::Multiplicity::REQUIRED);
                let ir_type = ir_type_of(resolution.as_ref(), scope.fallback_package(), type_ref);
                let ir_feature = ir::Feature::new(
                    &name.text,
                    ir::FeatureKind::Attribute,
                    ir_type,
                    multiplicity,
                );
                let ir_feature = apply_default(
                    ir_feature,
                    default.as_ref(),
                    resolution.as_ref(),
                    prep,
                    diags,
                );
                let mut ir_feature = apply_modifiers(ir_feature, feature.modifiers());
                ir_feature.description = doc.clone();
                let lowered_constraints = lower_constraints(
                    constraints,
                    ConstraintTarget::Attribute,
                    multiplicity.is_many(),
                    resolution.as_ref(),
                    prep,
                    &name.text,
                    diags,
                );
                ir_feature.constraints = lowered_constraints.constraints;
                ir_feature.collection = lowered_constraints.collection;
                features.push(ir_feature);
                records.push(FeatureRecord {
                    name: name.text.clone(),
                    kind: FKind::Attribute,
                    type_class: class_location(resolution.as_ref()),
                    type_text: type_ref.name.full_name(),
                    type_span: Some(type_ref.span),
                    opposite: None,
                    opposite_span: None,
                });
            }
            mox::FeatureDecl::Containment {
                doc,
                type_ref,
                multiplicity,
                name,
                opposite,
                constraints,
                ..
            } => {
                let (ir_feature, record) = lower_relation(
                    FKind::Containment,
                    type_ref,
                    multiplicity.as_ref(),
                    name,
                    opposite.as_ref(),
                    constraints,
                    ConstraintTarget::Containment,
                    prep,
                    scope,
                    diags,
                );
                let mut ir_feature = apply_modifiers(ir_feature, feature.modifiers());
                ir_feature.description = doc.clone();
                features.push(ir_feature);
                records.push(record);
            }
            mox::FeatureDecl::Reference {
                doc,
                type_ref,
                multiplicity,
                name,
                opposite,
                constraints,
                ..
            } => {
                let (ir_feature, record) = lower_relation(
                    FKind::Reference,
                    type_ref,
                    multiplicity.as_ref(),
                    name,
                    opposite.as_ref(),
                    constraints,
                    ConstraintTarget::Reference,
                    prep,
                    scope,
                    diags,
                );
                let mut ir_feature = apply_modifiers(ir_feature, feature.modifiers());
                ir_feature.description = doc.clone();
                features.push(ir_feature);
                records.push(record);
            }
            mox::FeatureDecl::Container {
                doc,
                type_ref,
                name,
                opposite,
                ..
            } => {
                // Containers never take a multiplicity in the grammar; the
                // driver forces OPTIONAL regardless. The grammar admits no
                // constraint block here either.
                let (ir_feature, record) = lower_relation(
                    FKind::Container,
                    type_ref,
                    None,
                    name,
                    opposite.as_ref(),
                    &[],
                    ConstraintTarget::Container,
                    prep,
                    scope,
                    diags,
                );
                let mut ir_feature = apply_modifiers(ir_feature, feature.modifiers());
                ir_feature.description = doc.clone();
                features.push(ir_feature);
                records.push(record);
            }
            mox::FeatureDecl::Op {
                doc,
                return_type,
                name,
                params,
                body,
                bodies,
                ..
            } => {
                // The `id`/`readonly` modifiers only apply to stored features,
                // so they warn here.
                let modifiers = feature.modifiers();
                if let Some(span) = modifiers.id {
                    diags.push(Diagnostic::warning(
                        "modifier 'id' has no effect on operations",
                        Some(span),
                    ));
                }
                if let Some(span) = modifiers.read_only {
                    diags.push(Diagnostic::warning(
                        "modifier 'readonly' has no effect on operations",
                        Some(span),
                    ));
                }
                // Tier 1: operations lower into `ClassDef.operations` with
                // their resolved signature; bodies must be per-target and are
                // carried verbatim. Body-less operations are abstract hooks.
                if body.is_some() && bodies.is_empty() {
                    diags.push(Diagnostic::error(
                        "operation bodies must be per-target in Tier 1 (e.g. `rust { ... }`)",
                        Some(body.expect("body span checked above")),
                    ));
                }
                let lowered_bodies = lower_target_bodies(
                    bodies,
                    source,
                    &format!("operation '{}'", name.text),
                    diags,
                );
                let return_resolution = scope.resolve(return_type, diags);
                let mut lowered_params = Vec::new();
                for param in params {
                    let resolution = scope.resolve(&param.type_ref, diags);
                    lowered_params.push(ir::OperationParam {
                        name: param.name.text.clone(),
                        type_: ir_type_of(
                            resolution.as_ref(),
                            scope.fallback_package(),
                            &param.type_ref,
                        ),
                        multiplicity: None,
                    });
                }
                let mut operation = ir::Operation::new(
                    &name.text,
                    ir_type_of(
                        return_resolution.as_ref(),
                        scope.fallback_package(),
                        return_type,
                    ),
                    lowered_params,
                );
                operation.description = doc.clone();
                operation.bodies = lowered_bodies;
                operations.push(operation);
                records.push(FeatureRecord {
                    name: name.text.clone(),
                    kind: FKind::Op,
                    type_class: None,
                    type_text: return_type.name.full_name(),
                    type_span: None,
                    opposite: None,
                    opposite_span: None,
                });
            }
            mox::FeatureDecl::Derived {
                doc,
                type_ref,
                multiplicity,
                name,
                body,
                bodies,
                constraints,
                ..
            } => {
                let resolution = scope.resolve(type_ref, diags);
                report_class_typed_feature(name, type_ref, resolution.as_ref(), diags);
                // Tier 2: a derived body must be the neutral expression
                // language, in a single `expr { ... }` block.
                if body.is_some() && bodies.is_empty() {
                    diags.push(Diagnostic::error(
                        "derived bodies must use the neutral expression language: \
                         get { expr { ... } }",
                        Some(body.expect("body span checked above")),
                    ));
                }
                let lowered_bodies = lower_target_bodies(
                    bodies,
                    source,
                    &format!("derived feature '{}'", name.text),
                    diags,
                );
                for target in lowered_bodies.keys() {
                    if target != "expr" {
                        let span = bodies
                            .iter()
                            .find(|body| body.target.text == *target)
                            .map(|body| body.target.span);
                        diags.push(Diagnostic::error(
                            "derived bodies must use the neutral expression language: \
                             get { expr { ... } }",
                            span,
                        ));
                    }
                }
                let multiplicity = multiplicity
                    .as_ref()
                    .map(|m| lower_multiplicity(m, diags))
                    .unwrap_or(ir::Multiplicity::OPTIONAL);
                let ir_type = ir_type_of(resolution.as_ref(), scope.fallback_package(), type_ref);
                let mut ir_feature = ir::Feature::new(
                    &name.text,
                    ir::FeatureKind::Attribute,
                    ir_type,
                    multiplicity,
                )
                .derived();
                ir_feature = apply_modifiers(ir_feature, feature.modifiers());
                ir_feature.description = doc.clone();
                ir_feature.bodies = lowered_bodies;
                let lowered_constraints = lower_constraints(
                    constraints,
                    ConstraintTarget::Derived,
                    multiplicity.is_many(),
                    resolution.as_ref(),
                    prep,
                    &name.text,
                    diags,
                );
                ir_feature.constraints = lowered_constraints.constraints;
                ir_feature.collection = lowered_constraints.collection;
                features.push(ir_feature);
                records.push(FeatureRecord {
                    name: name.text.clone(),
                    kind: FKind::Derived,
                    type_class: class_location(resolution.as_ref()),
                    type_text: type_ref.name.full_name(),
                    type_span: Some(type_ref.span),
                    opposite: None,
                    opposite_span: None,
                });
            }
        }
    }

    let mut class_def = ir::ClassDef::new(&decl.name.text, extends, features);
    class_def.description = decl.doc.clone();
    // The `table "…"` override is carried verbatim — consumers stop
    // re-deriving the table name the moment it is declared (issue #177).
    if let Some(table) = &decl.table {
        class_def = class_def.with_table_name(table.clone());
    }
    ClassRecord {
        package: package.to_string(),
        file: path.to_string(),
        name: decl.name.text.clone(),
        name_span: decl.name.span,
        def: class_def,
        features: records,
        operations,
    }
}

/// The flagship diagnostic: an attribute (or derived feature) whose type
/// resolves to a class almost certainly means the author meant `contains` or
/// `refers`.
fn report_class_typed_feature(
    name: &mox::Name,
    type_ref: &mox::TypeRef,
    resolution: Option<&Resolution>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(resolution) = resolution else {
        return;
    };
    if resolution.kind != Resolved::Class {
        return;
    }
    let class = type_ref.name.full_name();
    diags.push(
        Diagnostic::error(
            format!("feature '{}' has class type '{}'", name.text, class),
            Some(type_ref.span),
        )
        .with_help(format!(
            "did you mean `contains {class}[..] {}` or `refers {class}[..] {}`?",
            name.text, name.text
        ))
        .with_code(DiagnosticCode::AttributeWithClassType {
            feature: name.text.clone(),
            class,
            type_span: type_ref.span,
        }),
    );
}

/// Lowers a `contains` / `refers` / `container` feature.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_relation(
    kind: FKind,
    type_ref: &mox::TypeRef,
    multiplicity: Option<&mox::Multiplicity>,
    name: &mox::Name,
    opposite: Option<&mox::Name>,
    constraints: &[mox::Constraint],
    target: ConstraintTarget,
    prep: &PrepMaps<'_>,
    scope: &Scope<'_>,
    diags: &mut Vec<Diagnostic>,
) -> (ir::Feature, FeatureRecord) {
    let resolution = scope.resolve(type_ref, diags);
    if let Some(resolution) = &resolution {
        if resolution.kind != Resolved::Class {
            diags.push(Diagnostic::error(
                format!(
                    "feature '{}' is declared `{}` but type '{}' is not a class",
                    name.text,
                    kind.keyword(),
                    type_ref.name.full_name()
                ),
                Some(type_ref.span),
            ));
        }
    }

    let multiplicity = match kind {
        FKind::Container => ir::Multiplicity::OPTIONAL,
        _ => multiplicity
            .map(|m| lower_multiplicity(m, diags))
            .unwrap_or(ir::Multiplicity::MANY),
    };
    let ir_type = ir_type_of(resolution.as_ref(), scope.fallback_package(), type_ref);
    let mut ir_feature = ir::Feature::new(
        &name.text,
        kind.ir_kind()
            .expect("relations always lower to an IR kind"),
        ir_type,
        multiplicity,
    );
    let lowered_constraints = lower_constraints(
        constraints,
        target,
        multiplicity.is_many(),
        resolution.as_ref(),
        prep,
        &name.text,
        diags,
    );
    ir_feature.constraints = lowered_constraints.constraints;
    ir_feature.collection = lowered_constraints.collection;
    if let (Some(opposite), Some(class)) = (opposite, class_of(resolution.as_ref())) {
        ir_feature = ir_feature.with_opposite(class, &opposite.text);
    }

    let record = FeatureRecord {
        name: name.text.clone(),
        kind,
        type_class: class_location(resolution.as_ref()),
        type_text: type_ref.name.full_name(),
        type_span: Some(type_ref.span),
        opposite: opposite.map(|opposite| opposite.text.clone()),
        opposite_span: opposite.map(|opposite| opposite.span),
    };
    (ir_feature, record)
}

pub(crate) fn lower_multiplicity(
    multiplicity: &mox::Multiplicity,
    diags: &mut Vec<Diagnostic>,
) -> ir::Multiplicity {
    let (lower, upper) =
        engine_common::multiplicity_bounds(&multiplicity.kind, |slot, value| match slot {
            engine_common::MultiplicitySlot::Exact => {
                bound_value(value, multiplicity.span, diags).unwrap_or(1)
            }
            engine_common::MultiplicitySlot::Lower => {
                bound_value(value, multiplicity.span, diags).unwrap_or(0)
            }
            engine_common::MultiplicitySlot::Upper { lower } => {
                bound_value(value, multiplicity.span, diags).unwrap_or(lower)
            }
        });
    ir::Multiplicity::new(lower, upper)
}

/// Converts a multiplicity bound, reporting out-of-range values.
fn bound_value(value: i64, span: Span, diags: &mut Vec<Diagnostic>) -> Option<u32> {
    u32::try_from(value).ok().or_else(|| {
        diags.push(Diagnostic::error(
            format!("invalid multiplicity bound {value}: bounds must be between 0 and 4294967295"),
            Some(span),
        ));
        None
    })
}

/// Reports a literal default that cannot initialize the attribute's
/// declared type, pointing at the default literal.
fn report_default_mismatch(
    form: &str,
    literal: &str,
    feature: &ir::Feature,
    span: Span,
    resolution: &Resolution,
    diags: &mut Vec<Diagnostic>,
) {
    let description = match resolution.kind {
        Resolved::Primitive(primitive) => primitive.to_string(),
        Resolved::Enum => "enum-typed".to_string(),
        Resolved::Datatype => "datatype-typed".to_string(),
        Resolved::Class => "class-typed".to_string(),
        Resolved::Interface => "interface-typed".to_string(),
        Resolved::Vocabulary => "vocabulary-typed".to_string(),
    };
    let mut diagnostic = Diagnostic::error(
        format!(
            "{form} default '{literal}' on {description} attribute '{}'",
            feature.name
        ),
        Some(span),
    );
    if resolution.kind == Resolved::Datatype {
        diagnostic =
            diagnostic.with_help("datatype-typed attributes admit only a string literal default");
    }
    diags.push(diagnostic);
}

/// Applies an attribute default value. The `Name` form is an enum literal
/// reference on enum-typed attributes, or an entry-key reference on
/// vocabulary-typed attributes (lowered to `DefaultValue::String`, since
/// vocabulary keys are strings).
///
/// Literal defaults (`string`/`int`/`boolean`) are type-checked against the
/// attribute's declared type: a string literal initializes text (string,
/// char), a datatype's wrapped string, or a vocabulary key; an int literal
/// initializes a numeric primitive; a boolean literal initializes `boolean`
/// only. Unresolvable attribute types are left to the type diagnostic.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_default(
    feature: ir::Feature,
    default: Option<&mox::DefaultValue>,
    resolution: Option<&Resolution>,
    prep: &PrepMaps<'_>,
    diags: &mut Vec<Diagnostic>,
) -> ir::Feature {
    let Some(default) = default else {
        return feature;
    };
    // Date attributes take no declared defaults in this milestone (issue
    // #9): the surface has no date literal yet, and generated code anchors
    // required date fields at the runtime epoch.
    if matches!(
        resolution,
        Some(Resolution {
            kind: Resolved::Primitive(ir::PrimitiveType::Date),
            ..
        })
    ) {
        diags.push(Diagnostic::error(
            "date attributes take no default value in this milestone",
            Some(default_span(default)),
        ));
        return feature;
    }
    match default {
        mox::DefaultValue::Str { value, span } => {
            if let Some(resolution) = resolution {
                let admitted = match resolution.kind {
                    Resolved::Primitive(primitive) => {
                        matches!(
                            primitive,
                            ir::PrimitiveType::String | ir::PrimitiveType::Char
                        )
                    }
                    Resolved::Datatype | Resolved::Vocabulary => true,
                    _ => false,
                };
                if !admitted {
                    report_default_mismatch("string", value, &feature, *span, resolution, diags);
                }
            }
            feature.with_default(ir::DefaultValue::String(value.clone()))
        }
        mox::DefaultValue::Int { value, span } => {
            if let Some(resolution) = resolution {
                let admitted = matches!(resolution.kind, Resolved::Primitive(p) if p.is_numeric());
                if !admitted {
                    report_default_mismatch(
                        "int",
                        &value.to_string(),
                        &feature,
                        *span,
                        resolution,
                        diags,
                    );
                }
            }
            feature.with_default(ir::DefaultValue::Int(*value))
        }
        mox::DefaultValue::Bool { value, span } => {
            if let Some(resolution) = resolution {
                let admitted = resolution.kind == Resolved::Primitive(ir::PrimitiveType::Boolean);
                if !admitted {
                    report_default_mismatch(
                        "boolean",
                        &value.to_string(),
                        &feature,
                        *span,
                        resolution,
                        diags,
                    );
                }
            }
            feature.with_default(ir::DefaultValue::Bool(*value))
        }
        mox::DefaultValue::Name(name) => match resolution {
            Some(Resolution {
                kind: Resolved::Enum,
                name: enum_name,
                package,
            }) => {
                if let Some(enum_decl) =
                    prep.enum_decls.get(&(package.as_str(), enum_name.as_str()))
                {
                    if !enum_decl
                        .literals
                        .iter()
                        .any(|literal| literal.name.text == name.text)
                    {
                        diags.push(Diagnostic::error(
                            format!("enum '{enum_name}' has no literal '{}'", name.text),
                            Some(name.span),
                        ));
                    }
                }
                feature.with_default(ir::DefaultValue::EnumLiteral(name.text.clone()))
            }
            Some(Resolution {
                kind: Resolved::Vocabulary,
                name: vocab_name,
                package,
            }) => {
                // An absent key set means the vocabulary failed to load; that
                // error is already reported, so the default adds nothing.
                if let Some(keys) = prep
                    .vocab_keys
                    .get(&(package.as_str(), vocab_name.as_str()))
                {
                    if !keys.contains(name.text.as_str()) {
                        diags.push(Diagnostic::error(
                            format!("vocabulary '{vocab_name}' has no entry '{}'", name.text),
                            Some(name.span),
                        ));
                    }
                }
                // Keys are strings; `String` is the wire representation.
                feature.with_default(ir::DefaultValue::String(name.text.clone()))
            }
            Some(_) => {
                diags.push(Diagnostic::error(
                    format!(
                        "default value '{}' requires an enum-typed attribute",
                        name.text
                    ),
                    Some(name.span),
                ));
                feature
            }
            // The attribute type failed to resolve; that error is already
            // reported, so the default adds no information.
            None => feature,
        },
    }
}

/// The source span of a default value, for diagnostics about it.
fn default_span(default: &mox::DefaultValue) -> Span {
    match default {
        mox::DefaultValue::Str { span, .. }
        | mox::DefaultValue::Int { span, .. }
        | mox::DefaultValue::Bool { span, .. } => *span,
        mox::DefaultValue::Name(name) => name.span,
    }
}

/// Detects cycles in the class inheritance graph (class → class `extends`
/// edges, keyed by `(package, name)` so cross-package edges are followed),
/// reporting the first cycle found. Diagnostics are tagged with the file
/// that closes the cycle.
///
/// The detection is the verified `phero-verify` kernel: `link(super, sub)`
/// refuses exactly the edges that would close a cycle (kernel proofs V1/V2/
/// V5 — see `docs/VERIFICATION.md`), so a rejected link *is* a cycle, and
/// duplicate `extends` edges report success (they are not cycles). The
/// reported ring is reconstructed by walking the kernel's parent chains
/// from the refused edge's child around to itself; in models with several
/// independent cycles this is the first rejected edge in declaration order,
/// which may be a different rotation of the same ring than a depth-first
/// walk would name.
pub(crate) fn detect_inheritance_cycles(
    classes: &[ClassRecord],
    diags: &mut Vec<(String, Diagnostic)>,
) {
    let index: HashMap<(&str, &str), &ClassRecord> = classes
        .iter()
        .map(|class| ((class.package.as_str(), class.name.as_str()), class))
        .collect();
    let mut graph = phero_verify::Graph::new();
    // Node ids are assigned in `classes` order, so `keys[id.index()]` is
    // that node's `(package, name)` key.
    let keys: Vec<(&str, &str)> = classes
        .iter()
        .map(|class| (class.package.as_str(), class.name.as_str()))
        .collect();
    let mut ids: HashMap<(&str, &str), phero_verify::NodeId> = HashMap::new();
    for key in &keys {
        let id = graph.add_node();
        ids.insert(*key, id);
    }
    for class in classes {
        let sub = ids[&(class.package.as_str(), class.name.as_str())];
        for superclass in class
            .def
            .extends
            .iter()
            .filter_map(|type_ref| match type_ref {
                ir::TypeRef::Class { package, name, .. } => Some((package.as_str(), name.as_str())),
                _ => None,
            })
        {
            let Some(sup) = ids.get(&superclass) else {
                // Unknown super: a resolution diagnostic is emitted
                // elsewhere; the cycle pass treats it as a leaf, like the
                // kernel index miss it is.
                continue;
            };
            if graph.link(*sup, sub) {
                continue;
            }
            let ring: Vec<(&str, &str)> = if *sup == sub {
                vec![keys[sub.index()], keys[sub.index()]]
            } else {
                parent_ring(&graph, &keys, *sup, sub)
            };
            let record = index.get(&ring[0]).expect("cycle member is indexed");
            let names: Vec<&str> = ring.iter().map(|(_, name)| *name).collect();
            diags.push((
                record.file.clone(),
                Diagnostic::error(
                    format!("inheritance cycle detected: {}", names.join(" extends ")),
                    Some(record.name_span),
                ),
            ));
            return;
        }
    }
}

/// The containment ring that `link(sup, sub)` refused: the pre-existing
/// `sup ⇒* sub` chain (the kernel's parent edges, walked from `sup`) plus
/// the refused `sub ⇒ sup` edge that closes it. The refusal guarantees the
/// chain exists (`is_ancestor` found `sub` among `sup`'s containment
/// ancestors through these same edges).
fn parent_ring<'k>(
    graph: &phero_verify::Graph,
    keys: &[(&'k str, &'k str)],
    sup: phero_verify::NodeId,
    sub: phero_verify::NodeId,
) -> Vec<(&'k str, &'k str)> {
    let mut chain: Vec<phero_verify::NodeId> = vec![sup];
    let mut seen: Vec<phero_verify::NodeId> = vec![sup];
    while let Some(&cur) = chain.last() {
        if cur == sub {
            break;
        }
        let mut advanced = false;
        for &p in graph.parents_of(cur) {
            if !seen.contains(&p) {
                seen.push(p);
                chain.push(p);
                advanced = true;
                break;
            }
        }
        if !advanced {
            chain.pop();
        }
    }
    let mut ring: Vec<(&'k str, &'k str)> = chain.iter().map(|&n| keys[n.index()]).collect();
    ring.push(keys[sup.index()]);
    ring
}

/// Validates opposite pairing, mutually:
///
/// * `contains` (in `A`, typed `B`) ↔ `container` (in `B`, typed `A`);
/// * `refers` ↔ `refers`;
///
/// and requires each side's `opposite` to name the other feature back. Each
/// violated declaration produces one error naming both sides.
///
/// Relations are same-package only: a `contains`/`container` targeting a
/// foreign-package class is rejected outright (ownership does not span
/// packages), and a `refers` may only cross packages one-sided. Diagnostics
/// are tagged with the declaring file.
pub(crate) fn validate_opposites(classes: &[ClassRecord], diags: &mut Vec<(String, Diagnostic)>) {
    let index: HashMap<(&str, &str), &ClassRecord> = classes
        .iter()
        .map(|class| ((class.package.as_str(), class.name.as_str()), class))
        .collect();
    for class in classes {
        for feature in &class.features {
            if !matches!(
                feature.kind,
                FKind::Containment | FKind::Reference | FKind::Container
            ) {
                continue;
            }
            if let Some((target_package, _)) = &feature.type_class {
                if target_package != class.package.as_str() {
                    match feature.kind {
                        FKind::Containment | FKind::Container => {
                            diags.push((
                                class.file.clone(),
                                Diagnostic::error(
                                    format!(
                                        "cross-package ownership is not supported: feature '{}.{}' {} '{}'",
                                        class.name,
                                        feature.name,
                                        feature.kind.keyword(),
                                        feature.type_text,
                                    ),
                                    feature.type_span,
                                )
                                .with_help(
                                    "declare `contains` targets within the same package, or use `refers` for cross-package links",
                                ),
                            ));
                        }
                        _ if feature.opposite.is_some() => {
                            diags.push((
                                class.file.clone(),
                                Diagnostic::error(
                                    "cross-package opposites are not supported; declare the opposite within the same package",
                                    feature.opposite_span,
                                ),
                            ));
                        }
                        _ => {}
                    }
                    continue;
                }
            }
            let (Some((_, target)), Some(opposite)) = (&feature.type_class, &feature.opposite)
            else {
                continue;
            };
            let Some(opposite_span) = feature.opposite_span else {
                continue;
            };
            let Some(other) = index.get(&(class.package.as_str(), target.as_str())) else {
                continue;
            };
            let Some(counterpart) = other
                .features
                .iter()
                .find(|candidate| &candidate.name == opposite)
            else {
                diags.push((
                    class.file.clone(),
                    Diagnostic::error(
                        format!(
                            "opposite mismatch: '{}.{}' declares opposite '{}.{}', but class '{}' has no feature '{}'",
                            class.name, feature.name, target, opposite, target, opposite,
                        ),
                        Some(opposite_span),
                    ),
                ));
                continue;
            };
            let expected = match feature.kind {
                FKind::Containment => FKind::Container,
                FKind::Container => FKind::Containment,
                _ => FKind::Reference,
            };
            if counterpart.kind != expected {
                diags.push((
                    class.file.clone(),
                    Diagnostic::error(
                        format!(
                            "opposite mismatch: '{}.{}' expects '{}.{}' to be {}, but found {}",
                            class.name,
                            feature.name,
                            target,
                            opposite,
                            expected_description(feature.kind, &class.name),
                            counterpart.kind.describe(),
                        ),
                        Some(opposite_span),
                    ),
                ));
                continue;
            }
            if counterpart.type_class != Some((class.package.clone(), class.name.clone())) {
                diags.push((
                    class.file.clone(),
                    Diagnostic::error(
                        format!(
                            "opposite mismatch: '{}.{}' expects '{}.{}' to have type '{}', but found '{}'",
                            class.name, feature.name, target, opposite, class.name, counterpart.type_text,
                        ),
                        Some(opposite_span),
                    ),
                ));
                continue;
            }
            if counterpart.opposite.as_deref() != Some(feature.name.as_str()) {
                diags.push((
                    class.file.clone(),
                    Diagnostic::error(
                        format!(
                            "opposite mismatch: '{}.{}' does not declare opposite '{}.{}'",
                            target, opposite, class.name, feature.name,
                        ),
                        Some(opposite_span),
                    ),
                ));
            }
        }
    }
}

/// The "expected" half of an opposite-mismatch message.
fn expected_description(kind: FKind, class: &str) -> String {
    match kind {
        FKind::Containment => format!("a `container` feature of type '{class}'"),
        FKind::Container => format!("a `contains` feature of type '{class}'"),
        FKind::Reference => format!("a `refers` feature of type '{class}'"),
        _ => String::new(),
    }
}
