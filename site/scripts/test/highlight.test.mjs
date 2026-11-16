import assert from 'node:assert/strict';
import test from 'node:test';

import { keyFor, lookupEntry, sha256Hex } from '../../packages/highlighting/src/artifact.mjs';
import { escapeHtml, renderInlineHtml } from '../../packages/highlighting/src/render.mjs';

test('escapeHtml escapes the five HTML metacharacters', () => {
  assert.equal(escapeHtml(`<>&"'`), '&lt;&gt;&amp;&quot;&#39;');
});

test('renderInlineHtml escapes text and never emits markup for it', () => {
  const html = renderInlineHtml(`<b>&"'`, []);
  assert.equal(html, '&lt;b&gt;&amp;&quot;&#39;');
  assert.ok(!html.includes('<b>'));
});

test('renderInlineHtml preserves newlines exactly and adds no <br>', () => {
  const html = renderInlineHtml('a < b\nc\td', []);
  assert.equal(html, 'a &lt; b\nc\td');
  assert.ok(!html.includes('<br'));
});

test('renderInlineHtml handles an empty source and empty spans', () => {
  assert.equal(renderInlineHtml('', []), '');
  assert.equal(renderInlineHtml('plain', []), 'plain');
});

test('gaps between spans stay unclassified text', () => {
  const source = 'package support;';
  const spans = [{ start: 0, end: 7, class: 'keyword', modifiers: [] }];
  assert.equal(
    renderInlineHtml(source, spans),
    '<span class="hl-keyword">package</span> support;',
  );
});

test('an unknown class is skipped, not guessed at', () => {
  assert.equal(renderInlineHtml('x', [{ start: 0, end: 1, class: 'not-a-class', modifiers: [] }]), 'x');
});

test('a known class survives an unknown modifier; known modifiers become kebab-case classes', () => {
  assert.equal(
    renderInlineHtml('x', [{ start: 0, end: 1, class: 'string', modifiers: ['not-a-modifier'] }]),
    '<span class="hl-string">x</span>',
  );
  assert.equal(
    renderInlineHtml('x', [{ start: 0, end: 1, class: 'type', modifiers: ['defaultLibrary'] }]),
    '<span class="hl-type hl-default-library">x</span>',
  );
});

test('byte offsets are UTF-8, so non-ASCII text cannot shift a span', () => {
  const source = 'é😀x';
  // 'é' is 2 bytes, '😀' is 4, 'x' is 1: the span starts at byte 6.
  assert.equal(
    renderInlineHtml(source, [{ start: 6, end: 7, class: 'keyword', modifiers: [] }]),
    'é😀<span class="hl-keyword">x</span>',
  );
  assert.equal(
    renderInlineHtml(source, [{ start: 0, end: 6, class: 'string', modifiers: [] }]),
    '<span class="hl-string">é😀</span>x',
  );
});

test('keyFor includes the surface, so identical bytes cannot collide', () => {
  const source = 'package support;';
  assert.equal(keyFor('mox', source), keyFor('mox', source));
  assert.notEqual(keyFor('mox', source), keyFor('ddd', source));
});

function artifactWith(surface, source, spans = []) {
  const key = keyFor(surface, source);
  return {
    formatVersion: 1,
    entries: {
      [key]: {
        surface,
        sourceHash: `sha256:${sha256Hex(source)}`,
        degraded: false,
        spans,
      },
    },
  };
}

test('lookupEntry returns nothing for a missing pair', () => {
  const artifact = artifactWith('mox', 'package support;');
  assert.deepEqual(lookupEntry(artifact, 'ddd', 'package support;'), { entry: null, error: null });
});

test('lookupEntry returns the entry when surface and sourceHash agree', () => {
  const source = 'package support;';
  const artifact = artifactWith('mox', source);
  const { entry, error } = lookupEntry(artifact, 'mox', source);
  assert.equal(error, null);
  assert.equal(entry.surface, 'mox');
  assert.deepEqual(entry.spans, []);
});

test('lookupEntry reports a surface mismatch instead of rendering the wrong spans', () => {
  const source = 'package support;';
  const artifact = artifactWith('mox', source);
  const key = keyFor('mox', source);
  artifact.entries[key].surface = 'ddd';
  const { entry, error } = lookupEntry(artifact, 'mox', source);
  assert.equal(entry, null);
  assert.match(error, /says surface ddd/);
});

test('lookupEntry reports a sourceHash mismatch', () => {
  const source = 'package support;';
  const artifact = artifactWith('mox', source);
  const key = keyFor('mox', source);
  artifact.entries[key].sourceHash = 'sha256:0000';
  const { entry, error } = lookupEntry(artifact, 'mox', source);
  assert.equal(entry, null);
  assert.match(error, /sourceHash/);
});
