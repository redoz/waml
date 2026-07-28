# Task 20 Report: Independent VS Code Stdio Client

## Scope

- Modified only `packages/vscode/src/serverPath.test.ts`.
- Verified `packages/vscode/src/extension.ts` and `packages/vscode/src/serverPath.ts`
  without retaining production changes.
- Did not perform Task 21 retirement work.
- Preserved the pre-existing modification to `task-7-report.md`.

## Contract Locked

- Default executable resolution remains the bare `waml` command.
- Explicit `waml.serverPath` resolution remains authoritative after the
  `WAML_SERVER_PATH` development override.
- The extension passes the resolved command, `args: ["lsp"]`, and
  `TransportKind.stdio` to `vscode-languageclient`. The installed language
  client appends `--stdio`, yielding `waml lsp --stdio`.
- Activation creates and starts one language client; deactivation delegates
  shutdown to that client.
- Production imports contain no retired `@waml/*`, parser, syntax, or WASM
  domain dependency.

## RED / GREEN Evidence

- RED harness correction: the first run failed because the AST lookup only
  searched top-level declarations. The helper was corrected to traverse the
  activation function.
- RED mutation check: temporarily changed `args` from `["lsp"]` to
  `["serve"]`; the new test failed with
  `expected '["serve"]' to be '["lsp"]'`.
- GREEN: restored the unchanged retained implementation; all 9 extension tests
  passed.

## Verification

- `rtk pnpm --filter @waml/vscode test`: PASS, 9/9.
- `rtk pnpm --filter @waml/vscode build`: PASS.
- Exact prohibited production-import scan from the brief: PASS, no matches.
- `rtk cargo test -p waml-cli --test lsp_e2e`: PASS, 3/3.
- `rtk cargo fmt --all -- --check`: PASS.
- `rtk proxy pnpm --filter @waml/vscode exec npm pack --dry-run`: PASS; 11
  deterministic package entries reported.
- `rtk git diff --check`: PASS.
- `rtk cargo test --workspace --all-features`: FAIL in the pre-existing
  `waml::okf::tests::package_node_and_model_path` assertion
  `non-reserved projection produces one concept`. The focused rerun reproduces
  the same Rust failure. Task 20 changes only TypeScript tests and do not touch
  this code path.

## TokenSave

TokenSave returned two relevant contexts and saved approximately 2,610 tokens
in total (`2,054 + 556`).

## Checklist

- [x] Default/configured executable behavior covered.
- [x] Exact stdio launch options covered.
- [x] Client start/shutdown lifecycle covered.
- [x] Parser/model/WASM/retired-domain isolation covered.
- [x] `extension.ts` unchanged.
- [x] `serverPath.ts` unchanged.
- [x] No Task 21 work.
- [x] Focused VS Code, LSP e2e, format, packaging, import, and diff gates run.
- [ ] Full workspace gate green; blocked by the reproduced unrelated Rust OKF
      failure noted above.

## Formal Fix Round 1

### Review Findings Addressed

- Replaced the launch/start/stop AST and regex characterizations with tests
  that execute the exported `activate` and `deactivate` functions.
- Kept AST inspection only for the explicit prohibited-import architecture
  guard required by the brief.
- Pinned `vscode-languageclient`'s default error policy with
  `connectionOptions.maxRestartCount = 4`.
- Executed the installed dependency's `DefaultErrorHandler` implementation in
  the test to verify three protocol errors continue, the fourth shuts down,
  four rapid crashes restart, and the fifth rapid crash does not restart.
- Made activation await startup, report startup failure, dispose a failed
  client, stop an existing client before repeat activation, and clear the
  active client before shutdown so repeated deactivate calls are idempotent.
- Added a real `npm pack --dry-run --json` test and restricted package contents
  to compiled runtime JavaScript/declarations plus package metadata.

### RED / GREEN Evidence

- Lifecycle RED: 4 failures plus one unhandled rejection. The existing client
  omitted an explicit restart bound, did not stop a client on repeat
  activation, did not await/clean up a failed start, and stopped the same client
  twice on repeated deactivation.
- Lifecycle GREEN: all behavior tests pass after the minimal extension
  lifecycle changes.
- Packaging RED: the actual dry-run manifest contained
  `src/serverPath.test.ts`.
- Packaging GREEN: the actual manifest contains exactly five files:
  `dist/extension.{js,d.ts}`, `dist/serverPath.{js,d.ts}`, and `package.json`.

### Formal Fix Verification

- `rtk pnpm --filter @waml/vscode test`: PASS, 14/14.
- `rtk pnpm --filter @waml/vscode build`: PASS.
- Independent `npm pack --dry-run --json`: PASS, five runtime files and no
  source/tests.
- `rtk cargo test -p waml-cli --test lsp_e2e`: PASS, 3/3.
- `rtk cargo test -p waml-cli`: PASS, 66 tests across three suites.
- `rtk cargo check --workspace --all-features`: PASS with pre-existing
  duplicate-package/dead-code warnings.
- `rtk cargo fmt --all -- --check`: PASS.
- Prohibited production-import scan: PASS, no matches.
- `rtk git diff --check`: PASS.
- TokenSave saved approximately 502 tokens in this round.
