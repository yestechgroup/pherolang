//! The `.dmn` semantic walker: decision models, inputs, knowledge, decisions,
//! requirements, services, and views.

use phero_syntax::ast::{DmnDecision, DmnDecisionModel, DmnFile, DmnLogic, DmnMetadataKind};

use crate::class::{HighlightClass, HighlightModifier, HighlightModifiers};
use crate::engine::semantic::Ctx;

/// Walks a parsed `.dmn` file.
pub(crate) fn walk(ctx: &mut Ctx, file: &DmnFile) {
    if let Some(model) = &file.model {
        walk_model(ctx, model);
    }
}

fn walk_model(ctx: &mut Ctx, model: &DmnDecisionModel) {
    ctx.decl_name(&model.name, HighlightClass::Type);
    if let Some(namespace) = &model.namespace {
        for segment in &namespace.segments {
            ctx.push_with(
                segment.span,
                HighlightClass::Namespace,
                HighlightModifiers::new().with(HighlightModifier::Declaration),
            );
        }
    }
    for metadata in &model.metadata {
        let word = match metadata.kind {
            DmnMetadataKind::Id => "id",
            DmnMetadataKind::Version => "version",
            DmnMetadataKind::Effective => "effective",
            DmnMetadataKind::Jurisdiction => "jurisdiction",
        };
        ctx.word_in(metadata.span, word, HighlightClass::Annotation);
    }
    for input in &model.inputs {
        ctx.push_with(
            input.name.span,
            HighlightClass::Property,
            HighlightModifiers::new().with(HighlightModifier::Declaration),
        );
        ctx.type_ref(&input.type_ref);
    }
    for source in &model.knowledge_sources {
        ctx.decl_name(&source.name, HighlightClass::Type);
    }
    for business_model in &model.models {
        ctx.decl_name(&business_model.name, HighlightClass::Function);
        for parameter in &business_model.parameters {
            ctx.push_with(
                parameter.name.span,
                HighlightClass::Property,
                HighlightModifiers::new().with(HighlightModifier::Declaration),
            );
            ctx.type_ref(&parameter.type_ref);
        }
        ctx.type_ref(&business_model.output);
        walk_logic(ctx, &business_model.logic);
    }
    for decision in &model.decisions {
        walk_decision(ctx, decision);
    }
    for requirement in model
        .information_requirements
        .iter()
        .chain(&model.knowledge_requirements)
        .chain(&model.authority_requirements)
    {
        ctx.push(requirement.requiring.span, HighlightClass::Type);
        ctx.push(requirement.required.span, HighlightClass::Type);
    }
    for service in &model.services {
        ctx.decl_name(&service.name, HighlightClass::Function);
        for input in service
            .inputs
            .iter()
            .chain(&service.outputs)
            .chain(&service.encapsulates)
        {
            ctx.push(input.span, HighlightClass::Property);
        }
    }
    for view in &model.views {
        ctx.decl_name(&view.name, HighlightClass::Type);
        for shown in &view.show {
            ctx.push(shown.span, HighlightClass::Function);
        }
        if let Some(direction) = &view.direction {
            ctx.push(direction.span, HighlightClass::Constant);
        }
    }
}

fn walk_decision(ctx: &mut Ctx, decision: &DmnDecision) {
    ctx.decl_name(&decision.name, HighlightClass::Function);
    for output in &decision.outputs {
        if !output.name.text.is_empty() {
            ctx.push_with(
                output.name.span,
                HighlightClass::Property,
                HighlightModifiers::new().with(HighlightModifier::Declaration),
            );
        }
        ctx.type_ref(&output.type_ref);
    }
    if let Some(logic) = &decision.logic {
        walk_logic(ctx, logic);
    }
    for emit in &decision.emits {
        ctx.push(emit.event.span, HighlightClass::Type);
    }
}

fn walk_logic(ctx: &mut Ctx, logic: &DmnLogic) {
    if let DmnLogic::Invoke {
        model, bindings, ..
    } = logic
    {
        ctx.push(model.span, HighlightClass::Function);
        for binding in bindings {
            ctx.push(binding.name.span, HighlightClass::Property);
        }
    }
}
