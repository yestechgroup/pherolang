# Release runbook: v0.3.0 (Phero Lang)

The operator sequence for cutting **v0.3.0**, in the order the work actually
happens. Release *policy* — what a release consists of, the wire-break rule,
and the golden-regeneration discipline — is [VERSIONING.md](VERSIONING.md).
The non-diff operator actions (repo rename, DNS, social handles) are tracked
as owners in [REBRAND.md](REBRAND.md#operator-checklist); this page is their
command-level order for the release half, not a second checklist. The
old → new spelling and artifact map is [REBRAND.md](REBRAND.md).

Run every command from a clone of the repository, on `main`, with a clean
working tree and an authenticated `gh`.

## 0. Preconditions

The release is cut only from a green tree. All seven local gate steps must
report `*-OK`, the supply-chain gates must be clean, and `main`'s latest CI
run must be green:

```sh
scripts/ci-local.sh
scripts/ci-local.sh audit
gh run list --branch main --limit 1
```

The workspace version is already `0.3.0`; the `CHANGELOG.md` `0.3.0` entry
and the regenerated provenance goldens must be in that same PR
([VERSIONING.md](VERSIONING.md#what-a-release-consists-of)). Confirm the
asset names the site workflow downloads still match what this release will
publish — if `site.yml` pins a different spelling, fix it before tagging, or
the site's binary download 404s after the release:

```sh
grep -n 'PHEROLANG_ASSET_PATTERN' .github/workflows/site.yml
```

The pattern is `pherolang-*-x86_64-unknown-linux-gnu.tar.gz`; `detect`
resolves the concrete versioned asset from the latest release, so this tag's
`pherolang-0.3.0-x86_64-unknown-linux-gnu.tar.gz` matches without a pin
(`.github/workflows/release.yml` states the naming contract where the matrix
is declared).

## 1. Merge the rebrand PR

```sh
gh pr merge <PR> --merge
git switch main
git pull --ff-only
git status --short        # must be empty
```

## 2. Rename the repository

Capture the current remote URL **before** the rename; it is the only thing
that can prove the redirect afterwards, and the old slug is not written down
anywhere in this repository (the naming contract forbids it). `gh api`
substitutes `{owner}/{repo}` from the clone you are standing in:

```sh
old_url=$(gh repo view --json url -q .url)
gh api -X POST repos/{owner}/{repo}/rename -f new_name=pherolang
```

Now point the clone at the new home and confirm GitHub still redirects the
old URL (a `301`, and a fetch that still resolves):

```sh
git remote set-url origin https://github.com/yestechgroup/pherolang.git
git remote -v
curl -sI "$old_url" | head -1     # HTTP/… 301
git ls-remote "$old_url" HEAD     # succeeds via the redirect
```

The redirect is permanent for the old name, so existing pins and clones keep
resolving; new documentation and install commands use the new URL.

## 3. Repository metadata

```sh
gh repo edit \
  --description "Phero Lang — Verifiable Model-Driven Engineering" \
  --homepage https://pherolang.com \
  --add-topic modeling-language,code-generation,rust,dsl,json-schema,cedar-policy
gh api -X PATCH repos/{owner}/{repo} -F has_discussions=true
```

The social preview is a UI-only setting (there is no REST field for it):
export `site/brand/pherolang-tile.svg` to a 1280×640 PNG and upload it under
Settings → Social preview. `site/brand` arrives with the brand PR; do this
step after that lands.

## 4. Close the superseded brand PR

PR #237 (the standalone brand package) is superseded once its files are in
`main`. Verify before closing, so nothing is lost:

```sh
git ls-tree -r --name-only main site/brand | grep pherolang-tile.svg
gh pr close 237 --comment "Superseded: site/brand landed in main."
```

## 5. Tag and publish

The tag is the release trigger; `.github/workflows/release.yml` builds the
enabled targets, checksums them into `SHA256SUMS`, and publishes the GitHub
Release. The matrix is data: three targets run by default (linux x86_64, linux
aarch64, windows x86_64), and the two darwin rows are selected by the
repository variable `PHERO_MACOS_RUNNERS=true` once the organization has macOS
runners — the runner configuration currently answers "not supported" for
`macos-14`, and the Intel label never provisions.

```sh
git tag -a v0.3.0 -m "Phero Lang"
git push origin v0.3.0
gh run list --workflow release.yml --limit 1
gh run watch --exit-status "$(gh run list --workflow release.yml --limit 1 --json databaseId -q '.[0].databaseId')"
```

If the workflow fails after a fix that changes the workflow file itself, the
tag must be re-created: a dispatch reads the workflow at the ref, so it can
only re-run the version the tag already contains. Delete and re-tag at the
fixed `main` tip (safe while no release was published and no asset was
consumed), then let the push trigger a fresh run:

```sh
git push --delete origin v0.3.0
git tag -a v0.3.0 -m "Phero Lang" origin/main
git push origin v0.3.0
```

For a transient failure with an unchanged workflow, re-run it on the tag ref —
never from a branch, because the ref name is what names every asset:

```sh
gh workflow run release.yml --ref v0.3.0
```

Verify the published set (one archive per enabled target plus the manifest)
and validate the checksums as a user would:

```sh
gh release view v0.3.0 --json assets -q '.assets[].name'
gh release download v0.3.0 --dir /tmp/pherolang-0.3.0
(cd /tmp/pherolang-0.3.0 && sha256sum -c SHA256SUMS)
```

Expected names at version `0.3.0` with the default matrix — two unix archives
and one windows archive, plus the manifest:

```
pherolang-0.3.0-x86_64-unknown-linux-gnu.tar.gz
pherolang-0.3.0-aarch64-unknown-linux-gnu.tar.gz
pherolang-0.3.0-x86_64-pc-windows-msvc.zip
SHA256SUMS
```

With `PHERO_MACOS_RUNNERS=true`, add
`pherolang-0.3.0-x86_64-apple-darwin.tar.gz` and
`pherolang-0.3.0-aarch64-apple-darwin.tar.gz`.

## 6. Install smoke test

Prove the released tag builds for a consumer, not just on the CI runner:

```sh
cargo install --git https://github.com/yestechgroup/pherolang --tag v0.3.0 --locked phero-cli
pherolang check examples/support/support.mox
pherolang --version
```

`--version` must print `pherolang 0.3.0 (<target triple>)` (the target is
part of the banner precisely so a bug report can name the platform);
`pherolang -V` stays the bare `0.3.0` for scripts. `check` must exit `0`
with no diagnostics. If a previous `pherolang` is installed, add `--force`.

## 7. Rollback

There is nothing to roll back to: v0.3.0 is the first release of the renamed
project, so the previous state is "no release", not a live deployment.

If the release is wrong, withdraw it and re-run after a fix:

```sh
gh release delete v0.3.0 --yes
git push origin --delete v0.3.0
git tag -d v0.3.0
# fix on main, then repeat step 5 with the same tag
```

Only an unpublished or just-published release may be deleted this way. Once
consumers may have pinned it, ship a patch release instead — a deleted
release does not un-download.

## 8. Do not rewrite history

`v0.2.0` and its `CHANGELOG.md` section keep the pre-0.3 name on purpose:
past releases are history, not branding ([NAMING.md](NAMING.md#history)).
Do not amend, re-tag, or force-push `main` for this release — the rename is
a set of commits on top of the existing history, and the old-name guard
(`scripts/check-rebrand.sh`) already keeps the new tree clean.
