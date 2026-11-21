import { readFileSync } from 'node:fs';

import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import cloudflare from '@astrojs/cloudflare';

/**
 * docs.pherolang.com — Starlight over the repository's own `docs/`.
 *
 * The pages are NOT committed here: `scripts/ingest-docs.mjs` copies
 * `<repo>/docs/**` into `src/content/docs/{reference,engineering,migration,adr}/`
 * before every `dev`/`build`/`check` and rewrites their relative links, so the
 * reference cannot drift from the repository. See `site/data/CONTRACT.md`.
 *
 * `session: false` and `imageService: 'compile'` keep the deploy assets-only —
 * no KV namespace, no Images binding.
 *
 * The sidebar below is the whole of the docs information architecture:
 *
 *   Overview     the docs home, generated from `docs/README.md`
 *   Guide        user-facing Diátaxis pages, grouped by language from the
 *                content-model manifest (`site/data/docs-manifest.json`)
 *   Reference    the language manual, one page per surface
 *   Engineering  what is claimed, how it is proved, what a release ships
 *   Contributing the documentation standard (docs/documentation/)
 *   Migration    moving a 0.2 consumer, then cutting the current release
 *   Decisions    the numbered, immutable records
 *
 * Every group autogenerates one directory the ingestion step owns, so a new
 * `docs/*.md` appears without editing this file. Reading order is not sorted
 * alphabetically: each page carries `sidebar.order` in the frontmatter
 * synthesized by `ingest-docs.mjs` (Starlight's autogenerate sorts by it), and
 * labels are each page's own frontmatter `title`.
 *
 * The Guide group is the one exception: its pages live in nested language
 * directories, and Starlight labels nested autogenerate groups with the raw
 * directory name. It is therefore built from the manifest, which carries each
 * page's `language`, `type`, `order`, and `title` straight from the content
 * model. Nothing here is authored: the manifest is generated before every
 * `dev`/`build`/`check`.
 *
 * There is no `editLink` here on purpose: the generated copy is untracked, so a
 * repository-relative edit link would point at a path that does not exist.
 * Every page instead declares `editUrl` in its frontmatter, pointing at the
 * `docs/` source it was generated from.
 */

/**
 * GFM task lists render `<input type="checkbox" disabled>` with no accessible
 * name. They are decorative — the list text carries the meaning and the boxes
 * cannot be toggled — so hide them from the accessibility tree instead of
 * inventing a label for a control nobody can operate.
 */
const rehypeDecorativeTaskCheckboxes = () => (tree) => {
  const walk = (node) => {
    if (
      node?.type === 'element' &&
      node.tagName === 'input' &&
      node.properties?.type === 'checkbox' &&
      node.properties?.disabled !== undefined
    ) {
      node.properties['aria-hidden'] = 'true';
      node.properties.tabIndex = -1;
    }
    for (const child of node?.children ?? []) walk(child);
  };
  walk(tree);
};

/** Language group labels and order for the Guide sidebar. */
const GUIDE_LANGUAGES = [
  ['mox', 'Domain models (.mox)'],
  ['ddd', 'Application design (.ddd)'],
  ['evt', 'Event contracts (.evt)'],
  ['dmn', 'Decision models (.dmn)'],
  ['cmmn', 'Case management (.cmmn)'],
  ['ifml', 'Interaction flows (.ifml)'],
  ['uc', 'Use cases (.uc)'],
  ['deploy', 'Deployment models (.deploy)'],
  ['intent', 'Intent specifications (.intent)'],
  ['platform', 'Platform and tooling'],
];
const GUIDE_LANGUAGE_ORDER = GUIDE_LANGUAGES.map(([id]) => id);
const GUIDE_LANGUAGE_LABEL = Object.fromEntries(GUIDE_LANGUAGES);

/** Within a language: overview first, then learn, then do, then look up. */
const GUIDE_TYPE_RANK = {
  overview: 0,
  concept: 1,
  tutorial: 2,
  'how-to': 3,
  reference: 4,
  integration: 5,
};

/**
 * Build the Guide sidebar groups from the content-model manifest. Returns an
 * empty array when the manifest is absent or carries no Guide pages, so the
 * site still builds before the first Guide page lands.
 */
function guideGroups() {
  let generated = [];
  try {
    const manifest = JSON.parse(
      readFileSync(new URL('../data/docs-manifest.json', import.meta.url), 'utf8'),
    );
    generated = manifest.generated ?? [];
  } catch {
    return [];
  }

  const guide = generated.filter((page) => page.collection === 'guide');
  if (guide.length === 0) return [];

  const byLanguage = new Map();
  for (const page of guide) {
    const language = page.language ?? 'platform';
    if (!byLanguage.has(language)) byLanguage.set(language, []);
    byLanguage.get(language).push(page);
  }

  return [...byLanguage.entries()]
    .sort(
      ([a], [b]) =>
        (GUIDE_LANGUAGE_ORDER.indexOf(a) + 1 || Number.MAX_SAFE_INTEGER) -
        (GUIDE_LANGUAGE_ORDER.indexOf(b) + 1 || Number.MAX_SAFE_INTEGER),
    )
    .map(([language, items]) => ({
      label: GUIDE_LANGUAGE_LABEL[language] ?? language,
      items: items
        .slice()
        .sort(
          (a, b) =>
            (GUIDE_TYPE_RANK[a.type] ?? 9) - (GUIDE_TYPE_RANK[b.type] ?? 9) ||
            (a.order ?? 0) - (b.order ?? 0) ||
            String(a.title).localeCompare(String(b.title)),
        )
        .map((page) => ({ label: page.title, link: page.url })),
    }));
}

const guide = guideGroups();

export default defineConfig({
  site: 'https://docs.pherolang.com',
  output: 'static',
  session: false,
  trailingSlash: 'ignore',
  adapter: cloudflare({ imageService: 'compile' }),
  markdown: { rehypePlugins: [rehypeDecorativeTaskCheckboxes] },
  integrations: [
    starlight({
      title: 'Phero Lang',
      description:
        'Documentation for Phero Lang — the languages, the artifacts they compile to, and the toolchain.',
      customCss: ['./src/styles/theme.css'],
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/yestechgroup/pherolang',
        },
      ],
      lastUpdated: true,
      pagefind: true,
      pagination: true,
      sidebar: [
        { label: 'Overview', slug: 'index' },
        ...(guide.length > 0 ? [{ label: 'Guide', items: guide }] : []),
        {
          label: 'Reference',
          items: [{ autogenerate: { directory: 'reference' } }],
        },
        {
          label: 'Engineering',
          items: [{ autogenerate: { directory: 'engineering' } }],
        },
        {
          label: 'Contributing',
          items: [{ autogenerate: { directory: 'contributing' } }],
        },
        {
          label: 'Migration',
          items: [{ autogenerate: { directory: 'migration' } }],
        },
        {
          label: 'Decisions',
          items: [{ autogenerate: { directory: 'adr' } }],
        },
      ],
    }),
  ],
});
