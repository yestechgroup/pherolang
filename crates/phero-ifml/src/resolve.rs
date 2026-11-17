//! Cross-file module resolution for `.ifml` compilations.
//!
//! [`crate::parse_ifml`] parses one file; this module is the *compilation*
//! stage on top: it parses the main file, pulls in every `.ifml` import
//! through the [`IfmlImports`] bundle (the house-style import bundle —
//! mirroring `phero_driver::DomainImports`, the driver stays
//! filesystem-free), builds the module table (local modules shadow
//! imported ones), and resolves/validates every `use` statement in the
//! main file against it.
//!
//! The returned [`IfmlCompilation`] carries the main file's model
//! **unchanged** — imports verbatim, local modules only — plus the
//! [`IfmlIndex`] side-table, where resolution results live
//! ([`ModuleUseSite::resolved_module`]). The wire artifact stays
//! byte-identical; nothing a `use` references is ever rewritten into it.
//!
//! # Determinism
//!
//! Imports resolve in source order, depth-first; each file is parsed once
//! (a second import of the same path is skipped, but a path that re-enters
//! the active import stack is a cycle). Module names resolve: main-file
//! declarations win (duplicates within a file are an error, first
//! declaration wins); among imports, the first file to declare a name
//! claims it and later distinct files declaring it are ambiguous. A locally
//! shadowed name is claimed silently by the main file — imported
//! declarations of that name are ignored entirely.
//!
//! # Diagnostic codes
//!
//! The resolver's diagnostic codes (`E0001`–`E0009`) are allocated in the
//! central registry — the `phero-diagnostics` crate's `ifml_resolver` band,
//! re-exported below — and their meanings, fixes, and the allocation
//! ledger live in
//! [docs/DIAGNOSTICS.md](https://github.com/yestechgroup/pherolang/blob/main/docs/DIAGNOSTICS.md)
//! (`pherolang explain E0004` renders one). A new code is a table row
//! there, never a local constant.
//!
//! # The record and its file tag
//!
//! `.ifml` emits the workspace's one diagnostic record
//! ([`phero_diagnostics::Diagnostic<&'static str>`]) — no type of its own
//! (SK2 of epic pherolang#328, ADR 0005). A compilation spans several
//! files, so results are tagged `(path, Diagnostic)` exactly like every
//! driver surface's compilation struct: the tag owns the coordinate system,
//! the record's byte span indexes that file's text, and `code` carries the
//! stable registry code (`E0002`) so a host prints it without parsing the
//! message. An import-level finding with no natural span carries `None`
//! and the importing file's path.
//!
//! `compile_ifml_str` over a file with no imports and no uses produces zero
//! diagnostics.

use std::collections::{BTreeMap, BTreeSet};

use phero_diagnostics::Diagnostic;
use phero_ir::ifml::IfmlModel;

use crate::index::{IfmlIndex, LiteralKind, ModuleDeclSite, UseOverride};

pub use phero_diagnostics::ifml_resolver::{
    E_AMBIGUOUS_MODULE, E_CIRCULAR, E_DUPLICATE_MODULE, E_IMPORT_NOT_PROVIDED, E_INPUT_TYPE,
    E_MISSING_INPUT, E_PARSE, E_UNKNOWN_INPUT, E_UNKNOWN_MODULE,
};

/// The import content provided to a compilation: the `.ifml` source of every
/// import, keyed `(importing-file path, import path as written)`. The
/// resolver stays filesystem-free — hosts read the files and thread the
/// bundle through, exactly like `phero_driver::DomainImports`.
///
/// The empty bundle ([`IfmlImports::default`]) is the no-import
/// compilation: an `.ifml` import without a provider entry is the
/// ``imported ifml '<path>' was not provided`` error. Imports not ending in
/// `.ifml` (e.g. `.actor`) pass through untouched — a downstream concern.
///
/// ```
/// use phero_ifml::IfmlImports;
///
/// let mut imports = IfmlImports::default();
/// imports.provide("app.ifml", "pager.ifml", "module \"Pagination\" { input { page: Int } output { total: Int } }");
/// assert_eq!(imports.get("app.ifml", "pager.ifml"), Some("module \"Pagination\" { input { page: Int } output { total: Int } }"));
/// assert_eq!(imports.get("other.ifml", "pager.ifml"), None);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IfmlImports {
    imports: BTreeMap<(String, String), String>,
}

impl IfmlImports {
    /// Provides the source text of one import: `import_path` as written in
    /// `importing_path`'s `import "..."` statement. Re-providing the same
    /// key overwrites.
    pub fn provide(&mut self, importing_path: &str, import_path: &str, text: impl Into<String>) {
        self.imports.insert(
            (importing_path.to_string(), import_path.to_string()),
            text.into(),
        );
    }

    /// Every provided import as `((importing path, import path), text)`,
    /// in insertion order — hosts that post-process the whole bundle (the
    /// expansion's module table) walk it through here.
    pub fn iter(&self) -> impl Iterator<Item = ((&str, &str), &str)> {
        self.imports.iter().map(|((importing, import), text)| {
            ((importing.as_str(), import.as_str()), text.as_str())
        })
    }

    /// The provided text for one import, or `None`.
    pub fn get(&self, importing_path: &str, import_path: &str) -> Option<&str> {
        self.imports
            .get(&(importing_path.to_string(), import_path.to_string()))
            .map(String::as_str)
    }
}

/// A one-shot `.ifml` compilation: the main file's parsed model (unchanged,
/// artifact-identical to [`crate::parse_ifml`]) plus the span index with
/// all resolution results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfmlCompilation {
    /// The main file's own parsed model — imports verbatim, local modules
    /// only.
    pub model: IfmlModel,
    /// The span index for the whole compilation; `files[0]` is the main
    /// file. Use-site resolution lives on
    /// [`crate::ModuleUseSite::resolved_module`].
    pub index: IfmlIndex,
}

/// The result of walking one `.ifml` entry file's import tree: every
/// `.ifml` source in the walk as `(identity, text)` — the entry file
/// first, then each import under its as-written path, depth-first in
/// source order. Identities are unique (each file is walked once).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IfmlImportWalk {
    /// All walked `.ifml` sources, entry file first.
    pub sources: Vec<(String, String)>,
}

/// Walks one `.ifml` file's import tree without touching the filesystem —
/// the shared traversal behind every host that gathers `.ifml` sources
/// (the CLI's disk reader, test harnesses, the LSP). `fetch` is called for
/// **every** `import "..."` statement with the importing file's identity
/// and the import string as written; returning `Some((key, text))`
/// recurses into it as an `.ifml` file, returning `None` skips it
/// (non-`.ifml` imports and unreadable files are host concerns — collect
/// `.mox` domains through the same callback's side effects). The walk is
/// depth-first in source order; an identity already fully processed is
/// skipped and an identity re-entering the active path is a cycle the
/// resolver will diagnose — both mirror [`compile_ifml_str`]'s semantics,
/// so bundle keys built from this walk always match what the resolver
/// looks up.
///
/// The fetch returns the **host-resolved key** alongside the text — the
/// identity the host actually resolved the import under (a disk path, a
/// document URI). The walker itself only uses the text for recursion and
/// [`IfmlImportWalk::sources`]; importer identities stay the as-written
/// import strings, so bundle keys are unchanged. But hosts must uphold one
/// obligation for every resolved key they hand out: **a later fetch call
/// may use that resolved key as the importer identity** — nested gathering
/// (an imported `.actor` policy's own `.mox` imports, a deploy flow's
/// bridged walk) resolves relative to what the host handed back, so hosts
/// must map every key they hand out the same way they map the entry
/// identity. A host that cannot resolve an importer identity returns
/// `None`, and the gathering layer records the import as unresolved.
///
/// ```
/// use phero_ifml::walk_ifml_imports;
///
/// let walk = walk_ifml_imports("app.ifml", "import \"pager.ifml\";", &mut |importer, import| {
///     if importer == "app.ifml" && import == "pager.ifml" {
///         Some(("pager.ifml".to_string(), "view \"Pager\" {}".to_string()))
///     } else {
///         None
///     }
/// });
/// assert_eq!(walk.sources.len(), 2);
/// assert_eq!(walk.sources[0].0, "app.ifml");
/// assert_eq!(walk.sources[1].0, "pager.ifml");
/// ```
#[allow(clippy::type_complexity)]
pub fn walk_ifml_imports(
    entry_identity: &str,
    entry_source: &str,
    fetch: &mut dyn FnMut(&str, &str) -> Option<(String, String)>,
) -> IfmlImportWalk {
    let mut walk = IfmlImportWalk::default();
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut active: Vec<String> = Vec::new();
    walk_file(
        entry_identity,
        entry_source,
        fetch,
        &mut walk,
        &mut visited,
        &mut active,
    );
    walk
}

#[allow(clippy::type_complexity)]
fn walk_file(
    identity: &str,
    source: &str,
    fetch: &mut dyn FnMut(&str, &str) -> Option<(String, String)>,
    walk: &mut IfmlImportWalk,
    visited: &mut BTreeSet<String>,
    active: &mut Vec<String>,
) {
    if visited.contains(identity) || active.iter().any(|seen| seen == identity) {
        return;
    }
    visited.insert(identity.to_string());
    active.push(identity.to_string());
    walk.sources
        .push((identity.to_string(), source.to_string()));
    let imports = match crate::parser::parse_file_recovering(source).0 {
        Some(file) => file
            .imports
            .iter()
            .map(|import| import.path.clone())
            .collect(),
        None => Vec::new(),
    };
    for import in imports {
        // The host-resolved key is information for the gathering layer
        // (dedup, nested resolution); the walker itself needs only the
        // text, and identities stay the as-written import strings.
        if let Some((_resolved_key, text)) = fetch(identity, &import) {
            walk_file(&import, &text, fetch, walk, visited, active);
        }
    }
    active.pop();
}

/// Compiles in-memory `.ifml` source in one call: parse the main file,
/// resolve its `.ifml` imports through `imports`, build the module table,
/// and resolve/validate every `use` in the main file. Returns the model +
/// index, or every diagnostic found (compilation order).
///
/// ```
/// use phero_ifml::{compile_ifml_str, IfmlImports};
///
/// let compilation = compile_ifml_str("app.ifml", "view \"Home\" { component \"g\" { type: list; data: Item; } }", &IfmlImports::default())
///     .expect("compiles with no imports");
/// assert_eq!(compilation.model.views.len(), 1);
/// assert!(compilation.index.module_uses.is_empty());
/// ```
pub fn compile_ifml_str(
    path: &str,
    text: &str,
    imports: &IfmlImports,
) -> Result<IfmlCompilation, Vec<(String, Diagnostic<&'static str>)>> {
    let (compilation, diagnostics) = compile_ifml_lenient(path, text, imports);
    if diagnostics.is_empty() {
        Ok(compilation)
    } else {
        Err(diagnostics)
    }
}

/// [`compile_ifml_str`] without the all-or-nothing contract: the
/// compilation (possibly failing validation, with its import index intact —
/// module declarations from imported files are present even when their
/// resolution failed) always comes back alongside its diagnostics. Hosts
/// that degrade gracefully — the LSP navigating broken code — navigate the
/// index even while diagnostics are live.
pub fn compile_ifml_lenient(
    path: &str,
    text: &str,
    imports: &IfmlImports,
) -> (IfmlCompilation, Vec<(String, Diagnostic<&'static str>)>) {
    let mut diags = Vec::new();
    let ((model, mut index), issues) = crate::parser::parse_model_recovering_with_path(path, text);
    for issue in issues {
        diags.push((
            path.to_string(),
            Diagnostic::error(issue.message, Some(issue.span)).with_code(E_PARSE),
        ));
    }

    let mut active = vec![path.to_string()];
    let mut visited: BTreeSet<String> = BTreeSet::from([path.to_string()]);
    resolve_imports(
        path,
        &model.imports,
        imports,
        &mut active,
        &mut visited,
        &mut index,
        &mut diags,
    );

    resolve_module_uses(&mut index, &mut diags);

    (IfmlCompilation { model, index }, diags)
}

/// Depth-first, source-order import resolution: parse each `.ifml` import's
/// provided text, absorb its index, and recurse into its own imports.
/// Non-`.ifml` imports pass through untouched; cycles and missing content
/// are diagnostics that skip the file.
fn resolve_imports(
    importing_path: &str,
    import_paths: &[String],
    imports: &IfmlImports,
    active: &mut Vec<String>,
    visited: &mut BTreeSet<String>,
    index: &mut IfmlIndex,
    diags: &mut Vec<(String, Diagnostic<&'static str>)>,
) {
    for import_path in import_paths {
        if !import_path.ends_with(".ifml") {
            continue;
        }
        if active.iter().any(|seen| seen == import_path) {
            diags.push((
                importing_path.to_string(),
                Diagnostic::error(
                    format!("circular ifml import involving '{import_path}'"),
                    None,
                )
                .with_code(E_CIRCULAR),
            ));
            continue;
        }
        if visited.contains(import_path) {
            continue;
        }
        let Some(import_text) = imports.get(importing_path, import_path) else {
            diags.push((
                importing_path.to_string(),
                Diagnostic::error(
                    format!("imported ifml '{import_path}' was not provided"),
                    None,
                )
                .with_code(E_IMPORT_NOT_PROVIDED),
            ));
            continue;
        };
        let ((import_model, import_index), issues) =
            crate::parser::parse_model_recovering_with_path(import_path, import_text);
        for issue in issues {
            diags.push((
                import_path.to_string(),
                Diagnostic::error(issue.message, Some(issue.span)).with_code(E_PARSE),
            ));
        }
        let file = index.files.len();
        index.absorb(file, import_index);
        active.push(import_path.clone());
        visited.insert(import_path.clone());
        resolve_imports(
            import_path,
            &import_model.imports,
            imports,
            active,
            visited,
            index,
            diags,
        );
        active.pop();
    }
}

/// Builds the module table from the index (main file first, then imported
/// files in compilation order) and resolves/validates every use site in the
/// main file against it. Resolution results are written back into
/// `index.module_uses`.
pub(crate) fn resolve_module_uses(
    index: &mut IfmlIndex,
    diags: &mut Vec<(String, phero_diagnostics::Diagnostic<&'static str>)>,
) {
    let main_path = index.files[0].path.clone();

    // name -> (file, decl index within that file)
    let mut table: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut local_names: BTreeSet<String> = BTreeSet::new();
    let mut ambiguous: BTreeSet<String> = BTreeSet::new();

    let mut decls_by_file: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (global, decl) in index.module_decls.iter().enumerate() {
        decls_by_file.entry(decl.file).or_default().push(global);
    }

    for (&file, globals) in &decls_by_file {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for (position, &global) in globals.iter().enumerate() {
            let name = index.module_decls[global].name.clone();
            if !seen.insert(name.clone()) {
                let span = index.module_decls[global].span;
                diags.push((
                    index.files[file].path.clone(),
                    Diagnostic::error(
                        format!("duplicate module '{name}'"),
                        Some((span.0..span.1).into()),
                    )
                    .with_code(E_DUPLICATE_MODULE),
                ));
                continue;
            }
            if file == 0 {
                local_names.insert(name.clone());
                table.insert(name, (0, position));
            } else if local_names.contains(&name) {
                // The main file's module shadows imported ones silently.
                continue;
            } else if ambiguous.contains(&name) {
                continue;
            } else if let Some(&(first_file, _)) = table.get(&name) {
                let first_path = index.files[first_file].path.clone();
                let span = index.module_decls[global].span;
                diags.push((
                    index.files[file].path.clone(),
                    Diagnostic::error(
                        format!(
                            "ambiguous module '{name}' (declared in '{first_path}' and '{}')",
                            index.files[file].path
                        ),
                        Some((span.0..span.1).into()),
                    )
                    .with_code(E_AMBIGUOUS_MODULE),
                ));
                table.remove(&name);
                ambiguous.insert(name);
            } else {
                table.insert(name, (file, position));
            }
        }
    }

    // Every use in the compilation resolves — imported files' uses too, so
    // expansion and navigation see the full graph — but only the main
    // file's uses are validated and diagnosed: an imported file's unknown
    // target is that file's own compilation's report.
    for use_idx in 0..index.module_uses.len() {
        let (target, span, overrides, use_file) = {
            let site = &index.module_uses[use_idx];
            (
                site.target.clone(),
                site.span,
                site.overrides.clone(),
                site.file,
            )
        };
        if ambiguous.contains(&target) {
            continue;
        }
        let Some(&(decl_file, decl_position)) = table.get(&target) else {
            if use_file == 0 {
                diags.push((
                    main_path.clone(),
                    Diagnostic::error(
                        format!("unknown module '{target}'"),
                        Some((span.0..span.1).into()),
                    )
                    .with_code(E_UNKNOWN_MODULE),
                ));
            }
            continue;
        };
        let global = decls_by_file[&decl_file][decl_position];
        let decl = &index.module_decls[global];
        if use_file == 0 {
            validate_use(&target, decl, span, &overrides, &main_path, diags);
        }
        index.module_uses[use_idx].resolved_module = Some((decl_file, decl_position));
    }
}

/// Validates one resolved use's overrides against its module: every
/// override must name an input parameter or a module property; literal
/// overrides of inputs get the structural type sanity check; every
/// default-less input must receive an override.
fn validate_use(
    target: &str,
    decl: &ModuleDeclSite,
    use_span: (usize, usize),
    overrides: &[UseOverride],
    main_path: &str,
    diags: &mut Vec<(String, Diagnostic<&'static str>)>,
) {
    for override_item in overrides {
        let input = decl
            .input_params
            .iter()
            .find(|param| param.name == override_item.name);
        if input.is_none()
            && !decl
                .properties
                .iter()
                .any(|name| name == &override_item.name)
        {
            let span = override_item.span;
            diags.push((
                main_path.to_string(),
                Diagnostic::error(
                    format!(
                        "unknown input '{}' for module '{target}'",
                        override_item.name
                    ),
                    Some((span.0..span.1).into()),
                )
                .with_code(E_UNKNOWN_INPUT),
            ));
            continue;
        }
        if let (Some(param), Some(literal)) = (input, override_item.literal) {
            if !literal_suits_type(&param.type_ref, literal) {
                let span = override_item.span;
                diags.push((
                    main_path.to_string(),
                    Diagnostic::error(
                        format!(
                            "input '{}' of module '{target}' expects {}",
                            override_item.name, param.type_ref
                        ),
                        Some((span.0..span.1).into()),
                    )
                    .with_code(E_INPUT_TYPE),
                ));
            }
        }
    }
    for param in &decl.input_params {
        if !param.has_default && !overrides.iter().any(|o| o.name == param.name) {
            diags.push((
                main_path.to_string(),
                Diagnostic::error(
                    format!("module '{target}' requires input '{}'", param.name),
                    Some((use_span.0..use_span.1).into()),
                )
                .with_code(E_MISSING_INPUT),
            ));
        }
    }
}

/// The structural literal-vs-type sanity check: string literals suit
/// `String`, `Uuid`, and custom identifier types (mismatching only the
/// non-string primitives `Int`/`Float`/`Boolean`/`DateTime`); numbers suit
/// `Int`/`Float`; booleans suit `Boolean`.
/// Whether a literal override structurally suits a declared input type:
/// builtins go through the shared alias table (`crate::check::builtin_type`)
/// so the resolver and the expression checker can never disagree; any other
/// `type_ref` is a custom-identifier type, whose only literal shape is a
/// string (entity handles are opaque text). Shared with the checker's
/// command option-default rule, which is the same literal-vs-type question.
pub(crate) fn literal_suits_type(type_ref: &str, literal: LiteralKind) -> bool {
    match crate::check::builtin_type(type_ref) {
        Some(builtin) => builtin.suits(literal),
        None => literal == LiteralKind::Str,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_type_matrix() {
        assert!(literal_suits_type("String", LiteralKind::Str));
        assert!(literal_suits_type("Uuid", LiteralKind::Str));
        assert!(literal_suits_type("Color", LiteralKind::Str));
        assert!(!literal_suits_type("Int", LiteralKind::Str));
        assert!(!literal_suits_type("Float", LiteralKind::Str));
        assert!(!literal_suits_type("Boolean", LiteralKind::Str));
        assert!(!literal_suits_type("DateTime", LiteralKind::Str));

        assert!(literal_suits_type("Int", LiteralKind::Num));
        assert!(literal_suits_type("Float", LiteralKind::Num));
        assert!(!literal_suits_type("String", LiteralKind::Num));
        assert!(!literal_suits_type("Boolean", LiteralKind::Num));

        assert!(literal_suits_type("Boolean", LiteralKind::Bool));
        assert!(!literal_suits_type("Int", LiteralKind::Bool));
        assert!(!literal_suits_type("String", LiteralKind::Bool));
    }

    #[test]
    fn walker_walks_depth_first_in_source_order() {
        let walk = walk_ifml_imports(
            "app.ifml",
            "import \"a.ifml\";\nimport \"b.ifml\";",
            &mut |importer, import| match (importer, import) {
                ("app.ifml", "a.ifml") => {
                    Some(("a.ifml".to_string(), "import \"c.ifml\";".to_string()))
                }
                ("a.ifml", "c.ifml") => Some(("c.ifml".to_string(), "view \"C\" {}".to_string())),
                ("app.ifml", "b.ifml") => Some(("b.ifml".to_string(), "view \"B\" {}".to_string())),
                _ => None,
            },
        );
        let identities: Vec<&str> = walk.sources.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(identities, vec!["app.ifml", "a.ifml", "c.ifml", "b.ifml"]);
    }

    #[test]
    fn walker_dedupes_diamond_imports_and_skips_cycles() {
        let walk = walk_ifml_imports(
            "app.ifml",
            "import \"a.ifml\";\nimport \"b.ifml\";",
            &mut |importer, import| match (importer, import) {
                ("app.ifml", "a.ifml") | ("app.ifml", "b.ifml") => Some((
                    "shared.ifml".to_string(),
                    "import \"shared.ifml\";".to_string(),
                )),
                ("a.ifml", "shared.ifml") | ("b.ifml", "shared.ifml") => {
                    Some(("shared.ifml".to_string(), "view \"S\" {}".to_string()))
                }
                _ => None,
            },
        );
        let identities: Vec<&str> = walk.sources.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(
            identities,
            vec!["app.ifml", "a.ifml", "shared.ifml", "b.ifml"]
        );
    }

    #[test]
    fn walker_leaves_cycles_to_the_resolver() {
        let walk = walk_ifml_imports(
            "app.ifml",
            "import \"loop.ifml\";",
            &mut |importer, import| match (importer, import) {
                ("app.ifml", "loop.ifml") => {
                    Some(("loop.ifml".to_string(), "import \"app.ifml\";".to_string()))
                }
                _ => None,
            },
        );
        assert_eq!(walk.sources.len(), 2);
    }

    #[test]
    fn walker_skips_imports_the_host_declines() {
        let walk = walk_ifml_imports(
            "app.ifml",
            "import \"auth.actor\";\nimport \"model.mox\";",
            &mut |_importer, _import| None,
        );
        assert_eq!(walk.sources.len(), 1);
    }
}
