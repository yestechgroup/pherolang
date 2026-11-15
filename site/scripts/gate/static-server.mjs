/**
 * A static file server for one built tree, in about sixty lines.
 *
 * The `a11y` check must exercise what ships — `dist/client` — not a dev
 * server, and the two apps deploy as assets-only Workers, so "static host"
 * semantics are the whole contract: a URL maps to a file, a directory maps to
 * its `index.html`, and a miss is the 404 page with a 404 status. Wrangler
 * would only add a network hop and a dependency.
 *
 * Bound to 127.0.0.1 on an ephemeral port, so parallel gate runs never collide
 * and nothing is reachable off the machine.
 */

import { createReadStream, statSync } from 'node:fs';
import { createServer } from 'node:http';
import { extname, join, resolve, sep } from 'node:path';

const CONTENT_TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.xml': 'application/xml; charset=utf-8',
  '.txt': 'text/plain; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.webp': 'image/webp',
  '.avif': 'image/avif',
  '.ico': 'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.wasm': 'application/wasm',
  '.webmanifest': 'application/manifest+json',
  '.map': 'application/json; charset=utf-8',
};

const fileOrNull = (path) => {
  try {
    return statSync(path).isFile() ? path : null;
  } catch {
    return null;
  }
};

const directoryOrNull = (path) => {
  try {
    return statSync(path).isDirectory() ? path : null;
  } catch {
    return null;
  }
};

/**
 * Start serving `root` and return `{ origin, close }`.
 *
 * `close()` resolves once the listener is gone, so a failed check cannot leave
 * the event loop open and hang CI.
 */
export async function startStaticServer(root) {
  const base = resolve(root);

  const resolveRequest = (requestUrl) => {
    let pathname;
    try {
      pathname = decodeURIComponent(new URL(requestUrl, 'http://127.0.0.1').pathname);
    } catch {
      return { status: 400, file: null };
    }

    const relative = pathname === '/' ? 'index.html' : pathname.replace(/^\/+/, '');
    const target = resolve(join(base, relative));
    if (target !== base && !target.startsWith(`${base}${sep}`)) {
      return { status: 403, file: null };
    }

    const direct = fileOrNull(target);
    if (direct) return { status: 200, file: direct };

    const directory = directoryOrNull(target);
    if (directory) {
      const index = fileOrNull(join(directory, 'index.html'));
      if (index) return { status: 200, file: index };
    }

    const withExtension = fileOrNull(`${target}.html`);
    if (withExtension) return { status: 200, file: withExtension };

    return { status: 404, file: fileOrNull(join(base, '404.html')) };
  };

  const server = createServer((request, response) => {
    const { status, file } = resolveRequest(request.url ?? '/');
    if (!file) {
      response.writeHead(status, { 'content-type': 'text/plain; charset=utf-8' });
      response.end(`${status}\n`);
      return;
    }

    const type = CONTENT_TYPES[extname(file).toLowerCase()] ?? 'application/octet-stream';
    const headers = { 'content-type': type, 'cache-control': 'no-store' };
    if (request.method === 'HEAD') {
      response.writeHead(status, headers);
      response.end();
      return;
    }
    response.writeHead(status, headers);
    createReadStream(file).pipe(response);
  });

  await new Promise((done, fail) => {
    server.once('error', fail);
    server.listen(0, '127.0.0.1', done);
  });

  const { port } = server.address();
  return {
    origin: `http://127.0.0.1:${port}`,
    close: () => new Promise((done) => server.close(() => done())),
  };
}
