//! Aggregate/value invariants (DD5, issue pherolang#302): parse, type-check,
//! and lower the verbatim `invariant <expr>;` clauses against the designed
//! class as the implicit `self`.
//!
//! The typing contract is the guard contract: the expression must be boolean,
//! own/inherited stored features and derived features are readable, machine
//! states bind as enum-typed names, optional navigation follows R3, and there
//! is **no deferral** when the domain union lowered. A repository or global
//! read is an unknown name with a targeted help.

use phero_diagnostics::ddd::{E_DTO_INVARIANT, E_INVALID_INVARIANT};
use phero_syntax::ast as dsl;

use crate::diagnostic::Diagnostic;

use super::machines::MachineScopes;

/// Type-checks one design's invariants and returns the rebuilt artifacts'
/// expressions (already lowered by the caller) plus diagnostics.
pub(crate) fn check_design_invariants(
    design: &dsl::DddDesign,
    design_source: &str,
    resolved: Option<&(String, String)>,
    context: Option<&phero_expr::TypeContext>,
    machines: &MachineScopes,
    local: &mut Vec<Diagnostic>,
) {
    for span in &design.invariants {
        let text = design_source.get(span.start..span.end).unwrap_or("").trim();
        if text.is_empty() {
            local.push(
                Diagnostic::error(
                    format!(
                        "{E_INVALID_INVARIANT} invalid invariant in design '{}': the \
                         expression is empty",
                        design.class.text
                    ),
                    Some(*span),
                )
                .with_help("write a boolean expression, e.g. `total >= 0`"),
            );
            continue;
        }
        if design.stereotype == dsl::DddStereotype::Dto {
            local.push(
                Diagnostic::error(
                    format!(
                        "{E_DTO_INVARIANT} design '{}' is a dto; a transport shape carries \
                         no invariants",
                        design.class.text
                    ),
                    Some(*span),
                )
                .with_help(
                    "remove the invariant, or design the state owner as an `entity`/`value`",
                ),
            );
            continue;
        }
        let Some((package, class)) = resolved else {
            // Rule 3 already failed; the artifact is dropped.
            continue;
        };
        let Some(context) = context else {
            // A failed domain blocks the artifact; no deferred typing.
            continue;
        };
        let parsed = phero_expr::parse(text);
        for error in &parsed.errors {
            local.push(
                Diagnostic::error(
                    format!(
                        "{E_INVALID_INVARIANT} invalid invariant in design '{}': {}",
                        design.class.text, error.message
                    ),
                    Some(rebase(span.start, error.span)),
                )
                .with_help(invariant_help()),
            );
        }
        let Some(ast) = parsed.ast else {
            continue;
        };
        let mut checker = phero_expr::TypeChecker::new(context.clone()).with_self(package, class);
        // Machine states bind as enum-typed names (`status == Draft`), exactly
        // as they do in guards.
        for machine in machines
            .machines
            .iter()
            .filter(|machine| machine.class.0 == *package && machine.class.1 == *class)
        {
            let binding = phero_expr::Ty::from_type_ref(&phero_ir::TypeRef::Enum {
                package: machine.enum_type.0.clone(),
                name: machine.enum_type.1.clone(),
            });
            for state in &machine.states {
                checker = checker.with_binding(state.clone(), binding.clone());
            }
        }
        match checker.type_of(&ast) {
            Err(errors) => {
                for error in errors {
                    local.push(
                        Diagnostic::error(
                            format!(
                                "{E_INVALID_INVARIANT} invalid invariant in design '{}': {}",
                                design.class.text, error.message
                            ),
                            Some(rebase(span.start, error.span)),
                        )
                        .with_help(invariant_help()),
                    );
                }
            }
            Ok(ty) if ty != phero_expr::Ty::boolean() => {
                local.push(
                    Diagnostic::error(
                        format!(
                            "{E_INVALID_INVARIANT} invalid invariant in design '{}': must be \
                             boolean, found {ty}",
                            design.class.text
                        ),
                        Some(*span),
                    )
                    .with_help(invariant_help()),
                );
            }
            Ok(_) => {}
        }
    }
}

/// Rebase an expression-relative span onto the host source.
fn rebase(base: usize, span: phero_expr::ast::Span) -> phero_syntax::Span {
    (base + span.start..base + span.end).into()
}

/// The one help line every invariant typing failure carries.
fn invariant_help() -> &'static str {
    "an invariant reads only the designed class's state; cross-aggregate \
     consistency belongs in a process manager or an operation guard"
}
