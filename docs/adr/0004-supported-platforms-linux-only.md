# 0004 — Supported platforms are Linux x86_64 and aarch64 only

- **Status:** Accepted, 2026-11-16
- **Issue:** pherolang#295
- **Follows:** [0001](0001-rebrand-to-phero-lang.md) (the rebrand release
  matrix this narrows), pherolang#281 (the release matrix fix that made the
  macOS gap explicit)

## Context

`v0.3.0` published three assets: two Linux archives and a Windows zip. Only
the Linux pair was ever executed. The Windows leg built once, was smoke-tested
by no job, and carried no test coverage — `cargo-hack` and the test suite have
never run on Windows. The macOS rows in the matrix never built at all: the
organization's runner configuration answers
`configuration 'macos-14-arm64-us-default' is not supported`, and the Intel
label never provisions, so the two darwin rows sat behind a repository
variable that could only ever be unset (pherolang#281).

That is the whole problem. **A published archive that no job executes is a
support claim the project cannot back.** `v0.2.0` shipped no assets at all, so
withdrawing the Windows zip now costs no consumer anything, and the cost of
doing it later only rises: every extra release attaches the zip to another tag
someone may have scripted around.

## Decision

| # | Decision | Rationale |
|---|---|---|
| P1 | The supported set is `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`, glibc | Both are built, published, and smoke-tested today; that is the only pair the project can actually stand behind. |
| P2 | "Supported" means **built, tested, documented** — the release workflow produces an archive per target, CI runs the binary on `ubuntu-latest`, and `README.md`/`CONTRIBUTING.md` state the set | Each leg is checked by a different mechanism; dropping any one of them turns the claim back into an assumption. |
| P3 | Windows and macOS are **unsupported and untested**, said in one place instead of accumulating per-platform branches | The release matrix, the packaging steps, and the docs shrink to one shape: no `runner.os` guards, no `fromJSON` conditional, no per-platform archive format. |
| P4 | Adding a platform is a maintainer decision that ships with a CI job which **executes** the binary | The failure this ADR records is precisely a build leg with no execution behind it; requiring the job makes that failure impossible to repeat. |
| P5 | `Cargo.lock` keeps its `windows-sys`/`windows-link` entries | They are transitive facts of `anstyle`, `tokio`, `dirs` and friends. The dependency graph is not a support claim, and purging it is churn with no product effect. |

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Keep publishing the Windows zip | It would keep shipping a binary no job has ever run. "It compiles" is not "it works", and the first bug report would land on a platform nobody can reproduce. |
| Keep the macOS rows behind `PHERO_MACOS_RUNNERS` | A matrix row that is selected by a variable the org cannot set is dormant code pretending to be a plan. When macOS runners exist, the rows are a five-line edit — the ADR is the record, not a dead conditional. |
| Document the platform set without touching the matrix | The acceptance test for this decision is the published asset list. Prose that disagrees with what a tag produces is the failure mode being fixed. |
| Ship musl/static builds (`x86_64-unknown-linux-musl`) in the same change | A real "Linux only" follow-up, but a glibc-free binary is its own decision: it needs its own smoke test and a consumer who asks for it. Out of scope here. |
| Freeze the dependency set to Linux-only features | Same reasoning as P5, from the other direction: it would mean auditing upstream features for no consumer benefit. |
| Cross-build Windows from Linux | A cross-compiled binary is even less tested than a natively built one — it inherits the missing-execution problem and adds a toolchain nobody on the project runs. |

## Consequences

- **The release matrix is two rows, both `ubuntu-latest`.** No `runner.os`
  conditionals, no per-architecture `binary` key, no zip packaging step, and
  no `shasum` fallback for a runner we do not use. The archive contract is
  `pherolang-<version>-<target>.tar.gz` plus `SHA256SUMS`
  (`.github/workflows/release.yml`); `site.yml`'s
  `PHEROLANG_ASSET_PATTERN` matches the x86_64 Linux asset and is unaffected.
- **`v0.3.0`'s Windows zip is withdrawn.** The asset is deleted and
  `SHA256SUMS` regenerated over the two Linux archives, so the documented
  `sha256sum -c SHA256SUMS` flow keeps passing. The tag and the Linux assets
  are untouched — no re-tag, no history rewrite.
- **The gate is unchanged.** `scripts/ci-local.sh` already runs on one Linux
  runner; this decision removes requests for platforms it never ran on rather
  than adding a job.
- **The tree can be grepped for the claim.** `git grep -niE
  'windows|macos|darwin'` should return only: `Cargo.lock` and
  `site/pnpm-lock.yaml` transitivity, `slice::windows(` call sites,
  platform-neutral icon/font names (`apple-touch-icon.png`, `-apple-system`),
  and the deliberate "unsupported" statements in `README.md`,
  `CONTRIBUTING.md`, this ADR, and the site's install page.
- **No `cfg` purge.** The sweep is one `#[cfg(unix)]` in a symlink-loop test
  (`crates/phero-cli/tests/cli/completions.rs`), dropped because the gate is
  Linux-only; there is no other platform accommodation in our crates.
- **A future platform is a complete change.** Archive row, packaging, docs,
  and a CI job that runs the binary — the four-part checklist in P2 — not a
  matrix edit.

## References

- Issue pherolang#295 — the task list this ADR records the decision behind
- [0001](0001-rebrand-to-phero-lang.md) — the rebrand whose release matrix
  carried the dormant rows
- pherolang#281 — `ci(release): make the release matrix runnable and keep it
  data`
- [../RELEASE-0.3.0.md](../RELEASE-0.3.0.md) — the v0.3.0 operator runbook and
  the withdrawn-asset note
- [../VERSIONING.md](../VERSIONING.md) — what a release consists of
- [../../README.md](../../README.md) — the supported-platforms statement
- [../../CONTRIBUTING.md](../../CONTRIBUTING.md) — the contributor-facing
  prerequisites
