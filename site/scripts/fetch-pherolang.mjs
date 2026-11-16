#!/usr/bin/env node
/**
 * Fetch the *released* `pherolang` binary into `site/.bin/`.
 *
 * The site's model data must be what a user of the published tool generates,
 * so this downloads a release asset and verifies it against the release's
 * `SHA256SUMS` — it never runs `cargo build`. That is also what makes the CI
 * `Site` workflow's freshness job meaningful: it downloads the same asset and
 * expects the committed data under `site/data/` to be byte-identical.
 *
 * The default pin is the release the tree is prepared for (`0.4.0`, the first
 * release that ships `pherolang highlight`). Until that tag exists, an
 * unset `PHEROLANG_VERSION` falls back to the latest published release with a
 * loud warning; an explicit pin never falls back. The resolver in
 * `scripts/highlighting/runner.mjs` then refuses a release that cannot check
 * the current examples, so the site tooling builds this checkout instead —
 * the documented temporary exception (docs/HIGHLIGHTING.md), never a claim of
 * released-binary freshness.
 *
 * Usage:
 *   node scripts/fetch-pherolang.mjs            # pinned release, or latest published while the pin is pending
 *   node scripts/fetch-pherolang.mjs --check    # verify the pinned asset only
 *   PHEROLANG_VERSION=0.3.0 node scripts/...    # pin a release (default 0.4.0)
 */

import { createHash } from 'node:crypto';
import { existsSync, readdirSync } from 'node:fs';
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const REQUESTED = process.env.PHEROLANG_VERSION ?? '0.4.0';
const EXPLICIT = Boolean(process.env.PHEROLANG_VERSION);
const REPO = 'yestechgroup/pherolang';
const BIN_DIR = join(dirname(dirname(fileURLToPath(import.meta.url))), '.bin');
const BIN = join(BIN_DIR, 'pherolang');
const MARKER = join(BIN_DIR, 'VERSION');

const checkOnly = process.argv.includes('--check');
const key = `${process.platform}-${process.arch}`;

function fail(message) {
  console.error(`fetch-pherolang: ${message}`);
  process.exit(1);
}

function log(message) {
  console.log(`fetch-pherolang: ${message}`);
}

/** The archive shape one release publishes for the host platform. */
function targetFor(version) {
  const targets = {
    'linux-x64': {
      asset: `pherolang-${version}-x86_64-unknown-linux-gnu.tar.gz`,
      kind: 'tar',
    },
    'linux-arm64': {
      asset: `pherolang-${version}-aarch64-unknown-linux-gnu.tar.gz`,
      kind: 'tar',
    },
  };
  return targets[key] ?? null;
}

function baseFor(version) {
  return `https://github.com/${REPO}/releases/download/v${version}`;
}

async function fetchBytes(url) {
  const response = await fetch(url, { redirect: 'follow' });
  if (!response.ok) fail(`GET ${url} -> ${response.status} ${response.statusText}`);
  return Buffer.from(await response.arrayBuffer());
}

/** Like `fetchBytes`, but a 404 is data (the release/asset is absent). */
async function fetchOptional(url) {
  const response = await fetch(url, { redirect: 'follow' });
  if (response.status === 404) return null;
  if (!response.ok) fail(`GET ${url} -> ${response.status} ${response.statusText}`);
  return Buffer.from(await response.arrayBuffer());
}

function sha256(buffer) {
  return createHash('sha256').update(buffer).digest('hex');
}

/** The latest published release version, from the redirect GitHub serves. */
async function latestPublished() {
  const response = await fetch(`https://github.com/${REPO}/releases/latest`, {
    redirect: 'follow',
  });
  if (!response.ok) fail(`GET releases/latest -> ${response.status} ${response.statusText}`);
  const match = /\/releases\/tag\/v([0-9]+\.[0-9]+\.[0-9]+)$/.exec(response.url);
  if (!match) fail(`cannot read a release version from ${response.url}`);
  return match[1];
}

async function installedVersion() {
  if (!existsSync(BIN) || !existsSync(MARKER)) return null;
  return (await readFile(MARKER, 'utf8')).trim();
}

/**
 * Download and install `version`.
 *
 * Returns `false` when the release (or its asset) is not published yet and the
 * caller may fall back; every other failure is fatal.
 */
async function verifyAndInstall(version) {
  const target = targetFor(version);
  if (!target) {
    fail(
      [
        `no published pherolang asset for ${key}.`,
        `v${version} publishes linux-x64 and linux-arm64 archives. Windows and macOS are unsupported (ADR 0004).`,
        `On this platform either build from source (cargo build -p phero-cli) and pass`,
        `PHEROLANG_BIN=/path/to/pherolang to scripts/sync-model.mjs, or wait for a release.`,
      ].join('\n  '),
    );
  }

  const base = baseFor(version);
  const sumsBuffer = await fetchOptional(`${base}/SHA256SUMS`);
  if (sumsBuffer === null) return false;
  const sums = sumsBuffer.toString('utf8');
  const expected = sums
    .split('\n')
    .map((line) => line.trim().split(/\s+/))
    .find(([, name]) => name === target.asset)?.[0];

  if (!expected) {
    if (checkOnly) fail(`SHA256SUMS from v${version} does not list ${target.asset}`);
    return false;
  }

  log(`downloading ${target.asset}`);
  const archive = await fetchBytes(`${base}/${target.asset}`);
  const actual = sha256(archive);
  if (actual !== expected) {
    fail(`${target.asset} checksum mismatch\n  expected ${expected}\n  actual   ${actual}`);
  }
  log(`checksum ok (${actual.slice(0, 16)}…)`);

  if (checkOnly) {
    log(`${target.asset} verifies against v${version}`);
    return true;
  }

  const work = await mkdtemp(join(tmpdir(), 'pherolang-fetch-'));
  try {
    const archivePath = join(work, target.asset);
    await writeFile(archivePath, archive);
    const extract = join(work, 'unpacked');
    await mkdir(extract, { recursive: true });

    // Every supported asset is a gzipped tar with the binary at its root.
    execFileSync('tar', ['-xzf', archivePath, '-C', extract], { stdio: 'inherit' });

    const found = findBinary(extract);
    if (!found) fail(`${target.asset} does not contain a 'pherolang' binary`);

    await mkdir(BIN_DIR, { recursive: true });
    await writeFile(BIN, await readFile(found));
    await chmod(BIN, 0o755);
    await writeFile(MARKER, `${version}\n`);

    const reported = execFileSync(BIN, ['--version'], { encoding: 'utf8' }).trim();
    log(`installed ${BIN} (${reported})`);
  } finally {
    await rm(work, { recursive: true, force: true });
  }
  return true;
}

function findBinary(root) {
  const stack = [root];
  while (stack.length) {
    const dir = stack.pop();
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) stack.push(full);
      else if (entry.name === 'pherolang') return full;
    }
  }
  return null;
}

const current = await installedVersion();
if (current === REQUESTED && !checkOnly) {
  log(`already at v${current} (${BIN}); nothing to do`);
  process.exit(0);
}

const installed = await verifyAndInstall(REQUESTED);
if (installed) process.exit(0);

if (checkOnly) {
  fail(`v${REQUESTED} is not published yet (or has no ${key} asset)`);
}

if (EXPLICIT) {
  fail(`v${REQUESTED} is not published yet (or has no ${key} asset); see docs/RELEASE-0.4.0.md`);
}

const fallback = await latestPublished();
if (fallback === REQUESTED) {
  fail(`v${REQUESTED} has no usable release asset`);
}
log(`v${REQUESTED} is not published yet; falling back to the latest published release v${fallback}`);
log(`the site resolver will reject a release that cannot check this tree (docs/HIGHLIGHTING.md)`);
const ok = await verifyAndInstall(fallback);
if (!ok) fail(`v${fallback} has no usable release asset`);
