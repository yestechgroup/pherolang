//! `pherolang artifact check`: validate wire-format artifacts without the
//! originating model.
//!
//! With one or more `--domain <domain.json>` artifacts, a DDD design's
//! `domainFingerprint` is compared against the fingerprint computed over
//! those models (DF1, issue pherolang#324): a stale pair is refused before
//! generation.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::artifact_check;

pub(crate) fn check(files: Vec<PathBuf>, domain_files: Vec<PathBuf>) -> anyhow::Result<ExitCode> {
    if files.is_empty() {
        anyhow::bail!("no input files");
    }
    let expected = load_domain_fingerprint(&domain_files)?;
    let mut failed = false;
    for file in &files {
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("error: cannot read {}: {error}", file.display());
                failed = true;
                continue;
            }
        };
        match artifact_check::check_str(&file.display().to_string(), &text) {
            Ok(kind) => {
                if let Some(expected) = &expected {
                    if kind == artifact_check::ArtifactKind::DddModel {
                        match phero_ir::ddd::DddModel::from_json(&text) {
                            Ok(model) => {
                                if let Err(violation) =
                                    phero_backend_ddd::check_pairing(&model, expected)
                                {
                                    eprintln!("error: {}: {violation}", file.display());
                                    failed = true;
                                    continue;
                                }
                            }
                            Err(error) => {
                                // `check_str` already accepted the document,
                                // so this is a reader/checker divergence.
                                eprintln!("error: {}: {error}", file.display());
                                failed = true;
                                continue;
                            }
                        }
                    }
                }
                println!("OK {}", file.display());
            }
            Err(errors) => {
                for error in errors {
                    eprintln!("error: {error}");
                }
                failed = true;
            }
        }
    }
    if failed {
        Ok(ExitCode::FAILURE)
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

/// Reads the supplied domain artifacts and computes the fingerprint a
/// design must have been compiled against. No `--domain` files means no
/// pairing check (the artifact-only contract still requires the field).
fn load_domain_fingerprint(
    domain_files: &[PathBuf],
) -> anyhow::Result<Option<phero_ir::ddd::DomainFingerprint>> {
    if domain_files.is_empty() {
        return Ok(None);
    }
    let mut models = Vec::with_capacity(domain_files.len());
    for file in domain_files {
        models.push(read_domain_model(file)?);
    }
    Ok(Some(phero_driver::ddd::domain_fingerprint(&models)))
}

fn read_domain_model(file: &Path) -> anyhow::Result<phero_ir::Model> {
    let text = std::fs::read_to_string(file).map_err(|error| {
        anyhow::anyhow!("cannot read domain artifact {}: {error}", file.display())
    })?;
    phero_ir::Model::from_json(&text)
        .map_err(|error| anyhow::anyhow!("{} is not a domain artifact: {error}", file.display()))
}
