//! The cross-surface graph snapshot and the handlers that ride on it.
//!
//! Every navigation capability beyond `.mox` (and `.ifml`'s own
//! declaration-to-declaration walk) answers from **one** semantic index: a
//! [`GraphSnapshot`] — the compiled artifacts of the open document, its
//! open-document siblings and the disk siblings of its directory, fed to
//! [`phero_driver::graph::build`], plus each file's spanned
//! [`SurfaceIndex`](phero_driver::navigation::SurfaceIndex).
//!
//! Resolution is the graph's (`GraphIndex`); locations are the span tables'
//! (`SurfaceIndex`). A reference whose target the graph cannot resolve
//! produces **no location** — never an error and never a guess: that is the
//! artifact's "unresolved ⇒ no edge" discipline applied to the editor.
//!
//! # Snapshot scope
//!
//! The snapshot covers exactly one directory: the open document's own.
//! Candidate files are every supported surface file in that directory (a
//! formatter-golden `X.fmt.<ext>` is skipped) plus every **open** document
//! that lives there — an open buffer wins over its disk copy, so editing an
//! imported sibling refreshes cross-surface navigation without a save. A
//! sibling that does not compile contributes nothing (its own open editor
//! shows its diagnostics); the rest of the snapshot is unaffected.
//!
//! # Rename ownership
//!
//! Rename is enabled for declaration-owned names, and only in the document
//! that declares them: a `.ddd` capability clause naming a `.actor`
//! capability is not renamed from the `.ddd` document — the refusal names
//! the owning file. See [`rename_target`].

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use phero_driver::graph::{build, GraphFile, GraphFragment, GraphIndex, GraphInputs};
use phero_driver::navigation::{
    ifml_surface, mox_index, IndexedDefinition, IndexedReference, ReferenceContext, SurfaceIndex,
    SurfaceLookup,
};
use phero_driver::SurfaceKind;
use phero_ir::graph::{kinds, GraphModel, GraphNode};
use tower_lsp_server::ls_types::*;

use super::pipeline::{to_lsp_diagnostic, to_lsp_ifml_diagnostic};
use super::{DocumentSurface, PheroBackend};
use crate::position::PositionMap;

/// One compiled snapshot: the graph model and one spanned index per file it
/// covers.
pub(super) struct GraphSnapshot {
    /// The built artifact (also the resolution index's borrow source).
    pub(super) model: GraphModel,
    /// Every participating file, sorted by display path.
    pub(super) files: Vec<SnapshotFile>,
}

/// One participating file: where it is, what it says, and where its
/// declarations are.
pub(super) struct SnapshotFile {
    /// Root-relative display path — the graph's `file` attribution.
    pub(super) display: String,
    /// The client-facing URI (the open document's URI for an open file).
    pub(super) uri: Uri,
    /// The text the index was built from (the open buffer when one exists).
    pub(super) text: String,
    /// The root application name this file's declarations are keyed by
    /// (`.ddd` fragments only; `None` everywhere else). A fragment carries
    /// no `application` declaration of its own, so its span index and its
    /// graph nodes need the root's name supplied (DD8).
    pub(super) application: Option<String>,
    /// The file's spanned index.
    pub(super) index: SurfaceIndex,
}

/// The per-file compile memo behind the snapshot (pherolang#254).
///
/// The snapshot itself is still rebuilt whenever the open-document
/// signature moves, but a rebuild only **recompiles** the candidates whose
/// [`MemoEntry::key`] changed; every other entry's artifact and spanned
/// index are reused. The memo is bounded by the directory's candidate set:
/// [`PheroBackend::build_graph_snapshot`] prunes every entry whose path is no
/// longer a candidate on the build that observes the removal.
#[derive(Default)]
pub(super) struct GraphMemo {
    /// One entry per candidate file, keyed by its path.
    entries: HashMap<PathBuf, MemoEntry>,
}

/// One memoized candidate: the content key it was compiled under, the text
/// it was compiled from, and the compile's whole product.
struct MemoEntry {
    /// The invalidation key: own-text hash plus the open-or-disk text hash
    /// of every resolved import (see [`PheroBackend::compile_key`]).
    key: u64,
    /// The text the entry was built from — the snapshot's text for this
    /// file, open buffer when one exists.
    text: String,
    /// The compiled artifact, or `None` when the file did not compile (a
    /// non-compiling sibling contributes nothing, and is retried when its
    /// key moves).
    artifact: Option<SnapshotArtifact>,
    /// The root application name a `.ddd` fragment's index and nodes are
    /// keyed by (`None` for a root and every other surface).
    application: Option<String>,
    /// The diagnostics this compile produced (own-file only, filtered
    /// exactly like the publish pipeline's), already in LSP shape — the
    /// publish source for the surfaces whose only compile is this one
    /// (pherolang#255), read through
    /// [`PheroBackend::snapshot_diagnostics`].
    diagnostics: Vec<Diagnostic>,
    /// The file's spanned index, or `None` when its source does not parse.
    index: Option<SurfaceIndex>,
}

impl GraphMemo {
    /// The number of memoized candidates (the memory bound's observable
    /// half; tests use it to assert pruning).
    #[cfg(test)]
    pub(super) fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Whether the memo retains an entry for `path`.
    #[cfg(test)]
    pub(super) fn contains(&self, path: &Path) -> bool {
        self.entries.contains_key(path)
    }

    /// The diagnostics the memoized compile of `path` produced, in LSP
    /// shape (own-file only). [`PheroBackend::snapshot_diagnostics`] is the
    /// publish pipeline's reader; tests also assert the stored product.
    pub(super) fn diagnostics(&self, path: &Path) -> Option<&[Diagnostic]> {
        self.entries
            .get(path)
            .map(|entry| entry.diagnostics.as_slice())
    }
}

impl GraphSnapshot {
    /// The resolution index over this snapshot's model.
    pub(super) fn index(&self) -> GraphIndex<'_> {
        GraphIndex::new(&self.model)
    }

    /// The file with this display path.
    pub(super) fn file(&self, display: &str) -> Option<&SnapshotFile> {
        self.files.iter().find(|file| file.display == display)
    }

    /// The file the client opened under `uri`.
    pub(super) fn file_for_uri(&self, uri: &Uri) -> Option<&SnapshotFile> {
        self.files.iter().find(|file| file.uri == *uri)
    }

    /// The definition location of one graph node: the declaring file's span
    /// for the node's `(kind, name)`.
    ///
    /// The node's `file` is the file whose **artifact** declared it, which is
    /// not always the file that spells it: a `.ddd` fragment's declarations
    /// are merged into its root's artifact and attributed to the root (DD8).
    /// The snapshot therefore looks in the attributed file first and then in
    /// every other participating file, in display order — declaration names
    /// are application-wide unique, so the first match is the declaration.
    /// A node the snapshot's span tables do not cover yields **no** location,
    /// never a guess.
    pub(super) fn location(&self, node: &GraphNode) -> Option<Location> {
        let preferred = self.file(&node.file);
        // Within one application, declaration names are unique; across
        // applications they are not, so a file that belongs to the node's
        // application outranks one that does not. Non-`.ddd` files carry no
        // application name and fill the tail.
        let in_application =
            |file: &&SnapshotFile| file.application.as_deref() == Some(node.application.as_str());
        let same_application = self
            .files
            .iter()
            .filter(|file| file.display != node.file)
            .filter(in_application);
        let other_application = self
            .files
            .iter()
            .filter(|file| file.display != node.file)
            .filter(|file| !in_application(file));
        for file in preferred
            .into_iter()
            .chain(same_application)
            .chain(other_application)
        {
            let Some(definition) =
                file.index.definitions.iter().find(|definition| {
                    definition.kind == node.kind && definition.name == node.name
                })
            else {
                continue;
            };
            return Some(Location {
                uri: file.uri.clone(),
                range: PositionMap::new(&file.text).range_for(definition.name_span),
            });
        }
        None
    }

    /// The landing site of a `platformCapability` node: the closed
    /// deploy-vocabulary word has **no declaration site** (its table lives in
    /// the driver, not in a model file — issue pherolang#262), so the anchor is
    /// the first written `requires` mention of the word in the node's file.
    /// The file is the application's anchoring deploy file by construction,
    /// so a mention always exists; `None` only for a hand-edited artifact.
    pub(super) fn platform_anchor(&self, node: &GraphNode) -> Option<Location> {
        let file = self.file(&node.file)?;
        let map = PositionMap::new(&file.text);
        file.index
            .references
            .iter()
            .filter(|reference| {
                reference.context == ReferenceContext::DeployRequires && reference.name == node.name
            })
            .min_by_key(|reference| reference.span.start)
            .map(|reference| Location {
                uri: file.uri.clone(),
                range: map.range_for(reference.span),
            })
    }

    /// Every definition of `(kind, name)` in this file, as locations.
    fn definition_locations(&self, display: &str, kind: &str, name: &str) -> Vec<Location> {
        let Some(file) = self.file(display) else {
            return Vec::new();
        };
        let map = PositionMap::new(&file.text);
        file.index
            .definitions_of(kind, name)
            .into_iter()
            .map(|definition| Location {
                uri: file.uri.clone(),
                range: map.range_for(definition.name_span),
            })
            .collect()
    }

    /// Every reference in the snapshot that the graph binds to `node`.
    fn references_to(
        &self,
        index: &GraphIndex<'_>,
        node: &GraphNode,
        include_declaration: bool,
    ) -> Vec<Location> {
        let mut locations = Vec::new();
        if include_declaration {
            locations.extend(self.definition_locations(&node.file, &node.kind, &node.name));
        }
        for file in &self.files {
            let map = PositionMap::new(&file.text);
            for reference in &file.index.references {
                if !reference.targets_kind(&node.kind) {
                    continue;
                }
                let Some(target) = resolve(index, file, reference) else {
                    continue;
                };
                if target.id != node.id {
                    continue;
                }
                locations.push(Location {
                    uri: file.uri.clone(),
                    range: map.range_for(reference.span),
                });
            }
        }
        locations.sort_by(|left, right| {
            (
                left.uri.as_str(),
                &left.range.start.line,
                &left.range.start.character,
            )
                .cmp(&(
                    right.uri.as_str(),
                    &right.range.start.line,
                    &right.range.start.character,
                ))
        });
        locations
    }

    /// The node a definition site denotes: the graph node in this file with
    /// the same `(kind, name)`.
    fn node_of<'a>(
        &self,
        index: &GraphIndex<'a>,
        file: &SnapshotFile,
        definition: &IndexedDefinition,
    ) -> Option<&'a GraphNode> {
        index
            .nodes_by_kind_name(&definition.kind, &definition.name)
            .iter()
            .find(|node| node.file == file.display)
            .copied()
    }
}

impl PheroBackend {
    /// The graph snapshot for `uri`'s directory, built once and reused until
    /// the open-document set (or any open version) changes.
    ///
    /// The cache key is the directory plus every open document's
    /// `(uri, version)` in it; [`PheroBackend`] clears the cache on open,
    /// change, and close anyway, so an edit is never served from a stale
    /// snapshot. Disk-only siblings are re-read whenever the key changes —
    /// an external edit to an unopened sibling is picked up on the next
    /// open/change/close, not by watching the filesystem.
    pub(super) fn graph_snapshot(&self, uri: &Uri) -> Option<Arc<GraphSnapshot>> {
        let directory = uri.to_file_path()?.parent().map(Path::to_path_buf)?;
        let open = self.open_signature(&directory);
        {
            let cache = self.graph_cache.lock().unwrap();
            if let Some(cached) = cache.as_ref() {
                if cached.directory == directory && cached.open == open {
                    return Some(cached.snapshot.clone());
                }
            }
        }
        let snapshot = Arc::new(self.build_graph_snapshot(&directory)?);
        *self.graph_cache.lock().unwrap() = Some(super::GraphCache {
            directory,
            open,
            snapshot: snapshot.clone(),
        });
        Some(snapshot)
    }

    /// The diagnostics the snapshot's **memoized** compile attributed to
    /// `uri`'s own file, in LSP shape (pherolang#255).
    ///
    /// The four artifact surfaces whose compile lives only in the snapshot
    /// (`.evt`, `.deploy`, `.uc`, `.cmmn`) publish from here rather than
    /// compiling a second time: ensuring the snapshot exists for the
    /// document's directory runs [`PheroBackend::build_graph_snapshot`], which
    /// compiles the candidates whose content key moved (pherolang#254) and
    /// stores each file's own-file diagnostics. A URI that is not a file,
    /// or has no directory, has no snapshot — the caller publishes nothing.
    /// A file the memo never compiled (absent from the candidate set) also
    /// publishes nothing; a clean compile publishes an empty vector, which
    /// clears stale diagnostics.
    pub(super) fn snapshot_diagnostics(&self, uri: &Uri) -> Vec<Diagnostic> {
        // A snapshot is built from a directory on disk, so only a `file:`
        // URI names one. An opaque URI (`untitled:…`) is still dispatched to
        // its surface by extension, but has no directory to compile — the
        // pipeline publishes nothing rather than a bogus empty set.
        if !uri.scheme().as_str().eq_ignore_ascii_case("file") {
            return Vec::new();
        }
        let Some(path) = uri.to_file_path() else {
            return Vec::new();
        };
        if self.graph_snapshot(uri).is_none() {
            return Vec::new();
        }
        self.graph_memo
            .lock()
            .unwrap()
            .diagnostics(&path)
            .map(<[Diagnostic]>::to_vec)
            .unwrap_or_default()
    }

    /// The `(uri, version)` of every open document in `directory`, sorted —
    /// the snapshot cache key.
    fn open_signature(&self, directory: &Path) -> Vec<(String, i32)> {
        let documents = self.documents.lock().unwrap();
        let mut signature: Vec<(String, i32)> = documents
            .iter()
            .filter(|(uri, document)| {
                document.surface != DocumentSurface::Other
                    && uri
                        .to_file_path()
                        .is_some_and(|path| path.parent() == Some(directory))
            })
            .map(|(uri, document)| (uri.to_string(), document.version))
            .collect();
        signature.sort();
        signature
    }

    /// Builds the snapshot for one directory: enumerate, compile (memo
    /// hits reused), index, and prune the memo to the current candidates.
    ///
    /// The directory listing itself is re-read on every build: a `read_dir`
    /// plus one `stat` per entry is cheap next to a compile, and it keeps
    /// the candidate set honest without a filesystem watcher (an external
    /// edit to an unopened sibling still arrives on the next
    /// open/change/close, which is also when the snapshot cache is
    /// cleared).
    fn build_graph_snapshot(&self, directory: &Path) -> Option<GraphSnapshot> {
        let open: Vec<(Uri, DocumentSurface, String)> = {
            let documents = self.documents.lock().unwrap();
            documents
                .iter()
                .filter(|(uri, document)| {
                    document.surface != DocumentSurface::Other
                        && uri
                            .to_file_path()
                            .is_some_and(|path| path.parent() == Some(directory))
                })
                .map(|(uri, document)| (uri.clone(), document.surface, document.text.clone()))
                .collect()
        };
        let open_for = |path: &Path| -> Option<&(Uri, DocumentSurface, String)> {
            open.iter().find(|(uri, _, _)| {
                uri.to_file_path()
                    .is_some_and(|candidate| candidate.as_ref() == path)
            })
        };

        // Candidates: the directory's supported files, plus every open
        // document that lives there (which need not exist on disk).
        let mut candidates: BTreeMap<PathBuf, DocumentSurface> = BTreeMap::new();
        if let Ok(entries) = std::fs::read_dir(directory) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() || is_formatter_golden(&path) {
                    continue;
                }
                if let Some(surface) = surface_of_path(&path) {
                    candidates.insert(path, surface);
                }
            }
        }
        for (uri, surface, _) in &open {
            if let Some(path) = uri.to_file_path() {
                candidates.insert(path.into_owned(), *surface);
            }
        }

        // The memo lock is held across the build: a rebuild is one logical
        // unit (compile the misses, reuse the hits, prune), and tower-lsp
        // may dispatch navigation requests concurrently. Compiles never
        // reach back into the memo, so this cannot deadlock.
        let mut memo = self.graph_memo.lock().unwrap();
        let mut inputs = GraphInputs::default();
        let mut files: Vec<SnapshotFile> = Vec::new();
        let mut seen: HashSet<PathBuf> = HashSet::with_capacity(candidates.len());
        for (path, surface) in candidates {
            let Some(display) = display_path(&path) else {
                continue;
            };
            let (uri, text) = match open_for(&path) {
                Some((uri, _, text)) => (uri.clone(), text.clone()),
                None => {
                    let Some(uri) = Uri::from_file_path(&path) else {
                        continue;
                    };
                    let Ok(text) = std::fs::read_to_string(&path) else {
                        continue;
                    };
                    (uri, text)
                }
            };
            seen.insert(path.clone());
            // The application edges the graph clusters on stay exactly
            // `resolved_imports`; the compile key hashes the same imports'
            // text (plus `.mox`'s own schema/sigil imports).
            let imports = resolved_imports(surface, &text, &path);
            let key = self.compile_key(surface, &path, &text, &imports);
            // DD8: a `.ddd` fragment's declarations key by its root's
            // authored application name, which only the snapshot can resolve.
            let application = self.snapshot_application(surface, &uri, &text);
            // The text equality is a collision guard on top of the key: a
            // 64-bit hash is not a proof, and the own text is right here.
            let hit = memo
                .entries
                .get(&path)
                .is_some_and(|entry| entry.key == key && entry.text == text);
            if !hit {
                // One compile per candidate, never per memo hit — the
                // counter tests assert exactly this.
                self.compile_count.fetch_add(1, Ordering::Relaxed);
                let (artifact, diagnostics) =
                    self.compile_snapshot_file(surface, &path, &text, &uri);
                let index = surface_index(surface, &text, &display, application.as_deref());
                memo.entries.insert(
                    path.clone(),
                    MemoEntry {
                        key,
                        text: text.clone(),
                        artifact,
                        application,
                        diagnostics,
                        index,
                    },
                );
            }
            let Some(entry) = memo.entries.get(&path) else {
                continue;
            };
            if let Some(artifact) = &entry.artifact {
                push_artifact(
                    &mut inputs,
                    display.clone(),
                    &path,
                    imports,
                    artifact.clone(),
                );
                // DD8: a design root's fragments are files in the same
                // application — they own a file node and
                // `applications[].files` membership (the root artifact
                // already carries their declarations).
                if matches!(artifact, SnapshotArtifact::Design(_)) {
                    inputs
                        .design_fragments
                        .extend(self.snapshot_design_fragments(&uri, &text));
                }
            }
            if let Some(index) = &entry.index {
                files.push(SnapshotFile {
                    display,
                    uri,
                    text: entry.text.clone(),
                    application: entry.application.clone(),
                    index: index.clone(),
                });
            }
        }
        // Memory is bounded by the candidate set: a file removed from the
        // directory (or a deleted open buffer's sibling) is dropped now.
        memo.entries.retain(|path, _| seen.contains(path));
        files.sort_by(|left, right| left.display.cmp(&right.display));
        Some(GraphSnapshot {
            model: build(&inputs),
            files,
        })
    }

    /// The root application name a document's declarations key by, when the
    /// document is a `.ddd` file. A root carries its own `application`; a
    /// fragment needs its root's (DD8) — `ddd_app_sources` resolves it
    /// through the directory snapshot, and `None` means no root reaches it
    /// (the fragment is then reported as unreachable, not guessed).
    fn snapshot_application(
        &self,
        surface: DocumentSurface,
        uri: &Uri,
        text: &str,
    ) -> Option<String> {
        if surface != DocumentSurface::Known(SurfaceKind::Ddd) {
            return None;
        }
        let (_, root_text, _) = self.ddd_app_sources(uri, text)?;
        phero_syntax::parse_ddd(&root_text)
            .ast
            .and_then(|ast| ast.application)
            .map(|application| application.name.text)
    }

    /// The `.ddd` fragments of one design root, as graph input: a fragment
    /// owns no artifact of its own, but it is a file in the application
    /// (DD8), so the graph gets its file node and `applications[].files`
    /// membership — the same inputs the CLI's `graph build` supplies.
    fn snapshot_design_fragments(&self, uri: &Uri, text: &str) -> Vec<GraphFragment> {
        let Some((_, _, fragments)) = self.ddd_app_sources(uri, text) else {
            return Vec::new();
        };
        fragments
            .into_iter()
            .filter_map(|fragment| {
                let path = Path::new(&fragment.path);
                let display = display_path(path)?;
                let imports = resolved_imports(
                    DocumentSurface::Known(SurfaceKind::Ddd),
                    &fragment.source,
                    path,
                );
                Some(GraphFragment {
                    display,
                    path: fragment.path,
                    imports,
                })
            })
            .collect()
    }

    /// The invalidation key of one candidate (pherolang#254): its own text
    /// plus the **open-or-disk text** of every import it resolves, hashed
    /// with the std `DefaultHasher` (process-local; no dependency, no
    /// stability promise outside one server run).
    ///
    /// The import closure is load-bearing: a `.ddd` compiling against an
    /// edited `.mox` must recompile even though its own text is unchanged,
    /// and an unsaved `.mox` buffer wins over its disk copy exactly as the
    /// compile's own import fetch does ([`PheroBackend::fetch_open_or_disk`]).
    /// `resolved_imports` deliberately omits `.mox`'s `import
    /// schema`/`import sigil` declarations (they are not application
    /// edges), so those are hashed separately here.
    ///
    /// A content key was chosen over `(version, mtime)`: open buffers have
    /// no mtime, a version bump with identical text should not recompile,
    /// and only content tells a dependent's compile that an imported
    /// domain moved.
    fn compile_key(
        &self,
        surface: DocumentSurface,
        path: &Path,
        text: &str,
        imports: &[String],
    ) -> u64 {
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        text.hash(&mut hasher);
        for import in imports {
            import.hash(&mut hasher);
            hash_text(&mut hasher, self.fetch_open_or_disk(import));
        }
        if surface == DocumentSurface::Known(SurfaceKind::Mox) {
            for import in mox_schema_imports(text, path) {
                import.hash(&mut hasher);
                hash_text(&mut hasher, std::fs::read_to_string(&import).ok());
            }
        }
        // DD8: a `.ddd` document compiles as one application, so its key
        // covers the root text and every fragment's text — an edit to the
        // root (or a sibling fragment) must invalidate each participant's
        // index and diagnostics.
        if surface == DocumentSurface::Known(SurfaceKind::Ddd) {
            if let Some(uri) = Uri::from_file_path(path) {
                if let Some((root_path, root_text, fragments)) = self.ddd_app_sources(&uri, text) {
                    root_path.hash(&mut hasher);
                    root_text.hash(&mut hasher);
                    for fragment in fragments {
                        fragment.path.hash(&mut hasher);
                        fragment.source.hash(&mut hasher);
                    }
                }
            }
        }
        hasher.finish()
    }

    /// Compiles one snapshot file: its artifact (`None` when it does not
    /// compile — a non-compiling sibling contributes no nodes and no
    /// edges) and the diagnostics the compile attributed to the file
    /// itself, already in LSP shape.
    fn compile_snapshot_file(
        &self,
        surface: DocumentSurface,
        path: &Path,
        text: &str,
        uri: &Uri,
    ) -> (Option<SnapshotArtifact>, Vec<Diagnostic>) {
        let key = path.display().to_string();
        let map = PositionMap::new(text);
        let domain_inputs = |domains: &[(String, String)]| super::pipeline::domain_inputs(domains);
        let mut fetch = |resolved: &str| -> Option<String> { self.fetch_open_or_disk(resolved) };
        match surface {
            DocumentSurface::Known(SurfaceKind::Mox) => {
                // A schema/sigil import that cannot be read is not a compile
                // diagnostic (the import content never reached the driver).
                let Ok(imports) =
                    phero_driver::workspace_imports::collect_for_source(&key, path, text)
                else {
                    return (None, Vec::new());
                };
                let compilation = phero_driver::compile_str(&key, text, &imports);
                (
                    compilation.model.map(SnapshotArtifact::Domain),
                    compilation
                        .diagnostics
                        .iter()
                        .map(|diagnostic| to_lsp_diagnostic(diagnostic, &map))
                        .collect(),
                )
            }
            DocumentSurface::Known(SurfaceKind::Actor) => {
                let domains = phero_driver::project::gather_domain_sources(
                    &key,
                    text,
                    |source| {
                        phero_syntax::parse_actors(source)
                            .ast
                            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
                            .unwrap_or_default()
                    },
                    &mut fetch,
                );
                let compilation =
                    phero_driver::compile_actors_str(&key, text, &domain_inputs(&domains));
                (
                    compilation.model.map(SnapshotArtifact::Actor),
                    keyed_diagnostics(&compilation.diagnostics, &key, &map),
                )
            }
            DocumentSurface::Known(SurfaceKind::Ddd) => {
                let fragment = phero_syntax::parse_ddd(text)
                    .ast
                    .is_some_and(|ast| ast.application.is_none());
                // DD8: a fragment compiles through its root; only the root
                // owns the merged artifact. The import closure covers the
                // root **and** every fragment, each resolved relative to its
                // declaring file — a fragmented application compiles whole
                // or not at all.
                let (root_path, root_text, _) = self.ddd_app_sources(uri, text).unwrap_or((
                    key.clone(),
                    text.to_string(),
                    Vec::new(),
                ));
                let mut fetch =
                    |resolved: &str| -> Option<String> { self.fetch_open_or_disk(resolved) };
                let gathered = phero_driver::project::gather_ddd_app_imports(
                    &root_path, &root_text, &mut fetch,
                );
                let fragments: Vec<phero_driver::DddFragment> = gathered
                    .fragments
                    .iter()
                    .map(|(path, source)| phero_driver::DddFragment {
                        path: path.clone(),
                        source: source.clone(),
                    })
                    .collect();
                let inputs = super::pipeline::domain_inputs(&gathered.domains);
                let compilation =
                    phero_driver::compile_ddd_app_str(&root_path, &root_text, &fragments, &inputs);
                (
                    (!fragment)
                        .then(|| compilation.model.map(SnapshotArtifact::Design))
                        .flatten(),
                    keyed_diagnostics(&compilation.diagnostics, &key, &map),
                )
            }
            DocumentSurface::Known(SurfaceKind::Evt) => {
                let domains = phero_driver::project::gather_domain_sources(
                    &key,
                    text,
                    |source| {
                        phero_syntax::parse_evt(source)
                            .ast
                            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
                            .unwrap_or_default()
                    },
                    &mut fetch,
                );
                let compilation =
                    phero_driver::compile_evt_str(&key, text, &domain_inputs(&domains));
                (
                    compilation.model.map(SnapshotArtifact::Event),
                    keyed_diagnostics(&compilation.diagnostics, &key, &map),
                )
            }
            DocumentSurface::Known(SurfaceKind::Deploy) => {
                let Ok(pair) = phero_driver::project::gather_deploy_imports(
                    &key,
                    text,
                    |source| {
                        phero_syntax::parse_deploy(source)
                            .ast
                            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
                            .unwrap_or_default()
                    },
                    &mut fetch,
                ) else {
                    return (None, Vec::new());
                };
                let inputs = phero_driver::CompileInputs::from(pair);
                let compilation = phero_driver::compile_deploy_str(&key, text, &inputs);
                (
                    compilation.model.map(SnapshotArtifact::Deploy),
                    keyed_diagnostics(&compilation.diagnostics, &key, &map),
                )
            }
            DocumentSurface::Known(SurfaceKind::Ifml) => {
                let (snapshot, diagnostics) = self.compile_ifml(uri, text);
                (
                    snapshot.map(|snapshot| SnapshotArtifact::Flow(snapshot.compilation.model)),
                    diagnostics
                        .iter()
                        .filter(|(path, _)| path == uri.as_str())
                        .map(|(_, diagnostic)| to_lsp_ifml_diagnostic(diagnostic, &map))
                        .collect(),
                )
            }
            DocumentSurface::Known(SurfaceKind::Uc) => {
                let Ok(buckets) = phero_driver::project::gather_usecase_imports(
                    &key,
                    text,
                    |source| {
                        phero_syntax::parse_usecases(source)
                            .ast
                            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
                            .unwrap_or_default()
                    },
                    &mut fetch,
                ) else {
                    return (None, Vec::new());
                };
                let inputs = phero_driver::CompileInputs::from(buckets);
                let compilation = phero_driver::compile_usecase_str(&key, text, &inputs);
                (
                    compilation.model.map(SnapshotArtifact::UseCase),
                    keyed_diagnostics(&compilation.diagnostics, &key, &map),
                )
            }
            DocumentSurface::Known(SurfaceKind::Cmmn) => {
                let Ok(buckets) = phero_driver::project::gather_case_imports(
                    &key,
                    text,
                    &|source| {
                        phero_syntax::parse_cmmn(source)
                            .ast
                            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
                            .unwrap_or_default()
                    },
                    &mut fetch,
                ) else {
                    return (None, Vec::new());
                };
                let inputs = phero_driver::CompileInputs::from(buckets);
                let compilation = phero_driver::compile_cmmn_str(&key, text, &inputs);
                (
                    compilation.model.map(SnapshotArtifact::Case),
                    keyed_diagnostics(&compilation.diagnostics, &key, &map),
                )
            }
            DocumentSurface::Known(SurfaceKind::Dmn) => {
                let sources = self.dmn_sources_for(uri, text);
                let compilation = phero_driver::compile_dmn_str(&key, text, &sources.inputs);
                (
                    compilation.model.map(SnapshotArtifact::Decision),
                    keyed_diagnostics(&compilation.diagnostics, &key, &map),
                )
            }
            DocumentSurface::Known(SurfaceKind::Intent) => {
                let domains = phero_driver::project::gather_domain_sources(
                    &key,
                    text,
                    |source| {
                        phero_syntax::parse_intents(source)
                            .ast
                            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
                            .unwrap_or_default()
                    },
                    &mut fetch,
                );
                let compilation =
                    phero_driver::compile_intent_str(&key, text, &domain_inputs(&domains));
                (
                    compilation.model.map(SnapshotArtifact::Intent),
                    keyed_diagnostics(&compilation.diagnostics, &key, &map),
                )
            }
            DocumentSurface::Other => (None, Vec::new()),
        }
    }

    /// The number of memoized candidates (tests assert pruning).
    #[cfg(test)]
    pub(super) fn graph_memo_len(&self) -> usize {
        self.graph_memo.lock().unwrap().entry_count()
    }

    /// Whether the memo retains an entry for `path`.
    #[cfg(test)]
    pub(super) fn graph_memo_contains(&self, path: &Path) -> bool {
        self.graph_memo.lock().unwrap().contains(path)
    }

    /// The diagnostics the memoized compile of `path` produced.
    #[cfg(test)]
    pub(super) fn graph_memo_diagnostics(&self, path: &Path) -> Option<Vec<Diagnostic>> {
        self.graph_memo
            .lock()
            .unwrap()
            .diagnostics(path)
            .map(<[Diagnostic]>::to_vec)
    }
}

/// Feeds one optional import text into the compile key's hasher: the
/// presence byte distinguishes "absent" from "empty", so a deleted import
/// file moves the key.
fn hash_text(hasher: &mut DefaultHasher, text: Option<String>) {
    match text {
        Some(text) => {
            1u8.hash(hasher);
            text.hash(hasher);
        }
        None => 0u8.hash(hasher),
    }
}

/// The `import schema`/`import sigil` files one `.mox` source names,
/// resolved relative to it and canonicalized where they exist — hashed by
/// [`PheroBackend::compile_key`] because [`resolved_imports`] deliberately
/// leaves them out (they are not application edges).
fn mox_schema_imports(text: &str, path: &Path) -> Vec<String> {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let mut imports: Vec<String> = phero_syntax::parse(text)
        .ast
        .map(|ast| {
            ast.declarations
                .iter()
                .filter_map(|declaration| match declaration {
                    phero_syntax::ast::Decl::ImportSchema(import) => Some(import.path.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into_iter()
        .map(|import| {
            let resolved = directory.join(import);
            std::fs::canonicalize(&resolved)
                .map(|canonical| canonical.display().to_string())
                .unwrap_or_else(|_| resolved.display().to_string())
        })
        .collect();
    imports.sort();
    imports.dedup();
    imports
}

/// The diagnostics a keyed compile attributes to `key`, in LSP shape — the
/// same own-file filter the publish pipeline applies.
fn keyed_diagnostics(
    diagnostics: &[(String, phero_driver::Diagnostic)],
    key: &str,
    map: &PositionMap,
) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .filter(|(file, _)| file == key)
        .map(|(_, diagnostic)| to_lsp_diagnostic(diagnostic, map))
        .collect()
}

/// One compiled surface artifact, ready for its graph input bucket. Cloned
/// out of the memo on every snapshot rebuild (the build consumes owned
/// artifacts; the memo keeps the compile's product).
#[derive(Clone)]
enum SnapshotArtifact {
    /// A `.mox` domain model.
    Domain(phero_ir::Model),
    /// An `.actor` policy artifact.
    Actor(phero_ir::ActorModel),
    /// A `.ddd` design artifact.
    Design(phero_ir::ddd::DddModel),
    /// An `.evt` event-contract artifact.
    Event(phero_ir::events::EventModel),
    /// A `.deploy` deployment artifact.
    Deploy(phero_ir::deploy::DeployModel),
    /// An `.ifml` interaction-flow artifact.
    Flow(phero_ir::ifml::IfmlModel),
    /// A `.uc` use-case artifact.
    UseCase(phero_ir::usecases::UseCaseModel),
    /// A `.cmmn` case-management artifact.
    Case(phero_ir::cmmn::CmmnModel),
    /// A `.dmn` decision-model artifact.
    Decision(phero_ir::dmn::DmnModel),
    /// An `.intent` intent-specification artifact (issue pherolang#196): the
    /// requirement nodes and their traceability edges, fed to the graph's
    /// intent bucket (issue pherolang#363).
    Intent(phero_ir::intents::IntentModel),
}

/// Files one artifact into its graph input bucket.
fn push_artifact(
    inputs: &mut GraphInputs,
    display: String,
    path: &Path,
    imports: Vec<String>,
    artifact: SnapshotArtifact,
) {
    let key = path.display().to_string();
    match artifact {
        SnapshotArtifact::Domain(model) => inputs
            .domains
            .push(GraphFile::new(display, key, imports, model)),
        SnapshotArtifact::Actor(model) => inputs
            .actors
            .push(GraphFile::new(display, key, imports, model)),
        SnapshotArtifact::Design(model) => inputs
            .designs
            .push(GraphFile::new(display, key, imports, model)),
        SnapshotArtifact::Event(model) => inputs
            .events
            .push(GraphFile::new(display, key, imports, model)),
        SnapshotArtifact::Deploy(model) => inputs
            .deploys
            .push(GraphFile::new(display, key, imports, model)),
        SnapshotArtifact::Flow(model) => inputs
            .flows
            .push(GraphFile::new(display, key, imports, model)),
        SnapshotArtifact::UseCase(model) => inputs
            .use_cases
            .push(GraphFile::new(display, key, imports, model)),
        SnapshotArtifact::Case(model) => inputs
            .cases
            .push(GraphFile::new(display, key, imports, model)),
        SnapshotArtifact::Decision(model) => inputs
            .decisions
            .push(GraphFile::new(display, key, imports, model)),
        SnapshotArtifact::Intent(model) => inputs
            .intents
            .push(GraphFile::new(display, key, imports, model)),
    }
}

/// The document surface a path names, by extension — [`super::surface_of_path`]
/// lifted into the classifier the snapshot and publish pipeline match on
/// (`None` for a file no surface owns).
pub(super) fn surface_of_path(path: &Path) -> Option<DocumentSurface> {
    super::surface_of_path(path).map(DocumentSurface::Known)
}

/// `true` for a formatter-golden `X.fmt.<ext>` file — an output, not a
/// standalone model (the `check` project-mode rule).
pub(super) fn is_formatter_golden(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.contains(".fmt."))
}

/// The root-relative display path of one directory member: its file name
/// (the directory is the root).
fn display_path(path: &Path) -> Option<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

/// The spanned index of one file, built from its text alone — an index for
/// a source that does not parse is `None` (the other surfaces' builders
/// yield an empty index; `.mox`/`.ifml` project a parse, so absence is the
/// equivalent answer).
///
/// `application` is the root application name for a `.ddd` **fragment**
/// (DD8): a fragment's declarations are keyed by their merged application's
/// name, which its own source does not carry, so the snapshot supplies it.
/// Every other surface ignores it.
fn surface_index(
    surface: DocumentSurface,
    text: &str,
    display: &str,
    application: Option<&str>,
) -> Option<SurfaceIndex> {
    Some(match surface {
        DocumentSurface::Known(SurfaceKind::Mox) => mox_index(text),
        DocumentSurface::Known(SurfaceKind::Actor) => phero_driver::navigation::actors_index(text),
        DocumentSurface::Known(SurfaceKind::Ddd) => {
            phero_driver::navigation::ddd_index_with_application(text, application)
        }
        DocumentSurface::Known(SurfaceKind::Evt) => {
            phero_driver::navigation::evt_index(text, display)
        }
        DocumentSurface::Known(SurfaceKind::Deploy) => phero_driver::navigation::deploy_index(text),
        DocumentSurface::Known(SurfaceKind::Ifml) => {
            let (model, index) = phero_ifml::parse_ifml_indexed(text).ok()?;
            ifml_surface(&model, &index)
        }
        DocumentSurface::Known(SurfaceKind::Uc) => phero_driver::navigation::uc_index(text),
        DocumentSurface::Known(SurfaceKind::Cmmn) => phero_driver::navigation::cmmn_index(text),
        DocumentSurface::Known(SurfaceKind::Dmn) => {
            phero_driver::navigation::dmn_index(text, display)
        }
        // Intents carry no span table (v0.5): the file compiles in the
        // snapshot, but it owns no definitions, so it contributes no index.
        DocumentSurface::Known(SurfaceKind::Intent) => return None,
        DocumentSurface::Other => return None,
    })
}

/// The resolved import keys of one file: every written import string joined
/// with the file's directory, canonicalized where the file exists — the
/// host-normalized keys the graph clusters applications on. The publish
/// pipeline's compile-time joins reuse this (pherolang#249), so graph
/// clustering and capability pooling can never see different edges.
pub(super) fn resolved_imports(surface: DocumentSurface, text: &str, path: &Path) -> Vec<String> {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let mut keys: Vec<String> = written_imports(surface, text)
        .into_iter()
        .map(|import| {
            let resolved = directory.join(import);
            std::fs::canonicalize(&resolved)
                .map(|canonical| canonical.display().to_string())
                .unwrap_or_else(|_| resolved.display().to_string())
        })
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

/// The `import "…"` strings one surface's source writes.
fn written_imports(surface: DocumentSurface, text: &str) -> Vec<String> {
    match surface {
        // `.mox` imports are schema/sigil content, not application files.
        DocumentSurface::Known(SurfaceKind::Mox) | DocumentSurface::Other => Vec::new(),
        DocumentSurface::Known(SurfaceKind::Actor) => phero_syntax::parse_actors(text)
            .ast
            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
            .unwrap_or_default(),
        DocumentSurface::Known(SurfaceKind::Ddd) => phero_syntax::parse_ddd(text)
            .ast
            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
            .unwrap_or_default(),
        DocumentSurface::Known(SurfaceKind::Evt) => phero_syntax::parse_evt(text)
            .ast
            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
            .unwrap_or_default(),
        DocumentSurface::Known(SurfaceKind::Deploy) => phero_syntax::parse_deploy(text)
            .ast
            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
            .unwrap_or_default(),
        DocumentSurface::Known(SurfaceKind::Uc) => phero_syntax::parse_usecases(text)
            .ast
            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
            .unwrap_or_default(),
        DocumentSurface::Known(SurfaceKind::Cmmn) => phero_syntax::parse_cmmn(text)
            .ast
            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
            .unwrap_or_default(),
        DocumentSurface::Known(SurfaceKind::Intent) => phero_syntax::parse_intents(text)
            .ast
            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
            .unwrap_or_default(),
        DocumentSurface::Known(SurfaceKind::Dmn) => phero_driver::project::dmn_import_paths(text),
        DocumentSurface::Known(SurfaceKind::Ifml) => phero_ifml::parse_ifml(text)
            .map(|model| model.imports)
            .unwrap_or_default(),
    }
}

/// Binds one surface reference through the graph index, scoped to the
/// referring file's application.
fn resolve<'a>(
    index: &GraphIndex<'a>,
    file: &SnapshotFile,
    reference: &IndexedReference,
) -> Option<&'a GraphNode> {
    index.resolve_in(
        reference.target_kinds,
        &reference.name,
        index.application_of(&file.display),
    )
}

/// The node a **definition** request lands on: an action's bound designed
/// operation when the graph carries exactly one `binds` edge (the `.ifml`
/// payoff), else the node itself.
fn definition_target<'a>(index: &GraphIndex<'a>, node: &'a GraphNode) -> &'a GraphNode {
    if node.kind == kinds::ACTION {
        let mut bound = index
            .edges_from(&node.id)
            .iter()
            .filter(|edge| edge.kind == kinds::BINDS)
            .filter_map(|edge| index.node(&edge.to));
        if let (Some(target), None) = (bound.next(), bound.next()) {
            return target;
        }
    }
    node
}

/// Cross-surface go-to-definition: a declaration answers with its own
/// location; a reference answers with the location of the node the graph
/// binds it to.
pub(super) fn graph_goto_definition(
    snapshot: &GraphSnapshot,
    uri: &Uri,
    position: Position,
) -> Option<Location> {
    let file = snapshot.file_for_uri(uri)?;
    let index = snapshot.index();
    let map = PositionMap::new(&file.text);
    let offset = map.offset_for(position);
    match file.index.at(offset) {
        SurfaceLookup::Definition(definition) => Some(Location {
            uri: file.uri.clone(),
            range: map.range_for(definition.name_span),
        }),
        SurfaceLookup::Reference(reference) => {
            let node = resolve(&index, file, reference)?;
            let target = definition_target(&index, node);
            if target.kind == kinds::PLATFORM_CAPABILITY {
                // No declaration site: land on the word's first written
                // `requires` mention instead (pherolang#262).
                return snapshot.platform_anchor(target);
            }
            snapshot.location(target)
        }
        SurfaceLookup::None => None,
    }
}

/// Cross-surface hover: the graph node's facts (a capability's class and
/// holders, an action's bound operation, an event's payload, a component's
/// design binding).
pub(super) fn graph_hover(
    snapshot: &GraphSnapshot,
    uri: &Uri,
    position: Position,
) -> Option<Hover> {
    let file = snapshot.file_for_uri(uri)?;
    let index = snapshot.index();
    let offset = PositionMap::new(&file.text).offset_for(position);
    let node = match file.index.at(offset) {
        SurfaceLookup::Definition(definition) => {
            // A declaration the graph does not model as a node still hovers
            // with its kind and name, plus the open file's provenance footer.
            return Some(match snapshot.node_of(&index, file, definition) {
                Some(node) => node_hover(&index, node),
                None => with_provenance_footer(simple_hover(definition), &file.display),
            });
        }
        SurfaceLookup::Reference(reference) => resolve(&index, file, reference)?,
        SurfaceLookup::None => return None,
    };
    Some(node_hover(&index, node))
}

/// Appends the provenance footer for `file` to an already-rendered hover.
///
/// The surface is the file extension's; a file whose extension names no
/// surface keeps the hover exactly as it was (there is no docs page to
/// link).
fn with_provenance_footer(mut hover: Hover, file: &str) -> Hover {
    let Some(surface) = phero_highlight::Surface::from_path(Path::new(file)) else {
        return hover;
    };
    if let HoverContents::Markup(markup) = &mut hover.contents {
        markup
            .value
            .push_str(&crate::hover::provenance_footer(surface, file));
    }
    hover
}

/// The markdown hover of one graph node.
fn node_hover(index: &GraphIndex<'_>, node: &GraphNode) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: hover_markdown(index, node),
        }),
        range: None,
    }
}

/// The bare kind/name hover for a declaration no graph node matches.
fn simple_hover(definition: &IndexedDefinition) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!("**{} {}**", definition.kind, definition.name),
        }),
        range: None,
    }
}

/// Renders one graph node's hover markdown: the kind and name, then the
/// edges that make the node interesting to a reader, then — when the node's
/// file names a known surface — the provenance footer.
fn hover_markdown(index: &GraphIndex<'_>, node: &GraphNode) -> String {
    let mut lines = vec![format!("**{} {}**", node.kind, node.name)];
    let outgoing = index.edges_from(&node.id);
    let incoming = index.edges_to(&node.id);
    let target_names = |edge_kind: &str| -> Vec<String> {
        outgoing
            .iter()
            .filter(|edge| edge.kind == edge_kind)
            .filter_map(|edge| index.node(&edge.to))
            .map(|target| format!("`{}`", target.name))
            .collect()
    };
    let holder_names = |edge_kind: &str| -> Vec<String> {
        incoming
            .iter()
            .filter(|edge| edge.kind == edge_kind)
            .filter_map(|edge| index.node(&edge.from))
            .map(|holder| format!("`{}`", holder.name))
            .collect()
    };
    let push_line = |lines: &mut Vec<String>, label: &str, values: Vec<String>| {
        if !values.is_empty() {
            lines.push(format!("{label} {}", values.join(", ")));
        }
    };
    match node.kind.as_str() {
        kinds::CAPABILITY => {
            push_line(&mut lines, "on", target_names(kinds::ON));
            let mut permitted = holder_names(kinds::PERMITS);
            permitted.extend(holder_names(kinds::DELEGATES));
            push_line(&mut lines, "permitted to", permitted);
            push_line(&mut lines, "forbidden to", holder_names(kinds::FORBIDS));
            push_line(&mut lines, "never with", target_names(kinds::EXCLUDES));
            push_line(&mut lines, "requires", holder_names(kinds::REQUIRES));
        }
        kinds::ACTOR => {
            push_line(&mut lines, "extends", target_names(kinds::EXTENDS));
            push_line(&mut lines, "permits", target_names(kinds::PERMITS));
            push_line(&mut lines, "forbids", target_names(kinds::FORBIDS));
            push_line(&mut lines, "requires", holder_names(kinds::REQUIRES));
        }
        kinds::EVENT => {
            let payload: Vec<String> = outgoing
                .iter()
                .filter(|edge| edge.kind == kinds::TYPE_OF)
                .map(|edge| match edge.attributes.get("field") {
                    Some(field) => format!("`{field}`"),
                    None => format!("`{}`", edge.to),
                })
                .collect();
            push_line(&mut lines, "payload", payload);
            push_line(&mut lines, "published by", holder_names(kinds::PUBLISHES));
            push_line(&mut lines, "subscribed by", holder_names(kinds::SUBSCRIBES));
            push_line(&mut lines, "emitted by", holder_names(kinds::EMITS));
        }
        kinds::ACTION => {
            push_line(&mut lines, "binds", target_names(kinds::BINDS));
            push_line(
                &mut lines,
                "invoked by",
                holder_names(kinds::INVOKES_ACTION),
            );
        }
        kinds::DEPLOY_COMPONENT => {
            push_line(&mut lines, "design", target_names(kinds::BINDS_DESIGN));
            push_line(&mut lines, "flow", target_names(kinds::BINDS_FLOW));
            push_line(&mut lines, "requires", target_names(kinds::REQUIRES));
            push_line(&mut lines, "connects to", target_names(kinds::CONNECTS));
        }
        kinds::PLATFORM_CAPABILITY => {
            push_line(&mut lines, "required by", holder_names(kinds::REQUIRES));
            let providers: Vec<String> = phero_driver::platform_capability_targets(&node.name)
                .into_iter()
                .map(|target| format!("`{}`", target.keyword()))
                .collect();
            push_line(&mut lines, "provided by", providers);
        }
        kinds::SERVICE_OPERATION | kinds::REPOSITORY_OPERATION => {
            push_line(&mut lines, "carries", target_names(kinds::CARRIES));
            push_line(&mut lines, "invokes", target_names(kinds::INVOKES_DECISION));
            push_line(&mut lines, "emits", target_names(kinds::EMITS));
            push_line(&mut lines, "throws", target_names(kinds::THROWS));
        }
        kinds::ERROR => {
            push_line(&mut lines, "raised by", holder_names(kinds::THROWS));
        }
        _ => {}
    }
    let mut markdown = lines.join("\n");
    if let Some(surface) = phero_highlight::Surface::from_path(Path::new(&node.file)) {
        markdown.push_str(&crate::hover::provenance_footer(surface, &node.file));
    }
    markdown
}

/// Document symbols for a file the snapshot covers, straight from its
/// spanned index (one symbol per declaration, flat).
pub(super) fn graph_document_symbols(
    snapshot: &GraphSnapshot,
    uri: &Uri,
) -> Option<DocumentSymbolResponse> {
    let file = snapshot.file_for_uri(uri)?;
    let map = PositionMap::new(&file.text);
    #[allow(deprecated)] // `DocumentSymbol::deprecated` must be set explicitly
    let symbols: Vec<DocumentSymbol> = file
        .index
        .definitions
        .iter()
        .map(|definition| DocumentSymbol {
            name: definition.name.clone(),
            detail: Some(definition.kind.clone()),
            kind: lsp_kind(&definition.kind),
            tags: None,
            deprecated: None,
            range: map.range_for(definition.full_span),
            selection_range: map.range_for(definition.name_span),
            children: None,
        })
        .collect();
    Some(DocumentSymbolResponse::Nested(symbols))
}

/// Maps a graph node kind to its LSP symbol kind.
fn lsp_kind(kind: &str) -> SymbolKind {
    match kind {
        kinds::CLASS
        | kinds::ENUM
        | kinds::DATATYPE
        | kinds::VIEW
        | kinds::SERVICE
        | kinds::REPOSITORY
        | kinds::DTO
        | kinds::ERROR => SymbolKind::CLASS,
        kinds::FEATURE | kinds::CASE_FILE_ITEM | kinds::DEPLOY_COMPONENT => SymbolKind::FIELD,
        kinds::OPERATION
        | kinds::SERVICE_OPERATION
        | kinds::REPOSITORY_OPERATION
        | kinds::SEARCH
        | kinds::ACTION
        | kinds::COMMAND
        | kinds::CAPABILITY
        | kinds::DECISION
        | kinds::BKM => SymbolKind::METHOD,
        kinds::ACTOR | kinds::CASE_ROLE | kinds::INPUT_DATA => SymbolKind::INTERFACE,
        kinds::EVENT | kinds::SUBSCRIPTION | kinds::TRANSITION | kinds::PLAN_ITEM => {
            SymbolKind::EVENT
        }
        kinds::CHANNEL | kinds::COMPONENT | kinds::CONTAINER | kinds::DEPLOY_PROFILE => {
            SymbolKind::OBJECT
        }
        kinds::MODULE
        | kinds::DESIGN_APPLICATION
        | kinds::FLOW
        | kinds::DECISION_MODEL
        | kinds::CASE
        | kinds::DEPLOY_APPLICATION
        | kinds::KNOWLEDGE_SOURCE => SymbolKind::MODULE,
        kinds::USE_CASE | kinds::DECISION_SERVICE | kinds::DEPLOYMENT => SymbolKind::CLASS,
        _ => SymbolKind::VARIABLE,
    }
}

/// Every reference the graph binds to the node under the cursor.
pub(super) fn graph_references(
    snapshot: &GraphSnapshot,
    uri: &Uri,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let file = snapshot.file_for_uri(uri)?;
    let index = snapshot.index();
    let offset = PositionMap::new(&file.text).offset_for(position);
    let node = match file.index.at(offset) {
        SurfaceLookup::Definition(definition) => snapshot.node_of(&index, file, definition)?,
        SurfaceLookup::Reference(reference) => resolve(&index, file, reference)?,
        SurfaceLookup::None => return None,
    };
    Some(snapshot.references_to(&index, node, include_declaration))
}

/// What the cursor names for a rename: the graph node plus whether this
/// document owns its declaration.
enum RenameTarget<'a> {
    /// The name is declared in the document the rename started in.
    Owned(&'a GraphNode),
    /// The name is declared elsewhere — refused, naming that file.
    Foreign {
        node: &'a GraphNode,
        /// The owning file's display path.
        file: String,
    },
    /// The name is a word of the closed `.deploy` platform-capability
    /// vocabulary (pherolang#262) — renaming it would produce an
    /// unknown-capability compile error, so it is refused.
    Closed(&'a GraphNode),
}

/// Resolves the rename target under `position`.
///
/// A declaration site is owned by the document it appears in. A reference
/// site is owned by the file that declares the node the graph binds it to —
/// so a `.ddd` `capability` clause naming a `.actor` capability is
/// [`RenameTarget::Foreign`], and the rename is refused with the owner
/// named. The rule is the same one the artifact's `memberOf` edge encodes:
/// a declaration belongs to exactly one file.
fn rename_target<'a>(
    snapshot: &'a GraphSnapshot,
    index: &GraphIndex<'a>,
    file: &'a SnapshotFile,
    position: Position,
) -> Option<RenameTarget<'a>> {
    let offset = PositionMap::new(&file.text).offset_for(position);
    let target = |node: &'a GraphNode| {
        if node.kind == kinds::PLATFORM_CAPABILITY {
            RenameTarget::Closed(node)
        } else if node.file == file.display {
            RenameTarget::Owned(node)
        } else {
            RenameTarget::Foreign {
                node,
                file: node.file.clone(),
            }
        }
    };
    match file.index.at(offset) {
        SurfaceLookup::Definition(definition) => {
            snapshot.node_of(index, file, definition).map(target)
        }
        SurfaceLookup::Reference(reference) => resolve(index, file, reference).map(target),
        SurfaceLookup::None => None,
    }
}

/// The refusal message for a closed-vocabulary name: the word is not
/// authorable, so the rename cannot mean anything (pherolang#262).
fn closed_vocabulary_refusal(node: &GraphNode) -> String {
    format!(
        "cannot rename {} `{}`: it is part of the closed deploy capability vocabulary",
        node.kind, node.name
    )
}

/// The refusal message for a cross-document rename: it always names the
/// owning file (the issue's "explicit message").
fn foreign_refusal(node: &GraphNode, file: &str) -> String {
    format!(
        "cannot rename {} `{}`: it is declared in {file}; open that file to rename it",
        node.kind, node.name
    )
}

/// Cross-surface prepare-rename: the declaration's name range when this
/// document owns it, an error naming the owner otherwise.
pub(super) fn graph_prepare_rename(
    snapshot: &GraphSnapshot,
    uri: &Uri,
    position: Position,
) -> Result<Option<Range>, String> {
    let file = snapshot
        .file_for_uri(uri)
        .ok_or_else(|| "the document is not part of the current graph snapshot".to_string())?;
    let index = snapshot.index();
    match rename_target(snapshot, &index, file, position) {
        Some(RenameTarget::Owned(node)) => {
            Ok(snapshot
                .file(&node.file)
                .and_then(|owner| {
                    owner.index.definitions.iter().find(|definition| {
                        definition.kind == node.kind && definition.name == node.name
                    })
                })
                .map(|definition| PositionMap::new(&file.text).range_for(definition.name_span)))
        }
        Some(RenameTarget::Foreign { node, file }) => Err(foreign_refusal(node, &file)),
        Some(RenameTarget::Closed(node)) => Err(closed_vocabulary_refusal(node)),
        None => Ok(None),
    }
}

/// Cross-surface rename with the ownership rule: the declaration and every
/// reference **in this document** that the graph binds to it. A reference
/// to another document's declaration is refused, naming that file.
pub(super) fn graph_rename(
    snapshot: &GraphSnapshot,
    uri: &Uri,
    position: Position,
    new_name: &str,
) -> Result<Option<WorkspaceEdit>, String> {
    let file = snapshot
        .file_for_uri(uri)
        .ok_or_else(|| "the document is not part of the current graph snapshot".to_string())?;
    let index = snapshot.index();
    let node = match rename_target(snapshot, &index, file, position) {
        Some(RenameTarget::Owned(node)) => node,
        Some(RenameTarget::Foreign { node, file }) => {
            return Err(foreign_refusal(node, &file));
        }
        Some(RenameTarget::Closed(node)) => {
            return Err(closed_vocabulary_refusal(node));
        }
        None => return Ok(None),
    };
    let mut spans: Vec<phero_driver::Span> = file
        .index
        .definitions_of(&node.kind, &node.name)
        .into_iter()
        .map(|definition| definition.name_span)
        .collect();
    for reference in &file.index.references {
        if !reference.targets_kind(&node.kind) {
            continue;
        }
        if resolve(&index, file, reference).is_some_and(|target| target.id == node.id) {
            spans.push(reference.span);
        }
    }
    if spans.is_empty() {
        return Ok(None);
    }
    let map = PositionMap::new(&file.text);
    // Latest source position first — safe for overlapping edits.
    spans.sort_unstable_by_key(|span| std::cmp::Reverse(span.start));
    let edits = spans
        .into_iter()
        .map(|span| TextEdit {
            range: map.range_for(span),
            new_text: new_name.to_string(),
        })
        .collect();
    Ok(Some(WorkspaceEdit {
        changes: Some([(file.uri.clone(), edits)].into_iter().collect()),
        document_changes: None,
        change_annotations: None,
    }))
}
