# Export a Bundle

**Goal:** A reader takes the content out again, as a bundle file, as a static
site, or as files on disk.

**Why:** A format that a user cannot leave is a risk. Export makes the decision
to use WAML a small risk. A static site reaches a reader who installs no
software.

**Done when:** The browser writes its current model as a bundle file. The
export command writes a complete site that opens its embedded bundle. A saved
browser model becomes the next URL-fragment boot source.

**Status:** done
**MVP:** no

## Shipped behavior

#### BROWSER-001 — a share fragment has boot-source priority

**Applies to:** browser

**Given** a browser URL supplies a share fragment, an API query, and a bundle query
**And** the site supplies a boot configuration
**When** the browser selects its start source
**Then** it uses the share fragment before all other sources

**Evidence:** `crates/waml-editor/src/browser_boot.rs:48`

#### BROWSER-003 — a bundle query opens its bundle

**Applies to:** browser

**Given** a browser URL has a bundle query parameter
**When** the reader opens the URL
**Then** the browser fetches, decodes, and opens the named bundle

**Evidence:** `crates/waml-editor/src/browser_boot.rs:48` and `crates/waml-editor/src/browser_boot.rs:122`

#### BROWSER-004 — a static site boots without changing the visitor URL

**Applies to:** browser

**Given** a static site contains a bundle and its boot configuration
**When** the reader opens the site
**Then** the browser opens the configured bundle without adding a query to the visitor URL

**Evidence:** `scripts/export-site-browser.test.mjs::an exported site boots and exports its model back`

#### BROWSER-006 — download the current browser model as a bundle

**Applies to:** browser

**Given** the browser editor has an open model
**When** the author exports the model as a bundle
**Then** the browser downloads a WAML bundle with the current model content

**Evidence:** `scripts/export-site-browser.test.mjs::an exported site boots and exports its model back`

#### BROWSER-011 — export a directory as a self-contained static site

**Applies to:** browser

**Given** an author has a directory of WAML documents
**When** the author exports the directory as a site
**Then** the output contains the web artifact, bundle, and boot configuration and opens the bundle in a browser

**Evidence:** `scripts/export-site-browser.test.mjs::an exported site boots and exports its model back`

#### BROWSER-016 — a browser save replaces the share fragment

**Applies to:** browser

**Given** the browser editor is not saving through the served API
**And** its URL can contain a bundle query
**When** the author saves a changed model
**Then** the URL fragment contains the current model and the bundle query remains

**Evidence:** `crates/waml-editor/src/app/workspace.rs:101` and `crates/waml-editor/src/app/workspace.rs:301`

## Verification gaps

- BROWSER-001 — target: browser; The browser E2E proves that a share fragment overrides site boot configuration, but it does not provide competing API and bundle query sources or assert the full boot priority.
- BROWSER-003 — target: browser; The host test selects the URL but no browser test asserts that this query boot fetches and opens the bundle.
- BROWSER-016 — target: browser; The host test proves the fragment shape and precedence, but no headed browser test drives an editor save and observes browser_update_url.

## Notes

- These scenarios own browser boot, download, static-site, and URL seams. They
  do not copy shared bundle-codec or editor-model contracts.
- The site command uses the web artifact from the same product build.
- Image export does not exist and is outside these frozen behavior rows.
