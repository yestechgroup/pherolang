# 0011 — Documentation quality is a normative, hard-enforced standard

- **Status:** Accepted, 2026-11-21
- **Issue:** pherolang#373
- **Follows:** [0003](0003-compiler-backed-highlighting-is-the-source-of-truth.md)
  (the compiler as the source of truth for what a page may assert),
  [0005](0005-shared-surface-kernel-layering.md) (the surface table this
  standard's per-language architecture mirrors)
- **Does not revise** [0001](0001-rebrand-to-phero-lang.md),
  [0002](0002-static-hover-reference-panels.md), or
  [0004](0004-supported-platforms-linux-only.md).

## Context

Issue #373 proposes a documentation system for a language that is becoming a
modelling ecosystem: Diátaxis content types, a validated content model,
templates, an editorial standard, a separation of user and engineering
documentation, tested examples, and a maturity/conformance model.

The repository's documentation pipeline already works and is not the problem.
`docs/` is the only copy of the prose; `site/scripts/ingest-docs.mjs` projects
it into the Starlight site; `site/data/CONTRACT.md` makes the site a consumer of
the toolchain. What is lacking is the **content**: the current pages mix four
different readers' questions, lead with implementation detail, contain no
tutorials or overviews, cite issue numbers as headings, and carry examples that
nothing verifies. A pipeline that faithfully publishes inconsistent pages
produces inconsistency faster.

Three constraints shape the decision:

1. `docs/*.md` are load-bearing engineering contracts. `AGENTS.md` links their
   paths and anchors, the compiler's own rules are stated there, and moving
   them would break contributors' navigation. The user-facing layer must be
   additive, not a relocation.
2. The required CI workflow is Rust-only. The site workflow is deliberately not
   a required check, so a content gate that lives only in the site build can be
   skipped on exactly the pull requests that change documentation.
3. The repository's own conventions are already the right enforcement
   mechanism: one table and one pipeline, registries as generated data,
   generated artifacts checked by diff, and gates as steps in
   `scripts/ci-local.sh`.

## Decision

### D1 — Two documentation products, one architecture

Documentation splits by reader, not by file extension:

- **Guide** (`docs/guide/**`) — user-facing, Diátaxis-typed: overview,
  tutorial, how-to, concept, reference.
- **Engineering reference** (`docs/*.md`) — the compiler, artifacts, and
  development process. **Integration reference** is a visible page type for
  artifact contracts external tools consume; it is not hidden inside
  engineering lore.

Every language gets the same Guide tree. Sections are created only where there
is content; the architecture is a shape, not an obligation to write filler.

### D2 — The content model is machine-checked data

Every Guide page carries frontmatter: `title`, `description`, `type`,
`language`, `section`, `order`, `status`, `docStatus`, `audience`,
`prerequisites`, `related`, `lastReviewed`. Feature maturity (`status`) and
documentation maturity (`docStatus`) are separate fields.

The frontmatter is not advisory metadata. It is the input to the site's
navigation and the subject of the documentation gate. The required heading set
per type is part of the same contract: a template, a gate, and a page cannot
disagree.

### D3 — Enforcement has exactly two classes

- `[CI]` — a gate in `scripts/ci-local.sh`.
- `[REVIEW]` — a blocking row in `.github/PULL_REQUEST_TEMPLATE.md`.

There is no advisory class. A rule that cannot be checked yet is `[REVIEW]`
until its gate lands. This is the same discipline as the rest of the
repository: a claim is true because a gate says so, not because a document says
so.

### D4 — The content gate is compiler-backed and lives in `phero-cli`

The gate is `pherolang docs lint`, implemented in `phero-cli`, wired as a step
in `scripts/ci-local.sh`. It validates the content model, required headings,
terminology, implementation vocabulary, issue references, and — reusing the
CLI's compile entry points — every Phero fence in the Guide tree. Deliberately
invalid examples declare their expected diagnostic code in the fence info
string and must fail with exactly that code.

Rationale: the required workflow is Rust-only (constraint 2), and the
compiler-backed approach is the same one [ADR 0003](0003-compiler-backed-highlighting-is-the-source-of-truth.md)
established for highlighting. A Node linter in `site/` would be correct in
principle and skipped in practice.

### D5 — Exemplars are real pages

The standard is demonstrated by five published `.mox` Guide pages, not by
sample documents. They are the reference implementations: templates derive from
them, and a later gate asserts the templates have not drifted from them. `.mox`
is the entry language, so it gets the front door; `.ddd` remains the first
**rewrite** in the rollout because it is the worst current mixture.

### D6 — The standard is a governance set, not a wiki page

`docs/documentation/` holds the strategy, information architecture, content
model, editorial policy, style guide, glossary, and templates. `docs/README.md`
and the site land the reader in the Guide; the governance set is contributor
documentation, published under Contributing.

### D7 — The site remains a projection

No prose is authored in `site/`. The ingest step keeps authored frontmatter,
adds only what Starlight needs at build time (`editUrl`, `sidebar.order`), and
records the manifest the sidebar is built from. `site/data/CONTRACT.md` is
unchanged in principle; the Guide tree is one more input.

## Consequences

- The Guide tree is gated from its first day. The five `.mox` exemplars are
  written to pass the Phase 2 gates before those gates exist, and the gate PR
  must not need to relax them.
- `docs/*.md` are untouched in this phase. The routing table in
  `INFORMATION_ARCHITECTURE.md` classifies every current document and records
  which ones become Guide pages in the rollout.
- The `.ddd` pilot must preserve `AGENTS.md` anchors: user pages are new prose
  that links to `docs/DDD.md`, not a physical split of it. An anchor-migration
  table lands with the pilot if any section genuinely moves.
- A documentation change that breaks an example fails CI exactly as a code
  change that breaks a test does (E11).
- Terminology is a data file. Adding a synonym ban is a reviewable YAML diff,
  and a new keyword is a vocabulary decision rather than a style argument.
- The concept/problem route needs no new mechanism: `related`/`prerequisites`
  edges plus glossary anchors are the graph. A "modelling a lending
  application" page is an ordinary concept page when there is enough content to
  justify it.
- A rule added to the editorial policy lands with its enforcement. An
  unenforced rule is a defect of the pull request that added it.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Author user documentation directly in `site/phero-docs/src/content/` | Breaks `site/data/CONTRACT.md` (the site is a projection), splits the source of truth, and makes the prose invisible to `AGENTS.md`-driven contributor work. |
| Physically split `docs/DDD.md` into user and engineering files in the first phase | Breaks every `AGENTS.md` anchor and the compiler's own cross-references before the replacement pages exist. The Guide links to the contract from day one; the split happens, if ever, with a migration table. |
| A Node documentation linter under `site/` | Correct in principle, skipped in practice: the gate would live in a workflow that is explicitly not required. |
| Advisory lints with later hardening | "Everything hard" was chosen deliberately: an advisory rule is a suggestion, and suggestions were the prior state. `[REVIEW]` exists for rules a machine cannot check, not as a softer tier. |
| Organize the site around file extensions only | A reader modelling a lending application does not know that `.dmn` exists. The Guide tree is by language, the concept route is by problem, and both are derived from the same metadata. |
| A separate documentation repository | The prose is part of language development; separating it guarantees drift and makes E14/E15 unenforceable. |
