# 0012 — Rune is an in-tree surface; sigil comes into the monorepo

- **Status:** Accepted, 2026-11-28
- **Issue:** pherolang#392 (RN1, epic pherolang#391)
- **Follows:** [0003](0003-compiler-backed-highlighting-is-the-source-of-truth.md)
  (word tables are owned once, by the parser crates),
  [0005](0005-shared-surface-kernel-layering.md) (one lexer family, word
  registry and formatter stay together in `phero-syntax`; artifacts stay
  typed per surface)
- **Revises** [NAMING.md](../NAMING.md)'s external-toolchain row for sigil
  only. restingstate/xstate-ir, dsntk, and cmmn-rs stay external. No other
  ADR is reopened.

## Context

The FINOS Rune DSL (`.rosetta` files) reaches Phero Lang through one
declaration, `.mox` `import sigil "<path>"`. Its toolchain is
[yestechgroup/sigil](https://github.com/yestechgroup/sigil): six crates in a
separate workspace. pherolang pins four of them by git revision.

All measurements below were taken on pherolang `8e1558e` (`origin/main` when
this was written) and sigil `0e1e4b1` (sigil `main`). Each figure is
`wc -l` over the named file or crate, or an `rg` count.

- **The pin is stale.** pherolang pins `69115a7` (root `Cargo.toml`, the
  `sigil-*` rows). That is six commits and 58 files (+7.9k lines) behind
  sigil `main`: schema/transform parity, `W0101` deprecation warnings, and
  LSP navigation through transform refs.
- **One crate consumes it.** Only `crates/phero-driver/Cargo.toml` depends
  on the sigil crates. The whole contract runs through one bridge module,
  `crates/phero-driver/src/sigil.rs` (1,942 lines).
- **The upstream has one embedder.** sigil's CI is non-functional by its own
  AGENTS.md (Actions minutes exhausted, jobs killed). It has no tags and no
  CHANGELOG, and pherolang is its only consumer. Its
  `docs/api-stability.md` contract serves a boundary that only we cross.
- **Rune gets none of the editor or tooling surface.** `.rosetta` is not in
  the surface table (`crates/phero-parser/src/surface.rs`, 10 variants). It
  therefore has:
  - no `pherolang check`, `ir` or `fmt`;
  - no highlighting and no graph or navigation nodes;
  - LSP support only through the `import sigil` diagnostics routing that
    pherolang#398 added.

The sigil workspace, per crate (`src` lines / test lines):

| Crate | src | tests | Role |
|---|---|---|---|
| `sigil-diag` | 270 | 0 | spans, diagnostics, rendering |
| `sigil-syntax` | 5,271 | 0 | scannerless chumsky 0.13 parser, AST, `lower`, project discovery |
| `sigil-model` | 3,118 | 312 | the Ecore-mirroring IR, span-less `expr::Expr`, visitor |
| `sigil-resolve` | 2,733 | 990 | Xtext-style scoping, builtins, canonical JSON |
| `sigil-cli` | 150 | 385 | the `sigil` binary |
| `sigil-lsp` | 3,785 | 3,238 | a second language server |

### Measured duplication register

| ID | Duplication | Where (measured) | Owner |
|---|---|---|---|
| RD1 | Two span types | `sigil_diag::Span` vs `phero_parser::Span` (both byte offsets, half-open) | RN8 |
| RD2 | Three diagnostic records and two severities; codes are string literals | `sigil_diag::Diagnostic`, `sigil_resolve::ResolutionDiagnostic` (`sigil-resolve/src/lib.rs:30`), `phero_diagnostics::record::Diagnostic` (`record.rs`, 104 lines). Sigil's code table exists only in sigil `docs/compatibility.md`, and its numbers (`E0001`, `E0101`–`E0107`) collide with the `ifml.resolver` / `ifml.check` bands | RN8 |
| RD3 | Eleven hand-written Rune keyword tables in two crates, plus a hand-written TextMate grammar | `sigil-syntax`: `ELEMENT_KEYWORDS`, `UNSUPPORTED_KEYWORDS`, `STOP`, `DOC_REFERENCE_KEYWORDS` (`parser.rs`), `EXPRESSION_KEYWORDS` (`expr.rs`) · `sigil-lsp/src/features.rs`: `RUNE_KEYWORDS`, `RUST_KEYWORDS`, `TOP_LEVEL_KEYWORDS`, `BODY_KEYWORDS`, `TOP_KEYWORDS`, a second `ELEMENT_KEYWORDS` · `editors/vscode/syntaxes/rosetta.tmLanguage.json` (100 lines) | RN9, RN15, RN16 |
| RD4 | Two LSP stacks | `sigil-lsp` on `lsp-server` 0.10 + `lsp-types` 0.97 (3,785 src lines) vs `phero-lsp` on `tower-lsp-server` 0.23 (6,068 non-test lines) | RN15 |
| RD5 | Two position layers | sigil `position.rs` `LineIndex`, UTF-8 preferred (262 lines) vs `crates/phero-lsp/src/position.rs` `PositionMap`, UTF-16 (327 lines) | RN15 |
| RD6 | Three `.rosetta` discovery walks | `sigil_syntax::project::discover_rosetta_files`, `crates/phero-driver/src/workspace_imports.rs` `discover_rosetta_files`, `crates/phero-cli/src/commands/verify.rs:514` `collect_rosetta_candidates` | RN11 (done: one `phero_driver::workspace_imports::discover_rosetta_files`) |
| RD7 | Two chumsky majors in one lock file | `Cargo.lock`: `chumsky 0.10.1` (phero-parser, phero-syntax, phero-expr, phero-ifml) and `chumsky 0.13.0` (sigil-syntax) | RN2, RN3 |
| RD8 | A source-text expression bridge | `sigil.rs` prints the span-less `sigil_model::expr::Expr` as phero-expr text (`translate_expr` at `sigil.rs:1642`, `write_expr`, `fragment`), then re-parses it in `check_body` | RN12 |
| RD9 | Two highlighting sources | the TextMate grammar plus `sigil-lsp/src/semantic_tokens.rs` (723 lines) vs compiler-backed `phero-highlight` (ADR 0003) | RN16 |

**Correction to the epic's seed inventory.** pherolang#391 counted four
keyword lists. Re-measured, there are eleven tables (RD3). Several duplicate
each other inside `sigil-lsp` alone.

## Decision

### D1 — Monorepo; the upstream is archived

The sigil crates move into this repository (RN4, a provenance-preserving
subtree import at the current sigil `main`). The git pin and its `deny.toml`
`allow-git` row are then removed. `yestechgroup/sigil` gets a pointer to the
new home and is archived (RN6).

*Rationale.* There is one consumer, the upstream gate is dead, and every
change already has to cross a rev bump. A separate repository costs us a
pin, a second CI and an API-stability document, and buys an independence
nobody uses.

### D2 — The grammar goes into `phero-syntax`; model and resolve stay crates

| Today | Becomes | Issue |
|---|---|---|
| `sigil-syntax` | the `phero_syntax::rune` module (`parse_rune`, later `format_rune`) | RN9, RN17 |
| `sigil-model` | `phero-rune-model` | RN11 (done) |
| `sigil-resolve` | `phero-rune-resolve` | RN11 (done) |
| `sigil-diag` | retired into `phero-parser` (span) and `phero-diagnostics` (record, codes) | RN8 |
| `sigil-cli` | retired into `phero-cli` (`check`/`build`/`ir` on `.rosetta`) | RN10 |
| `sigil-lsp` | retired into `phero-lsp` (graph-snapshot serving) | RN15 |

*Rationale.* ADR 0005 D3 already decided that one lexer family, one token
table, one word registry and one formatter stay together in `phero-syntax`.
A Rune grammar crate beside it would be the per-surface crate split that
ADR rejected.

The model and the resolver are different. They are a self-contained
semantic layer whose behaviour is pinned by an external oracle (D4), with no
syntax coupling once `lower` moves. Keeping them as crates keeps that
boundary visible.

### D3 — One parser engine: the workspace moves to chumsky 0.13

The workspace moves from chumsky 0.10 up to 0.13, the version sigil already
uses. It does not port sigil down to 0.10 or carry two majors. This happens
in two golden-neutral steps: the kernel (`phero-parser`, `phero-expr`, RN2),
then the surface parsers (RN3). RN2 also decides whether `phero-parser`'s
`unsafe` carve-out can go. It exists only because chumsky 0.10's `Input`
trait requires `unsafe fn` implementations.

*Rationale.* Two majors in one lock file is a permanent tax on every
combinator helper, and the fold in D2 is impossible without one engine.
Upgrading moves the workspace forward instead of making the sigil port
target an older engine.

### D4 — The Java oracle is an opt-in deep gate

The behavioural oracle stays the authority on Rune semantics: `sigil model`
output must match the Java `com.regnosys.rosetta` 9.58.1 EMF dump after
normalisation. It runs from `scripts/ci-local.sh rune-oracle` and a nightly /
on-demand CI job (RN5), together with the CDM corpus snapshot (145 files) and
the CDM oracle differential (81/81 matched).

The committed oracle dumps become ordinary goldens that `cargo test` checks
with no Java. The default gate stays offline and JDK-free.

*Rationale.* "No network in tests" is a standing rule, and a JDK plus about
600 Maven jars on every PR is out of proportion. The dumps catch a
model-output change offline. The live oracle catches drift in the dumps
themselves.

### D5 — Rune diagnostics get their own bands; the codes are renumbered

Three new bands are added to the `phero-diagnostics` ledger. `E0500`–`E0599`
is free: the highest registered band, `driver.intent`, ends at `E0499`. Code
numbers are unique across prefixes, the way `ifml.lints` `W0150`–`W0199` sits
beside `ifml.check` `E0100`–`E0149`.

| Band | Range | Owner |
|---|---|---|
| `rune.syntax` | E0500–E0519 | the `phero_syntax::rune` parser |
| `rune.resolve` | E0520–E0579 | `phero-rune-resolve` |
| `rune.lints` | W0580–W0599 | `phero-rune-resolve` warnings |

The renumbering map:

| sigil code | New code | Meaning |
|---|---|---|
| `E0001` | `E0500` | syntax error |
| `E0101` | `E0520` | unknown type, super type, schema or serialization format, or expression symbol head |
| `E0102` | `E0521` | unknown annotation; `externalConfig` applied to a non-schema element |
| `E0103` | `E0522` | annotation has no such attribute |
| `E0104` | `E0523` | duplicate definition within a kind |
| `E0105` | `E0524` | inheritance cycle |
| `E0106` | `E0525` | reference target of the wrong kind |
| `E0107` | `E0526` | unknown attribute or enum value |
| `W0101` | `W0580` | reference to a `[deprecated]` element |
| `W0001` | — (not migrated) | the dead unsupported-construct skip: `UNSUPPORTED_KEYWORDS` is empty, and RN9 deletes the path |

RN8 does the allocation (ledger rows, `Entry` rows, the generated
`docs/DIAGNOSTICS.md`). This ADR fixes only the numbers.

*Rationale.* The registry is the one code authority, and sigil's numbers
collide with it. Today `pherolang explain E0104` cannot mean the Rune code.
Renumbering is a visible break, and it is acceptable before 1.0.

### D6 — The `.mox` import names the language

`import rune "<path>"` becomes the spelling (RN13). `import sigil` stays a
parse alias, and `pherolang fmt` rewrites it, following the DF4
`extend`→`children` precedent (pherolang#327). The driver vocabulary follows
(`SigilImports` → `RuneImports`, `ImportKind::Sigil` → `ImportKind::Rune`).

*Rationale.* `import schema` names the format, not a tool. Once sigil is an
internal implementation detail, a keyword naming it leaks history into every
model file.

### D7 — Naming

- **Rune** is the language: the FINOS Rune DSL, named the way the
  `.rosetta` files' own community names it.
- **`.rosetta`** is its file kind and becomes a `Surface` row (RN10).
- **sigil** is the historical name of the toolchain. Git history and this
  ADR keep it. No new public name uses it except the `import sigil` alias
  until it is removed.

[NAMING.md](../NAMING.md) stops listing sigil among the external toolchains.

## Consequences

**Settled; the RN issues cite this ADR instead of re-arguing:**

- The order follows the epic's phases:
  - **P0:** this ADR.
  - **P1:** one chumsky (RN2, RN3) and the staging import (RN4), then the
    parity gate (RN5) and the archive (RN6).
  - **P2:** one diagnostic record (RN8), the crate renames and one discovery
    walk (RN11), then the grammar fold (RN9) and the surface row (RN10).
  - **P3:** the editor and tooling issues.
  - **P4:** the documentation close-out (RN18).
- Every behaviour-preserving step keeps artifact bytes. A golden diff is a
  behaviour change and a stop signal. The exceptions are deliberate and land
  in their own commits:
  - the rev bump to sigil `main` in RN4;
  - the renumbered codes in RN8;
  - newly typed Rune bodies in RN12.
- RN9 rewrites the parser, so its fidelity is measured externally: the
  conformance goldens, the committed oracle dumps, and the CDM snapshot.
  Unit goldens alone do not count.
- Rune semantics do not change. The oracle defines scoping and the model.
  Whole-workspace resolution stays unless a benchmark argues for salsa
  (RN11).

**Pre-1.0 breaks, recorded here once:**

- renumbered Rune diagnostic codes (D5);
- the driver's `SigilImports` / `ImportKind::Sigil` API, renamed in RN13
  (coordinate the codegraph pin, as Epic SK did);
- the `sigil` and `sigil-lsp` binaries are removed (RN10, RN15).

**Retired documents:**

- sigil `docs/compatibility.md` is absorbed into `docs/RUNE.md` (RN18).
- sigil `docs/api-stability.md` has no reader once the crates are in-tree. It
  is retired, and anything still true about the Rune artifact moves to the
  phero-ir crate docs (RN10).

**Unchanged until the issue that changes it:**

- AGENTS.md, the root `Cargo.toml` comment and `docs/VERSIONING.md` describe
  the current pin. They stay true until RN4 removes the pin and RN18 rewrites
  the authority documents. Editing them earlier would describe a state that
  does not exist yet.

## Alternatives considered

- **Keep the separate repository and integrate Rune first-class across it.**
  Rejected. Every change would still need two PRs and a rev bump, sigil's
  dead CI would stay the only gate on half the code, and sharing kernel
  crates across two repositories inverts the dependency direction.
- **Import, but keep `yestechgroup/sigil` alive as a published mirror.**
  Rejected for now. There is no outside embedder to serve, and a mirror is a
  second place changes can land. It can be revisited if an external consumer
  appears.
- **Keep the `sigil-*` crate names.** Rejected. It adds a second naming
  family to a workspace whose crates are `phero-*` (NAMING.md).
- **Rename every crate `phero-rune-*`, including a `phero-rune-syntax`.**
  Rejected for the grammar. A separate syntax crate is the per-surface split
  ADR 0005 D3 rejects. Kept for model and resolve (D2).
- **Port sigil's parser down to chumsky 0.10.** Rejected. It moves the newer
  code onto the older engine, and it rules out the chance (checked in RN2)
  that 0.13 lets `phero-parser` drop its `unsafe` carve-out.
- **Keep two chumsky majors permanently.** Rejected (RD7). It is a permanent
  tax, and it blocks the fold in D2.
- **Require the Java oracle on every PR.** Rejected (D4). It brings JDK and
  network dependence into the default gate.
- **Drop the oracle and freeze its output as goldens.** Rejected. The
  goldens stay (D4), but without the live oracle nothing would detect a
  normalisation or oracle-version drift.

## Notes

- **RN11, whole-pool resolution (2026-11-28).** The rule that Rune
  resolution is whole-pool, with no per-file cache, was re-examined against
  salsa and kept. The `sigil-lsp` `reanalysis` bench re-parses and
  re-resolves on every change in 1.36 ms (2 files) and 7.8 ms (12 files).
  Per-file memoisation would need pool-keyed cross-file invalidation, which
  the RN15 graph snapshot owns. The decision and numbers live in the
  `phero-rune-resolve` crate docs.
