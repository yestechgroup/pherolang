//! Conformance tests for the `examples/` interaction flows: every
//! `examples/<app>/<name>.ifml` listed in [`EXAMPLES`] must exist, compile with
//! zero diagnostics when its imports resolve from disk exactly the way the
//! CLI resolves them, type-check against the union of its imported `.mox`
//! domains and the decision services of its imported `.dmn` models, and be
//! byte-identical to its own canonical formatting.

use std::fs;
use std::path::{Path, PathBuf};

use phero_driver::{compile_str, DomainImports};
use phero_ifml::{
    check_ifml_with_decisions, compile_ifml_str, Diagnostic, IfmlCompilation, IfmlImports,
};

/// The interaction-flow examples as `(name, app-relative stem)`, hardcoded
/// so a missing file fails loudly (the house convention for the `examples/`
/// harnesses). The name keys the golden manifest under
/// `examples/artifacts/`; the stem is the path under `examples/` — one
/// directory per application since pherolang#258.
pub const EXAMPLES: [(&str, &str); 3] = [
    ("cli", "support/cli"),
    ("ecommerce", "shop/ecommerce"),
    ("support", "support/support"),
];

const EXAMPLES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");

/// One example, ready for compilation: the main source, the `.ifml` import
/// bundle, every imported `.mox` domain as `(resolved path, source)`, and
/// every imported `.dmn` model as gathered
/// [`phero_driver::project::DecisionImport`]s — the raw material the
/// decision dimension compiles.
struct Example {
    path: String,
    source: String,
    imports: IfmlImports,
    domains: Vec<(String, String)>,
    decision_imports: Vec<phero_driver::project::DecisionImport>,
}

/// Reads `examples/<stem>.ifml` plus everything it imports through
/// [`phero_ifml::walk_ifml_imports`], mirroring the CLI's collection: the
/// fetch closure resolves each import relative to the importing file's
/// directory on disk, provides `.ifml` content under the walker's
/// `(importer identity, import-as-written)` keys, and collects `.mox`
/// domains (deduplicated by resolved path) plus `.dmn` decision models —
/// each with its own `.mox` imports and decision-table contents fetched
/// under the model's resolved path — as side effects. Disk paths are
/// tracked per identity here — the walker only sees identities.
fn load(stem: &str) -> Example {
    let source_path = Path::new(EXAMPLES_DIR).join(format!("{stem}.ifml"));
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("example {stem}.ifml must exist under examples/: {error}"));
    let mut example = Example {
        path: source_path.display().to_string(),
        source,
        imports: IfmlImports::default(),
        domains: Vec::new(),
        decision_imports: Vec::new(),
    };
    let mut disk_paths = std::collections::BTreeMap::new();
    disk_paths.insert(example.path.clone(), source_path);
    let dir_of = |path: &Path| -> PathBuf {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    };
    let walk =
        phero_ifml::walk_ifml_imports(&example.path, &example.source, &mut |importer, import| {
            let importer_disk = disk_paths.get(importer)?;
            let resolved = dir_of(importer_disk).join(import);
            if import.ends_with(".ifml") {
                let imported = fs::read_to_string(&resolved).unwrap_or_else(|error| {
                    panic!("read imported file {}: {error}", resolved.display())
                });
                let resolved_path = resolved.display().to_string();
                disk_paths.insert(import.to_string(), resolved);
                example.imports.provide(importer, import, imported.clone());
                Some((resolved_path, imported))
            } else if import.ends_with(".mox") {
                let resolved_path = resolved.display().to_string();
                if !example
                    .domains
                    .iter()
                    .any(|(existing, _)| existing == &resolved_path)
                {
                    let domain = fs::read_to_string(&resolved).unwrap_or_else(|error| {
                        panic!("read imported file {}: {error}", resolved.display())
                    });
                    example.domains.push((resolved_path, domain));
                }
                None
            } else if import.ends_with(".dmn") {
                let resolved_path = resolved.display().to_string();
                if !example
                    .decision_imports
                    .iter()
                    .any(|existing| existing.key == resolved_path)
                {
                    let model = fs::read_to_string(&resolved).unwrap_or_else(|error| {
                        panic!("read imported file {}: {error}", resolved.display())
                    });
                    // The model's own `.mox` imports and decision-table
                    // contents resolve relative to the model's resolved
                    // path — the CLI's gather discipline.
                    let mut decision = phero_driver::project::DecisionImport {
                        key: resolved_path.clone(),
                        source: model.clone(),
                        domains: Vec::new(),
                        tables: Vec::new(),
                        events: Vec::new(),
                    };
                    let imports: Vec<String> = phero_syntax::parse_dmn(&model)
                        .ast
                        .map(|ast| {
                            ast.imports
                                .iter()
                                .map(|import| import.path.clone())
                                .collect::<Vec<String>>()
                        })
                        .unwrap_or_default();
                    for domain_import in imports.clone() {
                        let domain_resolved = dir_of(&resolved).join(&domain_import);
                        let domain_key = domain_resolved.display().to_string();
                        if decision
                            .domains
                            .iter()
                            .any(|(existing, _)| existing == &domain_key)
                        {
                            continue;
                        }
                        let domain = fs::read_to_string(&domain_resolved).unwrap_or_else(|error| {
                            panic!(
                                "read imported file {} (imported by {}): {error}",
                                domain_resolved.display(),
                                resolved.display()
                            )
                        });
                        decision.domains.push((domain_key, domain));
                    }
                    let tables = phero_syntax::parse_dmn(&model)
                        .ast
                        .and_then(|ast| ast.model)
                        .map(|model| {
                            model
                                .decisions
                                .iter()
                                .filter_map(|decision| match &decision.logic {
                                    Some(phero_syntax::DmnLogic::Table { source, .. }) => {
                                        Some(source.clone())
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<String>>()
                        })
                        .unwrap_or_default();
                    for table in tables {
                        if decision
                            .tables
                            .iter()
                            .any(|(existing, _)| *existing == table)
                        {
                            continue;
                        }
                        if let Ok(content) = fs::read_to_string(dir_of(&resolved).join(&table)) {
                            decision.tables.push((table, content));
                        }
                    }
                    // The model's `.evt` imports compile into the events
                    // bucket (the emits dimension), keyed by the import
                    // string as written.
                    for import in imports {
                        if !import.ends_with(".evt")
                            || decision
                                .events
                                .iter()
                                .any(|(existing, _)| *existing == import)
                        {
                            continue;
                        }
                        let event_resolved = dir_of(&resolved).join(&import);
                        let event_compilation = phero_driver::compile_evt_str(
                            event_resolved.to_str().expect("utf-8 event path"),
                            &fs::read_to_string(&event_resolved).unwrap_or_else(|error| {
                                panic!(
                                    "read imported file {} (imported by {}): {error}",
                                    event_resolved.display(),
                                    resolved.display()
                                )
                            }),
                            &phero_driver::CompileInputs::from_domains(&decision.domains),
                        );
                        if let Some(model) = event_compilation.model {
                            decision.events.push((import, model));
                        }
                    }
                    example.decision_imports.push(decision);
                }
                None
            } else {
                None
            }
        });
    assert_eq!(walk.sources[0].0, example.path);
    example
}

fn render(diagnostics: &[(String, Diagnostic<&'static str>)]) -> String {
    diagnostics
        .iter()
        .map(|(file, diagnostic)| {
            let span = diagnostic.span.expect("a diagnostic carries a span");
            format!(
                "  {} in {file} at {}..{}: {}",
                diagnostic.code.unwrap_or("<none>"),
                span.start,
                span.end,
                diagnostic.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn compile(stem: &str) -> IfmlCompilation {
    let example = load(stem);
    match compile_ifml_str(&example.path, &example.source, &example.imports) {
        Ok(compilation) => compilation,
        Err(diagnostics) => panic!(
            "{stem}.ifml must compile with zero diagnostics:\n{}",
            render(&diagnostics)
        ),
    }
}

/// Compiles every imported `.mox` domain to Core IR and unions the
/// packages — the model [`check_ifml_with_decisions`] binds expressions
/// against.
fn domain_union(stem: &str) -> phero_ir::Model {
    let example = load(stem);
    assert!(
        !example.domains.is_empty(),
        "{stem}.ifml must import at least one .mox domain"
    );
    let mut union = phero_ir::Model::new();
    for (path, source) in &example.domains {
        let compilation = compile_str(path, source, &DomainImports::default());
        assert!(
            compilation.diagnostics.is_empty() && compilation.sigil_diagnostics.is_empty(),
            "imported domain {path} must compile cleanly:\n{:+?}",
            compilation.diagnostics
        );
        union.packages.extend(
            compilation
                .model
                .expect("a clean compile lowers a model")
                .packages,
        );
    }
    union
}

/// Compiles every imported `.dmn` model — the decision dimension
/// [`check_ifml_with_decisions`] resolves `data:` names against. A model
/// that fails to compile (its types must resolve against its own gathered
/// `.mox` imports; its tables must ingest) fails the harness loudly.
fn decisions(stem: &str) -> Vec<phero_ir::dmn::DmnModel> {
    let example = load(stem);
    phero_driver::project::compile_decision_imports(&example.decision_imports).unwrap_or_else(
        |message| panic!("{stem}.ifml's imported decision models must compile: {message}"),
    )
}

#[test]
fn every_example_compiles_and_resolves_its_module_uses() {
    for (name, stem) in &EXAMPLES {
        let compilation = compile(stem);
        assert!(
            !compilation.model.views.is_empty() || !compilation.model.commands.is_empty(),
            "{name}.ifml must declare at least one view or command"
        );
        // Web-flavored examples compose the pattern library; a CLI example
        // binds commands straight to actions and may carry no uses.
        if !compilation.model.views.is_empty() {
            assert!(
                !compilation.index.module_uses.is_empty(),
                "{name}.ifml must compose pattern-library modules"
            );
        }
        for use_site in compilation
            .index
            .module_uses
            .iter()
            .filter(|site| site.file == 0)
        {
            assert!(
                use_site.resolved_module.is_some(),
                "{name}.ifml: use '{}' must resolve to a module",
                use_site.target
            );
        }
    }
}

#[test]
fn every_example_type_checks_against_its_domain_union() {
    for (name, stem) in &EXAMPLES {
        let compilation = compile(stem);
        let union = domain_union(stem);
        let binding = check_ifml_with_decisions(&compilation, Some(&union), &[], &decisions(stem));
        assert!(
            binding.is_empty(),
            "{name}.ifml must type-check against its domains:\n{}",
            render(&binding)
        );
    }
}

/// The example's control/route manifest must byte-match its committed
/// golden artifact under `examples/artifacts/` (the `examples_artifacts`
/// precedent for the driver surfaces) and round-trip through
/// `from_json`. The manifest is derived the way `pherolang manifest` does:
/// compile, check (clean), then export with the example's imports and
/// its domain union.
///
/// Regenerate with `PHERO_UPDATE_FIXTURES=1 cargo test -p phero-ifml --test examples`.
#[test]
fn every_example_manifest_matches_its_golden_artifact() {
    for (name, stem) in &EXAMPLES {
        let example = load(stem);
        let compilation = compile(stem);
        let binding = check_ifml_with_decisions(
            &compilation,
            Some(&domain_union(stem)),
            &[],
            &decisions(stem),
        );
        assert!(
            binding.is_empty(),
            "{name}.ifml must check clean before manifesting:\n{}",
            render(&binding)
        );
        let union = domain_union(stem);
        let manifest = phero_ifml::manifest_ifml(&compilation, &example.imports, Some(&union));
        let json = manifest.to_json_pretty().expect("serialize manifest");

        let artifact = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/artifacts")
            .join(format!("{name}.manifest.json"));
        if std::env::var("PHERO_UPDATE_FIXTURES").is_ok() {
            std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
            std::fs::write(&artifact, &json).unwrap();
            eprintln!("updated {}", artifact.display());
            continue;
        }
        let expected = std::fs::read_to_string(&artifact).unwrap_or_else(|error| {
            panic!(
                "missing golden manifest {}; regenerate with PHERO_UPDATE_FIXTURES=1: {error}",
                artifact.display()
            )
        });
        assert_eq!(
            json, expected,
            "{name}.ifml's manifest drifted from the golden file"
        );

        let parsed = phero_ifml::IfmlManifest::from_json(&expected)
            .unwrap_or_else(|error| panic!("golden manifest {name}: {error}"));
        assert_eq!(
            parsed.to_json_pretty().expect("re-serialize"),
            expected,
            "{name} manifest round trip must be stable"
        );
    }
}

#[test]
fn every_example_is_fmt_canonical() {
    for (name, stem) in &EXAMPLES {
        let example = load(stem);
        assert_eq!(
            phero_ifml::format_ifml(&example.source),
            example.source,
            "{name}.ifml must already be in canonical form"
        );
    }
}

#[test]
fn the_example_list_covers_every_ifml_file_on_disk() {
    fn walk(dir: &Path, base: &Path, found: &mut Vec<String>) {
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .expect("readable examples directory")
            .map(|entry| entry.expect("readable entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if base == Path::new("") && (name == "artifacts" || name == "instances") {
                    continue;
                }
                walk(&path, &base.join(name.as_ref()), found);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "ifml")
            {
                let stem = path.file_stem().expect("named file").to_string_lossy();
                found.push(base.join(stem.as_ref()).to_string_lossy().into_owned());
            }
        }
    }
    let mut on_disk = Vec::new();
    walk(Path::new(EXAMPLES_DIR), Path::new(""), &mut on_disk);
    on_disk.sort();
    let mut listed: Vec<String> = EXAMPLES
        .iter()
        .map(|(_, stem)| (*stem).to_string())
        .collect();
    listed.sort();
    assert_eq!(
        listed, on_disk,
        "examples/**/*.ifml and EXAMPLES must stay in sync"
    );
}
