// End-to-end acceptance for `waml export site`, in a real browser over real
// HTTP.
//
// What this proves that no unit test can: an exported site BOOTS from its own
// `waml-boot.txt`, with the visitor's address bar left clean. The assembler's
// own tests see three synthetic files; here the real wasm fetches
// `waml-boot.txt`, reads the bundle path it names, fetches the real bundle,
// opens the model, and hands a `.waml` file back through the download bridge.
//
// `file://` is not a substitute and is not accepted: the editor FETCHES its
// bundle, and a file:// page has a null origin that fails that fetch. The
// server below is an ephemeral-port static host -- the same shape a user gets
// from `python -m http.server --directory site`.
//
// Headed, not headless: this build's shaders do not compile under Chromium's
// swiftshader (`webgl.compile_fail.vertex`, then a blank canvas), so a
// headless run would assert against a page that never rendered. It therefore
// needs a real GPU and is NOT wired into CI.
//
// Prerequisites, both deliberately outside the repo's dependency set:
//
//   1. A `waml` binary built with the editor embedded:
//        cargo makepad wasm build -p waml-editor --release --no-threads --strip
//        node scripts/prune-web-fonts.mjs \
//          target/makepad-wasm-app/release/waml-editor <makepad>/widgets/src
//        node scripts/brand-web-artifact.mjs target/makepad-wasm-app/release/waml-editor
//        node scripts/inject-runtime-shell.mjs \
//          target/makepad-wasm-app/release/waml-editor local
//        node scripts/package-web-artifact.mjs \
//          target/makepad-wasm-app/release/waml-editor target/web-embed
//        WAML_WEB_EMBED_DIR=target/web-embed \
//          cargo build -p waml-cli --release --features embed-web
//      Override the path with WAML_SITE_BIN.
//
//   2. Playwright, installed somewhere scratch and linked in (ESM resolution
//      ignores NODE_PATH, so a junction is the way -- see
//      scripts/measure-web-boot.mjs):
//        npm install --no-save playwright && npx playwright install chromium
//
// Both missing is the ordinary state of a checkout, so the test SKIPS there.
//
// NOT covered: driving an in-app edit. Every mutation in this editor is a
// canvas gesture with no DOM handle, and none of select/drag/double-click/the
// Add tool could be driven blind from Playwright. The share-URL half of the
// story is instead proven the way Task 4 specifies it -- a `#w1.` fragment
// outranks the site's own `waml-boot.txt`, and the export hands back what the
// session currently holds -- which is exactly what an edit produces once
// made.
//
// Run: node --test scripts/export-site-browser.test.mjs

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createServer } from "node:http";
import { existsSync } from "node:fs";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { extname, join, resolve } from "node:path";
import test from "node:test";

const BIN =
  process.env.WAML_SITE_BIN ??
  resolve("target/release", process.platform === "win32" ? "waml.exe" : "waml");

// Caption geometry in CSS pixels at devicePixelRatio 1 (Playwright's default),
// measured against a booted site at the fixed viewport below: the burger sits
// after the 280px navigation dock, and its menu's third row is
// **Export WAML bundle...**.
const BURGER = { x: 295, y: 17 };
const EXPORT_ROW = { x: 380, y: 120 };

const TYPES = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".mjs": "text/javascript",
  ".wasm": "application/wasm",
  ".css": "text/css",
  ".ttf": "font/ttf",
  ".otf": "font/otf",
  ".json": "application/json",
  ".ico": "image/x-icon",
  ".svg": "image/svg+xml",
  // The bundle is bytes; the editor decodes it itself.
  ".waml": "application/octet-stream",
};

const serve = async (root) => {
  const requested = [];
  const server = createServer(async (request, response) => {
    const rel = decodeURIComponent(request.url.split("?")[0]);
    requested.push(rel);
    const path = join(root, rel === "/" ? "/index.html" : rel);
    try {
      const body = await readFile(path);
      response.writeHead(200, {
        "content-type": TYPES[extname(path)] ?? "application/octet-stream",
      });
      response.end(body);
    } catch {
      response.writeHead(404);
      response.end("not found");
    }
  });
  await new Promise((done) => server.listen(0, "127.0.0.1", done));
  return {
    origin: `http://127.0.0.1:${server.address().port}`,
    requested,
    close: () => new Promise((done) => server.close(done)),
  };
};

const chromium = await import("playwright")
  .then((module) => module.chromium)
  .catch(() => null);

const skipReason = !existsSync(BIN)
  ? `no embed-web waml binary at ${BIN} (see the header of this file)`
  : !chromium
    ? "playwright is not installed (see the header of this file)"
    : null;

// The app is up once makepad's web.js removes cargo-makepad's loader element,
// which it does on the first presented frame.
const booted = (page) =>
  page.waitForFunction(() => !document.querySelector(".canvas_loader"), null, {
    timeout: 180_000,
  });

const exportBundle = async (page, into) => {
  const download = page.waitForEvent("download", { timeout: 60_000 });
  await page.mouse.click(BURGER.x, BURGER.y);
  await page.waitForTimeout(500);
  await page.mouse.click(EXPORT_ROW.x, EXPORT_ROW.y);
  const file = await download;
  assert.match(file.suggestedFilename(), /\.waml$/);
  const path = join(into, file.suggestedFilename());
  await file.saveAs(path);
  return readFile(path, "utf8");
};

// `skip` must be ABSENT, not null, when the prerequisites are there: node
// treats the property's presence as the skip signal and would report a passing
// run as skipped.
const options = skipReason ? { skip: skipReason } : {};

// Scenario: BROWSER-004
// Scenario: BROWSER-006
// Scenario: BROWSER-011
test("an exported site boots and exports its model back", options, async (t) => {
  const work = await mkdtemp(join(tmpdir(), "waml-site-e2e-"));
  t.after(() => rm(work, { recursive: true, force: true }));
  const site = join(work, "site");
  await writeFile(
    join(work, "order.md"),
    "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n- total: Money\n",
  );
  await writeFile(
    join(work, "customer.md"),
    "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n",
  );

  execFileSync(BIN, ["export", "site", work, "--out", site], { stdio: "pipe" });
  assert.ok(existsSync(join(site, "bundle.waml")), "the site must carry its bundle");

  const host = await serve(site);
  t.after(() => host.close());
  const browser = await chromium.launch({ headless: false });
  t.after(() => browser.close());
  const context = await browser.newContext({
    acceptDownloads: true,
    viewport: { width: 1280, height: 840 },
  });
  const page = await context.newPage();

  const failures = [];
  page.on("console", (message) => {
    const text = message.text();
    if (/panic|RuntimeError|Failed to fetch|could not load/i.test(text)) failures.push(text);
  });
  page.on("pageerror", (error) => failures.push(String(error)));

  await page.goto(`${host.origin}/`, { waitUntil: "load" });
  await booted(page);
  assert.deepEqual(failures, [], "the site must boot without panics or failed fetches");
  assert.ok(
    host.requested.includes("/waml-boot.txt"),
    "the editor must read the site's boot config",
  );
  assert.ok(host.requested.includes("/bundle.waml"), "the editor must fetch the site's bundle");
  // The whole point: the visitor's URL is untouched by the boot.
  assert.equal(
    await page.evaluate(() => location.search),
    "",
    "booting a site must not put a query in the address bar",
  );

  const exported = await exportBundle(page, work);
  assert.match(exported, /<!-- waml\/1 part /, "the download must be a Bundle Envelope v1");
  assert.match(exported, /# Order/);
  assert.match(exported, /# Customer/);
  assert.match(exported, /- total: Money/);

  // A share fragment is the more specific request, and it must win without the
  // site's config ever being consulted -- otherwise a shared edit would race
  // the site's own bundle.
  host.requested.length = 0;
  await writeFile(
    join(work, "customer.md"),
    "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n\n- loyalty: Tier\n",
  );
  const fragment = execFileSync(BIN, ["share", work, "--fragment-only"], { encoding: "utf8" }).trim();
  // goto + reload, not goto alone: the page is already at `${host.origin}/`,
  // and a URL differing only in its fragment is an in-page navigation -- the
  // wasm would never restart and would still hold the model it booted with.
  await page.goto(`${host.origin}/#${fragment}`, { waitUntil: "load" });
  await page.reload({ waitUntil: "load" });
  await booted(page);
  assert.deepEqual(failures, [], "a shared model must open without panics");
  assert.equal(
    host.requested.includes("/waml-boot.txt"),
    false,
    "a share link must not consult the site's boot config",
  );
  assert.equal(
    await page.evaluate(() => location.search),
    "",
    "a share link must not gain a query",
  );

  const shared = await exportBundle(page, work);
  assert.match(shared, /- loyalty: Tier/, "the export must follow the session, not the site file");
});
