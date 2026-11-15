//! The union integrity pass (pherolang#114), run from directory-mode `check`
//! as a second phase after the per-file project pipeline.
//!
//! Phase 1 (the project pipeline in `check`) checks every file
//! independently with its imports resolving map-first from the snapshot.
//! Since pherolang#128 it also owns the capability joins (existence and
//! coverage) as spanned compile-time errors. Phase 2 re-compiles each file
//! standalone (no project context — the semantics the integrity contract
//! is specified at) through the one shared [`compile_one`] pipeline, and
//! gathers the compiled artifacts of every successful check into
//! import-connected applications; [`phero_driver::check_integrity`] then
//! diagnoses the dangling cross-layer wiring each: errors for the joins the
//! compiler does not yet own, and the capability findings only as the lint
//! channel — suppressed for files phase 1 already failed (their spanned
//! compiler error rendered there). Per-file diagnostics are re-reported here
//! only when the standalone compile disagrees with phase 1 (or a file was
//! skipped there), so the union pass stays quiet in the common case.
//!
//! There are no per-surface compile arms here: the standalone re-check is
//! `CompileContext { project: None, reporting: OnFailure, serialize: false }`
//! plus [`compile_one`], and this pass only classifies the returned
//! artifact into its bucket.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use phero_driver::applications::{applications, ApplicationFile};
use phero_driver::{check_integrity, Diagnostic, SurfaceKind};
use phero_ir::ddd::DddModel;
use phero_ir::deploy::DeployModel;
use phero_ir::events::EventModel;
use phero_ir::ifml::IfmlModel;
use phero_ir::ActorModel;

use crate::project::Artifact;

use super::pipeline::{compile_one, CompileContext, Reporting};

/// Runs the pass over the expanded directory inputs; returns the number of
/// failing integrity diagnostics — error-severity always, plus the
/// warnings when `deny_warnings` is set (file failures already reported).
/// `checked_clean` carries the driver paths phase 1 compiled: a file that
/// failed there already owns its spanned compiler diagnostics, so the
/// pass's lint-channel warnings for it stay unprinted (pherolang#128).
pub(crate) fn run_union_pass(
    expanded: &[PathBuf],
    checked_clean: &BTreeSet<String>,
    deny_warnings: bool,
) -> anyhow::Result<usize> {
    let mut contributions: Vec<Contribution> = Vec::new();
    for file in expanded {
        // A `.ddd` fragment is merged into its root's artifact; compiling
        // it standalone would report a spurious "compile the root" error
        // (DD8).
        if SurfaceKind::of_path(file) == Some(SurfaceKind::Ddd)
            && std::fs::read_to_string(file).ok().is_some_and(|source| {
                phero_syntax::parse_ddd(&source)
                    .ast
                    .is_some_and(|ast| ast.application.is_none() && !ast.fragments.is_empty())
            })
        {
            continue;
        }
        if let Some(contribution) = check_contribution(file)? {
            contributions.push(contribution);
        }
    }
    Ok(report_union_integrity(
        &contributions,
        checked_clean,
        deny_warnings,
    ))
}

/// The compiled artifact a successful standalone check contributes to the
/// union integrity pass, with the raw material for clustering: the file's
/// display path (how diagnostics and the source map name it), its source
/// text, and the imports to cluster through.
struct Contribution {
    path: String,
    source: String,
    imports: Vec<ImportPath>,
    artifact: Option<Bucket>,
}

/// One import of a contribution, in the form its reader produced: the
/// `.ddd`/`.actor`/`.evt`/`.deploy` readers resolve imports against the
/// importing file's directory (the driver keys), while an `.ifml` artifact
/// carries the statements as written.
enum ImportPath {
    Resolved(String),
    AsWritten(String),
}

impl ImportPath {
    /// The file this import names, relative to the process working
    /// directory.
    fn target(&self, dir: &Path) -> PathBuf {
        match self {
            ImportPath::Resolved(path) => PathBuf::from(path),
            ImportPath::AsWritten(path) => dir.join(path),
        }
    }
}

/// One artifact bucket of the union pass. `.mox` domains and `.uc`/`.cmmn`
/// files contribute no artifact — domains are not a check input, and the
/// use case surfaces are consumers of the other buckets — but `.mox` files
/// still join the clustering as bridges between the surfaces importing
/// them.
enum Bucket {
    Design(DddModel),
    Actors(ActorModel),
    Events(EventModel),
    Deploys(DeployModel),
    Flows(IfmlModel),
}

/// Standalone check of one input through the shared pipeline: no project
/// snapshot (the union contract's resolution — no sibling joins, no cache),
/// diagnostics rendered only when this re-check disagrees with phase 1, and
/// no `OK` printing (phase 1 owns the per-file output). A successful compile
/// contributes the bucket the graph's cross-layer wiring needs.
fn check_contribution(file: &Path) -> anyhow::Result<Option<Contribution>> {
    let kind = SurfaceKind::of_path(file).unwrap_or(SurfaceKind::Mox);
    let mut ctx = CompileContext {
        project: None,
        expand: false,
        serialize: false,
        reporting: Reporting::OnFailure,
    };
    let Some(outcome) = compile_one(kind, file, &mut ctx)? else {
        return Ok(None);
    };
    let Some(artifact) = outcome.artifact else {
        return Ok(None);
    };
    // An `.ifml` contribution clusters through the imports **as written**
    // (its artifact's own list); every other surface's reader already
    // resolved them against the importing file's directory.
    let (artifact, imports) = match artifact {
        Artifact::Design(model) => (Some(Bucket::Design(model)), resolved(outcome.imports)),
        Artifact::Actor(model) => (Some(Bucket::Actors(model)), resolved(outcome.imports)),
        Artifact::Event(model) => (Some(Bucket::Events(model)), resolved(outcome.imports)),
        Artifact::Deploy(model) => (Some(Bucket::Deploys(model)), resolved(outcome.imports)),
        Artifact::Flow(model) => {
            let imports = model
                .imports
                .iter()
                .cloned()
                .map(ImportPath::AsWritten)
                .collect();
            (Some(Bucket::Flows(model)), imports)
        }
        // A `.mox` domain joins the clustering as a bridge node (no bucket):
        // the surfaces importing it share one application through it.
        Artifact::Domain(_) => (None, Vec::new()),
        // The use case surfaces are consumers of the other buckets, not
        // check inputs (the `.uc`/`.cmmn` precedent): they compile — so a
        // failure still re-reports — and contribute nothing.
        Artifact::UseCase(_) | Artifact::Case(_) | Artifact::Decision(_) => return Ok(None),
    };
    let source = std::fs::read_to_string(file)
        .map_err(|error| anyhow::anyhow!("cannot read {}: {error}", file.display()))?;
    Ok(Some(Contribution {
        path: file.display().to_string(),
        source,
        imports,
        artifact,
    }))
}

/// The resolved import keys of an outcome in the contribution's terms.
fn resolved(imports: Vec<String>) -> Vec<ImportPath> {
    imports.into_iter().map(ImportPath::Resolved).collect()
}

/// Clusters the contributions into import-connected applications and runs
/// [`check_integrity`] per application, rendering its diagnostics against
/// the owning files. Returns the number of failing diagnostics —
/// error-severity always, plus warnings when `deny_warnings` is set.
/// Warnings (the pherolang#128 lint channel for the capability joins) render
/// only for files phase 1 checked clean — a failed file's compiler error
/// is the report of record — and that suppression rule holds under
/// `deny_warnings` too.
///
/// Two files share an application when an import of one resolves to the
/// other (transitively, through the `.mox` domains every surface names) —
/// the house import graph, the same relation the per-surface compilers
/// already resolve. Unrelated applications in one checked tree never
/// cross-check each other, and an import that leaves the checked set (or
/// misses the disk) contributes no edge.
fn report_union_integrity(
    contributions: &[Contribution],
    checked_clean: &BTreeSet<String>,
    deny_warnings: bool,
) -> usize {
    // The clustering's inputs are host-normalized: canonicalize every
    // contribution path and every resolved import target here (the driver
    // stays filesystem-free and matches strings only). A path that cannot be
    // canonicalized (missing on disk) forms no node — the caller-side
    // precondition `phero_driver::applications` documents.
    let mut owners: Vec<usize> = Vec::with_capacity(contributions.len());
    let mut files: Vec<ApplicationFile> = Vec::with_capacity(contributions.len());
    // One node per canonical path, first contribution wins — the artifact set
    // the pre-hoist union-find (`contributor_of`) gathered.
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (index, contribution) in contributions.iter().enumerate() {
        let Ok(path) = std::fs::canonicalize(&contribution.path) else {
            continue;
        };
        let path = path.display().to_string();
        if !seen.insert(path.clone()) {
            continue;
        }
        let dir = Path::new(&contribution.path)
            .parent()
            .unwrap_or(Path::new("."));
        let imports = contribution
            .imports
            .iter()
            .filter_map(|import| std::fs::canonicalize(import.target(dir)).ok())
            .map(|target| target.display().to_string())
            .collect();
        files.push(ApplicationFile { path, imports });
        owners.push(index);
    }
    let ordered = applications(&files);

    let sources: BTreeMap<&str, &str> = contributions
        .iter()
        .map(|contribution| (contribution.path.as_str(), contribution.source.as_str()))
        .collect();
    let mut total_errors = 0usize;
    for members in &ordered {
        let mut designs = Vec::new();
        let mut actors = Vec::new();
        let mut events = Vec::new();
        let mut deploys = Vec::new();
        let mut flows = Vec::new();
        for &member in members {
            let contribution = &contributions[owners[member]];
            match &contribution.artifact {
                Some(Bucket::Design(model)) => {
                    designs.push((contribution.path.clone(), model.clone()))
                }
                Some(Bucket::Actors(model)) => {
                    actors.push((contribution.path.clone(), model.clone()))
                }
                Some(Bucket::Events(model)) => {
                    events.push((contribution.path.clone(), model.clone()))
                }
                Some(Bucket::Deploys(model)) => {
                    deploys.push((contribution.path.clone(), model.clone()))
                }
                Some(Bucket::Flows(model)) => {
                    flows.push((contribution.path.clone(), model.clone()))
                }
                None => {}
            }
        }
        if designs.is_empty()
            && actors.is_empty()
            && events.is_empty()
            && deploys.is_empty()
            && flows.is_empty()
        {
            continue;
        }
        let diagnostics = check_integrity(&designs, &actors, &events, &deploys, &flows);
        for (path, group) in group_by_path(&diagnostics) {
            let Some(source) = sources.get(path.as_str()) else {
                continue;
            };
            let errors = group
                .iter()
                .filter(|diagnostic| diagnostic.is_error())
                .count();
            let warnings = if deny_warnings {
                group
                    .iter()
                    .filter(|diagnostic| !diagnostic.is_error())
                    .count()
            } else {
                0
            };
            // pherolang#128: a lint-channel-only group for a file whose
            // phase-1 check already failed stays unprinted — the
            // compiler's spanned error rendered there.
            if errors == 0 && !checked_clean.contains(path.as_str()) {
                continue;
            }
            total_errors += errors + warnings;
            eprint!("{}", phero_driver::render(&path, source, &group));
        }
    }
    total_errors
}

/// Groups path-tagged diagnostics into consecutive same-path runs.
fn group_by_path(diagnostics: &[(String, Diagnostic)]) -> Vec<(String, Vec<Diagnostic>)> {
    let mut groups: Vec<(String, Vec<Diagnostic>)> = Vec::new();
    for (path, diagnostic) in diagnostics {
        match groups.last_mut() {
            Some((group_path, group)) if *group_path == *path => group.push(diagnostic.clone()),
            _ => groups.push((path.clone(), vec![diagnostic.clone()])),
        }
    }
    groups
}
