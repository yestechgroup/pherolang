# Writing a backend for Phero Lang

Phero Lang is a DSL for **modelling**. Backends are consumers of the Core IR —
not features of the compiler. Candidate backends (ACTUS cashflow mapping,
Accord/TemplateMark document generation, additional codegen targets) live as
**separate, out-of-tree crates** that consume the IR; nothing registers with
the compiler. This guide is the contract for writing one. The wire-format
rules summarized here are **normative in the [`phero-ir` crate
docs](https://docs.rs/phero-ir)** (also rendered by `cargo doc -p phero-ir`).

## The boundary

- A backend consumes **only** `phero_ir::Model` (plus `phero_ir::ActorModel` if
  it deals with the authorization dimension, or the standalone artifact its
  surface emits — `phero_ir::cmmn::CmmnModel` for the case-management
  backend) or the serialized artifact. Never the AST: parsing, name
  resolution, validation, and lowering into the IR happen upstream in
  `phero-driver`.
- Backends **register nothing and emit anything** — Rust code, JSON Schema,
  Cedar policies, documents, tables.
- Two integration modes:
  1. **Library**: depend on the `phero-ir` crate, deserialize with
     `Model::from_json` / `ActorModel::from_json` (or build the IR
     programmatically for tests).
  2. **Artifact**: run `pherolang ir <files> -o out.phero.json` and read the
     JSON. A `.mox` compile emits the `Model` artifact; a `.actor` compile
     emits the standalone `ActorModel` artifact.
- The in-tree backends (`phero-backend-rust`, `phero-backend-jsonschema`,
  `phero-backend-cedar`, `phero-backend-cmmn` — the last lowering
  `CmmnModel` + domain model to the cmmn-rs engine's `CaseDefinition`
  JSON, and `phero-backend-dmn` — lowering `DmnModel` + the markdown table
  contents into the dsntk engine model it evaluates, recording
  decision-table provenance) are reference implementations of exactly
  this discipline; reading them is the fastest way to learn the shapes.

## The serialized-artifact contract (summary)

Root shapes, as actually serialized:

| Artifact | Root |
| --- | --- |
| Domain IR (`Model`) | `{"formatVersion": N, "pheroVersion"?: "…", "packages": […]}` |
| Actor policy (`ActorModel`) | `{"formatVersion": N, "blocks"?: […]}` — block-less serializes as exactly `{"formatVersion": N}` |
| DDD design (`ddd::DddModel`) | `{"formatVersion": N, "applications"?: […], "modules"?: […]}` (both omitted when empty) |
| Event contract (`events::EventModel`) | `{"formatVersion": N, "events"?: […], "channels"?: […], "subscriptions"?: […]}` (all omitted when empty) |
| Deployment (`deploy::DeployModel`) | `{"formatVersion": N, "applications"?: […], "profiles"?: […], "deployments"?: […]}` (all omitted when empty) |
| Use case (`usecases::UseCaseModel`) | `{"formatVersion": N, "useCases"?: […]}` (omitted when empty) |
| Case management (`cmmn::CmmnModel`) | `{"formatVersion": N, "cases"?: […]}` (omitted when empty) |
| Semantic graph (`graph::GraphModel`) | `{"formatVersion": N, "applications"?: […], "nodes"?: […], "edges"?: […]}` (all omitted when empty; built by `pherolang graph build`) |
| Canonical instance | `{"$type": "phero.instance", "formatVersion": N, "model": "…", "objects": […]}` |

Every standalone root carries its own `*_FORMAT_VERSION` marker. Since the
Phero Lang rename those markers are **`2`** (the provenance key became
`pheroVersion` and the instance discriminator became `phero.instance`), with
two exceptions that carry no provenance marker and deliberately stayed at
`1`: the semantic graph (`GRAPH_MODEL_FORMAT_VERSION`) and the `graph query`
envelope. `phero-evidence`'s receipt and proof-register format versions also
stay at `1` — their shapes did not change. `pherolang artifact check`
classifies non-empty roots by their disjoint section keys and falls back to
the actor reading for the shared minimal `{"formatVersion": N}` bytes (now
`N = 2` for the bumped kinds, which is why a bare `{"formatVersion": 1}`
graph form is a version mismatch rather than an actor artifact).

- **Casing.** All field names and bare enum tags are `camelCase`
  (`formatVersion`, `isReadOnly`, `"crossReference"`). Note that `$type` /
  `$id` / `$ref` belong to the *instance* format; IR artifacts tag their
  enums **adjacently** as `{"type": "<variant>", "value": <payload>}`.
- **Version gate.** Readers accept exactly their own
  `FORMAT_VERSION` (domain artifacts) or `ACTOR_MODEL_FORMAT_VERSION`
  (actor artifacts) and reject anything else with an error naming the
  found and expected versions (`IrError::UnsupportedFormatVersion`).
  Canonical instances are versioned independently
  (`phero_runtime::INSTANCE_FORMAT_VERSION`).
- **Additive evolution.** Every field added after v1 is
  `#[serde(default)]`, so older artifacts keep loading; fields empty in the
  common case also `skip_serializing_if`, so artifacts for models that do
  not use a feature (no bodies, no vocabularies, no actors, no functions, no
  descriptions) stay **byte-identical** to older output. Unknown fields are
  ignored on read, never denied.
- **Operation bodies.** A `rust` body is embedded **verbatim** as the
  generated method's body (the signature convention lives in
  `phero_backend_rust::codegen`); an `expr` body is parsed, type-checked
  against the enclosing class and the operation's parameters, and lowered
  at generate time — a parse/type/lower failure is a generate-time error
  naming the operation (`operation '<op>' expr body …`). An operation with
  neither body emits **no** method: it stays a hand-written hook. The
  expression language is `docs/EXPRESSIONS.md`.
- **Functions and unknown body targets.** `Package.functions` carries
  sigil-lowered Rune functions as signature-only `Operation`s whose bodies
  are the ends-trimmed verbatim `"rosetta"` target plus — for functions
  whose statements the driver translated and checked — the canonical
  `"rune"` statement IR (wire contract rule 15, RUNE-BODIES). Backends
  skip body targets they do not own — the `rosetta` text is never emitted
  as code. The Rust backend emits each lowered function as a free function
  in the flattened module: a `rust` body verbatim; a checked `rune` body
  as straight-line Rust (guards → `assert!(expr, "guard '<name>' failed")`
  with the phase label for unnamed guards, aliases → sequential `let`s,
  `set`/`add` steps → assignment/`push` into the output local, which is
  introduced at its declared default shape — `Vec::new()`, `None`, the
  Tier-1 scalar default, or the arena's default-initialized object for a
  class-typed output; a `0..1` output therefore returns `Some`/`None`
  exactly as the checked expressions produce them, never an invented
  wrap); anything else — a deferred or statement-less function — a
  `todo!()` stub named after the function. A function's
  `return_multiplicity` shapes the emitted return type the same way a
  parameter's multiplicity shapes its parameters: `0..*` → `Vec<T>`,
  `0..1` → `Option<T>`, the single-valued convention → `T` (phero-expr types
  calls to the function identically). Bare `expr`-body calls (issue #63)
  lower to exactly these emitted functions, passing `res: &Resource` when
  the callee's body reaches the arena (a class-typed output, a `->` path
  step, or navigation on a class-typed value) — such functions take the
  arena as their first parameter. A `"rune"` payload that decodes but does
  not parse, type-check, or lower is a generate-time error naming the
  function and the slot (the `expr`-body bail convention): the driver
  never attaches an unchecked body, so a present-but-invalid target is
  artifact corruption, not deferral.
- **Stable feature ids.** `Feature::id` equals its 0-based declaration index
  at publication time and is frozen for wire formats; backends key on it.
- **Golden byte-identity.** The goldens under `tests/conformance/` are the
  spec: an emitter change that shifts a byte is a contract change. When
  developing *in* this workspace, `PHERO_UPDATE_FIXTURES=1` regenerates them
  and the diff is reviewed as a spec change.

## The `preserve_order` gotcha

`serde_json::Map` is a `BTreeMap` **unless any crate in the build graph
enables serde_json's `preserve_order` feature**, which flips it to insertion
order. `cedar-policy` (a dev-dependency of `phero-backend-cedar`) enables it,
so a workspace that pulls in Cedar codegen inherits insertion order
everywhere. Emitters that rely on implicit map ordering will emit keys in
whatever order that build happened to produce. **Canonicalize key order
yourself before serializing** — `canonical_key_order` in
`crates/phero-backend-common` recursively re-inserts
keys in a pinned order; the JSON Schema and Cedar backends both run it
over their emitted documents.

## Semver and `formatVersion` policy

The workspace is at **0.3.0 — pre-1.0**, so the usual "breaking = major"
rule does not apply yet:

- **Until 1.0, a breaking wire change bumps the MINOR crate version**
  (0.2.0, 0.3.0, …) *and* the affected `formatVersion` marker. Purely
  additive changes keep the format version and ride a normal minor/patch.
- Readers **accept their own version and reject any higher version with an
  error naming the version** — they never guess. Lower versions are equally
  rejected: there is no migration path inside one crate version.
- **Compatibility-matrix expectation.** Every phero-ir release documents the
  format versions it reads and writes, and out-of-tree backends declare the
  `formatVersion`(s) they accept, so consumers can pin both sides. The
  matrix so far:

| phero-ir | reads (domain) | writes (domain) | reads/writes (actor) | reads/writes (instance) |
| --- | --- | --- | --- | --- |
| 0.1.0 | 1 | 1 | 1 | 1 |
| 0.3.0 | 2 | 2 | 2 | 2 |

- `pheroVersion` inside an artifact is provenance only — consumers that must
  not care can ignore it.
- **0.3 does not read 0.2 artifacts.** There is no dual-accept window: the
  rename moved the provenance key and the instance discriminator, so a
  document carrying either pre-rename marker is rejected outright (see the
  migration note below).

### Consumer migration to 0.3

Bump the git rev **and** the package names in `[dependencies]` to their
`phero-*` spellings (the full old → new table is in
[REBRAND.md](REBRAND.md)), then regenerate every committed artifact — expect
`pheroVersion` in the provenance slot and `"formatVersion": 2` in every
file. A 0.2 artifact does not fail silently: `pherolang artifact check`
names it with the 0.2-era migration message (quoted verbatim in
[REBRAND.md](REBRAND.md)) rather than reporting a generic `formatVersion`
mismatch or an unknown root `$type`. The one-line regeneration command is
there too.

## The test kit: `pherolang artifact check`

Third-party backends can validate any serialized artifact against the wire
format without the originating model:

```
pherolang artifact check out.phero.json support.actors.phero.json instance.json
```

Per file it checks the invariants below, prints `OK <path>`, and exits `1`
naming the file and violation when anything fails:

- a 0.2-era artifact (an old provenance key or the old instance
  discriminator) is named with the migration message **before** any shape is
  considered — it is never mistaken for a current artifact;
- the root shape matches a known artifact kind (IR artifact, actor
  artifact, DDD design, event contract, deployment, use-case, or
  case-management artifact, or `$type: "phero.instance"` canonical
  instance);
- `formatVersion` is present, an unsigned integer, and supported — a higher
  version is rejected with an error naming it;
- structural keys are camelCase (a snake_case spelling of a known key is
  reported with the correct spelling; model-defined keys — feature names,
  target names — are exempt);
- instance sanity: `$id`s are well-formed (`<class>/<ordinal>`) and unique,
  every object carries `$type`, `$ref` objects carry nothing else, and
  every `$ref` resolves within the document.

The checker is dogfooded in CI over every golden artifact under
`tests/conformance/artifacts/`, so it stays honest as the format evolves.

## Backend-author checklist

1. **Consume** only `phero_ir::Model` / `ActorModel` (or the artifact) —
   never the AST; declare which `formatVersion`s you accept.
2. **Emit** deterministically; canonicalize `serde_json::Map` key order
   yourself (`preserve_order` gotcha above).
3. **Pin golden byte-identity**: keep golden outputs under version control
   and byte-compare them in your tests, the way this workspace does.
4. **Run `pherolang artifact check`** over your consumed IR artifacts and any
   instance documents you produce.
5. **Pin the `phero-ir` version** and record the compatibility matrix row for
   the format versions you read and write.
