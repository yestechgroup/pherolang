//! Turning command-line inputs into in-memory sources and driver compilations:
//! input expansion, source reading, and the thin readers for every artifact
//! surface. Import gathering itself lives in [`phero_driver::project`], so each
//! reader here only builds the disk fetch closure (the CLI's read-error policy
//! is a closure side channel) and assembles the pair the surface's compile
//! entry point takes. In project mode ([`crate::project::Project`]) the fetches
//! are map-first, disk-second. Plus the shared compile step over collected
//! sources.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use phero_driver::{
    compile_actor_imports, compile_decision_imports, compile_domain_union, compile_files,
    compile_str, gather_case_imports, gather_deploy_imports, gather_domain_sources,
    gather_ifml_project, gather_usecase_imports, ActorImport, DecisionImport, DomainImports,
    MultiCompilation, SchemaImports, SurfaceKind,
};

use crate::imports::{collect_schema_imports, collect_sigil_imports, SigilSources};
use crate::project::{Artifact, Project};

fn read_source(file: &Path) -> anyhow::Result<(String, String)> {
    let source = std::fs::read_to_string(file)
        .map_err(|error| anyhow::anyhow!("cannot read {}: {error}", file.display()))?;
    Ok((file.display().to_string(), source))
}

/// Reads every input as `(driver path, source)` pairs.
pub(crate) fn read_sources(files: &[PathBuf]) -> anyhow::Result<Vec<(String, String)>> {
    files.iter().map(|file| read_source(file)).collect()
}

/// Expands command-line inputs: each entry may be a file or a directory;
/// directories are scanned recursively for `*.mox` files (`.actor` files are
/// ignored). Results are sorted lexicographically by path — this ordering
/// determines the IR package order — and must be non-empty.
pub(crate) fn expand_inputs(files: &[PathBuf]) -> anyhow::Result<Vec<PathBuf>> {
    let mut expanded = Vec::new();
    for entry in files {
        if entry.is_dir() {
            collect_mox_files(entry, &mut expanded)?;
        } else {
            expanded.push(entry.clone());
        }
    }
    if expanded.is_empty() {
        if files.is_empty() {
            anyhow::bail!("no input files");
        }
        let named = files
            .iter()
            .map(|file| file.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::bail!("no .mox files found in {named}");
    }
    expanded.sort();
    Ok(expanded)
}

/// Expands `pherolang check` inputs: directories scan recursively for **every
/// supported surface** ([`SurfaceKind::ALL`]), not just `.mox` —
/// formatter-golden `X.fmt.<ext>` artifacts excepted (see
/// [`is_formatter_golden`]). Explicit file arguments pass through as-is.
pub(crate) fn expand_check_inputs(files: &[PathBuf]) -> anyhow::Result<Vec<PathBuf>> {
    let mut expanded = Vec::new();
    for entry in files {
        if entry.is_dir() {
            collect_supported_files(entry, &mut expanded)?;
        } else {
            expanded.push(entry.clone());
        }
    }
    if expanded.is_empty() {
        let named = files
            .iter()
            .map(|file| file.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::bail!("no model files found in {named}");
    }
    expanded.sort();
    Ok(expanded)
}

/// Recursively collects `*.mox` files under `dir`.
fn collect_mox_files(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    collect_with_extensions(dir, &["mox"], out)
}

/// Recursively collects files under `dir` whose extension is one of
/// `extensions`. Symlinks are never followed ([`WalkDir::follow_links`] is
/// `false` — a link loop cannot hang the scan), and the callers sort the final
/// list, so the output is deterministic regardless of readdir order.
/// Formatter-golden artifacts (see [`is_formatter_golden`]) are never
/// collected.
fn collect_with_extensions(
    dir: &Path,
    extensions: &[&str],
    out: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    for entry in WalkDir::new(dir).follow_links(false) {
        let entry =
            entry.map_err(|error| anyhow::anyhow!("cannot read {}: {error}", dir.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extensions.contains(&extension))
            && !is_formatter_golden(path)
        {
            out.push(path.to_path_buf());
        }
    }
    Ok(())
}

/// `true` for formatter-golden artifacts: the `X.fmt.<ext>` files the
/// formatter's golden tests pin byte-for-byte against a canonical source that
/// lives elsewhere (the GOLDENS table in phero-syntax's fmt tests). They are
/// outputs, not standalone models — their imports and vocabulary snapshots
/// only resolve next to their *sources* — so directory scans skip them.
/// Explicit file arguments pass through untouched.
fn is_formatter_golden(path: &Path) -> bool {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.ends_with(".fmt"))
}

/// Recursively collects files of every supported surface under `dir` — the
/// extension list is [`SurfaceKind::ALL`], the one surface table.
fn collect_supported_files(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    let extensions: Vec<&str> = SurfaceKind::ALL
        .iter()
        .map(|kind| kind.extension())
        .collect();
    collect_with_extensions(dir, &extensions, out)
}

/// Compiles in-memory sources into one model: a single source keeps the
/// single-file pipeline (identical diagnostics and IR); several sources
/// compile as one multi-package model. Schema imports declared by the sources
/// are read from disk first (see [`collect_schema_imports`]), then sigil
/// imports (see [`collect_sigil_imports`]).
pub(crate) fn compile_sources(
    sources: &[(String, String)],
) -> anyhow::Result<(MultiCompilation, SigilSources)> {
    let schema_imports = collect_schema_imports(sources)?;
    let sigil_sources = collect_sigil_imports(sources)?;
    let imports = DomainImports {
        schemas: schema_imports,
        sigil: sigil_sources.imports.clone(),
    };
    if let [(path, source)] = sources {
        let compilation = compile_str(path, source, &imports);
        Ok((
            MultiCompilation {
                model: compilation.model,
                diagnostics: compilation
                    .diagnostics
                    .into_iter()
                    .map(|diagnostic| (path.clone(), diagnostic))
                    .collect(),
                sigil_diagnostics: compilation.sigil_diagnostics,
            },
            sigil_sources,
        ))
    } else {
        Ok((compile_files(sources, &imports), sigil_sources))
    }
}

/// `true` for the given surface's paths — the one surface table decides.
macro_rules! surface_predicate {
    ($($name:ident => $kind:ident),* $(,)?) => {
        $(
            /// `true` when the path belongs to this surface.
            pub(crate) fn $name(file: &Path) -> bool {
                SurfaceKind::of_path(file) == Some(SurfaceKind::$kind)
            }
        )*
    };
}

surface_predicate! {
    is_ddd_file => Ddd,
    is_actor_file => Actor,
    is_evt_file => Evt,
    is_dmn_file => Dmn,
    is_uc_file => Uc,
    is_cmmn_file => Cmmn,
    is_deploy_file => Deploy,
    is_ifml_file => Ifml,
}

/// An `.ifml` file plus everything it imports, ready for
/// [`phero_ifml::compile_ifml_str`] and
/// [`phero_ifml::check_ifml_with_actors`].
pub(crate) struct IfmlInputs {
    /// The interaction file's path as given on the command line.
    pub(crate) path: String,
    /// The interaction file's source text.
    pub(crate) source: String,
    /// Every transitively imported `.ifml` source, keyed
    /// `(importing-file path, import path)`.
    pub(crate) ifml_imports: phero_ifml::IfmlImports,
    /// Every `.ifml` file in the compilation as `(resolver identity, source)`.
    pub(crate) sources: Vec<(String, String)>,
    /// The union of all imported `.mox` domain models, for typed binding;
    /// `None` when the file imports no domains (binding checks skip).
    pub(crate) domains: Option<phero_ir::Model>,
    /// Every imported `.actor` policy, compiled — the authorization
    /// dimension. Empty when the file imports no actor models.
    pub(crate) actors: Vec<phero_ir::ActorModel>,
    /// Every imported `.dmn` decision model, compiled — the decision
    /// dimension. Empty when the file imports no decision models.
    pub(crate) decisions: Vec<phero_ir::dmn::DmnModel>,
    /// The resolved disk paths of every surface import the walk fetched
    /// (`.ifml`, `.mox`, `.actor`, `.dmn`), deduplicated and sorted — the
    /// host-normalized keys the semantic graph's `imports` edges use.
    /// Decision-table references are not surface imports and stay out.
    pub(crate) imports: Vec<String>,
}

/// Reads an `.ifml` file plus every `.ifml`, `.mox`, `.actor`, and `.dmn` file
/// it imports, transitively. Import paths resolve relative to the importing
/// file's own directory on disk while the resolver keys each import by the
/// importing file's identity and the import string **as written** (the
/// resolver is filesystem-free). The CLI's read-error policy is the closure's
/// side channel: the first unreadable surface import errors after the walk,
/// naming the resolved path, while a gathered `.dmn` model's decision-table
/// references degrade to the driver's unprovided-table diagnostic. Imported
/// `.mox`/`.actor`/`.dmn` files are compiled for typed binding, authorization
/// and decision checking — lookup-before-compute within `project`, so a file
/// the batch already checked is reused.
pub(crate) fn read_ifml_inputs(
    file: &Path,
    mut project: Option<&mut Project>,
) -> anyhow::Result<IfmlInputs> {
    let source = std::fs::read_to_string(file)
        .map_err(|error| anyhow::anyhow!("cannot read {}: {error}", file.display()))?;
    let path = file.display().to_string();
    // Disk paths per identity: the walker only ever sees identities — the
    // as-written import strings `.ifml` recursion recurses under, and the
    // resolved keys this closure hands back (an imported `.actor` policy's own
    // `.mox` imports are fetched under the policy's resolved key) — so every
    // key handed out is mapped the same way the entry identity is. Other
    // extensions pass through unread unless the importer is a gathered `.dmn`
    // model, whose decision-table references they are.
    let mut disk_paths: BTreeMap<String, PathBuf> = BTreeMap::new();
    disk_paths.insert(path.clone(), file.to_path_buf());
    // The resolved surface imports the graph builder draws `imports` edges
    // from; a set because the walk can reach one file through several paths.
    let mut imports: BTreeSet<String> = BTreeSet::new();
    let mut read_error: Option<anyhow::Error> = None;
    let mut fetch = |importer: &str, import: &str| -> Option<(String, String)> {
        let known_surface = import.ends_with(".ifml")
            || import.ends_with(".mox")
            || import.ends_with(".actor")
            || import.ends_with(".dmn");
        if !known_surface {
            let importer_is_dmn = disk_paths.get(importer).is_some_and(|disk| {
                disk.extension().and_then(|extension| extension.to_str()) == Some("dmn")
            });
            if !importer_is_dmn {
                return None;
            }
        }
        let resolved = {
            let importer_disk = disk_paths.get(importer)?;
            dir_of(importer_disk).join(import)
        };
        let resolved_key = resolved.display().to_string();
        let fetched = match project.as_deref_mut() {
            Some(project) => project.fetch_path(&resolved_key, &resolved),
            None => std::fs::read_to_string(&resolved),
        };
        let text = match fetched {
            Ok(text) => text,
            Err(error) => {
                // Decision-table contents degrade to the driver's
                // unprovided-table diagnostic (E0209 names the written path);
                // a surface import that cannot be read is the CLI's hard
                // error, raised after the walk.
                if read_error.is_none() && known_surface {
                    read_error = Some(anyhow::anyhow!(
                        "cannot read imported file {} (imported by {}): {error}",
                        resolved.display(),
                        importer
                    ));
                }
                return None;
            }
        };
        disk_paths.insert(import.to_string(), resolved.clone());
        disk_paths.insert(resolved_key.clone(), resolved);
        if known_surface {
            imports.insert(resolved_key.clone());
        }
        Some((resolved_key, text))
    };
    let gathered = gather_ifml_project(&path, &source, &mut fetch);
    if let Some(error) = read_error {
        return Err(error);
    }
    let actors = compile_ifml_actors(&gathered.actors, project.as_deref_mut())?;
    let decisions = compile_ifml_decisions(&gathered.decisions, project)?;
    let domains = if gathered.domains.is_empty() {
        None
    } else {
        Some(compile_domain_union(&gathered.domains).map_err(|message| anyhow::anyhow!(message))?)
    };
    Ok(IfmlInputs {
        path,
        source,
        ifml_imports: gathered.imports,
        sources: gathered.sources,
        domains,
        actors,
        decisions,
        imports: imports.into_iter().collect(),
    })
}

/// The compiled `.actor` models behind a gathered `.ifml` project.
fn compile_ifml_actors(
    gathered: &[ActorImport],
    mut project: Option<&mut Project>,
) -> anyhow::Result<Vec<phero_ir::ActorModel>> {
    let mut actors = Vec::new();
    for actor in gathered {
        let compute = || {
            compile_actor_imports(std::slice::from_ref(actor))
                .map(|mut models| models.pop().expect("one policy, one model"))
                .map_err(|message| anyhow::anyhow!(message))
                .map(Artifact::Actor)
        };
        match cached_artifact(project.as_deref_mut(), &actor.key, compute)? {
            Artifact::Actor(model) => actors.push(model),
            _ => unreachable!("actor compiles produce actor artifacts"),
        }
    }
    Ok(actors)
}

/// The compiled `.dmn` decision models behind a gathered `.ifml` project.
fn compile_ifml_decisions(
    gathered: &[DecisionImport],
    mut project: Option<&mut Project>,
) -> anyhow::Result<Vec<phero_ir::dmn::DmnModel>> {
    let mut decisions = Vec::new();
    for decision in gathered {
        let compute = || {
            compile_decision_imports(std::slice::from_ref(decision))
                .map(|mut models| models.pop().expect("one import, one model"))
                .map_err(|message| anyhow::anyhow!(message))
                .map(Artifact::Decision)
        };
        match cached_artifact(project.as_deref_mut(), &decision.key, compute)? {
            Artifact::Decision(model) => decisions.push(model),
            _ => unreachable!("decision compiles produce decision artifacts"),
        }
    }
    Ok(decisions)
}

/// The artifact for `key`, reusing the project cache when one is present
/// (lookup-before-compute).
fn cached_artifact(
    project: Option<&mut Project>,
    key: &str,
    compute: impl FnOnce() -> anyhow::Result<Artifact>,
) -> anyhow::Result<Artifact> {
    match project {
        Some(project) => project.artifact(key, compute),
        None => compute(),
    }
}

/// The importing file's directory, `"."` for a bare file name — the rule every
/// import resolution here follows.
fn dir_of(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

/// A surface file plus the domain sources it imports, ready for the driver's
/// `compile_*_str` entry points. `.actor`/`.ddd`/`.evt` take this bundle
/// directly; `.uc`/`.cmmn`/`.dmn` wrap it with their pre-compiled buckets.
pub(crate) struct DomainPair {
    /// The surface file's path as given on the command line.
    pub(crate) path: String,
    /// The surface file's source text.
    pub(crate) source: String,
    /// Each imported domain as `(resolved path, source)`; the driver matches a
    /// surface-file import by exact string or by lexical resolution relative
    /// to the importing file's directory, so resolved paths keep vocabulary
    /// snapshots anchored to the declaring file.
    pub(crate) domains: Vec<(String, String)>,
    /// Each imported domain as `(import string as written, source)` — the
    /// driver tags a domain's diagnostics with its `DomainUnit` path (the
    /// import string), so rendering needs both keys.
    import_keys: Vec<(String, String)>,
    /// The domains' `import schema` content, keyed by domain path.
    schemas: SchemaImports,
    /// The domains' `import sigil` content, plus the rosetta sources for
    /// rendering sigil diagnostics.
    sigil: SigilSources,
}

impl DomainPair {
    /// The import bundle the driver entry point takes.
    pub(crate) fn imports(&self) -> DomainImports {
        DomainImports {
            schemas: self.schemas.clone(),
            sigil: self.sigil.imports.clone(),
        }
    }

    /// The source text of the file with the given driver path, if it is the
    /// surface file, an imported domain (by resolved path or by the import
    /// string the surface names it with), or a rosetta source.
    pub(crate) fn source_of(&self, path: &str) -> Option<&str> {
        if path == self.path {
            return Some(&self.source);
        }
        self.domains
            .iter()
            .find(|(name, _)| name == path)
            .map(|(_, source)| source.as_str())
            .or_else(|| {
                self.import_keys
                    .iter()
                    .find(|(name, _)| name == path)
                    .map(|(_, source)| source.as_str())
            })
            .or_else(|| {
                self.sigil
                    .sources
                    .iter()
                    .find(|(name, _)| name == path)
                    .map(|(_, source)| source.as_str())
            })
    }
}

/// One surface file, read once: its path, source, and the import strings its
/// surface declares (empty when the file does not parse — the compile reports
/// that).
struct SurfaceInput {
    file: PathBuf,
    path: String,
    source: String,
    imports: Vec<String>,
}

/// Reads one surface file and parses its written import strings.
fn read_surface(
    file: &Path,
    import_paths: &dyn Fn(&str) -> Vec<String>,
) -> anyhow::Result<SurfaceInput> {
    let (path, source) = read_source(file)?;
    let imports = import_paths(&source);
    Ok(SurfaceInput {
        file: file.to_path_buf(),
        path,
        source,
        imports,
    })
}

impl SurfaceInput {
    /// The domain pair over `domains`: schema/sigil content plus the written
    /// import → gathered text map the reporter resolves domain diagnostics
    /// through.
    fn pair(self, domains: Vec<(String, String)>) -> anyhow::Result<DomainPair> {
        let keys = render_keys(&self.file, &self.imports, &domains);
        self.pair_with_keys(keys, domains)
    }

    /// [`SurfaceInput::pair`] with the render keys already assembled — the
    /// `.ddd` reader adds each fragment's own domain spellings.
    fn pair_with_keys(
        self,
        import_keys: Vec<(String, String)>,
        domains: Vec<(String, String)>,
    ) -> anyhow::Result<DomainPair> {
        let schemas = collect_schema_imports(&domains)?;
        let sigil = collect_sigil_imports(&domains)?;
        Ok(DomainPair {
            path: self.path,
            source: self.source,
            import_keys,
            domains,
            schemas,
            sigil,
        })
    }
}

/// The CLI's hard-error fetch for the lenient gatherers: the first read
/// failure is recorded and surfaced after the gather with the io detail the
/// CLI has always printed. The gather itself only sees a miss.
fn recording_fetch<'a>(
    mut project: Option<&'a mut Project>,
    importer: &'a str,
    error: &'a mut Option<anyhow::Error>,
) -> impl FnMut(&str) -> Option<String> + 'a {
    move |resolved: &str| {
        let fetched = match project.as_deref_mut() {
            Some(project) => project.fetch(resolved),
            None => std::fs::read_to_string(resolved),
        };
        match fetched {
            Ok(text) => Some(text),
            Err(source) => {
                if error.is_none() {
                    *error = Some(anyhow::anyhow!(
                        "cannot read imported file {resolved} (imported by {importer}): {source}"
                    ));
                }
                None
            }
        }
    }
}

/// Reads a surface file plus every domain it imports; `import_paths` yields the
/// file's raw import strings (empty when the file does not parse — the driver
/// reports that). Import paths resolve relative to the importing file's own
/// directory (by [`gather_domain_sources`], deduplicated by resolved key) and
/// the **resolved** path is passed to the driver. A missing import file is a
/// clean error naming the resolved path: the gatherer skips misses silently
/// (the LSP's discipline), so this reader captures the failure in its fetch
/// closure and hard-errors after the call.
fn read_domain_pair(
    file: &Path,
    import_paths: impl Fn(&str) -> Vec<String>,
    project: Option<&mut Project>,
) -> anyhow::Result<DomainPair> {
    let read = read_surface(file, &import_paths)?;
    let mut read_error = None;
    let domains = {
        let mut fetch = recording_fetch(project, &read.path, &mut read_error);
        gather_domain_sources(&read.path, &read.source, import_paths, &mut fetch)
    };
    if let Some(error) = read_error {
        return Err(error);
    }
    read.pair(domains)
}

/// Each import string the surface writes, mapped to the text its resolved
/// gather fetched — the driver tags a domain's diagnostics with the import
/// string (its `DomainUnit` path) and the reporter resolves sources through
/// the pair.
fn render_keys(
    file: &Path,
    imports: &[String],
    domains: &[(String, String)],
) -> Vec<(String, String)> {
    let dir = dir_of(file);
    imports
        .iter()
        .filter_map(|import| {
            let key = dir.join(import).display().to_string();
            domains
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, text)| (import.clone(), text.clone()))
        })
        .collect()
}

/// An `.actor` file plus the domain sources it imports, ready for
/// [`phero_driver::compile_actors_str`].
pub(crate) type ActorPair = DomainPair;

/// Reads an `.actor` file plus every domain it imports, without a project
/// context — gen's cedar/tools flows stay disk-only (gen is not part of the
/// check/build project mode).
pub(crate) fn read_actor_pair(file: &Path) -> anyhow::Result<ActorPair> {
    read_actor_pair_in(file, None)
}

/// [`read_actor_pair`] within a project context: the policy's domain imports
/// fetch map-first from the snapshot.
pub(crate) fn read_actor_pair_in(
    file: &Path,
    project: Option<&mut Project>,
) -> anyhow::Result<ActorPair> {
    read_domain_pair(file, surface_imports(SurfaceKind::Actor), project)
}

/// A `.ddd` design root plus the domain sources and `.ddd` fragments it
/// imports, ready for [`phero_driver::compile_ddd_app_str_with_decisions`]
/// (DD8, issue pherolang#305).
pub(crate) struct DddDesignPair {
    /// The root file and its `.mox` imports.
    pub(crate) pair: DomainPair,
    /// The transitive `.ddd` fragment closure, `(resolved path, source)`.
    pub(crate) fragments: Vec<(String, String)>,
}

impl DddDesignPair {
    /// The source text of the root, an imported domain, or a fragment.
    pub(crate) fn source_of(&self, path: &str) -> Option<&str> {
        self.pair.source_of(path).or_else(|| {
            self.fragments
                .iter()
                .find(|(name, _)| name == path)
                .map(|(_, source)| source.as_str())
        })
    }
}

/// Reads a `.ddd` file plus every domain and `.ddd` fragment it imports.
///
/// The whole application is gathered first (fragments, then the root's and
/// each fragment's domains), so a miss or cycle is reported before compiling —
/// whole or nothing.
pub(crate) fn read_ddd_design(
    file: &Path,
    project: Option<&mut Project>,
) -> anyhow::Result<DddDesignPair> {
    let ddd_imports = surface_imports(SurfaceKind::Ddd);
    let read = read_surface(file, &ddd_imports)?;
    let mut read_error = None;
    let gathered = {
        let mut fetch = recording_fetch(project, &read.path, &mut read_error);
        phero_driver::project::gather_ddd_app_imports(&read.path, &read.source, &mut fetch)
    };
    if let Some(error) = read_error {
        return Err(error);
    }
    if let Some((resolved, importer)) = gathered.missing_domains.first() {
        anyhow::bail!("cannot read imported file {resolved} (imported by {importer})");
    }
    if let Some(missing) = gathered.missing.first() {
        anyhow::bail!("imported design fragment \"{missing}\" was not provided");
    }
    if let Some(cycle) = gathered.cycles.first() {
        anyhow::bail!("design import cycle through {cycle}");
    }
    let domains = gathered.domains;
    let mut import_keys = render_keys(file, &read.imports, &domains);
    // A fragment's `.mox` import renders under its own as-written string; the
    // first spelling of a duplicate key wins (the map is for diagnostics).
    for (fragment_path, fragment_source) in &gathered.fragments {
        for (written, text) in render_keys(
            Path::new(fragment_path),
            &ddd_imports(fragment_source),
            &domains,
        ) {
            if !import_keys.iter().any(|(existing, _)| *existing == written) {
                import_keys.push((written, text));
            }
        }
    }
    Ok(DddDesignPair {
        pair: read.pair_with_keys(import_keys, domains)?,
        fragments: gathered.fragments,
    })
}

/// The written import strings a surface declares (the driver's one
/// per-surface import reader).
fn surface_imports(surface: SurfaceKind) -> impl Fn(&str) -> Vec<String> {
    move |source| phero_driver::project::import_paths(surface, source)
}

/// An `.evt` contract file plus the domain sources it imports, ready for
/// [`phero_driver::compile_evt_str`].
pub(crate) type EvtPair = DomainPair;

/// Reads an `.evt` file plus every domain it imports.
pub(crate) fn read_evt_pair(file: &Path, project: Option<&mut Project>) -> anyhow::Result<EvtPair> {
    read_domain_pair(file, surface_imports(SurfaceKind::Evt), project)
}

/// A `.dmn` decision-model file and its compiled inputs, ready for
/// [`phero_driver::compile_dmn_str`].
pub(crate) struct DmnPair {
    /// The decision-model file plus its raw imported domains.
    pub(crate) pair: DomainPair,
    /// Each `logic table "<path>"` clause's markdown content, keyed by the
    /// path **as written** — the driver matches table paths exactly as
    /// written.
    pub(crate) tables: Vec<(String, String)>,
    /// Each imported `.evt` contract, compiled, keyed by the import string
    /// **as written** (rule 14's bucket).
    pub(crate) events: Vec<(String, phero_ir::events::EventModel)>,
}

/// Reads a `.dmn` file plus every import it declares: `.mox` paths join the
/// domain pair, each `import "<x>.evt"` compiles into the events bucket under
/// its written path, and each `logic table "<path>"` markdown file is read
/// relative to the `.dmn` file. An unreadable table is simply not provided (the
/// driver's unprovided-table diagnostic names the written path); a missing or
/// non-compiling `.evt` import is a clean error naming it.
pub(crate) fn read_dmn_pair(
    file: &Path,
    mut project: Option<&mut Project>,
) -> anyhow::Result<DmnPair> {
    let read = read_surface(file, &dmn_domain_imports)?;
    let tables = table_sources(file, &read.source);
    let events = phero_driver::project::gather_dmn_events(
        &read.path,
        &read.source,
        &mut project_fetch(project.as_deref_mut()),
    )
    .map_err(|message| anyhow::anyhow!(message))?;
    let mut read_error = None;
    let domains = {
        let mut fetch = recording_fetch(project, &read.path, &mut read_error);
        gather_domain_sources(&read.path, &read.source, dmn_domain_imports, &mut fetch)
    };
    if let Some(error) = read_error {
        return Err(error);
    }
    Ok(DmnPair {
        pair: read.pair(domains)?,
        tables,
        events,
    })
}

/// The raw `.dmn` import strings that join the domain gather — `.evt` paths
/// are the contract bucket instead.
fn dmn_domain_imports(source: &str) -> Vec<String> {
    phero_driver::project::dmn_import_paths(source)
        .into_iter()
        .filter(|path| !path.ends_with(".evt"))
        .collect()
}

/// The decision-table contents a `.dmn` source names: every `logic table
/// "<path>"` clause read from disk relative to the `.dmn` file and keyed as
/// written. An unreadable table is skipped (the driver names it); duplicates
/// gather once.
fn table_sources(file: &Path, source: &str) -> Vec<(String, String)> {
    let dir = dir_of(file);
    let mut tables: Vec<(String, String)> = Vec::new();
    for table in phero_driver::project::dmn_table_paths(source) {
        if tables.iter().any(|(name, _)| *name == table) {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(dir.join(&table)) {
            tables.push((table, content));
        }
    }
    tables
}

/// The plain disk-read fetch the driver gatherers take: the gatherer hands back
/// resolved paths, and a miss is simply `None` — the driver renders the pinned
/// missing-import message itself.
fn fetch_disk(resolved: &str) -> Option<String> {
    std::fs::read_to_string(resolved).ok()
}

/// A `.uc` file and its imports, ready for
/// [`phero_driver::compile_usecase_str`].
pub(crate) struct UcPair {
    /// The use case file plus its raw imported domains.
    pub(crate) pair: DomainPair,
    /// Each imported `.ddd` design, compiled, keyed by its resolved path.
    pub(crate) designs: Vec<(String, phero_ir::ddd::DddModel)>,
    /// Each imported `.actor` policy, compiled, keyed by its resolved path.
    pub(crate) actors: Vec<(String, phero_ir::ActorModel)>,
    /// Each imported `.evt` contract, compiled, keyed by its resolved path.
    pub(crate) events: Vec<(String, phero_ir::events::EventModel)>,
}

/// Reads a `.uc` file plus every import it declares, gathered through
/// [`gather_usecase_imports`]: paths resolve relative to the use case file's
/// own directory, bucketed by resolved extension, duplicates gathered once. A
/// missing import file is a clean error naming the resolved path (the driver's
/// message).
pub(crate) fn read_uc_pair(file: &Path, project: Option<&mut Project>) -> anyhow::Result<UcPair> {
    let read = read_surface(file, &surface_imports(SurfaceKind::Uc))?;
    let buckets = gather_usecase_imports(
        &read.path,
        &read.source,
        surface_imports(SurfaceKind::Uc),
        &mut project_fetch(project),
    )
    .map_err(|message| anyhow::anyhow!(message))?;
    Ok(UcPair {
        pair: read.pair(buckets.domains)?,
        designs: buckets.designs,
        actors: buckets.actors,
        events: buckets.events,
    })
}

/// A `.cmmn` file and its compiled imports — the [`UcPair`] counterpart.
pub(crate) struct CmmnPair {
    /// The case file plus its raw imported domains.
    pub(crate) pair: DomainPair,
    /// Each imported `.ddd` design, compiled, keyed by its resolved path.
    pub(crate) designs: Vec<(String, phero_ir::ddd::DddModel)>,
    /// Each imported `.actor` policy, compiled, keyed by its resolved path.
    pub(crate) actors: Vec<(String, phero_ir::ActorModel)>,
    /// Each imported `.evt` contract, compiled, keyed by its resolved path.
    pub(crate) events: Vec<(String, phero_ir::events::EventModel)>,
    /// Each imported `.dmn` decision model, compiled, keyed by its resolved
    /// path. Empty when the file imports no decision models.
    pub(crate) decisions: Vec<(String, phero_ir::dmn::DmnModel)>,
}

/// Reads a `.cmmn` file and gathers its imports exactly like [`read_uc_pair`],
/// through [`gather_case_imports`].
pub(crate) fn read_cmmn_pair(
    file: &Path,
    project: Option<&mut Project>,
) -> anyhow::Result<CmmnPair> {
    let read = read_surface(file, &surface_imports(SurfaceKind::Cmmn))?;
    let buckets = gather_case_imports(
        &read.path,
        &read.source,
        &surface_imports(SurfaceKind::Cmmn),
        &mut project_fetch(project),
    )
    .map_err(|message| anyhow::anyhow!(message))?;
    Ok(CmmnPair {
        pair: read.pair(buckets.domains)?,
        designs: buckets.designs,
        actors: buckets.actors,
        events: buckets.events,
        decisions: buckets.decisions,
    })
}

/// A `.deploy` file plus the pre-compiled design and flow artifacts its imports
/// resolve to, ready for [`phero_driver::compile_deploy_str`].
pub(crate) struct DeployPair {
    /// The deployment file's path as given on the command line.
    pub(crate) path: String,
    /// The deployment file's source text.
    pub(crate) source: String,
    /// Each imported `.ddd` design, compiled, keyed by its resolved path.
    pub(crate) designs: Vec<(String, phero_ir::ddd::DddModel)>,
    /// Each imported `.ifml` flow, compiled, keyed by its resolved path.
    pub(crate) flows: Vec<(String, phero_ir::ifml::IfmlModel)>,
}

impl DeployPair {
    /// The source text of the file with the given driver path — the deploy file
    /// itself (import artifacts arrive pre-compiled, so their diagnostics were
    /// rendered when they were gathered).
    pub(crate) fn source_of(&self, path: &str) -> Option<&str> {
        (path == self.path).then_some(self.source.as_str())
    }
}

/// Reads a `.deploy` file plus every `.ddd` design and `.ifml` flow it imports,
/// each compiled to its artifact — gathered through
/// [`gather_deploy_imports`]. A missing or non-compiling import is a clean
/// error; duplicate imports are gathered once.
pub(crate) fn read_deploy_pair(
    file: &Path,
    project: Option<&mut Project>,
) -> anyhow::Result<DeployPair> {
    let read = read_surface(file, &surface_imports(SurfaceKind::Deploy))?;
    let pairs = gather_deploy_imports(
        &read.path,
        &read.source,
        surface_imports(SurfaceKind::Deploy),
        &mut project_fetch(project),
    )
    .map_err(|message| anyhow::anyhow!(message))?;
    Ok(DeployPair {
        path: read.path,
        source: read.source,
        designs: pairs.designs,
        flows: pairs.flows,
    })
}

/// The fetch closure the bucket gatherers take: map-first within `project` (the
/// driver renders the pinned missing-import message itself, so a miss is simply
/// `None` — no io error to name), plain disk otherwise. The disk fallback fills
/// the map.
fn project_fetch(mut project: Option<&mut Project>) -> impl FnMut(&str) -> Option<String> + '_ {
    move |resolved: &str| -> Option<String> {
        match project.as_deref_mut() {
            Some(project) => project.fetch(resolved).ok(),
            None => fetch_disk(resolved),
        }
    }
}
