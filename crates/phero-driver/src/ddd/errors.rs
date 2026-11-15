//! The declared error vocabulary (DD7, issue pherolang#304): `error`
//! declarations at application and module scope, their visibility rules,
//! and the resolution of `throws` clauses.
//!
//! The table lowers **before** services and designs, because every `throws`
//! clause and every body raise resolves against it. Each declaration carries
//! its typed payload (resolved through the same signature helper the
//! operation signatures use). The standard prelude — `NotFound(resource,
//! key)` and `Conflict(detail)` — is implicit: it is never emitted in the
//! artifact's `errors` lists, is visible everywhere, and cannot be shadowed.
//!
//! Rules:
//!
//! - error names are application-wide unique (a duplicate is `E0374`);
//! - a user declaration may not shadow a prelude name (`E0374`);
//! - a payload field name is unique per declaration (`E0378`);
//! - a payload field may not embed a class designed as an entity (`E0376`,
//!   checked in the deferred pass once the designs are known);
//! - application-scoped errors are visible everywhere; a module-scoped error
//!   only inside its module (`E0375` names the declaring module and the
//!   application-scope fix); the prelude is always visible.

use std::collections::BTreeMap;

use phero_diagnostics::ddd::{
    E_DUPLICATE_ERROR, E_DUPLICATE_ERROR_FIELD, E_ERROR_PAYLOAD_ENTITY, E_MODULE_PRIVATE_ERROR,
    E_UNKNOWN_THROWN_ERROR,
};
use phero_expr::stmt::RaiseSignature;
use phero_ir::ddd::{DddParam, ErrorDecl, ErrorRef, ErrorScope};
use phero_syntax::ast as dsl;
use phero_syntax::ast::{is_prelude_error, PRELUDE_ERRORS};
use phero_syntax::Span;

use crate::diagnostic::Diagnostic;
use crate::lower::DomainPackage;

use super::dtos::{self, DesignPrep};
use super::*;

/// One lowered declaration of the vocabulary.
pub(crate) struct ErrorEntry {
    /// The declared name.
    pub(crate) name: String,
    /// Application or module scope (never the prelude — that is implicit).
    pub(crate) scope: ErrorScope,
    /// The declaring module, for a module-scoped entry.
    pub(crate) module: Option<String>,
    /// The lowered declaration (payload included).
    pub(crate) decl: ErrorDecl,
    /// The declaration name's span.
    pub(crate) span: Span,
}

/// The application's complete error vocabulary, in declaration order:
/// application scope first, then each module's private declarations.
pub(crate) struct ErrorTable {
    entries: Vec<ErrorEntry>,
    application: Vec<ErrorDecl>,
    modules: BTreeMap<String, Vec<ErrorDecl>>,
}

impl ErrorTable {
    /// The application-scoped declarations, in source order — the artifact's
    /// root `errors` list.
    pub(crate) fn application_errors(&self) -> &[ErrorDecl] {
        &self.application
    }

    /// One module's private declarations, in source order — its artifact
    /// `errors` list (empty when it declares none).
    pub(crate) fn module_errors(&self, module: &str) -> &[ErrorDecl] {
        self.modules.get(module).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The declared names visible to `module`: the prelude, every
    /// application-scoped error, and the module's own errors, in that order.
    fn visible_names(&self, module: &str) -> Vec<&str> {
        let mut names: Vec<&str> = PRELUDE_ERRORS.iter().map(|(name, _)| *name).collect();
        for entry in &self.entries {
            match entry.scope {
                ErrorScope::Application => names.push(&entry.name),
                ErrorScope::Module if entry.module.as_deref() == Some(module) => {
                    names.push(&entry.name)
                }
                _ => {}
            }
        }
        names
    }

    /// The declared error vocabulary of one module's operations, as the body
    /// checker consumes it: the prelude's fixed signatures plus every
    /// declared error visible in the module.
    pub(crate) fn raise_signatures(&self, module: &str) -> Vec<RaiseSignature> {
        let mut signatures = RaiseSignature::prelude();
        for entry in &self.entries {
            let visible = match entry.scope {
                ErrorScope::Application => true,
                ErrorScope::Module => entry.module.as_deref() == Some(module),
                ErrorScope::Prelude => false,
            };
            if visible {
                signatures.push(RaiseSignature::new(
                    entry.name.clone(),
                    dtos::body_params(&entry.decl.fields),
                ));
            }
        }
        signatures
    }

    /// Resolves one `throws` entry in `module`, recording `E0373`/`E0375`
    /// when it does not resolve. `context` names the operation
    /// (`operation 'pay' of service 'PaymentService'`).
    pub(crate) fn resolve_throw(
        &self,
        name: &dsl::Name,
        module: &str,
        context: &str,
        local: &mut Vec<Diagnostic>,
    ) -> Option<ErrorRef> {
        if is_prelude_error(&name.text) {
            return Some(ErrorRef::prelude(name.text.clone()));
        }
        let Some(entry) = self.entries.iter().find(|entry| entry.name == name.text) else {
            let declared = self.visible_names(module);
            let candidates: std::collections::BTreeSet<String> =
                declared.iter().map(|name| (*name).to_string()).collect();
            let suggestion = phero_diagnostics::did_you_mean_adaptive(&name.text, &candidates);
            let help = match &suggestion {
                Some(suggestion) => format!("did you mean '{suggestion}'?"),
                None => format!("errors in scope: {}", declared.join(", ")),
            };
            local.push(
                Diagnostic::error(
                    format!(
                        "{E_UNKNOWN_THROWN_ERROR} unknown error '{}' in throws of {context}",
                        name.text
                    ),
                    Some(name.span),
                )
                .with_help(help),
            );
            return None;
        };
        match entry.scope {
            ErrorScope::Application => Some(ErrorRef::application(name.text.clone())),
            ErrorScope::Module if entry.module.as_deref() == Some(module) => {
                Some(ErrorRef::module(name.text.clone(), module))
            }
            ErrorScope::Module => {
                let owning = entry.module.clone().unwrap_or_default();
                local.push(
                    Diagnostic::error(
                        format!(
                            "{E_MODULE_PRIVATE_ERROR} error '{}' is private to module \
                             '{owning}', so {context} in module '{module}' cannot name it",
                            name.text
                        ),
                        Some(name.span),
                    )
                    .with_help(format!(
                        "declare '{}' at application scope (move the `error` declaration \
                         out of module '{owning}') to use it across modules",
                        name.text
                    )),
                );
                None
            }
            ErrorScope::Prelude => Some(ErrorRef::prelude(name.text.clone())),
        }
    }

    /// The payload types of every declared error (the deferred entity-payload
    /// pass reads them against the completed design registry).
    pub(crate) fn payload_checks(&self) -> Vec<PayloadCheck<'_>> {
        let mut checks = Vec::new();
        for entry in &self.entries {
            for field in &entry.decl.fields {
                checks.push(PayloadCheck {
                    error: &entry.name,
                    field: &field.name,
                    type_: &field.type_,
                });
            }
        }
        checks
    }
}

/// One payload slot awaiting the entity-payload check (`E0376`).
pub(crate) struct PayloadCheck<'a> {
    /// The declaring error.
    pub(crate) error: &'a str,
    /// The field name.
    pub(crate) field: &'a str,
    /// The resolved field type.
    pub(crate) type_: &'a phero_ir::ddd::DddTypeRef,
}

/// Collects and validates the application's error declarations: lowers every
/// payload, enforces the application-wide uniqueness and the prelude
/// no-shadow rule (`E0374`), the per-declaration field uniqueness (`E0378`),
/// and returns the table the `throws` clauses and body checks resolve
/// against. The entity-payload rule (`E0376`) is deferred to
/// [`check_entity_payloads`] — the designs are not lowered yet.
pub(crate) fn collect_errors(
    application: &dsl::DddApplication,
    packages: &[DomainPackage],
    base: Option<&str>,
    prep: &DesignPrep,
    local: &mut Vec<Diagnostic>,
) -> ErrorTable {
    let mut table = ErrorTable {
        entries: Vec::new(),
        application: Vec::new(),
        modules: BTreeMap::new(),
    };
    let mut declared: Vec<(String, Span, String)> = Vec::new();
    for decl in &application.errors {
        if let Some(lowered) = lower_error_decl(
            decl,
            None,
            packages,
            base,
            &prep.dto_names,
            &mut declared,
            local,
        ) {
            table.entries.push(lowered);
        }
    }
    table.application = table
        .entries
        .iter()
        .filter(|entry| entry.scope == ErrorScope::Application)
        .map(|entry| entry.decl.clone())
        .collect();
    for module in &application.modules {
        let mut lowered = Vec::new();
        for decl in &module.errors {
            if let Some(entry) = lower_error_decl(
                decl,
                Some(&module.name.text),
                packages,
                base,
                &prep.dto_names,
                &mut declared,
                local,
            ) {
                table.entries.push(entry);
            }
        }
        lowered.extend(
            table
                .entries
                .iter()
                .filter(|entry| {
                    entry.scope == ErrorScope::Module
                        && entry.module.as_deref() == Some(module.name.text.as_str())
                })
                .map(|entry| entry.decl.clone()),
        );
        table.modules.insert(module.name.text.clone(), lowered);
    }
    table
}

/// Lowers one declaration, reporting the uniqueness rules. `declared`
/// carries the application-wide names seen so far and their owning context
/// (the application, or `module '<name>'`).
fn lower_error_decl(
    decl: &dsl::DddErrorDecl,
    module: Option<&str>,
    packages: &[DomainPackage],
    base: Option<&str>,
    dto_names: &[String],
    declared: &mut Vec<(String, Span, String)>,
    local: &mut Vec<Diagnostic>,
) -> Option<ErrorEntry> {
    let owner = module
        .map(|module| format!("module '{module}'"))
        .unwrap_or_else(|| "this application".to_string());
    let mut keep = true;
    if PRELUDE_ERRORS
        .iter()
        .any(|(prelude, _)| *prelude == decl.name.text)
    {
        local.push(
            Diagnostic::error(
                format!(
                    "{E_DUPLICATE_ERROR} error '{}' shadows a prelude error",
                    decl.name.text
                ),
                Some(decl.name.span),
            )
            .with_help(format!(
                "the standard prelude declares `{}`; its shape is fixed and always \
                 available — choose another name for a domain error",
                prelude_shape(&decl.name.text)
            )),
        );
        keep = false;
    } else if let Some((_, _, previous)) =
        declared.iter().find(|(name, _, _)| *name == decl.name.text)
    {
        local.push(
            Diagnostic::error(
                format!("{E_DUPLICATE_ERROR} duplicate error '{}'", decl.name.text),
                Some(decl.name.span),
            )
            .with_help(format!(
                "'{}' is already declared in {previous}; error names are \
                 application-wide unique",
                decl.name.text
            )),
        );
        keep = false;
    } else {
        declared.push((decl.name.text.clone(), decl.name.span, owner.clone()));
    }

    let mut fields = Vec::new();
    let mut field_names: Vec<&str> = Vec::new();
    for field in &decl.fields {
        if field_names.contains(&field.name.text.as_str()) {
            local.push(
                Diagnostic::error(
                    format!(
                        "{E_DUPLICATE_ERROR_FIELD} duplicate field '{}' in error '{}'",
                        field.name.text, decl.name.text
                    ),
                    Some(field.name.span),
                )
                .with_help(
                    "a payload field name is unique per error; a consumer builds one \
                     field per name in declaration order",
                ),
            );
            continue;
        }
        field_names.push(&field.name.text);
        let (type_, multiplicity) = signature_type(
            &field.type_ref,
            field.multiplicity.as_ref(),
            packages,
            base,
            dto_names,
            local,
        );
        fields.push(DddParam::new(field.name.text.clone(), type_).with_multiplicity(multiplicity));
    }

    keep.then(|| ErrorEntry {
        name: decl.name.text.clone(),
        scope: module.map_or(ErrorScope::Application, |_| ErrorScope::Module),
        module: module.map(str::to_string),
        decl: ErrorDecl::new(decl.name.text.clone())
            .with_description(decl.doc.clone())
            .with_fields(fields),
        span: decl.name.span,
    })
}

/// The standard prelude's rendered shape, for the no-shadow help line.
fn prelude_shape(name: &str) -> String {
    PRELUDE_ERRORS
        .iter()
        .find(|(prelude, _)| *prelude == name)
        .map(|(prelude, fields)| {
            format!(
                "{prelude}({})",
                fields
                    .iter()
                    .map(|(field, type_)| format!("{field}: {type_}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .unwrap_or_else(|| name.to_string())
}

/// `E0376` — an error payload is data: a field whose resolved type is a
/// class designed as an `entity` in this application is refused. Runs after
/// the design stage (the entity registry is complete then), one diagnostic
/// per offending field.
pub(crate) fn check_entity_payloads(
    table: &ErrorTable,
    entity_designs: &[EntityDesign],
) -> Vec<Diagnostic> {
    let mut local = Vec::new();
    for check in table.payload_checks() {
        let phero_ir::ddd::DddTypeRef::Domain(phero_ir::TypeRef::Class { package, name }) =
            check.type_
        else {
            continue;
        };
        if entity_designs
            .iter()
            .any(|design| design.package == *package && design.class == *name)
        {
            local.push(
                Diagnostic::error(
                    format!(
                        "{E_ERROR_PAYLOAD_ENTITY} error '{}' field '{}' carries entity \
                         '{}.{}'; an error payload is data",
                        check.error, check.field, package, name
                    ),
                    Some(
                        table
                            .entries
                            .iter()
                            .find(|entry| entry.name == check.error)
                            .map(|entry| entry.span)
                            .unwrap_or_else(|| Span::from(0..0)),
                    ),
                )
                .with_help(
                    "refer to the entity by id, embed a `value` object, or carry a DTO \
                     boundary — an error crosses boundaries with its payload unchanged",
                ),
            );
        }
    }
    local
}
