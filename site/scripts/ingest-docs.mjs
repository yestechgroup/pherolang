#!/usr/bin/env node
/**
 * Ingest the repository's `docs/` tree into the Starlight content tree.
 *
 * The docs site is a **projection**, not a fork: `<repo>/docs/` is the only
 * copy of the reference prose, and this script copies it into
 * `site/phero-docs/src/content/docs/` (gitignored) before every `dev`,
 * `build` and `check`. Add a document to `docs/` and it appears on the site;
 * nothing here has to be edited. See `site/data/CONTRACT.md`.
 *
 * What it does, in order:
 *
 *   1. **Places** every document. `docs/README.md` becomes the landing page
 *      (`index.md`, URL `/`); everything else lands in one of four
 *      collections, which is what the sidebar groups are built from:
 *
 *          reference/    the language manual          → /reference/<slug>/
 *          engineering/  claims, proofs, receipts     → /engineering/<slug>/
 *          migration/    rebrand + release runbook    → /migration/<slug>/
 *          adr/          decision records             → /adr/<slug>/
 *
 *      The collection table is curated (`PLACEMENT`); a document that is not
 *      in it still ships — it falls back to `reference/`, ordered last — so a
 *      new `docs/*.md` can never be silently dropped.
 *
 *   2. **Synthesises frontmatter** from the document's own `# h1` (Starlight
 *      requires a title; the repo docs carry none), plus the reading order
 *      the sidebar sorts by and an `editUrl` pointing at the *source* file
 *      (the generated copy is untracked, so Starlight's default edit link
 *      would point at a path that does not exist).
 *
 *   3. **Rewrites links** so they resolve on docs.pherolang.com. The rewrite
 *      is resolved against the *source* path and emitted for the *output*
 *      page — see `resolveLink` and its rule table below. Relative links are
 *      emitted as site-root URLs (`/reference/ddd/`) rather than
 *      `./ddd/`-style ones: a static page is served at `/reference/ddd/`, so a
 *      browser resolves `./ddd/` from it as `/reference/ddd/ddd/`. Astro 7
 *      passes markdown link targets through verbatim, so the emitted form has
 *      to be absolute to be correct from any depth.
 *
 *   4. **Writes the landing page** from `docs/README.md` (`buildLandingPage`),
 *      because `site/data/CONTRACT.md` forbids authoring prose in this
 *      repository: the home page is derived from the repository's own index.
 *
 *   5. **Writes `site/data/docs-manifest.json`**, the generator's own
 *      coverage record, which the site gate reads.
 *
 * Usage: node scripts/ingest-docs.mjs
 */

import { existsSync, statSync } from 'node:fs';
import { mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, join, posix } from 'node:path';
import { fileURLToPath } from 'node:url';

const SITE = dirname(dirname(fileURLToPath(import.meta.url)));
const ROOT = dirname(SITE);
const DOCS = join(ROOT, 'docs');
const OUT = join(SITE, 'phero-docs', 'src', 'content', 'docs');
const INVENTORY = join(SITE, 'data', 'docs-inventory.json');
const MANIFEST = join(SITE, 'data', 'docs-manifest.json');

const REPO = 'https://github.com/yestechgroup/pherolang';
const TREE = `${REPO}/tree/main`;
const BLOB = `${REPO}/blob/main`;
const EDIT = `${REPO}/edit/main`;

const log = (message) => console.log(`ingest-docs: ${message}`);
const warn = (message) => console.warn(`ingest-docs: ${message}`);

// ---------------------------------------------------------------------------
// 1. The four collections, and where each document sits in them.
// ---------------------------------------------------------------------------

/**
 * Sidebar group labels, in sidebar order. `adr` is scanned from disk; the rest
 * are placements. The sidebar in `phero-docs/astro.config.mjs` autogenerates
 * each directory and labels the group with these strings.
 */
const GROUPS = ['reference', 'engineering', 'migration', 'adr'];

/**
 * `slug → [collection, reading order]`. The order is written into the page
 * frontmatter as `sidebar.order`, which is what Starlight's autogenerate sorts
 * by — so the reading order is curated here, once, and the sidebar needs no
 * second copy of the page list.
 */
const PLACEMENT = {
  // Reference — the language manual, in the order a reader meets it.
  language: ['reference', 10],
  expressions: ['reference', 20],
  ddd: ['reference', 30],
  events: ['reference', 40],
  deploy: ['reference', 50],
  usecases: ['reference', 60],
  cmmn: ['reference', 70],
  dmn: ['reference', 80],
  ifml: ['reference', 90],
  graph: ['reference', 100],
  diagnostics: ['reference', 110],
  backends: ['reference', 120],
  versioning: ['reference', 130],
  naming: ['reference', 140],
  highlighting: ['reference', 150],

  // Engineering — the claim ladder, the register, the shipped artifact.
  semantics: ['engineering', 10],
  verification: ['engineering', 20],
  receipts: ['engineering', 30],
  // A landed design RFC: compiler internals, not a user-facing surface, and
  // not one of the numbered decisions — engineering is its only honest home.
  'rune-bodies': ['engineering', 40],

  // Migration — moving a 0.2 consumer, then cutting the release.
  rebrand: ['migration', 10],
  'release-0.3.0': ['migration', 20],
};

const FALLBACK_GROUP = 'reference';
const FALLBACK_ORDER = 900;

/** The ADR collection is scanned from `docs/adr/`, not placed by slug. */
const ADR_GROUP = 'adr';
const ADR_ORDER = 10;

const GROUP_LABEL = {
  reference: 'Reference',
  engineering: 'Engineering',
  migration: 'Migration',
  adr: 'Decisions',
};

// ---------------------------------------------------------------------------
// 2. Source text helpers.
// ---------------------------------------------------------------------------

/** YAML-safe quoting for a synthesized frontmatter value (JSON is YAML). */
const yaml = (value) => JSON.stringify(String(value ?? ''));

/**
 * A meta description: the document's own opening paragraph, flattened to one
 * line of plain text and cut at a sentence boundary (else on a word boundary).
 * Long table prose would otherwise become a 900-character
 * `<meta name="description">`.
 */
function metaDescription(text, limit = 160) {
  const flat = text
    .replace(/[`*]/g, '')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/\s+/g, ' ')
    .trim();
  if (flat.length <= limit) return flat;

  // Prefer a whole sentence (the tag may run a little long; a fragment reads
  // worse than a complete thought), then fall back to a word boundary.
  const generous = flat.slice(0, limit + 40);
  for (const end of ['. ', '? ', '! ']) {
    const at = generous.lastIndexOf(end);
    if (at >= 80) return generous.slice(0, at + 1);
  }
  const window = flat.slice(0, limit);
  const word = window.lastIndexOf(' ');
  return `${window.slice(0, word > 80 ? word : limit - 1).replace(/[,;:]$/, '')}…`;
}

/** The `# h1` of a document, and the body with that heading removed. */
function splitTitle(text) {
  const lines = text.split('\n');
  const index = lines.findIndex((line) => /^#\s+\S/.test(line));
  if (index < 0) return { title: null, body: lines };
  const title = lines[index].replace(/^#\s+/, '').trim();
  return { title, body: [...lines.slice(0, index), ...lines.slice(index + 1)] };
}

/** The first prose paragraph after the heading — skipped over tables/lists. */
function firstParagraph(lines) {
  const paragraph = [];
  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed === '') {
      if (paragraph.length > 0) break;
      continue;
    }
    if (/^(?:[-*+]\s|\d+\.\s|\||>|`{3}|#{1,6}\s)/.test(trimmed)) {
      if (paragraph.length > 0) break;
      continue;
    }
    paragraph.push(trimmed);
  }
  return paragraph.join(' ');
}

/** Split a document body into its intro and its `##` sections. */
function splitSections(lines) {
  const intro = [];
  const sections = new Map();
  let current = null;
  for (const line of lines) {
    const heading = /^##\s+(?!#)(.*)$/.exec(line);
    if (heading) {
      current = heading[1].trim();
      sections.set(current, []);
      continue;
    }
    if (current === null) intro.push(line);
    else sections.get(current).push(line);
  }
  return { intro, sections };
}

// ---------------------------------------------------------------------------
// 3. Link resolution — the rule table.
// ---------------------------------------------------------------------------

/**
 * Every relative link in the corpus is one of these. `resolveLink` resolves the
 * target against the *source* document to a repository path, then looks that
 * path up in the generated page table:
 *
 * | In the source                          | Emitted                                       |
 * |----------------------------------------|-----------------------------------------------|
 * | `LANGUAGE.md` (from `docs/`)           | `/reference/language/`                        |
 * | `LANGUAGE.md#importing-rune-models`    | `/reference/language/#importing-rune-models`   |
 * | `adr/README.md`                        | `/adr/`                                       |
 * | `../NAMING.md` (from `docs/adr/`)      | `/reference/naming/`                          |
 * | `0001-….md` (from `docs/adr/`)         | `/adr/0001-…/`                                |
 * | `../examples/`, `../examples/README.md`| the GitHub **tree/blob** for that path        |
 * | `../crates/…/*.rs`, `../formal/…`      | the GitHub **blob**                           |
 * | `../AGENTS.md`, `../CHANGELOG.md`, …   | the GitHub **blob** at the repository root    |
 * | `../SECURITY.md`, `../LICENSE`, …      | the GitHub **blob** (the generic file rule)   |
 * | `https:`, `mailto:`, `//`, `/`, `#…`   | unchanged                                     |
 *
 * The generic last rule is why a document that starts linking at a repository
 * file this script has never seen still works: anything under `docs/` that is
 * a page becomes a page URL; anything else in the repository becomes a GitHub
 * URL; a directory becomes a `tree`, a file a `blob`.
 */
function resolveLink(target, page, pages) {
  // Absolute URLs, protocol-relative URLs, site-root paths and bare anchors.
  if (/^(?:[a-z][a-z0-9+.-]*:|\/\/|\/|#)/i.test(target)) return target;

  const match = /^([^?#]*)([?#].*)?$/.exec(target);
  if (!match) return target;
  const [, rawPath, suffix = ''] = match;
  if (rawPath === '') return target;

  const resolved = posix.normalize(posix.join(posix.dirname(page.source), rawPath));

  // A link that climbs out of the repository is not ours to guess at.
  if (resolved === '..' || resolved.startsWith('../')) return target;

  // (a) A generated page.
  const targetPage = pages.bySource.get(resolved);
  if (targetPage) return `${targetPage.url}${suffix}`;

  // (b) A directory that has an index page (`docs/`, `docs/adr/`).
  const indexPage = pages.byDir.get(resolved);
  if (indexPage) return `${indexPage.url}${suffix}`;

  // (c) Anything else in the repository: GitHub, tree for a directory, blob
  //     for a file. `statSync` decides for real paths; the trailing slash and
  //     the extension heuristic cover paths that do not exist (yet).
  const absolute = join(ROOT, resolved);
  const isDirectory = existsSync(absolute)
    ? statSync(absolute).isDirectory()
    : rawPath.endsWith('/') || (posix.extname(resolved) === '' && pages.rootDirs.has(resolved.split('/')[0]));

  return `${isDirectory ? TREE : BLOB}/${resolved}${suffix}`;
}

/** Rewrite every inline markdown link in a document body. */
function rewriteLinks(text, page, pages) {
  return text.replace(/\]\(([^()\s]+)\)/g, (whole, target) => {
    const href = resolveLink(target, page, pages);
    return href === target ? whole : `](${href})`;
  });
}

// ---------------------------------------------------------------------------
// 4. Plan the output tree.
// ---------------------------------------------------------------------------

if (!existsSync(INVENTORY)) {
  console.error(
    'ingest-docs: missing site/data/docs-inventory.json — run `node scripts/sync-model.mjs` first.',
  );
  process.exit(1);
}

const inventory = JSON.parse(await readFile(INVENTORY, 'utf8'));

/**
 * Starlight derives a page's slug from the content file's path, and it strips
 * dots: `release-0.3.0.md` becomes `/migration/release-030/`. Normalising here
 * keeps the manifest's promised URL and the emitted route identical — a
 * mismatch is a 404 that no other gate would see.
 */
const fileSlug = (slug) => slug.replace(/\./g, '');

/**
 * One page per ingested document. `source` is the repository-relative path the
 * document lives at (the link resolver's anchor), `target` is the
 * collection-relative output path, `url` the route a reader gets. `index` marks
 * a collection's own landing page (`index.md` at the collection root).
 */
async function planPage({ source, slug, title, order, collection: placed, index }) {
  const text = await readFile(join(ROOT, source), 'utf8');
  const split = splitTitle(text);
  const placement = PLACEMENT[slug];
  const landing = source === 'docs/README.md';
  const isIndex = index ?? landing;
  const collection = placed ?? placement?.[0] ?? FALLBACK_GROUP;
  const resolvedOrder = order ?? placement?.[1] ?? FALLBACK_ORDER;

  if (!placement && !placed && !landing) {
    warn(
      `${source} is not in the placement table — landing in ${collection}/, ordered last. ` +
        'Add it to PLACEMENT in scripts/ingest-docs.mjs.',
    );
  }

  // Starlight strips dots from the content id (`release-0.3.0.md` →
  // `/migration/release-030/`), so the file name — and therefore the URL the
  // manifest promises — must be normalised the same way.
  const emitted = fileSlug(slug);

  const target = landing
    ? 'index.md'
    : isIndex
      ? `${collection}/index.md`
      : `${collection}/${emitted}.md`;

  const url = landing ? '/' : isIndex ? `/${collection}/` : `/${collection}/${emitted}/`;

  return {
    source,
    slug,
    target,
    url,
    collection,
    index: isIndex,
    order: resolvedOrder,
    title: split.title ?? title,
    description: metaDescription(firstParagraph(split.body)),
    body: split.body.join('\n'),
  };
}

const pages = {
  all: [],
  bySource: new Map(),
  byDir: new Map(),
  rootDirs: new Set(),
};

for (const dir of await readdir(ROOT, { withFileTypes: true })) {
  if (dir.isDirectory() && !dir.name.startsWith('.') && dir.name !== 'target') {
    pages.rootDirs.add(dir.name);
  }
}

for (const doc of inventory.docs) {
  pages.all.push(
    await planPage({
      source: doc.path,
      slug: doc.slug,
      title: doc.title,
    }),
  );
}

// The ADR directory is not in `docs-inventory.json` (sync-model only walks
// `docs/*.md`), so it is scanned here. The index page leads the group; numbered
// records sort by their number, so a new `0002-….md` lands in place.
const ADR_DIR = join(DOCS, 'adr');
const adrFiles = existsSync(ADR_DIR)
  ? (await readdir(ADR_DIR)).filter((name) => name.endsWith('.md')).sort()
  : [];

for (const name of adrFiles) {
  const isIndex = name === 'README.md';
  const number = Number.parseInt(name, 10);
  pages.all.push(
    await planPage({
      source: `docs/adr/${name}`,
      slug: isIndex ? 'adr' : name.replace(/\.md$/, '').toLowerCase(),
      title: name,
      collection: ADR_GROUP,
      index: isIndex,
      order: isIndex ? 0 : ADR_ORDER + (Number.isNaN(number) ? adrFiles.length : number),
    }),
  );
}

// The link resolver's two indexes.
for (const page of pages.all) {
  pages.bySource.set(page.source, page);
  if (page.index) pages.byDir.set(posix.dirname(page.source), page);
}

// A document on disk that the inventory does not list would never ship.
const listed = new Set(inventory.docs.map((doc) => doc.path));
for (const name of await readdir(DOCS)) {
  if (name.endsWith('.md') && !listed.has(`docs/${name}`)) {
    warn(`docs/${name} is on disk but not in docs-inventory.json — skipping it.`);
    warn('Regenerate the inventory: node scripts/sync-model.mjs');
  }
}

// ---------------------------------------------------------------------------
// 5. The docs landing page.
// ---------------------------------------------------------------------------

/**
 * `docs/README.md` is a good index but a repository file: two routing tables
 * and a "this index routes to the right document" voice. The landing page
 * keeps its routing value and loses the table furniture:
 *
 *   intro paragraph (verbatim)  →  "Reading paths" (the README's own bullets)
 *   →  "The documents" (one link per page, grouped like the sidebar).
 *
 * Everything here is derived from `docs/README.md`, `docs-inventory.json` and
 * the placement table — no prose is authored in this repository. The only edit
 * to the borrowed prose is the link text: a reader on the docs site follows
 * "The .mox language reference", not "LANGUAGE.md".
 */
function buildLandingPage(readme, allPages) {
  const lines = readme.split('\n');
  const h1 = lines.findIndex((line) => /^#\s+\S/.test(line));
  const { intro, sections } = splitSections(h1 < 0 ? lines : lines.slice(h1 + 1));

  const landing = allPages.find((page) => page.source === 'docs/README.md');
  const titleByUrl = new Map(allPages.map((page) => [page.url, page.title]));

  const rewrite = (text) =>
    rewriteLinks(text, landing, pages)
      // `[LANGUAGE.md](/reference/language/)` -> the page's own title. Only
      // labels that name a repository file are replaced.
      .replace(/\[([^\]]+)\]\((\/[^)\s]+)\)/g, (whole, label, href) => {
        const title = titleByUrl.get(href.split(/[?#]/)[0]);
        return title && /\.md$/.test(label) ? `[${title}](${href})` : whole;
      })
      // `[../AGENTS.md](https://github.com/…)` -> `[AGENTS.md](…)`: a reader
      // does not care about the directory the file sits in.
      .replace(/\[(\.\.?\/[^\]]+)\]\((https:\/\/github\.com\/[^)\s]+)\)/g, (whole, label, href) => {
        const name = label.replace(/^\.\.?\//, '').replace(/\/$/, '');
        return `[${name}](${href})`;
      });

  const out = [];

  const introText = intro.join('\n').trim();
  // Drop a closing sentence that only describes the page's own routing role
  // ("This index routes to the right document.") — true of a repository file,
  // noise on a docs home. Nothing is added.
  const statement = introText
    .replace(/\s*(?:This|The) (?:index|page|document) (?:routes|lists|links)[^.]*\.\s*$/i, '')
    .trim();
  if (statement) out.push(rewrite(statement), '');

  // The reading paths are already a short link list; they just need the links
  // rewritten for the new depth.
  const paths = (sections.get('Reading paths') ?? []).join('\n').trim();
  if (paths) {
    out.push('## Reading paths', '', rewrite(paths), '');
  }

  // One bullet per page, grouped exactly like the sidebar. The trailing tag is
  // the "Surface" cell of the README's own table, when it has one.
  const surface = new Map();
  for (const line of (sections.get('The language') ?? [])) {
    const cells = line.split('|').slice(1, -1).map((cell) => cell.trim());
    if (cells.length < 2) continue;
    const link = /\]\(([^)]+)\)/.exec(cells[0]);
    if (!link) continue;
    surface.set(
      resolveLink(link[1], landing, pages),
      cells[1].replace(/\|/g, ''),
    );
  }

  out.push('## The documents', '');
  for (const group of GROUPS) {
    const inGroup = allPages
      .filter((page) => page.collection === group && page !== landing)
      .sort((a, b) => a.order - b.order || (a.slug < b.slug ? -1 : a.slug > b.slug ? 1 : 0));
    if (inGroup.length === 0) continue;
    out.push(`### ${GROUP_LABEL[group]}`, '');
    for (const page of inGroup) {
      const tag = surface.get(page.url);
      out.push(`- [${page.title}](${page.url})${tag ? ` — ${tag}` : ''}`);
    }
    out.push('');
  }

  return out.join('\n').trim();
}

// ---------------------------------------------------------------------------
// 6. Write the tree.
// ---------------------------------------------------------------------------

await rm(OUT, { recursive: true, force: true });
for (const group of GROUPS) {
  // Created even when a group is empty: the sidebar autogenerates each one, and
  // an autogenerated directory that does not exist is a build error.
  await mkdir(join(OUT, group), { recursive: true });
}

const readme = existsSync(join(DOCS, 'README.md'))
  ? await readFile(join(DOCS, 'README.md'), 'utf8')
  : null;

let written = 0;

for (const page of pages.all) {
  const landing = page.source === 'docs/README.md';
  const body = landing
    ? buildLandingPage(readme ?? '', pages.all)
    : rewriteLinks(page.body, page, pages).replace(/^\s+/, '');

  const frontmatter = [
    '---',
    `title: ${yaml(page.title)}`,
    page.description ? `description: ${yaml(page.description)}` : null,
    `editUrl: ${yaml(`${EDIT}/${page.source}`)}`,
    `sidebar: { order: ${landing ? 0 : page.order} }`,
    landing
      ? 'tableOfContents: { minHeadingLevel: 2, maxHeadingLevel: 2 }'
      : 'tableOfContents: { minHeadingLevel: 2, maxHeadingLevel: 3 }',
    '---',
  ]
    .filter((line) => line !== null)
    .join('\n');

  await writeFile(join(OUT, page.target), `${frontmatter}\n\n${body}\n`);
  written += 1;
}

// The landing page is generated from `docs/README.md`; this keeps the build
// green if that file ever disappears.
if (!existsSync(join(OUT, 'index.md'))) {
  await writeFile(
    join(OUT, 'index.md'),
    ['---', 'title: Phero Lang documentation', '---', '', 'The reference manual.', ''].join('\n'),
  );
  warn('docs/README.md was absent — wrote a fallback landing page');
}

// ---------------------------------------------------------------------------
// 7. The generator's own manifest (the site gate asserts coverage against it).
// ---------------------------------------------------------------------------

const manifest = pages.all
  .slice()
  .sort(
    (a, b) =>
      GROUPS.indexOf(a.collection) - GROUPS.indexOf(b.collection) ||
      a.order - b.order ||
      (a.slug < b.slug ? -1 : a.slug > b.slug ? 1 : 0),
  )
  .map((page) => ({
    source: page.source,
    slug: page.slug,
    collection: page.collection,
    target: page.target,
    url: page.url,
  }));

await writeFile(
  MANIFEST,
  `${JSON.stringify({ formatVersion: 1, generated: manifest }, null, 2)}\n`,
);

log(`wrote ${written} document(s) into ${OUT.replace(`${ROOT}/`, '')}`);
log(`wrote site/data/docs-manifest.json (${manifest.length} page(s))`);
