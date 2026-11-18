#!/bin/bash
# The platform guard (ADR 0004, issue pherolang#284).
#
# Phero Lang publishes one platform: x86_64/aarch64 Linux. Windows and macOS
# were deliberately dropped, and the only thing that notices a revived
# instruction is this gate — no build or test would fail if the install page
# grew a PowerShell chain again, because nothing here runs on Windows.
#
# The detector is deliberately blunt, with exactly two shape exclusions so it
# can be blunt without false-positiving on ordinary Rust:
#   * `windows` as a word (capitalised or not) that is NOT preceded by `.` or
#     `:` — that excludes `slice.windows(2)` and `slice::windows(2)` while
#     still catching `windows`, `windows-sys`, `windows_amd64`, `windows-latest`.
#   * `msvc` and `pwsh` anywhere (they only appear in platform/toolchain text).
#   * `.zip` unless immediately followed by `(` — that excludes `Iterator::zip`
#     calls (`.zip(Some(..))`) while catching `.zip` archives and `*.zip` globs.
# The pattern is applied case-insensitively: `Windows`, `WINDOWS` and
# `windows` are the same claim.
#
# Allowlist, and why each entry earns it:
#   Cargo.lock                      transitive crate names (windows-sys,
#                                   windows-link, find-msvc-tools); ADR 0004 P5
#                                   says the dependency graph is not a support
#                                   claim
#   site/pnpm-lock.yaml             the same fact for the JS toolchain
#   README.md / CONTRIBUTING.md     the supported-platform statement itself
#   site .../install.astro          the honest "unsupported (ADR 0004)" prose
#   site .../index.astro            the same statement on the landing page
#   site/scripts/fetch-*.mjs        the named failure that tells a Windows
#                                   contributor the archive does not exist
#   docs/RELEASE-0.3.0.md           history: v0.3.0 shipped a Windows archive
#   docs/adr/0004-*.md, adr/README  the decision record and its index row
#   CHANGELOG.md                    past release notes are history
#   this script                     the detector and this allowlist
#
# A new occurrence is a platform decision, not a wordlist patch: add an
# allowlist row here with its reason, or remove the claim.
#
# Usage: scripts/check-platforms.sh [--allowlist] [--verbose]
#
# Exit codes: 0 clean, 1 platform tokens found, 2 usage error.
set -u
cd "$(dirname "$0")/.." || exit 1

# One pattern, one place. `git grep` sees tracked files only, so build
# output, target/ and .git can neither false-positive nor slow the gate.
PATTERN='(^|[^.A-Za-z0-9_:])windows([^a-z]|$)|msvc|pwsh|\.zip([^A-Za-z0-9_(]|$)'

ALLOWED_PATHS=(
  ':(exclude)Cargo.lock'
  ':(exclude)site/pnpm-lock.yaml'
  ':(exclude)README.md'
  ':(exclude)CONTRIBUTING.md'
  ':(exclude)site/phero-web/src/pages/install.astro'
  ':(exclude)site/phero-web/src/pages/index.astro'
  ':(exclude)site/scripts/fetch-pherolang.mjs'
  ':(exclude)docs/RELEASE-0.3.0.md'
  ':(exclude)docs/adr/0004-supported-platforms-linux-only.md'
  ':(exclude)docs/adr/README.md'
  ':(exclude)CHANGELOG.md'
  ':(exclude)scripts/check-platforms.sh'
)

case "${1:-}" in
  --allowlist)
    printf '%s\n' "${ALLOWED_PATHS[@]}" | sed 's/^:(exclude)//'
    exit 0
    ;;
  --verbose) verbose=1 ;;
  '') verbose=0 ;;
  *)
    echo "usage: scripts/check-platforms.sh [--allowlist] [--verbose]" >&2
    exit 2
    ;;
esac

# `-I` skips binary files, `-n` gives the line so the report is actionable,
# `-E` keeps the pattern readable, `-i` makes the claim case-insensitive.
if hits=$(git grep -nIEi "$PATTERN" -- . "${ALLOWED_PATHS[@]}"); then
  count=$(printf '%s\n' "$hits" | wc -l | tr -d ' ')
  files=$(printf '%s\n' "$hits" | cut -d: -f1 | sort -u | wc -l | tr -d ' ')
  {
    echo "PLATFORMS-FAIL: a Windows/platform token appears in $count place(s) across $files file(s)."
    echo
    printf '%s\n' "$hits" | head -60
    if [ "$count" -gt 60 ]; then
      echo "… and $((count - 60)) more."
    fi
    echo
    echo "The supported-platform decision is docs/adr/0004: Linux x86_64 and"
    echo "aarch64 only. Windows, macOS and aarch64-macOS are unsupported and"
    echo "the release pipeline publishes no archive for them."
    echo
    echo "If the occurrence is history or a deliberate unsupported-platform"
    echo "notice, add its path to ALLOWED_PATHS in scripts/check-platforms.sh"
    echo "— with a reason beside it. Otherwise remove the claim."
  } >&2
  exit 1
fi

if [ "$verbose" = 1 ]; then
  echo "checked $(git ls-files | wc -l | tr -d ' ') tracked files; no platform tokens outside the allowlist."
  echo "allowlisted:"
  printf '  %s\n' "${ALLOWED_PATHS[@]}"
fi
echo "PLATFORMS-OK"
