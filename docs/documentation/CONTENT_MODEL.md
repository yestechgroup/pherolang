# Content model

The content model is the frontmatter contract every page under `docs/guide/`
must satisfy, and the section structure each page type must follow. It exists so
that a page's kind, maturity, ordering, and audience are data a tool can check —
not conventions an author has to remember.

Rules that read "MUST" are normative. Every rule is enforced either by the
documentation gate in CI (`[CI]`, see `EDITORIAL_POLICY.md`) or by the blocking
pull-request checklist (`[REVIEW]`). Nothing in this document is advisory.

## 1. Frontmatter contract

Every page under `docs/guide/**` starts with a frontmatter block:

```yaml
---
title: Classes and features
description: Declare a class, its attributes, relationships, operations, and derived features.
type: reference
language: mox
section: language
order: 30
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/mox/overview.md]
related: [docs/guide/mox/concepts/modelling-not-coding.md]
lastReviewed: 2026-11-21
---
```

### Fields

| Field | Type | Required | Meaning |
|---|---|---|---|
| `title` | string | yes | Page title. MUST equal the page's `#` heading, character for character. |
| `description` | string | yes | One sentence, ≤ 160 characters, describing what the page answers. Used for search and link previews. |
| `type` | enum | yes | `overview`, `tutorial`, `how-to`, `concept`, `reference`, or `integration`. |
| `language` | enum | yes | `mox`, `ddd`, `ifml`, `dmn`, `cmmn`, `evt`, `deploy`, `uc`, `intent`, or `platform` (tooling that belongs to no single language — installation, editors). |
| `section` | string | yes | Kebab-case navigation grouping within the language (for example `language`, `testing`, `migration`). Not free-form prose. |
| `order` | integer | yes | Reading order within `(language, type)`. Lower sorts first. Gaps of ten leave room to insert. |
| `status` | enum | yes | The maturity of the documented **language feature**: `experimental`, `draft`, `stable`, or `deprecated`. |
| `docStatus` | enum | yes | The maturity of **this page**: `stub`, `draft`, or `complete`. |
| `audience` | string[] | yes | Who the page is for, in plain nouns: `[application developer]`, `[platform engineer]`, `[tool author]`. |
| `prerequisites` | path[] | no | Repository-root-relative paths to pages a reader should have read first. |
| `related` | path[] | no | Repository-root-relative paths to pages that continue the same task or concept. |
| `lastReviewed` | date | yes | ISO `YYYY-MM-DD`. Updated whenever the page is materially edited or its subject's semantics change. |

`status` and `docStatus` are deliberately separate. A stable language feature
can have a stub page, and a fully written page can describe an experimental
feature. Conflating the two hides both facts.

Values are YAML. A value containing `: `, a leading `-`, `#`, or a quote must be
quoted (`description: "Build it: two ways."`); the gate rejects frontmatter that
does not parse rather than guessing what the author meant.

### Rules

- **CM1 — Closed vocabulary.** `[CI]` Every enum field admits exactly the values
  listed above. An unknown value is a hard error naming the field and the page.
- **CM2 — Required fields.** `[CI]` A missing required field is a hard error. A
  field outside this table is also an error: authors extend the model by
  updating this document, not by inventing keys.
- **CM3 — Title agreement.** `[CI]` The frontmatter `title` and the body's `#`
  heading MUST match. The ingest step publishes the frontmatter title and
  strips the heading; a mismatch would publish a page whose title and body
  disagree.
- **CM4 — Status honesty.** `[REVIEW]` A page describing behavior that is not
  implemented MUST NOT say `stable`. "Proposed" work is documented in the
  proposal's own artifact (an ADR or RFC), not as if it existed.
- **CM5 — Review date.** `[REVIEW]` `lastReviewed` changes when the page's
  substance changes. Reformatting alone does not require a new date.
- **CM6 — Prerequisites and related resolve.** `[CI]` Every `prerequisites` and
  `related` entry MUST name an existing page source path.
- **CM7 — One overview per language.** `[CI]` A language that has any page MUST
  have its `type: overview` page before the first publish. Other page types
  link to it, and the site navigation depends on it.
- **CM8 — Frontmatter is not prose.** `[REVIEW]` No normative rule, disclaimer, or
  paragraph of explanation lives in frontmatter. Metadata describes the page;
  the page does the teaching.

## 2. Page structure

Each page type has a mandatory `##` heading set. Headings MUST appear as `##`
headings (not `#` or `###`), in the order listed, with no extra `##` heading
before, between, or after them unless the type says otherwise.

A section that does not apply is written with a one-line `None.` and keeps its
heading. Removing a heading changes the page's contract; adding one requires
changing this document in the same pull request.

### `overview`

| Heading | Content |
|---|---|
| `## What is it?` | The language in plain English, in one or two paragraphs. |
| `## What problems does it solve?` | The problem domain, stated from the user's side. |
| `## Key capabilities` | What a reader can model, as a short list. |
| `## A simple example` | One small, compiling example. |
| `## How it fits into Phero Lang` | Relationships to the other languages, in the ecosystem's terms. |
| `## Getting started` | Links to the language's tutorials. |
| `## Further reading` | Links to concepts and reference. |

### `tutorial`

| Heading | Content |
|---|---|
| `## What you'll learn` | Three to six concrete outcomes. |
| `## Prerequisites` | Tooling and prior reading. |
| `## What we're building` | The artifact the reader will hold at the end. |
| `## Step <n> — <title>` | One heading per step, numbered from 1. At least one is required. Each step ends with a checkable result. |
| `## Complete example` | The whole source, in one fence. |
| `## Expected result` | Exactly what the reader should see when it works. |
| `## Next steps` | Where to go after this tutorial. |

### `how-to`

| Heading | Content |
|---|---|
| `## Problem` | The single task the page solves, in one paragraph. |
| `## Prerequisites` | What the reader needs first. |
| `## Solution` | The steps, in order. |
| `## Example` | A compiling example showing the result. |
| `## Related reference` | Links to the authoritative reference pages. |

### `concept`

| Heading | Content |
|---|---|
| `## Overview` | The concept defined in one paragraph. |
| `## Why does it matter?` | The problem it solves and the cost of ignoring it. |
| `## How it works` | The mechanism, at the reader's level. |
| `## Example` | A compiling example demonstrating this concept and no other. |
| `## Relationship to other concepts` | Dependencies and interactions, named and linked. |
| `## Further reading` | Tutorials and reference that continue the thread. |

### `reference`

| Heading | Content |
|---|---|
| `## Syntax` | The exact grammar form, as a fence or a table. |
| `## Semantics` | What the construct means, precisely. |
| `## Constraints` | Validation rules and restrictions, each with its consequence. |
| `## Examples` | Compiling examples: at least one valid; invalid ones labeled with their expected diagnostic. |
| `## Diagnostics` | The diagnostic codes this construct can raise, or `None.` |
| `## Compatibility` | Wire/version/compatibility facts a consumer must know, or `None.` |

## 3. Rules for content inside a page

- **CM9 — One reader per page.** `[REVIEW]` A page answers one question for one
  audience. If a draft serves two Diátaxis types, split it; the type field has
  no "and".
- **CM10 — Fences are classified.** `[CI]` Every Phero fence in a page under
  `docs/guide/**` carries a surface and zero or more of these comma-separated
  attributes:

  | Form | Meaning |
  |---|---|
  | `<surface>` | A complete example. It compiles as a standalone unit. |
  | `<surface>,fragment` | A syntax illustration, not an example. |
  | `<surface>,invalid=<CODE>` | A deliberately invalid example; it must fail with exactly `<CODE>`. |
  | `<surface>,fixture=<PATH>` | The repository-root-relative source the example is compiled against: a `.mox` file for `.ddd`, a directory for `.ifml`. |

  Attributes combine (`mox,invalid=E0300`; `ddd,fixture=examples/support/support.mox`).
  `fragment` cannot combine with `invalid` or `fixture`. An unknown attribute is
  an error, and so is a complete fence that does not compile.

  A `.ddd` example resolves its imports against the design's directory, so it
  names its domain with `fixture=` and imports it by file name:

  ````text
  ```ddd,fixture=examples/support/support.mox
  import "support.mox";
  ```
  ````

  An `.ifml` example names its fixture directory (for example
  `examples/support`); its imports resolve against that directory, and the gate
  compiles the imported `.mox` domain and any `.actor` policies before running
  the interaction, authorization, and binding passes.

  A `.mox` example never carries `fixture` — it is self-contained.
- **CM11 — Examples are complete and self-contained.** `[CI]` A `mox` fence
  MUST compile on its own and MUST NOT carry `import` lines. Full applications
  belong in `examples/`, which CI compiles; link to them instead.
- **CM11a — Fragments illustrate syntax, never behavior.** `[CI]` A
  `mox,fragment` fence is allowed only in a `reference` page's Syntax section or
  a `how-to` page's Solution steps. A page MUST also carry at least one complete
  `mox` example. A fragment never replaces the proof that the construct works.
- **CM12 — No implementation vocabulary.** `[CI]` Guide pages (`overview`,
  `tutorial`, `how-to`, `concept`) MUST NOT name crate names, IR/artifact type
  names, parser or compiler internals, or backend mechanics. Detail is linked
  from Engineering. `reference` pages may name artifact facts in
  `## Compatibility`; `integration` pages may document wire contracts in full.
- **CM13 — No development history.** `[CI]` Guide pages MUST NOT contain issue
  numbers (`#123`, `pherolang#123`) or narrate a decision's history. Link to an
  ADR from a concept page when the rationale matters; never use the issue
  tracker as an information architecture.
- **CM14 — Terminology.** `[CI]` Canonical terms only, per `glossary.yaml`.
  Every abbreviation is defined in the glossary's `acronyms` list.
- **CM15 — Standards claims are labeled.** `[REVIEW]` When a page describes a
  construct derived from an external standard, it states one of: *industry
  standard*, *Phero implementation*, *Phero extension*, *Phero divergence*, or
  *proposed feature*. The four are never blurred into "supports IFML".
- **CM16 — Templates track their exemplars.** `[CI]` Each file in
  `docs/documentation/templates/` has the same heading sequence as its exemplar
  page (tutorial step headings collapse to one `Step` entry). Editing a
  template without the exemplar, or the exemplar without the template, is a
  finding naming both.

## 4. How the site consumes the model

The docs site is a projection of this tree; it does not own prose. The ingest
step (`site/scripts/ingest-docs.mjs`) copies each `docs/guide/**` page to
`site/phero-docs/src/content/docs/guide/<relative path>`, keeps the authored
frontmatter, and adds only the fields Starlight needs at build time
(`editUrl`, `sidebar.order`, and the table-of-contents depth).
`site/data/docs-manifest.json` records each page's source, URL, title, type,
language, section, and order — that manifest is the navigation's input, so a
page's position is data, not a hand-maintained list.

## 5. Changing the model

The model is code-adjacent: the gate in CI implements these rules. A change to
this document and the gate MUST land in the same pull request. If a rule cannot
be checked yet, it stays `[REVIEW]` and lives in the pull-request checklist
until its gate exists — it does not silently become advice.
