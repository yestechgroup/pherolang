//! The `.ddd` semantic walker: applications, modules, services, designs,
//! repositories, machines, searches, and executable statement bodies.

use phero_syntax::ast::{
    DddApplication, DddBody, DddChildOp, DddDecisionRef, DddDesign, DddErrorDecl, DddFile,
    DddModule, DddProjectionMember, DddRepositoryOp, DddSearch, DddSearchField, DddService,
    DddServiceOp, DddStmt,
};

use crate::class::{HighlightClass, HighlightModifier, HighlightModifiers};
use crate::engine::semantic::Ctx;

/// Walks a parsed `.ddd` file.
pub(crate) fn walk(ctx: &mut Ctx, file: &DddFile) {
    if let Some(application) = &file.application {
        walk_application(ctx, application);
    }
}

fn walk_application(ctx: &mut Ctx, application: &DddApplication) {
    ctx.decl_name(&application.name, HighlightClass::Namespace);
    if let Some(base) = &application.base {
        ctx.qualified(&base.package, HighlightClass::Namespace);
    }
    for error in &application.errors {
        walk_error_decl(ctx, error);
    }
    for module in &application.modules {
        walk_module(ctx, module);
    }
}

/// One `error <Name>(<fields>);` declaration (DD7): the name is a declared
/// type, the payload fields are parameters (`walk_param` gives them the
/// declaration modifier), and their types are type references.
fn walk_error_decl(ctx: &mut Ctx, error: &DddErrorDecl) {
    ctx.decl_name(&error.name, HighlightClass::Type);
    for field in &error.fields {
        walk_param(ctx, field);
    }
}

fn walk_module(ctx: &mut Ctx, module: &DddModule) {
    ctx.decl_name(&module.name, HighlightClass::Namespace);
    for error in &module.errors {
        walk_error_decl(ctx, error);
    }
    for service in &module.services {
        walk_service(ctx, service);
    }
    for design in &module.designs {
        walk_design(ctx, design);
    }
    for search in &module.searches {
        walk_search(ctx, search);
    }
    for extension in &module.extends {
        ctx.push(extension.class.span, HighlightClass::Type);
        for operation in &extension.operations {
            walk_child_op(ctx, operation);
        }
    }
}

fn walk_service(ctx: &mut Ctx, service: &DddService) {
    ctx.decl_name(&service.name, HighlightClass::Type);
    for dependency in &service.dependencies {
        ctx.push(dependency.span, HighlightClass::Type);
    }
    for decision in &service.decision_dependencies {
        walk_decision_ref(ctx, decision);
    }
    for operation in &service.operations {
        walk_service_op(ctx, operation);
    }
}

fn walk_decision_ref(ctx: &mut Ctx, decision: &DddDecisionRef) {
    ctx.qualified(&decision.name, HighlightClass::Function);
}

fn walk_service_op(ctx: &mut Ctx, operation: &DddServiceOp) {
    if let Some(return_type) = &operation.return_type {
        ctx.type_ref(return_type);
    }
    ctx.decl_name(&operation.name, HighlightClass::Function);
    for param in &operation.params {
        walk_param(ctx, param);
    }
    if let Some(delegation) = &operation.delegation {
        ctx.qualified(&delegation.target, HighlightClass::Type);
        ctx.push(delegation.operation.span, HighlightClass::Function);
    }
    for capability in &operation.capabilities {
        ctx.push(capability.span, HighlightClass::Function);
    }
    for decision in &operation.invokes {
        walk_decision_ref(ctx, decision);
    }
    for event in &operation.emits {
        ctx.push(event.span, HighlightClass::Type);
    }
    for error in &operation.throws {
        ctx.push(error.span, HighlightClass::Type);
    }
    if let Some(transition) = &operation.transition {
        if let Some(machine) = &transition.machine {
            ctx.push(machine.span, HighlightClass::Type);
        }
        ctx.push(transition.trigger.span, HighlightClass::Constant);
    }
    if let Some(condition) = operation.condition {
        ctx.expression(condition);
    }
    if let Some(receiver) = &operation.body_receiver {
        ctx.push(receiver.span, HighlightClass::Type);
    }
    if let Some(body) = &operation.body {
        walk_body(ctx, body);
    }
}

fn walk_param(ctx: &mut Ctx, param: &phero_syntax::ast::DddParam) {
    ctx.type_ref(&param.type_ref);
    ctx.push_with(
        param.name.span,
        HighlightClass::Parameter,
        HighlightModifiers::new().with(HighlightModifier::Declaration),
    );
}

fn walk_child_op(ctx: &mut Ctx, operation: &DddChildOp) {
    ctx.type_ref(&operation.return_type);
    ctx.decl_name(&operation.name, HighlightClass::Function);
    for param in &operation.params {
        walk_param(ctx, param);
    }
    ctx.push(operation.feature.span, HighlightClass::Property);
    ctx.push(operation.action.span, HighlightClass::Function);
    if let Some(condition) = operation.condition {
        ctx.expression(condition);
    }
}

fn walk_design(ctx: &mut Ctx, design: &DddDesign) {
    ctx.decl_name(&design.class, HighlightClass::Type);
    if let Some(source) = &design.dto_source {
        ctx.qualified(source, HighlightClass::Type);
    }
    if let Some(members) = &design.dto_members {
        match members {
            phero_syntax::ast::DddDtoMembers::Projection(members) => {
                for member in members {
                    match member {
                        DddProjectionMember::Select { name, .. } => {
                            ctx.push(name.span, HighlightClass::Property);
                        }
                        DddProjectionMember::Assign { name, expr, .. } => {
                            ctx.push(name.span, HighlightClass::Property);
                            ctx.expression(*expr);
                        }
                    }
                }
            }
            phero_syntax::ast::DddDtoMembers::Declared(members) => {
                for member in members {
                    ctx.type_ref(&member.type_ref);
                    ctx.push(member.name.span, HighlightClass::Property);
                }
            }
        }
    }
    let flags = &design.flags;
    for flag in [
        flags.scaffold,
        flags.auditable,
        flags.optimistic_locking,
        flags.non_persistent,
        flags.cache,
    ]
    .into_iter()
    .flatten()
    {
        ctx.push(flag.span, HighlightClass::Annotation);
    }
    if let Some(machine) = &design.machine {
        ctx.push(machine.field.span, HighlightClass::Property);
        ctx.type_ref(&machine.type_);
        if let Some(initial) = &machine.initial {
            ctx.push(initial.span, HighlightClass::Constant);
        }
        for terminal in &machine.terminals {
            ctx.push(terminal.span, HighlightClass::Constant);
        }
        for transition in &machine.transitions {
            ctx.push(transition.from.span, HighlightClass::Constant);
            ctx.push(transition.to.span, HighlightClass::Constant);
            if let Some(trigger) = &transition.trigger {
                ctx.push(trigger.span, HighlightClass::Constant);
            }
            if let Some(guard) = transition.guard {
                ctx.expression(guard);
            }
        }
    }
    if let Some(repository) = &design.repository {
        ctx.decl_name(&repository.name, HighlightClass::Type);
        for operation in &repository.operations {
            walk_repository_op(ctx, operation);
        }
    }
}

fn walk_repository_op(ctx: &mut Ctx, operation: &DddRepositoryOp) {
    if let Some(return_type) = &operation.return_type {
        ctx.type_ref(return_type);
    }
    ctx.decl_name(&operation.name, HighlightClass::Function);
    for param in &operation.params {
        walk_param(ctx, param);
    }
    for error in &operation.throws {
        ctx.push(error.span, HighlightClass::Type);
    }
    if let Some(condition) = operation.condition {
        ctx.expression(condition);
    }
    if let Some(body) = &operation.body {
        walk_body(ctx, body);
    }
}

fn walk_search(ctx: &mut Ctx, search: &DddSearch) {
    ctx.decl_name(&search.name, HighlightClass::Type);
    if let Some(entity) = &search.entity {
        ctx.qualified(entity, HighlightClass::Type);
    }
    for field in &search.text {
        walk_search_field(ctx, field);
    }
    for filter in &search.filters {
        ctx.member_path(filter, HighlightClass::Property);
    }
    for sort in &search.sort {
        ctx.member_path(sort, HighlightClass::Property);
    }
    for entry in &search.document {
        match entry {
            DddProjectionMember::Select { name, .. } => {
                ctx.push(name.span, HighlightClass::Property);
            }
            DddProjectionMember::Assign { name, expr, .. } => {
                ctx.push(name.span, HighlightClass::Property);
                ctx.expression(*expr);
            }
        }
    }
    if let Some(pagination) = &search.pagination {
        ctx.push(pagination.span, HighlightClass::Property);
    }
    for capability in &search.capabilities {
        ctx.push(capability.span, HighlightClass::Function);
    }
}

fn walk_search_field(ctx: &mut Ctx, field: &DddSearchField) {
    ctx.member_path(&field.property, HighlightClass::Property);
}

fn walk_body(ctx: &mut Ctx, body: &DddBody) {
    for statement in &body.stmts {
        walk_statement(ctx, statement);
    }
}

fn walk_statement(ctx: &mut Ctx, statement: &DddStmt) {
    match statement {
        DddStmt::Let { name, expr, raise } => {
            ctx.push_with(
                name.span,
                HighlightClass::Variable,
                HighlightModifiers::new().with(HighlightModifier::Declaration),
            );
            ctx.expression(*expr);
            if let Some(raise) = raise {
                ctx.push(raise.variant.span, HighlightClass::Constant);
            }
        }
        DddStmt::Assign { path, expr } => {
            let count = path.len();
            for (index, segment) in path.iter().enumerate() {
                let class = if index + 1 == count {
                    HighlightClass::Property
                } else {
                    HighlightClass::Variable
                };
                ctx.push(segment.span, class);
            }
            ctx.expression(*expr);
        }
        DddStmt::Guard { expr, raise } => {
            ctx.expression(*expr);
            ctx.push(raise.variant.span, HighlightClass::Constant);
        }
        DddStmt::If { cond, then, else_ } => {
            ctx.expression(*cond);
            for statement in then {
                walk_statement(ctx, statement);
            }
            for statement in else_ {
                walk_statement(ctx, statement);
            }
        }
        DddStmt::Return { expr } => {
            if let Some(expr) = expr {
                ctx.expression(*expr);
            }
        }
    }
}
