// Attribution probe: where does the residual ~1.8s cold boot go, now that
// shader linking is down to ~110ms?
//
// Same rules as scripts/measure-web-boot.mjs: headed, real GPU, cold profile.
// Headless/swiftshader/warm-profile numbers are void.
//
// Usage: node scripts/attribute-web-boot.mjs <dist-dir> [glsl-dump-dir]
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
      'cache-control': 'no-store',
    });
    res.end(body);
  } catch {
    res.writeHead(404).end('not found');
  }
});

await new Promise((r) => server.listen(0, r));
const port = server.address().port;

const browser = await chromium.launch({headless: false});
const page = await browser.newPage();

await page.addInitScript(() => {
  const s = {
    marks: [],          // {name, t} single-shot timeline points
    buckets: {},        // {name: {ms, calls}} accumulated wall time inside a call
    firstDrawMs: null,
    programs: [],
    sources: new Map(),
  };
  globalThis.__probe = s;

  const now = () => performance.now();
  const mark = (name) => {
    // Only the FIRST occurrence of each name; ordering is the whole point.
    if (!s.marks.some((m) => m.name === name)) s.marks.push({name, t: now()});
  };
  const markLast = (name) => {
    const hit = s.marks.find((m) => m.name === name);
    if (hit) hit.t = now();
    else s.marks.push({name, t: now()});
  };
  const bucket = (name, ms) => {
    const b = (s.buckets[name] ??= {ms: 0, calls: 0});
    b.ms += ms;
    b.calls += 1;
  };
  // Time a synchronous call into a bucket.
  const timed = (name, fn) => function (...args) {
    const t0 = now();
    try {
      return fn.apply(this, args);
    } finally {
      bucket(name, now() - t0);
    }
  };

  mark('scriptStart');

  // --- wasm: fetch, compile, instantiate -----------------------------------
  for (const name of ['compileStreaming', 'instantiateStreaming', 'compile', 'instantiate']) {
    const fn = WebAssembly[name];
    if (!fn) continue;
    WebAssembly[name] = function (...args) {
      mark(`wasm.${name}.start`);
      const t0 = now();
      const out = fn.apply(this, args);
      // Streaming variants return a promise; settle time is the real cost.
      return Promise.resolve(out).then((v) => {
        bucket(`wasm.${name}`, now() - t0);
        markLast(`wasm.${name}.end`);
        return v;
      });
    };
  }

  // --- network: which resources, when, how big -----------------------------
  const origFetch = globalThis.fetch;
  globalThis.fetch = function (input, init) {
    const url = String(typeof input === 'string' ? input : input?.url ?? '');
    const short = url.split('/').slice(-1)[0] || url;
    const t0 = now();
    mark(`fetch.first`);
    return origFetch.call(this, input, init).then((r) => {
      bucket(`fetch:${short}`, now() - t0);
      if (/\.ttf$/i.test(url)) markLast('font.lastFetchEnd');
      if (/\.wasm$/i.test(url)) markLast('wasm.fetchEnd');
      return r;
    });
  };
  const xhrOpen = XMLHttpRequest.prototype.open;
  const xhrSend = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.open = function (m, url, ...rest) {
    this.__url = String(url);
    return xhrOpen.call(this, m, url, ...rest);
  };
  XMLHttpRequest.prototype.send = function (...args) {
    const t0 = now();
    const short = (this.__url ?? '').split('/').slice(-1)[0];
    this.addEventListener('loadend', () => {
      bucket(`xhr:${short}`, now() - t0);
      if (/\.ttf$/i.test(this.__url ?? '')) markLast('font.lastXhrEnd');
    });
    return xhrSend.apply(this, args);
  };

  // --- GL context creation --------------------------------------------------
  const getContext = HTMLCanvasElement.prototype.getContext;
  HTMLCanvasElement.prototype.getContext = function (type, ...rest) {
    if (/webgl/i.test(String(type))) mark('gl.getContext');
    const t0 = now();
    const ctx = getContext.call(this, type, ...rest);
    if (/webgl/i.test(String(type))) bucket('gl.getContext', now() - t0);
    return ctx;
  };

  for (const Ctx of [WebGLRenderingContext, WebGL2RenderingContext]) {
    const proto = Ctx.prototype;

    const shaderSource = proto.shaderSource;
    proto.shaderSource = function (shader, src) {
      mark('shader.firstSource');
      markLast('shader.lastSource');
      s.sources.set(shader, src);
      // Split "time inside the GL call" from "time between GL calls". If the
      // 850ms window is gaps, the cost is upstream of JS: Rust GLSL codegen or
      // the wasm->JS string transfer, not ANGLE.
      const t0 = now();
      const out = shaderSource.call(this, shader, src);
      const dt = now() - t0;
      bucket('gl.shaderSource', dt);
      s.srcBytes = (s.srcBytes ?? 0) + (src?.length ?? 0);
      if (s.lastSourceEnd != null) bucket('gap.betweenShaderSource', t0 - s.lastSourceEnd);
      s.lastSourceEnd = t0 + dt;
      return out;
    };
    for (const n of ['createShader', 'createProgram', 'getShaderParameter', 'getShaderInfoLog']) {
      if (proto[n]) proto[n] = timed(`gl.${n}`, proto[n]);
    }

    const attachShader = proto.attachShader;
    proto.attachShader = function (program, shader) {
      const list = s.sources.get(program) ?? [];
      list.push(s.sources.get(shader) ?? '');
      s.sources.set(program, list);
      return attachShader.call(this, program, shader);
    };

    proto.compileShader = timed('gl.compileShader', proto.compileShader);
    proto.linkProgram = (function (fn) {
      return function (...args) {
        mark('link.first');
        markLast('link.last');
        const t0 = now();
        try {
          return fn.apply(this, args);
        } finally {
          bucket('gl.linkProgram', now() - t0);
        }
      };
    })(proto.linkProgram);

    // Introspection blocks the deferred D3D compile just like LINK_STATUS does.
    for (const n of ['getUniformLocation', 'getAttribLocation', 'getActiveUniform', 'getActiveAttrib']) {
      if (proto[n]) proto[n] = timed(`gl.${n}`, proto[n]);
    }

    const getProgramParameter = proto.getProgramParameter;
    proto.getProgramParameter = function (program, pname) {
      if (pname !== this.LINK_STATUS) {
        return getProgramParameter.call(this, program, pname);
      }
      mark('linkStatus.first');
      markLast('linkStatus.last');
      const t0 = now();
      const result = getProgramParameter.call(this, program, pname);
      const ms = now() - t0;
      bucket('gl.LINK_STATUS', ms);
      const src = (s.sources.get(program) ?? []).join('\n');
      s.programs.push({index: s.programs.length, ms, len: src.length, hasSlug: /io_slug_|io_scan_|slug_curve_count/.test(src)});
      return result;
    };

    // Texture + buffer uploads: the font atlas lands here.
    for (const n of ['texImage2D', 'texSubImage2D', 'compressedTexImage2D']) {
      if (proto[n]) proto[n] = (function (fn) {
        return function (...args) {
          mark(`gl.${n}.first`);
          const t0 = now();
          try {
            return fn.apply(this, args);
          } finally {
            bucket(`gl.${n}`, now() - t0);
          }
        };
      })(proto[n]);
    }
    for (const n of ['bufferData', 'bufferSubData']) {
      if (proto[n]) proto[n] = timed(`gl.${n}`, proto[n]);
    }
    if (proto.readPixels) proto.readPixels = timed('gl.readPixels', proto.readPixels);
    if (proto.finish) proto.finish = timed('gl.finish', proto.finish);

    for (const name of ['drawElements', 'drawArrays', 'drawElementsInstanced']) {
      const fn = proto[name];
      if (!fn) continue;
      proto[name] = function (...args) {
        if (s.firstDrawMs === null) {
          s.firstDrawMs = now();
          mark('firstDraw');
        }
        return fn.apply(this, args);
      };
    }
  }
});

await page.goto(`http://127.0.0.1:${port}/`, {waitUntil: 'commit'});
await page.waitForFunction(() => globalThis.__probe?.firstDrawMs !== null, null, {
  timeout: 180_000,
});

const report = await page.evaluate(() => {
  const s = globalThis.__probe;
  const nav = performance.getEntriesByType('navigation')[0];
  const res = performance.getEntriesByType('resource').map((r) => ({
    name: r.name.split('/').slice(-1)[0],
    start: +r.startTime.toFixed(1),
    dur: +r.duration.toFixed(1),
    size: r.encodedBodySize,
  }));
  return {
    firstDrawMs: +s.firstDrawMs.toFixed(1),
    programCount: s.programs.length,
    shaderSrcBytes: s.srcBytes ?? 0,
    perProgram: s.programs.map((p) => [p.len, +p.ms.toFixed(2)]),
    slugPrograms: s.programs.filter((p) => p.hasSlug).length,
    timeline: [...s.marks].sort((a, b) => a.t - b.t).map((m) => ({name: m.name, t: +m.t.toFixed(1)})),
    buckets: Object.fromEntries(
      Object.entries(s.buckets)
        .map(([k, v]) => [k, {ms: +v.ms.toFixed(1), calls: v.calls}])
        .filter(([, v]) => v.ms >= 1)
        .sort((a, b) => b[1].ms - a[1].ms),
    ),
    nav: nav && {
      responseEnd: +nav.responseEnd.toFixed(1),
      domContentLoaded: +nav.domContentLoadedEventEnd.toFixed(1),
    },
    resources: res.sort((a, b) => b.dur - a.dur).slice(0, 12),
  };
});

console.log(JSON.stringify(report, null, 2));

// Optional second argument: a directory to dump every program's GLSL into, so
// the 167 programs can be inventoried offline.
if (process.argv[3]) {
  const outDir = resolve(process.argv[3]);
  const {mkdir, writeFile} = await import('node:fs/promises');
  await mkdir(outDir, {recursive: true});
  const dump = await page.evaluate(() => {
    const s = globalThis.__probe;
    const out = [];
    for (const [key, val] of s.sources) {
      if (Array.isArray(val)) out.push(val); // programs only; shaders map to strings
    }
    return out;
  });
  let i = 0;
  for (const stages of dump) {
    await writeFile(join(outDir, `prog-${String(i).padStart(3, '0')}.glsl`), stages.join('\n// ---- stage break ----\n'));
    i++;
  }
  console.log(`dumped ${i} programs to ${outDir}`);
}

await browser.close();
server.close();
