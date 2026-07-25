# Agent Window Marker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--title <text>` and `--color <hex>` launch flags to `waml-editor` so several
concurrently-running windows can be told apart by eye.

**Architecture:** `cli.rs` grows two parsed fields. One new widget, `AgentMark`
(`agent_mark.rs`), owns both marks: a colour wash across the caption's title row and a
right-floated text badge. It is mounted **zero-width** as the first child of `title_row`
so it consumes no layout space and shifts no existing caption widget, then draws with
`draw_abs` over a row width the App measures and pushes in. `App` holds the parsed values
and re-pushes them after every theme reload.

**Tech Stack:** Rust 2021, makepad (redoz fork, `script_mod!` DSL), `DrawColor` /
`DrawText` draw pens.

**Spec:** `docs/superpowers/specs/2026-07-25-agent-window-marker-design.md` — read it
first. This plan implements it exactly.

## Global Constraints

- **No inline `font_size:` / `FontMember` in `waml-editor` chrome sources.** A gate test
  (`chrome_typography_gate`) bans it. Use an existing `mod.fonts` role. This plan uses
  `fonts.text_caption` throughout and adds no font role.
- **Clippy runs with `-D warnings`** in the gate. That promotes `dead_code` to a hard
  error: do not leave an unused helper, field, or constant behind between tasks.
- **Never edit the main checkout directly** — work in a git worktree.
- **`sdf.box(..., 0.0)` degenerates and floods** on this fork. Any rounded-rect SDF must
  use a non-zero radius (this plan uses `4.0`).
- Colour values in draw pens are built with `vec4(r, g, b, a)` and read via `.x/.y/.z/.w`.
- Every new module must be added to `crates/waml-editor/src/main.rs`'s `mod` list **and**
  registered in `App::script_mod` (see Task 2 — order is load-bearing).

---

### Task 1: Parse `--title` and `--color`

Adds the two flags to the hand-rolled argv parser and fixes the blank-window error path
that a mistyped flag would otherwise land you in.

**Files:**
- Modify: `crates/waml-editor/src/cli.rs` (whole file — `Args`, `parse`, tests)
- Modify: `crates/waml-editor/src/app.rs` (`handle_startup`, ~line 1229)

**Interfaces:**
- Consumes: nothing (first task).
- Produces:
  - `pub struct Args { pub dir: Option<PathBuf>, pub diagram: Option<String>, pub badge: Option<String>, pub tint: Option<[f32; 3]> }`
  - `pub fn parse(argv: &[String]) -> Result<Args, String>` (unchanged signature)
  - `tint` is sRGB components in `0.0..=1.0`, **not** a makepad `Vec4` — `cli.rs` stays
    makepad-free so its tests need no `Cx`.

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block in `crates/waml-editor/src/cli.rs`:

```rust
#[test]
fn parses_title_flag() {
    let a = parse(&argv(&["waml-editor", "some/dir", "--title", "veil-fix"])).unwrap();
    assert_eq!(a.dir, Some(PathBuf::from("some/dir")));
    assert_eq!(a.badge.as_deref(), Some("veil-fix"));
    assert_eq!(a.tint, None);
}

#[test]
fn parses_color_flag_six_digit_with_hash() {
    let a = parse(&argv(&["waml-editor", "--color", "#ff0080"])).unwrap();
    let t = a.tint.unwrap();
    assert!((t[0] - 1.0).abs() < 1e-6);
    assert!((t[1] - 0.0).abs() < 1e-6);
    assert!((t[2] - 128.0 / 255.0).abs() < 1e-6);
    assert_eq!(a.badge, None);
    assert_eq!(a.dir, None);
}

#[test]
fn parses_color_flag_without_hash_and_mixed_case() {
    let a = parse(&argv(&["waml-editor", "--color", "FF0080"])).unwrap();
    assert!((a.tint.unwrap()[0] - 1.0).abs() < 1e-6);
}

#[test]
fn parses_three_digit_shorthand_by_doubling_nibbles() {
    // f0a -> ff00aa
    let short = parse(&argv(&["waml-editor", "--color", "f0a"])).unwrap().tint.unwrap();
    let long = parse(&argv(&["waml-editor", "--color", "#ff00aa"])).unwrap().tint.unwrap();
    for i in 0..3 {
        assert!((short[i] - long[i]).abs() < 1e-6, "component {i}");
    }
}

#[test]
fn both_flags_compose_with_dir_and_diagram_in_any_order() {
    let a = parse(&argv(&[
        "waml-editor", "--title", "opus-3", "some/dir", "--color", "#2b8", "--diagram", "Orders",
    ]))
    .unwrap();
    assert_eq!(a.dir, Some(PathBuf::from("some/dir")));
    assert_eq!(a.diagram.as_deref(), Some("Orders"));
    assert_eq!(a.badge.as_deref(), Some("opus-3"));
    assert!(a.tint.is_some());
}

#[test]
fn empty_title_is_accepted() {
    let a = parse(&argv(&["waml-editor", "--title", ""])).unwrap();
    assert_eq!(a.badge.as_deref(), Some(""));
}

#[test]
fn missing_flag_values_are_errors() {
    assert!(parse(&argv(&["waml-editor", "--title"])).is_err());
    assert!(parse(&argv(&["waml-editor", "--color"])).is_err());
}

#[test]
fn bad_hex_is_an_error_naming_the_value() {
    let e = parse(&argv(&["waml-editor", "--color", "zzz"])).unwrap_err();
    assert!(e.contains("zzz"), "error should name the bad value, got: {e}");
    assert!(parse(&argv(&["waml-editor", "--color", "#ff00"])).is_err()); // wrong length
    assert!(parse(&argv(&["waml-editor", "--color", "#ff00800"])).is_err()); // wrong length
    assert!(parse(&argv(&["waml-editor", "--color", ""])).is_err());
}

#[test]
fn absent_flags_leave_both_fields_none() {
    let a = parse(&argv(&["waml-editor", "some/dir"])).unwrap();
    assert_eq!(a.badge, None);
    assert_eq!(a.tint, None);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor --lib cli::`
Expected: FAIL — `no field 'badge' on type 'Args'` (compile error is a valid failure here).

- [ ] **Step 3: Implement the parse**

In `crates/waml-editor/src/cli.rs`, extend `Args`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Args {
    pub dir: Option<PathBuf>,
    pub diagram: Option<String>,
    /// Badge text from `--title`. Identifies which agent launched this window;
    /// purely cosmetic, never interpreted.
    pub badge: Option<String>,
    /// sRGB components in `0.0..=1.0` from `--color`. Deliberately not a makepad
    /// `Vec4`, so this module stays makepad-free and its tests need no `Cx`.
    pub tint: Option<[f32; 3]>,
}
```

Add the hex parser above `parse`:

```rust
/// Parse `#rgb` / `#rrggbb` (leading `#` optional, case-insensitive) into sRGB
/// components in `0.0..=1.0`. Alpha forms are rejected: the blend factors are
/// fixed by the design, so a caller-supplied alpha would have no meaning.
fn parse_hex(s: &str) -> Option<[f32; 3]> {
    let h: Vec<char> = s.strip_prefix('#').unwrap_or(s).chars().collect();
    let nib = |c: char| c.to_digit(16).map(|d| d as u16);
    let rgb: [u16; 3] = match h.len() {
        // `f` -> `ff`: doubling the nibble, i.e. * 17.
        3 => [nib(h[0])? * 17, nib(h[1])? * 17, nib(h[2])? * 17],
        6 => [
            nib(h[0])? * 16 + nib(h[1])?,
            nib(h[2])? * 16 + nib(h[3])?,
            nib(h[4])? * 16 + nib(h[5])?,
        ],
        _ => return None,
    };
    Some([
        rgb[0] as f32 / 255.0,
        rgb[1] as f32 / 255.0,
        rgb[2] as f32 / 255.0,
    ])
}
```

Extend `parse`'s match, and its two new locals:

```rust
pub fn parse(argv: &[String]) -> Result<Args, String> {
    let mut dir: Option<PathBuf> = None;
    let mut diagram: Option<String> = None;
    let mut badge: Option<String> = None;
    let mut tint: Option<[f32; 3]> = None;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = Some(argv.get(i).cloned().ok_or("--diagram requires a value")?);
            }
            "--title" => {
                i += 1;
                badge = Some(argv.get(i).cloned().ok_or("--title requires a value")?);
            }
            "--color" => {
                i += 1;
                let raw = argv.get(i).ok_or("--color requires a value")?;
                tint = Some(
                    parse_hex(raw)
                        .ok_or_else(|| format!("--color: not a hex colour: {raw}"))?,
                );
            }
            other if dir.is_none() => dir = Some(PathBuf::from(other)),
            other => return Err(format!("unexpected argument: {other}")),
        }
        i += 1;
    }
    Ok(Args {
        dir,
        diagram,
        badge,
        tint,
    })
}
```

Also update the doc comment on `parse` to list the new flags:

```rust
/// Parse `argv` (including argv[0]).
/// Usage: `waml-editor [<okf-dir>] [--diagram <name>] [--title <text>] [--color <hex>]`.
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml-editor --lib cli::`
Expected: PASS, including the pre-existing `parses_dir_only`, `missing_dir_is_ok`, and
`unknown_flag_is_still_an_error` tests.

- [ ] **Step 5: Fix the blank-window error path**

In `crates/waml-editor/src/app.rs`, `handle_startup` currently bare-`return`s on a parse
error, leaving a window with no chrome and no explanation — which a typo'd `--color`
would now make routine. Change:

```rust
        let args = match crate::cli::parse(&argv) {
            Ok(a) => a,
            Err(e) => {
                log!("{e}");
                return;
            }
        };
```

to:

```rust
        let args = match crate::cli::parse(&argv) {
            Ok(a) => a,
            Err(e) => {
                // Land on the start screen rather than a blank window: a bad flag
                // should cost you the flag, not the session.
                log!("{e}");
                self.show_start_screen(cx);
                return;
            }
        };
```

- [ ] **Step 6: Run the gate**

Run: `cargo test -p waml-editor` then `cargo clippy -p waml-editor --all-targets -- -D warnings`
Expected: both PASS. `badge`/`tint` are read by nobody yet but are `pub` struct fields,
so `dead_code` does not fire.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/cli.rs crates/waml-editor/src/app.rs
git commit -m "feat(cli): --title and --color flags for per-agent window marking"
```

---

### Task 2: `AgentMark` widget — registration, mount, colour helpers

Creates the widget, wires its module registration and its DSL mount, and adds the two
pure colour functions with tests. It draws nothing yet: nothing has set its marks.

**Files:**
- Create: `crates/waml-editor/src/agent_mark.rs`
- Modify: `crates/waml-editor/src/main.rs` (`mod` list)
- Modify: `crates/waml-editor/src/app.rs` (`App::script_mod` registration; `title_row` DSL)

**Interfaces:**
- Consumes: nothing from Task 1 yet.
- Produces:
  - `pub struct AgentMark` — a widget registered as `mod.widgets.AgentMark`
  - `pub fn label_ink(fill: Vec4) -> Vec4`
  - `pub fn wash(base: Vec4, tint: Vec4, amount: f32) -> Vec4`
  - `pub fn script_mod(vm: &mut ScriptVm) -> ScriptValue` (generated by `script_mod!`)
  - DSL node id `agent_mark`, reachable as `ids!(agent_mark)`

- [ ] **Step 1: Write the failing tests**

Create `crates/waml-editor/src/agent_mark.rs` containing **only** the tests plus the two
function signatures, so the test failure is a real assertion failure rather than a missing
file:

```rust
//! Placeholder — replaced wholesale in Step 3.

use makepad_widgets::*;

pub fn label_ink(_fill: Vec4) -> Vec4 {
    vec4(0.0, 0.0, 0.0, 1.0)
}

pub fn wash(base: Vec4, _tint: Vec4, _amount: f32) -> Vec4 {
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_ink_is_near_white_on_a_dark_fill() {
        let ink = label_ink(vec4(0.05, 0.05, 0.10, 1.0));
        assert!(ink.x > 0.9 && ink.y > 0.9 && ink.z > 0.9, "got {ink:?}");
    }

    #[test]
    fn label_ink_is_near_black_on_a_light_fill() {
        let ink = label_ink(vec4(0.95, 0.95, 0.90, 1.0));
        assert!(ink.x < 0.2 && ink.y < 0.2 && ink.z < 0.2, "got {ink:?}");
    }

    #[test]
    fn label_ink_weights_green_over_blue() {
        // Pure green is perceptually bright (0.7152 luma) -> dark ink.
        // Pure blue is perceptually dark (0.0722 luma) -> light ink.
        assert!(label_ink(vec4(0.0, 1.0, 0.0, 1.0)).x < 0.5);
        assert!(label_ink(vec4(0.0, 0.0, 1.0, 1.0)).x > 0.5);
    }

    #[test]
    fn label_ink_is_always_opaque() {
        assert_eq!(label_ink(vec4(0.5, 0.5, 0.5, 1.0)).w, 1.0);
    }

    #[test]
    fn wash_at_zero_is_the_base() {
        let base = vec4(1.0, 1.0, 1.0, 1.0);
        let got = wash(base, vec4(1.0, 0.0, 0.0, 1.0), 0.0);
        assert!((got.x - 1.0).abs() < 1e-6);
        assert!((got.y - 1.0).abs() < 1e-6);
        assert!((got.z - 1.0).abs() < 1e-6);
    }

    #[test]
    fn wash_at_one_is_the_tint() {
        let got = wash(vec4(1.0, 1.0, 1.0, 1.0), vec4(0.2, 0.4, 0.6, 1.0), 1.0);
        assert!((got.x - 0.2).abs() < 1e-6);
        assert!((got.y - 0.4).abs() < 1e-6);
        assert!((got.z - 0.6).abs() < 1e-6);
    }

    #[test]
    fn wash_interpolates_linearly_and_stays_opaque() {
        let got = wash(vec4(1.0, 1.0, 1.0, 1.0), vec4(0.0, 0.0, 0.0, 1.0), 0.15);
        assert!((got.x - 0.85).abs() < 1e-6, "got {got:?}");
        assert_eq!(got.w, 1.0, "wash must stay opaque: it replaces a chrome fill");
    }
}
```

Add `mod agent_mark;` to `crates/waml-editor/src/main.rs`, in alphabetical position —
directly after `mod action_link;` and before `mod app;`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor --lib agent_mark::`
Expected: FAIL — `label_ink_is_near_white_on_a_dark_fill` and the `wash` interpolation
tests fail on the stub returns.

- [ ] **Step 3: Write the widget**

Replace the whole of `crates/waml-editor/src/agent_mark.rs`:

```rust
//! Per-agent window marker: a colour wash across the caption's title row plus a
//! right-floated text badge, both driven by the `--title` / `--color` launch
//! flags so several concurrently-running editor windows can be told apart by eye.
//!
//! **Zero layout footprint.** Mounted `width: 0.0` as the FIRST child of
//! `title_row`, so it takes no space in that `flow: Right` row (the burger and
//! the model name keep their exact positions) and paints UNDER them rather than
//! gelling over them. Everything is drawn with `draw_abs` across a row width
//! `App` measures and pushes in via `set_row_width` -- the same
//! measure-and-push shape as `DocTabs::left_overshoot` (`doc_tabs.rs`, driven
//! from `app.rs`). `title_row`'s own `clip_x: true` bounds the result.
//!
//! A custom widget rather than a tinted `SolidView` because `View::draw_bg` is a
//! `DrawQuad` whose `color` is a shader *instance*: this fork exposes
//! `set_uniform` for uniforms only and has no `apply_over`, so a runtime-settable
//! colour needs a Rust-typed `DrawColor` pen -- the pattern `canvas.rs` already
//! uses for `draw_rule` / `draw_veil`.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.AgentMarkBase = #(AgentMark::register_widget(vm))

    mod.widgets.AgentMark = set_type_default() do mod.widgets.AgentMarkBase{
        // Zero width: reserves no space in the `flow: Right` title row.
        width: 0.0
        height: Fill
        // Theme-sourced bases, so a light/dark toggle re-supplies them on
        // reload and the marks follow the palette for free.
        wash_base: atlas.field_bg
        chip_fallback: atlas.selection
        ink_fallback: atlas.text
        draw_wash +: { color: #0000 }
        draw_chip +: {
            color: #0000
            pixel: fn() {
                // Radius must be non-zero: `sdf.box(.., 0.0)` degenerates and
                // floods on this fork.
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, 4.0)
                sdf.fill(self.color)
                return sdf.result
            }
        }
        draw_label +: {
            color: #FFF
            // The model name's own role. Reused deliberately: adding a role
            // would drag `fonts.rs`, `script_gate.rs` and `fonts_overlay.rs`
            // along with it.
            text_style: fonts.text_caption
        }
    }
}

/// How far the title row's fill moves toward the tint. Low enough that the
/// window still reads as Atlas in both themes, high enough to spot across a
/// monitor.
const WASH_AMOUNT: f32 = 0.15;

/// Chip padding and its gap from the row's right edge.
const CHIP_PAD_X: f64 = 6.0;
const CHIP_PAD_Y: f64 = 2.0;
const CHIP_RIGHT_GAP: f64 = 6.0;

/// Pick badge ink by the fill's relative luminance (Rec. 709), so any hex an
/// agent passes stays readable. Pure, so it is unit-tested without a `Cx`.
pub fn label_ink(fill: Vec4) -> Vec4 {
    let luma = 0.2126 * fill.x + 0.7152 * fill.y + 0.0722 * fill.z;
    if luma > 0.55 {
        vec4(0.06, 0.08, 0.11, 1.0)
    } else {
        vec4(0.98, 0.98, 0.99, 1.0)
    }
}

/// Blend `base` toward `tint` by `amount`. Alpha is forced opaque: this
/// replaces a chrome fill, so a translucent result would let the caption bar's
/// own colour through and halve the effect. Pure, so it is unit-tested.
pub fn wash(base: Vec4, tint: Vec4, amount: f32) -> Vec4 {
    vec4(
        base.x + (tint.x - base.x) * amount,
        base.y + (tint.y - base.y) * amount,
        base.z + (tint.z - base.z) * amount,
        1.0,
    )
}

#[derive(Script, ScriptHook, Widget)]
pub struct AgentMark {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    /// The title-row band.
    #[redraw]
    #[live]
    draw_wash: DrawColor,
    /// The badge pill.
    #[redraw]
    #[live]
    draw_chip: DrawColor,
    /// The badge text.
    #[redraw]
    #[live]
    draw_label: DrawText,

    /// `atlas.field_bg` -- what the title row is washed away FROM.
    #[live]
    wash_base: Vec4,
    /// Chip fill when `--title` was given without `--color`.
    #[live]
    chip_fallback: Vec4,
    /// Chip ink when `--title` was given without `--color`.
    #[live]
    ink_fallback: Vec4,

    /// `--title` text. `None` draws no badge.
    #[rust]
    badge: Option<String>,
    /// `--color`. `None` draws no wash.
    #[rust]
    tint: Option<Vec4>,
    /// Title-row width, measured and pushed by `App`. Zero until the row has
    /// been laid out once, which draws nothing; the next pass fills it in.
    #[rust]
    row_w: f64,
}

impl Widget for AgentMark {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {
        // Inert decoration: no hit rect, so it can never steal a click from the
        // burger or the caption's window-drag region.
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        // Zero-width turtle: this yields a rect with the row's origin and height
        // but no width, which is exactly the anchor the `draw_abs` calls below
        // need.
        let anchor = cx.walk_turtle(walk);
        if self.row_w <= 0.0 || (self.badge.is_none() && self.tint.is_none()) {
            return DrawStep::done();
        }
        let row = Rect {
            pos: anchor.pos,
            size: dvec2(self.row_w, anchor.size.y),
        };

        if let Some(tint) = self.tint {
            self.draw_wash.color = wash(self.wash_base, tint, WASH_AMOUNT);
            self.draw_wash.draw_abs(cx, row);
        }

        if let Some(text) = self.badge.clone() {
            let fill = self.tint.unwrap_or(self.chip_fallback);
            let text_w = self
                .draw_label
                .layout(cx, 0.0, 0.0, None, false, Align::default(), &text)
                .size_in_lpxs
                .width as f64;
            let chip_w = text_w + CHIP_PAD_X * 2.0;
            let chip_h = row.size.y - CHIP_PAD_Y * 2.0;
            let chip = Rect {
                pos: dvec2(
                    row.pos.x + row.size.x - chip_w - CHIP_RIGHT_GAP,
                    row.pos.y + CHIP_PAD_Y,
                ),
                size: dvec2(chip_w, chip_h),
            };
            self.draw_chip.color = fill;
            self.draw_chip.draw_abs(cx, chip);

            self.draw_label.color = if self.tint.is_some() {
                label_ink(fill)
            } else {
                self.ink_fallback
            };
            // Same seating constant the other hand-drawn caption text uses
            // (`diagram_switcher.rs`): centre minus half the ~12px line box.
            let text_y = chip.pos.y + chip.size.y * 0.5 - 6.0;
            self.draw_label
                .draw_abs(cx, dvec2(chip.pos.x + CHIP_PAD_X, text_y), &text);
        }

        DrawStep::done()
    }
}

impl AgentMark {
    /// Set both marks. `App` calls this at startup AND after every theme reload
    /// (`Apply::Reload` resets `#[rust]` state, which would otherwise silently
    /// un-mark the window the first time an agent presses `T`).
    pub fn set_marks(&mut self, cx: &mut Cx, badge: Option<String>, tint: Option<Vec4>) {
        self.badge = badge;
        self.tint = tint;
        self.redraw(cx);
    }

    /// Width of the title row this sits in. `App` measures it; the caller
    /// change-guards, so this always redraws.
    pub fn set_row_width(&mut self, cx: &mut Cx, px: f64) {
        self.row_w = px;
        self.redraw(cx);
    }
}

#[cfg(test)]
mod tests {
    // ... keep the tests from Step 1 verbatim ...
}
```

Keep the `mod tests` block from Step 1 exactly as written — do not weaken an assertion to
match the implementation.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml-editor --lib agent_mark::`
Expected: PASS, all seven.

- [ ] **Step 5: Register the module — order is load-bearing**

In `crates/waml-editor/src/app.rs`, `impl AppMain for App { fn script_mod(...) }`, add the
registration **before** the `App`'s own DSL consumes it. Put it directly after
`crate::conflict_badge::script_mod(vm);` and before whatever follows it, with this comment:

```rust
        // `AgentMark` must register before `App`'s own DSL, which mounts it as a
        // child of `title_row`: a module's DSL resolves `mod.widgets.*` eagerly
        // at `use`-time, not lazily, so an unregistered child silently becomes a
        // dead, invisible node whose setters no-op. Green tests and review both
        // miss it.
        crate::agent_mark::script_mod(vm);
```

- [ ] **Step 6: Mount it in the title row**

In `crates/waml-editor/src/app.rs`, add `use mod.widgets.AgentMark` to the `script_mod!`
`use` list at the top (alongside `use mod.widgets.IconButton`).

Then make `agent_mark` the **first** child of `title_row`, before `menu_btn`:

```
                            // Per-agent window marker (--title / --color). FIRST
                            // child and zero-width: it reserves no space in this
                            // `flow: Right` row, so the burger and model name do
                            // not move, and drawing first puts its wash UNDER
                            // them instead of gelling over them. It draws across
                            // the full row via `draw_abs` and an App-measured
                            // width (`sync_agent_row`), bounded by this row's
                            // `clip_x`.
                            agent_mark := AgentMark{}
                            menu_btn := IconButton{ ... unchanged ... }
                            model_name := Label{ ... unchanged ... }
```

Do **not** change `menu_btn` or `model_name` — no `width: Fill` on the label, no
re-nesting. Their positions must be byte-identical to before.

- [ ] **Step 7: Run the gate and confirm nothing moved**

Run: `cargo test -p waml-editor` then `cargo clippy -p waml-editor --all-targets -- -D warnings`
Expected: both PASS.

Then launch and screenshot: `./scripts/run-native.ps1`
Expected: the caption looks **exactly** as it did before this task — no wash, no badge,
burger and model name unmoved. The widget is mounted but has nothing to draw.

- [ ] **Step 8: Commit**

```bash
git add crates/waml-editor/src/agent_mark.rs crates/waml-editor/src/main.rs crates/waml-editor/src/app.rs
git commit -m "feat(caption): AgentMark widget, mounted zero-width in the title row"
```

---

### Task 3: Wire `App` — state, row measurement, reload survival

Connects the parsed flags to the widget and keeps them alive across a theme reload.

**Files:**
- Modify: `crates/waml-editor/src/app.rs` (`App` struct, `handle_startup`, `rehydrate`,
  and the per-frame sync alongside `sync_dock_slots`)

**Interfaces:**
- Consumes: `cli::Args { badge, tint }` (Task 1); `AgentMark::set_marks(cx, Option<String>, Option<Vec4>)`
  and `AgentMark::set_row_width(cx, f64)` (Task 2).
- Produces: `App::apply_agent_marks(&mut self, cx: &mut Cx)`, `App::sync_agent_row(&mut self, cx: &mut Cx)`.

- [ ] **Step 1: Add the state**

In the `App` struct in `crates/waml-editor/src/app.rs`, alongside the other `#[rust]`
fields:

```rust
    /// `--title` badge text, retained so a theme live-edit reload can re-push it
    /// (`Apply::Reload` wipes the widget's own `#[rust]` state).
    #[rust]
    agent_badge: Option<String>,
    /// `--color` tint, retained for the same reason as `agent_badge`.
    #[rust]
    agent_tint: Option<Vec4>,
    /// Last-pushed title-row width, so `sync_agent_row` only pushes on a real
    /// change (same guard shape as `dock_slot_w`).
    #[rust]
    agent_row_w: f64,
```

- [ ] **Step 2: Add the two methods**

Add to the same `impl App` block that holds `sync_dock_slots`:

```rust
    /// Push the launch-flag marks into `AgentMark`. Called at startup AND from
    /// `rehydrate`: the `T` theme toggle goes through `cx.request_live_edit()`
    /// -> `Apply::Reload`, which resets the widget's `#[rust]` state, so without
    /// the second call both marks vanish the first time an agent toggles the
    /// theme and the window silently becomes indistinguishable again.
    fn apply_agent_marks(&mut self, cx: &mut Cx) {
        let badge = self.agent_badge.clone();
        let tint = self.agent_tint;
        if let Some(mut mark) = self
            .ui
            .widget(cx, ids!(agent_mark))
            .borrow_mut::<crate::agent_mark::AgentMark>()
        {
            mark.set_marks(cx, badge, tint);
        }
    }

    /// Measure the title row and push its width to `AgentMark`, which draws
    /// across it with `draw_abs` (it is mounted zero-width, so it cannot learn
    /// the row width from its own turtle). Same measure-and-push shape as
    /// `sync_tree_gap` feeding `DocTabs::set_left_overshoot`.
    fn sync_agent_row(&mut self, cx: &mut Cx) {
        if self.agent_badge.is_none() && self.agent_tint.is_none() {
            return;
        }
        let w = self.ui.widget(cx, ids!(title_row)).area().rect(cx).size.x;
        if (w - self.agent_row_w).abs() <= 0.5 {
            return;
        }
        self.agent_row_w = w;
        if let Some(mut mark) = self
            .ui
            .widget(cx, ids!(agent_mark))
            .borrow_mut::<crate::agent_mark::AgentMark>()
        {
            mark.set_row_width(cx, w);
        }
    }
```

- [ ] **Step 3: Call them**

In `handle_startup`, immediately after the successful `parse` and **before** the
`match args.dir`, stash and apply:

```rust
        self.agent_badge = args.badge.clone();
        self.agent_tint = args
            .tint
            .map(|[r, g, b]| vec4(r, g, b, 1.0));
        self.apply_agent_marks(cx);
```

In `rehydrate`, as the **first** statement — before the `if !self.editor_shown` early
return, so the marks survive a theme toggle on the start screen too:

```rust
    fn rehydrate(&mut self, cx: &mut Cx) {
        // First, before the start-screen early return: the marks apply to both
        // screens.
        self.apply_agent_marks(cx);
        self.agent_row_w = 0.0; // force `sync_agent_row` to re-push after reload
        if !self.editor_shown {
```

Finally, call `sync_agent_row` each frame from the same place `sync_dock_slots` is
already called — the tail of `App`'s `handle_event`, `crates/waml-editor/src/app.rs:2549`
at the time of writing:

```rust
        // Push each panel's DockState-driven slot width onto its reservation
        // spacer every frame (including NextFrame, so the peek-timer's own
        // dock transitions are picked up promptly).
        self.sync_dock_slots(cx);
        // Same shape for the marker's row width: it is mounted zero-width, so
        // `App` is the only thing that knows how wide the title row is.
        self.sync_agent_row(cx);
```

- [ ] **Step 4: Run the gate**

Run: `cargo test -p waml-editor` then `cargo clippy -p waml-editor --all-targets -- -D warnings`
Expected: both PASS.

- [ ] **Step 5: First real visual check**

```powershell
./scripts/run-native.ps1 crates/waml-editor/tests/fixtures/mini
```
Expected: unchanged caption (no flags passed). Then run the binary directly with flags:

```powershell
./target/debug/waml-editor.exe crates/waml-editor/tests/fixtures/mini --title agent-a --color '#e91e63'
```
Expected: a pink-washed title row with an `agent-a` pill at its right end.

Capture by **specific pid** in one PowerShell call — never by process name, and never
`Stop-Process` by name: that kills the user's own editor session.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/app.rs
git commit -m "feat(caption): drive AgentMark from the launch flags, surviving theme reload"
```

---

### Task 4: `run-native.ps1` passthrough and full interactive sign-off

Makes the flags reachable from the normal launch script, then verifies the things the
headless gate structurally cannot see.

**Files:**
- Modify: `scripts/run-native.ps1`

**Interfaces:**
- Consumes: the `--title` / `--color` CLI flags (Task 1).
- Produces: `-Title` / `-Color` script parameters.

- [ ] **Step 1: Add the parameters**

In `scripts/run-native.ps1`, extend the `param(...)` block:

```powershell
param(
    [Parameter(Position = 0)]
    [string]$Fixture,
    [switch]$Empty,
    [switch]$Optimized,
    # Per-agent window marker: badge text and wash colour, so several
    # concurrently-running editors can be told apart by eye.
    [string]$Title,
    [string]$Color
)
```

Build the passthrough array after the existing `$profileArgs` line — `[string[]]` for the
same reason `$profileArgs` uses it (a bare `@()` unwraps to a scalar that splats
character-by-character):

```powershell
[string[]]$markArgs = @()
if ($Title) { $markArgs += @('--title', $Title) }
if ($Color) { $markArgs += @('--color', $Color) }
```

Then append `@markArgs` to **both** `cargo run` invocations:

```powershell
if ($Empty) {
    cargo run -p waml-editor --bin waml-editor @profileArgs -- @markArgs
}
else {
    if (-not $Fixture) { $Fixture = 'crates/waml-editor/tests/fixtures/mini' }
    cargo run -p waml-editor --bin waml-editor @profileArgs -- $Fixture @markArgs
}
```

Note the `-Empty` branch gains a `--` separator it did not have before; without it the
mark flags would be read by cargo, not by the editor.

- [ ] **Step 2: Interactive sign-off**

The gate is headless and cannot assert on drawn pixels, and the caption is this
codebase's known gate-blind failure class — a broken caption ships clean tests. All of
the following are mandatory. Launch two windows with different marks and check:

- [ ] The badge sits at the right end of the title row, clear of min/max/close.
- [ ] The burger and the model name have **not moved a pixel** versus an unflagged
      window. Compare screenshots directly.
- [ ] `menu_btn` still opens its drop-down on press.
- [ ] `tree_btn` still toggles the tree column.
- [ ] Doc tabs still hover, activate, and close.
- [ ] Dragging the caption still moves the window, and dragging **on the wash** does too
      (the mark must not have become a drag dead-zone).
- [ ] The wash does not dim or gel the burger glyph or the model name.
- [ ] A long model path still clips at the row edge without colliding with the badge.
- [ ] Pressing `T` toggles the theme and **both marks survive**, re-blended against the
      new `field_bg`.
- [ ] `--title` alone renders a legible Atlas-coloured chip and no wash.
- [ ] `--color` alone renders the wash and no chip.
- [ ] A no-dir launch (`-Empty -Color '#2b8'`) shows the wash on the start screen.
- [ ] A bad hex (`--color zzz`) logs the error and lands on the start screen — not a
      blank window.

- [ ] **Step 3: Commit**

```bash
git add scripts/run-native.ps1
git commit -m "feat(scripts): -Title/-Color passthrough for run-native"
```

---

## Verification summary

| Layer | Covered by |
|---|---|
| Flag parsing, hex forms, error cases | Task 1 unit tests (`cargo test -p waml-editor --lib cli::`) |
| Colour maths (ink pick, wash blend) | Task 2 unit tests (`cargo test -p waml-editor --lib agent_mark::`) |
| Widget registration order | Task 2 Step 7 launch — an unregistered child draws nothing |
| Layout non-regression, caption interactivity, theme survival | Task 4 Step 2 interactive checklist |

The gate for every task is `cargo test -p waml-editor` plus
`cargo clippy -p waml-editor --all-targets -- -D warnings`, both green.
