# `site/data` — the interface between the toolchain and the site

Everything in this directory is **generated** and committed. Nothing here is
written by hand, and nothing here may be edited by hand: change the source
(`examples/`, `docs/`, `crates/`, `tests/`) and rerun the owning script.

`node scripts/sync-model.mjs` owns the model data; `node
scripts/sync-highlight.mjs` owns `highlighting/`. The CI `Site` workflow runs
both, and fails if `git diff` is non-empty. That is the entire honesty
mechanism: if the site says something the toolchain does not do, the build
breaks.

Both scripts resolve one site binary through
`site/scripts/highlighting/runner.mjs`: a release that can check the current
examples **and** provides `pherolang highlight`, otherwise a source build of
this checkout. The v0.3.0 release predates both, so the source build is a
documented temporary exception (`docs/HIGHLIGHTING.md`), never a claim of
released-binary freshness. The gate reports which binary produced the data.

```
PHEROLANG_VERSION=0.4.0 node scripts/fetch-pherolang.mjs  # pinned, sha256-verified; falls back to the latest published release while 0.4.0 is untagged
node scripts/sync-model.mjs          # graph, artifacts, evidence, docs inventory
node scripts/sync-highlight.mjs      # highlighting/spans.json
node scripts/sync-highlight.mjs --check  # fresh? no write
```

The pinned `0.4.0` is the first release with `pherolang highlight`. Until it is
tagged, `deploy` in `site.yml` refuses to run because the latest release
version does not equal the workspace version; previews still run. The operator
sequence is `docs/RELEASE-0.4.0.md`.

### `highlighting/spans.json` — the compiler-backed span artifact

The key is `sha256(surface + "\0" + exact source bytes)`; the entry carries the
CLI artifact's own `sourceHash` (over the source alone), `degraded`, and the
`spans[]` byte offsets/classes/modifiers. The inputs are exactly three buckets:

1. every Phero Lang fence in `docs/**/*.md` — normalized exactly as Expressive
   Code renders it (trailing whitespace trimmed, leading/trailing blank lines
   dropped); the script fails with the file and line on a mismatch;
2. every `examples/support/support.*` model;
3. every excerpt declared in `site/packages/highlighting/excerpts.json`.

There are no paths, filenames, timestamps, colours, or environment data in the
file. `formatVersion` is the site artifact's own gate (currently `1`); a
mismatch makes both renderers fall back to plaintext **and** mark the block, and
`gate/highlighting.mjs` fails. `generator.version` is provenance only: it must
equal the workspace version, enforced by `scripts/test/release.test.mjs`.
Regeneration is a no-op against the committed bytes when the source and binary
agree; a diff means stale data, not noise.

Both apps render the spans statically (the marketing `CodeBlock.astro` and the
docs Expressive Code plugin) and add no client JavaScript. `gate/budget.mjs`
proves every marketing page except the documented graph island on `/demo` is
zero-JS.

---

## Files

| File | Shape | Written by | Read by |
|---|---|---|---|
| `graph.json` | `phero_ir::graph::GraphModel` — `{ formatVersion, applications[], nodes[], edges[] }` | `pherolang graph build examples` | lane 5 (figure), lane 6 (island) |
| `surfaces.json` | `{ formatVersion, surfaces[] }` — the nine surfaces, authored words + derived status | sync-model (`scripts/surfaces.mjs` ⊕ evidence) | lane 3 (language pages), lane 5 (architecture) |
| `evidence.json` | `{ formatVersion, generator, counts, applications[], kinds{ nodes, edges }, crates[], surfaces[] }` | sync-model | gate, lane 5 (stats) |
| `artifacts/<id>.json` | one compiled artifact per surface (`mox.ts`…`deploy`) | `pherolang ir <example>` | lane 3 (viewer), lane 2 (proof) |
| `receipt.json` | `support.receipt.json`, copied | copy | lane 2/5 (verification story) |
| `register.json` | `docs/verification/register.json`, copied | copy | lane 2/5 (proof-obligation ladder) |
| `docs-inventory.json` | `{ formatVersion, docs[] }` — title, slug, size, lines | sync-model | `ingest-docs.mjs` |
| `docs-manifest.json` | `{ formatVersion, generated[] }` — `{ source, slug, collection, target, url }` per page, ADRs included | `ingest-docs.mjs` | gate (docs coverage), lane 3 (doc URLs) |
| `highlighting/spans.json` | `{ formatVersion, generator, entries }` — one `{ surface, sourceHash, degraded, spans[] }` per `sha256(surface + "\0" + exact source bytes)` key | `scripts/sync-highlight.mjs` | both apps' renderer, `gate/highlighting.mjs` |
| `support-model.json` | **lane 5's contract**, below | `scripts/render-graph.mjs` | lane 6 (island) |

### `support-model.json` — the demo projection (lane 5 → lane 6)

Baked layout, so the first paint is static SVG and JavaScript only enhances.
`application` is the authored application name when the component has exactly
one `.ddd` root (DD8) — here `SupportDesk` — otherwise the graph's file-based
id. `provenance` is **base-relative** (`support/support.mox`), exactly as the
graph artifact writes it — not repo-relative. `meta` carries the application id, the
source artifact, the graph totals and the per-perspective counts; each
perspective also carries a `viewport` (`width`/`height`) and a `groups[]` array
of cluster header boxes. Lane 6 needs only `nodes`/`edges`.

```jsonc
{
  "formatVersion": 1,
  "application": "SupportDesk",
  "perspectives": [
    {
      "id": "domain",                  // domain | decisions | events | workflows |
                                       // authorisation | experience | deployment | cases
      "label": "Domain",
      "surface": "mox",                // the surface whose nodes dominate this view
      "nodes": [
        {
          "id": "class:support/support.mox#nz.example.support.Ticket",
          "kind": "class",
          "label": "Ticket",           // short display name (never the fq name)
          "group": "nz.example.support", // lane 5's clustering key
          "x": 120, "y": 64, "w": 148, "h": 44,   // baked SVG geometry
          "detail": "…",               // one sentence, generated from kind + counts
          "provenance": "examples/support/support.mox"
        }
      ],
      "edges": [{ "from": "…", "to": "…", "kind": "refers" }]
    }
  ]
}
```

Rules: node ids are the graph artifact's ids verbatim (so a visitor can find the
same node in `graph.json`); every edge endpoint exists in the same perspective
(the gate asserts referential integrity); no two node boxes overlap and every
box sits inside its group and the viewport (asserted by the script); coordinates
are deterministic (`node scripts/render-graph.mjs` twice → identical bytes).

One slug caveat: `docs-inventory.json` keeps the document's own slug
(`release-0.3.0`) as its identity key, while the *emitted* page slug drops dots
(Starlight does), so `target`/`url` in the manifest read `release-030`. Compare
`url`, not `slug`, when resolving a link.

---

## Ownership rules (parallel lanes)

- **`site/package.json`, `pnpm-workspace.yaml`, `packages/tokens/**`, `packages/ui/**`,
  `phero-web/src/layouts/Base.astro`, both `astro.config.mjs` (except the docs
  sidebar) and both `wrangler.jsonc` are integrator-owned.** Do not edit them; ask.
- Lane 1 owns `site/phero-docs/**` (including its sidebar config and theme) and
  `scripts/ingest-docs.mjs`.
- Lanes 2–6 own only the files listed in their brief.
- `site/data/**` is generated. If you need a new field, edit `scripts/sync-model.mjs`
  (or `scripts/render-graph.mjs` for `support-model.json`) and say so.
- The one authored data file lives in the app, not here:
  `phero-web/src/data/commands.json` — every shell command the site shows, the
  single list `scripts/check-commands.mjs` executes and the gate runs.

## The docs site is not a copy

`phero-docs/src/content/docs/` is gitignored and regenerated from `<repo>/docs/`
on every `dev`/`build`/`check`. Never write prose into it, and never copy
`docs/` content into this repository — the docs site is a projection, not a fork.
