# Documentation standard

This directory is the governance set for Phero Lang documentation. It defines
what a page must be, so that the site can be a coherent product rather than a
collection of related documents.

## Read in this order

| Document | Answers |
|---|---|
| [DOCUMENTATION_STRATEGY.md](DOCUMENTATION_STRATEGY.md) | Why the documentation has this shape, and what it optimizes for. |
| [INFORMATION_ARCHITECTURE.md](INFORMATION_ARCHITECTURE.md) | Where a page belongs, and which of the two products it serves. |
| [CONTENT_MODEL.md](CONTENT_MODEL.md) | The frontmatter contract and the required headings for each page type. |
| [EDITORIAL_POLICY.md](EDITORIAL_POLICY.md) | The normative rules, each tagged `[CI]` or `[REVIEW]`. |
| [WRITING_STYLE_GUIDE.md](WRITING_STYLE_GUIDE.md) | How a Phero Lang page sounds, with worked rewrites. |
| [glossary.yaml](glossary.yaml) | Canonical terms, banned aliases, allowed acronyms, implementation vocabulary. |

The [templates](templates/) directory holds the five page skeletons. The five
`.mox` pages under [`docs/guide/mox/`](../guide/mox/overview.md) are the working
exemplars: they were written to this standard, follow every heading set, and
their examples compile under `pherolang check`.

## The deciding question

Before writing, answer: **which product is this page part of?**

- **Guide** — a user learning or doing something with the language
  (`docs/guide/`). Its four types are tutorial, how-to, concept, and reference,
  plus the language overview.
- **Engineering reference** — a contributor or an integrator who needs the exact
  contract (`docs/*.md`).
- **Integration reference** — an external tool author consuming an artifact.
  Visible in its own right, never buried in engineering lore.

If a page answers two, it becomes two pages. The detail belongs to one
authoritative page; everything else links to it.

## Enforcement

| Class | Meaning | Where it is checked |
|---|---|---|
| `[CI]` | A hard gate. The pull request cannot merge on a violation. | `pherolang docs lint`, wired as a step in `scripts/ci-local.sh` |
| `[REVIEW]` | A blocking reviewer duty, listed in the pull-request template. | `.github/PULL_REQUEST_TEMPLATE.md` |

There is no advisory class. The `[CI]` gates are specified in
`EDITORIAL_POLICY.md` §6 and `CONTENT_MODEL.md`; the review checklist exists so
that the rules no gate can express still block a merge.

## Adding a page

1. Choose the type and confirm it against `CONTENT_MODEL.md` §2.
2. Copy the matching skeleton from [`templates/`](templates/).
3. Fill the frontmatter. `title`, `description`, `type`, `language`, `section`,
   `order`, `status`, `docStatus`, `audience`, and `lastReviewed` are required.
4. Write the page. Follow the heading set exactly; delete nothing.
5. Run the documentation gate from the repository root:

   ```sh
   cargo run -q -p phero-cli -- docs lint
   ```

   It checks the frontmatter contract, the heading sets, page placement,
   terminology, implementation vocabulary, issue references, template drift,
   and compiles every `mox` fence (`mox,fragment` and `mox,invalid=<CODE>` are
   classified, not compiled as complete examples).
6. Add the routing-table row if the page introduces a new document.

The same gate is CI step 3/8 (`scripts/ci-local.sh docs-lint`). The `[REVIEW]`
rows in the pull-request template remain blocking for the rules a machine
cannot check.

## Changing the standard

The standard is code-adjacent: a rule exists only when its gate or checklist row
exists. A change to a governance document lands with the corresponding change
to the gate. A rule that cannot be checked yet is written as `[REVIEW]`, never
as advice.
