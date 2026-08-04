# Maintainability review — 2026-08-04

Dimension: Maintainability (full evaluation)
Files examined: ~25 read directly, plus tokensave graph scans (god-class, redundancy, dead-code, cycles) over the whole workspace.

---

### [M-1] `editor_session.rs` is a 3.4k-line god object at the centre of the editor
Severity: medium
File: `crates/waml-editor/src/editor_session.rs` (3417 lines)
Evidence: Largest hand-written file in the repo; every mutation path (`app/actions.rs` 1306 lines) funnels into it. tokensave god-class scan also flags `MarkdownEditor` (53 fields), `ClassDiagramSurface` (36), `App` (32).
Why it's wrong: Every feature touches this file; 44 in-module tests exist precisely because it cannot be tested in pieces. It is the single highest change-resistance point in the editor.
Suggested fix: Carve session concerns (dirty tracking, analysis wiring, undo, tab/document sync) into submodules with narrow interfaces, the way `app/` was already split out of `app.rs`.
Confidence: CONFIRMED

### [M-2] Stale-badge overlay duplicated verbatim across the two canvas surfaces
Severity: medium
File: `crates/waml-editor/src/canvas/behavior/mod.rs:20-49` and `crates/waml-editor/src/canvas/class/widget.rs:27-56`
Evidence: Both files contain identical `const STALE_BADGE_LABEL/WIDTH/HEIGHT/INSET`, an identical `fn stale_badge_rect(view_rect: Rect) -> Rect`, an identical `draw_stale_badge_overlay`, and even an identical test `stale_badge_is_fixed_to_the_canvas_top_right` (behavior/mod.rs:963 vs class/widget.rs:1813). tokensave redundancy: `similarity: 1.0, ast_isomorphic`.
Why it's wrong: Four magic constants plus draw logic in two files; a tweak to the badge (size, inset, label) must be made twice or the two diagram kinds drift.
Suggested fix: Move the constants + `stale_badge_rect` + `draw_stale_badge_overlay` (and the one test) into `canvas/mod.rs` or a shared `canvas/overlay.rs`.
Confidence: CONFIRMED

### [M-3] Accent-bucket hex palette duplicated as a second constant table
Severity: medium
File: `crates/waml-editor/src/accent.rs:27-38` vs `crates/waml-editor/src/node_design_editor.rs:574-579`
Evidence: `accent.rs` `bucket_color` maps buckets to `rgb(0x1496dc) … rgb(0x64748b)`; `node_design_editor.rs` re-declares the same eight hexes: `const ACCENTS: [u32; 8] = [0x1496dc, 0x00b4d2, 0x14bea0, 0x5a6ef0, 0xe69614, 0x3cbe5a, 0xeb4678, 0x64748b];` with only a comment ("the Atlas `bucket_*` hexes") tying them together. The private `fn rgb(hex: u32) -> Vec4` helper is also copied verbatim (accent.rs:14, node_design_editor.rs:565).
Why it's wrong: Re-tuning a bucket colour in `accent.rs` silently leaves the node-design editor's swatch row stale; nothing enforces the correspondence.
Suggested fix: Export the hex table (or `bucket_color`) from `accent.rs` and derive `ACCENTS` from it; share one `rgb` helper.
Confidence: CONFIRMED

### [M-4] Icon catalog is a five-way order-coupled parallel structure
Severity: medium
File: `crates/waml-editor/src/icons.rs:4481-4484` (enum at :4485, `IconSet` with 121 fields at :4093)
Evidence: "One variant per catalog glyph, in the exact `IconSet` field order (the load-bearing order invariant: enum == field == DSL == `ALL` == `label`)."
Why it's wrong: Adding one icon means editing five parallel lists in the same order in a 4987-line generated-plus-hand-maintained file; the invariant is enforced only by convention and counts (the batch generator `gen-all-icons.py` is stale, only per-glyph `gen-icon.py` works). Classic change resistance.
Suggested fix: Regenerate the whole parallel structure from one manifest (a macro or the python generator emitting enum + fields + DSL + `ALL` + `label` from a single glyph list), or at minimum add a test asserting the five surfaces agree.
Confidence: CONFIRMED

### [M-5] `waml-editor` exposes 2 modules; ~80 flat `mod` declarations live behind the binary root
Severity: medium
File: `crates/waml-editor/src/lib.rs` (2 lines: `pub mod editor_history; pub mod view_history;`), `crates/waml-editor/src/main.rs:5-84`
Evidence: `main.rs` declares ~78 sibling modules flat at the binary root; `lib.rs` exports only two.
Why it's wrong: Every other module is unreachable from `tests/`, forcing all coverage in-module and making any cross-module refactor a `main.rs`-mediated affair. The flat namespace (canvas, popups, panels, documents, chrome all at one level) hides the actual layering.
Suggested fix: Progressively move the module tree into `lib.rs` (private-by-default is fine — `pub(crate)` still permits an internal hierarchy), grouping panels/chrome/documents into parent modules.
Confidence: CONFIRMED

### [M-6] Deprecated `compat.rs` adapter is still the wire surface for DTO/CLI/LSP
Severity: medium
File: `crates/waml/src/compat.rs:1` ("//! Deprecated mixed-domain adapter retained for DTO, CLI, and LSP callers."), `#[doc(hidden)] pub enum Step` (:17), `pub struct Batch` (:24)
Evidence: 23 KB adapter translating legacy `crate::ops::Op` into `okf::Op`/`uml::Op`, policed by `crates/waml/tests/no_legacy_authority.rs` (22.6 KB).
Why it's wrong: A self-described deprecated layer that external consumers (ops-dto, CLI, LSP) still route through means every new op is added in two vocabularies (legacy `ops::Op` and the domain ops) plus the adapter — a three-file edit to add one case.
Suggested fix: Migrate `waml-ops-dto` and the LSP to speak `okf::Op`/`uml::Op` directly, then delete `compat.rs` and the policing test. If migration is deliberately staged, record the shrink plan next to the module doc.
Confidence: CONFIRMED (that it exists and is load-bearing); PLAUSIBLE (that it is not shrinking — no shrink tracking found).

### [M-7] `waml serve` is a stub while its server dependencies ship in the binary
Severity: low
File: `crates/waml-cli/src/serve/mod.rs:24-31`
Evidence: `pub fn run(args: ServeArgs) -> i32 { eprintln!("waml serve: not implemented yet …"); 2 }` — yet `axum`, `tokio`, `rand`, `subtle` are `waml-cli` dependencies and `ServeArgs` carries `#[allow(dead_code)]` fields "wired up by later tasks".
Why it's wrong: Dead surface + unused-dep footprint in the shipped CLI; the struct/deps encode a design that only exists in comments, which will drift before implementation lands.
Suggested fix: Either land the serve implementation or feature-gate the deps (`serve` feature) so the stub costs nothing.
Confidence: CONFIRMED

### [M-8] Entire `markdown_hosts` module parked behind `#[allow(dead_code)]`
Severity: low
File: `crates/waml-editor/src/main.rs:48-49`
Evidence: `#[allow(dead_code)] // Task 7 host API is mounted by the view integration after its focused review.` `mod markdown_hosts;`
Why it's wrong: Because the gate promotes `dead_code` to a hard error, dead code hides behind allows instead of being removed; this module joins other known dead-but-kept machinery (inspector `Peek` dock states, the ViewBar selection pill). Each allow is a spot the compiler can no longer police, and "after its focused review" has no owner or date.
Suggested fix: Mount it or delete it; if it must wait, reference a tracking issue/plan in the comment.
Confidence: CONFIRMED

### [M-9] Headless `solve/sizing.rs` numerically mirrors makepad's rasterizer
Severity: low
File: `crates/waml/src/solve/sizing.rs:22,116-133`
Evidence: "makepad rasterizes a DSL `font_size` given in POINTS at `pts * 96/72` logical"; `drawn_metrics` "mirror[s] `makepad_draw`'s layouter: the face's ascender/descender in ems are shifted" including the signed-descender fudge (:133).
Why it's wrong: The headless crate has no code dependency on makepad (boundary holds), but it hard-codes the fork's text-layout behaviour (PT_TO_LPX, asc/desc fudges, line-spacing multiply). A change in the fork's layouter silently invalidates every solved size. This is the accepted "one sizing rule for both frontends" design — but the coupling is one-directional and untested against the fork.
Suggested fix: Keep, but add a parity test in `waml-editor` that measures a run through makepad and asserts it matches `chrome_metrics` (the constants are currently pinned only against hand-transcribed numbers in sizing.rs's own tests).
Confidence: CONFIRMED (the mirroring); PLAUSIBLE (absence of a cross-crate parity test — none found).

### [M-10] `apply_dock` mirrored by hand between the two dock panels
Severity: low
File: `crates/waml-editor/src/tree_panel.rs:964-971` vs `crates/waml-editor/src/inspector_panel.rs:1010-1017`
Evidence: Byte-identical bodies; the inspector copy even documents it: "Mirrors `ProjectTree::apply_dock` exactly." Both wrap the shared pure `crate::dock::next`.
Why it's wrong: The transition table is shared but the apply/redraw shim is copied, along with the `toggle/open/close_dock` trio around it — a third dockable panel copies it again.
Suggested fix: A tiny `dock::apply(&mut DockState, ev) -> bool` (changed?) lets each panel keep only the `redraw` call.
Confidence: CONFIRMED

### [M-11] Indistinguishable `document*` module names at the editor root
Severity: low
File: `crates/waml-editor/src/{document.rs, documents.rs, document_host.rs, document_header.rs, doc_view.rs, doc_tabs.rs, okf_documents.rs, uml_documents.rs}`
Evidence: Eight sibling modules whose names differ only by suffix/plural; their roles are distinct (descriptor vs registry vs host seam vs chrome widget) but the names don't say which is which — `document.rs` holds `DocumentDescriptor/OpenDocument`, `documents.rs` the registry, `document_host.rs` the tab/view host.
Why it's wrong: A reader (or a new contributor) must open each to know where a concept lives; new document-adjacent code has no obvious home, which is how near-duplicates start.
Suggested fix: Group them under a `document/` parent module (`document/{descriptor,registry,host,header}.rs`) when M-5's restructuring happens.
Confidence: CONFIRMED

### [M-12] `recovery()` accessor copied per AST wrapper
Severity: low
File: `crates/waml/src/uml/syntax/ast.rs:131-138` and `:1047-1054`
Evidence: Identical `pub fn recovery(&self) -> impl Iterator<…>` filtering `SkippedTokensSyntax | BadToken` on two wrapper types (tokensave: `ast_isomorphic, similarity 1.0`).
Why it's wrong: The recovery-node convention is a language-wide rule expressed per-type; a new recovered node kind means editing every copy.
Suggested fix: Free function `fn recovery_children(node: &SyntaxNode<UmlLanguage>) -> impl Iterator…` (or a default method on a small trait) that all wrappers delegate to.
Confidence: CONFIRMED

### [M-13] Test-helper boilerplate cloned across integration test files
Severity: low
File: e.g. `crates/waml-syntax/tests/{markdown_blocks,markdown_inlines,markdown_extensions}.rs:8` (`parse`), `crates/waml/tests/{uml_attribute,uml_classifier,uml_diagram}_syntax.rs` (`analyze`, `contains`), `{sequence_language,uml_behavior}_syntax.rs` (`root`, `written`), `{formatter_actions,sequence_formatter}.rs` (`apply`), `incremental_analysis.rs:1202/1266` (`text_fingerprint`, `diagnostic_fingerprint` — copied from src)
Evidence: tokensave redundancy scan: 14 of the top-20 exact-duplicate pairs are these helpers (`similarity 1.0`).
Why it's wrong: Low individually, but `text_fingerprint` in `tests/incremental_analysis.rs:1202` duplicates the production `uml/syntax/mod.rs:442` implementation — if the production fingerprint changes, the test copy keeps passing against the stale rule instead of failing.
Suggested fix: A `tests/common/` (or `#[cfg(test)] pub` re-export for the fingerprint fns) per crate; prioritise de-duplicating the two fingerprint functions.
Confidence: CONFIRMED

---

## Not findings (checked, fine)

- **Headless boundary holds**: zero `makepad` code references in `waml`/`waml-syntax` sources (comment mentions only, verified by grep + literal search); manifests confirm no makepad dependency. `taffy` in `waml-editor` is headless-solver-only as documented.
- **One rule, both frontends**: measured sizing lives once in `waml::solve::sizing`; share/bundle encode/decode live once in `waml::{share,bundle_envelope}` and are consumed by CLI and editor alike. No native/web rule copies found.
- **No crate-level circular dependencies**: the workspace is a clean DAG. The intra-crate `ast.rs` ↔ `red.rs` module cycle in `waml-syntax` is the standard red/green tree design, not a defect. (tokensave's giant 190-file "cycle" spans crate boundaries that cargo makes impossible — tool artifact, discounted.)
- **`waml-syntax/src/lib.rs` re-export surface**: flat but grouped by module, and pinned by `tests/public_surface.rs` — deliberate, guarded.
- **`ScanTagKind` count**: recently derived from the exhaustive match (commit 9a29e227) — the previously fragile constant is now self-maintaining.
- **`leafbox`/`nrect` duplication** (`solve/route.rs:1337` vs `solve/mod.rs:640`): both inside `#[cfg(test)]` modules, with an explicit comment acknowledging the duplication and why.
- **`waml-ops-dto` "dead" helpers** (`one`, `default_true`, `is_false`): serde default/skip helpers — false positives of static dead-code analysis.
- **`chrome()` duplicated between `classifier_preview_view` and `source_view`**: identical values today but each view legitimately declares its own `BodyChrome`; coincidental, not a shared rule.
- **`accent.rs`'s existence as a separate module**: the module doc explains the split (harness-bin dead-code gate) — placement is deliberate and documented.
- **makepad SHA-pin prose comment** (`waml-editor/Cargo.toml:13-40`): long, but it is exactly the tribal knowledge that must live next to the pin; leave it.
