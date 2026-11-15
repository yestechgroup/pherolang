//! The `.mox` semantic walker: declarations, features, operations, actors.

use phero_syntax::ast::{
    ActorsDecl, AnnotationDecl, ClassDecl, DatatypeDecl, Decl, DefaultValue, EnumDecl, FeatureDecl,
    GrantEntryDecl, InterfaceDecl, Model, Param, RegisterDecl, VocabularyDecl, Wraps,
};

use crate::class::{HighlightClass, HighlightModifier, HighlightModifiers};
use crate::engine::semantic::Ctx;

/// Walks a parsed `.mox` model.
pub(crate) fn walk(ctx: &mut Ctx, model: &Model) {
    if let Some(package) = &model.package {
        for segment in &package.name.segments {
            ctx.push_with(
                segment.span,
                HighlightClass::Namespace,
                HighlightModifiers::new().with(HighlightModifier::Declaration),
            );
        }
    }
    for declaration in &model.declarations {
        walk_decl(ctx, declaration);
    }
}

fn walk_decl(ctx: &mut Ctx, declaration: &Decl) {
    match declaration {
        Decl::Class(class) => walk_class(ctx, class),
        Decl::Register(register) => walk_register(ctx, register),
        Decl::Interface(interface) => walk_interface(ctx, interface),
        Decl::Enum(enumeration) => walk_enum(ctx, enumeration),
        Decl::Datatype(datatype) => walk_datatype(ctx, datatype),
        Decl::Annotation(annotation) => walk_annotation(ctx, annotation),
        Decl::Vocabulary(vocabulary) => walk_vocabulary(ctx, vocabulary),
        Decl::Actors(actors) => walk_actors(ctx, actors),
        Decl::ImportSchema(import) => {
            if let Some(alias) = &import.alias {
                ctx.decl_name(alias, HighlightClass::Namespace);
            }
        }
    }
}

fn walk_class(ctx: &mut Ctx, class: &ClassDecl) {
    ctx.decl_name(&class.name, HighlightClass::Type);
    for parent in &class.extends {
        ctx.type_ref(parent);
    }
    for feature in &class.features {
        walk_feature(ctx, feature);
    }
}

fn walk_register(ctx: &mut Ctx, register: &RegisterDecl) {
    ctx.decl_name(&register.name, HighlightClass::Type);
    for attribute in &register.attributes {
        walk_feature(ctx, attribute);
    }
}

fn walk_interface(ctx: &mut Ctx, interface: &InterfaceDecl) {
    ctx.decl_name(&interface.name, HighlightClass::Type);
    for binding in &interface.bindings {
        ctx.push(binding.key.span, HighlightClass::Annotation);
    }
}

fn walk_enum(ctx: &mut Ctx, enumeration: &EnumDecl) {
    ctx.decl_name(&enumeration.name, HighlightClass::Type);
    for literal in &enumeration.literals {
        ctx.decl_name(&literal.name, HighlightClass::Constant);
    }
}

fn walk_datatype(ctx: &mut Ctx, datatype: &DatatypeDecl) {
    ctx.decl_name(&datatype.name, HighlightClass::Type);
    if let Some(Wraps::Named(name)) = &datatype.wraps {
        ctx.qualified(name, HighlightClass::Type);
    }
    for binding in &datatype.bindings {
        ctx.push(binding.key.span, HighlightClass::Annotation);
    }
    for body in datatype.create.iter().chain(&datatype.convert) {
        ctx.push(body.target.span, HighlightClass::Annotation);
    }
}

fn walk_annotation(ctx: &mut Ctx, annotation: &AnnotationDecl) {
    if let Some(name) = &annotation.name {
        ctx.decl_name(name, HighlightClass::Annotation);
    }
}

fn walk_vocabulary(ctx: &mut Ctx, vocabulary: &VocabularyDecl) {
    ctx.decl_name(&vocabulary.name, HighlightClass::Type);
    if let Some(key) = &vocabulary.key {
        ctx.push(key.span, HighlightClass::Property);
    }
    for facet in &vocabulary.facets {
        ctx.type_ref(&facet.type_ref);
        ctx.push(facet.name.span, HighlightClass::Property);
    }
}

/// Walks one feature: its modifiers, type, name, and nested declarations.
pub(crate) fn walk_feature(ctx: &mut Ctx, feature: &FeatureDecl) {
    let modifiers = feature.modifiers();
    if let Some(id) = modifiers.id {
        ctx.push(id, HighlightClass::Keyword);
    }
    if let Some(read_only) = modifiers.read_only {
        ctx.push(read_only, HighlightClass::Keyword);
    }
    let read_only = modifiers.read_only.is_some();
    let mut name_modifiers = HighlightModifiers::new().with(HighlightModifier::Declaration);
    if read_only {
        name_modifiers.insert(HighlightModifier::Readonly);
    }

    match feature {
        FeatureDecl::Attribute {
            type_ref,
            name,
            default,
            constraints,
            ..
        } => {
            ctx.type_ref(type_ref);
            ctx.push_with(name.span, HighlightClass::Property, name_modifiers);
            if let Some(default) = default {
                match default {
                    DefaultValue::Str { .. }
                    | DefaultValue::Int { .. }
                    | DefaultValue::Bool { .. } => {}
                    DefaultValue::Name(name) => ctx.reference(name),
                }
            }
            for constraint in constraints {
                ctx.push(constraint.name.span, HighlightClass::Annotation);
            }
        }
        FeatureDecl::Containment {
            type_ref,
            name,
            opposite,
            ..
        }
        | FeatureDecl::Reference {
            type_ref,
            name,
            opposite,
            ..
        } => {
            ctx.type_ref(type_ref);
            ctx.push_with(name.span, HighlightClass::Property, name_modifiers);
            if let Some(opposite) = opposite {
                ctx.push(opposite.span, HighlightClass::Property);
            }
        }
        FeatureDecl::Container {
            type_ref,
            name,
            opposite,
            ..
        } => {
            ctx.type_ref(type_ref);
            ctx.push_with(name.span, HighlightClass::Property, name_modifiers);
            if let Some(opposite) = opposite {
                ctx.push(opposite.span, HighlightClass::Property);
            }
        }
        FeatureDecl::Op {
            return_type,
            name,
            params,
            bodies,
            ..
        } => {
            ctx.type_ref(return_type);
            ctx.push_with(name.span, HighlightClass::Function, name_modifiers);
            for param in params {
                walk_param(ctx, param);
            }
            for body in bodies {
                ctx.push(body.target.span, HighlightClass::Annotation);
            }
        }
        FeatureDecl::Derived {
            type_ref,
            name,
            bodies,
            ..
        } => {
            ctx.type_ref(type_ref);
            ctx.push_with(name.span, HighlightClass::Property, name_modifiers);
            for body in bodies {
                ctx.push(body.target.span, HighlightClass::Annotation);
            }
        }
    }
}

/// Walks one operation parameter.
pub(crate) fn walk_param(ctx: &mut Ctx, param: &Param) {
    ctx.type_ref(&param.type_ref);
    ctx.push_with(
        param.name.span,
        HighlightClass::Parameter,
        HighlightModifiers::new().with(HighlightModifier::Declaration),
    );
}

/// Walks an `actors` block (standalone `.actor` files and inline `.mox`).
pub(crate) fn walk_actors(ctx: &mut Ctx, actors: &ActorsDecl) {
    ctx.decl_name(&actors.name, HighlightClass::Namespace);
    for actor in &actors.actors {
        ctx.decl_name(&actor.name, HighlightClass::Type);
        if let Some(parent) = &actor.extends {
            ctx.push(parent.span, HighlightClass::Type);
        }
    }
    for capability in &actors.capabilities {
        ctx.decl_name(&capability.name, HighlightClass::Function);
        ctx.type_ref(&capability.class);
    }
    for purpose in &actors.purposes {
        ctx.decl_name(&purpose.name, HighlightClass::Constant);
    }
    for obligation in &actors.obligations {
        ctx.decl_name(&obligation.name, HighlightClass::Function);
    }
    for grant in &actors.grants {
        ctx.push(grant.actor.span, HighlightClass::Type);
        for entry in &grant.entries {
            walk_grant_entry(ctx, entry);
        }
    }
    for delegation in &actors.delegations {
        ctx.decl_name(&delegation.name, HighlightClass::Function);
        ctx.push(delegation.from.span, HighlightClass::Type);
        ctx.push(delegation.to.span, HighlightClass::Type);
        if let Some(purpose) = &delegation.purpose {
            ctx.push(purpose.span, HighlightClass::Constant);
        }
        for entry in &delegation.entries {
            ctx.push(entry.capability.span, HighlightClass::Function);
            for obligation in &entry.obligations {
                ctx.push(obligation.span, HighlightClass::Function);
            }
        }
    }
    for never_both in &actors.never_both {
        for capability in &never_both.capabilities {
            ctx.push(capability.span, HighlightClass::Function);
        }
    }
}

fn walk_grant_entry(ctx: &mut Ctx, entry: &GrantEntryDecl) {
    match entry {
        GrantEntryDecl::Effect(effect) => {
            ctx.push(effect.capability.span, HighlightClass::Function);
            for obligation in &effect.obligations {
                ctx.push(obligation.span, HighlightClass::Function);
            }
            if let Some(when) = effect.when {
                ctx.expression(when);
            }
        }
        GrantEntryDecl::Cedar(body) => {
            ctx.push(body.target.span, HighlightClass::Annotation);
        }
    }
}
