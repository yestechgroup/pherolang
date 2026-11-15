/**
 * Check 7 — coverage, the honesty harness.
 *
 * Four intersections have to be complete before the site can be called a
 * projection of the toolchain rather than a description of it:
 *
 *   1. every surface in `data/surfaces.json` has a `/languages/<id>/` page;
 *   2. every URL in `data/docs-manifest.json` exists in the docs build — the
 *      manifest is the docs URL contract, so a page that builds at a different
 *      path than the manifest promises is a broken contract even when the site
 *      looks fine (nothing else ever checks the promise);
 *   3. every command id in `commands.json` is one `check-commands.mjs` knows
 *      about, and that script executes clean;
 *   4. every backend crate a surface claims exists in `data/evidence.json`,
 *      which is itself derived from `crates/*\/Cargo.toml`.
 *
 * A surface that vanishes from one of these sources and survives in another is
 * exactly the drift this check exists to catch, so set equality is asserted
 * wherever both sides are enumerable.
 */

import { join } from 'node:path';

import { APPS, COMMANDS, DATA, SITE } from './paths.mjs';
import { resolveInTree } from './links.mjs';
import { isFile, plural, readJson, sh, tail } from './util.mjs';

const NAME = 'coverage';

/** Command ids printed by `check-commands.mjs --list`, in output order. */
function listedCommandIds(output) {
  const ids = [];
  for (const line of output.split('\n')) {
    const match = /^(?:PASS|FAIL|EXEC|SKIP)\s+(\S+)/.exec(line.trim());
    if (match) ids.push(match[1]);
  }
  return ids;
}

export const check = {
  name: NAME,
  title: 'surfaces, docs URLs, commands and backend crates are all covered',

  run() {
    const lines = [];
    const problems = [];

    // --- 1. surfaces -> language pages ---------------------------------
    const surfaces = readJson(DATA.surfaces).surfaces ?? [];
    const evidence = readJson(DATA.evidence);
    const catalogue = join(APPS.web.dist, 'languages', 'index.html');
    if (!isFile(catalogue)) problems.push(`marketing: /languages/ catalogue page is missing`);

    const missingPages = [];
    for (const surface of surfaces) {
      const page = join(APPS.web.dist, 'languages', surface.id, 'index.html');
      if (!isFile(page)) missingPages.push(`/languages/${surface.id}/ (surface "${surface.name}")`);
    }
    if (surfaces.length === 0) problems.push('data/surfaces.json lists no surfaces');
    for (const missing of missingPages) problems.push(`marketing: no page for ${missing}`);
    lines.push(
      `${missingPages.length === 0 ? 'ok  ' : 'FAIL'} surfaces: ${surfaces.length - missingPages.length}/${surfaces.length} ` +
        `have /languages/<id>/index.html`,
    );

    // The two enumerable sides must agree, not merely intersect.
    const evidenceIds = (evidence.surfaces ?? []).map((surface) => surface.id);
    const surfaceIds = surfaces.map((surface) => surface.id);
    const onlyInSurfaces = surfaceIds.filter((id) => !evidenceIds.includes(id));
    const onlyInEvidence = evidenceIds.filter((id) => !surfaceIds.includes(id));
    for (const id of onlyInSurfaces) {
      problems.push(`data/surfaces.json declares "${id}" but data/evidence.json has no row for it`);
    }
    for (const id of onlyInEvidence) {
      problems.push(`data/evidence.json has a row for "${id}" but data/surfaces.json does not declare it`);
    }
    if (onlyInSurfaces.length === 0 && onlyInEvidence.length === 0) {
      lines.push(`ok   evidence: the same ${evidenceIds.length} surface ids appear in both generated files`);
    }

    // --- 2. docs manifest -> built docs tree ---------------------------
    const manifest = readJson(DATA.docsManifest).generated ?? [];
    const unresolved = [];
    for (const entry of manifest) {
      const resolved = resolveInTree(APPS.docs.dist, entry.url);
      if (!resolved.file) {
        unresolved.push(`${entry.url} (from ${entry.source}, target ${entry.target}) — ${resolved.reason}`);
      }
    }
    if (manifest.length === 0) problems.push('data/docs-manifest.json lists no generated documents');
    for (const entry of unresolved) problems.push(`docs: manifest url does not resolve: ${entry}`);
    lines.push(
      `${unresolved.length === 0 ? 'ok  ' : 'FAIL'} docs: ${manifest.length - unresolved.length}/${manifest.length} ` +
        'manifest URLs resolve in dist/client',
    );

    // --- 3. commands.json <-> check-commands.mjs -----------------------
    if (!isFile(COMMANDS.manifest)) {
      problems.push(`${COMMANDS.manifest} is missing`);
    } else if (!isFile(COMMANDS.script)) {
      problems.push(`${COMMANDS.script} is missing`);
    } else {
      const declared = (readJson(COMMANDS.manifest).commands ?? []).map((entry) => entry.id);

      const listed = sh('node', [COMMANDS.script, '--list'], { cwd: SITE, timeout: 120_000 });
      if (listed.status !== 0) {
        problems.push(`check-commands.mjs --list exited ${listed.status}`);
        if (listed.stderr.trim()) lines.push(tail(listed.stderr, 10).replace(/^/gm, '     | '));
      } else {
        const known = new Set(listedCommandIds(listed.stdout));
        const unlisted = declared.filter((id) => !known.has(id));
        const extra = [...known].filter((id) => !declared.includes(id));
        for (const id of unlisted) {
          problems.push(`commands.json declares "${id}" but check-commands.mjs never lists it`);
        }
        for (const id of extra) {
          problems.push(`check-commands.mjs lists "${id}" but commands.json does not declare it`);
        }
        lines.push(
          `${unlisted.length === 0 && extra.length === 0 ? 'ok  ' : 'FAIL'} commands: ` +
            `${declared.length} declared ids, ${known.size} listed by the executor`,
        );
      }

      if (!isFile(join(SITE, '.bin', process.platform === 'win32' ? 'pherolang.exe' : 'pherolang'))) {
        problems.push(
          'no released binary at site/.bin/pherolang — run `node scripts/fetch-pherolang.mjs` (the executor runs the printed commands against it)',
        );
      } else {
        const executed = sh('node', [COMMANDS.script], { cwd: SITE, timeout: 600_000 });
        const summary = executed.stdout.trim().split('\n').filter((line) => line !== '');
        const totals = summary[summary.length - 1] ?? '';
        if (executed.status !== 0) {
          problems.push(`check-commands.mjs exited ${executed.status} — ${totals || 'no summary'}`);
          for (const line of summary.filter((line) => line.startsWith('FAIL'))) {
            lines.push(`     ${line}`);
          }
          if (executed.stderr.trim()) lines.push(tail(executed.stderr, 20).replace(/^/gm, '     | '));
        } else {
          lines.push(`ok   commands: the executor ran them all — ${totals}`);
        }
      }
    }

    // --- 4. claimed backends exist in the derived crate list -----------
    const crates = new Set(evidence.crates ?? []);
    const missingCrates = [];
    for (const surface of surfaces) {
      for (const backend of surface.backends ?? []) {
        if (!crates.has(backend)) missingCrates.push(`${surface.id} claims ${backend}`);
      }
    }
    for (const missing of missingCrates) {
      problems.push(`backend crate not in data/evidence.json: ${missing}`);
    }
    if ((evidence.crates ?? []).length === 0) problems.push('data/evidence.json lists no crates');
    lines.push(
      `${missingCrates.length === 0 ? 'ok  ' : 'FAIL'} backends: every crate a surface claims is in the ` +
        `${crates.size}-crate evidence list`,
    );

    // --- evidence counts the pages quote ------------------------------
    const graph = (() => {
      try {
        return readJson(join(DATA.dataDir, 'graph.json'));
      } catch {
        return null;
      }
    })();
    if (graph) {
      const nodes = (graph.nodes ?? []).length;
      const edges = (graph.edges ?? []).length;
      const counts = evidence.counts ?? {};
      if (counts.nodes !== nodes || counts.edges !== edges) {
        problems.push(
          `data/evidence.json counts (${counts.nodes} nodes, ${counts.edges} edges) disagree with data/graph.json (${nodes} nodes, ${edges} edges) — pages print the evidence counts`,
        );
      } else {
        lines.push(`ok   evidence counts match graph.json: ${plural(nodes, 'node')}, ${plural(edges, 'edge')}`);
      }
    }

    if (problems.length > 0) {
      lines.push(`FAIL ${plural(problems.length, 'coverage problem')}:`);
      for (const problem of problems) lines.push(`     ${problem}`);
      return { status: 'fail', lines };
    }
    return { status: 'pass', lines };
  },
};

export { listedCommandIds };
