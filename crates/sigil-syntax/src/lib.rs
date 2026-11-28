//! The Rune (`.rosetta`) toolchain's syntax layer, now a thin crate over
//! [`phero_syntax::rune`] (RN9, ADR 0012 D2).
//!
//! The lexer, spanned AST and parser live in `phero-syntax`. What stays here
//! until RN11 is what depends on the semantic model or the filesystem:
//! - [`lower`] / [`lower_expression`]: AST → `phero_rune_model` (phero-syntax must
//!   not depend on the model);
//! - [`project`]: `.rosetta` discovery;
//! - the historical entry points [`parse`] / [`parse_expression_str`], which
//!   wrap [`phero_syntax::rune::parse_rune`] and report through
//!   `sigil_diag` (code `E0500`), so their callers are unchanged.

mod lower;
pub mod project;

pub use lower::{lower, lower_expression};
pub use phero_syntax::rune::ast;
pub use sigil_diag::{Diagnostic, SourceFile, Span};

use ast::{SourceUnit, SpannedExpr};

/// Parses one `.rosetta` file into its spanned AST plus syntax diagnostics.
///
/// **All or nothing:** the unit is `None` whenever any syntax error occurred,
/// even though [`phero_syntax::rune::parse_rune`] recovers a partial unit.
/// Every caller (resolution, the driver's `import sigil` bridge, project
/// loading, `sigil-lsp`) treats `Some` as "parsed, lower it", and lowering a
/// partial file would change which namespaces fail. Partial units are for the
/// phero editor tooling only.
pub fn parse(file: &SourceFile) -> (Option<SourceUnit>, Vec<Diagnostic>) {
    let result = phero_syntax::rune::parse_rune(&file.text);
    let ast = if result.errors.is_empty() {
        result.ast
    } else {
        None
    };
    let diagnostics = result
        .errors
        .into_iter()
        .map(|error| {
            Diagnostic::error(
                phero_diagnostics::E_RUNE_SYNTAX,
                error.message,
                file,
                error.span,
            )
        })
        .collect();
    (ast, diagnostics)
}

/// Parses one standalone Rune expression (the round-trip property test's
/// entry).
pub fn parse_expression_str(text: &str) -> Result<SpannedExpr, Vec<Diagnostic>> {
    let file = SourceFile::new("<expr>", text);
    phero_syntax::rune::parse_rune_expression(text).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| {
                Diagnostic::error(
                    phero_diagnostics::E_RUNE_SYNTAX,
                    error.message,
                    &file,
                    error.span,
                )
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_file_with_a_syntax_error_has_no_unit() {
        let file = super::SourceFile::new(
            "t.rosetta",
            "namespace t\ntype Good:\n\tx int (1..1)\ntype Broken\ntype Fine:\n\ty int (1..1)\n",
        );
        let (unit, diagnostics) = super::parse(&file);
        assert!(unit.is_none(), "the shim stays all-or-nothing");
        assert!(!diagnostics.is_empty());
    }
}
