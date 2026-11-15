//! DTO boundary lowering (DD3, issue pherolang#300): the three declaration
//! forms (mirror, projection, standalone), their member typing, the
//! input/output direction derivation, and the application-wide pre-pass that
//! makes DTO names resolvable before services lower.
//!
//! The pre-pass ([`collect_design_prep`]) runs once per application, before
//! the module loop, and gathers:
//!
//! - every DTO boundary's name, form, module and resolved source class, so a
//!   service in an earlier module may use a DTO declared later;
//! - every entity-designed class, so a DTO member can be refused when it
//!   would embed mutable entity state;
//! - the DTO-name collision diagnostics (duplicate boundary names, and a
//!   projection/standalone name shadowing a domain type — a mirror DTO's
//!   name *is* its source class and is exempt).
//!
//! [`lower_dto_shape`] runs in the design stage (stage 5) and lowers one
//! declaration's members:
//!
//! - **mirror** (`dto User`) copies every stored feature of the class
//!   (derived features excluded), in declaration order;
//! - **projection** (`dto BookSummary from Book { … }`) resolves a selected
//!   feature, a renamed plain path, or a typed derivation with the source
//!   class as `self`;
//! - **standalone** (`dto CreateBookRequest { … }`) resolves each declared
//!   member's type against the domain union or another DTO boundary.
//!
//! The direction is derived, never authored: any `derive` member makes the
//! boundary [`DtoDirection::Output`], and
//! [`check_dto_parameters`] then refuses an output-only DTO in a signature's
//! parameter slot.

use phero_diagnostics::ddd::{
    E_DTO_DUPLICATE_MEMBER, E_DTO_ENTITY_MEMBER, E_DTO_MEMBER_TYPE, E_DTO_NAME_COLLISION,
    E_DTO_PARAM_NOT_INPUT, E_DTO_UNKNOWN_MEMBER,
};
use phero_expr::{ExprKind, Ty};
use phero_ir::ddd::{DddParam, DddTypeRef, DtoDirection, DtoMember, DtoMemberKind, DtoShape};
use phero_ir::{Multiplicity, TypeRef};
use phero_syntax::ast as dsl;
use phero_syntax::Span;

use crate::diagnostic::Diagnostic;
use crate::lower::{self, DomainPackage, TopKind};

use super::*;

/// The synthetic package the DTO-typed statement-body slots bind under; the
/// angle-bracketed key cannot collide with a `.mox` package name.
pub(crate) const DTO_BINDING_PACKAGE: &str = "<dto>";

/// The source form a `dto` design declaration writes (DD3, pherolang#300).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DtoForm {
    /// `dto User` — every stored feature of the source class.
    Mirror,
    /// `dto BookSummary from Book { … }` — a shaped source projection.
    Projection,
    /// `dto CreateBookRequest { … }` — a declared, source-less boundary.
    Standalone,
}

/// One DTO boundary collected by the application-wide pre-pass.
pub(crate) struct DtoDecl {
    /// The boundary name (the DTO's own name for every form; for a mirror it
    /// equals the source class).
    pub(crate) name: String,
    /// The declaration form.
    pub(crate) form: DtoForm,
    /// The declaring module.
    pub(crate) module: String,
    /// The DTO name's span (the declaration site).
    pub(crate) span: Span,
    /// The resolved source class for a mirror/projection; `None` for a
    /// standalone DTO and for an unresolved source (the design stage's own
    /// rule-3 diagnostic stands).
    pub(crate) source: Option<(String, String)>,
}

/// The application-wide design facts gathered before services lower: the
/// DTO registry (requirement: services lower before designs, so signature
/// resolution must already know the DTO names), the non-mirror boundary
/// names signature resolution may fall back to, and the entity-designed
/// classes a DTO member may not carry.
#[derive(Default)]
pub(crate) struct DesignPrep {
    /// Every DTO declaration, in source order.
    pub(crate) dtos: Vec<DtoDecl>,
    /// The projection/standalone boundary names — the DTO fallback of
    /// signature and member type resolution. Mirror names are omitted: they
    /// are their source class's name, and the domain namespace resolves them
    /// first.
    pub(crate) dto_names: Vec<String>,
    /// The `(package, class)` keys of every entity-designed class.
    pub(crate) entity_classes: Vec<(String, String)>,
}

impl DesignPrep {
    /// The pre-pass declaration for one design declaration, matched by its
    /// name-site span (the declaration identity the parser preserves).
    pub(crate) fn declaration(&self, design: &dsl::DddDesign) -> Option<&DtoDecl> {
        self.dtos.iter().find(|dto| dto.span == design.class.span)
    }
}

/// The DTO form a design declaration writes, or `None` for a non-DTO
/// design. The parser guarantees a `from` design carries projection members
/// and a source-less member block carries declared members.
pub(crate) fn dto_form(design: &dsl::DddDesign) -> Option<DtoForm> {
    if design.stereotype != dsl::DddStereotype::Dto {
        return None;
    }
    Some(match (&design.dto_source, &design.dto_members) {
        (Some(_), _) => DtoForm::Projection,
        (None, Some(dsl::DddDtoMembers::Declared(_))) => DtoForm::Standalone,
        _ => DtoForm::Mirror,
    })
}

/// The application-wide design pre-pass (DD3): collects the DTO registry and
/// the entity-designed classes, and emits the DTO-name collision
/// diagnostics. Resolution here is **silent** — a failed source or entity
/// resolution contributes no entry, and the stage's own rule-3 error is the
/// one diagnostic.
pub(crate) fn collect_design_prep(
    application: &dsl::DddApplication,
    packages: &[DomainPackage],
    base: Option<&str>,
    local: &mut Vec<Diagnostic>,
) -> DesignPrep {
    let mut prep = DesignPrep::default();
    for module in &application.modules {
        for design in &module.designs {
            if design.stereotype == dsl::DddStereotype::Entity {
                let mut discarded = Vec::new();
                if let Some(key) = lower::resolve_ddd_class(
                    &class_type_ref(&design.class),
                    packages,
                    base,
                    "",
                    &mut discarded,
                ) {
                    prep.entity_classes.push(key);
                }
            }
            let Some(form) = dto_form(design) else {
                continue;
            };
            let name = design.class.text.clone();
            if prep.dtos.iter().any(|dto| dto.name == name) {
                local.push(
                    Diagnostic::error(
                        format!("{E_DTO_NAME_COLLISION} duplicate dto '{name}'"),
                        Some(design.class.span),
                    )
                    .with_help("DTO boundary names are unique across the application"),
                );
                continue;
            }
            // A projection/standalone DTO introduces a new application-wide
            // type name; a mirror DTO's name is its source class (exempt).
            if form != DtoForm::Mirror {
                if let Some(kind) = domain_decl_kind(packages, &name) {
                    local.push(
                        Diagnostic::error(
                            format!(
                                "{E_DTO_NAME_COLLISION} dto '{name}' collides with {kind} \
                                 '{name}'; a dto boundary name must not shadow a domain type"
                            ),
                            Some(design.class.span),
                        )
                        .with_help(
                            "rename the DTO; a mirror DTO (no `from`, no member block) is \
                             exempt because its name is its source class",
                        ),
                    );
                }
            }
            let source = match form {
                DtoForm::Standalone => None,
                DtoForm::Projection => design.dto_source.as_ref().and_then(|source| {
                    let mut discarded = Vec::new();
                    lower::resolve_ddd_class(
                        &qualified_type_ref(source),
                        packages,
                        base,
                        "",
                        &mut discarded,
                    )
                }),
                DtoForm::Mirror => {
                    let mut discarded = Vec::new();
                    lower::resolve_ddd_class(
                        &class_type_ref(&design.class),
                        packages,
                        base,
                        "",
                        &mut discarded,
                    )
                }
            };
            prep.dtos.push(DtoDecl {
                name: name.clone(),
                form,
                module: module.name.text.clone(),
                span: design.class.span,
                source,
            });
            if form != DtoForm::Mirror {
                prep.dto_names.push(name);
            }
        }
    }
    prep
}

/// Lowers one DTO design's boundary shape (DD3): the members in declaration
/// order and the derived direction. `resolved` is the source class (mirror
/// and projection; `None` for a standalone DTO or an unresolved source).
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_dto_shape(
    design: &dsl::DddDesign,
    form: DtoForm,
    resolved: Option<&(String, String)>,
    union: Option<&phero_ir::Model>,
    expr_context: Option<&phero_expr::TypeContext>,
    packages: &[DomainPackage],
    base: Option<&str>,
    prep: &DesignPrep,
    design_source: &str,
    local: &mut Vec<Diagnostic>,
) -> DtoShape {
    let name = design.class.text.clone();
    let source = resolved.map(|(package, class)| TypeRef::Class {
        package: package.clone(),
        name: class.clone(),
    });
    let members = match form {
        DtoForm::Mirror => mirror_members(union, resolved),
        DtoForm::Projection => projection_members(
            design,
            resolved,
            union,
            expr_context,
            prep,
            design_source,
            local,
        ),
        DtoForm::Standalone => declared_members(design, packages, base, prep, local),
    };
    let direction = if members
        .iter()
        .any(|member| member.kind == DtoMemberKind::Derive)
    {
        DtoDirection::Output
    } else {
        DtoDirection::Input
    };
    DtoShape::new(name, direction)
        .with_source(source)
        .with_members(members)
}

/// A mirror DTO's members: every stored feature of the source class (own and
/// inherited, derived features excluded), in declaration order, as a
/// single-segment selection carrying the feature's type and multiplicity.
fn mirror_members(
    union: Option<&phero_ir::Model>,
    resolved: Option<&(String, String)>,
) -> Vec<DtoMember> {
    let (Some(union), Some((package, class))) = (union, resolved) else {
        // The domain (or the source) failed to lower; its diagnostics already
        // block the artifact, so the shape stays member-less rather than
        // inventing a projection.
        return Vec::new();
    };
    class_features(union, package, class)
        .unwrap_or_default()
        .into_iter()
        .filter(|feature| !feature.is_derived)
        .map(|feature| {
            DtoMember::new(
                feature.name.clone(),
                DtoMemberKind::Select,
                DddTypeRef::domain(feature.type_.clone()),
            )
            .with_multiplicity(Some(feature.multiplicity))
            .with_path(vec![feature.name.clone()])
        })
        .collect()
}

/// A projection DTO's members: each `name;` selects a direct feature, each
/// `name = …` lowers as a selection when the right-hand side is a plain
/// feature path (with the path recorded) and as a typed derivation
/// otherwise.
fn projection_members(
    design: &dsl::DddDesign,
    resolved: Option<&(String, String)>,
    union: Option<&phero_ir::Model>,
    expr_context: Option<&phero_expr::TypeContext>,
    prep: &DesignPrep,
    design_source: &str,
    local: &mut Vec<Diagnostic>,
) -> Vec<DtoMember> {
    let Some(members) = design.dto_members.as_ref() else {
        return Vec::new();
    };
    let dsl::DddDtoMembers::Projection(entries) = members else {
        return Vec::new();
    };
    // Member typing needs the fully lowered union and the resolved source
    // class; both are already reported when absent (domain/rule-3 errors).
    let (Some(union), Some((package, class))) = (union, resolved) else {
        return Vec::new();
    };
    let mut out: Vec<DtoMember> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for entry in entries {
        match entry {
            dsl::DddProjectionMember::Select { name, .. } => {
                if duplicate_member(design, name, &names, local) {
                    continue;
                }
                let Some(feature) = feature_named(union, package, class, &name.text) else {
                    local.push(unknown_member(design, union, package, class, name));
                    continue;
                };
                if let Some(entity) = entity_type(&feature.type_, prep) {
                    local.push(entity_member(design, name, entity));
                }
                names.push(name.text.clone());
                out.push(selected_member(name.text.clone(), feature));
            }
            dsl::DddProjectionMember::Assign { name, expr, .. } => {
                if duplicate_member(design, name, &names, local) {
                    continue;
                }
                let raw = &design_source[expr.start..expr.end];
                let text = raw.trim();
                let offset = expr.start + raw.find(text).unwrap_or(0);
                let parsed = phero_expr::parse(text);
                if !parsed.errors.is_empty() {
                    for error in &parsed.errors {
                        local.push(member_parse_error(design, name, offset, error));
                    }
                    continue;
                }
                let Some(ast) = parsed.ast.as_ref() else {
                    continue;
                };
                if let Some(path) = plain_path(ast) {
                    match resolve_path(union, package, class, &path) {
                        Ok((type_, multiplicity, derived)) => {
                            if let Some(entity) = entity_type(&type_, prep) {
                                local.push(entity_member(design, name, entity));
                            }
                            names.push(name.text.clone());
                            // A path ending at a derived source feature is
                            // computed, not copied: lower it as a derivation
                            // with the path as the verbatim expression.
                            let member = if derived {
                                DtoMember::new(
                                    name.text.clone(),
                                    DtoMemberKind::Derive,
                                    DddTypeRef::domain(type_),
                                )
                                .with_multiplicity(multiplicity)
                                .with_expression(Some(path.join(".")))
                            } else {
                                DtoMember::new(
                                    name.text.clone(),
                                    DtoMemberKind::Select,
                                    DddTypeRef::domain(type_),
                                )
                                .with_multiplicity(multiplicity)
                                .with_path(path)
                            };
                            out.push(member);
                        }
                        Err(_segment) => {
                            local.push(
                                Diagnostic::error(
                                    format!(
                                        "{E_DTO_UNKNOWN_MEMBER} unknown dto path '{}' on '{}'",
                                        path.join("."),
                                        design.class.text
                                    ),
                                    Some(name.span),
                                )
                                .with_help(format!(
                                    "'{}' declares: {}",
                                    class,
                                    feature_list(union, package, class)
                                )),
                            );
                        }
                    }
                    continue;
                }
                // A computed member: type it with the source class as `self`
                // and carry the verbatim source text.
                let Some(context) = expr_context else {
                    continue;
                };
                let checker =
                    phero_expr::TypeChecker::new(context.clone()).with_self(package, class);
                match checker.type_of(ast) {
                    Ok(ty) => {
                        let (type_, multiplicity) = match ty_to_ddd(&ty) {
                            Ok(lowered) => lowered,
                            Err(reason) => {
                                local.push(member_type_error(
                                    design,
                                    name,
                                    offset,
                                    ast.span,
                                    &format!("the expression is typed `{ty}`"),
                                    reason,
                                ));
                                continue;
                            }
                        };
                        if let Some(entity) = entity_ddd(&type_, prep) {
                            local.push(entity_member(design, name, entity));
                        }
                        names.push(name.text.clone());
                        out.push(
                            DtoMember::new(name.text.clone(), DtoMemberKind::Derive, type_)
                                .with_multiplicity(multiplicity)
                                .with_expression(Some(text.to_string())),
                        );
                    }
                    Err(errors) => {
                        for error in &errors {
                            local.push(member_type_error(
                                design,
                                name,
                                offset,
                                error.span,
                                &error.message,
                                "fix the expression, or select a plain feature path",
                            ));
                        }
                    }
                }
            }
        }
    }
    out
}

/// A standalone DTO's members: each declared `type name;` resolves its type
/// against the domain union or another DTO boundary. An entity-designed
/// class is refused (project its fields instead).
fn declared_members(
    design: &dsl::DddDesign,
    packages: &[DomainPackage],
    base: Option<&str>,
    prep: &DesignPrep,
    local: &mut Vec<Diagnostic>,
) -> Vec<DtoMember> {
    let Some(members) = design.dto_members.as_ref() else {
        return Vec::new();
    };
    let dsl::DddDtoMembers::Declared(entries) = members else {
        return Vec::new();
    };
    let mut out: Vec<DtoMember> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for member in entries {
        if duplicate_member(design, &member.name, &names, local) {
            continue;
        }
        let before = local.len();
        let type_ =
            lower::resolve_ddd_type_ref(&member.type_ref, packages, base, &prep.dto_names, local);
        if local.len() > before {
            // Resolution diagnostics named the type; re-word them with the
            // member context so the boundary is visible too.
            for diagnostic in &mut local[before..] {
                diagnostic.message = format!(
                    "{E_DTO_MEMBER_TYPE} invalid dto member '{}' of '{}': {}",
                    member.name.text, design.class.text, diagnostic.message
                );
            }
            continue;
        }
        if let Some(entity) = entity_ddd(&type_, prep) {
            local.push(entity_member(design, &member.name, entity));
        }
        names.push(member.name.text.clone());
        out.push(
            DtoMember::new(member.name.text.clone(), DtoMemberKind::Declared, type_)
                .with_multiplicity(member.multiplicity.as_ref().map(lower_multiplicity)),
        );
    }
    out
}

/// Records a signature slot for the deferred DTO input-capability check when
/// it names a DTO boundary (DD3, pherolang#300).
pub(crate) fn record_dto_param(
    registries: &mut Registries<'_>,
    context: String,
    param: &str,
    type_: &DddTypeRef,
    span: Span,
) {
    if let DddTypeRef::Dto { dto } = type_ {
        registries.dto_param_checks.push(DtoParamCheck {
            context,
            param: param.to_string(),
            dto: dto.clone(),
            span,
        });
    }
}

/// The deferred input-capability check (DD3, DD3-06): a DTO with derived
/// members is output-only, so it cannot fill a signature's parameter slot.
/// The diagnostic names every derived member that a caller could not
/// supply.
pub(crate) fn check_dto_parameters(
    checks: &[DtoParamCheck],
    model: &phero_ir::ddd::DddModel,
) -> Vec<Diagnostic> {
    let mut local = Vec::new();
    for check in checks {
        let shape = model
            .modules
            .iter()
            .flat_map(|module| module.designs.iter())
            .filter_map(|design| design.dto_shape.as_ref())
            .find(|shape| shape.name == check.dto);
        let Some(shape) = shape else {
            continue;
        };
        if shape.direction != DtoDirection::Output {
            continue;
        }
        let derived: Vec<&str> = shape
            .members
            .iter()
            .filter(|member| member.kind == DtoMemberKind::Derive)
            .map(|member| member.name.as_str())
            .collect();
        let members = if derived.is_empty() {
            "derived members".to_string()
        } else {
            derived.join(", ")
        };
        local.push(
            Diagnostic::error(
                format!(
                    "{E_DTO_PARAM_NOT_INPUT} dto '{}' is output-only: parameter '{}' of \
                     {} is not input-capable; derived member(s): {members}",
                    check.dto, check.param, check.context
                ),
                Some(check.span),
            )
            .with_help(
                "an output-only DTO has derived members a caller cannot supply; use an \
                 input-capable DTO for the parameter, or make the derived member a select",
            ),
        );
    }
    local
}

/// The array of a DTO parameter's declared member types, converted for the
/// statement-body checker: a DTO-typed slot becomes the synthetic
/// `<dto>.<Name>` class (registered member-less), so passing and returning
/// DTO values type-check while field access reports a clear unknown feature
/// rather than an unknown class.
pub(crate) fn body_params(params: &[DddParam]) -> Vec<phero_ir::OperationParam> {
    params
        .iter()
        .map(|param| phero_ir::OperationParam {
            name: param.name.clone(),
            type_: body_type_ref(&param.type_),
            multiplicity: param.multiplicity,
        })
        .collect()
}

/// One DTO-typed or domain-typed slot as the statement-body checker's
/// [`TypeRef`]: DTOs map to a synthetic class in [`DTO_BINDING_PACKAGE`].
pub(crate) fn body_type_ref(type_: &DddTypeRef) -> TypeRef {
    match type_ {
        DddTypeRef::Domain(type_ref) => type_ref.clone(),
        DddTypeRef::Dto { dto } => TypeRef::Class {
            package: DTO_BINDING_PACKAGE.to_string(),
            name: dto.clone(),
        },
    }
}

/// The expression-language type of a signature slot; used by the condition
/// checker (machine guards, child-op guards) so a DTO parameter binds too.
pub(crate) fn expr_ty(type_: &DddTypeRef) -> Ty {
    match type_ {
        DddTypeRef::Domain(type_ref) => Ty::from_type_ref(type_ref),
        DddTypeRef::Dto { dto } => Ty::class(DTO_BINDING_PACKAGE, dto),
    }
}

/// `true` when `name` is an entity-designed class of the application.
fn entity_type<'a>(type_: &'a TypeRef, prep: &'a DesignPrep) -> Option<&'a str> {
    let TypeRef::Class { package, name } = type_ else {
        return None;
    };
    prep.entity_classes
        .iter()
        .any(|(entity_package, entity_name)| entity_package == package && entity_name == name)
        .then_some(name.as_str())
}

/// [`entity_type`] over a signature/member type slot; a DTO boundary is
/// never an entity.
fn entity_ddd<'a>(type_: &'a DddTypeRef, prep: &'a DesignPrep) -> Option<&'a str> {
    match type_ {
        DddTypeRef::Domain(type_ref) => entity_type(type_ref, prep),
        DddTypeRef::Dto { .. } => None,
    }
}

/// The feature named `name` on `package.class` (inherited features
/// included), or `None`.
fn feature_named<'a>(
    union: &'a phero_ir::Model,
    package: &str,
    class: &str,
    name: &str,
) -> Option<&'a phero_ir::Feature> {
    class_features(union, package, class)?
        .into_iter()
        .find(|feature| feature.name == name)
}

/// Resolves a plain feature path against the source class, inherited
/// features included: `Ok((type, multiplicity))` of the path's final
/// feature; `Err(segment)` names the first segment that does not resolve (or
/// traverses a non-class feature).
fn resolve_path(
    union: &phero_ir::Model,
    package: &str,
    class: &str,
    path: &[String],
) -> Result<(TypeRef, Option<Multiplicity>, bool), String> {
    let mut current = (package.to_string(), class.to_string());
    for (index, segment) in path.iter().enumerate() {
        let Some(feature) = feature_named(union, &current.0, &current.1, segment) else {
            return Err(segment.clone());
        };
        if index + 1 == path.len() {
            return Ok((
                feature.type_.clone(),
                Some(feature.multiplicity),
                feature.is_derived,
            ));
        }
        match &feature.type_ {
            TypeRef::Class { package, name } => current = (package.clone(), name.clone()),
            _ => return Err(segment.clone()),
        }
    }
    Err(path.last().cloned().unwrap_or_default())
}

/// The segments of an expression that is a plain feature path (`a` or
/// `a.b.c`), or `None` for anything with an operator, literal, call, or
/// safe-navigation step.
fn plain_path(expr: &phero_expr::Expr) -> Option<Vec<String>> {
    match &expr.kind {
        ExprKind::Name(name) => Some(vec![name.clone()]),
        ExprKind::FeatureAccess {
            receiver,
            name,
            optional_safe: false,
        } => {
            let mut path = plain_path(receiver)?;
            path.push(name.value.clone());
            Some(path)
        }
        _ => None,
    }
}

/// Maps an expression type onto a DTO member type plus its cardinality: a
/// list is `0..*`, an option is `0..1`, and anything the boundary cannot
/// carry (the `null` literal) is `None`.
fn ty_to_ddd(ty: &Ty) -> Result<(DddTypeRef, Option<Multiplicity>), &'static str> {
    match ty {
        Ty::Primitive(primitive) => Ok((DddTypeRef::domain(TypeRef::Primitive(*primitive)), None)),
        Ty::Named {
            kind,
            package,
            name,
        } => {
            let type_ref = match kind {
                phero_expr::NamedKind::Class => TypeRef::Class {
                    package: package.clone(),
                    name: name.clone(),
                },
                phero_expr::NamedKind::Enum => TypeRef::Enum {
                    package: package.clone(),
                    name: name.clone(),
                },
                phero_expr::NamedKind::Datatype => TypeRef::Datatype {
                    package: package.clone(),
                    name: name.clone(),
                },
                phero_expr::NamedKind::Interface => TypeRef::Interface {
                    package: package.clone(),
                    name: name.clone(),
                },
                phero_expr::NamedKind::Vocabulary => TypeRef::Vocabulary {
                    package: package.clone(),
                    name: name.clone(),
                },
            };
            Ok((DddTypeRef::domain(type_ref), None))
        }
        Ty::List(inner) => {
            let (type_ref, nested) = ty_to_ddd(inner)?;
            if nested.is_some() {
                return Err(
                    "a transport member has one cardinality; nested collections and \
                     optionals are not representable",
                );
            }
            Ok((type_ref, Some(Multiplicity::MANY)))
        }
        Ty::Option(inner) => {
            let (type_ref, nested) = ty_to_ddd(inner)?;
            if nested.is_some() {
                return Err(
                    "a transport member has one cardinality; nested collections and \
                     optionals are not representable",
                );
            }
            Ok((type_ref, Some(Multiplicity::OPTIONAL)))
        }
        Ty::Null => {
            Err("the null literal has no value type; a transport member needs a concrete type")
        }
    }
}

/// A selected member carrying the feature's type, multiplicity, and
/// single-source path. A **derived** source feature is computed, not copied,
/// so it lowers as a derivation with the feature name as its expression.
fn selected_member(name: String, feature: &phero_ir::Feature) -> DtoMember {
    let member = if feature.is_derived {
        DtoMember::new(
            name.clone(),
            DtoMemberKind::Derive,
            DddTypeRef::domain(feature.type_.clone()),
        )
        .with_expression(Some(name))
    } else {
        DtoMember::new(
            name.clone(),
            DtoMemberKind::Select,
            DddTypeRef::domain(feature.type_.clone()),
        )
        .with_path(vec![name])
    };
    member.with_multiplicity(Some(feature.multiplicity))
}

/// The duplicate-member rule: records the output name on first sight and
/// emits the coded error on a repeat.
fn duplicate_member(
    design: &dsl::DddDesign,
    name: &dsl::Name,
    names: &[String],
    local: &mut Vec<Diagnostic>,
) -> bool {
    if names.iter().any(|seen| seen == &name.text) {
        local.push(
            Diagnostic::error(
                format!(
                    "{E_DTO_DUPLICATE_MEMBER} dto '{}': duplicate member '{}'",
                    design.class.text, name.text
                ),
                Some(name.span),
            )
            .with_help("a DTO member is declared once per output name"),
        );
        true
    } else {
        false
    }
}

/// The unknown-member diagnostic, listing the source class's features.
fn unknown_member(
    design: &dsl::DddDesign,
    union: &phero_ir::Model,
    package: &str,
    class: &str,
    name: &dsl::Name,
) -> Diagnostic {
    Diagnostic::error(
        format!(
            "{E_DTO_UNKNOWN_MEMBER} unknown dto member '{}' on '{}'",
            name.text, design.class.text
        ),
        Some(name.span),
    )
    .with_help(format!(
        "'{class}' declares: {}",
        feature_list(union, package, class)
    ))
}

/// The entity-member diagnostic: a DTO may not carry a designed entity.
fn entity_member(design: &dsl::DddDesign, name: &dsl::Name, entity: &str) -> Diagnostic {
    Diagnostic::error(
        format!(
            "{E_DTO_ENTITY_MEMBER} dto member '{}' of '{}' carries entity '{entity}'; a \
             transport shape may not embed mutable entity state",
            name.text, design.class.text
        ),
        Some(name.span),
    )
    .with_help("project the entity's fields as plain members, or refer to it by id")
}

/// One rebased parse failure of a projection's assignment expression.
fn member_parse_error(
    design: &dsl::DddDesign,
    name: &dsl::Name,
    offset: usize,
    error: &phero_expr::ExprError,
) -> Diagnostic {
    Diagnostic::error(
        format!(
            "{E_DTO_MEMBER_TYPE} invalid dto member '{}' of '{}': {}",
            name.text, design.class.text, error.message
        ),
        Some(rebase(offset, error.span)),
    )
    .with_help("fix the projection expression")
}

/// One rebased typing failure of a projection's assignment expression.
fn member_type_error(
    design: &dsl::DddDesign,
    name: &dsl::Name,
    offset: usize,
    span: phero_expr::Span,
    detail: &str,
    help: &str,
) -> Diagnostic {
    Diagnostic::error(
        format!(
            "{E_DTO_MEMBER_TYPE} invalid dto member '{}' of '{}': {detail}",
            name.text, design.class.text
        ),
        Some(rebase(offset, span)),
    )
    .with_help(help)
}

/// Rebases an expression-relative span onto the design source.
fn rebase(offset: usize, span: phero_expr::Span) -> Span {
    (offset.saturating_add(span.start)..offset.saturating_add(span.end)).into()
}

/// The comma-joined feature names of a class, for the unknown-member help.
fn feature_list(union: &phero_ir::Model, package: &str, class: &str) -> String {
    class_features(union, package, class)
        .unwrap_or_default()
        .iter()
        .map(|feature| feature.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The domain declaration kind a DTO name would shadow, if any.
fn domain_decl_kind(packages: &[DomainPackage], name: &str) -> Option<&'static str> {
    packages
        .iter()
        .find_map(|package| package.kinds.get(name))
        .map(|kind| match kind {
            TopKind::Class => "class",
            TopKind::Interface => "interface",
            TopKind::Enum => "enum",
            TopKind::Datatype => "datatype",
            TopKind::Vocabulary => "vocabulary",
            TopKind::Actors => "actors block",
        })
}

/// A `QualifiedName` as a `TypeRef` (the shared resolution helpers' input).
pub(crate) fn qualified_type_ref(name: &dsl::QualifiedName) -> dsl::TypeRef {
    dsl::TypeRef {
        name: name.clone(),
        span: name.span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_member_carries_at_most_one_cardinality() {
        let list = ty_to_ddd(&Ty::List(Box::new(Ty::string()))).expect("a list");
        assert_eq!(list.1, Some(Multiplicity::MANY));
        let optional = ty_to_ddd(&Ty::Option(Box::new(Ty::string()))).expect("an option");
        assert_eq!(optional.1, Some(Multiplicity::OPTIONAL));

        let list_of_options = ty_to_ddd(&Ty::List(Box::new(Ty::Option(Box::new(Ty::string())))));
        assert!(
            list_of_options.is_err(),
            "nested cardinality is not representable"
        );
        let option_of_lists = ty_to_ddd(&Ty::Option(Box::new(Ty::List(Box::new(Ty::string())))));
        assert!(
            option_of_lists.is_err(),
            "nested cardinality is not representable"
        );
        assert!(ty_to_ddd(&Ty::Null).is_err(), "null has no value type");
    }
}
