//! `pherolang docs lint`: the documentation quality gate.
//!
//! The contract is `docs/documentation/CONTENT_MODEL.md` (structure and
//! frontmatter) and `docs/documentation/EDITORIAL_POLICY.md` (the normative
//! rules); this module is the `[CI]` half of that contract. It is deliberately
//! compiler-backed: every `mox` fence in the Guide tree is compiled through the
//! same entry point `pherolang check` uses, so a documentation example cannot
//! teach syntax the compiler rejects.
//!
//! The gates, in the order they are documented:
//!
//! | Gate | Rule ids | What it checks |
//! |---|---|---|
//! | content model | `CM1`–`CM8`, `IA1`–`IA3` | frontmatter present, closed vocabularies, required fields, heading sets, title agreement, prerequisite paths |
//! | examples | `CM10`–`CM11a`, `E11`–`E12` | fence classification, compilation, `mox,invalid=<CODE>` assertions, self-containment |
//! | vocabulary | `CM12`, `E5` | implementation vocabulary absent from reader-facing pages |
//! | history | `CM13`, `E16` | no issue numbers in Guide prose |
//! | terminology | `CM14`, `E6`–`E7` | canonical terms, banned aliases, defined acronyms |
//! | coverage | `CM7` | every documented language has an overview |
//! | templates | — | each template's heading sequence still matches its exemplar |
//!
//! Usage: `pherolang docs lint [paths...] [--glossary <file>]`, run from the
//! repository root. Paths default to `docs/guide`; the template drift check
//! always reads `docs/documentation/templates` and the exemplars under
//! `docs/guide/mox`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{bail, Context};
use clap::Subcommand;

/// The `docs` namespace. One verb for now: `lint`.
#[derive(Debug, Subcommand)]
pub(crate) enum DocsCommand {
    /// Lint documentation pages against the content model and editorial
    /// policy; exits `1` on any finding.
    Lint {
        /// Pages or directories to lint (default: `docs/guide`, and the
        /// template drift check under `docs/documentation/templates`).
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,
        /// Glossary source (default: `docs/documentation/glossary.yaml`).
        #[arg(long, value_name = "FILE")]
        glossary: Option<PathBuf>,
    },
}

/// Runs one `docs` subcommand.
pub(crate) fn run(action: DocsCommand) -> anyhow::Result<ExitCode> {
    match action {
        DocsCommand::Lint { paths, glossary } => lint(paths, glossary),
    }
}

// ---------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------

/// One lint finding: where, which rule, and what to do about it.
#[derive(Debug)]
struct Finding {
    path: PathBuf,
    line: usize,
    rule: &'static str,
    message: String,
}

/// The page facts the cross-page checks need.
struct PageFacts {
    path: PathBuf,
    page_type: String,
    language: String,
}

const REQUIRED_FIELDS: [&str; 10] = [
    "title",
    "description",
    "type",
    "language",
    "section",
    "order",
    "status",
    "docStatus",
    "audience",
    "lastReviewed",
];

const ALLOWED_FIELDS: [&str; 12] = [
    "title",
    "description",
    "type",
    "language",
    "section",
    "order",
    "status",
    "docStatus",
    "audience",
    "prerequisites",
    "related",
    "lastReviewed",
];

const PAGE_TYPES: [&str; 6] = [
    "overview",
    "tutorial",
    "how-to",
    "concept",
    "reference",
    "integration",
];

const LANGUAGES: [&str; 10] = [
    "mox", "ddd", "ifml", "dmn", "cmmn", "evt", "deploy", "uc", "intent", "platform",
];

const STATUSES: [&str; 4] = ["experimental", "draft", "stable", "deprecated"];
const DOC_STATUSES: [&str; 3] = ["stub", "draft", "complete"];

const OVERVIEW_HEADINGS: [&str; 7] = [
    "What is it?",
    "What problems does it solve?",
    "Key capabilities",
    "A simple example",
    "How it fits into Phero Lang",
    "Getting started",
    "Further reading",
];

const HOW_TO_HEADINGS: [&str; 5] = [
    "Problem",
    "Prerequisites",
    "Solution",
    "Example",
    "Related reference",
];

const CONCEPT_HEADINGS: [&str; 6] = [
    "Overview",
    "Why does it matter?",
    "How it works",
    "Example",
    "Relationship to other concepts",
    "Further reading",
];

const REFERENCE_HEADINGS: [&str; 6] = [
    "Syntax",
    "Semantics",
    "Constraints",
    "Examples",
    "Diagnostics",
    "Compatibility",
];

const INTEGRATION_HEADINGS: [&str; 4] = [
    "Contract summary",
    "Wire format",
    "Versioning",
    "Consumer checklist",
];

const TUTORIAL_PREFIX: [&str; 3] = ["What you'll learn", "Prerequisites", "What we're building"];
const TUTORIAL_SUFFIX: [&str; 3] = ["Complete example", "Expected result", "Next steps"];

/// `templates/<type>.md` → its exemplar in the Guide tree.
const TEMPLATE_EXEMPLARS: [(&str, &str); 5] = [
    ("overview", "docs/guide/mox/overview.md"),
    (
        "tutorial",
        "docs/guide/mox/tutorials/your-first-domain-model.md",
    ),
    ("how-to", "docs/guide/mox/how-to/add-an-enum.md"),
    ("concept", "docs/guide/mox/concepts/modelling-not-coding.md"),
    ("reference", "docs/guide/mox/reference/classes.md"),
];

fn lint(paths: Vec<PathBuf>, glossary: Option<PathBuf>) -> anyhow::Result<ExitCode> {
    let root = std::env::current_dir().context("cannot read the current directory")?;

    let glossary_path = glossary.unwrap_or_else(|| root.join("docs/documentation/glossary.yaml"));
    let glossary_text = fs::read_to_string(&glossary_path).with_context(|| {
        format!(
            "cannot read the glossary at {} — run from the repository root",
            glossary_path.display()
        )
    })?;
    let glossary = parse_glossary(&glossary_text).map_err(|message| {
        anyhow::anyhow!("invalid glossary {}: {message}", glossary_path.display())
    })?;

    let (scanned, default_guide) = if paths.is_empty() {
        (vec![root.join("docs/guide")], true)
    } else {
        (paths, false)
    };

    let mut findings = Vec::new();
    let mut pages = Vec::new();

    for path in &scanned {
        if !path.exists() {
            bail!("documentation path does not exist: {}", path.display());
        }
        let mut files = Vec::new();
        collect_markdown(path, &mut files)?;
        if files.is_empty() && default_guide {
            bail!("no documentation pages under {}", path.display());
        }
        for file in files {
            if let Some(facts) = analyze_page(&root, &file, &glossary, &mut findings) {
                pages.push(facts);
            }
        }
    }

    check_language_coverage(&pages, &mut findings);
    check_template_drift(&root, &mut findings)?;

    findings.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));

    let compiled = findings_compiled_count();
    if findings.is_empty() {
        println!(
            "docs-lint: OK — {} page(s), {compiled} example(s) compiled",
            pages.len()
        );
        return Ok(ExitCode::SUCCESS);
    }

    for finding in &findings {
        eprintln!(
            "{}:{}: {}: {}",
            finding.path.display(),
            finding.line,
            finding.rule,
            finding.message
        );
    }
    eprintln!(
        "docs-lint: FAIL — {} finding(s) across {} page(s)",
        findings.len(),
        pages.len()
    );
    Ok(ExitCode::FAILURE)
}

/// Number of complete examples compiled during the run (a process-global so
/// the reporting tail can stay a plain function; the gate is single-threaded).
fn findings_compiled_count() -> usize {
    COMPILED.with(|count| count.get())
}

thread_local! {
    static COMPILED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn bump_compiled() {
    COMPILED.with(|count| count.set(count.get() + 1));
}

fn collect_markdown(path: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    if path.is_file() {
        if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path.to_path_buf());
        }
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(path)
        .with_context(|| format!("cannot read directory {}", path.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()?;
    entries.sort();
    for entry in entries {
        collect_markdown(&entry, out)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Per-page analysis
// ---------------------------------------------------------------------------

fn analyze_page(
    root: &Path,
    path: &Path,
    glossary: &Glossary,
    findings: &mut Vec<Finding>,
) -> Option<PageFacts> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            findings.push(Finding {
                path: path.to_path_buf(),
                line: 1,
                rule: "CM0",
                message: format!("cannot read page: {error}"),
            });
            return None;
        }
    };

    let (frontmatter, body, body_start) = match split_frontmatter(&source) {
        Ok(parsed) => parsed,
        Err(message) => {
            findings.push(Finding {
                path: path.to_path_buf(),
                line: message.line,
                rule: "CM2",
                message: message.message,
            });
            return None;
        }
    };

    let facts = check_frontmatter(root, path, &frontmatter, body, body_start, findings);

    let structure = scan_structure(body, body_start);
    let page_type = facts.as_ref().map(|facts| facts.page_type.clone());
    if let Some(page_type) = page_type.as_deref() {
        check_headings(path, page_type, &structure.headings, findings);
    }
    check_fences(root, path, &structure, findings);

    let prose = prose_lines(body, body_start, &structure);
    if let Some(page_type) = page_type.as_deref() {
        if matches!(page_type, "overview" | "tutorial" | "how-to" | "concept") {
            check_implementation_vocabulary(path, page_type, &prose, glossary, findings);
        }
    }
    check_history(path, &prose, findings);
    check_terminology(path, &prose, glossary, page_type.as_deref(), findings);

    facts
}

type Frontmatter = BTreeMap<String, FieldValue>;

#[derive(Debug, Clone, PartialEq, Eq)]
enum FieldValue {
    Scalar(String),
    List(Vec<String>),
}

impl FieldValue {
    fn scalar(&self) -> Option<&str> {
        match self {
            FieldValue::Scalar(value) => Some(value),
            FieldValue::List(_) => None,
        }
    }

    fn list(&self) -> Option<&[String]> {
        match self {
            FieldValue::List(values) => Some(values),
            FieldValue::Scalar(_) => None,
        }
    }
}

#[derive(Debug)]
struct FrontmatterError {
    line: usize,
    message: String,
}

/// Splits `---` frontmatter from the body. Returns the fields, the body, and
/// the 1-based line number of the body's first line.
fn split_frontmatter(source: &str) -> Result<(Frontmatter, &str, usize), FrontmatterError> {
    let lines: Vec<&str> = source.split('\n').collect();
    if lines.first().is_none_or(|line| line.trim_end() != "---") {
        return Err(FrontmatterError {
            line: 1,
            message: "missing frontmatter: every Guide page starts with a `---` block \
                      (docs/documentation/CONTENT_MODEL.md §1)"
                .to_string(),
        });
    }
    let end = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, line)| line.trim_end() == "---")
        .map(|(index, _)| index)
        .ok_or_else(|| FrontmatterError {
            line: 1,
            message: "unterminated frontmatter: no closing `---` line".to_string(),
        })?;

    let mut fields = Frontmatter::new();
    for (index, line) in lines.iter().enumerate().take(end).skip(1) {
        let number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') || trimmed.starts_with('-') {
            return Err(FrontmatterError {
                line: number,
                message: "unsupported frontmatter form: only top-level `key: value` and \
                          inline `[a, b]` lists are allowed (CONTENT_MODEL.md §1)"
                    .to_string(),
            });
        }
        let Some((key, raw)) = trimmed.split_once(':') else {
            return Err(FrontmatterError {
                line: number,
                message: format!("frontmatter line `{trimmed}` is not `key: value`"),
            });
        };
        let key = key.trim().to_string();
        if fields.contains_key(&key) {
            return Err(FrontmatterError {
                line: number,
                message: format!("duplicate frontmatter field `{key}`"),
            });
        }
        fields.insert(key, parse_field_value(raw.trim()));
    }

    // The body is the source from the byte after the closing `---` line.
    let mut offset = 0usize;
    let mut body_offset = source.len();
    for (index, line) in lines.iter().enumerate() {
        let next = (offset + line.len() + 1).min(source.len());
        if index == end {
            body_offset = next;
            break;
        }
        offset = next;
    }
    let body = &source[body_offset..];
    Ok((fields, body, end + 2))
}

fn parse_field_value(raw: &str) -> FieldValue {
    if raw.starts_with('[') && raw.ends_with(']') {
        FieldValue::List(split_inline_list(&raw[1..raw.len() - 1]))
    } else {
        FieldValue::Scalar(unquote(raw))
    }
}

fn unquote(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= 2 {
        let bytes = trimmed.as_bytes();
        let first = bytes[0];
        let last = bytes[trimmed.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    trimmed.to_string()
}

fn split_inline_list(inner: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in inner.chars() {
        match quote {
            Some(open) => {
                if ch == open {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            None => match ch {
                '"' | '\'' => quote = Some(ch),
                ',' => {
                    let item = current.trim();
                    if !item.is_empty() {
                        items.push(item.to_string());
                    }
                    current.clear();
                }
                _ => current.push(ch),
            },
        }
    }
    let item = current.trim();
    if !item.is_empty() {
        items.push(item.to_string());
    }
    items
}

fn check_frontmatter(
    root: &Path,
    path: &Path,
    frontmatter: &Frontmatter,
    body: &str,
    _body_start: usize,
    findings: &mut Vec<Finding>,
) -> Option<PageFacts> {
    let line = 1;
    let mut ok = true;

    let mut push = |rule: &'static str, message: String| {
        findings.push(Finding {
            path: path.to_path_buf(),
            line,
            rule,
            message,
        });
    };

    for key in frontmatter.keys() {
        if !ALLOWED_FIELDS.contains(&key.as_str()) {
            push(
                "CM2",
                format!("unknown frontmatter field `{key}` — extend CONTENT_MODEL.md first"),
            );
            ok = false;
        }
    }
    for field in REQUIRED_FIELDS {
        if !frontmatter.contains_key(field) {
            push(
                "CM2",
                format!("missing required frontmatter field `{field}`"),
            );
            ok = false;
        }
    }

    let scalar = |field: &str| {
        frontmatter
            .get(field)
            .and_then(FieldValue::scalar)
            .map(str::to_string)
    };
    let list = |field: &str| {
        frontmatter
            .get(field)
            .and_then(FieldValue::list)
            .map(<[String]>::to_vec)
    };

    let page_type = scalar("type");
    if let Some(value) = &page_type {
        if !PAGE_TYPES.contains(&value.as_str()) {
            push(
                "CM1",
                format!("`type: {value}` is not one of {}", PAGE_TYPES.join(", ")),
            );
            ok = false;
        }
    }
    let language = scalar("language");
    if let Some(value) = &language {
        if !LANGUAGES.contains(&value.as_str()) {
            push(
                "CM1",
                format!("`language: {value}` is not one of {}", LANGUAGES.join(", ")),
            );
            ok = false;
        }
    }
    if let Some(value) = scalar("status") {
        if !STATUSES.contains(&value.as_str()) {
            push(
                "CM1",
                format!("`status: {value}` is not one of {}", STATUSES.join(", ")),
            );
            ok = false;
        }
    }
    if let Some(value) = scalar("docStatus") {
        if !DOC_STATUSES.contains(&value.as_str()) {
            push(
                "CM1",
                format!(
                    "`docStatus: {value}` is not one of {}",
                    DOC_STATUSES.join(", ")
                ),
            );
            ok = false;
        }
    }
    if let Some(value) = scalar("section") {
        if !is_kebab_case(&value) {
            push(
                "CM1",
                format!("`section: {value}` must be kebab-case (for example `language-semantics`)"),
            );
            ok = false;
        }
    }
    if let Some(value) = scalar("order") {
        if value.parse::<i64>().is_err() {
            push("CM2", format!("`order: {value}` must be an integer"));
            ok = false;
        }
    }
    if let Some(value) = scalar("lastReviewed") {
        if !is_iso_date(&value) {
            push(
                "CM2",
                format!("`lastReviewed: {value}` must be an ISO date (YYYY-MM-DD)"),
            );
            ok = false;
        }
    }
    if let Some(value) = scalar("description") {
        let len = value.chars().count();
        if len > 160 {
            push(
                "CM1",
                format!("`description` is {len} characters; the limit is 160"),
            );
        }
    }
    match list("audience") {
        Some(audience) if !audience.is_empty() => {}
        Some(_) => {
            push(
                "CM2",
                "`audience` must name at least one audience".to_string(),
            );
            ok = false;
        }
        None => {}
    }

    // CM3: the frontmatter title and the body H1 must agree.
    let body_title = first_h1(body);
    match (scalar("title"), body_title) {
        (Some(title), Some(h1)) if title == h1 => {}
        (Some(title), Some(h1)) => push(
            "CM3",
            format!("frontmatter title \"{title}\" does not match the `#` heading \"{h1}\""),
        ),
        (Some(_), None) => push("CM3", "the page has no `#` heading".to_string()),
        (None, _) => {}
    }

    // CM6: prerequisites and related resolve to real page sources.
    for field in ["prerequisites", "related"] {
        for target in list(field).unwrap_or_default() {
            if !root.join(&target).is_file() {
                push(
                    "CM6",
                    format!("`{field}` entry `{target}` does not name an existing page"),
                );
                ok = false;
            }
        }
    }

    // IA3: the page's directory must agree with its type, and its language
    // directory with its `language` field.
    let (directory_language, directory_type) = page_location(path);
    if let (Some(type_value), Some(directory_type)) = (&page_type, directory_type) {
        if type_value != &directory_type {
            push(
                "IA3",
                format!(
                    "`type: {type_value}` does not match the directory, which holds `{directory_type}` pages"
                ),
            );
            ok = false;
        }
    }
    if let (Some(language), Some(directory_language)) = (&language, directory_language) {
        if language != &directory_language {
            push(
                "IA3",
                format!(
                    "`language: {language}` does not match the language directory `{directory_language}`"
                ),
            );
            ok = false;
        }
    }

    if !ok {
        return None;
    }
    Some(PageFacts {
        path: path.to_path_buf(),
        page_type: page_type?,
        language: language?,
    })
}

fn is_kebab_case(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return false;
    }
    let month: u32 = value[5..7].parse().unwrap_or(0);
    let day: u32 = value[8..10].parse().unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

fn first_h1(body: &str) -> Option<String> {
    let mut in_fence = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(rest) = line.strip_prefix("# ") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// The language directory and the type directory a page sits in, relative to
/// the Guide root.
fn page_location(path: &Path) -> (Option<String>, Option<String>) {
    let components: Vec<String> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str().map(str::to_string))
        .collect();
    let Some(guide) = components.iter().position(|part| part == "guide") else {
        return (None, None);
    };
    let language = components.get(guide + 1).cloned();
    let file = components.last().cloned().unwrap_or_default();
    let directory_type = if file == "overview.md" {
        Some("overview".to_string())
    } else {
        components
            .get(guide + 2)
            .map(|directory| match directory.as_str() {
                "tutorials" => "tutorial",
                "how-to" => "how-to",
                "concepts" => "concept",
                "reference" => "reference",
                other => other,
            })
            .map(str::to_string)
    };
    (language, directory_type)
}

// ---------------------------------------------------------------------------
// Structure: headings and fences
// ---------------------------------------------------------------------------

struct Fence {
    info: String,
    body: String,
    /// 1-based line of the opening fence.
    line: usize,
    /// The `##` heading the fence sits under, if any.
    section: Option<String>,
}

struct Structure {
    headings: Vec<(String, usize)>,
    fences: Vec<Fence>,
    /// Indented Phero fences: (line, info).
    indented: Vec<(usize, String)>,
    /// Every fenced region as 0-based body line indices (start, end), so the
    /// prose scan never reads code as prose.
    spans: Vec<(usize, usize)>,
}

fn scan_structure(body: &str, body_start: usize) -> Structure {
    let lines: Vec<&str> = body.split('\n').collect();
    let mut headings = Vec::new();
    let mut fences = Vec::new();
    let mut indented = Vec::new();
    let mut spans = Vec::new();
    let mut current_section: Option<String> = None;
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        let line_number = body_start + index;

        // An indented Phero fence is a site-pipeline violation even though
        // Markdown may accept it; record it and skip to its closing fence so
        // its body is not scanned as prose.
        if line.starts_with(char::is_whitespace) {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("```") {
                if rest.trim_start().starts_with("mox") {
                    indented.push((line_number, rest.trim().to_string()));
                }
                let start = index;
                index += 1;
                while index < lines.len() && lines[index].trim() != "```" {
                    index += 1;
                }
                spans.push((start, index.min(lines.len().saturating_sub(1))));
                index += 1;
                continue;
            }
        }

        if let Some(rest) = line.strip_prefix("```") {
            let info = rest.trim().to_string();
            let start = index;
            index += 1;
            let mut body_lines = Vec::new();
            while index < lines.len() && !lines[index].starts_with("```") {
                body_lines.push(lines[index]);
                index += 1;
            }
            spans.push((start, index.min(lines.len().saturating_sub(1))));
            fences.push(Fence {
                info,
                body: body_lines.join("\n"),
                line: line_number,
                section: current_section.clone(),
            });
            index += 1; // closing fence
            continue;
        }

        if let Some(rest) = line.strip_prefix("## ") {
            if !rest.starts_with('#') {
                let heading = rest.trim().to_string();
                current_section = Some(heading.clone());
                headings.push((heading, line_number));
            }
        }
        index += 1;
    }

    Structure {
        headings,
        fences,
        indented,
        spans,
    }
}

fn check_headings(
    path: &Path,
    page_type: &str,
    headings: &[(String, usize)],
    findings: &mut Vec<Finding>,
) {
    let texts: Vec<&str> = headings.iter().map(|(text, _)| text.as_str()).collect();
    let first_line = headings.first().map_or(1, |(_, line)| *line);

    let mut push = |message: String| {
        findings.push(Finding {
            path: path.to_path_buf(),
            line: first_line,
            rule: "CM4",
            message,
        });
    };

    match page_type {
        "overview" => compare_sequence(&texts, &OVERVIEW_HEADINGS, &mut push),
        "how-to" => compare_sequence(&texts, &HOW_TO_HEADINGS, &mut push),
        "concept" => compare_sequence(&texts, &CONCEPT_HEADINGS, &mut push),
        "reference" => compare_sequence(&texts, &REFERENCE_HEADINGS, &mut push),
        "integration" => compare_sequence(&texts, &INTEGRATION_HEADINGS, &mut push),
        "tutorial" => check_tutorial(&texts, &mut push),
        _ => {}
    }
}

fn compare_sequence(texts: &[&str], expected: &[&str], push: &mut impl FnMut(String)) {
    if texts == expected {
        return;
    }
    for (index, heading) in expected.iter().enumerate() {
        match texts.get(index) {
            Some(actual) if actual == heading => {}
            Some(actual) => {
                push(format!(
                    "expected `## {heading}` at position {}, found `## {actual}`",
                    index + 1
                ));
                return;
            }
            None => {
                push(format!("missing required heading `## {heading}`"));
                return;
            }
        }
    }
    if let Some(extra) = texts.get(expected.len()) {
        push(format!("unexpected extra heading `## {extra}`"));
    }
}

fn check_tutorial(texts: &[&str], push: &mut impl FnMut(String)) {
    let mut index = 0;
    for heading in TUTORIAL_PREFIX {
        match texts.get(index) {
            Some(actual) if *actual == heading => index += 1,
            Some(actual) => {
                push(format!("expected `## {heading}`, found `## {actual}`"));
                return;
            }
            None => {
                push(format!("missing required heading `## {heading}`"));
                return;
            }
        }
    }

    let mut steps = 0;
    while let Some(heading) = texts.get(index) {
        let Some(number) = tutorial_step_number(heading) else {
            break;
        };
        steps += 1;
        if number != steps {
            push(format!(
                "tutorial steps must be numbered from 1 in order; found step {number} where {steps} was expected"
            ));
            return;
        }
        index += 1;
    }
    if steps == 0 {
        push("a tutorial needs at least one `## Step <n> — <title>` heading".to_string());
        return;
    }

    for heading in TUTORIAL_SUFFIX {
        match texts.get(index) {
            Some(actual) if *actual == heading => index += 1,
            Some(actual) => {
                push(format!("expected `## {heading}`, found `## {actual}`"));
                return;
            }
            None => {
                push(format!("missing required heading `## {heading}`"));
                return;
            }
        }
    }
    if let Some(extra) = texts.get(index) {
        push(format!("unexpected extra heading `## {extra}`"));
    }
}

fn tutorial_step_number(heading: &str) -> Option<usize> {
    let rest = heading.strip_prefix("Step ")?;
    let (number, title) = rest.split_once(" — ")?;
    if title.trim().is_empty() {
        return None;
    }
    number.trim().parse().ok()
}

#[derive(Default)]
struct FenceOptions {
    fragment: bool,
    invalid: Option<String>,
    fixture: Option<String>,
}

fn parse_fence_options(suffix: &str) -> Result<FenceOptions, String> {
    let mut options = FenceOptions::default();
    let suffix = suffix.trim();
    if suffix.is_empty() {
        return Ok(options);
    }
    for part in suffix.split(',') {
        let part = part.trim();
        if part == "fragment" {
            options.fragment = true;
        } else if let Some(code) = part.strip_prefix("invalid=") {
            let code = code.trim();
            if code.is_empty() {
                return Err("`invalid=` needs the expected diagnostic code".to_string());
            }
            options.invalid = Some(code.to_string());
        } else if let Some(fixture) = part.strip_prefix("fixture=") {
            let fixture = fixture.trim();
            if fixture.is_empty() {
                return Err("`fixture=` needs a repository-root path".to_string());
            }
            options.fixture = Some(fixture.to_string());
        } else {
            return Err(format!(
                "unknown fence attribute `{part}` — use `fragment`, `invalid=<CODE>`, or `fixture=<PATH>`"
            ));
        }
    }
    Ok(options)
}

/// One compiled fence: its errors (messages only) and a rendered diagnostic
/// document for the expected-code assertion.
struct FenceOutcome {
    errors: Vec<String>,
    rendered: String,
}

impl FenceOutcome {
    fn ok(&self) -> bool {
        self.errors.is_empty()
    }

    fn first_error(&self) -> &str {
        self.errors.first().map_or("", String::as_str)
    }
}

fn check_fences(root: &Path, path: &Path, structure: &Structure, findings: &mut Vec<Finding>) {
    for (line, info) in &structure.indented {
        findings.push(Finding {
            path: path.to_path_buf(),
            line: *line,
            rule: "CM10",
            message: format!(
                "Phero fence `{info}` is indented; the site pipeline requires an unindented fence"
            ),
        });
    }

    let mut complete = 0usize;
    let mut illustrative = 0usize;

    for fence in &structure.fences {
        let Some((surface, suffix)) = split_fence_info(&fence.info) else {
            continue;
        };
        // Non-Phero languages belong to the site's fence-language gate, not
        // this one.
        if !is_phero_surface(&surface) {
            continue;
        }
        let surface = surface.to_ascii_lowercase();

        let options = match parse_fence_options(&suffix) {
            Ok(options) => options,
            Err(message) => {
                findings.push(Finding {
                    path: path.to_path_buf(),
                    line: fence.line,
                    rule: "CM10",
                    message,
                });
                continue;
            }
        };

        if options.fragment {
            illustrative += 1;
            if options.invalid.is_some() || options.fixture.is_some() {
                findings.push(Finding {
                    path: path.to_path_buf(),
                    line: fence.line,
                    rule: "CM10",
                    message: "a fragment does not compile; drop `invalid`/`fixture`".to_string(),
                });
            }
            if !fence_allows_fragment(fence) {
                findings.push(Finding {
                    path: path.to_path_buf(),
                    line: fence.line,
                    rule: "CM11a",
                    message: format!(
                        "`{surface},fragment` is allowed only under `## Syntax` (reference pages) or `## Solution` (how-to pages)"
                    ),
                });
            }
            continue;
        }

        let outcome = if surface == "mox" {
            if options.fixture.is_some() {
                findings.push(Finding {
                    path: path.to_path_buf(),
                    line: fence.line,
                    rule: "CM10",
                    message: "`mox` examples are self-contained; `fixture` is for `.ddd` fences"
                        .to_string(),
                });
                continue;
            }
            compile_mox_example(&fence.body)
        } else if surface == "ddd" {
            match compile_ddd_example(root, path, &fence.body, options.fixture.as_deref()) {
                Ok(outcome) => outcome,
                Err(message) => {
                    findings.push(Finding {
                        path: path.to_path_buf(),
                        line: fence.line,
                        rule: "CM10",
                        message,
                    });
                    continue;
                }
            }
        } else if surface == "dmn" {
            match compile_dmn_example(root, &fence.body, options.fixture.as_deref()) {
                Ok(outcome) => outcome,
                Err(message) => {
                    findings.push(Finding {
                        path: path.to_path_buf(),
                        line: fence.line,
                        rule: "CM10",
                        message,
                    });
                    continue;
                }
            }
        } else if surface == "ifml" {
            match compile_ifml_example(root, &fence.body, options.fixture.as_deref()) {
                Ok(outcome) => outcome,
                Err(message) => {
                    findings.push(Finding {
                        path: path.to_path_buf(),
                        line: fence.line,
                        rule: "CM10",
                        message,
                    });
                    continue;
                }
            }
        } else {
            findings.push(Finding {
                path: path.to_path_buf(),
                line: fence.line,
                rule: "CM10",
                message: format!(
                    "the documentation gate does not compile `{surface}` fences yet; teach `{surface}` from its own guide tree"
                ),
            });
            continue;
        };

        match &options.invalid {
            Some(code) => {
                if outcome.ok() {
                    findings.push(Finding {
                        path: path.to_path_buf(),
                        line: fence.line,
                        rule: "CM10",
                        message: format!(
                            "invalid example compiled; expected it to fail with `{code}`"
                        ),
                    });
                } else if !mentions_code(&outcome.rendered, code) {
                    findings.push(Finding {
                        path: path.to_path_buf(),
                        line: fence.line,
                        rule: "CM10",
                        message: format!(
                            "invalid example failed without `{code}`; first error: {}",
                            outcome.first_error()
                        ),
                    });
                }
            }
            None => {
                if outcome.ok() {
                    complete += 1;
                    bump_compiled();
                } else {
                    findings.push(Finding {
                        path: path.to_path_buf(),
                        line: fence.line,
                        rule: "CM10",
                        message: format!("example does not compile: {}", outcome.first_error()),
                    });
                }
            }
        }
    }

    if illustrative > 0 && complete == 0 {
        findings.push(Finding {
            path: path.to_path_buf(),
            line: structure.fences.first().map_or(1, |fence| fence.line),
            rule: "CM11a",
            message:
                "a page with syntax illustrations must also carry at least one complete example"
                    .to_string(),
        });
    }
}

fn compile_mox_example(body: &str) -> FenceOutcome {
    let compilation =
        phero_driver::compile_str("fence.mox", body, &phero_driver::DomainImports::default());
    let errors: Vec<String> = compilation
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.is_error())
        .map(|diagnostic| diagnostic.message.clone())
        .collect();
    let rendered = phero_driver::render("fence.mox", body, &compilation.diagnostics);
    FenceOutcome { errors, rendered }
}

/// Compiles one `.ddd` fence. A `.ddd` file resolves its imports against the
/// design's directory, so the gate supplies the named fixture domain under the
/// path the fence's single `import` line names.
fn compile_ddd_example(
    root: &Path,
    page: &Path,
    body: &str,
    fixture: Option<&str>,
) -> Result<FenceOutcome, String> {
    let design_dir = repo_relative_dir(root, page);
    let mut imports = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("import ") {
            continue;
        }
        match parse_ddd_import(trimmed) {
            Some(path) => imports.push(path),
            None => {
                return Err(format!(
                    "unsupported `.ddd` import `{trimmed}` — the gate supplies one domain import (`import \"<file>.mox\";`)"
                ))
            }
        }
    }

    let domains = match (fixture, imports.as_slice()) {
        (Some(fixture), [import]) => {
            let text = fs::read_to_string(root.join(fixture)).map_err(|error| {
                format!("cannot read the fixture domain `{fixture}`: {error}")
            })?;
            vec![(join_posix(&design_dir, import), text)]
        }
        (Some(_), []) => {
            return Err(
                "a `fixture` needs an `import` line naming the domain file".to_string(),
            )
        }
        (Some(_), _) => {
            return Err(
                "one fixture per fence — a `.ddd` example imports exactly one domain".to_string(),
            )
        }
        (None, []) => Vec::new(),
        (None, _) => {
            return Err(
                "a `.ddd` example must name its domain: add `fixture=<repository-root path>` to the fence"
                    .to_string(),
            )
        }
    };

    let inputs = phero_driver::CompileInputs::from_domains(&domains);
    let design_path = join_posix(&design_dir, "fence.ddd");
    let compilation = phero_driver::compile_ddd_str(&design_path, body, &inputs);
    let errors: Vec<String> = compilation
        .diagnostics
        .iter()
        .filter(|(_, diagnostic)| diagnostic.is_error())
        .map(|(path, diagnostic)| format!("{path}: {}", diagnostic.message))
        .collect();
    let rendered = errors.join("\n");
    Ok(FenceOutcome { errors, rendered })
}

/// Compiles one `.ifml` fence. The fixture names a directory (for example
/// `examples/support`) that the fence's imports resolve against; the gate
/// supplies the domain (`.mox`) and any `.actor` policies, then runs the
/// interaction, authorization, and binding passes.
fn compile_ifml_example(
    root: &Path,
    body: &str,
    fixture: Option<&str>,
) -> Result<FenceOutcome, String> {
    let Some(fixture) = fixture else {
        return Err(
            "an `.ifml` example must name its fixture directory: `ifml,fixture=<dir>`".to_string(),
        );
    };
    let fixture_dir = fixture.trim_end_matches('/');
    let mut imports = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("import ") {
            continue;
        }
        match parse_ddd_import(trimmed) {
            Some(path) => imports.push(path),
            None => {
                return Err(format!(
                    "unsupported `.ifml` import `{trimmed}` — the gate supplies a `.mox` domain and `.actor` policies"
                ))
            }
        }
    }
    if imports.is_empty() {
        return Err(
            "an `.ifml` example must import its domain: `import \"support.mox\";`".to_string(),
        );
    }

    let design_path = join_posix(fixture_dir, "fence.ifml");
    let mut provided = Vec::new();
    let mut domain = None;
    let mut actor_sources = Vec::new();
    for import in &imports {
        let resolved = join_posix(fixture_dir, import);
        let text = fs::read_to_string(root.join(&resolved))
            .map_err(|error| format!("cannot read the imported file `{resolved}`: {error}"))?;
        match Path::new(&resolved)
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("mox") => {
                if domain.is_some() {
                    return Err(
                        "one domain per `.ifml` example — import exactly one `.mox` file".to_string(),
                    );
                }
                domain = Some((resolved.clone(), text.clone()));
            }
            Some("actor") => actor_sources.push((resolved.clone(), text.clone())),
            _ => {
                return Err(format!(
                    "the documentation gate compiles `.ifml` examples with a `.mox` domain and `.actor` policies; `{import}` is not supported yet"
                ))
            }
        }
        provided.push((import.clone(), text));
    }
    let (domain_path, domain_text) =
        domain.ok_or_else(|| "an `.ifml` example must import one `.mox` domain".to_string())?;

    let domain_compilation = phero_driver::compile_str(
        &domain_path,
        &domain_text,
        &phero_driver::DomainImports::default(),
    );
    if let Some(error) = domain_compilation
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.is_error())
    {
        return Err(format!(
            "the fixture domain does not compile: {}",
            error.message
        ));
    }
    let domain_model = domain_compilation
        .model
        .ok_or_else(|| "the fixture domain did not produce a model".to_string())?;

    let actor_inputs =
        phero_driver::CompileInputs::from_domains(&[(domain_path.clone(), domain_text)]);
    let mut actors = Vec::new();
    for (path, source) in actor_sources {
        let compilation = phero_driver::compile_actors_str(&path, &source, &actor_inputs);
        if let Some((_, error)) = compilation
            .diagnostics
            .iter()
            .find(|(_, diagnostic)| diagnostic.is_error())
        {
            return Err(format!(
                "actor policy `{path}` does not compile: {}",
                error.message
            ));
        }
        let Some(model) = compilation.model else {
            return Err(format!("actor policy `{path}` did not produce a model"));
        };
        actors.push(model);
    }

    let mut ifml_imports = phero_ifml::IfmlImports::default();
    for (import, text) in provided {
        ifml_imports.provide(&design_path, &import, text);
    }

    let compilation = match phero_ifml::compile_ifml_str(&design_path, body, &ifml_imports) {
        Ok(compilation) => compilation,
        Err(diagnostics) => {
            let rendered = diagnostics
                .iter()
                .map(|(path, diagnostic)| format_ifml_diagnostic(path, diagnostic))
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(FenceOutcome {
                errors: vec![rendered.clone()],
                rendered,
            });
        }
    };
    let diagnostics =
        phero_ifml::check_ifml_with_actors(&compilation, Some(&domain_model), &actors);
    let rendered = diagnostics
        .iter()
        .map(|(path, diagnostic)| format_ifml_diagnostic(path, diagnostic))
        .collect::<Vec<_>>()
        .join("\n");
    let errors = diagnostics
        .iter()
        .filter(|(_, diagnostic)| diagnostic.is_error())
        .map(|(path, diagnostic)| format_ifml_diagnostic(path, diagnostic))
        .collect();
    Ok(FenceOutcome { errors, rendered })
}

fn format_ifml_diagnostic(
    path: &str,
    diagnostic: &phero_diagnostics::Diagnostic<&'static str>,
) -> String {
    match diagnostic.code {
        Some(code) => format!("{path}: {code} {}", diagnostic.message),
        None => format!("{path}: {}", diagnostic.message),
    }
}

/// Compiles one `.dmn` fence. The fixture names a directory; imports (`.mox`,
/// `.evt`) resolve against it, and every `logic table "<path>"` markdown file is
/// read from the same directory and supplied keyed by the path as written.
fn compile_dmn_example(
    root: &Path,
    body: &str,
    fixture: Option<&str>,
) -> Result<FenceOutcome, String> {
    let Some(fixture) = fixture else {
        return Err(
            "a `.dmn` example must name its fixture directory: `dmn,fixture=<dir>`".to_string(),
        );
    };
    let fixture_dir = fixture.trim_end_matches('/');
    let design_path = join_posix(fixture_dir, "fence.dmn");

    let mut imports = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("import ") {
            match parse_quoted_path(rest) {
                Some(path) => imports.push(path),
                None => {
                    return Err(format!(
                        "unsupported `.dmn` import `{trimmed}` — expected `import \"<file>\";`"
                    ))
                }
            }
        }
    }

    let mut domains: Vec<(String, String)> = Vec::new();
    let mut events: Vec<(String, phero_ir::events::EventModel)> = Vec::new();
    for import in &imports {
        let resolved = join_posix(fixture_dir, import);
        let text = fs::read_to_string(root.join(&resolved))
            .map_err(|error| format!("cannot read the imported file `{resolved}`: {error}"))?;
        match Path::new(&resolved)
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("mox") => {
                let compilation = phero_driver::compile_str(
                    &resolved,
                    &text,
                    &phero_driver::DomainImports::default(),
                );
                if let Some(error) = compilation
                    .diagnostics
                    .iter()
                    .find(|diagnostic| diagnostic.is_error())
                {
                    return Err(format!(
                        "the fixture domain `{resolved}` does not compile: {}",
                        error.message
                    ));
                }
                domains.push((resolved, text));
            }
            Some("evt") => {
                let inputs = phero_driver::CompileInputs::from_domains(&domains);
                let compilation = phero_driver::compile_evt_str(&resolved, &text, &inputs);
                if let Some((_, error)) = compilation
                    .diagnostics
                    .iter()
                    .find(|(_, diagnostic)| diagnostic.is_error())
                {
                    return Err(format!(
                        "the fixture event contract `{resolved}` does not compile: {}",
                        error.message
                    ));
                }
                let Some(model) = compilation.model else {
                    return Err(format!(
                        "the fixture event contract `{resolved}` did not produce a model"
                    ));
                };
                // `.dmn` matches `.evt` imports by the path as written; the
                // contract itself was compiled with its resolved path so its
                // own domain import resolves.
                events.push((import.clone(), model));
            }
            _ => {
                return Err(format!(
                    "the documentation gate compiles `.dmn` examples with a `.mox` domain and `.evt` contracts; `{import}` is not supported yet"
                ))
            }
        }
    }

    let mut tables = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("logic table ") {
            let Some(path) = parse_quoted_path(rest) else {
                return Err(format!(
                    "a `logic table` needs a quoted path, found `{trimmed}`"
                ));
            };
            let resolved = join_posix(fixture_dir, &path);
            let text = fs::read_to_string(root.join(&resolved)).map_err(|error| {
                format!("cannot read the decision table `{path}` under `{fixture}`: {error}")
            })?;
            tables.push((path, text));
        }
    }

    let inputs = phero_driver::CompileInputs::from_domains(&domains)
        .with_events(events)
        .with_tables(tables);
    let compilation = phero_driver::compile_dmn_str(&design_path, body, &inputs);
    // Driver diagnostics already carry their registry code inside the message
    // text (the `E02xx` prefix), so rendering the message is sufficient.
    let rendered = compilation
        .diagnostics
        .iter()
        .map(|(path, diagnostic)| format!("{path}: {}", diagnostic.message))
        .collect::<Vec<_>>()
        .join("\n");
    let errors = compilation
        .diagnostics
        .iter()
        .filter(|(_, diagnostic)| diagnostic.is_error())
        .map(|(path, diagnostic)| format!("{path}: {}", diagnostic.message))
        .collect();
    Ok(FenceOutcome { errors, rendered })
}

fn parse_quoted_path(rest: &str) -> Option<String> {
    let rest = rest.trim().trim_end_matches(';').trim();
    let inner = rest.strip_prefix('"')?.strip_suffix('"')?;
    Some(inner.to_string())
}

fn parse_ddd_import(line: &str) -> Option<String> {
    let rest = line.strip_prefix("import ")?;
    let rest = rest.trim_end().strip_suffix(';')?;
    let rest = rest.trim();
    let inner = rest.strip_prefix('"')?.strip_suffix('"')?;
    Some(inner.to_string())
}

fn repo_relative_dir(root: &Path, path: &Path) -> String {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let relative = parent.strip_prefix(root).unwrap_or(parent);
    relative
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
}

fn join_posix(base: &str, relative: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in base.split('/').chain(relative.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

fn fence_allows_fragment(fence: &Fence) -> bool {
    matches!(fence.section.as_deref(), Some("Syntax") | Some("Solution"))
}

fn is_phero_surface(surface: &str) -> bool {
    matches!(
        surface.to_ascii_lowercase().as_str(),
        "mox" | "ddd" | "ifml" | "dmn" | "cmmn" | "evt" | "deploy" | "uc" | "intent" | "actor"
    )
}

fn split_fence_info(info: &str) -> Option<(String, String)> {
    let info = info.trim();
    if info.is_empty() {
        return None;
    }
    let Some((surface, suffix)) = info.split_once(',') else {
        return Some((info.to_string(), String::new()));
    };
    Some((surface.trim().to_string(), suffix.trim().to_string()))
}

fn mentions_code(rendered: &str, code: &str) -> bool {
    rendered.match_indices(code).any(|(index, _)| {
        let before = rendered[..index].chars().next_back();
        let after = rendered[index + code.len()..].chars().next();
        !before.is_some_and(|ch| ch.is_ascii_alphanumeric())
            && !after.is_some_and(|ch| ch.is_ascii_alphanumeric())
    })
}

// ---------------------------------------------------------------------------
// Vocabulary, history, terminology
// ---------------------------------------------------------------------------

/// The page lines that are prose: everything outside a fenced region, with
/// markdown link targets and bare URLs masked so a URL cannot masquerade as an
/// abbreviation or a term.
fn prose_lines(body: &str, body_start: usize, structure: &Structure) -> Vec<(usize, String)> {
    let lines: Vec<&str> = body.split('\n').collect();
    let mut blocked = vec![false; lines.len()];
    for (start, end) in &structure.spans {
        for slot in blocked.iter_mut().take(*end + 1).skip(*start) {
            *slot = true;
        }
    }

    lines
        .iter()
        .enumerate()
        .filter(|(index, _)| !blocked[*index])
        .map(|(index, line)| (body_start + index, mask_links(line)))
        .collect()
}

fn mask_links(line: &str) -> String {
    let mut masked = String::with_capacity(line.len());
    let mut index = 0;
    while index < line.len() {
        if line[index..].starts_with("](") {
            match line[index + 2..].find(')') {
                Some(close) => index += 2 + close + 1,
                None => {
                    masked.push_str(&line[index..]);
                    break;
                }
            }
            continue;
        }
        if line[index..].starts_with("http://") || line[index..].starts_with("https://") {
            while index < line.len() && !line.as_bytes()[index].is_ascii_whitespace() {
                index += 1;
            }
            continue;
        }
        let Some(ch) = line[index..].chars().next() else {
            break;
        };
        masked.push(ch);
        index += ch.len_utf8();
    }
    masked
}

fn check_implementation_vocabulary(
    path: &Path,
    _page_type: &str,
    prose: &[(usize, String)],
    glossary: &Glossary,
    findings: &mut Vec<Finding>,
) {
    let mut reported: BTreeSet<String> = BTreeSet::new();
    for (line, text) in prose {
        for token in tokens(text) {
            let lower = token.to_ascii_lowercase();
            let is_implementation = glossary.implementation_words.contains(&lower)
                || glossary.implementation_identifiers.contains(token)
                || glossary
                    .implementation_patterns
                    .iter()
                    .any(|prefix| token.starts_with(prefix));
            if is_implementation && reported.insert(lower) {
                findings.push(Finding {
                    path: path.to_path_buf(),
                    line: *line,
                    rule: "CM12",
                    message: format!(
                        "implementation vocabulary `{token}` in a reader-facing page — link to the engineering reference instead"
                    ),
                });
            }
        }
    }
}

fn check_history(path: &Path, prose: &[(usize, String)], findings: &mut Vec<Finding>) {
    let mut reported = false;
    for (line, text) in prose {
        let has_issue = tokens(text).any(|token| {
            token.len() > 1
                && token.starts_with('#')
                && token[1..].chars().all(|ch| ch.is_ascii_digit())
        }) || text.to_ascii_lowercase().contains("pherolang#");
        if has_issue && !reported {
            findings.push(Finding {
                path: path.to_path_buf(),
                line: *line,
                rule: "CM13",
                message: "issue numbers do not belong in Guide prose; link to an ADR from a concept page instead"
                    .to_string(),
            });
            reported = true;
        }
    }
}

fn check_terminology(
    path: &Path,
    prose: &[(usize, String)],
    glossary: &Glossary,
    page_type: Option<&str>,
    findings: &mut Vec<Finding>,
) {
    let mut reported: BTreeSet<String> = BTreeSet::new();
    for (line, text) in prose {
        let masked = mask_canonical(text, &glossary.canonical_terms);

        // Aliases may be multi-word, so match them against the whole line.
        for term in &glossary.terms {
            for alias in &term.aliases {
                if find_ascii_ci_word(&masked, alias) && reported.insert(alias.to_ascii_lowercase())
                {
                    findings.push(Finding {
                        path: path.to_path_buf(),
                        line: *line,
                        rule: "CM14",
                        message: format!(
                            "`{alias}` is an alias of `{}` — use the canonical term (docs/documentation/glossary.yaml)",
                            term.term
                        ),
                    });
                }
            }
        }

        // An abbreviation must be defined in prose; a code span is a literal
        // (a command, output, or identifier), not an abbreviation use.
        let prose_only = strip_inline_code(&masked);
        for token in tokens(&prose_only) {
            if token.len() < 2 || !token.chars().all(|ch| ch.is_ascii_uppercase()) {
                continue;
            }
            match glossary
                .acronyms
                .iter()
                .find(|acronym| acronym.acronym == token)
            {
                None => {
                    if reported.insert(token.to_ascii_lowercase()) {
                        findings.push(Finding {
                            path: path.to_path_buf(),
                            line: *line,
                            rule: "CM14",
                            message: format!(
                                "abbreviation `{token}` is not defined in docs/documentation/glossary.yaml"
                            ),
                        });
                    }
                }
                Some(acronym) => {
                    let allowed = match acronym.scope.as_str() {
                        "all" => true,
                        "concept" => page_type == Some("concept"),
                        _ => false,
                    };
                    if !allowed && reported.insert(token.to_ascii_lowercase()) {
                        findings.push(Finding {
                            path: path.to_path_buf(),
                            line: *line,
                            rule: "CM14",
                            message: format!(
                                "abbreviation `{token}` has scope `{}` in the glossary and may not appear on this page",
                                acronym.scope
                            ),
                        });
                    }
                }
            }
        }
    }
}

/// Case-insensitive whole-word search for an ASCII needle.
fn find_ascii_ci_word(haystack: &str, needle: &str) -> bool {
    let needle_bytes = needle.as_bytes();
    if needle_bytes.is_empty() || needle_bytes.len() > haystack.len() {
        return false;
    }
    let hay = haystack.as_bytes();
    let starts = hay
        .windows(needle_bytes.len())
        .enumerate()
        .filter(|(_, window)| window.eq_ignore_ascii_case(needle_bytes))
        .map(|(index, _)| index);
    for index in starts {
        let before = haystack[..index].chars().next_back();
        let after = haystack[index + needle_bytes.len()..].chars().next();
        if !before.is_some_and(is_word_character) && !after.is_some_and(is_word_character) {
            return true;
        }
    }
    false
}

fn is_word_character(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '-'
}

/// Blanks out inline code spans, preserving byte positions for line numbers.
fn strip_inline_code(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for ch in line.chars() {
        if ch == '`' {
            in_code = !in_code;
            out.push(' ');
        } else if in_code {
            for _ in 0..ch.len_utf8() {
                out.push(' ');
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn mask_canonical(text: &str, terms: &[String]) -> String {
    let mut masked = text.to_string();
    for term in terms {
        masked = replace_ascii_ci(&masked, term);
    }
    masked
}

fn replace_ascii_ci(haystack: &str, needle: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
    let hay = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();
    let mut out = String::with_capacity(haystack.len());
    let mut copied = 0;
    let mut index = 0;
    while index + needle_bytes.len() <= hay.len() {
        if hay[index..index + needle_bytes.len()].eq_ignore_ascii_case(needle_bytes) {
            out.push_str(&haystack[copied..index]);
            out.push(' ');
            copied = index + needle_bytes.len();
            index = copied;
        } else {
            index += 1;
        }
    }
    out.push_str(&haystack[copied..]);
    out
}

fn tokens(line: &str) -> impl Iterator<Item = &str> {
    line.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_' && ch != '#')
        .filter(|token| !token.is_empty())
}

// ---------------------------------------------------------------------------
// Glossary
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct Glossary {
    terms: Vec<GlossaryTerm>,
    canonical_terms: Vec<String>,
    acronyms: Vec<Acronym>,
    implementation_words: BTreeSet<String>,
    implementation_identifiers: BTreeSet<String>,
    implementation_patterns: Vec<String>,
}

#[derive(Debug, Default)]
struct GlossaryTerm {
    term: String,
    aliases: Vec<String>,
}

#[derive(Debug)]
struct Acronym {
    acronym: String,
    scope: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GlossarySection {
    None,
    Terms,
    Acronyms,
    Implementation,
}

fn parse_glossary(text: &str) -> Result<Glossary, String> {
    let mut glossary = Glossary::default();
    let mut section = GlossarySection::None;
    let mut current_term: Option<GlossaryTerm> = None;
    let mut current_acronym: Option<Acronym> = None;
    let mut implementation_key: Option<&str> = None;

    let finish_term = |glossary: &mut Glossary, current: &mut Option<GlossaryTerm>| {
        if let Some(term) = current.take() {
            glossary.canonical_terms.push(term.term.clone());
            glossary.terms.push(term);
        }
    };
    let finish_acronym = |glossary: &mut Glossary, current: &mut Option<Acronym>| {
        if let Some(acronym) = current.take() {
            glossary.acronyms.push(acronym);
        }
    };

    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();

        if indent == 0 {
            finish_term(&mut glossary, &mut current_term);
            finish_acronym(&mut glossary, &mut current_acronym);
            implementation_key = None;
            section = match trimmed {
                "terms:" => GlossarySection::Terms,
                "acronyms:" => GlossarySection::Acronyms,
                "implementation_vocabulary:" => GlossarySection::Implementation,
                "formatVersion:" => GlossarySection::None,
                other if other.starts_with("formatVersion:") => GlossarySection::None,
                other => {
                    return Err(format!(
                        "line {}: unknown glossary section `{other}`",
                        index + 1
                    ))
                }
            };
            continue;
        }

        match section {
            GlossarySection::Terms => {
                if let Some(rest) = trimmed.strip_prefix("- ") {
                    finish_term(&mut glossary, &mut current_term);
                    let (key, value) = split_key_value(rest, index)?;
                    let mut term = GlossaryTerm::default();
                    if key == "term" {
                        term.term = value;
                    } else {
                        return Err(format!(
                            "line {}: a term entry must start with `term:`",
                            index + 1
                        ));
                    }
                    current_term = Some(term);
                } else {
                    let (key, value) = split_key_value(trimmed, index)?;
                    let term = current_term.as_mut().ok_or_else(|| {
                        format!("line {}: term field outside an entry", index + 1)
                    })?;
                    match key.as_str() {
                        "definition" => {}
                        "aliases" => term.aliases = parse_inline_list(&value),
                        other => {
                            return Err(format!("line {}: unknown term field `{other}`", index + 1))
                        }
                    }
                }
            }
            GlossarySection::Acronyms => {
                if let Some(rest) = trimmed.strip_prefix("- ") {
                    finish_acronym(&mut glossary, &mut current_acronym);
                    let (key, value) = split_key_value(rest, index)?;
                    if key != "acronym" {
                        return Err(format!(
                            "line {}: an acronym entry must start with `acronym:`",
                            index + 1
                        ));
                    }
                    current_acronym = Some(Acronym {
                        acronym: value,
                        scope: "all".to_string(),
                    });
                } else {
                    let (key, value) = split_key_value(trimmed, index)?;
                    let acronym = current_acronym.as_mut().ok_or_else(|| {
                        format!("line {}: acronym field outside an entry", index + 1)
                    })?;
                    match key.as_str() {
                        "expansion" => {}
                        "scope" => acronym.scope = value,
                        other => {
                            return Err(format!(
                                "line {}: unknown acronym field `{other}`",
                                index + 1
                            ))
                        }
                    }
                }
            }
            GlossarySection::Implementation => {
                if let Some(rest) = trimmed.strip_prefix("- ") {
                    let key = implementation_key.ok_or_else(|| {
                        format!(
                            "line {}: implementation vocabulary item outside a list",
                            index + 1
                        )
                    })?;
                    let value = unquote(rest);
                    match key {
                        "words" => {
                            glossary
                                .implementation_words
                                .insert(value.to_ascii_lowercase());
                        }
                        "identifiers" => {
                            glossary.implementation_identifiers.insert(value);
                        }
                        "patterns" => glossary.implementation_patterns.push(value),
                        other => {
                            return Err(format!(
                                "line {}: unknown implementation list `{other}`",
                                index + 1
                            ))
                        }
                    }
                } else {
                    let (key, value) = split_key_value(trimmed, index)?;
                    if !value.is_empty() {
                        return Err(format!(
                            "line {}: implementation list `{key}` must be a block list",
                            index + 1
                        ));
                    }
                    implementation_key = Some(match key.as_str() {
                        "words" => "words",
                        "identifiers" => "identifiers",
                        "patterns" => "patterns",
                        other => {
                            return Err(format!(
                                "line {}: unknown implementation list `{other}`",
                                index + 1
                            ))
                        }
                    });
                }
            }
            GlossarySection::None => {}
        }
    }
    finish_term(&mut glossary, &mut current_term);
    finish_acronym(&mut glossary, &mut current_acronym);

    if glossary.canonical_terms.is_empty() {
        return Err("glossary declares no terms".to_string());
    }
    Ok(glossary)
}

fn split_key_value(line: &str, index: usize) -> Result<(String, String), String> {
    let Some((key, value)) = line.split_once(':') else {
        return Err(format!("line {}: expected `key: value`", index + 1));
    };
    let key = key.trim().to_string();
    let value = value.trim().to_string();
    if key.is_empty() {
        return Err(format!("line {}: empty key", index + 1));
    }
    Ok((key, value))
}

fn parse_inline_list(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if !(trimmed.starts_with('[') && trimmed.ends_with(']')) {
        return Vec::new();
    }
    split_inline_list(&trimmed[1..trimmed.len() - 1])
}

// ---------------------------------------------------------------------------
// Cross-page checks
// ---------------------------------------------------------------------------

fn check_language_coverage(pages: &[PageFacts], findings: &mut Vec<Finding>) {
    let mut languages: BTreeMap<String, (bool, PathBuf)> = BTreeMap::new();
    for page in pages {
        let entry = languages
            .entry(page.language.clone())
            .or_insert_with(|| (false, page.path.clone()));
        if page.page_type == "overview" {
            entry.0 = true;
        }
    }
    for (language, (has_overview, path)) in languages {
        if !has_overview {
            findings.push(Finding {
                path,
                line: 1,
                rule: "CM7",
                message: format!(
                    "language `{language}` has pages but no `overview` page; the overview is the entry point the navigation depends on"
                ),
            });
        }
    }
}

fn check_template_drift(root: &Path, findings: &mut Vec<Finding>) -> anyhow::Result<()> {
    let templates = root.join("docs/documentation/templates");
    if !templates.is_dir() {
        findings.push(Finding {
            path: templates,
            line: 1,
            rule: "CM16",
            message: "missing the template directory — run from the repository root".to_string(),
        });
        return Ok(());
    }

    for (page_type, exemplar) in TEMPLATE_EXEMPLARS {
        let template_path = templates.join(format!("{page_type}.md"));
        let exemplar_path = root.join(exemplar);

        let template = match structure_of(&template_path) {
            Ok(structure) => structure,
            Err(message) => {
                findings.push(Finding {
                    path: template_path,
                    line: 1,
                    rule: "CM16",
                    message,
                });
                continue;
            }
        };
        let exemplar_structure = match structure_of(&exemplar_path) {
            Ok(structure) => structure,
            Err(message) => {
                findings.push(Finding {
                    path: exemplar_path,
                    line: 1,
                    rule: "CM16",
                    message,
                });
                continue;
            }
        };

        let template_headings = normalize_for_drift(&template.headings);
        let exemplar_headings = normalize_for_drift(&exemplar_structure.headings);
        if template_headings != exemplar_headings {
            findings.push(Finding {
                path: template_path,
                line: 1,
                rule: "CM16",
                message: format!(
                    "template structure drifted from its exemplar `{exemplar}`: expected {:?}, found {:?}",
                    exemplar_headings, template_headings
                ),
            });
        }
    }
    Ok(())
}

fn structure_of(path: &Path) -> Result<Structure, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let (_, body, start) = split_frontmatter(&source)
        .map_err(|error| format!("{}:{}: {}", path.display(), error.line, error.message))?;
    Ok(scan_structure(body, start))
}

/// Collapses `## Step <n> — …` runs to one `Step` entry so a template's single
/// placeholder matches any number of tutorial steps.
fn normalize_for_drift(headings: &[(String, usize)]) -> Vec<String> {
    let mut normalized = Vec::new();
    let mut in_steps = false;
    for (heading, _) in headings {
        if tutorial_step_number(heading).is_some() {
            if !in_steps {
                normalized.push("Step".to_string());
                in_steps = true;
            }
        } else {
            in_steps = false;
            normalized.push(heading.clone());
        }
    }
    normalized
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD_OVERVIEW: &str = "---\n\
title: Test\n\
description: A test page.\n\
type: overview\n\
language: mox\n\
section: start\n\
order: 10\n\
status: stable\n\
docStatus: complete\n\
audience: [application developer]\n\
lastReviewed: 2026-11-21\n\
---\n\
# Test\n\
\n\
## What is it?\n\nText.\n\n\
## What problems does it solve?\n\nText.\n\n\
## Key capabilities\n\nText.\n\n\
## A simple example\n\nText.\n\n\
## How it fits into Phero Lang\n\nText.\n\n\
## Getting started\n\nText.\n\n\
## Further reading\n\nText.\n";

    #[test]
    fn frontmatter_parses_scalars_and_inline_lists() {
        let (fields, body, start) = split_frontmatter(GOOD_OVERVIEW).unwrap();
        assert_eq!(
            fields.get("title").and_then(FieldValue::scalar),
            Some("Test")
        );
        assert_eq!(
            fields.get("audience").and_then(FieldValue::list),
            Some(&["application developer".to_string()][..])
        );
        assert!(body.starts_with("# Test"));
        assert_eq!(start, 13);
    }

    #[test]
    fn frontmatter_rejects_block_lists_and_duplicates() {
        let bad = "---\ntitle: A\n  nested: b\n---\n# A\n";
        assert!(split_frontmatter(bad).is_err());
        let duplicate = "---\ntitle: A\ntitle: B\n---\n# A\n";
        assert!(split_frontmatter(duplicate).is_err());
    }

    #[test]
    fn overview_headings_must_match_exactly() {
        let mut findings = Vec::new();
        let structure = scan_structure(GOOD_OVERVIEW, 1);
        check_headings(
            Path::new("page.md"),
            "overview",
            &structure.headings,
            &mut findings,
        );
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn an_extra_heading_is_a_finding() {
        let mut findings = Vec::new();
        let structure = scan_structure("# T\n\n## What is it?\n\n## What problems does it solve?\n\n## Key capabilities\n\n## A simple example\n\n## How it fits into Phero Lang\n\n## Getting started\n\n## Further reading\n\n## Extra\n", 1);
        check_headings(
            Path::new("page.md"),
            "overview",
            &structure.headings,
            &mut findings,
        );
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("unexpected extra heading"));
    }

    #[test]
    fn tutorial_steps_must_be_numbered_in_order() {
        let body = "# T\n\n## What you'll learn\n\n## Prerequisites\n\n## What we're building\n\n## Step 1 — A\n\n## Step 3 — C\n\n## Complete example\n\n## Expected result\n\n## Next steps\n";
        let mut findings = Vec::new();
        check_headings(
            Path::new("page.md"),
            "tutorial",
            &scan_structure(body, 1).headings,
            &mut findings,
        );
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("numbered from 1"));
    }

    #[test]
    fn fences_are_classified_and_compiled() {
        let body = "# T\n\n## A simple example\n\n```mox\npackage nz.example.a\n\nclass A {\n    String name\n}\n```\n";
        let structure = scan_structure(body, 1);
        let mut findings = Vec::new();
        check_fences(
            Path::new("."),
            Path::new("page.md"),
            &structure,
            &mut findings,
        );
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_broken_example_is_a_finding() {
        let body = "# T\n\n```mox\npackage nz.example.a\n\nclass A {\n    Missing name\n}\n```\n";
        let mut findings = Vec::new();
        check_fences(
            Path::new("."),
            Path::new("page.md"),
            &scan_structure(body, 1),
            &mut findings,
        );
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("does not compile"));
    }

    #[test]
    fn an_invalid_example_must_fail_with_the_named_code() {
        let body = "# T\n\n```mox,invalid=E0300\npackage nz.example.a\n\nclass PayRun {\n    String a\n}\n\nclass PayRunType {\n    String b\n}\n```\n";
        let mut findings = Vec::new();
        check_fences(
            Path::new("."),
            Path::new("page.md"),
            &scan_structure(body, 1),
            &mut findings,
        );
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_fragment_needs_a_complete_example() {
        let body = "# T\n\n## Syntax\n\n```mox,fragment\nclass A { }\n```\n";
        let mut findings = Vec::new();
        check_fences(
            Path::new("."),
            Path::new("page.md"),
            &scan_structure(body, 1),
            &mut findings,
        );
        assert!(findings
            .iter()
            .any(|finding| finding.message.contains("at least one complete")));
    }

    #[test]
    fn indented_phero_fences_are_rejected() {
        let body = "# T\n\n1. step\n\n   ```mox\n   package nz.example.a\n   ```\n";
        let mut findings = Vec::new();
        check_fences(
            Path::new("."),
            Path::new("page.md"),
            &scan_structure(body, 1),
            &mut findings,
        );
        assert!(findings
            .iter()
            .any(|finding| finding.message.contains("is indented")));
    }

    #[test]
    fn issue_numbers_are_rejected_in_prose() {
        let mut findings = Vec::new();
        check_history(
            Path::new("page.md"),
            &[(10, "See issue #373 for the rationale.".to_string())],
            &mut findings,
        );
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn glossary_parses_terms_aliases_and_scopes() {
        let text = "formatVersion: 1\nterms:\n  - term: Phero Lang\n    definition: x\n    aliases: [Phero]\nacronyms:\n  - acronym: IR\n    expansion: x\n    scope: engineering\nimplementation_vocabulary:\n  words:\n    - parser\n  identifiers:\n    - DddModel\n  patterns:\n    - phero-\n";
        let glossary = parse_glossary(text).unwrap();
        assert_eq!(glossary.terms.len(), 1);
        assert_eq!(glossary.terms[0].aliases, vec!["Phero".to_string()]);
        assert!(glossary.implementation_words.contains("parser"));
        assert!(glossary.implementation_identifiers.contains("DddModel"));
        assert_eq!(glossary.implementation_patterns, vec!["phero-".to_string()]);
    }

    #[test]
    fn aliases_are_not_matched_inside_canonical_terms() {
        let glossary = Glossary {
            terms: vec![GlossaryTerm {
                term: "Phero Lang".to_string(),
                aliases: vec!["Phero".to_string()],
            }],
            canonical_terms: vec!["Phero Lang".to_string()],
            ..Glossary::default()
        };
        let mut findings = Vec::new();
        check_terminology(
            Path::new("page.md"),
            &[(1, "Phero Lang is a modelling ecosystem.".to_string())],
            &glossary,
            Some("overview"),
            &mut findings,
        );
        assert!(findings.is_empty(), "{findings:?}");

        let mut findings = Vec::new();
        check_terminology(
            Path::new("page.md"),
            &[(1, "Phero is a modelling ecosystem.".to_string())],
            &glossary,
            Some("overview"),
            &mut findings,
        );
        assert_eq!(findings.len(), 1);
    }

    fn repo_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    #[test]
    fn a_ddd_example_compiles_against_its_fixture() {
        let body = "# T\n\n## A simple example\n\n```ddd,fixture=examples/support/support.mox\nimport \"support.mox\";\n\napplication SupportDesk {\n    base nz.example.support\n    module desk {\n        entity Ticket repository TicketRepository {\n            findById;\n            findAll;\n            save;\n            delete;\n        }\n        machine state: TicketState {\n            initial Open;\n            Open -> InProgress via Claim;\n            InProgress -> Resolved via Resolve;\n            terminal Resolved;\n        }\n    }\n}\n```\n";
        let mut findings = Vec::new();
        check_fences(
            &repo_root(),
            Path::new("docs/guide/ddd/tutorials/page.md"),
            &scan_structure(body, 1),
            &mut findings,
        );
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_ddd_example_without_a_fixture_is_a_finding() {
        let body =
            "# T\n\n```ddd\nimport \"support.mox\";\n\napplication A {\n    base a.b\n}\n```\n";
        let mut findings = Vec::new();
        check_fences(
            &repo_root(),
            Path::new("docs/guide/ddd/tutorials/page.md"),
            &scan_structure(body, 1),
            &mut findings,
        );
        assert!(
            findings
                .iter()
                .any(|finding| finding.message.contains("must name its domain")),
            "{findings:?}"
        );
    }

    #[test]
    fn a_ddd_invalid_example_must_fail_with_the_named_code() {
        let body = "# T\n\n```ddd,invalid=E0364,fixture=examples/support/support.mox\nimport \"support.mox\";\n\napplication SupportDesk {\n    base nz.example.support\n    module desk {\n        abstract value RefundDecision\n    }\n}\n```\n";
        let mut findings = Vec::new();
        check_fences(
            &repo_root(),
            Path::new("docs/guide/ddd/how-to/page.md"),
            &scan_structure(body, 1),
            &mut findings,
        );
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn an_ifml_example_compiles_against_its_fixture() {
        let body = "# T\n\n## A simple example\n\n```ifml,fixture=examples/support\nimport \"support.mox\";\n\nview \"Inbox\" {\n    component \"list\" {\n        type: list;\n        data: Ticket;\n        fields: [ticketNo, title];\n    }\n}\n```\n";
        let mut findings = Vec::new();
        check_fences(
            &repo_root(),
            Path::new("docs/guide/ifml/tutorials/page.md"),
            &scan_structure(body, 1),
            &mut findings,
        );
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn an_ifml_authorization_reference_compiles_with_its_actor_policy() {
        let body = "# T\n\n```ifml,fixture=examples/support\nimport \"support.mox\";\nimport \"support.actor\";\n\nview \"Queue\" {\n    requires: [ResolveTicket];\n    component \"details\" {\n        type: details;\n        data: Ticket;\n    }\n}\n```\n";
        let mut findings = Vec::new();
        check_fences(
            &repo_root(),
            Path::new("docs/guide/ifml/how-to/page.md"),
            &scan_structure(body, 1),
            &mut findings,
        );
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn an_ifml_invalid_example_must_fail_with_the_named_code() {
        let body = "# T\n\n```ifml,invalid=E0100,fixture=examples/support\nimport \"support.mox\";\n\nview \"Home\" {\n    component \"list\" {\n        type: list;\n        data: Unknown;\n    }\n}\n```\n";
        let mut findings = Vec::new();
        check_fences(
            &repo_root(),
            Path::new("docs/guide/ifml/how-to/page.md"),
            &scan_structure(body, 1),
            &mut findings,
        );
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_dmn_example_compiles_with_its_external_table() {
        let body = "# T\n\n```dmn,fixture=examples/support\nimport \"support.mox\";\n\ndecision_model RefundDecisions {\n    namespace nz.example.support.decisions;\n    inputs {\n        requested_cents: int;\n    }\n    decisions {\n        decision RefundAuthority {\n            outputs {\n                requires_approval: boolean;\n                approver_role: string;\n                maximum_amount: int;\n            }\n            logic table \"./tables/refund-authority.md\"\n        }\n    }\n    requirements {\n        information {\n            RefundAuthority requires requested_cents;\n        }\n    }\n}\n```\n";
        let mut findings = Vec::new();
        check_fences(
            &repo_root(),
            Path::new("docs/guide/dmn/tutorials/page.md"),
            &scan_structure(body, 1),
            &mut findings,
        );
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_dmn_missing_decision_table_is_a_finding() {
        let body = "# T\n\n```dmn,fixture=examples/support\nimport \"support.mox\";\n\ndecision_model M {\n    namespace nz.example.m;\n    inputs {\n        requested_cents: int;\n    }\n    decisions {\n        decision D {\n            output boolean;\n            logic table \"./tables/nope.md\"\n        }\n    }\n}\n```\n";
        let mut findings = Vec::new();
        check_fences(
            &repo_root(),
            Path::new("docs/guide/dmn/how-to/page.md"),
            &scan_structure(body, 1),
            &mut findings,
        );
        assert!(
            findings
                .iter()
                .any(|finding| finding.message.contains("cannot read the decision table")),
            "{findings:?}"
        );
    }

    #[test]
    fn a_dmn_invalid_example_must_fail_with_the_named_code() {
        let body = "# T\n\n```dmn,invalid=E0204,fixture=examples/support\nimport \"support.mox\";\n\ndecision_model Bad {\n    namespace nz.example.bad;\n    inputs {\n        ticket: Ticket;\n        ticket: Ticket;\n    }\n}\n```\n";
        let mut findings = Vec::new();
        check_fences(
            &repo_root(),
            Path::new("docs/guide/dmn/how-to/page.md"),
            &scan_structure(body, 1),
            &mut findings,
        );
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_dmn_example_loads_its_event_contract_for_the_emits_rule() {
        let body = "# T\n\n```dmn,fixture=examples/support\nimport \"support.mox\";\nimport \"support.evt\";\n\ndecision_model RefundEvents {\n    namespace nz.example.support.decisions;\n    inputs {\n        requested_cents: int;\n    }\n    decisions {\n        decision Raise {\n            output boolean;\n            logic expression \"requested_cents > 0\"\n            emits {\n                TicketRaised;\n            }\n        }\n    }\n}\n```\n";
        let mut findings = Vec::new();
        check_fences(
            &repo_root(),
            Path::new("docs/guide/dmn/concepts/page.md"),
            &scan_structure(body, 1),
            &mut findings,
        );
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn real_guide_tree_passes_the_gate() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let root = root.canonicalize().unwrap();
        let glossary_path = root.join("docs/documentation/glossary.yaml");
        let glossary = parse_glossary(&fs::read_to_string(glossary_path).unwrap()).unwrap();

        let mut findings = Vec::new();
        let mut pages = Vec::new();
        let mut files = Vec::new();
        collect_markdown(&root.join("docs/guide"), &mut files).unwrap();
        assert!(!files.is_empty());
        for file in files {
            if let Some(facts) = analyze_page(&root, &file, &glossary, &mut findings) {
                pages.push(facts);
            }
        }
        check_language_coverage(&pages, &mut findings);
        check_template_drift(&root, &mut findings).unwrap();
        assert!(findings.is_empty(), "{findings:#?}");
    }
}
