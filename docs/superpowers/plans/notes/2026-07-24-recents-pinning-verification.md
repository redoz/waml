# Recents pinning + VS-tight rows — visual verification (2026-07-24)

Plan: `2026-07-24-recents-pinning-vs-rows.md`, Task 5. The cargo/clippy gate never
boots the script VM, so everything below was checked by launching the
worktree's own build (`target/debug/waml-editor.exe`, built directly — not via
`scripts/run-native.ps1`, to keep full control of the launched pid) and
screenshotting the window by its specific process id (never by window name,
never killing by process name — both captured pids, 61276 and 57824, were
stopped individually by `Stop-Process -Id <pid>`).

Verification used a hand-crafted `~/.waml/editor.json` with 7 fake recents
(pointing at scratch directories under the session scratchpad, so
`prune_missing`'s `path.exists()` check keeps them) plus one real fixture
already on disk. One entry (`Model Four Pinned`) carried a hand-seeded
`pinned_at`. The **real** `~/.waml/editor.json` was backed up before this and
restored byte-for-byte afterward (diffed clean) — no user data was lost, though
note the caveat below about concurrent writers.

## Checked headlessly (Steps 1, 2, 4) — verdict per check

- **Row anatomy (Step 2):** confirmed `[Package glyph] [title / path tight] [when, right] [pin]`
  exactly as specced. Zoomed crop shows the left glyph, the title/path stacked
  with no loose gap (the `-2.0` spacing pulls them tight), the `when` stamp
  right-aligned on the title row, and the pin anchored at the row's right
  edge. PASS.
- **Fonts (Step 2):** title renders in the heavier `text_label` weight, path in
  the smaller/dimmer `text_menu` weight — visually distinct as specced. PASS.
- **Pinned-state render (Step 3, the provable half):** the seeded
  `pinned_at` row rendered with a persistently-visible **accent-blue** pin
  icon (not needing hover, since `pinned` forces the pin visible per
  `recent_row.rs` draw_walk) and sorted into the top block **ahead of** a
  more-recently-opened unpinned entry — proving both the accent tint and the
  pin-block-first sort work end to end. PASS.
- **5-row box fit (Step 4):** FAILED on the first pass, FIXED, re-verified
  PASS. See "Defect found + fix" below.
- **Scroll reachability (Step 4):** a synthetic mouse-wheel event (not a
  hover/click gesture — a scripted wheel delta, in bounds for headless
  verification) scrolled the 8-row list from rows 1-5 to rows 4-8 with no
  clipping or overscroll artifacts at the bottom. PASS.

## Defect found + fix: `ROW_HEIGHT` was undersized

The plan's original `RecentRowView::ROW_HEIGHT = 30.0` assumed a single
16px-glyph-driven row pitch. The real row is a **two-line** `text_label` /
`text_menu` text stack plus the 2x3 padding, which renders far taller. The
first screenshot (`ROW_HEIGHT = 30.0`) showed only ~2.3 rows fitting in the
five-row box (a screenshot mismatch per the plan's own rule: "fix it... do not
lower the bar to close the task"). Measured the real per-row pitch from a
zoomed crop (~60-62px between successive row title baselines at 96 DPI / 100%
Windows display scale) and retuned:

```
pub const ROW_HEIGHT: f64 = 61.0; // was 30.0
```

Re-verified: exactly five full rows visible, no partial sixth peeking in, no
fifth-row clipping, at `box_h = 5 * 61.0 + 2 * 5.0 = 315.0`.

## Owed interactive sign-off (pointer-driven — NOT faked or skipped, genuinely deferred)

The human is AFK for this run; a real pointer is required for these. Per the
plan's unattended-run rules, they are recorded here rather than synthesized:

- **Hover reveal:** hovering an *unpinned* row should fade its pin in from
  hidden (only pinned rows are always-visible here); not driven by a real
  pointer in this run.
- **Pin-hover tint:** hovering the pin specifically (vs. just the row) should
  light it to accent even when the row itself is unpinned.
- **Pin-click-does-not-open-the-model:** clicking the pin anchor must toggle
  the pin and must NOT also fire `Clicked`/open the recent (the code hit-tests
  the pin's `FingerUp` before the row body's, per `recent_row.rs`, but this is
  unverified with a live pointer).
- **Live re-sort on toggle:** clicking a pin should persist + reload the start
  screen so the row visibly jumps into (or out of) the pinned top block
  without restarting the app.

## Caveat: shared `~/.waml/editor.json` under concurrent activity

While this verification was in progress, a real recent entry (`Mini`,
pointing at another worktree's fixture) was written into the live
`~/.waml/editor.json` by what appears to be a *different*, concurrently
running `waml-editor` process (plausible given the number of parallel
plan-worker worktrees active this run) — its `opened_at` landed between the
two screenshots taken here. This did not affect the verification (the row
anatomy/sort/pin checks above are independent of it), and the file was
restored from a pre-verification backup at the end, but it's worth noting for
anyone else scripting against the same shared config path: treat `~/.waml/`
as a machine-shared resource, back it up before mutating it, and expect it to
move under you.
