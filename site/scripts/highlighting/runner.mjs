/**
 * Resolve the `pherolang` binary the site tooling runs.
 *
 * The site contract (#343) is **released-binary freshness with no source
 * build**: the committed data is what the released binary generates, and a
 * release that predates the DSL on main produces the *named skip* — never a
 * fallback build that would make the data track unreleased DSL. Resolution:
 *
 *   1. `PHEROLANG_BIN`, when set, is the caller's deliberate choice and must
 *      satisfy the requested capability;
 *   2. `site/.bin/pherolang` (the checksum-verified release) when it can check
 *      the current examples and, when required, provides `highlight`;
 *   3. `pherolang` on PATH under the same rules;
 *   4. otherwise the result is `{ binary: null, kind: 'unavailable', reason }`
 *      and `reason` carries the named skip (tag + failing file). There is no
 *      step 4 build — the caller reports the skip.
 */

import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { join } from 'node:path';

import { probeRelease, skipMessage } from '../probe-release.mjs';

export function which(command) {
  try {
    return (
      execFileSync(process.platform === 'win32' ? 'where' : 'which', [command], {
        encoding: 'utf8',
        stdio: ['ignore', 'pipe', 'ignore'],
      })
        .split('\n')[0]
        .trim() || null
    );
  } catch {
    return null;
  }
}

export function resolveExecutable(value) {
  if (!value) return null;
  if (value.includes('/') || /^[A-Za-z]:[\\/]/.test(value)) return existsSync(value) ? value : null;
  return which(value);
}

/** True when the binary advertises the `highlight` subcommand. */
export function supportsHighlight(binary) {
  try {
    execFileSync(binary, ['highlight', '--help'], { stdio: 'ignore', timeout: 30_000 });
    return true;
  } catch {
    return false;
  }
}

/** The binary's own provenance line (`pherolang X.Y.Z (target)`) as a `vX.Y.Z` tag. */
export function versionTag(binary) {
  try {
    const banner = execFileSync(binary, ['--version'], { encoding: 'utf8', timeout: 30_000 });
    const match = /(\d+\.\d+\.\d+)/.exec(banner);
    return match ? `v${match[1]}` : 'the latest release';
  } catch {
    return 'the latest release';
  }
}

/**
 * Why a binary cannot regenerate the site data: the named skip when it
 * cannot compile `examples/`, otherwise the missing capability.
 */
export function incapabilityReason(binary, root, { requireHighlight }) {
  if (requireHighlight && !supportsHighlight(binary)) {
    return 'the binary does not provide `pherolang highlight`';
  }
  const verdict = probeRelease({ binary, examplesDir: join(root, 'examples') });
  if (!verdict.compatible) {
    return skipMessage({ tag: versionTag(binary), failingFile: verdict.failingFile });
  }
  return null;
}

/**
 * Resolve a binary that can check the tree and, when requested, highlight.
 *
 * Returns `{binary, kind}` where kind is `PHEROLANG_BIN`, `release`, `PATH`, or
 * `source-build`. Throws when an explicit `PHEROLANG_BIN` cannot do the job.
 */
export function resolveSiteBinary(root, site, { requireHighlight = false, log = console.log } = {}) {
  const explicit = process.env.PHEROLANG_BIN;
  if (explicit) {
    const binary = resolveExecutable(explicit);
    if (!binary) throw new Error(`PHEROLANG_BIN='${explicit}' does not exist`);
    const reason = incapabilityReason(binary, root, { requireHighlight });
    if (reason) {
      throw new Error(
        `PHEROLANG_BIN='${explicit}' cannot regenerate the site data: ${reason} ` +
          'Point it at a capable build — there is no source-build fallback (#343).',
      );
    }
    return { binary, kind: 'PHEROLANG_BIN' };
  }

  const candidates = [];
  const release = join(site, '.bin', process.platform === 'win32' ? 'pherolang.exe' : 'pherolang');
  if (existsSync(release)) candidates.push({ binary: release, kind: 'release' });
  const onPath = which('pherolang');
  if (onPath) candidates.push({ binary: onPath, kind: 'PATH' });

  let firstFailure = null;
  for (const candidate of candidates) {
    const reason = incapabilityReason(candidate.binary, root, { requireHighlight });
    if (!reason) return candidate;
    if (!firstFailure) {
      firstFailure = reason;
      log(`${candidate.kind} binary cannot regenerate the site data: ${reason}`);
    }
  }

  return {
    binary: null,
    kind: 'unavailable',
    reason:
      firstFailure ??
      'no pherolang binary found — fetch the release with: node scripts/fetch-pherolang.mjs',
  };
}
