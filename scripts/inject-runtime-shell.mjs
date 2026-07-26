// Inject the two pieces of runtime behaviour the hosted editor needs into a
// `cargo makepad wasm build` artifact: the branded loading screen, and the
// new-version check.
//
// Like scripts/brand-web-artifact.mjs, this patches the GENERATED index.html
// rather than owning a checked-in copy -- cargo-makepad writes that file, so a
// checked-in copy would mean re-syncing the whole wasm loader every time the
// makepad pin moves. Every insertion is anchored on a string from
// cargo-makepad's template and fails loudly if that string is gone, so a
// restructured template breaks the build instead of silently shipping an
// artifact with no loader and no update check.
//
// Idempotent: a rerun over an already-injected artifact is a no-op.
//
// Usage: node scripts/inject-runtime-shell.mjs <artifact-dir> <build-id>

import { readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [artifactDir, buildId] = process.argv.slice(2);
if (!artifactDir || !buildId) {
  console.error("usage: node scripts/inject-runtime-shell.mjs <artifact-dir> <build-id>");
  process.exit(1);
}

const MARKER = "waml-runtime-shell";
const indexPath = join(artifactDir, "index.html");
let html = readFileSync(indexPath, "utf8");

if (html.includes(MARKER)) {
  console.log("inject-runtime-shell: already injected");
  process.exit(0);
}

// ---------------------------------------------------------------------------
// version.json -- the update check's source of truth.
//
// `wasmBytes` is the DECOMPRESSED size, and it is load-bearing for the loader:
// GitHub Pages gzips/brotlis the wasm on the fly, so `Content-Length` on the
// response is the encoded length while the bytes we count off the stream are
// decoded. Dividing by the header would sail past 100%.
// ---------------------------------------------------------------------------

const wasmName = "waml-editor.wasm";
const wasmBytes = statSync(join(artifactDir, wasmName)).size;
writeFileSync(
  join(artifactDir, "version.json"),
  `${JSON.stringify({ build: buildId, wasmBytes }, null, 2)}\n`,
);

// ---------------------------------------------------------------------------
// Loading screen.
//
// cargo-makepad emits a `.canvas_loader` div containing a grey "Loading.." bar,
// and deletes the whole element once the app presents its first frame (see
// `remove_canvas_loader` in makepad's web.js). So we are free to replace its
// contents wholesale; teardown still works.
//
// The mark is the six-segment WAML logo, read from the same waml.svg the rest
// of the project uses. Two stacked copies: a dim ghost, and a full-colour one
// whose segments illuminate left-to-right as their portions download.
// ---------------------------------------------------------------------------

const logoSvg = readFileSync("waml.svg", "utf8");
const groupMatch = logoSvg.match(/<g\s+stroke-width[\s\S]*?<\/g>/);
if (!groupMatch) {
  console.error("inject-runtime-shell: could not find the polygon group in waml.svg");
  process.exit(1);
}
// Ids are duplicated across the two copies below, so strip them rather than
// emit an invalid document.
const polygons = [...groupMatch[0].matchAll(/<polygon\b[^>]*\/>/g)].map(
  ([polygon]) => polygon.replace(/\s+id="[^"]*"/g, ""),
);
if (polygons.length !== 6) {
  console.error(
    `inject-runtime-shell: expected 6 logo segments, found ${polygons.length}`,
  );
  process.exit(1);
}
const VIEW_BOX = "-13.2521 -38.848 68.4573 40.848";
const ghostSegments = polygons.join("");
const progressSegments = polygons
  .map((polygon, index) =>
    polygon.replace(
      "<polygon",
      `<polygon class='waml_loader_segment' style='--segment-index: ${index}'`,
    ),
  )
  .join("");

const LOADER_BODY = `
        <div class='canvas_loader' data-phase='loading'>
            <div class='waml_loader_content'>
                <svg class='waml_loader_mark' viewBox='${VIEW_BOX}' aria-label='Loading WAML'>
                    <g opacity='0.16'>${ghostSegments}</g>
                    <g>${progressSegments}</g>
                </svg>
                <div class='waml_loader_status' role='status' aria-live='polite'>Loading…</div>
            </div>
        </div>`;

// Anchor on cargo-makepad's loader markup. Matched loosely on the class so a
// whitespace change in the template does not break the build, but the element
// itself disappearing still does.
const loaderRe = /<div class='canvas_loader'[\s\S]*?<\/div>\s*<\/div>/;
if (!loaderRe.test(html)) {
  console.error(`inject-runtime-shell: could not find the canvas_loader div in ${indexPath}`);
  process.exit(1);
}
html = html.replace(loaderRe, LOADER_BODY.trim());

const LOADER_CSS = `
        <style data-${MARKER}>
            .canvas_loader {
                display: flex;
                align-items: center;
                justify-content: center;
                background-color: #16181c;
            }
            /* makepad's own ".canvas_loader div" rule (the grey status strip)
               no longer matches anything: the child it styled is gone, replaced
               by this svg. Sizing here is therefore standalone, not an override. */
            .canvas_loader .waml_loader_mark {
                width: min(38vw, 260px);
                height: auto;
                background: none;
                padding: 0;
                overflow: visible;
            }
            .canvas_loader .waml_loader_content {
                display: flex;
                flex-direction: column;
                align-items: center;
                gap: 18px;
            }
            .canvas_loader .waml_loader_segment {
                opacity: 0;
                transition: opacity 140ms linear;
            }
            .canvas_loader .waml_loader_status {
                color: #8f96a3;
                font-family: system-ui, sans-serif;
                font-size: 13px;
                letter-spacing: 0.02em;
            }
            .waml_update_toast {
                position: fixed;
                right: 16px;
                bottom: 16px;
                z-index: 10;
                display: flex;
                align-items: center;
                gap: 12px;
                padding: 10px 12px 10px 14px;
                border-radius: 8px;
                background: #22262e;
                border: 1px solid #363c47;
                box-shadow: 0 6px 24px rgba(0, 0, 0, 0.45);
                color: #d6dae2;
                font-family: system-ui, sans-serif;
                font-size: 13px;
            }
            .waml_update_toast button {
                font: inherit;
                color: #16181c;
                background: #f0aa3c;
                border: 0;
                border-radius: 5px;
                padding: 6px 10px;
                cursor: pointer;
            }
        </style>`;

// ---------------------------------------------------------------------------
// Runtime script.
//
// CLASSIC script, not a module: it has to install the fetch wrapper before
// cargo-makepad's `<script type='module'>` runs, and modules are deferred while
// classic scripts execute at parse time. Position in head is therefore not
// load-bearing, but ordering relative to the module script is.
// ---------------------------------------------------------------------------

const RUNTIME_JS = `
        <script data-${MARKER}>
        (function () {
            var BUILD = ${JSON.stringify(buildId)};
            var WASM_BYTES = ${wasmBytes};
            var POLL_MS = 5 * 60 * 1000;

            // -- download progress -------------------------------------------
            //
            // makepad calls WebAssembly.compileStreaming(fetch(url)) with no
            // progress hook, so wrap fetch and re-body the response through a
            // counting stream. Headers are carried over verbatim because
            // compileStreaming rejects anything that is not application/wasm.
            var progress = 0;
            var applyProgress = function () {
                var segments = document.querySelectorAll('.waml_loader_segment');
                for (var index = 0; index < segments.length; index += 1) {
                    var segment = segments[index];
                    var level = Math.max(
                        0,
                        Math.min(1, progress * segments.length - index)
                    );
                    segment.style.opacity = level.toFixed(3);
                }
            };
            var setProgress = function (p) {
                progress = Math.max(progress, Math.min(1, p));
                applyProgress();
            };
            document.addEventListener('DOMContentLoaded', applyProgress);

            var nativeFetch = window.fetch.bind(window);
            window.fetch = function (input, init) {
                var url = typeof input === 'string' ? input : (input && input.url) || '';
                if (url.indexOf('.wasm') === -1) {
                    return nativeFetch(input, init);
                }
                // Cache-bust on the build id. The wasm filename is fixed by
                // cargo-makepad's generated loader, and GitHub Pages lets us set
                // no headers, so the query string is the only lever: a new build
                // is a new URL and cannot be served from a stale entry.
                if (typeof input === 'string' && url.indexOf('?') === -1) {
                    input = url + '?v=' + encodeURIComponent(BUILD);
                }
                return nativeFetch(input, init).then(function (response) {
                    if (!response.ok || !response.body || !window.ReadableStream) {
                        return response;
                    }
                    var read = 0;
                    var reader = response.body.getReader();
                    var counted = new ReadableStream({
                        pull: function (controller) {
                            return reader.read().then(function (chunk) {
                                if (chunk.done) {
                                    setProgress(1);
                                    controller.close();
                                    return;
                                }
                                read += chunk.value.byteLength;
                                // Denominator is the build-time decompressed
                                // size; see version.json above. Clamp anyway so
                                // a mismatch degrades to a stuck bar, never a
                                // bar that overshoots.
                                setProgress(WASM_BYTES ? read / WASM_BYTES : 0);
                                controller.enqueue(chunk.value);
                            });
                        },
                        cancel: function (reason) {
                            return reader.cancel(reason);
                        }
                    });
                    return new Response(counted, {
                        status: response.status,
                        statusText: response.statusText,
                        headers: response.headers
                    });
                });
            };

            // -- update check ------------------------------------------------
            var shown = false;
            var lastCheck = 0;

            var showToast = function (nextBuild) {
                if (shown) { return; }
                shown = true;
                var toast = document.createElement('div');
                toast.className = 'waml_update_toast';
                var text = document.createElement('span');
                text.textContent = 'New version available.';
                var button = document.createElement('button');
                button.textContent = 'Update';
                button.addEventListener('click', function () {
                    // The editor keeps the open document in location.hash (see
                    // App::sync_share_url), so the current model rides along
                    // without this script ever calling into wasm.
                    //
                    // The ?v= query is what actually forces the reload: Pages
                    // serves index.html with a ~10 minute max-age and we cannot
                    // override it, so reloading the same URL could re-run the
                    // stale document and prompt again immediately.
                    //
                    // replace(), not assign(): the pre-update page is not
                    // somewhere Back should return to.
                    location.replace(
                        location.pathname + '?v=' + encodeURIComponent(nextBuild) + location.hash
                    );
                });
                toast.appendChild(text);
                toast.appendChild(button);
                document.body.appendChild(toast);
            };

            var check = function () {
                if (shown) { return; }
                lastCheck = Date.now();
                nativeFetch('./version.json', { cache: 'no-store' })
                    .then(function (r) { return r.ok ? r.json() : null; })
                    .then(function (v) {
                        if (v && v.build && v.build !== BUILD) {
                            showToast(v.build);
                        }
                    })
                    .catch(function () {
                        // Offline or a transient 5xx: the next tick retries.
                    });
            };

            setInterval(check, POLL_MS);
            document.addEventListener('visibilitychange', function () {
                // A tab left open for days is the case this whole feature
                // exists for, so check on refocus -- throttled, because
                // visibilitychange fires on every alt-tab.
                if (document.visibilityState === 'visible' && Date.now() - lastCheck > 60000) {
                    check();
                }
            });
        })();
        </script>`;

const anchor = "<meta charset='utf-8'>";
if (!html.includes(anchor)) {
  console.error(`inject-runtime-shell: could not find ${anchor} in ${indexPath}`);
  process.exit(1);
}
html = html.replace(anchor, `${anchor}${LOADER_CSS}${RUNTIME_JS}`);

writeFileSync(indexPath, html);
console.log(
  `inject-runtime-shell: build ${buildId}, ${wasmName} ${wasmBytes} bytes, loader + update check injected`,
);
