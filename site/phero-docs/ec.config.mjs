import { defineEcConfig } from '@astrojs/starlight/expressive-code';
import { pheroHighlightPlugin } from '../packages/highlighting/src/ec-plugin.mjs';

/**
 * SH4, Option D: compiler-backed spans inside Expressive Code's own markup.
 *
 * The plugin never re-renders a block — it splits the text nodes EC already
 * produced and wraps the compiler's byte ranges in `hl-*` spans — so EC keeps
 * ownership of the frame, the copy button, line numbers, text markers, theme
 * switching and its accessibility behaviour. The alternative (a bespoke
 * Shiki/HTML pipeline) would have had to reimplement all of that.
 *
 * The `hl-*` rules themselves are shared with pherolang.com and live in
 * `packages/highlighting/src/highlight.css`, imported by `theme.css`.
 */
export default defineEcConfig({ plugins: [pheroHighlightPlugin()] });
