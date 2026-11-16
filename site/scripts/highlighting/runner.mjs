/**
 * Resolve the `pherolang` binary the site tooling runs.
 *
 * The released v0.3.0 binary predates both `pherolang highlight` and the DD9
 * examples on this tree, so it cannot be used to regenerate or verify the
 * committed site data. This module makes that explicit:
 *
 *   1. `PHEROLANG_BIN`, when set, is the caller's deliberate choice and must
 *      satisfy the requested capability;
 *   2. `site/.bin/pherolang` is used only when it can check the current support
 *      example (model data) and, when required, provides `highlight`;
 *   3. `pherolang` on PATH under the same rules;
 *   4. otherwise the CLI is built from this checkout
 *      (`cargo build -q -p phero-cli`).
 *
 * Step 4 is the documented temporary exception, never a claim of
 * released-binary freshness. See docs/HIGHLIGHTING.md and site/README.md.
 */

import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { join } from 'node:path';

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

/**
 * True when the binary can check the current support design.
 *
 * `support.ddd` imports `support.mox`, so this is a cheap, representative
 * probe of the DSL the tree uses. A release that predates the tree fails; a
 * source build of the same commit passes.
 */
export function canCheckTree(binary, root) {
  try {
    execFileSync(binary, ['check', 'examples/support/support.ddd'], {
      cwd: root,
      stdio: 'ignore',
      timeout: 120_000,
    });
    return true;
  } catch {
    return false;
  }
}

function sourceBuild(root, log) {
  log(
    'the fetched release cannot check the current examples (or lacks `pherolang highlight`); ' +
      'building the CLI from this commit (temporary exception — docs/HIGHLIGHTING.md)',
  );
  try {
    execFileSync('cargo', ['build', '-q', '-p', 'phero-cli'], {
      cwd: root,
      stdio: 'inherit',
      timeout: 900_000,
    });
  } catch (error) {
    throw new Error(`source build failed: ${error.message}`);
  }
  const binary = join(root, 'target', 'debug', process.platform === 'win32' ? 'pherolang.exe' : 'pherolang');
  if (!existsSync(binary)) throw new Error(`source build produced no binary at ${binary}`);
  return binary;
}

/**
 * Resolve a binary that can check the tree and, when requested, highlight.
 *
 * Returns `{binary, kind}` where kind is `PHEROLANG_BIN`, `release`, `PATH`, or
 * `source-build`. Throws when an explicit `PHEROLANG_BIN` cannot do the job.
 */
export function resolveSiteBinary(root, site, { requireHighlight = false, log = console.log } = {}) {
  const capability = (binary) =>
    (!requireHighlight || supportsHighlight(binary)) && canCheckTree(binary, root);

  const explicit = process.env.PHEROLANG_BIN;
  if (explicit) {
    const binary = resolveExecutable(explicit);
    if (!binary) throw new Error(`PHEROLANG_BIN='${explicit}' does not exist`);
    if (!capability(binary)) {
      throw new Error(
        `PHEROLANG_BIN='${explicit}' cannot ` +
          `${requireHighlight ? 'provide `pherolang highlight` and ' : ''}check the current examples. ` +
          'Point it at a build of this commit, or unset it to use the documented source fallback.',
      );
    }
    return { binary, kind: 'PHEROLANG_BIN' };
  }

  const release = join(site, '.bin', process.platform === 'win32' ? 'pherolang.exe' : 'pherolang');
  if (existsSync(release) && capability(release)) return { binary: release, kind: 'release' };
  if (existsSync(release)) {
    log(
      'the fetched release predates the current examples' +
        (requireHighlight ? ' or `pherolang highlight`' : '') +
        '; not using it.',
    );
  }

  const onPath = which('pherolang');
  if (onPath && capability(onPath)) return { binary: onPath, kind: 'PATH' };

  return { binary: sourceBuild(root, log), kind: 'source-build' };
}
