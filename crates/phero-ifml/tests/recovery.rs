//! The lenient recovery contract: a broken document still produces a partial
//! model, a navigable index, and one span-accurate `E_PARSE` per error.

use phero_ifml::{compile_ifml_lenient, Diagnostic, IfmlCompilation, IfmlImports, E_PARSE};

fn lenient(source: &str) -> (IfmlCompilation, Vec<(String, Diagnostic<&'static str>)>) {
    compile_ifml_lenient("app.ifml", source, &IfmlImports::default())
}

fn span_bounds(span: Option<phero_parser::Span>) -> (usize, usize) {
    let span = span.expect("the diagnostic carries a span");
    (span.start, span.end)
}

fn span_text<'a>(source: &'a str, diagnostic: &Diagnostic<&'static str>) -> &'a str {
    let (start, end) = span_bounds(diagnostic.span);
    &source[start..end]
}

fn assert_all_parse_errors(diagnostics: &[(String, Diagnostic<&'static str>)]) {
    assert!(!diagnostics.is_empty(), "expected at least one diagnostic");
    assert!(
        diagnostics
            .iter()
            .all(|(_, diagnostic)| diagnostic.code == Some(E_PARSE)),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics.iter().all(|(_, diagnostic)| diagnostic
            .span
            .is_some_and(|span| span.start != 0 || span.end != 0)),
        "every recovered span must be non-zero: {diagnostics:?}"
    );
    assert!(
        diagnostics
            .windows(2)
            .all(|pair| span_bounds(pair[0].1.span).0 <= span_bounds(pair[1].1.span).0),
        "diagnostics are in source order: {diagnostics:?}"
    );
}

#[test]
fn missing_semicolon_recovers_the_property_and_the_next_member() {
    let source = r#"
view "V" {
    component "c" {
        type: list
        data: Item;
    }
}
"#;
    let (compilation, diagnostics) = lenient(source);
    assert_all_parse_errors(&diagnostics);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].1.message.contains("type"), "{diagnostics:?}");
    let component = &compilation.model.views[0].components[0];
    // `type` is promoted out of the property bag, so its recovery shows up as
    // the component type; `data` proves the next member was not swallowed.
    assert_eq!(
        component.component_type,
        Some(phero_ifml::ComponentType::List)
    );
    let keys: Vec<&str> = component
        .properties
        .iter()
        .map(|property| property.key.as_str())
        .collect();
    assert_eq!(keys, ["data"], "the next member was not swallowed");
    assert_eq!(
        compilation.index.components.len(),
        1,
        "the component remains navigable"
    );
}

#[test]
fn missing_closing_brace_at_eof_keeps_earlier_declarations() {
    let source = r#"
view "A" {
    component "grid" {
        type: list;
        data: Item;
    }
}
view "B" {
"#;
    let (compilation, diagnostics) = lenient(source);
    assert_all_parse_errors(&diagnostics);
    assert_eq!(compilation.model.views.len(), 2, "{diagnostics:?}");
    assert_eq!(compilation.index.views.len(), 2);
    assert!(
        diagnostics
            .iter()
            .any(|(_, diagnostic)| diagnostic.message.contains('}')),
        "{diagnostics:?}"
    );
}

#[test]
fn a_stray_token_between_declarations_is_one_junk_region() {
    let source = r#"
view "A" { }
@ @ @
view "B" { }
"#;
    let (compilation, diagnostics) = lenient(source);
    assert_all_parse_errors(&diagnostics);
    assert_eq!(compilation.model.views.len(), 2, "{diagnostics:?}");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
}

#[test]
fn an_unclosed_string_keeps_preceding_declarations() {
    let source = r#"
view "A" {
    component "grid" {
        type: list;
        data: Item;
    }
}
view "B" {
    label: "oops
}
"#;
    let (compilation, diagnostics) = lenient(source);
    assert_all_parse_errors(&diagnostics);
    assert!(!compilation.model.views.is_empty(), "{diagnostics:?}");
    assert_eq!(compilation.model.views[0].name, "A");
}

#[test]
fn a_committed_view_survives_a_missing_body() {
    let source = r#"view "X""#;
    let (compilation, diagnostics) = lenient(source);
    assert_all_parse_errors(&diagnostics);
    assert_eq!(compilation.model.views.len(), 1, "{diagnostics:?}");
    assert_eq!(compilation.model.views[0].name, "X");
    assert_eq!(compilation.index.views.len(), 1);
}

#[test]
fn multiple_errors_are_reported_once_each_with_real_spans() {
    let source = r#"
view "V" {
    component "c" {
        type: list
        data: Item
    }
}
"#;
    let (compilation, diagnostics) = lenient(source);
    assert_all_parse_errors(&diagnostics);
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    for (_, diagnostic) in &diagnostics {
        // Missing-`;` diagnostics highlight the value they follow.
        assert!(!span_text(source, diagnostic).is_empty(), "{diagnostic:?}");
    }
    assert_eq!(
        compilation.model.views[0].components[0].component_type,
        Some(phero_ifml::ComponentType::List)
    );
    assert_eq!(compilation.model.views[0].components[0].properties.len(), 1);
}

#[test]
fn a_broken_import_still_contributes_its_partial_module() {
    let main = r#"import "pager.ifml";
view "V" {
    use "Pager" { page: 1; };
}
"#;
    let mut imports = IfmlImports::default();
    imports.provide(
        "app.ifml",
        "pager.ifml",
        r#"module "Pager" {
    input { page: Int }
    output { selected: Uuid }
"#,
    );
    let (compilation, diagnostics) = compile_ifml_lenient("app.ifml", main, &imports);
    assert!(!diagnostics.is_empty());
    assert!(
        diagnostics
            .iter()
            .any(|(file, diagnostic)| file == "pager.ifml" && diagnostic.code == Some(E_PARSE)),
        "{diagnostics:?}"
    );
    assert_eq!(compilation.index.module_decls.len(), 1, "{diagnostics:?}");
    assert_eq!(compilation.index.module_uses.len(), 1);
    assert!(
        compilation.index.module_uses[0].resolved_module.is_some(),
        "the partial imported module resolves the use: {diagnostics:?}"
    );
}

#[test]
fn deep_nesting_is_a_guard_diagnostic_not_a_stack_overflow() {
    let source = "view \"V\" { ".to_string() + &"(".repeat(10_000) + &")".repeat(10_000) + " }";
    let (compilation, diagnostics) = lenient(&source);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].1.code.unwrap(), E_PARSE);
    assert!(
        diagnostics[0].1.message.contains("nesting depth"),
        "{diagnostics:?}"
    );
    assert!(compilation.model.views.is_empty());
}
