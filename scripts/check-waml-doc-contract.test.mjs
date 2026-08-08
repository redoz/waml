import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import test from "node:test";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { checkDocsContract } from "./check-waml-doc-contract.mjs";

const execFileAsync = promisify(execFile);
const checkerPath = fileURLToPath(new URL("./check-waml-doc-contract.mjs", import.meta.url));

async function check(files) {
  const repositoryRoot = await mkdtemp(join(tmpdir(), "waml-doc-contract-"));
  for (const [path, text] of Object.entries(files)) {
    const absolute = join(repositoryRoot, path);
    await mkdir(dirname(absolute), { recursive: true });
    await writeFile(absolute, text);
  }
  return checkDocsContract(join(repositoryRoot, "docs/waml"), repositoryRoot);
}

const canonical = `# Tabs

**Status:** done

#### TAB-001 — a new preview replaces the old preview

**Applies to:** shared

**Given** one document is open in the preview tab
**When** the reader selects a different document
**Then** the editor replaces the old preview

**Evidence:** \`crates/waml-editor/src/doc_tabs.rs::preview_replaces_preview\`
`;

test("accepts the canonical shipped scenario form", async () => {
  const errors = await check({
    "docs/waml/goals/tabs.md": canonical,
    "crates/waml-editor/src/doc_tabs.rs":
      "// Scenario: TAB-001\n#[test]\nfn preview_replaces_preview() {}\n",
  });
  assert.deepEqual(errors, []);
});

test("accepts stable multi-segment identifiers verbatim", async () => {
  for (const id of ["SEQ-MSG-1", "SEQ-ORD-1", "SEQ-FRAG-10"]) {
    const legacy = canonical.replaceAll("TAB-001", id);
    const errors = await check({
      "docs/waml/goals/sequence.md": legacy,
      "crates/waml-editor/src/doc_tabs.rs":
        `// Scenario: ${id}\n#[test]\nfn preview_replaces_preview() {}\n`,
    });
    assert.deepEqual(errors, [], id);
  }
});

test("accepts shipped source evidence with an explicit native verification gap", async () => {
  const sourceOnly = `${canonical}\n## Verification gaps\n\n- TAB-001 — target: native; No native test asserts the tab replacement result.\n`;
  const errors = await check({
    "docs/waml/goals/tabs.md": sourceOnly,
    "crates/waml-editor/src/doc_tabs.rs":
      "pub fn preview_replaces_preview() {}\n",
  });
  assert.deepEqual(errors, []);
});

test("rejects source-only shipped evidence without a verification gap", async () => {
  const errors = await check({
    "docs/waml/goals/tabs.md": canonical,
    "crates/waml-editor/src/doc_tabs.rs":
      "pub fn preview_replaces_preview() {}\n",
  });
  assert.equal(errors.some((error) => error.includes("Verification gaps")), true);
});

test("accepts browser source evidence with an explicit browser verification gap", async () => {
  const browser = canonical
    .replaceAll("TAB-001", "WEB-001")
    .replace("**Applies to:** shared", "**Applies to:** browser")
    .replace("crates/waml-editor/src/doc_tabs.rs", "crates/waml-editor/src/browser_boot.rs");
  const errors = await check({
    "docs/waml/goals/web.md": `${browser}\n## Verification gaps\n\n- WEB-001 — target: browser; No browser-specific test asserts the boot result.\n`,
    "crates/waml-editor/src/browser_boot.rs":
      "pub fn preview_replaces_preview() {}\n",
  });
  assert.deepEqual(errors, []);
});

test("rejects a verification gap with the wrong target", async () => {
  const withWrongGap = `${canonical}\n## Verification gaps\n\n- TAB-001 — target: browser; No native test asserts the tab replacement result.\n`;
  const errors = await check({
    "docs/waml/goals/tabs.md": withWrongGap,
    "crates/waml-editor/src/doc_tabs.rs":
      "pub fn preview_replaces_preview() {}\n",
  });
  assert.equal(errors.some((error) => error.includes("native test is absent")), true);
});

test("rejects duplicate scenario identifiers with both paths", async () => {
  const errors = await check({
    "docs/waml/goals/a.md": canonical,
    "docs/waml/goals/b.md": canonical,
    "crates/waml-editor/src/doc_tabs.rs": "// Scenario: TAB-001\n",
  });
  assert.equal(errors.some((error) => error.includes("a.md") && error.includes("b.md")), true);
});

test("rejects shipped scenarios without applicability or evidence", async () => {
  const errors = await check({
    "docs/waml/goals/tabs.md": canonical
      .replace("**Applies to:** shared\n\n", "")
      .replace(/\n\*\*Evidence:\*\*.*\n/, "\n"),
  });
  assert.equal(errors.some((error) => error.includes("Applies to")), true);
  assert.equal(errors.some((error) => error.includes("Evidence")), true);
});

test("rejects implemented and unverified goal status text", async () => {
  const errors = await check({
    "docs/waml/goals/a.md": "# A\n\n**Status:** implemented\n",
    "docs/waml/goals/b.md": "# B\n\n**Status:** partial — unverified\n",
  });
  assert.equal(errors.length, 2);
});

test("rejects a shared scenario whose evidence is browser-only", async () => {
  const errors = await check({
    "docs/waml/goals/tabs.md": canonical.replace(
      "crates/waml-editor/src/doc_tabs.rs",
      "crates/waml-editor/src/browser_boot.rs",
    ),
    "crates/waml-editor/src/browser_boot.rs":
      "// Scenario: TAB-001\n#[test]\nfn preview_replaces_preview() {}\n",
  });
  assert.equal(errors.some((error) => error.includes("native")), true);
});

test("rejects a browser scenario whose evidence is not browser-specific or a parity seam", async () => {
  const errors = await check({
    "docs/waml/goals/web.md": canonical
      .replace("TAB-001", "WEB-001")
      .replace("**Applies to:** shared", "**Applies to:** browser"),
    "crates/waml-editor/src/doc_tabs.rs":
      "// Scenario: WEB-001\n#[test]\nfn preview_replaces_preview() {}\n",
  });
  assert.equal(errors.some((error) => error.includes("browser-specific")), true);
});

test("rejects a scenario identifier absent from its cited test file", async () => {
  const errors = await check({
    "docs/waml/goals/tabs.md": canonical,
    "crates/waml-editor/src/doc_tabs.rs": "#[test]\nfn preview_replaces_preview() {}\n",
  });
  assert.equal(errors.some((error) => error.includes("Scenario: TAB-001")), true);
});

test("rejects Given-When-Then text under planned or horizon behavior", async () => {
  const errors = await check({
    "docs/waml/goals/tabs.md": canonical.replace("# Tabs", "# Tabs\n\n## Planned behavior"),
    "crates/waml-editor/src/doc_tabs.rs": "// Scenario: TAB-001\n",
  });
  assert.equal(errors.some((error) => error.includes("planned")), true);
});

test("prints deterministic diagnostics and exits one from the CLI", async () => {
  const repositoryRoot = await mkdtemp(join(tmpdir(), "waml-doc-contract-cli-"));
  const document = join(repositoryRoot, "docs/waml/goals/a.md");
  await mkdir(dirname(document), { recursive: true });
  await writeFile(document, "# A\n\n**Status:** implemented\n");

  await assert.rejects(
    execFileAsync(process.execPath, [checkerPath, "docs/waml"], {
      cwd: repositoryRoot,
    }),
    (error) => {
      assert.equal(error.code, 1);
      assert.match(error.stdout, /^docs\/waml\/goals\/a\.md:3: /m);
      return true;
    },
  );
});
