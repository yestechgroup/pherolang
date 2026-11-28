//! Diagnostics rendering for the command handlers: the ariadne render
//! helper and the per-file grouped reporters for multi-model, `.actor`,
//! `.ddd`, `.evt`, `.dmn`, `.deploy`, `.uc`, `.cmmn`, and `.ifml` compiles.

use phero_driver::{render, ActorCompilation};

use crate::inputs::{ActorPair, DddDesignPair, DeployPair, DomainPair, EvtPair};

/// Renders a compilation's diagnostics grouped per file: each file's
/// diagnostics are ariadne-rendered against that file's own source, the way
/// `check` renders single-model diagnostics. The `.mox` diagnostics render
/// first, then the `import sigil` diagnostics against their rosetta sources.
pub(crate) fn report_multi_diagnostics(
    sources: &[(String, String)],
    rosetta_sources: &[(String, String)],
    diagnostics: &[(String, phero_driver::Diagnostic)],
    sigil_diagnostics: &[(String, phero_driver::Diagnostic)],
) {
    let lookup = |path: &str| {
        sources
            .iter()
            .chain(rosetta_sources)
            .find(|(name, _)| name == path)
            .map(|(_, source)| source.as_str())
    };
    render_grouped(diagnostics, lookup);
    render_grouped(sigil_diagnostics, lookup);
}

/// Renders Rune diagnostics grouped per file, each against its pool text
/// (RN10).
pub(crate) fn report_rune_diagnostics(
    diagnostics: &[(String, phero_driver::Diagnostic)],
    pool: &[(String, String)],
) {
    render_grouped(diagnostics, |path| {
        pool.iter()
            .find(|(key, _)| key == path)
            .map(|(_, text)| text.as_str())
    });
}

/// Renders an actor compilation's diagnostics grouped per file: each file's
/// diagnostics are ariadne-rendered against that file's own source, the way
/// `check` renders single-model diagnostics.
pub(crate) fn report_actor_diagnostics(pair: &ActorPair, compilation: &ActorCompilation) {
    render_grouped(&compilation.diagnostics, |path| pair.source_of(path));
}

/// The `.intent` diagnostics (issue pherolang#196): the same grouped
/// rendering as the actor surface (the file plus its imported domains).
pub(crate) fn report_intent_diagnostics(
    pair: &crate::inputs::DomainPair,
    compilation: &phero_driver::IntentCompilation,
) {
    render_grouped(&compilation.diagnostics, |path| pair.source_of(path));
}

/// Renders a `.ddd` compilation's diagnostics grouped per file: each file's
/// diagnostics are ariadne-rendered against that file's own source, the way
/// `check` renders single-model diagnostics.
pub(crate) fn report_ddd_diagnostics(
    pair: &DddDesignPair,
    compilation: &phero_driver::DddCompilation,
) {
    render_grouped(&compilation.diagnostics, |path| pair.source_of(path));
}

/// Renders an `.evt` compilation's diagnostics grouped per file: each file's
/// diagnostics are ariadne-rendered against that file's own source, the way
/// `check` renders single-model diagnostics.
pub(crate) fn report_evt_diagnostics(pair: &EvtPair, compilation: &phero_driver::EventCompilation) {
    render_grouped(&compilation.diagnostics, |path| pair.source_of(path));
}

/// Renders a `.dmn` compilation's diagnostics grouped per file: each file's
/// diagnostics are ariadne-rendered against that file's own source, the way
/// `check` renders single-model diagnostics.
pub(crate) fn report_dmn_diagnostics(
    pair: &DomainPair,
    compilation: &phero_driver::DmnCompilation,
) {
    render_grouped(&compilation.diagnostics, |path| pair.source_of(path));
}

/// Renders a `.deploy` compilation's diagnostics (all tagged with the deploy
/// file's path; its imports arrive pre-compiled and their diagnostics were
/// rendered when gathered).
pub(crate) fn report_deploy_diagnostics(
    pair: &DeployPair,
    compilation: &phero_driver::DeployCompilation,
) {
    render_grouped(&compilation.diagnostics, |path| pair.source_of(path));
}

/// Renders a `.uc` compilation's diagnostics grouped per file: each file's
/// diagnostics are ariadne-rendered against that file's own source, the way
/// `check` renders single-model diagnostics (design, actor, and event
/// imports arrive pre-compiled and their diagnostics were rendered when
/// gathered).
pub(crate) fn report_usecase_diagnostics(
    pair: &DomainPair,
    compilation: &phero_driver::UseCaseCompilation,
) {
    render_grouped(&compilation.diagnostics, |path| pair.source_of(path));
}

/// Renders a `.cmmn` compilation's diagnostics grouped per file,
/// ariadne-rendered against each file's own source — the case file and
/// every imported domain.
pub(crate) fn report_cmmn_diagnostics(
    pair: &DomainPair,
    compilation: &phero_driver::CmmnCompilation,
) {
    render_grouped(&compilation.diagnostics, |path| pair.source_of(path));
}

/// Renders an `.ifml` compilation's diagnostics (resolver, typed-binding
/// checker, and lints) grouped per file, ariadne-rendered against each
/// file's own source — the main interaction file and every imported
/// `.ifml` file. Severity is carried through: lints render as warnings.
///
/// The records are the workspace's shared diagnostic (SK2 of epic
/// pherolang#328) tagged by file, so this reporter renders them through
/// the same [`render_grouped`] path as every other surface. The visible
/// `.ifml` line keeps the `"{code} {message}"` join the CLI has always
/// printed — joined here, at print time, from the record's `code`; the
/// stored message stays bare.
pub(crate) fn report_ifml_diagnostics(
    sources: &[(String, String)],
    diagnostics: &[(String, phero_diagnostics::Diagnostic<&'static str>)],
) {
    render_grouped_with(
        diagnostics,
        |path| {
            sources
                .iter()
                .find(|(name, _)| name == path)
                .map(|(_, source)| source.as_str())
        },
        |diagnostic| match &diagnostic.code {
            Some(code) => format!("{code} {}", diagnostic.message),
            None => diagnostic.message.clone(),
        },
    );
}

/// Groups diagnostics by file path and renders each group against its
/// source; paths with no known source are skipped. Groups keep the
/// diagnostics' own order and file contiguity.
fn render_grouped<'s, C: Clone>(
    diagnostics: &[(String, phero_diagnostics::Diagnostic<C>)],
    source_of: impl Fn(&str) -> Option<&'s str>,
) {
    render_grouped_with(diagnostics, source_of, |diagnostic| {
        diagnostic.message.clone()
    });
}

/// [`render_grouped`] with a report-line decorator: the line rendered for a
/// record is `line(record)`, used by the `.ifml` reporter to join its
/// stable code onto the message without touching the stored record.
fn render_grouped_with<'s, C: Clone>(
    diagnostics: &[(String, phero_diagnostics::Diagnostic<C>)],
    source_of: impl Fn(&str) -> Option<&'s str>,
    line: impl Fn(&phero_diagnostics::Diagnostic<C>) -> String,
) {
    if diagnostics.is_empty() {
        return;
    }
    let mut groups: Vec<(&str, Vec<phero_diagnostics::Diagnostic<C>>)> = Vec::new();
    for (path, diagnostic) in diagnostics {
        let mut rendered = diagnostic.clone();
        rendered.message = line(diagnostic);
        match groups.last_mut() {
            Some((group_path, group)) if *group_path == path.as_str() => group.push(rendered),
            _ => groups.push((path.as_str(), vec![rendered])),
        }
    }
    for (path, group) in groups {
        let Some(source) = source_of(path) else {
            continue;
        };
        eprint!("{}", render(path, source, &group));
    }
}
