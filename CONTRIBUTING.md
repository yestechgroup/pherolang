# Contributing

Bug reports, failing test cases, documentation fixes, and new backends are
all welcome. This page is the path from a clone to a merged pull request.

## Prerequisites

- **Linux x86_64.** `x86_64-unknown-linux-gnu` (glibc) is the supported
  target, and `ubuntu-latest` is the only runner the gate executes on —
  Windows, macOS, and Linux aarch64 are unsupported and untested. See
  [docs/adr/0004-supported-platforms-linux-only.md](docs/adr/0004-supported-platforms-linux-only.md).
- A stable Rust toolchain, installed through [rustup](https://rustup.rs).
- `git`.

The dependencies pinned by git revision — the Rune DSL toolchain among them
— are public repositories fetched automatically by `cargo`. There is no
vendoring step and no private registry.

## Build and run

```sh
cargo build --workspace --all-targets
cargo run -p phero-cli -- check tests/conformance/
```

The second command checks the conformance corpus: one snapshot of every
supported surface, each file checked with its imports resolved from that
snapshot, then the artifacts clustered into applications and the
cross-layer wiring diagnosed. It is the quickest end-to-end smoke test.

## The gate

Before you open a pull request, run:

```sh
scripts/ci-local.sh
```

It runs seven steps — build, rebrand, test, clippy, fmt, receipt, docs — and
every step must report `*-OK`. One `*-FAIL` is a real failure. CI invokes
this same script, so a green local run and a green CI run are the same gate.
The step sequence and the list of files the format gate covers live only in
`scripts/ci-local.sh`: do not copy them into a workflow or a document.

Two further entry points:

- `scripts/ci-local.sh audit` — the supply-chain gates (`cargo deny check`
  and `cargo audit`). CI runs them as separate jobs and both must stay
  clean.
- `scripts/dev-test.sh [args…]` — a plain `cargo test` under the CI
  environment (no debug info, no incremental compilation, a disk-backed
  `TMPDIR`, `--jobs 2`). Prefer it to a bare `cargo test --workspace`, which
  leaves debug info on and grows `target/` fast enough to fill a disk.

Golden files are byte-compared. Regenerate them with:

```sh
PHERO_UPDATE_FIXTURES=1 cargo test --workspace
```

Then read the diff. Goldens are the specification: an unexplained byte
change is either the point of your change or a bug in it, and you have to
know which. Never commit a regenerated golden you have not read; a
hand-edited golden is a review reject.

## What a pull request needs

- A focused diff: one change, its tests, and the goldens it moves.
- A test for any behaviour change. Fixing a bug means a test that fails
  before the fix and passes after it.
- No hand-edited goldens and no unexplained golden churn.
- `scripts/check-rebrand.sh` green. The spelling contract for the product
  and every identifier that carries its name is
  [docs/NAMING.md](docs/NAMING.md); the guard fails on a revived old-name
  token outside its allowlist, so a new legitimate occurrence is an
  allowlist row with a reason, not a wordlist patch.
- A description that says what changed, why, and how you verified it.

## Where the documentation lives

- [AGENTS.md](AGENTS.md) — the compiler's invariants, crate ownership,
  commands, and the traps that cost people a day.
- [docs/README.md](docs/README.md) — the index; every document is one hop
  from here.
- [docs/LANGUAGE.md](docs/LANGUAGE.md) — the language reference. The
  grammar is stated once, in the parser module docs, and the reference
  links to it rather than restating it.

## Fork pull requests and the site

The `site` deployment workflow (`.github/workflows/site.yml`) never deploys
from a fork, because secrets are not available to fork builds. A pull
request from a fork gets the verify job only. Deployments come from
same-repository branches, so a site change proposed from a fork needs a
maintainer to bring the branch into the repository before it can ship.

## Code of conduct and security

Participation is governed by [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

Do not report a vulnerability in an issue or a pull request. The private
channels and the scope are in [SECURITY.md](SECURITY.md).
