import { spawnSync } from 'node:child_process';
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import assert from 'node:assert/strict';
import test from 'node:test';

import { listModelFiles, probeRelease, skipMessage } from '../probe-release.mjs';

const SCRIPT = new URL('../probe-release.mjs', import.meta.url).pathname;

/**
 * A stub `pherolang`: `check` exits 0 unless the target contains `fails` in
 * its path or is a directory holding such a file — mirroring real `check`,
 * where a directory fails when any contained model fails.
 */
function writeStub(dir, { failGlob = 'fails' } = {}) {
  const stub = join(dir, 'pherolang-stub');
  writeFileSync(
    stub,
    [
      '#!/bin/sh',
      'target="$2"',
      `case "$target" in`,
      `  *${failGlob}*) echo "Error: missing \`package\` declaration" >&2; echo "1 of 1 files failed" >&2; exit 1 ;;`,
      'esac',
      `if find "$target" -name '*${failGlob}*' -print -quit 2>/dev/null | grep -q .; then`,
      '  echo "Error: missing `package` declaration" >&2; echo "1 of 1 files failed" >&2',
      '  exit 1',
      'fi',
      'exit 0',
    ].join('\n'),
  );
  chmodSync(stub, 0o755);
  return stub;
}

function scratch() {
  return mkdtempSync(join(tmpdir(), 'probe-release-test-'));
}

test('a compatible release: the directory check passes', () => {
  const dir = scratch();
  try {
    const binary = writeStub(dir);
    const examples = join(dir, 'examples');
    mkdirSync(examples, { recursive: true });
    writeFileSync(join(examples, 'support.mox'), 'model');
    const verdict = probeRelease({ binary, examplesDir: examples });
    assert.deepEqual(verdict, { compatible: true });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('an incompatible release names the first failing file', () => {
  const dir = scratch();
  try {
    const binary = writeStub(dir);
    const examples = join(dir, 'examples');
    mkdirSync(join(examples, 'support'), { recursive: true });
    writeFileSync(join(examples, 'support', 'support.mox'), 'model');
    writeFileSync(join(examples, 'support', 'fails.ddd'), 'model');
    const verdict = probeRelease({ binary, examplesDir: examples });
    assert.equal(verdict.compatible, false);
    assert.match(verdict.failingFile, /fails\.ddd$/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('the skip file carries the named skip, verbatim', () => {
  const dir = scratch();
  try {
    const binary = writeStub(dir);
    const examples = join(dir, 'examples');
    mkdirSync(join(examples, 'support'), { recursive: true });
    writeFileSync(join(examples, 'support', 'fails.ddd'), 'model');
    const skipFile = join(dir, 'skip-note.txt');
    const result = spawnSync('node', [
      SCRIPT,
      '--binary',
      binary,
      '--examples',
      examples,
      '--tag',
      'v9.9.9',
      '--skip-file',
      skipFile,
    ]);
    assert.equal(result.status, 2, `expected exit 2, got ${result.status}: ${result.stderr}`);
    const note = skipMessage({ tag: 'v9.9.9', failingFile: join(examples, 'support', 'fails.ddd') });
    assert.equal(readFileSync(skipFile, 'utf8'), `${note}\n`);
    assert.match(String(result.stderr), /the latest release \(v9\.9\.9\) predates the DSL on main/);
    assert.match(String(result.stderr), /fails\.ddd uses syntax it does not parse/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('a compatible run exits 0 and prints the verdict JSON', () => {
  const dir = scratch();
  try {
    const binary = writeStub(dir);
    const examples = join(dir, 'examples');
    mkdirSync(examples, { recursive: true });
    const result = spawnSync('node', [SCRIPT, '--binary', binary, '--examples', examples, '--tag', 'v0.4.0']);
    assert.equal(result.status, 0);
    assert.deepEqual(JSON.parse(String(result.stdout)), { compatible: true, tag: 'v0.4.0' });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('a missing binary is an infrastructure failure (exit 1), never a named skip', () => {
  const dir = scratch();
  try {
    const examples = join(dir, 'examples');
    mkdirSync(examples, { recursive: true });
    const result = spawnSync('node', [
      SCRIPT,
      '--binary',
      join(dir, 'no-such-binary'),
      '--examples',
      examples,
      '--tag',
      'v0.4.0',
    ]);
    assert.equal(result.status, 1);
    assert.match(String(result.stderr), /binary does not exist/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('a missing examples directory is an infrastructure failure (exit 1)', () => {
  const dir = scratch();
  try {
    const binary = writeStub(dir);
    const result = spawnSync('node', [
      SCRIPT,
      '--binary',
      binary,
      '--examples',
      join(dir, 'no-such-dir'),
      '--tag',
      'v0.4.0',
    ]);
    assert.equal(result.status, 1);
    assert.match(String(result.stderr), /examples directory does not exist/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('the named skip has the exact contracted shape', () => {
  assert.equal(
    skipMessage({ tag: 'v1.2.3', failingFile: 'examples/support/support.ddd' }),
    'the latest release (v1.2.3) predates the DSL on main: ' +
      'examples/support/support.ddd uses syntax it does not parse — regenerate after the next release',
  );
});

test('listModelFiles walks recursively, sorts, and only takes model surfaces', () => {
  const dir = scratch();
  try {
    mkdirSync(join(dir, 'a', 'nested'), { recursive: true });
    writeFileSync(join(dir, 'a', 'z.actor'), '');
    writeFileSync(join(dir, 'a', 'nested', 'a.mox'), '');
    writeFileSync(join(dir, 'a', 'readme.md'), '');
    writeFileSync(join(dir, 'a', 'b.ddd'), '');
    const files = listModelFiles(dir).map((path) => path.slice(dir.length + 1));
    assert.deepEqual(files, ['a/b.ddd', 'a/nested/a.mox', 'a/z.actor']);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
