# Clean Site Boot URL Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An exported site boots its own `bundle.waml` without ever showing `?bundle=bundle.waml` in the address bar.

**Architecture:** Today the boot source reaches the wasm through the URL: `scripts/inject-runtime-shell.mjs` injects a snippet that `history.replaceState`s `?bundle=bundle.waml` (stamped in by `crates/waml-cli/src/site.rs`) onto a bare URL, because the editor reads `WebParams.search` once at startup. Replace that channel with a sibling file: `waml export site` writes the same query string into `waml-boot.txt` next to `index.html`, and the editor — only when the URL asks for nothing — fetches `./waml-boot.txt` and feeds its contents through the existing `select_browser_boot` parser. The URL stays `/`, the boot-source grammar is unchanged, and an explicit `?bundle=` / `?api=` / `#w1.` link still wins because the config is consulted only in the `Start` arm.

**Tech Stack:** Rust (`waml-editor` wasm32 target, `waml-cli`), makepad `cx.http_request`, Node test runner (`node --test`) for the `scripts/*.test.mjs` suites, Playwright for `scripts/export-site-browser.test.mjs`.

## Global Constraints

- Use ASD-STE100 Simplified Technical English in all comments and docs.
- Comments explain **why**, in the voice of the surrounding files — those files carry dense rationale comments; match that density, do not strip it.
- Every launch of the native editor during verification must pass `-Title <slug>` (see `AGENTS.md`).
- The boot-config file name is `waml-boot.txt`, declared **once** in `crates/waml-editor/src/browser_boot.rs` as `BOOT_CONFIG_FILE` and repeated as a literal only in the CLI, the two Node scripts, and their tests.
- The config file holds a query string: it starts with `?` and is parsed by the existing `select_browser_boot`. No JSON, no new dependency.
- The `Start` arm is the ONLY place that reads the config. A share fragment or an explicit query must never trigger the config fetch.
- A missing or unparsable config is never an error the user sees: it leaves the start screen up. A raw `cargo makepad wasm build` artifact carries no config file and must boot silently.
- Full gate for every task: `cargo test --workspace` and, for tasks that touch `scripts/`, `node --test scripts/`.
- Do not add a `Co-Authored-By` trailer to commits.

---

### Task 1: The pure boot-config parser

Add the file name constant and a pure function that turns fetched config bytes into a `BrowserBootSource`. No I/O, no wasm — this is host-testable and lands with its own tests.

**Files:**
- Modify: `crates/waml-editor/src/browser_boot.rs` (module doc at lines 1–13; add const + function after `select_browser_boot`, which ends at line 60; tests in the `mod tests` block that starts at line 131)

**Interfaces:**
- Consumes: `select_browser_boot(search: &str, hash: &str) -> Result<BrowserBootSource, String>` and `enum BrowserBootSource { Share(String), Api { base, token }, Bundle(String), Start }`, both already in this file.
- Produces:
  - `pub(crate) const BOOT_CONFIG_FILE: &str = "waml-boot.txt";`
  - `pub(crate) fn select_site_boot(config: &str) -> Result<BrowserBootSource, String>`

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block at the bottom of `crates/waml-editor/src/browser_boot.rs`:

```rust
    #[test]
    fn a_site_config_naming_a_bundle_boots_it() {
        assert_eq!(
            select_site_boot("?bundle=bundle.waml\n").unwrap(),
            BrowserBootSource::Bundle("bundle.waml".to_string())
        );
    }

    #[test]
    fn a_site_config_naming_a_server_boots_the_api() {
        assert_eq!(
            select_site_boot("?api=/api").unwrap(),
            BrowserBootSource::Api {
                base: "/api".to_string(),
                token: None,
            }
        );
    }

    #[test]
    fn an_html_error_page_is_not_a_site_config() {
        let error = select_site_boot("<!doctype html><title>404</title>").unwrap_err();
        assert!(error.contains("waml-boot.txt"), "{error}");
    }

    #[test]
    fn an_empty_site_config_is_the_start_screen() {
        assert_eq!(select_site_boot("   \n").unwrap(), BrowserBootSource::Start);
    }

    #[test]
    fn a_site_config_can_never_be_a_share_link() {
        // The fragment is the page's, not the file's: a config that tries to
        // carry one would let an exported site's own bytes decide what a
        // visitor's URL means.
        assert_eq!(
            select_site_boot("?bundle=a.waml#w1.abc").unwrap(),
            BrowserBootSource::Bundle("a.waml".to_string())
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor browser_boot`
Expected: FAIL — `cannot find function 'select_site_boot' in this scope`

- [ ] **Step 3: Write the implementation**

Insert after `select_browser_boot` (after line 60) in `crates/waml-editor/src/browser_boot.rs`:

```rust
/// The file an exported site carries beside its editor, naming what to boot.
///
/// The boot source used to travel in the URL, which meant every visitor saw
/// `?bundle=bundle.waml` in the address bar for a choice they did not make. A
/// sibling file carries the same string out of sight. It holds a query string,
/// not a new format, so one grammar and one parser serve both channels.
pub(crate) const BOOT_CONFIG_FILE: &str = "waml-boot.txt";

/// Pick the boot source a site declares for itself.
///
/// The contents are a query string as `select_browser_boot` reads one. A host
/// that serves an error page (or an `index.html`) at this path answers 200 with
/// HTML, so anything that does not open with `?` is refused rather than parsed
/// into nonsense. The empty file is not an error: it is a site that declares
/// the start screen.
///
/// Only the page URL can carry a share fragment, so the config is parsed with
/// an empty `hash` and cannot reach [`BrowserBootSource::Share`].
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn select_site_boot(config: &str) -> Result<BrowserBootSource, String> {
    let query = config.trim();
    if query.is_empty() {
        return Ok(BrowserBootSource::Start);
    }
    if !query.starts_with('?') {
        return Err(format!(
            "{BOOT_CONFIG_FILE} must hold a query string that starts with '?'"
        ));
    }
    let query = query.split('#').next().unwrap_or(query);
    select_browser_boot(query, "")
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml-editor browser_boot`
Expected: PASS, all tests in the module.

- [ ] **Step 5: Update the module doc**

In `crates/waml-editor/src/browser_boot.rs`, replace lines 5–8 of the module doc:

```rust
//! request and the only one the user can have edited by hand. Then `?api=`,
//! then a static `?bundle=` as exported sites use, and finally the start
//! screen when the URL says nothing.
```

with:

```rust
//! request and the only one the user can have edited by hand. Then `?api=`,
//! then a static `?bundle=`, and finally the start screen -- which, on an
//! exported site, is where [`select_site_boot`] takes over: the site's own
//! `waml-boot.txt` names what to open when the URL names nothing, so the
//! address bar stays clean.
```

- [ ] **Step 6: Gate and commit**

```bash
cargo test --workspace
git add crates/waml-editor/src/browser_boot.rs
git commit -m "feat(editor): parse a site's own boot config"
```

---

### Task 2: The editor fetches its site config

Wire the `Start` arm to fetch `./waml-boot.txt` and route the answer. The bundle fetch that already exists moves into a helper so both entry points share it.

**Files:**
- Modify: `crates/waml-editor/src/app.rs` — `handle_startup` (wasm32, lines 712–760), `handle_http_response` (lines 764–787), `handle_http_request_error` (lines 789–797), and the `pending_boot_bundle` field doc at line 671.

**Interfaces:**
- Consumes: `browser_boot::BOOT_CONFIG_FILE`, `browser_boot::select_site_boot`, `browser_boot::select_browser_boot`, `browser_boot::decode_boot_bundle`, `browser_boot::boot_fetch_error` (Task 1 and existing).
- Produces: nothing other tasks call — this is the wasm wiring.

- [ ] **Step 1: Add the bundle-fetch helper**

In `crates/waml-editor/src/app.rs`, add above `handle_http_response` (before line 762):

```rust
    /// Start the fetch of a Bundle Envelope v1 file, from either channel.
    ///
    /// The start screen holds the window until the answer lands -- never a
    /// blank one -- and stays put if the fetch fails.
    #[cfg(target_arch = "wasm32")]
    fn start_boot_bundle_fetch(&mut self, cx: &mut Cx, url: String) {
        self.pending_boot_bundle = Some(url.clone());
        cx.http_request(live_id!(boot_bundle), HttpRequest::new(url, HttpMethod::GET));
    }
```

- [ ] **Step 2: Rewrite the `handle_startup` match arms**

Replace lines 744–759 of `crates/waml-editor/src/app.rs`:

```rust
            crate::browser_boot::BrowserBootSource::Bundle(url) => {
                // The fetch lands in `handle_http_response`. Until it does the
                // start screen holds the window -- never a blank one -- and it
                // stays put if the fetch fails.
                self.pending_boot_bundle = Some(url.clone());
                self.show_start_screen(cx);
                cx.http_request(
                    live_id!(boot_bundle),
                    HttpRequest::new(url, HttpMethod::GET),
                );
            }
            // `?api=` is selected for, but no live model server exists yet; the
            // URL is honoured as far as "not a bundle, not a share link".
            crate::browser_boot::BrowserBootSource::Api { .. }
            | crate::browser_boot::BrowserBootSource::Start => self.show_start_screen(cx),
```

with:

```rust
            crate::browser_boot::BrowserBootSource::Bundle(url) => {
                self.show_start_screen(cx);
                self.start_boot_bundle_fetch(cx, url);
            }
            // `?api=` is selected for, but no live model server exists yet; the
            // URL is honoured as far as "not a bundle, not a share link".
            crate::browser_boot::BrowserBootSource::Api { .. } => self.show_start_screen(cx),
            // The URL names nothing, so ask the page's own site config. An
            // exported site answers with the query it used to push into the
            // address bar; a raw artifact answers 404 and the start screen is
            // already up, so nothing more happens.
            crate::browser_boot::BrowserBootSource::Start => {
                self.show_start_screen(cx);
                cx.http_request(
                    live_id!(boot_config),
                    HttpRequest::new(
                        format!("./{}", crate::browser_boot::BOOT_CONFIG_FILE),
                        HttpMethod::GET,
                    ),
                );
            }
```

- [ ] **Step 3: Route the config response**

Replace `handle_http_response` (lines 762–787) of `crates/waml-editor/src/app.rs` with:

```rust
    /// A boot fetch came back: either the site config or the bundle it named.
    ///
    /// Anything other than a 2xx carrying what was asked for leaves the start
    /// screen up. The config's failures are quiet -- a build served straight
    /// out of `cargo makepad wasm build` has no config file, and a 404 there is
    /// the normal case, not a fault to report.
    #[cfg(target_arch = "wasm32")]
    fn handle_http_response(&mut self, cx: &mut Cx, request_id: LiveId, response: &HttpResponse) {
        let ok = response.status_code >= 200 && response.status_code < 300;
        let body = response.get_body().map(Vec::as_slice).unwrap_or(&[]);
        if request_id == live_id!(boot_config) {
            if !ok {
                return;
            }
            let Ok(config) = std::str::from_utf8(body) else {
                return;
            };
            match crate::browser_boot::select_site_boot(config) {
                Ok(crate::browser_boot::BrowserBootSource::Bundle(url)) => {
                    self.start_boot_bundle_fetch(cx, url)
                }
                Ok(_) => {}
                Err(e) => log!("could not read this site's boot config: {e}"),
            }
            return;
        }
        if request_id != live_id!(boot_bundle) {
            return;
        }
        let Some(url) = self.pending_boot_bundle.take() else {
            return;
        };
        if !ok {
            log!(
                "{}",
                crate::browser_boot::boot_fetch_error(&url, Some(response.status_code))
            );
            return;
        }
        match crate::browser_boot::decode_boot_bundle(body) {
            Ok(bundle) => {
                self.open_bundle(cx, bundle, "exported".to_string(), None);
                self.show_editor(cx);
            }
            Err(e) => log!("could not open {url}: {e}"),
        }
    }
```

- [ ] **Step 4: Leave a failed config fetch silent**

`handle_http_request_error` (lines 789–797) already returns early for any id other than `boot_bundle`, so a dead config fetch is silent by construction. Confirm the guard is unchanged and add the reason to its body:

```rust
    /// A boot fetch never produced a response. The config's failure is silent
    /// on purpose (see `handle_http_response`); the bundle's is named, because
    /// a site that promised a bundle and did not deliver one is a fault.
    #[cfg(target_arch = "wasm32")]
    fn handle_http_request_error(&mut self, _cx: &mut Cx, request_id: LiveId, _error: &HttpError) {
        if request_id != live_id!(boot_bundle) {
            return;
        }
        if let Some(url) = self.pending_boot_bundle.take() {
            log!("{}", crate::browser_boot::boot_fetch_error(&url, None));
        }
    }
```

- [ ] **Step 5: Update the field doc**

At `crates/waml-editor/src/app.rs:671`, replace `/// URL of the in-flight `?bundle=` boot fetch, so its response can name it` with:

```rust
    /// URL of the in-flight boot-bundle fetch -- asked for by the page URL or
    /// by the site's own config -- so its response can name it in an error.
```

- [ ] **Step 6: Build both targets**

Run: `cargo test --workspace`
Expected: PASS.

Run: `cargo makepad wasm build -p waml-editor` (or `cargo check -p waml-editor --target wasm32-unknown-unknown` if `cargo-makepad` is not installed)
Expected: compiles — the new arms are `#[cfg(target_arch = "wasm32")]` and are not covered by the host build.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/app.rs
git commit -m "feat(editor): boot an exported site from its config file"
```

---

### Task 3: `export site` writes the config instead of patching the URL

The CLI stops rewriting `index.html` and writes `waml-boot.txt`. The sentinel contract between the artifact and the assembler goes away with it.

**Files:**
- Modify: `crates/waml-cli/src/site.rs` — module doc (lines 1–10), `BOOT_QUERY_SENTINEL` (lines 20–26), `SiteError` (lines 42–76), `assemble_site` (lines 101–118), tests (lines 215–287)

**Interfaces:**
- Consumes: `SiteSource::{Static, Api}`, `BUNDLE_FILE` (existing, unchanged).
- Produces: `pub(crate) const BOOT_CONFIG_FILE: &str = "waml-boot.txt";` in `site.rs`, matching `browser_boot::BOOT_CONFIG_FILE` by value. (The two crates do not share a dependency edge for this one string; the browser test in Task 5 is what proves they agree.)

- [ ] **Step 1: Write the failing tests**

In the `mod tests` block of `crates/waml-cli/src/site.rs`, replace `a_static_site_carries_the_editor_the_bundle_and_a_bundle_boot_url` (lines 219–233) and `an_api_site_has_no_bundle_and_boots_from_the_server` (lines 235–244) with:

```rust
    #[test]
    fn a_static_site_carries_the_editor_the_bundle_and_a_bundle_boot_config() {
        let artifact = artifact(vec![
            asset("index.html", &index_html()),
            asset("waml-editor.wasm", "wasm bytes"),
        ]);

        let site = assemble_site(&artifact, SiteSource::Static(b"bundle bytes".to_vec())).unwrap();

        assert_eq!(site["waml-editor.wasm"], b"wasm bytes");
        assert_eq!(site[BUNDLE_FILE], b"bundle bytes");
        assert_eq!(site[BOOT_CONFIG_FILE], b"?bundle=bundle.waml");
        // The address bar is the visitor's; the site says nothing in it.
        assert_eq!(
            String::from_utf8(site["index.html"].clone()).unwrap(),
            index_html()
        );
    }

    #[test]
    fn an_api_site_has_no_bundle_and_boots_from_the_server() {
        let artifact = artifact(vec![asset("index.html", &index_html())]);

        let site = assemble_site(&artifact, SiteSource::Api).unwrap();

        assert!(!site.contains_key(BUNDLE_FILE));
        assert_eq!(site[BOOT_CONFIG_FILE], b"?api=/api");
    }
```

Delete `an_unpatched_index_is_rejected` (lines 279–287) — an artifact no longer carries a placeholder to be patched.

Change the `index_html()` helper (lines 215–217) to:

```rust
    fn index_html() -> String {
        "<!doctype html><title>waml</title>".to_string()
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-cli site`
Expected: FAIL — `cannot find value 'BOOT_CONFIG_FILE' in this scope`.

- [ ] **Step 3: Write the implementation**

In `crates/waml-cli/src/site.rs`, replace the `BOOT_QUERY_SENTINEL` const and its doc (lines 20–26) with:

```rust
/// The file a site declares its boot source in, read by the editor at startup.
///
/// It holds a query string (`?bundle=…` or `?api=…`) because that is the
/// grammar the editor already parses for URLs. Writing it beside the editor,
/// rather than pushing it into the address bar, keeps a visitor's URL clean.
/// Must match `BOOT_CONFIG_FILE` in `crates/waml-editor/src/browser_boot.rs`.
pub(crate) const BOOT_CONFIG_FILE: &str = "waml-boot.txt";
```

Replace the body of `assemble_site` from line 101 through line 117 with:

```rust
    if !files.contains_key("index.html") {
        return Err(SiteError::MissingIndex);
    }
    let boot_query = match &source {
        SiteSource::Static(_) => format!("?bundle={BUNDLE_FILE}"),
        SiteSource::Api => "?api=/api".to_string(),
    };
    files.insert(BOOT_CONFIG_FILE.to_string(), boot_query.into_bytes());

    if let SiteSource::Static(bundle) = source {
        files.insert(BUNDLE_FILE.to_string(), bundle);
    }
```

Delete the `MissingBootSentinel` variant from `SiteError` (line 47) and its `Display` arm (lines 64–68).

Replace lines 3–7 of the module doc with:

```rust
//! A site is the built editor plus one boot source: either a `bundle.waml`
//! sitting next to it (`waml export site DIR`) or a live model server (the
//! future `waml serve`). The two differ by exactly two things -- whether a
//! bundle file is written, and what `waml-boot.txt` names -- so they share one
//! assembler rather than growing two writers that drift.
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml-cli site`
Expected: PASS. If `MissingBootSentinel` is still referenced anywhere the compiler will name the site; remove those references.

- [ ] **Step 5: Gate and commit**

```bash
cargo test --workspace
git add crates/waml-cli/src/site.rs
git commit -m "feat(cli): declare a site's boot source in waml-boot.txt"
```

---

### Task 4: Drop the URL rewrite from the artifact, and verify the config instead

The injected boot snippet is what actually put the query in the address bar. It goes, and `verify-web-artifact.mjs` moves its check from `index.html` to the config file.

**Files:**
- Modify: `scripts/inject-runtime-shell.mjs` — the "Boot URL" section (lines 457–479) and the injection/placeholder-count code (lines 481–494)
- Modify: `scripts/inject-runtime-shell.test.mjs` — the `runBoot`/`bootSource` helpers and the three boot tests (lines 461–512), plus the `BOOT_QUERY_SENTINEL` declaration wherever it sits
- Modify: `scripts/verify-web-artifact.mjs:148-177`
- Modify: `scripts/verify-web-artifact.test.mjs` — the boot-query cases

**Interfaces:**
- Consumes: `waml-boot.txt` as written by Task 3.
- Produces: an artifact whose `index.html` carries no boot snippet at all.

- [ ] **Step 1: Write the failing verifier test**

In `scripts/verify-web-artifact.test.mjs`, replace the existing boot-query cases (they build an `index.html` whose `data-waml-boot-url` snippet holds a query) with two cases built on the config file. Follow the fixture helpers already in that file; the shape is:

```js
test("a site whose boot config names a missing bundle is rejected", async (t) => {
    const dir = await artifactFixture(t, {
        "index.html": "<!doctype html><script src='./app.js'></script>",
        "app.js": "",
        "waml-boot.txt": "?bundle=bundle.waml",
    });

    const { code, output } = runVerify(dir);

    assert.equal(code, 1);
    assert.match(output, /bundle\.waml is not in the site/);
});

test("a site whose boot config is not a query string is rejected", async (t) => {
    const dir = await artifactFixture(t, {
        "index.html": "<!doctype html><script src='./app.js'></script>",
        "app.js": "",
        "waml-boot.txt": "<!doctype html><title>404</title>",
    });

    const { code, output } = runVerify(dir);

    assert.equal(code, 1);
    assert.match(output, /is not a query string/);
});
```

If `artifactFixture` / `runVerify` are named differently in that file, use the existing names — do not add new helpers.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `node --test scripts/verify-web-artifact.test.mjs`
Expected: FAIL — the verifier exits 0 because it still looks in `index.html`.

- [ ] **Step 3: Rewrite the verifier check**

Replace `scripts/verify-web-artifact.mjs:148-177` with:

```js
// The boot source, for the shape that has one: an exported site declares it in
// waml-boot.txt, and a query naming a bundle must name a file that is actually
// there. A site that boots `?bundle=bundle.waml` without the bundle opens the
// start screen -- green tick, empty editor, exactly the failure class above. A
// raw cargo-makepad artifact carries no config, which is not a fault.
const BOOT_CONFIG_FILE = "waml-boot.txt";
const bootConfigPath = join(artifactDir, BOOT_CONFIG_FILE);
if (existsSync(bootConfigPath)) {
  const query = readFileSync(bootConfigPath, "utf8").trim();
  if (!query.startsWith("?")) {
    console.error(
      `verify-web-artifact: ${BOOT_CONFIG_FILE} holds ${JSON.stringify(query)}, ` +
        `which is not a query string.`,
    );
    process.exit(1);
  }
  const bundle = query.match(/^\?bundle=(.+)$/);
  if (bundle && !existsSync(join(artifactDir, decodeURIComponent(bundle[1])))) {
    console.error(
      `verify-web-artifact: the site boots from ${query} but ${bundle[1]} is not in the site.`,
    );
    process.exit(1);
  }
  console.log(`verify-web-artifact: boots from ${query}`);
}
```

- [ ] **Step 4: Delete the boot snippet from the injector**

In `scripts/inject-runtime-shell.mjs`, delete the whole "Boot URL" section (lines 457–479: the comment block, `BOOT_QUERY_SENTINEL`, and `BOOT_JS`) and the placeholder-count check (lines 488–494). Change the injection at line 486 from

```js
html = html.replace(anchor, `${anchor}${LOADER_CSS}${BOOT_JS}${RUNTIME_JS}`);
```

to

```js
html = html.replace(anchor, `${anchor}${LOADER_CSS}${RUNTIME_JS}`);
```

Add, above the `const anchor = ...` line, the reason the snippet is gone:

```js
// No boot-URL snippet: the artifact says nothing about what to open. An
// exported site declares that in waml-boot.txt (crates/waml-cli/src/site.rs),
// which the editor reads only when the URL asks for nothing -- so a visitor
// never sees a query they did not type.
```

- [ ] **Step 5: Delete the injector's boot tests**

In `scripts/inject-runtime-shell.test.mjs`, delete `runBoot` (lines 461–472), the three boot tests (lines 474–512), the `bootSource` helper, and the `BOOT_QUERY_SENTINEL` declaration. Then add one test that the snippet is really gone:

```js
test("the generated index.html rewrites no url", async (t) => {
    const { artifactDir, html } = await injectFixture();
    t.after(() => rm(artifactDir, { recursive: true, force: true }));

    assert.equal(html.includes("data-waml-boot-url"), false);
    assert.equal(html.includes("replaceState"), false);
});
```

- [ ] **Step 6: Run the script tests**

Run: `node --test scripts/`
Expected: PASS. `node --check scripts/inject-runtime-shell.mjs` if a syntax error is suspected after the deletions.

- [ ] **Step 7: Commit**

```bash
git add scripts/inject-runtime-shell.mjs scripts/inject-runtime-shell.test.mjs scripts/verify-web-artifact.mjs scripts/verify-web-artifact.test.mjs
git commit -m "feat(web): stop rewriting the visitor's url at boot"
```

---

### Task 5: Prove it in a real browser

The two crates agree on `waml-boot.txt` only by literal. This end-to-end test is what holds them together, and it is also the only place the relative fetch URL (`./waml-boot.txt`) is exercised against a real makepad wasm build.

**Files:**
- Modify: `scripts/export-site-browser.test.mjs` — the boot assertions (lines 177–183) and the share-link case (lines 191–211), plus the header comment at lines 6–12 and 45

**Interfaces:**
- Consumes: everything from Tasks 1–4, built into a real site.

- [ ] **Step 1: Rewrite the boot assertions**

Replace lines 177–183 of `scripts/export-site-browser.test.mjs`:

```js
  // The injected boot snippet rewrites a bare URL before the wasm reads it.
  await page.waitForFunction(() => location.search === "?bundle=bundle.waml", null, {
```

...through the `host.requested.includes("/bundle.waml")` assertion, with:

```js
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
```

- [ ] **Step 2: Rewrite the share-link case**

Replace lines 200–211 (the `page.goto` with `?bundle=bundle.waml#${fragment}` and the assertion that the boot query survives) with:

```js
  // A share fragment is the more specific request, and it must win without the
  // site's config ever being consulted -- otherwise a shared edit would race
  // the site's own bundle.
  host.requested.length = 0;
  await page.goto(`${host.origin}/#${fragment}`, { waitUntil: "load" });
  await booted(page);
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
```

If `host.requested` is not a plain array, clear it the way that file's fixture allows; do not change its type.

- [ ] **Step 3: Update the header comment**

At `scripts/export-site-browser.test.mjs:45`, replace the line about the share fragment outranking the site's bundle so it names the config file rather than the query, and adjust lines 6–12 to say the site is booted from `waml-boot.txt` rather than from a rewritten URL. Keep the existing `file://`-origin rationale intact.

- [ ] **Step 4: Run the browser test**

Run: `node --test scripts/export-site-browser.test.mjs`
Expected: PASS. This test builds a real wasm artifact and a real site; it is slow. If it skips for a missing Playwright browser, install it as the file's header directs and rerun — a skip is not a pass for this task.

- [ ] **Step 5: Commit**

```bash
git add scripts/export-site-browser.test.mjs
git commit -m "test(web): prove an exported site boots with a clean url"
```

---

### Task 6: Say so in the README

**Files:**
- Modify: `README.md:40-57`

- [ ] **Step 1: Update the section**

In `README.md`, replace the sentence at line 51 (`Open the served page and the editor boots that bundle.`) with:

```markdown
Open the served page and the editor boots that bundle. The site names it in a
`waml-boot.txt` beside `index.html`, which the editor reads only when the URL
asks for nothing — so the address bar stays at `/` and a `?bundle=` or `#w1.`
link a visitor typed still wins.
```

- [ ] **Step 2: Full gate**

Run: `cargo test --workspace`
Run: `node --test scripts/`
Expected: PASS both.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: describe the site boot config"
```

---

## Verification after the plan

The Pages workflow (`.github/workflows/pages.yml`) needs no change: it already runs `verify-web-artifact.mjs` against `target/pages`, which now carries `waml-boot.txt`. The one manual check worth doing before the deploy is believed:

```bash
cargo build -p waml-cli --release --features embed-web   # with WAML_WEB_EMBED_DIR set
./target/release/waml export site docs/waml --out target/pages
node scripts/verify-web-artifact.mjs target/pages
python -m http.server --directory target/pages
```

Open `http://localhost:8000/`, confirm the model opens and the address bar still reads `/`.
