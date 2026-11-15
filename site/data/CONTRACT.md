# `site/data` — the interface between the toolchain and the site

Everything in this directory is **generated** by `node scripts/sync-model.mjs`
and committed. Nothing here is written by hand, and nothing here may be edited
by hand: change the source (`examples/`, `docs/`, `crates/`, `tests/`) and rerun
the script.

The CI `Site` workflow downloads the released `pherolang` asset, runs the same
script, and fails if `git diff` is non-empty. That is the entire honesty
mechanism: if the site says something the toolchain does not do, the build breaks.

```
node scripts/fetch-pherolang.mjs     # once: released v0.3.0 binary, sha256-verified
node scripts/sync-model.mjs          # regenerate this directory
```

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
| `docs-manifest.json` | `{ formatVersion, generated[] }` — source → output mapping | `ingest-docs.mjs` | gate (docs coverage) |
| `support-model.json` | **lane 5's contract**, below | `scripts/render-graph.mjs` | lane 6 (island) |

### `support-model.json` — the demo projection (lane 5 → lane 6)

Baked layout, so the first paint is static SVG and JavaScript only enhances:

```jsonc
{
  "formatVersion": 1,
  "application": "support/support.mox",
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
(the gate asserts referential integrity); coordinates are deterministic
(`node scripts/render-graph.mjs` twice → identical bytes).

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

## The docs site is not a copy

`phero-docs/src/content/docs/` is gitignored and regenerated from `<repo>/docs/`
on every `dev`/`build`/`check`. Never write prose into it, and never copy
`docs/` content into this repository — the docs site is a projection, not a fork.
