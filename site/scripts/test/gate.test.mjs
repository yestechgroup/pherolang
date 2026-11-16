import assert from 'node:assert/strict';
import test from 'node:test';

import {
  classifyFenceLanguage,
  codeBlockScripts,
  scanDocsHtml,
  scanMarketingHtml,
} from '../gate/highlighting.mjs';

const PHERO_LANGUAGES = ['mox', 'ddd', 'actor', 'dmn', 'evt', 'uc', 'cmmn', 'ifml', 'deploy'];

test('marketing: a highlighted Phero block passes', () => {
  const html =
    '<figure><pre tabindex="0" class="overflow"><code class="language-mox">' +
    '<span class="hl-keyword">package</span> support;</code></pre></figure>';
  const { blocks, findings } = scanMarketingHtml(html);
  assert.deepEqual(findings, []);
  assert.equal(blocks.length, 1);
  assert.deepEqual(
    { language: blocks[0].language, highlighted: blocks[0].highlighted, missing: blocks[0].missing, raw: blocks[0].raw },
    { language: 'mox', highlighted: true, missing: false, raw: false },
  );
});

test('marketing: a raw Phero block is a finding', () => {
  const { blocks, findings } = scanMarketingHtml('<pre><code class="language-mox">package support;</code></pre>');
  assert.equal(blocks.length, 1);
  assert.equal(blocks[0].raw, true);
  assert.equal(blocks[0].highlighted, false);
  assert.equal(findings.length, 1);
  assert.match(findings[0], /language-mox.*raw/);
});

test('marketing: the missing marker is a finding even without hl-* classes', () => {
  const html =
    '<pre data-phero-highlight="missing"><code class="language-ddd">application Support;</code></pre>';
  const { blocks, findings } = scanMarketingHtml(html);
  assert.equal(blocks.length, 1);
  assert.equal(blocks[0].missing, true);
  assert.equal(blocks[0].raw, false);
  assert.equal(findings.length, 1);
  assert.match(findings[0], /missing/);
});

test('marketing: non-Phero code blocks are ignored', () => {
  const { blocks, findings } = scanMarketingHtml(
    '<pre><code class="language-sh">cargo test</code></pre>' +
      '<pre><code class="language-rust">fn main() {}</code></pre>',
  );
  assert.deepEqual(blocks, []);
  assert.deepEqual(findings, []);
});

test('marketing: the language class may sit among other classes', () => {
  const html = '<pre><code class="hl-keyword language-mox"><span class="hl-keyword">package</span></code></pre>';
  const { blocks, findings } = scanMarketingHtml(html);
  assert.equal(blocks.length, 1);
  assert.equal(blocks[0].language, 'mox');
  assert.deepEqual(findings, []);
});

test('docs: a highlighted Phero block passes', () => {
  const html =
    '<pre data-language="mox" class="astro-code"><code>' +
    '<span class="hl-keyword">package</span></code></pre>';
  const { blocks, findings } = scanDocsHtml(html);
  assert.deepEqual(findings, []);
  assert.equal(blocks.length, 1);
  assert.equal(blocks[0].language, 'mox');
  assert.equal(blocks[0].highlighted, true);
});

test('docs: a raw data-language block is a finding', () => {
  const { blocks, findings } = scanDocsHtml('<pre data-language="ddd"><code>application Support;</code></pre>');
  assert.equal(blocks.length, 1);
  assert.equal(blocks[0].raw, true);
  assert.equal(findings.length, 1);
  assert.match(findings[0], /raw/);
});

test('docs: a page containing the missing marker is a finding', () => {
  const html =
    '<div data-phero-highlight="missing"></div>' +
    '<pre data-language="mox"><code><span class="hl-keyword">x</span></code></pre>';
  const { findings } = scanDocsHtml(html);
  assert.equal(findings.length, 1);
  assert.match(findings[0], /data-phero-highlight="missing"/);
});

test('docs: a missing-marked Phero block is not also reported raw', () => {
  const html = '<pre data-language="mox" data-phero-highlight="missing"><code>x</code></pre>';
  const { blocks, findings } = scanDocsHtml(html);
  assert.equal(blocks.length, 1);
  assert.equal(blocks[0].missing, true);
  assert.equal(blocks[0].raw, false);
  assert.equal(findings.length, 1);
});

test('docs: the marker mentioned in prose is not an attribute', () => {
  const html =
    '<p>The <code>data-phero-highlight="missing"</code> marker fails the gate.</p>' +
    '<pre data-language="mox"><code><span class="hl-keyword">x</span></code></pre>';
  const { findings } = scanDocsHtml(html);
  assert.deepEqual(findings, []);
});

test('docs: non-Phero fences are ignored', () => {
  const { blocks, findings } = scanDocsHtml('<pre data-language="sh"><code>cargo test</code></pre>');
  assert.deepEqual(blocks, []);
  assert.deepEqual(findings, []);
});

test('fence classification: Phero surfaces, exempt languages, unsupported ones', () => {
  for (const language of PHERO_LANGUAGES) assert.equal(classifyFenceLanguage(language), 'phero', language);
  for (const language of ['', 'text', 'txt', 'markdown', 'rust', 'sh', 'bash', 'json', 'html']) {
    assert.equal(classifyFenceLanguage(language), 'exempt', language);
  }
  for (const language of ['cobol', 'brainfuck', 'not-a-language']) {
    assert.equal(classifyFenceLanguage(language), 'unsupported', language);
  }
});

test('fence classification normalizes case and brace/dot spellings', () => {
  assert.equal(classifyFenceLanguage('MOX'), 'phero');
  assert.equal(classifyFenceLanguage('.mox'), 'phero');
  assert.equal(classifyFenceLanguage('{.mox}'), 'phero');
  assert.equal(classifyFenceLanguage('  bash  '), 'exempt');
});

test('codeBlockScripts finds a real script inside a pre and ignores escaped text', () => {
  assert.deepEqual(codeBlockScripts('<pre><code>&lt;script&gt;</code></pre>').hits, []);
  const { blocks, hits } = codeBlockScripts(
    '<pre><code>safe</code></pre><pre><script>alert(1)</script></pre>',
  );
  assert.equal(blocks, 2);
  assert.deepEqual(hits, [1]);
});
