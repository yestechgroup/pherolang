//! `pherolang highlight`: compiler-backed syntax highlighting.
//!
//! The command is deliberately a compiler consumer, not a second lexer: it
//! reads each input, calls `phero_highlight::highlight`, and emits either
//! the stable JSON artifact or the semantic-class HTML fragment. Nothing
//! here resolves imports, checks a model, or touches the network.
//!
//! Inputs follow the same expansion rules as `check`/`build`: a directory
//! recursively contributes every supported surface. A single input may be
//! printed to stdout or written to one file; several inputs require an
//! output directory and are written one artifact per input, lexicographically
//! ordered.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::ValueEnum;

use phero_highlight::{
    highlight, to_html, HighlightArtifact, HighlightMode, HighlightOptions, Surface,
};

use crate::inputs::expand_check_inputs;

/// The output encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum Format {
    /// The versioned highlight artifact.
    Json,
    /// The semantic-class HTML fragment.
    Html,
}

/// The parse mode. The command defaults to `strict` for files and `lenient`
/// for stdin, so a docs fence can never fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum Mode {
    /// Parse and record diagnostics; still emits a lexical result on failure.
    Strict,
    /// Parse if possible, suppress diagnostics, and always emit a result.
    Lenient,
}

pub(crate) fn run(
    files: Vec<PathBuf>,
    format: Format,
    mode: Option<Mode>,
    surface: Option<String>,
    output: Option<PathBuf>,
) -> anyhow::Result<ExitCode> {
    if files.is_empty() {
        return usage("no input files (use `-` to read stdin)");
    }

    let forced = match surface.as_deref() {
        Some(id) => match Surface::from_id(id) {
            Some(surface) => Some(surface),
            None => {
                return usage(&format!(
                    "unknown surface `{id}`; expected one of: {}",
                    Surface::ALL
                        .iter()
                        .map(|surface| surface.id())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            }
        },
        None => None,
    };

    let stdin = files.iter().any(|file| file == Path::new("-"));
    if stdin {
        if files.len() != 1 {
            return usage("stdin (`-`) cannot be combined with other inputs");
        }
        let Some(surface) = forced else {
            return usage("stdin requires `--surface <id>`");
        };
        let source = read_stdin()?;
        emit_one(
            stdin_name(),
            &source,
            surface,
            mode.unwrap_or(Mode::Lenient),
            format,
            output.as_deref(),
        )?;
        return Ok(ExitCode::SUCCESS);
    }

    let expanded = if files.iter().any(|file| file.is_dir()) {
        if forced.is_some() {
            return usage("`--surface` cannot be combined with a directory input");
        }
        expand_check_inputs(&files)?
    } else {
        let mut expanded = files;
        expanded.sort();
        expanded
    };

    if forced.is_some() && expanded.len() != 1 {
        return usage("`--surface` is only valid for one input file");
    }

    if expanded.len() > 1 {
        let Some(dir) = output.as_deref() else {
            return usage("several inputs require `--output <dir>`");
        };
        if dir.exists() && !dir.is_dir() {
            return usage("`--output` must be a directory for several inputs");
        }
        std::fs::create_dir_all(dir)?;
    }

    if expanded.len() > 1 {
        let dir = output.as_deref().expect("validated output directory");
        for file in &expanded {
            let source = std::fs::read_to_string(file)
                .map_err(|error| anyhow::anyhow!("cannot read {}: {error}", file.display()))?;
            let surface = Surface::from_path(file).ok_or_else(|| {
                anyhow::anyhow!(
                    "cannot infer a Phero Lang surface from {}; pass `--surface <id>`",
                    file.display()
                )
            })?;
            let rendered = render(&source, surface, mode.unwrap_or(Mode::Strict), format)?;
            let path = output_path(dir, file, format);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, rendered)
                .map_err(|error| anyhow::anyhow!("cannot write {}: {error}", path.display()))?;
        }
        return Ok(ExitCode::SUCCESS);
    }

    let file = &expanded[0];
    let source = std::fs::read_to_string(file)
        .map_err(|error| anyhow::anyhow!("cannot read {}: {error}", file.display()))?;
    let surface = match forced {
        Some(surface) => surface,
        None => Surface::from_path(file).ok_or_else(|| {
            anyhow::anyhow!(
                "cannot infer a Phero Lang surface from {}; pass `--surface <id>`",
                file.display()
            )
        })?,
    };
    emit_one(
        file,
        &source,
        surface,
        mode.unwrap_or(Mode::Strict),
        format,
        output.as_deref(),
    )
    .map(|()| ExitCode::SUCCESS)
}

fn read_stdin() -> anyhow::Result<String> {
    let mut source = String::new();
    std::io::stdin()
        .read_to_string(&mut source)
        .map_err(|error| anyhow::anyhow!("cannot read stdin: {error}"))?;
    Ok(source)
}

fn stdin_name() -> &'static Path {
    Path::new("stdin")
}

fn emit_one(
    file: &Path,
    source: &str,
    surface: Surface,
    mode: Mode,
    format: Format,
    output: Option<&Path>,
) -> anyhow::Result<()> {
    let rendered = render(source, surface, mode, format)?;
    match output {
        Some(path) if path.is_dir() => {
            let path = output_path(path, file, format);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, rendered)
                .map_err(|error| anyhow::anyhow!("cannot write {}: {error}", path.display()))?;
        }
        Some(path) => {
            std::fs::write(path, rendered)
                .map_err(|error| anyhow::anyhow!("cannot write {}: {error}", path.display()))?;
        }
        None => println!("{rendered}"),
    }
    Ok(())
}

fn render(source: &str, surface: Surface, mode: Mode, format: Format) -> anyhow::Result<String> {
    let options = HighlightOptions {
        mode: match mode {
            Mode::Strict => HighlightMode::Strict,
            Mode::Lenient => HighlightMode::Lenient,
        },
        include_comments: true,
    };
    let result = highlight(source, surface, options);
    match format {
        Format::Json => HighlightArtifact::new(source, surface, result)
            .to_json_pretty()
            .map_err(|error| anyhow::anyhow!("cannot serialize highlight artifact: {error}")),
        Format::Html => Ok(to_html(source, &result.spans, surface)),
    }
}

/// `<out>/<input path as given>.<json|html>`, mirroring `build`'s artifact
/// path so an absolute input never swallows the output directory.
fn output_path(out: &Path, input: &Path, format: Format) -> PathBuf {
    let extension = match format {
        Format::Json => "json",
        Format::Html => "html",
    };
    let suffixed = format!("{}.{extension}", input.display());
    let mut path = out.to_path_buf();
    for component in Path::new(&suffixed).components() {
        match component {
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {}
            component => path.push(component.as_os_str()),
        }
    }
    path
}

fn usage(message: &str) -> anyhow::Result<ExitCode> {
    eprintln!("error: {message}");
    Ok(ExitCode::from(2))
}
