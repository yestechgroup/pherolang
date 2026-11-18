#!/bin/bash
# The CI environment block, in exactly one place.
#
# `scripts/ci-local.sh` (the gate) and `scripts/regen-goldens.sh` (golden
# regeneration) must run under the same environment, or a regenerated golden
# is a different byte shape from the one the gate compares (see the
# `preserve_order` note in docs/GOLDENS.md). Sourcing this file also enters
# the repository root, so a caller may live anywhere.
#
# Mirrors ci.yml's env block — CARGO_INCREMENTAL=0 and
# RUSTFLAGS="-C debuginfo=0" (the dsntk disk-budget tweak) — and points
# TMPDIR at disk-backed target/tmp (the doctest SIGBUS remedy; the
# AGENTS.md testing-quirks gotcha).
#
# Usage (bash only): . "$(dirname "$0")/ci-env.sh"
set -u
cd "$(dirname "${BASH_SOURCE[0]}")/.." || return 1
mkdir -p target/tmp
export TMPDIR="$PWD/target/tmp"
export CARGO_TERM_COLOR=always
export CARGO_INCREMENTAL=0
export RUSTFLAGS="-C debuginfo=0"
