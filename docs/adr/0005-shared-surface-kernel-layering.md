# 0005 — The surface kernel is already shared; the model stays typed per surface

- **Status:** Accepted, 2026-11-17
- **Issue:** pherolang#329 (SK1, epic pherolang#328)
- **Follows:** [0003](0003-compiler-backed-highlighting-is-the-source-of-truth.md)
  (the word-table ownership this ADR confirms), epic pherolang#117 (the
  import-gathering convergence this ADR measures rather than restarts)
- **Does not revise** [0001](0001-rebrand-to-phero-lang.md),
  [0002](0002-static-hover-reference-panels.md), or
  [0004](0004-supported-platforms-linux-only.md). No decision here reopens a
  prior ADR.

## Context

Epic SK exists because nine surfaces (`mox`, `ifml`, `ddd`, `actor`, `evt`,
`deploy`, `uc`, `cmmn`, `dmn`) look like nine independent languages and a
reader reasonably asks why they do not share machinery. They already share
most of it. What is left is convergence work on four seams — diagnostics,
import discovery, compile inputs, surface identity — plus `.ifml`'s parallel
interfaces. The unsequenced proposal preserved in pherolang#328 proposed a
much larger reorganization (a crate tree, a universal model enum, a universal
reference type, spans on every model node, a meta-language). Reviewed against
the tree, most of that machinery exists under other names, and the remainder
would trade working structure for churn.

**What the surfaces already share**, with the file that proves it:

| Shared thing | Owner | Measure |
|---|---|---|
| One chumsky input, `kw`, nesting/junk helpers, `ParseError` | `crates/phero-parser/src/` (`input.rs`, `combinators.rs`, `nesting.rs`, `grammar_doc.rs`) | 398 lines |
| One `.mox`-family lexer, one `Token` table | `crates/phero-syntax/src/lexer.rs` | 699 lines |
| One public word-table registry (nine surface views) | `crates/phero-syntax/src/words.rs` | 538 lines |
| Shared AST vocabulary (`Name`, `QualifiedName`, `TypeRef`, `Multiplicity`, `Constraint`, `Import`) | `crates/phero-syntax/src/ast/shared.rs` | 470 lines |
| Shared lexical parser combinators | `crates/phero-syntax/src/parser/shared.rs` | 350 lines |
| One typed-expression engine, one `DomainTypes` class/named-type index | `crates/phero-expr/src/checker/`, `checker/context.rs` | 1,904 lines |
| One `when`/condition skeleton behind six surfaces | `crates/phero-driver/src/conditions.rs` | 457 lines |
| One domain-union lowering (`LoweredDomains`, `import_matches`) | `crates/phero-driver/src/union.rs`, `engine_common.rs` | 231 + 369 lines |
| One engine tail (`engine_diags`, `engine_finish`) | `crates/phero-driver/src/pipeline.rs` | 56 lines |
| One surface table (`SurfaceKind`, 9 variants, `of_extension`/`of_path`) | `crates/phero-driver/src/surface.rs` | 146 lines |
| One per-surface CLI dispatch (`compile_one`, 9 exhaustive arms, 17 lines) and one per-surface result shape | `crates/phero-cli/src/commands/pipeline.rs:143`, `crates/phero-driver/src/compilation.rs` (`compilation_struct!` ×6) | 595 + 254 lines |
| One shared result tail (`entry::finish_surface`, 7 call sites) | `crates/phero-driver/src/entry/mod.rs:12` | 47 lines |
| One runtime resolution table (`Symbols`) and one span-table contract (`SurfaceIndex`) | `crates/phero-driver/src/graph/resolve.rs:45`, `navigation/surface.rs:327` | 361 + 456 lines |
| One diagnostic-code registry (110 entries, 8 bands) | `crates/phero-diagnostics/src/` | 1,775 lines |
| Eleven typed wire roots, each with its own `formatVersion` gate | `crates/phero-ir/src/` (`lib.rs:195–205`) | 10,373 non-test lines |
| Graph as one derived, versioned artifact and an `artifact check` kind | `crates/phero-driver/src/graph/`, `crates/phero-ir/src/graph.rs` | 6,976 + 703 lines |

**What is actually duplicated** is narrow and named (the register below).

**Audit method.** Read-only, on `epic/sk-1-3` (= `origin/main` at the time of
writing, `4627086`). Every LOC figure is `wc -l` over the named file or
directory on that commit; in-source `#[cfg(test)]` modules are called out where
they distort a
number (`phero-lsp` 7,467 of 13,076 lines are in `server/tests/`;
`phero-ifml/src/check/tests/` is 2,693 of the 4,958 `check/` lines;
`phero-ir` carries 2,723 test lines in `tests.rs` + `ddd/tests.rs`). Claims
about ownership were checked by reading the module docs, not only the file
names. Counts of parameters, entries and call sites are `rg` counts, quoted
with the pattern shape in prose.

### Measured duplication register

| ID | Duplication | Where (measured) | Owner |
|---|---|---|---|
| DR1 | Two `Severity` enums with identical variants | `crates/phero-diagnostics/src/lib.rs:59` (21 lines with its impl) vs `crates/phero-driver/src/diagnostic.rs:12` (8 lines) · 10 lines use `Severity::` in driver `src/`, 11 in `phero-ifml` | SK2 |
| DR2 | Two diagnostic record shapes, plus two host converters between them | `phero-ifml/src/resolve.rs:111` `IfmlDiagnostic` (`code: &'static str`, `span: (usize, usize)`, `file: String`, 47 lines with constructors) vs `phero-driver/src/diagnostic.rs:162` `Diagnostic` (`span: Option<Span>`, `code: Option<DiagnosticCode>`, 13 lines) · converters at `phero-cli/src/report.rs:100–133` (34) and `phero-lsp/src/server/pipeline.rs:972–997` (26) | SK2 |
| DR3 | Two independent surface-identity tables, and three path→surface decoders layered on them | `phero-driver/src/surface.rs` (146) vs `phero-highlight/src/surface.rs` (202) · `phero-lsp/src/server/mod.rs:57` `surface_for`, `server/graph.rs:1002` `surface_of_path`, `server/semantic_tokens.rs:253` `surface_of_uri` (two of them decode the same `Uri` to the two different enums) | SK5 |
| DR4 | Per-surface import discovery (8 gatherers) and per-surface disk readers (10 readers) | `phero-driver/src/project.rs` 1,157 · `phero-cli/src/inputs.rs` 1,022 | SK3 |
| DR5 | Near-identical gather-result shapes | `UcBuckets` `project.rs:656` (4 domain/artifact fields) vs `CmmnBuckets` `:730` (the same 4 + `decisions`) · `DeployPairs` `:853`, `DddAppImports` `:550` and `IfmlProject` `:104` each carry their own `missing`/`cycle`/`unresolved` spelling | SK3, SK4 |
| DR6 | Raw compile-input slices instead of a typed bundle | 40 `&[(String, …)]` parameters across 13 public entry functions in `crates/phero-driver/src/entry/*.rs` (`compile_str` takes `DomainImports` alone; `.ifml` has no driver entry point) | SK4 |
| DR7 | `.ifml`'s parallel diagnostic, import and span interfaces | `IfmlDiagnostic` (above), `IfmlImports` `phero-ifml/src/resolve.rs:72`, `IfmlIndex` `phero-ifml/src/index.rs` (716) | SK6 |
| DR8 | Seven identical `lex → parse → map errors` tails, 23 recovery parsers | `parser/{ddd,deploy,evt,uc,cmmn,dmn,actor}.rs` tails: 25–28 lines each (183–199 total) · the 23 `junk_*` parsers are per-grammar stop sets and are *not* shareable | unfiled (see Consequences) |

### Corrections to the review's seed inventory

Four claims in pherolang#329's seed evidence were re-measured and need
adjusting; they are recorded here so SK2–SK6 cite the measured version.

1. **There are two `Severity` enums, not three.** `IfmlDiagnostic` re-exports
   the registry's class (`phero-ifml/src/resolve.rs:51`,
   `pub use phero_diagnostics::Severity;`), so `.ifml` and the registry agree
   by construction. The split is registry vs driver.
2. **Raw buckets are not in *every* `compile_*_str`.** Thirteen public entry
   functions take 40 raw slice parameters between them, but `compile_str`
   (`entry/mox.rs:158`) takes `DomainImports` alone, `compile_files`
   (`:136`) takes the multi-file domain list rather than import buckets, and
   `.ifml` never crosses a driver entry point at all.
3. **`project.rs` is a converged substrate, not pre-convergence duplication.**
   Its module docs record Phase B of pherolang#117: two fetch shapes
   (two-key for `.ifml` walks, path-keyed elsewhere), one gather per surface,
   message strings pinned by CLI tests. What remains is the shape residue
   (DR5) and the fact that the CLI still owns a parallel reader layer (DR4).
4. **Surface identity has *three* decoders, not two.** Besides the two enums,
   the LSP decodes a document path three times: `surface_for` (`Uri` → driver
   kind), `surface_of_path` (`Path` → driver kind) and `surface_of_uri` (`Uri`
   → highlighter surface), the last wrapping the *other* enum.

The proposal's expectation that "30–50% of the implementation machinery can
become genuinely shared" is not supported by measurement. The kernel is
already shared; the deletable duplication is bounded by roughly
1,157 + 1,022 (import discovery and readers) + 348 (surface identity) +
~130 (diagnostic shape and converters) + ~190 (parser tails) ≈ **2,850 lines**,
about 2% of the workspace's ~142k lines of `src` (~227k including `tests/`).
That is an upper bound on deletable code, not a lower bound on the work.

## Decision

### D1 — Typed per-surface artifacts remain the wire IR

No universal `PheroModel`, no `ModelKind`, no universal node enum. Eleven
roots, each with an independent `formatVersion` marker, are the contract
(`crates/phero-ir/src/lib.rs:195–205`); `SurfaceKind` is how a host names which
one it holds (`crates/phero-driver/src/surface.rs`).

*Rationale.* A universal enum cannot grow additively: serde rejects unknown
tags, so a new surface or node kind becomes a wire break for every consumer,
and every golden artifact under `tests/conformance/` becomes unstable by
construction. The graph already proves the additive alternative works — its
kind vocabulary is open *strings* (`phero-ir/src/graph.rs`, `kinds`), precisely
so it can grow without a break.

### D2 — No `phero-core` / `phero-model` / `phero-symbols` / `phero-graph` / `phero-compiler` crates

The jobs are owned: `phero-parser` (span, tokens, parser input), `phero-ir`
(wire types), `phero-diagnostics` (codes), `phero-expr` (expressions),
`phero-driver` (resolve, lower, graph, navigation).

*Rationale.* Measured against DR1–DR4 in the register, each proposed crate would
either be empty or create a cycle. `phero-driver` already depends on
`phero-syntax`, `phero-ifml`, `phero-expr`, `phero-diagnostics` and `phero-ir`
(the `[dependencies]` block at `crates/phero-driver/Cargo.toml:12`), so a
`phero-compiler` above it has no dependents; a `phero-graph` below it would
need `phero-ir` plus the driver's
resolvers and would then be depended on by the driver — the same crate, split
by a manifest. The one honest extraction candidate is the *shared parser
scaffolding* inside `phero-syntax` (DR8), and it is measured, not assumed.

### D3 — `phero-syntax` stays one crate

One lexer (`lexer.rs`), one token table (`Token`), one parser input
(`phero_parser::Tokens`), one word-table and highlight registry (`words.rs`,
and the highlighter's registry in `phero-highlight` per
[ADR 0003](0003-compiler-backed-highlighting-is-the-source-of-truth.md)), one
formatter family (`src/fmt/`, 6,324 lines).

*Rationale.* The three invariants this epic must not break are inside that
crate: (a) the EBNF authority for the whole `.mox` family lives once in
`parser/mod.rs`'s module docs (lines 1–890 of that file); (b) `words.rs` is
derived from the lexer and the parser tables and its tests fail on drift;
(c) the highlighter and the grammar generator consume the same tables. Nine
per-surface crates would make (a) a nine-way cross-reference, (b) nine drift
tests, and (c) a dependency fan-out with no consumer.

### D4 — Wire artifacts stay span-free

Locations live in the syntax ASTs and in the per-surface span tables
(`crates/phero-driver/src/navigation/`, 4,545 lines). There is no `Span` in
`crates/phero-ir/src` (checked: zero matches outside test modules).

*Rationale.* Artifacts are hashed, receipted and published: adding spans makes
every artifact change whenever a comment or blank line moves, breaks
byte-identical goldens and the verification receipts' `sources`/artifact
hashes, and hands every backend a coordinate system it does not share with any
other file. The graph proves the alternative: it carries the host-resolved
display path and nothing else (`docs/GRAPH.md`), and the editor gets locations
from the span tables keyed by the graph's own node keys
(`navigation/surface.rs`).

### D5 — Graph remains a derived, versioned projection

It is already a standalone artifact (`GRAPH_MODEL_FORMAT_VERSION`), an
`ArtifactKind::GraphModel` in `pherolang artifact check`, and is never parsed
back into a compiler input.

*Rationale.* The proposal's "graph becomes the real Phero IR" inverts the one
direction that makes the graph cheap: it is built **post-compile** from
already-computed typed artifacts plus host-resolved imports
(`phero-driver/src/graph/mod.rs:1–40`), so it can be best-effort over a
partially compiling tree and can never become a second parser. Making it the
source IR would require the graph to carry everything the artifacts carry
(including their per-surface version gates) and would put a projection in the
position of the thing projected.

### D6 — `.ifml` stays a separate crate; the boundary is documented, not reversed

`phero-driver` depends on `phero-ifml` (`Cargo.toml:16`), not the reverse.
`.ifml` keeps its own logos lexer + chumsky parser, its own spanned AST and its
own `IfmlIndex`.

*Rationale.* Dependency direction. `phero-ifml` depends on `phero-parser`,
`phero-ir`, `phero-expr` and `phero-diagnostics` and only *dev*-depends on the
driver; reversing it would make the nine-surface driver depend on a crate that
depends back on it. SK6 is deletions at the seams (diagnostic, import, surface
identity) plus a recorded carve-out — not a parser migration.

### D7 — Meta-language and universal annotations are out of scope

No Phero Meta Model, no compiler generator, no universal `@annotation`
infrastructure in this epic. Annotations are `.mox` package-level today
(`LANGUAGE.md`), and a universal system is a feature, not a refactor.

*Rationale.* A meta-language is a product bet that would rewrite the
grammar authority (D3) into generated parsers and make the EBNF a build
artifact. Nothing in the measured duplication (DR1–DR8) is caused by the absence
of one, so it cannot be justified as convergence work.

## Layer inventory

Ten layers, as the issue scopes them. "Shared today" and "surface-local" are
about *code ownership*, not about whether a concept appears on several
surfaces. LOC is `wc -l` on the named path.

| Layer | Owning files (measured LOC) | Genuinely shared | Stays surface-local | Disposition |
|---|---|---|---|---|
| **Parse** | `phero-parser/src/` 398; `phero-syntax/src/lexer.rs` 699; `words.rs` 538; `parser/` 10,190 in 12 files (`ddd` 2,768 · `dmn` 1,282 · `mod` 1,101 · `cmmn` 886 · `evt` 865 · `deploy` 757 · `mox` 710 · `uc` 678 · `actor` 436 · `shared` 350 · `docs` 238 · `imports` 119); contextual word tables `ddd.rs` 583 · `dmn.rs` 99 · `cmmn.rs` 72 · `deploy.rs` 48 · `usecases.rs` 46; `.ifml` `lexer.rs` 307 + `parser/` 2,328 + `words.rs` 78 | one `Token` table, one `Tokens` input, `kw`, `shared.rs` combinators, the junk-region recovery contract, the 7-way identical `parse_*` tail (183–199 lines) | every grammar production; the 23 `junk_*` stop-sets (surface-specific by definition); `.ifml`'s own lexer/parser | **adopt** one crate / one lexer (D3); **defer** the ~190-line tail as a measured, unfiled candidate (DR8) |
| **AST + spans** | `phero-syntax/src/ast/` 2,916 in 9 files (`ddd` 879 · `mox` 480 · `shared` 470 · `dmn` 286 · `cmmn` 204 · `deploy` 204 · `uc` 191 · `evt` 165 · `mod` 37); `phero-ifml/src/ast.rs` 646; `Span`/`ParseError` from `phero-parser` | `shared.rs` vocabulary; `Span` is byte-offset and universal *in the AST* already (`Name { text, span, escaped }` and every declaration carries spans) | per-surface node types and their parsing | **reject** the `Node<T> { value, span }` universal wrapper; **reject** spans in wire artifacts (D4) |
| **Resolution** | `phero-expr/src/checker/` 1,904 (`context.rs` 209 = `DomainTypes`; `mod` 286 · `ops` 364 · `navigation` 408 · `algebra` 235 · `shared` 220 · `lints` 182) + `parser.rs` 482 + `stmt.rs` 832 + `body.rs` 441; `phero-driver/src/graph/resolve.rs` 361 (`Symbols`, `Symbol { id, name, kind, file, application }`); `lower/resolve.rs` 311 + `lower/names.rs` 301; `phero-ifml/src/resolve.rs` 670 | `DomainTypes` for every surface's typed expressions; `Symbols` for the graph's union-wide, per-application resolution; `NavigationIndex`/`SurfaceIndex` for spans | scope construction per surface (self class, machine states, injections, per-event and synthetic settings universes) | **adopt** as implemented; **reject** `Ref<T>` as a wire type (§5) |
| **Validation** | `phero-driver`: `deploy.rs` 1,571 · `usecases.rs` 1,391 · `cmmn.rs` 1,047 · `dmn.rs` 918 · `events.rs` 515 · `ddd/` 9,058 (15 files; `services` 1,198 · `mod` 1,051 · `dtos` 929 · `graph` 908 · `designs` 892 · `bodies` 753 · `machines` 653 · `searches` 500 · `errors` 438 · `conditions` 438 · `child_ops` 403 · `queries` 335 · `links` 250 · `values` 168 · `invariants` 142) · `lower/` 5,670 (8 files) · `integrity.rs` 773; `phero-ifml/src/check/` 4,958 (2,693 in-src tests) | `conditions.rs` 457 (the six-surface `when` skeleton), `engine_common.rs` 369 (import-hit tables, union types, actor queries, multiplicity mapping), `pipeline.rs` 56, `union.rs` 231 | every semantic rule; the pinning of diagnostic order, wording and span policy | **adopt** the existing shared helpers; **extract** only the compile-input bundle (SK4), never a validation framework |
| **Diagnostics** | `phero-diagnostics/src/` 1,775 in 10 files, 110 entries, 8 bands incl. `driver.payload`; `phero-driver/src/diagnostic.rs` 392; `IfmlDiagnostic` in `phero-ifml/src/resolve.rs:111`; reporters `phero-cli/src/report.rs` 158 | one registry and allocation ledger; one rendering (`ariadne`, colourless, sorted by span) used by CLI and LSP | the driver's payload-carrying `DiagnosticCode`; `.ifml`'s structured `code` + `file` | **extract** one record and one severity (SK2) — bounded by the cycle in Consequences |
| **Imports** | `phero-driver/src/imports.rs` 234 (`DomainImports`, `SchemaImports`, `SigilImports`); `project.rs` 1,157 (8 gatherers); `workspace_imports.rs` 216; `phero-cli/src/inputs.rs` 1,022 (10 readers); `phero-cli/src/project.rs` 838 (snapshot + cache); `IfmlImports` + `walk_ifml_imports` in `phero-ifml/src/resolve.rs` | the two fetch shapes and the gatherer substrate (Phase B of pherolang#117); `DomainImports` as the content-provider shape | reader policy (the CLI hard-errors, the LSP degrades); the as-written vs resolved key rules per surface | **extract** one closure walker (SK3); **adopt** `DomainImports` |
| **Span/navigation** | `phero-driver/src/navigation/` 4,545 in 12 files (one is a 54-line test helper; `mox` 896 · `ddd` 731 · `surface` 456 · `dmn` 447 · `cmmn` 445 · `deploy` 356 · `actors` 301 · `usecases` 295 · `ifml` 257 · `evt` 229 · `mod` 78); `phero-ifml/src/index.rs` 716 | `SurfaceIndex`/`IndexedDefinition`/`IndexedReference` contract; the `(kind, name)` key shared with the graph's node ids; three documented exclusions | every builder, and each surface's coverage choices | **adopt** the contract; builders stay local |
| **LSP** | `crates/phero-lsp/src/` 13,076 (7,467 in `server/tests/`): `graph.rs` 1,576 · `pipeline.rs` 1,007 · `ifml.rs` 846 · `mod.rs` 720 · `hover.rs` 465 · `semantic_tokens.rs` 371 · `position.rs` 327 · `mox.rs` 266 | one directory-scoped graph snapshot per open `(uri, version)` set; diagnostics are each surface's own compile; `position.rs` byte→UTF-16 | per-surface publish arms; the `.ifml` one-shot path | **adopt**; **extract** only the surface identity (SK5) — three decoders today (DR3) |
| **Pipeline** | `phero-driver/src/surface.rs` 146 · `compilation.rs` 254 (`compilation_struct!` ×6 + manual `DeployCompilation`) · `entry/` 2,036 in 9 modules · `entry/mod.rs:12` `finish_surface` (7 call sites); `phero-cli/src/commands/pipeline.rs` 595 (`compile_one` at 143–159) · `report.rs` 158 | the surface table, the dispatch, the shared tail, the result macro, the reporting helpers | each arm's engine call and each reporter's source lookup | **adopt** as shipped; **reject** the `Language` trait with associated types (§11) |
| **Wire artifacts** | `phero-ir/src/` 13,096 (10,373 non-test) — 11 roots + `types.rs` 235 (`TypeRef` used by 12 non-test modules) + `wire.rs` 505; `phero-ir/src/graph.rs` 703; graph builder `phero-driver/src/graph/` 6,976 (`analytics` 1,509 · `index` 941 · `query` 801 · `ddd` 721 · `mod` 623 · `ifml` 586 · `resolve` 361 · `ids` 261 · …) | `TypeRef`/`Multiplicity`/`DefaultValue`, the wire contract (casing, adjacent tagging, version gate, 406 `#[serde(default` attributes), the open graph kind vocabulary | every root's fields and its own version marker | **adopt** typed roots (D1); **reject** a universal enum |

## Disposition of pherolang#328's original proposal

One row per section of the preserved proposal. These are dispositions of
*proposals*, so a "reject" here means "no further work in this epic", not
"this is wrong for all time"; the reason column says what would have to change.

| § | Topic | Disposition | Reason / where it lives today |
|---|---|---|---|
| 1 | Split Phero into four conceptual layers (Syntax / Model / Domain / Behaviour / Interaction) | **reject** as a physical split | The layers are already real as crate responsibilities — `phero-parser` → `phero-syntax`/`phero-expr` → `phero-driver` → `phero-ir`; a literal `Phero Syntax`/`Phero Model` crate pair is the §4 tree (D2). |
| 2 | A `PheroModel` trait + `ModelKind` + one shared semantic vocabulary | **reject** | Eleven typed roots + `SurfaceKind` are the contract (D1); a closed `ModelKind` cannot grow additively. The *value* vocabulary (`TypeRef`, `Multiplicity`, `DefaultValue`) is already shared in `phero-ir/src/types.rs`. |
| 3 | `.ddd`/`.ifml` reveal common semantic primitives (Entity→NamedElement, Navigation→Transition, …) | **reject** the type-level unification | The observed overlap is naming, not structure: the shared primitives that do exist (name, type ref, multiplicity, expression, constraint) already live once in `ast/shared.rs` and `phero-expr`. Merging `Navigation` and `Transition` would put two artifact shapes behind one type for no consumer. |
| 4 | Create a `phero-core` / `phero-model` crate tree (`phero-resolve`, per-surface crates) | **reject** | D2. Every proposed crate either has no dependents above the driver or would cycle with it; per-surface crates also break the one-lexer and one-word-table invariants (D3). |
| 5 | A common `Reference` (`Ref<T> { name, resolved: Option<Id<T>> }`) | **reject** as a wire type; **adopt** at runtime | Resolution is already shared at runtime by `graph::resolve::Symbols` (`graph/resolve.rs:45`) and expressed in the editor surface as `IndexedReference { target_kinds, … }`. Putting `Ref<T>` in artifacts would make every artifact change when a name resolves differently, and `Id<T>` has no stable identity across surfaces. |
| 6 | A shared symbol table (`SymbolTable`, `Symbol { id, name, kind, span }`) | **adopt** as implemented, no new type | The graph's `Symbol` carries `id`, `name`, `kind`, `file`, `application` (`graph/resolve.rs:30`); the `span` half is `IndexedDefinition` (`navigation/surface.rs`). One table plus one span table keyed by the same node key is the shipped design; adding a third would be the drift the navigation anti-drift gate exists to prevent. |
| 7 | A `ModelContext` with eight registries, `compile_x(source, context)` | **defer** to SK4 | SK4 is the inert bundle. The registries are exactly the over-abstraction risk the epic names: if the bundle answers resolution questions it has become a second `DomainTypes`/`Symbols`. |
| 8 | One shared expression language, shared type checking | **adopt** (already shipped) | `phero-expr` (`DomainTypes` + `TypeChecker`) types every surface's conditions and bodies, and `phero-driver/src/conditions.rs` is the one six-surface `when` skeleton. `docs/EXPRESSIONS.md` is the normative contract. |
| 9 | A universal annotation system (`@deprecated`, `@route`, …) | **reject** for this epic | Annotations are `.mox` package-level today; a universal mechanism is a language feature with wire consequences, not convergence work. Nothing in DR1–DR8 is caused by its absence. |
| 10 | Universal source locations (`Node<T> { value, span }` on every AST node) | **reject** | The ASTs are already spanned — every `Name`, declaration and reference carries a byte span (`phero-syntax/src/ast/shared.rs:8`). The wrapper is a whole-tree rewrite that adds no location and would push spans toward the artifacts, which D4 forbids. |
| 11 | A common compiler pipeline with a `Language` trait (`type Ast; type Model; parse/lower/validate`) | **adopt** the pipeline; **reject** the trait | The pipeline exists as `SurfaceKind` + `compile_one` (9 exhaustive arms) + `entry::finish_surface` + `compilation_struct!`. The associated-type trait cannot express the real differences: `.deploy` has no `domains_model`, `.mox` is multi-file, `.ifml` never enters a driver entry point, and each engine pins its own diagnostic order. |
| 12 | A Phero Meta Model / meta-language compiler generator | **reject** | A product research spike, not this epic. It would turn the EBNF authority (`parser/mod.rs`) and the word tables into generated artifacts and replace the hand-written formatters (6,324 lines) — a rewrite with no measured duplication behind it. |
| 13 | Distinguish three classes of shared concept (infrastructure / semantic primitives / domain concepts) | **adopt** as the placement rule | Level 1 → `phero-parser`, `phero-syntax`, `phero-ir`, `phero-expr`, `phero-diagnostics`; Level 2 only where the typed shape is genuinely identical (name, type ref, expression, constraint); Level 3 always per surface. This is the rule D2/D3 follow, stated once. |
| 14 | A first-class common graph model; "the real Phero IR" | **adopt** the graph as a first-class derived artifact; **reject** "graph is the source IR" | It already is first-class: a versioned artifact (`GRAPH_MODEL_FORMAT_VERSION`), an `artifact check` kind, `graph build`/`query`/analytics. D5 records why the direction stays one-way. |
| 15 | Refuse one universal IR: typed specialized models + shared semantic infrastructure + a graph of relationships | **adopt** | This is D1 + D5 + D6 stated as one sentence, and it is what the tree already does. No code change follows; the ADR makes it citable. |
| 16 | A concrete refactoring target workspace (`phero-core`, `phero-symbols`, `phero-model`, `phero-graph`, `phero-compiler`, nine per-surface crates, `ddd → model → core`) | **reject** | D2 + D3. Its proposed direction (`ddd → model → core`) inverts the tree's own: the driver is *above* the syntax crates and *below* the backends, and `.ddd` is a parser module plus a driver engine, not a crate. |
| 17 | Second-order benefit: "Phero is a modelling system with multiple domain-specific views", and audit before rewriting | **adopt** the framing and the order; no code consequence | The framing is already the docs' language (`docs/README.md`), and this ADR is the audit it asks for first. The claim that 30–50% of the machinery can become shared is corrected by measurement (Context). |

**Rejected proposals, stated once for future proposers.** Universal model enum
(`PheroModel`/`ModelKind`) → wire breaks and unstable goldens. Per-surface
crates → three invariants inside `phero-syntax` (one lexer, one word table, one
EBNF authority) become nine-way drift. Spans in wire artifacts → hashes,
receipts and byte-identical goldens all become input-sensitive. Meta-language /
compiler generator → rewrites the grammar authority and the formatters without
removing measured duplication. `phero-core`/`phero-model`/`phero-symbols`/
`phero-graph`/`phero-compiler` → empty or cyclic against the existing
dependency graph. `Ref<T>` as a wire type → resolution state in artifacts,
unstable identity, no consumer.

## Consequences

**Settled; SK2–SK6 may cite this ADR instead of re-arguing:**

- The kernel exists; the remaining work is five seams, seven measured
  duplications (DR1–DR7), not a reorganization. Each number is in the register
  above and can be re-measured with `wc -l`.
- No new crate, no universal enum, no spans in artifacts, no meta-language, no
  per-surface crate split. A proposal that needs one of those needs a new ADR
  that supersedes this one, not a PR.
- `.ifml`'s carve-out is real and directional: `phero-driver → phero-ifml`.
  SK6 deletes parallel interfaces at the seams; it does not move the parser.
- A "behavior-preserving" refactor that changes any artifact byte has changed
  behavior (the epic's stop signal). This ADR adds no exception.

**Explicitly open, in epic order:**

1. **SK2 — record and code carrier.** A shared record cannot own
   `phero_driver::DiagnosticCode`: `phero-driver` depends on
   `phero-diagnostics` (`Cargo.toml:14`), so putting the payload in the
   registry crate is a cycle; putting it in `phero-ifml` inverts D6; putting a
   shared record in the driver leaves `.ifml` depending on the driver. So SK2
   chooses between (a) the record owns only severity/message/span and the code
   stays a per-surface associated payload, (b) numeric codes become a
   structured field everywhere (today the driver *prefixes* the code into the
   message text — `dmn.rs:147`, `integrity.rs:52`, `lower/names.rs:255` —
   while `.ifml` carries `code: &'static str`), and (c) the file tag moves into
   the record (today it is the `Vec<(String, Diagnostic)>` tuple). All three
   are pre-1.0 acceptable; none is prescribed here.
2. **SK3 — walker policy shapes.** The gatherers differ by contract, not just
   by traversal: the CLI hard-errors on the first missing import, the LSP
   degrades (`project.rs:44–57`), `.ifml` walks two-key while the rest are
   path-keyed, `DeployPairs` carries two artifact lists while `UcBuckets`
   carries four buckets plus the raw domains, and each result type spells its
   own `missing`/`cycle`/`unresolved` fields. The walker must carry policy, or
   it will become the union of the strictest rules.
3. **SK4 — compile-input bundle.** Shape only (DR6: 40 raw slice parameters,
   `DddFragment` and the bucket types), and it must stay inert — no
   resolution, no caching, no global state (the epic's named risk).
4. **SK5 — surface identity.** Which enum wins, and which features move:
   `SurfaceKind` has `of_extension`/`of_path`/`extension`; `phero_highlight::Surface`
   adds serde, `id`, `display_name`, `aliases` and `from_path`. The highlighter
   is released-binary and site-artifact coupled (ADR 0003), so SK5 must not
   churn the highlight wire or the generated grammar package. The LSP's three
   decoders (DR3) are the first deletions.
5. **SK6 — `.ifml` carve-out.** `IfmlDiagnostic`, `IfmlImports` and the LSP's
   `.ifml` pipeline align with SK2/SK3/SK5; the lexer, parser, AST, `IfmlIndex`
   and formatter stay.
6. **Unfiled — parser scaffolding.** The seven `parse_*` tails are measured at
   25–28 lines each (183–199 lines total) and are mechanically identical. That
   is *material but small*: file a new issue citing this ADR if and when someone is already in
   those files; do not open a migration for it. The 23 `junk_*` parsers are
   per-grammar stop sets and are not a candidate.

**Limitations of this audit.**

- LOC is a proxy. `wc -l` counts comments and blank lines, and in-source test
  modules inflate four numbers; those are called out rather than adjusted, so
  the figures stay reproducible from the command in the method paragraph.
- No token-level clone detection and no line-by-line audit of the two largest
  engines (`ddd.rs` 2,768, `dmn.rs` 1,282 in the parser; `ddd/` 9,058 in the
  driver). Duplication *inside* one engine is not classified here; only the
  cross-surface seams are.
- Host duplication was sampled at the reader/dispatch/reporting seams (CLI
  `inputs.rs`, `report.rs`; LSP `pipeline.rs`, `graph.rs`,
  `semantic_tokens.rs`), not exhaustively. A deeper CLI/LSP audit may find more
  than DR3–DR4.
- Nothing was compiled or tested: no build, no fixture regeneration, no artifact
  hash comparison. The claims are structural, and the numbers are `wc -l`.
- `.ifml` internals are described only to the depth SK6 needs; the crate's
  checker and manifest are `docs/IFML.md`'s subject.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Build the `phero-core` / `phero-model` / `phero-symbols` / `phero-graph` / `phero-compiler` tree | Every crate is either empty or cyclic against the current dependency graph (`phero-driver` already sits on `phero-syntax`, `phero-ifml`, `phero-expr`, `phero-diagnostics`, `phero-ir`). It would also scatter the files that pin the invariants — `parser/mod.rs` (EBNF), `words.rs`, `lexer.rs`, the formatters — across crates, separating each from the drift tests that keep it honest. |
| A universal `PheroModel` / `ModelKind` / universal node enum | Closed enums cannot grow additively; a new surface or node kind becomes a wire break, and every golden artifact plus the verification receipts' hashes churn. The graph's open string kinds show the additive shape that works. |
| Nine per-surface crates | The one-lexer, one-word-table and one-EBNF-authority invariants (D3) are the reason new surfaces are cheap today; the split pays compile-time isolation the workspace does not need (a full `cargo test` already runs the same nine surfaces) and charges nine drift tests. |
| Spans on every artifact node | Artifacts are hashed and published; span churn invalidates goldens, receipts and freshness gates, and hands backends a coordinate system no other file shares. Spans belong to the AST and the span tables, which is where they are. |
| `Ref<T>` as a wire type | Resolution differs per host (disk, URI, test closure), so a resolved `Id<T>` would either be unstable across hosts or need a second resolution pass — the runtime tables already do this without touching the wire. |
| A meta-language / compiler generator | The largest single rewrite on the table (grammar, ASTs, formatters, highlighter tables) justified by no measured duplication. It is a product bet, not this epic. |
| Reading the graph back as the source IR | Inverts the one-way projection that lets `graph build` be best-effort over a partially compiling tree; the graph would have to reproduce every root's version gate and semantics. |
| Doing the whole convergence in one change | The five seams touch published driver/ifml/lsp surfaces. Sequencing them (SK2/SK3/SK5 in parallel, then SK4/SK6) keeps each PR's golden diff reviewable and makes "the numbers recorded in the PRs" meaningful (the epic's exit criterion). |
