# Golden fixtures

A **golden** is a committed file that a test compares byte-for-byte against
freshly generated output. There are roughly 200 of them: IR artifacts under
`tests/conformance/`, generated engines and schemas, instance JSON, formatter
output, highlight artifacts, the generated editor grammar package, and the two
generated pages under `docs/`.

The rule, in one line: **goldens are the spec; when one changes, read the
diff.** A golden update is never "just regenerate" — it is a contract change
that needs a reason in the commit message. Body-less and vocabulary-less
models in particular must stay byte-identical after any emitter change.

## The one regeneration command

```sh
scripts/regen-goldens.sh           # regenerate every golden, then show the tree
scripts/regen-goldens.sh --check   # regenerate; fail if anything changed
```

It runs `PHERO_UPDATE_FIXTURES=1 cargo test --workspace --no-fail-fast
--jobs 2` under `scripts/ci-env.sh` — the same environment `scripts/ci-local.sh`
uses (see [the traps](#traps) for why both details matter). `--check` refuses
to start on a dirty tree, so its verdict means "these goldens are fresh", not
"these goldens plus my uncommitted work are fresh".

A per-suite regeneration is fine when you know which writer you touched
(`PHERO_UPDATE_FIXTURES=1 cargo test -p phero-driver --test ddd_golden`), but
a release or a cross-cutting change goes through the script.

`scripts/ci-local.sh` (the full gate) never regenerates: it compares, and a
stale golden is red there first.

## Writer inventory

Derived from `git grep -l PHERO_UPDATE_FIXTURES -- '*.rs'`. Three hits are
**not** writers and must not be "fixed" into the pattern: `phero-cli`'s
`commands/verify.rs` only prints the env var in an error hint,
`phero-evidence`'s crate docs mention it, and `phero-testkit` documents why it
deliberately has no write path.

| Suite (test binary) | Writes |
| --- | --- |
| `phero-driver` `golden_artifact` | `tests/conformance/artifacts/*.phero.json` (domain and `.actors.phero.json` pairs) |
| `phero-driver` `ddd_golden` | `tests/conformance/ddd/*.ddd.json`, including `ddd/app/app.ddd.json` |
| `phero-driver` `ddd_machine_golden` | `tests/conformance/ddd/payrun.ddd.json`, `payrun.xstate.json` |
| `phero-driver` `ddd_machine_refs` | `tests/conformance/ddd/payrun-ref.ddd.json` |
| `phero-driver` `evt_golden` | `tests/conformance/events/orders.evt.json` |
| `phero-driver` `deploy_golden` | `tests/conformance/deploy/svc.deploy.json` |
| `phero-driver` `usecase_golden` | `tests/conformance/usecases/delivery.uc.json` |
| `phero-driver` `intent_golden` | `tests/conformance/intents/delivery.intent.json` |
| `phero-driver` `cmmn_golden` | `tests/conformance/cmmn/{lending,decision}.cmmn.json` |
| `phero-driver` `dmn_golden` | `tests/conformance/dmn/*.dmn.json` |
| `phero-driver` `sigil_golden` | `tests/conformance/sigil/{basic,funcs}.mox.json` |
| `sigil-cli` `conformance` | `tests/rune/conformance/**/expected.json` (also `UPDATE_EXPECT=1`); the CDM snapshot `tests/rune/cdm/master-snapshot.json` regenerates only with `UPDATE_CDM_SNAPSHOT=1` and a fetched corpus |
| `phero-driver` `examples_artifacts` | `examples/artifacts/*.{evt,ddd,uc,deploy,cmmn,dmn,intent}.json` |
| `phero-driver` `graph_analytics` | `tests/conformance/graph/app.graph.json`, `tests/conformance/graph/queries/*.json` |
| `phero-backend-cedar` `golden` | `tests/conformance/cedar/Support.cedar`, `Support.cedarschema.json` |
| `phero-backend-jsonschema` `golden_api` / `golden_wire` | `tests/conformance/schemas/*.api.schema.json` / `*.wire.schema.json` |
| `phero-backend-rust` `codegen_end_to_end` | `tests/conformance/instances/library.instance.json` (plus the scratch crate under `target/scratch/`) |
| `phero-backend-rust` scratch `example_assertions` | `examples/instances/*.instance.json` |
| `phero-backend-cmmn` `generate` | `tests/conformance/cmmn/lending.case.json` |
| `phero-backend-cmmn` `examples` | `examples/artifacts/support.case.json` |
| `phero-backend-dmn` `examples` | `examples/artifacts/support.decision.json` |
| `phero-cli` `cli/graph` | `examples/artifacts/examples.graph.json`, `tests/conformance/graph/queries/examples.*.json` |
| `phero-cli` `cli/highlight` | `tests/highlight/json/support.*.json` |
| `phero-highlight` `corpus` | `tests/highlight/json/{corpus,fence}.*.json`, `tests/highlight/summary/{corpus,docs-fences}.json` |
| `phero-highlight` `grammar` | `site/packages/highlighting/grammars/*.tmLanguage.json`, `site/packages/highlighting/language-configurations/*.json` |
| `phero-ifml` `golden` | `tests/conformance/ifml/{app,cli}.ifml.json`, `{app,cli}.manifest.json` |
| `phero-ifml` `examples` | `examples/artifacts/*.manifest.json` |
| `phero-ifml` `patterns` | `tests/conformance/ifml/patterns/*.ifml.json` |
| `phero-syntax` `fmt` | `tests/conformance/fmt/*.fmt.*` (text, not JSON) |
| `phero-syntax` `cmmn_file` | `tests/conformance/fmt/cmmn.fmt.cmmn` (also pinned here) |
| `phero-diagnostics` `registry` | `docs/DIAGNOSTICS.md` |
| `phero-verify` `trusted_register` | `docs/VERIFICATION.md` (the block between the generated-register markers) |

## Byte shapes

Measured from the committed files, not assumed. The writers use
`serde_json::to_string_pretty()` plus a bare `std::fs::write`, so JSON shape
is a property of the writer, not of git.

| Class | Shape |
| --- | --- |
| IR / artifact goldens (`tests/conformance/**/*.json`, `examples/artifacts/*.json`) | pretty JSON, **no trailing newline** |
| `tests/conformance/graph/queries/*.json` | pretty JSON **with one trailing newline** — deliberately, so the golden matches what `pherolang graph query` prints |
| `tests/highlight/json/**` | pretty JSON, no trailing newline |
| `tests/highlight/summary/**` | pretty JSON with one trailing newline |
| `site/packages/highlighting/**/*.json` | pretty JSON with one trailing newline |
| `docs/DIAGNOSTICS.md`, `docs/VERIFICATION.md` | Markdown with one trailing newline |
| `tests/conformance/fmt/*.fmt.*` | the formatter's own text output, which ends with one newline |
| `tests/rune/conformance/**/expected.json` | sigil's canonical model JSON (`sigil_resolve::canonical_json`), pretty, with one trailing newline |

Two rules follow:

- **Key order** is not the serializer's to decide. `serde_json::Map` is a
  `BTreeMap` unless some crate in the build graph enables `preserve_order`;
  `cedar-policy` (a dev-dependency of `phero-backend-cedar`) does, so a
  workspace-wide build emits insertion order while a narrow `-p` build emits
  alphabetical order. Emitters that care run `canonical_key_order`
  (`phero-backend-common`); goldens that don't care are still byte-compared,
  which is why the regeneration run must be workspace-wide.
- **Hand-authored fixtures are not writer-owned.** `examples/instances/{iot,pricing,shapes}.instance.json`
  and the schema/vocabulary inputs under `tests/conformance/models/` are
  written by a person; the writers never touch them, and they must not be
  normalized to a writer's shape.

## Traps

1. **Fail-fast hides unwritten goldens.** Without `--no-fail-fast`, one stale
   golden in an early-alphabet crate stops later test binaries from being
   scheduled, so the writers that needed to run never do and the failure
   points at a symptom. The script passes the flag.
2. **A narrow run changes bytes.** `cargo test -p <crate>` unifies fewer
   features than `--workspace`; under `preserve_order` that rewrites key
   order in JSON goldens. The committed shape is the workspace one.
3. **`PHERO_UPDATE_FIXTURES` is not uniformly truthy-tested.** Most writers
   check `is_ok()` (any value, including `0`, regenerates); the
   `phero-backend-dmn` examples writer checks `== "1"`. Always set
   `PHERO_UPDATE_FIXTURES=1`.
4. **Not every golden has a writer.** The black-box CLI comparisons read
   committed goldens and have no update branch; the receipt
   (`examples/artifacts/support.receipt.json`) is written by
   `pherolang verify -o …`, not by the env var. Those are regenerated by
   their owning command, and `scripts/ci-local.sh` checks freshness.

## Release-time provenance

`pheroVersion` is part of the artifact bytes, so a version bump invalidates
every artifact golden and the verification receipt. That regeneration is a
release step, and the exact sequence lives in
[RELEASE-0.4.0.md](RELEASE-0.4.0.md): bump the workspace version, run
`scripts/regen-goldens.sh`, regenerate the receipt with the `pherolang verify`
command `scripts/ci-local.sh receipt` prints on failure, then run the full
gate and read every diff.
