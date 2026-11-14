//! `pherolang lsp`: serve the language server over stdin/stdout.

use std::process::ExitCode;

pub(crate) fn run() -> anyhow::Result<ExitCode> {
    phero_lsp::run_stdio()?;
    Ok(ExitCode::SUCCESS)
}
