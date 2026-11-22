//! The per-invocation project snapshot (Phase C of #117): every supported
//! file under the command-line directory roots gathered into one
//! `resolved key → text` map, plus the compile-once [`Artifact`] cache.
//!
//! `pherolang check <dir>` expands its argument into the supported-surface
//! listing ([`crate::inputs::expand_check_inputs`]) and hands the listing
//! to [`Project::gather`]: every file is read once into the snapshot. The
//! per-surface readers in [`crate::inputs`] then fetch imports
//! **map-first, disk-second** — the CLI's analog of the LSP's
//! open-documents-first rule. An import that resolves inside the project
//! is served from the map; one that resolves outside the root is read from
//! disk relative to its importer **and inserted into the map**, so a later
//! consumer of the same file reuses it. Read errors keep today's
//! byte-identical messages: fetches return [`std::io::Result`] and each
//! reader's side channel still renders its own
//! `cannot read imported file … (imported by …)` text.
//!
//! The snapshot also owns the import-connected application relation
//! (pherolang#249): every member's written import statements are parsed once
//! at gather time (no compile) and clustered with
//! [`phero_driver::applications::applications`], so the compile-time
//! capability/design/decision joins pool **the file's application**, never
//! its directory. A snapshot without an import edge between two files keeps
//! them apart even when they share a directory.
//!
//! The snapshot also owns the artifact cache: every CLI-level compile of a
//! project file lands in it under the file's resolved key, and consumers
//! compile sibling imports **lookup-before-compute** — an `.ifml` flow
//! whose `.actor` import the batch already checked reuses that compiled
//! [`Artifact::Actor`] instead of recompiling. The surface import graph is
//! acyclic by grammar (`.mox` is the root; `.ifml` imports only
//! `.ifml`/`.mox`/`.actor` — intra-`.ifml` cycles are the resolver's
//! E0003), so a plain map with a lookup before each compute suffices.
//! Compiles that happen inside `phero_driver`'s gatherers (a `.deploy`
//! file's imported designs and flows, a `.uc` file's four buckets) consume
//! the map's texts but cannot consult this cache — the cache boundary is
//! the CLI's own readers. Top-level per-file checks always compile and
//! render (the byte-identical `OK`/warning output is the contract); the
//! cache dedupes the import-side compiles.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use phero_driver::applications::{applications, ApplicationFile};
use phero_driver::SurfaceKind;

use crate::inputs::{read_actor_pair_in, read_ddd_design, read_dmn_pair, read_evt_pair};

/// One compiled surface artifact — the same JSON `pherolang ir` emits for
/// the surface, kept typed so consumers can pull the model they need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Artifact {
    /// A `.mox` domain model (the Core IR `Model`).
    Domain(phero_ir::Model),
    /// An `.actor` policy (the standalone `ActorModel`).
    Actor(phero_ir::ActorModel),
    /// A `.ddd` design (the standalone `DddModel`).
    Design(phero_ir::ddd::DddModel),
    /// An `.evt` contract (the standalone `EventModel`).
    Event(phero_ir::events::EventModel),
    /// A `.dmn` decision model (the standalone `DmnModel`).
    Decision(phero_ir::dmn::DmnModel),
    /// A `.uc` use case pattern (the standalone `UseCaseModel`).
    UseCase(phero_ir::usecases::UseCaseModel),
    /// A `.cmmn` case (the standalone `CmmnModel`).
    Case(phero_ir::cmmn::CmmnModel),
    /// A `.deploy` deployment (the standalone `DeployModel`).
    Deploy(phero_ir::deploy::DeployModel),
    /// An `.ifml` flow (the standalone `IfmlModel`).
    Flow(phero_ir::ifml::IfmlModel),
    /// An `.intent` intent specification (the standalone `IntentModel`,
    /// issue pherolang#196).
    Intents(phero_ir::intents::IntentModel),
}

impl Artifact {
    /// The artifact JSON exactly `pherolang ir` emits for the surface.
    pub(crate) fn to_json_pretty(&self) -> anyhow::Result<String> {
        match self {
            Artifact::Domain(model) => Ok(model.to_json_pretty()?),
            Artifact::Actor(model) => Ok(model.to_json_pretty()?),
            Artifact::Design(model) => Ok(model.to_json_pretty()?),
            Artifact::Event(model) => Ok(model.to_json_pretty()?),
            Artifact::Decision(model) => Ok(model.to_json_pretty()?),
            Artifact::UseCase(model) => Ok(model.to_json_pretty()?),
            Artifact::Case(model) => Ok(model.to_json_pretty()?),
            Artifact::Deploy(model) => Ok(model.to_json_pretty()?),
            Artifact::Flow(model) => Ok(model.to_json_pretty()?),
            Artifact::Intents(model) => Ok(model.to_json_pretty()?),
        }
    }
}

/// The in-memory snapshot of every supported file under the command-line
/// roots, plus the compile-once artifact cache.
#[derive(Debug, Default)]
pub(crate) struct Project {
    /// Resolved key → source text. Keys are the resolved disk-path strings
    /// the readers compute (`dir_of(importer).join(import)` displayed) —
    /// the same strings the driver receives as file paths.
    sources: BTreeMap<String, String>,
    /// Resolved key → compiled artifact (lookup-before-compute).
    artifacts: BTreeMap<String, Artifact>,
    /// Resolved key → the member keys of its import-connected application,
    /// itself included (pherolang#249). Built once by [`Project::gather`]; a
    /// key outside the snapshot has no entry.
    applications: BTreeMap<String, Vec<String>>,
    /// How many compiles the cache has performed — the hit/miss observer
    /// the unit tests pin.
    computes: usize,
}

impl Project {
    /// Reads every file of an `expand_check_inputs`-style listing into the
    /// snapshot. Unreadable entries are skipped: their per-file check
    /// re-reads them through today's reader path and fails there, with
    /// today's message, at today's point in the batch output.
    pub(crate) fn gather(expanded: &[PathBuf]) -> Project {
        let mut project = Project::default();
        for file in expanded {
            if let Ok(text) = std::fs::read_to_string(file) {
                project.sources.insert(file.display().to_string(), text);
            }
        }
        project.applications = application_members(&project.sources);
        project
    }

    /// The snapshot's text for a resolved key, without touching disk.
    #[cfg(test)]
    pub(crate) fn text(&self, key: &str) -> Option<&str> {
        self.sources.get(key).map(String::as_str)
    }

    /// The snapshot's text for a resolved key (the tree-level diagnostic
    /// renderer's source lookup), without touching disk.
    pub(crate) fn source_of(&self, key: &str) -> Option<&str> {
        self.sources.get(key).map(String::as_str)
    }

    /// The tree-level `.ddd` problems a directory batch must report before
    /// compiling, as **spanned diagnostics** anchored at the offending file
    /// (DD8, issue pherolang#305):
    ///
    /// - `E0390` — a fragment no root reaches;
    /// - `E0391` — a fragment reached by more than one root;
    /// - `E0392` — two roots with one authored application name;
    /// - `E0393` — a `.ddd` import cycle;
    /// - `E0394` — a root imported as a fragment;
    /// - `E0395` — a declared fragment the snapshot does not contain.
    ///
    /// Each diagnostic is tagged with the file whose source anchors it, so
    /// `check`/`build` render it through the ordinary ariadne reporter.
    pub(crate) fn ddd_tree_errors(&mut self) -> Vec<(String, phero_driver::Diagnostic)> {
        use phero_diagnostics::ddd::{
            E_DESIGN_APPLICATION_NAME_DUPLICATE, E_DESIGN_FRAGMENT_MISSING,
            E_DESIGN_FRAGMENT_MULTI_ROOT, E_DESIGN_FRAGMENT_ORPHAN, E_DESIGN_IMPORT_CYCLE,
            E_DESIGN_ROOT_AS_FRAGMENT,
        };

        let ddd_keys: Vec<String> = self
            .sources
            .keys()
            .filter(|key| SurfaceKind::of_path(Path::new(key)) == Some(SurfaceKind::Ddd))
            .cloned()
            .collect();
        let mut roots: Vec<(String, String, phero_syntax::Span)> = Vec::new(); // path, app name, name span
        for key in &ddd_keys {
            let Some(source) = self.sources.get(key) else {
                continue;
            };
            if let Some(application) = phero_syntax::parse_ddd(source)
                .ast
                .and_then(|ast| ast.application)
            {
                roots.push((key.clone(), application.name.text, application.name.span));
            }
        }
        let mut errors: Vec<(String, phero_driver::Diagnostic)> = Vec::new();
        let mut owner: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut sites: BTreeMap<String, Vec<phero_driver::project::DddFragmentSite>> =
            BTreeMap::new();
        for (root, _, _) in &roots {
            let source = self.sources.get(root).cloned().unwrap_or_default();
            let mut fetch = |resolved: &str| self.fetch(resolved).ok();
            let gathered = phero_driver::project::gather_ddd_app_imports(root, &source, &mut fetch);
            for (fragment, fragment_source) in &gathered.fragments {
                owner
                    .entry(fragment.clone())
                    .or_default()
                    .push(root.clone());
                // A root imported as a fragment: the import site is the
                // anchor, and the fragment's own `application` is the fact.
                if phero_syntax::parse_ddd(fragment_source)
                    .ast
                    .is_some_and(|ast| ast.application.is_some())
                {
                    if let Some(site) = gathered
                        .sites
                        .iter()
                        .find(|site| &site.resolved == fragment)
                    {
                        errors.push((
                            site.importer.clone(),
                            phero_driver::Diagnostic::error(
                                format!(
                                    "{E_DESIGN_ROOT_AS_FRAGMENT} a design application root \
                                     cannot be imported as a fragment; remove its `application` \
                                     declaration or import its root instead"
                                ),
                                Some(site.span),
                            ),
                        ));
                    }
                }
            }
            for missing in &gathered.missing {
                if let Some(site) = gathered.sites.iter().find(|site| &site.import == missing) {
                    errors.push((
                        site.importer.clone(),
                        phero_driver::Diagnostic::error(
                            format!(
                                "{E_DESIGN_FRAGMENT_MISSING} imported design fragment \
                                 \"{missing}\" was not provided"
                            ),
                            Some(site.span),
                        ),
                    ));
                }
            }
            for cycle in &gathered.cycles {
                // The cycle is closed by the last import that names the
                // revisited fragment — that is the site to point at.
                if let Some(site) = gathered
                    .sites
                    .iter()
                    .rev()
                    .find(|site| &site.resolved == cycle)
                {
                    errors.push((
                        site.importer.clone(),
                        phero_driver::Diagnostic::error(
                            format!("{E_DESIGN_IMPORT_CYCLE} design import cycle through {cycle}"),
                            Some(site.span),
                        ),
                    ));
                }
            }
            sites.insert(root.clone(), gathered.sites);
        }
        for key in &ddd_keys {
            if roots.iter().any(|(root, _, _)| root == key) {
                continue;
            }
            let source = self.sources.get(key).cloned().unwrap_or_default();
            // A fragment's own first declaration is the most useful anchor;
            // a file with nothing parseable anchors at its start.
            let fallback = phero_syntax::parse_ddd(&source)
                .ast
                .and_then(|ast| ast.fragments.first().map(|module| module.span))
                .unwrap_or_else(|| (0..0).into());
            match owner.get(key) {
                None => errors.push((
                    key.clone(),
                    phero_driver::Diagnostic::error(
                        format!(
                            "{E_DESIGN_FRAGMENT_ORPHAN} design fragment {key} has no root: \
                             no `.ddd` application imports it"
                        ),
                        Some(fallback),
                    ),
                )),
                Some(owners) if owners.len() > 1 => {
                    // Anchor at the import that makes the fragment shared —
                    // the second (in path order) root's declaration.
                    let mut sorted = owners.clone();
                    sorted.sort();
                    let second = &sorted[1];
                    let span = sites
                        .get(second)
                        .and_then(|sites| {
                            sites
                                .iter()
                                .find(|site| &site.resolved == key)
                                .map(|site| site.span)
                        })
                        .unwrap_or(fallback);
                    errors.push((
                        second.clone(),
                        phero_driver::Diagnostic::error(
                            format!(
                                "{E_DESIGN_FRAGMENT_MULTI_ROOT} design fragment {key} is \
                                 imported by more than one root: {}",
                                owners.join(", ")
                            ),
                            Some(span),
                        ),
                    ));
                }
                Some(_) => {}
            }
        }
        let mut names: BTreeMap<&str, Vec<(&str, phero_syntax::Span)>> = BTreeMap::new();
        for (root, name, span) in &roots {
            names.entry(name).or_default().push((root.as_str(), *span));
        }
        for (name, named_roots) in names {
            if named_roots.len() > 1 {
                let mut sorted = named_roots.clone();
                sorted.sort_by_key(|(path, _)| *path);
                let (anchor, span) = sorted[1];
                errors.push((
                    anchor.to_string(),
                    phero_driver::Diagnostic::error(
                        format!(
                            "{E_DESIGN_APPLICATION_NAME_DUPLICATE} duplicate design application \
                             name '{name}' in {}",
                            named_roots
                                .iter()
                                .map(|(path, _)| *path)
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        Some(span),
                    ),
                ));
            }
        }
        errors
    }

    /// Whether `path` is a `.ddd` fragment (top-level modules, no
    /// `application`) — the files a directory batch compiles through their
    /// root and never checks standalone (DD8).
    pub(crate) fn is_ddd_fragment(&self, path: &Path) -> bool {
        if SurfaceKind::of_path(path) != Some(SurfaceKind::Ddd) {
            return false;
        }
        let key = path.display().to_string();
        self.sources.get(&key).is_some_and(|source| {
            phero_syntax::parse_ddd(source)
                .ast
                .is_some_and(|ast| ast.application.is_none() && !ast.fragments.is_empty())
        })
    }

    /// Map-first, disk-second fetch: the snapshot's text for `key`, or a
    /// disk read of `path` — filled into the map on success, so a later
    /// consumer of the same file never re-reads. `Err` carries the io
    /// error so callers render today's message byte-for-byte.
    pub(crate) fn fetch_path(&mut self, key: &str, path: &Path) -> std::io::Result<String> {
        if let Some(text) = self.sources.get(key) {
            return Ok(text.clone());
        }
        let text = std::fs::read_to_string(path)?;
        self.sources.insert(key.to_string(), text.clone());
        Ok(text)
    }

    /// The path-keyed fetch shape the driver gatherers take
    /// (`FnMut(&str) -> Option<String>`): the resolved key **is** the disk
    /// path, so the key locates both the map slot and the fallback read.
    pub(crate) fn fetch(&mut self, key: &str) -> std::io::Result<String> {
        self.fetch_path(key, Path::new(key))
    }

    /// The cached artifact for `key`, if any — lookup without compute (the
    /// sibling gathers check the cache before re-reading and compiling).
    pub(crate) fn peek(&self, key: &str) -> Option<Artifact> {
        self.artifacts.get(key).cloned()
    }

    /// Records a freshly compiled artifact under `key`.
    pub(crate) fn store(&mut self, key: &str, artifact: &Artifact) {
        self.computes += 1;
        self.artifacts.insert(key.to_string(), artifact.clone());
    }

    /// The snapshot's keys ending in `extension` (with the leading dot),
    /// sorted — the receipt generator's whole-snapshot enumeration
    /// (`pherolang verify` passes `""`). The compile-time cross-surface
    /// gathers no longer use it: they pool the file's **application**
    /// ([`Project::application_of`], pherolang#249).
    pub(crate) fn keys_with_extension(&self, extension: &str) -> Vec<String> {
        self.sources
            .keys()
            .filter(|key| key.ends_with(extension))
            .cloned()
            .collect()
    }

    /// The member keys of `key`'s import-connected application, `key`
    /// itself included (pherolang#249) — the pool the compile-time
    /// capability/design/decision joins resolve against instead of the
    /// directory. Empty when `key` is outside the gathered snapshot (or no
    /// other file shares its component): the joins then receive an empty
    /// sibling slice, which is exactly the plain `compile_*_str` behavior.
    pub(crate) fn application_of(&self, key: &str) -> Vec<String> {
        self.applications.get(key).cloned().unwrap_or_default()
    }

    /// The compiled `.actor` artifacts of `file`'s import-connected
    /// application, the file itself excluded — the capability pool a
    /// design's rule-10 check resolves against at compile time
    /// (pherolang#128's J3). One gather per artifact kind; a sibling that
    /// fails to compile contributes nothing (parity with the union pass,
    /// which keeps only the successful artifacts).
    pub(crate) fn sibling_actors(&mut self, file: &Path) -> Vec<(String, phero_ir::ActorModel)> {
        let key = file.display().to_string();
        let members = self.application_of(&key);
        let mut models = Vec::new();
        for member in members {
            if member == key || SurfaceKind::of_path(Path::new(&member)) != Some(SurfaceKind::Actor)
            {
                continue;
            }
            match self.peek(&member) {
                Some(Artifact::Actor(model)) => models.push((member.clone(), model)),
                Some(_) => {}
                None => {
                    let Ok(pair) = read_actor_pair_in(Path::new(&member), Some(self)) else {
                        continue;
                    };
                    let compilation =
                        phero_driver::compile_actors_str(&pair.path, &pair.source, pair.inputs());
                    if let Some(model) = compilation.model {
                        self.store(&member, &Artifact::Actor(model.clone()));
                        models.push((member, model));
                    }
                }
            }
        }
        models
    }

    /// The compiled `.dmn` artifacts of `file`'s application
    /// (pherolang#78) — the decision services a design's `invokes`/`inject
    /// decision` links resolve against (rule 17). Gathered exactly like
    /// [`Project::sibling_actors`].
    pub(crate) fn sibling_decisions(
        &mut self,
        file: &Path,
    ) -> Vec<(String, phero_ir::dmn::DmnModel)> {
        let key = file.display().to_string();
        let members = self.application_of(&key);
        let mut models = Vec::new();
        for member in members {
            if member == key || SurfaceKind::of_path(Path::new(&member)) != Some(SurfaceKind::Dmn) {
                continue;
            }
            match self.peek(&member) {
                Some(Artifact::Decision(model)) => models.push((member.clone(), model)),
                Some(_) => {}
                None => {
                    let Ok(pair) = read_dmn_pair(Path::new(&member), Some(self)) else {
                        continue;
                    };
                    let compilation =
                        phero_driver::compile_dmn_str(&pair.path, &pair.source, pair.inputs());
                    if let Some(model) = compilation.model {
                        self.store(&member, &Artifact::Decision(model.clone()));
                        models.push((member, model));
                    }
                }
            }
        }
        models
    }

    /// The compiled `.evt` artifacts of `file`'s application (pherolang#382)
    /// — the event contracts a design's `emits` links resolve against
    /// (rule 23). Gathered exactly like [`Project::sibling_actors`].
    pub(crate) fn sibling_events(
        &mut self,
        file: &Path,
    ) -> Vec<(String, phero_ir::events::EventModel)> {
        let key = file.display().to_string();
        let members = self.application_of(&key);
        let mut models = Vec::new();
        for member in members {
            if member == key || SurfaceKind::of_path(Path::new(&member)) != Some(SurfaceKind::Evt) {
                continue;
            }
            match self.peek(&member) {
                Some(Artifact::Event(model)) => models.push((member.clone(), model)),
                Some(_) => {}
                None => {
                    let Ok(pair) = read_evt_pair(Path::new(&member), Some(self)) else {
                        continue;
                    };
                    let compilation =
                        phero_driver::compile_evt_str(&pair.path, &pair.source, pair.inputs());
                    if let Some(model) = compilation.model {
                        self.store(&member, &Artifact::Event(model.clone()));
                        models.push((member, model));
                    }
                }
            }
        }
        models
    }

    /// The compiled `.ddd` artifacts of `file`'s application (pherolang#249,
    /// the file itself excluded) — the carrier set an actor policy's grant
    /// coverage resolves against (pherolang#128's J4). Gathered exactly like
    /// [`Project::sibling_actors`].
    pub(crate) fn sibling_designs(
        &mut self,
        file: &Path,
    ) -> Vec<(String, phero_ir::ddd::DddModel)> {
        let key = file.display().to_string();
        let members = self.application_of(&key);
        let mut models = Vec::new();
        for member in members {
            if member == key || SurfaceKind::of_path(Path::new(&member)) != Some(SurfaceKind::Ddd) {
                continue;
            }
            match self.peek(&member) {
                Some(Artifact::Design(model)) => models.push((member.clone(), model)),
                Some(_) => {}
                None => {
                    let Ok(design) = read_ddd_design(Path::new(&member), Some(self)) else {
                        continue;
                    };
                    if phero_syntax::parse_ddd(&design.pair.source)
                        .ast
                        .is_none_or(|ast| ast.application.is_none())
                    {
                        // A fragment is compiled through its root; it never
                        // owns an artifact (DD8).
                        continue;
                    }
                    let fragments: Vec<phero_driver::DddFragment> = design
                        .fragments
                        .iter()
                        .map(|(path, source)| phero_driver::DddFragment {
                            path: path.clone(),
                            source: source.clone(),
                        })
                        .collect();
                    let compilation = phero_driver::compile_ddd_app_str(
                        &design.pair.path,
                        &design.pair.source,
                        &fragments,
                        design.pair.inputs(),
                    );
                    if let Some(model) = compilation.model {
                        self.store(&member, &Artifact::Design(model.clone()));
                        models.push((member, model));
                    }
                }
            }
        }
        models
    }

    /// Lookup-before-compute: the cached artifact for `key`, or
    /// `compute()` run exactly once and stored. The import graph is
    /// acyclic by grammar, so a plain map suffices — no cycle machinery.
    pub(crate) fn artifact(
        &mut self,
        key: &str,
        compute: impl FnOnce() -> anyhow::Result<Artifact>,
    ) -> anyhow::Result<Artifact> {
        if let Some(hit) = self.artifacts.get(key) {
            return Ok(hit.clone());
        }
        let artifact = compute()?;
        self.computes += 1;
        self.artifacts.insert(key.to_string(), artifact.clone());
        Ok(artifact)
    }

    /// How many compiles the cache has performed.
    #[cfg(test)]
    pub(crate) fn computes(&self) -> usize {
        self.computes
    }
}

/// The import-connected application of every snapshot member (pherolang#249):
/// each member's written imports are parsed (no compile), resolved the way
/// the readers resolve them — relative to the importing file's directory,
/// canonicalized where the path exists — and clustered with
/// [`phero_driver::applications::applications`].
///
/// Both sides of the relation are host-normalized identically (a member
/// path and a resolved import target each go through [`normalize`]), so a
/// member and the import naming it compare equal exactly when the
/// filesystem agrees — the driver's precondition. An import that names no
/// member contributes no edge (out-of-snapshot imports included, the CLI
/// snapshot's "the set is the directory listing" rule), and a `.mox`
/// member declares no surface imports: it is a node without out-edges, the
/// bridge the other surfaces join through.
fn application_members(sources: &BTreeMap<String, String>) -> BTreeMap<String, Vec<String>> {
    let paths: Vec<String> = sources.keys().cloned().collect();
    let files: Vec<ApplicationFile> = paths
        .iter()
        .map(|path| ApplicationFile {
            path: normalize(Path::new(path)),
            imports: written_imports(Path::new(path), &sources[path])
                .into_iter()
                .map(|import| resolve_import(Path::new(path), &import))
                .collect(),
        })
        .collect();
    let mut members = BTreeMap::new();
    for component in applications(&files) {
        let component: Vec<String> = component
            .into_iter()
            .map(|member| paths[member].clone())
            .collect();
        for key in &component {
            members.insert(key.clone(), component.clone());
        }
    }
    members
}

/// The `import "…"` strings one snapshot member writes, dispatched on the
/// one surface table. `.mox` writes none (its `import schema`/`import
/// sigil` targets are vocabulary content, not application files), and every
/// other surface's written imports are its parse's `imports` list — the same
/// strings the reader later resolves, so the clustering never invents an
/// edge the compile would not follow.
fn written_imports(path: &Path, text: &str) -> Vec<String> {
    match SurfaceKind::of_path(path) {
        Some(SurfaceKind::Actor) => phero_syntax::parse_actors(text)
            .ast
            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
            .unwrap_or_default(),
        Some(SurfaceKind::Ddd) => phero_syntax::parse_ddd(text)
            .ast
            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
            .unwrap_or_default(),
        Some(SurfaceKind::Evt) => phero_syntax::parse_evt(text)
            .ast
            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
            .unwrap_or_default(),
        Some(SurfaceKind::Deploy) => phero_syntax::parse_deploy(text)
            .ast
            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
            .unwrap_or_default(),
        Some(SurfaceKind::Uc) => phero_syntax::parse_usecases(text)
            .ast
            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
            .unwrap_or_default(),
        Some(SurfaceKind::Cmmn) => phero_syntax::parse_cmmn(text)
            .ast
            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
            .unwrap_or_default(),
        // `.dmn` imports are domains plus `.evt` contracts; both are
        // surface files the snapshot may hold, so both are edges.
        Some(SurfaceKind::Dmn) => phero_driver::project::dmn_import_paths(text),
        Some(SurfaceKind::Ifml) => phero_ifml::parse_ifml(text)
            .map(|model| model.imports)
            .unwrap_or_default(),
        Some(SurfaceKind::Intent) => phero_syntax::parse_intents(text)
            .ast
            .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
            .unwrap_or_default(),
        // A `.mox` member declares no surface imports; an unknown extension
        // is no application file at all.
        Some(SurfaceKind::Mox) | None => Vec::new(),
    }
}

/// One written import resolved the way the readers resolve it: relative to
/// the importing file's own directory, then host-normalized.
fn resolve_import(importer: &Path, import: &str) -> String {
    let directory = importer
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    normalize(&directory.join(import))
}

/// Host-normalizes one path: the canonical path where it exists, else the
/// path as written. Both the members and the resolved import targets go
/// through this, so the relation is plain string equality afterwards.
fn normalize(path: &Path) -> String {
    std::fs::canonicalize(path)
        .map(|canonical| canonical.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh scratch directory per test, so concurrently running tests
    /// never share files (the integration-suite rule, in-process).
    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "phero-cli-project-tests-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    const MOX: &str = "package demo\n\nclass Book { String title }\n";

    fn domain_artifact() -> Artifact {
        let compilation =
            phero_driver::compile_str("t.mox", MOX, &phero_driver::DomainImports::default());
        Artifact::Domain(compilation.model.expect("compiles"))
    }

    #[test]
    fn gather_builds_a_map_from_a_listing() {
        let dir = scratch("gather");
        let a = dir.join("a.mox");
        std::fs::write(&a, MOX).expect("write a");
        let b = dir.join("nested/b.mox");
        std::fs::create_dir_all(b.parent().unwrap()).expect("mkdir nested");
        std::fs::write(&b, MOX).expect("write b");
        let missing = dir.join("missing.mox");

        let project = Project::gather(&[a.clone(), b.clone(), missing.clone()]);

        assert_eq!(project.text(&a.display().to_string()), Some(MOX));
        assert_eq!(project.text(&b.display().to_string()), Some(MOX));
        assert_eq!(
            project.text(&missing.display().to_string()),
            None,
            "unreadable entries are skipped; their per-file check fails with today's message"
        );
    }

    #[test]
    fn fetch_serves_the_map_before_disk() {
        let dir = scratch("map-first");
        let path = dir.join("a.mox");
        std::fs::write(&path, MOX).expect("write a");
        let mut project = Project::gather(std::slice::from_ref(&path));
        std::fs::remove_file(&path).expect("remove from disk");

        let key = path.display().to_string();
        assert_eq!(
            project.fetch(&key).expect("map hit"),
            MOX,
            "a project file is served from the snapshot even after it leaves disk"
        );
    }

    #[test]
    fn fetch_falls_back_to_disk_and_fills_the_map() {
        let dir = scratch("disk-fallback");
        let listed = dir.join("a.mox");
        std::fs::write(&listed, MOX).expect("write listed");
        // An import resolving outside the gathered root.
        let outside = dir.join("outside.mox");
        std::fs::write(&outside, MOX).expect("write outside");
        let mut project = Project::gather(&[listed]);

        let key = outside.display().to_string();
        assert_eq!(
            project.fetch(&key).expect("disk fallback"),
            MOX,
            "an out-of-root import is read from disk"
        );
        std::fs::remove_file(&outside).expect("remove from disk");
        assert_eq!(
            project.fetch(&key).expect("map hit after fill"),
            MOX,
            "the disk fallback filled the map"
        );
        assert_eq!(project.text(&key), Some(MOX));
    }

    #[test]
    fn cache_compiles_once_per_key() {
        let mut project = Project::default();
        let recomputed = std::cell::Cell::new(false);

        let first = project
            .artifact("a.mox", || Ok(domain_artifact()))
            .expect("first compute");
        let second = project
            .artifact("a.mox", || {
                recomputed.set(true);
                Ok(domain_artifact())
            })
            .expect("second lookup");

        assert_eq!(first, second);
        assert_eq!(project.computes(), 1, "hit: no recompute");
        assert!(!recomputed.get(), "the second compute closure never ran");
    }

    #[test]
    fn cache_misses_compute_and_insert_per_key() {
        let mut project = Project::default();

        let _ = project
            .artifact("a.mox", || Ok(domain_artifact()))
            .expect("a");
        let _ = project
            .artifact("b.mox", || Ok(domain_artifact()))
            .expect("b");
        assert_eq!(project.computes(), 2, "distinct keys compute once each");

        let _ = project
            .artifact("a.mox", || Ok(domain_artifact()))
            .expect("a again");
        assert_eq!(project.computes(), 2, "a.mox stays cached");
    }

    #[test]
    fn applications_cluster_by_resolved_imports_not_by_directory() {
        let dir = scratch("applications");
        // Two applications in one directory: desk's design and policy both
        // import desk.mox (the bridge), shop's design imports shop.mox.
        std::fs::write(dir.join("desk.mox"), MOX).expect("write desk mox");
        std::fs::write(dir.join("shop.mox"), MOX).expect("write shop mox");
        let design = dir.join("desk.ddd");
        std::fs::write(
            &design,
            "import \"desk.mox\"\n\napplication Desk {\n    module desk {\n        value Book\n    }\n}\n",
        )
        .expect("write design");
        let actor = dir.join("desk.actor");
        std::fs::write(
            &actor,
            "import \"desk.mox\"\n\nactors DeskPolicy {\n    actor Agent\n}\n",
        )
        .expect("write policy");
        let other = dir.join("shop.ddd");
        std::fs::write(
            &other,
            "import \"shop.mox\"\n\napplication Shop {\n    module shop {\n        value Book\n    }\n}\n",
        )
        .expect("write other design");

        let desk_mox = dir.join("desk.mox").display().to_string();
        let shop_mox = dir.join("shop.mox").display().to_string();
        let project = Project::gather(&[
            dir.join("desk.mox"),
            dir.join("shop.mox"),
            design.clone(),
            actor.clone(),
            other.clone(),
        ]);

        let mut desk_app = vec![actor.display().to_string(), design.display().to_string()];
        desk_app.insert(0, desk_mox.clone());
        desk_app.sort();
        assert_eq!(
            project.application_of(&design.display().to_string()),
            desk_app,
            "the design's application is the imported domain plus its importers"
        );
        assert_eq!(
            project.application_of(&actor.display().to_string()),
            desk_app,
            "the policy shares the design's application through the domain bridge"
        );
        assert_eq!(
            project.application_of(&other.display().to_string()),
            vec![other.display().to_string(), shop_mox],
            "the unrelated design's application holds no desk member"
        );
        assert!(
            project.application_of("not-in-the-snapshot.ddd").is_empty(),
            "outside the snapshot there is no application, so the joins stay silent"
        );
    }
}
