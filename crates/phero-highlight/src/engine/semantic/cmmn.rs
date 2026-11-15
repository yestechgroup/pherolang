//! The `.cmmn` semantic walker: cases, roles, case-file items, plan items,
//! sentries, and decision-task invocations.

use phero_syntax::ast::{CmmnCase, CmmnFile, CmmnFileItem, CmmnItemDecl, CmmnItemKind, CmmnRole};

use crate::class::{HighlightClass, HighlightModifier, HighlightModifiers};
use crate::engine::semantic::Ctx;

/// Walks a parsed `.cmmn` file.
pub(crate) fn walk(ctx: &mut Ctx, file: &CmmnFile) {
    for import in &file.imports {
        if let Some(alias) = &import.alias {
            ctx.decl_word_in(import.span, alias, HighlightClass::Namespace);
        }
    }
    for case in &file.cases {
        walk_case(ctx, case);
    }
}

fn walk_case(ctx: &mut Ctx, case: &CmmnCase) {
    ctx.push_with(
        case.name_span,
        HighlightClass::Namespace,
        HighlightModifiers::new().with(HighlightModifier::Declaration),
    );
    for role in &case.roles {
        walk_role(ctx, role);
    }
    for item in &case.case_file {
        walk_file_item(ctx, item);
    }
    for item in &case.plan_items {
        walk_plan_item(ctx, item);
    }
}

fn walk_role(ctx: &mut Ctx, role: &CmmnRole) {
    ctx.push_with(
        role.name_span,
        HighlightClass::Type,
        HighlightModifiers::new().with(HighlightModifier::Declaration),
    );
    if let Some(actor_span) = role.actor_span {
        ctx.push(actor_span, HighlightClass::Type);
    }
}

fn walk_file_item(ctx: &mut Ctx, item: &CmmnFileItem) {
    ctx.type_ref(&item.type_ref);
    ctx.push_with(
        item.name_span,
        HighlightClass::Property,
        HighlightModifiers::new().with(HighlightModifier::Declaration),
    );
}

fn walk_plan_item(ctx: &mut Ctx, item: &CmmnItemDecl) {
    ctx.push_with(
        item.name_span,
        HighlightClass::Function,
        HighlightModifiers::new().with(HighlightModifier::Declaration),
    );
    if let Some(performer_span) = item.performer_span {
        ctx.push(performer_span, HighlightClass::Type);
    }
    if let Some(execute) = &item.execute {
        ctx.qualified_text(execute.reference_span, HighlightClass::Function);
    }
    if let Some(invokes) = &item.invokes {
        ctx.qualified_text(invokes.name_span, HighlightClass::Function);
    }
    for input in &item.inputs {
        ctx.push(input.span, HighlightClass::Property);
    }
    for output in &item.outputs {
        ctx.push(output.span, HighlightClass::Property);
    }
    for criterion in item.entry.iter().chain(&item.exit) {
        for part in &criterion.on_parts {
            ctx.push(part.source_span, HighlightClass::Type);
        }
        if let Some(when) = criterion.when {
            ctx.expression(when);
        }
    }
    if let CmmnItemKind::Stage { items } = &item.kind {
        for nested in items {
            walk_plan_item(ctx, nested);
        }
    }
}
