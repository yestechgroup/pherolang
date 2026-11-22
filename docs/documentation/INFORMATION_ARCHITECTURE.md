# Information architecture

Phero Lang documentation has one architecture across every language. Readers
arrive with one of two questions, and the site answers both:

1. **Browse by language** — "I know I need `.ddd`; teach me it."
2. **Browse by concept or problem** — "I am modelling lending; which languages do
   I need, and in what order?"

The first is a tree; the second is a set of routes through it. Both are derived
from page metadata (`CONTENT_MODEL.md`), never from a hand-maintained list of
links.

## 1. The two documentation products

| Product | Question it answers | Lives in | Site group |
|---|---|---|---|
| **Guide** | "Teach me", "how do I", "what does this mean to me" | `docs/guide/**` | Guide |
| **Engineering reference** | "Exactly what does the compiler do", "how is this built" | `docs/*.md` | Reference / Engineering |
| **Integration reference** | "What does a tool that consumes this artifact need to know" | `docs/*.md`, page type `integration` | Reference |
| **Migration** | "How do I move an existing consumer" | `docs/REBRAND.md`, release runbooks | Migration |
| **Decisions** | "Why is it this way" | `docs/adr/**` | Decisions |

The distinction is about the reader, not the file extension:

| Question | Where it is answered |
|---|---|
| How do I declare an aggregate? | Guide — `.ddd` reference |
| What does an aggregate mean? | Guide — `.ddd` concept |
| How does aggregate derivation work semantically? | Guide reference + `docs/DDD.md` |
| How does the compiler derive it? | Engineering — `docs/DDD.md` |
| Why was that lowering strategy chosen? | ADR |
| How do I consume a `.ddd` artifact? | Integration reference |
| What is the exact JSON shape? | Integration reference |

Implementation detail is never removed from the repository. It is routed to the
page that owns it and linked from the page that needs it.

## 2. Guide tree — one shape for every language

```
docs/guide/
├── mox/
│   ├── overview.md
│   ├── tutorials/
│   ├── how-to/
│   ├── concepts/
│   └── reference/
├── ddd/            (same five sections, as content exists)
├── ifml/
├── dmn/
├── cmmn/
├── evt/
├── deploy/
├── uc/
├── intent/
└── platform/       (installation, editors — no language of their own)
```

Rules:

- **IA1** — Every language that has any Guide page has `overview.md` first.
- **IA2** — A section is created when there is something meaningful to document.
  An empty `tutorials/` directory is not an information architecture; it is a
  promise nobody kept.
- **IA3** — A page's type is its Diátaxis section: `tutorials/` holds
  `type: tutorial`, `how-to/` holds `type: how-to`, and so on. `overview.md` is
  `type: overview`. The directory and the type cannot disagree.
- **IA4** — Reference pages answer what the language does; concept pages answer
  why and how it fits; they do not trade places.
- **IA5** — A language page never explains another language's concept in full.
  It links to that language's page and applies it here.

## 3. Routing table

Every document that exists today has exactly one home. The table is normative:
a new document is added here in the same pull request that adds the file.

A document may appear twice below. That is deliberate: a Guide page is new
prose written for the reader, while the current contract page remains the
Engineering authority and is linked — not moved, and not split before its
replacement exists.

### Guide (user-facing; Diátaxis)

| Current document | Becomes |
|---|---|
| `docs/INSTALL.md` | `guide/platform/tutorials/install.md` (tutorial) and a short how-to for upgrades |
| `docs/EDITORS.md` | `guide/platform/how-to/configure-an-editor.md` |
| `docs/LANGUAGE.md` | `guide/mox/**` — overview, tutorials, how-tos, concepts, reference; the grammar authority stays in the parser module docs |
| `docs/EXPRESSIONS.md` | `guide/mox/reference/expressions.md` (user-facing rules) + engineering page for the type checker |
| `docs/DIAGNOSTICS.md` | `guide/platform/reference/diagnostics.md` (every code explained) |
| `docs/DDD.md` | `guide/ddd/**` (landed: overview, tutorial, three how-tos, two concepts, four reference pages) + Engineering contract |
| `docs/IFML.md` | `guide/ifml/**` (landed: overview, tutorial, three how-tos, two concepts — one is the OMG conformance page — three reference pages) + Engineering contract |
| `docs/DMN.md` | `guide/dmn/**` (landed: overview, tutorial, two how-tos, two concepts — one is the OMG conformance page — two reference pages) + Engineering contract |
| `docs/EVENTS.md` | `guide/evt/**` (landed: overview, concept, reference, how-to) + Engineering contract |
| `docs/CMMN.md` | `guide/cmmn/**` (landed: overview, two concepts — one is the OMG conformance page — and reference) + Engineering contract |
| `docs/DEPLOY.md` | `guide/deploy/**` (landed: overview, concept, reference, how-to) + Engineering contract |
| `docs/USECASES.md` | `guide/uc/**` (landed: overview, concept, reference) + Engineering contract |
| `docs/INTENTS.md` | `guide/intent/**` (landed: overview, concept, reference) + Engineering contract |
| `docs/VERSIONING.md` | `guide/platform/reference/compatibility.md` + Engineering policy |

### Engineering reference (compiler, artifacts, development)

| Document | Home |
|---|---|
| `docs/DDD.md`, `docs/IFML.md`, `docs/DMN.md`, `docs/CMMN.md`, `docs/EVENTS.md`, `docs/DEPLOY.md`, `docs/USECASES.md`, `docs/INTENTS.md`, `docs/LANGUAGE.md` | Engineering — the normative artifact contracts (also integration reference where consumers depend on them) |
| `docs/BACKENDS.md` | Engineering / integration — the artifact consumption contract |
| `docs/GRAPH.md`, `docs/SEMANTICS.md`, `docs/HIGHLIGHTING.md`, `docs/VERIFICATION.md`, `docs/RECEIPTS.md`, `docs/GOLDENS.md`, `docs/RUNE-BODIES.md` | Engineering |
| `docs/NAMING.md` | Engineering — the spelling contract every page links to |
| `docs/verification/**` | Engineering (generated) |

### Migration and decisions

| Document | Home |
|---|---|
| `docs/REBRAND.md`, `docs/RELEASE-*.md` | Migration |
| `docs/adr/**` | Decisions (numbered, immutable) |
| `docs/documentation/**` | This standard — contributor documentation; published under Contributing |

`docs/README.md` stays the index and the site landing page derives from it.

## 4. Navigation

Navigation is generated from frontmatter, in one pass:

- **Guide** — grouped by `language`, ordered by `order` within each type. The
  language label comes from the language's `overview` title.
- **Reference** — the existing pages, in the reading order the index records.
- **Engineering**, **Migration**, **Decisions**, **Contributing** — as today.

`site/data/docs-manifest.json` is the interface: the ingest step writes it, and
the site's sidebar is built from it. A page's position is therefore a property
of the page, not of a configuration file that drifts.

## 5. The concept/problem route (later, but designed now)

A reader modelling a lending application needs `.mox`, `.ddd`, `.dmn`, `.cmmn`,
`.evt`, `.ifml`, and `.deploy` — in a specific order, for a specific reason.
That route is data:

- Each page's `related` and `prerequisites` fields already form a graph.
- Each glossary term (`glossary.yaml`) is a concept anchor.
- A "modelling a lending application" page is a `type: concept` page in
  `guide/platform/concepts/` whose parts are ordinary links to the per-language
  pages.

No new mechanism is required. When there are enough pages for the route to be
useful, the concept page is written and its links become the navigation. The
architecture does not need a second tree; it needs the pages.

## 6. Adding a language

A new language is a checklist, not a design exercise:

1. `guide/<language>/overview.md` with `type: overview`, `language`,
   `section`, `order`.
2. Its tutorial (one is enough to start), its how-to set, its concepts, and its
   reference — written when there is something meaningful to say.
3. The language's row in `languages.yaml` when the maturity model lands (the
   implementation maturity and the standards alignment are separate axes).
4. The Engineering row that owns its artifact contract.
5. The routing-table row above.
