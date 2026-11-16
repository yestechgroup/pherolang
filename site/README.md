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

node scripts/fetch-pherolang.mjs   # released v0.3.0 binary, sha256-verified

pnpm dev                     # both apps: :4321 (web) and :4322 (docs)
```

`pnpm dev` chains the docs ingestion, so the docs site always reflects the
current `docs/` tree.

### Regenerate the model data

```sh
node scripts/sync-model.mjs  # site/data/**, from examples/ + docs/ + crates/
```

The output is committed. `git diff --exit-code site/data` after a sync must be
empty — that is the same check CI runs against the released binary.

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

### The gate (this is the merge condition)

```sh
node scripts/gate.mjs        # build, links, html, contrast, a11y, budget,
                             # coverage, freshness, rebrand
node scripts/gate.mjs --list # the checks, in order
node scripts/gate.mjs --only=links,a11y --no-build
```

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
  packages/highlighting/  GENERATED TextMate grammars + language configurations
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
