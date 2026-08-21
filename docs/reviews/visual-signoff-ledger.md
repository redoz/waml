# Visual sign-off ledger

Features that landed on `origin/main` without anyone looking at them.

Every entry below is code that shipped, passed its gate, and was never
visually verified by a human. The headless gate cannot see any of it: it
proves the code runs, not that the pixels are right. Several of these landed
unattended overnight, which is exactly when nobody was watching.

Until today this list existed only in one agent's session memory, which is
not a place a project can keep an obligation. That is the finding this file
closes (audit row A21); the obligations themselves are still open.

**How to sign off.** Run the editor (`run.ps1`, or `/run` with a fixture),
drive the listed case by hand, and record the verdict here — replace the
`OWED` line with the date, what you saw, and any defect filed. Do not delete
a row; a signed-off row is the evidence that it was checked.

**Two traps** when verifying by screenshot on this machine: capture and kill
by PID in one step or you will hit the user's own editor window, and never
try to judge pixels from PowerShell — look at the image.

| # | Feature | Landed | What is unverified |
|---|---|---|---|
| V1 | Diagram pens | `bf1303ec` | The whole stroke-weight ladder on both canvases. Class edges deliberately went 3.0 -> 2.0 and the behavior divider 1.4 -> 1.0: **these are intended, do not "restore" them.** Lifeline stems and interaction frames went 1.2 -> 1.5, which quantisation DOUBLES at dpi 1 (1.4 rounds to one device pixel, 1.5 floors to two) — that is the one most likely to read as a regression. |
| V2 | Viewer font-size control | `6e4d3cb0` | Nobody has looked at this feature at all. The archived plan carries a 7-row verification table. Row 5 has an escape hatch: if the source editor's caret/IME geometry does not follow the scale, defer the source half rather than ship it half-scaled. |
| V3 | UML extension row icons | `22672758` | V1–V6 from the plan. Core OKF folders should draw the book glyph, UML packages the box glyph, **and every other glyph must be unchanged** — that last guarantee was silently false once already. |
| V4 | Granular projection mask | `dc5f5fa8` | 8 visual checks from the plan, plus 2 medium review warnings. A disabled stage is a silent skip, so a wrong mask looks like a working view with content missing — the failure mode is invisible by construction. **Partly guarded now:** `project_tree_lists_every_row_of_the_bundle` pins the default projection's exact row list, so a stage that starts dropping rows fails a test instead of looking fine. It only covers the default mask — driving the projection menu is not yet a harness operation — so the 8 checks stand. |
| V5 | Surface-routed navigation | `8d0e9a78` | The forcing case by hand: folder tab -> view source -> back, across surfaces. Plus 3 medium review warnings. |
| V6 | Book mode, phase 1 | `8c1b8204` | A folder read as one continuous scroll: diagram embeds live and capped, open-full to a tab, the tree working as a TOC in both directions. 3 design calls also owed. |
| V7 | Bundle search | `679c8994` | **Partly automated.** The `ui` scenarios now settle palette sections, results-tab grouping, find-strip scoping, and F3/Shift+F3 traversal with wrap — those are state, and state does not need eyes. What is still owed: that the palette, results tab and find strip *look* right (spacing, section headers, the strip's placement), and `DocView::reveal` actually scrolling the document to the landed hit. The reveal target is drawn inside a view that exposes no semantic item, so no assertion can reach it today. |
| V8 | Conflict list, grouped delete | `0944e0f` | The 4th `PopupRoot` surface: badge-open, focus, delete -> re-solve -> refresh-open. Hit rects are Turtle-derived and the trash is a drawn glyph rather than a child widget, so misalignment is the thing to look for. |
| V9 | Drag-to-place constraints | S1–S4 + VIZ | Grey no-go zones for existing constraints, the diagonal drop authoring BOTH placements, and "updating" rather than "adding" when you drop into grey. |
| V10 | Connector repair phases | `fd8f305f` | Two defects were fixed in the phases that run after A*. The sparse tests cannot see connector quality — this needs eyes on real diagrams. |
| V11 | Recents pinning | `e6e3733` | Per-row pin on the start screen. The pin hit-test must gate on the same verdict as the draw, which is the bug that was fixed last time. |
| V12 | Docked collapsible panels | `2731490` | **Partly obsolete — read this before verifying.** The Peek state machine was deleted as dead on 2026-08-21 (`475a3959`), so peek-slide feel and the ~600ms auto-collapse are no longer things that exist. What remains to verify: flag legibility, pin reflow, and both panels pinned at once. Two known by-design nits, not bugs: a `FLAG_W` (28px) edge gutter when pinned, and the flag label being a per-glyph vertical stack rather than rotated text. |
| V14 | Connector quality repairs | `90ffcf0f` | Five real router defects fixed, and two goldens moved with them. Look at: the activity fixture's `Retry`/`Check` back-edge pair, which should now read as two distinct lines rather than one; the state-machine's `Active` self-loop, now 24px off its border instead of 16 — confirm it clears the frame and any label. Then a wide fan-in: `hub_spread` now sometimes rebuilds a connector as a 5-point shape instead of a 3-point one, which is provably sound but a taste call. Endpoints also slide along their border during `nudge`, which should read as better fan-out but interacts with arrowheads and label placement that no test sees. 52 shared-run overlaps remain on dense synthetic scenes (none on authored fixtures) — whether any is visible at render size is a judgement no assertion can make. |
| V15 | Dock chrome extraction (`DockChrome`) | branch `a15-dock-chrome-extraction` | A **pure state move**, not a feature: the eleven dock/caption fields left `App` for `app::dock_chrome`. No widget changed parent and no `script_mod` order changed, and `app/tests/shell.rs` still pins slot/host/body geometry, both splitter drags and the toggle-seat continuity. Three things those tests structurally cannot see. **(a) The narrow-mode dismiss gesture.** The `MouseMove` arm of `route_narrow_dock_pointer` was deleted — it only wrote `pointer_in_narrow_dock`, which nothing read. Shrink the window under 640px, open the tree panel so it floats, and tap the canvas: the panel must still dismiss, and tapping *inside* the panel must not. **(b) The toggle handoff, animated.** Tests seat `DockMotion` at its end states, so no test has ever run the 180ms curve. Drag the window across 640/680 with the tree column open and watch the `[|]` toggle slide out to the column edge and back to the row head — it must stay continuous, with no jump of the button's own 32px width at the handoff. **(c) The chrome seam break.** `sync_chrome_seam`'s change guard moved into `DockChrome::commit_seam_break` unchanged; confirm the hairline under the caption still breaks at the active tab card and follows it when you switch tabs. |
| V13 | Splash logo pulse variants | `67bd394`, **branch `ui-tweaks-2`, NOT on main** | Not a sign-off — a **choice**. Six variants are built and rendering; the owner picks one and the rest get deleted. `cargo run -p waml-editor --bin logo_pulse_harness` stacks all six, top to bottom modes 1..6. |
| V16 | Inspector element-picker item identity | branch `worktree-agent-a4bc54d355b193180` | A **defect fix with one widget-identity side effect**, not a feature. The picker's synthetic `"src->tgt#N"` edge key is gone; rows carry a `Subject` and their `SelectBox` item ids are now *positional* (`picker_item_id(idx)`) rather than hashed from the row's key, and member-card `FlatList` ids key on the row's label rather than its node key. Every projection is unit-tested and the row order is unchanged, so nothing should move — but item ids are what makepad reuses widget state by, so this is the one thing tests cannot see. Open a diagram's element picker: every row must still be pickable and land on the element its label names, the highlighted row must follow the canvas selection, and scrolling the flyout must not leave a row wearing another row's lead glyph. Then open a group's member cards and check the same. **The forcing case, if you want one:** a diagram with a group named exactly after one of its nodes — those two rows used to hash to the same id, so the group row selected the node; now they are distinct. |

## What the rendering gate covers, and what it does not (audit row A05)

There is now one automated gate on pixels:
`the_light_cycle_canvas_is_drawn_the_way_its_reference_was` in
`crates/waml-editor/tests/ui.rs`. It drives the headless editor to a state
machine, crops to the diagram surface, and compares the canvas against a
stored **ink mask** -- one bit per pixel, background or not. It settles
geometry: where connectors run, how thick a stroke is once quantised, where
glyphs sit and how big they are. It settles nothing about colour, antialias
quality, or whether a route reads *well*.

**It closes no row below, and the honest reason is worth stating.** Every row
here is an obligation to look at something that already shipped, and a
reference recorded today records what shipped -- including whatever is wrong
with it. A gate can stop the NEXT change from going unseen; it cannot see the
last one. What it changes is which of these rows can regress silently again:

| Row | After the gate |
|---|---|
| V1 Diagram pens | **Partly guarded going forward, one canvas only.** A stroke that quantises to a different number of device pixels fails the gate -- which is exactly V1's stated worst case (1.5 flooring to two device pixels at dpi 1). But the gated canvas draws behavior routes and state boxes; class edges, lifeline stems, interaction frames, group hulls and label leaders are not in it. The colour half of V1 is not covered at all: an ink mask throws colour away by construction. Sign-off still owed. |
| V10 Connector repair phases | **Change-detection going forward.** A repair phase that starts rewriting connectors fails the gate instead of shipping green. The gate hands a human two drawings and a red/green overlay; whether the new one is *better* is still the human's call, which is what V10 is really about. Sign-off still owed. |
| V14 Connector quality repairs | Same as V10, and this is the change the gate was built against: `regression_proof` in `waml-ui-test` inks `90ffcf0f`'s before and after route sets from the repo's own history and shows the comparator fails on them by six times its tolerance. Sign-off still owed -- nobody has looked at the fixtures. |
| V2, V3, V4, V5, V6, V7, V8, V9, V11, V12, V13, V15, V16 | **Not covered.** Everything the gate compares is inside one diagram canvas. Tree row icons (V3), the projection menu (V4), navigation across surfaces (V5), book mode (V6), the palette and find strip (V7), popup surfaces (V8), drag preview (V9), start-screen pins (V11), dock flags (V12), the splash variants (V13), dock chrome (V15) and the element picker's flyout (V16) are all outside the crop, or are about temporal feel, or are hit-testing against drawn rather than laid-out things. No mask can reach any of them. |

**One step is outstanding before the gate enforces anywhere.** Linux is its
platform of record (see the third trap above), and no Linux reference could
be recorded from a Windows machine, so none shipped in the landing commit.
Until one is committed the gate records its own capture and passes advisory,
saying so in its trace. The first Linux CI run uploads that recording as the
`rendering-gate-<run id>` artifact; download it, commit
`recorded-references/light-cycle.linux-x86_64.ink` into
`crates/waml-editor/tests/references/`, and the gate starts enforcing.
`crates/waml-editor/tests/README.md` has the recipe.

## Why this keeps happening

The automated plan runner cannot do visual verification, so a plan whose
final task is "look at it" either stalls or lands unverified. The runner
takes the second path, which is right — but the obligation then has nowhere
to go except a human's memory. This file is that place.

A plan that ends in a visual check should add its row here as its last task,
in the same commit that lands the feature.

## What does not have to stay here

Before adding a row, check whether the obligation is really about pixels.
Several of the entries above are not: "did the tree lose a row", "did the
cursor wrap at the end of the hit list", "did the selection land where the
navigation said it would" are questions about *state*, and
`crates/waml-ui-test` can settle those against a real headless editor
without a human in the loop. Its crate docs carry the standing line between
what it can decide and what it cannot; V4 and V7 above are each partly
discharged that way.

A row belongs here when the claim is about how something was *drawn* —
stroke weight, glyph identity, spacing, legibility, feel — or when the thing
being hit-tested is drawn rather than laid out as a widget, so it has no
snapshot entry to assert against. Those are the rows that will still be here
next year, and that is fine. What is not fine is a state question sitting in
this file because writing the scenario looked like more work than looking
once.
