# Versioning and release policy

How Phero Lang versions, what compatibility each version line promises, and
what a release consists of. The consumer-facing drift record is
[CHANGELOG.md](../CHANGELOG.md).

## Versioning scheme

Semver-flavored: `MAJOR.MINOR.PATCH`.

- **0.x (today)**: the language is stabilizing. Breaking changes to the
  grammar, the IR shape, or the CLI are allowed — but every one of them
  must (a) land in the changelog, and (b) include its golden-regeneration
  notes so downstream consumers can see the exact drift. 0.x minor
  releases are the release ladder's rungs (0.2 → 0.3 → …); patch
  releases are fixes and dependency security bumps with no surface
  change.
- **1.0 (the commitment)**: the wire formats freeze. Within
  a given `formatVersion`, evolution is **additive only**: new optional fields
  may appear (with `#[serde(default)]` + `skip_serializing_if`, so
  body-less models stay byte-identical), but no field is renamed, no
  field changes meaning, no existing artifact stops parsing, and no
  golden changes without a `formatVersion` bump. A `formatVersion`
  bump is the only sanctioned breaking change after 1.0, and it must be
  a MAJOR release with a migration note.

## What each artifact promises

- **The Core IR / per-surface artifacts** (`phero_ir` models, the
  `pherolang ir` output): versioned by `formatVersion` (gate-checked on
  read) and stamped with `pheroVersion` provenance. `pheroVersion` is
  provenance, not a compatibility gate — readers gate on `formatVersion`
  only.
- **Generated code** (the Rust backend): target-language code may change
  between 0.x minors; the *runtime* crate (`phero-runtime`) that generated
  code links against is versioned with the workspace, and generated
  output pins its support-lib expectations at generation time.
- **The CLI** (`pherolang`): subcommand surface is additive within 0.x;
  script-breaking changes allowed at minor bumps with changelog notes.

## The 0.3 wire break (Phero Lang rename)

**0.3 does not read 0.2 artifacts.** There is no dual-accept window. The
rename moved the provenance key (`pheroVersion`), the canonical-instance
discriminator (`phero.instance`), and the JSON-Schema `$id` URN, so every
artifact whose bytes depend on them moved to `formatVersion` 2. This is the
**single allowed pre-1.0 wire break**: the version gate's whole purpose is
to stop two differently shaped generations from claiming the same number,
and renaming a key without bumping the marker would do exactly that. No
other 0.x release is permitted the same shortcut.

A saved 0.2 artifact fails `pherolang artifact check` with one named
migration message — never a generic `formatVersion` mismatch and never an
unknown root `$type`:

```
0.2-era <pre-0.3 name> artifact — regenerate with pherolang 0.3
```

The rendered text spells the old product name; [REBRAND.md](REBRAND.md)
quotes it verbatim, because this policy document sits outside the rebrand
guard's allowlist. Regenerate with the one-line command:

```sh
PHERO_UPDATE_FIXTURES=1 cargo test --workspace
```

The markers that stayed at 1 are the ones with no provenance dependency:
`GRAPH_MODEL_FORMAT_VERSION`, the `graph query` envelope version, and the
receipt and proof-register format versions. Their shapes did not change, so
bumping them would invalidate committed artifacts for a rename that never
touched them. The `formatVersion` semantics below (exact-match gate,
additive-only inside a version) are unchanged.

## Golden fixtures are the spec

`tests/conformance/` and `examples/artifacts/` goldens are byte-compared
by the suite. The rule for touching them: regenerate with
`PHERO_UPDATE_FIXTURES=1 cargo test --workspace`, then **read the diff** —
goldens are the spec, not an output. Body-less/vocabulary-less models
must stay byte-identical.

## What a release consists of

1. The milestone's PRs are merged with the full local gate green
   (`scripts/ci-local.sh`; `audit` subcommand for the supply-chain
   gates) and CI green.
2. The workspace version is bumped in the release PR; every golden
   containing `pheroVersion` is regenerated in the same PR, and the diff
   review confirms the only content change is the provenance line.
3. `CHANGELOG.md` gains the release entry (Keep a Changelog format,
   linked to the milestone PRs).
4. At milestone closeout: `git tag vX.Y.Z` on main, the tag is pushed,
   and a GitHub Release is published with notes drawn from the
   changelog. Every later release gets a tag + release notes; the
   changelog is the single place a consumer checks for drift.
5. `docs/VERIFICATION.md` (the proof register) is untouched by
   dependency-only releases; a toolchain bump that affects
   `phero-verify` must re-run `cargo creusot -p phero-verify` and update the
   register in the same PR.

## Dependency policy (short form)

- Security advisories (`cargo audit`) are fixed on the current minor;
  the CI `audit` and `deny` jobs keep the tree at zero.
- Git-pinned toolchains (sigil, restingstate, dsntk) move by rev, with
  the compatibility boundary named in the workspace `Cargo.toml`
  comment.
- New dependencies need a job the stdlib-ish status quo can't do, and an
  equivalence story when they replace hand-written logic (see the
  `heck`/instance-id precedent in #220).
