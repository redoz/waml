# Observability review — 2026-08-04

Dimension: Observability (full evaluation)
Files examined: ~18 (waml-editor: app.rs, app/workspace.rs, app/actions.rs, app/navigation.rs, class_diagram_view.rs, behavior_doc_view.rs, scene.rs, source_view.rs, statusbar.rs, browser_boot.rs, config.rs; waml: diagnostic.rs, solve/mod.rs; waml-cli: main.rs, commands.rs, lsp/server.rs; scripts/inject-runtime-shell.mjs; makepad fork platform/src/os/web/web.rs)

## Findings

### [O-1] Editor failures are console-only; the GUI user is never told
Severity: high
File: crates/waml-editor/src/app/workspace.rs:284,292,535; crates/waml-editor/src/app.rs:740
Evidence:
```rust
Err(BackingTransitionError::Load(error)) => { ... log!("{error}"); return false; }   // open_dir
if let Err(error) = crate::bundle_export::export_bundle(...) {
    log!("failed to export the WAML bundle: {error}"); }                             // export
Err(e) => { log!("could not open the model in this link: {e}"); self.show_start_screen(cx); }  // web share
```
Why it's wrong: `log!` goes to stderr (native) or the browser console (wasm). A user who clicks a recents entry, opens a folder via the file dialog, exports a bundle, or follows a broken `#w1.` share link and fails gets the start screen or nothing, with zero in-app message. Only *save* failures reach the statusbar (`sync_save_error`); open/export/boot failures have no UI surface at all. On a natively double-clicked GUI app there is no console, so the message is lost entirely.
Suggested fix: route these errors through the existing statusbar message channel (or a toast), the way `save_or_retry` → `sync_save_error` already does.
Confidence: CONFIRMED

### [O-2] Class-diagram build diagnostics are Debug-dumped to the console, never shown in the UI
Severity: medium
File: crates/waml-editor/src/class_diagram_view.rs:331,424-426
Evidence:
```rust
for d in &diags {
    log!("diagnostic: {d:?}");
}
```
Why it's wrong: `build_scene` returns structured `Diagnostic`s (code, message, file, line), but the class view prints them with `{:?}` — a format written for the parser author, not the document author — and only to the console. The behavior view does this right (`diagnostics_status` → `set_solver_diagnostics` on the statusbar, deduplicated, behavior_doc_view.rs:818-825); the class view has the same statusbar channel available (it uses it for `projection_status` at :929) but never feeds scene diagnostics into it. A document whose class diagram silently drops content shows a degraded canvas with nothing said in-app.
Suggested fix: mirror behavior_doc_view's `diagnostics_status` summary into `set_solver_diagnostics` for the class view; format with `d.message`, not `{:?}`.
Confidence: CONFIRMED

### [O-3] No logging framework, no levels, no persistence — one session later, nothing survives
Severity: medium
File: workspace-wide (zero `tracing`/`log` deps; only makepad `log!` and CLI `eprintln!`)
Evidence: `grep` across `crates/*/src` finds no `tracing::`/`log::` use; the editor's only channel is makepad's `log!` (unleveled, unstructured, stdout/console-only), used interchangeably for real failures (`app/workspace.rs:178 "failed to save open document"`) and TODO stubs (`app/actions.rs:250 "New model: not yet implemented"`).
Why it's wrong: there is no way to distinguish an error from a stub note, no document/node context convention, and nothing is captured to disk — a user reporting "it lost my edit yesterday" leaves the debugger with no artifact. The structured diagnostic pipeline covers document problems, but the editor's own operational failures (save, load, export, boot) have no durable record.
Suggested fix: adopt one thin leveled logging seam (even a project macro over `log!`/`eprintln!` with a level + component prefix), and on native mirror warnings/errors to a `~/.waml/log` ring file.
Confidence: CONFIRMED

### [O-4] A post-boot wasm panic leaves a frozen canvas with no on-page indication
Severity: medium
File: scripts/inject-runtime-shell.mjs:377-394; makepad fork C:\dev\makepad\platform\src\os\web\web.rs:1368-1374
Evidence:
```rust
pub fn panic_hook(info: &panic::PanicHookInfo) { crate::error!("{}", info) }  // console only
```
```js
WebAssembly.compileStreaming = function () { ... safelySetPhase('error'); ... }  // covers compile only
```
Why it's wrong: the panic hook names the cause in the browser console (good — web parity on the *naming* holds), but the loader's `error` phase only fires for compile/instantiate failures. A panic after boot leaves the last-drawn frame frozen; a non-developer user sees a hung page and never opens the console. Native at least visibly terminates the process.
Suggested fix: have the panic hook (or a JS `error`/`unhandledrejection` listener installed by the runtime shell) flip a visible "the editor crashed — reload" banner on the page.
Confidence: CONFIRMED for the mechanics; PLAUSIBLE that no other layer intervenes (no such handler found in the runtime shell or web.js).

### [O-5] Human `waml check` output drops the column span the diagnostic carries
Severity: low
File: crates/waml-cli/src/commands.rs:39-46
Evidence:
```rust
lines.push(format!("{}:{}: {}[{}]: {}", d.file, d.line, severity_str(...), d.code.as_str(), d.message));
```
Why it's wrong: `Diagnostic.span` (byte range within the line) exists and is emitted in `render_json` (:66), but the human rendering prints only `file:line`, so a long line with a malformed attribute makes the user hunt for the column that the tool already knows.
Suggested fix: append `:{start}` (or `:{start}-{end}`) when `d.span` is `Some`.
Confidence: CONFIRMED

### [O-6] `?api=` boot parameter is accepted and silently does nothing
Severity: low
File: crates/waml-editor/src/app.rs:749-751
Evidence:
```rust
// `?api=` is selected for, but no live model server exists yet; the
// URL is honoured as far as "not a bundle, not a share link".
crate::browser_boot::BrowserBootSource::Api { .. } => self.show_start_screen(cx),
```
Why it's wrong: a user given an `?api=` URL lands on the start screen with no message anywhere — not even a console line — that the parameter was recognized but unimplemented. The neighboring stub paths at least `log!`.
Suggested fix: `log!` (and ideally show) "live model server URLs are not supported yet" when the Api source is selected.
Confidence: CONFIRMED

### [O-7] Markdown asset events discarded unchecked — an image can silently fail to load
Severity: low
File: crates/waml-editor/src/source_view.rs:192-194
Evidence:
```rust
for event in host.drain_events() {
    let _ = assets.apply_event(event);
}
```
Why it's wrong: `apply_event`'s `Result` is dropped with no comment marking the discard deliberate, in the presentation-compile path. Nearby failures in the same file are logged (`source_view.rs:418 "image measurement layout failed"`), so a failed asset event is the one step in this chain that vanishes — the likely symptom is an image that never appears with nothing said.
Suggested fix: `if let Err(e) = assets.apply_event(event) { log!("asset event failed: {e:?}"); }` or a comment stating why the error is meaningless here.
Confidence: CONFIRMED (discard); PLAUSIBLE (user-visible symptom).

### [O-8] Corrupt config is silently reset — recents/theme vanish with no note
Severity: low
File: crates/waml-editor/src/config.rs:53-59
Evidence:
```rust
Err(_) => {
    // Preserve the corrupt file for forensics, then start clean.
    let _ = std::fs::rename(&path, dir.join(format!("{file}.bak")));
    T::default()
}
```
Why it's wrong: the `.bak` rescue is good and deliberate, but nothing logs that it happened — a user whose recents list disappears has no breadcrumb pointing at the `.bak`, and the parse error itself (which file, what was wrong) is discarded.
Suggested fix: `log!("waml-editor: {file} was corrupt ({e}); backed up to {file}.bak")` before returning defaults.
Confidence: CONFIRMED

## Not findings (checked, fine)

- CLI error hygiene: all ~37 error prints uniformly prefixed `waml: `, distinct exit codes (0 clean / 1 diagnostics / 2 IO), `--json` output includes span (crates/waml-cli/src/main.rs, commands.rs:146).
- Core diagnostic model is strong: stable kebab-case codes, per-code severity, file+line+span+revisioned provenance (`with_provenance`), serde-shaped (crates/waml/src/diagnostic.rs).
- LSP surfaces internal failures to the client via `log_message(ERROR/WARNING)` (lsp/server.rs:106-112,126); publication ordering vs. state races is tested.
- Save failures ARE user-visible: `save_or_retry` → `save_feedback` → statusbar `set_save_error`, with scheduled retry (app/workspace.rs:160-186).
- Behavior view reports solver diagnostics to the statusbar, deduplicated, plus a named stale-projection banner ("Stale behavior: source revision N…", behavior_doc_view.rs:862-874).
- Dropped layout placements are projected into visible `SceneConflict`s, not swallowed (scene.rs:546,660, `project_conflicts`).
- Web boot fetch errors have named, unit-tested messages (`boot_fetch_error`, browser_boot.rs:123 + tests), and boot never leaves a blank window — the start screen is the deliberate fallback.
- Boot-config 404/non-UTF8 quietness is deliberate and documented ("a 404 there is the normal case", app.rs:771-774,831-833).
- wasm panic hook exists in the fork and names the panic (payload + location) in the console (web.rs:1368-1374) — console parity with native stderr holds.
- Loader phase reporting is defensively wrapped so it "must never replace the compiler outcome" (inject-runtime-shell.mjs:370-376); update-check network failures are deliberately quiet with retry.
- No per-frame logging found in any draw path; behavior solver log is change-deduplicated (`status != self.last_diagnostics`).
- Config store failures are logged with the value being persisted (config.rs:270,294,306).
- Core crates (waml, waml-syntax) show essentially no swallowed `Result`s outside tests.
