//! Entity/value/dto design lowering: repositories and their operations,
//! plus the value-contract, dto-ops, bare-`String`-status, and scaffold
//! rules (12, 14, 15, 16). A `value` design also lowers the fixed strict
//! [`ValueSemantics`](phero_ir::ddd::ValueSemantics) block here (DD2, issue
//! pherolang#299); the rules that keep the properties honest are deferred to
//! [`super::values`].

use phero_diagnostics::ddd::{E_AGGREGATE_NON_ENTITY, E_DTO_ENTITY_ONLY, E_DTO_OPERATION};
use phero_ir::ddd::{
    BuiltinRepositoryOp, DddParam, Design, DesignFlags, Invariant, Repository, RepositoryOperation,
    Stereotype,
};
use phero_syntax::ast as dsl;

use crate::diagnostic::Diagnostic;
use crate::lower::{self, DomainPackage};

use super::dtos::{DesignPrep, DtoForm};
use super::errors::ErrorTable;
use super::machines::{class_operations, MachineScopes};
use super::*;

/// Stage 5 — one module's designs: rule 3 (the target resolves), rule 2
/// (one design per class), rule 4 (flag/stereotype compatibility), the
/// stereotype contracts (rules 12, 15, 14), the repository (rule 5, rule
/// 2, op lowering) and machine lowering, and the entity registries the
/// deferred rules 6 and 11 read. Returns the lowered designs and the
/// stage's diagnostics.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_module_designs<'a>(
    module: &'a dsl::DddModule,
    design_source: &str,
    packages: &[DomainPackage],
    base: Option<&str>,
    union: Option<&phero_ir::Model>,
    machines: &MachineScopes,
    expr_context: Option<&phero_expr::TypeContext>,
    prep: &DesignPrep,
    error_table: &ErrorTable,
    registries: &mut Registries<'a>,
) -> (Vec<Design>, Vec<Diagnostic>) {
    let mut local: Vec<Diagnostic> = Vec::new();
    let mut designs_out = Vec::new();
    // Every resolved design class paired with its artifact position: the
    // child-operation stage (5b, issue pherolang#179) attaches `extend`
    // blocks to their target design by class. DTO designs are not aggregate
    // roots, so they never enter this index.
    let mut design_index: Vec<((String, String), usize)> = Vec::new();
    for design in &module.designs {
        let prepared = prep.declaration(design);
        let form = prepared.map(|decl| decl.form).or_else(|| dto_form(design));
        let module_label = prepared
            .map(|decl| decl.module.as_str())
            .unwrap_or(&module.name.text);
        let authored = prepared
            .map(|decl| decl.name.as_str())
            .unwrap_or(design.class.text.as_str());
        // Rule 3 — the design target must resolve to a domain class. A
        // projection resolves its `from` source; a standalone DTO has no
        // source at all; every other design resolves its class name.
        let design_help = match form {
            Some(DtoForm::Projection) => format!(
                "the projection source of dto '{authored}' in module '{module_label}' must \
                 target a class declared in the imported domains"
            ),
            _ => format!(
                "the design of '{authored}' in module '{module_label}' must target a class \
                 declared in the imported domains"
            ),
        };
        let resolved = match form {
            Some(DtoForm::Standalone) => None,
            Some(DtoForm::Projection) => design
                .dto_source
                .as_ref()
                .and_then(|source| {
                    lower::resolve_ddd_class(
                        &qualified_type_ref(source),
                        packages,
                        base,
                        &design_help,
                        &mut local,
                    )
                })
                .or_else(|| prepared.and_then(|decl| decl.source.clone())),
            _ => lower::resolve_ddd_class(
                &class_type_ref(&design.class),
                packages,
                base,
                &design_help,
                &mut local,
            )
            .or_else(|| prepared.and_then(|decl| decl.source.clone())),
        };

        // Rule 2 — one design per class per application, keyed by the
        // resolved class (two spellings of one class collide). Projection
        // and standalone DTOs introduce their own boundary names instead
        // (the pre-pass checks those for uniqueness); a projection may
        // legitimately coexist with its source class's entity design.
        let key = match form {
            Some(DtoForm::Projection | DtoForm::Standalone) => None,
            _ => Some(
                resolved
                    .clone()
                    .unwrap_or_else(|| (String::new(), design.class.text.clone())),
            ),
        };
        if let Some(key) = &key {
            if registries.design_keys.contains(key) {
                local.push(
                    Diagnostic::error(
                        format!("duplicate design for class '{}'", design.class.text),
                        Some(design.class.span),
                    )
                    .with_help("a class has one design per application"),
                );
            } else {
                registries.design_keys.push(key.clone());
            }
        }

        // Rule 4 — flag/stereotype compatibility.
        check_flags(design, &mut local);

        // DD4 (pherolang#301): the authored aggregate boundary is entity-only;
        // an `aggregate <Root>` target resolves like every design reference.
        let mut resolved_aggregate = None;
        if let Some(boundary) = &design.boundary {
            if design.stereotype != dsl::DddStereotype::Entity {
                local.push(
                    Diagnostic::error(
                        format!(
                            "{E_AGGREGATE_NON_ENTITY} design '{}' declares an aggregate boundary, \
                             but is a {}; only an entity has an aggregate boundary",
                            design.class.text,
                            stereotype_label(design.stereotype)
                        ),
                        Some(boundary.span),
                    )
                    .with_help("remove the boundary clause, or design the class as an `entity`"),
                );
            } else if let dsl::DddBoundary::Aggregate(target) = &boundary.kind {
                resolved_aggregate = lower::resolve_ddd_class(
                    &qualified_type_ref(target),
                    packages,
                    base,
                    &format!(
                        "aggregate root '{}' of '{}' must target a class declared in the \
                         imported domains",
                        target.full_name(),
                        design.class.text
                    ),
                    &mut local,
                );
            }
        }

        // Rule 12 — the value contract.
        local.extend(rule_12_value_contract(design, resolved.as_ref(), union));

        // Rule 15 — a dto carries no behavior (DD3: an error, not the
        // pre-DD3 warning). A projection reads a source class that may
        // legitimately carry ops, so the rule covers the mirror form (the
        // boundary *is* the class's shape).
        local.extend(rule_15_dto_ops_errors(
            design,
            form,
            resolved.as_ref(),
            union,
        ));

        // Rule 16 — status-like fields are typed lifecycles.
        local.extend(rule_16_bare_string_status(design, resolved.as_ref(), union));

        // Rule 14 — scaffold declares the repository it scaffolds around.
        if let Some(diagnostic) = rule_14_scaffold_repository(design) {
            local.push(diagnostic);
        }

        let mut design_out = match form {
            Some(DtoForm::Standalone) => Design::standalone_dto(design.class.text.clone()),
            Some(DtoForm::Projection) => Design::new(
                design
                    .dto_source
                    .as_ref()
                    .map(|source| source.full_name())
                    .unwrap_or_default(),
                Stereotype::Dto,
            ),
            _ => Design::new(design.class.text.clone(), stereotype(design)),
        };
        // The resolved companion (DD1): the class the authored name binds
        // to. Absent only when rule 3 already failed (the artifact drops)
        // or for a standalone DTO (which has no source class at all).
        design_out.type_ref = resolved
            .as_ref()
            .map(|(package, class)| phero_ir::TypeRef::Class {
                package: package.clone(),
                name: class.clone(),
            });
        design_out.is_abstract = design.is_abstract;
        design_out.flags = flags(design);
        // DD2 (pherolang#299): every value design — and only a value
        // design — lowers the fixed strict semantics block.
        design_out.semantics = value_semantics(design);

        // DD5 (pherolang#302): invariants lower verbatim, in declaration
        // order. The type-check pass is deferred to `invariants.rs`.
        design_out.invariants = design
            .invariants
            .iter()
            .map(|span| {
                Invariant::new(design_source.get(span.start..span.end).unwrap_or("").trim())
            })
            .collect();
        super::invariants::check_design_invariants(
            design,
            design_source,
            resolved.as_ref(),
            expr_context,
            machines,
            &mut local,
        );

        // DD3 (pherolang#300): every dto design lowers its boundary shape.
        if let Some(form) = form {
            let shape = lower_dto_shape(
                design,
                form,
                resolved.as_ref(),
                union,
                expr_context,
                packages,
                base,
                prep,
                design_source,
                &mut local,
            );
            design_out.dto_shape = Some(shape);
            if let Some(machine) = &design.machine {
                local.push(
                    Diagnostic::error(
                        format!(
                            "{E_DTO_ENTITY_ONLY} dto design '{}' carries a machine block; a \
                             dto is a transfer shape with no lifecycle",
                            design.class.text
                        ),
                        Some(machine.span),
                    )
                    .with_help("remove the `machine` block, or design the class as an `entity`"),
                );
            }
        }

        if let Some(repository) = &design.repository {
            let (repository_out, stage_diags) = lower_design_repository(
                design,
                repository,
                module,
                resolved.as_ref(),
                packages,
                base,
                union,
                machines,
                expr_context,
                design_source,
                prep,
                error_table,
                registries,
            );
            local.extend(stage_diags);
            design_out.repository = repository_out;
        }

        if let Some(machine) = &design.machine {
            if form.is_none() {
                design_out.machine = lower_design_machine(
                    machine,
                    design,
                    resolved.as_ref(),
                    union,
                    packages,
                    base,
                    &machines.machines,
                    expr_context,
                    design_source,
                    &mut local,
                );
            }
        }

        if design.stereotype == dsl::DddStereotype::Entity {
            if let Some((package, class)) = resolved.as_ref() {
                // Every stereotyped entity is a potential container; those
                // with a repository are additionally candidates for the
                // non-root verdict.
                registries.entity_designs.push(EntityDesign {
                    package: package.clone(),
                    class: class.clone(),
                    module: module.name.text.clone(),
                    authored: design.boundary.clone(),
                    resolved_aggregate: resolved_aggregate.clone(),
                    span: design.span,
                });
                if design_out.repository.is_some() {
                    registries.root_candidates.push(RootCandidate {
                        repository_span: design.repository.as_ref().expect("checked above").span,
                        authored: design.class.text.clone(),
                        package: package.clone(),
                        class: class.clone(),
                    });
                }
            }
        }

        // DD2 (pherolang#299): every value design awaits the deferred strict
        // value-semantics pass, which reads the completed entity-design
        // registry (so an entity designed later still convicts).
        if design.stereotype == dsl::DddStereotype::Value {
            registries.value_designs.push(ValueDesign {
                resolved: resolved.clone(),
                module: module.name.text.clone(),
                authored: design.class.text.clone(),
                span: design.class.span,
                is_abstract: design.is_abstract,
            });
        }

        if let (None, Some(key)) = (form, resolved) {
            design_index.push((key, designs_out.len()));
        }
        designs_out.push(design_out);
    }
    // Stage 5b — the module's `extend` blocks (issue pherolang#179), attached
    // to their target designs after every design lowered (a block may
    // precede its target in source order). Diagnostics ride the design
    // stage's emission slot.
    local.extend(lower_module_child_operations(
        module,
        design_source,
        packages,
        base,
        union,
        expr_context,
        prep,
        &design_index,
        &mut designs_out,
        registries,
    ));
    (designs_out, local)
}
/// Rule 12 — the value contract: a value design is an immutable value
/// object (Sculptor's ValueObject default, with no `!immutable` opt-out),
/// so every stored feature of the class must be `readonly` in the domain
/// model. Feature-level checks run only against a fully lowered union (a
/// failed domain may hide features; its diagnostics already block the
/// artifact).
fn rule_12_value_contract(
    design: &dsl::DddDesign,
    resolved: Option<&(String, String)>,
    union: Option<&phero_ir::Model>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if design.stereotype == dsl::DddStereotype::Value {
        if let (Some((package, class)), Some(union)) = (resolved, union) {
            if let Some(features) = class_features(union, package, class) {
                for feature in features {
                    if !feature.is_derived && !feature.is_read_only {
                        diagnostics.push(
                            Diagnostic::error(
                                format!(
                                    "value design '{}': feature '{}' is not \
                                     readonly; a value object is immutable",
                                    design.class.text, feature.name
                                ),
                                Some(design.class.span),
                            )
                            .with_help(
                                "mark the feature `readonly` in the domain model, \
                                 or design the class `entity` or `dto`",
                            ),
                        );
                    }
                }
            }
        }
    }
    diagnostics
}
/// Rule 15 — a dto carries no behavior (DD3, issue pherolang#300): a DTO is
/// a behavior-free transport shape, so an `op` on its class is an error —
/// the pre-DD3 warning promoted. `derived` features stay legal (a computed
/// wire field, not behavior). The mirror form is the one whose boundary
/// *is* the class's shape; a projection reads a source class that may
/// legitimately carry behavior, so its ops are not the boundary's. The
/// union is consulted only when fully lowered (a failed domain may hide
/// ops).
fn rule_15_dto_ops_errors(
    design: &dsl::DddDesign,
    form: Option<DtoForm>,
    resolved: Option<&(String, String)>,
    union: Option<&phero_ir::Model>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if form != Some(DtoForm::Mirror) {
        return diagnostics;
    }
    if let (Some((package, class)), Some(union)) = (resolved, union) {
        if let Some(operations) = class_operations(union, package, class) {
            for operation in operations {
                diagnostics.push(
                    Diagnostic::error(
                        format!(
                            "{E_DTO_OPERATION} dto design '{}': class carries op '{}'; a \
                             dto is a transfer record and carries no behavior",
                            design.class.text, operation.name
                        ),
                        Some(design.class.span),
                    )
                    .with_help(
                        "move the behavior to an aggregate-owned class `op`, or declare \
                         a service operation that delegates to it; a `derived` member \
                         stays legal",
                    ),
                );
            }
        }
    }
    diagnostics
}
/// Rule 16 — a status-like field (`status` or `state`) typed as a bare
/// `String` on a designed entity is a lifecycle the model can never state:
/// with a machine declared, the enum-typed binding check already fires, so
/// the bare-`String` shape is exactly the silent no-machine case. The
/// check is entity semantics (the machine rule's scope): value objects and
/// dtos are exempt, and enum-typed fields stay legal — an enum without a
/// machine is a legitimate modeling choice. Feature-level checks run only
/// against a fully lowered union (a failed domain may hide features; its
/// diagnostics already block the artifact), one diagnostic per offending
/// field.
fn rule_16_bare_string_status(
    design: &dsl::DddDesign,
    resolved: Option<&(String, String)>,
    union: Option<&phero_ir::Model>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if design.stereotype != dsl::DddStereotype::Entity {
        return diagnostics;
    }
    if let (Some((package, class)), Some(union)) = (resolved, union) {
        if let Some(features) = class_features(union, package, class) {
            for feature in features {
                if matches!(feature.name.as_str(), "status" | "state")
                    && feature.type_
                        == phero_ir::TypeRef::Primitive(phero_ir::PrimitiveType::String)
                {
                    diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "entity '{}' field '{}' is a bare String; status-like \
                                 fields carry a state machine",
                                design.class.text, feature.name
                            ),
                            Some(design.class.span),
                        )
                        .with_help(format!(
                            "declare an enum in the domain model and a machine \
                             '{}: <Enum>' on the design",
                            feature.name
                        )),
                    );
                }
            }
        }
    }
    diagnostics
}
/// Rule 14 — scaffold materializes the CRUD operations (Sculptor
/// auto-creates both the repository and the service); the artifact stays
/// validation-only — nothing is synthesized — so `scaffold` audits that
/// the author declared the repository it would scaffold around. The
/// service remains the author's.
fn rule_14_scaffold_repository(design: &dsl::DddDesign) -> Option<Diagnostic> {
    if design.flags.scaffold.is_some_and(|flag| !flag.negated)
        && design.stereotype == dsl::DddStereotype::Entity
        && design.repository.is_none()
    {
        Some(
            Diagnostic::error(
                format!(
                    "scaffolded entity '{}' declares no repository",
                    design.class.text
                ),
                Some(design.flags.scaffold.expect("checked above").span),
            )
            .with_help(format!(
                "add `repository {}Repository {{ findById; findAll; save; \
                 delete; }}` — scaffold generates the CRUD operations over it",
                design.class.text
            )),
        )
    } else {
        None
    }
}
/// One design's repository: rule 5 (repositories live on entity designs
/// only), rule 2 (repository names are unique application-wide), the
/// operation lowering (built-ins and rule-7 signatures), and the registry
/// entry the deferred rules 8 and 9 read. Returns the lowered repository
/// and the stage's diagnostics.
#[allow(clippy::too_many_arguments)]
fn lower_design_repository<'a>(
    design: &'a dsl::DddDesign,
    repository: &'a dsl::DddRepository,
    module: &'a dsl::DddModule,
    resolved: Option<&(String, String)>,
    packages: &[DomainPackage],
    base: Option<&str>,
    union: Option<&phero_ir::Model>,
    machines: &MachineScopes,
    expr_context: Option<&phero_expr::TypeContext>,
    design_source: &str,
    prep: &DesignPrep,
    error_table: &ErrorTable,
    registries: &mut Registries<'a>,
) -> (Option<Repository>, Vec<Diagnostic>) {
    let mut local: Vec<Diagnostic> = Vec::new();

    // Rule 5 — repositories live on entity designs only. The DTO variant
    // carries the DD3 entity-only code (a DTO has no persistence).
    if design.stereotype != dsl::DddStereotype::Entity {
        let code = if design.stereotype == dsl::DddStereotype::Dto {
            format!("{E_DTO_ENTITY_ONLY} ")
        } else {
            String::new()
        };
        local.push(Diagnostic::error(
            format!(
                "{code}repository '{}' requires an entity design; '{}' is designed as a {}",
                repository.name.text,
                design.class.text,
                stereotype_label(design.stereotype)
            ),
            Some(repository.span),
        ));
    }

    // Rule 2 — repository names are unique application-wide.
    if registries
        .repositories
        .iter()
        .any(|(name, _)| *name == repository.name.text)
    {
        local.push(
            Diagnostic::error(
                format!("duplicate repository '{}'", repository.name.text),
                Some(repository.name.span),
            )
            .with_help(format!(
                "'{}' is already declared in this application",
                repository.name.text
            )),
        );
    }

    let operations = lower_repository_ops(
        repository,
        module,
        packages,
        base,
        &mut local,
        design_source,
        resolved,
        union,
        machines,
        expr_context,
        &prep.dto_names,
        error_table,
        registries,
    );
    if !registries
        .repositories
        .iter()
        .any(|(name, _)| *name == repository.name.text)
    {
        registries.repositories.push((
            repository.name.text.clone(),
            RepositoryEntry {
                module: module.name.text.clone(),
                ops: repository
                    .operations
                    .iter()
                    .map(|op| op.name.text.clone())
                    .collect(),
                op_throws: operations
                    .iter()
                    .map(|operation| {
                        (
                            operation.name.clone(),
                            operation
                                .throws
                                .iter()
                                .map(|reference| reference.name.clone())
                                .collect(),
                        )
                    })
                    .collect(),
            },
        ));
    }
    (
        Some(Repository {
            name: repository.name.text.clone(),
            operations,
        }),
        local,
    )
}
/// The wire stereotype of a parsed design.
fn stereotype(design: &dsl::DddDesign) -> Stereotype {
    match design.stereotype {
        dsl::DddStereotype::Entity => Stereotype::Entity,
        dsl::DddStereotype::Value => Stereotype::Value,
        dsl::DddStereotype::Dto => Stereotype::Dto,
    }
}
/// The lowercase stereotype noun used in diagnostics.
pub(crate) fn stereotype_label(stereotype: dsl::DddStereotype) -> &'static str {
    match stereotype {
        dsl::DddStereotype::Entity => "entity",
        dsl::DddStereotype::Value => "value",
        dsl::DddStereotype::Dto => "dto",
    }
}
/// The wire design flags of a parsed design. `auditable` and
/// `optimisticLocking` default **on** for entity designs (Sculptor's
/// polarity): absence lowers to the stereotype default, `!flag` lowers to
/// `false`, a positive occurrence to `true`. The remaining flags default
/// off, so only a positive occurrence sets them.
fn flags(design: &dsl::DddDesign) -> DesignFlags {
    let entity = design.stereotype == dsl::DddStereotype::Entity;
    let effective = |occurrence: Option<dsl::DddFlagUse>, default_on: bool| match occurrence {
        Some(flag) => !flag.negated,
        None => entity && default_on,
    };
    DesignFlags {
        scaffold: design.flags.scaffold.is_some_and(|flag| !flag.negated),
        auditable: effective(design.flags.auditable, true),
        optimistic_locking: effective(design.flags.optimistic_locking, true),
        non_persistent: design
            .flags
            .non_persistent
            .is_some_and(|flag| !flag.negated),
        cache: design.flags.cache.is_some_and(|flag| !flag.negated),
    }
}
/// Rule 4 — flag/stereotype compatibility plus negation legality:
/// `scaffold`, `auditable`, and `optimisticLocking` belong to entity
/// designs, `nonPersistent` to value and dto designs; `cache` is legal
/// everywhere. A negated flag is only meaningful where the flag defaults
/// on (`!auditable`/`!optimisticLocking` on an entity design) — negating a
/// default-off flag is a written contradiction, and an entity-only flag is
/// rejected on value/dto designs whatever its polarity.
fn check_flags(design: &dsl::DddDesign, local: &mut Vec<Diagnostic>) {
    for (occurrence, keyword) in [
        (design.flags.scaffold, "scaffold"),
        (design.flags.non_persistent, "nonPersistent"),
        (design.flags.cache, "cache"),
    ] {
        if let Some(flag) = occurrence.filter(|flag| flag.negated) {
            local.push(Diagnostic::error(
                format!("'!{keyword}' is meaningless; '{keyword}' defaults to off"),
                Some(flag.span),
            ));
        }
    }
    let entity_only = [
        (design.flags.scaffold, "scaffold"),
        (design.flags.auditable, "auditable"),
        (design.flags.optimistic_locking, "optimisticLocking"),
    ];
    match design.stereotype {
        dsl::DddStereotype::Entity => {
            if let Some(span) = design.flags.non_persistent.map(|flag| flag.span) {
                local.push(Diagnostic::error(
                    format!(
                        "flag 'nonPersistent' requires a value or dto design; '{}' is \
                         designed as an entity",
                        design.class.text
                    ),
                    Some(span),
                ));
            }
        }
        dsl::DddStereotype::Value | dsl::DddStereotype::Dto => {
            for (occurrence, flag) in entity_only {
                if let Some(span) = occurrence.map(|flag| flag.span) {
                    let code = if design.stereotype == dsl::DddStereotype::Dto {
                        format!("{E_DTO_ENTITY_ONLY} ")
                    } else {
                        String::new()
                    };
                    local.push(Diagnostic::error(
                        format!(
                            "{code}flag '{flag}' requires an entity design; '{}' is designed \
                             as a {}",
                            design.class.text,
                            stereotype_label(design.stereotype)
                        ),
                        Some(span),
                    ));
                }
            }
        }
    }
}
/// Lowers one repository's operations: built-ins keep their keyword name and
/// no signature; declared operations type-check their signature against the
/// domain model. Rule 2 — operation names are unique within one repository.
#[allow(clippy::too_many_arguments)]
fn lower_repository_ops<'a>(
    repository: &'a dsl::DddRepository,
    module: &'a dsl::DddModule,
    packages: &[DomainPackage],
    base: Option<&str>,
    local: &mut Vec<Diagnostic>,
    source: &str,
    self_class: Option<&(String, String)>,
    union: Option<&phero_ir::Model>,
    machines: &MachineScopes,
    context: Option<&phero_expr::TypeContext>,
    dto_names: &[String],
    error_table: &ErrorTable,
    registries: &mut Registries<'a>,
) -> Vec<RepositoryOperation> {
    let mut ops: Vec<String> = Vec::new();
    let mut operations = Vec::new();
    for op in &repository.operations {
        if ops.contains(&op.name.text) {
            local.push(
                Diagnostic::error(
                    format!(
                        "duplicate operation '{}' in repository '{}'",
                        op.name.text, repository.name.text
                    ),
                    Some(op.name.span),
                )
                .with_help(format!(
                    "'{}' is already declared in this repository",
                    op.name.text
                )),
            );
        } else {
            ops.push(op.name.text.clone());
        }
        if let Some(builtin) = op.builtin {
            operations.push(
                RepositoryOperation::builtin(
                    op.name.text.clone(),
                    match builtin {
                        dsl::DddBuiltinOp::FindById => BuiltinRepositoryOp::FindById,
                        dsl::DddBuiltinOp::FindAll => BuiltinRepositoryOp::FindAll,
                        dsl::DddBuiltinOp::FindByExample => BuiltinRepositoryOp::FindByExample,
                        dsl::DddBuiltinOp::FindByKeys => BuiltinRepositoryOp::FindByKeys,
                        dsl::DddBuiltinOp::Save => BuiltinRepositoryOp::Save,
                        dsl::DddBuiltinOp::Delete => BuiltinRepositoryOp::Delete,
                    },
                )
                .with_protected(op.is_protected),
            );
        } else {
            let (return_type, return_multiplicity) = op
                .return_type
                .as_ref()
                .map(|type_ref| {
                    signature_type(
                        type_ref,
                        op.multiplicity.as_ref(),
                        packages,
                        base,
                        dto_names,
                        local,
                    )
                })
                .map_or((None, None), |(type_, mult)| (Some(type_), mult));
            let params: Vec<DddParam> = op
                .params
                .iter()
                .map(|param| {
                    let (type_, multiplicity) = signature_type(
                        &param.type_ref,
                        param.multiplicity.as_ref(),
                        packages,
                        base,
                        dto_names,
                        local,
                    );
                    record_dto_param(
                        registries,
                        format!(
                            "operation '{}' of repository '{}'",
                            op.name.text, repository.name.text
                        ),
                        &param.name.text,
                        &type_,
                        param.type_ref.span,
                    );
                    DddParam::new(param.name.text.clone(), type_).with_multiplicity(multiplicity)
                })
                .collect();
            // The declared operation's `throws` vocabulary (DD7): resolved
            // against the application's error table and the module's private
            // declarations. Built-ins take no clause (the parser refuses it).
            let op_context = format!(
                "operation '{}' of repository '{}'",
                op.name.text, repository.name.text
            );
            let throws: Vec<phero_ir::ddd::ErrorRef> = op
                .throws
                .iter()
                .filter_map(|name| {
                    error_table.resolve_throw(name, &module.name.text, &op_context, local)
                })
                .collect();
            // DD9: a query operation's `where`/`sort`/`page` clauses, typed
            // against the designed class and the operation's parameters. A
            // non-query operation gets `None` and the historical shape.
            let query = super::queries::lower_query(
                op,
                super::queries::QueryCheck {
                    label: op_context.clone(),
                    source,
                    design_class: self_class,
                    union,
                    context,
                    params: &params,
                },
                local,
            );
            // A `capability` clause is legal only on the query form (a read
            // authorization); the parser refuses it elsewhere.
            let mut capabilities: Vec<String> = Vec::new();
            for capability in &op.capabilities {
                capabilities.push(capability.text.clone());
                registries.capability_checks.push(CapabilityCheck {
                    context: op_context.clone(),
                    capability,
                });
            }
            operations.push(
                RepositoryOperation::declared(op.name.text.clone(), return_type, params)
                    .with_return_multiplicity(return_multiplicity)
                    .with_protected(op.is_protected)
                    .with_condition(
                        op.condition
                            .map(|span| crate::conditions::when_interior(source, span).to_string()),
                    )
                    .with_throws(throws)
                    .with_capabilities(
                        capabilities
                            .iter()
                            .map(|name| phero_ir::ddd::CapabilityRef::new(name.clone()))
                            .collect(),
                    )
                    .with_query(query),
            );
            if let Some(condition) = op.condition {
                check_condition(
                    source,
                    condition,
                    ConditionCheck {
                        label: format!(
                            "operation '{}' of repository '{}'",
                            op.name.text, repository.name.text
                        ),
                        module: &module.name.text,
                        self_class,
                        scopes: machines,
                        injections: &[],
                        dependencies: &[],
                        union: None,
                        context,
                        params: &[],
                    },
                    local,
                );
            }
        }
    }
    operations
}
