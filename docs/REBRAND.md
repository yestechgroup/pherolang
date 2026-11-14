# Rebranding: rexlang → Phero Lang

rexlang is now **Phero Lang**. From 0.3.0 the compiler is `pherolang`, the
crates are `phero-*`, the serialized artifacts carry `pheroVersion`, and the
repository is `yestechgroup/pherolang`. The language is unchanged: the
grammar, the file extensions, and the surface pipeline are exactly as they
were. The spelling rules live in [NAMING.md](NAMING.md); this page is the
mechanical map for an existing user.

The one step you cannot avoid is regenerating artifacts — 0.3 does not read
0.2 artifacts:

```sh
PHERO_UPDATE_FIXTURES=1 cargo test --workspace
```

## CLI, install, and repository

| Before | After |
|---|---|
| binary `rexlang <command>` | `pherolang <command>` |
| `cargo run -p rex-cli -- <command>` | `cargo run -p phero-cli -- <command>` |
| `cargo install --git https://github.com/yestechgroup/rexlang ... rex-cli` | `cargo install --git https://github.com/yestechgroup/pherolang --tag vX.Y.Z phero-cli` |
| completions and man pages emitted as `rexlang.*` | `pherolang.*` |
| `rexlang --version` banner | `pherolang --version` |
| repo `yestechgroup/rexlang` | `yestechgroup/pherolang` |

Every subcommand keeps its name and flags: `check`, `fmt`, `ir`, `build`,
`graph`, `gen`, `artifact check`, `explain`, `vocab fetch`, `completions`,
`man`, `lsp`.

## Environment variables

| Before | After | Used by |
|---|---|---|
| `REX_UPDATE_FIXTURES` | `PHERO_UPDATE_FIXTURES` | golden regeneration across the suite |
| `REX_VERSION` | `PHERO_VERSION` | the test-local version constant (`CARGO_PKG_VERSION`) |
| `REX_DISK_MIN_GB` | `PHERO_DISK_MIN_GB` | `scripts/ci-local.sh` free-space warning |
| `REX_EXAMPLES_DIR` | `PHERO_EXAMPLES_DIR` | the examples harness |
| `REX_INSTANCE_OUT` | `PHERO_INSTANCE_OUT` | instance emission in the examples |
| `REX_CURRENCY_INSTANCE_OUT` | `PHERO_CURRENCY_INSTANCE_OUT` | currency-instance emission |
| `REXFMT` | `PHEROFMT` | formatter override |

`REXFMT` is the odd one out: it carries no underscore, so its replacement is
`PHEROFMT`, not `PHERO_FMT`. Every other name takes the `REX_` → `PHERO_`
prefix mechanically. `REX_VERSION` is a test-local constant rather than a
process variable, but it is renamed with the rest and keeps
`CARGO_PKG_VERSION` as its value source.

## Crates and public types

The rule is `rex-<name>` → `phero-<name>`, and all 22 workspace crates are
covered:

| Before | After |
|---|---|
| `rex-syntax` | `phero-syntax` |
| `rex-parser` | `phero-parser` |
| `rex-ir` | `phero-ir` |
| `rex-expr` | `phero-expr` |
| `rex-ifml` | `phero-ifml` |
| `rex-driver` | `phero-driver` |
| `rex-backend-rust` | `phero-backend-rust` |
| `rex-backend-jsonschema` | `phero-backend-jsonschema` |
| `rex-backend-cedar` | `phero-backend-cedar` |
| `rex-backend-cmmn` | `phero-backend-cmmn` |
| `rex-backend-dmn` | `phero-backend-dmn` |
| `rex-backend-common` | `phero-backend-common` |
| `rex-vocab` | `phero-vocab` |
| `rex-lsp` | `phero-lsp` |
| `rex-runtime` | `phero-runtime` |
| `rex-cli` | `phero-cli` |
| `rex-verify` | `phero-verify` |
| `rex-diagnostics` | `phero-diagnostics` |
| `rex-evidence` | `phero-evidence` |
| `rex-strings` | `phero-strings` |
| `rex-dates` | `phero-dates` |
| `rex-testkit` | `phero-testkit` |

Three public types rename with the prefix: `RexBackend` → `PheroBackend`,
`RexError` → `PheroError`, `RexExpr` → `PheroExpr`. Rust imports follow the
module path, so `use rex_driver::…` becomes `use phero_driver::…`, and
**generated Rust** links against `phero_runtime::` instead of
`rex_runtime::`.

## Wire keys and format versions

| Surface | Before | After |
|---|---|---|
| Provenance key | `rexVersion` | `pheroVersion` |
| Instance discriminator | `"$type": "rex.instance"` | `"$type": "phero.instance"` |
| JSON-Schema `$id` URN | `urn:rex:model:<pkg>:<profile>` | `urn:phero:model:<pkg>:<profile>` |
| Every format version | `"formatVersion": 1` | `"formatVersion": 2` |

All ten format constants plus `INSTANCE_FORMAT_VERSION` go `1` → `2`,
because every one of those artifacts now carries a different provenance
key:

| Constant | Artifact |
|---|---|
| `FORMAT_VERSION` | Core IR (`Model`) |
| `ACTOR_MODEL_FORMAT_VERSION` | `ActorModel` |
| `DDD_MODEL_FORMAT_VERSION` | `DddModel` |
| `IFML_MODEL_FORMAT_VERSION` | `IfmlModel` |
| `IFML_MANIFEST_FORMAT_VERSION` | control/route manifest |
| `EVENT_MODEL_FORMAT_VERSION` | `EventModel` |
| `DEPLOY_MODEL_FORMAT_VERSION` | `DeployModel` |
| `USE_CASE_MODEL_FORMAT_VERSION` | `UseCaseModel` |
| `CMMN_MODEL_FORMAT_VERSION` | `CmmnModel` |
| `DMN_MODEL_FORMAT_VERSION` | `DmnModel` |
| `INSTANCE_FORMAT_VERSION` | canonical instance JSON |

Three markers deliberately **stay at 1**: `GRAPH_MODEL_FORMAT_VERSION`, the
`graph query` envelope version, and the receipt and proof-register format
versions. None of them carries a provenance key, so none of them changes
shape; bumping them would invalidate committed graph and receipt artifacts
for a rename that never touched them.

## Artifact naming and filesystem paths

| Before | After |
|---|---|
| `model.rex.json` | `model.phero.json` |
| `target/rex` (default `build` output) | `target/phero` |
| `tests/conformance/ifml/rexlang/` | `tests/conformance/ifml/phero/` |
| `formal/lean/RexExpr.lean` | `formal/lean/PheroExpr.lean` |
| `target/scratch/rex-examples-test` | `target/scratch/phero-examples-test` |
| `rex-cli-tests-*` temp dirs | `phero-cli-tests-*` |

`build` derives each output name from its input path, so `.phero.json` is a
convention documented in the READMEs, not something the command enforces —
an old-named file still builds, it just embeds the new wire generation.

## LSP identity

| LSP surface | Before | After |
|---|---|---|
| Diagnostic `source` | `rexlang` | `pherolang` |
| Server/client identity (`SOURCE_NAME`) | `rexlang` | `pherolang` |
| Binary the client launches | `…/rexlang lsp` | `…/pherolang lsp` |

Client settings keys are unchanged — the VS Code extension still speaks
`mox.server.path`, `mox.server.args`, and `mox.server.languages`, because
the language id did not move.

## What does not change

- **Grammar and file extensions.** `.mox`, `.ddd`, `.actor`, `.dmn`,
  `.evt`, `.uc`, `.cmmn`, `.ifml`, `.deploy`.
- **The wire rules.** camelCase keys, adjacent tagging, the
  `formatVersion` gate, and additive-only evolution.
- **Fixture namespacing.** `nz.example.*` package names.
- **Diagnostic codes.** `E0104` and the rest of the allocation ledger are
  numbers, so `pherolang explain` answers the same codes as before.
- **External toolchains.** sigil, restingstate/xstate-ir, dsntk, cmmn-rs.
- **History.** The v0.2.0 tag and every historical `CHANGELOG.md` section
  stay under the old name.

## 0.2 artifacts are rejected

0.3 does not read 0.2 artifacts, and there is no dual-accept window. A saved
artifact that still carries `rexVersion` or `"$type": "rex.instance"` fails
`pherolang artifact check` with:

```
0.2-era rexlang artifact — regenerate with pherolang 0.3
```

To regenerate, run the migration command once:

```sh
PHERO_UPDATE_FIXTURES=1 cargo test --workspace
```

This is the single allowed pre-1.0 wire break. It is a clean break on
purpose: renaming the provenance key without bumping `formatVersion` would
leave two artifact generations sharing a version number while differing in
shape, which is exactly what the version gate exists to prevent.
[VERSIONING.md](VERSIONING.md) states the policy; the 0.3.0 changelog entry
records the break.

## Downstream consumers

- Bump the git revision **and** the package names in your
  `[dependencies]`: `rex-ir` → `phero-ir`, `rex-driver` →
  `phero-driver`, and so on. The old paths in the pinned revision still
  resolve, but nothing under them is the new API.
- Regenerate every artifact you commit. Expect `pheroVersion` in the
  provenance slot and `"formatVersion": 2` in every file.
- If you generate Rust, the emitted code imports `phero_runtime::`; add
  `phero-runtime` to the generated crate's manifest at the same revision
  and drop `rex-runtime`.
- Run `pherolang artifact check` over your committed artifacts before
  merging — it names every stale file rather than failing generically.

## Operator checklist

Actions that do not appear in a repository diff:

- [ ] Rename the GitHub repository `yestechgroup/rexlang` →
      `yestechgroup/pherolang`, then confirm old URLs redirect — owner:
- [ ] Set About: description, homepage `https://pherolang.com`, topics,
      social preview image — owner:
- [ ] Enable GitHub Discussions — owner:
- [ ] Point the open web epics (#226–#236) at the new repo and asset names
      — owner:
- [ ] Tag `v0.3.0` "Phero Lang" and publish release notes from the
      changelog — owner:
- [ ] Publish renamed release assets (binaries for all five targets plus
      `SHA256SUMS`) — owner:
- [ ] 301 `rexlang.com` → `pherolang.com`, and `docs.rexlang.com` →
      `docs.pherolang.com` — owner:
- [ ] Create the `hello@` and `security@` aliases (open question on the
      epic) — owner:
- [ ] Secure the `@pherolang` social handles (same open question) —
      owner:
