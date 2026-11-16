/**
 * Narrow highlighting goldens, extracted from built HTML only.
 *
 * One fragment per app: the first Phero `mox` block that actually ships. The
 * fragment is compared byte-for-byte, so a class rename, a lost span, or a
 * changed wrapper is visible in review without storing a whole page.
 *
 * The CI runs this after both apps build. When a `dist/` tree is absent the
 * test skips with the path it wanted instead of failing: `node --test` is also
 * the cheap local loop, and a missing build is not a highlighting regression.
 *
 * Regenerate with `PHERO_UPDATE_GOLDENS=1` (from the repository root):
 *
 *   PHERO_UPDATE_GOLDENS=1 node --test site/scripts/test/
 */

import assert from 'node:assert/strict';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import test from 'node:test';

import { APPS, SITE } from '../gate/paths.mjs';

const GOLDEN_DIR = join(SITE, 'scripts', 'test', 'golden');
const UPDATE = process.env.PHERO_UPDATE_GOLDENS === '1';

const marketingPage = join(APPS.web.dist, 'languages', 'mox', 'index.html');
const docsPage = join(APPS.docs.dist, 'reference', 'language', 'index.html');

/** The first `<code class="language-mox">…</code>` in the built page. */
function firstMarketingCode(html) {
  const match = /<code\b[^>]*\bclass\s*=\s*"[^"]*\blanguage-mox\b[^"]*"[^>]*>[\s\S]*?<\/code>/i.exec(html);
  return match ? match[0] : null;
}

/** The first `<pre data-language="mox" …>…</pre>` in the built page. */
function firstDocsPre(html) {
  const match = /<pre\b[^>]*\bdata-language\s*=\s*"mox"[^>]*>[\s\S]*?<\/pre>/i.exec(html);
  return match ? match[0] : null;
}

function compareGolden(name, fragment) {
  const path = join(GOLDEN_DIR, name);
  if (UPDATE) {
    mkdirSync(GOLDEN_DIR, { recursive: true });
    writeFileSync(path, `${fragment}\n`, 'utf8');
  }
  assert.ok(
    existsSync(path),
    `missing golden ${path} — regenerate with PHERO_UPDATE_GOLDENS=1 node --test site/scripts/test/`,
  );
  assert.equal(`${fragment}\n`, readFileSync(path, 'utf8'), `${name} drifted from the built page`);
}

test('marketing: the first language-mox block matches its golden', (t) => {
  if (!existsSync(marketingPage)) {
    t.skip(`missing ${marketingPage} — build site/phero-web before running this test`);
    return;
  }
  const fragment = firstMarketingCode(readFileSync(marketingPage, 'utf8'));
  assert.ok(fragment, `${marketingPage} has no <code class="language-mox"> block`);
  compareGolden('marketing-language-mox.html', fragment);
});

test('docs: the first data-language="mox" block matches its golden', (t) => {
  if (!existsSync(docsPage)) {
    t.skip(`missing ${docsPage} — build site/phero-docs before running this test`);
    return;
  }
  const fragment = firstDocsPre(readFileSync(docsPage, 'utf8'));
  assert.ok(fragment, `${docsPage} has no <pre data-language="mox"> block`);
  compareGolden('docs-language-mox.html', fragment);
});
