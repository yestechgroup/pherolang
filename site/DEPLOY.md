# Deploying the Phero Lang site

Two static Cloudflare Workers, one account, no server code. This page is the
operator runbook: what is committed, what has been verified locally, what the
GitHub workflow does with the account's credentials, and the exact order to
follow when the account owner connects Cloudflare.

Every command below was run in this repository unless a step is marked
**unverified** — nothing in this file describes a dashboard screen that has not
been seen or a flag that has not been checked against the installed Wrangler.

---

## 1. What ships

| | Marketing | Documentation |
|---|---|---|
| Directory | `site/phero-web` | `site/phero-docs` |
| Worker name (`wrangler.jsonc`) | `pherolang-web` | `pherolang-docs` |
| Origin | `https://pherolang.com` (and `www`) | `https://docs.pherolang.com` |
| Build output | `dist/client` | `dist/client` |
| Compatibility date | `2026-11-01` | `2026-11-01` |
| Runtime shape | static assets only | static assets only |
| 404 handling | `assets.not_found_handling: "404-page"` | same |
| Bindings | none | none |

Both apps set `output: 'static'`, `session: false` and
`adapter: cloudflare({ imageService: 'compile' })`. That combination is why the
deploy needs no KV namespace and no Images binding: sessions are off, so the
adapter injects no `SESSION` binding, and images are optimised at build time, so
the Worker never calls the Images API. The deploy is a file upload.

`wrangler.jsonc` is not the file Cloudflare reads at deploy time: the adapter
writes `dist/client/wrangler.json` with `assets.directory` rewritten to `"."`
(the client directory is the root) and Wrangler follows that redirect. Both
files are in the repository, so the redirect is inspectable.

The static-asset control files live in each app's `public/` directory and are
copied to `dist/client/` by the build:

| File | Purpose | Section |
|---|---|---|
| `_headers` | security headers, `Cache-Control`, preview noindex | [§6](#6-response-headers-and-the-csp) |
| `_redirects` (marketing only) | `/docs/*` → `docs.pherolang.com` | [§4](#4-custom-domains-and-the-docs-redirect) |
| `robots.txt` | crawl policy + sitemap pointer | [§7](#7-robotstxt-and-sitemaps) |

---

## 2. What is verified today

Run from `site/` after `fnm use` (the pin is `site/.nvmrc`, Node `24.16.0`) and
`pnpm install`. Wrangler resolves to `4.162.0` here.

**Both apps build.** `pnpm -C phero-web build` emits 3 pages plus
`sitemap-index.xml`; `pnpm -C phero-docs build` emits 24 pages, a
`404.html`, a Pagefind index and its own sitemap. Both write `dist/client`.

**The asset control files parse.** The build prints what the asset server
accepted, so a pattern it cannot read is visible in the build log rather than
only at the edge: `Parsed 3 valid header rules.` and `Parsed 2 valid redirect
rules.` for the marketing app, `Parsed 5 valid header rules.` for the docs app
(the `/*` baseline, the `/_astro/*` cache rule, the two Pagefind cache rules,
and the preview-hostname rule).

**Both dry-runs pass and report no bindings.** This is the whole deploy risk in
one line each — no KV, no Images, no D1, nothing to provision:

```console
$ pnpm -C phero-web exec wrangler deploy --dry-run
 ⛅️ wrangler 4.162.0
────────────────────
Using redirected Wrangler configuration.
 - Configuration being used: "dist/client/wrangler.json"
 - Original user's configuration: "wrangler.jsonc"
 - Deploy configuration file: ".wrangler/deploy/config.json"
✨ Read 24 files from the assets directory .../site/phero-web/dist/client
Total Upload: 0.36 KiB / gzip: 0.25 KiB
No bindings found.
--dry-run: exiting now.

$ pnpm -C phero-docs exec wrangler deploy --dry-run
 ⛅️ wrangler 4.162.0
────────────────────
Using redirected Wrangler configuration.
 - Configuration being used: "dist/client/wrangler.json"
 - Original user's configuration: "wrangler.jsonc"
 - Deploy configuration file: ".wrangler/deploy/config.json"
✨ Read 109 files from the assets directory .../site/phero-docs/dist/client
Total Upload: 0.36 KiB / gzip: 0.25 KiB
No bindings found.
--dry-run: exiting now.
```

**The headers, redirects and 404s behave as intended in a real asset server.**
Each app was served with `pnpm -C <app> exec wrangler dev --port <port>` (the
same assets engine as production, locally) and probed with `curl`:

| Probe | Observed |
|---|---|
| `curl -sSI http://127.0.0.1:8787/` | `200`, `content-security-policy`, `strict-transport-security`, `x-frame-options: DENY`, `x-content-type-options: nosniff`, `referrer-policy`, `permissions-policy`, `cache-control: public, max-age=0, must-revalidate` |
| `/…/_astro/index.<hash>.css` | `cache-control: public, max-age=31536000, immutable` (one value, not a comma-joined pair) |
| `curl -sSI …/docs/reference/ddd/` | `301`, `location: https://docs.pherolang.com/reference/ddd/` |
| `curl -sSI …/docs` | `301`, `location: https://docs.pherolang.com/` |
| docs `…/no-such-page` | `404`, `content-type: text/html`, all security headers present, body is Starlight's `404.html` (10,762 bytes) |
| marketing `…/nope` | `404`, **0-byte body** — see [§8](#8-the-marketing-app-has-no-404-page-open-item) |
| `…/robots.txt` | the file from `public/`, with the absolute sitemap URL |

**The CSP does not break the built pages.** Headless Chromium 140 loaded the
local origins with the exact headers above:

- The docs pages came back with `data-theme` and `data-has-sidebar` set on
  `<html>` — those attributes are written by Starlight's inline bootstrap, so
  inline scripts execute under `'unsafe-inline'`. A control page served with
  `script-src 'self'` kept its pre-script title, confirming the check can fail.
- Pagefind's search worker instantiates a WebAssembly module (the payload of
  `pagefind/wasm.en.pagefind` is a gzip-wrapped wasm binary behind a 12-byte
  `pagefind_dcd` header). Compiling it with `script-src 'self'` alone fails with
  *"Refused to compile or instantiate WebAssembly module because 'unsafe-eval' is
  not an allowed source of script"*; with `'wasm-unsafe-eval'` compilation is
  permitted. That is the evidence for the one non-obvious directive, and it is
  the reason `'unsafe-eval'` is nowhere in either policy.

### What is *not* verified

Nothing has been deployed to Cloudflare. No account, zone, DNS record, custom
domain, certificate or redirect rule has been created, so these remain
**unverified** until §5 is executed:

- that `wrangler deploy` succeeds with the token (only `--dry-run` ran);
- the shape of the `*.workers.dev` version-preview URL — the preview job greps
  the upload log for it, and `_headers` matches it with the placeholder pattern
  Cloudflare documents for this file;
- the zone-level `www` → apex redirect rule ([§4](#4-custom-domains-and-the-docs-redirect));
- certificate issuance for the apex and the docs subdomain;
- anything about response time, uptime, or the analytics beacon.

The deploy path itself is reachable today: the `v0.3.0` release carries
`pherolang-0.3.0-x86_64-unknown-linux-gnu.tar.gz`, which is the asset name
`site.yml` resolves, so the `freshness` job will actually run (not skip) and the
`deploy` job's `needs.freshness.result == 'success'` gate can be satisfied.

---

## 3. The pipeline that deploys

`.github/workflows/site.yml` is the contract. It is not edited by this change and
it is deliberately **not** a required status check: it triggers only on
`site/**` plus its own file, so a DSL-only PR would sit "expected" forever.

| Job | Runs when | What it does |
|---|---|---|
| `detect` | always | resolves three facts: apps scaffolded (`site/pnpm-lock.yaml`), a released binary asset matching `pherolang-*-x86_64-unknown-linux-gnu.tar.gz`, and whether the two Cloudflare secrets are set |
| `verify` | apps exist | builds both apps on a fork-safe runner (no secrets) |
| `freshness` | apps exist and a release asset exists | downloads the released binary, runs `pnpm -C site run sync-model`, fails on any `git diff` |
| `preview` | same-repo PR + apps + secrets | `wrangler versions upload` for both apps, then upserts one sticky PR comment headed `site-preview` with the two preview URLs |
| `deploy` | push to `main` + apps + `freshness` **succeeded** | `wrangler deploy` for both apps, into the GitHub `production` environment |
| `drift` | nightly 03:17 UTC or `workflow_dispatch` | regenerates the model data and opens/updates the `site/model-data-drift` PR |

### The two secrets

| Secret | Read by | Never read by |
|---|---|---|
| `CLOUDFLARE_API_TOKEN` | `preview`, `deploy` (as job env) | `verify`, `freshness`, `drift` |
| `CLOUDFLARE_ACCOUNT_ID` | `preview`, `deploy` (as job env) | `verify`, `freshness`, `drift` |

`detect` cannot read the `secrets` context in a job-level `if:`, so it evaluates
`secrets.CLOUDFLARE_API_TOKEN != '' && secrets.CLOUDFLARE_ACCOUNT_ID != ''` and
publishes the answer as the `cloudflareSecrets` output; `preview` and `deploy`
both gate on it. On a fork PR every secret is empty, which is exactly the
intended answer: no preview upload is attempted.

Consequences worth knowing before debugging a red run:

- Both jobs read the credentials as environment variables, so they are only
  present for the two jobs that need them. A missing secret shows up as
  `preview`/`deploy` skipped (with a notice), not as an auth failure.
- `deploy` needs `freshness` to *succeed* — a skipped `freshness` (no release
  asset) leaves `deploy` skipped too, by design. If `freshness` fails, the
  committed model data under `site/data/` is stale against `examples/`; the
  nightly `drift` job opens the fix as a PR.
- One pipeline runs per ref (`concurrency: site-${{ github.ref }}`,
  `cancel-in-progress: true`), so a second push supersedes the first instead of
  racing two version uploads.

---

## 4. Custom domains and the docs redirect

Three hostnames, two Workers, one zone (`pherolang.com`).

| Hostname | Serves | Mechanism |
|---|---|---|
| `pherolang.com` | `pherolang-web` | Worker **Custom Domain** |
| `docs.pherolang.com` | `pherolang-docs` | Worker **Custom Domain** |
| `www.pherolang.com` | nothing | zone **Redirect Rule** → apex, `301` |

Custom Domains are the right mechanism here because they point *all* paths of a
hostname at the Worker and Cloudflare creates the DNS record and certificate
itself. Two constraints from Cloudflare's documentation apply:

- Custom Domains need an active zone and exact hostname match, so `www` cannot
  ride on the apex Custom Domain — it needs its own redirect.
- A Custom Domain cannot be added on a hostname that already has a `CNAME`
  record. Check before adding: Workers & Pages → the Worker → Settings →
  Domains & Routes → Add → Custom Domain.

The `www` redirect is **zone-level** (Dashboard → the `pherolang.com` zone →
Rules → Redirect Rules → Single Redirect), because `_redirects` matches paths
only and Cloudflare's own docs list domain-level redirects as unsupported there:

- When: `http.host eq "www.pherolang.com"`
- Then: static redirect, status `301`, target
  `concat("https://pherolang.com", http.request.uri.path)`
- Preserve query string: on

For the rule to run, `www` needs a **proxied** DNS record. Cloudflare's docs
recommend an originless placeholder — a proxied `A` record for `www` to
`192.0.2.0`, or a proxied `AAAA` record to `100::` — with the caveat that the
record must be proxied, not DNS-only.

### The `/docs/*` contract

The documentation is not a path on the marketing site; it is a separate origin.
`site/phero-web/public/_redirects` therefore carries:

```
/docs  https://docs.pherolang.com/  301
/docs/*  https://docs.pherolang.com/:splat  301
```

Static rule first, dynamic second — Cloudflare follows the first match. Verified
locally against the built assets: `/docs/reference/ddd/` → `301` to
`https://docs.pherolang.com/reference/ddd/`, and bare `/docs` → `301` to the
docs home. Redirects run before `_headers`, so the redirect response also
carries the security headers.

Keep the marketing site's internal links pointing at
`https://docs.pherolang.com/…` directly. The redirect exists for old bookmarks and
hand-typed URLs, not as a routing layer; a link that relies on it costs every
visitor a round trip.

---

## 5. First-deploy checklist

In order. Each step ends with the command that proves it. Run the `pnpm` commands
from `site/` with the pinned Node (`fnm use` → `24.16.0`).

### 5.1 Verify the zone

Dashboard → add `pherolang.com` as a zone → set the registrar's nameservers to
the two Cloudflare issued. Zone status must read **Active**.

```sh
dig +short NS pherolang.com
curl -sSI https://pherolang.com/ | head -1   # whatever the zone serves today
```

### 5.2 Create the API token and the account ID, then store both as repository secrets

Dashboard → Account API tokens → Create Token → **Edit Cloudflare Workers**
template, scoped to this one account (and, if the template asks, the
`pherolang.com` zone). Minimum useful permissions: Workers Scripts edit. The
account ID is on the same page's right-hand panel ("Account ID").

```sh
gh secret set CLOUDFLARE_API_TOKEN --repo yestechgroup/pherolang
gh secret set CLOUDFLARE_ACCOUNT_ID --repo yestechgroup/pherolang
gh secret list --repo yestechgroup/pherolang   # both names present
```

### 5.3 Prove the token locally before trusting CI with it

```sh
export CLOUDFLARE_API_TOKEN='…' CLOUDFLARE_ACCOUNT_ID='…'
pnpm -C phero-web exec wrangler whoami        # prints the account and token permissions
pnpm -C phero-web exec wrangler deploy --dry-run   # "No bindings found."
pnpm -C phero-docs exec wrangler deploy --dry-run  # "No bindings found."
```

If `whoami` prints no account, the token is scoped to the wrong account or has no
Workers Scripts permission — fix it now, not in CI.

### 5.4 Create both Worker projects with a first deploy

Either merge a `site/**` change to `main` (the workflow deploys when `freshness`
passes) or bootstrap once from a workstation:

```sh
pnpm -C phero-web exec wrangler deploy
pnpm -C phero-docs exec wrangler deploy
```

Then confirm the upload-only shape took effect. Both workers now exist and are
reachable on their `workers.dev` hostnames (whose exact subdomain `wrangler`
prints at the end of the deploy):

```sh
curl -sSI https://pherolang-web.<account-subdomain>.workers.dev/ | head -1   # 200
curl -sSI https://pherolang-docs.<account-subdomain>.workers.dev/ | head -1  # 200
```

### 5.5 Attach the two Custom Domains

Workers & Pages → `pherolang-web` → Settings → Domains & Routes → Add → Custom
Domain → `pherolang.com`. Repeat on `pherolang-docs` for `docs.pherolang.com`.
Cloudflare creates the DNS records and the certificate; both hostnames must stay
on the same zone.

```sh
curl -sSI https://pherolang.com/ | head -1        # HTTP/2 200
curl -sSI https://docs.pherolang.com/ | head -1   # HTTP/2 200
```

If either returns `522`/`1016`, the Custom Domain is not attached (or the record
is a competing `CNAME`) — re-check the Domains & Routes list, not the DNS page.

### 5.6 Add the `www` redirect rule

Zone → Rules → Redirect Rules → Single Redirect, exactly as specified in §4,
after creating the proxied placeholder record for `www`.

```sh
curl -sSI https://www.pherolang.com/getting-started \
  | grep -iE '^(HTTP|location)'                   # 301 → https://pherolang.com/getting-started
```

### 5.7 Verify the deployed contract end to end

```sh
# security headers on both origins
curl -sSI https://pherolang.com/ | grep -iE 'content-security-policy|strict-transport-security|x-frame-options|x-content-type-options|referrer-policy|permissions-policy'
curl -sSI https://docs.pherolang.com/ | grep -i content-security-policy

# immutable hashed assets: take whichever asset the page actually references
asset=$(curl -sS https://pherolang.com/ | grep -oE '/_astro/[^"]+\.(css|js)' | head -1)
curl -sSI "https://pherolang.com${asset}" | grep -i cache-control

# the /docs/* 301 contract on BOTH sides (bare and deep path)
curl -sSI https://pherolang.com/docs | grep -iE '^(HTTP|location)'
curl -sSI https://pherolang.com/docs/reference/ddd/ | grep -iE '^(HTTP|location)'

# 404s
curl -sS -o /dev/null -w '%{http_code} %{size_download}\n' https://docs.pherolang.com/no-such-page   # 404, body > 0
curl -sS -o /dev/null -w '%{http_code} %{size_download}\n' https://pherolang.com/no-such-page        # 404, body 0 until §8 lands

# crawl surface
curl -sS https://pherolang.com/robots.txt
curl -sS https://pherolang.com/sitemap-index.xml | head -3
curl -sS https://docs.pherolang.com/robots.txt
```

### 5.8 Confirm the PR preview path

Open a same-repo PR that touches `site/**`. `preview` must upload both versions
and the sticky comment must show two `*.workers.dev` URLs. If it fails with "no
preview URL", version previews are disabled for the account — the job greps the
upload log for a `*.workers.dev` URL and fails when there is none.

### 5.9 Rehearse the rollback once, on purpose

```sh
pnpm -C phero-web exec wrangler versions list        # 10 most recent versions
pnpm -C phero-web exec wrangler rollback <version-id>
pnpm -C phero-web exec wrangler deployments status   # what production points at
```

Rollback is per Worker; rolling back the marketing site does not roll back the
docs site. `pnpm -C phero-web exec wrangler tail pherolang-web` streams live logs
if a deploy needs watching.

---

## 6. Response headers and the CSP

Both `_headers` files begin with the same block on `/*`:

```
X-Content-Type-Options: nosniff
Referrer-Policy: strict-origin-when-cross-origin
X-Frame-Options: DENY
Permissions-Policy: geolocation=(), microphone=(), camera=()
Strict-Transport-Security: max-age=31536000; includeSubDomains
Cache-Control: public, max-age=0, must-revalidate
```

`X-Frame-Options: DENY` is kept alongside `frame-ancestors 'none'` in the CSP:
the CSP is the modern control, the header is the one older browsers read. HSTS
does **not** carry `preload`, because preload is a one-way commitment submitted
to a browser vendor's list; add it deliberately, not as a default.

### The two policies

Marketing (`site/phero-web/public/_headers`):

```
Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; worker-src 'self'; manifest-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'; upgrade-insecure-requests
```

Documentation (`site/phero-docs/public/_headers`):

```
Content-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; worker-src 'self'; manifest-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'; upgrade-insecure-requests
```

`'unsafe-eval'` appears in neither. There is no CSP report endpoint: violations
are console-only until a collector exists ([§9](#9-not-covered-yet)).

### Which directive had to be relaxed, and why

Everything below is measured against the built HTML, not assumed. The marketing
app builds 3 pages; the docs app builds 24.

| Directive | Relaxation | Evidence |
|---|---|---|
| `script-src` (docs) | `'unsafe-inline'` | 10 inline scripts on every docs page (6 on `404.html`), 9 distinct script bodies across the 24 pages: Starlight's theme bootstrap, its theme-select sync, the search-shortcut hint, four module scripts that define custom elements (theme select, language select, sidebar pane, sidebar state), and two aria-hidden sidebar scroll-state scripts |
| `script-src` (docs) | `'wasm-unsafe-eval'` | Pagefind 1.6 ships a wasm-bindgen worker; `pagefind/wasm.en.pagefind` gunzips to a `pagefind_dcd` header followed by a real wasm module. Without the directive the browser refuses to compile it and site search is dead |
| `script-src` (marketing) | **none** — `'self'` only | 0 inline scripts across the 3 pages the app builds today. The graph island the plan adds is a same-origin module under `/_astro/`, which `'self'` covers |
| `style-src` (both) | `'unsafe-inline'` | Docs: 6,050 inline `style` attributes across 24 pages, up to 833 on one page — Starlight/Expressive Code writes per-token colour variables (`--0`, `--1`), tree depth (`--depth`) and icon sizes (`--sl-icon-size`) as attributes. Marketing: 3, on `install`/`getting-started` (callout accents). An inline `style` **attribute** can only be admitted wholesale or by `'unsafe-hashes'` plus a hash of each value, and 6,050 distinct values is not a hash list — so `'unsafe-inline'` is the only practical source |
| `img-src` (both) | `data:` | The docs stylesheets embed their icons as `data:image/svg+xml` backgrounds; Pagefind does the same. A `data:` image cannot execute script in an `<img>`/CSS context |

Why `'unsafe-inline'` for scripts on the docs app and not the 9 hashes: a
`public/_headers` file is static, so the hash list cannot be produced by the
build that produces the HTML. It would have to be regenerated by hand after every
Starlight patch and every theme change, and a stale hash breaks the theme
bootstrap silently — no CI gate reads a browser console. The honest trade is:
inline scripts stay confined to the docs origin, `script-src 'self'` still guards
the marketing origin, and everything else is locked to `'self'`.
If you later want the strict form, the hashes are computable from the build
output and would live in the docs `_headers` in place of `'unsafe-inline'`; that
work needs a build hook (in `astro.config.mjs` or `package.json`), which is why
it is not in this change.

**Before adding a script to a page, remember:** on the marketing origin it must
be an external same-origin module (Astro's default `<script>` bundling, or an
`/_astro/` file) — `<script is:inline>` will be blocked. JSON-LD
(`<script type="application/ld+json">`) is unaffected, because it is a data block
and never executed. On the docs origin a new inline script is already covered by
`'unsafe-inline'`.

### Cache rules, and the trap they avoid

```
/*
  Cache-Control: public, max-age=0, must-revalidate

/_astro/*
  ! Cache-Control
  Cache-Control: public, max-age=31536000, immutable
```

Cloudflare merges the headers of **every** matching rule, and a repeated header is
joined with a comma. A bare `/_astro/*` rule next to the `/*` rule would therefore
produce `Cache-Control: public, max-age=0, must-revalidate, public, max-age=31536000, immutable`,
and RFC 9111 makes the first `max-age` win — the immutable rule would silently do
nothing. The `! Cache-Control` detach (applied after the broader rule, as the
format requires) removes the inherited value first. Verified: the served
`/_astro/*` response carries exactly `public, max-age=31536000, immutable`.

The docs app adds the same treatment for `/pagefind/index/*` and
`/pagefind/fragment/*`, whose filenames carry a content hash. `pagefind.js`,
`pagefind-ui.js` and the Pagefind CSS keep their names between deploys and stay
on the revalidating rule.

The Astro Cloudflare adapter can inject its own `/_astro/*` immutable rule, but
it skips the injection when the existing `_headers` already declares
`Cache-Control` for that path. Here it skips: `dist/client/_headers` is
byte-identical to `public/_headers` and contains exactly one `/_astro/*` block.
That was checked after a build, not assumed.

### Preview and `workers.dev` hostnames

`https://:version.:subdomain.workers.dev/*` gets `X-Robots-Tag: noindex`. Those
hostnames serve the same bytes as the canonical origins, and nothing should index
them. A `_headers` rule allows a single splat, so a hostname wildcard plus a path
wildcard (`https://*.workers.dev/*`) is not expressible; the placeholder form is
the one Cloudflare documents. The plain
`pherolang-web.<account-subdomain>.workers.dev` hostname matches it too, which is
the intended outcome — the canonical URL is the custom domain.

---

## 7. `robots.txt` and sitemaps

Both apps allow everything and point at their own sitemap:

```
# pherolang.com
User-agent: *
Allow: /
Sitemap: https://pherolang.com/sitemap-index.xml
```

```
# docs.pherolang.com
User-agent: *
Allow: /
Sitemap: https://docs.pherolang.com/sitemap-index.xml
```

`@astrojs/sitemap` writes `sitemap-index.xml` plus a numbered
`sitemap-0.xml` into `dist/client` for both apps, so the pointer is never a
guess. The index is what `robots.txt` must name (a sitemap index is valid in a
`Sitemap:` line).

There is **no `/design` route today** — `site/phero-web/src/pages/` holds the
public pages only, and the layout's `noindex` prop exists for the internal design
gallery that has not been built. When it is:

1. give the page `noindex` (the prop already emits
   `<meta name="robots" content="noindex, nofollow">`), and
2. add `Disallow: /design` to the marketing `robots.txt`.

Both, not either: a `Disallow` alone stops crawlers from ever reading the
`noindex` tag, and a `noindex` alone leaves the URL crawlable. The commit that
adds the page should add the robots line and this note in its PR description.

---

## 8. The marketing app has no 404 page (open item)

Starlight generates `dist/client/404.html` for the docs app automatically. The
marketing app does not: Astro only emits `404.html` when an
`src/pages/404.astro` exists, and there is none.

That matters because `wrangler.jsonc` sets
`"not_found_handling": "404-page"`. Cloudflare looks for the nearest `404.html`
in the assets directory; with none present, the observed behaviour on the built
output is a `404` with a **0-byte body**:

```console
$ curl -sS -o /dev/null -w '%{http_code} %{size_download}\n' http://127.0.0.1:8787/nope
404 0
```

The status code is right and the security headers are still applied, so this is a
usability gap, not a safety one — but a visitor who mistypes a URL gets a blank
page.

**Fix (integrator-owned, not in this change):** add
`site/phero-web/src/pages/404.astro`, which uses `Base` so it inherits the nav,
footer and `<head>`:

```astro
---
import Base from '../layouts/Base.astro';
import Prose from '@ui/Prose.astro';
import Button from '@ui/Button.astro';
---

<Base
  title="Page not found — Phero Lang"
  description="That page does not exist. Start from the homepage, the platform overview, or the documentation."
>
  <div class="py-16 sm:py-24">
    <p class="font-mono text-xs uppercase tracking-[0.2em] text-[var(--text-muted)]">Error 404</p>
    <h1 class="mt-6 max-w-[26ch] font-display text-display-2 font-semibold text-balance">
      That page does not exist.
    </h1>
    <Prose class="mt-6">
      <p>
        The URL was mistyped, or the page moved. The homepage, the
        <a href="/platform">platform overview</a> and the
        <a href="https://docs.pherolang.com/">documentation</a> all still work.
      </p>
    </Prose>
    <div class="mt-8 flex flex-wrap gap-3">
      <Button href="/">Go to the homepage</Button>
      <Button href="https://docs.pherolang.com/" variant="secondary">Read the docs</Button>
    </div>
  </div>
</Base>
```

The classes and components are the ones the other pages already use
(`SectionHeader`, `Prose`, `Button` from `@ui`, colours via the
`packages/tokens` custom properties), so the 404 does not invent a second visual
language.

Astro writes it to `dist/client/404.html` and `not_found_handling` starts serving
it with no configuration change. This is deliberately **not** a
`public/404.html`: that file would be hand-maintained chrome that drifts from the
layout the moment the nav changes.

The docs app needs nothing: its 404 already returns a 10.7 KB Starlight page with
the full header stack.

---

## 9. Not covered yet

| Gap | Where it lands | Why it is not here |
|---|---|---|
| **Analytics** | `site/phero-web/src/layouts/Base.astro` (+ Starlight's `head` option for the docs app), both integrator-owned; plus a `site.yml` env var | No runtime JS ships from `public/`. See the snippet below — it is inert until a token exists |
| **Uptime / canary** | a scheduled workflow, or Cloudflare Health Checks | `site.yml` has a `drift` timer but nothing that fetches the live origins. The one external check today is a human running §5.7 |
| **Claim-drift gate** | `site/scripts/gate/**` | The gate (lane 7) checks links, markup, axe, tokens, bundle size, surface coverage and freshness. Nothing cross-checks a *prose claim* against `site/data/evidence.json`, so a sentence can outlive the artifact that supported it. The honesty mechanisms that do exist are `freshness` (data vs `examples/`) and the byte-compared goldens |
| **Forms / email** | EP6 (design-partner form + Turnstile Worker) | Out of scope for this epic. The CTA is a `mailto:` link. `form-action 'self'` in the CSP is ready for a same-origin endpoint |
| **CSP reporting** | a collector + `report-to`/`report-uri` in both `_headers` | No endpoint exists; adding a broken one is worse than none |
| **`www`/apex HSTS preload** | zone settings + the `preload` token | A one-way commitment; see §6 |

### The analytics snippet (inert until a token exists)

The recommended shape is the manual Cloudflare Web Analytics beacon. There is no
`analytics.js` in `public/`, on purpose: injecting a `<script>` from JavaScript
needs `'unsafe-inline'` on the marketing origin, which currently runs
`script-src 'self'`, and the beacon would still be subject to `script-src` once
injected. A static tag costs one line and no relaxation of anything else.

To enable it, **both** of these changes are required:

1. Add the tag to the `<head>` of `site/phero-web/src/layouts/Base.astro` (and to
   the docs app's Starlight `head` option in `site/phero-docs/astro.config.mjs`
   if the docs should be measured):

   ```html
   <script defer src="https://static.cloudflareinsights.com/beacon.min.js"
           data-cf-beacon='{"token": "<site-token>"}'></script>
   ```

   The token is a build-time constant; there is no env plumbing for it in either
   `astro.config.mjs`, so wiring one in means a `site.yml` edit too.

2. **Widen the CSP in the same commit**, or the beacon is silently blocked:

   - `script-src`: add `https://static.cloudflareinsights.com`
   - `connect-src`: add `https://cloudflareinsights.com` (the beacon posts there)

Dashboard-injected analytics (the "automatic" setup) is injected at the edge and
lands *inside* the document, so these two CSP entries are needed for that path as
well. Enabling analytics without the CSP edit produces a page that reports
nothing and logs a violation only in the visitor's console.

---

## 10. Quick reference

```sh
# from site/, with `fnm use`
pnpm -r build                                     # both apps → dist/client
pnpm -C phero-web exec wrangler deploy --dry-run
pnpm -C phero-docs exec wrangler deploy --dry-run
pnpm -C phero-web exec wrangler dev --port 8787   # serve the built assets locally
pnpm -C phero-web exec wrangler whoami
pnpm -C phero-web exec wrangler versions list
pnpm -C phero-web exec wrangler rollback <version-id>
pnpm -C phero-web exec wrangler tail pherolang-web
```

| Symptom | Likely cause |
|---|---|
| `preview`/`deploy` skipped, notice about credentials | one of the two repository secrets is unset (fork PRs are meant to skip) |
| `deploy` skipped, `freshness` skipped | no release asset matching `pherolang-*-x86_64-unknown-linux-gnu.tar.gz` |
| `freshness` red, `git diff` non-empty | `site/data/` is stale against `examples/`; merge the `site/model-data-drift` PR |
| `preview` red: "printed no preview URL" | version preview URLs are disabled for the account, or the upload itself failed — read the upload log |
| assets uncached, or a second `max-age` in the response | a new `_headers` rule sets `Cache-Control` without a `! Cache-Control` detach |
| blank body on a marketing 404 | the missing `src/pages/404.astro` ([§8](#8-the-marketing-app-has-no-404-page-open-item)) |
| search returns nothing on the docs site | `'wasm-unsafe-eval'` was dropped from the docs CSP, or `worker-src` no longer allows `'self'` |
