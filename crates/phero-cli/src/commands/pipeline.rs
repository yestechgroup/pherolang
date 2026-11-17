//! The one per-surface compile pipeline: `check`, `ir`, `build`, `graph`,
//! `verify`, and the union integrity pass's phase-2 re-check all reach a
//! file's compilation through [`compile_one`]. The multi-file `.mox` model
//! pipeline and the artifact-JSON output tail live here too.
//!
//! The surface table is [`phero_driver::SurfaceKind`] (one extension map for
//! every host). [`compile_one`] is the **one** per-surface match in the
//! CLI, and it is exhaustive with no `_` arm: a new surface is a compile
//! error here until its pipeline exists. The
//! `every_surface_kind_compiles_through_compile_one` test pins every
//! present arm's behavior.

use std::path::{Path, PathBuf};

use phero_driver::{
    compile_actors_str, compile_cmmn_str, compile_ddd_app_str, compile_deploy_str, compile_dmn_str,
    compile_evt_str, compile_usecase_str, DddFragment, SurfaceKind,
};

use crate::inputs::{
    compile_sources, read_actor_pair_in, read_cmmn_pair, read_ddd_design, read_deploy_pair,
    read_dmn_pair, read_evt_pair, read_ifml_inputs, read_sources, read_uc_pair,
};
use crate::project::{Artifact, Project};
use crate::report::{
    report_actor_diagnostics, report_cmmn_diagnostics, report_ddd_diagnostics,
    report_deploy_diagnostics, report_dmn_diagnostics, report_evt_diagnostics,
    report_ifml_diagnostics, report_multi_diagnostics, report_usecase_diagnostics,
};

/// One surface compile: the outcome, or `None` once the diagnostics are
/// rendered (the file did not compile).
type Compiled = anyhow::Result<Option<SurfaceOutcome>>;

/// How a [`compile_one`] call renders the diagnostics it produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reporting {
    /// Render every finding, warnings included: `check`, `ir`, `build`,
    /// `graph`, and `verify` — the compile *is* the command's output.
    Always,
    /// Render a failed compile's diagnostics only. The union integrity
    /// pass's standalone re-check (pherolang#114/#128) stays quiet when it
    /// agrees with phase 1, whose output is the record of the common case.
    OnFailure,
}

/// The request description behind every compile: the shared shape behind
/// project mode, standalone mode, and the union pass's re-check.
pub(crate) struct CompileContext<'a> {
    /// The per-invocation project snapshot (directory batch mode), or
    /// `None` for standalone resolution. This is also the **sibling
    /// scope**: with a snapshot, `.ddd`/`.actor` compiles pool the
    /// siblings of the file's import-connected application (pherolang#249,
    /// [`Project::sibling_actors`]) and every artifact lands in the cache
    /// lookup-before-compute; without one there are no siblings and nothing
    /// is stored — exactly the plain `compile_*_str` behavior.
    pub(crate) project: Option<&'a mut Project>,
    /// `ir --expand`'s opt-in inlining of `.ifml` module uses. Only ever
    /// affects the emitted JSON.
    pub(crate) expand: bool,
    /// Whether the artifact JSON is serialized. The union pass consumes
    /// only the typed artifact and the resolved imports, so it skips the
    /// work.
    pub(crate) serialize: bool,
    /// How this call renders its diagnostics.
    pub(crate) reporting: Reporting,
}

impl CompileContext<'_> {
    /// Whether this call renders a compile's diagnostics: always under
    /// [`Reporting::Always`]; under [`Reporting::OnFailure`] only a failed
    /// compile (whose diagnostics are the report).
    fn renders(&self, failed: bool) -> bool {
        self.reporting == Reporting::Always || failed
    }

    /// Records `artifact` in the snapshot's cache (project mode only) so
    /// sibling consumers reuse it lookup-before-compute.
    fn store(&mut self, file: &Path, artifact: &Artifact) {
        if let Some(project) = self.project.as_deref_mut() {
            project.store(&file.display().to_string(), artifact);
        }
    }
}

/// The multi-file `.mox` pipeline: read the sources, compile them as one
/// multi-package model, and render the diagnostics grouped per file (the
/// `.mox` diagnostics first, then the `import sigil` diagnostics against
/// their rosetta sources).
fn compile_reported_sources(
    files: &[PathBuf],
) -> anyhow::Result<(Vec<(String, String)>, phero_driver::MultiCompilation)> {
    let sources = read_sources(files)?;
    let (compilation, sigil_sources) = compile_sources(&sources)?;
    report_multi_diagnostics(
        &sources,
        &sigil_sources.sources,
        &compilation.diagnostics,
        &compilation.sigil_diagnostics,
    );
    Ok((sources, compilation))
}

/// The `ir` emission tail: write `json` to `out`, or print it when no
/// output path was given.
pub(crate) fn emit_json(json: String, out: Option<&Path>) -> anyhow::Result<()> {
    match out {
        Some(out_path) => std::fs::write(out_path, json)?,
        None => println!("{json}"),
    }
    Ok(())
}

/// One compile outcome shared by `check`, `ir`, `build`, and `graph`: the
/// `OK` lines `check` prints, the artifact JSON `ir`/`build` emit, and the
/// typed artifact plus resolved imports `graph` and the union pass consume —
/// one compile feeds all of them.
pub(crate) struct SurfaceOutcome {
    /// The compiling sources' driver paths; `check` prints one
    /// `OK <path>` line each (a multi-`.mox` model prints one per file).
    pub(crate) ok: Vec<String>,
    /// The artifact JSON exactly `pherolang ir` emits for the surface (empty
    /// when the caller asked for no serialization).
    pub(crate) json: String,
    /// How many warning-severity diagnostics were rendered. Warnings never
    /// fail a compile; `check --deny-warnings` turns them into failures.
    pub(crate) warnings: usize,
    /// The typed artifact of the compiling surface — the semantic graph's
    /// input; `check`/`ir`/`build` ignore it.
    pub(crate) artifact: Option<Artifact>,
    /// The surface's resolved import paths, as its reader resolved them
    /// (the host normalizes them for the graph). Empty for a domain model,
    /// whose `import schema`/`import sigil` targets are not graph surfaces.
    pub(crate) imports: Vec<String>,
}

/// The one per-surface dispatch: read `file` within the context's sibling
/// scope (or standalone), compile it, and render diagnostics per the
/// context. `Ok(None)` means the file did not compile — the diagnostics are
/// already rendered. Every CLI command and the union pass's phase 2 call
/// this.
pub(crate) fn compile_one(
    kind: SurfaceKind,
    file: &Path,
    ctx: &mut CompileContext<'_>,
) -> Compiled {
    match kind {
        SurfaceKind::Mox => compile_domains(std::slice::from_ref(&file.to_path_buf()), ctx),
        SurfaceKind::Ifml => compile_ifml(file, ctx),
        SurfaceKind::Ddd => compile_ddd(file, ctx),
        SurfaceKind::Actor => compile_actor(file, ctx),
        SurfaceKind::Evt => compile_evt(file, ctx),
        SurfaceKind::Deploy => compile_deploy(file, ctx),
        SurfaceKind::Uc => compile_usecase(file, ctx),
        SurfaceKind::Cmmn => compile_cmmn(file, ctx),
        SurfaceKind::Dmn => compile_dmn(file, ctx),
    }
}

/// The per-surface pipeline behind `check`, `ir`, and `build`: one file
/// compiles through [`compile_one`] (an unknown extension keeps compiling
/// as a domain, the historical default), several compile as one
/// multi-package `.mox` model. `expand` is `ir --expand`'s opt-in inlining
/// of `.ifml` module uses; check, build, graph, and verify pass `false`.
pub(crate) fn compile_surface(
    files: &[PathBuf],
    project: Option<&mut Project>,
    expand: bool,
) -> Compiled {
    let mut ctx = CompileContext {
        project,
        expand,
        serialize: true,
        reporting: Reporting::Always,
    };
    match files {
        [file] => {
            let kind = SurfaceKind::of_path(file).unwrap_or(SurfaceKind::Mox);
            compile_one(kind, file, &mut ctx)
        }
        _ => compile_domains(files, &mut ctx),
    }
}

/// The `.mox` pipeline, for one file or a multi-package model. The grouped
/// per-file reporter runs unconditionally: the multi-file model has no
/// single compilation to gate on, and the domain surface has always
/// rendered its warnings in both `check` phases.
fn compile_domains(files: &[PathBuf], ctx: &mut CompileContext<'_>) -> Compiled {
    let (sources, compilation) = compile_reported_sources(files)?;
    let Some(model) = compilation.model else {
        return Ok(None);
    };
    let ok = sources.iter().map(|(path, _)| path.clone()).collect();
    let artifact = Artifact::Domain(model);
    if let [file] = files {
        ctx.store(file, &artifact);
    }
    let json = if ctx.serialize {
        artifact.to_json_pretty()?
    } else {
        String::new()
    };
    Ok(Some(SurfaceOutcome {
        ok,
        json,
        warnings: 0,
        artifact: Some(artifact),
        imports: Vec::new(),
    }))
}

/// The `.ifml` pipeline. Warnings (the `ifml.lints` band) render but never
/// fail the file; only error-severity diagnostics do. `check
/// --deny-warnings` flips the rule via the outcome's warning count.
fn compile_ifml(file: &Path, ctx: &mut CompileContext<'_>) -> Compiled {
    let inputs = read_ifml_inputs(file, ctx.project.as_deref_mut())?;
    let compilation =
        match phero_ifml::compile_ifml_str(&inputs.path, &inputs.source, &inputs.ifml_imports) {
            Ok(compilation) => compilation,
            Err(diagnostics) => {
                report_ifml_diagnostics(&inputs.sources, &diagnostics);
                return Ok(None);
            }
        };
    let binding = phero_ifml::check_ifml_with_decisions(
        &compilation,
        inputs.domains.as_ref(),
        &inputs.actors,
        &inputs.decisions,
    );
    let warnings = binding
        .iter()
        .filter(|(_, diagnostic)| !diagnostic.severity.is_error())
        .count();
    let failed = warnings < binding.len();
    if ctx.renders(failed) && !binding.is_empty() {
        let sources: Vec<(String, String)> = compilation
            .index
            .files
            .iter()
            .map(|file| (file.path.clone(), file.text.clone()))
            .collect();
        report_ifml_diagnostics(&sources, &binding);
    }
    if failed {
        return Ok(None);
    }
    let artifact = Artifact::Flow(compilation.model.clone());
    ctx.store(file, &artifact);
    let imports = inputs.imports.clone();
    let json = if ctx.serialize {
        let model = if ctx.expand {
            phero_ifml::expand_ifml(&compilation, &inputs.ifml_imports)
        } else {
            compilation.model
        };
        model.to_json_pretty()?
    } else {
        String::new()
    };
    Ok(Some(SurfaceOutcome {
        ok: vec![inputs.path.clone()],
        json,
        warnings,
        artifact: Some(artifact),
        imports,
    }))
}

/// The `.ddd` pipeline: the application-scoped `.actor` joins (pherolang#128
/// J3) and `.dmn` decision links (pherolang#78 rule 17) feed the compile. An
/// empty pool is exactly the plain `compile_ddd_str` behavior.
fn compile_ddd(file: &Path, ctx: &mut CompileContext<'_>) -> Compiled {
    let mut design = read_ddd_design(file, ctx.project.as_deref_mut())?;
    let actors = ctx
        .project
        .as_deref_mut()
        .map_or_else(Vec::new, |project| project.sibling_actors(file));
    let decisions = ctx
        .project
        .as_deref_mut()
        .map_or_else(Vec::new, |project| project.sibling_decisions(file));
    design
        .pair
        .extend_inputs(|inputs| inputs.with_actors(actors).with_decisions(decisions));
    let fragments: Vec<DddFragment> = design
        .fragments
        .iter()
        .map(|(path, source)| DddFragment {
            path: path.clone(),
            source: source.clone(),
        })
        .collect();
    let compilation = compile_ddd_app_str(
        &design.pair.path,
        &design.pair.source,
        &fragments,
        design.pair.inputs(),
    );
    if ctx.renders(compilation.model.is_none()) {
        report_ddd_diagnostics(&design, &compilation);
    }
    let Some(model) = compilation.model else {
        return Ok(None);
    };
    let mut imports = import_paths(design.pair.inputs().domains());
    imports.extend(design.fragments.iter().map(|(path, _)| path.clone()));
    imports.sort();
    imports.dedup();
    let mut ok = vec![design.pair.path.clone()];
    ok.extend(design.fragments.iter().map(|(path, _)| path.clone()));
    outcome_many(ctx, file, Artifact::Design(model), ok, imports)
}

/// [`outcome`] with several compiling source paths (a design root plus its
/// fragments): `check` prints one `OK` line per source.
fn outcome_many(
    ctx: &mut CompileContext<'_>,
    file: &Path,
    artifact: Artifact,
    ok: Vec<String>,
    imports: Vec<String>,
) -> Compiled {
    ctx.store(file, &artifact);
    let json = if ctx.serialize {
        artifact.to_json_pretty()?
    } else {
        String::new()
    };
    Ok(Some(SurfaceOutcome {
        ok,
        json,
        warnings: 0,
        artifact: Some(artifact),
        imports,
    }))
}

/// The `.actor` pipeline: grant coverage resolves against the
/// application-scoped `.ddd` artifacts (pherolang#128 J4). An empty set is
/// exactly the plain `compile_actors_str` behavior.
fn compile_actor(file: &Path, ctx: &mut CompileContext<'_>) -> Compiled {
    let mut pair = read_actor_pair_in(file, ctx.project.as_deref_mut())?;
    let designs = ctx
        .project
        .as_deref_mut()
        .map_or_else(Vec::new, |project| project.sibling_designs(file));
    pair.extend_inputs(|inputs| inputs.with_designs(designs));
    let compilation = compile_actors_str(&pair.path, &pair.source, pair.inputs());
    if ctx.renders(compilation.model.is_none()) {
        report_actor_diagnostics(&pair, &compilation);
    }
    let Some(model) = compilation.model else {
        return Ok(None);
    };
    let imports = import_paths(pair.inputs().domains());
    outcome(ctx, file, Artifact::Actor(model), pair.path, imports)
}

/// The `.evt` pipeline.
fn compile_evt(file: &Path, ctx: &mut CompileContext<'_>) -> Compiled {
    let pair = read_evt_pair(file, ctx.project.as_deref_mut())?;
    let compilation = compile_evt_str(&pair.path, &pair.source, pair.inputs());
    if ctx.renders(compilation.model.is_none()) {
        report_evt_diagnostics(&pair, &compilation);
    }
    let Some(model) = compilation.model else {
        return Ok(None);
    };
    let imports = import_paths(pair.inputs().domains());
    outcome(ctx, file, Artifact::Event(model), pair.path, imports)
}

/// The `.deploy` pipeline.
fn compile_deploy(file: &Path, ctx: &mut CompileContext<'_>) -> Compiled {
    let pair = read_deploy_pair(file, ctx.project.as_deref_mut())?;
    let compilation = compile_deploy_str(&pair.path, &pair.source, pair.inputs());
    if ctx.renders(compilation.model.is_none()) {
        report_deploy_diagnostics(&pair, &compilation);
    }
    let Some(model) = compilation.model else {
        return Ok(None);
    };
    let imports = merge_import_paths(&[
        import_paths(pair.inputs().designs()),
        import_paths(pair.inputs().flows()),
    ]);
    outcome(ctx, file, Artifact::Deploy(model), pair.path, imports)
}

/// The `.uc` pipeline.
fn compile_usecase(file: &Path, ctx: &mut CompileContext<'_>) -> Compiled {
    let pair = read_uc_pair(file, ctx.project.as_deref_mut())?;
    let compilation = compile_usecase_str(&pair.path, &pair.source, pair.inputs());
    if ctx.renders(compilation.model.is_none()) {
        report_usecase_diagnostics(&pair, &compilation);
    }
    let Some(model) = compilation.model else {
        return Ok(None);
    };
    let inputs = pair.inputs();
    let imports = merge_import_paths(&[
        import_paths(inputs.domains()),
        import_paths(inputs.designs()),
        import_paths(inputs.actors().unwrap_or_default()),
        import_paths(inputs.events()),
    ]);
    outcome(ctx, file, Artifact::UseCase(model), pair.path, imports)
}

/// The `.cmmn` pipeline.
fn compile_cmmn(file: &Path, ctx: &mut CompileContext<'_>) -> Compiled {
    let pair = read_cmmn_pair(file, ctx.project.as_deref_mut())?;
    let compilation = compile_cmmn_str(&pair.path, &pair.source, pair.inputs());
    if ctx.renders(compilation.model.is_none()) {
        report_cmmn_diagnostics(&pair, &compilation);
    }
    let Some(model) = compilation.model else {
        return Ok(None);
    };
    let inputs = pair.inputs();
    let imports = merge_import_paths(&[
        import_paths(inputs.domains()),
        import_paths(inputs.designs()),
        import_paths(inputs.actors().unwrap_or_default()),
        import_paths(inputs.events()),
        import_paths(inputs.decisions()),
    ]);
    outcome(ctx, file, Artifact::Case(model), pair.path, imports)
}

/// The `.dmn` pipeline.
fn compile_dmn(file: &Path, ctx: &mut CompileContext<'_>) -> Compiled {
    let pair = read_dmn_pair(file, ctx.project.as_deref_mut())?;
    let compilation = compile_dmn_str(&pair.path, &pair.source, pair.inputs());
    if ctx.renders(compilation.model.is_none()) {
        report_dmn_diagnostics(&pair, &compilation);
    }
    let Some(model) = compilation.model else {
        return Ok(None);
    };
    let imports = merge_import_paths(&[
        import_paths(pair.inputs().domains()),
        written_import_paths(file, pair.inputs().events()),
    ]);
    outcome(ctx, file, Artifact::Decision(model), pair.path, imports)
}

/// The tail every single-artifact surface shares: cache the typed artifact
/// lookup-before-compute and wrap the outcome with its `OK` path, resolved
/// import keys, and — when asked — the artifact JSON.
fn outcome(
    ctx: &mut CompileContext<'_>,
    file: &Path,
    artifact: Artifact,
    ok: String,
    imports: Vec<String>,
) -> Compiled {
    ctx.store(file, &artifact);
    let json = if ctx.serialize {
        artifact.to_json_pretty()?
    } else {
        String::new()
    };
    Ok(Some(SurfaceOutcome {
        ok: vec![ok],
        json,
        warnings: 0,
        artifact: Some(artifact),
        imports,
    }))
}

/// The resolved import paths of one `(path, payload)` reader bucket,
/// deduplicated and sorted — the host-normalized keys `pherolang graph build`
/// clusters applications and draws `imports` edges from.
fn import_paths<T>(buckets: &[(String, T)]) -> Vec<String> {
    let mut paths: Vec<String> = buckets.iter().map(|(path, _)| path.clone()).collect();
    paths.sort();
    paths.dedup();
    paths
}

/// [`import_paths`] for a `.dmn` events bucket, whose keys are the import
/// strings **as written** (the driver matches events entries exactly as
/// written): resolve each against the decision file's own directory, the
/// rule every import resolution here follows.
fn written_import_paths<T>(file: &Path, buckets: &[(String, T)]) -> Vec<String> {
    let directory = file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    buckets
        .iter()
        .map(|(written, _)| directory.join(written).display().to_string())
        .collect()
}

/// The union of several import-path buckets, deduplicated and sorted.
fn merge_import_paths(groups: &[Vec<String>]) -> Vec<String> {
    let mut paths: Vec<String> = groups.iter().flatten().cloned().collect();
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The workspace fixture application: one **compiling** file per
    /// surface (`tests/conformance/graph/app.*`).
    fn fixture(kind: SurfaceKind) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/conformance/graph")
            .join(format!("app.{}", kind.extension()))
    }

    /// The surface an outcome's artifact names — the test's own mapping, so
    /// a new `Artifact` variant is a compile error here too.
    fn artifact_surface(artifact: &Artifact) -> SurfaceKind {
        match artifact {
            Artifact::Domain(_) => SurfaceKind::Mox,
            Artifact::Actor(_) => SurfaceKind::Actor,
            Artifact::Design(_) => SurfaceKind::Ddd,
            Artifact::Event(_) => SurfaceKind::Evt,
            Artifact::Deploy(_) => SurfaceKind::Deploy,
            Artifact::UseCase(_) => SurfaceKind::Uc,
            Artifact::Case(_) => SurfaceKind::Cmmn,
            Artifact::Decision(_) => SurfaceKind::Dmn,
            Artifact::Flow(_) => SurfaceKind::Ifml,
        }
    }

    /// Every `SurfaceKind` has a working arm: `compile_one`'s match is
    /// exhaustive with no `_`, so a new variant does not compile until its
    /// pipeline exists, and this test proves each present arm reaches its
    /// own pipeline — the fixture compiles, returns its one `OK` path, and
    /// carries the artifact of the kind that was dispatched.
    #[test]
    fn every_surface_kind_compiles_through_compile_one() {
        for kind in SurfaceKind::ALL {
            let file = fixture(kind);
            let mut ctx = CompileContext {
                project: None,
                expand: false,
                serialize: false,
                reporting: Reporting::OnFailure,
            };
            let outcome = compile_one(kind, &file, &mut ctx)
                .unwrap_or_else(|error| panic!("{}: {error:#}", file.display()))
                .unwrap_or_else(|| panic!("{} did not compile", file.display()));
            assert_eq!(outcome.ok, [file.display().to_string()]);
            assert_eq!(
                artifact_surface(outcome.artifact.as_ref().expect("an artifact")),
                kind,
                "{} fed the wrong arm",
                kind.extension()
            );
            assert!(outcome.json.is_empty(), "serialize: false skips the JSON");
        }
    }
}
