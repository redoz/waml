import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import test from "node:test";
import { mkdtemp, mkdir, rm, symlink, writeFile } from "node:fs/promises";
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

async function checkPrepared(t, files, prepare) {
  const repositoryRoot = await mkdtemp(join(tmpdir(), "waml-doc-contract-"));
  t.after(() => rm(repositoryRoot, { force: true, recursive: true }));
  for (const [path, text] of Object.entries(files)) {
    const absolute = join(repositoryRoot, path);
    await mkdir(dirname(absolute), { recursive: true });
    await writeFile(absolute, text);
  }
  await prepare(repositoryRoot);
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

test("accepts a marked VS Code test as native evidence", async () => {
  const vscode = canonical.replace(
    "crates/waml-editor/src/doc_tabs.rs::preview_replaces_preview",
    "editors/vscode/src/serverPath.test.ts::starts configured server",
  );
  const errors = await check({
    "docs/waml/goals/vscode.md": vscode,
    "editors/vscode/src/serverPath.test.ts":
      '// Scenario: TAB-001\nit("starts configured server", () => {});\n',
  });
  assert.deepEqual(errors, []);
});

test("does not count a marked browser script as native evidence", async () => {
  const browserOnly = canonical.replace(
    "crates/waml-editor/src/doc_tabs.rs::preview_replaces_preview",
    "scripts/export-site-browser.test.mjs::opens exported site",
  );
  const errors = await check({
    "docs/waml/goals/share.md": browserOnly,
    "scripts/export-site-browser.test.mjs":
      '// Scenario: TAB-001\ntest("opens exported site", () => {});\n',
  });
  assert.equal(errors.some((error) => error.includes("native test is absent")), true);
});

test("continues to count a marked crate test as native evidence", async () => {
  const crateTest = canonical.replace(
    "crates/waml-editor/src/doc_tabs.rs::preview_replaces_preview",
    "crates/waml-cli/tests/lsp_e2e.rs::publishes_diagnostics",
  );
  const errors = await check({
    "docs/waml/goals/lsp.md": crateTest,
    "crates/waml-cli/tests/lsp_e2e.rs":
      "// Scenario: TAB-001\n#[test]\nfn publishes_diagnostics() {}\n",
  });
  assert.deepEqual(errors, []);
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

test("sorts diagnostics by Unicode code points", async () => {
  const errors = await check({
    "docs/waml/goals/\u{10000}.md": "# Supplementary\n\n**Status:** implemented\n",
    "docs/waml/goals/\u{e000}.md": "# Private use\n\n**Status:** implemented\n",
  });

  assert.deepEqual(errors, [
    "docs/waml/goals/\u{e000}.md:3: invalid **Status:** value",
    "docs/waml/goals/\u{10000}.md:3: invalid **Status:** value",
  ]);
});

test("checks architecture view source ownership", async () => {
  const cases = [
    {
      name: "accepts direct sources",
      files: {
        "docs/waml/architecture/views/direct.md": "---\nsources:\n---\n# Direct\n",
      },
      expected: [],
    },
    {
      name: "accepts a sourced implementation concept",
      files: {
        "docs/waml/architecture/views/linked.md":
          "# Linked\n\n[Implementation](../concepts/implementation/linked.md)\n",
        "docs/waml/architecture/concepts/implementation/linked.md":
          "---\nsources:\n---\n# Linked implementation\n",
      },
      expected: [],
    },
    {
      name: "rejects a view without ownership evidence",
      files: {
        "docs/waml/architecture/views/missing.md": "# Missing\n",
      },
      expected: [
        "docs/waml/architecture/views/missing.md:1: view needs frontmatter sources or a link to ../concepts/implementation/",
      ],
    },
    {
      name: "rejects an unsourced implementation concept",
      files: {
        "docs/waml/architecture/views/linked.md":
          "# Linked\n\n[Implementation](../concepts/implementation/linked.md)\n",
        "docs/waml/architecture/concepts/implementation/linked.md": "---\ntype: Concept\n---\n# Linked\n",
      },
      expected: [
        "docs/waml/architecture/views/linked.md:3: linked implementation concept needs frontmatter sources",
      ],
    },
    {
      name: "rejects a concept link that escapes docs/waml",
      files: {
        "docs/waml/architecture/views/escape.md":
          "# Escape\n\n[Implementation](../concepts/implementation/../../../../outside.md)\n",
        "docs/outside.md": "---\nsources:\n---\n# Outside\n",
      },
      expected: [
        "docs/waml/architecture/views/escape.md:3: implementation concept link escapes docs/waml",
      ],
    },
  ];

  for (const fixture of cases) {
    assert.deepEqual(await check(fixture.files), fixture.expected, fixture.name);
  }
});

test("reports an architecture evidence link to a directory", async (t) => {
  const errors = await checkPrepared(
    t,
    {
      "docs/waml/architecture/views/directory.md":
        "# Directory\n\n[Implementation](../concepts/implementation/source)\n",
    },
    (repositoryRoot) =>
      mkdir(join(repositoryRoot, "docs/waml/architecture/concepts/implementation/source"), {
        recursive: true,
      }),
  );

  assert.deepEqual(errors, [
    "docs/waml/architecture/views/directory.md:3: implementation concept link is not a file",
  ]);
});

test("reports an evidence directory instead of reading it", async (t) => {
  const document = canonical.replace(
    "crates/waml-editor/src/doc_tabs.rs::preview_replaces_preview",
    "crates/waml-editor/src::preview_replaces_preview",
  );
  const errors = await checkPrepared(
    t,
    { "docs/waml/goals/tabs.md": document },
    (repositoryRoot) => mkdir(join(repositoryRoot, "crates/waml-editor/src"), { recursive: true }),
  );

  assert.equal(
    errors.includes("docs/waml/goals/tabs.md:13: evidence path is not a file: crates/waml-editor/src"),
    true,
  );
});

test("reports an evidence path escape on its document line", async () => {
  const document = canonical.replace(
    "crates/waml-editor/src/doc_tabs.rs::preview_replaces_preview",
    "../outside.rs::preview_replaces_preview",
  );
  const errors = await check({ "docs/waml/goals/tabs.md": document });

  assert.equal(
    errors.includes("docs/waml/goals/tabs.md:13: invalid evidence path ../outside.rs"),
    true,
  );
});

test("reports evidence that resolves through a symlink outside the repository", async (t) => {
  const outsideRoot = await mkdtemp(join(tmpdir(), "waml-doc-contract-outside-"));
  t.after(() => rm(outsideRoot, { force: true, recursive: true }));
  const outsideFile = join(outsideRoot, "outside.rs");
  await writeFile(outsideFile, "pub fn preview_replaces_preview() {}\n");

  let linkError;
  const errors = await checkPrepared(
    t,
    { "docs/waml/goals/tabs.md": canonical },
    async (repositoryRoot) => {
      const link = join(repositoryRoot, "crates/waml-editor/src/doc_tabs.rs");
      await mkdir(dirname(link), { recursive: true });
      try {
        await symlink(outsideFile, link, "file");
      } catch (error) {
        linkError = error;
      }
    },
  );
  if (linkError?.code === "EPERM" || linkError?.code === "EACCES") {
    t.skip(`symbolic links are unavailable: ${linkError.code}`);
    return;
  }
  if (linkError) throw linkError;

  assert.equal(
    errors.includes(
      "docs/waml/goals/tabs.md:13: evidence path escapes the repository: crates/waml-editor/src/doc_tabs.rs",
    ),
    true,
  );
});

test("rejects stale, orphan, and duplicate verification gaps", async () => {
  const sourceGap = `${canonical}\n## Verification gaps\n\n- TAB-001 — target: native; No native test asserts the result.\n`;
  const cases = [
    {
      name: "stale",
      files: {
        "docs/waml/goals/tabs.md": sourceGap,
        "crates/waml-editor/src/doc_tabs.rs":
          "// Scenario: TAB-001\n#[test]\nfn preview_replaces_preview() {}\n",
      },
      expected: "docs/waml/goals/tabs.md:17: Verification gaps item is stale for TAB-001",
    },
    {
      name: "orphan",
      files: {
        "docs/waml/goals/gaps.md":
          "# Gaps\n\n## Verification gaps\n\n- TAB-001 — target: native; No native test asserts the result.\n",
      },
      expected: "docs/waml/goals/gaps.md:5: Verification gaps item has no shipped scenario",
    },
    {
      name: "duplicate",
      files: {
        "docs/waml/goals/tabs.md": `${sourceGap}- TAB-001 — target: native; A second reason exists.\n`,
        "crates/waml-editor/src/doc_tabs.rs": "pub fn preview_replaces_preview() {}\n",
      },
      expected: "docs/waml/goals/tabs.md:18: duplicate Verification gaps item TAB-001",
    },
  ];

  for (const fixture of cases) {
    const errors = await check(fixture.files);
    assert.equal(errors.includes(fixture.expected), true, fixture.name);
  }
});

test("accepts browser allowlist evidence and an explicit parity seam", async () => {
  const browser = canonical
    .replaceAll("TAB-001", "WEB-001")
    .replace("**Applies to:** shared", "**Applies to:** browser")
    .replace(
      "crates/waml-editor/src/doc_tabs.rs::preview_replaces_preview",
      "scripts/export-site-browser.test.mjs::browser boot",
    );
  const browserErrors = await check({
    "docs/waml/goals/web.md": browser,
    "scripts/export-site-browser.test.mjs":
      '// Scenario: WEB-001\ntest("browser boot", () => {});\n',
  });
  assert.deepEqual(browserErrors, []);

  const parity = canonical.replace(
    "`crates/waml-editor/src/doc_tabs.rs::preview_replaces_preview`",
    "`crates/waml-editor/src/doc_tabs.rs::preview_replaces_preview` `scripts/export-site-browser.test.mjs::preview parity`",
  );
  const parityErrors = await check({
    "docs/waml/goals/tabs.md": parity,
    "crates/waml-editor/src/doc_tabs.rs":
      "// Scenario: TAB-001\n#[test]\nfn preview_replaces_preview() {}\n",
    "scripts/export-site-browser.test.mjs":
      '// Scenario: TAB-001\ntest("preview parity", () => {});\n',
  });
  assert.deepEqual(parityErrors, []);
});

test("accepts an existing evidence file line", async () => {
  const document = `${canonical.replace(
    "crates/waml-editor/src/doc_tabs.rs::preview_replaces_preview",
    "crates/waml-editor/src/doc_tabs.rs:1",
  )}\n## Verification gaps\n\n- TAB-001 — target: native; No native test asserts the result.\n`;
  const errors = await check({
    "docs/waml/goals/tabs.md": document,
    "crates/waml-editor/src/doc_tabs.rs": "one source line",
  });

  assert.deepEqual(errors, []);
});

test("reports an evidence line outside the file", async () => {
  const document = `${canonical.replace(
    "`crates/waml-editor/src/doc_tabs.rs::preview_replaces_preview`",
    "`crates/waml-editor/src/doc_tabs.rs:1` `crates/waml-editor/src/doc_tabs.rs:2`",
  )}\n## Verification gaps\n\n- TAB-001 — target: native; No native test asserts the result.\n`;
  const errors = await check({
    "docs/waml/goals/tabs.md": document,
    "crates/waml-editor/src/doc_tabs.rs": "one source line",
  });

  assert.deepEqual(errors, [
    "docs/waml/goals/tabs.md:13: evidence line is outside the file: crates/waml-editor/src/doc_tabs.rs:2",
  ]);
});
