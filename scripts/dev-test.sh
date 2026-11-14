#!/bin/bash
# `cargo test` with the CI env — the cheap local path (issue #261).
#
# A bare `cargo test --workspace` compiles with the workspace default
# debuginfo=2 and incremental compilation on, and cargo never collects the
# old hashes under target/debug/build/<pkg>/<hash>/out/. Repeated worktree
# builds have reached 100+ GB that way, and at 100% disk a doctest link dies
# with `ld terminated with signal 7 [Bus error]` (the #87-class failure,
# disk-induced). This wrapper exports exactly what scripts/ci-local.sh
# exports — RUSTFLAGS="-C debuginfo=0", CARGO_INCREMENTAL=0,
# TMPDIR=$PWD/target/tmp (the tmpfs-backed doctest remedy), --jobs 2 — while
# leaving Cargo.toml profiles untouched (debugging the compiler with debug=2
# is worth keeping; see the AGENTS.md operational bullet).
#
# Usage: scripts/dev-test.sh [cargo test args...]
#   scripts/dev-test.sh                                  # the workspace
#   scripts/dev-test.sh -p phero-driver --test navigation   # one suite
#   PHERO_UPDATE_FIXTURES=1 scripts/dev-test.sh             # regenerate goldens
set -u
cd "$(dirname "$0")/.." || exit 1
export CARGO_INCREMENTAL=0
export RUSTFLAGS="-C debuginfo=0"
export TMPDIR="$PWD/target/tmp"
mkdir -p "$TMPDIR"

# Cap the parallel jobs like CI does, unless the caller chose their own.
jobs=(-j 2)
for arg in "$@"; do
  case "$arg" in
    -j | --jobs | -j[0-9]* | --jobs=*) jobs=() ;;
  esac
done

exec cargo test "${jobs[@]}" "$@"
