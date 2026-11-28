# Naming

The normative spelling contract for the product, the toolchain, and
everything that carries their name. This is the only document that states
the rules: `README.md`, `AGENTS.md`, the surface docs, and the web epics
link here instead of restating them. A spelling question this page does not
answer is a missing row, not a local judgment call.

## The name

| Context | Form |
|---|---|
| Product, first mention | **Phero Lang** |
| Wordmark | `pherolang` |
| CLI binary | `pherolang` |
| Crates | `phero-*` (`phero-ir`, `phero-driver`, `phero-cli`, `phero-runtime`, …) |
| Repo | `yestechgroup/pherolang` |
| Origins | `pherolang.com`, `docs.pherolang.com` |
| Descriptor | "Phero Lang — Verifiable Model-Driven Engineering" |

## The two never-rules

**Never bare `Phero` in copy.** Product prose always says "Phero Lang", two
words. The single token is reserved for nothing — it is not a shorthand, not
a heading, not a logo lockup.

**Never `phero` as a binary or Cargo package name.** The binary is
`pherolang`; the Cargo packages are `phero-<name>` or `phero_<name>`. The
bare token appears in no `[[bin]]`, no `[package] name`, no install command.
(Model namespaces inside `.mox`/`.dmn` fixtures are a separate vocabulary:
the conformance corpus uses `phero.conformance.*` and `phero.auth`, which is
the fixture namespace, not a package on a registry.)

Both rules exist because the bare token is already taken. `phero` is the
name of an npm package (a TypeScript full-stack framework) and of the Go
multi-agent framework `henomis/phero`. A search for the bare word lands on
those projects; the compounds are what this project can own. The product
name stays two words for the same reason: it never renders as a single
token those projects already answer to.

## Identifier conventions

| Thing | Convention | Example |
|---|---|---|
| Crate name | `phero-<name>`, hyphenated | `phero-ir`, `phero-backend-common` |
| Rust module path | `phero_<name>`, underscored | `phero_driver::compile_str` |
| Public type prefix | `Phero` | `PheroBackend`, `PheroError`, `PheroExpr` |
| Environment variable | `PHERO_*` | `PHERO_UPDATE_FIXTURES` |
| Wire provenance key | `pheroVersion` | every emitted artifact |
| Instance discriminator | `phero.instance` | canonical instance `$type` |
| Artifact file convention | `.phero.json` | `model.phero.json` |
| Default build output | `target/phero` | `pherolang build <dir>` |
| Issue reference | `pherolang#NNN` | `pherolang#270` |
| LSP diagnostic source | `pherolang` | the editor's Problems panel |

The `Phero` type prefix is used only where a type is part of a
consumer-facing surface: `PheroBackend`, `PheroError`, `PheroExpr`. Every
other type keeps its descriptive name (`Compilation`, `ActorModel`,
`IfmlModel`) — a prefix on every type is noise, not branding.

## What the name does not touch

- **Grammar and file extensions.** `.mox`, `.ddd`, `.actor`, `.dmn`,
  `.evt`, `.uc`, `.cmmn`, `.ifml`, `.deploy` contain no product name and do
  not change. No parser changes for the rebrand at all.
- **Fixture namespacing.** `nz.example.*` is a namespace, not a brand, and
  it stays. Fixture packages that did spell the old name (`rexlang.auth`,
  `rex.conformance.*`) were renamed with everything else.
- **The wire rules.** camelCase keys, adjacent tagging, the
  `formatVersion` gate, and additive-only evolution are properties of the
  format, not of the name.
- **External toolchain names.** restingstate/xstate-ir, dsntk, and cmmn-rs
  are other people's projects and keep their names.
- **Rune.** The FINOS Rune DSL is the language and `.rosetta` is its file
  kind. Its toolchain is in-tree ([ADR 0012](adr/0012-rune-is-an-in-tree-surface.md)):
  the `phero_syntax::rune` module plus the `phero-rune-model` and
  `phero-rune-resolve` crates. **sigil** is the toolchain's historical name.
  Git history keeps it, but no new public name uses it, except the
  `import sigil` alias until that alias is removed.

## History

`rexlang` is the pre-0.3 name. `CHANGELOG.md` and the v0.2.0 tag keep it —
past releases are history, not branding — and [REBRAND.md](REBRAND.md) is
the old → new migration table for a 0.2 user. The name is now held in place
mechanically: `scripts/check-rebrand.sh` fails the gate on an old-name token
outside its allowlist, and the allowlist is exactly the historical
changelog, [REBRAND.md](REBRAND.md), this page, the architecture decision
record, the guard script itself, the deliberate legacy-artifact fixture, and
the legacy-detection module.
