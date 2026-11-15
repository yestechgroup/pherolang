//! The `.ifml` semantic walker: views, containers, components, fields,
//! columns, events, navigation, modules, and CLI commands.
//!
//! `.ifml` names are string literals in the source. Their AST spans either
//! cover the whole string token or its interior; [`Ctx::string_in`] expands
//! them to the enclosing token so a highlighted name always replaces an atomic
//! string rather than splitting one.

use phero_ifml::ast::{
    Body, BodyItem, ColumnDecl, ColumnKind, CommandMember, ComponentBody, ComponentMember,
    Declaration, EventAction, EventHandler, Expr, ExprKind, FieldDecl, IfmlFile, ModuleDecl,
    ModuleItem, ModuleUse, ParamDecl, Params, Property, PropertyRef, ValueExpr, ValueExprKind,
};

use crate::class::HighlightClass;
use crate::engine::semantic::Ctx;

/// Walks a parsed `.ifml` file.
pub(crate) fn walk(ctx: &mut Ctx, file: &IfmlFile) {
    for declaration in &file.declarations {
        walk_declaration(ctx, declaration);
    }
}

fn walk_declaration(ctx: &mut Ctx, declaration: &Declaration) {
    match declaration {
        Declaration::View(view) => {
            ctx.decl_string(view.name_span, HighlightClass::Type);
            walk_body(ctx, &view.body);
        }
        Declaration::Action(action) => {
            ctx.decl_string(action.name_span, HighlightClass::Function);
            for property in &action.properties {
                walk_property(ctx, property);
            }
            for event in &action.events {
                walk_event(ctx, event);
            }
        }
        Declaration::Module(module) => walk_module(ctx, module),
        Declaration::Actor(actor) => {
            ctx.decl_string(actor.name_span, HighlightClass::Type);
            for property in &actor.properties {
                walk_property(ctx, property);
            }
        }
        Declaration::Command(command) => {
            ctx.decl_string(command.name_span, HighlightClass::Function);
            walk_command_members(ctx, &command.members);
        }
    }
}

fn walk_module(ctx: &mut Ctx, module: &ModuleDecl) {
    ctx.decl_string(module.name_span, HighlightClass::Namespace);
    walk_params(ctx, &module.input);
    walk_params(ctx, &module.output);
    for property in &module.properties {
        walk_property(ctx, property);
    }
    for item in &module.items {
        match item {
            ModuleItem::Container(container) => {
                ctx.decl_string(container.name_span, HighlightClass::Type);
                walk_body(ctx, &container.body);
            }
            ModuleItem::Component(component) => {
                ctx.decl_string(component.name_span, HighlightClass::Type);
                walk_component_body(ctx, &component.body);
            }
            ModuleItem::Event(event) => walk_event(ctx, event),
            ModuleItem::Use(use_) => walk_use(ctx, use_),
        }
    }
}

fn walk_body(ctx: &mut Ctx, body: &Body) {
    if let Some(params) = &body.params {
        walk_params(ctx, params);
    }
    for property in &body.properties {
        walk_property(ctx, property);
    }
    for item in &body.items {
        match item {
            BodyItem::Container(container) => {
                ctx.decl_string(container.name_span, HighlightClass::Type);
                walk_body(ctx, &container.body);
            }
            BodyItem::Component(component) => {
                ctx.decl_string(component.name_span, HighlightClass::Type);
                walk_component_body(ctx, &component.body);
            }
            BodyItem::Event(event) => walk_event(ctx, event),
            BodyItem::Use(use_) => walk_use(ctx, use_),
            BodyItem::Condition(condition) => walk_expr(ctx, &condition.expr),
        }
    }
}

fn walk_params(ctx: &mut Ctx, params: &Params) {
    for parameter in &params.decls {
        walk_param(ctx, parameter);
    }
}

fn walk_param(ctx: &mut Ctx, parameter: &ParamDecl) {
    ctx.decl_word_in(parameter.span, &parameter.name, HighlightClass::Parameter);
    ctx.word_in(parameter.span, &parameter.type_ref, HighlightClass::Type);
}

fn walk_property(ctx: &mut Ctx, property: &Property) {
    ctx.word_in(property.span, &property.key, HighlightClass::Property);
    walk_value(ctx, &property.value);
}

fn walk_component_body(ctx: &mut Ctx, body: &ComponentBody) {
    for property in &body.properties {
        walk_property(ctx, property);
    }
    for member in &body.members {
        match member {
            ComponentMember::Column(column) => walk_column(ctx, column),
            ComponentMember::Field(field) => walk_field(ctx, field),
            ComponentMember::Chart(chart) => {
                ctx.word_in(chart.span, &chart.kind, HighlightClass::Constant);
                for property in &chart.properties {
                    walk_property(ctx, property);
                }
            }
            ComponentMember::Condition(condition) => walk_expr(ctx, &condition.expr),
            ComponentMember::Event(event) => walk_event(ctx, event),
        }
    }
}

fn walk_column(ctx: &mut Ctx, column: &ColumnDecl) {
    match &column.kind {
        ColumnKind::Lookup { field, lookup } => {
            walk_property_ref(ctx, column.span, field);
            ctx.last_word_in(column.span, lookup, HighlightClass::Function);
        }
        ColumnKind::Field { field } => walk_property_ref(ctx, column.span, field),
        ColumnKind::Expression { expr } => walk_expr(ctx, expr),
    }
}

fn walk_property_ref(ctx: &mut Ctx, within: phero_syntax::ast::Span, reference: &PropertyRef) {
    ctx.word_in(within, &reference.entity, HighlightClass::Type);
    ctx.word_in(within, &reference.property, HighlightClass::Property);
}

fn walk_field(ctx: &mut Ctx, field: &FieldDecl) {
    ctx.decl_word_in(field.span, &field.name, HighlightClass::Property);
    ctx.word_in(field.span, &field.input, HighlightClass::Type);
    for property in &field.properties {
        walk_property(ctx, property);
    }
}

fn walk_event(ctx: &mut Ctx, event: &EventHandler) {
    ctx.push(event.type_span, HighlightClass::Type);
    for parameter in &event.params {
        ctx.word_in(event.span, parameter, HighlightClass::Parameter);
    }
    for capability in &event.requires {
        ctx.push(capability.span, HighlightClass::Function);
    }
    if let Some(condition) = &event.condition {
        walk_expr(ctx, condition);
    }
    match &event.action {
        EventAction::Navigate {
            target_span,
            binding,
            ..
        }
        | EventAction::Refresh {
            target_span,
            binding,
            ..
        } => {
            ctx.string_in(*target_span, HighlightClass::Type);
            if let Some(binding) = binding {
                for (key, value) in &binding.pairs {
                    ctx.word_in(binding.span, key, HighlightClass::Property);
                    walk_expr(ctx, value);
                }
            }
        }
        EventAction::ActionInvocation {
            name_span, body, ..
        } => {
            ctx.string_in(*name_span, HighlightClass::Function);
            if let Some(body) = body {
                for property in &body.properties {
                    walk_property(ctx, property);
                }
                for event in &body.events {
                    walk_event(ctx, event);
                }
            }
        }
        EventAction::Stay => {}
    }
}

fn walk_use(ctx: &mut Ctx, use_: &ModuleUse) {
    ctx.string_in(use_.target_span, HighlightClass::Namespace);
    if let Some(alias) = &use_.alias {
        ctx.word_in(use_.span, alias, HighlightClass::Namespace);
    }
    for property in &use_.properties {
        walk_property(ctx, property);
    }
}

fn walk_command_members(ctx: &mut Ctx, members: &[CommandMember]) {
    for member in members {
        match member {
            CommandMember::Property(property) => walk_property(ctx, property),
            CommandMember::Argument(argument) => {
                ctx.decl_word_in(argument.span, &argument.name, HighlightClass::Parameter);
                ctx.word_in(argument.span, &argument.type_ref, HighlightClass::Type);
            }
            CommandMember::Option(option) => {
                ctx.decl_word_in(option.span, &option.name, HighlightClass::Parameter);
                ctx.word_in(option.span, &option.type_ref, HighlightClass::Type);
            }
            CommandMember::Prompt(prompt) => {
                ctx.decl_word_in(prompt.span, &prompt.name, HighlightClass::Parameter);
                ctx.word_in(prompt.span, &prompt.kind, HighlightClass::Constant);
            }
            CommandMember::Event(event) => walk_event(ctx, event),
            CommandMember::Use(use_) => walk_use(ctx, use_),
            CommandMember::Condition(condition) => walk_expr(ctx, &condition.expr),
            CommandMember::Command(command) => {
                ctx.decl_string(command.name_span, HighlightClass::Function);
                walk_command_members(ctx, &command.members);
            }
        }
    }
}

fn walk_value(ctx: &mut Ctx, value: &ValueExpr) {
    match &value.kind {
        ValueExprKind::Ident(_) => ctx.push(value.span, HighlightClass::Variable),
        ValueExprKind::String(_) | ValueExprKind::Number(_) | ValueExprKind::Bool(_) => {}
        ValueExprKind::Call {
            name_span, args, ..
        } => {
            ctx.push(*name_span, HighlightClass::Function);
            for argument in args {
                walk_expr(ctx, argument);
            }
        }
        ValueExprKind::Field {
            object, field_span, ..
        } => {
            walk_value(ctx, object);
            ctx.push(*field_span, HighlightClass::Property);
        }
        ValueExprKind::Group(inner) => walk_value(ctx, inner),
        ValueExprKind::Unary { operand, .. } => walk_value(ctx, operand),
        ValueExprKind::Binary { left, right, .. } => {
            walk_value(ctx, left);
            walk_value(ctx, right);
        }
        ValueExprKind::Array(items) => {
            for item in items {
                walk_value(ctx, item);
            }
        }
        ValueExprKind::Object(members) => {
            for member in members {
                ctx.word_in(member.span, &member.key, HighlightClass::Property);
                walk_value(ctx, &member.value);
            }
        }
    }
}

fn walk_expr(ctx: &mut Ctx, expr: &Expr) {
    match &expr.kind {
        ExprKind::Ident(_) => ctx.push(expr.span, HighlightClass::Variable),
        ExprKind::StringLit(_) | ExprKind::NumLit(_) | ExprKind::BoolLit(_) => {}
        ExprKind::Field {
            object, field_span, ..
        } => {
            walk_expr(ctx, object);
            ctx.push(*field_span, HighlightClass::Property);
        }
        ExprKind::Binary { left, right, .. } => {
            walk_expr(ctx, left);
            walk_expr(ctx, right);
        }
        ExprKind::Unary { operand, .. } => walk_expr(ctx, operand),
        ExprKind::Group(inner) => walk_expr(ctx, inner),
        ExprKind::Call {
            name_span, args, ..
        } => {
            ctx.push(*name_span, HighlightClass::Function);
            for argument in args {
                walk_expr(ctx, argument);
            }
        }
    }
}
