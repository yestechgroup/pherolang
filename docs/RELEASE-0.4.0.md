# Release runbook: v0.4.0 (Phero Lang)

The operator sequence for cutting **v0.4.0**, the first release whose binary
contains `pherolang highlight` and the generated editor-grammar commands. The
release PR (SH8, issue pherolang#294) already bumps the workspace version and
regenerates every provenance-bearing golden; this page covers the tag, the
publish, and the post-publish site regeneration. Policy is
[VERSIONING.md](VERSIONING.md); the highlighting contract is
[HIGHLIGHTING.md](HIGHLIGHTING.md).

Run every command from a clean clone of the repository, on `main`, with an
authenticated `gh`.

> **Acceptance status — executed 2026-11-17.** v0.4.0 was tagged at `ba2b267`
> and published with one asset,
> `pherolang-0.4.0-x86_64-unknown-linux-gnu.tar.gz`, plus `SHA256SUMS`.
> `site/data` was regenerated with the released binary and
> `git diff --exit-code site/data` was empty, so SH8's acceptance is satisfied.
> The steps below are retained as the reproducible runbook.

## 0. Preconditions

The release is cut only from a green tree. All seven local gate steps must
report `*-OK`, the site gate must pass, and `main`'s latest CI run must be
green:

```sh
scripts/ci-local.sh
scripts/ci-local.sh audit
pnpm -C site install --frozen-lockfile
node site/scripts/gate.mjs
gh run list --branch main --limit 1
```

The workspace version is already `0.4.0`, the `CHANGELOG.md` entry is in the
same PR, and the regenerated provenance goldens were reviewed (the only
content change should be the version line). Confirm the site's asset pattern
still matches what the release publishes:

```sh
grep -n 'PHEROLANG_ASSET_PATTERN' .github/workflows/site.yml
```

## 1. Verify the contract in a local build

The release workflow repeats these against the packaged binary. Running them
locally first turns an avoidable matrix failure into a one-minute check:

```sh
cargo build -q -p phero-cli
target/debug/pherolang --version
printf 'class A { }\n' | target/debug/pherolang highlight --surface mox --format json -
tmp="$(mktemp -d)"
target/debug/pherolang grammar --out "$tmp"
target/debug/pherolang grammar --check "$tmp"
target/debug/pherolang grammar --check site/packages/highlighting
```

The JSON must report `"formatVersion": 1` and `"surface": "mox"`. The
committed grammar package must pass `--check`; if it does not, the metadata
changed without regeneration — fix that before tagging.

## 2. Tag and publish

```sh
git switch main
git pull --ff-only
git status --short        # must be empty
git tag -a v0.4.0 -m "Phero Lang 0.4.0"
git push origin v0.4.0
```

The `Release` workflow builds the target matrix, runs the highlight/grammar
contract check on each built binary, assembles `SHA256SUMS`, and publishes the
GitHub Release. If a leg fails, fix the tree, re-tag, and push the tag again;
the workflow resolves the version from the tag, never the manifest.

```sh
gh release view v0.4.0 --json assets -q '.assets[].name'
gh release download v0.4.0 --dir /tmp/pherolang-0.4.0
(cd /tmp/pherolang-0.4.0 && sha256sum -c SHA256SUMS)
tar -xzf /tmp/pherolang-0.4.0/pherolang-0.4.0-x86_64-unknown-linux-gnu.tar.gz \
  -C /tmp/pherolang-0.4.0
/tmp/pherolang-0.4.0/pherolang --version
/tmp/pherolang-0.4.0/pherolang highlight --help
```

The last command is the release contract: v0.3.0 exited non-zero here. Do not
declare the release accepted until it prints the usage text.

## 3. Regenerate the site data with the released binary

`site/scripts/fetch-pherolang.mjs` pins `0.4.0`. If the tag already exists it
downloads it; an explicit pin never falls back:

```sh
PHEROLANG_VERSION=0.4.0 node site/scripts/fetch-pherolang.mjs
node site/scripts/sync-model.mjs
node site/scripts/sync-highlight.mjs
pnpm -C site install --frozen-lockfile
node site/scripts/gate.mjs
```

`sync-model` and `sync-highlight` now resolve `site/.bin/pherolang`, which can
check the tree and provides `pherolang highlight`. The expected `git diff`
under `site/data/` is empty — the published binary and the source build are
the same commit. If it is not empty, review the diff: the generator stamp or a
real span change is a release finding, not noise to commit blindly.

Commit the regenerated data if the diff is non-empty, then update
`docs/HIGHLIGHTING.md`'s **Release binary status** section: the published
version is now the source, the source-built exception is historical, and the
`deploy` version gate is satisfied. `site/README.md` and `site/DEPLOY.md` say
the same thing in their own words.

## 4. Confirm production

The `Site` `deploy` job only runs when the latest release version equals the
workspace version, so production stayed on the previous release during the PR
window. After the tag:

```sh
gh run list --workflow Site --limit 3
curl -fsS https://pherolang.com/install | grep -o 'v0\.4\.0' | head -1
```

The site's `freshness` job regenerates with the released asset and fails on a
diff, so a green `Site` run after the tag is the released-binary freshness
claim. Until that runs, do not describe the committed data as
released-binary-fresh.

## 5. If the release slips

If v0.4.0 cannot be tagged, keep the source-built data and the documented
exception. Set `PHEROLANG_VERSION` only to a release that actually provides
`pherolang highlight` and can check the tree; the resolver rejects anything
else. SH8's release acceptance remains **blocked**, and the epic closeout says
so rather than claiming a released binary is ready.
