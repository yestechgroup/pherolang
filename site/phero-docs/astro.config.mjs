import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import cloudflare from '@astrojs/cloudflare';

/**
 * docs.pherolang.com — Starlight over the repository's own `docs/`.
 *
 * The pages are NOT committed here: `scripts/ingest-docs.mjs` copies
 * `<repo>/docs/*.md` into `src/content/docs/reference/` before the build and
 * rewrites their relative links, so the reference cannot drift from the
 * repository. See `site/data/CONTRACT.md`.
 *
 * `session: false` and `imageService: 'compile'` keep the deploy assets-only —
 * no KV namespace, no Images binding.
 *
 * Lane 1 owns this file's `sidebar` (it knows the ingested tree); the shape
 * below is the seeded minimum that builds before ingestion lands.
 */
export default defineConfig({
  site: 'https://docs.pherolang.com',
  output: 'static',
  session: false,
  trailingSlash: 'ignore',
  adapter: cloudflare({ imageService: 'compile' }),
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
      editLink: {
        baseUrl: 'https://github.com/yestechgroup/pherolang/edit/main/docs/',
      },
      lastUpdated: true,
      pagefind: true,
      sidebar: [
        {
          label: 'Overview',
          items: [{ label: 'Documentation', link: '/' }],
        },
      ],
    }),
  ],
});
