//! The nine-surface metadata registry.
//!
//! This is where the **highlight-only** facts about a surface live: the lexer
//! family, the word tables (borrowed from the compiler's own `words` modules,
//! never copied), the builtin type names, the declaration-word table, the
//! documentation path, and the TextMate/LSP mappings. The surface identity
//! itself — variants, `ALL` order, extensions, id, display name, aliases —
//! is [`phero_parser::Surface`] (re-exported as [`Surface`]); the duplicated
//! identity fields on [`SurfaceMetadata`] are a rendering convenience and the
//! tests assert they equal the shared type's.
//!
//! The word tables are references into [`phero_syntax::words`] and
//! [`phero_ifml::words`]. That is the anti-drift contract: a keyword is added
//! to the parser's table, never to a second list here, and the tests fail when
//! a parser table and this registry disagree.

use crate::class::{HighlightClass, HighlightModifier};
use crate::surface::Surface;

/// Which lexer family a surface uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LexerKind {
    /// The shared `phero-syntax` lexer family (`.mox`, `.actor`, `.ddd`,
    /// `.evt`, `.deploy`, `.uc`, `.cmmn`, `.dmn`).
    MoxFamily,
    /// The `phero-ifml` lexer.
    Ifml,
}

/// The builtin primitive type names every non-`.ifml` surface shares.
///
/// The lowercase spellings are the language primitives (`docs/LANGUAGE.md`);
/// the capitalized spellings are the conventional wrapper types vocabulary
/// authors use. Both classify as [`HighlightClass::Type`] with the
/// [`HighlightModifier::DefaultLibrary`] modifier.
pub const BUILTIN_TYPES: &[&str] = &[
    "string", "int", "long", "short", "float", "double", "boolean", "byte", "char", "date",
    "String", "Int", "Long", "Short", "Float", "Double", "Boolean", "Byte", "Char", "Uuid",
    "DateTime", "Instant", "Decimal", "Blob", "bytes",
];

/// The boolean literals shared by every non-`.ifml` surface.
const BOOLEAN_LITERALS: &[&str] = phero_syntax::words::BOOLEAN_WORDS;

/// The `.ddd` literals: booleans plus the ranking values.
const DDD_ALL_LITERALS: &[&str] = &["true", "false", "bm25", "tfIdf", "exact"];

/// One surface's complete metadata.
#[derive(Debug)]
pub struct SurfaceMetadata {
    /// The surface this entry describes.
    pub surface: Surface,
    /// The stable surface id, matching `site/data/surfaces.json`.
    pub id: &'static str,
    /// The file extension, without the leading dot.
    pub extension: &'static str,
    /// Alternative ids accepted by [`Surface::from_id`].
    pub aliases: &'static [&'static str],
    /// The human-readable name.
    pub display_name: &'static str,
    /// The repository-relative documentation path.
    pub documentation: &'static str,
    /// The lexer family the surface is lexed with.
    pub lexer: LexerKind,
    /// The surface's **real lexer keywords** (or, for `.ifml`, its structural
    /// words — the surface has no keyword tokens).
    pub keywords: &'static [&'static str],
    /// The surface's contextual words: ordinary identifiers that are special
    /// only in a grammar position.
    pub contextual: &'static [&'static str],
    /// The surface's literals (booleans, plus the `.ddd` ranking values).
    pub literals: &'static [&'static str],
    /// The builtin type names the surface recognizes.
    pub builtin_types: &'static [&'static str],
    /// Declaration word → the class of the name it introduces. This is the
    /// fallback table; the semantic pass prefers exact AST spans.
    pub declarations: &'static [(&'static str, HighlightClass)],
}

/// `.mox` metadata.
pub static MOX: SurfaceMetadata = SurfaceMetadata {
    surface: Surface::Mox,
    id: "mox",
    extension: "mox",
    aliases: &["domain", "model"],
    display_name: "Domain model",
    documentation: "docs/LANGUAGE.md",
    lexer: LexerKind::MoxFamily,
    keywords: phero_syntax::words::MOX_KEYWORDS,
    contextual: phero_syntax::words::MOX_CONTEXTUAL,
    literals: BOOLEAN_LITERALS,
    builtin_types: BUILTIN_TYPES,
    declarations: &[
        ("package", HighlightClass::Namespace),
        ("class", HighlightClass::Type),
        ("interface", HighlightClass::Type),
        ("enum", HighlightClass::Type),
        ("type", HighlightClass::Type),
        ("annotation", HighlightClass::Annotation),
        ("vocabulary", HighlightClass::Type),
        ("register", HighlightClass::Type),
        ("actors", HighlightClass::Namespace),
        ("import", HighlightClass::Namespace),
    ],
};

/// `.ifml` metadata. The surface has no keyword tokens: `STRUCTURAL_WORDS` are
/// identifiers the parser dispatches on, and `TYPE_REFS` are its primitive
/// type names.
pub static IFML: SurfaceMetadata = SurfaceMetadata {
    surface: Surface::Ifml,
    id: "ifml",
    extension: "ifml",
    aliases: &["flow", "interaction"],
    display_name: "Interaction flow",
    documentation: "docs/IFML.md",
    lexer: LexerKind::Ifml,
    keywords: phero_ifml::words::STRUCTURAL_WORDS,
    contextual: &[],
    literals: phero_ifml::words::BOOLEANS,
    builtin_types: phero_ifml::words::TYPE_REFS,
    declarations: &[
        ("view", HighlightClass::Type),
        ("container", HighlightClass::Type),
        ("component", HighlightClass::Type),
        ("action", HighlightClass::Function),
        ("module", HighlightClass::Namespace),
        ("actor", HighlightClass::Type),
        ("command", HighlightClass::Function),
        ("params", HighlightClass::Parameter),
        ("input", HighlightClass::Parameter),
        ("output", HighlightClass::Parameter),
        ("argument", HighlightClass::Parameter),
        ("option", HighlightClass::Parameter),
        ("prompt", HighlightClass::Property),
        ("field", HighlightClass::Property),
        ("column", HighlightClass::Property),
        ("use", HighlightClass::Namespace),
        ("domain", HighlightClass::Namespace),
    ],
};

/// `.ddd` metadata.
pub static DDD: SurfaceMetadata = SurfaceMetadata {
    surface: Surface::Ddd,
    id: "ddd",
    extension: "ddd",
    aliases: &["design"],
    display_name: "Application design",
    documentation: "docs/DDD.md",
    lexer: LexerKind::MoxFamily,
    keywords: phero_syntax::words::DDD_KEYWORDS,
    contextual: phero_syntax::words::DDD_CONTEXTUAL,
    literals: DDD_ALL_LITERALS,
    builtin_types: BUILTIN_TYPES,
    declarations: &[
        ("application", HighlightClass::Namespace),
        ("base", HighlightClass::Namespace),
        ("module", HighlightClass::Namespace),
        ("service", HighlightClass::Type),
        ("repository", HighlightClass::Type),
        ("entity", HighlightClass::Type),
        ("value", HighlightClass::Type),
        ("dto", HighlightClass::Type),
        ("search", HighlightClass::Type),
        ("machine", HighlightClass::Type),
        ("children", HighlightClass::Type),
        ("extend", HighlightClass::Type),
        ("root", HighlightClass::Keyword),
        ("aggregate", HighlightClass::Keyword),
        ("invariant", HighlightClass::Keyword),
        ("inject", HighlightClass::Variable),
        ("decision", HighlightClass::Function),
        ("import", HighlightClass::Namespace),
    ],
};

/// `.actor` metadata.
pub static ACTOR: SurfaceMetadata = SurfaceMetadata {
    surface: Surface::Actor,
    id: "actor",
    extension: "actor",
    aliases: &["policy", "authorisation", "authorization"],
    display_name: "Authorisation",
    documentation: "docs/LANGUAGE.md",
    lexer: LexerKind::MoxFamily,
    keywords: phero_syntax::words::ACTOR_KEYWORDS,
    contextual: phero_syntax::words::ACTOR_CONTEXTUAL,
    literals: BOOLEAN_LITERALS,
    builtin_types: BUILTIN_TYPES,
    declarations: &[
        ("actors", HighlightClass::Namespace),
        ("actor", HighlightClass::Type),
        ("agent", HighlightClass::Type),
        ("capability", HighlightClass::Function),
        ("obligation", HighlightClass::Function),
        ("purpose", HighlightClass::Constant),
        ("grant", HighlightClass::Namespace),
        ("delegation", HighlightClass::Function),
        ("never_both", HighlightClass::Annotation),
        ("cedar", HighlightClass::Annotation),
        ("import", HighlightClass::Namespace),
    ],
};

/// `.evt` metadata.
pub static EVT: SurfaceMetadata = SurfaceMetadata {
    surface: Surface::Evt,
    id: "evt",
    extension: "evt",
    aliases: &["event"],
    display_name: "Event contract",
    documentation: "docs/EVENTS.md",
    lexer: LexerKind::MoxFamily,
    keywords: phero_syntax::words::EVT_KEYWORDS,
    contextual: phero_syntax::words::EVT_CONTEXTUAL,
    literals: BOOLEAN_LITERALS,
    builtin_types: BUILTIN_TYPES,
    declarations: &[
        ("event", HighlightClass::Type),
        ("channel", HighlightClass::Type),
        ("subscription", HighlightClass::Function),
        ("publishes", HighlightClass::Type),
        ("import", HighlightClass::Namespace),
    ],
};

/// `.deploy` metadata.
pub static DEPLOY: SurfaceMetadata = SurfaceMetadata {
    surface: Surface::Deploy,
    id: "deploy",
    extension: "deploy",
    aliases: &["deployment"],
    display_name: "Deployment",
    documentation: "docs/DEPLOY.md",
    lexer: LexerKind::MoxFamily,
    keywords: phero_syntax::words::DEPLOY_KEYWORDS,
    contextual: phero_syntax::words::DEPLOY_CONTEXTUAL,
    literals: BOOLEAN_LITERALS,
    builtin_types: BUILTIN_TYPES,
    declarations: &[
        ("application", HighlightClass::Namespace),
        ("component", HighlightClass::Type),
        ("profile", HighlightClass::Namespace),
        ("deployment", HighlightClass::Function),
        ("target", HighlightClass::Variable),
        ("defaults", HighlightClass::Property),
        ("mappings", HighlightClass::Property),
        ("connects", HighlightClass::Function),
        ("import", HighlightClass::Namespace),
    ],
};

/// `.uc` metadata.
pub static UC: SurfaceMetadata = SurfaceMetadata {
    surface: Surface::Uc,
    id: "uc",
    extension: "uc",
    aliases: &["usecase", "use-case", "usecases"],
    display_name: "Use case pattern",
    documentation: "docs/USECASES.md",
    lexer: LexerKind::MoxFamily,
    keywords: phero_syntax::words::UC_KEYWORDS,
    contextual: phero_syntax::words::UC_CONTEXTUAL,
    literals: BOOLEAN_LITERALS,
    builtin_types: BUILTIN_TYPES,
    declarations: &[
        ("usecase", HighlightClass::Function),
        ("subject", HighlightClass::Type),
        ("actor", HighlightClass::Type),
        ("message", HighlightClass::Type),
        ("occurrence", HighlightClass::Variable),
        ("perform", HighlightClass::Function),
        ("realizes", HighlightClass::Function),
        ("import", HighlightClass::Namespace),
    ],
};

/// `.cmmn` metadata.
pub static CMMN: SurfaceMetadata = SurfaceMetadata {
    surface: Surface::Cmmn,
    id: "cmmn",
    extension: "cmmn",
    aliases: &["case"],
    display_name: "Case management",
    documentation: "docs/CMMN.md",
    lexer: LexerKind::MoxFamily,
    keywords: phero_syntax::words::CMMN_KEYWORDS,
    contextual: phero_syntax::words::CMMN_CONTEXTUAL,
    literals: BOOLEAN_LITERALS,
    builtin_types: BUILTIN_TYPES,
    declarations: &[
        ("case", HighlightClass::Namespace),
        ("role", HighlightClass::Type),
        ("case_file", HighlightClass::Property),
        ("task", HighlightClass::Function),
        ("service", HighlightClass::Function),
        ("decision", HighlightClass::Function),
        ("milestone", HighlightClass::Function),
        ("stage", HighlightClass::Function),
        ("import", HighlightClass::Namespace),
    ],
};

/// `.dmn` metadata.
pub static DMN: SurfaceMetadata = SurfaceMetadata {
    surface: Surface::Dmn,
    id: "dmn",
    extension: "dmn",
    aliases: &["decision", "decisions"],
    display_name: "Decision model",
    documentation: "docs/DMN.md",
    lexer: LexerKind::MoxFamily,
    keywords: phero_syntax::words::DMN_KEYWORDS,
    contextual: phero_syntax::words::DMN_CONTEXTUAL,
    literals: BOOLEAN_LITERALS,
    builtin_types: BUILTIN_TYPES,
    declarations: &[
        ("decision_model", HighlightClass::Namespace),
        ("namespace", HighlightClass::Namespace),
        ("inputs", HighlightClass::Property),
        ("output", HighlightClass::Property),
        ("outputs", HighlightClass::Property),
        ("knowledge", HighlightClass::Property),
        ("source", HighlightClass::Type),
        ("model", HighlightClass::Function),
        ("decision", HighlightClass::Function),
        ("service", HighlightClass::Function),
        ("view", HighlightClass::Type),
        ("logic", HighlightClass::Property),
        ("import", HighlightClass::Namespace),
    ],
};

/// `.intent` metadata (issue pherolang#196).
pub static INTENTS: SurfaceMetadata = SurfaceMetadata {
    surface: Surface::Intent,
    id: "intent",
    extension: "intent",
    aliases: &["intents"],
    display_name: "Intent specification",
    documentation: "docs/INTENTS.md",
    lexer: LexerKind::MoxFamily,
    keywords: phero_syntax::words::INTENTS_KEYWORDS,
    contextual: phero_syntax::words::INTENTS_CONTEXTUAL,
    literals: BOOLEAN_LITERALS,
    builtin_types: BUILTIN_TYPES,
    declarations: &[
        ("intent", HighlightClass::Function),
        ("element", HighlightClass::Variable),
        ("check", HighlightClass::Property),
        ("import", HighlightClass::Namespace),
    ],
};

/// Every metadata entry, in [`Surface::ALL`] order.
pub static ALL: [&SurfaceMetadata; 10] = [
    &MOX, &IFML, &DDD, &ACTOR, &EVT, &DEPLOY, &UC, &CMMN, &DMN, &INTENTS,
];

/// The metadata entry for `surface`.
#[must_use]
pub fn metadata_for(surface: Surface) -> &'static SurfaceMetadata {
    match surface {
        Surface::Mox => &MOX,
        Surface::Ifml => &IFML,
        Surface::Ddd => &DDD,
        Surface::Actor => &ACTOR,
        Surface::Evt => &EVT,
        Surface::Deploy => &DEPLOY,
        Surface::Uc => &UC,
        Surface::Cmmn => &CMMN,
        Surface::Dmn => &DMN,
        Surface::Intent => &INTENTS,
    }
}

/// Every metadata entry, in [`Surface::ALL`] order.
#[must_use]
pub fn all_metadata() -> &'static [&'static SurfaceMetadata] {
    &ALL
}

/// Classifies a word against the surface's tables.
///
/// Precedence: literals → [`HighlightClass::Constant`], real keywords →
/// [`HighlightClass::Keyword`], contextual words →
/// [`HighlightClass::Keyword`], builtin types → [`HighlightClass::Type`].
/// Returns `None` for any other identifier.
#[must_use]
pub fn keyword_class(surface: Surface, word: &str) -> Option<HighlightClass> {
    let metadata = metadata_for(surface);
    if metadata.literals.contains(&word) {
        return Some(HighlightClass::Constant);
    }
    if metadata.keywords.contains(&word) {
        return Some(HighlightClass::Keyword);
    }
    if metadata.contextual.contains(&word) {
        return Some(HighlightClass::Keyword);
    }
    if metadata.builtin_types.contains(&word) {
        return Some(HighlightClass::Type);
    }
    None
}

/// `true` when `word` is one of the surface's builtin type names.
#[must_use]
pub fn is_builtin_type(surface: Surface, word: &str) -> bool {
    metadata_for(surface).builtin_types.contains(&word)
}

/// The TextMate scope for a class, optionally refined by a modifier.
///
/// Every path is total: a modifier that has no dedicated scope falls back to
/// the class scope, and every returned scope ends in `.pherolang`.
#[must_use]
pub fn scope_for(class: HighlightClass, modifier: Option<HighlightModifier>) -> &'static str {
    match modifier {
        Some(HighlightModifier::Documentation) if class == HighlightClass::Comment => {
            "comment.block.documentation.pherolang"
        }
        Some(HighlightModifier::Readonly)
            if matches!(class, HighlightClass::Variable | HighlightClass::Property) =>
        {
            "variable.other.readonly.pherolang"
        }
        Some(HighlightModifier::DefaultLibrary)
            if matches!(class, HighlightClass::Type | HighlightClass::Function) =>
        {
            "support.type.pherolang"
        }
        Some(HighlightModifier::Deprecated) => "invalid.deprecated.pherolang",
        Some(
            HighlightModifier::Declaration
            | HighlightModifier::Definition
            | HighlightModifier::Documentation
            | HighlightModifier::Readonly
            | HighlightModifier::DefaultLibrary,
        )
        | None => class_scope(class),
    }
}

/// The base TextMate scope for a class (no modifier refinement).
#[must_use]
pub fn class_scope(class: HighlightClass) -> &'static str {
    match class {
        HighlightClass::Keyword => "keyword.control.pherolang",
        HighlightClass::Type => "entity.name.type.pherolang",
        HighlightClass::Function => "entity.name.function.pherolang",
        HighlightClass::Namespace => "entity.name.namespace.pherolang",
        HighlightClass::Variable => "variable.other.pherolang",
        HighlightClass::Parameter => "variable.parameter.pherolang",
        HighlightClass::Property => "variable.other.property.pherolang",
        HighlightClass::String => "string.quoted.double.pherolang",
        HighlightClass::Number => "constant.numeric.pherolang",
        HighlightClass::Comment => "comment.pherolang",
        HighlightClass::Operator => "keyword.operator.pherolang",
        HighlightClass::Punctuation => "punctuation.pherolang",
        HighlightClass::Annotation => "entity.name.function.decorator.pherolang",
        HighlightClass::Constant => "constant.language.pherolang",
        HighlightClass::Escape => "constant.character.escape.pherolang",
        HighlightClass::Error => "invalid.illegal.pherolang",
    }
}

/// The standard LSP semantic-token type for a class, if one exists.
///
/// Only the standard LSP semantic-token vocabulary is emitted: a legend built
/// from this mapping is deployable against any standard client without the
/// server having to negotiate a custom token type.
///
/// Both [`HighlightClass::String`] and [`HighlightClass::Escape`] map to
/// `"string"`: the LSP vocabulary has no escape type of its own.
///
/// [`HighlightClass::Punctuation`] and [`HighlightClass::Error`] have no
/// standard LSP type, so they return `None` and are **omitted** from LSP
/// semantic tokens: punctuation is the editor grammar's job, and lexical
/// errors are reported as diagnostics rather than as coloured tokens.
#[must_use]
pub fn lsp_token_type(class: HighlightClass) -> Option<&'static str> {
    match class {
        HighlightClass::Keyword => Some("keyword"),
        HighlightClass::Type => Some("type"),
        HighlightClass::Function => Some("function"),
        HighlightClass::Namespace => Some("namespace"),
        HighlightClass::Variable => Some("variable"),
        HighlightClass::Parameter => Some("parameter"),
        HighlightClass::Property => Some("property"),
        HighlightClass::String => Some("string"),
        HighlightClass::Number => Some("number"),
        HighlightClass::Comment => Some("comment"),
        HighlightClass::Operator => Some("operator"),
        HighlightClass::Punctuation => None,
        HighlightClass::Annotation => Some("decorator"),
        HighlightClass::Constant => Some("enumMember"),
        HighlightClass::Escape => Some("string"),
        HighlightClass::Error => None,
    }
}

/// The standard LSP semantic-token modifier spelling.
#[must_use]
pub fn lsp_modifier(modifier: HighlightModifier) -> &'static str {
    modifier.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn assert_unique<'a>(words: impl IntoIterator<Item = &'a str>, table: &str, surface: Surface) {
        let mut seen = BTreeSet::new();
        for word in words {
            assert!(
                seen.insert(word),
                "{} metadata `{table}` has a duplicate word `{word}`",
                surface.id()
            );
        }
    }

    #[test]
    fn every_surface_has_one_entry_in_compiler_order() {
        assert_eq!(ALL.len(), Surface::ALL.len());
        for (entry, surface) in ALL.into_iter().zip(Surface::ALL) {
            assert_eq!(entry.surface, surface);
            assert_eq!(entry.id, surface.id());
            assert_eq!(entry.extension, surface.extension());
            assert_eq!(entry.aliases, surface.aliases());
            assert_eq!(entry.display_name, surface.display_name());
            assert_eq!(metadata_for(surface).id, surface.id());
        }
        assert_eq!(all_metadata().len(), 10);
    }

    #[test]
    fn all_tables_are_duplicate_free() {
        for metadata in ALL {
            let surface = metadata.surface;
            assert_unique(metadata.keywords.iter().copied(), "keywords", surface);
            assert_unique(metadata.contextual.iter().copied(), "contextual", surface);
            assert_unique(metadata.literals.iter().copied(), "literals", surface);
            assert_unique(
                metadata.builtin_types.iter().copied(),
                "builtin_types",
                surface,
            );
            assert_unique(
                metadata.declarations.iter().map(|(word, _)| *word),
                "declarations",
                surface,
            );
        }
    }

    #[test]
    fn every_real_keyword_is_a_compiler_keyword() {
        let real: BTreeSet<&str> = phero_syntax::words::KEYWORDS.iter().copied().collect();
        for metadata in ALL {
            if metadata.lexer == LexerKind::Ifml {
                continue;
            }
            for keyword in metadata.keywords {
                assert!(
                    real.contains(keyword),
                    "{} lists `{keyword}` as a keyword but it is not a lexer keyword",
                    metadata.id
                );
            }
        }
    }

    #[test]
    fn every_parser_table_word_appears_in_metadata() {
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
            for word in keywords.iter().chain(contextual) {
                assert!(
                    metadata.keywords.contains(word) || metadata.contextual.contains(word),
                    "{} metadata is missing the parser word `{word}`",
                    surface.id()
                );
            }
            for word in w::BOOLEAN_WORDS {
                assert!(metadata.literals.contains(word));
            }
        }
        for word in phero_ifml::words::STRUCTURAL_WORDS {
            assert!(metadata_for(Surface::Ifml).keywords.contains(word));
        }
    }

    #[test]
    fn ddds_ranking_words_are_literals_and_classify_as_constants() {
        for word in phero_syntax::words::DDD_LITERALS {
            assert!(metadata_for(Surface::Ddd).literals.contains(word));
            assert_eq!(
                keyword_class(Surface::Ddd, word),
                Some(HighlightClass::Constant)
            );
        }
    }

    #[test]
    fn container_is_a_mox_keyword_but_not_a_deploy_keyword() {
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
    fn keyword_class_is_total_and_precise() {
        assert_eq!(
            keyword_class(Surface::Mox, "class"),
            Some(HighlightClass::Keyword)
        );
        assert_eq!(
            keyword_class(Surface::Mox, "true"),
            Some(HighlightClass::Constant)
        );
        assert_eq!(
            keyword_class(Surface::Mox, "String"),
            Some(HighlightClass::Type)
        );
        assert_eq!(
            keyword_class(Surface::Mox, "readonly"),
            Some(HighlightClass::Keyword)
        );
        assert_eq!(keyword_class(Surface::Mox, "Ticket"), None);
        assert_eq!(
            keyword_class(Surface::Ifml, "view"),
            Some(HighlightClass::Keyword)
        );
        assert_eq!(
            keyword_class(Surface::Ifml, "Int"),
            Some(HighlightClass::Type)
        );
        assert_eq!(
            keyword_class(Surface::Ifml, "false"),
            Some(HighlightClass::Constant)
        );
    }

    #[test]
    fn every_scope_and_lsp_mapping_is_total_and_namespaced() {
        for class in HighlightClass::ALL {
            let base = scope_for(class, None);
            assert_eq!(base, class_scope(class));
            assert!(base.ends_with(".pherolang"), "scope {base}");
            let token = lsp_token_type(class);
            if matches!(class, HighlightClass::Punctuation | HighlightClass::Error) {
                assert!(token.is_none(), "{class} has no standard LSP type");
            } else {
                let token = token.unwrap_or_else(|| panic!("{class} has no LSP type"));
                assert!(!token.is_empty());
                assert!(
                    token.chars().all(|c| c.is_ascii_alphanumeric()),
                    "`{token}` is not a standard LSP token type name"
                );
            }
            for modifier in HighlightModifier::ALL {
                let scope = scope_for(class, Some(modifier));
                assert!(scope.ends_with(".pherolang"), "scope {scope}");
                assert_eq!(lsp_modifier(modifier), modifier.as_str());
            }
        }
    }

    #[test]
    fn documentation_comment_has_a_dedicated_scope() {
        assert_eq!(
            scope_for(
                HighlightClass::Comment,
                Some(HighlightModifier::Documentation)
            ),
            "comment.block.documentation.pherolang"
        );
        assert_eq!(
            scope_for(HighlightClass::Comment, None),
            "comment.pherolang"
        );
    }
}
