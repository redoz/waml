// Non-mutating browser acceptance check for Mermaid reading-view extensions.
//
// Usage:
//   node scripts/mermaid-browser-check.mjs <path-to-waml(.exe)> <fixture-dir> <screenshot-path>
import {spawn} from 'node:child_process';
import {createHash} from 'node:crypto';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname, isAbsolute, join, relative, resolve} from 'node:path';

const PREFIX = 'mermaid-browser-check';
const COMPLETION_TIMEOUT_MS = 60_000;
const URL_PATTERN = /(http:\/\/127\.0\.0\.1:\d+\/\?api=\/api#token=[^\s]+)/;
const TRACE_MARKER = 'WAML_TEST_EXTENSION_PENDING';
const TRACE_PATTERN =
  /(?:^| - )WAML_TEST_EXTENSION_PENDING generation=(\d+) count=(\d+) ready=(\d+) failed=(\d+) loading=(\d+)$/;
const EXPECTED_OUTCOMES = Object.freeze({ready: 8, failed: 1, loading: 0});

function usage() {
  console.error(
    `usage: node scripts/mermaid-browser-check.mjs ` +
      '<path-to-waml(.exe)> <fixture-dir> <screenshot-path>',
  );
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function pathIsInside(parent, candidate) {
  const pathFromParent = relative(parent, candidate);
  return pathFromParent === '' || (!pathFromParent.startsWith('..') && !isAbsolute(pathFromParent));
}

function delay(milliseconds) {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));
}

function findChromiumExecutable() {
  if (process.env.PLAYWRIGHT_CHROMIUM_PATH) {
    return resolve(process.env.PLAYWRIGHT_CHROMIUM_PATH);
  }
  const installRoot = join(
    process.env.LOCALAPPDATA ?? join(process.env.USERPROFILE ?? '.', 'AppData', 'Local'),
    'ms-playwright',
  );
  for (const revision of ['chromium-1234', 'chromium-1228', 'chromium-1217']) {
    const executable = join(installRoot, revision, 'chrome-win64', 'chrome.exe');
    if (existsSync(executable)) return executable;
  }
  return null;
}

class CdpClient {
  constructor(url) {
    this.nextId = 1;
    this.pending = new Map();
    this.listeners = new Map();
    this.socket = new WebSocket(url);
    this.opened = new Promise((resolveOpen, rejectOpen) => {
      this.socket.addEventListener('open', resolveOpen, {once: true});
      this.socket.addEventListener('error', rejectOpen, {once: true});
    });
    this.socket.addEventListener('message', (event) => {
      const message = JSON.parse(String(event.data));
      if (message.id) {
        const pending = this.pending.get(message.id);
        if (!pending) return;
        this.pending.delete(message.id);
        if (message.error) pending.reject(new Error(`${pending.method}: ${message.error.message}`));
        else pending.resolve(message.result ?? {});
        return;
      }
      for (const listener of this.listeners.get(message.method) ?? []) {
        listener(message.params ?? {});
      }
    });
    this.socket.addEventListener('close', () => {
      for (const pending of this.pending.values()) {
        pending.reject(new Error(`CDP socket closed while waiting for ${pending.method}.`));
      }
      this.pending.clear();
    });
  }

  on(method, listener) {
    const listeners = this.listeners.get(method) ?? [];
    listeners.push(listener);
    this.listeners.set(method, listeners);
  }

  async send(method, params = {}) {
    await this.opened;
    const id = this.nextId++;
    const result = new Promise((resolveCommand, rejectCommand) => {
      this.pending.set(id, {method, resolve: resolveCommand, reject: rejectCommand});
    });
    this.socket.send(JSON.stringify({id, method, params}));
    return result;
  }

  close() {
    this.socket.close();
  }
}

async function evaluate(cdp, expression, awaitPromise = false) {
  const result = await cdp.send('Runtime.evaluate', {
    expression,
    awaitPromise,
    returnByValue: true,
  });
  if (result.exceptionDetails) {
    throw new Error(result.exceptionDetails.text ?? 'browser evaluation failed.');
  }
  return result.result?.value;
}

async function captureCanvas(cdp) {
  const rect = await evaluate(
    cdp,
    `(() => {
      const canvas = document.querySelector('canvas');
      if (!canvas) return null;
      const rect = canvas.getBoundingClientRect();
      return {x: rect.x + scrollX, y: rect.y + scrollY, width: rect.width, height: rect.height};
    })()`,
  );
  if (!rect || rect.width <= 0 || rect.height <= 0) {
    throw new Error('missing or empty canvas.');
  }
  const screenshot = await cdp.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
    clip: {...rect, scale: 1},
  });
  return Buffer.from(screenshot.data, 'base64');
}

async function assertMakepadCompatibleWasm(bootUrl) {
  const wasmUrl = new URL('/waml-editor.wasm', bootUrl);
  const response = await fetch(wasmUrl, {headers: {'Accept-Encoding': 'br'}});
  if (!response.ok) {
    throw new Error(`could not fetch packaged WASM import table: HTTP ${response.status}.`);
  }
  const wasmBytes = Buffer.from(await response.arrayBuffer());
  if (!wasmBytes.includes(Buffer.from(TRACE_MARKER))) {
    throw new Error(
      'packaged WASM does not include the browser-test-trace feature marker.',
    );
  }
  const module = await WebAssembly.compile(wasmBytes);
  const importModules = [...new Set(WebAssembly.Module.imports(module).map((entry) => entry.module))];
  const forbidden = importModules.filter((name) => name.startsWith('__wbindgen_'));
  if (forbidden.length) {
    throw new Error(
      `packaged WASM is incompatible with Makepad's env-only bridge; forbidden imports: ` +
        forbidden.join(', '),
    );
  }
  console.log(`${PREFIX}: packaged WASM has test trace and a Makepad-compatible import table`);
}

const binPath = process.argv[2] ? resolve(process.argv[2]) : null;
const fixtureDir = process.argv[3] ? resolve(process.argv[3]) : null;
const screenshotPath = process.argv[4] ? resolve(process.argv[4]) : null;

if (!binPath || !fixtureDir || !screenshotPath) {
  usage();
  process.exit(1);
}

const fixturePath = join(fixtureDir, 'index.md');
if (!existsSync(binPath)) {
  console.error(`${PREFIX}: no binary at ${binPath}. Build with --features embed-web first.`);
  process.exit(1);
}
if (!existsSync(fixturePath)) {
  console.error(`${PREFIX}: no fixture at ${fixturePath}.`);
  process.exit(1);
}
if (pathIsInside(fixtureDir, screenshotPath)) {
  console.error(`${PREFIX}: screenshot path must be outside the fixture directory.`);
  process.exit(1);
}

const fixtureBefore = readFileSync(fixturePath);
mkdirSync(dirname(screenshotPath), {recursive: true});

console.log(`${PREFIX}: launching ${binPath} serve ${fixtureDir} --port 0 --no-open`);
const child = spawn(binPath, ['serve', fixtureDir, '--port', '0', '--no-open'], {
  stdio: ['ignore', 'pipe', 'pipe'],
});

let stdout = '';
let stderr = '';
let earlyServerExit = null;
let terminatingServer = false;
child.stdout.on('data', (chunk) => {
  stdout += chunk.toString();
});
child.stderr.on('data', (chunk) => {
  stderr += chunk.toString();
});
child.on('error', (error) => {
  if (!terminatingServer) earlyServerExit = {error};
});
child.on('exit', (code, signal) => {
  if (!terminatingServer) earlyServerExit = {code, signal};
});

let cdp = null;
let chromiumChild = null;
let chromiumProfile = null;
let earlyChromiumExit = null;
let fatalBrowserError = null;
const traces = [];
const browserConsole = [];
const deadline = Date.now() + COMPLETION_TIMEOUT_MS;

function assertHealthy() {
  if (earlyServerExit) {
    if (earlyServerExit.error) {
      throw new Error(`could not start waml serve: ${earlyServerExit.error}`);
    }
    throw new Error(
      `waml serve exited early (code ${earlyServerExit.code}, signal ${earlyServerExit.signal}).` +
        `\nstdout:\n${stdout}\nstderr:\n${stderr}`,
    );
  }
  if (earlyChromiumExit) {
    throw new Error(
      `Chromium exited early (code ${earlyChromiumExit.code}, signal ${earlyChromiumExit.signal}).`,
    );
  }
  if (fatalBrowserError) throw fatalBrowserError;
  if (Date.now() >= deadline) {
    throw new Error(`${COMPLETION_TIMEOUT_MS / 1000}-second completion timeout expired.`);
  }
}

async function waitFor(description, predicate) {
  while (true) {
    assertHealthy();
    const value = await predicate();
    if (value) return value;
    await delay(20);
  }
}

async function stopServer() {
  if (child.exitCode !== null || child.signalCode !== null) return;
  terminatingServer = true;
  child.kill();
  const stopped = new Promise((resolveStopped) => child.once('close', resolveStopped));
  await Promise.race([stopped, delay(5_000)]);
  if (child.exitCode === null && child.signalCode === null) {
    child.kill('SIGKILL');
    await new Promise((resolveStopped) => child.once('close', resolveStopped));
  }
}

async function stopChromium() {
  if (!chromiumChild || chromiumChild.exitCode !== null || chromiumChild.signalCode !== null) return;
  chromiumChild.kill();
  const stopped = new Promise((resolveStopped) => chromiumChild.once('close', resolveStopped));
  await Promise.race([stopped, delay(5_000)]);
  if (chromiumChild.exitCode === null && chromiumChild.signalCode === null) {
    chromiumChild.kill('SIGKILL');
    await new Promise((resolveStopped) => chromiumChild.once('close', resolveStopped));
  }
}

let exitCode = 0;
try {
  const url = await waitFor('printed server URL', () => stdout.match(URL_PATTERN)?.[1] ?? null);
  console.log(`${PREFIX}: boot URL ${url}`);
  await assertMakepadCompatibleWasm(url);

  const chromiumExecutable = findChromiumExecutable();
  if (!chromiumExecutable) {
    throw new Error(
      'no installed Playwright Chromium found. Set PLAYWRIGHT_CHROMIUM_PATH to chrome.exe.',
    );
  }
  chromiumProfile = mkdtempSync(join(tmpdir(), 'waml-mermaid-chromium-'));
  let chromiumStderr = '';
  chromiumChild = spawn(
    chromiumExecutable,
    [
      '--headless=new',
      '--remote-debugging-port=0',
      `--user-data-dir=${chromiumProfile}`,
      '--no-first-run',
      '--no-default-browser-check',
      '--disable-background-networking',
      'about:blank',
    ],
    {stdio: ['ignore', 'ignore', 'pipe'], windowsHide: true},
  );
  chromiumChild.stderr.on('data', (chunk) => {
    chromiumStderr += chunk.toString();
  });
  chromiumChild.on('error', (error) => {
    fatalBrowserError ??= new Error(`could not start Chromium: ${error}`);
  });
  chromiumChild.on('exit', (code, signal) => {
    earlyChromiumExit = {code, signal};
  });
  const devtoolsUrl = await waitFor('Chromium DevTools URL', () =>
    chromiumStderr.match(/DevTools listening on (ws:\/\/[^\s]+)/)?.[1] ?? null,
  );
  const devtoolsHttp = devtoolsUrl.replace(/^ws:/, 'http:').replace(/\/devtools\/browser\/.*$/, '');
  const targets = await waitFor('Chromium page target', async () => {
    const response = await fetch(`${devtoolsHttp}/json/list`);
    if (!response.ok) return null;
    const entries = await response.json();
    return entries.find((entry) => entry.type === 'page') ?? null;
  });
  cdp = new CdpClient(targets.webSocketDebuggerUrl);
  let pageLoaded = false;
  cdp.on('Page.loadEventFired', () => {
    pageLoaded = true;
  });
  cdp.on('Runtime.consoleAPICalled', ({args, type}) => {
    const text = (args ?? [])
      .map((argument) => argument.value ?? argument.description ?? '')
      .join(' ');
    browserConsole.push(`${type}: ${text}`);
    if (/panic/i.test(text)) {
      fatalBrowserError ??= new Error(`console reported a panic: ${text}`);
    }
    if (!text.includes(TRACE_MARKER)) return;
    const match = text.match(TRACE_PATTERN);
    if (!match) {
      fatalBrowserError ??= new Error(`malformed pending trace: ${text}`);
      return;
    }
    const trace = {
      generation: match[1],
      count: Number(match[2]),
      ready: Number(match[3]),
      failed: Number(match[4]),
      loading: Number(match[5]),
    };
    if (trace.count !== trace.loading) {
      fatalBrowserError ??= new Error(`pending/loading trace mismatch: ${text}`);
      return;
    }
    traces.push(trace);
  });
  cdp.on('Runtime.exceptionThrown', ({exceptionDetails}) => {
    const detail = exceptionDetails?.exception?.description ?? exceptionDetails?.text ?? 'unknown error';
    fatalBrowserError ??= new Error(`page error: ${detail}`);
  });
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width: 1280,
    height: 900,
    deviceScaleFactor: 1,
    mobile: false,
  });
  pageLoaded = false;
  await cdp.send('Page.navigate', {url});
  await waitFor('page load', () => pageLoaded);
  await waitFor('canvas', async () => evaluate(cdp, "document.querySelector('canvas') !== null"));
  assertHealthy();

  const positive = await waitFor('positive pending trace', () => {
    const index = traces.findIndex((trace) => trace.count > 0 && trace.loading > 0);
    return index < 0 ? null : {index, trace: traces[index]};
  });
  const loadingCanvas = await captureCanvas(cdp);
  const loadingHash = sha256(loadingCanvas);
  console.log(
    `${PREFIX}: generation ${positive.trace.generation} pending ${positive.trace.count}; ` +
      `loading canvas ${loadingHash}`,
  );

  const settled = await waitFor('same-generation settled outcome trace', () =>
    traces.slice(positive.index + 1).find(
      (trace) =>
        trace.generation === positive.trace.generation &&
        trace.count === 0 &&
        trace.ready === EXPECTED_OUTCOMES.ready &&
        trace.failed === EXPECTED_OUTCOMES.failed &&
        trace.loading === EXPECTED_OUTCOMES.loading,
    ),
  );
  console.log(
    `${PREFIX}: generation ${settled.generation} ready ${settled.ready}, failed ${settled.failed}, ` +
      `loading ${settled.loading}; fixture contract satisfied`,
  );

  let stableCount = 0;
  let previousHash = null;
  let stableCanvas = null;
  while (stableCount < 3) {
    assertHealthy();
    await evaluate(cdp, 'new Promise((resolveFrame) => requestAnimationFrame(resolveFrame))', true);
    const frame = await captureCanvas(cdp);
    const frameHash = sha256(frame);
    if (frameHash === previousHash) {
      stableCount += 1;
    } else {
      previousHash = frameHash;
      stableCount = 1;
    }
    stableCanvas = frame;
  }

  const stableHash = sha256(stableCanvas);
  if (stableHash === loadingHash) {
    throw new Error('stable post-zero canvas matches the early loading canvas.');
  }
  console.log(`${PREFIX}: three stable post-zero canvas frames ${stableHash}`);

  assertHealthy();
  const metrics = await cdp.send('Page.getLayoutMetrics');
  const contentSize = metrics.cssContentSize ?? metrics.contentSize;
  const fullPage = await cdp.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
    clip: {x: 0, y: 0, width: contentSize.width, height: contentSize.height, scale: 1},
  });
  writeFileSync(screenshotPath, Buffer.from(fullPage.data, 'base64'));
  assertHealthy();

  const fixtureAfter = readFileSync(fixturePath);
  if (!fixtureAfter.equals(fixtureBefore)) {
    throw new Error('fixture bytes changed during the browser check.');
  }
  console.log(`${PREFIX}: fixture bytes unchanged`);
  console.log(`${PREFIX}: PASS`);
} catch (error) {
  const consoleContext = browserConsole.length
    ? `\nbrowser console:\n${browserConsole.join('\n')}`
    : '';
  console.error(`${PREFIX}: FAIL -- ${error?.message ?? error}${consoleContext}`);
  exitCode = 1;
} finally {
  if (cdp) cdp.close();
  await stopChromium();
  if (chromiumProfile && pathIsInside(tmpdir(), chromiumProfile)) {
    rmSync(chromiumProfile, {recursive: true, force: true});
  }
  if (earlyServerExit && exitCode === 0) {
    console.error(`${PREFIX}: FAIL -- waml serve exited before the check released it.`);
    exitCode = 1;
  }
  const fixtureAfter = readFileSync(fixturePath);
  if (!fixtureAfter.equals(fixtureBefore)) {
    console.error(`${PREFIX}: FAIL -- fixture bytes changed during cleanup.`);
    exitCode = 1;
  }
  await stopServer();
}

process.exit(exitCode);
