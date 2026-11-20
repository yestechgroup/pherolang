# ADR 0010: Intent specifications are a declaration surface

* Issue: [#196](https://github.com/yestechgroup/pherolang/issues/196) (v0.5 — Evidence; Proof-Carrying Models epic #202, Phase P1)
* Status: accepted
* Related: [ADR 0005](0005-shared-surface-kernel-layering.md) (the shared surface kernel), #137 (the receipts follow-up)

## Context

#196 makes intent a first-class artifact: requirement statements with
stable ids (`LOAN-001`), traceability links to model elements, and a
coverage check — every intent maps to ≥ 1 model element and ≥ 1 check, with
unmapped intents warning. Two design decisions had to be made explicitly.

### Decision 1 — the intent→element link: declaration form, not doc-comment anchors

The issue left the mechanism open ("doc-comment anchors or a dedicated
declaration form — design spike decides"). The evidence is one-sided:

| Criterion | Doc-comment anchors | Dedicated declaration form (chosen) |
|---|---|---|
| `.evt`/`.ifml` linkable targets (required) | **Fails**: comment retention exists on only five surfaces (`.mox`, `.ddd`, `.uc`, `.dmn`, `.cmmn`); `.evt`, `.deploy`, `.actor`, and all of `phero-ifml` drop comments at `lex()`. Covering `.ifml`/`.evt` would need lexer changes in a second crate plus an anchor mini-language inside comment text. | Links are names resolved at application level — every surface is linkable uniformly. |
| Stable ids | Parsed out of comment text — fragile, formatter-hostile. | First-class declarations, checked (`E0450`–`E0451`). |
| IR blast radius | Comments threaded into ASTs; new fields on existing artifacts. | An isolated new artifact; the existing artifacts stay byte-identical. |

### Decision 2 — a new `.intent` surface, not `intent` declarations inside `.mox`

Extending `.mox` is the smaller diff but the bigger blast radius: new
grammar and formatter layout rules on the most load-bearing surface (its
fmt round-trip is byte-pinned), `intent` fields added to the Core IR `Model`
wire format, and intents trapped inside domain files — a project's intents
are application-level claims, and the `.ddd`/`.ifml`/`.evt`/`.actor` links
would read as foreign bodies in a domain model. The one-surface-table
discipline makes a file kind a *table row*, not an appended sentence; the
`.uc` surface is the precedent for a lightweight, project-local, standalone
artifact with no in-tree backend.

## Decision

Intent specifications are the tenth source surface: `.intent`, one row in
`phero_parser::Surface`, parsed/formatted in `phero-syntax`, compiled by
`phero-driver` to the versioned `phero_ir::intents::IntentModel` artifact,
and checked by a project-level **coverage pass** hosted in the union pass's
phase 2 (the `check_integrity` channel; the `driver.integrity` band's
`X0009`–`X0014`). Single-surface intent errors are the `driver.intent` band
(`E0450`–`E0454`).

Consequences, deliberately:

- **Spans never serialize.** The wire artifact stays span-free; the compile
  returns a span table beside it, which the coverage pass renders spanned
  diagnostics from — the `.ifml` auth-sites discipline.
- **Intents contribute no graph nodes or edges in v0.5**, and carry no
  navigation index: the traceability edges are the coverage pass's, and
  #137's intent → element → verification receipt table is the follow-up.
  The pipeline still compiles every `.intent` file (one compile per
  surface), and the editor publishes its per-file diagnostics.
- **Link names are carried as authored.** The artifact never resolves a
  link; resolution is the coverage pass's, over the application's compiled
  artifacts.
- **The check vocabulary is model-native**: a designed class's invariants
  and a `.dmn` decision. It is an extension point — new check kinds are
  vocabulary rows plus resolution sources, not new syntax.
