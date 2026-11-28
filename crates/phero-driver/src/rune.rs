//! The standalone `.rosetta` compile unit (RN10, ADR 0012 D7).
//!
//! A `.rosetta` file compiles together with a **pool** of other Rune files,
//! because Rune namespaces import each other by name, not by path. The host
//! gathers the pool. The CLI uses
//! [`crate::workspace_imports::rosetta_candidates`], the same pool an
//! `import sigil` reads. The driver stays filesystem-free and receives texts
//! only.
//!
//! [`compile_rune`] selects the files it resolves the way `import sigil`
//! does: the named files, then every pool file whose namespace the selection
//! imports, transitively (`sigil.rs`, the module contract). With
//! [`RuneSelection::Exact`], the given files are resolved as they are, which
//! is the old `sigil model <files>` behaviour.
//!
//! # The artifact is `sigil-model/1`, a documented carve-out
//!
//! The model is `phero_rune_resolve::canonical_json`, byte-for-byte the JSON
//! the Rune conformance goldens, the CDM snapshot and the Java oracle
//! comparison pin. It does not follow the `phero-ir` wire rules: there is no
//! `formatVersion` gate, its keys are sigil's, and `pherolang artifact check`
//! does not classify it. A `phero_ir` Rune wire root is a follow-up decision
//! (RN10).

use std::collections::HashSet;

use phero_syntax::Span;

use crate::diagnostic::Diagnostic;
use crate::sigil::{parse_candidate, select, Candidate};

/// Which pool files a [`compile_rune`] call resolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuneSelection {
    /// The named files plus the namespace closure they import, the
    /// `import sigil` rule.
    Closure,
    /// Every pool file, as given.
    Exact,
}

/// The result of one [`compile_rune`] call.
#[derive(Debug, Clone)]
pub struct RuneCompilation {
    /// The `sigil-model/1` model JSON, or `None` when a resolved file has a
    /// syntax error (a broken file is never lowered, the shim's rule).
    pub model: Option<serde_json::Value>,
    /// Every diagnostic, keyed by the pool key of the file it belongs to.
    pub diagnostics: Vec<(String, Diagnostic)>,
}

impl RuneCompilation {
    /// `true` when any diagnostic is an error.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|(_, diagnostic)| diagnostic.is_error())
    }

    /// The diagnostics of one file.
    pub fn diagnostics_of<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a Diagnostic> {
        self.diagnostics
            .iter()
            .filter(move |(file, _)| file == key)
            .map(|(_, diagnostic)| diagnostic)
    }
}

/// Parses, lowers and resolves a Rune pool. `pool` is `(key, text)` pairs,
/// and `named` are the keys being compiled; every named key must be in the
/// pool, and a missing one is ignored.
#[must_use]
pub fn compile_rune(
    named: &[&str],
    pool: &[(String, String)],
    selection: RuneSelection,
) -> RuneCompilation {
    let mut seen: HashSet<&str> = HashSet::new();
    let candidates: Vec<Candidate> = pool
        .iter()
        .filter(|(key, _)| seen.insert(key.as_str()))
        .map(|(key, text)| parse_candidate("", key, text))
        .collect();

    let mut selected: Vec<usize> = Vec::new();
    let mut frontier: Vec<String> = Vec::new();
    let mut seen_files: HashSet<(String, String)> = HashSet::new();
    match selection {
        RuneSelection::Exact => {
            for index in 0..candidates.len() {
                select(
                    index,
                    &candidates,
                    &mut frontier,
                    &mut selected,
                    &mut seen_files,
                );
            }
        }
        RuneSelection::Closure => {
            for key in named {
                if let Some(index) = candidates.iter().position(|c| c.key == *key) {
                    select(
                        index,
                        &candidates,
                        &mut frontier,
                        &mut selected,
                        &mut seen_files,
                    );
                }
            }
            let mut changed = true;
            while changed {
                changed = false;
                for index in 0..candidates.len() {
                    if selected.contains(&index) {
                        continue;
                    }
                    let Some(file) = &candidates[index].file else {
                        continue;
                    };
                    if frontier.contains(&file.namespace)
                        && select(
                            index,
                            &candidates,
                            &mut frontier,
                            &mut selected,
                            &mut seen_files,
                        )
                    {
                        changed = true;
                    }
                }
            }
        }
    }

    // Syntax diagnostics: every named file's (it is what was asked for, and
    // it may not have parsed), then every other resolved file's.
    let mut diagnostics: Vec<(String, Diagnostic)> = Vec::new();
    let mut broken = false;
    let mut reported: HashSet<usize> = HashSet::new();
    let named_indices = named
        .iter()
        .filter_map(|key| candidates.iter().position(|c| c.key == *key));
    for index in named_indices.chain(selected.iter().copied()) {
        if !reported.insert(index) {
            continue;
        }
        let candidate = &candidates[index];
        if candidate.file.is_none() {
            broken = true;
        }
        diagnostics.extend(candidate.diagnostics.iter().cloned());
    }
    // A named file that has no parse is broken whatever its diagnostics say,
    // and so is every candidate the exact selection was asked to resolve.
    if selection == RuneSelection::Exact && candidates.iter().any(|c| c.file.is_none()) {
        broken = true;
    }
    if broken {
        return RuneCompilation {
            model: None,
            diagnostics,
        };
    }

    let files = selected
        .iter()
        .filter_map(|&index| candidates[index].file.clone())
        .collect();
    let resolution = phero_rune_resolve::resolve(files);
    for diagnostic in &resolution.diagnostics {
        let span = diagnostic.span.map(|span| Span::from(span.start..span.end));
        let message = format!("{} {}", diagnostic.code, diagnostic.message);
        let mapped = match diagnostic.severity {
            sigil_diag::Severity::Error => Diagnostic::error(message, span),
            sigil_diag::Severity::Warning => Diagnostic::warning(message, span),
        }
        .with_help(format!("at {}", diagnostic.path));
        diagnostics.push((diagnostic.file.clone(), mapped));
    }
    let sources: Vec<(String, String)> = selected
        .iter()
        .map(|&index| {
            (
                candidates[index].key.clone(),
                candidates[index].text.clone(),
            )
        })
        .collect();
    let model = phero_rune_resolve::canonical_json(&resolution, &sources);
    RuneCompilation {
        model: Some(model),
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(files: &[(&str, &str)]) -> Vec<(String, String)> {
        files
            .iter()
            .map(|(key, text)| (key.to_string(), text.to_string()))
            .collect()
    }

    fn file_names(model: &serde_json::Value) -> Vec<String> {
        model["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| file["name"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn closure_selects_the_imported_namespaces_only() {
        let pool = pool(&[
            (
                "main.rosetta",
                "namespace a\nimport b.*\ntype A:\n\tb B (1..1)\n",
            ),
            ("dep.rosetta", "namespace b\ntype B:\n\tx int (1..1)\n"),
            ("other.rosetta", "namespace c\ntype C:\n\tx int (1..1)\n"),
        ]);
        let compilation = compile_rune(&["main.rosetta"], &pool, RuneSelection::Closure);
        assert!(!compilation.has_errors(), "{:?}", compilation.diagnostics);
        let names = file_names(compilation.model.as_ref().unwrap());
        assert!(names.contains(&"main.rosetta".to_string()), "{names:?}");
        assert!(names.contains(&"dep.rosetta".to_string()), "{names:?}");
        assert!(!names.contains(&"other.rosetta".to_string()), "{names:?}");
        assert_eq!(
            compilation.model.as_ref().unwrap()["format"],
            "sigil-model/1"
        );
    }

    #[test]
    fn exact_resolves_every_given_file() {
        let pool = pool(&[
            ("one.rosetta", "namespace a\ntype A:\n\tx int (1..1)\n"),
            ("two.rosetta", "namespace c\ntype C:\n\tx int (1..1)\n"),
        ]);
        let compilation = compile_rune(&[], &pool, RuneSelection::Exact);
        let names = file_names(compilation.model.as_ref().unwrap());
        assert!(names.contains(&"one.rosetta".to_string()));
        assert!(names.contains(&"two.rosetta".to_string()));
    }

    #[test]
    fn a_syntax_error_has_no_model_and_names_the_file() {
        let pool = pool(&[("bad.rosetta", "namespace a\ntype Broken\n")]);
        let compilation = compile_rune(&["bad.rosetta"], &pool, RuneSelection::Closure);
        assert!(compilation.model.is_none());
        assert!(compilation.has_errors());
        assert!(compilation.diagnostics_of("bad.rosetta").next().is_some());
    }

    #[test]
    fn an_unknown_type_is_a_resolution_error_with_a_model() {
        let pool = pool(&[("a.rosetta", "namespace a\ntype A:\n\tx Missing (1..1)\n")]);
        let compilation = compile_rune(&["a.rosetta"], &pool, RuneSelection::Closure);
        assert!(compilation.model.is_some());
        let error = compilation.diagnostics_of("a.rosetta").next().unwrap();
        assert!(error.is_error());
        assert!(
            error
                .message
                .starts_with(phero_diagnostics::E_RUNE_UNKNOWN_SYMBOL),
            "{}",
            error.message
        );
    }
}
