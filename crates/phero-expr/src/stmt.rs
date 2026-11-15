//! The procedural statement IR ([`StmtBody`]) and its typechecker — issue
//! #178.
//!
//! [`StmtBody`] is a sequential statement IR beside
//! [`RuneBody`](crate::body::RuneBody): the Rune body is the sigil-lowered
//! guard/alias/`set` shape, while [`StmtBody`] is the closed procedural
//! subset design operations need — `let`, `let … else error …`,
//! `assign`, `guard … else error …`, `if`, and `return`, with every failure
//! path raising a declared error ([`BodyRaise`]) — the caller supplies the
//! raised vocabulary ([`RaiseSignature`]) and, when it wants the subset
//! enforced, the operation's `throws` set. The body is orchestration-only:
//! one primary aggregate, feature-path assignment, and calls to the
//! aggregate's own `op`s (DD7, issue pherolang#304).
//!
//! Expression slots are **phero-expr source text**, the same discipline as the
//! Rune body: phero-ir never depends on phero-expr, so the driver serializes a
//! checked body to one canonical JSON string (the derives below, byte-pinned
//! by the `tests/stmt.rs` shape tests) and re-parses/type-checks each slot
//! from its text at compile time. `.ddd` operations are the first consumer;
//! [`check_stmt_body`] is the whole contract.
//!
//! Typing and flow rules (all errors are collected in statement order, each
//! labeled with its slot):
//!
//! - **scope**: the operation's parameters are bound per the parameter rules
//!   (`List` for to-many, `Option` when the lower bound is 0), every
//!   [`StmtReceiver`] binds as a class-typed root (so `<receiver>.<op>(…)`
//!   type-checks against the synthetic repository class the caller
//!   registered), and `let`/`let … else` locals join the scope in declaration
//!   order — visible only to later statements. `if` branches check in child
//!   scopes: branch locals never escape.
//! - **assign**: `path[0]` must be a local (parameters and receivers are
//!   read-only); the remaining segments navigate class features
//!   (supertypes included), and the navigated slot unifies exactly with the
//!   assigned expression — no implicit conversions.
//! - **guard** conditions are `boolean`; a raise names one of the
//!   [`RaiseSignature`]s the caller provides (the operation's declared error
//!   vocabulary plus the standard prelude, whose `NotFound(resource, key)`
//!   and `Conflict(detail)` payloads are strings), its arguments type-check
//!   exactly against the declared payload (arity included), and — when the
//!   caller supplies the operation's `throws` set — every raised name must
//!   be declared there (explicit and complete, never inferred).
//! - **orchestration only**: the body orchestrates one aggregate; it never
//!   implements algorithms. A whole-local reassignment (`x = …`) is refused
//!   (assignment targets an aggregate feature path), and a non-primary
//!   receiver may not run `findById`/`findAll` (a body binds one primary
//!   repository), so one body never loads a second aggregate.
//! - **flow**: when the operation declares an output, every successful path
//!   must `return` — a `guard`/`let … else` failure is an *error* exit whose
//!   success continues, and an `if` exits only when both branches do.
//!   Falling off the end of the body is then an error labeled `body`. A
//!   void operation (no output) may fall through, and may not return a
//!   value. Statements after an exit are unreachable but not diagnosed.

use phero_ir::OperationParam;
use serde::{Deserialize, Serialize};

use crate::body::{BodyError, BodyOutput};
use crate::checker::{feature_in_class, feature_result_ty, result_ty, DomainTypes, TypeChecker};
use crate::types::Ty;

/// A sequential operation body: statements in execution order. Serialized as
/// the operation's body target's canonical JSON; an empty body is `{}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StmtBody {
    /// The statements, in execution order; omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stmts: Vec<Stmt>,
}

/// One statement of a [`StmtBody`]. Every expression slot is phero-expr source
/// text, parsed and type-checked at compile time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Stmt {
    /// `let name = expr;` — binds any value type.
    Let {
        /// The bound name.
        name: String,
        /// The initializer, phero-expr source text.
        expr: String,
    },
    /// `let name = expr else error V(args);` — `expr` must be
    /// `Option<T>`; the statement binds the unwrapped `T` on success and
    /// raises `error` when the value is absent.
    LetElse {
        /// The bound name.
        name: String,
        /// The optional initializer, phero-expr source text.
        expr: String,
        /// The raise taken when the value is absent.
        error: BodyRaise,
    },
    /// `path = expr;` — `path[0]` is a local, the rest navigate class
    /// features. The final slot unifies exactly with `expr`.
    Assign {
        /// The assignment path; the first segment is a local binding.
        path: Vec<String>,
        /// The assigned expression, phero-expr source text.
        expr: String,
    },
    /// `guard expr else error V(args);` — `expr` must be `boolean`.
    Guard {
        /// The guard condition, phero-expr source text.
        expr: String,
        /// The raise taken when the condition is false.
        error: BodyRaise,
    },
    /// `if cond { … } else { … }` — both branches are statement blocks
    /// checked in child scopes.
    If {
        /// The condition; must be `boolean`.
        cond: String,
        /// The then-branch.
        then: Vec<Stmt>,
        /// The else-branch; empty when the source omitted it.
        #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "else")]
        else_: Vec<Stmt>,
    },
    /// `return;` or `return expr;`
    Return {
        /// The returned expression; absent for a void return.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expr: Option<String>,
    },
}

/// A raised error: one declared error name plus its argument expressions
/// (phero-expr source text). The vocabulary is the caller's
/// [`RaiseSignature`] list — never hardcoded here; the standard prelude's
/// `NotFound(resource, key)` and `Conflict(detail)` come from
/// [`RaiseSignature::prelude`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyRaise {
    /// The error name (`NotFound`, `PaymentDeclined`, …).
    pub variant: String,
    /// The argument expressions, in written order; omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
}

/// One class-typed binding visible to a [`StmtBody`] beside the operation's
/// parameters: `self` or an injected repository. The caller registers the
/// synthetic class (with its operations) in the [`DomainTypes`] index under
/// `(package, class)`, so `<name>.<op>(…)` resolves through the ordinary
/// call machinery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StmtReceiver<'a> {
    /// The binding name in scope.
    pub name: &'a str,
    /// The synthetic repository class's package key in [`DomainTypes`].
    pub package: &'a str,
    /// The synthetic repository class's name in [`DomainTypes`].
    pub class: &'a str,
    /// `true` for the operation's primary repository (`self`): the one
    /// aggregate the body orchestrates. Loading through any other receiver
    /// (`findById`/`findAll`) is a cross-aggregate load and refused.
    pub is_primary: bool,
}

/// One declared error a body may raise (DD7, issue pherolang#304): its name
/// and typed payload fields, in declaration order. The driver builds one per
/// declared error in scope plus [`RaiseSignature::prelude`]'s two standard
/// signatures; the raise itself carries only the name and its argument
/// source text (phero-expr does not depend on the IR).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaiseSignature {
    /// The error name (`PaymentDeclined`, `NotFound`, …).
    pub name: String,
    /// The payload fields, in declaration order.
    pub fields: Vec<OperationParam>,
}

impl RaiseSignature {
    /// Creates a raise signature from its name and payload fields.
    pub fn new(name: impl Into<String>, fields: Vec<OperationParam>) -> Self {
        Self {
            name: name.into(),
            fields,
        }
    }

    /// The standard prelude's fixed signatures: `NotFound(resource: string,
    /// key: string)` and `Conflict(detail: string)`. Always available, never
    /// shadowable, and a prelude raise still has to appear in `throws`.
    pub fn prelude() -> Vec<Self> {
        let string = || phero_ir::TypeRef::Primitive(phero_ir::PrimitiveType::String);
        let field = |name: &str| OperationParam {
            name: name.to_string(),
            type_: string(),
            multiplicity: None,
        };
        vec![
            Self::new("NotFound", vec![field("resource"), field("key")]),
            Self::new("Conflict", vec![field("detail")]),
        ]
    }

    /// The declaration rendered for an error message:
    /// `NotFound(resource: string, key: string)`.
    fn describe(&self) -> String {
        format!(
            "{}({})",
            self.name,
            self.fields
                .iter()
                .map(|field| format!("{}: {}", field.name, Ty::from_type_ref(&field.type_)))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// Everything one body is checked against.
struct StmtCx<'a> {
    /// The class/type index.
    context: &'a DomainTypes,
    /// The declared output type, lifted per the return multiplicity; `None`
    /// for a void operation.
    output: Option<&'a Ty>,
    /// The parameter and receiver bindings, in declaration order.
    roots: Vec<(String, Ty)>,
    /// Parameter names (assignment roots are rejected).
    params: Vec<String>,
    /// Receiver names (assignment roots are rejected).
    receivers: Vec<String>,
    /// The non-primary receivers: `findById`/`findAll` through one is a
    /// cross-aggregate load. The class keys catch an alias
    /// (`let other = AuditRepository; other.findById(…)`), the names the
    /// direct call.
    secondary_receivers: Vec<String>,
    secondary_classes: Vec<Ty>,
    primary_classes: Vec<Ty>,
    /// The declared error vocabulary a raise may name, prelude included.
    raises: &'a [RaiseSignature],
    /// The operation's declared `throws` set; `None` disables the subset
    /// check (the crate's own focused tests), `Some(&[])` means no raise is
    /// allowed.
    throws: Option<&'a [&'a str]>,
}

/// Types a statement body against `context`: `params` are the operation's
/// parameters (lifted per the parameter rules), `output` the declared output
/// slot (absent for a void operation), `receivers` the class-typed bindings
/// the caller registered synthetically (the first primary), `raises` the
/// declared error vocabulary (prelude included), and `throws` the
/// operation's declared set — `None` skips the subset check, `Some(&[])`
/// allows no raise at all. Every error is reported per slot, in statement
/// order.
pub fn check_stmt_body(
    context: &DomainTypes,
    params: &[OperationParam],
    output: Option<BodyOutput<'_>>,
    receivers: &[StmtReceiver<'_>],
    raises: &[RaiseSignature],
    throws: Option<&[&str]>,
    body: &StmtBody,
) -> Result<(), Vec<BodyError>> {
    let output_ty = output.map(|output| result_ty(output.type_, output.multiplicity));
    let roots = params
        .iter()
        .map(|param| {
            (
                param.name.clone(),
                result_ty(&param.type_, param.multiplicity.as_ref()),
            )
        })
        .chain(receivers.iter().map(|receiver| {
            (
                receiver.name.to_string(),
                Ty::class(receiver.package, receiver.class),
            )
        }))
        .collect();
    let cx = StmtCx {
        context,
        output: output_ty.as_ref(),
        roots,
        params: params.iter().map(|param| param.name.clone()).collect(),
        receivers: receivers
            .iter()
            .map(|receiver| receiver.name.to_string())
            .collect(),
        secondary_receivers: receivers
            .iter()
            .filter(|receiver| !receiver.is_primary)
            .map(|receiver| receiver.name.to_string())
            .collect(),
        secondary_classes: receivers
            .iter()
            .filter(|receiver| !receiver.is_primary)
            .map(|receiver| Ty::class(receiver.package, receiver.class))
            .collect(),
        primary_classes: receivers
            .iter()
            .filter(|receiver| receiver.is_primary)
            .map(|receiver| Ty::class(receiver.package, receiver.class))
            .collect(),
        raises,
        throws,
    };

    let mut errors = Vec::new();
    let mut locals = Vec::new();
    let exits = cx.check_block(&body.stmts, &mut locals, &mut errors);
    if cx.output.is_some() && !exits {
        errors.push(BodyError {
            slot: "body".to_string(),
            message: "missing return: not every path returns a value, and the operation \
                      declares an output"
                .to_string(),
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

impl StmtCx<'_> {
    /// The bindings a statement sees: the roots plus every local whose
    /// initializer checked cleanly, in declaration order.
    fn scope(&self, locals: &[(String, Option<Ty>)]) -> Vec<(String, Ty)> {
        self.roots
            .iter()
            .cloned()
            .chain(
                locals
                    .iter()
                    .filter_map(|(name, ty)| ty.clone().map(|ty| (name.clone(), ty))),
            )
            .collect()
    }

    /// Checks a statement block, returning whether every path through it
    /// exits (returns). Statements after an exit are still checked (no
    /// unreachable-code diagnostics).
    fn check_block(
        &self,
        stmts: &[Stmt],
        locals: &mut Vec<(String, Option<Ty>)>,
        errors: &mut Vec<BodyError>,
    ) -> bool {
        let mut exits = false;
        for stmt in stmts {
            exits |= self.check_stmt(stmt, locals, errors);
        }
        exits
    }

    /// Checks one statement; `true` when it unconditionally exits the block.
    fn check_stmt(
        &self,
        stmt: &Stmt,
        locals: &mut Vec<(String, Option<Ty>)>,
        errors: &mut Vec<BodyError>,
    ) -> bool {
        match stmt {
            Stmt::Let { name, expr } => {
                let slot = format!("let '{name}'");
                let ty = self.type_in(&self.scope(locals), expr, &slot, errors);
                locals.push((name.clone(), ty));
                false
            }
            Stmt::LetElse { name, expr, error } => {
                let slot = format!("let-else '{name}'");
                let scope = self.scope(locals);
                let bound = match self.type_in(&scope, expr, &slot, errors) {
                    Some(Ty::Option(inner)) => Some(*inner),
                    Some(other) => {
                        errors.push(BodyError {
                            slot: slot.clone(),
                            message: format!(
                                "let-else requires an Option-typed initializer (the `else` \
                                 unwraps it), found {other}"
                            ),
                        });
                        None
                    }
                    None => None,
                };
                self.check_raise(&scope, error, &slot, errors);
                locals.push((name.clone(), bound));
                false
            }
            Stmt::Assign { path, expr } => {
                self.check_assign(path, expr, locals, errors);
                false
            }
            Stmt::Guard { expr, error } => {
                let slot = format!("guard '{expr}'");
                let scope = self.scope(locals);
                if let Some(ty) = self.type_in(&scope, expr, &slot, errors) {
                    if ty != Ty::boolean() {
                        errors.push(BodyError {
                            slot: slot.clone(),
                            message: format!("guard condition must be boolean, found {ty}"),
                        });
                    }
                }
                self.check_raise(&scope, error, &slot, errors);
                false
            }
            Stmt::If { cond, then, else_ } => {
                let slot = format!("if '{cond}'");
                let scope = self.scope(locals);
                if let Some(ty) = self.type_in(&scope, cond, &slot, errors) {
                    if ty != Ty::boolean() {
                        errors.push(BodyError {
                            slot,
                            message: format!("if condition must be boolean, found {ty}"),
                        });
                    }
                }
                // Branch locals never escape their branch.
                let mut then_locals = locals.clone();
                let then_exits = self.check_block(then, &mut then_locals, errors);
                let mut else_locals = locals.clone();
                let else_exits = self.check_block(else_, &mut else_locals, errors);
                // An empty `else` block never exits, so a missing `else`
                // correctly falls through.
                then_exits && else_exits
            }
            Stmt::Return { expr } => {
                match (self.output, expr) {
                    (Some(output_ty), Some(text)) => {
                        let scope = self.scope(locals);
                        if let Some(ty) = self.type_in(&scope, text, "return", errors) {
                            if ty != *output_ty {
                                errors.push(BodyError {
                                    slot: "return".to_string(),
                                    message: format!(
                                        "return types {ty} but the output slot is {output_ty}"
                                    ),
                                });
                            }
                        }
                    }
                    (Some(_), None) => errors.push(BodyError {
                        slot: "return".to_string(),
                        message: "a value-returning operation must return a value".to_string(),
                    }),
                    (None, Some(_)) => errors.push(BodyError {
                        slot: "return".to_string(),
                        message: "a void operation must not return a value".to_string(),
                    }),
                    (None, None) => {}
                }
                true
            }
        }
    }

    /// `path = expr;`: the root must be a local, the remaining segments walk
    /// class features (supertypes included), and the final slot unifies
    /// exactly with the assigned expression.
    fn check_assign(
        &self,
        path: &[String],
        expr: &str,
        locals: &[(String, Option<Ty>)],
        errors: &mut Vec<BodyError>,
    ) {
        let Some((root, rest)) = path.split_first() else {
            errors.push(BodyError {
                slot: "assign".to_string(),
                message: "assign requires a target path".to_string(),
            });
            return;
        };
        let slot = format!("assign '{}'", path.join("."));
        // Locals win: a `let` may shadow a parameter of the same name.
        let base = match locals.iter().rev().find(|(name, _)| name == root) {
            Some((_, Some(ty))) => ty.clone(),
            Some((_, None)) => {
                errors.push(BodyError {
                    slot,
                    message: format!("local '{root}' failed to check, so its type is unknown"),
                });
                return;
            }
            None if self.params.iter().any(|name| name == root) => {
                errors.push(BodyError {
                    slot,
                    message: format!(
                        "cannot assign to parameter '{root}': assignment targets a local \
                         binding introduced by `let`"
                    ),
                });
                return;
            }
            None if self.receivers.iter().any(|name| name == root) => {
                errors.push(BodyError {
                    slot,
                    message: format!(
                        "cannot assign to receiver '{root}': assignment targets a local \
                         binding introduced by `let`"
                    ),
                });
                return;
            }
            None => {
                errors.push(BodyError {
                    slot,
                    message: format!(
                        "unknown local '{root}': assignment targets a local binding introduced \
                         by `let`"
                    ),
                });
                return;
            }
        };

        if rest.is_empty() {
            errors.push(BodyError {
                slot,
                message: format!(
                    "cannot reassign the local '{root}': the body is orchestration-only and \
                     assigns an aggregate feature path (`{root}.<feature> = …`); move the \
                     computation into a domain `op`"
                ),
            });
            return;
        }
        let slot_ty = match self.navigate(base, rest, &slot, errors) {
            Some(ty) => ty,
            None => return,
        };
        let scope = self.scope(locals);
        let Some(expr_ty) = self.type_in(&scope, expr, &slot, errors) else {
            return;
        };
        if expr_ty != slot_ty {
            errors.push(BodyError {
                slot,
                message: format!("assign types {expr_ty} but the target slot is {slot_ty}"),
            });
        }
    }

    /// Validates one raise against the declared vocabulary: the name
    /// resolves, the operation's `throws` set contains it (when a set was
    /// supplied), and every argument types exactly against the declared
    /// payload field.
    fn check_raise(
        &self,
        scope: &[(String, Ty)],
        raise: &BodyRaise,
        slot: &str,
        errors: &mut Vec<BodyError>,
    ) {
        let Some(signature) = self
            .raises
            .iter()
            .find(|signature| signature.name == raise.variant)
        else {
            let declared: Vec<&str> = self
                .raises
                .iter()
                .map(|signature| signature.name.as_str())
                .collect();
            let declared = if declared.is_empty() {
                "no errors are declared in scope".to_string()
            } else {
                format!("declared: {}", declared.join(", "))
            };
            errors.push(BodyError {
                slot: slot.to_string(),
                message: format!(
                    "unknown error `{}`: declare it with `error {}(...)` or raise a \
                     prelude error (`NotFound`, `Conflict`); {declared}",
                    raise.variant, raise.variant
                ),
            });
            return;
        };
        if let Some(throws) = self.throws {
            if !throws.iter().any(|name| *name == raise.variant) {
                let declared = if throws.is_empty() {
                    "the operation declares no `throws` clause, so it raises no error".to_string()
                } else {
                    format!("declared: {}", throws.join(", "))
                };
                errors.push(BodyError {
                    slot: slot.to_string(),
                    message: format!(
                        "error `{}` is not declared in the operation's `throws`; {declared}",
                        raise.variant
                    ),
                });
            }
        }
        if raise.args.len() != signature.fields.len() {
            errors.push(BodyError {
                slot: slot.to_string(),
                message: format!(
                    "error `{}` expects {} argument{}, found {}: `{}`",
                    raise.variant,
                    signature.fields.len(),
                    if signature.fields.len() == 1 { "" } else { "s" },
                    raise.args.len(),
                    signature.describe()
                ),
            });
            return;
        }
        for (arg, field) in raise.args.iter().zip(&signature.fields) {
            let expected = result_ty(&field.type_, field.multiplicity.as_ref());
            if let Some(ty) = self.type_in(scope, arg, slot, errors) {
                if ty != expected {
                    errors.push(BodyError {
                        slot: slot.to_string(),
                        message: format!(
                            "error argument `{}` types {ty} but the declared payload \
                             field is {expected} (`{}`)",
                            field.name,
                            signature.describe()
                        ),
                    });
                }
            }
        }
    }

    /// The orchestration-only aggregate rule: a body binds one primary
    /// repository, so `findById`/`findAll` through any other receiver is a
    /// cross-aggregate load. Walks every expression slot's parsed form.
    fn check_aggregate_loads(
        &self,
        expr: &crate::Expr,
        scope: &[(String, Ty)],
        slot: &str,
        errors: &mut Vec<BodyError>,
    ) {
        let secondary: Vec<&str> = self
            .secondary_receivers
            .iter()
            .map(String::as_str)
            .collect();
        if secondary.is_empty() {
            return;
        }
        walk_calls(expr, &mut |receiver, name| {
            let crate::ast::ExprKind::Name(binding) = &receiver.kind else {
                return;
            };
            if !matches!(name.value.as_str(), "findById" | "findAll") {
                return;
            }
            // Direct call through a non-primary receiver, or an alias whose
            // binding carries a non-primary repository's class (the same
            // class as the primary is the same aggregate — an alias of the
            // primary is not a second aggregate).
            let aliased_secondary = scope
                .iter()
                .rev()
                .find(|(name, _)| name == binding)
                .map(|(_, ty)| {
                    self.secondary_classes.contains(ty) && !self.primary_classes.contains(ty)
                })
                .unwrap_or(false);
            if secondary.contains(&binding.as_str()) || aliased_secondary {
                errors.push(BodyError {
                    slot: slot.to_string(),
                    message: format!(
                        "`{}.{}` is a cross-aggregate load: the body is orchestration-only \
                         and binds one primary repository (`self`)",
                        binding, name.value
                    ),
                });
            }
        });
    }

    /// Walks a class feature path from `base`; `None` when a segment fails
    /// (the error is recorded).
    fn navigate(
        &self,
        base: Ty,
        path: &[String],
        slot: &str,
        errors: &mut Vec<BodyError>,
    ) -> Option<Ty> {
        let mut ty = base;
        for segment in path {
            let Ty::Named {
                kind: crate::types::NamedKind::Class,
                package,
                name: class,
            } = &ty
            else {
                errors.push(BodyError {
                    slot: slot.to_string(),
                    message: format!(
                        "cannot navigate '{segment}': the path so far types {ty}, not a class"
                    ),
                });
                return None;
            };
            match feature_in_class(self.context, package, class, segment) {
                Some(feature) => ty = feature_result_ty(&feature),
                None => {
                    errors.push(BodyError {
                        slot: slot.to_string(),
                        message: format!("unknown feature '{segment}' on class '{class}'"),
                    });
                    return None;
                }
            }
        }
        Some(ty)
    }

    /// Parses and types one expression slot under `scope`, recording every
    /// failure as a [`BodyError`] on `slot`. Returns the expression's type
    /// when it checked cleanly.
    fn type_in(
        &self,
        scope: &[(String, Ty)],
        text: &str,
        slot: &str,
        errors: &mut Vec<BodyError>,
    ) -> Option<Ty> {
        let parsed = crate::parse(text);
        if let Some(error) = parsed.errors.first() {
            errors.push(BodyError {
                slot: slot.to_string(),
                message: error.message.clone(),
            });
            return None;
        }
        let ast = parsed.ast?;
        self.check_aggregate_loads(&ast, scope, slot, errors);
        let mut checker = TypeChecker::new(self.context.clone());
        for (name, ty) in scope {
            checker = checker.with_binding(name.clone(), ty.clone());
        }
        match checker.type_of(&ast) {
            Ok(ty) => Some(ty),
            Err(mut expr_errors) => {
                let message = if expr_errors.len() == 1 {
                    expr_errors.pop().expect("one error").message
                } else {
                    expr_errors
                        .into_iter()
                        .map(|error| error.message)
                        .collect::<Vec<_>>()
                        .join("; ")
                };
                errors.push(BodyError {
                    slot: slot.to_string(),
                    message,
                });
                None
            }
        }
    }
}

/// Walks every call node of an expression, handing each call's receiver
/// expression and written operation name to `visit`. Used by the
/// orchestration-only aggregate rule; the walk covers every expression
/// shape, so a call nested in an `if`, a lambda, or a binary operand is
/// found too.
fn walk_calls(
    expr: &crate::Expr,
    visit: &mut dyn FnMut(&crate::Expr, &crate::ast::Spanned<String>),
) {
    use crate::ast::ExprKind;
    match &expr.kind {
        ExprKind::Call {
            receiver,
            name,
            args,
            ..
        } => {
            visit(receiver, name);
            walk_calls(receiver, visit);
            for arg in args {
                walk_calls(arg, visit);
            }
        }
        ExprKind::FeatureAccess { receiver, .. } => walk_calls(receiver, visit),
        ExprKind::Coalesce { value, default } => {
            walk_calls(value, visit);
            walk_calls(default, visit);
        }
        ExprKind::FunctionCall { args, .. } => {
            for arg in args {
                walk_calls(arg, visit);
            }
        }
        ExprKind::Algebra {
            receiver, lambda, ..
        } => {
            walk_calls(receiver, visit);
            if let Some((_, body)) = lambda {
                walk_calls(body, visit);
            }
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            walk_calls(lhs, visit);
            walk_calls(rhs, visit);
        }
        ExprKind::Unary { expr, .. } => walk_calls(expr, visit),
        ExprKind::If { cond, then, else_ } => {
            walk_calls(cond, visit);
            walk_calls(then, visit);
            walk_calls(else_, visit);
        }
        ExprKind::Let { init, body, .. } => {
            walk_calls(init, visit);
            walk_calls(body, visit);
        }
        ExprKind::Lambda { body, .. } => walk_calls(body, visit),
        ExprKind::ListLiteral(items) => {
            for item in items {
                walk_calls(item, visit);
            }
        }
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::String(_)
        | ExprKind::Bool(_)
        | ExprKind::Null
        | ExprKind::Date { .. }
        | ExprKind::Name(_) => {}
    }
}
