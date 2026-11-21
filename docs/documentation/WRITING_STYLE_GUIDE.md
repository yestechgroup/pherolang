# Writing style guide

This guide is the working reference for how a Phero Lang page sounds. It is
human-reviewed (`[REVIEW]` in `EDITORIAL_POLICY.md`), except where a rule names
a mechanical check. Sentences below marked **Bad** are real text from this
repository's earlier documentation; they are kept as examples of what to fix,
not as style to imitate.

## 1. Lead with what the reader can do

The first paragraph answers "what is this and why would I use it?" — not "how is
it built?". The reader has a problem; the page is about that problem.

**Bad** — the current opening of the `.ddd` reference:

> A Sculptor-style domain-driven-design layer over `.mox` domain models. The
> domain model defines the data (entities, features, constraints); the `.ddd`
> design defines the application architecture on top of it... Parsed by
> **phero-syntax** (`parse_ddd`, the chumsky parser) at compile time, lowered
> and validated by **phero-driver**...

**Better:**

> A `.ddd` design describes the application built on top of a domain model: its
> modules, services, repositories, and the lifecycle rules of its entities.
> Write it when the data model alone cannot answer how the system behaves.

The second sentence names the reader's decision. The compiler's internals do not
belong in the same paragraph as the reason to use the language.

**Bad** — the current opening of the `.ifml` reference:

> A Domain-Specific Language that is semantically aligned with OMG IFML 1.0 but
> uses a modern, C-like expression-heavy syntax instead of XML/XMI. Lexed by a
> **logos** lexer and parsed by **chumsky** into a spanned syntax tree...

**Better:**

> `.ifml` describes how people move through an application: the screens they
> see, the actions they take, and the data those screens show. It is aligned
> with OMG IFML 1.0 and written as text rather than XML/XMI.

Note the second version keeps the standards claim — and labels it, as
`EDITORIAL_POLICY.md` E9 requires — while dropping the lexer inventory.

## 2. Plain English

- One idea per sentence. If a sentence needs a semicolon to survive, split it.
- Prefer the concrete noun: "the file", "the model", "the compiler", not "the
  artifact surface" or "the semantics layer".
- Define a term the first time it appears; link to its authoritative page.
- No marketing adjectives in reference pages. "Powerful", "seamless", and
  "robust" are not facts.
- No unexplained abbreviations. If an abbreviation is not in
  `glossary.yaml#acronyms`, write the words.
- Keep historical context out. How the language arrived at a design is an ADR's
  subject, not a reference page's.

**Bad:**

> The language facilitates the declarative specification of semantically aligned
> interaction models.

**Better:**

> `.ifml` lets you describe how users interact with an application: its views,
> navigation, events, and data bindings.

**Bad:**

> An aggregate MUST be defined around a root entity in all cases where the
> design declares containment.

**Better:**

> Every aggregate has one root entity. A class becomes an aggregate root when
> the design says so with `root`, or when it owns other classes with
> `contains`; `docs/DDD.md` defines the derivation precisely.

The second version says the rule and links to the authority. If the first
version's MUST is real, restate it as a rule; if it is not absolute, do not use
MUST at all.

## 3. Structure

- A page carries one Diátaxis type and follows that type's headings
  (`CONTENT_MODEL.md` §2). Do not paste a tutorial into a reference page to
  avoid creating a second page.
- The title names the thing, not the task, except in a how-to or tutorial where
  the task is the thing: "Classes and features" (reference), "Your first domain
  model" (tutorial), "Add an enum" (how-to).
- Headings are questions or noun phrases, not labels: "Why does it matter?"
  rather than "Motivation".
- A concept page links to the reference page that states the rules; it does not
  restate them.

## 4. Code and examples

- Tag every fence with its surface: `mox`, `ddd`, `ifml`, `dmn`, `cmmn`, `evt`,
  `deploy`, `uc`, `intent`, `json`, `sh`.
- A Guide example is complete enough to compile on its own, and small enough to
  read in one screen. No `import` lines (`CONTENT_MODEL.md` CM11).
- A syntax illustration that is not a working file is labeled
  `<surface>,fragment` and appears only in a reference Syntax section or a
  how-to Solution step. A page that shows fragments still carries a complete
  example.
- A `.ddd` example names the domain it is compiled against:
  `ddd,fixture=examples/support/support.mox`, with `import "support.mox";` in
  the fence. A `.mox` example never needs a fixture.
- Show the smallest example that demonstrates the point. A reference page's
  example may show every option; a tutorial's example shows one step.
- Invalid examples are labeled: `mox,invalid=E0300`, and the prose says what the
  reader should learn from the failure.
- Prefer realistic names (`Ticket`, `RefundRequest`) over `Foo` and `Bar`.
- When a full application demonstrates the point better than a snippet, link to
  the example under `examples/` and quote only the relevant lines.

## 5. Terminology

`glossary.yaml` is the terminology authority. It lists:

- `terms`: the canonical spelling and a one-line definition. When a page uses a
  term in a new sense, the glossary changes in the same pull request.
- `aliases` per term: spellings that must not appear. "Phero" alone, "mox
  model", and "DDD design DSL" are aliases of canonical terms, not synonyms.
- `acronyms`: every abbreviation a Guide page may use, with its expansion and
  its allowed scope.

The product name is **Phero Lang** — two words, never bare "Phero". The
normative spelling contract for the binary, crates, environment variables, and
wire keys is `docs/NAMING.md`; the glossary links to it and does not restate it.

## 6. Normative language

Use RFC 2119 keywords only where the statement is genuinely normative about the
language or its artifacts:

| Word | Meaning |
|---|---|
| MUST | An absolute requirement. |
| MUST NOT | An absolute prohibition. |
| SHOULD | Recommended; a deviation needs a stated justification. |
| SHOULD NOT | Generally discouraged. |
| MAY | Optional. |

If a sentence would still be true with "usually" or "normally", it is not a
MUST. Every MUST in a reference page should be traceable to a validation rule
or a semantic guarantee; if it is not, it is prose.

## 7. Links

- Link to the authoritative page instead of repeating its content.
- Use repository-relative links in page bodies (`../reference/classes.md`). They
  work on GitHub and are rewritten for the published site.
- Do not link to GitHub issues from Guide pages (E16). Link to ADRs from concept
  pages when the rationale is the point; link to the reference page when the
  rule is the point.
- A link's text says what the reader will find: "the grammar for `class`", not
  "here".

## 8. Before you publish

Run this list against the page:

1. Does the opening paragraph say what this is and why the reader cares?
2. Does the page have exactly one `type`, and do its headings match that type?
3. Is every term either defined here or linked to its authoritative page?
4. Would a reader who knows the language but not the implementation understand
   every sentence?
5. Does every example compile (or declare its expected diagnostic)?
6. Is every standards claim labeled?
7. Is there any sentence that only makes sense to a compiler contributor? Move
   it to Engineering or link to it.
8. Does the page repeat another page's explanation? Replace the repetition with
   a link.
