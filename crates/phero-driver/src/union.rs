//! The shared domain-union pipeline: [`import_matches`] (origin-relative
//! import resolution) and [`lower_domains`] — the lowering of every
//! imported domain against one union namespace that the `.actor`, `.ddd`,
//! `.evt`, `.dmn`, `.uc`, and `.cmmn` entry points share.

use salsa::Database as Db;

use crate::lower;
use crate::{parse_query, Diagnostic, DomainImports, ParseOutput, SourceFile};

/// Matches an origin-file import against a provided file: the exact import
/// string wins (the historical contract — [`compile_actors_str`]
/// callers pass the paths they were given), with a lexical resolution of the
/// import relative to the origin file's directory as the fallback (the CLI
/// passes resolved paths so vocabulary snapshots locate the declaring
/// file's `vocab/` directory regardless of the process CWD). Shared by the
/// actor pipeline and the `.deploy` binding resolution.
pub(crate) fn import_matches(import: &str, origin_path: &str, candidate: &str) -> bool {
    fn normalize(path: impl AsRef<std::path::Path>) -> std::path::PathBuf {
        let mut out = std::path::PathBuf::new();
        for component in path.as_ref().components() {
            match component {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    out.pop();
                }
                other => out.push(other.as_os_str()),
            }
        }
        out
    }
    if candidate == import {
        return true;
    }
    let base = std::path::Path::new(origin_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new(""));
    normalize(base.join(import)) == normalize(candidate)
}
/// The domain lowering shared by the `.actor` and `.ddd` pipelines: resolve
/// the origin file's `import` declarations against the provided domain
/// files, lower every named domain against the **union namespace of all
/// imports** (the same rule multi-file `.mox` compiles follow), and assemble
/// the per-domain results the actor/design tails consume.
pub(crate) struct LoweredDomains {
    /// One unit per imported domain (imports in first-appearance order).
    pub(crate) domain_units: Vec<lower::DomainUnit>,
    /// The union model of all imported domains — only meaningful when every
    /// domain lowered successfully. The first sigil-declaring domain carries
    /// the compilation's synthetic packages. Cedar class lookup and `.ddd`
    /// design resolution consume this instead of recompiling the domains
    /// standalone (which cannot resolve cross-package references).
    pub(crate) domains_model: Option<phero_ir::Model>,
    /// Sigil content diagnostics keyed by rosetta path (rendered against
    /// the retained rosetta texts); each caller appends them after its own
    /// diagnostics, following the domain ones.
    pub(crate) sigil_diagnostics: Vec<(String, Diagnostic)>,
    /// Every `.mox` import string, **as written**, that resolved to a
    /// provided domain (deduplicated). The design prologue's "was not
    /// provided" check reads this instead of the lowered units, so a second
    /// spelling of one file (`"a.mox"` from the root, `"../a.mox"` from a
    /// fragment) is provided too.
    pub(crate) provided_imports: Vec<String>,
}

/// One file whose `import` declarations name domains: the file's resolved
/// path (the base for semantic import resolution) and its import list.
///
/// A `.ddd` application root and each imported **fragment** are distinct
/// origins (DD8, issue pherolang#305): an import string resolves relative to
/// the directory of the file that declares it, so merging the fragment
/// imports under the root's path would silently resolve them against the
/// wrong directory.
pub(crate) struct DomainOrigin<'a> {
    /// The declaring file's resolved path.
    pub path: &'a str,
    /// The file's own `import` declarations.
    pub imports: &'a [phero_syntax::Import],
}

/// The body shared by [`compile_actors_with`] and [`compile_ddd_with`]:
/// resolve `imports` against `domains` (deduplicated by **resolved domain
/// path**, in first-appearance order; only imported domains are compiled —
/// extras are ignored so their diagnostics do not leak), lower every named
/// domain against the shared union namespace, and collect the results.
///
/// A domain with errors yields no model while the remaining domains keep
/// resolving, so the tails can still bind capabilities and resolve designs
/// against the surviving domains.
pub(crate) fn lower_domains(
    db: &dyn Db,
    origin_path: &str,
    imports: &[phero_syntax::Import],
    domains: &[SourceFile],
    provided: &DomainImports,
) -> LoweredDomains {
    lower_domains_from(
        db,
        &[DomainOrigin {
            path: origin_path,
            imports,
        }],
        domains,
        provided,
    )
}

/// [`lower_domains`] over several origins — a `.ddd` root plus its imported
/// fragments. Each import resolves against the directory of the file that
/// declares it; two spellings (or two files) that name the same domain
/// lower it **once**, keyed by the resolved domain path.
pub(crate) fn lower_domains_from(
    db: &dyn Db,
    origins: &[DomainOrigin<'_>],
    domains: &[SourceFile],
    provided: &DomainImports,
) -> LoweredDomains {
    // `(import as written, resolved file)` — the written spelling names the
    // unit (the design prologue's provision check and domain-diagnostic
    // rendering both key by it, the historical contract), the resolved path
    // deduplicates. A second spelling of one file is provided but not
    // lowered twice.
    let mut lookups: Vec<(String, SourceFile)> = Vec::new();
    let mut provided_imports: Vec<String> = Vec::new();
    for origin in origins {
        for import in origin.imports {
            // `.dmn` imports are not domains — they name the design's
            // decision models, which the host provides as pre-compiled
            // `DmnModel`s (the decisions bucket). A plain compile simply
            // ignores them; treating them as `.mox` sources would mis-parse
            // the artifact. A `.ddd` import is a fragment, merged by the
            // entry point before this runs.
            if import.path.ends_with(".dmn") || import.path.ends_with(".ddd") {
                continue;
            }
            if let Some(domain) = domains
                .iter()
                .find(|file| import_matches(&import.path, origin.path, &file.path(db)))
            {
                if !provided_imports.contains(&import.path) {
                    provided_imports.push(import.path.clone());
                }
                if !lookups
                    .iter()
                    .any(|(_, existing)| existing.path(db) == domain.path(db))
                {
                    lookups.push((import.path.clone(), *domain));
                }
            }
        }
    }

    let mut paths: Vec<String> = Vec::new();
    let mut sources: Vec<String> = Vec::new();
    let mut parse_outputs: Vec<ParseOutput> = Vec::new();
    for (_, domain) in &lookups {
        paths.push(domain.path(db));
        sources.push(domain.text(db));
        parse_outputs.push(parse_query(db, *domain));
    }
    let units: Vec<lower::MultiFile<'_>> = lookups
        .iter()
        .enumerate()
        .map(|(index, _)| lower::MultiFile {
            path: &paths[index],
            source: &sources[index],
            ast: parse_outputs[index].ast.as_ref(),
            parse_diagnostics: &parse_outputs[index].diagnostics,
        })
        .collect();
    let compilations = lower::compile_union_per_file(&units, provided);

    let sigil_diagnostics: Vec<(String, Diagnostic)> = compilations
        .iter()
        .flat_map(|compilation| compilation.sigil_diagnostics.iter().cloned())
        .collect();

    let domains_model = if compilations
        .iter()
        .all(|compilation| compilation.model.is_some())
    {
        let mut union = phero_ir::Model::new();
        for compilation in &compilations {
            if let Some(model) = &compilation.model {
                union.packages.extend(model.packages.clone());
            }
        }
        Some(union)
    } else {
        None
    };

    let domain_units: Vec<lower::DomainUnit> = lookups
        .into_iter()
        .zip(sources)
        .zip(parse_outputs)
        .zip(compilations)
        .map(
            |((((path, _), source), parse), compilation)| lower::DomainUnit {
                path,
                source,
                model: compilation.model,
                ast: parse.ast,
                diagnostics: compilation.diagnostics,
            },
        )
        .collect();

    LoweredDomains {
        domain_units,
        domains_model,
        sigil_diagnostics,
        provided_imports,
    }
}
/// The one-shot wrappers' domain sources: each `(path, source)` pair as a
/// salsa input file, duplicate paths read once (first occurrence wins).
pub(crate) fn domain_files(db: &dyn Db, domains: &[(String, String)]) -> Vec<SourceFile> {
    let mut seen: Vec<&str> = Vec::new();
    domains
        .iter()
        .filter(|(path, _)| {
            if seen.contains(&path.as_str()) {
                return false;
            }
            seen.push(path.as_str());
            true
        })
        .map(|(path, source)| SourceFile::new(db, path.clone(), source.clone()))
        .collect()
}
