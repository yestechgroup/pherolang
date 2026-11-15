# Changelog

All notable changes to Phero Lang are documented here. The project was named
rexlang before 0.3.0; the historical sections keep the old name. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/); versioning
policy in [docs/VERSIONING.md](docs/VERSIONING.md).

## [Unreleased]

### Added

- **Declared error vocabulary and a bounded executable-body contract in the
  `.ddd` surface** (DD7, issue #304, epic #297): an application or module
  now declares its domain errors with typed payloads
  (`error PaymentDeclined(DeclineReason reason, String reference);`), and
  declared, delegated, and body service operations plus declared repository
  operations carry a `throws` clause naming exactly the errors they can
  raise. `NotFound(resource, key)` and `Conflict(detail)` are the standard
  prelude — always available, never emitted in the artifact, never
  shadowable. Application-scoped errors are visible everywhere and are the
  cross-module contract; module-scoped errors are private (a cross-module
  reference is `E0375`, naming the fix); error names are application-wide
  unique (`E0374`) with unique payload fields (`E0378`), and an error
  payload may not embed a designed entity (`E0376`). `throws` is explicit
  and complete — no inference: a body raise outside it is `E0360` naming the
  declared set, a declared-but-unraised entry stays legal, and a
  delegation's `throws` must be a superset of its target's (`E0377`). The
  statement body is now stated as **orchestration-only**: a body may call a
  domain `op` on a loaded local (`let submitted = run.submit();`), assign a
  feature path, and raise declared errors, while loops, local reassignment,
  compound assignment, collection mutation, `try`/`catch`, cross-aggregate
  loads, and bare expression statements are named parse/compile errors. The
  artifact gains sparse `errors` at the model root and on each module plus
  resolved `throws` references (`{name, scope, module?}`), `formatVersion`
  stays 2, and `phero-backend-ddd` / `pherolang artifact check` enforce the
  vocabulary, the resolved references, the body-raise subset, and the
  delegation-superset contract from the artifact alone. A declared error
  node and its `throws` edge join the semantic graph, editor navigation,
  highlighting, and the LSP symbol table. `docs/DDD.md` gains the error
  vocabulary section, rules 21–23, the rewritten rule 19, and the
  orchestration-only posture with its explicit "not yet" list;
  `docs/BACKENDS.md`, `docs/GRAPH.md`, and `docs/EXPRESSIONS.md` carry the
  wire, graph, and statement-layer contracts. `examples/support` migrates:
  `updateTicket` raises the new application error `TicketAlreadyResolved`.
- **Resolved identities and provenance in the `.ddd` artifact** (DD1, issue
  #298, epic #297): `DddModel` gains `pheroVersion` and a resolved companion
  for every reference a consumer acts on — `Design.typeRef`,
  `Delegation.targetRef`, `Service.dependencyRefs`/`decisionDependencyRefs`,
  `ServiceOperation.capabilityRefs`/`emittedRefs`/`decisionRefs`,
  `SearchDef.entityRef`, and search-member `typeRef`s. `formatVersion` stays
  **2**; the additions are optional, so body-less artifacts stay
  byte-identical apart from the provenance line. `pherolang artifact check`
  now validates the DDD contract from the artifact alone (resolved
  companions, aligned parallel lists, stereotype/flag legality, search
  references, provenance). `docs/DDD.md` gains the resolution table, the
  consumer contract, and the Sculptor "deliberately different / absent"
  mapping column.
- **Strict value semantics in the `.ddd` surface** (DD2, issue #299, epic
  #297): a `value` design now means all four properties together — no
  identity, immutable, deep-immutable (nested values and collections), and
  structural equality over the stored features with derived features
  excluded — with **no `!immutable` opt-out**. New compile rules, checked
  against the application's designs (one class may be a value in one
  application and an entity in another): R-V1, a value's target class may
  not declare an `id`-modified feature; R-V2, a value may not own a
  `contains` whose target is designed as an `entity`; R-V3, a value may not
  declare a `refers` to a designed `entity` (an error, not a warning —
  refer by id instead: `refers String personId`); and `abstract value` is
  refused. Value designs carry the fixed artifact block `semantics` —
  `{"identity":"none","immutable":true,"deepImmutable":true,"equality":"structural"}`
  — and the field is sparse, so entity/dto designs and value-less models stay
  byte-identical and `formatVersion` stays 2; `pherolang artifact check`
  requires the block on every value design and refuses it elsewhere.
  `examples/support` migrates: `RefundDecision` keeps
  `readonly String decisionId` as stored state but no longer marks it `id`.
  The Rust backend does not consume `.ddd`, so the in-tree proof is the
  compile-rule matrix plus the artifact contract, and the consumer (codegraph)
  contract — constructor-only type, private/immutable fields, no `Default`,
  immutable collection views, structural equality/hash in declaration
  order, derived features computed, surrogate id never public or
  equality-bearing — is written into `docs/DDD.md`.
- **DTO boundaries in the `.ddd` surface** (DD3, issue #300, epic #297): a
  `dto` design is now a transport shape in three forms — **mirror**
  (`dto User`, every stored feature of the class), **projection**
  (`dto BookSummary from Book { title; isbn; rating = playLength / 60; }`,
  selecting a feature, renaming a plain path, or deriving with the source
  class as `self`), and **standalone** (`dto CreateBookRequest { String
  title; }`, no domain class). Members are ordered and their names are the
  wire names; duplicate output names are errors, and a member may not
  embed or select a designed entity. Direction is derived: any derived
  member makes the DTO `output`, and an output DTO is refused in a
  parameter slot (a return type may be output). Boundary names are
  application-wide unique and may not shadow a domain type (mirror DTOs
  are exempt), and a DTO carries no `op` (now an **error**, not the
  pre-DD3 warning), repository, machine, or entity-only flag. The artifact
  adds `dtoShape` (`name`, optional `source`, `direction`, ordered
  `members`) and the `DddTypeRef` signature slot — domain refs stay the
  byte-identical `TypeRef` object, DTO refs are `{"dto":"Name"}`;
  `Design.class` is optional for standalone DTOs, `formatVersion` stays 2,
  and domain-only signatures stay byte-identical. A search `document`
  entry and a DTO projection share one projection AST (`document DtoName;`
  convergence is a follow-up, not shipped). `phero-backend-ddd` /
  `pherolang artifact check` enforce the DTO contract from the artifact
  alone (a `dtoShape` on every dto design and nowhere else, well-formed
  ordered members, resolvable DTO references, no output DTO as a
  parameter), and `docs/DDD.md` gains the section, the rules, the wire
  shapes, the consumer contract, and the Sculptor mapping update.

## [0.3.0] — Phero Lang (2026-11-14)

"Phero Lang": the project is renamed, and the wire tells the truth about it.
The language, grammar, and file extensions are unchanged; this release is the
rename plus its one deliberate wire-contract break. Pre-0.3 artifacts are
**rejected**, not silently misread.

### Added

- **Legacy-artifact detection in `pherolang artifact check`** (epic #270,
  R2): a 0.2-era artifact — one carrying the old provenance key or the old
  instance discriminator — is reported before any shape sniffing with the
  exact message `0.2-era rexlang artifact — regenerate with pherolang 0.3`,
  never a generic `formatVersion` mismatch or an unknown root `$type`.
  A committed fixture (`tests/conformance/artifacts/legacy-0.2.rex.json`)
  pins the message, and a matrix test covers every classified artifact kind.

- **`phero_ir::graph::GraphModel`** (issue #239, the semantic-graph slice
  1/8): the unified, versioned semantic-graph wire artifact — applications,
  nodes (`class`, `serviceOperation`, `decision`, `event`, `view`,
  `planItem`, …), and edges (`imports`, `permits`, `binds`, `emits`,
  `executes`, `deploys`, …). Node and edge kinds are an **open,
  string-typed vocabulary** so the artifact can grow additively before the
  v0.7 freeze; an empty graph serializes as exactly `{"formatVersion":1}`,
  unknown kinds round-trip, and there is no `pheroVersion` stamp (goldens
  never churn on release bumps). Normative tables in `docs/GRAPH.md`.
- **`phero_driver::graph` + `phero_driver::applications`** (issue #241, slice
  2/8): the pure post-compile builder over all nine surfaces — `.mox`,
  `.actor`, `.ddd` (state machines included), `.evt`, `.dmn`, `.ifml`,
  `.deploy`, `.uc`, `.cmmn` — with cross-surface resolution mirroring the
  compiler (union-wide capability/actor pools, the ASCII-case-insensitive
  `action → operation` rule, ambiguity flagged, unresolved references emit
  **no** edge). The import-connected-application union-find moved out of
  the CLI into the driver and `check`'s integrity pass now consumes it
  (its diagnostics are byte-identical).
- **`GraphIndex` + `pherolang graph query`** (issue #242, slice 4/8): the
  artifact-only query surface — `node`, `list`, `affected-by`,
  `dependencies` — each in one versioned envelope, running from a committed
  `graph.json` with no sources and no recompile. `pherolang artifact check`
  classifies `GraphModel` artifacts (before the deploy rule) and names
  violations precisely.
- **Spanned `SurfaceIndex` navigation indexes for all seven artifact
  surfaces** (issues #245/#244, slices 6/8 and 7/8): `.actor`, `.ddd`,
  `.evt`, `.deploy`, `.uc`, `.cmmn`, `.dmn` each publish definitions and
  references with byte spans, and every definition's `(kind, name)` **is**
  the graph builder's node key — pinned by the anti-drift equality test
  over all nine surfaces. The `.mox` index moved verbatim into
  `navigation/mox.rs` (behavior frozen).
- **`pherolang graph build`** (issue #240, the semantic-graph slice 3/8): compile a
  tree of model files into **one** deterministic, versioned
  `phero_ir::graph::GraphModel` — the import-connected applications, the full
  node/edge vocabulary, and the semantic edges between every surface —
  printed to stdout or `-o`. Directories enter project mode like
  `check`/`build`; display paths are root-relative POSIX (base = the input
  roots' common ancestor), so the artifact never carries a machine path.
  Emission is best-effort over the compiling subset (the `build` precedent).
  The pipeline's `SurfaceOutcome` now carries each file's typed artifact and
  resolved imports (and `IfmlInputs` its imports), so the graph re-reads and
  re-compiles nothing; `check`/`ir`/`build` output is unchanged. Pinned by
  `examples/artifacts/examples.graph.json` (the whole `examples/` tree, base
  `examples/`) and its `crates/phero-cli/tests/cli/graph.rs` suite.
- **`pherolang graph query … separation-of-duty|monotonicity|dead-capabilities|reachability`**
  (issue #246, the semantic-graph slice 5/8): the four cross-policy analytics
  run on the artifact **alone** and answer in the same versioned envelope as
  the structural queries — per application (unrelated applications never
  cross-check), conservative may-analysis (`when` guards are satisfiable), and
  an explicit `unknown` for opaque `cedar` grants, which never become a
  definite yes or no. `--fail-on-findings` exits `1` only on a definite
  finding (a violated pair, a monotonicity gap, a `dead` capability, an
  unreachable target), so CI can gate without making a policy finding a
  language error. The compiler's `lineage`/`effective_permits` semantics are
  rederived from the artifact (`crates/phero-driver/src/graph/analytics.rs`).
  Pinned by the self-contained `tests/conformance/graph/` fixture — `app.*`
  across all nine surfaces plus the unrelated `other.*` application (its
  `cedar` policy is the `unknown` case) — with `app.graph.json` and one
  `queries/app.*` golden per analytic.
- **Cross-surface LSP navigation over the graph index** (issue #243, the
  semantic-graph slice 8/8): every surface now answers
  definition/references/hover/document-symbols from one semantic index. The
  server builds a **graph snapshot** from the open document, its
  open-document siblings, and the disk siblings of its directory (open
  buffers win over disk; a non-compiling sibling contributes nothing),
  resolving names with the graph's `GraphIndex` and landing on spans from
  each file's `SurfaceIndex` — `.actor`→`.mox` class, `.ddd` capability →
  `.actor`, `invokes` → `.dmn`, `emits` → `.evt`, `.evt` consumer →
  `.deploy` component, `.ifml` `requires:`/`action(...)` → capability /
  bound design operation, deploy bindings → module, `.uc`/.cmmn` →
  designed operation, `.dmn` emits → event, and inline `.mox` actors. Rename
  is declaration-owned and document-scoped: a reference to another file's
  declaration is refused with a message naming the owner. `NavigationIndex`
  (`.mox`) and `IfmlIndex` (`.ifml`) gained `SurfaceIndex` projections, and
  the anti-drift equality test now covers all nine surfaces. No artifact,
  wire-format, or golden change.
- **`phero-evidence`**, the evidence-spine crate (issue #203): the
  machine-readable proof-obligation register's data model, canonical parse,
  semantic validation, and markdown rendering. Pure — serde/serde_json only,
  no filesystem or compiler access.
- **`docs/verification/register.json`** (issue #203): the register's source
  of truth — 17 obligation rows with closed statuses
  (`proved`/`tested`/`argued`/`assumed`), resolvable evidence pointers,
  triggers and review dates on every open or assumed row, the four #173
  Tier-3 holds, and the bidirectional `#[trusted]` inventory. The tables in
  `docs/VERIFICATION.md` are now **generated** from it.
- **`docs/SEMANTICS.md`** (issue #205): the claim ladder (C1–C5), the
  `WF`/`WT`/`Secure`/`Consistent` judgment forms, theorem **S** with its
  named assumptions, the explicit non-goals with reversal triggers, and the
  complete #189 section mapping. Supersedes #189.
- **`pherolang verify`** (issue #197) and the versioned **verification
  receipt** (`formatVersion` 1): one canonical, deterministic JSON document
  per model set — the source closure hashed, every artifact hashed (pinned
  against its committed golden), the generator version, and the register's
  rows exactly as the register states them. `--check` validates a receipt
  standalone (issue #206: no register, models, toolchain, or network);
  `--check-fresh` regenerates and byte-compares (issue #207). Receipts
  **never** say "verified": the four-word register vocabulary is closed and
  `verified` is rejected by name; `mutantKilled: null` +
  `limits: ["mutants-unrecorded"]` records the #208 gap out loud.
- **`examples/artifacts/support.receipt.json`**: the flagship receipt for
  the `support` example — nine surfaces, the pattern-library closure, and
  the external decision table.
- **`docs/RECEIPTS.md`**: what a receipt is and is not, its schema and
  closed vocabularies, the commands, and the staleness classes.
- **Editor diagnostics for every artifact surface** (issue #255):
  `.evt`, `.deploy`, `.uc`, and `.cmmn` documents now publish their
  compiler diagnostics (spanned, with the `pherolang` source) instead of
  silently accepting errors. They reuse the already memoized graph-snapshot
  compile — no second compile per keystroke — and a clean document publishes
  an empty set that clears stale diagnostics; a document with no directory
  snapshot (a non-file URI) publishes nothing.

### Changed

- **The product is now Phero Lang** (epic #270): the binary is `pherolang`,
  the crates are `phero-*` (`phero-ir` → `phero-ir`, `phero-driver` →
  `phero-driver`, …), environment variables are `PHERO_*`, the repository is
  `yestechgroup/pherolang`, generated Rust links against `phero_runtime::`,
  and the LSP identity (diagnostic `source`, server/client name) is
  `pherolang`. The full old → new table is in
  [docs/REBRAND.md](docs/REBRAND.md); the spelling contract is
  [docs/NAMING.md](docs/NAMING.md).
- **Wire provenance and discriminator renamed**: the artifact provenance key
  is `pheroVersion` (was `pheroVersion`), the canonical-instance discriminator
  is `phero.instance` (was `phero.instance`), and the JSON-Schema `$id` URN is
  `urn:phero:model:<pkg>:<profile>`. The artifact file convention is
  `.phero.json`.
- **`formatVersion` moves 1 → 2** for every artifact whose shape changed:
  `FORMAT_VERSION`, `ACTOR_MODEL_FORMAT_VERSION`,
  `DDD_MODEL_FORMAT_VERSION`, `IFML_MODEL_FORMAT_VERSION`,
  `IFML_MANIFEST_FORMAT_VERSION`, `EVENT_MODEL_FORMAT_VERSION`,
  `DEPLOY_MODEL_FORMAT_VERSION`, `USE_CASE_MODEL_FORMAT_VERSION`,
  `CMMN_MODEL_FORMAT_VERSION`, `DMN_MODEL_FORMAT_VERSION`, and
  `INSTANCE_FORMAT_VERSION`. This is the single allowed pre-1.0 wire break.
  `GRAPH_MODEL_FORMAT_VERSION`, the `graph query` envelope version, and the
  receipt/proof-register format versions **stay at 1** — they carry no
  provenance key and their shapes did not change.
- **Regenerate every artifact.** 0.3 does not read 0.2 artifacts:

  ```sh
  PHERO_UPDATE_FIXTURES=1 cargo test --workspace
  ```

  Policy and the exact failure mode are in
  [docs/VERSIONING.md](docs/VERSIONING.md) and
  [docs/BACKENDS.md](docs/BACKENDS.md).

- **Compile-time cross-surface joins are scoped to the application** (issue
  #249): the `.ddd` capability pool, the `.ddd` decision bucket, and the
  `.actor` grant-coverage carrier set pool the checked file's
  **import-connected application** (`phero_driver::applications`) — the same
  relation the union integrity pass already used — instead of every sibling
  in the directory. A design in one application is no longer validated
  against a policy in another; an application with no counterpart surface
  keeps the joins silent (the documented opt-in behavior). `check examples/`
  no longer rejects `orders.ddd`'s `PlaceOrder` against `support.actor`, and
  `graph examples/` exits `0` with the full tree.
- **`examples/` is split into per-application directories** (issue #258):
  `examples/{support,shop,library,org,iot,shapes,pricing,lending}/`, each a
  complete vertical slice; `artifacts/`, `instances/`, and the README stay
  at the root. The support and shop designs now emit the events their
  `.evt` contracts declare and carry the service operations their `.ifml`
  actions bind to, so every application is wired end to end and
  `check examples/` exits `0`. Graph display paths (and the graph, query,
  and receipt goldens) carry the application directory. The union pass's
  phase-2 `.ifml` re-check now keeps the decision and actor dimensions
  phase 1 uses, so a `data:` binding to a decision output is no longer
  re-reported as a disagreement.
- **`pherolang graph` is now a pure namespace with two verbs** (issue #257):
  `pherolang graph build <paths>... [-o <file>]` compiles sources into one
  artifact, and `pherolang graph query <graph.json|-> …` answers from a
  committed artifact. The previous shape overloaded a positional `PATHS`
  list with an optional `query` subcommand, and clap matches the subcommand
  first, so a directory literally named `query` could not be passed
  directly. **Breaking for the unreleased command only**: `pherolang graph
  <paths>` becomes `pherolang graph build <paths>`, and bare `pherolang graph`
  is now a clap usage error. The artifact bytes and the query envelopes are
  unchanged — no golden moves.
- The `#[trusted]` register gate (issue #89) is now the full **register
  drift gate** (#203): a dead evidence pointer, a status outside the closed
  vocabulary (`"verified"` is rejected by name), an `argued`/`assumed`/
  mirrored row without a trigger and review date, or a `#[trusted]` that is
  not bidirectionally registered all fail `cargo test`.
- `scripts/ci-local.sh` gains the `receipt` step (CI 5/6) and
  `.github/workflows/ci.yml` invokes it: a stale flagship receipt — a source,
  artifact, or register change without regeneration — is CI-red with the
  exact command to run (issue #207).

- **`.ifml` re-platformed from Pest to the workspace's one parser stack**
  (issue #212, sub-issues #264–#268): a logos lexer, a chumsky parser
  producing a fully spanned syntax tree (`phero-ifml::ast`), and a single
  AST → `(IfmlModel, IfmlIndex)` lowering. The wire artifacts and formatter
  are byte-identical; no fixture was regenerated.
- The lenient path (`compile_ifml_lenient`, backing the language server) now
  returns a **partial model and index** on broken input, with one
  byte-span-accurate `E_PARSE` per error instead of a single `(0,0)`
  diagnostic. A bracket-depth guard bounds untrusted editor input.
- The normative grammar moved from `grammar/ifml.pest` to the EBNF in
  `crates/phero-ifml/src/parser/mod.rs`; grammar-doc tests keep the EBNF and
  the contextual word tables in sync. `docs/IFML.md`, `docs/LANGUAGE.md`, and
  `AGENTS.md` now point there.

### Removed

- The pre-0.3 names are gone from the working tree: no `phero-*` crate, no
  `pherolang` binary, no `PHERO_*` environment variable, and no old wire marker
  outside the history, the migration tables, and the legacy-detection
  module. `scripts/check-rebrand.sh` fails CI on a revived spelling.

**Scope note.** No crate was ever published to crates.io and no release
asset ever shipped, so the blast radius of the rename and the wire break is
internal: this repository, its pinned downstream consumers, and its
committed artifacts. The regeneration command above is the whole migration.

- `pest` and `pest_derive` leave the dependency graph. The Pest-generated
  `phero_ifml::Rule` and `phero_ifml::IfmlParser` exports are gone; no in-tree
  consumer used them, and `IfmlParseError` keeps its public shape.

## [0.2.0] — 2026-11-14

"Foundation & trust": the security/dependency foundation, the release
ladder's base. No language, grammar, wire-format, or artifact changes —
the nine surfaces are frozen as a set; the only golden diff this release
is `rexVersion` provenance moving `0.1.0` → `0.2.0`.

### Security

- **`cargo audit` is fully clean** and enforced in CI (see *Added*).
  Advisories fixed this release:
  - RUSTSEC-2026-0308 — salsa 0.23.0 → 0.28.5 (use-after-free in interned
    values/cached results; salsa backs the LSP, which processes untrusted
    editor input). Migration: `returns(clone)` on the tracked queries and
    the `SourceFile` input getters for 0.28's references-by-default
    (#213). Zero behavioral change — navigation, compile, and the full
    in-process LSP suites pass byte-for-byte.
  - RUSTSEC-2026-0386 — tokio 1.53.1 → 1.54.1 (AsyncFd I/O safety;
    rexlang does not use AsyncFd) (#211).
  - RUSTSEC-2026-0486 — rustix 1.1.4 → 1.1.6 via the proptest/tempfile
    dev graph (xattr zero-length buffer) (#209; advisory postdated the
    #173 survey).
- `cargo deny` (advisories, licenses, sources) gates every PR with a
  permissive license allow-list and scoped LGPL exceptions for the
  Creusot proof stdlib (`deny.toml`) (#215).

### Changed

- **The language server runs on the maintained fork**: `tower-lsp` 0.20
  (dormant since 2023) → `tower-lsp-server` 0.23 stable — new `ls_types`
  types (the `fluent_uri`-backed `Uri` replacing `url::Url`), native
  async-in-trait, `offset_encoding` left absent so positions stay UTF-16.
  Dispatch contract unchanged; LSP suite 91/91 plus a real-stdio smoke
  test (#214).
- Dev/test dependency refresh (#209, #216): pest 2.9.6 (stricter grammar
  validation, Unicode 18, stack guard), serde_json 1.0.154, clap 4.6.7,
  sha2 0.11.1, proptest 1.12, ureq 3.5.2, thiserror 2.0.21;
  jsonschema 0.55 → 0.61 (wire-schema conformance suites green),
  ariadne 0.5 → 0.6 (**zero rendering churn** — goldens byte-identical),
  cedar-policy 4.12 → 4.14.1 (dev-only; the backend stays unlinked).
  `creusot-std` deliberately held at 0.14 pending the matching Creusot
  toolchain (#220).
- The unused `askama` workspace entry is removed; `cargo-machete` is the
  recommended recurring check for this class (#209, #220).

### Added

- `rexlang fmt` and `fmt --check` accept directories with `check`'s exact
  input semantics — recursive scan over every supported surface, sorted,
  formatter-golden `X.fmt.<ext>` outputs skipped; the natural CI pairing
  `check model/ && fmt --check model/` now works (#217, closes #136).
- CI parity: `.github/workflows/ci.yml` invokes `scripts/ci-local.sh` for
  every gate — the script is the single source of truth for the gate
  sequence and the fixture-glob list, so local and remote can never
  drift, and a new fixture directory is a one-file edit (#218, closes
  #121).
- Two fast CI supply-chain jobs: `cargo audit` and `cargo deny check`;
  `scripts/ci-local.sh audit` replicates both locally (#215).
- `rexlang completions <shell>` (bash/zsh/fish/powershell/elvish) and
  `rexlang man` — rendered from the CLI's own clap definition, so they
  cannot drift from it (#219).
- Directory scans (check/build/fmt/ir) run on `walkdir` with
  `follow_links(false)`; deterministic order and symlink-loop immunity
  pinned by tests (#219).

### Fixed

- `rexlang fmt --check <dir>` no longer errors with "Is a directory"
  (closes #136, above).

### Deferred (tracked in #220)

- `line-index`/`text-size` behind the LSP position mapping;
  `assert_cmd`/`predicates`/`tempfile` for the CLI tests; the
  chumsky 0.13 / logos 0.16 dedup spikes; check-time `regex-syntax`
  validation of `.ifml` regex literals; `jiff`/`heck` trials;
  `creusot-std` 0.15 (toolchain-coupled).

### Earlier history

rexlang 0.1.0 was a version stamp, not a tagged release — no tags or
releases exist before 0.2.0. For everything prior to this entry, read the
commit log (`git log v0.2.0` ancestry); the per-surface references under
`docs/` describe each surface as shipped.

[0.2.0]: https://github.com/yestechgroup/rexlang/releases/tag/v0.2.0
[0.3.0]: https://github.com/yestechgroup/pherolang/releases/tag/v0.3.0
