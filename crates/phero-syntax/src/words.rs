//! The compiler's public word tables: real lexer keywords and the
//! per-surface contextual words.
//!
//! This module is the one place a consumer (the highlighter, an editor
//! grammar generator, a hover provider, documentation tooling) asks *what
//! words does this surface know?*. The tables are derived from their
//! owners — [`crate::lexer`] for the real keywords and the per-surface
//! parser modules for the contextual words — and the tests at the bottom
//! fail when a table and its owner drift apart.
//!
//! A word is **not** a keyword on every surface. `container` is a real
//! lexer keyword and therefore a `.mox` keyword, but `.deploy` treats it as
//! an ordinary word in value position; [`MOX_KEYWORDS`] and the other
//! per-surface tables carry exactly the real keyword tokens each grammar
//! recognizes. Contextual words are ordinary identifiers that are special
//! only in a grammar position; they are listed per surface for the same
//! reason.

/// Every real keyword of [`crate::lexer::Token`], in the lexer's declaration
/// order. Each entry is checked against the lexer by a test.
pub const KEYWORDS: &[&str] = &[
    "package",
    "annotation",
    "as",
    "class",
    "extends",
    "interface",
    "enum",
    "type",
    "wraps",
    "opaque",
    "contains",
    "refers",
    "container",
    "opposite",
    "op",
    "derived",
    "true",
    "false",
    "vocabulary",
    "from",
    "version",
    "key",
    "facet",
    "actors",
    "actor",
    "agent",
    "capability",
    "grant",
    "permit",
    "forbid",
    "when",
    "obligation",
    "on",
    "never_both",
    "delegation",
    "purpose",
    "cedar",
    "event",
    "channel",
    "publishes",
    "subscription",
    "import",
];

/// The boolean literals every surface may spell. They are real `.mox` lexer
/// keywords and ordinary identifiers in `.ifml`; highlighting classifies
/// them as constants on every surface.
pub const BOOLEAN_WORDS: &[&str] = &["true", "false"];

/// The real lexer keywords the `.mox` grammar recognizes.
pub const MOX_KEYWORDS: &[&str] = KEYWORDS;

/// The contextual words of `.mox` and the inline `actors` block: feature
/// modifiers, import sigils, the `register` declaration word, the `table`
/// annotation, datatype blocks, and the constraint keywords.
pub const MOX_CONTEXTUAL: &[&str] = &[
    "id",
    "readonly",
    "schema",
    "sigil",
    "to",
    "register",
    "table",
    "create",
    "convert",
    "format",
    "unique",
    "set",
    "pattern",
    "minLength",
    "maxLength",
    "minimum",
    "maximum",
];

/// The real lexer keywords the `.actor` grammar recognizes.
pub const ACTOR_KEYWORDS: &[&str] = &[
    "import",
    "actors",
    "actor",
    "agent",
    "capability",
    "grant",
    "permit",
    "forbid",
    "when",
    "obligation",
    "on",
    "never_both",
    "delegation",
    "purpose",
    "cedar",
    "from",
    "extends",
];

/// The contextual words of `.actor`: the session-kind modifier and the
/// delegation `to`.
pub const ACTOR_CONTEXTUAL: &[&str] = &["human", "to"];

/// The real lexer keywords the `.ddd` grammar recognizes.
pub const DDD_KEYWORDS: &[&str] = &["import", "capability", "when", "version", "from"];

/// The contextual words of `.ddd`, in the same order as the parser's
/// grammar: declarations, clauses, flags, built-ins, search members, the
/// executable-statement words, and the parser-only ranking values.
pub const DDD_CONTEXTUAL: &[&str] = &[
    "application",
    "base",
    "module",
    "service",
    "search",
    "children",
    "extend",
    "root",
    "aggregate",
    "invariant",
    "abstract",
    "repository",
    "inject",
    "decision",
    "invokes",
    "compatible",
    "emits",
    "transition",
    "throws",
    "protected",
    "machine",
    "initial",
    "terminal",
    "via",
    "boost",
    "analyzer",
    "limit",
    "max",
    "cursor",
    "where",
    "page",
    "asc",
    "desc",
    "ranking",
    "custom",
    "pagination",
    "entity",
    "value",
    "dto",
    "scaffold",
    "auditable",
    "optimisticLocking",
    "nonPersistent",
    "cache",
    "findById",
    "findAll",
    "findByExample",
    "findByKeys",
    "save",
    "delete",
    "text",
    "filters",
    "sort",
    "document",
    "let",
    "guard",
    "else",
    "error",
    "if",
    "return",
];

/// The `.ddd` ranking values that are constants, not keywords.
pub const DDD_LITERALS: &[&str] = &["bm25", "tfIdf", "exact"];

/// The real lexer keywords the `.evt` grammar recognizes.
pub const EVT_KEYWORDS: &[&str] = &[
    "import",
    "event",
    "channel",
    "publishes",
    "subscription",
    "version",
    "when",
    "type",
];

/// The contextual words of `.evt`: subscription/channel words and the
/// delivery-policy clause words.
pub const EVT_CONTEXTUAL: &[&str] = &[
    "events",
    "consumer",
    "of",
    "scope",
    "owner",
    "classification",
    "retention",
    "pii",
    "cloudEvents",
    "source",
    "subject",
    "delivery",
    "mode",
    "maxAttempts",
    "backoffSeconds",
    "timeoutSeconds",
    "deadLetter",
];

/// The real lexer keywords the `.deploy` grammar recognizes. The deploy
/// grammar is contextual by design: `container`, `event`, and the rest are
/// ordinary words in value position.
pub const DEPLOY_KEYWORDS: &[&str] = &["import"];

/// The contextual words of `.deploy`.
pub const DEPLOY_CONTEXTUAL: &[&str] = &[
    "application",
    "component",
    "connects",
    "profile",
    "deployment",
    "target",
    "defaults",
    "mappings",
    "require",
    "prohibit",
    "requires",
    "design",
    "flow",
    "env",
    "use",
    "configure",
    "for",
];

/// The real lexer keywords the `.uc` grammar recognizes.
pub const UC_KEYWORDS: &[&str] = &["import", "as", "actor", "from", "when"];

/// The contextual words of `.uc`.
pub const UC_CONTEXTUAL: &[&str] = &[
    "usecase",
    "subject",
    "occurrence",
    "perform",
    "message",
    "of",
    "to",
    "first",
    "then",
    "start",
    "done",
    "requires",
    "realizes",
    "source",
    "target",
];

/// The real lexer keywords the `.cmmn` grammar recognizes.
pub const CMMN_KEYWORDS: &[&str] = &["import", "as", "version", "when", "on"];

/// The contextual words of `.cmmn`.
pub const CMMN_CONTEXTUAL: &[&str] = &[
    "case",
    "roles",
    "role",
    "case_file",
    "entry",
    "exit",
    "performer",
    "execute",
    "invokes",
    "compatible",
    "inputs",
    "outputs",
    "required",
    "repeats",
    "auto",
    "discretionary",
    "stage",
    "task",
    "service",
    "decision",
    "milestone",
];

/// The real lexer keywords the `.dmn` grammar recognizes.
pub const DMN_KEYWORDS: &[&str] = &["import", "version", "when"];

/// The contextual words of `.dmn`.
pub const DMN_CONTEXTUAL: &[&str] = &[
    "decision_model",
    "namespace",
    "id",
    "effective",
    "jurisdiction",
    "inputs",
    "knowledge",
    "source",
    "model",
    "parameters",
    "output",
    "outputs",
    "decisions",
    "decision",
    "logic",
    "table",
    "expression",
    "invoke",
    "emits",
    "requirements",
    "information",
    "requires",
    "invokes",
    "authority",
    "governed_by",
    "services",
    "service",
    "encapsulates",
    "views",
    "view",
    "show",
    "direction",
    "description",
    "reference",
    "name",
];

/// The real lexer keywords the `.intent` grammar recognizes (issue
/// pherolang#196): the shared `import` is the only real keyword — the
/// declaration and link words are contextual.
pub const INTENTS_KEYWORDS: &[&str] = &["import"];

/// The contextual words of `.intent`. The link kind words (`class`,
/// `capability`, `event`, `view`, `command`, `op`, `invariant`,
/// `decision`) are deliberately **not** listed: they are an open,
/// driver-validated vocabulary in plain name position, never
/// grammar-special.
pub const INTENTS_CONTEXTUAL: &[&str] = &["intent", "element", "check"];

/// Every word the Rune (`.rosetta`) grammar matches by text (RN9, ADR 0013).
/// Rune has **no** reserved identifiers: every word lexes as a Rune
/// identifier and is special only where the grammar asks for it, so CDM
/// attributes named `condition`, `source`, `scope` or `version` stay names.
/// The hyphenated keywords and the `T+N` timing words are separate
/// ([`RUNE_HYPHEN_KEYWORDS`], [`RUNE_TIMING_WORDS`]).
pub const RUNE_CONTEXTUAL: &[&str] = &[
    "ASATP",
    "False",
    "True",
    "absent",
    "add",
    "alias",
    "all",
    "and",
    "annotation",
    "any",
    "as",
    "basicType",
    "body",
    "choice",
    "condition",
    "contains",
    "corpus",
    "count",
    "default",
    "disjoint",
    "displayName",
    "distinct",
    "docReference",
    "eligibility",
    "else",
    "empty",
    "enrich",
    "enum",
    "exists",
    "extends",
    "extract",
    "filter",
    "first",
    "flatten",
    "for",
    "from",
    "func",
    "function",
    "if",
    "import",
    "in",
    "ingest",
    "inputs",
    "is",
    "isEvent",
    "isProduct",
    "item",
    "join",
    "label",
    "last",
    "library",
    "max",
    "metaType",
    "min",
    "multiple",
    "namespace",
    "only",
    "optional",
    "or",
    "output",
    "override",
    "prefix",
    "projection",
    "provision",
    "rationale",
    "rationale_author",
    "recordType",
    "reduce",
    "report",
    "reportedField",
    "reporting",
    "required",
    "reverse",
    "root",
    "rule",
    "ruleReference",
    "schema",
    "scope",
    "segment",
    "set",
    "single",
    "sort",
    "source",
    "structured_provision",
    "sum",
    "switch",
    "then",
    "type",
    "typeAlias",
    "version",
    "when",
    "with",
];

/// The hyphenated Rune keywords. Each one is a single lexer token (Xtext's
/// global keywords), longest first where one is a prefix of another.
pub const RUNE_HYPHEN_KEYWORDS: &[&str] = &[
    "to-zoned-date-time",
    "to-date-time",
    "to-date",
    "to-string",
    "to-number",
    "to-int",
    "to-time",
    "to-enum",
    "only-element",
    "one-of",
    "with-meta",
    "as-key",
    "post-condition",
    "real-time",
];

/// The report timing words `T+1` … `T+5`: three touching tokens each, so
/// `T + 1` in an expression stays arithmetic.
pub const RUNE_TIMING_WORDS: &[&str] = &["T+1", "T+2", "T+3", "T+4", "T+5"];

/// The words that start a top-level Rune element. A name position whose
/// body runs into the next element never consumes one (Xtext lexes them as
/// keyword tokens).
pub const RUNE_ELEMENT_WORDS: &[&str] = &[
    "namespace",
    "type",
    "enum",
    "choice",
    "annotation",
    "typeAlias",
    "basicType",
    "recordType",
    "library",
    "func",
    "report",
    "rule",
    "schema",
    "body",
    "corpus",
    "segment",
    "metaType",
];

/// The words a report reference list (`body`, `corpus`, `segment`, rule
/// references) stops at: the element words plus the report's structural
/// words.
pub const RUNE_REPORT_STOP_WORDS: &[&str] = &[
    "in",
    "from",
    "when",
    "with",
    "type",
    "and",
    "reporting",
    "eligibility",
    "namespace",
    "enum",
    "choice",
    "annotation",
    "typeAlias",
    "basicType",
    "recordType",
    "library",
    "func",
    "report",
    "rule",
    "schema",
    "body",
    "corpus",
    "segment",
    "metaType",
];

/// The words that end a `docReference` segment list.
pub const RUNE_DOC_REFERENCE_WORDS: &[&str] = &[
    "rationale",
    "rationale_author",
    "structured_provision",
    "provision",
    "reportedField",
];

/// The expression-operator words. In Xtext they are keyword tokens, never
/// identifiers, so a symbol, feature or key position in an expression never
/// consumes one (unless `^`-escaped).
pub const RUNE_EXPRESSION_WORDS: &[&str] = &[
    "then", "and", "or", "contains", "disjoint", "default", "join", "exists", "single", "multiple",
    "is", "absent", "count", "flatten", "distinct", "reverse", "first", "last", "sum", "min",
    "max", "sort", "choice", "switch", "as", "filter", "extract", "reduce", "if", "else", "True",
    "False", "empty", "item", "only", "required", "optional", "any", "all",
];

/// Rune's **reserved** words: the grammar's keyword tokens that the Java
/// implementation (Xtext) reserves, so a plain occurrence is never a name
/// there. It excludes the words Xtext's `ValidID` rule re-allows as names
/// (`condition`, `source`, `version`, `scope`, `ingest`, `enrich`,
/// `projection`). This parser reserves nothing ([`RUNE_CONTEXTUAL`]), but a
/// rename or a generated name must still avoid these to stay valid for the
/// reference implementation. Every entry is a [`RUNE_CONTEXTUAL`] word except
/// `super`, an Xtext keyword with no production here.
pub const RUNE_RESERVED_WORDS: &[&str] = &[
    "absent",
    "add",
    "alias",
    "all",
    "and",
    "annotation",
    "any",
    "as",
    "basicType",
    "body",
    "choice",
    "contains",
    "corpus",
    "count",
    "default",
    "disjoint",
    "displayName",
    "docReference",
    "eligibility",
    "else",
    "empty",
    "enum",
    "exists",
    "extends",
    "extract",
    "False",
    "filter",
    "first",
    "flatten",
    "for",
    "from",
    "func",
    "function",
    "if",
    "import",
    "in",
    "inputs",
    "is",
    "item",
    "join",
    "label",
    "last",
    "library",
    "max",
    "metaType",
    "min",
    "multiple",
    "namespace",
    "only",
    "or",
    "output",
    "override",
    "prefix",
    "provision",
    "rationale",
    "reduce",
    "report",
    "reportedField",
    "reporting",
    "reverse",
    "root",
    "rule",
    "ruleReference",
    "schema",
    "segment",
    "set",
    "single",
    "sort",
    "sum",
    "super",
    "switch",
    "then",
    "True",
    "type",
    "typeAlias",
    "when",
    "with",
];

/// The Rune boolean literal words (`literal = STRING | "True" | "False" | …`),
/// a subset of [`RUNE_CONTEXTUAL`] the highlighter classifies as constants.
pub const RUNE_BOOLEAN_WORDS: &[&str] = &["True", "False"];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_rune_reserved_word_is_a_rune_grammar_word() {
        unique(RUNE_RESERVED_WORDS, "RUNE_RESERVED_WORDS");
        for word in RUNE_RESERVED_WORDS {
            assert!(
                RUNE_CONTEXTUAL.contains(word) || *word == "super",
                "reserved Rune word `{word}` is not in RUNE_CONTEXTUAL"
            );
        }
    }

    fn unique(words: &[&str], table: &str) {
        let set: BTreeSet<&str> = words.iter().copied().collect();
        assert_eq!(
            set.len(),
            words.len(),
            "{table} contains a duplicate word: {words:?}"
        );
    }

    #[test]
    fn every_table_is_duplicate_free() {
        unique(KEYWORDS, "KEYWORDS");
        unique(BOOLEAN_WORDS, "BOOLEAN_WORDS");
        for (name, words) in [
            ("MOX_CONTEXTUAL", MOX_CONTEXTUAL),
            ("ACTOR_KEYWORDS", ACTOR_KEYWORDS),
            ("ACTOR_CONTEXTUAL", ACTOR_CONTEXTUAL),
            ("DDD_KEYWORDS", DDD_KEYWORDS),
            ("DDD_CONTEXTUAL", DDD_CONTEXTUAL),
            ("DDD_LITERALS", DDD_LITERALS),
            ("EVT_KEYWORDS", EVT_KEYWORDS),
            ("EVT_CONTEXTUAL", EVT_CONTEXTUAL),
            ("DEPLOY_KEYWORDS", DEPLOY_KEYWORDS),
            ("DEPLOY_CONTEXTUAL", DEPLOY_CONTEXTUAL),
            ("UC_KEYWORDS", UC_KEYWORDS),
            ("UC_CONTEXTUAL", UC_CONTEXTUAL),
            ("CMMN_KEYWORDS", CMMN_KEYWORDS),
            ("CMMN_CONTEXTUAL", CMMN_CONTEXTUAL),
            ("DMN_KEYWORDS", DMN_KEYWORDS),
            ("DMN_CONTEXTUAL", DMN_CONTEXTUAL),
            ("INTENTS_KEYWORDS", INTENTS_KEYWORDS),
            ("INTENTS_CONTEXTUAL", INTENTS_CONTEXTUAL),
            ("RUNE_CONTEXTUAL", RUNE_CONTEXTUAL),
            ("RUNE_HYPHEN_KEYWORDS", RUNE_HYPHEN_KEYWORDS),
            ("RUNE_TIMING_WORDS", RUNE_TIMING_WORDS),
            ("RUNE_ELEMENT_WORDS", RUNE_ELEMENT_WORDS),
            ("RUNE_REPORT_STOP_WORDS", RUNE_REPORT_STOP_WORDS),
            ("RUNE_DOC_REFERENCE_WORDS", RUNE_DOC_REFERENCE_WORDS),
            ("RUNE_EXPRESSION_WORDS", RUNE_EXPRESSION_WORDS),
        ] {
            unique(words, name);
        }
    }

    #[test]
    fn every_surface_keyword_is_a_real_lexer_keyword() {
        let real: BTreeSet<&str> = KEYWORDS.iter().copied().collect();
        for (name, words) in [
            ("ACTOR_KEYWORDS", ACTOR_KEYWORDS),
            ("DDD_KEYWORDS", DDD_KEYWORDS),
            ("EVT_KEYWORDS", EVT_KEYWORDS),
            ("DEPLOY_KEYWORDS", DEPLOY_KEYWORDS),
            ("UC_KEYWORDS", UC_KEYWORDS),
            ("CMMN_KEYWORDS", CMMN_KEYWORDS),
            ("DMN_KEYWORDS", DMN_KEYWORDS),
            ("INTENTS_KEYWORDS", INTENTS_KEYWORDS),
        ] {
            for word in words {
                assert!(
                    real.contains(word),
                    "{name} lists `{word}`, which is not a lexer keyword"
                );
            }
        }
    }

    #[test]
    fn every_keyword_lexes_as_exactly_one_keyword_token() {
        for keyword in KEYWORDS {
            let tokens = crate::lexer::lex(keyword).expect("keyword lexes");
            assert_eq!(
                tokens.len(),
                1,
                "`{keyword}` must be exactly one token to be a keyword"
            );
            assert_eq!(
                tokens[0].0.keyword(),
                Some(*keyword),
                "`{keyword}` is listed as a keyword but lexes otherwise"
            );
        }
    }

    #[test]
    fn real_keyword_tables_are_subsets_of_the_lexer_table() {
        let real: BTreeSet<&str> = KEYWORDS.iter().copied().collect();
        assert!(MOX_KEYWORDS.iter().all(|word| real.contains(word)));
    }

    #[test]
    fn contextual_tables_do_not_contain_real_keywords() {
        for (name, words) in [
            ("MOX_CONTEXTUAL", MOX_CONTEXTUAL),
            ("ACTOR_CONTEXTUAL", ACTOR_CONTEXTUAL),
            ("DDD_CONTEXTUAL", DDD_CONTEXTUAL),
            ("EVT_CONTEXTUAL", EVT_CONTEXTUAL),
            ("DEPLOY_CONTEXTUAL", DEPLOY_CONTEXTUAL),
            ("UC_CONTEXTUAL", UC_CONTEXTUAL),
            ("CMMN_CONTEXTUAL", CMMN_CONTEXTUAL),
            ("DMN_CONTEXTUAL", DMN_CONTEXTUAL),
            ("INTENTS_CONTEXTUAL", INTENTS_CONTEXTUAL),
        ] {
            for word in words {
                // `if` is a `.mox` keyword token and a `.ddd` statement word;
                // it is the one deliberate overlap (the `.ddd` statement
                // grammar matches the real token, while the table documents
                // the word for highlighter/text editors).
                if *word == "if" {
                    continue;
                }
                assert!(
                    !KEYWORDS.contains(word),
                    "{name} lists the real keyword `{word}` as contextual"
                );
            }
        }
    }

    /// The `pub(crate) const NAME: &str = "word";` declarations of a
    /// surface module: the parser's contextual-word authority.
    fn string_constants(source: &str) -> BTreeSet<&str> {
        source
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                let rest = line.strip_prefix("pub(crate) const ")?;
                let (_, rest) = rest.split_once(": &str = \"")?;
                let (word, _) = rest.split_once('"')?;
                Some(word)
            })
            .collect()
    }

    #[test]
    fn every_parser_contextual_constant_is_registered() {
        for (source, table) in [
            (include_str!("ddd.rs"), DDD_CONTEXTUAL),
            (include_str!("deploy.rs"), DEPLOY_CONTEXTUAL),
            (include_str!("dmn.rs"), DMN_CONTEXTUAL),
            (include_str!("usecases.rs"), UC_CONTEXTUAL),
            (include_str!("cmmn.rs"), CMMN_CONTEXTUAL),
        ] {
            for word in string_constants(source) {
                // A constant may name an existing real lexer keyword rather
                // than a contextual word — `.ddd`'s `FROM` names the
                // already-real `from` token (the DTO projection source
                // word). Those live in the surface's `*_KEYWORDS` table.
                if KEYWORDS.contains(&word) {
                    continue;
                }
                assert!(
                    table.contains(&word),
                    "parser constant `{word}` is missing from its public word table"
                );
            }
        }
    }

    /// Logos `#[token("...")]` literals that are punctuation/operators, not
    /// keywords. `->` is the deploy arrow.
    const NON_KEYWORD_TOKEN_LITERALS: &[&str] =
        &["->", ".", ",", "(", ")", "{", "}", "[", "]", "=", "=>", "*"];

    #[test]
    fn every_logos_token_literal_is_classified() {
        let source = include_str!("lexer.rs");
        let mut seen = 0usize;
        for line in source.lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("#[token(\"") else {
                continue;
            };
            let Some((literal, _)) = rest.split_once('"') else {
                continue;
            };
            seen += 1;
            assert!(
                KEYWORDS.contains(&literal) || NON_KEYWORD_TOKEN_LITERALS.contains(&literal),
                "the lexer token literal `{literal}` is neither a keyword nor a documented punctuation/operator"
            );
        }
        assert!(seen >= KEYWORDS.len(), "the scan found no token literals");
    }

    #[test]
    fn cross_surface_words_are_deliberate() {
        assert!(MOX_KEYWORDS.contains(&"container"));
        assert!(!DEPLOY_KEYWORDS.contains(&"container"));
        assert!(!DDD_KEYWORDS.contains(&"container"));
        assert!(!EVT_KEYWORDS.contains(&"container"));
        assert!(!UC_KEYWORDS.contains(&"container"));
        assert!(!CMMN_KEYWORDS.contains(&"container"));
        assert!(!DMN_KEYWORDS.contains(&"container"));
        assert!(!ACTOR_KEYWORDS.contains(&"container"));
        assert!(DDD_CONTEXTUAL.contains(&"service"));
        assert!(DMN_CONTEXTUAL.contains(&"service"));
        assert!(!MOX_CONTEXTUAL.contains(&"service"));
    }
}
