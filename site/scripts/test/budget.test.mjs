import assert from 'node:assert/strict';
import test from 'node:test';

import {
  JS_ISLAND_PAGES,
  classifyPageScripts,
  isExecutableScript,
} from '../gate/budget.mjs';

test('an external module is executable', () => {
  const { external, inline } = classifyPageScripts(
    '<script type="module" src="/_astro/graph.js"></script>',
  );
  assert.deepEqual(external, ['/_astro/graph.js']);
  assert.equal(inline.length, 0);
});

test('an inline module is executable', () => {
  const { external, inline } = classifyPageScripts('<script type="module">console.log(1)</script>');
  assert.equal(external.length, 0);
  assert.equal(inline.length, 1);
});

test('a classic inline script with no type is executable', () => {
  assert.equal(isExecutableScript({}), true);
  assert.equal(isExecutableScript({ type: 'text/javascript' }), true);
  assert.equal(isExecutableScript({ type: 'application/javascript' }), true);
});

test('JSON data blocks are not executable', () => {
  const html = [
    '<script type="application/json" data-graph-explorer-data>{}</script>',
    '<script type="application/ld+json">{"@type":"Thing"}</script>',
  ].join('\n');
  assert.deepEqual(classifyPageScripts(html), { external: [], inline: [] });
});

test('the only documented island is the graph explorer page', () => {
  assert.deepEqual([...JS_ISLAND_PAGES], ['demo/index.html']);
});
