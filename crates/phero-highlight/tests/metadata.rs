//! Registry drift tests: the nine-surface metadata must match the compiler's
//! word tables and the site's surface catalogue.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use phero_highlight::{
    all_metadata, class_scope, keyword_class, lsp_modifier, lsp_token_type, metadata_for,
    scope_for, HighlightClass, HighlightModifier, Surface,
};

fn site_surfaces() -> BTreeMap<String, String> {
    let json: serde_json::Value =
        serde_json::from_str(include_str!("../../../site/data/surfaces.json"))
            .expect("the site surface catalogue is valid JSON");
    json["surfaces"]
        .as_array()
        .expect("surfaces array")
        .iter()
        .map(|entry| {
            (
                entry["id"].as_str().expect("id").to_string(),
                entry["extension"].as_str().expect("extension").to_string(),
            )
        })
        .collect()
}

#[test]
fn every_surface_matches_the_site_catalogue() {
    let site = site_surfaces();
    assert_eq!(site.len(), 9, "the site lists nine surfaces");
    assert_eq!(all_metadata().len(), 9);

    let mut seen = BTreeSet::new();
    for metadata in all_metadata() {
        let extension = site
            .get(metadata.id)
            .unwrap_or_else(|| panic!("site catalogue is missing `{}`", metadata.id));
        assert_eq!(
            *extension,
            format!(".{}", metadata.extension),
            "`{}` extension drifted from the site catalogue",
            metadata.id
        );
        assert!(seen.insert(metadata.id));
        assert_eq!(metadata.surface.id(), metadata.id);
        assert_eq!(metadata.surface.extension(), metadata.extension);
        assert_eq!(metadata.surface.display_name(), metadata.display_name);
        assert_eq!(metadata.aliases, metadata.surface.aliases());
        assert!(!metadata.documentation.is_empty());
    }
    let site_ids: BTreeSet<&str> = site.keys().map(String::as_str).collect();
    assert_eq!(seen, site_ids);
}

#[test]
fn metadata_order_is_the_compiler_order_and_deterministic() {
    for (index, surface) in Surface::ALL.into_iter().enumerate() {
        assert_eq!(all_metadata()[index].surface, surface);
        assert_eq!(metadata_for(surface).id, surface.id());
        assert_eq!(
            metadata_for(surface) as *const _,
            all_metadata()[index] as *const _
        );
    }
    assert_eq!(
        Surface::ALL.map(Surface::id),
        ["mox", "ifml", "ddd", "actor", "evt", "deploy", "uc", "cmmn", "dmn"]
    );
}

#[test]
fn every_shared_surface_has_metadata_and_round_trips() {
    // Iterates the shared identity's `ALL` (not the registry's), so a surface
    // added to `phero_parser::Surface` fails here until it has a registry row.
    assert_eq!(all_metadata().len(), Surface::ALL.len());
    for surface in Surface::ALL {
        let entry = metadata_for(surface);
        assert_eq!(entry.surface, surface);
        assert_eq!(entry.id, surface.id());
        assert_eq!(entry.extension, surface.extension());
        assert_eq!(entry.display_name, surface.display_name());
        assert_eq!(entry.aliases, surface.aliases());
        assert!(!entry.documentation.is_empty());
        assert_eq!(Surface::of_extension(surface.extension()), Some(surface));
        let path = Path::new("support").with_extension(surface.extension());
        assert_eq!(Surface::of_path(&path), Some(surface));
        assert_eq!(Surface::from_path(&path), Some(surface));
        assert_eq!(Surface::from_id(surface.id()), Some(surface));
        assert_eq!(
            Surface::from_id(&surface.id().to_ascii_uppercase()),
            Some(surface)
        );
    }
}

#[test]
fn every_table_is_duplicate_free() {
    for metadata in all_metadata() {
        for (name, table) in [
            ("keywords", metadata.keywords),
            ("contextual", metadata.contextual),
            ("literals", metadata.literals),
            ("builtin_types", metadata.builtin_types),
        ] {
            let unique: BTreeSet<&str> = table.iter().copied().collect();
            assert_eq!(
                unique.len(),
                table.len(),
                "{} metadata `{name}` has a duplicate",
                metadata.id
            );
        }
        let declarations: BTreeSet<&str> = metadata
            .declarations
            .iter()
            .map(|(word, _)| *word)
            .collect();
        assert_eq!(
            declarations.len(),
            metadata.declarations.len(),
            "{} metadata `declarations` has a duplicate",
            metadata.id
        );
    }
}

#[test]
fn every_surface_keyword_is_a_real_compiler_keyword() {
    let real: BTreeSet<&str> = phero_syntax::words::KEYWORDS.iter().copied().collect();
    for metadata in all_metadata() {
        if metadata.id == "ifml" {
            // `.ifml` has no keyword tokens: its structural words are ordinary
            // identifiers the parser dispatches on.
            assert_eq!(metadata.keywords, phero_ifml::words::STRUCTURAL_WORDS);
            continue;
        }
        for keyword in metadata.keywords {
            assert!(
                real.contains(keyword),
                "{} lists `{keyword}`, which the compiler lexer does not know",
                metadata.id
            );
        }
    }
}

#[test]
fn every_parser_table_word_appears_in_the_registry() {
    use phero_syntax::words as w;
    let cases: [(Surface, &[&str], &[&str]); 8] = [
        (Surface::Mox, w::MOX_KEYWORDS, w::MOX_CONTEXTUAL),
        (Surface::Actor, w::ACTOR_KEYWORDS, w::ACTOR_CONTEXTUAL),
        (Surface::Ddd, w::DDD_KEYWORDS, w::DDD_CONTEXTUAL),
        (Surface::Evt, w::EVT_KEYWORDS, w::EVT_CONTEXTUAL),
        (Surface::Deploy, w::DEPLOY_KEYWORDS, w::DEPLOY_CONTEXTUAL),
        (Surface::Uc, w::UC_KEYWORDS, w::UC_CONTEXTUAL),
        (Surface::Cmmn, w::CMMN_KEYWORDS, w::CMMN_CONTEXTUAL),
        (Surface::Dmn, w::DMN_KEYWORDS, w::DMN_CONTEXTUAL),
    ];
    for (surface, keywords, contextual) in cases {
        let metadata = metadata_for(surface);
        assert_eq!(metadata.keywords, keywords);
        assert_eq!(metadata.contextual, contextual);
        for word in keywords.iter().chain(contextual) {
            assert!(
                metadata.keywords.contains(word) || metadata.contextual.contains(word),
                "{} is missing `{word}`",
                surface.id()
            );
        }
    }
    for word in phero_syntax::words::DDD_LITERALS {
        assert!(metadata_for(Surface::Ddd).literals.contains(word));
    }
    for word in phero_syntax::words::BOOLEAN_WORDS {
        for surface in [
            Surface::Mox,
            Surface::Actor,
            Surface::Ddd,
            Surface::Evt,
            Surface::Deploy,
            Surface::Uc,
            Surface::Cmmn,
            Surface::Dmn,
        ] {
            assert!(metadata_for(surface).literals.contains(word));
        }
    }
    assert_eq!(
        metadata_for(Surface::Ifml).keywords,
        phero_ifml::words::STRUCTURAL_WORDS
    );
    assert_eq!(
        metadata_for(Surface::Ifml).builtin_types,
        phero_ifml::words::TYPE_REFS
    );
    assert_eq!(
        metadata_for(Surface::Ifml).literals,
        phero_ifml::words::BOOLEANS
    );
}

#[test]
fn container_is_a_mox_keyword_and_not_a_deploy_keyword() {
    assert!(metadata_for(Surface::Mox).keywords.contains(&"container"));
    assert!(!metadata_for(Surface::Deploy)
        .keywords
        .contains(&"container"));
    assert!(!metadata_for(Surface::Deploy)
        .contextual
        .contains(&"container"));
    assert_eq!(
        keyword_class(Surface::Mox, "container"),
        Some(HighlightClass::Keyword)
    );
    assert_eq!(keyword_class(Surface::Deploy, "container"), None);
}

#[test]
fn scopes_and_lsp_mappings_are_total() {
    let mut scopes = BTreeSet::new();
    let mut lsp_types = BTreeSet::new();
    for class in HighlightClass::ALL {
        let scope = scope_for(class, None);
        assert_eq!(scope, class_scope(class));
        assert!(scope.ends_with(".pherolang"), "scope `{scope}`");
        scopes.insert(scope);
        // Only standard LSP types are emitted: `Punctuation` and `Error` have
        // none and are omitted (the editor grammar owns punctuation; lexical
        // errors are diagnostics).
        if let Some(token) = lsp_token_type(class) {
            assert!(!token.is_empty());
            lsp_types.insert(token);
        } else {
            assert!(
                matches!(class, HighlightClass::Punctuation | HighlightClass::Error),
                "{class} has no standard LSP type"
            );
        }
    }
    assert!(
        lsp_token_type(HighlightClass::Punctuation).is_none(),
        "punctuation is left to the editor grammar"
    );
    assert!(
        lsp_token_type(HighlightClass::Error).is_none(),
        "errors are reported as diagnostics"
    );
    assert_eq!(scopes.len(), 16, "every class has its own base scope");
    for class in HighlightClass::ALL {
        for modifier in HighlightModifier::ALL {
            let scope = scope_for(class, Some(modifier));
            assert!(scope.ends_with(".pherolang"));
            assert!(!scope.is_empty());
            assert_eq!(lsp_modifier(modifier), modifier.as_str());
        }
    }
    assert_eq!(
        lsp_types.len(),
        13,
        "string and escape share `string`; punctuation and error are omitted"
    );
}

#[test]
fn builtin_types_are_classified_for_every_surface() {
    for surface in Surface::ALL {
        for builtin in metadata_for(surface).builtin_types {
            assert_eq!(
                keyword_class(surface, builtin),
                Some(HighlightClass::Type),
                "{} builtin `{builtin}`",
                surface.id()
            );
        }
    }
}

#[test]
fn declaration_tables_name_a_real_class() {
    for metadata in all_metadata() {
        for (word, class) in metadata.declarations {
            assert!(
                metadata.keywords.contains(word) || metadata.contextual.contains(word),
                "{} declaration word `{word}` is not in the surface vocabulary",
                metadata.id
            );
            assert!(HighlightClass::ALL.contains(class));
        }
    }
}
