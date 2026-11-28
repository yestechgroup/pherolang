//! Host-side workspace import collection: `import schema` and `import
//! sigil` resolution from disk, relative to the declaring `.mox` file. The
//! driver core stays filesystem-free — every compile entry point receives
//! texts through [`crate::DomainImports`] — and this module is the
//! sanctioned bridge for hosts (the CLI, the LSP) that own filesystem
//! access: give it the sources you are about to compile and it gathers the
//! import content the compile needs.

use std::path::{Path, PathBuf};

use crate::{DomainImports, SchemaImports, SigilImports};

/// Collects the JSON content of every `import schema` declaration across
/// `sources`, resolving each import path **relative to the declaring `.mox`
/// file's directory** (the `.actor` import-resolution precedent: the CLI
/// owns filesystem access, the driver receives texts only). A missing
/// import file is a clean error naming the resolved path. Syntax errors in
/// a source surface later as compile diagnostics, so they are not fatal
/// here.
pub fn collect_schema_imports(sources: &[(String, String)]) -> anyhow::Result<SchemaImports> {
    let mut imports = SchemaImports::new();
    for (path, source) in sources {
        let Some(ast) = &phero_syntax::parse(source).ast else {
            continue;
        };
        let dir = crate::engine_common::dir_of(path);
        for decl in &ast.declarations {
            let phero_syntax::ast::Decl::ImportSchema(import) = decl else {
                continue;
            };
            let resolved = dir.join(&import.path);
            let json = std::fs::read_to_string(&resolved).map_err(|error| {
                anyhow::anyhow!(
                    "cannot read imported schema {} (imported by {}): {error}",
                    resolved.display(),
                    path
                )
            })?;
            imports.insert(path.clone(), import.path.clone(), json);
        }
    }
    Ok(imports)
}

/// The rosetta sources a compilation's `import sigil` declarations resolve
/// to: every file handed to the driver, as `(tag path, source text)`. The
/// tag path is what sigil diagnostics are labeled with.
pub struct SigilSources {
    /// The provider entries keyed `(mox path, import path)` — the named
    /// imports plus the discovered candidates.
    pub imports: SigilImports,
    /// Every provided rosetta source keyed by its tag path, for rendering
    /// sigil diagnostics against their own text.
    pub sources: Vec<(String, String)>,
}

/// Collects the rosetta content of every `import sigil` declaration across
/// `sources`, resolving each import path **relative to the declaring `.mox`
/// file's directory** (the `.actor` import-resolution precedent: the CLI
/// owns filesystem access, the driver receives texts only). A missing import
/// file is a clean error naming the resolved path. Syntax errors in a source
/// surface later as compile diagnostics, so they are not fatal here.
///
/// Rosetta-internal namespace imports are satisfied transitively: every
/// `*.rosetta` file under the named file's directory is read as a candidate
/// (mirroring sigil's project discovery: dot and `target` directories are
/// skipped, depth-capped), keyed by its resolved path. The driver lowers
/// only the namespace closure the named file actually needs, so unrelated
/// siblings never leak into the artifact.
pub fn collect_sigil_imports(sources: &[(String, String)]) -> anyhow::Result<SigilSources> {
    let mut sigil = SigilSources {
        imports: SigilImports::new(),
        sources: Vec::new(),
    };
    for (path, source) in sources {
        let Some(ast) = &phero_syntax::parse(source).ast else {
            continue;
        };
        let dir = crate::engine_common::dir_of(path);
        for decl in &ast.declarations {
            let phero_syntax::ast::Decl::ImportSchema(import) = decl else {
                continue;
            };
            if import.kind != phero_syntax::ast::ImportKind::Sigil {
                continue;
            }
            let resolved = dir.join(&import.path);
            let text = std::fs::read_to_string(&resolved).map_err(|error| {
                anyhow::anyhow!(
                    "cannot read imported sigil {} (imported by {}): {error}",
                    resolved.display(),
                    path
                )
            })?;
            sigil
                .imports
                .insert(path.clone(), import.path.clone(), text.clone());
            sigil.sources.push((import.path.clone(), text));
            // Candidate pool: every rosetta file next to the named import.
            for candidate in rosetta_candidates(&resolved) {
                let candidate_path = candidate.display().to_string();
                if sigil
                    .sources
                    .iter()
                    .any(|(existing, _)| *existing == candidate_path)
                {
                    continue;
                }
                let text = std::fs::read_to_string(&candidate).map_err(|error| {
                    anyhow::anyhow!(
                        "cannot read imported sigil {} (discovered for {} imported by {}): {error}",
                        candidate.display(),
                        import.path,
                        path
                    )
                })?;
                sigil
                    .imports
                    .insert(path.clone(), candidate_path.clone(), text.clone());
                sigil.sources.push((candidate_path, text));
            }
        }
    }
    Ok(sigil)
}

/// The deepest directory [`discover_rosetta_files`] descends to: the root is
/// depth 0, and a directory deeper than this is not read. The cap also bounds
/// symlink cycles.
pub const ROSETTA_MAX_DEPTH: usize = 16;

/// Every `*.rosetta` file under `roots`: the one `.rosetta` discovery walk
/// the CLI, `verify`, the LSP and the Rune tools share (RN11).
///
/// - A root that is a directory is walked; a root that is a file is taken
///   as-is when its name ends in `.rosetta`.
/// - An entry whose name starts with `.` or is exactly `target` is skipped
///   with its whole subtree, files and directories alike.
/// - The walk follows symlinks and stops below [`ROSETTA_MAX_DEPTH`].
/// - Names are matched case-sensitively; a name that is not UTF-8 is skipped,
///   and so is a directory or entry that cannot be read.
/// - The result is sorted and deduplicated, so it never depends on the order
///   the file system returns entries in.
pub fn discover_rosetta_files(roots: &[PathBuf]) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        if depth > ROSETTA_MAX_DEPTH {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if name.starts_with('.') || name == "target" {
                continue;
            }
            if path.is_dir() {
                walk(&path, depth + 1, out);
            } else if name.ends_with(".rosetta") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    for root in roots {
        if root.is_dir() {
            walk(root, 0, &mut out);
        } else if root
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".rosetta"))
        {
            out.push(root.clone());
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The `import sigil` candidate pool for the named rosetta file: every
/// [`discover_rosetta_files`] result under its directory except the named
/// file itself. A candidate changes the lowering, so `verify` counts each one
/// as a source.
pub fn rosetta_candidates(named: &Path) -> Vec<PathBuf> {
    let base = crate::engine_common::dir_of(named);
    discover_rosetta_files(&[base])
        .into_iter()
        .filter(|path| path != named)
        .collect()
}

/// Collects the import content for one source whose **driver key** differs
/// from its disk location — the LSP case: documents key by URI, imports
/// resolve relative to the file on disk. The bundle is gathered from disk
/// under the disk path, then re-keyed under `driver_key` so the driver
/// matches it against the compiling source. The sigil candidate pool comes
/// along: every discovered rosetta sibling re-keys too.
pub fn collect_for_source(
    driver_key: &str,
    disk_path: &Path,
    source: &str,
) -> anyhow::Result<DomainImports> {
    let disk_key = disk_path.display().to_string();
    let schemas = collect_schema_imports(&[(disk_key.clone(), source.to_string())])?;
    let sigil = collect_sigil_imports(&[(disk_key.clone(), source.to_string())])?;

    let declared = phero_syntax::parse(source)
        .ast
        .map(|ast| {
            ast.declarations
                .iter()
                .filter_map(|decl| match decl {
                    phero_syntax::ast::Decl::ImportSchema(import) => {
                        Some((import.path.clone(), import.kind))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut rekeyed_schemas = SchemaImports::new();
    let mut rekeyed_sigil = SigilImports::new();
    for (import_path, kind) in declared {
        let is_sigil = kind == phero_syntax::ast::ImportKind::Sigil;
        if is_sigil {
            for (tag, _) in &sigil.sources {
                if let Some(text) = sigil.imports.get(&disk_key, tag) {
                    rekeyed_sigil.insert(driver_key.to_string(), tag.clone(), text.to_string());
                }
            }
        } else if let Some(json) = schemas.get(&disk_key, &import_path) {
            rekeyed_schemas.insert(
                driver_key.to_string(),
                import_path.clone(),
                json.to_string(),
            );
        }
    }
    Ok(DomainImports {
        schemas: rekeyed_schemas,
        sigil: rekeyed_sigil,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    static NEXT_ID: AtomicU32 = AtomicU32::new(0);

    /// A fresh directory, removed again on drop.
    struct TempTree(PathBuf);

    impl TempTree {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "phero-driver-rosetta-{}-{}-{name}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::SeqCst),
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            TempTree(dir)
        }

        fn write(&self, relative: &str) -> PathBuf {
            let path = self.0.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).expect("create parent dirs");
            std::fs::write(&path, "namespace test\n").expect("write file");
            path
        }

        fn root(&self) -> Vec<PathBuf> {
            vec![self.0.clone()]
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn discovery_is_sorted_and_recursive() {
        let tree = TempTree::new("sorted");
        let z = tree.write("z.rosetta");
        let e = tree.write("b/d/e.rosetta");
        let a = tree.write("a.rosetta");
        let c = tree.write("b/c.rosetta");
        tree.write("b/notes.txt");
        tree.write("b/upper.ROSETTA");
        assert_eq!(discover_rosetta_files(&tree.root()), [a, c, e, z]);
    }

    #[test]
    fn discovery_skips_dot_entries_and_target() {
        let tree = TempTree::new("skips");
        let kept = tree.write("kept.rosetta");
        let nested = tree.write("nested/also_kept.rosetta");
        tree.write(".hidden/dropped.rosetta");
        tree.write(".dotfile.rosetta");
        tree.write("target/dropped.rosetta");
        tree.write("nested/target/dropped.rosetta");
        tree.write("nested/.hidden/dropped_too.rosetta");
        // A name that merely contains `target` is not skipped.
        let targets = tree.write("targets/kept.rosetta");
        assert_eq!(
            discover_rosetta_files(&tree.root()),
            [kept, nested, targets]
        );
    }

    #[test]
    fn discovery_stops_below_the_depth_cap() {
        let tree = TempTree::new("depth");
        let at_cap = "d/".repeat(ROSETTA_MAX_DEPTH);
        let reached = tree.write(&format!("{at_cap}at_cap.rosetta"));
        tree.write(&format!("{at_cap}d/too_deep.rosetta"));
        assert_eq!(discover_rosetta_files(&tree.root()), [reached]);
    }

    #[test]
    fn discovery_is_deterministic_across_runs() {
        let tree = TempTree::new("deterministic");
        for relative in [
            "m/n/o/deep.rosetta",
            "m/p/q/deep.rosetta",
            "m/p/deep.rosetta",
            "m/deep.rosetta",
            "sibling.rosetta",
        ] {
            tree.write(relative);
        }
        let first = discover_rosetta_files(&tree.root());
        for _ in 0..4 {
            assert_eq!(discover_rosetta_files(&tree.root()), first);
        }
    }

    #[test]
    fn file_and_directory_roots_are_merged_and_deduplicated() {
        let tree = TempTree::new("roots");
        let one = tree.write("a/one.rosetta");
        let two = tree.write("b/two.rosetta");
        let loose = tree.write("loose.rosetta");
        let other = tree.write("b/other.txt");
        let roots = [
            tree.0.join("b"),
            loose.clone(),
            tree.0.join("a"),
            one.clone(),
            other,
            tree.0.join("missing"),
        ];
        assert_eq!(discover_rosetta_files(&roots), [one, two, loose]);
    }

    #[test]
    fn candidates_are_the_named_files_siblings_without_it() {
        let tree = TempTree::new("candidates");
        let named = tree.write("model/main.rosetta");
        let sibling = tree.write("model/dep.rosetta");
        let nested = tree.write("model/extra/extra.rosetta");
        tree.write("model/target/built.rosetta");
        tree.write("elsewhere.rosetta");
        assert_eq!(rosetta_candidates(&named), [sibling, nested]);
    }
}
