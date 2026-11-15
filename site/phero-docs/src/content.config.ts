import { defineCollection } from 'astro:content';
import { docsLoader } from '@astrojs/starlight/loaders';
import { docsSchema } from '@astrojs/starlight/schema';

/**
 * The `docs` collection is populated at build time by
 * `scripts/ingest-docs.mjs` from `<repo>/docs/*.md` — see `site/data/CONTRACT.md`.
 * It is gitignored: the repository's `docs/` is the only copy of the prose.
 */
export const collections = {
  docs: defineCollection({ loader: docsLoader(), schema: docsSchema() }),
};
