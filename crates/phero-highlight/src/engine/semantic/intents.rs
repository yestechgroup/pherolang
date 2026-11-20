//! The `.intent` semantic walker: intent declarations and their links.

use phero_syntax::ast::{IntentFile, IntentLinkSlot};

use crate::class::{HighlightClass, HighlightModifier, HighlightModifiers};
use crate::engine::semantic::Ctx;

/// Walks a parsed `.intent` file: the imports' aliases (never written on
/// this surface, but the shared shape keeps the walker total), each
/// intent's stable id (the declaration), and each link's kind word and
/// dotted name.
pub(crate) fn walk(ctx: &mut Ctx, file: &IntentFile) {
    for import in &file.imports {
        if let Some(alias) = &import.alias {
            ctx.decl_word_in(import.span, alias, HighlightClass::Namespace);
        }
    }
    for intent in &file.intents {
        walk_intent(ctx, intent);
    }
}

fn walk_intent(ctx: &mut Ctx, intent: &phero_syntax::ast::IntentDecl) {
    ctx.push_with(
        intent.id.span,
        HighlightClass::Function,
        HighlightModifiers::new().with(HighlightModifier::Declaration),
    );
    for link in &intent.links {
        let class = match link.slot {
            IntentLinkSlot::Element => HighlightClass::Variable,
            IntentLinkSlot::Check => HighlightClass::Property,
        };
        ctx.push(link.kind.span, class);
        ctx.qualified(
            &phero_syntax::QualifiedName {
                segments: link_name_segments(link),
                span: link.name_span,
            },
            HighlightClass::Function,
        );
    }
}

/// The link's dotted name, re-segmented for the qualified walker: the
/// authored spelling splits at the dots it carries.
fn link_name_segments(link: &phero_syntax::ast::IntentLink) -> Vec<phero_syntax::Name> {
    link.name
        .split('.')
        .map(|segment| phero_syntax::Name {
            text: segment.to_string(),
            span: link.name_span,
            escaped: false,
        })
        .collect()
}
