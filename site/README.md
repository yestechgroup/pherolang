# The Phero Lang web presence

Two Astro apps, one workspace, one rule: **the site is a consumer of the
toolchain, not a description of it.** Every diagram, count, artifact and status
on these sites is generated from this repository by the released `pherolang`
binary. If the site says something the toolchain does not do, the build fails.

| App | Origin | Purpose |
|---|---|---|
| `phero-web` | `https://pherolang.com` | marketing, language pages, architecture, the graph explorer |
| `phero-docs` | `https://docs.pherolang.com` | the reference manual, generated from `<repo>/docs/` |

Deployment: `.github/workflows/site.yml` (path-filtered to `site/**`), Workers
static assets, two projects. The account-owner checklist is
[`DEPLOY.md`](./DEPLOY.md).

## Run it locally

Node is pinned to `24.16.0` (Astro 7 requires `>= 22.23.4`); the workspace is
pnpm 10.

```sh
cd site
fnm use                      # or: nvm use  (respects .nvmrc)
pnpm install

node scripts/fetch-pherolang.mjs   # pinned v0.4.0 (falls back to the latest published release while untagged), sha256-verified

pnpm dev                     # both apps: :4321 (web) and :4322 (docs)
```

`pnpm dev` chains the docs ingestion, so the docs site always reflects the
current `docs/` tree.

### Regenerate the model data

```sh
node scripts/sync-model.mjs      # graph, artifacts, evidence, docs inventory
node scripts/sync-highlight.mjs  # site/data/highlighting/spans.json
```

Both outputs are committed. `git diff --exit-code site/data` after a sync must
be empty — that is the same check CI runs.

`sync-highlight` writes one content-addressed entry per `(surface, exact source
bytes)` pair, covering every Phero fence in `docs/**/*.md`, every
`examples/support/support.*` model, and every excerpt declared in
`site/packages/highlighting/excerpts.json`. Both apps read the same artifact
through `site/packages/highlighting/src/`; no app runs the compiler at build
time and nothing is generated per request.

**Release-binary status:** the latest release, v0.3.0, predates both
`pherolang highlight` and the DD9 examples. The tree is prepared for **v0.4.0
(the first release with the command)**; the tag/publish is the operator step
in [`docs/RELEASE-0.4.0.md`](../docs/RELEASE-0.4.0.md). Until then:

- `scripts/fetch-pherolang.mjs` pins `0.4.0`; while that tag is unpublished an
  unset `PHEROLANG_VERSION` falls back to the latest published release with a
  warning (an explicit pin never falls back).
- the shared resolver (`scripts/highlighting/runner.mjs`) refuses a binary that
  cannot check `examples/support/support.ddd` or lacks `highlight`
  (`PHEROLANG_BIN` first, then the fetched `.bin`, then `PATH`) and otherwise
  builds the CLI from the checkout (`cargo build -q -p phero-cli`).
- `sync-model` and `sync-highlight` both use that resolver, so one binary
  produces the model data, the highlight artifact, and the commands the gate
  executes.

This is a temporary exception, not released-binary freshness; the `Site`
workflow installs Rust for the freshness/drift jobs and warns when the fallback
runs. Its `deploy` job waits until the latest release version equals the
workspace version, so production never advertises a download that does not
exist. After the tag, regenerate with the released binary and update
[`docs/HIGHLIGHTING.md`](../docs/HIGHLIGHTING.md#release-binary-status).

The docs app integration is an Expressive Code plugin
(`site/phero-docs/ec.config.mjs`) that adds the compiler's `hl-*` classes inside
Expressive Code's own line ASTs, so copy buttons, line numbers, theme switching
and a11y are untouched. The chosen path and its trade-offs are recorded in
[`docs/HIGHLIGHTING.md`](../docs/HIGHLIGHTING.md#docs-app--the-chosen-integration).

### Validate the generated editor grammars

The TextMate grammars and language configurations under
`site/packages/highlighting/` are generated from the highlighter metadata (never
hand-edited):

```sh
# from the repository root
cargo run -q -p phero-cli -- grammar --out site/packages/highlighting
cargo run -q -p phero-cli -- grammar --check site/packages/highlighting
node site/scripts/check-grammars.mjs   # Shiki load + tokenization + scope pins
```

`node site/scripts/check-grammars.mjs` is also
`pnpm -C site run check:grammars`. It loads every grammar in Shiki, tokenizes
`examples/support/support.*`, pins the representative scopes, and compares the
lexical scopes with the committed compiler-backed artifacts. The Rust test
`cargo test -p phero-highlight` byte-compares the committed package, so a hand
edit fails even without Node. The site may use these files as the fallback
grammar; there is no second copy.

### The hover prototype (SH7)

`/demo/hover` is the SH7 decision surface: compiler-backed spans, a small
authored symbol index (`site/packages/highlighting/hover.json`) keyed to graph
node ids, and a CSS-only preview card over a permanent reference panel. The
page ships no client JavaScript. The decision and its costs are recorded in
[ADR 0002](../docs/adr/0002-static-hover-reference-panels.md).

```sh
node scripts/gate.mjs --only=hover   # index, zero-script, budget, keyboard, axe
```

`a11y` also covers `/demo/hover` in light and dark. The browser checks are
local-only; `pnpm test` includes the pure committed-index validation that CI's
`Site verify` job runs.

### The gate (this is the merge condition)

```sh
node scripts/gate.mjs        # build, links, html, contrast, a11y, budget,
                             # coverage, highlighting, hover, freshness, rebrand
node scripts/gate.mjs --list # the checks, in order
node scripts/gate.mjs --only=links,a11y --no-build
```

`highlighting` proves every Phero block in both built apps carries compiler
classes, no raw Phero block remains, every docs fence language is covered or
explicitly exempt, and the committed artifact covers exactly the current
sources. `budget` proves every marketing page except the documented graph
island on `/demo` is zero-JS, and keeps the island under 12 KB gzip. `hover`
proves the prototype's index resolves, its page emits no `<script>`, its
stylesheet stays under 8 KiB gzip, and hover/focus/Enter work in a browser.
`freshness` reruns both sync scripts and fails on any `site/data` diff. The
code-token colours are part of `contrast`/`a11y` in both modes.

The Node suite (`pnpm test` → `node --test scripts/test/*.test.mjs`) covers the
pure scanners, the highlight artifact lookup, the hover index, the zero-JS
classifier, and the release-artifact/version contract. CI's `Site verify` job
runs it after the build and the highlighting gate.

The release-window guard: `site.yml` `detect` reports the workspace version and
the latest release version; `deploy` requires them to be equal. A 0.4.0 bump PR
therefore merges with previews but does not deploy production until v0.4.0 is
tagged.

`a11y` uses axe over the built output through Playwright. If the browser is
missing, the gate fails with the install command rather than skipping:

```sh
pnpm exec playwright install chromium
```

### Build and smoke-test a deploy

```sh
pnpm -C phero-web build && pnpm -C phero-docs build
pnpm -C phero-web exec wrangler deploy --dry-run
pnpm -C phero-docs exec wrangler deploy --dry-run   # "No bindings found."
```

## Layout

```
site/
  package.json            workspace root: scripts, engines, pnpm build allowlist
  pnpm-workspace.yaml     the two apps
  .nvmrc                  24.16.0
  brand/                  the identity assets (mark, lockup, favicons, social card)
  packages/tokens/        the ONLY place a colour, font or radius is defined
  packages/ui/            shared primitives (nav, footer, button, prose, …)
  packages/highlighting/  GENERATED grammars/configs + authored excerpts/hover indexes
  data/                   GENERATED model data + CONTRACT.md (read it)
  scripts/                fetch, sync, render, ingest, check, gate
  phero-web/              the marketing app
  phero-docs/             the docs app (Starlight)
```

Two things that surprise people:

- `packages/ui` is **not** a workspace package. It is aliased into the apps
  (`@ui`), which keeps Vite compiling the `.astro` sources instead of treating
  them as pre-built dependencies. Because of that, `phero-web/src/styles/global.css`
  must keep its `@source` line or the shared primitives lose their utilities.
- `phero-docs/src/content/docs/` is **gitignored**. It is regenerated from
  `<repo>/docs/` on every dev/build. Never write prose there.

## Brand rules that are enforced, not suggested

- The product is **Phero Lang**, two words. The wordmark and the binary are
  `pherolang`. The crates are `phero-*`. The pre-0.3 name is gated by
  `scripts/check-rebrand.sh`.
- Colours come from `packages/tokens` only; `contrast.json` pins the measured
  WCAG pairs and the gate recomputes every ratio.
- Electric lime is a fill and an accent on graphite. It is never text on warm
  white (1.15:1).
