import { fileURLToPath } from 'node:url';

import { defineConfig } from 'astro/config';
import cloudflare from '@astrojs/cloudflare';
import mdx from '@astrojs/mdx';
import sitemap from '@astrojs/sitemap';
import tailwindcss from '@tailwindcss/vite';

/**
 * pherolang.com — static-first, zero client JS except the graph island.
 *
 * `session: false` matters: without it the Cloudflare adapter injects a
 * `SESSION` KV binding into the deploy config, which would require a KV
 * namespace in the account. The site never uses sessions.
 *
 * `imageService: 'compile'` keeps image optimisation at build time so the
 * Worker needs no Images binding either — the deploy is assets only.
 *
 * The `@ui` / `@tokens` / `@data` aliases point at real source directories
 * (not workspace packages) so Vite compiles the shared components instead of
 * treating them as pre-built dependencies.
 */
const ui = fileURLToPath(new URL('../packages/ui/src', import.meta.url));
const tokens = fileURLToPath(new URL('../packages/tokens/src', import.meta.url));
const data = fileURLToPath(new URL('../data', import.meta.url));

export default defineConfig({
  site: 'https://pherolang.com',
  output: 'static',
  session: false,
  trailingSlash: 'ignore',
  build: { inlineStylesheets: 'auto' },
  adapter: cloudflare({ imageService: 'compile', prerenderEnvironment: 'node' }),
  integrations: [mdx(), sitemap()],
  vite: {
    plugins: [tailwindcss()],
    resolve: {
      alias: {
        '@ui': ui,
        '@tokens': tokens,
        '@data': data,
      },
    },
  },
});
