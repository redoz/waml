import { readdir, readFile, realpath, stat } from "node:fs/promises";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const ID_SOURCE = String.raw`[A-Z][A-Z0-9]*(?:-[A-Z][A-Z0-9]*)*-[0-9]+`;
const SCENARIO = new RegExp(`^#### (${ID_SOURCE}) — ([a-z].+)$`);
const GAP = new RegExp(`^- (${ID_SOURCE}) — target: (native|browser); (.+[.!?])$`);
const STATUS = /^\*\*Status:\*\* (done|partial|planned|horizon)$/;
const EVIDENCE_REF = /`([^`]+)`/g;
const REF = /^(?<path>[A-Za-z0-9._/-]+)(?:::(?<symbol>[A-Za-z0-9_ .:'"-]+)|:(?<line>[1-9][0-9]*))$/;
const NON_SHIPPED_SECTIONS = new Set([
  "Planned behavior",
  "Unsupported behavior",
  "Discrepancies",
]);
const BROWSER_TEST_PATHS = [
  /^crates\/waml-cli\/tests\/serve_e2e\.rs$/,
  /^scripts\/export-site-browser\.test\.mjs$/,
  /^scripts\/serve-browser-check\.mjs$/,
  /^scripts\/.*browser.*\.test\.mjs$/,
];
const BROWSER_IMPLEMENTATION_PATHS = [
  /^crates\/waml-editor\/src\/(browser_boot|platform_browser|api_save)\.rs$/,
  /^crates\/waml-cli\/src\/site\.rs$/,
  /^crates\/waml-cli\/src\/serve\//,
  /^scripts\//,
];

function compareCodePoints(left, right) {
  const leftPoints = [...left];
  const rightPoints = [...right];
  const length = Math.min(leftPoints.length, rightPoints.length);
  for (let index = 0; index < length; index += 1) {
    const leftPoint = leftPoints[index].codePointAt(0);
    const rightPoint = rightPoints[index].codePointAt(0);
    if (leftPoint !== rightPoint) return leftPoint - rightPoint;
  }
  return leftPoints.length - rightPoints.length;
}

function normalizedPath(root, file) {
  return relative(root, file).split(sep).join("/");
}

function addError(errors, document, line, reason) {
  errors.push({ document, line, reason });
}

async function walkMarkdown(directory, repositoryRoot) {
  const files = [];

  async function walk(current) {
    const entries = await readdir(current, { withFileTypes: true });
    entries.sort((left, right) => compareCodePoints(left.name, right.name));
    for (const entry of entries) {
      const absolute = resolve(current, entry.name);
      if (entry.isDirectory()) {
        await walk(absolute);
      } else if (entry.isFile() && entry.name.endsWith(".md")) {
        files.push(absolute);
      }
    }
  }

  await walk(directory);
  return files.sort((left, right) =>
    compareCodePoints(normalizedPath(repositoryRoot, left), normalizedPath(repositoryRoot, right)),
  );
}

function parseScenarioBlock(lines, document, headingLine, errors) {
  let cursor = 0;
  const refs = [];

  function consume(expression, name) {
    const line = lines[cursor];
    if (line !== undefined && expression.test(line.text)) {
      cursor += 1;
      return line;
    }
    addError(errors, document, line?.number ?? headingLine, `expected ${name}`);
    return undefined;
  }

  const applies = consume(/^\*\*Applies to:\*\* (shared|native|browser)$/, "**Applies to:** shared, native, or browser");
  consume(/^\*\*Given\*\* .+$/, "a **Given** line");
  while (cursor < lines.length && /^\*\*And\*\* .+$/.test(lines[cursor].text)) {
    cursor += 1;
  }
  consume(/^\*\*When\*\* .+$/, "a **When** line");
  consume(/^\*\*Then\*\* .+$/, "a **Then** line");
  while (cursor < lines.length && /^\*\*And\*\* .+$/.test(lines[cursor].text)) {
    cursor += 1;
  }
  const evidence = consume(/^\*\*Evidence:\*\* .+$/, "an **Evidence:** line");

  if (cursor < lines.length) {
    addError(errors, document, lines[cursor].number, "scenario has trailing nonblank text");
  }

  if (evidence) {
    EVIDENCE_REF.lastIndex = 0;
    for (const match of evidence.text.matchAll(EVIDENCE_REF)) {
      const reference = match[1];
      const parsed = REF.exec(reference);
      if (!parsed) {
        addError(errors, document, evidence.number, `invalid evidence reference ${reference}`);
        continue;
      }
      refs.push({
        path: parsed.groups.path,
        symbol: parsed.groups.symbol,
        line: parsed.groups.line ? Number(parsed.groups.line) : undefined,
        document,
        documentLine: evidence.number,
      });
    }
    if (refs.length === 0) {
      addError(errors, document, evidence.number, "**Evidence:** needs at least one backtick reference");
    }
  }

  return {
    applicability: applies?.text.slice("**Applies to:** ".length),
    refs,
  };
}

function frontmatterLines(text) {
  const lines = text.split(/\r?\n/);
  const start = lines.findIndex((line) => line === "---");
  if (start < 0) return [];
  const end = lines.findIndex((line, index) => index > start && line === "---");
  return end < 0 ? [] : lines.slice(start + 1, end);
}

function hasSourcesFrontmatter(text) {
  return frontmatterLines(text).some((line) => /^sources:\s*$/.test(line));
}

// A diagram document groups its members with headings — a use-case view's
// `#### ` heading names a BAND (uml::use_case::classify_band), not a
// scenario. Scenarios live in the goal documents; only those are scanned for
// `#### <ID> — <description>`.
function isDiagramDocument(text) {
  return frontmatterLines(text).some((line) => /^type:\s*uml\.\w*Diagram\s*$/.test(line));
}

async function inspectArchitectureViews(documents, docsRoot, repositoryRoot, errors) {
  const viewsPrefix = "docs/waml/architecture/views/";
  for (const document of documents) {
    const documentPath = normalizedPath(repositoryRoot, document);
    if (!documentPath.startsWith(viewsPrefix)) continue;

    const text = await readFile(document, "utf8");
    if (hasSourcesFrontmatter(text)) continue;

    const lines = text.split(/\r?\n/);
    const link = lines
      .map((line, index) => ({ line, number: index + 1 }))
      .find(({ line }) => /\]\(\.\.\/concepts\/implementation\/[^)\s]+\)/.test(line));
    if (!link) {
      addError(errors, documentPath, 1, "view needs frontmatter sources or a link to ../concepts/implementation/");
      continue;
    }

    const targetMatch = /\]\((\.\.\/concepts\/implementation\/[^)\s]+)\)/.exec(link.line);
    const target = resolve(dirname(document), targetMatch[1]);
    let realDocsRoot;
    let realTarget;
    try {
      realDocsRoot = await realpath(docsRoot);
      realTarget = await realpath(target);
    } catch {
      addError(errors, documentPath, link.number, "implementation concept link does not resolve");
      continue;
    }
    const contained = relative(realDocsRoot, realTarget);
    if (
      contained === "" ||
      isAbsolute(contained) ||
      contained === ".." ||
      contained.startsWith(`..${sep}`)
    ) {
      addError(errors, documentPath, link.number, "implementation concept link escapes docs/waml");
      continue;
    }
    let targetInfo;
    try {
      targetInfo = await stat(realTarget);
    } catch {
      addError(errors, documentPath, link.number, "implementation concept link cannot be inspected");
      continue;
    }
    if (!targetInfo.isFile()) {
      addError(errors, documentPath, link.number, "implementation concept link is not a file");
      continue;
    }
    let concept;
    try {
      concept = await readFile(realTarget, "utf8");
    } catch {
      addError(errors, documentPath, link.number, "linked implementation concept cannot be read");
      continue;
    }
    if (!hasSourcesFrontmatter(concept)) {
      addError(errors, documentPath, link.number, "linked implementation concept needs frontmatter sources");
    }
  }
}

function isBrowserTest(path) {
  return BROWSER_TEST_PATHS.some((expression) => expression.test(path));
}

function isBrowserImplementation(path) {
  return BROWSER_IMPLEMENTATION_PATHS.some((expression) => expression.test(path));
}

function invalidReferencePath(path) {
  return (
    isAbsolute(path) ||
    path.includes("\\") ||
    path.split("/").some((segment) => segment === "" || segment === "." || segment === "..")
  );
}

async function inspectEvidence(scenario, repositoryRoot, target) {
  const errors = [];
  let hasSource = false;
  let hasEvidence = false;
  let hasTargetTest = false;
  const realRoot = await realpath(repositoryRoot);

  for (const reference of scenario.refs) {
    if (invalidReferencePath(reference.path)) {
      addError(errors, scenario.document, reference.documentLine, `invalid evidence path ${reference.path}`);
      continue;
    }

    const absolute = resolve(repositoryRoot, reference.path);
    let realFile;
    let fileInfo;
    try {
      realFile = await realpath(absolute);
      fileInfo = await stat(realFile);
    } catch {
      addError(errors, scenario.document, reference.documentLine, `evidence file does not exist: ${reference.path}`);
      continue;
    }
    const contained = relative(realRoot, realFile);
    if (
      contained === "" ||
      isAbsolute(contained) ||
      contained === ".." ||
      contained.startsWith(`..${sep}`)
    ) {
      addError(errors, scenario.document, reference.documentLine, `evidence path escapes the repository: ${reference.path}`);
      continue;
    }
    if (!fileInfo.isFile()) {
      addError(errors, scenario.document, reference.documentLine, `evidence path is not a file: ${reference.path}`);
      continue;
    }

    let lines;
    try {
      lines = (await readFile(realFile, "utf8")).split(/\r?\n/);
    } catch {
      addError(errors, scenario.document, reference.documentLine, `evidence file cannot be read: ${reference.path}`);
      continue;
    }
    if (reference.line !== undefined) {
      if (reference.line > lines.length) {
        addError(errors, scenario.document, reference.documentLine, `evidence line is outside the file: ${reference.path}:${reference.line}`);
      } else {
        hasSource = true;
      }
      continue;
    }

    const symbolLine = lines.findIndex((line) => line.includes(reference.symbol));
    if (symbolLine < 0) {
      addError(errors, scenario.document, reference.documentLine, `evidence symbol is absent: ${reference.path}::${reference.symbol}`);
      continue;
    }
    const window = lines.slice(Math.max(0, symbolLine - 12), symbolLine + 1);
    const testCandidate =
      reference.path.includes("/tests/") ||
      reference.path.includes(".test.") ||
      isBrowserTest(reference.path) ||
      window.some((line) => line.includes("#[test]") || line.includes("test(") || line.includes("it("));
    if (!testCandidate) {
      hasSource = true;
      continue;
    }

    const marker = `Scenario: ${scenario.id}`;
    if (!window.some((line) => line.trim() === `// ${marker}` || line.trim() === `# ${marker}`)) {
      addError(errors, scenario.document, reference.documentLine, `cited test needs // ${marker} or # ${marker}`);
      continue;
    }
    hasEvidence = true;
    const nativeTest =
      (reference.path.startsWith("crates/") || reference.path.startsWith("editors/vscode/")) &&
      !isBrowserTest(reference.path) &&
      !isBrowserImplementation(reference.path);
    const browserTest = isBrowserTest(reference.path);
    if ((target === "native" && nativeTest) || (target === "browser" && browserTest)) {
      hasTargetTest = true;
    }
  }

  if (!hasSource && !hasEvidence) {
    addError(errors, scenario.document, scenario.line, "evidence has no source or valid marked test");
  }
  return { errors, hasSource: hasSource || hasEvidence, hasTargetTest };
}

function sortErrors(errors) {
  return errors.sort((left, right) => {
    const document = compareCodePoints(left.document, right.document);
    if (document !== 0) return document;
    if (left.line !== right.line) return left.line - right.line;
    return compareCodePoints(left.reason, right.reason);
  });
}

export async function checkDocsContract(docsRoot, repositoryRoot) {
  const errors = [];
  const scenarios = new Map();
  const gaps = new Map();
  const sourceEvidence = new Map();
  const documents = await walkMarkdown(docsRoot, repositoryRoot);

  for (const document of documents) {
    const documentPath = normalizedPath(repositoryRoot, document);
    const source = await readFile(document, "utf8");
    const lines = source.split(/\r?\n/);
    const scenariosAreScanned = !isDiagramDocument(source);
    let section = "";
    for (let index = 0; index < lines.length; index += 1) {
      const text = lines[index];
      const number = index + 1;
      if (text.startsWith("## ")) section = text.slice(3);
      if (text.startsWith("**Status:**") && !STATUS.test(text)) {
        addError(errors, documentPath, number, "invalid **Status:** value");
      }
      if (section === "Verification gaps" && text.startsWith("- ")) {
        const gap = GAP.exec(text);
        if (!gap) {
          addError(errors, documentPath, number, "invalid Verification gaps item");
        } else if (gaps.has(gap[1])) {
          addError(errors, documentPath, number, `duplicate Verification gaps item ${gap[1]}`);
        } else {
          gaps.set(gap[1], { document: documentPath, line: number, target: gap[2], reason: gap[3] });
        }
      }
      if (!scenariosAreScanned || !text.startsWith("#### ")) continue;

      const heading = SCENARIO.exec(text);
      if (!heading) {
        addError(errors, documentPath, number, "invalid scenario heading");
        continue;
      }
      if (NON_SHIPPED_SECTIONS.has(section)) {
        addError(errors, documentPath, number, `scenario is in ${section.toLowerCase()}`);
      }
      const block = [];
      for (let cursor = index + 1; cursor < lines.length && !/^#{1,4} /.test(lines[cursor]); cursor += 1) {
        if (lines[cursor].trim() !== "") block.push({ text: lines[cursor], number: cursor + 1 });
      }
      const parsed = parseScenarioBlock(block, documentPath, number, errors);
      if (scenarios.has(heading[1])) {
        const previous = scenarios.get(heading[1]);
        addError(errors, documentPath, number, `duplicate scenario ${heading[1]} also occurs in ${previous.document}`);
      } else {
        scenarios.set(heading[1], {
          id: heading[1],
          document: documentPath,
          line: number,
          applicability: parsed.applicability,
          refs: parsed.refs,
          section,
        });
      }
    }
  }

  await inspectArchitectureViews(documents, docsRoot, repositoryRoot, errors);

  for (const scenario of [...scenarios.values()].sort((left, right) => compareCodePoints(left.id, right.id))) {
    const target = scenario.applicability === "browser" ? "browser" : "native";
    const result = await inspectEvidence(scenario, repositoryRoot, target);
    errors.push(...result.errors);
    sourceEvidence.set(scenario.id, result.hasSource);
    const gap = gaps.get(scenario.id);
    if (result.hasTargetTest) {
      if (gap) addError(errors, gap.document, gap.line, `Verification gaps item is stale for ${scenario.id}`);
    } else if (!gap || gap.document !== scenario.document || gap.target !== target) {
      const reason =
        target === "native"
          ? "native test is absent; add an item under Verification gaps"
          : "browser-specific test is absent; add an item under Verification gaps";
      addError(errors, scenario.document, scenario.line, reason);
    }
  }

  for (const [id, gap] of gaps) {
    if (!sourceEvidence.get(id)) {
      addError(errors, gap.document, gap.line, "Verification gaps item has no shipped scenario");
    }
  }

  return sortErrors(errors).map(({ document, line, reason }) => `${document}:${line}: ${reason}`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1])) {
  const docsArgument = process.argv[2];
  if (!docsArgument) {
    console.error("Usage: node scripts/check-waml-doc-contract.mjs docs/waml");
    process.exitCode = 1;
  } else {
    const errors = await checkDocsContract(resolve(docsArgument), process.cwd());
    for (const error of errors) console.log(error);
    process.exitCode = errors.length === 0 ? 0 : 1;
  }
}
