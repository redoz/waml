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
