# 0001 — Rebrand rexlang to Phero Lang

- **Status:** Accepted, 2026-11-14
- **Issue:** pherolang#270
- **Supersedes:** the naming decision D1 in pherolang#227

## Context

The project acquired `pherolang.com`. That made a name change possible and,
for the first time, cheap in a specific and measurable way. Every
alternative cost arrives after the first release:

- No crate in the workspace is published to crates.io, so no package name
  is claimed and no dependent exists to deprecate.
- v0.2.0 shipped no release assets, and the release pipeline (#236) is
  still open — it will be authored once, under the final name, instead of
  built and then renamed.
- The only external consumers are rev-pinned sibling repositories, which
  move by editing one revision string.
- The web presence exists only as brand SVGs and open epics (#226–#236).
  No page, route, deployment, or DNS record has been built.

### The recorded reversal

The earlier web-presence charter (#227, decision D1) locked the name as
`rexlang`. Its reasoning was search, not identity: an unrelated REXYGEN VS
Code extension owned the results for the name, so differentiation was to be
carried by pairing the name with its descriptor rather than by renaming.

That reasoning is superseded. Its premise was that the token was contested
and the contest was unwinning; the acquired origin changes the trade. The
old collision disappears with the name — `Phero Lang` does not answer to
REXYGEN's query. The new collision is narrower: it attaches to the bare
token `phero` (the npm package and Go's `henomis/phero`), not to the
compound. That is a spelling problem with a spelling fix, so it is handled
by the never-bare-token rules in [../NAMING.md](../NAMING.md) rather than by
avoiding the name. Formal trademark and search clearance for the compound
remains an open question on the epic and is not claimed here.

## Decision

| # | Decision | Rationale |
|---|---|---|
| R1 | Full rename: crates `rex-*` → `phero-*`, binary `rexlang` → `pherolang`, wire keys and environment variables included | A partial rename leaves `rex_*` imports in every file forever; pre-1.0 with nothing published is the only free moment. |
| R2 | The binary is `pherolang`, never `phero` | Bare `phero` collides with the npm package and `henomis/phero`; the compound is the defensible name. |
| R3 | Wire break with a format-version bump | Every surface stamps provenance, so every artifact generation changes; the alternative is two shapes sharing one version number. |
| R4 | Rewrite the web epics in place | Nothing under #226–#236 is implemented, so editing titles and bodies costs a day and leaves no redirect debt. |
| R5 | Reinterpret the mark, do not redraw it | The 64-unit grid, crossing strokes, terminal nodes, and lime core stay; only the rationale changes from "X from *rex*" to "many surfaces converge on one semantic core", which fits *phero* ("to carry") better. |

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Keep `rexlang` and lean on the descriptor | A descriptor is copy, not a token: it does not move the search result that motivated D1, and it leaves the acquired origin unused. The cost of renaming only rises after the first published release. |
| Rename to bare `phero` | Directly collides with the npm `phero` package and `henomis/phero`; an unqualified token is the one thing the project cannot claim on the registries it would publish to. |
| Partial rename: crates only, binary unchanged | Splits one identity across two spellings and leaves `rex_*` imports in every file permanently — the exact split-brain failure the naming contract exists to prevent. |
| Dual-accept old wire keys for a deprecation window | Two artifact generations would share `formatVersion` while differing in shape, which is the failure the gate exists to catch. There is also no population to protect: no published crate and no release asset ever carried a 0.2 artifact. |
| Rename everything, including git history and old tags | Consumers pin by revision; rewriting history breaks those pins to no benefit. The v0.2.0 tag is immutable history, and `git mv` plus `git log --follow` already preserve the file-level trail. |

## Consequences

- **Wire break.** `rexVersion` → `pheroVersion`, `"rex.instance"` →
  `"phero.instance"`, `.rex.json` → `.phero.json`, and all ten format
  constants plus `INSTANCE_FORMAT_VERSION` go `1` → `2`. Graph, graph-query,
  receipt, and register formats keep version `1` because they carry no
  provenance key.
- **One-time regeneration.** Committed goldens and downstream artifacts are
  rewritten with `PHERO_UPDATE_FIXTURES=1 cargo test --workspace`; a second
  clean run must produce no diff.
- **Downstream churn.** The four rev-pinned consumers bump one revision and
  regenerate their artifacts; the blast radius is internal and bounded.
- **A guard, not a reminder.** `scripts/check-rebrand.sh` fails the gate on
  an old-name token outside its allowlist, which is exactly: historical
  `CHANGELOG.md` sections, this ADR, [../REBRAND.md](../REBRAND.md),
  [../NAMING.md](../NAMING.md), the deliberate legacy-artifact fixture, and
  the legacy-detection module. Every other reintroduction is a build
  failure.
- **The cost of deferring.** After crates.io publication or a released
  artifact, the same rename becomes a deprecation programme across external
  dependents and a migration note for every consumer — the delta this
  decision exists to avoid.

## References

- Epic pherolang#270 — decisions R1–R5, the naming contract, the sequence
- Sub-issues pherolang#271–#278 — naming/guard, rename, wire break,
  metadata and release, downstream, web, brand, site deployment
- [../NAMING.md](../NAMING.md) — the normative spelling contract
- [../REBRAND.md](../REBRAND.md) — the old → new migration table
- [../VERSIONING.md](../VERSIONING.md) — what 0.3 promises about the break
- [../../CHANGELOG.md](../../CHANGELOG.md) — the 0.3.0 entry
