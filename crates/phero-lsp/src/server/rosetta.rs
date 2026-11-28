//! `import sigil` diagnostics in the editor (pherolang#398, RN7 of epic
//! pherolang#391).
//!
//! A `.mox` compile returns the diagnostics of its `import sigil`
//! declarations apart from its own ([`phero_driver::Compiled::sigil_diagnostics`]),
//! each tagged with where it belongs — the CLI routes them the same way
//! (`phero-cli/src/report.rs`):
//!
//! - a tag equal to the `.mox` document's driver key is an **import-level**
//!   diagnostic (a missing provider entry, a namespace collision): it
//!   publishes on the `.mox` document itself, against its own text;
//! - any other tag names a **rosetta content** file (the import path as
//!   written for a named import, the disk path for a discovered candidate):
//!   it publishes on that file's URI, against that file's text, **and** the
//!   `.mox` document gets one anchor diagnostic per broken rosetta file on
//!   the `import sigil` declaration that brought it in — the model was
//!   dropped because of it, so the `.mox` must say why.
//!
//! `.rosetta` is not a phero surface: nothing else publishes on its URI, so
//! this module owns those publishes. Several `.mox` documents may import
//! one rosetta file, so its published set is the union over every owning
//! `.mox` ([`RosettaDiagnostics`]); recompiling or closing an owner
//! republishes exactly the rosetta URIs whose set it touched.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use tower_lsp_server::ls_types::*;

use super::pipeline::to_lsp_diagnostic;
use crate::position::PositionMap;

/// The rosetta diagnostics currently published, per rosetta URI and per
/// owning `.mox` document.
#[derive(Default)]
pub(super) struct RosettaDiagnostics {
    by_file: BTreeMap<Uri, BTreeMap<Uri, Vec<Diagnostic>>>,
}

impl RosettaDiagnostics {
    /// Replaces everything `owner` contributes with `fresh` and returns the
    /// rosetta URIs whose published set may have changed (every file the
    /// owner contributed to before or contributes to now), each with its
    /// new union.
    pub(super) fn replace_owner(
        &mut self,
        owner: &Uri,
        fresh: Vec<(Uri, Vec<Diagnostic>)>,
    ) -> Vec<(Uri, Vec<Diagnostic>)> {
        let mut touched: BTreeSet<Uri> = BTreeSet::new();
        for (file, owners) in &mut self.by_file {
            if owners.remove(owner).is_some() {
                touched.insert(file.clone());
            }
        }
        for (file, diagnostics) in fresh {
            touched.insert(file.clone());
            if !diagnostics.is_empty() {
                self.by_file
                    .entry(file)
                    .or_default()
                    .insert(owner.clone(), diagnostics);
            }
        }
        self.by_file.retain(|_, owners| !owners.is_empty());
        touched
            .into_iter()
            .map(|file| {
                let union = self.union_for(&file);
                (file, union)
            })
            .collect()
    }

    /// The published set of one rosetta file: every owner's diagnostics,
    /// deduplicated (two `.mox` files importing one broken rosetta report
    /// the same parse error once).
    pub(super) fn union_for(&self, file: &Uri) -> Vec<Diagnostic> {
        let mut union: Vec<Diagnostic> = Vec::new();
        for diagnostics in self.by_file.get(file).into_iter().flat_map(|o| o.values()) {
            for diagnostic in diagnostics {
                if !union.contains(diagnostic) {
                    union.push(diagnostic.clone());
                }
            }
        }
        union
    }
}

/// The routed `import sigil` diagnostics of one `.mox` compile.
pub(super) struct RoutedSigilDiagnostics {
    /// Diagnostics for the `.mox` document itself: the import-level ones
    /// plus one anchor per broken rosetta file.
    pub(super) own: Vec<Diagnostic>,
    /// Content diagnostics per rosetta URI, in that file's coordinates.
    /// Every rosetta file the compile was handed appears, clean ones with an
    /// empty vector, so a fixed file's stale set clears.
    pub(super) rosetta: Vec<(Uri, Vec<Diagnostic>)>,
}

/// Routes one `.mox` compile's `sigil_diagnostics`.
///
/// `driver_key` is the key the `.mox` compiled under, `disk_path` its file
/// on disk (imports resolve relative to it), `text`/`map` its source, and
/// `imports` the bundle the compile was handed (the rosetta texts the
/// content diagnostics' spans index into).
pub(super) fn route_sigil_diagnostics(
    driver_key: &str,
    disk_path: Option<&Path>,
    text: &str,
    map: &PositionMap,
    imports: &phero_driver::DomainImports,
    diagnostics: &[(String, phero_driver::Diagnostic)],
) -> RoutedSigilDiagnostics {
    let directory = disk_path
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let declarations = sigil_declarations(text, &directory);

    let mut own = Vec::new();
    // Every provided rosetta file, so a clean one clears its stale set.
    let mut by_tag: BTreeMap<String, Vec<&phero_driver::Diagnostic>> = imports
        .sigil
        .entries()
        .iter()
        .filter(|(mox, _, _)| mox == driver_key)
        .map(|(_, tag, _)| (tag.clone(), Vec::new()))
        .collect();
    for (tag, diagnostic) in diagnostics {
        if tag == driver_key {
            own.push(to_lsp_diagnostic(diagnostic, map));
        } else {
            by_tag.entry(tag.clone()).or_default().push(diagnostic);
        }
    }

    let mut rosetta = Vec::new();
    for (tag, content) in by_tag {
        let path = normalize(&directory.join(&tag));
        let Some(uri) = Uri::from_file_path(&path) else {
            continue;
        };
        let source = imports.sigil.get(driver_key, &tag).unwrap_or_default();
        let rosetta_map = PositionMap::new(source);
        let published: Vec<Diagnostic> = content
            .iter()
            .map(|diagnostic| to_lsp_diagnostic(diagnostic, &rosetta_map))
            .collect();
        let errors: Vec<(&phero_driver::Diagnostic, &Diagnostic)> = content
            .iter()
            .zip(&published)
            .filter(|(diagnostic, _)| diagnostic.is_error())
            .map(|(diagnostic, published)| (*diagnostic, published))
            .collect();
        if let Some((first, first_published)) = errors.first() {
            if let Some(span) = anchor_for(&declarations, &tag, &path) {
                own.push(anchor_diagnostic(
                    map.range_for(span),
                    &display_tag(&tag, &directory),
                    errors.len(),
                    first,
                    Location {
                        uri: uri.clone(),
                        range: first_published.range,
                    },
                ));
            }
        }
        rosetta.push((uri, published));
    }
    RoutedSigilDiagnostics { own, rosetta }
}

/// One `import sigil` declaration of the `.mox` source: its span, the path
/// as written, and the directory the import's candidate pool is drawn from.
struct SigilDeclaration {
    span: phero_syntax::Span,
    written: String,
    pool: PathBuf,
}

fn sigil_declarations(text: &str, directory: &Path) -> Vec<SigilDeclaration> {
    let Some(ast) = phero_syntax::parse(text).ast else {
        return Vec::new();
    };
    ast.declarations
        .iter()
        .filter_map(|declaration| match declaration {
            phero_syntax::ast::Decl::ImportSchema(import)
                if import.kind == phero_syntax::ast::ImportKind::Sigil =>
            {
                let resolved = normalize(&directory.join(&import.path));
                Some(SigilDeclaration {
                    span: import.span,
                    written: import.path.clone(),
                    pool: resolved.parent().map(Path::to_path_buf).unwrap_or_default(),
                })
            }
            _ => None,
        })
        .collect()
}

/// The declaration a broken rosetta file is anchored at: the import that
/// names it, else the first import whose candidate pool contains it, else
/// the first `import sigil` of the file.
fn anchor_for(
    declarations: &[SigilDeclaration],
    tag: &str,
    path: &Path,
) -> Option<phero_syntax::Span> {
    declarations
        .iter()
        .find(|declaration| declaration.written == tag)
        .or_else(|| {
            declarations
                .iter()
                .find(|declaration| path.starts_with(&declaration.pool))
        })
        .or_else(|| declarations.first())
        .map(|declaration| declaration.span)
}

/// The anchor on the `.mox` import: names the rosetta file, the error
/// count, and the first error, and links to it. It carries no code — it is
/// an editor presentation of the rosetta file's own diagnostics, which keep
/// theirs.
fn anchor_diagnostic(
    range: Range,
    file: &str,
    errors: usize,
    first: &phero_driver::Diagnostic,
    location: Location,
) -> Diagnostic {
    let plural = if errors == 1 { "error" } else { "errors" };
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some(super::SOURCE_NAME.to_string()),
        message: format!(
            "imported rosetta file '{file}' has {errors} {plural}; the first: {}",
            first.message
        ),
        related_information: Some(vec![DiagnosticRelatedInformation {
            location,
            message: first.message.clone(),
        }]),
        tags: None,
        data: None,
    }
}

/// A rosetta tag as the user wrote it: a named import keeps its written
/// path; a discovered candidate (a disk path) shows relative to the `.mox`
/// directory when it lives under it.
fn display_tag(tag: &str, directory: &Path) -> String {
    let path = Path::new(tag);
    if !path.is_absolute() {
        return tag.to_string();
    }
    let base = normalize(directory);
    normalize(path)
        .strip_prefix(&base)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| tag.to_string())
}

/// The canonical path where it exists (so `a/../b.rosetta` and a
/// discovered `b.rosetta` name one URI), else the path unchanged.
fn normalize(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
