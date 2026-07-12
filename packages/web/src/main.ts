import "./app.css";
import { initWasm } from "@uaml/okf";

// The WASM core is the source of truth — block first render until it's ready. The
// app (and, transitively, the bundle-as-truth store in `bootstrap.ts`) is only
// imported AFTER `initWasm()` resolves, so `build_model`/`apply_ops` are always
// callable synchronously downstream. On failure we show a hard load error and do
// NOT mount (no TS fallback).
const target = document.getElementById("app")!;

void initWasm().then(
  async () => {
    const [{ mount }, { default: App }] = await Promise.all([import("svelte"), import("./App.svelte")]);
    mount(App, { target });
  },
  (err) => {
    console.error("Failed to initialize the UAML engine", err);
    target.innerHTML =
      '<div style="display:flex;height:100vh;align-items:center;justify-content:center;font-family:system-ui,sans-serif;color:#334155;padding:24px;text-align:center;font-size:14px">Couldn’t load the editor engine. Please reload the page.</div>';
  },
);
