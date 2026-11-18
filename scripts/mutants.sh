#!/bin/bash
# The #208 anti-vacuity mutant harness.
#
# The contract: a row with `"status": "tested"` is only worth what its own
# pinned tests can falsify. So each tested row carries exactly one
# `evidence` entry of `"kind": "mutant"` — a recorded patch that breaks the
# checking code the row is about — and the row's own `"kind": "test"`
# pointers must kill it. A mutant that survives its row's tests means the
# tests are green but vacuous: the run is red and names the row
# (MUTANT-SURVIVED), exactly as loudly as a genuine kill is quiet.
#
# This harness never invents tests: it runs only the pointers the register
# records for that row. It never leaves the tree mutated: every path out of
# a row (kill, survive, build failure, SIGINT/TERM) restores the tree, and
# a leftover change is a hard failure, not a warning. The harness refuses a
# dirty start for the same reason scripts/regen-goldens.sh --check does —
# with the caller's own edits in the tree, "restored" is not a fact.
#
# Data source: docs/verification/register.json (override with
# PHERO_REGISTER=<path>, so the harness can be exercised against a fixture
# copy). Parsed with jq. Rows are processed in id order.
#
# Pointer -> command mapping (all paths are repo-relative):
#
#   crates/<pkg>/tests/<X>.rs               -> cargo test -p <pkg> --test <X>
#   crates/<pkg>/tests/<A>/<B>.rs           -> cargo test -p <pkg> --test <A> <B>
#     (a nested file is a module of the sibling tests/<A>/main.rs target, so
#      <A> names the cargo target and <B> is the module-path substring filter;
#      no tests/<A>/main.rs means there is no cargo test target at all)
#   crates/<pkg>/src/.../<F>.rs[:symbol]    -> cargo test -p <pkg> <symbol>
#                                              cargo test -p <pkg> <F> without a symbol
#     (cargo filters on the test's path, which embeds the source module stem)
#   anything else (docs, JSON goldens, tests/conformance/...) -> not runnable
#
# A `:symbol` suffix on a `tests/...` pointer becomes the filter argument
# after `--test <target>` — the register already spells test pointers that
# way (e.g. `crates/phero-verify/tests/invariants.rs:p11_...`). Skipped
# pointers are always printed, never silently dropped; a row with zero
# runnable test pointers is a hard failure naming the row.
#
# Per row:
#   1. git apply --check -> PATCH-ROTTEN <row> when the patch no longer applies
#   2. git apply        -> the reverse is installed on EXIT/INT/TERM
#   3. cargo test --no-run -p <packages> -> a build failure is MUTANT-BROKEN
#      <row> (never a kill): the mutation must compile, or it proves nothing
#   4. the row's commands in order; the first non-zero exit is the kill
#      (MUTANT-KILLED <row> naming the command); all zero is MUTANT-SURVIVED
#   5. revert, then require `git status --porcelain` to be empty
#
# A green kill is only meaningful if the row's tests were green *before* the
# patch: the baseline is `scripts/ci-local.sh test` (the seven-step gate),
# so run this after the gate, not instead of it.
#
# Usage: scripts/mutants.sh            # run every tested row
#        scripts/mutants.sh --list     # print the mapping; no build, no mutation
#
# Exit codes: 0 every tested row printed MUTANT-KILLED; 1 a survivor, a broken
# mutation, a rotten patch, a missing mutant pointer, or a row with no
# runnable test pointer; 2 usage.
set -euo pipefail
. "$(dirname "$0")/ci-env.sh" || exit 1

REGISTER="${PHERO_REGISTER:-docs/verification/register.json}"

# Per-row state, filled by load_rows(). Parallel indexed arrays (not
# associative ones) keep this runnable on the oldest bash the repo supports.
row_ids=()
row_patch=()
row_mutant_count=()
row_tests=()   # every kind=="test" pointer for the row, newline-joined

# Non-empty while a patch is applied, so the EXIT trap knows to restore.
applied_patch=""
failure_lines=""

usage() {
  cat >&2 <<'EOF'
usage: scripts/mutants.sh [--list]

  (no arguments)  apply each tested row's recorded mutant, run only that row's
                  pinned tests, revert; a survivor is red and names the row
  --list          print row -> patch -> package(s) -> command(s) and the
                  skipped pointers; touches neither the tree nor a build

Register: docs/verification/register.json (override with PHERO_REGISTER).
Exit codes: 0 every row killed; 1 any survivor/broken/rotten/unusable row;
            2 usage.
EOF
}

# ---------------------------------------------------------------------------
# Tree hygiene
# ---------------------------------------------------------------------------

require_clean_tree() {
  if [ -n "$(git status --porcelain)" ]; then
    echo "MUTANTS-FAIL: the working tree is dirty before the run." >&2
    echo "The harness applies recorded patches to this tree; a modified file" >&2
    echo "would make 'restored' unverifiable." >&2
    echo "Commit or stash first." >&2
    git status --short >&2
    exit 1
  fi
}

# Paths a patch touches. `--numstat` field 3 is the new path for ordinary
# hunks; this is the belt-and-braces path only (a rename hunk spells its
# field 3 as a description, which is why the reverse-apply is the real path).
patch_paths() {
  local patch="$1"
  git apply --numstat "$patch" 2>/dev/null | awk -F'\t' '{ print $3 }'
}

# Reverse the applied patch. The reverse is the normal path; the fallback
# exists so a corrupted/skipped revert can never leave mutated code behind.
revert_patch() {
  local patch="$applied_patch"
  [ -n "$patch" ] || return 0
  applied_patch=""

  if git apply -R "$patch" 2>/dev/null; then
    return 0
  fi

  echo "MUTANTS-WARN: reverse-applying $patch failed; restoring its paths by hand." >&2
  local paths=()
  local path
  while IFS= read -r path; do
    if [ -n "$path" ]; then
      paths+=("$path")
    fi
  done < <(patch_paths "$patch")

  if [ "${#paths[@]}" -gt 0 ]; then
    # Tracked paths come back from the index; a path the patch created is
    # untracked, so clean only those exact paths (scoped, never `-x`, and
    # the tree started clean by contract).
    git checkout -- "${paths[@]}" 2>/dev/null || true
    git clean -fd -- "${paths[@]}" >/dev/null 2>&1 || true
  fi
}

on_exit() {
  local status=$?
  revert_patch || true
  exit "$status"
}

assert_tree_restored() {
  local id="$1"
  if [ -n "$(git status --porcelain)" ]; then
    echo "MUTANTS-FAIL: the tree is still dirty after reverting $id." >&2
    git status --short >&2
    exit 1
  fi
}

# ---------------------------------------------------------------------------
# Register -> rows
# ---------------------------------------------------------------------------

load_rows() {
  if [ ! -f "$REGISTER" ]; then
    echo "MUTANTS-FAIL: register '$REGISTER' not found (PHERO_REGISTER overrides the default)." >&2
    exit 1
  fi
  if ! command -v jq >/dev/null 2>&1; then
    echo "MUTANTS-FAIL: jq is required to read $REGISTER." >&2
    exit 1
  fi

  local kind id a b last
  while IFS=$'\t' read -r kind id a b; do
    case "$kind" in
      ROW)
        row_ids+=("$id")
        row_mutant_count+=("$a")
        row_patch+=("$b")
        row_tests+=("")
        ;;
      TEST)
        if [ "${#row_ids[@]}" -gt 0 ]; then
          last=$((${#row_ids[@]} - 1))
          row_tests[last]="${row_tests[last]}$a"$'\n'
        fi
        ;;
    esac
  done < <(
    jq -r '
      (.obligations // [])
      | map(select(.status == "tested"))
      | sort_by([(.id | sub("^[^0-9]*"; "") | tonumber? // 0), .id])
      | .[] as $row
      | (["ROW", $row.id,
          (($row.evidence // []) | map(select(.kind == "mutant")) | length | tostring),
          (($row.evidence // []) | map(select(.kind == "mutant") | .pointer) | join(", "))]
         | @tsv),
        (($row.evidence // [])[]
         | select(.kind == "test")
         | ["TEST", $row.id, .pointer] | @tsv)
    ' "$REGISTER"
  )
}

# ---------------------------------------------------------------------------
# Pointer -> command
# ---------------------------------------------------------------------------

# Sets:
#   MAP_STATUS  "cmd" or "skip"
#   MAP_PKG     the crate name when the pointer is inside crates/<pkg>/
#   MAP_CMD     the command line to run (single spaces, no quoting needed:
#               package names, target names and test symbols are bare tokens)
#   MAP_SKIP    the reason, for an honest `skip (...)` line
map_pointer() {
  local pointer="$1"
  local path="$pointer"
  local symbol=""
  MAP_STATUS="skip"
  MAP_PKG=""
  MAP_CMD=""
  MAP_SKIP="not a cargo test"

  if [[ "$pointer" == *:* ]]; then
    path="${pointer%%:*}"
    symbol="${pointer#*:}"
  fi

  if [[ "$path" != crates/*/* ]]; then
    MAP_SKIP="not a cargo test (no crates/<pkg>/ prefix)"
    return 0
  fi

  local rest="${path#crates/}"
  local pkg="${rest%%/*}"
  local inside="${rest#*/}"
  MAP_PKG="$pkg"

  case "$inside" in
    tests/*)
      local tail="${inside#tests/}"
      if [[ "$tail" == */* ]]; then
        # Nested: tests/<A>/<B>.rs is a module of the tests/<A>/main.rs target.
        local target="${tail%%/*}"
        local sub="${tail#*/}"
        if [ ! -f "crates/$pkg/tests/$target/main.rs" ]; then
          MAP_SKIP="no cargo test target (tests/$target/main.rs is absent)"
          return 0
        fi
        local filter="${sub%.rs}"
        filter="${filter//\//::}"
        MAP_STATUS="cmd"
        MAP_CMD="cargo test -p $pkg --test $target $filter"
      else
        local target="${tail%.rs}"
        MAP_STATUS="cmd"
        MAP_CMD="cargo test -p $pkg --test $target"
      fi
      if [ -n "$symbol" ]; then
        MAP_CMD="$MAP_CMD $symbol"
      fi
      ;;
    src/*)
      local filter="$symbol"
      if [ -z "$filter" ]; then
        filter="${inside##*/}"
        filter="${filter%.rs}"
      fi
      MAP_STATUS="cmd"
      MAP_CMD="cargo test -p $pkg $filter"
      ;;
    *)
      MAP_SKIP="not a cargo test"
      ;;
  esac
}

# ---------------------------------------------------------------------------
# --list: the mapping, without touching the tree
# ---------------------------------------------------------------------------

list_rows() {
  echo "=== mutants: --list (no tree changes, no build) ==="
  echo "register: $REGISTER"
  echo "tested rows: ${#row_ids[@]}"

  local missing_patches=0
  local i id patch packages commands skips pointer pkg note
  for i in "${!row_ids[@]}"; do
    id="${row_ids[$i]}"
    patch="${row_patch[$i]}"
    packages=""
    commands=""
    skips=""

    while IFS= read -r pointer; do
      [ -n "$pointer" ] || continue
      map_pointer "$pointer"
      if [ "$MAP_STATUS" = "cmd" ]; then
        pkg="$MAP_PKG"
        case " $packages " in
          *" $pkg "*) ;;
          *) packages="${packages:+$packages }$pkg" ;;
        esac
        commands="${commands}    cmd: $MAP_CMD"$'\n'
      else
        skips="${skips}    skip (not a cargo test): $pointer  [$MAP_SKIP]"$'\n'
      fi
    done <<< "${row_tests[$i]}"

    echo
    if [ "${row_mutant_count[$i]}" -eq 1 ]; then
      echo "$id -> $patch -> ${packages:-<none>}"
      note=""
    else
      missing_patches=$((missing_patches + 1))
      echo "$id -> <no mutant pointer> -> ${packages:-<none>}"
      note="    note: expected exactly one evidence entry with kind \"mutant\"; found ${row_mutant_count[$i]}"
    fi
    [ -n "$commands" ] && printf '%s' "$commands"
    [ -n "$skips" ] && printf '%s' "$skips"
    [ -n "$note" ] && printf '%s\n' "$note"
  done

  echo
  if [ "$missing_patches" -gt 0 ]; then
    echo "MUTANTS-NOTE: $missing_patches tested row(s) carry no \"mutant\" evidence pointer yet."
    echo "  This harness is plumbing until #208's recorded patches land; --list shows the"
    echo "  rows it can see and the mapping it would run. A real run fails those rows by name."
  fi
  echo "MUTANTS-LIST-OK"
}

# ---------------------------------------------------------------------------
# The run
# ---------------------------------------------------------------------------

run_rows() {
  local total="${#row_ids[@]}"
  local killed=0 survived=0 broken=0 rotten=0 unusable=0
  local i id patch pointer

  echo "=== mutants: $REGISTER ($total tested rows) ==="
  for i in "${!row_ids[@]}"; do
    id="${row_ids[$i]}"
    patch="${row_patch[$i]}"
    echo
    echo "--- $id ---"

    if [ "${row_mutant_count[$i]}" -ne 1 ]; then
      echo "MUTANT-NO-PATCH $id: expected exactly one evidence entry of kind \"mutant\", found ${row_mutant_count[$i]}."
      unusable=$((unusable + 1))
      failure_lines="${failure_lines}  - $id: no single mutant pointer"$'\n'
      continue
    fi

    local packages=()
    local commands=()
    while IFS= read -r pointer; do
      [ -n "$pointer" ] || continue
      map_pointer "$pointer"
      if [ "$MAP_STATUS" = "cmd" ]; then
        commands+=("$MAP_CMD")
        local seen=0 pkg
        for pkg in ${packages[@]+"${packages[@]}"}; do
          if [ "$pkg" = "$MAP_PKG" ]; then
            seen=1
          fi
        done
        if [ "$seen" -eq 0 ]; then
          packages+=("$MAP_PKG")
        fi
      else
        echo "  skip (not a cargo test): $pointer  [$MAP_SKIP]"
      fi
    done <<< "${row_tests[$i]}"

    if [ "${#commands[@]}" -eq 0 ]; then
      echo "MUTANT-UNUSABLE $id: no runnable cargo test pointer (every test pointer was skipped)."
      unusable=$((unusable + 1))
      failure_lines="${failure_lines}  - $id: no runnable test pointer"$'\n'
      continue
    fi

    if [ ! -f "$patch" ]; then
      echo "PATCH-ROTTEN $id: mutant patch '$patch' does not exist."
      rotten=$((rotten + 1))
      failure_lines="${failure_lines}  - $id: patch file missing ($patch)"$'\n'
      continue
    fi
    if ! git apply --check "$patch"; then
      echo "PATCH-ROTTEN $id: $patch does not apply to this tree — regenerate the mutant."
      rotten=$((rotten + 1))
      failure_lines="${failure_lines}  - $id: patch rotten ($patch)"$'\n'
      continue
    fi

    git apply "$patch"
    applied_patch="$patch"

    local build=(cargo test --no-run)
    local pkg
    for pkg in "${packages[@]}"; do
      build+=(-p "$pkg")
    done
    echo "  build: ${build[*]}"
    if ! "${build[@]}"; then
      echo "MUTANT-BROKEN $id: the mutated tree does not build — not a kill."
      revert_patch
      assert_tree_restored "$id"
      broken=$((broken + 1))
      failure_lines="${failure_lines}  - $id: mutated tree did not build"$'\n'
      continue
    fi

    local killed_by="" cmd
    local argv=()
    for cmd in "${commands[@]}"; do
      echo "  run: $cmd"
      read -r -a argv <<< "$cmd"
      if ! "${argv[@]}"; then
        killed_by="$cmd"
        break
      fi
    done

    if [ -n "$killed_by" ]; then
      echo "MUTANT-KILLED $id ($killed_by)"
      killed=$((killed + 1))
    else
      echo "MUTANT-SURVIVED $id (${commands[$((${#commands[@]} - 1))]})"
      survived=$((survived + 1))
      failure_lines="${failure_lines}  - $id: survived every pinned test"$'\n'
    fi

    revert_patch
    assert_tree_restored "$id"
  done

  echo
  echo "=== mutants summary: $total tested rows — $killed killed, $survived survived, $broken broken, $rotten rotten, $unusable unusable ==="
  if [ -n "$failure_lines" ]; then
    echo "Failures:"
    printf '%s' "$failure_lines"
    echo "MUTANTS-FAIL: the anti-vacuity run is red — see the row failures above."
    echo "A survivor means the row's tests are green but vacuous (fix the mutant or"
    echo "the tests); a broken mutation, rotten patch, or unusable row means the"
    echo "recorded mutant itself needs fixing."
    exit 1
  fi
  echo "MUTANTS-OK: every tested row's recorded mutant was killed by its own pinned tests."
}

main() {
  if [ "$#" -eq 0 ]; then
    require_clean_tree
    load_rows
    run_rows
    return 0
  fi

  case "$1" in
    --list)
      if [ "$#" -ne 1 ]; then
        usage
        exit 2
      fi
      load_rows
      list_rows
      ;;
    *)
      usage
      exit 2
      ;;
  esac
}

trap on_exit EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
main "$@"
