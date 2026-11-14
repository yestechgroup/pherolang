//! The spanned syntax tree for `.ifml`, the chumsky front half's product.
//!
//! Every node owns its strings and carries a byte [`Span`]. The shapes mirror
//! the grammar (see the parser module docs for the normative EBNF), not the
//! wire IR: lowering ([`crate::lower`]) is the only place that knows
//! `phero_ir::ifml`. Expression values (`x: <value>` and object members) use
//! [`ValueExpr`]; bound conditions and call arguments use [`Expr`]. Keeping
//! two expression nodes is what makes the AST's array/object shape explicit
//! instead of inferred from a parse tree.

use phero_ir::ifml::{BinOp, UnaryOp};
use phero_parser::Span;

/// A whole `.ifml` file: the import/domain prologue and the declarations, in
/// source order.
#[derive(Debug, Clone, PartialEq)]
pub struct IfmlFile {
    /// `import "..."` declarations, in source order.
    pub imports: Vec<ImportDecl>,
    /// `domain "..." { schema "..." }` declarations, in source order.
    pub domains: Vec<DomainDecl>,
    /// Top-level declarations in source order.
    pub declarations: Vec<Declaration>,
    /// The whole file.
    pub span: Span,
}

/// An `import "path";` declaration. The path is unescaped.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    /// The unescaped import path.
    pub path: String,
    /// The whole declaration.
    pub span: Span,
}

/// A `domain "name" { schema "path"; }` declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct DomainDecl {
    /// The unescaped domain name.
    pub name: String,
    /// The unescaped schema path.
    pub schema: String,
    /// The whole declaration.
    pub span: Span,
}

/// A top-level declaration.
#[derive(Debug, Clone, PartialEq)]
pub enum Declaration {
    /// A `view "Name" { ... }`.
    View(Box<ViewDecl>),
    /// An `action "Name" { ... }`.
    Action(Box<ActionDecl>),
    /// A `module "Name" { ... }`.
    Module(Box<ModuleDecl>),
    /// An `actor "Name" { ... }`.
    Actor(Box<ActorDecl>),
    /// A `command "Name" { ... }` (subcommands nest).
    Command(Box<CommandDecl>),
}

/// A view declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewDecl {
    /// The unescaped name.
    pub name: String,
    /// The `"..."` name token's span.
    pub name_span: Span,
    /// The body.
    pub body: Body,
    /// The whole declaration.
    pub span: Span,
}

/// A container declaration (inside a view, container, or module body).
#[derive(Debug, Clone, PartialEq)]
pub struct ContainerDecl {
    /// The unescaped name.
    pub name: String,
    /// The `"..."` name token's span.
    pub name_span: Span,
    /// The body.
    pub body: Body,
    /// The whole declaration.
    pub span: Span,
}

/// A view or container body, in source order.
#[derive(Debug, Clone, PartialEq)]
pub struct Body {
    /// The optional `params { ... };` block.
    pub params: Option<Params>,
    /// The optional `label "...";` declaration.
    pub label: Option<Label>,
    /// Direct `key: value;` assignments, in source order.
    pub properties: Vec<Property>,
    /// Nested containers/components/events/uses/conditions, in source order.
    pub items: Vec<BodyItem>,
    /// The braces (`{` through `}`).
    pub span: Span,
}

/// One member of a view/container body.
#[derive(Debug, Clone, PartialEq)]
pub enum BodyItem {
    /// A nested container.
    Container(Box<ContainerDecl>),
    /// A component.
    Component(Box<ComponentDecl>),
    /// An `on ... -> ...;` handler.
    Event(EventHandler),
    /// A `use "Module" ...;`.
    Use(ModuleUse),
    /// An `if <expr>;` condition.
    Condition(Condition),
}

/// A `params { ... };` block.
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    /// The parameters, in source order.
    pub decls: Vec<ParamDecl>,
    /// The braces.
    pub span: Span,
}

/// One `name: Type (= default)?` parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDecl {
    /// The parameter name.
    pub name: String,
    /// The declared type as written.
    pub type_ref: String,
    /// The optional literal default.
    pub default: Option<DefaultValue>,
    /// From the name through the type, or through the default when present.
    pub span: Span,
}

/// A `param_default` literal: string, number, or boolean.
#[derive(Debug, Clone, PartialEq)]
pub enum DefaultValue {
    /// A quoted string, unescaped.
    String(String),
    /// A number, negative when the `-` was adjacent.
    Number(f64),
    /// `true` / `false`.
    Bool(bool),
}

/// A `label "...";` declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    /// The unescaped label text.
    pub text: String,
    /// The whole declaration.
    pub span: Span,
}

/// A `key: <value>;` property assignment. The span covers key through the
/// trailing `;` exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// The property key.
    pub key: String,
    /// The value.
    pub value: ValueExpr,
    /// Key through `;`.
    pub span: Span,
}

/// A component declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentDecl {
    /// The unescaped name.
    pub name: String,
    /// The `"..."` name token's span.
    pub name_span: Span,
    /// The body.
    pub body: ComponentBody,
    /// The whole declaration.
    pub span: Span,
}

/// A component body.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentBody {
    /// Direct properties, in source order.
    pub properties: Vec<Property>,
    /// Typed/event members, in source order.
    pub members: Vec<ComponentMember>,
    /// The braces.
    pub span: Span,
}

/// One member of a component body.
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentMember {
    /// A `column ...;`.
    Column(ColumnDecl),
    /// A `field ...;`.
    Field(FieldDecl),
    /// A `chart kind { ... };`.
    Chart(ChartDecl),
    /// An `if <expr>;` condition.
    Condition(Condition),
    /// An `on ... -> ...;` handler.
    Event(EventHandler),
}

/// A table column.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDecl {
    /// The unescaped column label.
    pub label: String,
    /// What the column binds.
    pub kind: ColumnKind,
    /// The whole declaration.
    pub span: Span,
}

/// The payload of a `column` declaration.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnKind {
    /// `lookup Entity.Property via lookup`.
    Lookup {
        /// The referenced property.
        field: PropertyRef,
        /// The lookup operation.
        lookup: String,
    },
    /// `field Entity.Property`.
    Field {
        /// The referenced property.
        field: PropertyRef,
    },
    /// `expr <expression>`.
    Expression {
        /// The bound expression.
        expr: Expr,
    },
}

/// An `Entity.Property` reference.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyRef {
    /// The entity name.
    pub entity: String,
    /// The property name.
    pub property: String,
}

/// A typed form field.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    /// The field name.
    pub name: String,
    /// The input type as written (lowered through `InputFieldType`).
    pub input: String,
    /// The optional `{ ... }` body properties.
    pub properties: Vec<Property>,
    /// The whole declaration.
    pub span: Span,
}

/// An inline chart declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartDecl {
    /// The chart kind as written (a closed vocabulary).
    pub kind: String,
    /// The optional `{ ... }` body properties.
    pub properties: Vec<Property>,
    /// The whole declaration.
    pub span: Span,
}

/// An `if <expr>;` condition statement.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    /// The condition expression.
    pub expr: Expr,
    /// The whole statement.
    pub span: Span,
}

/// An `on <type>(params)? requires: [...]? if <expr>? -> <action>;` handler.
#[derive(Debug, Clone, PartialEq)]
pub struct EventHandler {
    /// The event type as written.
    pub event_type: String,
    /// The event type token's span.
    pub type_span: Span,
    /// The optional parameter names.
    pub params: Vec<String>,
    /// The `requires:` capabilities, with their token spans.
    pub requires: Vec<AuthName>,
    /// The optional `if` condition.
    pub condition: Option<Expr>,
    /// The action.
    pub action: EventAction,
    /// The whole handler.
    pub span: Span,
}

/// One name in an authorization list, with its source span.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthName {
    /// The name as written.
    pub name: String,
    /// The name token's span.
    pub span: Span,
}

/// The action an event handler dispatches to.
#[derive(Debug, Clone, PartialEq)]
pub enum EventAction {
    /// `navigate("View", binding?)`.
    Navigate {
        /// The unescaped target name.
        target: String,
        /// The target string token's span.
        target_span: Span,
        /// The optional parameter binding.
        binding: Option<ParameterBinding>,
    },
    /// `refresh("Component", binding?)`.
    Refresh {
        /// The unescaped target name.
        target: String,
        /// The target string token's span.
        target_span: Span,
        /// The optional parameter binding.
        binding: Option<ParameterBinding>,
    },
    /// `action("Name", body?)`.
    ActionInvocation {
        /// The unescaped action name.
        name: String,
        /// The action string token's span.
        name_span: Span,
        /// The optional action body.
        body: Option<ActionBody>,
    },
    /// `stay`.
    Stay,
}

/// A `{ key: expr, ... }` parameter binding.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterBinding {
    /// The pairs, in source order.
    pub pairs: Vec<(String, Expr)>,
    /// The braces.
    pub span: Span,
}

/// An inline action body.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionBody {
    /// Direct properties.
    pub properties: Vec<Property>,
    /// Event handlers.
    pub events: Vec<EventHandler>,
    /// The braces.
    pub span: Span,
}

/// A `use "Module" as alias { ... };` statement.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleUse {
    /// The unescaped target module name.
    pub module: String,
    /// The target string token's span.
    pub target_span: Span,
    /// The optional alias.
    pub alias: Option<String>,
    /// Override properties, in source order.
    pub properties: Vec<Property>,
    /// The whole statement.
    pub span: Span,
}

/// An action declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionDecl {
    /// The unescaped name.
    pub name: String,
    /// The name token's span.
    pub name_span: Span,
    /// Direct properties.
    pub properties: Vec<Property>,
    /// Event handlers.
    pub events: Vec<EventHandler>,
    /// The whole declaration.
    pub span: Span,
}

/// A module declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleDecl {
    /// The unescaped name.
    pub name: String,
    /// The name token's span.
    pub name_span: Span,
    /// The required `input` parameters.
    pub input: Params,
    /// The required `output` parameters.
    pub output: Params,
    /// Direct properties.
    pub properties: Vec<Property>,
    /// Containers/components/events/uses, in source order.
    pub items: Vec<ModuleItem>,
    /// The whole declaration.
    pub span: Span,
}

/// One member of a module body.
#[derive(Debug, Clone, PartialEq)]
pub enum ModuleItem {
    /// A nested container.
    Container(Box<ContainerDecl>),
    /// A component.
    Component(Box<ComponentDecl>),
    /// An event handler.
    Event(EventHandler),
    /// A `use` statement.
    Use(ModuleUse),
}

/// An actor declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ActorDecl {
    /// The unescaped name.
    pub name: String,
    /// The name token's span.
    pub name_span: Span,
    /// Direct properties.
    pub properties: Vec<Property>,
    /// The whole declaration.
    pub span: Span,
}

/// A command declaration (and its nested subcommands).
#[derive(Debug, Clone, PartialEq)]
pub struct CommandDecl {
    /// The unescaped name.
    pub name: String,
    /// The name token's span.
    pub name_span: Span,
    /// The body members, in source order. Lowering partitions them into the
    /// IR lists while walking, so index sites keep source order across kinds.
    pub members: Vec<CommandMember>,
    /// The whole declaration.
    pub span: Span,
}

/// One member of a command body.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandMember {
    /// A direct property assignment.
    Property(Property),
    /// A positional argument.
    Argument(CommandArgument),
    /// An option.
    Option(CommandOption),
    /// A prompt.
    Prompt(CommandPrompt),
    /// An event handler.
    Event(EventHandler),
    /// A `use` statement.
    Use(ModuleUse),
    /// An `if` condition.
    Condition(Condition),
    /// A nested subcommand.
    Command(Box<CommandDecl>),
}

/// A command's positional argument.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandArgument {
    /// The argument name.
    pub name: String,
    /// The declared type as written.
    pub type_ref: String,
    /// The whole declaration.
    pub span: Span,
}

/// A command option.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandOption {
    /// The option name.
    pub name: String,
    /// The declared type as written.
    pub type_ref: String,
    /// The optional literal default.
    pub default: Option<DefaultValue>,
    /// The whole declaration.
    pub span: Span,
}

/// A command prompt.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandPrompt {
    /// The prompt name.
    pub name: String,
    /// The prompt kind as written (lowered through `PromptKind`).
    pub kind: String,
    /// The optional literal default.
    pub default: Option<DefaultValue>,
    /// The whole declaration.
    pub span: Span,
}

/// A value-position expression: the expression surface plus arrays and
/// objects.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueExpr {
    /// The shape.
    pub kind: ValueExprKind,
    /// The whole expression.
    pub span: Span,
}

/// The shape of a [`ValueExpr`].
#[derive(Debug, Clone, PartialEq)]
pub enum ValueExprKind {
    /// A bare identifier.
    Ident(String),
    /// A quoted string, unescaped.
    String(String),
    /// A number.
    Number(f64),
    /// `true` / `false`.
    Bool(bool),
    /// A call whose arguments are the expression surface.
    Call {
        /// The callee name.
        name: String,
        /// The callee name token's span.
        name_span: Span,
        /// The arguments.
        args: Vec<Expr>,
    },
    /// `object.field` (folded left).
    Field {
        /// The receiver.
        object: Box<ValueExpr>,
        /// The field name.
        field: String,
        /// The field name token's span.
        field_span: Span,
    },
    /// Parenthesized expression (preserved).
    Group(Box<ValueExpr>),
    /// A prefix operator.
    Unary {
        /// The operator.
        op: UnaryOp,
        /// The operand.
        operand: Box<ValueExpr>,
    },
    /// A binary operator.
    Binary {
        /// The left operand.
        left: Box<ValueExpr>,
        /// The operator.
        op: BinOp,
        /// The right operand.
        right: Box<ValueExpr>,
    },
    /// An array literal.
    Array(Vec<ValueExpr>),
    /// An object literal (`;`-separated members).
    Object(Vec<ObjectMember>),
}

/// One `key: value` object-literal member.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectMember {
    /// The member key.
    pub key: String,
    /// The member value (never an object literal).
    pub value: ValueExpr,
    /// Key through the value.
    pub span: Span,
}

/// A bound expression: the expression surface without arrays or objects.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    /// The shape.
    pub kind: ExprKind,
    /// The whole expression.
    pub span: Span,
}

/// The shape of an [`Expr`].
#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// A bare identifier.
    Ident(String),
    /// A quoted string, unescaped.
    StringLit(String),
    /// A number.
    NumLit(f64),
    /// `true` / `false`.
    BoolLit(bool),
    /// `object.field` (folded left).
    Field {
        /// The receiver.
        object: Box<Expr>,
        /// The field name.
        field: String,
        /// The field name token's span.
        field_span: Span,
    },
    /// A binary operator.
    Binary {
        /// The left operand.
        left: Box<Expr>,
        /// The operator.
        op: BinOp,
        /// The right operand.
        right: Box<Expr>,
    },
    /// A prefix operator.
    Unary {
        /// The operator.
        op: UnaryOp,
        /// The operand.
        operand: Box<Expr>,
    },
    /// Parenthesized expression (preserved).
    Group(Box<Expr>),
    /// A call.
    Call {
        /// The callee name.
        name: String,
        /// The callee name token's span.
        name_span: Span,
        /// The arguments.
        args: Vec<Expr>,
    },
}
