//! Differential parity between the Pest parser (still live in phase 1) and
//! the chumsky front half. Valid inputs must produce byte-identical models,
//! including `PropertyAssignment::span`; index sites are compared
//! structurally and their spans are checked by slicing the source.

use std::fs;
use std::path::{Path, PathBuf};

use rex_ifml::{
    manifest_ifml, parse_ifml_indexed, parse_ifml_v2_with_path, IfmlCompilation, IfmlImports,
    IfmlIndex,
};

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn corpus() -> Vec<(String, String)> {
    let mut files = vec![
        "tests/conformance/ifml/app.ifml".to_string(),
        "tests/conformance/ifml/cli.ifml".to_string(),
        "examples/support/cli.ifml".to_string(),
        "examples/shop/ecommerce.ifml".to_string(),
        "examples/support/support.ifml".to_string(),
    ];
    let patterns = repo_path("patterns/ifml");
    for entry in fs::read_dir(patterns).expect("patterns/ifml exists") {
        let path = entry.expect("readable entry").path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("ifml") {
            files.push(
                path.strip_prefix(repo_path(""))
                    .expect("inside repo")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    files.sort();
    files
        .into_iter()
        .map(|relative| {
            let text = fs::read_to_string(repo_path(&relative))
                .unwrap_or_else(|error| panic!("read {relative}: {error}"));
            (relative, text)
        })
        .collect()
}

/// Full model equality, spans included.
#[test]
fn v2_reproduces_every_corpus_model() {
    for (path, text) in corpus() {
        let pest = parse_ifml_indexed(&text).unwrap_or_else(|error| {
            panic!("pest parses {path}: {error}");
        });
        let v2 = parse_ifml_v2_with_path("", &text)
            .unwrap_or_else(|error| panic!("v2 parses {path}: {error}"));
        assert_eq!(v2.0, pest.0, "model parity for {path}");
    }
}

/// The index must agree structurally, and every span must slice to the same
/// source text modulo Pest's trailing inter-token whitespace.
#[test]
fn v2_reproduces_every_corpus_index() {
    for (path, text) in corpus() {
        let (_, pest) = parse_ifml_indexed(&text).unwrap();
        let (_, v2) = parse_ifml_v2_with_path("", &text).unwrap();
        assert_eq!(
            structural(&v2),
            structural(&pest),
            "index structure for {path}"
        );
        assert_spans(&text, &v2, &pest, &path);
    }
}

type ParamShape = (String, String, bool);
type OverrideShape = (String, Option<String>);
type ModuleDeclShape = (String, Vec<ParamShape>, Vec<String>);
type ModuleUseShape = (String, Option<String>, Vec<OverrideShape>);

/// The index without spans: names, targets, aliases, order, literal kinds.
#[derive(Debug, PartialEq)]
struct Structural {
    module_decls: Vec<ModuleDeclShape>,
    module_uses: Vec<ModuleUseShape>,
    action_targets: Vec<(String, String)>,
    auth_sites: Vec<(String, String)>,
    data_sites: Vec<String>,
    views: Vec<String>,
    actions: Vec<String>,
    actors: Vec<String>,
    components: Vec<String>,
    containers: Vec<String>,
    commands: Vec<String>,
}

fn structural(index: &IfmlIndex) -> Structural {
    Structural {
        module_decls: index
            .module_decls
            .iter()
            .map(|site| {
                (
                    site.name.clone(),
                    site.input_params
                        .iter()
                        .map(|param| {
                            (
                                param.name.clone(),
                                param.type_ref.clone(),
                                param.has_default,
                            )
                        })
                        .collect(),
                    site.properties.clone(),
                )
            })
            .collect(),
        module_uses: index
            .module_uses
            .iter()
            .map(|site| {
                (
                    site.target.clone(),
                    site.alias.clone(),
                    site.overrides
                        .iter()
                        .map(|over| {
                            (
                                over.name.clone(),
                                over.literal.map(|literal| format!("{literal:?}")),
                            )
                        })
                        .collect(),
                )
            })
            .collect(),
        action_targets: index
            .action_targets
            .iter()
            .map(|site| (site.target.clone(), site.action.clone()))
            .collect(),
        auth_sites: index
            .auth_sites
            .iter()
            .map(|site| (format!("{:?}", site.kind), site.name.clone()))
            .collect(),
        data_sites: index
            .data_sites
            .iter()
            .map(|site| site.name.clone())
            .collect(),
        views: index.views.iter().map(|site| site.name.clone()).collect(),
        actions: index.actions.iter().map(|site| site.name.clone()).collect(),
        actors: index.actors.iter().map(|site| site.name.clone()).collect(),
        components: index
            .components
            .iter()
            .map(|site| site.name.clone())
            .collect(),
        containers: index
            .containers
            .iter()
            .map(|site| site.name.clone())
            .collect(),
        commands: index
            .commands
            .iter()
            .map(|site| site.name.clone())
            .collect(),
    }
}

/// Assert the v2 span slices the same visible text as Pest. Pest's
/// non-atomic `parameter_decl` (and declaration-ending rules) absorbed
/// trailing whitespace; the exact v2 span is allowed to be tighter, but not
/// to point at different text.
fn assert_spans(text: &str, v2: &IfmlIndex, pest: &IfmlIndex, path: &str) {
    for (label, v2_spans, pest_spans) in [
        (
            "module_decls",
            v2.module_decls
                .iter()
                .map(|site| site.span)
                .collect::<Vec<_>>(),
            pest.module_decls
                .iter()
                .map(|site| site.span)
                .collect::<Vec<_>>(),
        ),
        (
            "module_uses",
            v2.module_uses
                .iter()
                .map(|site| site.span)
                .collect::<Vec<_>>(),
            pest.module_uses
                .iter()
                .map(|site| site.span)
                .collect::<Vec<_>>(),
        ),
        (
            "views",
            v2.views.iter().map(|site| site.span).collect::<Vec<_>>(),
            pest.views.iter().map(|site| site.span).collect::<Vec<_>>(),
        ),
        (
            "components",
            v2.components
                .iter()
                .map(|site| site.span)
                .collect::<Vec<_>>(),
            pest.components
                .iter()
                .map(|site| site.span)
                .collect::<Vec<_>>(),
        ),
        (
            "containers",
            v2.containers
                .iter()
                .map(|site| site.span)
                .collect::<Vec<_>>(),
            pest.containers
                .iter()
                .map(|site| site.span)
                .collect::<Vec<_>>(),
        ),
        (
            "commands",
            v2.commands.iter().map(|site| site.span).collect::<Vec<_>>(),
            pest.commands
                .iter()
                .map(|site| site.span)
                .collect::<Vec<_>>(),
        ),
    ] {
        assert_eq!(v2_spans.len(), pest_spans.len(), "{label} count in {path}");
        for (v2_span, pest_span) in v2_spans.into_iter().zip(pest_spans) {
            let v2_text = text[v2_span.0..v2_span.1].trim_end();
            let pest_text = text[pest_span.0..pest_span.1].trim_end();
            assert_eq!(v2_text, pest_text, "{label} span in {path}");
        }
    }

    for (label, v2_spans, pest_spans) in [
        (
            "action_targets",
            v2.action_targets
                .iter()
                .map(|site| site.name_span)
                .collect::<Vec<_>>(),
            pest.action_targets
                .iter()
                .map(|site| site.name_span)
                .collect::<Vec<_>>(),
        ),
        (
            "auth_sites",
            v2.auth_sites
                .iter()
                .map(|site| site.name_span)
                .collect::<Vec<_>>(),
            pest.auth_sites
                .iter()
                .map(|site| site.name_span)
                .collect::<Vec<_>>(),
        ),
        (
            "data_sites",
            v2.data_sites
                .iter()
                .map(|site| site.name_span)
                .collect::<Vec<_>>(),
            pest.data_sites
                .iter()
                .map(|site| site.name_span)
                .collect::<Vec<_>>(),
        ),
    ] {
        assert_eq!(v2_spans.len(), pest_spans.len(), "{label} count in {path}");
        for (v2_span, pest_span) in v2_spans.into_iter().zip(pest_spans) {
            assert_eq!(
                &text[v2_span.0..v2_span.1],
                &text[pest_span.0..pest_span.1],
                "{label} span in {path}"
            );
        }
    }
}

/// The v2 path's serialized model must byte-match the committed artifacts.
#[test]
fn v2_model_matches_the_committed_artifacts() {
    for (model, artifact) in [
        (
            "tests/conformance/ifml/app.ifml",
            "tests/conformance/ifml/app.ifml.json",
        ),
        (
            "tests/conformance/ifml/cli.ifml",
            "tests/conformance/ifml/cli.ifml.json",
        ),
    ] {
        let source = fs::read_to_string(repo_path(model)).unwrap();
        let (v2, _) = parse_ifml_v2_with_path(model, &source).unwrap();
        let json = v2.to_json_pretty().expect("serialize");
        let expected = fs::read_to_string(repo_path(artifact)).unwrap();
        assert_eq!(json, expected, "golden model for {model}");
    }
}

/// The v2 model + index fed through the unchanged manifest derivation must
/// byte-match the committed manifest artifacts.
#[test]
fn v2_manifest_matches_the_committed_artifacts() {
    for (model, artifact) in [
        (
            "tests/conformance/ifml/app.ifml",
            "tests/conformance/ifml/app.manifest.json",
        ),
        (
            "tests/conformance/ifml/cli.ifml",
            "tests/conformance/ifml/cli.manifest.json",
        ),
    ] {
        let source = fs::read_to_string(repo_path(model)).unwrap();
        let (model_value, mut index) = parse_ifml_v2_with_path(model, &source).unwrap();
        rex_ifml::resolve_ifml_v2_module_uses(&mut index);
        let compilation = IfmlCompilation {
            model: model_value,
            index,
        };
        let manifest = manifest_ifml(&compilation, &IfmlImports::default(), None);
        let json = manifest.to_json_pretty().expect("serialize manifest");
        let expected = fs::read_to_string(repo_path(artifact)).unwrap();
        assert_eq!(json, expected, "golden manifest for {model}");
    }
}
