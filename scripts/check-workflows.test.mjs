// GitHub Actions rejects a workflow file it cannot parse, and a workflow that
// fails to parse fails *before* any job runs — including this repo's whole CI
// gate, which pages.yml calls into. That makes a malformed workflow the one
// class of breakage no in-CI check can catch after the fact, so it is checked
// here, where `node --test "scripts/*.test.mjs"` runs it on every pull request.
//
// This exists because a scripted edit once wrote a literal \x01 into pages.yml
// (a shell backslash escape that got collapsed), and the deploy died with
// "This run likely failed because of a workflow file issue" and no log at all.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const DIR = ".github/workflows";
const files = readdirSync(DIR).filter((f) => f.endsWith(".yml") || f.endsWith(".yaml"));

test("there are workflows to check", () => {
  assert.ok(files.length > 0, `no workflow files found in ${DIR}`);
});

for (const file of files) {
  const path = join(DIR, file);

  test(`${file} contains no control characters`, () => {
    const text = readFileSync(path, "utf8");
    // Tab, newline and carriage return are the only control characters YAML
    // allows; anything else is a corrupted edit.
    const bad = [...text].findIndex(
      (ch) => ch < " " && ch !== "\t" && ch !== "\n" && ch !== "\r",
    );
    assert.equal(
      bad,
      -1,
      bad === -1
        ? ""
        : `control character U+${text.codePointAt(bad).toString(16).padStart(4, "0")} at offset ${bad}`,
    );
  });

  test(`${file} uses spaces, not tabs, for indentation`, () => {
    const lines = readFileSync(path, "utf8").split("\n");
    const tabbed = lines.findIndex((line) => /^\s*\t/.test(line));
    assert.equal(tabbed, -1, `tab indentation on line ${tabbed + 1}; YAML forbids it`);
  });
}
