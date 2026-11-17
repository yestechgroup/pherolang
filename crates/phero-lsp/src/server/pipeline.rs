//! Document state, compilation, and diagnostic plumbing: the
//! `PheroBackend` methods behind every LSP capability — the per-surface
//! publish pipeline, the `.ifml` lenient compile and import closure, the
//! `.ddd`/`.dmn` domain-source resolution and the `.dmn` decision-table
//! reader, and the snapshot/lock adapters the request handlers run
//! through.

use phero_driver::{NavigationIndex, SurfaceKind};
use tower_lsp_server::ls_types::*;

use super::graph::{is_formatter_golden, resolved_imports, surface_of_path};
use super::{DocumentSurface, PheroBackend, SOURCE_NAME};
use crate::position::PositionMap;

/// One candidate file of a directory snapshot: where it lives, its surface,
/// and the text the application relation reads (an open buffer wins over
/// disk).
type DirectoryCandidate = (std::path::PathBuf, DocumentSurface, String);

/// The import closure of one `.ifml` document, gathered by
/// [`PheroBackend::import_closure`].
#[derive(Default)]
pub(super) struct IfmlImportClosure {
    /// The resolver bundle, keyed `(importer identity, import-as-written)`.
    pub(super) bundle: phero_ifml::IfmlImports,
    /// Imported `.mox` domain sources, deduplicated by resolved path.
    pub(super) domains: Vec<(String, String)>,
    /// Imported `.actor` policies, compiled — the authorization dimension
    /// `check_ifml_with_actors` resolves `requires:`/`roles:` against.
    pub(super) actors: Vec<phero_ir::ActorModel>,
    /// Imported `.dmn` decision models, compiled — the decision dimension
    /// `check_ifml_with_decisions` resolves `data:` names against.
    pub(super) decisions: Vec<phero_ir::dmn::DmnModel>,
    /// Resolved disk path per import identity (the main file included).
    disk_paths: std::collections::BTreeMap<String, std::path::PathBuf>,
    /// URL (or identity string) of every fetched import — the transitive
    /// import set.
    pub(super) import_urls: Vec<String>,
}

/// The gathered compile inputs of one open `.dmn` document (see
/// [`PheroBackend::dmn_sources_for`]): one compile-input bundle carrying the
/// imported `.mox` domain sources, the `import schema`/`import sigil`
/// content they declare, the decision-table contents the `logic table`
/// clauses name, and the compiled `.evt` contracts the `import
/// "<path>.evt"` lines name (rule 14's bucket).
pub(super) struct DmnSources {
    pub(super) inputs: phero_driver::CompileInputs,
}

/// The compiled state of one open `.ifml` document for the navigation
/// handlers: the compilation (its index carries resolver-populated
/// `resolved_module` links) plus the resolved URI of every indexed file,
/// in [`phero_ifml::IfmlIndex::files`] order.
pub(super) struct IfmlSnapshot {
    pub(super) compilation: phero_ifml::IfmlCompilation,
    pub(super) file_urls: Vec<String>,
    /// The union of imported `.mox` domains — feature completions resolve
    /// against it. `None` when the flow imports no domains.
    pub(super) domains: Option<phero_ir::Model>,
}

impl PheroBackend {
    /// Recompiles the document and publishes its diagnostics, then
    /// re-publishes every open `.ifml` document that imports it — an edit
    /// to an imported file must refresh its consumers or their diagnostics
    /// go stale.
    pub(super) async fn publish(&self, uri: &Uri, version: Option<i32>) {
        self.publish_document(uri, version).await;
        for dependent in self.dependents_of(uri) {
            self.publish_document(&dependent, None).await;
        }
    }

    /// Recompiles one document and publishes its diagnostics.
    pub(super) async fn publish_document(&self, uri: &Uri, version: Option<i32>) {
        let snapshot = {
            let documents = self.documents.lock().unwrap();
            documents
                .get(uri)
                .map(|document| (document.surface, document.text.clone(), document.file))
        };
        let Some((surface, text, file)) = snapshot else {
            return;
        };
        let map = PositionMap::new(&text);
        let lsp_diagnostics = match surface {
            DocumentSurface::Known(SurfaceKind::Mox) => {
                let Some(file) = file else {
                    return;
                };
                // Workspace-aware: `import schema`/`import sigil` resolve
                // from disk like the CLI's, so models carrying them stop
                // showing permanent false "not provided" errors. Documents
                // key by URI; imports resolve relative to the file on disk.
                let imports = uri
                    .to_file_path()
                    .and_then(|path| {
                        phero_driver::workspace_imports::collect_for_source(
                            uri.as_str(),
                            &path,
                            &text,
                        )
                        .ok()
                    })
                    .unwrap_or_default();
                let db = self.db.lock().unwrap();
                phero_driver::compile_with_imports(&*db, file, &imports)
                    .diagnostics
                    .iter()
                    .map(|diagnostic| to_lsp_diagnostic(diagnostic, &map))
                    .collect()
            }
            DocumentSurface::Known(SurfaceKind::Ifml) => {
                let (_snapshot, diagnostics) = self.compile_ifml(uri, &text);
                diagnostics
                    .iter()
                    .filter(|(path, _)| path == uri.as_str())
                    .map(|(_, diagnostic)| to_lsp_ifml_diagnostic(diagnostic, &map))
                    .collect()
            }
            DocumentSurface::Known(SurfaceKind::Ddd) => {
                // The driver keys by the disk path when the document lives
                // on disk — the CLI's semantics, where imports resolve
                // lexically against the real path and sibling artifacts
                // share one key space. The URI string stays the fallback
                // for documents that have none (and remains the publish
                // filter). Before pherolang#128 this compiled under the URI
                // string, which silently broke import resolution.
                let driver_path = uri
                    .to_file_path()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| uri.as_str().to_string());
                // pherolang#128 (J3 at compile time): the capability clauses
                // resolve against every `.actor` artifact of the open
                // design's directory — the empty slice is the exact
                // `compile_ddd_str` behavior. pherolang#78 (rule 17): the
                // `invokes`/`inject decision` clauses resolve the same way
                // against every `.dmn` artifact of the directory.
                let actors = self.sibling_actor_models(uri);
                let decisions = self.sibling_decision_models(uri);
                // DD8 (issue pherolang#305): a fragment compiles through
                // its root; a root gathers its fragment closure. The import
                // closure spans the root **and** every fragment, each
                // resolved relative to its declaring file.
                let app = self.ddd_app_sources(uri, &text);
                let (root_path, root_text, _) =
                    app.unwrap_or((driver_path.clone(), text.clone(), Vec::new()));
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
                let inputs = domain_inputs(&gathered.domains)
                    .with_actors(actors)
                    .with_decisions(decisions);
                phero_driver::compile_ddd_app_str(&root_path, &root_text, &fragments, &inputs)
                    .diagnostics
                    .iter()
                    .filter(|(file, _)| file == &driver_path)
                    .map(|(_, diagnostic)| to_lsp_diagnostic(diagnostic, &map))
                    .collect()
            }
            DocumentSurface::Known(SurfaceKind::Actor) => {
                let driver_path = uri
                    .to_file_path()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| uri.as_str().to_string());
                let inputs = self.actor_sources_for(uri, &text);
                // pherolang#128 (J4 at compile time): grant coverage resolves
                // against every `.ddd` artifact of the open policy's
                // directory — the empty slice is the exact
                // `compile_actors_str` behavior.
                let designs = self.sibling_design_models(uri);
                let inputs = inputs.with_designs(designs);
                phero_driver::compile_actors_str(&driver_path, &text, &inputs)
                    .diagnostics
                    .iter()
                    .filter(|(file, _)| file == &driver_path)
                    .map(|(_, diagnostic)| to_lsp_diagnostic(diagnostic, &map))
                    .collect()
            }
            DocumentSurface::Known(SurfaceKind::Dmn) => {
                // The driver keys by the disk path when the document lives
                // on disk — the CLI's semantics, where imports resolve
                // lexically against the real path and sibling designs pool
                // this artifact by the same key. The URI string stays the
                // fallback for documents that have none (and remains the
                // publish filter).
                let driver_path = uri
                    .to_file_path()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| uri.as_str().to_string());
                let sources = self.dmn_sources_for(uri, &text);
                phero_driver::compile_dmn_str(&driver_path, &text, &sources.inputs)
                    .diagnostics
                    .iter()
                    .filter(|(file, _)| file == &driver_path)
                    .map(|(_, diagnostic)| to_lsp_diagnostic(diagnostic, &map))
                    .collect()
            }
            // The four artifact surfaces compile only inside the graph
            // snapshot (pherolang#243/#254): publish the diagnostics its
            // memoized compile already produced for this file instead of
            // compiling a second time (pherolang#255). A non-file URI (or one
            // with no directory) has no snapshot — publish nothing, never a
            // bogus empty set; a clean file publishes an empty vector, which
            // clears its stale diagnostics.
            DocumentSurface::Known(SurfaceKind::Evt)
            | DocumentSurface::Known(SurfaceKind::Deploy)
            | DocumentSurface::Known(SurfaceKind::Uc)
            | DocumentSurface::Known(SurfaceKind::Cmmn) => self.snapshot_diagnostics(uri),
            DocumentSurface::Other => Vec::new(),
        };
        self.client
            .publish_diagnostics(uri.clone(), lsp_diagnostics, version)
            .await;
    }

    /// Compiles one open `.ifml` document: its import tree is gathered
    /// through [`PheroBackend::import_closure`] — open documents first, then
    /// disk relative to the importing file — the imported `.mox` files are
    /// compiled and unioned into the typed-binding domain, the imported
    /// `.actor` files are compiled into the authorization dimension, and
    /// the file is compiled and checked in one pass. Diagnostics
    /// referencing other files are dropped here (they surface when that
    /// file is opened).
    pub(super) fn compile_ifml(
        &self,
        uri: &Uri,
        text: &str,
    ) -> (
        Option<IfmlSnapshot>,
        Vec<(String, phero_ifml::Diagnostic<&'static str>)>,
    ) {
        let closure = self.import_closure(uri, text);
        let main_path = uri.to_string();
        // Lenient: diagnostics ride along, the index stays navigable on
        // broken code — completions and definitions must work while the
        // user types.
        let (compilation, mut diagnostics) =
            phero_ifml::compile_ifml_lenient(&main_path, text, &closure.bundle);
        let domain_union = domain_union(&closure.domains);
        let binding = phero_ifml::check_ifml_with_decisions(
            &compilation,
            domain_union.as_ref(),
            &closure.actors,
            &closure.decisions,
        );
        diagnostics.extend(binding);
        // Resolved URI per indexed file (same order as `index.files`), so
        // cross-file navigation can produce `Location`s. Unresolved
        // identities fall back to their as-written path, which `Uri::join`
        // can still resolve relative to the importer.
        let mut file_urls = Vec::with_capacity(compilation.index.files.len());
        for file in &compilation.index.files {
            let url = closure
                .disk_paths
                .get(&file.path)
                .and_then(Uri::from_file_path)
                .map(|url| url.to_string())
                .unwrap_or_else(|| file.path.clone());
            file_urls.push(url);
        }
        (
            Some(IfmlSnapshot {
                compilation,
                file_urls,
                domains: domain_union,
            }),
            diagnostics,
        )
    }

    /// The imported domain sources of one open `.ddd` document, plus the
    /// import bundle their `import schema`/`import sigil` declarations
    /// need, gathered through [`phero_driver::project::gather_domain_sources`].
    /// Import paths resolve relative to the design file's directory (the
    /// CLI's `read_ddd_design` semantics), and an open `.mox` document at
    /// the resolved path is preferred over its disk copy — editing an
    /// imported domain refreshes the design without a save. A missing
    /// import file is skipped here: the driver reports the unprovided
    /// import as a diagnostic.
    /// The root application and `.ddd` fragment closure of one open `.ddd`
    /// document (DD8): the document itself when it is a root, else the
    /// directory's root whose import closure reaches it. Open buffers win
    /// over disk. `None` when no root reaches a fragment.
    pub(super) fn ddd_app_sources(
        &self,
        uri: &Uri,
        text: &str,
    ) -> Option<(String, String, Vec<phero_driver::DddFragment>)> {
        let path = uri.to_file_path()?;
        let key = path.display().to_string();
        let is_root = |source: &str| {
            phero_syntax::parse_ddd(source)
                .ast
                .is_some_and(|ast| ast.application.is_some())
        };
        let (root_path, root_text) = if is_root(text) {
            (key, text.to_string())
        } else {
            let directory = path.parent()?;
            let mut found = None;
            for (candidate, surface, candidate_text) in self.directory_candidates(directory) {
                if surface != DocumentSurface::Known(SurfaceKind::Ddd) || !is_root(&candidate_text)
                {
                    continue;
                }
                let candidate_key = candidate.display().to_string();
                let mut fetch = |resolved: &str| self.fetch_open_or_disk(resolved);
                let gathered = phero_driver::project::gather_ddd_fragments(
                    &candidate_key,
                    &candidate_text,
                    &mut fetch,
                );
                if gathered
                    .fragments
                    .iter()
                    .any(|(fragment, _)| fragment == &key)
                {
                    found = Some((candidate_key, candidate_text));
                    break;
                }
            }
            found?
        };
        let mut fetch = |resolved: &str| self.fetch_open_or_disk(resolved);
        let gathered =
            phero_driver::project::gather_ddd_fragments(&root_path, &root_text, &mut fetch);
        let fragments = gathered
            .fragments
            .into_iter()
            .map(|(path, source)| phero_driver::DddFragment { path, source })
            .collect();
        Some((root_path, root_text, fragments))
    }

    /// The compile-input bundle of one open `.actor` policy: the `.mox`
    /// domains it imports (open documents win over disk) plus their import
    /// content.
    pub(super) fn actor_sources_for(&self, uri: &Uri, text: &str) -> phero_driver::CompileInputs {
        let mut domains: Vec<(String, String)> = Vec::new();
        if let Some(path) = uri.to_file_path() {
            let path_key = path.display().to_string();
            let mut fetch =
                |resolved: &str| -> Option<String> { self.fetch_open_or_disk(resolved) };
            domains = phero_driver::project::gather_domain_sources(
                &path_key,
                text,
                |source| {
                    phero_syntax::parse_actors(source)
                        .ast
                        .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
                        .unwrap_or_default()
                },
                &mut fetch,
            );
        }
        domain_inputs(&domains)
    }

    /// The `.mox` domains one open `.dmn` decision model imports (open
    /// documents win over disk), the import bundle its compile resolves
    /// against, the decision-table contents its `logic table` clauses
    /// name — read from disk next to the document through
    /// [`dmn_table_sources`] — and the compiled `.evt` contracts its
    /// `import "<path>.evt"` lines name (rule 14's bucket, gathered the
    /// same open-documents-first way). A table file that cannot be read is
    /// not provided: the driver's unprovided-table diagnostic (E0209)
    /// names the written path. A contract that cannot be read or compile
    /// contributes nothing: the driver's unprovided-import diagnostic
    /// (rule 1) names it — the editor degrades gracefully.
    pub(super) fn dmn_sources_for(&self, uri: &Uri, text: &str) -> DmnSources {
        let mut domains: Vec<(String, String)> = Vec::new();
        let mut tables: Vec<(String, String)> = Vec::new();
        let mut events: Vec<(String, phero_ir::events::EventModel)> = Vec::new();
        if let Some(path) = uri.to_file_path() {
            let path_key = path.display().to_string();
            let mut fetch =
                |resolved: &str| -> Option<String> { self.fetch_open_or_disk(resolved) };
            domains = phero_driver::project::gather_domain_sources(
                &path_key,
                text,
                |source| {
                    phero_driver::project::dmn_import_paths(source)
                        .into_iter()
                        .filter(|path| !path.ends_with(".evt"))
                        .collect()
                },
                &mut fetch,
            );
            events = phero_driver::project::gather_dmn_events(&path_key, text, &mut fetch)
                .unwrap_or_default();
            if let Some(dir) = path.parent() {
                tables = dmn_table_sources(dir, text);
            }
        }
        DmnSources {
            inputs: domain_inputs(&domains)
                .with_tables(tables)
                .with_events(events),
        }
    }

    /// The text for a resolved path — the open document that lives there
    /// wins over disk (the editor's buffer is the truth).
    pub(super) fn fetch_open_or_disk(&self, resolved: &str) -> Option<String> {
        let resolved = std::path::PathBuf::from(resolved);
        let open = {
            let documents = self.documents.lock().unwrap();
            documents.iter().find_map(|(open_uri, document)| {
                if document.surface != DocumentSurface::Known(SurfaceKind::Mox) {
                    return None;
                }
                let open_path = open_uri.to_file_path()?;
                (open_path == resolved).then(|| document.text.clone())
            })
        };
        open.or_else(|| std::fs::read_to_string(&resolved).ok())
    }

    /// The `(path, text)` pairs of one open document's **application**
    /// siblings with `extension`, sorted, excluding `except` — the pool the
    /// compile-time capability/design/decision joins gather from
    /// (pherolang#249: the import-connected application, never the
    /// directory). An open document wins over its on-disk text (the
    /// editor's buffer is the truth); files that cannot be read contribute
    /// nothing.
    ///
    /// The relation is recomputed per call — a `.ddd` publish gathers
    /// actors and decisions, so the directory is walked twice. It is a
    /// `read_dir` plus one parse per candidate: the expensive compile stays
    /// behind the graph snapshot's cache, and a second cache keyed by
    /// directory + open signature would only pay for itself on a much
    /// larger directory than the LSP's one-directory snapshot scope.
    fn siblings_in_application(
        &self,
        except: &std::path::Path,
        kind: SurfaceKind,
    ) -> Vec<(std::path::PathBuf, String)> {
        let Some(directory) = except.parent() else {
            return Vec::new();
        };
        let candidates = self.directory_candidates(directory);
        let files: Vec<phero_driver::applications::ApplicationFile> = candidates
            .iter()
            .map(
                |(path, surface, text)| phero_driver::applications::ApplicationFile {
                    path: normalize_path(path),
                    imports: resolved_imports(*surface, text, path),
                },
            )
            .collect();
        let except_key = normalize_path(except);
        let Some(except_index) = files.iter().position(|file| file.path == except_key) else {
            // The document itself is always a candidate (it is open and
            // lives in this directory); absence means no pool at all.
            return Vec::new();
        };
        let components = phero_driver::applications::applications(&files);
        let Some(component) = components
            .iter()
            .find(|members| members.contains(&except_index))
        else {
            return Vec::new();
        };
        component
            .iter()
            .filter_map(|&member| {
                let (path, _, text) = &candidates[member];
                (path != except && SurfaceKind::of_path(path) == Some(kind))
                    .then(|| (path.clone(), text.clone()))
            })
            .collect()
    }

    /// Every candidate file of the open document's directory snapshot
    /// (pherolang#249): each supported-surface file it holds (a
    /// formatter-golden `X.fmt.<ext>` skipped) plus every **open** document
    /// that lives there — open buffers win over disk copies and need not
    /// exist on disk. Unreadable files contribute nothing. This is the
    /// LSP's analog of the CLI's gathered project snapshot: imports leaving
    /// the directory contribute no application edge.
    fn directory_candidates(&self, directory: &std::path::Path) -> Vec<DirectoryCandidate> {
        let open: Vec<(std::path::PathBuf, DocumentSurface, String)> = {
            let documents = self.documents.lock().unwrap();
            documents
                .iter()
                .filter(|(uri, document)| {
                    document.surface != DocumentSurface::Other
                        && uri
                            .to_file_path()
                            .is_some_and(|path| path.parent() == Some(directory))
                })
                .filter_map(|(uri, document)| {
                    let path = uri.to_file_path()?.into_owned();
                    Some((path, document.surface, document.text.clone()))
                })
                .collect()
        };
        let mut candidates: std::collections::BTreeMap<std::path::PathBuf, DocumentSurface> =
            Default::default();
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
        for (path, surface, _) in &open {
            candidates.insert(path.clone(), *surface);
        }
        let mut snapshot: Vec<DirectoryCandidate> = candidates
            .into_iter()
            .filter_map(|(path, surface)| {
                let text = open
                    .iter()
                    .find(|(open_path, _, _)| *open_path == path)
                    .map(|(_, _, text)| text.clone())
                    .or_else(|| std::fs::read_to_string(&path).ok())?;
                Some((path, surface, text))
            })
            .collect();
        snapshot.sort_by(|left, right| left.0.cmp(&right.0));
        snapshot
    }

    /// The pooled `.actor` artifacts of the open design's **import-connected
    /// application** (pherolang#249; open documents win over disk): the
    /// capability table rule 10 resolves against at compile time
    /// (pherolang#128's J3). An empty set is exactly the plain
    /// `compile_ddd_str` behavior. A sibling that fails to compile
    /// contributes nothing — its own open editor shows its diagnostics.
    fn sibling_actor_models(&self, except: &Uri) -> Vec<(String, phero_ir::ActorModel)> {
        let Some(path) = except.to_file_path() else {
            return Vec::new();
        };
        let mut models = Vec::new();
        for (sibling, text) in self.siblings_in_application(&path, SurfaceKind::Actor) {
            let key = sibling.display().to_string();
            let mut fetch =
                |resolved: &str| -> Option<String> { self.fetch_open_or_disk(resolved) };
            let domains = phero_driver::project::gather_domain_sources(
                &key,
                &text,
                |source| {
                    phero_syntax::parse_actors(source)
                        .ast
                        .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
                        .unwrap_or_default()
                },
                &mut fetch,
            );
            let actor = phero_driver::project::ActorImport {
                key: key.clone(),
                source: text.clone(),
                domains,
            };
            if let Ok(compiled) =
                phero_driver::project::compile_actor_imports(std::slice::from_ref(&actor))
            {
                models.extend(compiled.into_iter().map(|model| (key.clone(), model)));
            }
        }
        models
    }

    /// The compiled `.ddd` artifacts of the open policy's **application**
    /// (pherolang#249; open documents win over disk): the carrier set grant
    /// coverage resolves against at compile time (pherolang#128's J4). Gathered
    /// exactly like [`PheroBackend::sibling_actor_models`] — an empty set is
    /// the plain `compile_actors_str` behavior; a sibling that fails to
    /// compile contributes nothing.
    fn sibling_design_models(&self, except: &Uri) -> Vec<(String, phero_ir::ddd::DddModel)> {
        let Some(path) = except.to_file_path() else {
            return Vec::new();
        };
        let mut models = Vec::new();
        for (sibling, text) in self.siblings_in_application(&path, SurfaceKind::Ddd) {
            // DD8: a fragment owns no artifact of its own — it is merged by
            // its root's compile, and the root may be a different sibling.
            if phero_syntax::parse_ddd(&text)
                .ast
                .is_none_or(|ast| ast.application.is_none())
            {
                continue;
            }
            let key = sibling.display().to_string();
            let mut fetch =
                |resolved: &str| -> Option<String> { self.fetch_open_or_disk(resolved) };
            let gathered = phero_driver::project::gather_ddd_app_imports(&key, &text, &mut fetch);
            let fragments: Vec<phero_driver::DddFragment> = gathered
                .fragments
                .iter()
                .map(|(path, source)| phero_driver::DddFragment {
                    path: path.clone(),
                    source: source.clone(),
                })
                .collect();
            let inputs = domain_inputs(&gathered.domains);
            let compilation = phero_driver::compile_ddd_app_str(&key, &text, &fragments, &inputs);
            if let Some(model) = compilation.model {
                models.push((key, model));
            }
        }
        models
    }

    /// The compiled `.dmn` artifacts of the open design's **application**,
    /// keyed by their disk paths (pherolang#249; rule 17's decisions bucket,
    /// pherolang#78 — the keys are what the provision check resolves `.dmn`
    /// import paths against). Gathered exactly like
    /// [`PheroBackend::sibling_design_models`] (open documents win over disk);
    /// a sibling that fails to compile contributes nothing. Decision tables
    /// resolve from disk next to each sibling through [`dmn_table_sources`] —
    /// an unprovided table names it and the sibling drops out. The sibling's
    /// imported `.evt` contracts gather the same way (rule 14's bucket), so a
    /// model with `emits` still compiles into the pool.
    fn sibling_decision_models(&self, except: &Uri) -> Vec<(String, phero_ir::dmn::DmnModel)> {
        let Some(path) = except.to_file_path() else {
            return Vec::new();
        };
        let mut models = Vec::new();
        for (sibling, text) in self.siblings_in_application(&path, SurfaceKind::Dmn) {
            let key = sibling.display().to_string();
            let mut fetch =
                |resolved: &str| -> Option<String> { self.fetch_open_or_disk(resolved) };
            let domains = phero_driver::project::gather_domain_sources(
                &key,
                &text,
                |source| {
                    phero_driver::project::dmn_import_paths(source)
                        .into_iter()
                        .filter(|path| !path.ends_with(".evt"))
                        .collect()
                },
                &mut fetch,
            );
            let events = phero_driver::project::gather_dmn_events(&key, &text, &mut fetch)
                .unwrap_or_default();
            let tables = sibling
                .parent()
                .map(|dir| dmn_table_sources(dir, &text))
                .unwrap_or_default();
            let inputs = domain_inputs(&domains)
                .with_tables(tables)
                .with_events(events);
            let compilation = phero_driver::compile_dmn_str(&key, &text, &inputs);
            if let Some(model) = compilation.model {
                models.push((key, model));
            }
        }
        models
    }

    /// The import closure of one `.ifml` document, gathered through
    /// [`phero_driver::project::gather_ifml_project`]: the resolver bundle,
    /// the imported `.mox` domain sources, the imported `.actor` policies
    /// (compiled by [`phero_driver::project::compile_actor_imports`]), the
    /// imported `.dmn` decision models (compiled by
    /// [`phero_driver::project::compile_decision_imports`]), every resolved
    /// disk path (identity → path), and the URLs of all fetched imports —
    /// the transitive import set that reverse-dependency lookups match
    /// against. Open documents win over disk (the editor's buffer is the
    /// truth). Anything unresolvable or non-compiling degrades
    /// silently: `.ifml` misses surface as resolver diagnostics, missing
    /// `.mox`/`.actor`/`.dmn` imports shrink the typed/authorization/
    /// decision dimensions (an empty actors or decisions slice skips the
    /// pass) — the editor must stay usable in incomplete workspaces; the
    /// CLI's hard error is the authoring gate.
    pub(super) fn import_closure(&self, uri: &Uri, text: &str) -> IfmlImportClosure {
        let main_path = uri.to_string();
        let mut closure = IfmlImportClosure::default();
        if let Some(path) = uri.to_file_path() {
            closure
                .disk_paths
                .insert(main_path.clone(), path.into_owned());
        }
        let dir_of = |path: &std::path::Path| -> std::path::PathBuf {
            path.parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf()
        };
        let documents = self.documents.lock().unwrap();
        let mut fetch = |importer: &str, import: &str| -> Option<(String, String)> {
            let importer_disk = closure.disk_paths.get(importer)?;
            let resolved = dir_of(importer_disk).join(import);
            let fetched = (|| {
                if let Some(url) = Uri::from_file_path(&resolved) {
                    if let Some(document) = documents.get(&url) {
                        return Some(document.text.clone());
                    }
                }
                std::fs::read_to_string(&resolved).ok()
            })()?;
            let url = Uri::from_file_path(&resolved)
                .map(|url| url.to_string())
                .unwrap_or_else(|| resolved.display().to_string());
            closure.import_urls.push(url);
            let key = resolved.display().to_string();
            // Both identities must resolve (the walker contract's host
            // obligation): the as-written import string (what the
            // resolver's index and nested `.ifml` walks use) and the
            // resolved key (what nested gathering hands back as an
            // importer identity — an `.actor` policy's own domain
            // imports).
            closure
                .disk_paths
                .insert(import.to_string(), resolved.clone());
            closure.disk_paths.insert(key.clone(), resolved);
            Some((key, fetched))
        };
        let project = phero_driver::project::gather_ifml_project(&main_path, text, &mut fetch);
        // The walk is done: release the document lock before compiling.
        drop(documents);
        closure.bundle = project.imports;
        closure.domains = project.domains;
        // A policy that fails to compile degrades to no authorization
        // checking rather than errors against a partial dimension,
        // mirroring `domain_union`.
        closure.actors =
            phero_driver::project::compile_actor_imports(&project.actors).unwrap_or_default();
        // The decision dimension degrades the same way: a model that
        // fails to compile contributes nothing (its own open editor
        // shows its diagnostics), and an empty slice skips the pass.
        closure.decisions =
            phero_driver::project::compile_decision_imports(&project.decisions).unwrap_or_default();
        closure
    }

    /// Open `.ifml` documents whose import closure contains `changed` —
    /// the reverse-dependency set that must be re-published when `changed`
    /// moves, or their diagnostics go stale.
    pub(super) fn dependents_of(&self, changed: &Uri) -> Vec<Uri> {
        let changed_key = changed.to_string();
        // Snapshot the open `.ifml` documents and drop the lock before
        // walking: `import_closure` takes the same lock.
        let candidates: Vec<(Uri, String)> = {
            let documents = self.documents.lock().unwrap();
            documents
                .iter()
                .filter(|(url, document)| {
                    **url != *changed
                        && document.surface == DocumentSurface::Known(SurfaceKind::Ifml)
                })
                .map(|(url, document)| (url.clone(), document.text.clone()))
                .collect()
        };
        let mut dependents = Vec::new();
        for (url, text) in candidates {
            let closure = self.import_closure(&url, &text);
            if closure.import_urls.contains(&changed_key) {
                dependents.push(url);
            }
        }
        dependents
    }

    /// Whether `uri` is an open `.ifml` document.
    pub(super) fn open_ifml_document(&self, uri: &Uri) -> bool {
        self.documents
            .lock()
            .unwrap()
            .get(uri)
            .is_some_and(|document| document.surface == DocumentSurface::Known(SurfaceKind::Ifml))
    }

    /// Compiles every open `.ifml` document into a snapshot. References
    /// and rename match by NAME across snapshots: a module's uses live in
    /// its dependents, which the focused document's import closure never
    /// sees.
    pub(super) fn open_ifml_snapshots(&self) -> Vec<IfmlSnapshot> {
        let docs: Vec<(Uri, String)> = {
            let documents = self.documents.lock().unwrap();
            documents
                .iter()
                .filter(|(_, document)| {
                    document.surface == DocumentSurface::Known(SurfaceKind::Ifml)
                })
                .map(|(url, document)| (url.clone(), document.text.clone()))
                .collect()
        };
        docs.iter()
            .filter_map(|(uri, text)| self.compile_ifml(uri, text).0)
            .collect()
    }

    /// Runs `f` with the compiled `.ifml` snapshot of the document — the
    /// resolution-populated index plus per-file URIs. Recompiles per
    /// request, consistent with [`Self::with_navigation`].
    pub(super) fn with_ifml<T>(
        &self,
        uri: &Uri,
        f: impl FnOnce(&IfmlSnapshot) -> Option<T>,
    ) -> Option<T> {
        let snapshot = {
            let documents = self.documents.lock().unwrap();
            let document = documents.get(uri)?;
            (
                document.text.clone(),
                document.surface == DocumentSurface::Known(SurfaceKind::Ifml),
            )
        };
        let (text, is_ifml) = snapshot;
        if !is_ifml {
            return None;
        }
        let (snapshot, _diagnostics) = self.compile_ifml(uri, &text);
        let snapshot = snapshot?;
        f(&snapshot)
    }

    /// Runs `f` with the navigation index of the document's current text.
    /// The index is rebuilt per request; `f` receives the position map (for
    /// offset/position conversion) and the index.
    pub(super) fn with_navigation<T>(
        &self,
        uri: &Uri,
        f: impl FnOnce(&PositionMap, &NavigationIndex) -> Option<T>,
    ) -> Option<T> {
        let documents = self.documents.lock().unwrap();
        let document = documents.get(uri)?;
        let map = PositionMap::new(&document.text);
        let index = NavigationIndex::build_or_empty(&document.text);
        f(&map, &index)
    }
}

/// Converts a driver diagnostic into its LSP shape: severity mapped, span
/// mapped through [`PositionMap`], help text appended as a hint line, and
/// the structured code attached as `code` + `data`.
pub(super) fn to_lsp_diagnostic(
    diagnostic: &phero_driver::Diagnostic,
    map: &PositionMap,
) -> Diagnostic {
    let range = diagnostic
        .span
        .map(|span| map.range_for(span))
        .unwrap_or_default();
    let mut message = diagnostic.message.clone();
    if let Some(help) = &diagnostic.help {
        message.push_str("\nhint: ");
        message.push_str(help);
    }
    let (code, data) = match &diagnostic.code {
        Some(code) => (
            Some(NumberOrString::String(code.name().to_string())),
            Some(serde_json::to_value(code).expect("diagnostic codes serialize")),
        ),
        None => (None, None),
    };
    Diagnostic {
        range,
        severity: Some(match diagnostic.severity {
            phero_driver::Severity::Error => DiagnosticSeverity::ERROR,
            phero_driver::Severity::Warning => DiagnosticSeverity::WARNING,
        }),
        code,
        code_description: None,
        source: Some(SOURCE_NAME.to_string()),
        message,
        related_information: None,
        tags: None,
        data,
    }
}

/// The decision-table contents a `.dmn` source names: every `logic table
/// "<path>"` clause of a decision, resolved relative to `dir` and read
/// from disk, keyed by the path **as written** — the CLI's `table_sources`
/// reader, ported (phero-lsp must not depend on phero-cli; the driver stays
/// filesystem-free). A table file that cannot be read is skipped: the
/// driver's unprovided-table diagnostic (E0209) names the written path.
/// Duplicated table references are gathered once.
fn dmn_table_sources(dir: &std::path::Path, source: &str) -> Vec<(String, String)> {
    let written: Vec<String> = phero_syntax::parse_dmn(source)
        .ast
        .and_then(|ast| ast.model)
        .map(|model| {
            model
                .decisions
                .iter()
                .filter_map(|decision| match &decision.logic {
                    Some(phero_syntax::DmnLogic::Table { source, .. }) => Some(source.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    let mut tables: Vec<(String, String)> = Vec::new();
    for table in written {
        if tables.iter().any(|(name, _)| *name == table) {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(dir.join(&table)) {
            tables.push((table, content));
        }
    }
    tables
}

/// Host-normalizes one path for the application relation (pherolang#249): the
/// canonical path where it exists, else the path as written — the two sides
/// (member paths and resolved import targets) go through the same function,
/// so `phero_driver::applications` sees plain string equality.
fn normalize_path(path: &std::path::Path) -> String {
    std::fs::canonicalize(path)
        .map(|canonical| canonical.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

/// The compile-input bundle for gathered domain sources: the domains
/// themselves plus their own `import schema`/`import sigil` content, which
/// resolves from disk relative to each domain file (the domains' driver
/// paths are disk paths, so no re-keying is needed).
pub(super) fn domain_inputs(domains: &[(String, String)]) -> phero_driver::CompileInputs {
    let schemas =
        phero_driver::workspace_imports::collect_schema_imports(domains).unwrap_or_default();
    let sigil = phero_driver::workspace_imports::collect_sigil_imports(domains).ok();
    phero_driver::CompileInputs::from_domains(domains).with_imports(phero_driver::DomainImports {
        schemas,
        sigil: sigil.map(|sigil| sigil.imports).unwrap_or_default(),
    })
}

/// Converts an `.ifml` diagnostic — the workspace's shared record tagged
/// with the registry's stable string code — into its LSP shape: the code
/// string as `code`, the span mapped through the position map, and the
/// severity class mapped (checker/resolver errors stay errors, the lint
/// band (`W0150`–) publishes as warnings). Import-level findings with no
/// span publish at the document start, exactly as their `0..0` used to
/// map. The record's own fields are read directly; there is no parallel
/// `.ifml` type to rebuild from (SK2 of epic pherolang#328).
pub(super) fn to_lsp_ifml_diagnostic(
    diagnostic: &phero_ifml::Diagnostic<&'static str>,
    map: &PositionMap,
) -> Diagnostic {
    Diagnostic {
        range: diagnostic
            .span
            .map(|span| map.range_for(span))
            .unwrap_or_default(),
        severity: Some(match diagnostic.severity {
            phero_ifml::Severity::Error => DiagnosticSeverity::ERROR,
            phero_ifml::Severity::Warning => DiagnosticSeverity::WARNING,
        }),
        code: diagnostic
            .code
            .map(|code| NumberOrString::String(code.to_string())),
        code_description: None,
        source: Some(SOURCE_NAME.to_string()),
        message: diagnostic.message.clone(),
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Unions every imported `.mox` domain into one model for typed binding,
/// through [`phero_driver::project::compile_domain_union`]. `None` when
/// there are no domains — or any fails to compile, in which case the LSP
/// degrades to structural checks rather than reporting binding errors
/// against a partial union.
pub(super) fn domain_union(domains: &[(String, String)]) -> Option<phero_ir::Model> {
    if domains.is_empty() {
        return None;
    }
    // Any gather/compile failure inside the driver degrades to no typed
    // checking rather than errors against a partial union.
    phero_driver::project::compile_domain_union(domains).ok()
}

impl PheroBackend {}
