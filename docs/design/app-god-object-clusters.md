# `App`: the field clusters, and the order to extract them

Audit finding **A15 ("App shell")**: *"54-field `App` god object, impl across 7
files/~7,800 lines, linear per-feature accretion."*

The line count is not the problem. The problem is that every new feature has
exactly one obvious place to put its state, so it keeps going there — and the
next feature after that. This file exists so the next person does not have to
re-derive which fields belong together before they can move any of them.

## Method

Every `self.<field>` reference in `app.rs` and the six `app/*.rs` impl files was
counted per file. Fields that are read and written by the same code, in the same
file, at the same points in the frame, are a type trying to exist. Fields spread
evenly across all seven files are not — they are the app shell itself.

Counts below are `self.<field>` references in the seven impl files at the time of
writing (test files excluded). The seven files are `app.rs` (1,639),
`actions.rs` (2,274), `event.rs` (403), `menus.rs` (483), `navigation.rs`
(1,007), `shell.rs` (980), `workspace.rs` (1,061).

## The clusters

| # | Cluster | Fields | Refs | Lives in | Status |
|---|---|---|---|---|---|
| C1 | **Dock chrome** | 11 | 54 | `shell` (44), `workspace` (7), `actions` (3) | **extracted** → `app::dock_chrome::DockChrome` |
| C11 | Core shell | 9 | 242 | everywhere | stays on `App` — this *is* the app shell |
| C2 | Search sessions | 5 | 30 | `actions` (17), `event` (4), `navigation` (3), `app` (2), `workspace` (2) | **partly extracted** (`SessionSearch`) |
| C4 | Web / remote boot | 5 | 22 | `app` (16), `workspace` (6) | open |
| C6 | View history + deferred navigation | 5 | 28 | `navigation` (25), `actions` (2), `workspace` (1) | **partly extracted** (`DeferredAnchorRestore`) |
| C5 | Open project + save | 4 | 28 | `workspace` (24), one each in `app`/`event`/`shell`/`actions` | **extracted** → `app::open_project::OpenProject` |
| C7 | Projection + tree cache | 4 | 27 | `navigation` (18), `workspace` (3), `actions` (3), `shell` (1) | **extracted** → `app::projection::Projection` |
| C3 | Command palette | 3 | 8 | `actions` (8) | **extracted** (`OpenPalette`) |
| C8 | Zoom | 3 | 8 | `actions` (8) | **extracted** (`zoom::Zoom`) |
| C9 | Agent window marks | 3 | 9 | `shell` (6), `app` (2), `workspace` (1) | **extracted** (`AgentMarks`) |
| C10 | Context-menu subject | 2 | 5 | `actions` (5) | **extracted** (`ContextMenuSubject`) |

### C1 — Dock chrome (11 fields) — **extracted**

`narrow`, `pointer_in_narrow_dock`, `dock_layout`, `dock_widths`,
`dock_rubber`, `tree_motion`, `inspector_motion`, `dock_next_frame`,
`tree_btn_slot_w`, `seam_break`, `tree_toggle_mounted`.

Everything the two docked columns and the caption chrome that tracks them
remember between frames. Now `App::dock: DockChrome`, in
`crates/waml-editor/src/app/dock_chrome.rs`; the invariants that hold over the
group are stated in that module's docs.

### C2 — Search sessions (5 fields) — **partly extracted**

`search`, `find`, `session_search`, `stepped_session_index`, `pending_reveal`.

The bundle index plus the two live cursors over it — the Ctrl+F document-scoped
one and the bundle-wide F3 one — and the reveal that is waiting on a tab to
draw. `stepped_session_index` exists only because the cursor cannot be
re-derived from a landing; that is the invariant a type would carry.

**The coupled pair is extracted** as `app::session_search::SessionSearch`:
`step` stamps the index it lands on as part of advancing, and `mark_landing`
takes the stamp before it looks at whether a session is live. Held apart,
`end_session_search` cleared the session and left the stamp behind — dead
today only because the one producer and the one consumer sit inside a single
synchronous call, which you could only learn by reading the whole chain.

**The other three should NOT be forced together.** They are three different
kinds of thing that happen to be about searching:

* `search` is the bundle-wide `SearchState` *index* — an engine, like
  `session` and `documents`. Every search surface reads it. It belongs with
  C11, not with the cursors over it.
* `find` is already one well-shaped `Option<SearchSession>` for the Ctrl+F
  strip, document-scoped and deliberately independent of the bundle-wide one.
  There is no second field to couple it to.
* `pending_reveal` is not a search session at all; it is a deferred apply, and
  its siblings are `pending_fragment` and `anchor_restore`. See "Why C6's
  remainder is not a type" above for the move those three want.

### C3 — Command palette (3 fields)

`palette_sections`, `palette_query`, `palette_hidden`.

The state of ONE open Ctrl+K palette. All three are set together on open, read
together on a row commit, and meaningless while it is closed — so they are one
`Option<OpenPalette>`, not three fields. Covered by `app/tests/palette.rs`.

### C4 — Web / remote boot (5 fields)

`api_backend`, `pending_boot_bundle`, `pending_boot_index_hash`,
`pending_api_boot`, `pending_api_save`.

Four in-flight-request slots and the backend one of them commits. Every one is
`#[cfg(target_arch = "wasm32")]` in practice (all five carry a
`cfg_attr(..., allow(dead_code))` for the native build).

### C5 — Open project + save (4 fields) — **extracted**

`open_dir`, `open_name`, `save_timer`, `save_feedback`.

Where the open bundle came from, what to call it, and the debounce/feedback of
writing it back. Now `App::project: OpenProject`, in
`crates/waml-editor/src/app/open_project.rs`.

The cluster's prediction held: opening, replacing and closing all have to touch
every one of the four, and each of the three touched a different subset.
`open_bundle` cancelled the debounce and reset the error but did not set the
name (a separate statement twenty lines later did); `open_dir` set the
directory and nothing else; `close_model` cancelled the debounce and cleared
the directory but **left the closed project's name behind**. Nothing read that
stale name, which is why it survived — with the fields apart there was no one
place where "abandon the project" was written down. `OpenProject::close` is
that place now.

Note `dir` and `name` stay two fields rather than collapsing into one
`Option`: a browser session has a bundle decoded from the URL fragment, so it
has a real name and no directory. `dir.is_none()` does not mean "nothing is
open".

`prevent_quit_after_failed_save` and `should_flush_save` stayed free functions
in `workspace.rs`: they are over `Event` and `Result`, and touch none of these
fields.

### C6 — View history + deferred navigation (5 fields)

`view_history`, `pending_fragment`, `pending_anchor_restore`,
`anchor_restore_generation`, `history_controls_visible`.

**The last two are extracted** as `navigation::DeferredAnchorRestore`: scheduling
bumps the generation and stamps the new restore with it, applying compares. Held
apart, "bump then stamp" was two statements in the right order and nothing said
the stamp had to come from the bump.

The back/forward stack, the three things a navigation can still owe once the
target tab draws, and the generation counter that tells a superseded restore to
give up. `history_controls_visible` is the caption-side mount guard for the
history pair.

#### Why C6's remainder is not a type

The row above says C6 is worth finishing. Having looked at it with C7 done: the
remaining three should **not** move as a group, and the map was wrong to say
they should.

* `view_history` is `crate::view_history::ViewHistory`, which is already a
  type, with **nine** public methods that `app/navigation.rs` calls. Wrapping
  it so it can carry one `bool` means writing nine forwarding methods to hold
  one field. That is more code and one more indirection to buy nothing; the
  `#[rust] view_history: ViewHistory` line is not the god-object problem.
* `history_controls_visible` is a "last applied" write guard for the two
  caption buttons — the same shape as `DockChrome::tree_btn_slot_w`, but it
  guards the *history pair*, which `DockChrome` has nothing to do with. It is
  one bool with two references, both adjacent, in one function.
* `pending_fragment` is not history at all. It is a deferred apply, and its
  real siblings are `pending_reveal` (listed under C2) and the already-extracted
  `anchor_restore` — all three are "what a navigation still owes once the target
  tab draws", and all three are applied together by `handle_draw_restores`.
  **That** is the type here, cutting across C6 and C2 rather than along them.
  Worth doing as its own move, with one caveat found while reading it: the doc
  on `apply_pending_reveal` claims it clears "same as `apply_pending_fragment`",
  and it does not — `apply_pending_reveal` uses `take()` and drops a reveal that
  lands on the wrong tab, while `apply_pending_fragment` returns early and keeps
  waiting. Decide which one is right *before* unifying them, because a type
  that makes them share a rule will pick one.

### C7 — Projection + tree cache (4 fields) — **extracted**

`nav_state`, `projection_mask`, `nav_tree`, `chain_limits`.

The tree's scope, the session-wide middleware mask, the descent cap, and the
memoised build. `nav_tree`'s cache key was literally `(revision, mask, chain
cap)` — the type was already written down, as a tuple. Now
`App::projection: Projection`, in `crates/waml-editor/src/app/projection.rs`.

The key and the build now live in one private function
(`Projection::ensure_tree`), which closed a latent staleness: the key stored
`limits.max_depth`, not `limits`, so it was correct only while `ChainLimits` had
exactly one field. A second field would have produced a silently stale tree — a
working view with content missing. It keys on the whole `ChainLimits` now.

`nav_state` survived the move but is worth flagging: **nothing in the shipping
UI ever sets a scope other than the root.** The only non-default writes are in
`app/tests/navigation.rs`, which set one by hand to assert that a navigation
does not clobber it. `nav::view_of`'s scoping is real and tested; the control
that would drive it was never built. Kept because the tree panel's scope title
reads through the same path, and deleting it would delete a capability rather
than dead state — but a future reader should know it is currently inert.

### C8 — Zoom (3 fields)

`wheel_zoom`, `wheel_zoom_target`, `zoom_state`.

A wheel-delta accumulator, the target it is banking for, and the live percent
per target. The accumulator resets when the target changes; that coupling is the
whole reason the second field exists.

### C9 — Agent window marks (3 fields)

`agent_badge`, `agent_tint`, `agent_row_w`.

`--title` / `--color` launch flags, retained so a theme live-edit reload can
re-push them, plus the last-pushed row width. Note `agent_row_w` is the same
"last applied, guard the write" shape as C1's `tree_btn_slot_w`.

### C10 — Context-menu subject (2 fields)

`node_menu_key`, `folder_menu_address`.

What the currently-open context menu is *about*, stashed at open time because
the committed menu id carries no subject. Two `Option<String>`s where at most
one is ever `Some` — an enum, mis-spelled as two fields.

### C11 — Core shell (9 fields) — stays

`ui`, `session`, `documents`, `markdown_assets`, `markdown_emphasis`,
`editor_shown`, `start_recents`, `fps_meter`, `capture_ready`.

The widget tree, the model, the open documents, and a handful of
genuinely-app-wide settings. `ui` (78 refs), `session` (83) and `documents` (55)
are read from six of the seven files each; that is not accretion, that is what
`App` is for. Do not chase these.

## Why C1 was extracted first

Best ratio of moved fields to risk, by some distance:

* **Biggest single cluster** — 11 of 54 fields, 20% of the god object, in one
  move.
* **Concentrated.** 44 of its 54 references are in one file (`shell.rs`); the
  other ten are in two files and are all one-liners. A cluster touched from all
  seven files would have been a bad first pick regardless of size.
* **It moves state, not widget ownership.** Not one widget changed parent, and
  no `script_mod` registration order changed — which is where a makepad
  refactor breaks invisibly.
* **It is the one UI cluster with real headless coverage.**
  `app/tests/shell.rs` mounts the production shell, *draws* the dock, and
  asserts slot/host/body rects wide and narrow, both splitter drags including
  the collapse-with-rubber and the reopen, and the toggle-seat continuity across
  the whole collapse. Those tests pin the extraction to observable geometry
  rather than to "it compiles".

Runners-up and why not:

* **C4 (web boot, 5 fields)** is equally concentrated but `cargo test
  --workspace` cannot execute a single line of it — it is all wasm-gated. A
  refactor whose only verification is a type-check is not a first move.
* **C6 (view history, 5 fields)** is the best-covered cluster in the file, but
  its 25 references in `navigation.rs` are threaded through the deferred-restore
  generation logic, which is the subtlest thing in the app module. Worth doing;
  not worth doing first.
* **C8 / C3 / C10 (3+3+2 fields)** are each a single-file, near-zero-risk move.
  Cheap, but they leave the shape of the problem untouched. All three are done,
  along with C6's and C2's coupled pairs. **What is left of C6** was expected to
  move with C7; it did not, and should not — see "Why C6's remainder is not a
  type" under C6 above.

Two fields did not survive the move, both write-only records that no code read:

* `pointer_in_narrow_dock` — residue of the `Peek` state machine deleted in
  `475a3959`. It was written on every narrow-mode mouse move and never read.
* `dock_next_frame` — the `NextFrame` handle from `cx.new_next_frame()`. The
  frame is requested by the call; storing the handle did nothing. The one test
  that read it was asserting "the motion is still running", which is
  `DockChrome::motions_active()`.

Both were invisible while `#[derive(Script)]` was generating a read of every
field on `App`. Moving a cluster into a plain struct is what surfaces them —
which is an argument for doing the rest.

## Recommended order for the remaining clusters

Cheap-and-safe first, so the pattern is established before the risky ones; the
untestable one last.

**Next one to take: the deferred-apply trio** — `pending_fragment`,
`pending_reveal` and the already-extracted `anchor_restore`, which cuts across
C6 and C2 rather than along either. See "Why C6's remainder is not a type"
under C6 above for the one question to settle first. After that, **C4 (item 9)**
is all that is left, and it needs a browser.

`App` is at 29 fields (28 `#[rust]` plus `ui`), down from the 54 the audit
found. C6's remainder and the rest of C2 are deliberately NOT extractions;
items 5 and 8 say why.

1. ~~**C8 Zoom (3)**~~ — **done.** `zoom::Zoom` holds the percent-per-target,
   the wheel accumulator and the target it banks for, because the reset-on-
   target-change coupling is the only reason the third field exists. The
   caller no longer has to remember to reset: `bank_wheel` takes an
   `Option<ZoomTarget>` and forgets the target itself when a surface has no
   zoomable chrome.
   The cheapest possible next move.
2. ~~**C3 Palette (3)**~~ — **done.** One file, one lifetime (open → commit →
   closed), so it collapsed to `Option<OpenPalette>`. Covered by
   `app/tests/palette.rs`.
3. ~~**C10 Context-menu subject (2)**~~ — **done.** One file, and the move
   *fixed* something: two `Option`s that must never both be `Some` became one
   enum.
4. ~~**C9 Agent marks (3)**~~ — **done.** Two files, nine references. No test
   coverage, but the behaviour is "a coloured pill in the caption", which a
   human verifies in one glance.
5. **C6's remainder (3) — DO NOT extract as a group.** Attempted and rejected;
   see "Why C6's remainder is not a type" below. Take C2 next instead.
6. ~~**C7 Projection + tree cache (4)**~~ — **done**, and taken *before* C6
   rather than after: it turned out to be the self-contained half of
   `navigation.rs` (mask, cap, cache, scope), where C6's remainder is the
   back/forward stack threaded through `transition_to_location`. Doing the
   independent one first kept both diffs readable.
7. ~~**C5 Open project + save (4)**~~ — **done.** `app/tests/workspace.rs`
   covers the free functions, `open_bundle`, and the close-then-reopen round
   trip; the native disk write itself is still only covered by a type-check, so
   a failing save by hand is worth doing before trusting it further.
8. **C2 Search sessions (5)** — spread over four files and the most entangled
   with `documents` and `ui`. Its coupled pair (`session_search` +
   `stepped_session_index`) is **done**; the remaining three are three
   different kinds of thing and should stay apart. See the C2 section.
9. **C4 Web / remote boot (5)** — last, not because it is hard but because
   nothing in `cargo test --workspace` executes it. Move it only in a session
   that can verify a `?bundle=` / `?api=` boot in a real browser.

C11 stays where it is.

## The rule this file is really for

`App` will grow another field the next time a feature lands. The question to ask
is not "does it fit on `App`" — everything fits on `App` — but **"which of the
clusters above does it belong to, and is that cluster a type yet?"** If the
answer is C1, it goes in `DockChrome` and `App` does not change. That is the
whole point of doing the first one.
