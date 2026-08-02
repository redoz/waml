// Cold-boot probe for the web build. Measures where first-frame time actually
// goes, which is neither download nor wasm compile but shader program linking.
//
// Must run headed on a real GPU with a cold profile. Headless swiftshader
// reports ~3s total and hides the defect completely; a warm profile reports
// ~1.9s because Chrome caches linked programs on disk. Both read green while
// real first-time visitors freeze for 35 seconds.
//
// Playwright is deliberately not a repo dependency. Install it somewhere
// scratch and link it in; NODE_PATH does NOT work here, because ESM resolution
// ignores it and walks up from the script's own directory instead:
//
//   npm install --no-save playwright        # in a scratch dir
//   mklink /J <repo>\node_modules <scratch>\node_modules
//
// Usage: node scripts/measure-web-boot.mjs <dist-dir>
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {extname, join, resolve} from 'node:path';
import {chromium} from 'playwright';

const TYPES = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.mjs': 'text/javascript',
  '.wasm': 'application/wasm',
  '.css': 'text/css',
  '.ttf': 'font/ttf',
  '.json': 'application/json',
};

const root = resolve(process.argv[2] ?? '.');

const server = createServer(async (req, res) => {
  const rel = decodeURIComponent(req.url.split('?')[0]);
  const path = join(root, rel === '/' ? '/index.html' : rel);
  try {
    const body = await readFile(path);
    res.writeHead(200, {
      'content-type': TYPES[extname(path)] ?? 'application/octet-stream',
      // Defeat any HTTP caching so every run is genuinely cold.
      'cache-control': 'no-store',
    });
    res.end(body);
  } catch {
    res.writeHead(404).end('not found');
  }
});

await new Promise((r) => server.listen(0, r));
const port = server.address().port;

// launch() (not launchPersistentContext) gets a fresh throwaway profile every
// run, which is what makes the measurement cold.
const browser = await chromium.launch({headless: false});
const page = await browser.newPage();

await page.addInitScript(() => {
  const state = {
    firstDrawMs: null,
    linkStatusTotalMs: 0,
    programs: [],
    sources: new Map(),
  };
  globalThis.__probe = state;

  for (const Ctx of [WebGLRenderingContext, WebGL2RenderingContext]) {
    const proto = Ctx.prototype;

    // Remember each shader's source so slug programs can be identified.
    const shaderSource = proto.shaderSource;
    proto.shaderSource = function (shader, src) {
      state.sources.set(shader, src);
      return shaderSource.call(this, shader, src);
    };

    const attachShader = proto.attachShader;
    proto.attachShader = function (program, shader) {
      const list = state.sources.get(program) ?? [];
      list.push(state.sources.get(shader) ?? '');
      state.sources.set(program, list);
      return attachShader.call(this, program, shader);
    };

    // ANGLE defers the real D3D compile to the LINK_STATUS query, so this is
    // where the cost surfaces even though linkProgram itself reports 0ms.
    const getProgramParameter = proto.getProgramParameter;
    proto.getProgramParameter = function (program, pname) {
      if (pname !== this.LINK_STATUS) {
        return getProgramParameter.call(this, program, pname);
      }
      const t0 = performance.now();
      const result = getProgramParameter.call(this, program, pname);
      const ms = performance.now() - t0;
      state.linkStatusTotalMs += ms;
      const src = (state.sources.get(program) ?? []).join('\n');
      state.programs.push({
        index: state.programs.length,
        ms,
        hasSlug: /io_slug_|io_scan_|slug_curve_count/.test(src),
      });
      return result;
    };

    for (const name of ['drawElements', 'drawArrays', 'drawElementsInstanced']) {
      const fn = proto[name];
      if (!fn) continue;
      proto[name] = function (...args) {
        if (state.firstDrawMs === null) state.firstDrawMs = performance.now();
        return fn.apply(this, args);
      };
    }
  }
});

await page.goto(`http://127.0.0.1:${port}/`, {waitUntil: 'commit'});
// The third argument is the options bag; the second is the (unused) arg passed
// into the page function. Collapsing them silently leaves the default 30s
// timeout in place, which is shorter than the freeze being measured.
await page.waitForFunction(() => globalThis.__probe?.firstDrawMs !== null, null, {
  timeout: 180_000,
});

const report = await page.evaluate(() => {
  const s = globalThis.__probe;
  const programs = s.programs.map(({index, ms, hasSlug}) => ({index, ms, hasSlug}));
  return {
    firstDrawMs: s.firstDrawMs,
    linkStatusTotalMs: s.linkStatusTotalMs,
    programCount: programs.length,
    slugPrograms: programs.filter((p) => p.hasSlug),
    top5: [...programs].sort((a, b) => b.ms - a.ms).slice(0, 5),
  };
});

console.log(JSON.stringify(report, null, 2));

await browser.close();
server.close();
