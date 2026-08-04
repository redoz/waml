# WAML seven-dimension review — aggregate summary

Date: 2026-08-04. Aggregated from `correctness.md`, `security.md`, `performance.md`, `resilience.md`, `maintainability.md`, `observability.md`, `testability.md` (+ `MAP.md`). All HIGH findings and all PLAUSIBLE-flagged findings were adversarially re-verified against source; verdicts below.

## Verdict

The codebase is in unusually good health where it matters most: the adversarial-input surfaces (share links, bundle envelopes, incremental reparse, the pulldown seam, `write_back`'s staging/rollback transaction) are defensive, typed, and well-tested — Correctness found **zero** high findings across the riskiest code in the repo, and Security's decode paths (64 MiB inflation caps, typed errors, path-traversal defense) are exemplary. The findings that survive verification cluster in three shapes instead: (1) **performance-by-construction** problems — the incremental reparse path pays an unconditional full oracle parse per keystroke, and the editor canvas re-runs taffy layout per node per frame; (2) **an observability/feedback vacuum** — no logging framework anywhere, and open/export/boot failures visible only in a console the GUI user never sees; (3) **safety nets that exist but never run** — four fuzz targets excluded from CI, 4 of 5 web-artifact test suites ungated on PRs, and the editor structurally untestable behind a two-line `lib.rs`. Nothing was refuted; the reviewers' evidence was accurate throughout. Health: solid core, undernourished edges.

## All findings (post-merge, post-verification)

Merged rows keep all contributing IDs; severity is the post-verification value.

| ID(s) | Severity | Dimension(s) | File:line | Summary | Verdict |
|---|---|---|---|---|---|
| P-1 | high | performance | crates/waml-syntax/src/incremental.rs:933-962 | Every incremental Markdown reparse also runs a full oracle parse in the shipping dialect | VERIFIED — unconditional production code, no cfg(test)/debug gate; `WAML_DEFAULT` includes `WAML_SECTIONS` (text.rs:43-51), so it fires per keystroke |
| P-2 | high | performance | crates/waml-editor/src/canvas/class/render/nodes.rs:96; card/mod.rs:332 | Full taffy tree build + layout per node per frame in `draw_card` | VERIFIED — `card::measure(...)` confirmed inside the per-node draw loop |
| P-3 | high | performance | crates/waml/src/solve/route.rs:134-161 | Router rebuilds obstacles + full visibility graph per edge; O(E·N³) worst case | VERIFIED (complexity by inspection; wall-clock unmeasured — benchmark before scheduling a rewrite) |
| S-1 | high | security, resilience | crates/waml/src/frontmatter.rs:147-155 | Unbounded recursion in `parse_value` — hostile `[[[[…]]]]` frontmatter overflows the stack (abort) | VERIFIED — no depth guard on parse, render, or untagged serde path; reachable from any hostile document |
| R-1 | high | resilience | crates/waml/src/uml/analysis.rs:353 (+~20 sites) | UML analysis `expect`s cross-layer seam invariants; a seam bug panics the in-process editor / poisons wasm | VERIFIED — production `expect` on the reuse-miss path confirmed; firing it requires a seam invariant break, which has four recent-commit precedent. High stands |
| O-1 / R-3 | high | observability, resilience | crates/waml-editor/src/app/workspace.rs:284-292,535; app.rs:740 | Open/export/share-boot failures go only to `log!`; GUI user sees a silent no-op (save_feedback even reset to Ok) | VERIFIED — merged: same root cause reported by both dimensions |
| T-1 / S-5 / R-6 | high | testability, security, resilience | Cargo.toml:4 (`exclude = ["fuzz"]`); .github/workflows/ci.yml | Four fuzz targets for the highest-risk surfaces never run in CI | VERIFIED — merged three reports; kept T-1's high (this target class already caught a shipped bug) |
| T-2 | high | testability | .github/workflows/ci.yml:90 | PR CI runs 1 of 5 web-artifact `.test.mjs` suites; the rest gate only at deploy time | VERIFIED — `node --test scripts/inject-runtime-shell.test.mjs` is the only PR step |
| T-3 | high | testability | crates/waml-editor/src/inspector_panel.rs:583; canvas/class/widget.rs:1213 | Complexity-55 `draw_walk` mixes pure state decisions into an untestable draw path; the turtle-balance bug class already shipped once | VERIFIED — the in-code comment itself documents the hazard |
| C-1 | medium | correctness | crates/waml-cli/src/lsp/map.rs:213-223 | Out-of-range LSP diagnostics silently relocate to line 0 | unreviewed-low (evidence quoted, self-evident) |
| S-2 | medium | security | crates/waml-editor/src/browser_boot.rs:25-26,53-56 | Bearer token designed into the URL query string (history/log/Referer leak), latent until `serve` ships | unreviewed-low |
| S-3 | medium | security | crates/waml/src/source.rs:37-47; waml-cli/src/io.rs:426-441 | Bundle paths admit NTFS ADS names (`a:b.md`) — only a byte-index-1 colon is rejected | VERIFIED — validation gap real (`get(1)` check only); exact ADS rename behavior unexecuted, PLAUSIBLE annotation on impact retained |
| R-2 | medium | resilience | crates/waml/src/analysis.rs:600-608,1433,1447 | One shell-level bad document makes the whole bundle unopenable | unreviewed-low (evidence structural) |
| P-4 | medium | performance | crates/waml-syntax/src/incremental.rs:664-932; markdown/snapshot.rs:325-457 | Incremental path does Θ(document) work many times per edit (shell_map ×2, source reconstruction, multiple full-tree walks) | unreviewed-low (structure consistent with the verified P-1 region) |
| P-5 | medium | performance | crates/waml-editor/src/scene.rs:529-535 | O(n²) node-size loop; `node_of` map built 30 lines too late; `drawable_edges` computed twice | unreviewed-low |
| P-6 | medium | performance | crates/waml-markdown-editor/src/widget.rs:793-853 | Full draw-command list rebuilt, copied, and 6×-iterated every frame incl. pure scrolls | unreviewed-low |
| M-1 / T-9 | medium | maintainability, testability | crates/waml-editor/src/editor_session.rs (3417 lines) | God object at the editor's centre; testable only whole (44 whole-session tests) | merged; unreviewed-low |
| M-2 | medium | maintainability | canvas/behavior/mod.rs:20-49 vs canvas/class/widget.rs:27-56 | Stale-badge overlay (consts, fn, test) duplicated verbatim across surfaces | unreviewed-low (tokensave similarity 1.0) |
| M-3 | medium | maintainability | accent.rs:27-38 vs node_design_editor.rs:574-579 | Accent hex palette + `rgb` helper duplicated as a second constant table | unreviewed-low |
| M-4 | medium | maintainability | crates/waml-editor/src/icons.rs:4481-4485 | Icon catalog is a five-way order-coupled parallel structure, enforced by convention only | unreviewed-low |
| M-5 / T-4 | medium | maintainability, testability | crates/waml-editor/src/lib.rs:1-2; main.rs:5-84 | `lib.rs` exports 2 modules; ~80 flat mods private to the binary — cross-module editor behaviour has no test seam | merged; VERIFIED (shape corroborated by both reviews + MAP) |
| M-6 | medium | maintainability | crates/waml/src/compat.rs:1,17,24 | Deprecated compat adapter is still the wire surface for DTO/CLI/LSP; three-file edit per new op | unreviewed-low; "not shrinking" stays PLAUSIBLE (no shrink tracking found) |
| M-7 / T-7 | medium | maintainability, testability, security | crates/waml-cli/src/serve/mod.rs:8-31 | `waml serve` stub ships axum/rand/subtle deps + a frozen arg shape with no behaviour or tests behind them | merged; kept T-7's medium |
| T-5 | medium | testability | crates/waml-editor/tests/README.md:3-5,227-234 | Test-strategy doc stale: claims no lib.rs, blesses skipping property tests for a bug fixed in 10f66dc9 | unreviewed-low (MEMORY corroborates the fix commit) |
| T-6 | medium | testability | crates/waml-editor/tests/fixtures/mini/ | Verification-of-record mutates committed shared fixtures (orders-diagram.md routinely dirty) | unreviewed-low (MEMORY corroborates) |
| T-8 | medium | testability | editors/vscode/src/extension.ts | Extension activation/spawn path entirely untested; only serverPath.ts covered | unreviewed-low |
| O-2 | medium | observability | crates/waml-editor/src/class_diagram_view.rs:331,424-426 | Class-view scene diagnostics Debug-dumped to console; behavior view does it right via statusbar | unreviewed-low |
| O-3 | medium | observability | workspace-wide | No logging framework, levels, or persistence anywhere; failures and TODO stubs share one channel | unreviewed-low (grep-established absence) |
| O-4 | medium | observability | scripts/inject-runtime-shell.mjs:377-394; fork web.rs:1368-1374 | Post-boot wasm panic = frozen canvas, no on-page indication (loader `error` phase covers compile only) | unreviewed-low; "no other layer intervenes" stays PLAUSIBLE |
| C-2 | low | correctness | crates/waml-cli/src/io.rs:275-281 | Case-insensitive duplicate-target check rejects legit case-differing files on Linux | unreviewed-low |
| C-3 | low | correctness | crates/waml-cli/src/lsp/map.rs:179-188 | Stale doc comment describes a departed frontmatter function | unreviewed-low |
| C-4 | low | correctness | crates/waml/src/bundle_envelope.rs:256 | Part markers matched by substring, not line-anchored; hand-edited envelopes can mis-split | unreviewed-low (impact PLAUSIBLE per reviewer — hand-authored envelopes only; accepted) |
| C-5 | low | correctness | crates/waml/src/share.rs:166-187 | base64url decoder accepts non-canonical encodings (dangling bits, len%4==1) | unreviewed-low |
| C-6 | low | correctness | crates/waml/src/frontmatter.rs:7-17 | `FmValue::Num(f64)` derives PartialEq — NaN would break change detection; currently unreachable | VERIFIED latent — derive confirmed; NUM_RE + untagged JSON provably exclude NaN today. Stays low |
| S-4 | low | security | crates/waml-cli/src/io.rs:290-299 vs :353-364 | symlink/type screen races the commit rename (TOCTOU); needs an attacker already inside the bundle root | unreviewed-low |
| S-6 | low | security | crates/waml-cli/src/commands.rs:132-136 | `--export-name` interpolated unescaped into generated TypeScript (self-injection surviving into checked-in output) | unreviewed-low |
| P-7 | low | performance | canvas/class/render/nodes.rs:56-63 | Per-frame HashSet of cloned Strings for focus keys | VERIFIED (seen while checking P-2) |
| P-8 | low | performance | canvas/class/render/nodes.rs:67-81 | No viewport culling; offscreen nodes pay full draw + the P-2 measure | VERIFIED (loop has no view-rect intersection test) |
| P-9 | low | performance | crates/waml/src/uml/analysis.rs:264-271 | O(concepts × documents) catalog scan per analysis pass | unreviewed-low |
| R-4 | low | resilience | app/workspace.rs:680; config.rs:159 | `SystemTime::now()` compiles for wasm, guarded only by recents being empty there | VERIFIED — both sites confirmed un-cfg'd; the panic today stays PLAUSIBLE-only (needs non-empty recents on wasm). Low stands |
| R-5 | low | resilience | crates/waml-cli/src/io.rs:409 | Committed write reported as failure when only staging cleanup fails | unreviewed-low |
| M-8 | low | maintainability | crates/waml-editor/src/main.rs:48-49 | Whole `markdown_hosts` module parked behind `#[allow(dead_code)]` with an ownerless comment | unreviewed-low |
| M-9 | low | maintainability | crates/waml/src/solve/sizing.rs:22,116-133 | Headless sizing hard-codes the makepad fork's layouter numerics with no cross-crate parity test | unreviewed-low; absent-parity-test stays PLAUSIBLE (none found) |
| M-10 | low | maintainability | tree_panel.rs:964-971 vs inspector_panel.rs:1010-1017 | `apply_dock` shim byte-identical between dock panels | unreviewed-low |
| M-11 | low | maintainability | crates/waml-editor/src/{document*,doc*}.rs | Eight indistinguishable `document*` sibling modules | unreviewed-low |
| M-12 | low | maintainability | crates/waml/src/uml/syntax/ast.rs:131,1047 | `recovery()` accessor copied per AST wrapper | unreviewed-low |
| M-13 | low | maintainability | waml-syntax + waml test suites | Test-helper boilerplate cloned across suites; `text_fingerprint` duplicates production code | unreviewed-low |
| O-5 | low | observability | crates/waml-cli/src/commands.rs:39-46 | Human `waml check` output drops the column span it already carries | unreviewed-low |
| O-6 | low | observability | crates/waml-editor/src/app.rs:749-751 | `?api=` accepted and silently does nothing | unreviewed-low |
| O-7 | low | observability | crates/waml-editor/src/source_view.rs:192-194 | Asset-event Results dropped uncommented; an image can silently fail to load | unreviewed-low; user-visible symptom stays PLAUSIBLE |
| O-8 | low | observability | crates/waml-editor/src/config.rs:53-59 | Corrupt config silently reset; the `.bak` rescue is never logged | unreviewed-low |
| T-10 | low | testability | crates/waml/src/uml/{syntax/parser.rs,analysis.rs} | Direct unit coverage of parser internals thin; safety net is integration-shaped (debugging cost, not shipping risk) | unreviewed-low; extent stays PLAUSIBLE (inferred, no coverage run) |

Post-merge totals: **52 findings** (58 raw, 6 absorbed by 5 merges). **High: 9 · Medium: 20 · Low: 23.** Refuted: 0. Downgraded: 0 (three highs carry annotations: P-3 wall-clock unmeasured, R-1 reachability requires a seam invariant break, S-3 ADS rename behavior unexecuted).

**Contradictions:** none found. The closest tension — Correctness clearing `incremental.rs` while Performance condemns it (P-1) — is not a contradiction: the oracle parse is *why* the incremental path is correct, and *why* it is slow. Resilience and Correctness independently agree the MAP's "panic density" counts are almost entirely `#[cfg(test)]` code.

## Top 10 to fix

1. **P-1** — Demote the oracle full-parse to a debug/fuzz cross-check (or derive its two checks from the window parse): it single-handedly negates the incremental machinery for every production keystroke.
2. **S-1** — Depth-cap `frontmatter::parse_value`/`render_value`/serde: the one hostile-input abort reachable from any document, share link, or bundle; trivial fix.
3. **O-1/R-3** — Route open/export/boot failures through the statusbar channel saves already use: silent no-ops on user gestures are the worst UX failure mode present.
4. **R-1** — Convert the UML analysis `expect`s into `AnalysisError` returns: the error channel exists two functions away, and the seam these invariants depend on needed four fixes last week.
5. **T-2** — One-line CI change (`node --test "scripts/*.test.mjs"`): the exact deploy-time-only failure mode has already happened once (dead Pages deploy, exit 0).
6. **T-1/S-5/R-6** — Wire the fuzz targets into a bounded scheduled CI job: three dimensions independently flagged the same dead safety net over the riskiest code.
7. **P-2 (+P-8)** — Cache `Placed` per node on scene revision and add viewport culling: removes taffy-per-node-per-frame and directly attacks the known 500-1200 ms zoom cost.
8. **R-2** — Quarantine shell-failed documents instead of refusing the whole bundle: one oversize file must not make a project unopenable (compounded by O-1's silence).
9. **T-3** — Extract the pure per-draw decisions out of the complexity-55 `draw_walk`s: the turtle-balance bug class already shipped once and is currently untestable.
10. **P-3** — Build the visibility graph once per solve and mask per-edge: superlinear routing on the keystroke path will not stay bounded as diagrams grow (measure first — unmeasured).

## Cross-cutting themes

- **No logging/feedback spine** (observability, resilience): zero `tracing`/`log` usage anywhere; makepad `log!` doubles as both error channel and TODO marker; open/export/boot/scene-diagnostic failures are console-only (O-1/R-3, O-2, O-3, O-6, O-8). One thin leveled seam plus the existing statusbar channel would resolve five findings.
- **Safety nets that never run** (security, resilience, testability): fuzz targets excluded from CI (T-1/S-5/R-6), 4/5 script suites ungated on PRs (T-2), a stale test-strategy doc blessing skips (T-5), vscode activation untested (T-8).
- **The two-line `lib.rs`** (maintainability, testability): M-5/T-4 is the structural cause of T-3's untestable draw logic, M-1/T-9's whole-session-only tests, and the thin `tests/` directory — one restructuring unlocks all three.
- **Per-frame recomputation in the canvas** (performance): P-2, P-5, P-6, P-7, P-8 are one habit — deriving per-frame data that only changes on scene/selection revision — across two crates.
- **Shipped-but-inert surface** (security, maintainability, testability): the `waml serve` stub with live axum/rand/subtle deps (M-7/T-7), a token design already leaning on the URL query string (S-2), `markdown_hosts` behind `#[allow(dead_code)]` (M-8) — the clippy `-D warnings` gate pushes dead code into allows instead of out of the tree.
- **Duplication by convention, not construction** (maintainability): M-2, M-3, M-4, M-10, M-12, M-13 — verbatim copies held in sync only by comments; the icon catalog's five-way order coupling is the sharpest instance.
- **wasm platform traps** (resilience, correctness): `SystemTime::now()` sites guarded by data flow instead of cfg (R-4), while `bundle_envelope.rs` shows the project already knows the correct pattern.

## Refuted / downgraded

**None.** Every HIGH and every PLAUSIBLE-flagged claim checked out against source. Verification specifics:

- **P-1 (specifically scrutinized as requested):** `incremental.rs:933` — `if dialect.waml_sections() { let oracle = crate::markdown::parser::parse_with_structure(...) }` sits at the end of the *successful* incremental path with no `cfg(test)`, `debug_assertions`, or harness gate of any kind; `MarkdownDialect::WAML_DEFAULT` (text.rs:43-51) sets `WAML_SECTIONS`. The claim "runs in production on every incremental reparse of the shipping dialect" is accurate. One annotation: the oracle's result *is* load-bearing (it drives two fallback-to-full decisions at :939-961), so the fix must replace those checks, not merely delete the call — the reviewer's suggested fix already acknowledges this.
- **Annotations retained on three verified highs:** P-3 (complexity confirmed by inspection, wall-clock impact never measured — benchmark before a rewrite), R-1 (the `expect` at uml/analysis.rs:353 is production code on the reuse-miss path, but firing it requires a structure-map/island invariant break; kept high on the strength of four seam-hardening commits in one week), S-3 (the colon-validation gap in `BundlePath::parse` is real — only `as_bytes().get(1)` is screened; the ADS-write endgame via `fs::rename` under a `\\?\` root was not executed — kept medium per the reviewer).
- **C-6:** verified as stated — the `PartialEq` derive on `Num(f64)` exists, and both admission paths (regex `^-?\d+(\.\d+)?$`, untagged JSON) provably exclude NaN today. Correctly filed as a latent low; already at the floor.
- **R-4:** both `SystemTime::now()` sites (workspace.rs:680, config.rs:159) confirmed un-cfg-gated and compiled for wasm; the empty-recents data guard is the only thing preventing the panic. Low stands.
