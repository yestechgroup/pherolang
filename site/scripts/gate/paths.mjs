/**
 * Every path and origin the gate reasons about, in one place.
 *
 * Nothing here is discovered at run time: the two apps, their built trees and
 * their origins are the contract the deploy workflow (`site.yml`, integrator
 * owned) already assumes. A check that cannot find one of these directories
 * reports that fact instead of inventing a fallback.
 */

import os from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/** `site/scripts/gate` */
export const HERE = dirname(fileURLToPath(import.meta.url));
/** `site/` */
export const SITE = resolve(HERE, '..', '..');
/** repository root */
export const REPO = resolve(SITE, '..');

/**
 * The two deployed origins and the trees that back them.
 *
 * `minPages` is the floor, not the target: it is the number of routes the
 * Phase-1 scope commits to (nine language pages + catalogue + homepage +
 * platform x2 + getting started + install + architecture + demo + about + 404
 * for marketing; 21 ingested documents + landing + 404 + the ADR index for
 * docs). A page that disappears takes the count under the floor.
 */
export const APPS = {
  web: {
    key: 'web',
    label: 'marketing',
    packageDir: join(SITE, 'phero-web'),
    dist: join(SITE, 'phero-web', 'dist', 'client'),
    origin: 'https://pherolang.com',
    minPages: 19,
  },
  docs: {
    key: 'docs',
    label: 'docs',
    packageDir: join(SITE, 'phero-docs'),
    dist: join(SITE, 'phero-docs', 'dist', 'client'),
    origin: 'https://docs.pherolang.com',
    minPages: 24,
  },
};

/** Hostname -> app key. A link to one of these resolves against the other tree. */
export const ORIGIN_APP = {
  'pherolang.com': 'web',
  'www.pherolang.com': 'web',
  'docs.pherolang.com': 'docs',
};

/** Where the redirect contract lives, in source and in the built tree. */
export const REDIRECTS_SOURCE = join(APPS.web.packageDir, 'public', '_redirects');
export const REDIRECTS_BUILT = join(APPS.web.dist, '_redirects');

/** Generated data — read-only for this lane. */
export const DATA = {
  surfaces: join(SITE, 'data', 'surfaces.json'),
  evidence: join(SITE, 'data', 'evidence.json'),
  docsManifest: join(SITE, 'data', 'docs-manifest.json'),
  highlight: join(SITE, 'data', 'highlighting', 'spans.json'),
  dataDir: join(SITE, 'data'),
};

/** The shell commands the two install pages print, and `check-commands.mjs` runs. */
export const COMMANDS = {
  manifest: join(SITE, 'phero-web', 'src', 'data', 'commands.json'),
  script: join(SITE, 'scripts', 'check-commands.mjs'),
};

export const TOKENS = {
  contrast: join(SITE, 'packages', 'tokens', 'contrast.json'),
  css: join(SITE, 'packages', 'tokens', 'src', 'tokens.css'),
};

/** The released compiler the data pipeline is generated with. */
export const PHEROLANG_BIN = join(
  SITE,
  '.bin',
  process.platform === 'win32' ? 'pherolang.exe' : 'pherolang',
);

/** Client-JS budget for the marketing app, gzipped. */
export const JS_BUDGET_BYTES = 12 * 1024;

/**
 * Reports go to the OS temp directory, never into the repository: `site/`
 * is the integrator's, and a gate that writes into the tree it is measuring
 * cannot be trusted to report on it.
 */
export const REPORT_DIR = join(os.tmpdir(), 'pherolang-site-gate');
