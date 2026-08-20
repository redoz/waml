# Audit remediation ledger

Tracks every finding in [assessment.md](../../assessment.md) (greybeard audit at
HEAD a6a267e2, 2026-08-20) from raised to closed. One row per appendix finding,
in the audit's own order, so `A<n>` is a stable handle across sessions.

**Status vocabulary** — deliberately narrow, so the ledger cannot decay into the
state the audit caught `issues.md` in:

| Status | Meaning |
|---|---|
| `open` | Not started. |
| `wip` | Being worked now; a branch exists. |
| `done` | Landed on `main`, with the commit named. |
| `decision` | Blocked on a product/strategy call that is the maintainer's to make, not an engineering task. |
| `wontfix` | Deliberately declined, with the reason recorded. |

**The rule:** a row moves to `done` only when the fix is on `main` and the
evidence line names the commit. Anything else stays `wip`. If a fix turns out to
be wrong later, the row reopens rather than a new row being appended.

## Progress

<!-- progress -->

`█░░░░░░░░░░░░░░░░░░░░░░░` **3/53 closed** — 49 open, 1 wip, 3 done

## Findings

| ID | Sev | Area | Status | Evidence / notes |
|---|---|---|---|---|
| A01 | C | Trust layer | `done` | PreparedCandidate::diagnostics() is now the one aggregate stream: quarantined documents surface as document-quarantined errors, and the three swallow arms in validate.rs return an analysis-failed diagnostic instead of an empty (=clean) vec. The vacuous rename all() assertion now asserts on errors. Predicted wave of latent doc-contract failures did NOT materialise: check docs/waml is still exit 0, 2 pre-existing warnings. |
| A02 | C | Distribution | `open` | No way to install the native editor: zero release tags, no installer, no binaries; extension unpublishable |
| A03 | C | Product definition | `open` | No external user, adoption path, or competitive confrontation exists anywhere in docs/ |
| A04 | C | Accessibility | `open` | The flagship zero-install reader renders all text into a GPU canvas: zero screen-reader accessibility, no browser find, no text indexing/SEO — for… |
| A05 | C | Rendering gate | `open` | No automated rendering regression gate; pixel tool manual and unwired; in-CI screenshot test asserts file existence only. Precise platform gap: the… |
| A06 | H | CI safety net | `wip` | Build repair landed 9ccf0e7a (cargo-fuzz pinned to the gnu host triple); it now finds real bugs. Corpus persistence and red-run notification still open. |
| A07 | H | Data integrity | `done` | Corrected: NOT live data loss — the only wire field (AttrSet.mult) already carried skip_serializing_if and round-trips Unchanged correctly (verified). The real defect was the latent trap: serializing Unchanged emitted null, so any future field forgetting the attribute would silently turn 'leave alone' into 'delete'. Unchanged now fails to serialize with a message naming the fix; three-state round-trip pinned by test. |
| A08 | H | Triage discipline | `open` | issues.md frozen since 2026-08-05 through heavy landing; mixes verifiably-fixed and verifiably-live P1s (verified: last commit cb64c9c0 2026-08-05) |
| A09 | H | Doc contract | `done` | Re-gated by A01: the contract's command now reports quarantined documents and analysis failure. Verified after the fix — docs/waml is genuinely clean, not silently clean. CI step ordering is tracked separately as A35. |
| A10 | H | Supply chain | `open` | Zero dependency auditing (no cargo-audit/cargo-deny/deny.toml anywhere); all four GUI crates + the Pages build tooling routed through one personal… |
| A11 | H | Layering | `open` | okf substrate transitively depends on the UML tier; the "mechanical okf-core split" header claim is fiction |
| A12 | H | Layering | `open` | analysis.rs is a hub with three mutual-dependency directions; OkfAnalysis back-patched with UML data |
| A13 | H | Cohesion | `open` | uml/analysis.rs is a 4,610-line dumping ground: extraction + validation + projection + orchestration; 415- and 405-line functions |
| A14 | H | Performance ceiling | `open` | Every edit triggers bundle-wide semantic reanalysis — the project's own open P2 and the real scaling limit; incremental parsing feeds non-increment… |
| A15 | H | App shell | `open` | 54-field App god object, impl across 7 files/~7,800 lines, linear per-feature accretion |
| A16 | H | Fork seam | `open` | Fork SHA hand-copied into 5 entries across 4 manifests; pages.yml pins a different SHA by prose comment |
| A17 | H | Fork inventory | `open` | 44 commits / +2992−563 fork divergence inventoried only in a Cargo.toml comment; `wip:` commit in pinned lineage |
| A18 | H | Wire contract | `open` | HTTP envelope + responses defined twice as unlinked serde types; only DocumentWrite shared |
| A19 | H | Solver depth | `open` | Solver internals ~10% tested; invariant-preserving quality regressions escape — proven by fd8f305f |
| A20 | H | LSP | `open` | Half-built LSP corrupts state: no didSave/watched files; close restores stale startup disk bytes |
| A21 | H | Verification debt | `open` | 10+ landed features owe visual sign-off, ledger exists only in agent session memory |
| A22 | H | Effort allocation | `open` | Three company-sized products + a forked framework, solo; every MVP area "partial"; channels that reach users (extension, LSP, image export) explici… |
| A23 | H | Interop | `open` | No mermaid/plantuml import/export, no SVG/PNG export — diagrams cannot leave the toolchain |
| A24 | H | Sustainability | `open` | Bus factor 1 with contribution actively repelled: purged single-author history, no releases, no CONTRIBUTING, stub CoC |
| A25 | M | Licensing | `open` | MPL-2.0 repo distributes compiled MIT code (entire makepad fork) + merman (MIT OR Apache-2.0) via Pages and every exported site, with no NOTICE/THI… |
| A26 | M | Serve security | `open` | Competent core (CSPRNG token, constant-time compare, loopback default, Host/origin checks) but the token is accepted and printed as a `?token=` que… |
| A27 | M | Surfaces | `open` | BodyWidgets show_* = five hand-copied sibling lists drifted from the 8-surface authority; compensated at 3 scattered sites; `set_behavior_canvas_vi… |
| A28 | M | ViewOutcome | `open` | Ten-Option command bag; own comments admit each field exists because the channel couldn't express it |
| A29 | M | Solve pipeline | `open` | Positional 3-list coupling in build_scene; desync admitted in comments, guarded by debug_assert; orchestration lives editor-side |
| A30 | M | Solve API | `open` | Entry-point accretion: route ×4 tiers, stress ×3 with layout/layout_grouped production-dead; no facade, 15 editor entry points into 8 submodules |
| A31 | M | Error handling | `open` | Two incompatible regimes: structured DiagCode vs stringly EditError/BundleError::Analysis(String)/Result<_,String> family; AnalysisError flattened… |
| A32 | M | cfg seams | `open` | 155 target_arch cfgs, 16 files, no shared facade; markdown_extensions/mod.rs = 53 cfgs interleaving both platforms |
| A33 | M | Dead code | `open` | ~170-190 `#[allow(dead_code)]` defeat the -D warnings gate; blanket allow on DocView trait; Peek/radial/node_design_editor compiled into the shippi… |
| A34 | M | CI ordering | `open` | Doc-contract runs before compilation; this exact ordering already masked CI 3 days. wasm PR check omits waml-editor |
| A35 | M | Panic hygiene | `open` | 110-116 unwraps in the adversarial-input island parser; content-reachable expects; no catch_unwind at LSP/wasm entries |
| A36 | M | Frontmatter grammar | `open` | YAML-subset quoting split across two crates — three quote-scanner implementations for one grammar |
| A37 | M | UI-test harness | `open` | 3,583 lines + proc-macro crate serve 4 scenarios (~900 lines/scenario) |
| A38 | M | Chain layering | `open` | Chain::build name-checks "hide" to validate params inline |
| A39 | M | Planning hygiene | `open` | 74 active plans mixing implemented/abandoned/horizon; atproto collab plan (unstarted, MVP-contradicting) in the active queue |
| A40 | M | Incremental parser | `open` | Ω(n) per-edit floor (≈4 full-document passes); zero reparse benchmarks for 95K chars of machinery; multi-section edits silently full-reparse — and… |
| A41 | M | Workspace hygiene | `open` | 65 branches (50 merged), 10 stashes incl. stray-edits-on-main, root .test_out5.log, root lsp-demo/ |
| A42 | M | OKF spec governance | `open` | Spec vendored by wholesale replacement, no upstream SHA or drift check; WAML profile has one implementation |
| A43 | L | DiagCode | `open` | Kebab-case wire names maintained twice (serde rename + 80-arm as_str) |
| A44 | L | Solver wire | `open` | Stringly keys; `key: Option<String>` retrofit for duplicate edges |
| A45 | L | Naming | `open` | waml-syntax "domain-neutral" headline false; 4 modules named "navigation"; NavCategory is a 10-variant identity mirror of RowKind |
| A46 | L | Dead API | `open` | validate_from_source has zero callers on a diagnostics-swallowing path; uml::lower::referrers kept alive by one test port |
| A47 | L | List primitives | `open` | Three copy-adapted scrollbar geometries with duplicated tests; list widgets built by lineage copying |
| A48 | L | Arc-identity rename | `open` | Move ops must preserve text Arcs or Renamed degrades to Removed+Inserted; documented, unenforced |
| A49 | L | Property spread | `open` | Edit-op layer example-based only; no apply→write→reparse round-trip property |
| A50 | L | Extension packaging | `open` | No vsce/ovsx/bundler; bundled-binary resolution self-documented as dead |
| A51 | L | Manifest honesty | `open` | waml-editor's description says "read-only GPU viewer" while shipping two write backends. (The workspace-1.80/editor-1.95 MSRV split is the intended… |
| A52 | L | classifier_page | `open` | "CLI can emit the identical page" has no CLI consumer |
| A53 | L | Parser fn size | `open` | Island parser fns at 326/277/267/263 lines — defensible, trending toward the uml/analysis.rs failure mode |
