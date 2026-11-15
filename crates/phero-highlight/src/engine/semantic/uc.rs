//! The `.uc` semantic walker: use cases, participants, steps, messages,
//! successions, and `realizes` bindings.

use phero_syntax::ast::{UcDecl, UcEndpoint, UcFile, UcMessage, UcParticipant, UcStep};

use crate::class::{HighlightClass, HighlightModifier, HighlightModifiers};
use crate::engine::semantic::Ctx;

/// Walks a parsed `.uc` file.
pub(crate) fn walk(ctx: &mut Ctx, file: &UcFile) {
    for import in &file.imports {
        if let Some(alias) = &import.alias {
            ctx.decl_word_in(import.span, alias, HighlightClass::Namespace);
        }
    }
    for use_case in &file.use_cases {
        walk_use_case(ctx, use_case);
    }
}

fn walk_use_case(ctx: &mut Ctx, use_case: &UcDecl) {
    ctx.push_with(
        use_case.name_span,
        HighlightClass::Function,
        HighlightModifiers::new().with(HighlightModifier::Declaration),
    );
    if let Some(subject) = &use_case.subject {
        walk_participant(ctx, subject);
    }
    for actor in &use_case.actors {
        walk_participant(ctx, actor);
    }
    for message in &use_case.messages {
        walk_message(ctx, message);
    }
    for realizes in &use_case.realizes {
        ctx.qualified_text(realizes.reference_span, HighlightClass::Function);
    }
    // `UcSuccession` carries only its whole-statement span; the individual
    // node references stay lexical rather than inventing sub-spans.
}

fn walk_participant(ctx: &mut Ctx, participant: &UcParticipant) {
    ctx.push_with(
        participant.name_span,
        HighlightClass::Variable,
        HighlightModifiers::new().with(HighlightModifier::Declaration),
    );
    if let Some(type_span) = participant.type_span {
        ctx.qualified_text(type_span, HighlightClass::Type);
    }
    for step in &participant.steps {
        walk_step(ctx, step);
    }
    // `UcOccurrence` (`occurrence m.source;`) carries no per-name span.
}

fn walk_step(ctx: &mut Ctx, step: &UcStep) {
    ctx.push_with(
        step.name_span,
        HighlightClass::Function,
        HighlightModifiers::new().with(HighlightModifier::Declaration),
    );
    ctx.push(step.operation_span, HighlightClass::Function);
    for capability in &step.requires {
        ctx.push(capability.span, HighlightClass::Function);
    }
    if let Some(when) = step.when {
        ctx.expression(when);
    }
}

fn walk_message(ctx: &mut Ctx, message: &UcMessage) {
    if let Some(payload) = &message.payload {
        ctx.type_ref(payload);
    }
    if let Some(source) = &message.source {
        walk_endpoint(ctx, source);
    }
    if let Some(target) = &message.target {
        walk_endpoint(ctx, target);
    }
}

fn walk_endpoint(ctx: &mut Ctx, endpoint: &UcEndpoint) {
    ctx.qualified_text(endpoint.span, HighlightClass::Variable);
}
