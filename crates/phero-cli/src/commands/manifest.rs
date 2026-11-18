//! `pherolang manifest <file.ifml>`: compile the interaction flow, gate on
//! the checker, and export the declaration-backed control/route manifest
//! (issue #118).

use std::path::PathBuf;
use std::process::ExitCode;

use crate::inputs::{is_ifml_file, read_ifml_inputs};
use crate::report::report_ifml_diagnostics;

use super::pipeline::emit_json;

pub(crate) fn run(files: Vec<PathBuf>, out: Option<PathBuf>) -> anyhow::Result<ExitCode> {
    let [file] = &files[..] else {
        anyhow::bail!("manifest takes exactly one .ifml interaction-flow file");
    };
    if !is_ifml_file(file) {
        anyhow::bail!(
            "manifest requires an .ifml interaction-flow file, got {}",
            file.display()
        );
    }
    let inputs = read_ifml_inputs(file, None)?;
    let compilation =
        match phero_ifml::compile_ifml_str(&inputs.path, &inputs.source, &inputs.ifml_imports) {
            Ok(compilation) => compilation,
            Err(diagnostics) => {
                report_ifml_diagnostics(&inputs.sources, &diagnostics);
                return Ok(ExitCode::FAILURE);
            }
        };
    // The full decisions-aware pass: domains + actors + decision services.
    // A `data:` naming a decision service (e.g. `AssessRefund` in
    // `support.dmn`) is only in scope through it — the actors-only check
    // reports it as an unknown domain entity (E0100, issue #283).
    let binding = phero_ifml::check_ifml_with_decisions(
        &compilation,
        inputs.domains.as_ref(),
        &inputs.actors,
        &inputs.decisions,
    );
    if !binding.is_empty() {
        let sources: Vec<(String, String)> = compilation
            .index
            .files
            .iter()
            .map(|file| (file.path.clone(), file.text.clone()))
            .collect();
        report_ifml_diagnostics(&sources, &binding);
        return Ok(ExitCode::FAILURE);
    }
    let manifest =
        phero_ifml::manifest_ifml(&compilation, &inputs.ifml_imports, inputs.domains.as_ref());
    emit_json(manifest.to_json_pretty()?, out.as_deref())?;
    Ok(ExitCode::SUCCESS)
}
