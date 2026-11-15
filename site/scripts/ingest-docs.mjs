#!/usr/bin/env node
/**
 * Ingest the repository's `docs/*.md` into the Starlight content tree.
 *
 * The docs site is generated, never duplicated: `<repo>/docs/` is the only
 * copy of the reference prose, and this script projects it into
 * `site/phero-docs/src/content/docs/` (gitignored) before every build. That is
 * the G4 gate in the epic made mechanical — if a document is added to `docs/`,
 * this script picks it up, and nothing has to be edited here.
 *
 * What it does:
 *   1. reads `site/data/docs-inventory.json` (emitted by sync-model.mjs) for
 *      each document's title and description;
 *   2. writes `reference/<slug>.md` with synthesized frontmatter — the repo
 *      docs carry an `# h1` and no YAML, and Starlight requires a title;
 *   3. rewrites relative links so they resolve on docs.pherolang.com:
 *        `../examples/x`   -> the GitHub tree
 *        `../CHANGELOG.md` -> the GitHub blob
 *        `SIBLING.md`      -> `./sibling/` inside the docs site
 *
 * Lane 1 owns this file's mapping table and the sidebar; the shape below is the
 * seeded minimum that makes the docs site build from the real corpus.
 *
 * Usage: node scripts/ingest-docs.mjs
 */

import { existsSync } from 'node:fs';
import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const SITE = dirname(dirname(fileURLToPath(import.meta.url)));
const ROOT = dirname(SITE);
const OUT = join(SITE, 'phero-docs', 'src', 'content', 'docs');
const INVENTORY = join(SITE, 'data', 'docs-inventory.json');

const REPO = 'https://github.com/yestechgroup/pherolang';
const TREE = `${REPO}/tree/main`;
const BLOB = `${REPO}/blob/main`;

const log = (message) => console.log(`ingest-docs: ${message}`);

if (!existsSync(INVENTORY)) {
  console.error(
    'ingest-docs: missing site/data/docs-inventory.json — run `node scripts/sync-model.mjs` first.',
  );
  process.exit(1);
}

const inventory = JSON.parse(await readFile(INVENTORY, 'utf8'));

/** YAML-safe quoting for a synthesized frontmatter value. */
function yaml(value) {
  return JSON.stringify(String(value ?? ''));
}

/**
 * Rewrite every repo-relative link into something a reader of
 * docs.pherolang.com can actually follow.
 */
function rewriteLinks(text) {
  return text
    // Repo paths that live outside the docs site.
    .replace(/\]\(\.\.\/examples\/?([^)\s]*)\)/g, (_, rest) => `](${TREE}/examples${rest ? `/${rest}` : ''})`)
    .replace(/\]\(\.\.\/CHANGELOG\.md\)/g, `](${BLOB}/CHANGELOG.md)`)
    .replace(/\]\(\.\.\/AGENTS\.md\)/g, `](${BLOB}/AGENTS.md)`)
    .replace(/\]\(\.\.\/README\.md\)/g, `](${BLOB}/README.md)`)
    .replace(/\]\(\.\.\/CONTRIBUTING\.md\)/g, `](${BLOB}/CONTRIBUTING.md)`)
    // Sibling documents: `LANGUAGE.md` -> `./language/`, `adr/README.md` -> `./adr/`.
    .replace(/\]\((?!https?:|\/|#)([\w./-]+)\.md\)/g, (whole, target) => {
      const withoutExtension = target.replace(/\.md$/, '');
      if (withoutExtension === 'README' || withoutExtension.endsWith('/README')) {
        const dir = withoutExtension.replace(/\/?README$/, '');
        return `](./${dir ? `${dir}/` : ''})`;
      }
      return `](./${withoutExtension.toLowerCase()}/)`;
    })
    // JSON / data files referenced from prose.
    .replace(/\]\((?!https?:|\/|#)([\w./-]+\.json)\)/g, `](${BLOB}/$1)`);
}

await rm(OUT, { recursive: true, force: true });
await mkdir(join(OUT, 'reference'), { recursive: true });

let written = 0;

for (const doc of inventory.docs) {
  const source = join(ROOT, doc.path);
  const body = await readFile(source, 'utf8');

  // Drop the leading `# Title` — Starlight renders the frontmatter title, and a
  // second h1 would be a duplicate heading.
  const withoutTitle = body.replace(/^#\s+.*\r?\n/, '');
  const content = rewriteLinks(withoutTitle).replace(/^\s+/, '');

  const frontmatter = [
    '---',
    `title: ${yaml(doc.title)}`,
    doc.description ? `description: ${yaml(doc.description)}` : null,
    'tableOfContents: { minHeadingLevel: 2, maxHeadingLevel: 3 }',
    '---',
    '',
  ]
    .filter((line) => line !== null)
    .join('\n');

  // README.md is the docs landing page; everything else is a reference page.
  const target =
    doc.slug === 'index'
      ? join(OUT, 'index.md')
      : join(OUT, 'reference', `${doc.slug}.md`);

  await writeFile(target, `${frontmatter}${content}\n`);
  written += 1;
}

// A generated landing page is not authored here: if docs/README.md exists it
// already routes the reader. This fallback keeps the build green if it ever
// disappears.
if (!existsSync(join(OUT, 'index.md'))) {
  await writeFile(
    join(OUT, 'index.md'),
    ['---', 'title: Phero Lang documentation', '---', '', 'The reference manual.', ''].join('\n'),
  );
  log('docs/README.md was absent — wrote a fallback landing page');
}

// The generator's own manifest, so the gate can assert coverage.
await writeFile(
  join(SITE, 'data', 'docs-manifest.json'),
  `${JSON.stringify(
    {
      formatVersion: 1,
      generated: inventory.docs.map((doc) => ({
        source: doc.path,
        slug: doc.slug,
        title: doc.title,
        target: doc.slug === 'index' ? 'index.md' : `reference/${doc.slug}.md`,
      })),
    },
    null,
    2,
  )}\n`,
);

log(`wrote ${written} document(s) into ${OUT.replace(`${ROOT}/`, '')}`);
