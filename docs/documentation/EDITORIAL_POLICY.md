# Editorial policy

This policy governs every page published from this repository. It is normative:
"must" and "MUST" mean the same thing here, and a violation is a blocked pull
request, not a preference.

The policy has three enforcement classes:

- **`[CI]`** — a gate in `scripts/ci-local.sh` fails on violation.
- **`[REVIEW]`** — a blocking row in `.github/PULL_REQUEST_TEMPLATE.md`; the
  reviewer must be able to point at the page and say the rule holds.
- **`[CI] + [REVIEW]`** — mechanically checked for the easy cases, reviewed for
  the substance.

There is no advisory class. A rule either blocks or does not exist; a rule that
cannot be checked yet is `[REVIEW]` until its gate lands, and the gate lands
before the rule is relied upon.

## 1. What documentation is for

- **E1 — Documentation serves its reader.** `[REVIEW]` Every page states who it
  is for, what question it answers, and what the reader can do afterwards
  (frontmatter `audience`, `description`; body opening). A page that cannot
  answer all three is not ready to publish.
  *The page-purpose test:* if a page tries to answer two unrelated questions,
  split it. If it answers none, delete it.
- **E2 — One authoritative page per concept.** `[REVIEW]` A technical concept is
  explained once. Every other page links to that explanation instead of
  repeating it. Explanations that disagree are worse than no explanation.
- **E3 — User documentation and engineering documentation are separate
  products.** `[CI] + [REVIEW]` Pages under `docs/guide/` describe the language
  as a user meets it. Compiler internals, IR shapes, lowering strategy, and
  repository layout belong to the Engineering pages. Artifact and wire contracts
  that external consumers depend on belong to an `integration` page — visible,
  labeled, and not hidden inside engineering lore.
- **E4 — Documentation describes what exists.** `[REVIEW]` Behavior that is not
  implemented is not described as implemented. `status` distinguishes
  experimental, draft, stable, and deprecated features; `docStatus` distinguishes
  stub, draft, and complete pages. Neither field may be optimistic.

## 2. Voice

- **E5 — Write for the reader, not the implementer.** `[CI] + [REVIEW]` Guide
  pages lead with user-visible behavior. Implementation vocabulary (crate names,
  type names, parser/compiler internals, backend mechanics) is a hard gate for
  those pages and a review question for reference pages.
  Bad: "Parsed by the chumsky parser and lowered by the driver into a versioned
  artifact."
  Better: "Phero reads the file, checks it, and produces a model your tools can
  consume."
  See `WRITING_STYLE_GUIDE.md` for the working vocabulary list and worked
  rewrites.
- **E6 — Plain English is not a lower standard.** `[REVIEW]` Short sentences,
  concrete nouns, no marketing adjectives in reference, no unexplained
  abbreviations. Precision comes from saying exactly what happens, not from
  sounding formal.
- **E7 — Introduce a term before using it.** `[REVIEW]` A page never assumes the
  reader knows a term because the implementation team does. First use is a
  definition or a link to the authoritative page.
- **E8 — Normative words are reserved.** `[CI] + [REVIEW]` `MUST`, `MUST NOT`,
  `SHOULD`, `SHOULD NOT`, and `MAY` appear only in genuinely normative
  statements, in the sense of RFC 2119. A language rule that is absolute says
  MUST; a convention with justified exceptions says SHOULD; anything else is
  ordinary prose. Using MUST for emphasis is a defect.

## 3. Standards and Phero

- **E9 — Label the provenance of every standards-adjacent claim.** `[REVIEW]`
  When a construct derives from an external specification (OMG IFML, DMN, CMMN,
  Sculptor, Xcore), the page states which relationship holds:

  | Label | Meaning |
  |---|---|
  | Industry standard | Defined by an external specification; Phero does not own it. |
  | Phero implementation | How Phero realizes the standard's concept. |
  | Phero extension | Functionality Phero adds beyond the standard. |
  | Phero divergence | A deliberate difference from the standard. |
  | Proposed feature | Not implemented; presented as a proposal, never as fact. |

  "Supports IFML" without a relationship label is a claim the reader cannot
  verify. Name the standard, the version, and the relationship.
- **E10 — Never imply conformance by resemblance.** `[REVIEW]` Using a
  standard's terminology does not make an implementation conformant. A
  conformance statement is a page of its own, with supported, partial,
  unsupported, extended, and divergent concepts listed.

## 4. Examples

- **E11 — Examples are executable, or labeled.** `[CI]` Every Phero fence in a
  Guide page is classified with attributes: a complete example that compiles,
  `fragment` (a syntax illustration, only in a reference Syntax section or a
  how-to Solution step), or `invalid=<CODE>` (fails with exactly that code);
  `.ddd` examples name their domain and `.ifml` examples their fixture directory with `fixture=<PATH>`. An
  unclassified fence is a defect.
- **E12 — Examples are self-contained.** `[CI]` A Guide example teaches one
  thing. It carries no imports and no unrelated declarations; complete
  applications are linked from `examples/`, which CI compiles as a whole. A
  page that uses fragments still carries at least one complete example.
- **E13 — Examples are not copied from fiction.** `[REVIEW]` An example must be
  a real compilable model with realistic names, not pseudo-code. If a snippet
  cannot compile, it is not an example.

## 5. Maintenance

- **E14 — A feature is not complete until it is documented.** `[REVIEW]` A
  language change that adds or alters syntax, semantics, validation, or an
  artifact updates the affected pages, or records an explicit waiver in the pull
  request with the follow-up issue that will close it.
- **E15 — Semantics changes trigger documentation review.** `[REVIEW]` The same
  pull request that changes behavior updates the page that describes it and the
  page's `lastReviewed`.
- **E16 — No development history in language documentation.** `[CI]` Issue
  numbers and development chronology do not appear in Guide pages. Design
  rationale lives in an ADR; link to it from a concept page. A reference page
  describes the language as it is.
- **E17 — No silent drift.** `[CI] + [REVIEW]` A page whose subject changed but
  whose text did not is a defect of the pull request that changed the subject.
  The manifest, frontmatter, and gates make the stale page visible; the review
  makes it correct.
- **E18 — Deletion is a documentation change.** `[REVIEW]` Removing a feature
  removes its pages or marks them `deprecated` with a migration link. A page for
  a feature that no longer exists is worse than no page.

## 6. Enforcement map

The gate is `pherolang docs lint` (CI step 3/8, `scripts/ci-local.sh
docs-lint`). Its checks, by the rule ids they implement:

| Gate | Rules |
|---|---|
| content model | `CM1`–`CM8`, `IA1`–`IA3` (frontmatter, enums, title agreement, prerequisite paths, page placement, one overview per language) |
| headings | `CM4` (the mandatory heading set per page type, in order) |
| examples | `CM10`–`CM11a`, `E11`–`E12` (fence classification, compilation, invalid-example codes, self-containment) |
| vocabulary | `CM12`, `E5` (implementation vocabulary absent from reader-facing pages) |
| history | `CM13`, `E16` (no issue numbers in Guide prose) |
| terminology | `CM14`, `E6`–`E7` (canonical terms, banned aliases, defined acronyms and scopes) |
| templates | `CM16` (template heading sequences track their exemplars) |

Everything else is `[REVIEW]`, and every `[REVIEW]` rule is a row in
`.github/PULL_REQUEST_TEMPLATE.md`. A rule is never both unlisted and
unenforced: adding a rule to this policy without a gate row means adding a
checklist row in the same pull request.

`CM15` (standards labeling) and the page-purpose rules are reviewed, not
machine-checked: a gate cannot tell whether "supports IFML" is an honest
conformance claim. The reviewer can, and the checklist makes them.
