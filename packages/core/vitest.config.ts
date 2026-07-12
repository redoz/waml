import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "jsdom",
    // Several suites (model, ops-adapter, templates, url) each instantiate the
    // ~2 MB inlined WASM core in their own beforeAll. Running files in parallel
    // means many concurrent instantiations; under a cold cache and the combined
    // `pnpm -r test` load this occasionally crashed a worker, cascading sibling
    // files to fail and their tests to be skipped (a rare, cold-start-only flake).
    // Serializing the files here caps peak WASM instantiation to one at a time.
    // Costs a few extra seconds (serialized jsdom setup); worth it to kill the flake.
    fileParallelism: false,
  },
});
