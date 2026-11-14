//! AST → `(IfmlModel, IfmlIndex)` lowering.
//!
//! One traversal builds the wire model and the span index together, so the
//! two can never disagree about what was written. The behaviour is moved from
//! the Pest walkers rather than redesigned: property-bag extraction, component
//! type promotion, chart validation, module/container/view body assembly, and
//! the index's site collection are all the same rules.

use rex_ir::ifml as ir;
use rex_ir::ifml::Float;
use rex_parser::Span;

use crate::ast;
use crate::ast::{Body, BodyItem, ComponentMember, Declaration, DefaultValue, IfmlFile, Property};
use crate::index::{
    AuthKind, AuthSite, DataSite, IfmlIndex, IndexParam, LiteralKind, ModuleDeclSite,
    ModuleUseSite, NamedSite, TargetSite, UseOverride,
};

/// A lowering-time authoring error (the chart validations the PEG grammar
/// could not express). The strict entries surface it as an
/// [`IfmlParseError`]; phase 2 turns it into a diagnostic on the lenient path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LowerError {
    /// The construct-specific message.
    pub message: String,
    /// The offending source span.
    pub span: Span,
}

impl LowerError {
    fn new(span: Span, message: String) -> Self {
        Self { message, span }
    }
}

fn pair(span: Span) -> (usize, usize) {
    (span.start, span.end)
}

/// Lower a parsed file, recording it in the index under `path`.
pub(crate) fn lower(
    file: &IfmlFile,
    path: &str,
    text: &str,
) -> (ir::IfmlModel, IfmlIndex, Vec<LowerError>) {
    let mut errors = Vec::new();
    let mut index = IfmlIndex::single_file(path, text);
    let mut views = Vec::new();
    let mut actions = Vec::new();
    let mut modules = Vec::new();
    let mut actors = Vec::new();
    let mut commands = Vec::new();

    let domains = file
        .domains
        .iter()
        .map(|domain| ir::DomainDeclaration {
            name: domain.name.clone(),
            schema_name: domain.schema.clone(),
        })
        .collect();

    for declaration in &file.declarations {
        match declaration {
            Declaration::View(view) => {
                index
                    .views
                    .push(named_site(&view.name, view.name_span, view.span));
                views.push(lower_view(view, text, &mut index, &mut errors));
            }
            Declaration::Action(action) => {
                index
                    .actions
                    .push(named_site(&action.name, action.name_span, action.span));
                actions.push(ir::ActionDeclaration {
                    name: action.name.clone(),
                    properties: lower_properties(&action.properties),
                    events: lower_events(&action.events, &mut index),
                });
            }
            Declaration::Module(module) => {
                index.module_decls.push(module_site(module));
                modules.push(lower_module(module, text, &mut index, &mut errors));
            }
            Declaration::Actor(actor) => {
                index
                    .actors
                    .push(named_site(&actor.name, actor.name_span, actor.span));
                actors.push(ir::ActorDeclaration {
                    name: actor.name.clone(),
                    properties: lower_properties(&actor.properties),
                });
            }
            Declaration::Command(command) => {
                commands.push(lower_command(command, &mut index));
            }
        }
    }

    let imports = file
        .imports
        .iter()
        .map(|import| import.path.clone())
        .collect();
    (
        ir::IfmlModel::new(domains, views, actions, modules, actors, commands, imports),
        index,
        errors,
    )
}

// ── Views / containers / bodies ─────────────────────────────────────────

fn named_site(name: &str, name_span: Span, span: Span) -> NamedSite {
    NamedSite {
        file: 0,
        name: name.to_string(),
        name_span: pair(name_span),
        span: pair(span),
    }
}

fn lower_view(
    view: &ast::ViewDecl,
    text: &str,
    index: &mut IfmlIndex,
    errors: &mut Vec<LowerError>,
) -> ir::ViewDeclaration {
    // The Pest index captured view-level `roles:`/`requires:` before walking
    // the body; keep that order.
    for property in &view.body.properties {
        capture_view_auth(property, index);
    }
    let body = lower_body(&view.body, text, index, errors);
    let is_landmark = extract_bool_property(&body.properties, "landmark");
    let is_xor = extract_bool_property(&body.properties, "xor");
    let is_modal = extract_bool_property(&body.properties, "modal");
    let position = extract_position_property(&body.properties);
    let roles = extract_roles_property(&body.properties);
    let requires = extract_requires_property(&body.properties);
    let properties = body
        .properties
        .into_iter()
        .filter(|property| property.key != "roles" && property.key != "requires")
        .collect();

    ir::ViewDeclaration {
        name: view.name.clone(),
        label: body.label,
        is_landmark,
        is_xor,
        is_modal,
        params: body.params,
        properties,
        containers: body.containers,
        components: body.components,
        events: body.events,
        module_uses: body.module_uses,
        roles,
        requires,
        condition: body.condition,
        position,
    }
}

fn lower_container(
    container: &ast::ContainerDecl,
    text: &str,
    index: &mut IfmlIndex,
    errors: &mut Vec<LowerError>,
) -> ir::ContainerDeclaration {
    let body = lower_body(&container.body, text, index, errors);
    let position = extract_position_property(&body.properties);
    ir::ContainerDeclaration {
        name: container.name.clone(),
        label: body.label,
        is_default: extract_bool_property(&body.properties, "default"),
        is_xor: extract_bool_property(&body.properties, "xor"),
        params: body.params,
        properties: body.properties,
        containers: body.containers,
        components: body.components,
        events: body.events,
        module_uses: body.module_uses,
        condition: body.condition,
        position,
    }
}

/// A body lowered into the IR lists the declaration needs.
struct LoweredBody {
    params: Vec<ir::ParameterDecl>,
    label: Option<String>,
    properties: Vec<ir::PropertyAssignment>,
    containers: Vec<ir::ContainerDeclaration>,
    components: Vec<ir::ComponentDeclaration>,
    events: Vec<ir::EventHandler>,
    module_uses: Vec<ir::ModuleUse>,
    condition: Option<ir::Expression>,
}

fn lower_body(
    body: &Body,
    text: &str,
    index: &mut IfmlIndex,
    errors: &mut Vec<LowerError>,
) -> LoweredBody {
    let mut lowered = LoweredBody {
        params: body
            .params
            .as_ref()
            .map(|params| lower_params(&params.decls))
            .unwrap_or_default(),
        label: body.label.as_ref().map(|label| label.text.clone()),
        properties: lower_properties(&body.properties),
        containers: Vec::new(),
        components: Vec::new(),
        events: Vec::new(),
        module_uses: Vec::new(),
        condition: None,
    };

    for item in &body.items {
        match item {
            BodyItem::Container(container) => {
                index.containers.push(named_site(
                    &container.name,
                    container.name_span,
                    container.span,
                ));
                lowered
                    .containers
                    .push(lower_container(container, text, index, errors));
            }
            BodyItem::Component(component) => {
                index.components.push(named_site(
                    &component.name,
                    component.name_span,
                    component.span,
                ));
                capture_component_data(component, index);
                lowered
                    .components
                    .push(lower_component(component, text, index, errors));
            }
            BodyItem::Event(event) => {
                lowered.events.push(lower_event(event, index));
            }
            BodyItem::Use(use_decl) => {
                index.module_uses.push(module_use_site(use_decl));
                lowered.module_uses.push(lower_use(use_decl));
            }
            BodyItem::Condition(condition) => {
                lowered.condition = Some(lower_expr(&condition.expr));
            }
        }
    }
    lowered
}

// ── Components ──────────────────────────────────────────────────────────

fn lower_component(
    component: &ast::ComponentDecl,
    text: &str,
    index: &mut IfmlIndex,
    errors: &mut Vec<LowerError>,
) -> ir::ComponentDeclaration {
    let mut properties = lower_properties(&component.body.properties);
    let mut columns = Vec::new();
    let mut fields = Vec::new();
    let mut chart_spec = None;
    let mut events = Vec::new();
    let mut condition = None;

    for member in &component.body.members {
        match member {
            ComponentMember::Column(column) => columns.push(lower_column(column)),
            ComponentMember::Field(field) => fields.push(lower_field(field, text)),
            ComponentMember::Chart(chart) => {
                chart_spec = Some(lower_chart(chart, errors));
            }
            ComponentMember::Condition(statement) => {
                condition = Some(lower_expr(&statement.expr));
            }
            ComponentMember::Event(event) => events.push(lower_event(event, index)),
        }
    }

    let mut component_type = extract_component_type(&mut properties);
    let chart_ref = extract_chart_ref(&mut properties);
    let mut spec = None;

    let columns_allowed =
        component_type.is_none() || component_type == Some(ir::ComponentType::Table);
    if !columns.is_empty() && columns_allowed {
        if component_type.is_none() {
            component_type = Some(ir::ComponentType::Table);
        }
        let pagination = extract_pagination(&mut properties);
        spec = Some(ir::ComponentSpec::Table(ir::TableSpec {
            columns,
            pagination,
        }));
    }

    let fields_allowed =
        component_type.is_none() || component_type == Some(ir::ComponentType::Form);
    if spec.is_none() && !fields.is_empty() && fields_allowed {
        if component_type.is_none() {
            component_type = Some(ir::ComponentType::Form);
        }
        spec = Some(ir::ComponentSpec::Form(ir::FormSpec { fields }));
    }

    let chart_allowed =
        component_type.is_none() || component_type == Some(ir::ComponentType::Chart);
    if let Some(reference) = chart_ref {
        if chart_spec.is_some() {
            errors.push(LowerError::new(
                component.span,
                format!(
                    "component \"{}\": `chart: \"{}\"` cannot be combined with an inline chart declaration",
                    component.name, reference
                ),
            ));
            chart_spec = None;
        }
        if chart_allowed {
            if component_type.is_none() {
                component_type = Some(ir::ComponentType::Chart);
            }
            spec = Some(ir::ComponentSpec::ChartRef(reference));
        }
    }
    if spec.is_none() {
        if let Some(chart) = chart_spec {
            if chart_allowed {
                if component_type.is_none() {
                    component_type = Some(ir::ComponentType::Chart);
                }
                spec = Some(ir::ComponentSpec::Chart(chart));
            }
        }
    }

    ir::ComponentDeclaration {
        name: component.name.clone(),
        component_type,
        spec,
        properties,
        events,
        condition,
    }
}

fn lower_column(column: &ast::ColumnDecl) -> ir::ColumnDef {
    match &column.kind {
        ast::ColumnKind::Lookup { field, lookup } => ir::ColumnDef::Lookup {
            label: column.label.clone(),
            field: property_ref(field),
            lookup: lookup.clone(),
        },
        ast::ColumnKind::Field { field } => ir::ColumnDef::Field {
            label: column.label.clone(),
            field: property_ref(field),
        },
        ast::ColumnKind::Expression { expr } => ir::ColumnDef::Expression {
            label: column.label.clone(),
            expr: lower_expr(expr),
        },
    }
}

fn property_ref(reference: &ast::PropertyRef) -> ir::PropertyRef {
    ir::PropertyRef {
        entity: reference.entity.clone(),
        property: reference.property.clone(),
    }
}

fn lower_field(field: &ast::FieldDecl, text: &str) -> ir::FieldDef {
    let mut required = false;
    let mut validations = Vec::new();
    let mut values = Vec::new();
    let mut messages = Vec::new();
    for property in &field.properties {
        let value = lower_value(&property.value);
        match property.key.as_str() {
            "required" => {
                if let ir::ValueExpression::Bool(value) = value {
                    required = value;
                }
            }
            "validations" => {
                validations.extend(validation_expressions(&property.value, text));
            }
            "values" => values = string_list(&value),
            "messages" => messages = string_list(&value),
            _ => {}
        }
    }
    ir::FieldDef {
        name: field.name.clone(),
        input: ir::InputFieldType::from(field.input.as_str()),
        required,
        validations,
        values,
        messages,
    }
}

fn validation_expressions(value: &ast::ValueExpr, text: &str) -> Vec<ir::Expression> {
    match &value.kind {
        ast::ValueExprKind::Array(items) => items
            .iter()
            .map(|item| lower_value_as_expr(item, text))
            .collect(),
        _ => Vec::new(),
    }
}

fn string_list(value: &ir::ValueExpression) -> Vec<String> {
    let mut out = Vec::new();
    if let ir::ValueExpression::Array(items) = value {
        for item in items {
            match item {
                ir::ValueExpression::String(text) => out.push(text.clone()),
                ir::ValueExpression::Identifier(identifier) => out.push(identifier.clone()),
                _ => {}
            }
        }
    }
    out
}

fn extract_component_type(
    properties: &mut Vec<ir::PropertyAssignment>,
) -> Option<ir::ComponentType> {
    let mut component_type = None;
    properties.retain(|property| {
        if property.key == "type" && component_type.is_none() {
            if let ir::ValueExpression::Identifier(identifier) = &property.value {
                component_type = Some(ir::ComponentType::from(identifier.as_str()));
                return false;
            }
        }
        true
    });
    component_type
}

fn extract_pagination(properties: &mut Vec<ir::PropertyAssignment>) -> bool {
    let mut pagination = true;
    properties.retain(|property| {
        if property.key == "pagination" {
            if let ir::ValueExpression::Bool(value) = property.value {
                pagination = value;
                return false;
            }
        }
        true
    });
    pagination
}

fn extract_chart_ref(properties: &mut Vec<ir::PropertyAssignment>) -> Option<String> {
    let mut chart_ref = None;
    properties.retain(|property| {
        if property.key == "chart" && chart_ref.is_none() {
            if let ir::ValueExpression::String(reference) = &property.value {
                chart_ref = Some(reference.clone());
                return false;
            }
        }
        true
    });
    chart_ref
}

/// The chart kind vocabularies are closed and exact-case (the PEG grammar had
/// no identifier fallback); the parser already rejected anything else, so the
/// fallback is unreachable.
fn chart_kind(kind: &str) -> ir::ChartKind {
    match kind {
        "line" => ir::ChartKind::Line,
        "pie" => ir::ChartKind::Pie,
        "radar" => ir::ChartKind::Radar,
        "metric" => ir::ChartKind::Metric,
        "org" => ir::ChartKind::Org,
        _ => ir::ChartKind::Bar,
    }
}

fn chart_aggregate(word: &str) -> Option<ir::ChartAggregate> {
    match word.to_ascii_lowercase().as_str() {
        "sum" => Some(ir::ChartAggregate::Sum),
        "avg" => Some(ir::ChartAggregate::Avg),
        "count" => Some(ir::ChartAggregate::Count),
        "min" => Some(ir::ChartAggregate::Min),
        "max" => Some(ir::ChartAggregate::Max),
        _ => None,
    }
}

fn lower_chart(chart: &ast::ChartDecl, errors: &mut Vec<LowerError>) -> ir::ChartSpec {
    let mut spec = ir::ChartSpec {
        kind: chart_kind(&chart.kind),
        label_field: None,
        value_fields: Vec::new(),
        hierarchy_field: None,
        label_fields: Vec::new(),
        detail_fields: Vec::new(),
        max_depth: None,
        aggregate: None,
    };
    for property in &chart.properties {
        match (&property.value.kind, property.key.as_str()) {
            (ast::ValueExprKind::Ident(identifier), "label") => {
                spec.label_field = Some(identifier.clone());
            }
            (ast::ValueExprKind::Array(items), "values") => {
                for item in items {
                    if let ast::ValueExprKind::Ident(identifier) = &item.kind {
                        spec.value_fields.push(identifier.clone());
                    }
                }
            }
            (ast::ValueExprKind::Ident(identifier), "hierarchy") => {
                spec.hierarchy_field = Some(identifier.clone());
            }
            (ast::ValueExprKind::Array(items), "labels") => {
                for item in items {
                    if let ast::ValueExprKind::Ident(identifier) = &item.kind {
                        spec.label_fields.push(identifier.clone());
                    }
                }
            }
            (ast::ValueExprKind::Array(items), "details") => {
                for item in items {
                    if let ast::ValueExprKind::Ident(identifier) = &item.kind {
                        spec.detail_fields.push(identifier.clone());
                    }
                }
            }
            (ast::ValueExprKind::Number(number), "maxDepth") => {
                spec.max_depth = Some(number.max(0.0) as u32);
            }
            (ast::ValueExprKind::Ident(identifier), "aggregate") => {
                match chart_aggregate(identifier) {
                    Some(aggregate) => spec.aggregate = Some(aggregate),
                    None => errors.push(LowerError::new(
                        property.span,
                        format!(
                            "unknown aggregate `{identifier}` (expected sum | avg | count | min | max)"
                        ),
                    )),
                }
            }
            _ => {}
        }
    }
    spec
}

// ── Events ──────────────────────────────────────────────────────────────

fn lower_events(events: &[ast::EventHandler], index: &mut IfmlIndex) -> Vec<ir::EventHandler> {
    events
        .iter()
        .map(|event| lower_event(event, index))
        .collect()
}

fn lower_event(event: &ast::EventHandler, index: &mut IfmlIndex) -> ir::EventHandler {
    for required in &event.requires {
        index.auth_sites.push(AuthSite {
            file: 0,
            kind: AuthKind::Capability,
            name: required.name.clone(),
            name_span: pair(required.span),
        });
    }
    let action = match &event.action {
        ast::EventAction::Navigate {
            target,
            target_span,
            binding,
        } => {
            index.action_targets.push(TargetSite {
                file: 0,
                target: target.clone(),
                action: "navigate".to_string(),
                name_span: pair(*target_span),
            });
            ir::EventAction::Navigate {
                target: target.clone(),
                binding: binding.as_ref().map(lower_binding),
            }
        }
        ast::EventAction::Refresh {
            target,
            target_span,
            binding,
        } => {
            index.action_targets.push(TargetSite {
                file: 0,
                target: target.clone(),
                action: "refresh".to_string(),
                name_span: pair(*target_span),
            });
            ir::EventAction::Refresh {
                target: target.clone(),
                binding: binding.as_ref().map(lower_binding),
            }
        }
        ast::EventAction::ActionInvocation {
            name,
            name_span,
            body,
        } => {
            index.action_targets.push(TargetSite {
                file: 0,
                target: name.clone(),
                action: "action".to_string(),
                name_span: pair(*name_span),
            });
            let body = body.as_ref().map(|body| ir::ActionBody {
                properties: lower_properties(&body.properties),
                handlers: lower_events(&body.events, index),
            });
            ir::EventAction::ActionInvocation {
                name: name.clone(),
                body,
            }
        }
        ast::EventAction::Stay => ir::EventAction::Stay,
    };

    ir::EventHandler {
        event_type: event_type(&event.event_type),
        params: event.params.clone(),
        requires: event
            .requires
            .iter()
            .map(|required| required.name.clone())
            .collect(),
        condition: event.condition.as_ref().map(lower_expr),
        action,
    }
}

fn event_type(event: &str) -> ir::EventType {
    match event {
        "select" => ir::EventType::Select,
        "submit" => ir::EventType::Submit,
        "click" => ir::EventType::Click,
        "change" => ir::EventType::Change,
        "load" => ir::EventType::Load,
        "save" => ir::EventType::Save,
        "cancel" => ir::EventType::Cancel,
        "delete" => ir::EventType::Delete,
        "confirm" => ir::EventType::Confirm,
        "back" => ir::EventType::Back,
        other => ir::EventType::Custom(other.to_string()),
    }
}

fn lower_binding(binding: &ast::ParameterBinding) -> ir::ParameterBinding {
    ir::ParameterBinding {
        pairs: binding
            .pairs
            .iter()
            .map(|(key, expr)| (key.clone(), lower_expr(expr)))
            .collect(),
    }
}

// ── Modules / uses / commands ───────────────────────────────────────────

fn index_param(param: &ast::ParamDecl) -> IndexParam {
    IndexParam {
        name: param.name.clone(),
        type_ref: param.type_ref.clone(),
        span: pair(param.span),
        has_default: param.default.is_some(),
    }
}

fn module_site(module: &ast::ModuleDecl) -> ModuleDeclSite {
    ModuleDeclSite {
        file: 0,
        name: module.name.clone(),
        name_span: pair(module.name_span),
        span: pair(module.span),
        input_params: module.input.decls.iter().map(index_param).collect(),
        properties: module
            .properties
            .iter()
            .map(|property| property.key.clone())
            .collect(),
    }
}

fn module_use_site(use_decl: &ast::ModuleUse) -> ModuleUseSite {
    ModuleUseSite {
        file: 0,
        target: use_decl.module.clone(),
        name_span: pair(use_decl.target_span),
        alias: use_decl.alias.clone(),
        span: pair(use_decl.span),
        overrides: use_decl
            .properties
            .iter()
            .map(|property| UseOverride {
                name: property.key.clone(),
                span: pair(property.span),
                literal: literal_kind(&property.value),
            })
            .collect(),
        resolved_module: None,
    }
}

fn literal_kind(value: &ast::ValueExpr) -> Option<LiteralKind> {
    match &value.kind {
        ast::ValueExprKind::String(_) => Some(LiteralKind::Str),
        ast::ValueExprKind::Number(_) => Some(LiteralKind::Num),
        ast::ValueExprKind::Bool(_) => Some(LiteralKind::Bool),
        ast::ValueExprKind::Group(inner) => literal_kind(inner),
        ast::ValueExprKind::Unary { operand, .. } => literal_kind(operand),
        _ => None,
    }
}

fn lower_use(use_decl: &ast::ModuleUse) -> ir::ModuleUse {
    ir::ModuleUse {
        module: use_decl.module.clone(),
        alias: use_decl.alias.clone(),
        properties: lower_properties(&use_decl.properties),
    }
}

fn lower_module(
    module: &ast::ModuleDecl,
    text: &str,
    index: &mut IfmlIndex,
    errors: &mut Vec<LowerError>,
) -> ir::ModuleDeclaration {
    let mut containers = Vec::new();
    let mut components = Vec::new();
    let mut events = Vec::new();
    let mut module_uses = Vec::new();
    for item in &module.items {
        match item {
            ast::ModuleItem::Container(container) => {
                index.containers.push(named_site(
                    &container.name,
                    container.name_span,
                    container.span,
                ));
                containers.push(lower_container(container, text, index, errors));
            }
            ast::ModuleItem::Component(component) => {
                index.components.push(named_site(
                    &component.name,
                    component.name_span,
                    component.span,
                ));
                capture_component_data(component, index);
                components.push(lower_component(component, text, index, errors));
            }
            ast::ModuleItem::Event(event) => events.push(lower_event(event, index)),
            ast::ModuleItem::Use(use_decl) => {
                index.module_uses.push(module_use_site(use_decl));
                module_uses.push(lower_use(use_decl));
            }
        }
    }
    ir::ModuleDeclaration {
        name: module.name.clone(),
        input_params: lower_params(&module.input.decls),
        output_params: lower_params(&module.output.decls),
        properties: lower_properties(&module.properties),
        containers,
        components,
        events,
        module_uses,
    }
}

fn lower_command(command: &ast::CommandDecl, index: &mut IfmlIndex) -> ir::CommandDeclaration {
    index
        .commands
        .push(named_site(&command.name, command.name_span, command.span));

    let mut arguments = Vec::new();
    let mut options = Vec::new();
    let mut prompts = Vec::new();
    let mut events = Vec::new();
    let mut module_uses = Vec::new();
    let mut subcommands = Vec::new();
    let mut properties = Vec::new();
    let mut condition = None;

    // Source order is the index contract: nested command sites, use sites,
    // and action targets are recorded as they are written.
    for member in &command.members {
        match member {
            ast::CommandMember::Property(property) => {
                properties.push(lower_property(property));
            }
            ast::CommandMember::Argument(argument) => arguments.push(ir::CommandArgument {
                name: argument.name.clone(),
                type_ref: argument.type_ref.clone(),
            }),
            ast::CommandMember::Option(option) => options.push(ir::CommandOption {
                name: option.name.clone(),
                type_ref: option.type_ref.clone(),
                default: option.default.as_ref().map(lower_default),
            }),
            ast::CommandMember::Prompt(prompt) => prompts.push(ir::CommandPrompt {
                name: prompt.name.clone(),
                kind: ir::PromptKind::from(prompt.kind.as_str()),
                default: prompt.default.as_ref().map(lower_default),
            }),
            ast::CommandMember::Event(event) => events.push(lower_event(event, index)),
            ast::CommandMember::Use(use_decl) => {
                index.module_uses.push(module_use_site(use_decl));
                module_uses.push(lower_use(use_decl));
            }
            ast::CommandMember::Condition(statement) => {
                condition = Some(lower_expr(&statement.expr));
            }
            ast::CommandMember::Command(subcommand) => {
                subcommands.push(lower_command(subcommand, index));
            }
        }
    }

    let description = extract_description_property(&properties);
    let requires = extract_requires_property(&properties);
    properties.retain(|property| property.key != "description" && property.key != "requires");

    ir::CommandDeclaration {
        name: command.name.clone(),
        description,
        arguments,
        options,
        prompts,
        events,
        module_uses,
        subcommands,
        properties,
        requires,
        condition,
    }
}

// ── Values / expressions ────────────────────────────────────────────────

fn lower_params(params: &[ast::ParamDecl]) -> Vec<ir::ParameterDecl> {
    params
        .iter()
        .map(|param| ir::ParameterDecl {
            name: param.name.clone(),
            type_ref: param.type_ref.clone(),
            default: param.default.as_ref().map(lower_default),
        })
        .collect()
}

fn lower_default(value: &DefaultValue) -> ir::ValueExpression {
    match value {
        DefaultValue::String(text) => ir::ValueExpression::String(text.clone()),
        DefaultValue::Number(number) => ir::ValueExpression::Number(Float::new(*number)),
        DefaultValue::Bool(value) => ir::ValueExpression::Bool(*value),
    }
}

fn lower_property(property: &Property) -> ir::PropertyAssignment {
    ir::PropertyAssignment {
        key: property.key.clone(),
        value: lower_value(&property.value),
        span: Some(pair(property.span)),
    }
}

fn lower_properties(properties: &[Property]) -> Vec<ir::PropertyAssignment> {
    properties.iter().map(lower_property).collect()
}

fn lower_value(value: &ast::ValueExpr) -> ir::ValueExpression {
    match &value.kind {
        ast::ValueExprKind::Ident(text) => ir::ValueExpression::Identifier(text.clone()),
        ast::ValueExprKind::String(text) => ir::ValueExpression::String(text.clone()),
        ast::ValueExprKind::Number(number) => ir::ValueExpression::Number(Float::new(*number)),
        ast::ValueExprKind::Bool(value) => ir::ValueExpression::Bool(*value),
        ast::ValueExprKind::Call { name, args, .. } => {
            ir::ValueExpression::Call(name.clone(), args.iter().map(lower_expr_as_value).collect())
        }
        ast::ValueExprKind::Field { object, field, .. } => ir::ValueExpression::FieldAccess {
            object: Box::new(lower_value(object)),
            field: field.clone(),
        },
        ast::ValueExprKind::Group(inner) => {
            ir::ValueExpression::Group(Box::new(lower_value(inner)))
        }
        ast::ValueExprKind::Unary { op, operand } => ir::ValueExpression::UnaryOp {
            op: op.clone(),
            operand: Box::new(lower_value(operand)),
        },
        ast::ValueExprKind::Binary { left, op, right } => ir::ValueExpression::BinOp {
            left: Box::new(lower_value(left)),
            op: op.clone(),
            right: Box::new(lower_value(right)),
        },
        ast::ValueExprKind::Array(items) => {
            ir::ValueExpression::Array(items.iter().map(lower_value).collect())
        }
        ast::ValueExprKind::Object(members) => ir::ValueExpression::Object(
            members
                .iter()
                .map(|member| ir::ObjectMember {
                    key: member.key.clone(),
                    value: lower_value(&member.value),
                })
                .collect(),
        ),
    }
}

fn lower_expr(expr: &ast::Expr) -> ir::Expression {
    match &expr.kind {
        ast::ExprKind::Ident(text) => ir::Expression::Ident(text.clone()),
        ast::ExprKind::StringLit(text) => ir::Expression::StringLit(text.clone()),
        ast::ExprKind::NumLit(number) => ir::Expression::NumLit(Float::new(*number)),
        ast::ExprKind::BoolLit(value) => ir::Expression::BoolLit(*value),
        ast::ExprKind::Field { object, field, .. } => ir::Expression::FieldExpr {
            object: Box::new(lower_expr(object)),
            field: field.clone(),
        },
        ast::ExprKind::Binary { left, op, right } => ir::Expression::BinOp {
            left: Box::new(lower_expr(left)),
            op: op.clone(),
            right: Box::new(lower_expr(right)),
        },
        ast::ExprKind::Unary { op, operand } => ir::Expression::UnaryOp {
            op: op.clone(),
            operand: Box::new(lower_expr(operand)),
        },
        ast::ExprKind::Group(inner) => ir::Expression::Group(Box::new(lower_expr(inner))),
        ast::ExprKind::Call { name, args, .. } => ir::Expression::Call {
            name: name.clone(),
            args: args.iter().map(lower_expr).collect(),
        },
    }
}

/// Lower an expression in a value position (call arguments are the one
/// place the IR mixes the two surfaces).
fn lower_expr_as_value(expr: &ast::Expr) -> ir::ValueExpression {
    match &expr.kind {
        ast::ExprKind::Ident(text) => ir::ValueExpression::Identifier(text.clone()),
        ast::ExprKind::StringLit(text) => ir::ValueExpression::String(text.clone()),
        ast::ExprKind::NumLit(number) => ir::ValueExpression::Number(Float::new(*number)),
        ast::ExprKind::BoolLit(value) => ir::ValueExpression::Bool(*value),
        ast::ExprKind::Field { object, field, .. } => ir::ValueExpression::FieldAccess {
            object: Box::new(lower_expr_as_value(object)),
            field: field.clone(),
        },
        ast::ExprKind::Binary { left, op, right } => ir::ValueExpression::BinOp {
            left: Box::new(lower_expr_as_value(left)),
            op: op.clone(),
            right: Box::new(lower_expr_as_value(right)),
        },
        ast::ExprKind::Unary { op, operand } => ir::ValueExpression::UnaryOp {
            op: op.clone(),
            operand: Box::new(lower_expr_as_value(operand)),
        },
        ast::ExprKind::Group(inner) => {
            ir::ValueExpression::Group(Box::new(lower_expr_as_value(inner)))
        }
        ast::ExprKind::Call { name, args, .. } => {
            ir::ValueExpression::Call(name.clone(), args.iter().map(lower_expr_as_value).collect())
        }
    }
}

/// Lower a value in an expression position. Arrays/objects cannot be written
/// there by the grammar; if a malformed AST reaches this (the old parser's
/// `fallback_expr` path), keep its source text as an identifier rather than
/// panicking.
fn lower_value_as_expr(value: &ast::ValueExpr, text: &str) -> ir::Expression {
    match &value.kind {
        ast::ValueExprKind::Ident(identifier) => ir::Expression::Ident(identifier.clone()),
        ast::ValueExprKind::String(string) => ir::Expression::StringLit(string.clone()),
        ast::ValueExprKind::Number(number) => ir::Expression::NumLit(Float::new(*number)),
        ast::ValueExprKind::Bool(value) => ir::Expression::BoolLit(*value),
        ast::ValueExprKind::Call { name, args, .. } => ir::Expression::Call {
            name: name.clone(),
            args: args.iter().map(lower_expr).collect(),
        },
        ast::ValueExprKind::Field { object, field, .. } => ir::Expression::FieldExpr {
            object: Box::new(lower_value_as_expr(object, text)),
            field: field.clone(),
        },
        ast::ValueExprKind::Group(inner) => {
            ir::Expression::Group(Box::new(lower_value_as_expr(inner, text)))
        }
        ast::ValueExprKind::Unary { op, operand } => ir::Expression::UnaryOp {
            op: op.clone(),
            operand: Box::new(lower_value_as_expr(operand, text)),
        },
        ast::ValueExprKind::Binary { left, op, right } => ir::Expression::BinOp {
            left: Box::new(lower_value_as_expr(left, text)),
            op: op.clone(),
            right: Box::new(lower_value_as_expr(right, text)),
        },
        ast::ValueExprKind::Array(_) | ast::ValueExprKind::Object(_) => {
            ir::Expression::Ident(text[value.span.start..value.span.end].to_string())
        }
    }
}

// ── Property-bag extractors (moved from the Pest walkers) ───────────────

fn extract_bool_property(properties: &[ir::PropertyAssignment], key: &str) -> bool {
    properties
        .iter()
        .find(|property| property.key == key)
        .and_then(|property| match property.value {
            ir::ValueExpression::Bool(value) => Some(value),
            _ => None,
        })
        .unwrap_or(false)
}

fn extract_position_property(properties: &[ir::PropertyAssignment]) -> Option<ir::Position> {
    let property = properties
        .iter()
        .find(|property| property.key == "position")?;
    let members = match &property.value {
        ir::ValueExpression::Object(members) => members,
        _ => return None,
    };
    let as_number = |value: &ir::ValueExpression| -> Option<f64> {
        match value {
            ir::ValueExpression::Number(number) => Some(number.value()),
            ir::ValueExpression::UnaryOp {
                op: ir::UnaryOp::Neg,
                operand,
            } => match operand.as_ref() {
                ir::ValueExpression::Number(number) => Some(-number.value()),
                _ => None,
            },
            _ => None,
        }
    };
    let get = |key: &str| {
        members
            .iter()
            .find(|member| member.key == key)
            .and_then(|member| as_number(&member.value))
    };
    match (get("x"), get("y")) {
        (Some(x), Some(y)) => Some(ir::Position {
            x: Float::new(x),
            y: Float::new(y),
        }),
        _ => None,
    }
}

fn extract_roles_property(properties: &[ir::PropertyAssignment]) -> Vec<String> {
    extract_identifier_list_property(properties, "roles")
}

fn extract_requires_property(properties: &[ir::PropertyAssignment]) -> Vec<String> {
    extract_identifier_list_property(properties, "requires")
}

fn extract_description_property(properties: &[ir::PropertyAssignment]) -> Option<String> {
    properties
        .iter()
        .find(|property| property.key == "description")
        .and_then(|property| match &property.value {
            ir::ValueExpression::String(text) => Some(text.clone()),
            _ => None,
        })
}

fn extract_identifier_list_property(
    properties: &[ir::PropertyAssignment],
    key: &str,
) -> Vec<String> {
    properties
        .iter()
        .find(|property| property.key == key)
        .and_then(|property| match &property.value {
            ir::ValueExpression::Array(items) => Some(
                items
                    .iter()
                    .filter_map(|item| match item {
                        ir::ValueExpression::Identifier(identifier) => Some(identifier.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap_or_default()
}

// ── Index captures ──────────────────────────────────────────────────────

/// View-level `roles:`/`requires:`: only the array form counts, and every
/// identifier inside the first array literal is a site (the Pest
/// `descendants` behaviour).
fn capture_view_auth(property: &Property, index: &mut IfmlIndex) {
    let kind = match property.key.as_str() {
        "requires" => AuthKind::Capability,
        "roles" => AuthKind::Role,
        _ => return,
    };
    let Some(array) = first_array(&property.value) else {
        return;
    };
    collect_idents(array, &mut |name, span| {
        index.auth_sites.push(AuthSite {
            file: 0,
            kind,
            name: name.to_string(),
            name_span: pair(span),
        });
    });
}

fn first_array(value: &ast::ValueExpr) -> Option<&ast::ValueExpr> {
    match &value.kind {
        ast::ValueExprKind::Array(_) => Some(value),
        ast::ValueExprKind::Field { object, .. } | ast::ValueExprKind::Group(object) => {
            first_array(object)
        }
        ast::ValueExprKind::Unary { operand, .. } => first_array(operand),
        ast::ValueExprKind::Binary { left, right, .. } => {
            first_array(left).or_else(|| first_array(right))
        }
        _ => None,
    }
}

fn collect_idents(value: &ast::ValueExpr, sink: &mut impl FnMut(&str, Span)) {
    match &value.kind {
        ast::ValueExprKind::Ident(name) => sink(name, value.span),
        ast::ValueExprKind::Call {
            name,
            name_span,
            args,
        } => {
            sink(name, *name_span);
            for arg in args {
                collect_expr_idents(arg, sink);
            }
        }
        ast::ValueExprKind::Field {
            object,
            field,
            field_span,
        } => {
            collect_idents(object, sink);
            sink(field, *field_span);
        }
        ast::ValueExprKind::Group(inner) => collect_idents(inner, sink),
        ast::ValueExprKind::Unary { operand, .. } => collect_idents(operand, sink),
        ast::ValueExprKind::Binary { left, right, .. } => {
            collect_idents(left, sink);
            collect_idents(right, sink);
        }
        ast::ValueExprKind::Array(items) => {
            for item in items {
                collect_idents(item, sink);
            }
        }
        ast::ValueExprKind::Object(members) => {
            for member in members {
                collect_idents(&member.value, sink);
            }
        }
        ast::ValueExprKind::String(_)
        | ast::ValueExprKind::Number(_)
        | ast::ValueExprKind::Bool(_) => {}
    }
}

fn collect_expr_idents(expr: &ast::Expr, sink: &mut impl FnMut(&str, Span)) {
    match &expr.kind {
        ast::ExprKind::Ident(name) => sink(name, expr.span),
        ast::ExprKind::Call {
            name,
            name_span,
            args,
        } => {
            sink(name, *name_span);
            for arg in args {
                collect_expr_idents(arg, sink);
            }
        }
        ast::ExprKind::Field {
            object,
            field,
            field_span,
        } => {
            collect_expr_idents(object, sink);
            sink(field, *field_span);
        }
        ast::ExprKind::Group(inner) => collect_expr_idents(inner, sink),
        ast::ExprKind::Unary { operand, .. } => collect_expr_idents(operand, sink),
        ast::ExprKind::Binary { left, right, .. } => {
            collect_expr_idents(left, sink);
            collect_expr_idents(right, sink);
        }
        ast::ExprKind::StringLit(_) | ast::ExprKind::NumLit(_) | ast::ExprKind::BoolLit(_) => {}
    }
}

/// A component body's direct `data:` binding: only the bare identifier form.
fn capture_component_data(component: &ast::ComponentDecl, index: &mut IfmlIndex) {
    for property in &component.body.properties {
        if property.key != "data" {
            continue;
        }
        if let ast::ValueExprKind::Ident(name) = &property.value.kind {
            index.data_sites.push(DataSite {
                file: 0,
                name: name.clone(),
                name_span: pair(property.value.span),
            });
        }
    }
}
