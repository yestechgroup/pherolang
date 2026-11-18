# Verification (proof-obligation register)

Phero Lang's formal-verification strategy follows
[issue #83](https://github.com/yestechgroup/pherolang/issues/83): identify the
properties that must **always** hold, specify them once, and let a prover —
[Creusot](https://creusot.rs) (Rust → Coma → Why3 → SMT) — discharge them for
**all** inputs, complementing unit tests (particular examples) and property
tests (many generated inputs). The goal is deliberately not "formally verify
Phero Lang"; it is *a formally specified, incrementally verified semantic
foundation that the languages, transformations and generators depend on*.

## What is verified today

The crate `phero-verify` hosts the first verified component: a semantic-graph
kernel (uniquely identified nodes, multi-parent DAG containment shaped like
Phero Lang's `extends` lists, deduplicated reference edges) specified with
Creusot contracts. **`cargo creusot -p phero-verify` is fully green**
("Proved (42 files)"): every proof obligation below is SMT-discharged.

- `Graph::well_formed` (`crates/phero-verify/src/lib.rs`) is the kernel
  invariant: node count below the `u32` id bound, every parent/edge endpoint
  valid, and the child half of the parent/child agreement. The mutators
  carry `requires(well_formed(*self))` / `ensures(well_formed(^self))`, so
  *any* observable graph is well-formed and *every* mutation provably
  preserves well-formedness (issue property 5). Per-mutation facts are
  bridged against the trusted mutation primitives' contracts
  (`push_default_node`, `add_parent`, `push_containment`,
  `push_reference` — one-statement bodies, exact model-effect contracts).
- `Graph::add_node` carries the freshness contract (issue property 1): the
  returned id equals the previous node count, the count grows by one, and
  the id differs from every pre-existing id — combined with ids being
  indices, identities are unique forever.
- `Graph::link` / `Graph::reference` carry endpoint-existence preconditions
  (issue property 2); `reference` additionally ensures a recorded edge
  stays recorded. `link` models Phero Lang's `extends` lists: several parents
  are legal, a duplicated edge is an idempotent success (never a cycle),
  and a refused edge is exactly a closing cycle — which is how
  `phero-driver`'s inheritance/actor cycle passes now detect cycles (see V3).
- `Graph::is_ancestor` is proved safe (indexing) and terminating (bounded
  parent walk); `link`'s cycle rejection relies on it operationally.
- Containment acyclicity (issue property 3) is *enforced operationally* by
  `link`'s ancestor guard and pinned by the property tests. The reachability
  machinery the formal statement needs is now defined and partly proved:
  bounded-`k` reachability (`anc_k` reflexive, `anc_strict_k` strict), the
  budgeted closures (`anc` / `anc_strict`, witness ≤ node count — the
  executable walk's budget), `acyclic` (no strict cycle within the budget),
  and the proved **shift lemma** (`lemma_anc_shift`, induction on the
  bound): bounded reachability in both directions transfers across any
  mutation that keeps every old parent list and appends parentless slots —
  the shape of every mutator except `link`. What remains open is exactly
  the hard residue: linking the executable walk to the predicate
  (`is_ancestor`'s functional spec, register V7) and preserving `acyclic`
  across `link` — both need a path-shortening (loop-cutting) lemma and,
  for completeness, a DFS-completeness invariant; this is the registered
  open obligation below. The
  parent/child agreement for the *whole* containment edge set (not just the
  newly added edge) is likewise carried per-edge by `link`'s ensures plus
  the property tests rather than by the consolidated invariant.
- Issue property 4 ("type references always resolve") is carried by the Core
  IR design itself: `phero_ir::TypeRef` is *never an unresolved name*, and the
  driver's resolution emits diagnostics instead of unresolved references,
  with any error severity blocking publication of the model
  (`compile_file`). See the register's pipeline rows.

The same properties are pinned empirically by `crates/phero-verify/tests/`
(unit tests named `p1_*`…`p5_*` plus proptest mutation sequences; register
row V11 adds the `p11_*` interaction-reference discipline — see its row
below). The oracle's two bounded walk helpers are themselves proved against
the kernel's loop discipline: `contains_edge` carries the full functional
spec (`result == has_contains(parent, child)`), and `has_containment_cycle`
is proved memory-safe and terminating (out-of-range parent edges — possible
only on malformed graphs — are skipped rather than indexed), shrinking the
`#[trusted]` set to the oracle dispatcher and the four one-statement
mutation primitives (V8/V9). The
property tests are not decoration: during development they shrank a real
counterexample (stale subtree state under re-rooting) that the unit suite
missed, and the prover caught a second: `well_formed` originally allowed
`len == u32::MAX`, under which `add_node`'s fresh id would overflow — the
preservation ensures was *unprovable because false*, exactly the class of
mistake formal specification exists to catch.

Avoidances, recorded for the next toolchain bump: the `Hash`,
`PartialOrd` and `Ord` derives are omitted from `NodeId` because creusot-std
0.14.0 leaves their refinement obligations unresolved upstream; std
`Vec::contains` is avoided in `link` (its spec carries an unprovable
resolution obligation) in favour of a proved bounded loop; in-crate
`phero-ir` proofs are blocked by the same family of gaps (the wire types'
standard `PartialEq`/`Hash`/`Ord` derives demand `DeepModel` instances and
refinement obligations creusot-std 0.14.0 does not discharge — hence the
`feature_ids` spec mirror, to be moved in-crate on a toolchain bump); and the
`#[trusted]` mutation primitives exist because the prover cannot yet
re-establish the consolidated invariant directly after a raw
`Vec`-indexing borrow (quantifier instantiation across the borrow boundary)
— with the primitives in place everything else is proved.

## Running the proofs

Contracts compile away under plain `cargo build` (stable); CI runs only the
ordinary test/clippy/fmt gates. To discharge the proof obligations, install
Creusot (see <https://guide.creusot.rs/installation.html>) and run:

```sh
cargo creusot -p phero-verify
```

`creusot-std` is pinned to the matching `0.14.x` line in the workspace
manifest; the proof setup lives in `why3find.json` (repo root, provers
alt-ergo + z3, tactic `split_vc`) and the generated Coma lands in `verif/`
(git-ignored, like `.creusot-cache*`). Notes from the first bring-up:

- the pinned Why3 revision ships `compute_specified` as a
  transform-with-arguments, which this `why3find` cannot use as a tactic —
  the `Tactic compute_specified not found (skipped)` warning is cosmetic;
- `creusot-std`'s contracts must still compile (warning-free) under plain
  stable rustc — CI depends on it.

## Proof-obligation register

Statuses: **proved** (SMT-discharged under `cargo creusot`), **tested**
(pinned by the test suites), **argued** (manual argument + tests, formal
proof open), **assumed** (`#[trusted]` or absent checks — an assumption, not
a proof). Every `#[trusted]` in the tree must have a row here.

A **tested** row additionally carries exactly one `mutant` evidence pointer —
a recorded mutation of its checking code (`tests/mutants/<row>.patch`) that
the row's own pinned tests must **kill** — and `mutantKilled: true`. The kill
is re-run by `scripts/mutants.sh` (CI job `mutants`): the patch is applied,
only that row's tests run, and the tree is reverted; a surviving mutant is
red CI naming the row, so a green suite that pins nothing cannot pass for
evidence. `proved` rows are exempt (their anti-vacuity story is the proof
run); `argued`/`assumed` rows claim no executable evidence.

The tables are generated from the machine-readable register source
`docs/verification/register.json` — hand-edit the JSON, never the tables —
and pinned by the drift gate in `crates/phero-verify/tests/trusted_register.rs`
(regenerate the block with `PHERO_UPDATE_FIXTURES=1 cargo test -p phero-verify`).

<!-- BEGIN GENERATED REGISTER (source: docs/verification/register.json; regenerate: PHERO_UPDATE_FIXTURES=1 cargo test -p phero-verify) -->
| # | Property (issue #83 wording) | Formal specification | Enforcing implementation | Status |
|---|---|---|---|---|
| V1 | Nodes always have unique identities | `add_node` `ensures`: fresh id = old count, count + 1, id ∉ old ids | `Graph::add_node` (`phero-verify`); ids are arena indices, never reused | proved |
| V2 | Relationships always reference existing nodes | `well_formed` endpoint clauses; `link`/`reference` `requires` | `Graph` API (only constructor of ids); driver `resolve*` for the real IR | proved |
| V3 | Containment graphs cannot contain cycles | reachability machinery defined and partly proved: `anc_k`/`anc_strict_k` (bounded-`k` reflexive/strict closure), `anc`/`anc_strict` (budgeted witness ≤ node count), `acyclic` (no strict cycle within the budget); **`lemma_anc_shift` proved** — bounded reachability (reflexive + strict, both directions) transfers across any mutation keeping old parent lists and appending parentless slots (induction on the bound) | `link` ancestor guard (`is_ancestor`: proved safe + terminating); `tests/invariants.rs` `p3_*` + proptest `p3_containment_stays_acyclic`; **`phero-driver` inheritance/actor cycle passes now detect cycles via rejected `link` calls** (`detect_inheritance_cycles` / `detect_actor_cycles` on `phero_verify::Graph`); use-case `check_graph` still DFS-based | argued + tested — spec machinery proved (shift lemma); `is_ancestor` functional spec and acyclicity preservation under `link` open — they need a path-shortening (loop-cutting) lemma plus a DFS-completeness invariant, deliberately budgeted as #93's hard residue; driver detection kernel-backed |
| V4 | Type references always resolve | `phero_ir::TypeRef` "never an unresolved name" (crate docs, wire rule set) | `phero-driver` resolution + error-severity block in `compile_file`; `phero-expr::DomainTypes` for expressions | tested |
| V5 | Mutations preserve all established invariants | `well_formed(*self)` → `well_formed(^self)` on every mutator | every `phero-verify` mutator; bridged against the trusted mutation primitives; proptest sequences with `check_invariants` oracle | proved — the children-agreement clause for the *whole* edge set is carried per-edge by `link`'s ensures + tests; consolidated `children_ok` forall documented open |
| V6 | Reference edges are recorded and deduplicated | `reference` `ensures` (`has_ref` persists), `requires` endpoints | `Graph::reference`; `p2_*` tests | proved |
| V7 | `is_ancestor` is a safe, terminating guard | `requires(well_formed)`, loop variant `budget` | bounded parent walk in `is_ancestor` | proved — safety/termination; functional spec = V3, open |
| V8 | `check_invariants` (executable oracle) mirrors the invariant | `contains_edge`: full functional spec (`ensures result == has_contains(parent, child)`); `has_containment_cycle`: safety + termination (bounded mark-on-push walk; `requires` the kernel's `u32` length bound and a valid `start`) | `Graph::check_invariants` (`#[trusted]` — it must inspect *arbitrary*, possibly malformed graphs, states the invariant excludes) + `contains_edge` / `has_containment_cycle` (proved against the logical `has_contains` / `is_ancestor`-shaped loop discipline) | assumed — deliberate assumption, not an oversight — the dispatcher is test-side (validated by the property suites, never relied on by proofs) and must inspect arbitrary, possibly malformed graphs, the very states the invariant excludes; the prover cannot re-establish the consolidated invariant across that raw `Vec`-borrow boundary (quantifier instantiation across the borrow). Both walk helpers are proved. Shrinking path (#90's spirit): shrinking the trusted set is always a welcome PR |
| V9 | Mutation primitives do exactly what their contracts state | full model-effect contracts (snoc + changed fields) | `push_default_node`, `add_parent`, `push_containment`, `push_reference` (all `#[trusted]`, one-statement bodies) | assumed — deliberate assumption, not an oversight — the one-statement bodies exist because the prover cannot re-establish the consolidated invariant directly after a raw `Vec`-indexing borrow (quantifier instantiation across the borrow boundary); bodies assumed, full model-effect contracts proved (see the `#[trusted]` policy). Shrinking path (#90's spirit): shrinking the trusted set is always a welcome PR |
| V10 | Wire rule 3: a feature's id is its 0-based declaration index within one publication (an intra-publication ordinal, never a cross-publication identity); `assign_feature_ids` establishes it and changes nothing else | `phero_verify::feature_ids::assign_ids` `ensures` (length unchanged, `id(i) == i` for all slots, within the `u32` bound) | proved over the spec mirror `phero_verify::feature_ids` (identical loop shape + trusted one-statement primitive `set_id`); `phero-ir`'s `ClassDef::assign_feature_ids` pinned executably by `crates/phero-ir/tests/feature_ids.rs`, including the DF2 renumbering characterization (`inserting_a_feature_renumbers_later_ids`): cross-publication identity is `(package, class name, feature name)`, not the id | proved (spec mirror) — in-crate `phero-ir` translation blocked upstream (see avoidances); the identity contract is an intra-publication ordinal, not a cross-publication key (DF2, issue pherolang#325) |
| V11 | Interaction references always resolve to declarations ([issue #112](https://github.com/yestechgroup/pherolang/issues/112)): every interaction reference edge — `action("X")`, `navigate("X")`, `refresh("X")` — terminates at a **declaration-backed** node, never a synthesized one | every invocation edge's endpoint ∈ the declared set (`InvocationGraph::check_invariants` in `crates/phero-verify/tests/invariants.rs`, `p11_*`) | `phero-ifml`'s `check_references` cross-reference pass (`E0104`–`E0106`, entry file plus **instantiated-module** sites, union-wide declarations; container targets included) flowing into `pherolang check`'s error block and the LSP's published diagnostics — and, as the artifact boundary, `pherolang manifest`: the control/route manifest ([issue #118](https://github.com/yestechgroup/pherolang/issues/118), `phero_ir::ifml_manifest`) is derived post-check, so every manifest edge's target resolved by construction and consumers cannot re-derive resolution or synthesize endpoints; the proptest validates the oracle against an independent recomputation over random histories | tested — honest staging per V4's precedent — the kernel tracks no node metadata yet; the graph-level `proved` form belongs in `phero-verify` once the ingest stops synthesizing endpoints from invocations, the codegraph companion — which the manifest boundary now makes structurally impossible on the artifact path |
| V12 | Lifecycle wiring always resolves across the artifact union ([issue #114](https://github.com/yestechgroup/pherolang/issues/114)): a capability clause names a declared capability; a granted or delegated capability is carried by an operation or search; an `.ifml` action declaration binds to a service operation (case-insensitive verb equality); a subscription consumer names a declared deploy component; flow `requires:`/`roles:` resolve into the actor layer; an operation's `emits` event name is declared in some `.evt` contract; a declared event is emitted by at least one service operation (channel `publishes`/subscription lists are consumption-side and never satisfy production). Since pherolang#128 the two capability joins resolve at **compile time** (rule 10 pooling every provided actor model, with the `unknownCapability` did-you-mean code; grant coverage in the actor engine, code `grantedCapabilityUncarried`) — the union pass keeps them as its lint channel (warning severity). Every union-pass diagnostic carries its stable registry code prefixed on the message (`X0001`–`X0008`, the `driver.integrity` band of [the diagnostic-code registry](DIAGNOSTICS.md)), and an action whose case-insensitive mirror matches **several** service operations binds to no unique backend — an error (`X0006`), the mirror naming exactly one `(service, operation)` carrier | `phero_driver::check_integrity` over the gathered artifact buckets — a pure function, pinned at the artifact level with exact diagnostic text plus its silent gates in `crates/phero-driver/tests/integrity.rs`; the promoted capability joins are pinned in `crates/phero-driver/tests/ddd.rs` (rule 10, did-you-mean), `crates/phero-driver/tests/actors_file.rs` (coverage), `crates/phero-cli/tests/cli.rs` (directory mode, phase 1 owns the errors), and `crates/phero-lsp/src/server/tests.rs` (diagnostics + the rename quick-fix) | the CLI's directory batch mode: per-file artifacts cluster into import-connected applications (the house import graph) and the pass runs per application, error diagnostics failing the batch; the compile-time capability joins run in the CLI project pipeline and the LSP per document against the sibling artifacts, with the union-pass lint suppressed for files phase 1 already failed; per-surface single-file compiles are unchanged | tested — enforcement landed; the kernel/proptest staging follows V11's precedent once the graph ingest stops synthesizing endpoints — the same codegraph companion |
| V13 | State machines are closed over their enum, and lifecycle guards are predicates: every machine state (initial, terminal, transition endpoint) is a literal of the machine's enum type, and every `when` condition — transition guard, service operation, repository operation — types as `boolean` | state-set membership ⊆ the enum's literal list; `check_condition`'s result type = `boolean` | `lower_design_machine` + `check_condition` in `crates/phero-driver/src/ddd.rs`, pinned by `crates/phero-driver/tests/ddd_machines.rs` (six tests: three boolean sites, typo'd endpoint, non-literal initial/terminal, all-literals clean) | tested |
| V14 | Obligations are a declared vocabulary: every grant or delegation entry's obligation names resolve against the union of the actors blocks' `obligation` declarations | references ⊆ the union declared set | `validate_actor_obligations` in `crates/phero-driver/src/lower/actors.rs` over the `ActorsRecord` obligation refs, pinned by `crates/phero-driver/tests/actors.rs` (IR shape, grant/delegation errors with spans, duplicates, union pooling) | tested |
| V15 | Domain-type lookup agreement (issue #94, the first verified statement for expression typing): a domain-type index is **total** over what was registered — every key resolves to its registration, last registration winning — and **merge preserves per-source lookups** (the merged-in index wins on collision, the surviving index resolves everywhere else) | `TypeIndex::lookup` full functional spec (`result == lookup_spec(entries, key)`), `insert` replace semantics, `merge` right-wins concat semantics (all proved; helper `lemma_bound_at_prefix`, induction on the prefix length) | spec mirror `phero_verify::domain_types` — insert-replaces + linear-scan lookup standing in for `phero_expr::DomainTypes`' `BTreeMap` (whose std specs creusot-std 0.14.0 cannot translate — the V10 upstream-gap family), executably pinned by `crates/phero-verify/tests/domain_types.rs` and shape-pinned on the real API by `crates/phero-expr/tests/typecheck.rs` (`domain_types_is_assembled_incrementally_and_merges`); moves in-crate on a toolchain bump | proved (spec mirror) |
| V16 | Class→artifact correspondence at the IR → JSON Schema boundary (issue #95, the first proved transformation statement): the collector admits exactly the declarations with distinct class names (duplicates rejected — `Context::collect`'s `bail!`), and the emitted `$defs` are in correspondence with the declared classes — **one def per class, in declaration order, none dropped, none synthesized** | `correspondence::collect` two-directional contract (`Some` ⇒ keys unchanged + `distinct`; `None` ⇒ a witnessed `has_duplicate`), `def_keys` identity emit (`result@ == classes@`) | spec mirror `phero_verify::correspondence` — interned keys standing in for class names, identity emit standing in for `wire_defs`' per-class loop (the `phero-ir` wire types cannot translate under creusot-std 0.14.0 — the V10 upstream-gap family); executably pinned by `crates/phero-verify/tests/correspondence.rs` and on the real emitter by `crates/phero-backend-jsonschema`'s `wire_defs_map_every_declared_class` / `collect_rejects_duplicate_class_names`; moves in-crate on a toolchain bump | proved (spec mirror) |
| V17 | Stripped-name uniqueness within a package (issue #177): two declarations — classes or enums, in any combination — whose names strip to the same module identifier (one trailing `Type` suffix, then snake_case; `PayRun` and `PayRunType` both strip to `pay_run`), or two classes mapping to the same effective table name (declared `table "…"` override, else the derived default), are rejected; a pair where either class is a containment target is exempt (a contained value object shares its owner's module downstream by design), and a pair already diagnosed as E0300 is not re-diagnosed as E0301 (one rename resolves both) | both keys are pure functions of the declarations (`stripped_module_name`, the effective table) plus the containment set; the table key's derivation matches `phero_backend_common::snake_case` exactly, so the check's keys equal what consumers derive | `check_name_uniqueness` in `crates/phero-driver/src/lower/names.rs` over per-package records, wired into all three lowering pipelines and skipped when the model already failed to lower (no cascade); the diagnostics carry the `driver.domain` codes (E0300/E0301) prefixed on the message and span the later declaration; the derivation vectors, both codes, the exemption, the override's IR flow, and the no-cascade discipline are pinned by `crates/phero-driver/tests/domain_names.rs`, the `lower::names` unit tests, `crates/phero-ir/tests/class_table_name.rs` (wire byte-identity), and the `name_uniqueness` conformance fixture's IR golden | tested |

### Pipeline-level obligations (issue's five boundaries)

| Boundary | What we want (issue) | Current enforcement | Status |
|---|---|---|---|
| Parsing | parsed structures satisfy AST invariants | logos+chumsky parser tests; `pherolang fmt` byte-identical round-trip goldens | tested |
| Semantic analysis | invalid models cannot pass validation | driver validators: duplicate names, reference resolution, opposite pairing, inheritance/actor cycles, `never_both`; error ⇒ no model | tested |
| Intermediate representation | IR satisfies structural/semantic invariants | `phero-ir` wire contract (version gate, frozen feature ids, deterministic equality) + golden/byte-identity suites; `phero-verify` kernel as the future verified core | tested (kernel subset: proved) |
| Transformations | transformations preserve meaning | golden artifacts per surface + examples harness; **V16: the IR → JSON Schema class correspondence is proved over its spec mirror** (`phero_verify::correspondence`) | tested (mirror proved; full preservation = issue phase 8, open) |
| Code generation | generated artefacts satisfy contracts | backends consume only `phero_ir::Model`; scratch-crate `cargo test` e2e; schema conformance proptests | tested |

### Evidence

| ID | Status | Evidence | Trigger | Review |
|---|---|---|---|---|
| V1 | proved | `proof`:`crates/phero-verify/src/lib.rs:add_node`<br>`test`:`crates/phero-verify/tests/invariants.rs:p1_fresh_ids_are_unique_and_resolvable`<br>`test`:`crates/phero-verify/tests/invariants.rs:p1_earlier_ids_survive_later_growth`<br>`test`:`crates/phero-verify/tests/invariants.rs:p1_ids_are_never_reused` | — | — |
| V2 | proved | `proof`:`crates/phero-verify/src/lib.rs:well_formed`<br>`proof`:`crates/phero-verify/src/lib.rs:link`<br>`test`:`crates/phero-verify/tests/invariants.rs:p2_no_dangling_relationships_ever` | — | — |
| V3 | argued | `proof`:`crates/phero-verify/src/lib.rs:lemma_anc_shift`<br>`code`:`crates/phero-verify/src/lib.rs:link`<br>`test`:`crates/phero-verify/tests/invariants.rs:p3_containment_stays_acyclic`<br>`code`:`crates/phero-driver/src/lower/classes.rs:detect_inheritance_cycles`<br>`code`:`crates/phero-driver/src/lower/actors.rs:detect_actor_cycles`<br>`test`:`crates/phero-driver/tests/cycle_detection.rs` | a time-boxed formal attempt under epic #202's T1 — one focused session; if the box is exceeded, downgrade the row with a written rationale, never leave it silently open | 2027-02-13 |
| V4 | tested | `code`:`crates/phero-driver/src/lower/resolve.rs:resolve_single`<br>`code`:`crates/phero-driver/src/entry/mox.rs:compile_file`<br>`code`:`crates/phero-expr/src/checker/context.rs:DomainTypes`<br>`test`:`crates/phero-driver/tests/compile.rs`<br>`mutant`:`tests/mutants/V4.patch` | — | — |
| V5 | proved | `proof`:`crates/phero-verify/src/lib.rs:link`<br>`test`:`crates/phero-verify/tests/invariants.rs:p5_reparenting_keeps_invariants`<br>`test`:`crates/phero-verify/tests/invariants.rs:p5_invariants_hold_after_random_mutation_sequences` | — | — |
| V6 | proved | `proof`:`crates/phero-verify/src/lib.rs:reference`<br>`test`:`crates/phero-verify/tests/invariants.rs:p2_reference_edges_deduplicate` | — | — |
| V7 | proved | `proof`:`crates/phero-verify/src/lib.rs:is_ancestor` | — | — |
| V8 | assumed | `code`:`crates/phero-verify/src/lib.rs:check_invariants`<br>`proof`:`crates/phero-verify/src/lib.rs:contains_edge`<br>`proof`:`crates/phero-verify/src/lib.rs:has_containment_cycle`<br>`test`:`crates/phero-verify/tests/invariants.rs:p5_invariants_hold_after_random_mutation_sequences` | shrinking the trusted set is always a welcome PR (#90's spirit): prove the dispatcher's cycle walk against the kernel's loop discipline, or retire it with a creusot-std bump that closes the borrow-boundary gap | 2027-05-14 |
| V9 | assumed | `code`:`crates/phero-verify/src/lib.rs:push_default_node`<br>`code`:`crates/phero-verify/src/lib.rs:add_parent`<br>`code`:`crates/phero-verify/src/lib.rs:push_containment`<br>`code`:`crates/phero-verify/src/lib.rs:push_reference` | shrinking the trusted set is always a welcome PR (#90's spirit); a creusot-std bump that re-establishes the consolidated invariant across a raw `Vec` borrow (release notes) retires the primitives | 2027-05-14 |
| V10 | proved | `mirror`:`crates/phero-verify/src/feature_ids.rs:assign_ids`<br>`code`:`crates/phero-ir/src/class.rs:assign_feature_ids`<br>`test`:`crates/phero-ir/tests/feature_ids.rs:ids_equal_declaration_indices`<br>`test`:`crates/phero-ir/tests/feature_ids.rs:inserting_a_feature_renumbers_later_ids` | the next creusot-std/Creusot toolchain bump — its release notes are the trigger: watch them for the derive-gap family closing, then move the mirror in-crate | 2027-05-14 |
| V11 | tested | `code`:`crates/phero-ifml/src/check/references.rs:check_references`<br>`code`:`crates/phero-ifml/src/manifest.rs:manifest_ifml`<br>`test`:`crates/phero-verify/tests/invariants.rs:p11_dangling_invocation_edge_trips_the_oracle`<br>`test`:`crates/phero-verify/tests/invariants.rs:p11_declared_invocation_edge_passes_the_oracle`<br>`test`:`crates/phero-verify/tests/invariants.rs:p11_invocation_edges_terminate_at_declaration_backed_nodes`<br>`test`:`crates/phero-ifml/src/check/tests/interaction_reference.rs:dangling_action_invocation_is_reported`<br>`test`:`crates/phero-ifml/src/check/tests/instantiated_modules.rs:instantiated_module_dangling_references_are_diagnosed`<br>`mutant`:`tests/mutants/V11.patch` | — | — |
| V12 | tested | `code`:`crates/phero-driver/src/integrity.rs:check_integrity`<br>`test`:`crates/phero-driver/tests/integrity.rs`<br>`test`:`crates/phero-driver/tests/ddd/rule_10_capabilities.rs`<br>`test`:`crates/phero-driver/tests/actors_file.rs`<br>`test`:`crates/phero-cli/tests/cli/integrity.rs`<br>`test`:`crates/phero-lsp/src/server/tests/capability_joins.rs`<br>`mutant`:`tests/mutants/V12.patch` | — | — |
| V13 | tested | `code`:`crates/phero-driver/src/ddd/machines.rs:lower_design_machine`<br>`code`:`crates/phero-driver/src/conditions.rs:check_condition`<br>`test`:`crates/phero-driver/tests/ddd_machines.rs`<br>`mutant`:`tests/mutants/V13.patch` | — | — |
| V14 | tested | `code`:`crates/phero-driver/src/lower/actors.rs:validate_actor_obligations`<br>`test`:`crates/phero-driver/tests/actors/obligations.rs`<br>`mutant`:`tests/mutants/V14.patch` | — | — |
| V15 | proved | `mirror`:`crates/phero-verify/src/domain_types.rs:lookup_spec`<br>`test`:`crates/phero-verify/tests/domain_types.rs:lookup_matches_the_logical_spec`<br>`test`:`crates/phero-expr/tests/typecheck.rs:domain_types_is_assembled_incrementally_and_merges` | the next creusot-std/Creusot toolchain bump — its release notes are the trigger: watch them for the std-spec translation gap closing, then move the mirror in-crate | 2027-05-14 |
| V16 | proved | `mirror`:`crates/phero-verify/src/correspondence.rs:collect`<br>`test`:`crates/phero-verify/tests/correspondence.rs:collect_admits_distinct_keys_and_emits_one_def_per_class`<br>`test`:`crates/phero-backend-jsonschema/src/lib.rs:wire_defs_map_every_declared_class`<br>`test`:`crates/phero-backend-jsonschema/src/lib.rs:collect_rejects_duplicate_class_names` | the next creusot-std/Creusot toolchain bump — its release notes are the trigger: watch them for the `phero-ir` wire types translating, then move the mirror in-crate | 2027-05-14 |
| V17 | tested | `code`:`crates/phero-driver/src/lower/names.rs:check_name_uniqueness`<br>`test`:`crates/phero-driver/tests/domain_names.rs`<br>`test`:`crates/phero-ir/tests/class_table_name.rs`<br>`test`:`tests/conformance/artifacts/name_uniqueness.phero.json`<br>`mutant`:`tests/mutants/V17.patch` | — | — |

### Deliberate holds

| Item | Reason | Trigger | Review |
|---|---|---|---|
| nightly-CI proof job (#88) | closed as a **documented non-goal**, not delivered: no proof workflow exists in `.github/workflows/` (only `ci.yml`), and prover availability and cost keep proofs local and per-release. The sanctioned form of proof automation is epic P6/T3's per-release proof-log artifact (`verif/` manifest + why3find summary) referenced by the release receipt — a nightly prover job would need its own go/no-go | the next release tag — where the P6/T3 per-release proof-log artifact would be produced | 2027-02-13 |
| `creusot-std` 0.14.0 → 0.15.0 | proofs are toolchain-coupled: a stdlib bump needs the matching Creusot upgrade, a full `cargo creusot -p phero-verify` re-run, and a register review; held through v0.2 per the #173 plan | the Creusot toolchain upgrade lands (#220) | 2027-02-13 |
| `rowan` / `cstree` / `ungrammar` (lossless CST) | would fix `.ifml` comment loss and enable structural navigation, but demands a full parser rewrite and rowan is alpha (#173 Tier-3) | a concrete need for lossless comments or structural navigation, or rowan leaves alpha | 2027-05-14 |
| `comemo` / `async-lsp` | architectural alternatives to salsa / tower-lsp-server; held unless there is a concrete limitation (#173 Tier-3) | a concrete salsa or tower-lsp-server limitation surfaces | 2027-05-14 |
| `serde_jcs` (RFC 8785 canonical JSON) | canonical JSON for signed/hashed artifacts; the current key sorting is not full JCS and signing is not on the roadmap (#173 Tier-3) | artifact signing joins the roadmap | 2027-05-14 |
<!-- END GENERATED REGISTER -->

## Second-prover evaluation (issue #96)

The phase-7 question — whether a second theorem prover (Lean) earns a
place next to Creusot — was answered by a spike: `formal/lean/` carries a
paper semantics of the `docs/EXPRESSIONS.md` fragment R1–R4 with twelve
theorem restatements (`lean PheroExpr.lean`, no `sorry`, core-only
toolchain pinned in `lean-toolchain`). The verdict, with the effort and
maintenance analysis, is recorded in `formal/lean/README.md`:
**recommendation down** — executable specs plus Creusot cover the
covered-best fragment, the next Lean-shaped steps carry the full
toolchain-maintenance burden without a proving consumer, and the file
stays as the re-runnable artifact pending a creusot-std translation
unblock or a defect class executable specs cannot state.

## `#[trusted]` policy

`#[trusted]` means the prover assumes the body: it is an **assumption**, not
a proof (issue #83's explicit warning). Rules for this repository:

1. every `#[trusted]` must have a register row above, with the reason it is
   not proved;
2. `#[trusted]` may not be used on anything that feeds a proved contract's
   *meaning* — the mutation primitives' contracts state their full model
   effect and everything else is proved against those contracts, so the
   trusted surface is minimal and auditable by eye (one-statement bodies);
3. shrinking the trusted set is always a welcome PR.

## Non-goals (for now)

- Creusot in CI — issue #88's nightly proof job was closed as a
  **documented non-goal, not delivered**: no proof workflow exists in
  `.github/workflows/` (only `ci.yml`), and prover availability and cost
  keep proofs local/per-release. The sanctioned form of proof automation is
  epic P6/T3's per-release proof-log artifact (see the deliberate holds
  above); a nightly job would need its own go/no-go.
- Formal specs inside `phero-driver`/`phero-syntax` (salsa and parser internals
  stay outside the verified kernel; the kernel is the seed of the verified
  semantic foundation, per the issue's roadmap phases 4–8).
- Semantic-preservation proofs for lowerings (phase 8).
