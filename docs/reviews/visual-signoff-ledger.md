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

**Adding a row is not free.** Before adding one, read
"[Which half is state](#which-half-is-state)" below. Several rows here were
never really about pixels, and one of the things this file is for is keeping
state questions out of a human's queue.

**The Verdict column** says which instrument the row needs: `state` for
anything `crates/waml-ui-test` can settle against a real headless editor,
`pixels` for ink inside a diagram canvas, `human` for everything else.
`blocked` means the question IS state and the harness still cannot reach it
— the row then names what it would take. No row is `state` alone: every
entry here also carries a looking-at-it obligation that no test discharges.

| # | Feature | Landed | Verdict | What is unverified |
|---|---|---|---|---|
| V1 | Diagram pens | `bf1303ec` | pixels + human | The whole stroke-weight ladder on both canvases. Class edges deliberately went 3.0 -> 2.0 and the behavior divider 1.4 -> 1.0: **these are intended, do not "restore" them.** Lifeline stems and interaction frames went 1.2 -> 1.5, which quantisation DOUBLES at dpi 1 (1.4 rounds to one device pixel, 1.5 floors to two) — that is the one most likely to read as a regression. **Partly guarded now, on two canvases:** the rendering gate compares ink on the behavior canvas (`light-cycle`) and, as of today, on the class canvas (`orders` — the class association edge, three card borders, the compartment rules). A stroke that quantises to a different number of device pixels fails those. Not covered: lifeline stems and interaction frames (a sequence canvas, which no fixture and no scenario opens — this is V1's own worst case and it is the uncovered one), group hulls, label leaders, and colour, which an ink mask throws away by construction. |
| V2 | Viewer font-size control | `6e4d3cb0` | human | Nobody has looked at this feature at all. The archived plan carries a 7-row verification table. Row 5 has an escape hatch: if the source editor's caret/IME geometry does not follow the scale, defer the source half rather than ship it half-scaled. **Human, and here is why no scenario helps.** The control (`font_size_control`, in the document header) does lay out, so a scenario could press it and watch a rect change size. That is not the question: the question is whether text at the new scale is *legible*, and both instruments this project has are blind to it — an ink mask says only that ink moved, and a rect says only that a box grew. Row 5's caret is the one state-shaped part of the row, and `waml-markdown-editor` paints it as `draw_caret` rather than mounting it, so it has no snapshot entry either. |
| V3 | UML extension row icons | `22672758` | human (state half already covered) | V1–V6 from the plan. Core OKF folders should draw the book glyph, UML packages the box glyph, **and every other glyph must be unchanged** — that last guarantee was silently false once already. **The state half is already covered, and was before today** — no scenario was written for it because one would only re-assert the model through a widget tree. `tree.rs`'s unit tests pin which `Icon` each row *selects*: `the_packages_fixture_draws_a_box_for_its_declared_package`, `the_bundle_root_node_boxes_a_uml_domain_top`, `the_root_node_draws_the_okf_bundle_root_glyph_for_an_undeclared_top`, and `tree_and_folder_view_agree_across_book_box_and_folder_glyphs`, which holds the tree and the folder view to the same `IconId` table so the two surfaces cannot disagree. What is left is purely a drawing question: does the book glyph read as a book at 16px, does the box read as a box, and are the others unchanged *as drawn*. Open `tests/fixtures/packages`, which exists for exactly this look. |
| V4 | Granular projection mask | `dc5f5fa8` | state, blocked | 8 visual checks from the plan, plus 2 medium review warnings. A disabled stage is a silent skip, so a wrong mask looks like a working view with content missing — the failure mode is invisible by construction. **Partly guarded:** `project_tree_lists_every_row_of_the_bundle` pins the default projection's exact row list, so a stage that starts dropping rows fails a test instead of looking fine. **Driving the projection menu is still not a harness operation, and it is now clear what it would take.** The control that OPENS the menu (`view_mode_btn` in the tree panel's header) is a real `IconButton` with a real rect, so that half is already reachable. The menu it opens is not: `MenuPopup` draws its rows rather than mounting them, exposes no `semantic_items`, and offers no keyboard path, so nothing can *choose* a stage. And a way to choose one would not be enough on its own — `Mini` has no marked items, so masking `hide` would change no rows. Menu semantic items **plus** a fixture built to have something a stage removes, and the 8 checks shrink to the ones about how the checklist looks. |
| V5 | Surface-routed navigation | `8d0e9a78` | state (partly) + human | The forcing case by hand: folder tab -> view source -> back, across surfaces. Plus 3 medium review warnings. **Partly automated now.** `a_route_across_surfaces_leaves_exactly_one_of_them_showing` crosses the surface boundary three times (canvas -> folder listing -> canvas -> raw source -> folder listing) and holds the centre to *exactly one* surface at every stop — which is the half that fails quietly, and has: `DocView`'s `show_*` family used to carry a hand-copied surface list per method, five of those copies never learned about `behavior_canvas_wrap`, and leaving a behavior tab left its canvas drawing underneath its replacement. A stale surface is *behind* the live one, so that bug is invisible to a screenshot as well as to a human. **The `back` in the forcing case is still owed, and cannot be automated at all.** View history has exactly two triggers — the caption's arrow pair and the mouse's fourth/fifth buttons — and the driver cannot send a thumb button while the caption band does not lay out headless (see the walls below). Nor can any assertion name the active *tab*: `DocTabs` draws its tabs into its own rects and exposes no semantic items, so the surface is the only observable for where a route landed. |
| V6 | Book mode, phase 1 | `8c1b8204` | human + state, blocked | A folder read as one continuous scroll: diagram embeds live and capped, open-full to a tab, the tree working as a TOC in both directions. 3 design calls also owed. **Mostly human, with one piece now cheap.** `expect_active_surface(Book)` exists and would settle "open-full lands on the book surface, and takes the centre from its siblings" in one line — but nothing can reach the book surface today, because routing a click to it needs a directory declaring `view: book` in its `index.md` and no fixture declares one. A fixture is the whole cost of that line. The rest of the row is not state: whether the scroll reads as continuous, whether an embed is capped at a sensible height, and whether the TOC feels like a TOC are all judgements. |
| V7 | Bundle search | `679c8994` | state (mostly done) + human | **Substantially automated now.** The `ui` scenarios settle palette sections, results-tab grouping (and that the results tab takes the centre from the canvas it was escalated over), find-strip scoping, and F3/Shift+F3 traversal with wrap. As of today `committing_a_hit_opens_its_document_and_selects_its_tree_row` adds the landing: commit the palette's top hit for a query that matches exactly one document, and the right document opens, on the right surface for its kind, with its tree row selected. That is the "did the selection land where the navigation said it would" half. **Still owed:** that the palette, results tab and find strip *look* right (spacing, section headers, the strip's placement), and the SCROLL half of `DocView::reveal` — whether the landed document is actually scrolled to put the hit in view. The scroll half is not blocked by the harness any more, it is blocked by the fixture: every document in `Mini` fits on one screen, so there is nothing a reveal could scroll. A document taller than the viewport would settle it. |
| V8 | Conflict list, grouped delete | `0944e0f` | human | The 4th `PopupRoot` surface: badge-open, focus, delete -> re-solve -> refresh-open. Hit rects are Turtle-derived and the trash is a drawn glyph rather than a child widget, so misalignment is the thing to look for. **Human, behind three separate walls.** The trash is drawn, so it has no snapshot entry and no rect to click; the conflict list is a popup surface that draws its rows and exposes no semantic items, so nothing can reach a row either; and neither `Mini` nor `Behavior` — the only two workspaces the harness stages — produces a conflict, so nothing opens the badge in the first place. Any two of those three would still leave it human. |
| V9 | Drag-to-place constraints | S1–S4 + VIZ | human | Grey no-go zones for existing constraints, the diagonal drop authoring BOTH placements, and "updating" rather than "adding" when you drop into grey. **Human.** The driver can drag (`try_drag_by`), but only from a widget, and a diagram node is drawn into the canvas surface rather than mounted — there is nothing to start the drag on. The grey zones are painted into the same surface. And the part of the row that is really at stake is the *preview* during the drag, which is temporal by nature. |
| V10 | Connector repair phases | `fd8f305f` | human | Two defects were fixed in the phases that run after A*. The sparse tests cannot see connector quality — this needs eyes on real diagrams. **Change-detection going forward, now on two canvases** (behavior and class). A repair phase that starts rewriting connectors fails the gate instead of shipping green; whether the new route is *better* is still a human's call, which is what this row is about. |
| V11 | Recents pinning | `e6e3733` | human | Per-row pin on the start screen. The pin hit-test must gate on the same verdict as the draw, which is the bug that was fixed last time. **Human, and structurally so:** the row this is about is a drawn pin, and "does the hit rect agree with the drawn rect" is the one question a snapshot cannot ask, because the drawn thing has no snapshot entry to compare against. `StartScreen` exposes no semantic items either, so even the row list is out of reach. |
| V12 | Docked collapsible panels | `2731490` | human | **Partly obsolete — read this before verifying.** The Peek state machine was deleted as dead on 2026-08-21 (`475a3959`), so peek-slide feel and the ~600ms auto-collapse are no longer things that exist. What remains to verify: flag legibility, pin reflow, and both panels pinned at once. Two known by-design nits, not bugs: a `FLAG_W` (28px) edge gutter when pinned, and the flag label being a per-glyph vertical stack rather than rotated text. **Human.** Flag legibility is a drawing question by definition. Pin reflow and both-panels-pinned are geometry, and `app/tests/shell.rs` already pins slot/host/body geometry and both splitter drags at unit level — what it cannot do is tell you the result looks right. The `[T]` toggle that drives the tree column lives in the caption band, so no headless scenario can press it either. |
| V14 | Connector quality repairs | `90ffcf0f` | human | Five real router defects fixed, and two goldens moved with them. Look at: the activity fixture's `Retry`/`Check` back-edge pair, which should now read as two distinct lines rather than one; the state-machine's `Active` self-loop, now 24px off its border instead of 16 — confirm it clears the frame and any label. Then a wide fan-in: `hub_spread` now sometimes rebuilds a connector as a 5-point shape instead of a 3-point one, which is provably sound but a taste call. Endpoints also slide along their border during `nudge`, which should read as better fan-out but interacts with arrowheads and label placement that no test sees. 52 shared-run overlaps remain on dense synthetic scenes (none on authored fixtures) — whether any is visible at render size is a judgement no assertion can make. |
| V15 | Dock chrome extraction (`DockChrome`) | branch `a15-dock-chrome-extraction` | human | A **pure state move**, not a feature: the eleven dock/caption fields left `App` for `app::dock_chrome`. No widget changed parent and no `script_mod` order changed, and `app/tests/shell.rs` still pins slot/host/body geometry, both splitter drags and the toggle-seat continuity. Three things those tests structurally cannot see. **(a) The narrow-mode dismiss gesture.** The `MouseMove` arm of `route_narrow_dock_pointer` was deleted — it only wrote `pointer_in_narrow_dock`, which nothing read. Shrink the window under 640px, open the tree panel so it floats, and tap the canvas: the panel must still dismiss, and tapping *inside* the panel must not. **(b) The toggle handoff, animated.** Tests seat `DockMotion` at its end states, so no test has ever run the 180ms curve. Drag the window across 640/680 with the tree column open and watch the `[\|]` toggle slide out to the column edge and back to the row head — it must stay continuous, with no jump of the button's own 32px width at the handoff. **(c) The chrome seam break.** `sync_chrome_seam`'s change guard moved into `DockChrome::commit_seam_break` unchanged; confirm the hairline under the caption still breaks at the active tab card and follows it when you switch tabs. **All three are human, each for its own reason.** (a) needs a window narrower than 640px and the driver has no resize operation at all. (b) is a 180ms curve — temporal by definition. (c) is a hairline drawn into the caption band, which does not lay out headless, next to a tab strip that has no snapshot entry. |
| V17 | Deferred-navigation trio (`DeferredNavigation`) | branch `worktree-agent-a2df94ac496240a18` | human | A state move with **one deliberate behaviour change**, and the change is the reason for the row. The three things a navigation can still owe once its target tab draws — the `#fragment` a link asked to scroll to, the search hit a results row asked to reveal, the anchor a history traversal is restoring — became one `app::deferred::DeferredNavigation`, and the type makes them share one rule: a deferred apply is abandoned if some *other* document is what draws. Reveal and anchor restore already behaved that way. **The fragment did not** — it returned early without clearing, so it stayed armed and fired the next time its document became active, which could be an unrelated later visit. That path is gone. **Drive three cases.** (a) *The ordinary one still works:* click a markdown link carrying `#section` to another document — it must open and scroll to that heading, status bar clear. (b) *The missing one still reports once:* same, with a `#section` that does not exist — "Section not found: …" appears exactly once and does not come back on the next draw. (c) *The abandoned one no longer surprises:* follow a `#section` link and switch tabs before it lands, then reopen that document some other way (a tree row click) — it must **not** jump to the heading, and must not show a stale "Section not found". Before this change it did both. **Human, for two reasons.** (c) is a sub-frame race: the mismatch needs the user to move between the transition and the very next draw, and headless the two are one synchronous call — the unit tests can only hand-arm the state, which is exactly what they do. And (a) is V7's unsolved half: every document in `Mini` fits on one screen, so no fixture has anything a fragment could scroll. |
| V13 | Splash logo pulse variants | `67bd394`, **branch `ui-tweaks-2`, NOT on main** | human (a choice) | Not a sign-off — a **choice**. Six variants are built and rendering; the owner picks one and the rest get deleted. `cargo run -p waml-editor --bin logo_pulse_harness` stacks all six, top to bottom modes 1..6. |
| V16 | Inspector element-picker item identity | branch `worktree-agent-a4bc54d355b193180` | human | A **defect fix with one widget-identity side effect**, not a feature. The picker's synthetic `"src->tgt#N"` edge key is gone; rows carry a `Subject` and their `SelectBox` item ids are now *positional* (`picker_item_id(idx)`) rather than hashed from the row's key, and member-card `FlatList` ids key on the row's label rather than its node key. Every projection is unit-tested and the row order is unchanged, so nothing should move — but item ids are what makepad reuses widget state by, so this is the one thing tests cannot see. Open a diagram's element picker: every row must still be pickable and land on the element its label names, the highlighted row must follow the canvas selection, and scrolling the flyout must not leave a row wearing another row's lead glyph. Then open a group's member cards and check the same. **The forcing case, if you want one:** a diagram with a group named exactly after one of its nodes — those two rows used to hash to the same id, so the group row selected the node; now they are distinct. |

## What a headless scenario cannot reach in this editor

Five structural walls, found by trying. They are worth knowing before
writing a scenario, and several rows above are behind one of them rather
than behind "this is about pixels".

1. **The caption band does not lay out under the headless backend.** The
   editor's chrome is a custom caption bar (`main_window.caption_bar`), and
   headless it never draws: `caption_col`, `title_row`, `doc_tabs`, the
   burger, the search button, the `[T]` tree toggle and both history arrows
   all appear in the widget snapshot with `visible: true` and a rect of
   `0x0`, while `center_column` starts at `y = 0` — the band occupies no
   height at all. A locator refuses a widget with no rect, so every control
   mounted in the caption is unclickable, however visible it claims to be.
   (V5, V12, V15.)
2. **`FlatList` pools its items out of the window.** The results tab's hit
   rows are real `SearchResultRow` widgets, not semantic items — and still
   unreachable, because a pooled item reaches the snapshot with an empty
   `window_id` and a `0x0` rect. Readable, unclickable. (V7; the palette's
   keyboard path is the way in.)
3. **Popup surfaces draw their rows and offer no keyboard path.**
   `MenuPopup` and the conflict list mount no children and expose no
   `semantic_items`, so a row in either has nothing for a selector to
   resolve and no chord to arm it. The palette is the exception — it is
   keyboard-driven, which is why search is the most automated row here.
   (V4, V8.)
4. **The driver cannot resize the window.** There is no resize operation, so
   narrow mode (under 640px) is unreachable. (V15a.)
5. **Windows headless renders nothing.** `HeadlessLoadedModule::load` is
   `#[cfg(unix)]` in the pinned fork, so on Windows every shader compiles,
   none loads, and every capture is a flat grey rectangle. State assertions
   run fine; pixels are Linux-only. Anything pixel-based must be recorded
   from CI.

## What the rendering gate covers, and what it does not (audit row A05)

There are now two automated gates on pixels, both in
`crates/waml-editor/tests/ui.rs`:

* `the_light_cycle_canvas_is_drawn_the_way_its_reference_was` — the
  **behavior** canvas: transition routes, a self-loop, a long back edge,
  state boxes.
* `the_orders_canvas_is_drawn_the_way_its_reference_was` — the **class**
  canvas: one class association edge, three class cards with their
  compartment rules, an abstract title and a stereotype.

Each drives the headless editor to a document, crops to the diagram
surface, and compares the canvas against a stored **ink mask** — one bit per
pixel, background or not. They settle geometry: where connectors run, how
thick a stroke is once quantised, where glyphs sit and how big they are.
They settle nothing about colour, antialias quality, or whether a route
reads *well*.

**They close no row in this ledger, and the honest reason is worth stating.** Every
row here is an obligation to look at something that already shipped, and a
reference recorded today records what shipped — including whatever is wrong
with it. A gate can stop the NEXT change from going unseen; it cannot see
the last one. What it changes is which of these rows can regress silently
again:

| Row | After the gate |
|---|---|
| V1 Diagram pens | **Partly guarded going forward, on two canvases.** A stroke that quantises to a different number of device pixels fails the gate — which is exactly V1's stated worst case. The class canvas added today covers the deliberate 3.0 -> 2.0 class edge and the card compartment rules. Lifeline stems and interaction frames — the *most* likely regression in the row — are on a sequence canvas that no fixture and no scenario opens, and the colour half of V1 is not covered at all: an ink mask throws colour away by construction. Sign-off still owed. |
| V10 Connector repair phases | **Change-detection going forward.** A repair phase that starts rewriting connectors fails the gate instead of shipping green. The gate hands a human two drawings and a red/green overlay; whether the new one is *better* is still the human's call, which is what V10 is really about. Sign-off still owed. |
| V14 Connector quality repairs | Same as V10, and this is the change the gate was built against: `regression_proof` in `waml-ui-test` inks `90ffcf0f`'s before and after route sets from the repo's own history and shows the comparator fails on them by six times its tolerance. Sign-off still owed — nobody has looked at the fixtures. |
| V2, V3, V4, V5, V6, V7, V8, V9, V11, V12, V13, V15, V16, V17 | **Not covered.** Everything the gate compares is inside one diagram canvas. Tree row icons (V3), the projection menu (V4), navigation across surfaces (V5), book mode (V6), the palette and find strip (V7), popup surfaces (V8), drag preview (V9), start-screen pins (V11), dock flags (V12), the splash variants (V13), dock chrome (V15) and the deferred-navigation trio (V17) are all outside the crop, or are about temporal feel, or are hit-testing against drawn rather than laid-out things. No mask can reach any of them. |

**One reference is outstanding.** Linux is the platform of record (wall 5
above), and the `orders` reference added today could not be recorded from a
Windows machine, so none shipped with it. Until one is committed that gate
records its own capture and passes advisory, saying so in its trace. The
first Linux CI run uploads the recording as the `rendering-gate-<run id>`
artifact; download it, commit
`recorded-references/orders.linux-x86_64.ink` into
`crates/waml-editor/tests/references/`, and the second gate starts
enforcing too. `light-cycle.linux-x86_64.ink` went through exactly this and
is already enforcing. `crates/waml-editor/tests/README.md` has the recipe.

## Why this keeps happening

The automated plan runner cannot do visual verification, so a plan whose
final task is "look at it" either stalls or lands unverified. The runner
takes the second path, which is right — but the obligation then has nowhere
to go except a human's memory. This file is that place.

A plan that ends in a visual check should add its row here as its last task,
in the same commit that lands the feature.

## Which half is state

Before adding a row, check whether the obligation is really about pixels.
Several of the entries above are not: "did the tree lose a row", "did the
cursor wrap at the end of the hit list", "did the selection land where the
navigation said it would", "is this surface the only one showing" are
questions about *state*, and `crates/waml-ui-test` can settle those against
a real headless editor without a human in the loop. Its crate docs carry the
standing line between what it can decide and what it cannot; V4, V5 and V7
above are each partly discharged that way, and V3 turned out to have been
discharged by unit tests before it was ever written down.

A row belongs here when the claim is about how something was *drawn* —
stroke weight, glyph identity, spacing, legibility, feel — or when the thing
being hit-tested is drawn rather than laid out as a widget, so it has no
snapshot entry to assert against, or when it is behind one of the five walls
above. Those are the rows that will still be here next year, and that is
fine. What is not fine is a state question sitting in this file because
writing the scenario looked like more work than looking once.

**And one thing a scenario can never do for a row here.** Every obligation
above is "look at whether this shipped correctly". A test written today
pins what shipped, whatever that is. So a new scenario moves a row from
"could regress again unseen" to "cannot", and never from OWED to signed
off. Where a row below says "partly automated", read it as exactly that.
