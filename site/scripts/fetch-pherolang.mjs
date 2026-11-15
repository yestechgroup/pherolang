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
 * Usage:
 *   node scripts/fetch-pherolang.mjs            # download if missing
 *   node scripts/fetch-pherolang.mjs --check    # verify the pinned asset only
 *   PHEROLANG_VERSION=0.3.0 node scripts/...    # pin a release (default 0.3.0)
 */

import { createHash } from 'node:crypto';
import { existsSync, readdirSync } from 'node:fs';
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const VERSION = process.env.PHEROLANG_VERSION ?? '0.3.0';
const REPO = 'yestechgroup/pherolang';
const BIN_DIR = join(dirname(dirname(fileURLToPath(import.meta.url))), '.bin');
const BIN = join(BIN_DIR, process.platform === 'win32' ? 'pherolang.exe' : 'pherolang');
const MARKER = join(BIN_DIR, 'VERSION');

/** The release assets v0.3.0 actually publishes. */
const TARGETS = {
  'linux-x64': { asset: `pherolang-${VERSION}-x86_64-unknown-linux-gnu.tar.gz`, kind: 'tar' },
  'linux-arm64': { asset: `pherolang-${VERSION}-aarch64-unknown-linux-gnu.tar.gz`, kind: 'tar' },
  'win32-x64': { asset: `pherolang-${VERSION}-x86_64-pc-windows-msvc.zip`, kind: 'zip' },
};

const checkOnly = process.argv.includes('--check');
const key = `${process.platform}-${process.arch}`;
const target = TARGETS[key];

function fail(message) {
  console.error(`fetch-pherolang: ${message}`);
  process.exit(1);
}

function log(message) {
  console.log(`fetch-pherolang: ${message}`);
}

if (!target) {
  fail(
    [
      `no published pherolang asset for ${key}.`,
      `v${VERSION} publishes: ${Object.values(TARGETS).map((t) => t.asset).join(', ')}.`,
      `On this platform either build from source (cargo build -p phero-cli) and pass`,
      `PHEROLANG_BIN=/path/to/pherolang to scripts/sync-model.mjs, or wait for a release.`,
    ].join('\n  '),
  );
}

const base = `https://github.com/${REPO}/releases/download/v${VERSION}`;

async function fetchBytes(url) {
  const response = await fetch(url, { redirect: 'follow' });
  if (!response.ok) fail(`GET ${url} -> ${response.status} ${response.statusText}`);
  return Buffer.from(await response.arrayBuffer());
}

function sha256(buffer) {
  return createHash('sha256').update(buffer).digest('hex');
}

async function installedVersion() {
  if (!existsSync(BIN) || !existsSync(MARKER)) return null;
  const recorded = (await readFile(MARKER, 'utf8')).trim();
  return recorded === VERSION ? VERSION : null;
}

async function verifyAndInstall() {
  log(`downloading ${target.asset}`);
  const sums = (await fetchBytes(`${base}/SHA256SUMS`)).toString('utf8');
  const expected = sums
    .split('\n')
    .map((line) => line.trim().split(/\s+/))
    .find(([, name]) => name === target.asset)?.[0];

  if (!expected) fail(`SHA256SUMS from v${VERSION} does not list ${target.asset}`);

  const archive = await fetchBytes(`${base}/${target.asset}`);
  const actual = sha256(archive);
  if (actual !== expected) {
    fail(`${target.asset} checksum mismatch\n  expected ${expected}\n  actual   ${actual}`);
  }
  log(`checksum ok (${actual.slice(0, 16)}…)`);

  if (checkOnly) {
    log(`${target.asset} verifies against v${VERSION}`);
    return;
  }

  const work = await mkdtemp(join(tmpdir(), 'pherolang-fetch-'));
  try {
    const archivePath = join(work, target.asset);
    await writeFile(archivePath, archive);
    const extract = join(work, 'unpacked');
    await mkdir(extract, { recursive: true });

    if (target.kind === 'tar') {
      execFileSync('tar', ['-xzf', archivePath, '-C', extract], { stdio: 'inherit' });
    } else {
      // Windows ships a .zip; `tar` on Windows 10+ is bsdtar and reads zip.
      execFileSync('tar', ['-xf', archivePath, '-C', extract], { stdio: 'inherit' });
    }

    const found = findBinary(extract);
    if (!found) fail(`${target.asset} does not contain a 'pherolang' binary`);

    await mkdir(BIN_DIR, { recursive: true });
    await writeFile(BIN, await readFile(found));
    await chmod(BIN, 0o755);
    await writeFile(MARKER, `${VERSION}\n`);

    const reported = execFileSync(BIN, ['--version'], { encoding: 'utf8' }).trim();
    log(`installed ${BIN} (${reported})`);
  } finally {
    await rm(work, { recursive: true, force: true });
  }
}

function findBinary(root) {
  const stack = [root];
  while (stack.length) {
    const dir = stack.pop();
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) stack.push(full);
      else if (entry.name === 'pherolang' || entry.name === 'pherolang.exe') return full;
    }
  }
  return null;
}

const current = await installedVersion();
if (current && !checkOnly) {
  log(`already at v${current} (${BIN}); nothing to do`);
  process.exit(0);
}

await verifyAndInstall();
