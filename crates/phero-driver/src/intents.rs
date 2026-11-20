//! The `.intent` engine (issue pherolang#196): stable-id and link
//! validation, and the intent artifact's lowering.
//!
//! The rules (diagnostics order: the file's parse diagnostics, then the
//! import errors, then each imported domain's own diagnostics, then, per
//! intent in source order, its id check, statement check, and — per link in
//! source order — kind and duplicate checks):
//!
//! - the id is the stable shape (`E0450`): one or more hyphen-separated
//!   segments, all non-empty, the first `[A-Z][A-Z0-9]*`, every later
//!   segment all-digits or `[A-Z][A-Z0-9]*`, and at least one later segment
//!   all-digits (`LOAN-001`, `REFUND-DESK-007`);
//! - ids are unique within the file (`E0451`; across files the coverage
//!   pass reports `X0014`);
//! - the statement is non-empty after trimming (`E0452`);
//! - an `element` link's kind is one of `class`, `capability`, `event`,
//!   `view`, `command`, `op`, and a `check` link's kind is one of
//!   `invariant`, `decision` (`E0453`);
//! - a slot's kind+name pair is unique per intent (`E0454`).
//!
//! Link **names** are never resolved here: the artifact carries them as
//! authored, and the project-level coverage pass (`crates/phero-driver/src/
//! coverage.rs`, the `X0009`+ band) resolves them against the application's
//! compiled artifacts. Imports resolve against raw `.mox` domains only —
//! links are not imports (the `.actor` gatherer discipline).

use phero_diagnostics::intents::{
    E_DUPLICATE_INTENT_ID, E_DUPLICATE_LINK, E_EMPTY_INTENT_STATEMENT, E_MALFORMED_INTENT_ID,
    E_UNKNOWN_LINK_KIND,
};
use phero_diagnostics::Severity;
use phero_ir::intents::{IntentDef, IntentLink, IntentLinkKind, IntentModel};
use phero_syntax::ast as dsl;
use phero_syntax::Span;

/// One span site of a compiled `.intent` file: where an intent's id and one
/// of its links sit (byte offsets into the source). Never serialized — the
/// wire artifact stays span-free; the coverage pass renders spanned
/// diagnostics from this table (the `.ifml` auth-sites discipline).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentLinkSite {
    /// The intent's stable id.
    pub intent: String,
    /// Span of the id as written.
    pub intent_span: Span,
    /// The slot the link fills.
    pub slot: dsl::IntentLinkSlot,
    /// The kind word as written (`class`, `decision`, ...).
    pub kind: String,
    /// The dotted name as written.
    pub name: String,
    /// Span of the dotted name as written.
    pub name_span: Span,
}

use crate::diagnostic::Diagnostic;
use crate::engine_common;
use crate::lower::DomainUnit;
use crate::union;

/// The stable-id shape: see the module docs. `LOAN-001` is the canonical
/// example.
fn is_stable_id(text: &str) -> bool {
    fn lead_segment(segment: &str) -> bool {
        let mut chars = segment.chars();
        matches!(chars.next(), Some(c) if c.is_ascii_uppercase())
            && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    }
    let segments: Vec<&str> = text.split('-').collect();
    if segments.len() < 2 || segments.iter().any(|segment| segment.is_empty()) {
        return false;
    }
    if !lead_segment(segments[0]) {
        return false;
    }
    let mut digit_segment = false;
    for segment in &segments[1..] {
        if segment.chars().all(|c| c.is_ascii_digit()) {
            digit_segment = true;
        } else if !lead_segment(segment) {
            return false;
        }
    }
    digit_segment
}

/// The element/check slot vocabularies, as authored words.
fn slot_kinds(slot: dsl::IntentLinkSlot) -> &'static [IntentLinkKind] {
    match slot {
        dsl::IntentLinkSlot::Element => &IntentLinkKind::ELEMENT_KINDS,
        dsl::IntentLinkSlot::Check => &IntentLinkKind::CHECK_KINDS,
    }
}

fn slot_name(slot: dsl::IntentLinkSlot) -> &'static str {
    match slot {
        dsl::IntentLinkSlot::Element => "element",
        dsl::IntentLinkSlot::Check => "check",
    }
}

/// Lowers one `.intent` file: validation per the module rules, then the
/// artifact. Returns the artifact (`None` when any error-severity
/// diagnostic was produced — the general rule), the tagged diagnostics, and
/// the span table ([`IntentLinkSite`]) the coverage pass renders spanned
/// diagnostics from.
pub(crate) fn compile_intent_file(
    path: &str,
    ast: Option<&dsl::IntentFile>,
    parse_diagnostics: &[Diagnostic],
    domains: &[DomainUnit],
) -> (
    Option<IntentModel>,
    Vec<(String, Diagnostic)>,
    Vec<IntentLinkSite>,
) {
    let imports = ast.map(|file| file.imports.as_slice()).unwrap_or_default();

    // Import hits against the one domains bucket (links are not imports).
    let hits: Vec<[bool; 1]> = imports
        .iter()
        .map(|import| {
            [domains
                .iter()
                .any(|unit| union::import_matches(&import.path, path, &unit.path))]
        })
        .collect();
    let import_diagnostics = engine_common::import_hit_diagnostics(
        imports,
        &hits,
        None,
        Some("an `.intent` file imports domains only — links are not imports"),
        |_| None,
    );

    let diags = crate::pipeline::engine_diags(path, parse_diagnostics, import_diagnostics, domains);

    let Some(ast) = ast else {
        return (None, diags, Vec::new());
    };

    // The `.intent` file's own diagnostics collect here (tagged with `path`
    // at the end), per intent in source order.
    let mut local: Vec<Diagnostic> = Vec::new();
    let mut sites: Vec<IntentLinkSite> = Vec::new();
    let mut model = IntentModel::new();
    let mut declared_ids: Vec<(String, Span)> = Vec::new();

    for decl in &ast.intents {
        // Rule 1 — the stable-id shape.
        if !is_stable_id(&decl.id.text) {
            local.push(
                Diagnostic::error(
                    format!(
                        "{} malformed intent id '{}': write an uppercase lead segment plus at \
                         least one all-digits segment (LOAN-001)",
                        E_MALFORMED_INTENT_ID, decl.id.text
                    ),
                    Some(decl.id.span),
                )
                .with_help(
                    "the id is the traceability key (#196): requirements and receipts key on it",
                ),
            );
            continue;
        }
        // Rule 2 — ids are unique within the file.
        if declared_ids.iter().any(|(id, _)| *id == decl.id.text) {
            local.push(
                Diagnostic::error(
                    format!(
                        "{} duplicate intent id '{}'",
                        E_DUPLICATE_INTENT_ID, decl.id.text
                    ),
                    Some(decl.id.span),
                )
                .with_help("renumber one of the intents so ids stay unique"),
            );
            continue;
        }
        declared_ids.push((decl.id.text.clone(), decl.id.span));

        // Rule 3 — the statement is the requirement; an empty one is refused.
        let statement = decl.statement.trim();
        if statement.is_empty() {
            local.push(
                Diagnostic::error(
                    format!(
                        "{} intent '{}' carries an empty statement",
                        E_EMPTY_INTENT_STATEMENT, decl.id.text
                    ),
                    Some(decl.statement_span),
                )
                .with_help("write the requirement as the intent's quoted statement"),
            );
            continue;
        }

        // Rule 4/5 — per link: the slot's kind vocabulary, then uniqueness
        // of the slot's kind+name pair.
        let mut lowered_links: Vec<IntentLink> = Vec::new();
        for link in &decl.links {
            let kind_word = link.kind.text.as_str();
            let Some(kind) = slot_kinds(link.slot)
                .iter()
                .copied()
                .find(|kind| kind.as_str() == kind_word)
            else {
                let allowed: Vec<&str> = slot_kinds(link.slot)
                    .iter()
                    .map(|kind| kind.as_str())
                    .collect();
                local.push(
                    Diagnostic::error(
                        format!(
                            "{} unknown {} kind '{}' in intent '{}'",
                            E_UNKNOWN_LINK_KIND,
                            slot_name(link.slot),
                            kind_word,
                            decl.id.text
                        ),
                        Some(link.kind.span),
                    )
                    .with_help(format!(
                        "the {} kinds are {}",
                        slot_name(link.slot),
                        allowed.join(", ")
                    )),
                );
                continue;
            };
            if lowered_links
                .iter()
                .any(|existing| existing.kind == kind && existing.name == link.name)
            {
                local.push(
                    Diagnostic::error(
                        format!(
                            "{} duplicate {} link '{}:{}' in intent '{}'",
                            E_DUPLICATE_LINK,
                            slot_name(link.slot),
                            kind_word,
                            link.name,
                            decl.id.text
                        ),
                        Some(link.span),
                    )
                    .with_help("remove the repeated link line — one link per element or check"),
                );
                continue;
            }
            lowered_links.push(IntentLink {
                kind,
                name: link.name.clone(),
            });
        }

        let (elements, checks): (Vec<IntentLink>, Vec<IntentLink>) =
            lowered_links.into_iter().partition(|link| {
                matches!(
                    link.kind,
                    IntentLinkKind::Class
                        | IntentLinkKind::Capability
                        | IntentLinkKind::Event
                        | IntentLinkKind::View
                        | IntentLinkKind::Command
                        | IntentLinkKind::Op
                )
            });
        // The span table records every link as written (resolved or not —
        // the unknown ones are exactly the rows the coverage pass needs
        // spans for).
        for link in &decl.links {
            sites.push(IntentLinkSite {
                intent: decl.id.text.clone(),
                intent_span: decl.id.span,
                slot: link.slot,
                kind: link.kind.text.clone(),
                name: link.name.clone(),
                name_span: link.name_span,
            });
        }
        model.intents.push(IntentDef {
            id: decl.id.text.clone(),
            statement: statement.to_string(),
            elements,
            checks,
        });
    }

    let clean = !local
        .iter()
        .chain(diags.iter().map(|(_, diagnostic)| diagnostic))
        .any(|diagnostic| diagnostic.severity == Severity::Error);
    let mut tagged = diags;
    tagged.extend(
        local
            .into_iter()
            .map(|diagnostic| (path.to_string(), diagnostic)),
    );
    let artifact = clean.then_some(model);
    (artifact, tagged, sites)
}
