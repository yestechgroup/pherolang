//! `pherolang grammar`: write or check the generated editor grammar package.
//!
//! The command is the distribution step for [`phero_highlight::grammar`]: it
//! generates the ten TextMate grammars and language configurations from the
//! same metadata the highlighter uses, writes them into the canonical
//! `site/packages/highlighting` location (or any `--out` directory), and
//! `--check` proves the committed bytes are exactly what the generator
//! produces. It never reads model sources and never touches the network.

use std::path::PathBuf;
use std::process::ExitCode;

use phero_highlight::{check_grammars, GrammarProblemKind};

pub(crate) fn run(out: Option<PathBuf>, check: Option<PathBuf>) -> anyhow::Result<ExitCode> {
    match (out, check) {
        (Some(out), None) => {
            let written = phero_highlight::write_grammars(&out)?;
            println!(
                "wrote {} generated files to {}",
                written.len(),
                out.display()
            );
            Ok(ExitCode::SUCCESS)
        }
        (None, Some(check)) => match check_grammars(&check) {
            Ok(()) => {
                println!("grammar package is up to date: {}", check.display());
                Ok(ExitCode::SUCCESS)
            }
            Err(problems) => {
                for problem in &problems {
                    eprintln!("{problem}");
                }
                let stale = problems
                    .iter()
                    .filter(|problem| problem.kind == GrammarProblemKind::Stale)
                    .count();
                let missing = problems
                    .iter()
                    .filter(|problem| problem.kind == GrammarProblemKind::Missing)
                    .count();
                let unexpected = problems
                    .iter()
                    .filter(|problem| problem.kind == GrammarProblemKind::Unexpected)
                    .count();
                eprintln!(
                    "grammar check failed: {missing} missing, {stale} stale, \
                     {unexpected} unexpected"
                );
                Ok(ExitCode::FAILURE)
            }
        },
        _ => usage("pass exactly one of `--out <dir>` or `--check <dir>`"),
    }
}

fn usage(message: &str) -> anyhow::Result<ExitCode> {
    eprintln!("error: {message}");
    Ok(ExitCode::from(2))
}
