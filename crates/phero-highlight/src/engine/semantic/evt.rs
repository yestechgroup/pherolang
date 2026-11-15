//! The `.evt` semantic walker: events, channels, subscriptions, delivery.

use phero_syntax::ast::EvtFile;

use crate::class::HighlightClass;
use crate::engine::semantic::Ctx;

/// Walks a parsed `.evt` file.
pub(crate) fn walk(ctx: &mut Ctx, file: &EvtFile) {
    for event in &file.events {
        ctx.decl_name(&event.name, HighlightClass::Type);
        for field in &event.fields {
            ctx.push_with(
                field.name.span,
                HighlightClass::Property,
                crate::class::HighlightModifiers::new()
                    .with(crate::class::HighlightModifier::Declaration),
            );
            ctx.type_ref(&field.ty);
        }
        if let Some(owner) = &event.owner {
            ctx.push(owner.span, HighlightClass::Variable);
        }
        if let Some(classification) = &event.classification {
            ctx.push(classification.span, HighlightClass::Constant);
        }
        for field in &event.pii {
            ctx.push(field.span, HighlightClass::Property);
        }
    }
    for channel in &file.channels {
        ctx.decl_name(&channel.name, HighlightClass::Type);
        if let Some(scope) = &channel.scope {
            ctx.push(scope.span, HighlightClass::Variable);
        }
        for publishes in &channel.publishes {
            ctx.push(publishes.event.span, HighlightClass::Type);
        }
    }
    for subscription in &file.subscriptions {
        ctx.decl_name(&subscription.name, HighlightClass::Function);
        for event in &subscription.events {
            ctx.push(event.span, HighlightClass::Type);
        }
        ctx.push(subscription.consumer.span, HighlightClass::Variable);
        if let Some(scope) = &subscription.scope {
            ctx.push(scope.span, HighlightClass::Variable);
        }
        if let Some(when) = subscription.when {
            ctx.expression(when);
        }
        if let Some(delivery) = &subscription.delivery {
            if let Some(mode) = &delivery.mode {
                ctx.push(mode.span, HighlightClass::Constant);
            }
            if let Some(dead_letter) = &delivery.dead_letter {
                ctx.push(dead_letter.span, HighlightClass::Type);
            }
        }
    }
}
