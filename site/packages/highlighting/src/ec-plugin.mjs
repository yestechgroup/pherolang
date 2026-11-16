/**
 * Expressive Code plugin: compiler-backed classes inside EC's own structure.
 *
 * Expressive Code keeps ownership of frames, the copy button, gutters/line
 * numbers, text markers and theme switching. This plugin only splits the
 * rendered text nodes of a Phero Lang block and wraps the compiler's byte
 * ranges in `hl-*` spans. Nothing is re-rendered, so EC's UX is preserved.
 *
 * The plugin is a plain object (no runtime import of EC types) so it can be
 * loaded from `site/phero-docs/ec.config.mjs` without adding a dependency.
 */

import { loadArtifact, lookupEntry } from './artifact.mjs';
import { buildByteMap, piecesForRange, utf8Length } from './render.mjs';
import { classNamesFor, isPheroSurface } from './taxonomy.mjs';

function classList(value) {
  if (Array.isArray(value)) return value;
  if (typeof value === 'string') return value.split(/\s+/).filter(Boolean);
  return [];
}

/** The `.code` container of one rendered line, if present. */
function findCodeElement(node) {
  if (!node || node.type !== 'element') return null;
  if (classList(node.properties?.className).includes('code')) return node;
  for (const child of node.children ?? []) {
    const found = findCodeElement(child);
    if (found) return found;
  }
  return null;
}

function textNodes(node, out = []) {
  for (const child of node.children ?? []) {
    if (child.type === 'text') out.push(child);
    else if (child.type === 'element') textNodes(child, out);
  }
  return out;
}

/**
 * Replace text nodes in `parent` with pieces of the same text, wrapping the
 * ranges that carry a class. `pieces` cover the line exactly, in order.
 */
function applyPieces(parent, pieces) {
  let offset = 0;
  let pointer = 0;
  let ok = true;

  const visit = (node) => {
    if (!Array.isArray(node.children)) return;
    const next = [];
    for (const child of node.children) {
      if (child.type === 'text') {
        const value = child.value ?? '';
        const start = offset;
        const end = start + value.length;
        let local = start;
        while (local < end) {
          while (pointer < pieces.length && pieces[pointer].end <= local) pointer += 1;
          const piece = pieces[pointer];
          if (!piece || piece.start > local) {
            ok = false;
            break;
          }
          const stop = Math.min(end, piece.end);
          if (stop <= local) {
            ok = false;
            break;
          }
          const text = value.slice(local - start, stop - start);
          if (piece.classNames) {
            next.push({
              type: 'element',
              tagName: 'span',
              properties: { className: piece.classNames.split(' ') },
              children: [{ type: 'text', value: text }],
            });
          } else {
            next.push({ type: 'text', value: text });
          }
          local = stop;
        }
        offset = end;
      } else {
        if (child.type === 'element') visit(child);
        next.push(child);
      }
    }
    node.children = next;
  };

  visit(parent);
  return ok && offset === pieces[pieces.length - 1]?.end;
}

export function pheroHighlightPlugin() {
  /** Per-code-block analysis; WeakMap so the render lifetime owns it. */
  const analysis = new WeakMap();
  const reported = new Set();
  let artifactState;

  const artifact = () => {
    if (artifactState === undefined) artifactState = loadArtifact();
    return artifactState;
  };

  const report = (message) => {
    if (reported.has(message)) return;
    reported.add(message);
    console.warn(`[pherolang-highlight] ${message}`);
  };

  const lineOffsets = (codeBlock) => {
    const lines = codeBlock.getLines();
    const offsets = [];
    let offset = 0;
    for (let index = 0; index < lines.length; index += 1) {
      offsets.push(offset);
      offset += utf8Length(lines[index].text) + (index < lines.length - 1 ? 1 : 0);
    }
    return offsets;
  };

  return {
    name: 'pherolang-highlight',
    hooks: {
      postprocessAnalyzedCode({ codeBlock }) {
        if (!isPheroSurface(codeBlock.language)) return;
        const source = codeBlock.code;
        const { artifact: data, error } = artifact();
        if (!data) {
          report(`${error} — Phero blocks will render as plaintext until the artifact is regenerated.`);
          analysis.set(codeBlock, { status: 'missing' });
          return;
        }
        const { entry, error: lookupError } = lookupEntry(data, codeBlock.language, source);
        if (lookupError) {
          report(lookupError);
          analysis.set(codeBlock, { status: 'missing' });
          return;
        }
        if (!entry) {
          report(
            `no ${codeBlock.language} span entry for a ${source.length}-character block — ` +
              'run node scripts/sync-highlight.mjs',
          );
          analysis.set(codeBlock, { status: 'missing' });
          return;
        }
        analysis.set(codeBlock, {
          status: 'ok',
          entry,
          source,
          byteMap: buildByteMap(source),
          offsets: lineOffsets(codeBlock),
        });
      },

      postprocessRenderedLine({ codeBlock, line, lineIndex, renderData }) {
        const info = analysis.get(codeBlock);
        if (!info || info.status !== 'ok') return;

        const lineStart = info.offsets[lineIndex] ?? 0;
        const lineEnd = lineStart + utf8Length(line.text);
        const pieces = piecesForRange(
          info.source,
          info.entry.spans,
          info.byteMap,
          lineStart,
          lineEnd,
          classNamesFor,
        );
        if (pieces.length === 0) return;

        const code = findCodeElement(renderData.lineAst);
        if (!code) {
          info.status = 'incomplete';
          return;
        }

        const rendered = textNodes(code)
          .map((node) => node.value ?? '')
          .join('');
        if (rendered !== line.text) {
          info.status = 'incomplete';
          return;
        }

        if (!applyPieces(code, pieces)) info.status = 'incomplete';
      },

      postprocessRenderedBlock({ codeBlock, renderData }) {
        const info = analysis.get(codeBlock);
        if (!info) return;
        const block = renderData.blockAst;
        if (!block.properties) block.properties = {};
        block.properties['data-phero-highlight'] =
          info.status === 'ok' ? codeBlock.language : info.status;
      },
    },
  };
}
