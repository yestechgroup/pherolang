# 0002 — Static hover reference panels, not a browser IDE

- **Status:** Accepted, 2026-11-16
- **Issue:** pherolang#293 (SH7, epic pherolang#286)
- **Depends on:** SH4 pherolang#316 (static compiler-backed spans) and SH6
  pherolang#317 (LSP semantic tokens and hover)
- **Defers:** Option B — a Monaco/CodeMirror editor with a browser LSP bridge

## Context

Epic pherolang#286 set out to make highlighting compiler-backed everywhere:
one taxonomy, one span stream, and no hand-written grammar. By the time SH7
was scoped, two of those consumers already existed. SH4 renders the
compiler's spans on the static site, and SH6 answers LSP semantic tokens and
hover from the same taxonomy. What remained was a public surface for symbol
documentation: a reader looking at a class, an operation, or a capability
should be able to see its signature and where it is documented.

There were two honest shapes for that surface:

- **Option A — a static hover reference.** Precompute the spans and a small
  symbol index at build time, render an ordinary page, and let CSS reveal a
  preview card over a permanent reference panel. No editor, no client
  JavaScript, no runtime service.
- **Option B — a browser IDE.** Embed Monaco or CodeMirror, speak LSP to a
  running language service over a Worker, WebSocket proxy, or a WASM build of
  the compiler, and let the editor own hover, completion, diagnostics, and
  navigation.

Option B is a real product bet, not a straw man. It is the right shape when
editing, completion, or diagnostics become requirements. This record decides
what to build for hover documentation now, and why the editor is deferred
rather than rejected.

The deciding context is narrow and measurable:

- The public site is prerendered Astro on Cloudflare Workers static assets.
  Both apps already ship static code blocks with no client JavaScript; the
  code-block path is a build-time lookup, not a runtime one.
- SH4 already commits the exact span stream the site renders, and SH6
  already defines the classification, hover content, and per-surface
  documentation links.
- Hover documentation does not require a compiler at read time. It requires
  the compiler's answers, and those can be computed once at build time.
- The demonstration the epic needs is a decision and a measured surface, not
  a general-purpose editor. Two examples on two surfaces are enough to show
  the behaviour and price it.

## Decision

| # | Decision | Rationale |
|---|---|---|
| R1 | Ship a static hover surface at `/demo/hover`, not an editor | It answers the documentation question with prerendered HTML and CSS, and introduces no runtime dependency. |
| R2 | Compiler-derived identities, authored prose | Spans come from the SH4 artifact, symbol ids from the committed semantic graph, and documentation links from the surfaces table plus the docs manifest. The summary is the one freely authored sentence; the signature is authored text that must appear verbatim in the excerpt. |
| R3 | The permanent reference panel is the accessibility contract | Every floating card duplicates an entry that is always visible and carries the real documentation link. The card is `aria-hidden` and has no focusable content. |
| R4 | Zero client JavaScript | The page emits no `<script>` at all. CSS `:has()` chooses which preview is visible; it never carries information that exists nowhere else. |
| R5 | Option B is deferred, not rejected | Monaco/CodeMirror plus an LSP bridge is the path when editing, completion, or diagnostics are required. SH6 remains the editor path and B would reuse it. |
| R6 | The interactive gate is local-only by choice | `site/scripts/gate/hover.mjs` runs inside `node site/scripts/gate.mjs` (index validation, zero-script assertion, gzip CSS budget, Playwright hover/focus/keyboard, focused-state axe, reduced motion). CI's `Site verify` job runs `pnpm -C site test`, which covers the pure committed-index validation, not the browser checks. |

Option A consumes the SH4 span stream and the same taxonomy and vocabulary as
SH6, but it does **not** consume SH6's LSP transport. There is no LSP client,
server, worker, or WebSocket in this slice. The editor remains SH6's story;
this decision only says the public hover surface does not need it.

Option B, if taken later, would reuse SH6 rather than invent a second
language-service path: the same `phero-lsp` implementation, the same legend,
and the same hover content, behind a browser transport and an editor
component.

## The prototype

The prototype is one marketing route with two examples and six symbols:

- `/demo/hover` renders the `.mox` example (a `Ticket` class and its
  `raiseRefund`/`approveRefund` operations) and the `.ddd` example (a
  `TicketService`, its `raiseRefund` operation, and the `RaiseRefund`
  capability).
- `site/data/highlighting/spans.json` is the SH4 artifact. The page looks up
  the exact excerpt bytes and renders the same `hl-*` spans as every other
  Phero Lang block on the site.
- `site/packages/highlighting/hover.json` is the authored index. Each entry
  names a compiler graph node id from `site/data/graph.json`, the SH4 span
  class and `declaration` modifier that identify the token, and a signature
  that must appear (whitespace-normalised) in the excerpt. The documentation
  link is derived from the node file's surface through `surfaces.json` and
  `docs-manifest.json`; a missing page is an error, never a guessed URL.
- `site/packages/highlighting/src/hover.mjs` is the pure build-time logic.
  `site/phero-web/src/components/HoverCode.astro` renders one example,
  `site/phero-web/src/pages/demo/hover.astro` is the route, and
  `site/packages/highlighting/src/hover.css` is the presentation.

The symbol in the code is an ordinary anchor to its panel entry. Pressing
Enter moves focus to that entry (`tabindex="-1"`), and the documentation link
is next in tab order. Hovering or focusing a symbol marks the matching
permanent entry as well as showing the floating card, so the two views cannot
disagree about which symbol is active.

## Costs

Option A was measured on this tree. Option B was not built in this slice, so
its numbers are **estimates** from published package sizes and planned
deployment shapes. Gzip figures use Node's zlib at level 9.

| Cost | Option A (measured) | Option B (estimated, not measured) |
|---|---|---|
| Client JavaScript | 0 bytes; the built page contains no `<script>` | An editor bundle plus a client and protocol layer |
| Editor payload | None | Monaco 0.57.0 unpacked is 101,674,083 bytes including every language and worker; a minimal bundle is typically ~1 MB gzipped |
| Alternative editor payload | None | CodeMirror 6 unpacked: `@codemirror/view` 1,264,544; `@codemirror/state` 440,367; `@codemirror/language` 291,304; `@codemirror/lsp-client` 291,667 bytes; a bundled minimal editor is estimated 150–250 KB gzipped |
| Hover stylesheet | 4,133 bytes raw / 1,235 bytes gzip (gate budget 8 KiB gzip) | Theme and widget CSS in addition to the editor's own |
| Hover index | 3,683 bytes raw / 1,093 bytes gzip; build-time only, never sent to the browser | Symbol data would come from the language service at runtime |
| Built page | 54,265 bytes raw / 8,440 bytes gzip, including the code, the permanent panel, and the cards | Editor shell plus fetched language data |
| Language service transport | None | A WASM build of the Rust LSP estimated at 5–15 MB (not measured; includes compiler and parser), or a Worker/WebSocket proxy with a `pherolang lsp` sidecar |
| Hosting | One more prerendered static asset on the existing Workers static-assets deployment; no new binding, KV, Durable Object, container, secret, or runtime process | Workers/Durable Objects/Containers, WebSocket duration billing, concurrency, cold start, auth/secrets, logs |
| Accessibility work | The shared tokens plus one gate check | Editor virtualization, hover widgets, focus/keyboard complexity, and Monaco's limited screen-reader mode need a separate accessibility spike |
| Maintenance | The authored index and its validator | Editor, LSP client, bridge, protocol version skew, and CSP; WASM needs `'wasm-unsafe-eval'` or a worker origin |
| Build | One extra static Astro page; no compiler run in the browser | A client build plus a service deployment pipeline |

The site now builds 20 marketing HTML pages against a floor of 19. Nothing
about the deployment shape changes: the route is one more file in the same
static asset set.

These numbers are why Option B is deferred. They are not a claim that it
cannot be built.

## Accessibility

The floating card is a visual duplicate, never the source of truth:

- Every annotated symbol has a permanent entry below the code with its kind,
  name, signature, summary, and documentation link. The panel is always
  visible and is ordinary document content.
- The card is `aria-hidden="true"` and contains no links, buttons, or other
  focusable elements, so it cannot create a focus trap or a keyboard detour.
- The symbol is an anchor rather than a hover-only target. Enter moves focus
  to the panel entry; the docs link follows in tab order. Focus and `:target`
  both mark the active entry.
- CSS `:has()` only decides whether a card is painted. A browser without
  `:has()` loses the preview and keeps every fact.
- The surface introduces no animation or transition. The global
  `prefers-reduced-motion` policy is unchanged, and the card appears as an
  instant state change.
- `site/scripts/gate/hover.mjs` checks the focused state with axe and repeats
  the card check under `prefers-reduced-motion: reduce`; the `a11y` check
  covers `/demo/hover` in light and dark.
- Colours come from the existing `--code-*` and surface tokens, so the
  measured contrast pairs cover the new panel rather than adding a palette.

This is not equivalent to an editor's accessible hover widget, and it is not
claimed to be. It is a documentation surface whose interactive affordance is
entirely optional.

## Failure behaviour

A missing or unreadable index (invalid JSON), an unresolved graph node, a
missing span, a signature that is not in the excerpt, or a node with no
documentation page makes that example fall back to the plain highlighted
block and sets `data-hover-fallback` on the wrapper. The page still builds,
the other example still renders, and the fallback is visible rather than
silent.

The `hover` site gate fails on that marker, so the prototype cannot disappear
quietly. Resolution never guesses: a link is emitted only when the surfaces
table and docs manifest name a page.

## What is deferred

- **Option B in full.** Monaco or CodeMirror, a browser LSP client, and a
  transport (WASM or a Worker/WebSocket proxy). Editing, completion, code
  actions, diagnostics squiggles, rename, and multi-file projects are out of
  scope for Option A.
- **Symbol coverage.** The index annotates two examples and six symbols. It
  is not a site-wide symbol table and does not attempt to cover every fence
  or every language page.
- **Authoring convenience.** Adding an entry is a deliberate edit: the
  compiler must already have emitted the node, and the summary is written by
  hand.
- **Mobile editor UX.** The card and panel are responsive; there is no
  mobile editing or touch-hover interaction to tune.
- **Runtime anything.** No API route, no server-rendered hover endpoint, and
  no per-request compiler work.

## Alternatives considered

| Alternative | Why not now |
|---|---|
| Option B: Monaco/CodeMirror plus an LSP bridge | It solves editing, completion, and diagnostics — requirements this slice does not have — at the cost of a large client bundle and either a multi-megabyte WASM artifact or a hosted sidecar. Deferred until those requirements are real. |
| Hand-authored hover markup with no compiler linkage | It would be a second symbol table and a second place for spans to drift. The index is small precisely because every identity is checked against the compiler's committed data. |
| A screenshot, GIF, or video of an editor's hover | It shows the idea but cannot be read, focused, zoomed, or documented, and it ages the moment the UI changes. |
| A runtime hover endpoint | It adds a service, latency, caching, and a failure mode to answer questions that are known at build time. |
| Site-wide hover coverage in this slice | The prose is authored per symbol; coverage is a content cost, not a rendering one. Two examples prove the surface and keep the index reviewable. |
| Shipping Option B behind a flag | A dormant editor still carries the bundle, transport, and accessibility work. A flag would hide the cost, not remove it. |

## Consequences

- **The trade is explicit.** Option A is not a real editor. It cannot edit,
  complete, diagnose, navigate across files, or run a language service in the
  browser. It is a documentation surface for symbols the compiler has already
  emitted.
- **SH6 remains the editor path.** The static surface reuses the taxonomy and
  the span stream, not the LSP transport. A future Option B starts from SH6
  rather than from this page.
- **The index is small on purpose.** It covers two examples and six symbols,
  and its summaries are authored. Spans, nodes, signatures, and links are
  compiler-derived or compiler-validated, so the authored surface is one
  sentence per symbol.
- **The gate is honest about where it runs.** The browser checks are local by
  explicit choice. CI still runs the pure committed-index validation through
  `pnpm -C site test`; the interactive checks are part of the local merge
  gate, not a CI claim.
- **Reversal is additive.** Option B can replace `/demo/hover`, or live
  beside it, without touching the compiler or the SH4 artifact. The cost of
  changing this decision later is the editor work itself, not a migration of
  the static page.
- **Deferring has a price too.** Readers who want completion or diagnostics
  must use an editor with SH6's LSP today. That is the cost this decision
  accepts in exchange for shipping the documentation surface now.

## References

- [../HIGHLIGHTING.md](../HIGHLIGHTING.md) — the highlighting contract and
  the static hover surface
- [../GRAPH.md](../GRAPH.md) — the semantic graph, its node ids, and the LSP
  contract this decision does not consume
- [../../site/packages/highlighting/hover.json](../../site/packages/highlighting/hover.json)
  — the authored symbol index
- [../../site/packages/highlighting/src/hover.mjs](../../site/packages/highlighting/src/hover.mjs)
  — the build-time validator and resolver
- [../../site/phero-web/src/components/HoverCode.astro](../../site/phero-web/src/components/HoverCode.astro)
  — the annotated example component and its permanent panel
- [../../site/phero-web/src/pages/demo/hover.astro](../../site/phero-web/src/pages/demo/hover.astro)
  — the `/demo/hover` route
- [../../site/scripts/gate/hover.mjs](../../site/scripts/gate/hover.mjs) —
  the local hover gate
- [0001](0001-rebrand-to-phero-lang.md) — the naming decision this one is
  numbered after
