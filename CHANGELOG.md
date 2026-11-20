# Changelog

All notable changes to Phero Lang are documented here. The project was named
rexlang before 0.3.0; the historical sections keep the old name. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/); versioning
policy in [docs/VERSIONING.md](docs/VERSIONING.md).

## [Unreleased]

### Added

- **Intent traceability is walkable in the semantic graph** (issue
  [#363](https://github.com/yestechgroup/pherolang/issues/363)): `.intent`
  files contribute `intent` nodes (name = the stable id) and two edge
  families — `expresses` for each resolved `element` link and `verifiedBy`
  for each resolved `check` link — resolved through the graph's own node
  table (`Symbols::by_suffix`, unique match only, so an ambiguous name emits
  no edge). `affected-by` on an expressing class, capability, event, view,
  command, operation, or decision now reaches the requirements that state
  it, and `.intent` gained a span table (`intents_index`) that joins the
  nine existing surfaces in the anti-drift gate.

- **Intent specifications are a source surface** (issue
  [#196](https://github.com/yestechgroup/pherolang/issues/196), the
  Proof-Carrying Models epic's Phase P1): `.intent` files state requirements
  as model data — `intent LOAN-001 "<statement>"` with stable ids, links to
  the model elements that express them (`element class:/capability:/event:/
  view:/command:/op:`), and the model-native checks that verify them
  (`check invariant:/decision:`). The tenth row of the one surface table
  compiles to the versioned `phero_ir::intents::IntentModel` artifact
  (`formatVersion` 1); the project-level **coverage pass** — the union
  pass's new `X0009`–`X0014` codes, rendered spanned from a compile-time
  span table that never serializes — warns on every intent nothing
  expresses and nothing verifies (honesty over reach), errors on broken
  links with the did-you-mean candidate, and errors on cross-file duplicate
  ids. The single-surface checks are the new `driver.intent` band
  (`E0450`–`E0454`). The flagship `support.intent` is fully covered and
  pinned clean; the orphaned-intent and broken-link fixtures pin the exact
  texts. Decision record:
  [ADR 0010](docs/adr/0010-intent-specifications-are-a-declaration-surface.md);
  contract: [docs/INTENTS.md](docs/INTENTS.md).

### Removed

- **The `.ddd` `extend` child-operation alias is gone from the grammar**
  (DF4, issue [#327](https://github.com/yestechgroup/pherolang/issues/327),
  tracked by
  [#359](https://github.com/yestechgroup/pherolang/issues/359)): the 0.4
  deprecation has aged out on schedule. A source writing
  `extend <Class> { ... }` now fails to parse — the warning code `E0387` is
  retired with it and the number is reserved against reuse — while
  `pherolang fmt` still rewrites the spelling to
  `children <Class> { ... }`, the mechanical migration path. The
  child-operation diagnostics (`E0355`–`E0358`) now name the canonical
  `children` spelling.

## [0.4.0] — Phero Lang (2026-11-17)

Planned as **v0.4.0** — the first release whose binary ships
`pherolang highlight`. The workspace version is `0.4.0`; the tag and publish
are the operator step in
[docs/RELEASE-0.4.0.md](docs/RELEASE-0.4.0.md). Until that tag exists the
committed site data is source-built (the documented exception in
[docs/HIGHLIGHTING.md](docs/HIGHLIGHTING.md#release-binary-status)), and
production deploy waits for the released version to match the workspace
version.

### Added

- **Design artifacts now name the domain shape they were compiled against**
  (DF1, issue [#324](https://github.com/yestechgroup/pherolang/issues/324)):
  every `DddModel` carries a `domainFingerprint` — a SHA-256 over the
  canonicalized imported domain models (`pheroVersion` stripped) plus the
  sorted package names — emitted even when the design imports no domains.
  `pherolang artifact check design.ddd.json --domain domain.mox.json`
  recomputes it and refuses a stale pair with a diagnostic naming both
  package sets; `phero-backend-ddd` requires a well-formed block and exposes
  `check_pairing` for consumers. It is a staleness check, not a signature.
- **Search document shapes can reuse a DTO boundary** (DF3, issue
  [#326](https://github.com/yestechgroup/pherolang/issues/326)): a search
  may write `document BookSummary;` instead of repeating an inline block.
  The boundary must be a same-module mirror/projection of the search's
  entity; the artifact carries `documentRef` and no inline entries, and the
  inline/named forms are mutually exclusive (`E0400`–`E0402`). A search and
  a response DTO now share one declaration without drift.
- **Quality gates, checked corpus and release readiness for highlighting**
  (SH8, issue [#294](https://github.com/yestechgroup/pherolang/issues/294),
  epic #286): `tests/highlight/corpus/` now covers every surface with valid,
  truncated, and parse-error inputs plus Unicode/emoji/combining, CRLF,
  escapes, doc comments, contextual words in real and ordinary positions, and
  `.ifml` lookalikes. The compact summaries
  (`tests/highlight/summary/{corpus,docs-fences}.json`), representative
  exact-span goldens, and committed invalid-version fixtures are byte-pinned;
  a new surface without a corpus case, or a keyword missing from the registry,
  fails by name. The CLI suite adds the mode/output/surface/exit-code edge
  cases, `scripts/ci-local.sh` runs `grammar --check`, and the site gate adds
  the zero-JS page rule and the release-version deploy guard. The release
  workflow now proves the shipped binary exposes `pherolang highlight` and
  round-trips the generated grammar package. Documentation gains the corpus,
  troubleshooting, performance and `formatVersion` policy sections plus
  [ADR 0003](docs/adr/0003-compiler-backed-highlighting-is-the-source-of-truth.md);
  the 0.4.0 release runbook is [docs/RELEASE-0.4.0.md](docs/RELEASE-0.4.0.md).

- **Static hover reference panels on the site** (SH7, issue
  [#293](https://github.com/yestechgroup/pherolang/issues/293), epic #286):
  `/demo/hover` adds a compiler-backed hover surface with no editor and no
  client JavaScript. Spans come from the committed SH4 artifact and symbol
  identities from the semantic graph; a small authored index
  (`site/packages/highlighting/hover.json`) names each node, span class and
  signature, and the documentation link is derived from the surface and docs
  manifest. A CSS-only `:has()` card previews the symbol while a permanent
  reference panel always carries the signature, summary and real docs link;
  the card is `aria-hidden`, so nothing is hover-only. A missing or invalid
  entry falls back to plain highlighted code and fails the local hover gate.
  Measured: 0 bytes of client JavaScript, 1,235 bytes gzip of hover CSS, and
  8,440 bytes gzip for the built page. Monaco/CodeMirror plus a browser LSP
  bridge (Option B) is deferred;
  [`docs/adr/0002`](docs/adr/0002-static-hover-reference-panels.md) records
  the decision and the estimated costs.
- **LSP semantic tokens and hover provenance** (SH6, issue
  [#292](https://github.com/yestechgroup/pherolang/issues/292), epic #286):
  `phero-lsp` now answers `textDocument/semanticTokens/full` from the same
  `phero-highlight` span stream as the CLI and the site, with a stable,
  standard-only legend (13 token types, 6 modifiers) derived from
  `HighlightClass`/`HighlightModifier` rather than a second mapping. Tokens
  are converted to UTF-16 once, split at line boundaries, delta-encoded, and
  cached per document version; all nine surfaces are covered (`.ifml` uses
  its parse-only path, needing no domain or import resolution), and
  `Punctuation`/`Error` are omitted by decision — punctuation belongs to the
  editor grammar, and lexical errors are diagnostics. Hover on every surface
  keeps its existing kind/signature/doc-comment content and appends a
  provenance footer naming the surface, the file, and the surface's
  documentation link; an unresolved reference still yields no hover. Tests
  cover the legend, multi-line splitting, delta encoding, the per-version
  cache, and hover footers; `docs/HIGHLIGHTING.md` documents the contract.
- **Typed repository queries in the `.ddd` surface** (DD9, issue
  [#306](https://github.com/yestechgroup/pherolang/issues/306), epic #297): a
  declared repository operation may now be a **finder** — a `where`
  predicate typed against the designed class plus the operation's
  parameters (bare features and `self.<feature>` both resolve; a
  parameter/feature collision is an error with the `self.` help), an
  explicit `sort` over direct stored scalar features (a class reference, a
  collection, and a derived feature are each refused with the reason), and
  bounded `page` metadata (`limit`/`max`/`cursor`, `limit > max` refused).
  A query is read-only by construction: no statement body, no `when` guard,
  no `throws`; `capability` is the read authorization and resolves against
  the application's `.actor` artifacts like a service operation's. The
  artifact gains the sparse `query` block (`predicate` verbatim, `sort` with
  resolved `fieldType` and `direction`, `pagination`, `readOnly: true`) and
  the resolved `capabilities` list; `DDD_MODEL_FORMAT_VERSION` stays 2 and
  query-less artifacts are byte-identical. `phero-backend-ddd` /
  `pherolang artifact check` enforce the read-only invariants from the
  artifact alone, and `E0396`–`E0399` are the new compile-time codes.
  `docs/DDD.md` gains the query section, the `search` vs `query` decision
  table, the Sculptor `findByQuery`/`findByCondition` mapping, and the
  consumer contract (parameterized store query, no string interpolation,
  `0..1` optional, no mutation or events, named generation-time
  rejections); `docs/BACKENDS.md` carries the wire shape; `examples/shop`
  gains real finders.
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
- **Collection semantics in the `.mox` surface** (DD6, issue #303, epic
  #297): a feature now carries a collection kind alongside its multiplicity.
  The constraint block extends to many-valued attributes, `contains`,
  `refers`, and `derived` features and gains the value-less contextual
  `set`: `T[]` is an ordered list (duplicates allowed), `T[] { unique }` an
  ordered distinct list, and `T[] { set }` an unordered, duplicate-free set
  (`{ unique set }` is legal and formats to `{ set }`). The Core IR gains
  the sparse `Feature.collection` (`"set"`, absent for a list) and a set
  lowers `constraints.unique: true` at the same time, so the semantic kind
  and the schema rule cannot diverge; list-only artifacts stay
  byte-identical. `pherolang artifact check` rejects a set without `unique`,
  a collection kind on a single-valued feature, or one on a `container`, and
  value bounds stay attribute-only (a relationship/derived bound is an error
  naming its family). The Rust backend emits `std::collections::BTreeSet<T>`
  for sets (order-insensitive equality, deterministic serialization) and
  rejects `float`/`double` set elements at generation time with the
  documented ordered-unique-list fallback; enums, datatypes, and
  vocabularies gain `PartialOrd, Ord`. JSON Schema keeps `uniqueItems: true`
  and invents no unordered keyword, so the list/set distinction lives in the
  artifact. `Map`, `Bag`, persistence ordering (`orderby`/`orderColumn`), and
  generic `Collection<T>` are documented non-goals with workarounds, `.ddd`
  signatures deliberately carry no collection kind, and the expression
  language types a set as `List(T)` (order unspecified). The
  `library` example gains `String[] tags { set }` with a dedup/order
  behavioral assertion.
- **Authored aggregate boundaries and the `children` block** (DD4, issue
  [#301](https://github.com/yestechgroup/pherolang/issues/301), epic
  [#297](https://github.com/yestechgroup/pherolang/issues/297)): an entity
  design may declare `root` (its own aggregate) or `aggregate <Root>`
  (membership in a root designed in the same module) after its flags. The
  authored decision outranks the containment derivation; a design that
  authors none still derives its root from the topmost designed entity that
  contains it, and incomplete or ambiguous containment is **unknown**, never
  a guess. The artifact gains the sparse `isAggregateRoot` (absent = unknown,
  and an incomplete domain union warns `E0385` per affected entity) plus, on
  a member, `aggregate {root, origin}` with `origin` `"authored"` or
  `"derived"`; rules 6 (repository placement) and 13 (cross-reference
  constraint) read the effective boundary, and `pherolang artifact check`
  requires the verdict. The child-operation block is renamed `extend` →
  `children`; `extend` still parses as a deprecated alias with warning
  `E0387`, and the formatter rewrites it. New codes `E0379`–`E0387`.
  `formatVersion` stays 2.
- **Invariants on entity and value designs** (DD5, issue
  [#302](https://github.com/yestechgroup/pherolang/issues/302), epic
  [#297](https://github.com/yestechgroup/pherolang/issues/297)):
  `invariant <expr>;` is repeatable after the design flags/boundary and
  before the repository/machine block. The expression is captured verbatim
  and typed with the designed class as `self`: it must be boolean, machine
  states bind as enum-typed names, collection algebra is available, optional
  navigation follows R3, and a repository or global read is `E0388`; a dto
  invariant is `E0389`. There is no deferral when the union lowered. The
  artifact gains the sparse ordered `invariants: [{expression}]`
  (`formatVersion` stays 2), and the consumer contract checks the predicates
  at value construction, aggregate save (root and every member), and after
  every transition, reporting
  `InvariantViolation { class, aggregateRoot, invariantIndex }`.
- **Multi-file design applications** (DD8, issue
  [#305](https://github.com/yestechgroup/pherolang/issues/305), epic
  [#297](https://github.com/yestechgroup/pherolang/issues/297)): a `.ddd`
  file with `application` is a **root**; top-level `module` blocks without
  one make a **fragment**. Fragments may import `.mox`, `.dmn`, and `.ddd`
  fragments; modules reopen across fragments and their members concatenate
  in first-appearance/import order, while duplicate members remain the
  existing application-wide errors. New entry points
  `compile_ddd_app_str`/`compile_ddd_app_str_with_decisions` produce one
  merged `DddModel` per application and no fragment artifact; single-file
  applications stay byte-identical. The CLI compiles roots (`check` prints
  `OK` for roots and fragments, `build` emits one artifact per root, `ir` on
  a fragment names the root, `fmt` formats fragments normally, and `verify`
  hashes fragment sources without emitting a fragment artifact), the LSP
  publishes fragment diagnostics through the root, and the semantic graph
  treats fragments as part of the root's application. Graph
  `applications[].id` now uses the authored `application <Name>` when a
  component has exactly one `.ddd` root (otherwise the smallest member path
  remains the fallback) — a pre-1.0 value change; the graph `formatVersion`
  stays 1.

### Changed

- **`extend` has a recorded removal milestone** (DF4, issue
  [#327](https://github.com/yestechgroup/pherolang/issues/327)): the
  deprecated child-operation alias warns `E0387` with "deprecated since 0.4
  and removed in 0.5"; `pherolang fmt` remains the migration path. 0.5 turns
  the warning into a compile error (the formatter still rewrites), and 1.0
  removes the grammar alternative. No in-tree `.ddd` source uses it.
- **Feature ids are documented as intra-publication ordinals, not cross-
  publication identities** (DF2, issue
  [#325](https://github.com/yestechgroup/pherolang/issues/325)): the wire
  contract (`phero-ir` rule 3, the `Feature::id` docs, `docs/BACKENDS.md`,
  and register row V10) now says exactly what is guaranteed. Inserting or
  reordering a feature renumbers later slots (a test pins it); consumers key
  continuity on `(package, class name, feature name)`. No wire-shape change,
  `formatVersion` stays.
- **`.ddd` resolution documentation corrected** (Phase 0 follow-up): the
  `compile_ddd_str` reference no longer claims resolution is validation-only
  — the artifact keeps the authored spelling and adds the resolved companion,
  matching the shipped DD1 contract (issue
  [#298](https://github.com/yestechgroup/pherolang/issues/298)).
- **Multi-file `.ddd` applications are correct end to end** (DD8 follow-up
  audit, issue [#305](https://github.com/yestechgroup/pherolang/issues/305)).
  An import now resolves relative to the file that **declares** it, so a
  fragment's `import "domain.mox"` names the fragment's own directory; the
  CLI, the LSP snapshot, the `.uc`/`.deploy`/`.cmmn` importers, and the
  actor join's design pool all compile a fragmented root **whole** (root plus
  every fragment plus every domain) instead of silently compiling the root
  alone. The five tree-level failures — orphan fragment, fragment under two
  roots, duplicate application name, import cycle, root imported as a
  fragment, and a missing fragment — are now **spanned diagnostics** with
  stable codes `E0390`–`E0395` anchored at the offending import (the
  fragment's first module for an orphan) instead of plain `Error:` lines.
  Editor navigation for fragments is wired through: a fragment's span index
  is built with the root application name, a declaration search falls back
  from the node's attributed file to the file that spells it, the fragment's
  content key covers the root and every sibling fragment, and the editor
  snapshot supplies the fragment file inputs the CLI's `graph build` already
  did (file node and `applications[].files` membership). Capability-noun
  classification now reads a design's effective aggregate boundary from the
  artifact (`isAggregateRoot`/`aggregate`) and stays silent on an undecided
  boundary, falling back to containment derivation only for a hand-built
  artifact with no verdict.

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
[0.4.0]: https://github.com/yestechgroup/pherolang/releases/tag/v0.4.0
[0.3.0]: https://github.com/yestechgroup/pherolang/releases/tag/v0.3.0
