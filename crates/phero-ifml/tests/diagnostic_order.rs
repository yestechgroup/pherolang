//! Characterization (SK2, epic pherolang#328, issue pherolang#330): the
//! resolver + checker diagnostic order of one multi-file `.ifml`
//! compilation, pinned **before** the shared diagnostic record replaces
//! `IfmlDiagnostic`.
//!
//! The SK2 refactor is behavior-preserving, so this sequence — codes,
//! files, severity, messages, and spans — must not move. The compile order
//! is the resolver's (imports depth-first in source order, then use-site
//! validation), then the checker's passes (cross-reference, authorization,
//! decisions, lints) in `check_ifml_with_decisions`'s order.

use phero_ifml::{check_ifml, compile_ifml_lenient, IfmlDiagnostic, IfmlImports, Severity};

/// A syntactically valid imported file with two module declarations of the
/// same name: one deterministic resolver error (`E0004`, duplicate module)
/// attributed to the **imported** file.
const BROKEN_MODULE: &str = "module \"Panel\" { input { page: Int } output { total: Int } }\n\
                             module \"Panel\" { input { page: Int } output { total: Int } }\n";

/// One import the host never provided (`E0001`), one import with a
/// duplicate module (`E0004` in the imported file), one unknown `use`
/// target (`E0006`), and one dead-action lint (`W0150`) — four findings
/// across two files.
const APP: &str = r#"import "missing.ifml";
import "broken.ifml";

view "Home" {
    use "Ghost" as ghost { };
}

action "Orphan" {
    target: Item;
}
"#;

fn diagnostics() -> Vec<IfmlDiagnostic> {
    let mut imports = IfmlImports::default();
    imports.provide("app.ifml", "broken.ifml", BROKEN_MODULE);
    let (compilation, mut diagnostics) = compile_ifml_lenient("app.ifml", APP, &imports);
    diagnostics.extend(check_ifml(&compilation, None));
    diagnostics
}

#[test]
fn multi_file_diagnostics_keep_their_compilation_order() {
    let diagnostics = diagnostics();
    let seen: Vec<(&str, &str)> = diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.code, diagnostic.file.as_str()))
        .collect();
    assert_eq!(
        seen,
        vec![
            ("E0001", "app.ifml"),
            ("E0005", "broken.ifml"),
            ("E0006", "app.ifml"),
            ("W0150", "app.ifml"),
        ],
        "{diagnostics:#?}"
    );

    let messages: Vec<&str> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert_eq!(
        messages,
        vec![
            "imported ifml 'missing.ifml' was not provided",
            "duplicate module 'Panel'",
            "unknown module 'Ghost'",
            "action 'Orphan' is declared but never invoked",
        ]
    );

    let severities: Vec<Severity> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.severity)
        .collect();
    assert_eq!(
        severities,
        vec![
            Severity::Error,
            Severity::Error,
            Severity::Error,
            Severity::Warning,
        ],
        "resolver and checker findings are errors; the lint band warns"
    );

    // Every span stays in its own file's byte coordinates; an import-level
    // finding with no natural span stays at 0..0.
    let (start, end) = diagnostics[0].span;
    assert_eq!((start, end), (0, 0));
    let (start, end) = diagnostics[1].span;
    assert_eq!(
        &BROKEN_MODULE[start..end],
        "module \"Panel\" { input { page: Int } output { total: Int } }",
        "the duplicate-module finding points at the second declaration"
    );
    let (start, end) = diagnostics[2].span;
    assert_eq!(&APP[start..end], "use \"Ghost\" as ghost { };");
    let (start, end) = diagnostics[3].span;
    assert_eq!(&APP[start..end], "\"Orphan\"");
}
