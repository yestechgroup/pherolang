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
 *   Reference    the language manual, one page per surface
 *   Engineering  what is claimed, how it is proved, what a release ships
 *   Migration    moving a 0.2 consumer, then cutting the current release
 *   Decisions    the numbered, immutable records
 *
 * Every group autogenerates one directory the ingestion step owns, so a new
 * `docs/*.md` appears without editing this file. Reading order is not sorted
 * alphabetically: each page carries `sidebar.order` in the frontmatter
 * synthesized by `ingest-docs.mjs` (Starlight's autogenerate sorts by it), and
 * labels are each page's own frontmatter `title`.
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
        {
          label: 'Reference',
          items: [{ autogenerate: { directory: 'reference' } }],
        },
        {
          label: 'Engineering',
          items: [{ autogenerate: { directory: 'engineering' } }],
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
