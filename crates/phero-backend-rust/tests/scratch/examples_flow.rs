//! Shared flow for the examples harness: locates the seven canonical
//! examples, compiles each through [`phero_driver::compile_str`], and turns
//! the generated Rust code into a namespaced module for the scratch crates
//! (`target/scratch/phero-codegen-test` and `target/scratch/phero-examples-test`).
//!
//! The example list is hardcoded: a missing file fails loudly instead of
//! being skipped, so the harness is the spec for the suite's shape. Each
//! entry is `(module name, app-relative stem)`: the module name is the
//! scratch-crate module the assertion suite imports (the file stem), the
//! stem is the example's path under `examples/` (one directory per
//! application since pherolang#258).
//!
//! This module is compiled into both `codegen_end_to_end.rs` and
//! `examples_conformance.rs` via `#[path]`; helpers used by only one of the
//! two binaries carry `#[allow(dead_code)]` so neither binary warns.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The seven canonical examples as `(module name, app-relative stem)`.
/// Order is also the module declaration order in the scratch crate's
/// `lib.rs`.
pub const EXAMPLES: [(&str, &str); 7] = [
    ("library", "library/library"),
    ("ecommerce", "shop/ecommerce"),
    ("org", "org/org"),
    ("iot", "iot/iot"),
    ("shapes", "shapes/shapes"),
    ("support", "support/support"),
    ("pricing", "pricing/pricing"),
];

pub fn workspace_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../")
            .canonicalize()
            .expect("workspace root")
    })
}

pub fn examples_dir() -> PathBuf {
    workspace_root().join("examples")
}

/// `examples/<app>/<stem>.mox` — the example's path; `stem` is the
/// app-relative stem from [`EXAMPLES`].
pub fn example_path(stem: &str) -> PathBuf {
    examples_dir().join(format!("{stem}.mox"))
}

/// The canonical instance documents stay at `examples/instances/`, keyed by
/// the example's file stem (not its application directory).
#[allow(dead_code)]
pub fn instance_path(name: &str) -> PathBuf {
    examples_dir()
        .join("instances")
        .join(format!("{name}.instance.json"))
}

/// The example's source text; a missing file is a harness failure naming
/// the expected path (never a silent skip).
pub fn example_source(stem: &str) -> String {
    let path = example_path(stem);
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "missing canonical example {}: {error} — the examples suite is part of the harness",
            path.display()
        )
    })
}

/// The example's `import sigil` content, gathered through the sanctioned
/// disk bridge (`phero_driver::workspace_imports::collect_sigil_imports` —
/// the CLI collector) so the rosetta walk is scoped to the example's own
/// directory. Examples without sigil imports yield an empty bundle, so
/// every example compiles through the same path.
pub fn example_imports(stem: &str) -> phero_driver::DomainImports {
    let path = example_path(stem);
    let sources = [(
        path.to_str().expect("utf-8 example path").to_string(),
        example_source(stem),
    )];
    let sigil = phero_driver::workspace_imports::collect_sigil_imports(&sources)
        .expect("gather sigil imports for the example");
    phero_driver::DomainImports {
        schemas: phero_driver::SchemaImports::new(),
        sigil: sigil.imports,
    }
}

/// Compiles an example to the Core IR with **zero** diagnostics allowed.
pub fn compile_example(stem: &str) -> phero_ir::Model {
    let path = example_path(stem);
    let compilation = phero_driver::compile_str(
        path.to_str().expect("utf-8 example path"),
        &example_source(stem),
        &example_imports(stem),
    );
    assert!(
        compilation.diagnostics.is_empty(),
        "example {stem}.mox must compile with zero diagnostics: {:?}",
        compilation.diagnostics
    );
    compilation.model.expect("example lowered to IR")
}

/// The example's generated Rust code, wrapped as a namespaced module body:
/// `pub mod <module> { ... }`.
pub fn example_module(module: &str, stem: &str) -> String {
    let model = compile_example(stem);
    let generated = phero_backend_rust::generate(&model).expect("generate Rust code");
    let models_rs = &generated["models.rs"];
    format!("pub mod {module} {{\n{models_rs}\n}}\n")
}

/// All example modules concatenated (the scratch crate declares one module
/// per example, each with its own arena `Resource` and id types).
pub fn example_modules() -> String {
    EXAMPLES
        .iter()
        .map(|(module, stem)| example_module(module, stem))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The scratch crate that hosts the example modules and the behavioral
/// assertion tests (`tests/scratch/example_assertions.rs`). Used by
/// `examples_conformance.rs`; `codegen_end_to_end.rs` assembles its own
/// larger crate around the same pieces.
#[allow(dead_code)]
pub fn examples_scratch_files() -> Vec<(PathBuf, String)> {
    let runtime_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/phero-runtime")
        .canonicalize()
        .expect("phero-runtime path");

    let lib_rs = format!(
        "{}\n{}",
        example_modules(),
        include_str!("example_assertions.rs")
    );

    vec![
        (
            Path::new("Cargo.toml").to_path_buf(),
            format!(
                "[package]\n\
                 name = \"phero-examples-test\"\n\
                 version = \"0.1.0\"\n\
                 edition = \"2021\"\n\
                 publish = false\n\n\
                 # Standalone scratch crate: never a member of any workspace.\n\
                 [workspace]\n\n\
                 [lib]\n\
                 path = \"src/lib.rs\"\n\n\
                 [dependencies]\n\
                 phero-runtime = {{ path = {:?} }}\n\
                 slotmap = \"1\"\n\
                 serde_json = \"1\"\n",
                runtime_path
            ),
        ),
        (Path::new("src/lib.rs").to_path_buf(), lib_rs),
    ]
}
