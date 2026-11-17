//! SK6 (epic pherolang#328, ADR 0005): the three seams `.ifml` shares with
//! the rest of the workspace, pinned at this crate's boundary.
//!
//! `.ifml` stays a separate crate — `phero-driver` depends on `phero-ifml`,
//! so the opposite direction would be a cycle (ADR 0005 D6). What it *does*
//! share lives below both crates: the diagnostic record
//! ([`phero_diagnostics::Diagnostic`], SK2), the surface identity
//! ([`phero_parser::Surface`], SK5), and the import-walk contract (this
//! crate's two-key `IfmlImports` / `walk_ifml_imports`, Phase B of
//! pherolang#117). These tests fail if any of the three is ever re-spelled
//! locally.

use std::path::Path;

use phero_ifml::{compile_ifml_str, walk_ifml_imports, IfmlImports};

/// Takes the workspace's one diagnostic record, whatever crate the caller
/// spelled it through. A local `.ifml` record with the same fields would not
/// satisfy this signature.
fn shared_record(
    record: phero_diagnostics::Diagnostic<&'static str>,
) -> phero_diagnostics::Diagnostic<&'static str> {
    record
}

#[test]
fn the_diagnostic_record_is_the_shared_one() {
    let record: phero_ifml::Diagnostic<&'static str> =
        phero_ifml::Diagnostic::error("boom", None).with_code(phero_ifml::E_PARSE);

    // Type identity, field-by-field: severity, message, span, help, code.
    let record = shared_record(record);
    assert_eq!(record.severity, phero_diagnostics::Severity::Error);
    assert_eq!(record.message, "boom");
    assert_eq!(record.span, None);
    assert_eq!(record.help, None);
    assert_eq!(record.code, Some(phero_ifml::E_PARSE));

    // The severity class is the registry's, re-exported rather than cloned.
    let severity: phero_diagnostics::Severity = phero_ifml::Severity::Warning;
    assert_eq!(severity, phero_diagnostics::Severity::Warning);
}

#[test]
fn a_compilation_tags_each_record_with_its_file() {
    // The file tag stays outside the record: a multi-file compilation is a
    // `Vec<(path, Diagnostic)>`, the shape every driver surface uses.
    let diagnostics: Vec<(String, phero_diagnostics::Diagnostic<&'static str>)> = compile_ifml_str(
        "app.ifml",
        "import \"pager.ifml\";",
        &IfmlImports::default(),
    )
    .expect_err("the import was not provided");

    assert_eq!(diagnostics.len(), 1);
    let (path, record) = &diagnostics[0];
    assert_eq!(path, "app.ifml");
    assert_eq!(record.code, Some(phero_ifml::E_IMPORT_NOT_PROVIDED));
    assert_eq!(record.span, None, "an import-level finding has no span");
}

#[test]
fn the_surface_identity_is_the_shared_one() {
    use phero_parser::Surface;

    assert_eq!(Surface::Ifml.id(), "ifml");
    assert_eq!(Surface::Ifml.extension(), "ifml");
    assert_eq!(Surface::of_extension("ifml"), Some(Surface::Ifml));
    assert_eq!(
        Surface::of_path(Path::new("examples/support/support.ifml")),
        Some(Surface::Ifml)
    );
    assert_eq!(
        Surface::from_path(Path::new("Support.IFML")),
        Some(Surface::Ifml)
    );

    let ifml_surfaces: Vec<Surface> = Surface::ALL
        .into_iter()
        .filter(|surface| *surface == Surface::Ifml)
        .collect();
    assert_eq!(
        ifml_surfaces.len(),
        1,
        "one `.ifml` variant, not two tables"
    );
}

#[test]
fn the_import_bundle_is_keyed_by_importer_and_as_written_path() {
    // Two keys, both load-bearing: the importing file's identity and the
    // import string exactly as written. The same written path under a
    // different importer is a different entry, and the resolved path is
    // never the key.
    let mut imports = IfmlImports::default();
    imports.provide("app.ifml", "patterns/pager.ifml", "module \"Pager\" {}");
    imports.provide("other.ifml", "patterns/pager.ifml", "module \"Pager\" {}");
    imports.provide("app.ifml", "./patterns/pager.ifml", "module \"Pager\" {}");

    assert_eq!(
        imports.get("app.ifml", "patterns/pager.ifml"),
        Some("module \"Pager\" {}")
    );
    assert_eq!(
        imports.get("other.ifml", "patterns/pager.ifml"),
        Some("module \"Pager\" {}"),
        "the importer identity is part of the key"
    );
    assert_eq!(
        imports.get("app.ifml", "./patterns/pager.ifml"),
        Some("module \"Pager\" {}"),
        "`./` is part of the as-written key, not normalised away"
    );
    assert_eq!(
        imports.get("app.ifml", "/abs/patterns/pager.ifml"),
        None,
        "a resolved path is not a bundle key"
    );
}

#[test]
fn the_walker_hands_the_host_identities_and_as_written_paths() {
    // The host resolves an import to a disk path (the returned key); the
    // walker keeps using the as-written import string as the next file's
    // identity, so bundle keys built from this walk match what the resolver
    // looks up. A host that provided the resolved key instead would miss.
    let mut seen: Vec<(String, String)> = Vec::new();
    let walk = walk_ifml_imports(
        "/repo/app.ifml",
        "import \"patterns/pager.ifml\";",
        &mut |importer, import| {
            seen.push((importer.to_string(), import.to_string()));
            match (importer, import) {
                ("/repo/app.ifml", "patterns/pager.ifml") => Some((
                    "/repo/patterns/pager.ifml".to_string(),
                    "import \"../shared/base.ifml\";".to_string(),
                )),
                ("patterns/pager.ifml", "../shared/base.ifml") => Some((
                    "/repo/shared/base.ifml".to_string(),
                    "view \"Base\" {}".to_string(),
                )),
                _ => None,
            }
        },
    );

    assert_eq!(
        seen,
        vec![
            (
                "/repo/app.ifml".to_string(),
                "patterns/pager.ifml".to_string()
            ),
            (
                "patterns/pager.ifml".to_string(),
                "../shared/base.ifml".to_string()
            ),
        ],
        "fetch receives (importer identity, import as written), never a resolved key as identity"
    );

    let identities: Vec<&str> = walk
        .sources
        .iter()
        .map(|(identity, _)| &identity[..])
        .collect();
    assert_eq!(
        identities,
        vec![
            "/repo/app.ifml",
            "patterns/pager.ifml",
            "../shared/base.ifml"
        ],
        "the entry identity is the host's; nested identities are the as-written imports"
    );
}
