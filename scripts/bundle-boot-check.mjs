// Browser verification for the `?bundle=` boot channel: serves an exported
// site and asserts the whole boot chain ran, rather than merely that nothing
// panicked.
//
//   waml-boot.txt            -> select_site_boot   (the site-config channel)
//   bundle.waml              -> claim_boot_bundle  (the boot the config named)
//   bundle.waml.search-index -> arm_index/claim_index
//
// The index-asset request is the load-bearing assertion. `start_boot_index_fetch`
// is reached only once `claim_boot_bundle()` returned the armed URL AND the
// bundle decoded, so observing that third request proves the claim path worked.
// Verified against a negative control: stubbing `claim_boot_bundle` to return
// `None` stops the chain at bundle.waml and fails this check.
//
// A failed `open_bundle` logs "failed to analyze replacement bundle" through
// tracing, which lands in the console, so its absence is the success signal for
// the open itself. Nothing here reads pixels: a Windows headless run renders
// nothing at all (the fork's shader loader is `#[cfg(unix)]`), so a screenshot
// would be blank whether the boot worked or not.
//
// Not a gate step -- the companion to scripts/serve-browser-check.mjs, which
// covers the `?api=` channel. Both are standalone node scripts with their own
// verdict and a non-zero exit.
//
// playwright-core drives the ms-playwright chromium-1228 build directly. ESM
// resolution ignores NODE_PATH and walks up from this file, so install it
// somewhere scratch and link it in:
//
//   npm install --no-save playwright-core   # in a scratch dir
//   mklink /J <repo>\node_modules <scratch>\node_modules
//
// Set PLAYWRIGHT_CHROMIUM_PATH if the browser is not at the default
// %LOCALAPPDATA%\ms-playwright location.
//
// Usage: node scripts/bundle-boot-check.mjs <site-dir>
//   where <site-dir> is the output of `waml export site <bundle> --out <dir>`.
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {extname, join, normalize, resolve} from 'node:path';
import {chromium} from 'playwright-core';
import {existsSync} from 'node:fs';

const siteDir = resolve(process.argv[2]);
const MIME = {
  '.html': 'text/html',
  '.wasm': 'application/wasm',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.ttf': 'application/ttf',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
  '.txt': 'text/plain',
  '.md': 'text/markdown',
  '.waml': 'application/octet-stream',
};

const server = createServer(async (req, res) => {
  const path = decodeURIComponent(req.url.split('?')[0]);
  const rel = normalize(path === '/' ? 'index.html' : path.replace(/^\/+/, ''));
  const file = join(siteDir, rel);
  if (!file.startsWith(siteDir) || !existsSync(file)) {
    res.writeHead(404).end('not found');
    return;
  }
  try {
    const body = await readFile(file);
    res.writeHead(200, {'Content-Type': MIME[extname(file)] ?? 'application/octet-stream'});
    res.end(body);
  } catch (err) {
    res.writeHead(500).end(String(err));
  }
});

function findChromium() {
  if (process.env.PLAYWRIGHT_CHROMIUM_PATH) return process.env.PLAYWRIGHT_CHROMIUM_PATH;
  return join(
    process.env.LOCALAPPDATA,
    'ms-playwright',
    'chromium-1228',
    'chrome-win64',
    'chrome.exe',
  );
}

let exitCode = 0;
let browser = null;
try {
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const url = `http://127.0.0.1:${server.address().port}/`;
  console.log(`bundle-boot-check: serving ${siteDir} at ${url}`);

  browser = await chromium.launch({headless: true, executablePath: findChromium()});
  const page = await browser.newPage();

  const requested = [];
  const consoleErrors = [];
  page.on('request', (r) => requested.push(new URL(r.url()).pathname));
  page.on('console', (m) => {
    if (m.type() === 'error') consoleErrors.push(m.text());
  });
  page.on('pageerror', (e) => consoleErrors.push(String(e)));

  await page.goto(url, {waitUntil: 'load'});
  await page.waitForSelector('canvas', {timeout: 30_000});
  // The boot chain is three sequential fetches after first paint.
  await page.waitForTimeout(8_000);

  const want = ['/waml-boot.txt', '/bundle.waml', '/bundle.waml.search-index'];
  const missing = want.filter((p) => !requested.includes(p));
  console.log(`bundle-boot-check: requested ${JSON.stringify(requested.filter((p) => want.includes(p)))}`);
  if (missing.length) {
    throw new Error(
      `the boot chain stopped early; never requested ${missing.join(', ')}.\nall paths: ${JSON.stringify(requested)}`,
    );
  }

  const bad = consoleErrors.filter((l) => /panic|failed to analyze|could not open/i.test(l));
  if (bad.length) throw new Error(`console reported a boot failure:\n${bad.join('\n')}`);

  console.log('bundle-boot-check: boot chain complete, bundle opened, no console failure');
  console.log('bundle-boot-check: PASS');
} catch (err) {
  console.error(`bundle-boot-check: FAIL -- ${err.message ?? err}`);
  exitCode = 1;
} finally {
  if (browser) await browser.close();
  server.close();
}
process.exit(exitCode);
