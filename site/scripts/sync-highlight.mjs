#!/usr/bin/env node
/**
 * Regenerate the committed compiler-backed highlight artifact.
 *
 * Output: `site/data/highlighting/spans.json`, one content-addressed entry per
 * `(surface, exact source bytes)` pair. Inputs are the docs fences, the
 * `examples/support/support.*` models, and the exact excerpts in
 * `site/packages/highlighting/excerpts.json`.
 *
 * Binary policy (#343 — released-binary freshness, no source build):
 *   1. `PHEROLANG_BIN` when set (it must provide `pherolang highlight` and
 *      compile `examples/`);
 *   2. the fetched release at `site/.bin/pherolang`, only if it can;
 *   3. `pherolang` on PATH, only if it can;
 *   4. otherwise the artifact is not regenerated: the named skip (tag +
 *      failing file) is printed and the command exits 1. A release that
 *      predates the DSL on main skips — it never falls back to a build of
 *      this commit, which would make the artifact track unreleased DSL.
 *
 * Usage:
 *   node scripts/sync-highlight.mjs            # write the artifact
 *   node scripts/sync-highlight.mjs --check    # compare in memory, exit 1 on drift
 */

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  HIGHLIGHT_ARTIFACT_FORMAT_VERSION,
  artifactPath,
  keyFor,
  sha256Hex,
} from '../packages/highlighting/src/artifact.mjs';
import { resolveSiteBinary } from './highlighting/runner.mjs';
import { collectSources } from './highlighting/sources.mjs';

const SITE = dirname(dirname(fileURLToPath(import.meta.url)));
const ROOT = dirname(SITE);
const OUTPUT = artifactPath(ROOT);
const CHECK = process.argv.includes('--check');

const log = (message) => console.log(`sync-highlight: ${message}`);

function fail(message) {
  console.error(`sync-highlight: ${message}`);
  process.exit(1);
}

function resolveRunner() {
  try {
    const resolved = resolveSiteBinary(ROOT, SITE, { requireHighlight: true, log });
    if (resolved.kind === 'unavailable') {
      fail(`skipped — ${resolved.reason}`);
    }
    return resolved;
  } catch (error) {
    fail(error.message);
  }
}

function validateArtifact(artifact, surface, source, origin, binary) {
  if (!artifact || artifact.formatVersion !== HIGHLIGHT_ARTIFACT_FORMAT_VERSION) {
    fail(
      `${origin}: ${binary} emitted formatVersion ${artifact?.formatVersion ?? 'absent'}; ` +
        `expected ${HIGHLIGHT_ARTIFACT_FORMAT_VERSION}`,
    );
  }
  if (artifact.surface !== surface) {
    fail(`${origin}: expected surface '${surface}', got '${artifact.surface}'`);
  }
  const expected = `sha256:${sha256Hex(source)}`;
  if (artifact.sourceHash !== expected) {
    fail(`${origin}: sourceHash ${artifact.sourceHash} does not match ${expected}`);
  }
  if (!Array.isArray(artifact.spans)) fail(`${origin}: artifact has no spans array`);
  if (!artifact.generator || artifact.generator.name !== 'pherolang' || typeof artifact.generator.version !== 'string') {
    fail(`${origin}: artifact has no pherolang generator stamp`);
  }
  if (typeof artifact.degraded !== 'boolean') fail(`${origin}: artifact has no degraded flag`);
}

function highlight(runner, source, surface, origin) {
  const args = ['highlight', '--surface', surface, '--format', 'json', '--mode', 'lenient', '-'];
  let stdout;
  try {
    stdout = execFileSync(runner.binary, args, {
      input: source,
      encoding: 'utf8',
      cwd: ROOT,
      timeout: 120_000,
      maxBuffer: 64 * 1024 * 1024,
      stdio: ['pipe', 'pipe', 'pipe'],
    });
  } catch (error) {
    const stderr = error.stderr?.toString?.() ?? '';
    fail(`${origin}: \`pherolang highlight\` failed (exit ${error.status ?? '?'}):\n${stderr || error.message}`);
  }

  let artifact;
  try {
    artifact = JSON.parse(stdout);
  } catch (error) {
    fail(`${origin}: invalid JSON from \`pherolang highlight\`: ${error.message}`);
  }
  validateArtifact(artifact, surface, source, origin, runner.binary);
  return artifact;
}

function artifactFor(runner, sources, problems) {
  if (problems.length > 0) {
    fail(`the source corpus is not well formed:\n  ${problems.join('\n  ')}`);
  }
  if (sources.length === 0) fail('no Phero Lang sources were collected');

  const entries = new Map();
  const origins = new Map();
  let generator = null;

  for (const item of sources) {
    if (typeof item.source !== 'string') fail(`${item.origin}: no source text`);
    const key = keyFor(item.surface, item.source);
    if (entries.has(key)) continue;

    const artifact = highlight(runner, item.source, item.surface, item.origin);
    if (generator === null) {
      generator = { name: artifact.generator.name, version: artifact.generator.version };
    } else if (
      generator.name !== artifact.generator.name ||
      generator.version !== artifact.generator.version
    ) {
      fail(
        `${item.origin}: generator ${artifact.generator.name} ${artifact.generator.version} ` +
          `differs from ${generator.name} ${generator.version}`,
      );
    }

    entries.set(key, {
      surface: item.surface,
      sourceHash: artifact.sourceHash,
      degraded: artifact.degraded,
      spans: artifact.spans,
    });
    origins.set(key, item.origin);
  }

  const sorted = Object.fromEntries([...entries.entries()].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
  const artifact = {
    formatVersion: HIGHLIGHT_ARTIFACT_FORMAT_VERSION,
    generator,
    entries: sorted,
  };
  return { artifact, origins };
}

function describeDiff(next, previous, origins) {
  const before = previous?.entries ?? {};
  const after = next.entries ?? {};
  const missing = Object.keys(after).filter((key) => !(key in before));
  const extra = Object.keys(before).filter((key) => !(key in after));
  const changed = Object.keys(after).filter(
    (key) => key in before && JSON.stringify(before[key]) !== JSON.stringify(after[key]),
  );

  const lines = [];
  if (missing.length) {
    lines.push(`${missing.length} new entr${missing.length === 1 ? 'y' : 'ies'}:`);
    for (const key of missing.slice(0, 10)) lines.push(`  + ${key}  ${origins.get(key) ?? ''}`.trimEnd());
  }
  if (extra.length) {
    lines.push(`${extra.length} stale entr${extra.length === 1 ? 'y' : 'ies'}:`);
    for (const key of extra.slice(0, 10)) lines.push(`  - ${key}`);
  }
  if (changed.length) {
    lines.push(`${changed.length} changed entr${changed.length === 1 ? 'y' : 'ies'}:`);
    for (const key of changed.slice(0, 10)) lines.push(`  ~ ${key}  ${origins.get(key) ?? ''}`.trimEnd());
  }
  if (missing.length === 0 && extra.length === 0 && changed.length === 0) {
    lines.push('only the generator stamp or format changed');
  }
  return lines.join('\n');
}

async function main() {
  const runner = resolveRunner();
  log(`using ${relative(ROOT, runner.binary) || runner.binary} (${runner.kind})`);

  const { sources, problems, docsFences } = collectSources(ROOT);
  const { artifact, origins } = artifactFor(runner, sources, problems);
  const text = `${JSON.stringify(artifact, null, 2)}\n`;

  log(
    `collected ${sources.length} source(s): ${docsFences.length} docs fences, ` +
      `${sources.filter((item) => item.origin?.startsWith('examples/')).length} examples, ` +
      `${sources.filter((item) => item.origin?.startsWith('excerpts.json')).length} excerpts`,
  );
  log(`${Object.keys(artifact.entries).length} content-addressed entries`);

  if (CHECK) {
    if (!existsSync(OUTPUT)) fail(`missing ${OUTPUT} — run node scripts/sync-highlight.mjs`);
    const current = readFileSync(OUTPUT, 'utf8');
    if (current === text) {
      log('fresh: the committed artifact matches a regeneration');
      return;
    }
    let previous = null;
    try {
      previous = JSON.parse(current);
    } catch {
      /* the diff below still reports the byte difference */
    }
    console.error('sync-highlight: the committed artifact is stale');
    if (previous) console.error(describeDiff(artifact, previous, origins));
    process.exit(1);
  }

  await mkdir(dirname(OUTPUT), { recursive: true });
  await writeFile(OUTPUT, text);
  log(`wrote ${relative(ROOT, OUTPUT)}`);
}

await main();
