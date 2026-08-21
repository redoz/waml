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
| V13 | Splash logo pulse variants | `67bd394`, **branch `ui-tweaks-2`, NOT on main** | Not a sign-off — a **choice**. Six variants are built and rendering; the owner picks one and the rest get deleted. `cargo run -p waml-editor --bin logo_pulse_harness` stacks all six, top to bottom modes 1..6. |

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
