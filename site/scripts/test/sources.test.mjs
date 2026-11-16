import assert from 'node:assert/strict';
import test from 'node:test';

import { keyFor, loadArtifact } from '../../packages/highlighting/src/artifact.mjs';
import { allExcerptSpecs } from '../../packages/highlighting/src/excerpts.mjs';
import { PHERO_SURFACE_IDS, isPheroSurface } from '../../packages/highlighting/src/taxonomy.mjs';
import { collectSources, normalizeEcCode } from '../highlighting/sources.mjs';
import { REPO } from '../gate/paths.mjs';

const collected = collectSources(REPO);
const { artifact, error } = loadArtifact(REPO);

const artifactKeys = new Set(Object.keys(artifact?.entries ?? {}));
const sourceKeys = new Set(collected.sources.map((source) => keyFor(source.surface, source.source)));

test('collectSources reports no problems for the committed corpus', () => {
  assert.deepEqual(collected.problems, []);
  assert.equal(error, null);
  assert.ok(artifact, error ?? 'the highlight artifact did not load');
});

test('all nine example surfaces are collected', () => {
  const examples = collected.sources
    .filter((source) => source.origin.startsWith('examples/support/'))
    .map((source) => source.surface);
  assert.deepEqual([...new Set(examples)].sort(), [...PHERO_SURFACE_IDS].sort());
});

test('every Phero fence in docs is collected', () => {
  const pheroFences = collected.docsFences.filter((fence) => isPheroSurface(fence.lang));
  assert.ok(pheroFences.length > 0, 'the docs corpus has no Phero fences');
  for (const fence of pheroFences) {
    assert.ok(
      sourceKeys.has(keyFor(fence.lang, fence.raw)),
      `${fence.origin}: the ${fence.lang} fence is not a collected source`,
    );
  }
});

test('every excerpt declared in excerpts.json is collected', () => {
  const specs = allExcerptSpecs(REPO);
  assert.ok(specs.length > 0, 'excerpts.json declares no excerpts');
  const origins = new Set(collected.sources.map((source) => source.origin));
  for (const spec of specs) {
    assert.ok(origins.has(`excerpts.json#${spec.key}`), `excerpts.json#${spec.key} is not a collected source`);
  }
});

test('every collected source has a committed artifact entry', () => {
  for (const source of collected.sources) {
    assert.ok(
      artifactKeys.has(keyFor(source.surface, source.source)),
      `${source.origin} (surface ${source.surface}) has no artifact entry`,
    );
  }
});

test('the committed artifact has no stale entries', () => {
  for (const key of artifactKeys) {
    assert.ok(sourceKeys.has(key), `artifact entry ${key} maps to no collected source`);
  }
});

test('docs fences equal Expressive Code normalization', () => {
  for (const fence of collected.docsFences) {
    assert.equal(
      fence.raw,
      normalizeEcCode(fence.raw),
      `${fence.origin}: the ${fence.lang || 'plain'} fence is not normalized`,
    );
  }
});
