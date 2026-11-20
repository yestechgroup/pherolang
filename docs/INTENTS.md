# Intent specifications (`.intent`)

An `.intent` file states **requirements as model data**: one requirement per
`intent` declaration, with a stable id (`LOAN-001`) that requirements,
artifacts, and (later, #137's receipts) key on, the model elements that
express it, and the model-native checks that verify it. The compiler never
loses a requirement again: the project-level **coverage pass** reports every
intent nothing expresses and nothing verifies — honesty over reach.

This page is the surface's wire and tooling contract. The normative grammar
(EBNF) lives exactly once, in the parser module docs:
[`crates/phero-syntax/src/parser/intents.rs`](../crates/phero-syntax/src/parser/intents.rs).

## Example

```intent
import "../examples/support/support.mox"

intent SUPPORT-001 "A refund above the desk's auto-approval limit is decided by a human." {
    element class:RefundRequest;
    element capability:RaiseRefund;
    check decision:RefundAuthority;
}
```

- **Imports come first**: an `import` after the first `intent` is a syntax
  error. Imports are **raw `.mox` domains only** (the `.actor` discipline) —
  they exist so an intents file clusters into the right application. Links
  are *not* imports: they name elements of the whole application.
- **The stable id** is written verbatim (`LOAN-001`): an uppercase lead
  segment plus at least one hyphen-separated all-digits segment. The
  compiler slices it exactly as written — leading zeros survive.
- **The statement** is the requirement, quoted. An empty statement is
  refused (`E0452`).

## Links

Every link is `kind:name;` under one of two slots:

| Slot | Keyword | Kinds | Resolves against |
|---|---|---|---|
| element | `element` | `class`, `capability`, `event`, `view`, `command`, `op` | the imported domain union · the application's actor policies · its event contracts · its flows' views and CLI commands · its designs' operations (`Service.operation`) |
| check | `check` | `invariant`, `decision` | a designed class carrying at least one invariant · a `.dmn` decision |

A kind in the wrong slot is a compile error (`E0453`); a repeated
kind+name pair is `E0454`. **The artifact keeps every name exactly as
authored** — resolution is the coverage pass's job, never the wire's.

## The coverage pass

`pherolang check <dir>` runs the pass per application in its phase 2, over
the compiled artifacts:

- every intent must map to **≥ 1 resolved element** and **≥ 1 resolved
  check** — a missing slot **warns** (`X0009` / `X0010`), because an
  uncovered intent is a claim nobody can honor and the honest state should
  be visible, not hidden;
- a link naming nothing of its kind **errors**, spanned, with a
  did-you-mean candidate when one is close (`X0011` / `X0012`);
- a bare name matching several targets of its kind **errors** — the pass
  refuses to guess a traceability target (`X0013`);
- a stable id declared twice across the application's intent files
  **errors** (`X0014`; within one file the surface check is `E0451`).

See [docs/DIAGNOSTICS.md](DIAGNOSTICS.md) for every code, or
`pherolang explain X0009` for one.

## The artifact

`pherolang ir x.intent` emits the versioned `IntentModel` wire artifact
(`formatVersion` 1, camelCase, empty collections omitted): the intents in
declaration order, each with its `id`, `statement`, `elements`, and
`checks` (`kind` tag + authored `name`). Spans are never serialized — the
compile's span table travels beside the artifact for the coverage pass's
spanned diagnostics, the `.ifml` auth-sites discipline.

## Tooling status

- `pherolang check` / `ir` / `build` compile `.intent` files; `fmt`
  canonicalizes them (imports first, one blank line between intents, one
  link per line, canonical `;`).
- The editor publishes per-file compile diagnostics for open `.intent`
  documents. The coverage pass is a project-level phase-2 pass, like the
  integrity checks; it is not editor-hosted in v0.5.
- Intent files contribute `intent` nodes plus one `expresses`/`verifiedBy`
  edge per resolved link (issue #363) — `affected-by` on an expressing
  capability, class, event, view, command, operation, or decision reaches
  the requirement that states it. The coverage pass stays the authoring
  authority; a link the graph cannot bind uniquely emits **no edge**, never
  a guess. #137's intent → element → verification receipt table is the
  follow-up (#364).
