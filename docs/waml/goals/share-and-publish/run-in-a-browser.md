# Run in a Browser

**Goal:** The same editor runs as a web artifact and shows the same views as
the desktop form.

**Why:** One application, two delivery forms. A separate web viewer would be a
second product to keep correct.

**Done when:** The web form renders every view the native form does, boots in a
time a reader will wait through, and reports a failure instead of showing a
blank canvas.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The artifact is built with `cargo makepad wasm build -p waml-editor --release
  --no-threads`. `--no-threads` is required: the threaded build needs
  cross-origin isolation headers that GitHub Pages cannot set.
- Boot time was the headline problem and is largely fixed: batched shader
  linking took boot from roughly nine seconds to under two.
- The web renderer has known gaps against the native one — diagonal-blindness
  in some drawing paths among them — so "same views" is not yet literally true.
- A web artifact verification script guards against the build silently shipping
  without its JavaScript glue.
