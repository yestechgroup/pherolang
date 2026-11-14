#!/bin/bash
# Local replication of .github/workflows/ci.yml — the CI gates, exactly,
# before pushing. Run this until every step reports *-OK; only then push
# (GitHub Actions credits are not a test bench).
#
# This script is the SINGLE SOURCE OF TRUTH for the gate sequence and the
# fixture-glob list: ci.yml's steps invoke the subcommands below, so local
# and remote can never drift (issue #121). Adding a fixture directory or
# surface means editing FIXTURE_FORMAT_GLOBS here — in exactly one file.
#
# Mirrors CI's env: CARGO_INCREMENTAL=0 and RUSTFLAGS="-C debuginfo=0"
# (the dsntk disk-budget tweak, workflow env block), and the Tests step
# runs under TMPDIR=$PWD/target/tmp with --jobs 2 (the doctest SIGBUS
# remedy — the AGENTS.md testing-quirks gotcha). Small-machine guardrails:
# nice, orphaned rustc/lld killed between steps, plus the informational
# disk-budget print/warning before the build (issue #261). On a beefier
# machine you can drop the nice and the job cap.
#
# Usage: scripts/ci-local.sh [all|build|rebrand|test|clippy|fmt|receipt|docs|audit]
#        (default: all — the full sequence; ~30-60 min cold)
set -u
cd "$(dirname "$0")/.." || exit 1
mkdir -p target/tmp
export TMPDIR="$PWD/target/tmp"
export CARGO_TERM_COLOR=always
export CARGO_INCREMENTAL=0
export RUSTFLAGS="-C debuginfo=0"

step() { echo; echo "=== $1 ==="; }
kill_orphans() {
  pkill -f "[r]ustc" 2>/dev/null
  pkill -f "[r]ust-lld" 2>/dev/null
  sleep 1
}

# Disk budget (issue #261): cargo never collects the old hashes under
# target/debug/build/<pkg>/<hash>/out/, and with the workspace default
# debuginfo=2 one worktree has reached 100+ GB. When the filesystem fills,
# doctest linking dies with `ld terminated with signal 7 [Bus error]` — the
# #87-class failure, disk-induced (see the AGENTS.md operational bullet).
# Informational only: this warns, and never changes a gate's pass/fail
# semantics. Override the floor with PHERO_DISK_MIN_GB.
PHERO_DISK_MIN_GB="${PHERO_DISK_MIN_GB:-20}"
case "$PHERO_DISK_MIN_GB" in
  '' | *[!0-9]*) PHERO_DISK_MIN_GB=20 ;;
esac
disk_budget() {
  echo "--- disk budget (warn only; floor ${PHERO_DISK_MIN_GB} GB, PHERO_DISK_MIN_GB) ---"
  df -h .
  if [ -d target ]; then
    du -sh target
  else
    echo "target: absent (cold build)"
  fi
  free_gb=$(df -Pk . | awk 'NR == 2 { printf "%d", $4 / 1024 / 1024 }')
  if [ "${free_gb:-0}" -lt "$PHERO_DISK_MIN_GB" ]; then
    echo "WARNING: ${free_gb} GB free on this filesystem, below the ${PHERO_DISK_MIN_GB} GB floor."
    echo "  Reclaim safely (workspace crates only; external deps survive):"
    echo "    rm -rf target/debug/build/phero-*"
    echo "  Never delete target/debug/build wholesale: it is the live build cache."
  fi
}

# The `pherolang fmt --check` fixture gate — the one copy of the list.
#
# Directory arguments are used where the gate set is provably identical to
# the expansion (examples/, patterns/ifml/, and the conformance surface
# directories whose only model files are the gated ones — the formatter-
# golden `X.fmt.<ext>` skip is built into directory mode). Deliberate
# globs remain where a directory would WIDEN the gate:
# - tests/conformance/models/: holds the formatter's non-canonical golden
#   sources (models/events.evt, models/deploy.deploy, ...) next to the two
#   gated canonical files — a directory scan here would fail by design.
# - tests/conformance/{ddd,usecases,cmmn,dmn}/: the imported companion
#   domains (.mox/.actor/.evt) are committed canonically but are not part
#   of today's gate; widening is a deliberate future PR, not a side effect.
# - tests/conformance/ifml/phero/: decisions.dmn is not currently gated;
#   gate app/cli/auth explicitly.
FIXTURE_FORMAT_GLOBS=(
  examples
  patterns/ifml
  tests/conformance/events
  tests/conformance/deploy
  tests/conformance/graph
  tests/conformance/sigil
  tests/conformance/models/*.mox
  tests/conformance/models/*.actor
  tests/conformance/ddd/*.ddd
  tests/conformance/usecases/*.uc
  tests/conformance/cmmn/*.cmmn
  tests/conformance/dmn/*.dmn
  tests/conformance/ifml/app.ifml
  tests/conformance/ifml/cli.ifml
  tests/conformance/ifml/phero/auth.mox
  tests/conformance/ifml/phero/auth.actor
)

step_build() {
  step "CI 1/7: cargo build --workspace --all-targets"
  disk_budget
  nice -n 19 cargo build --workspace --all-targets && echo "BUILD-OK" || echo "BUILD-FAIL"
  kill_orphans
}

# The rebrand guard (epic #270, task P0-03): fail on a revived pre-0.3
# spelling. It runs early and cheaply — before the expensive test step —
# because a reintroduced old name is a documentation/identifier regression
# that no other gate can see. The contract it enforces is docs/NAMING.md.
step_rebrand() {
  step "CI 2/7: rebrand guard (scripts/check-rebrand.sh)"
  ./scripts/check-rebrand.sh && echo "REBRAND-STEP-OK" || echo "REBRAND-STEP-FAIL"
}

step_test() {
  step "CI 3/7: TMPDIR=\$PWD/target/tmp cargo test --workspace --jobs 2"
  # Issue #261: the test step recreates target/tmp, so clear the previous
  # run's leftovers first (doctest linking writes there; a stale pile of
  # scratch artifacts is exactly the disk pressure this budgets).
  rm -rf target/tmp/* 2>/dev/null
  mkdir -p target/tmp
  test_log=target/tmp/ci-local-tests.log
  if nice -n 19 cargo test --workspace --jobs 2 > "$test_log" 2>&1; then
    grep "test result:" "$test_log" | tail -20
    echo "TESTS-OK"
  else
    grep -E "test result:|FAILED|panicked at|error\[|^error" "$test_log" | head -80
    echo "TESTS-FAIL (full output: $test_log)"
  fi
  kill_orphans
}

step_clippy() {
  step "CI 4/7: clippy -D warnings"
  nice -n 19 cargo clippy --workspace --all-targets -- -D warnings && echo "CLIPPY-OK" || echo "CLIPPY-FAIL"
  kill_orphans
}

step_fmt() {
  step "CI 5/7: format check"
  nice -n 19 cargo fmt --all -- --check && echo "RUSTFMT-OK" || echo "RUSTFMT-FAIL"
  nice -n 19 cargo run -q -p phero-cli -- fmt --check "${FIXTURE_FORMAT_GLOBS[@]}" \
    && echo "PHEROFMT-OK" || echo "PHEROFMT-FAIL"
  kill_orphans
}

# The receipt freshness gate (issue #207): regenerate the flagship receipt
# from the sources it names and byte-compare with the committed file. The
# receipt is a golden — a source, artifact, or register change without
# regeneration is red here, with the exact command to run (the same
# regenerate-and-compare discipline as the fmt fixture gate).
step_receipt() {
  step "CI 6/7: receipt freshness (pherolang verify --check-fresh)"
  if nice -n 19 cargo run -q -p phero-cli -- verify --check-fresh \
      examples/artifacts/support.receipt.json; then
    echo "RECEIPT-OK"
  else
    echo "RECEIPT-FAIL"
    echo "Regenerate the flagship receipt (and read the diff — it is the spec):"
    echo "  cargo run -q -p phero-cli -- verify \\"
    echo "    examples/support/support.actor examples/support/support.cmmn \\"
    echo "    examples/support/support.ddd examples/support/support.deploy \\"
    echo "    examples/support/support.dmn examples/support/support.evt \\"
    echo "    examples/support/support.ifml examples/support/support.mox \\"
    echo "    examples/support/support.uc \\"
    echo "    -o examples/artifacts/support.receipt.json"
  fi
  kill_orphans
}

step_docs() {
  # The strict rustdoc gate: broken intra-doc links are warnings by
  # default; -D warnings makes them errors (issue #175). cargo doc runs no
  # doctests, but it inherits the script's TMPDIR export like every step.
  step "CI 7/7: RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps"
  nice -n 19 env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps \
    && echo "DOC-OK" || echo "DOC-FAIL"
  kill_orphans
}

# Optional subcommand: the advisory/license/supply-chain gate that CI runs
# as separate fast jobs (`audit` + `deny`), locally in one shot.
if [ "${1:-}" = "audit" ]; then
  step "audit: cargo deny check + cargo audit"
  nice -n 19 cargo deny check && echo "DENY-OK" || echo "DENY-FAIL"
  nice -n 19 cargo audit && echo "AUDIT-OK" || echo "AUDIT-FAIL"
  exit 0
fi

case "${1:-all}" in
  build)  step_build ;;
  rebrand) step_rebrand ;;
  test)   step_test ;;
  clippy) step_clippy ;;
  fmt)    step_fmt ;;
  receipt) step_receipt ;;
  docs)   step_docs ;;
  all)
    step_build
    step_rebrand
    step_test
    step_clippy
    step_fmt
    step_receipt
    step_docs
    echo
    echo "ALL SEVEN STEPS MUST END IN *-OK BEFORE PUSHING."
    ;;
  *)
    echo "usage: scripts/ci-local.sh [all|build|rebrand|test|clippy|fmt|receipt|docs|audit]" >&2
    exit 2
    ;;
esac
echo "CI-LOCAL-DONE"
