//! DD9 typed repository queries (issue pherolang#306): the `where`/`sort`/
//! `page` clauses of a declared repository operation.
//!
//! A query is **read-only by construction**. This module owns the three
//! things the artifact cannot infer later:
//!
//! 1. **Read-only shape** — no statement body, no `when` guard (the
//!    predicate is `where`; two predicates would be ambiguous), no `throws`
//!    (a finder raises no declared domain error). Each refusal is `E0396`.
//! 2. **Predicate typing** — the predicate is phero-expr, typed against the
//!    designed class plus the operation's parameters, and must be boolean.
//!    Bare features and `self.<feature>` both resolve; a parameter that
//!    collides with a feature is an error with the `self.` help, because a
//!    silent shadow in a selection predicate is a data-leak bug. `E0397`.
//! 3. **Sort and page** — a sort key must be a **direct stored scalar**
//!    feature (a class reference needs a join, a collection has no stored
//!    scalar order, a derived feature cannot be indexed) `E0398`; `limit`/
//!    `max` are positive page sizes with `limit <= max`, and `cursor`
//!    selects key-set pagination `E0399`.
//!
//! The predicate text stays verbatim in the artifact (the `when`-condition
//! rule): typing is proven here, so the consumer never re-types it — it
//! lowers the same expression to the store's query language and **never**
//! string-interpolates a parameter value.

use phero_diagnostics::ddd::{E_QUERY_PAGE, E_QUERY_PREDICATE, E_QUERY_READ_ONLY, E_QUERY_SORT};
use phero_ir::ddd::{DddParam, QueryPagination, QuerySortKey, QuerySpec, SortDirection};
use phero_ir::TypeRef;

use crate::diagnostic::Diagnostic;

use super::class_features;

/// The scope a query operation's clauses check against.
pub(crate) struct QueryCheck<'a> {
    /// The human context the messages embed ("operation 'x' of repository
    /// 'Y'").
    pub(crate) label: String,
    /// The combined design source (for slicing the predicate).
    pub(crate) source: &'a str,
    /// The designed class, when the design resolved one.
    pub(crate) design_class: Option<&'a (String, String)>,
    /// The fully lowered union; `None` when an imported domain failed to
    /// lower (the feature checks then defer — the domain diagnostics already
    /// block the artifact).
    pub(crate) union: Option<&'a phero_ir::Model>,
    /// The shared expression context; `None` under the same deferral.
    pub(crate) context: Option<&'a phero_expr::DomainTypes>,
    /// The operation's declared parameters.
    pub(crate) params: &'a [DddParam],
}

/// Lowers one query operation's `where`/`sort`/`page` clauses, or `None` when
/// the operation is not a query. Errors land in `local`; the caller drops the
/// artifact on any error, so a partial `QuerySpec` never ships.
pub(crate) fn lower_query(
    operation: &phero_syntax::ast::DddRepositoryOp,
    check: QueryCheck<'_>,
    local: &mut Vec<Diagnostic>,
) -> Option<QuerySpec> {
    let query = operation.query.as_ref()?;
    let label = check.label.as_str();
    let mut ok = true;

    // --- read-only shape (E0396) --------------------------------------------
    if operation.body.is_some() {
        local.push(Diagnostic::error(
            format!("{E_QUERY_READ_ONLY} {label} carries a statement body; a query is read-only"),
            Some(operation.span),
        ));
        ok = false;
    }
    if operation.condition.is_some() {
        local.push(Diagnostic::error(
            format!(
                "{E_QUERY_READ_ONLY} {label} takes a `when` guard; a query's predicate is `where`"
            ),
            operation.condition,
        ));
        ok = false;
    }
    if !operation.throws.is_empty() {
        local.push(Diagnostic::error(
            format!(
                "{E_QUERY_READ_ONLY} {label} declares `throws`; a query raises no declared error"
            ),
            Some(operation.throws[0].span),
        ));
        ok = false;
    }

    // --- predicate (E0397) --------------------------------------------------
    let predicate = crate::conditions::when_interior(check.source, query.predicate).to_string();
    if predicate.trim().is_empty() {
        local.push(Diagnostic::error(
            format!(
                "{E_QUERY_PREDICATE} invalid query predicate in {label}: the predicate is empty"
            ),
            Some(query.predicate),
        ));
        ok = false;
    }

    let class = check.design_class;
    let features = class.and_then(|(package, name)| {
        check
            .union
            .and_then(|union| class_features(union, package, name))
    });

    // A parameter that shadows a feature makes `name` ambiguous in a
    // selection predicate — the silent mix-up is a data-leak bug, so it is
    // an error rather than a shadowing rule.
    if let (Some(class), Some(features)) = (class, features.as_ref()) {
        for param in check.params {
            if features.iter().any(|feature| feature.name == param.name) {
                local.push(
                    Diagnostic::error(
                        format!(
                            "{E_QUERY_PREDICATE} query parameter '{}' in {label} collides with \
                             a feature of '{}'",
                            param.name, class.1
                        ),
                        Some(query.predicate),
                    )
                    .with_help(format!(
                        "write `self.{}` for the feature, or rename the parameter",
                        param.name
                    )),
                );
                ok = false;
            }
        }
    }

    if let (Some((package, class_name)), Some(_)) = (class, check.context) {
        ok &= check_predicate(
            &predicate,
            query.predicate,
            package,
            class_name,
            &check,
            local,
        );
    }

    // --- sort (E0398) -------------------------------------------------------
    let mut sort: Vec<QuerySortKey> = Vec::new();
    for key in &query.sort {
        let Some(features) = features.as_ref() else {
            // Deferred: the union did not lower, so there is no feature
            // table to resolve against. The domain diagnostics block the
            // artifact anyway.
            break;
        };
        let Some(feature) = features
            .iter()
            .find(|feature| feature.name == key.field.text)
        else {
            local.push(Diagnostic::error(
                format!(
                    "{E_QUERY_SORT} invalid sort key '{}' in {label}: the designed class has no \
                     such feature",
                    key.field.text
                ),
                Some(key.span),
            ));
            ok = false;
            continue;
        };
        let reason = if feature.is_derived {
            Some("the feature is derived (not stored, so it cannot be indexed)")
        } else if feature.multiplicity.is_many() {
            Some("the feature is a collection (no stored scalar order)")
        } else {
            match feature.type_ {
                TypeRef::Class { .. } | TypeRef::Interface { .. } => {
                    Some("the feature is a class reference (ordering needs a join)")
                }
                _ => None,
            }
        };
        if let Some(reason) = reason {
            local.push(Diagnostic::error(
                format!(
                    "{E_QUERY_SORT} invalid sort key '{}' in {label}: {reason}",
                    key.field.text
                ),
                Some(key.span),
            ));
            ok = false;
            continue;
        }
        sort.push(QuerySortKey {
            field: key.field.text.clone(),
            field_type: feature.type_.clone(),
            direction: match key.direction {
                Some(phero_syntax::ast::DddSortDirection::Desc) => SortDirection::Desc,
                _ => SortDirection::Asc,
            },
        });
    }

    // --- page (E0399) -------------------------------------------------------
    let pagination = query.page.as_ref().map(|page| {
        let mut limit: Option<u32> = None;
        let mut max: Option<u32> = None;
        for (value, slot, name) in [
            (page.limit, &mut limit, "limit"),
            (page.max, &mut max, "max"),
        ] {
            let Some(value) = value else {
                continue;
            };
            match u32::try_from(value) {
                Ok(value) if value > 0 => *slot = Some(value),
                _ => {
                    local.push(Diagnostic::error(
                        format!(
                            "{E_QUERY_PAGE} invalid pagination in {label}: {name} must be a \
                             positive page size, found {value}"
                        ),
                        Some(page.span),
                    ));
                    ok = false;
                }
            }
        }
        if let (Some(limit), Some(max)) = (limit, max) {
            if limit > max {
                local.push(
                    Diagnostic::error(
                        format!(
                            "{E_QUERY_PAGE} invalid pagination in {label}: limit {limit} exceeds \
                             max {max}"
                        ),
                        Some(page.span),
                    )
                    .with_help("raise `max`, lower `limit`, or drop one of them"),
                );
                ok = false;
            }
        }
        QueryPagination {
            limit,
            max,
            cursor: page.cursor,
        }
    });

    ok.then_some(QuerySpec {
        predicate,
        sort,
        pagination,
        read_only: true,
    })
}

/// Types the predicate: parse, bind `self` plus the parameters, and require
/// boolean. Returns `false` when any diagnostic was emitted.
fn check_predicate(
    predicate: &str,
    span: phero_syntax::Span,
    package: &str,
    class: &str,
    check: &QueryCheck<'_>,
    local: &mut Vec<Diagnostic>,
) -> bool {
    let label = check.label.as_str();
    // Rebases an expression-relative span into the combined design source,
    // capped inside the parentheses (the `when` skeleton's rule).
    let interior_start = span.start + 1;
    let cap = span.end.saturating_sub(1).min(check.source.len());
    let rebase = |relative: phero_expr::ast::Span| -> phero_syntax::Span {
        let start = (interior_start + relative.start).min(cap);
        let end = (interior_start + relative.end).min(cap);
        (start..end).into()
    };
    let parsed = phero_expr::parse(predicate);
    let mut ok = true;
    for error in &parsed.errors {
        local.push(Diagnostic::error(
            format!(
                "{E_QUERY_PREDICATE} invalid query predicate in {label}: {}",
                error.message
            ),
            Some(rebase(error.span)),
        ));
        ok = false;
    }
    let Some(ast) = parsed.ast else {
        return false;
    };
    let Some(context) = check.context else {
        return ok;
    };
    let mut checker = phero_expr::TypeChecker::new(context.clone())
        .with_self(package, class)
        // `self.<feature>` is the explicit spelling; the expression language
        // has no `self` keyword, so the query scope binds it as a class-typed
        // root and ordinary feature access does the rest.
        .with_binding("self", phero_expr::Ty::class(package, class));
    for param in check.params {
        let mut ty = super::dtos::expr_ty(&param.type_);
        if param.multiplicity.as_ref().is_some_and(|m| m.is_many()) {
            ty = ty.list();
        }
        checker = checker.with_binding(param.name.clone(), ty);
    }
    match checker.type_of(&ast) {
        Err(errors) => {
            for error in errors {
                local.push(Diagnostic::error(
                    format!(
                        "{E_QUERY_PREDICATE} invalid query predicate in {label}: {}",
                        error.message
                    ),
                    Some(rebase(error.span)),
                ));
            }
            false
        }
        Ok(ty) if ty != phero_expr::Ty::boolean() => {
            local.push(Diagnostic::error(
                format!(
                    "{E_QUERY_PREDICATE} invalid query predicate in {label}: predicate must be \
                     boolean, found {ty}"
                ),
                Some(rebase(ast.span)),
            ));
            false
        }
        Ok(_) => ok,
    }
}
