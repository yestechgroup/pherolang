#!/bin/bash
# The one golden-regeneration command (issue #223).
#
# `PHERO_UPDATE_FIXTURES=1` makes the golden writers rewrite their fixtures
# instead of comparing them. Two traps make the naive invocation wrong:
#
#   1. `cargo test --workspace` is fail-fast: one stale golden in an early
#      crate stops later test binaries from ever being scheduled, so the
#      writers that actually needed to run silently never do. This script
#      passes `--no-fail-fast`.
#   2. The environment decides the byte shape. A narrow `-p <crate>` run
#      unifies fewer features than `--workspace`, which flips
#      `serde_json::Map` ordering and rewrites key order in every JSON
#      golden (the `preserve_order` trap). This script uses the same
#      environment the gate does, via scripts/ci-env.sh.
#
# Usage: scripts/regen-goldens.sh           # regenerate, then show the diff
#        scripts/regen-goldens.sh --check   # regenerate and fail if the tree
#                                           # is dirty (a freshness check)
#
# Exit codes: 0 clean, 1 the run or the --check verdict failed, 2 usage.
set -euo pipefail
. "$(dirname "$0")/ci-env.sh"

check=0
case "${1:-}" in
  --check) check=1 ;;
  '') ;;
  *)
    echo "usage: scripts/regen-goldens.sh [--check]" >&2
    exit 2
    ;;
esac

# A freshness check is only meaningful from a known-clean starting point:
# otherwise a pre-existing modification is indistinguishable from a stale
# golden. Refusing up front also keeps `git status --short` at the end a
# precise answer instead of a pile of the caller's own edits.
if [ "$check" = 1 ] && [ -n "$(git status --porcelain)" ]; then
  echo "REGEN-CHECK-FAIL: the working tree is dirty before the run." >&2
  echo "Commit or stash first — a freshness check from a dirty tree cannot" >&2
  echo "tell a stale golden from your own edit." >&2
  git status --short >&2
  exit 1
fi

echo "=== regenerating goldens: PHERO_UPDATE_FIXTURES=1 cargo test --workspace --no-fail-fast --jobs 2 ==="
status=0
PHERO_UPDATE_FIXTURES=1 cargo test --workspace --no-fail-fast --jobs 2 || status=$?

echo
echo "=== working tree after regeneration ==="
git status --short

if [ "$status" -ne 0 ]; then
  echo
  echo "REGEN-FAIL: the test run exited $status. Some writers may not have run."
  echo "Read the failures above before trusting the diff."
  exit "$status"
fi

if [ "$check" = 1 ]; then
  if [ -n "$(git status --porcelain)" ]; then
    echo
    echo "REGEN-CHECK-FAIL: goldens are stale — the run changed the working tree."
    exit 1
  fi
  echo
  echo "REGEN-CHECK-OK: every golden matches its writer byte-for-byte."
  exit 0
fi

echo
echo "Read the diff — goldens are the spec (docs/GOLDENS.md):"
echo "  git diff --stat"
echo "  git diff"
