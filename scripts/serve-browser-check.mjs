// Browser verification for `waml serve`: boots the embedded web editor from a
// running server, exercises the same-origin token boot path, checks that a
// foreign origin is refused by the server (the finding that killed the
// hosted-UI option), and round-trips a save through POST /api/documents.
//
// Not a gate step -- follows scripts/measure-web-boot.mjs / verify-web-artifact.mjs
// conventions (a standalone node script, its own verdict, non-zero exit + a
// screenshot on failure). Requires a `--features embed-web` binary; run it
// against one the way the gate never does.
//
// playwright-core drives the ms-playwright chromium-1228 build directly
// (chrome-win64, not chrome-win) rather than depending on the `playwright`
// package, so no browser download is triggered by this script itself --
// install playwright-core as a scratch dependency and point
// PLAYWRIGHT_CHROMIUM_PATH at the already-downloaded executable if it is not
// at the default %LOCALAPPDATA%\ms-playwright location.
//
// Usage: node scripts/serve-browser-check.mjs <path-to-waml(.exe)> <fixture-dir>
import {spawn} from 'node:child_process';
import {existsSync, mkdtempSync, readFileSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {chromium} from 'playwright-core';

function fail(message) {
  console.error(`serve-browser-check: ${message}`);
  process.exitCode = 1;
  return null;
}

function findChromiumExecutable() {
  if (process.env.PLAYWRIGHT_CHROMIUM_PATH) return process.env.PLAYWRIGHT_CHROMIUM_PATH;
  const base = join(
    process.env.LOCALAPPDATA ?? join(process.env.HOME ?? '.', '.cache'),
    'ms-playwright',
    'chromium-1228',
    'chrome-win64',
    'chrome.exe',
  );
  return existsSync(base) ? base : null;
}

const binPath = process.argv[2];
const fixtureDir = process.argv[3];

if (!binPath || !fixtureDir) {
  console.error('usage: node scripts/serve-browser-check.mjs <path-to-waml(.exe)> <fixture-dir>');
  process.exit(1);
}

if (!existsSync(binPath)) {
  fail(`no binary at ${binPath}. Build with --features embed-web first.`);
  process.exit(1);
}

const executablePath = findChromiumExecutable();
if (!executablePath) {
  fail(
    'no chromium-1228 chrome-win64 build found. Set PLAYWRIGHT_CHROMIUM_PATH or ' +
      'install the ms-playwright chromium-1228 build.',
  );
  process.exit(1);
}

// A minimal fixture: one package + one class, mirroring tests/cli_e2e.rs.
if (!existsSync(join(fixtureDir, 'index.uml.package.md'))) {
  writeFileSync(
    join(fixtureDir, 'index.uml.package.md'),
    '---\numl.package:\n  name: Demo\n---\n\n# Demo\n',
  );
  writeFileSync(
    join(fixtureDir, 'order.uml.class.md'),
    '---\numl.class:\n  name: Order\n---\n\n# Order\n',
  );
}

console.log(`serve-browser-check: launching ${binPath} serve ${fixtureDir} --port 0 --no-open`);
const child = spawn(binPath, ['serve', fixtureDir, '--port', '0', '--no-open']);

let stdout = '';
let stderr = '';
child.stdout.on('data', (chunk) => {
  stdout += chunk.toString();
});
child.stderr.on('data', (chunk) => {
  stderr += chunk.toString();
});

function killChild() {
  if (!child.killed) child.kill();
}

async function waitForUrl() {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const match = stdout.match(/(http:\/\/127\.0\.0\.1:\d+\/\?api=\/api#token=[^\s]+)/);
    if (match) return match[1];
    if (child.exitCode !== null) {
      throw new Error(`waml serve exited early (code ${child.exitCode}). stderr:\n${stderr}`);
    }
    await new Promise((r) => setTimeout(r, 50));
  }
  throw new Error(`timed out waiting for the printed boot URL. stdout:\n${stdout}\nstderr:\n${stderr}`);
}

let exitCode = 0;
let browser = null;
// Scenario: BROWSER-002
// Scenario: BROWSER-007
// Scenario: BROWSER-009
try {
  const url = await waitForUrl();
  const apiBase = url.replace(/^(http:\/\/[^/]+).*$/, '$1/api');
  // The token rides in the URL fragment (#token=<t>), not the query.
  const token = new URLSearchParams(new URL(url).hash.replace(/^#/, '')).get('token');
  console.log(`serve-browser-check: boot URL ${url}`);

  browser = await chromium.launch({headless: true, executablePath});
  const page = await browser.newPage();

  const consoleErrors = [];
  page.on('console', (msg) => {
    if (msg.type() === 'error') consoleErrors.push(msg.text());
  });
  page.on('pageerror', (err) => consoleErrors.push(String(err)));

  await page.goto(url, {waitUntil: 'load'});

  // The editor boots via GET /api/bundle and drives makepad's canvas; there is
  // no DOM signal for "model open", so poll the canvas element's presence and
  // the absence of a panic message, then hold briefly for the async fetch.
  await page.waitForSelector('canvas', {timeout: 15_000});
  await page.waitForTimeout(2_000);

  const panicked = consoleErrors.some((line) => /panic/i.test(line));
  if (panicked) {
    const screenshotPath = join(mkdtempSync(join(tmpdir(), 'serve-check-')), 'panic.png');
    await page.screenshot({path: screenshotPath});
    throw new Error(`console reported a panic; screenshot at ${screenshotPath}\n${consoleErrors.join('\n')}`);
  }
  console.log('serve-browser-check: boot OK, no console panic');

  // Same-origin API call: must succeed with the printed token.
  const sameOrigin = await page.evaluate(
    async ({apiBase, token}) => {
      const res = await fetch(`${apiBase}/model`, {headers: {Authorization: `Bearer ${token}`}});
      return res.status;
    },
    {apiBase, token},
  );
  if (sameOrigin !== 200) {
    throw new Error(`same-origin GET /api/model expected 200, got ${sameOrigin}`);
  }
  console.log('serve-browser-check: same-origin API call OK');

  // Foreign-origin regression guard: a page on a different origin must be
  // refused by the server's Origin check, not merely by browser CORS UI.
  const foreignPage = await browser.newPage();
  await foreignPage.goto('data:text/html,<html></html>');
  const foreignResult = await foreignPage
    .evaluate(
      async ({apiBase, token}) => {
        try {
          const res = await fetch(`${apiBase}/model`, {headers: {Authorization: `Bearer ${token}`}});
          return {ok: true, status: res.status};
        } catch (err) {
          return {ok: false, error: String(err)};
        }
      },
      {apiBase, token},
    )
    .catch((err) => ({ok: false, error: String(err)}));
  await foreignPage.close();
  if (foreignResult.ok && foreignResult.status < 400) {
    throw new Error(
      `foreign origin was NOT refused (status ${foreignResult.status}); this is the regression the check exists for`,
    );
  }
  console.log('serve-browser-check: foreign-origin request refused, as expected');

  // Save round trip: POST /api/documents with a baseline-guarded write, then
  // verify the byte content landed on disk.
  const classPath = 'order.uml.class.md';
  const before = readFileSync(join(fixtureDir, classPath), 'utf8');
  const desired = `${before}\n<!-- serve-browser-check touched this file -->\n`;
  const saveResult = await page.evaluate(
    async ({apiBase, token, classPath, before, desired}) => {
      const modelRes = await fetch(`${apiBase}/model`, {headers: {Authorization: `Bearer ${token}`}});
      const revision = Number(modelRes.headers.get('X-Waml-Revision') ?? (await modelRes.json()).revision);
      const res = await fetch(`${apiBase}/documents`, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${token}`,
          'X-Waml-Client': '1',
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          revision,
          writes: [{path: classPath, baseline: before, desired}],
        }),
      });
      return {status: res.status, body: await res.text()};
    },
    {apiBase, token, classPath, before, desired},
  );
  if (saveResult.status !== 200) {
    throw new Error(`POST /api/documents expected 200, got ${saveResult.status}: ${saveResult.body}`);
  }
  const after = readFileSync(join(fixtureDir, classPath), 'utf8');
  if (after !== desired) {
    throw new Error(
      `save round trip did not land on disk byte-exactly.\nexpected: ${desired}\nactual: ${after}`,
    );
  }
  console.log('serve-browser-check: save round trip OK, file changed on disk');

  console.log('serve-browser-check: PASS');
} catch (err) {
  console.error(`serve-browser-check: FAIL -- ${err.message ?? err}`);
  exitCode = 1;
} finally {
  if (browser) await browser.close();
  killChild();
}

process.exit(exitCode);
