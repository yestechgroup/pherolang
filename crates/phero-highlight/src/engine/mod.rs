//! The highlight engine: lexical pass → conservative contextual pass →
//! compiler-backed semantic overlays → one merged, sorted span list.
//!
//! # Never-fails contract
//!
//! [`highlight`] cannot panic or return an error. Every surface parse is the
//! fault-tolerant one; a source that does not parse still yields its lexical
//! highlights (`degraded = true`). Overlay bounds are validated, invalid spans
//! are dropped, and the final merge guarantees sorted, non-overlapping,
//! non-empty byte spans.
//!
//! # Where the semantics come from
//!
//! The semantic pass parses with the **same parser the compiler uses** and
//! walks the spanned AST. There is no second grammar, no regex keyword list,
//! and no source re-reading: the fine-grained semantics are only as good as the
//! compiler's own spans, which is the point.

pub(crate) mod lexical;
pub(crate) mod semantic;

use std::fmt;
use std::ops::Range;

use crate::class::{HighlightClass, HighlightModifiers, HighlightSpan};
use crate::metadata::{metadata_for, LexerKind};
use crate::surface::Surface;

use lexical::{LexToken, LexTokenKind, Lexed};

/// How much diagnostic detail a highlight run reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HighlightMode {
    /// Report parse/lex diagnostics and still highlight best-effort.
    Strict,
    /// Highlight best-effort and suppress diagnostics (degradation is still
    /// reported truthfully through [`HighlightResult::degraded`]).
    #[default]
    Lenient,
}

/// Options for one highlight run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightOptions {
    /// Diagnostic reporting mode.
    pub mode: HighlightMode,
    /// Whether comment spans are emitted.
    pub include_comments: bool,
}

impl Default for HighlightOptions {
    fn default() -> Self {
        Self {
            mode: HighlightMode::Lenient,
            include_comments: true,
        }
    }
}

impl HighlightOptions {
    /// Lenient mode with comments included (the default).
    #[must_use]
    pub const fn lenient() -> Self {
        Self {
            mode: HighlightMode::Lenient,
            include_comments: true,
        }
    }

    /// Strict mode with comments included.
    #[must_use]
    pub const fn strict() -> Self {
        Self {
            mode: HighlightMode::Strict,
            include_comments: true,
        }
    }

    /// Strict mode with comments omitted.
    #[must_use]
    pub const fn strict_no_comments() -> Self {
        Self {
            mode: HighlightMode::Strict,
            include_comments: false,
        }
    }
}

/// One parser diagnostic reported in [`HighlightMode::Strict`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightDiagnostic {
    /// Byte range the diagnostic refers to.
    pub span: Range<usize>,
    /// The compiler's message.
    pub message: String,
}

/// The result of one highlight run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightResult {
    /// Sorted, non-overlapping byte spans.
    pub spans: Vec<HighlightSpan>,
    /// `true` when the compiler could not parse the source cleanly; the spans
    /// are then lexical plus whatever partial AST the parser recovered.
    pub degraded: bool,
    /// Parse/lex diagnostics; empty in [`HighlightMode::Lenient`].
    pub diagnostics: Vec<HighlightDiagnostic>,
}

/// Highlights `source` for `surface`.
///
/// The returned spans are in source **byte** offsets, sorted, non-overlapping,
/// non-empty, and in bounds. Whitespace and any uncovered gap stays implicit.
#[must_use]
pub fn highlight(source: &str, surface: Surface, options: HighlightOptions) -> HighlightResult {
    let lexed = lexical::lex_surface(surface, source);
    let contextual = contextual_overlays(surface, source, &lexed.tokens);

    let mut ctx = semantic::Ctx::new(source, surface, &lexed.tokens);
    let (mut degraded, mut diagnostics) = semantic_pass(surface, source, &mut ctx);
    let semantic = std::mem::take(&mut ctx.out);
    drop(ctx);

    let lexical_ok = lexed.error_count_is_zero();
    let mut spans = assemble(source, lexed.spans, contextual, semantic, &lexed.protected);

    if !options.include_comments {
        spans.retain(|span| span.class != HighlightClass::Comment);
    }

    if options.mode == HighlightMode::Strict {
        // Lexical error regions are diagnostics too; the parse errors come
        // first so the ordering stays declaration-before-token.
        let mut lexical: Vec<HighlightDiagnostic> = spans
            .iter()
            .filter(|span| span.class == HighlightClass::Error)
            .map(|span| HighlightDiagnostic {
                span: span.start..span.end,
                message: "invalid token".to_string(),
            })
            .collect();
        diagnostics.append(&mut lexical);
        diagnostics.sort_by_key(|diagnostic| (diagnostic.span.start, diagnostic.span.end));
    } else {
        diagnostics.clear();
    }

    if !lexical_ok {
        degraded = true;
    }

    HighlightResult {
        spans,
        degraded,
        diagnostics,
    }
}

impl Lexed<'_> {
    /// `true` when no lexical error regions were produced.
    fn error_count_is_zero(&self) -> bool {
        !self
            .spans
            .iter()
            .any(|span| span.class == HighlightClass::Error)
    }
}

/// Runs the surface's compiler parser and walks the recovered AST.
fn semantic_pass(
    surface: Surface,
    source: &str,
    ctx: &mut semantic::Ctx<'_>,
) -> (bool, Vec<HighlightDiagnostic>) {
    match surface {
        Surface::Mox => {
            let result = phero_syntax::parse(source);
            if let Some(ast) = &result.ast {
                semantic::mox::walk(ctx, ast);
            }
            parse_degradation(result.ast.is_some(), &result.errors)
        }
        Surface::Actor => {
            let result = phero_syntax::parse_actors(source);
            if let Some(ast) = &result.ast {
                semantic::actor::walk(ctx, ast);
            }
            parse_degradation(result.ast.is_some(), &result.errors)
        }
        Surface::Ddd => {
            let result = phero_syntax::parse_ddd(source);
            if let Some(ast) = &result.ast {
                semantic::ddd::walk(ctx, ast);
            }
            parse_degradation(result.ast.is_some(), &result.errors)
        }
        Surface::Evt => {
            let result = phero_syntax::parse_evt(source);
            if let Some(ast) = &result.ast {
                semantic::evt::walk(ctx, ast);
            }
            parse_degradation(result.ast.is_some(), &result.errors)
        }
        Surface::Deploy => {
            let result = phero_syntax::parse_deploy(source);
            if let Some(ast) = &result.ast {
                semantic::deploy::walk(ctx, ast);
            }
            parse_degradation(result.ast.is_some(), &result.errors)
        }
        Surface::Uc => {
            let result = phero_syntax::parse_usecases(source);
            if let Some(ast) = &result.ast {
                semantic::uc::walk(ctx, ast);
            }
            parse_degradation(result.ast.is_some(), &result.errors)
        }
        Surface::Cmmn => {
            let result = phero_syntax::parse_cmmn(source);
            if let Some(ast) = &result.ast {
                semantic::cmmn::walk(ctx, ast);
            }
            parse_degradation(result.ast.is_some(), &result.errors)
        }
        Surface::Dmn => {
            let result = phero_syntax::parse_dmn(source);
            if let Some(ast) = &result.ast {
                semantic::dmn::walk(ctx, ast);
            }
            parse_degradation(result.ast.is_some(), &result.errors)
        }
        Surface::Ifml => {
            let (ast, errors) = phero_ifml::parse_ifml_syntax_recovering(source);
            if let Some(ast) = &ast {
                semantic::ifml::walk(ctx, ast);
            }
            let found_ast = ast.is_some();
            let mut diagnostics = Vec::with_capacity(errors.len());
            for error in errors {
                diagnostics.push(HighlightDiagnostic {
                    span: error.span.start..error.span.end,
                    message: error.message,
                });
            }
            (!found_ast || !diagnostics.is_empty(), diagnostics)
        }
    }
}

fn parse_degradation(
    found_ast: bool,
    errors: &[phero_syntax::ParseError],
) -> (bool, Vec<HighlightDiagnostic>) {
    let diagnostics = errors
        .iter()
        .map(|error| HighlightDiagnostic {
            span: error.span.start..error.span.end,
            message: error.message.clone(),
        })
        .collect();
    (!found_ast || !errors.is_empty(), diagnostics)
}

/// The conservative contextual-keyword pass: an identifier in the surface's
/// contextual table becomes a keyword only in a recognizable grammar position.
fn contextual_overlays(
    surface: Surface,
    source: &str,
    tokens: &[LexToken<'_>],
) -> Vec<HighlightSpan> {
    if metadata_for(surface).lexer == LexerKind::Ifml {
        // `.ifml` has no contextual table: its structural words are already
        // keyword-classified by the lexical pass.
        return Vec::new();
    }
    let contextual = metadata_for(surface).contextual;
    let mut overlays = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let LexTokenKind::Ident { text, escaped } = &token.kind else {
            continue;
        };
        if *escaped || !contextual.contains(text) {
            continue;
        }
        if contextual_position(source, tokens, index, text) {
            overlays.push(HighlightSpan::new(
                token.start,
                token.end,
                HighlightClass::Keyword,
            ));
        }
    }
    overlays
}

/// Word pairs where the left word licenses the right contextual word.
const PAIRS: &[(&str, &str)] = &[
    ("from", "to"),
    ("permit", "human"),
    ("forbid", "human"),
    ("inject", "decision"),
    ("logic", "table"),
    ("logic", "expression"),
    ("logic", "invoke"),
    ("version", "compatible"),
    ("first", "then"),
    ("first", "start"),
    ("first", "done"),
    ("then", "start"),
    ("then", "done"),
    ("message", "of"),
    ("subject", "of"),
    ("role", "as"),
    ("import", "as"),
    ("import", "schema"),
    ("import", "sigil"),
    ("use", "as"),
    ("id", "readonly"),
    ("readonly", "id"),
    ("deployment", "for"),
    ("application", "for"),
    ("abstract", "entity"),
    ("abstract", "value"),
    ("abstract", "dto"),
    ("discretionary", "task"),
    ("discretionary", "service"),
    ("discretionary", "decision"),
];

/// Clause words that are contextual only when they follow a completed
/// signature (`)`) or a reference word.
const AFTER_IDENT_CLAUSE_WORDS: &[&str] = &[
    "emits",
    "invokes",
    "transition",
    "compatible",
    "table",
    "expression",
    "invoke",
    "of",
    "to",
    "from",
    "then",
];

fn contextual_position(source: &str, tokens: &[LexToken<'_>], index: usize, word: &str) -> bool {
    if token_starts_line(source, tokens[index].start) {
        return true;
    }
    let Some(previous) = index.checked_sub(1).and_then(|i| tokens.get(i)) else {
        return false;
    };
    if matches!(
        previous.kind,
        LexTokenKind::OpenBrace | LexTokenKind::CloseBrace | LexTokenKind::Semi
    ) {
        return true;
    }
    if let Some(previous_word) = previous.word() {
        if PAIRS
            .iter()
            .any(|(left, right)| *left == previous_word && *right == word)
        {
            return true;
        }
        if AFTER_IDENT_CLAUSE_WORDS.contains(&word)
            && matches!(
                previous.kind,
                LexTokenKind::Ident { .. } | LexTokenKind::Keyword(_)
            )
        {
            return true;
        }
    }
    if matches!(previous.kind, LexTokenKind::RParen) && AFTER_IDENT_CLAUSE_WORDS.contains(&word) {
        return true;
    }
    // A pair whose left side is the first word of the line: `deployment prod
    // for App`, `message OrderPlaced of Order`.
    if let Some(head) = first_token_on_line(source, tokens, index) {
        if let Some(head_word) = head.word() {
            if PAIRS
                .iter()
                .any(|(left, right)| *left == head_word && *right == word)
            {
                return true;
            }
        }
    }
    false
}

/// `true` when only whitespace precedes `start` on its line.
fn token_starts_line(source: &str, start: usize) -> bool {
    let bytes = source.as_bytes();
    let mut index = start;
    while index > 0 {
        index -= 1;
        match bytes[index] {
            b' ' | b'\t' | b'\r' | b'\x0c' => continue,
            b'\n' => return true,
            _ => return false,
        }
    }
    true
}

/// The first code token on the same line as `tokens[index]`.
fn first_token_on_line<'a, 'b>(
    source: &str,
    tokens: &'b [LexToken<'a>],
    index: usize,
) -> Option<&'b LexToken<'a>> {
    let start = tokens[index].start;
    let line_start = source[..start]
        .rfind('\n')
        .map_or(0, |position| position + 1);
    tokens
        .iter()
        .find(|token| token.start >= line_start && token.start <= start)
}

/// One priority-layer piece used by the boundary sweep.
struct Piece {
    start: usize,
    end: usize,
    class: HighlightClass,
    modifiers: HighlightModifiers,
    layer: u8,
    order: usize,
}

/// Merges the three layers into the final span list.
fn assemble(
    source: &str,
    base: Vec<HighlightSpan>,
    contextual: Vec<HighlightSpan>,
    semantic: Vec<HighlightSpan>,
    protected: &[(usize, usize)],
) -> Vec<HighlightSpan> {
    let mut pieces: Vec<Piece> = Vec::new();
    let push_layer = |spans: Vec<HighlightSpan>, layer: u8, pieces: &mut Vec<Piece>| {
        for (order, span) in spans.into_iter().enumerate() {
            if let Some(span) = sanitize(source, span) {
                pieces.push(Piece {
                    start: span.start,
                    end: span.end,
                    class: span.class,
                    modifiers: span.modifiers,
                    layer,
                    order,
                });
            }
        }
    };
    push_layer(base, 0, &mut pieces);
    push_layer(contextual, 1, &mut pieces);

    let mut semantic_order = 0;
    for span in semantic {
        let Some(span) = sanitize(source, span) else {
            continue;
        };
        if !allowed_overlay(protected, span.start, span.end) {
            continue;
        }
        pieces.push(Piece {
            start: span.start,
            end: span.end,
            class: span.class,
            modifiers: span.modifiers,
            layer: 2,
            order: semantic_order,
        });
        semantic_order += 1;
    }

    pieces.sort_by_key(|piece| piece.start);
    let mut points: Vec<usize> = Vec::with_capacity(pieces.len() * 2);
    for piece in &pieces {
        points.push(piece.start);
        points.push(piece.end);
    }
    points.sort_unstable();
    points.dedup();

    let mut out: Vec<HighlightSpan> = Vec::new();
    let mut active: Vec<&Piece> = Vec::new();
    let mut next = 0;
    for window in points.windows(2) {
        let (left, right) = (window[0], window[1]);
        while next < pieces.len() && pieces[next].start <= left {
            active.push(&pieces[next]);
            next += 1;
        }
        active.retain(|piece| piece.end > left);
        let Some(top) = active.iter().max_by_key(|piece| (piece.layer, piece.order)) else {
            continue;
        };
        let span = HighlightSpan::with_modifiers(left, right, top.class, top.modifiers);
        if let Some(last) = out.last_mut() {
            if last.end == span.start
                && last.class == span.class
                && last.modifiers == span.modifiers
            {
                last.end = span.end;
                continue;
            }
        }
        out.push(span);
    }
    out
}

/// Validates bounds, ordering, non-emptiness, and char boundaries.
fn sanitize(source: &str, span: HighlightSpan) -> Option<HighlightSpan> {
    if span.start >= span.end || span.end > source.len() {
        return None;
    }
    if !source.is_char_boundary(span.start) || !source.is_char_boundary(span.end) {
        return None;
    }
    Some(span)
}

/// An overlay may not cut through a comment or string token; an exact match is
/// allowed (that is how `.ifml` names replace their atomic string).
fn allowed_overlay(protected: &[(usize, usize)], start: usize, end: usize) -> bool {
    let first = protected.partition_point(|(_, protected_end)| *protected_end <= start);
    for (protected_start, protected_end) in &protected[first..] {
        if *protected_start >= end {
            break;
        }
        if *protected_start != start || *protected_end != end {
            return false;
        }
    }
    true
}

impl fmt::Display for HighlightMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Strict => "strict",
            Self::Lenient => "lenient",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::class::HighlightModifier;

    fn classes(source: &str, surface: Surface) -> Vec<(usize, usize, HighlightClass)> {
        highlight(source, surface, HighlightOptions::default())
            .spans
            .into_iter()
            .map(|span| (span.start, span.end, span.class))
            .collect()
    }

    fn find<'a>(source: &'a str, spans: &'a [HighlightSpan], text: &str) -> Vec<&'a HighlightSpan> {
        spans
            .iter()
            .filter(|span| source.get(span.start..span.end) == Some(text))
            .collect()
    }

    fn assert_invariants(source: &str, spans: &[HighlightSpan]) {
        let mut previous_end = 0;
        for span in spans {
            assert!(
                span.start < span.end,
                "empty span at {}..{}",
                span.start,
                span.end
            );
            assert!(span.end <= source.len(), "out-of-bounds span");
            assert!(span.start >= previous_end, "overlapping or unsorted spans");
            assert!(source.is_char_boundary(span.start));
            assert!(source.is_char_boundary(span.end));
            previous_end = span.end;
        }
    }

    #[test]
    fn mox_semantics_cover_declarations_types_operations_and_parameters() {
        let source = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/support/support.mox"
        ))
        .expect("example source");
        let result = highlight(&source, Surface::Mox, HighlightOptions::default());
        assert!(
            !result.degraded,
            "the example parses: {:?}",
            result.diagnostics
        );
        assert_invariants(&source, &result.spans);

        let ticket = find(&source, &result.spans, "Ticket");
        assert!(ticket.iter().any(|span| span.class == HighlightClass::Type
            && span.modifiers.contains(HighlightModifier::Declaration)));
        assert!(ticket.iter().any(|span| span.class == HighlightClass::Type
            && !span.modifiers.contains(HighlightModifier::Declaration)));

        let operation = find(&source, &result.spans, "raiseRefund");
        assert!(operation
            .iter()
            .any(|span| span.class == HighlightClass::Function
                && span.modifiers.contains(HighlightModifier::Declaration)));

        let parameter = find(&source, &result.spans, "cents");
        assert!(!parameter.is_empty());
        assert!(parameter
            .iter()
            .any(|span| span.class == HighlightClass::Parameter
                && span.modifiers.contains(HighlightModifier::Declaration)));

        let feature = find(&source, &result.spans, "refundCents");
        assert!(feature
            .iter()
            .any(|span| span.class == HighlightClass::Property));

        let readonly = find(&source, &result.spans, "cents")
            .into_iter()
            .any(|span| span.modifiers.contains(HighlightModifier::Readonly));
        assert!(readonly, "the readonly feature keeps its modifier");
    }

    #[test]
    fn comments_strings_escapes_and_numbers_are_lexical() {
        let source = "// note\nclass A {\n  String s = \"a\\tb\"\n  int n = 42\n}";
        let result = highlight(source, Surface::Mox, HighlightOptions::default());
        assert_invariants(source, &result.spans);
        let spans = &result.spans;
        assert_eq!(
            find(source, spans, "// note")[0].class,
            HighlightClass::Comment
        );
        let escape = spans
            .iter()
            .find(|span| span.class == HighlightClass::Escape)
            .expect("escape span");
        assert_eq!(&source[escape.start..escape.end], "\\t");
        assert_eq!(find(source, spans, "42")[0].class, HighlightClass::Number);
        assert!(find(source, spans, "\"a")
            .iter()
            .any(|span| span.class == HighlightClass::String));
        assert!(find(source, spans, "b\"")
            .iter()
            .any(|span| span.class == HighlightClass::String));
    }

    #[test]
    fn doc_comments_carry_the_documentation_modifier() {
        let source = "/// docs\nclass A";
        let result = highlight(source, Surface::Mox, HighlightOptions::default());
        let comment = result
            .spans
            .iter()
            .find(|span| span.class == HighlightClass::Comment)
            .expect("comment");
        assert!(comment.modifiers.contains(HighlightModifier::Documentation));
    }

    #[test]
    fn comments_can_be_omitted() {
        let source = "// gone\nclass A";
        let result = highlight(
            source,
            Surface::Mox,
            HighlightOptions {
                include_comments: false,
                ..HighlightOptions::default()
            },
        );
        assert!(!result
            .spans
            .iter()
            .any(|span| span.class == HighlightClass::Comment));
        assert!(find(source, &result.spans, "class").len() == 1);
    }

    #[test]
    fn contextual_keywords_are_recognized_in_grammar_positions() {
        let ddd = "import \"x\";\napplication A {\n  base b\n  module m {\n    service S {\n      inject decision Foo;\n      abstract entity E;\n    }\n  }\n}";
        let result = highlight(ddd, Surface::Ddd, HighlightOptions::default());
        for word in ["service", "abstract", "entity", "decision"] {
            assert!(
                find(ddd, &result.spans, word)
                    .iter()
                    .any(|span| span.class == HighlightClass::Keyword),
                "`{word}` should be a contextual keyword"
            );
        }

        let dmn = "decision_model D {\n  logic table \"x.md\"\n}";
        let result = highlight(dmn, Surface::Dmn, HighlightOptions::default());
        for word in ["decision_model", "logic", "table"] {
            assert!(find(dmn, &result.spans, word)
                .iter()
                .any(|span| span.class == HighlightClass::Keyword));
        }

        let uc = "usecase Checkout {\n  subject cashier: User;\n  perform authorise;\n  realizes payments.TakePayment.charge;\n  first start then done;\n}";
        let result = highlight(uc, Surface::Uc, HighlightOptions::default());
        for word in [
            "usecase", "subject", "perform", "realizes", "start", "then", "done",
        ] {
            assert!(find(uc, &result.spans, word)
                .iter()
                .any(|span| span.class == HighlightClass::Keyword));
        }
    }

    #[test]
    fn contextual_words_stay_variables_in_value_positions() {
        let deploy = "import \"a.ddd\";\napplication A {\n  component api: api {\n    runtime: container\n  }\n}";
        let result = highlight(deploy, Surface::Deploy, HighlightOptions::default());
        let container = find(deploy, &result.spans, "container");
        assert_eq!(container.len(), 1);
        assert_ne!(container[0].class, HighlightClass::Keyword);
        assert_eq!(container[0].class, HighlightClass::Constant);
    }

    #[test]
    fn ifml_names_are_atomic_and_classified_inside_strings() {
        let source = "view \"Inbox\" {\n    params { tickets: Ticket };\n    component \"escalations\" {\n        type: list;\n        data: Ticket;\n    }\n}";
        let result = highlight(source, Surface::Ifml, HighlightOptions::default());
        assert_invariants(source, &result.spans);
        let view = find(source, &result.spans, "\"Inbox\"");
        assert_eq!(view.len(), 1, "the whole string token is one span");
        assert_eq!(view[0].class, HighlightClass::Type);
        assert!(view[0].modifiers.contains(HighlightModifier::Declaration));
        let component = find(source, &result.spans, "\"escalations\"");
        assert_eq!(component.len(), 1);
        assert_eq!(component[0].class, HighlightClass::Type);
        let tickets = find(source, &result.spans, "tickets");
        assert!(!tickets.is_empty());
        assert!(tickets
            .iter()
            .any(|span| span.class == HighlightClass::Parameter));
        let data = find(source, &result.spans, "data");
        assert!(data
            .iter()
            .any(|span| span.class == HighlightClass::Property));
    }

    #[test]
    fn truncated_and_garbage_sources_never_fail_and_report_degradation() {
        let fragments = [
            "",
            "class",
            "class A {",
            ")))) $$ %%",
            "\"unterminated",
            "view \"X\" {",
            "decision_model",
            "\u{1f600} \u{1f600}",
        ];
        for surface in Surface::ALL {
            for fragment in fragments {
                let result = highlight(fragment, surface, HighlightOptions::default());
                assert_invariants(fragment, &result.spans);
                assert!(
                    result.diagnostics.is_empty(),
                    "lenient suppresses diagnostics"
                );
            }
        }
        let result = highlight("class A {", Surface::Mox, HighlightOptions::strict());
        assert!(result.degraded);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn highlighting_is_idempotent() {
        let source = "class A { op int f(int x) { expr { x } } }";
        let first = highlight(source, Surface::Mox, HighlightOptions::default());
        let second = highlight(source, Surface::Mox, HighlightOptions::default());
        assert_eq!(first, second);
        assert_eq!(classes(source, Surface::Mox), classes(source, Surface::Mox));
    }

    #[test]
    fn an_empty_source_has_no_spans_and_is_not_degraded_for_a_full_parse() {
        let result = highlight("", Surface::Mox, HighlightOptions::default());
        assert!(result.spans.is_empty());
        assert!(!result.degraded);
    }
}
