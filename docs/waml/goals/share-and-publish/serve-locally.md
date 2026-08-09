# Serve Locally

**Goal:** One command serves the editor on the loopback interface against a
local directory. A reader opens a bundle in a browser with no build step and
with no published site.

**Why:** This command uses the same web artifact and views as the published
form, and it reads and writes the files of the author.

**Done when:** The command serves the embedded web editor on the loopback
interface. It gives an operations interface for a selected directory. Browser
edits write to disk through that interface. A wider bind is opt-in and warns.

**Status:** done
**MVP:** no

## Shipped behavior

#### BROWSER-002 — the printed serve URL starts the browser artifact

**Applies to:** browser

**Given** an author serves a local directory
**When** the reader opens the printed API URL with its fragment token
**Then** the browser artifact starts from the served API without a console panic

**Evidence:** `crates/waml-editor/src/browser_boot.rs:48`, `crates/waml-editor/src/browser_boot.rs:165`, and `crates/waml-cli/src/serve/mod.rs:30`

#### BROWSER-007 — a same-origin served editor reads the authenticated model

**Applies to:** browser

**Given** the served editor and model API have the same origin
**And** the editor has the printed token
**When** the browser reads the model API
**Then** the API returns the authenticated model

**Evidence:** `crates/waml-cli/src/serve/routes.rs:151` and `crates/waml-cli/src/serve/guard.rs:100`

#### BROWSER-008 — a foreign origin cannot use the authenticated API

**Applies to:** browser

**Given** an API request has a valid token and a foreign origin
**When** the browser sends the request to the served API
**Then** the server rejects the request with status 403

**Evidence:** `crates/waml-cli/src/serve/guard.rs:100`

#### BROWSER-009 — a browser document save uses the baseline guard

**Applies to:** browser

**Given** the browser editor has the baseline for an open document
**When** it posts a document change to the served API
**Then** the API writes the changed document only when the baseline matches

**Evidence:** `crates/waml-cli/src/serve/routes.rs:230`

#### BROWSER-010 — a conflicting browser save reports the current revision

**Applies to:** browser

**Given** the served directory has a newer revision than the browser baseline
**When** the browser saves its document change
**Then** the conflict result reports the current served revision

**Evidence:** `crates/waml-editor/src/api_save.rs:81` and `crates/waml-cli/src/serve/routes.rs:253`

#### BROWSER-017 — the diagnostics API returns diagnostics and its revision

**Applies to:** browser

**Given** a served model has diagnostics and a current revision
**When** the browser reads `/api/diagnostics`
**Then** the response contains the diagnostics and the current revision

**Evidence:** `crates/waml-cli/src/serve/routes.rs:167`

#### BROWSER-018 — an operations batch is atomic and reports changed files

**Applies to:** browser

**Given** the browser has the current served revision and an operations batch
**When** it posts the batch to `/api/ops`
**Then** the API applies the complete batch atomically and returns the changed files

**Evidence:** `crates/waml-cli/src/serve/routes.rs:195` and `crates/waml-cli/src/serve/state.rs:78`

#### BROWSER-019 — a request without a valid token is refused before body validation

**Applies to:** browser

**Given** a served API request has no valid bearer or query token
**And** its body can be malformed
**When** the browser sends the request
**Then** the API returns status 401 before it validates the body

**Evidence:** `crates/waml-cli/src/serve/guard.rs:100` and `crates/waml-cli/src/serve/routes.rs:86`

#### BROWSER-020 — a mutating request requires the WAML client header

**Applies to:** browser

**Given** an authenticated mutating API request omits `X-Waml-Client: 1`
**When** the browser sends the request
**Then** the API rejects the request with status 403

**Evidence:** `crates/waml-cli/src/serve/guard.rs:100` and `crates/waml-cli/src/serve/routes.rs:86`

## Verification gaps

- BROWSER-002 — target: browser; The headed serve check runs as top-level script code, so no addressable browser test call asserts startup from the printed API URL and fragment token without a console panic.
- BROWSER-007 — target: browser; The headed serve check performs the authenticated same-origin read as top-level script code, but no addressable browser test call asserts the HTTP 200 result.
- BROWSER-008 — target: browser; The exact host route test proves the server 403 result, but no browser test distinguishes server rejection from browser CORS enforcement.
- BROWSER-009 — target: browser; The headed serve check performs the baseline-guarded document write as top-level script code, but no addressable browser test call asserts the API result and changed disk bytes.
- BROWSER-010 — target: browser; Host tests cover the 409 wire contract, but no browser test causes a conflicting save and observes the browser result.
- BROWSER-017 — target: browser; The route test proves the HTTP response, but no headed browser test reads or presents served diagnostics.
- BROWSER-018 — target: browser; Host route tests prove success and rejection, but no headed browser test performs an operations API batch or observes its result.
- BROWSER-019 — target: browser; Host route tests prove 401 responses, but no headed browser test observes the missing-token refusal.
- BROWSER-020 — target: browser; The host route test proves the 403 guard, but no headed browser test observes the anti-CSRF client-header refusal.

## Notes

- `waml serve <dir>` binds to `127.0.0.1` by default. A wider bind is opt-in.
- The token stays in the URL fragment. The browser sends it in the request; it
  is not part of the normal request URL.
- The model, diagnostics, document-write, and operations routes are separate
  public workflows. Their scenarios do not stand in for browser verification.
