# Text Editor Integration

**Goal:** VS Code connects Markdown documents to the WAML language server and
reports setup and launch failures clearly.

**Why:** An integration that needs manual recovery is an integration that few
persons use.

**Done when:** The extension resolves and starts one server with a bounded
restart policy, reports missing or failed servers, and stops the client during
replacement or deactivation. Incorrect-version detection remains planned.

**Status:** partial
**MVP:** no

## Shipped behavior

#### VSCODE-001 — resolve the language-server executable in priority order

**Applies to:** native

**Given** one or more server paths can come from the environment, an explicit setting, a bundled binary, or `PATH`
**When** VS Code resolves the language-server executable
**Then** it uses the first runnable source in that priority order

**Evidence:** `editors/vscode/src/serverPath.ts:34`

#### VSCODE-002 — launch one Markdown language client with bounded restarts

**Applies to:** native

**Given** VS Code resolves a runnable WAML language server
**When** the extension activates for Markdown documents
**Then** it starts one stdio client with the bounded restart policy

**Evidence:** `editors/vscode/src/serverPath.test.ts:207` and `editors/vscode/src/serverPath.test.ts::pins the installed client's bounded default error and crash-restart behavior`

#### VSCODE-003 — report a missing runnable server

**Applies to:** native

**Given** no configured, bundled, or `PATH` server is runnable
**When** the extension activates
**Then** VS Code shows a clear error and does not construct a language client

**Evidence:** `editors/vscode/src/serverPath.test.ts::reports an unresolved executable without constructing a client`

#### VSCODE-004 — repeated activation stops the previous client first

**Applies to:** native

**Given** the extension already has an active language client
**When** VS Code activates the extension again
**Then** the extension stops the previous client before it starts another client

**Evidence:** `editors/vscode/src/serverPath.test.ts::stops the previous client before a repeated activation starts another`

#### VSCODE-005 — a launch failure is reported and cleaned up

**Applies to:** native

**Given** the resolved language client fails to start
**When** the extension handles the launch failure
**Then** it stops and clears the failed client and shows the failure message

**Evidence:** `editors/vscode/src/serverPath.test.ts::cleans up and reports a launch failure`

#### VSCODE-006 — deactivation stops the active client exactly once

**Applies to:** native

**Given** the extension has an active language client
**When** VS Code deactivates the extension one or more times
**Then** the extension stops that client exactly once and clears it

**Evidence:** `editors/vscode/src/serverPath.test.ts::deactivate stops the active client once and clears it`

#### VSCODE-008 — the missing-server action opens WAML settings

**Applies to:** native

**Given** VS Code shows the missing-server error with a settings action
**When** the user selects that action
**Then** VS Code opens the WAML settings

**Evidence:** `editors/vscode/src/extension.ts:19`

## Planned behavior

- BHV-VSC-007 — A clear error for an incorrect language-server version has no passing acceptance scenario.

## Verification gaps

- VSCODE-001 — target: native; No test proves a runnable bare waml command on PATH; the existing PATH test proves only the missing-command error path.
- VSCODE-008 — target: native; The extension wires the action to workbench.action.openSettings, but the mocked VS Code test never selects the action and does not assert the command.

## Notes

- The extension is a separate Node project and uses the standard language
  client.
- Publication in an extension marketplace is outside these frozen behavior
  rows.
