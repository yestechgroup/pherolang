//! Shared helpers for the highlight integration tests: the repository root,
//! the span invariants every corpus case must hold, and the docs-fence
//! collector that mirrors the site's Expressive Code normalization.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use phero_highlight::{HighlightSpan, Surface};

/// The repository root, resolved from this crate's manifest directory.
pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root")
}

/// Every span invariant the engine promises: ordered, non-overlapping, in
/// bounds, non-empty, and on UTF-8 boundaries.
pub fn assert_invariants(source: &str, spans: &[HighlightSpan]) {
    let mut previous_end = 0;
    for span in spans {
        assert!(span.start < span.end, "empty span in {source:?}");
        assert!(span.end <= source.len(), "out-of-bounds span");
        assert!(
            span.start >= previous_end,
            "overlapping or unsorted spans at {}..{}",
            span.start,
            span.end
        );
        assert!(source.is_char_boundary(span.start));
        assert!(source.is_char_boundary(span.end));
        previous_end = span.end;
    }
}

/// The nine `examples/support/support.*` files, in path order.
pub fn example_files() -> Vec<(Surface, PathBuf)> {
    let directory = repository_root().join("examples/support");
    let mut files = Vec::new();
    for entry in fs::read_dir(&directory).expect("examples/support") {
        let path = entry.expect("directory entry").path();
        if !path.is_file() {
            continue;
        }
        if let Some(surface) = Surface::from_path(&path) {
            files.push((surface, path));
        }
    }
    files.sort_by(|left, right| left.1.cmp(&right.1));
    files
}

/// One Phero Lang fenced block from `docs/**/*.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocFence {
    /// Repository-relative path, POSIX separators.
    pub file: String,
    /// 1-based line of the opening fence.
    pub line: usize,
    /// The surface the fence declares.
    pub surface: Surface,
    /// Expressive Code's plaintext: right-trimmed lines, no leading/trailing
    /// blank lines, joined with `\n`.
    pub source: String,
}

/// Expressive Code's plaintext normalization, reproduced exactly.
pub fn normalize_ec_code(raw: &str) -> String {
    let mut lines: Vec<&str> = raw
        .split('\n')
        .map(|line| line.trim_end_matches([' ', '\t', '\r']))
        .collect();
    while lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn fence_info(info: &str) -> &str {
    let first = info.split_whitespace().next().unwrap_or("");
    first
        .trim_start_matches('{')
        .trim_start_matches('.')
        .trim_end_matches('}')
        .trim_start_matches('.')
        .trim_end_matches('.')
}

fn walk_markdown(directory: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(directory)
        .expect("docs directory")
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            walk_markdown(&path, out);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("md") {
            out.push(path);
        }
    }
}

/// Every Phero Lang fence in `docs/**/*.md`, in file/line order.
///
/// The parser mirrors `site/scripts/highlighting/sources.mjs`: fences start at
/// column zero, open with three or more backticks or tildes, and close with a
/// run of the same marker that is at least as long. The content is normalized
/// exactly as Expressive Code renders it, because that is the byte sequence
/// the site hashes.
pub fn documentation_fences() -> Vec<DocFence> {
    let root = repository_root();
    let mut files = Vec::new();
    walk_markdown(&root.join("docs"), &mut files);

    let mut fences = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).expect("read markdown");
        let relative = path
            .strip_prefix(&root)
            .expect("docs file under the repository root")
            .to_string_lossy()
            .replace('\\', "/");

        struct OpenFence<'text> {
            marker: char,
            length: usize,
            line: usize,
            surface: Option<Surface>,
            content: Vec<&'text str>,
        }

        let mut open: Option<OpenFence<'_>> = None;
        for (index, line) in text.split('\n').enumerate() {
            if let Some(current) = open.as_mut() {
                let trimmed = line.trim_start();
                let closing = trimmed.starts_with(current.marker)
                    && trimmed.chars().take_while(|c| *c == current.marker).count()
                        >= current.length;
                if closing && trimmed.trim_end_matches(current.marker).trim().is_empty() {
                    if let Some(surface) = current.surface {
                        fences.push(DocFence {
                            file: relative.clone(),
                            line: current.line,
                            surface,
                            source: normalize_ec_code(&current.content.join("\n")),
                        });
                    }
                    open = None;
                } else {
                    current.content.push(line);
                }
                continue;
            }

            let trimmed = line.trim_start();
            let marker = trimmed.chars().next();
            if !matches!(marker, Some('`') | Some('~')) {
                continue;
            }
            let marker = marker.expect("marker");
            let length = trimmed.chars().take_while(|c| *c == marker).count();
            if length < 3 {
                continue;
            }
            let info = &trimmed[length..];
            let surface = Surface::ALL
                .into_iter()
                .find(|surface| surface.id() == fence_info(info));
            open = Some(OpenFence {
                marker,
                length,
                line: index + 1,
                surface,
                content: Vec::new(),
            });
        }
        assert!(
            open.is_none(),
            "{relative}: unterminated code fence opened on line {}",
            open.map(|fence| fence.line).unwrap_or_default()
        );
    }

    fences.sort_by(|left, right| left.file.cmp(&right.file).then(left.line.cmp(&right.line)));
    fences
}
