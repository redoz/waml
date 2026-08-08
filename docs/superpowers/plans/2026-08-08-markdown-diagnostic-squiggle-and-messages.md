# Markdown Diagnostic Squiggle and End-of-Row Messages Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the flat 2px diagnostic underline with a severity-coloured antialiased squiggle, and draw each diagnostic's message at the end of the visual row it ends on.

**Architecture:** Message placement is computed in the pure `build_draw_commands` layer as a new `DrawCommand::DiagnosticMessage` variant (bucketed per visual line, worst severity wins, ellipsized against a measured mono advance carried in `PresentationStyles`). The widget grows a dedicated `DrawSquiggle` quad shader (analytic distance to a sine curve — NOT `sdf.box(..., 0)`, which floods the quad in this fork) and three severity theme colours replacing the single `diagnostic_color`. Both the markdown editor widget and the WAML source view get this through the one shared pipeline; no opt-in flag.

**Tech Stack:** Rust, makepad (redoz fork, script_mod DSL), workspace crates `waml-markdown-editor` and `waml-editor`.

**Spec:** `docs/superpowers/specs/2026-08-08-markdown-diagnostic-squiggle-and-messages-design.md` (approved, ff49d20a).

## Global Constraints

- Squiggle: amplitude ~1.5px, period ~4px, stroke ~1px, antialiased; **never `sdf.box(..., 0)`** — analytic distance to the sine curve only.
- Wave phase locked to absolute document x, carried on the instance, so the squiggle does not crawl on scroll or reflow.
- `MESSAGE_GAP` is exactly `12.0` logical px.
- Message severity order: Error > Warning > Information; ties break on earliest `range.start()`.
- Contested line suffix is `" +N"` where N counts the losing diagnostics on that line.
- Ellipsize drops characters from the end and appends `…`; no wrapping, no row growth, no hard clip.
- Message is decoration: not selectable; caret motion and hit-testing ignore it entirely (it never enters glyph clusters or caret stops, which the design already guarantees — do not add hit-test code for it).
- Message style: mono face at the gutter size convention (`GUTTER_FONT_SIZE * 0.75`, matching `paint_gutter`), coloured by severity, no leading glyph.
- Accepted limitation (do not "fix"): CJK/wide characters overrun the `chars * advance` width slightly; diagnostic messages are ASCII parser text.
- The squiggle shader gets **no unit test**; the visual check is the explicit human-owned final task. No implementation task may block on a visual check.
- Full gate for EVERY task before committing: `cargo test --workspace` at the repo root, then in `editors/vscode`: `pnpm install && pnpm build && pnpm lint && pnpm test`.
- Commit style: `feat(...)`/`docs(...)` subject + body only, NO Co-Authored-By trailer.
- Use relative repo paths only; never hardcode an absolute worktree path.

## File Map

- `crates/waml-markdown-editor/src/presentation/style.rs` — `PresentationStyles` gains `diagnostic_message_advance` (Task 1).
- `crates/waml-markdown-editor/src/presentation/draw.rs` — `DrawCommand::DiagnosticMessage` variant (Task 2), emission rule + `MESSAGE_GAP` (Task 3).
- `crates/waml-markdown-editor/src/widget.rs` — advance measurement + cache key (Task 4), severity colours (Task 5), squiggle field + `squiggle_rect` + decoration paint split (Task 6), message paint arm (Task 7).
- `crates/waml-markdown-editor/src/squiggle.rs` — NEW, `DrawSquiggle` shader (Task 6).
- `crates/waml-editor/src/bin/markdown_presentation_harness.rs` — theme colour rename (Task 5).
- Tests: `crates/waml-markdown-editor/tests/presentation_style.rs`, `tests/draw_layers.rs`, `tests/presentation_model.rs`.
- `crates/waml-editor/src/source_view.rs` already builds `PresentedDiagnostic`s with severities and messages for both syntax and semantic diagnostics — **no change needed there**; it inherits everything through the shared pipeline.

---

### Task 1: `PresentationStyles` carries a measured mono advance

**Files:**
- Modify: `crates/waml-markdown-editor/src/presentation/style.rs:50-63`
- Test: `crates/waml-markdown-editor/tests/presentation_style.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces: `PresentationStyles::diagnostic_message_advance(&self) -> f64`, `PresentationStyles::with_diagnostic_message_advance(self, advance: f64) -> Self`, `pub const DIAGNOSTIC_MESSAGE_ADVANCE_FALLBACK: f64 = 6.6;` (in `style.rs`, `pub` so the crate and tests can reference it). `PresentationStyles` stays `Clone + Copy + Debug + Default + PartialEq` but **loses `Eq`** (it now holds an `f64`).

- [ ] **Step 1: Write the failing test**

Append to `crates/waml-markdown-editor/tests/presentation_style.rs`:

```rust
#[test]
fn balanced_styles_carry_the_fallback_message_advance_until_a_measurement_arrives() {
    use waml_markdown_editor::presentation::style::DIAGNOSTIC_MESSAGE_ADVANCE_FALLBACK;
    let styles = PresentationStyles::balanced();
    assert_eq!(
        styles.diagnostic_message_advance(),
        DIAGNOSTIC_MESSAGE_ADVANCE_FALLBACK
    );
    let measured = styles.with_diagnostic_message_advance(7.25);
    assert_eq!(measured.diagnostic_message_advance(), 7.25);
    // A degenerate measurement can never divide the ellipsize budget by zero.
    let degenerate = styles.with_diagnostic_message_advance(0.0);
    assert!(degenerate.diagnostic_message_advance() >= 1.0);
}
```

If `presentation::style` is not publicly reachable, re-export `DIAGNOSTIC_MESSAGE_ADVANCE_FALLBACK` from `crates/waml-markdown-editor/src/presentation/mod.rs` and import it from there instead.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-markdown-editor --test presentation_style balanced_styles_carry`
Expected: FAIL to compile — no such methods.

- [ ] **Step 3: Implement**

In `style.rs`, replace the unit struct (keep the existing doc comments on `balanced()`):

```rust
/// Advance of one mono character at the diagnostic-message size when no
/// measurement is available (pure tests, headless callers). Matches the
/// widget's `GUTTER_DIGIT_WIDTH` fallback for the same face and size.
pub const DIAGNOSTIC_MESSAGE_ADVANCE_FALLBACK: f64 = 6.6;

/// Resolves a presentation text role into its style roles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PresentationStyles {
    /// Advance of one character of the mono face at the diagnostic-message
    /// size. Measured by the widget through the gutter-metrics path and
    /// installed via `with_diagnostic_message_advance`; the fallback serves
    /// pure construction. Ellipsizing is `chars * advance` — wide (CJK)
    /// characters overrun slightly, which is accepted.
    diagnostic_message_advance: f64,
}

impl Default for PresentationStyles {
    fn default() -> Self {
        Self::balanced()
    }
}
```

In the `impl PresentationStyles` block, change `balanced()` and add the accessors:

```rust
    pub fn balanced() -> Self {
        Self {
            diagnostic_message_advance: DIAGNOSTIC_MESSAGE_ADVANCE_FALLBACK,
        }
    }

    /// The same styles with a measured mono advance. Clamped to at least one
    /// logical pixel so a degenerate measurement can never blow up the
    /// ellipsize budget.
    pub fn with_diagnostic_message_advance(mut self, advance: f64) -> Self {
        self.diagnostic_message_advance = if advance.is_finite() { advance.max(1.0) } else { DIAGNOSTIC_MESSAGE_ADVANCE_FALLBACK };
        self
    }

    pub fn diagnostic_message_advance(&self) -> f64 {
        self.diagnostic_message_advance
    }
```

Note the derive list dropped `Default` and `Eq`: `Default` is now the manual impl above (a derived one would carry a 0.0 advance), and `Eq` is impossible over `f64`. Build the workspace; the compiler will point at any site that relied on `Eq` (none are expected — every known construction site calls `balanced()` and keeps compiling unchanged).

- [ ] **Step 4: Run the full gate**

Run: `cargo test --workspace`, then in `editors/vscode`: `pnpm install && pnpm build && pnpm lint && pnpm test`.
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-markdown-editor/src/presentation/style.rs crates/waml-markdown-editor/tests/presentation_style.rs
git commit -m "feat(markdown-editor): carry a measured mono advance on PresentationStyles"
```

---

### Task 2: `DrawCommand::DiagnosticMessage` variant

**Files:**
- Modify: `crates/waml-markdown-editor/src/presentation/draw.rs:103-188` (variant, `layer()`, `translated()`)
- Modify: `crates/waml-markdown-editor/src/widget.rs:1259` (`paint_command` placeholder arm)
- Test: `crates/waml-markdown-editor/tests/draw_layers.rs`

**Interfaces:**
- Consumes: `PresentedDiagnosticSeverity` (existing, `presentation/draw.rs:21`).
- Produces (later tasks and the emission rule rely on these exact names):

```rust
DrawCommand::DiagnosticMessage {
    line: TextRange,                       // the visual line's source_range
    rect: Rect,                            // x past the row text, y/height from the visual line
    text: Arc<str>,                        // winner's message, possibly " +N" suffixed, possibly ellipsized
    severity: PresentedDiagnosticSeverity, // the winning diagnostic's severity
}
```

- [ ] **Step 1: Write the failing test**

Append to `crates/waml-markdown-editor/tests/draw_layers.rs`:

```rust
#[test]
fn a_diagnostic_message_command_sits_on_the_decoration_layer_and_translates() {
    let command = DrawCommand::DiagnosticMessage {
        line: range(0, 4),
        rect: Rect {
            pos: dvec2(100.0, 20.0),
            size: dvec2(60.0, 18.0),
        },
        text: Arc::from("unexpected token"),
        severity: PresentedDiagnosticSeverity::Error,
    };
    assert_eq!(command.layer(), DrawLayer::Decoration);
    let moved = command.translated(dvec2(10.0, 5.0));
    let DrawCommand::DiagnosticMessage { rect, text, severity, line } = moved else {
        panic!("translation must preserve the variant");
    };
    assert_eq!(rect.pos, dvec2(110.0, 25.0));
    assert_eq!(rect.size, dvec2(60.0, 18.0));
    assert_eq!(text.as_ref(), "unexpected token");
    assert_eq!(severity, PresentedDiagnosticSeverity::Error);
    assert_eq!(line, range(0, 4));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-markdown-editor --test draw_layers a_diagnostic_message_command`
Expected: FAIL to compile — no such variant.

- [ ] **Step 3: Implement**

In `draw.rs`, add to the `DrawCommand` enum (after `Decoration`):

```rust
    /// A diagnostic's message drawn at the end of the visual row it ends on.
    /// Pure decoration: never document text, never selectable, invisible to
    /// caret motion and hit-testing.
    DiagnosticMessage {
        line: TextRange,
        rect: Rect,
        text: Arc<str>,
        severity: PresentedDiagnosticSeverity,
    },
```

In `DrawCommand::layer()` add:

```rust
            Self::DiagnosticMessage { .. } => DrawLayer::Decoration,
```

In `DrawCommand::translated()` add:

```rust
            Self::DiagnosticMessage {
                line,
                rect,
                text,
                severity,
            } => Self::DiagnosticMessage {
                line: *line,
                rect: translate(*rect),
                text: text.clone(),
                severity: *severity,
            },
```

In `widget.rs` `paint_command` (the exhaustive match at ~1259), add a staged no-op arm that Task 7 replaces with real painting:

```rust
            // Painted for real once the severity colours and mono-run painting
            // land (see the diagnostic-squiggle plan, Task 7).
            DrawCommand::DiagnosticMessage { .. } => {}
```

In `tests/draw_layers.rs`, extend the `command_rect_positions` helper (line ~44) with:

```rust
        DrawCommand::DiagnosticMessage { rect, .. } => vec![rect.pos],
```

Build the workspace and fix any other exhaustive `match command` the compiler flags (the known matches in `source_view.rs` and the presentation harness are `if let`/`find_map` with catch-alls and need nothing; if one does flag, give it an explicit `DrawCommand::DiagnosticMessage { .. } => None`-style skip arm, not a `_`).

- [ ] **Step 4: Run the full gate**

Run: `cargo test --workspace`, then in `editors/vscode`: `pnpm install && pnpm build && pnpm lint && pnpm test`.
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-markdown-editor/src/presentation/draw.rs crates/waml-markdown-editor/src/widget.rs crates/waml-markdown-editor/tests/draw_layers.rs
git commit -m "feat(markdown-editor): add the DiagnosticMessage draw-command variant"
```

---

### Task 3: Message emission rule in `build_draw_commands`

**Files:**
- Modify: `crates/waml-markdown-editor/src/presentation/draw.rs:306-315` (extend after the diagnostic-underline loop)
- Test: `crates/waml-markdown-editor/tests/draw_layers.rs`, `crates/waml-markdown-editor/tests/presentation_model.rs`

**Interfaces:**
- Consumes: `DrawCommand::DiagnosticMessage` (Task 2), `styles.diagnostic_message_advance()` (Task 1), `LayoutSnapshot::visual_lines()` / `glyph_clusters()` / `viewport_width()` (existing, `layout/geometry.rs`).
- Produces: `pub const MESSAGE_GAP: f64 = 12.0;` in `draw.rs` (exported so tests and callers share one number). Emission ordering: message commands are appended after all underline `Decoration` commands and before the `EmbeddedBlock` loop, in ascending visual-line order, so the existing layer-order invariants hold untouched.

- [ ] **Step 1: Write the failing tests (bucketing, severity, tie, +N)**

Append to `crates/waml-markdown-editor/tests/draw_layers.rs` (the file's existing helpers `range`, `t`, `cluster`, `text_item`, `selection`, and `VisualLine::for_test` are in scope; `MESSAGE_GAP` comes from the `presentation::draw` import list — add it there):

```rust
/// Two visual lines with real clusters, for the emission-rule tests.
/// Line 0 covers 0..6 (clusters end at x=10+6*10=70), line 1 covers 6..12.
fn two_line_snapshot(source_len: usize) -> Arc<LayoutSnapshot> {
    Arc::new(LayoutSnapshot::from_parts_for_test(
        DocumentRevision::INITIAL,
        dvec2(600.0, 200.0),
        vec![
            VisualLine::for_test(range(0, 6), 20.0, 18.0),
            VisualLine::for_test(range(6, source_len), 40.0, 18.0),
        ],
        vec![
            cluster(1, 0, 0..6, 10.0, TextRole::Body),
            cluster(1, 1, 6..source_len, 10.0, TextRole::Body),
        ],
        Vec::new(),
    ))
}

fn diagnostic(
    bounds: std::ops::Range<usize>,
    severity: PresentedDiagnosticSeverity,
    message: &str,
) -> PresentedDiagnostic {
    PresentedDiagnostic {
        revision: DocumentRevision::INITIAL,
        range: range(bounds.start, bounds.end),
        severity,
        message: Arc::from(message),
    }
}

fn message_commands(
    source: &str,
    layout: Arc<LayoutSnapshot>,
    diagnostics: Vec<PresentedDiagnostic>,
) -> Vec<DrawCommand> {
    let plan = PresentationPlan {
        revision: DocumentRevision::INITIAL,
        source_len: t(source.len()),
        items: Arc::from([text_item(1, 0, 0..source.len(), TextRole::Body)]),
        links: Arc::from([]),
        blocks: Arc::from([]),
        diagnostics: Arc::from([]),
    };
    let frame = PresentationFrame {
        revision: DocumentRevision::INITIAL,
        layout,
        active_owners: Arc::from([]),
        diagnostics: diagnostics.into(),
        assets: Arc::new(EmbeddedAssetFrame {
            revision: DocumentRevision::INITIAL,
            items: Arc::from([]),
        }),
    };
    build_draw_commands(
        &frame,
        &plan,
        &PresentationStyles::balanced(),
        &selection(source, 0, 0),
        None,
    )
    .unwrap()
    .iter()
    .filter(|command| matches!(command, DrawCommand::DiagnosticMessage { .. }))
    .cloned()
    .collect()
}

#[test]
fn diagnostics_bucket_onto_the_visual_line_holding_their_end_offset() {
    let source = "abcdefghijkl";
    let commands = message_commands(
        source,
        two_line_snapshot(source.len()),
        vec![
            // Starts on line 0 but ENDS on line 1 -> the message rides line 1.
            diagnostic(2..9, PresentedDiagnosticSeverity::Warning, "spans"),
        ],
    );
    assert_eq!(commands.len(), 1);
    let DrawCommand::DiagnosticMessage { line, rect, .. } = &commands[0] else {
        unreachable!()
    };
    assert_eq!(*line, range(6, 12));
    assert_eq!(rect.pos.y, 40.0);
    assert_eq!(rect.size.y, 18.0);
}

#[test]
fn worst_severity_wins_a_contested_line_and_counts_the_losers() {
    let source = "abcdefghijkl";
    let commands = message_commands(
        source,
        two_line_snapshot(source.len()),
        vec![
            diagnostic(0..2, PresentedDiagnosticSeverity::Information, "info"),
            diagnostic(3..5, PresentedDiagnosticSeverity::Error, "the error"),
            diagnostic(1..4, PresentedDiagnosticSeverity::Warning, "warn"),
        ],
    );
    assert_eq!(commands.len(), 1, "one message per line");
    let DrawCommand::DiagnosticMessage { text, severity, .. } = &commands[0] else {
        unreachable!()
    };
    assert_eq!(text.as_ref(), "the error +2");
    assert_eq!(*severity, PresentedDiagnosticSeverity::Error);
}

#[test]
fn severity_ties_break_on_the_earliest_start() {
    let source = "abcdefghijkl";
    let commands = message_commands(
        source,
        two_line_snapshot(source.len()),
        vec![
            diagnostic(3..5, PresentedDiagnosticSeverity::Error, "later"),
            diagnostic(1..4, PresentedDiagnosticSeverity::Error, "earlier"),
        ],
    );
    let DrawCommand::DiagnosticMessage { text, .. } = &commands[0] else {
        unreachable!()
    };
    assert_eq!(text.as_ref(), "earlier +1");
}

#[test]
fn contested_lines_stay_independent_and_commands_come_in_line_order() {
    let source = "abcdefghijkl";
    let commands = message_commands(
        source,
        two_line_snapshot(source.len()),
        vec![
            diagnostic(7..9, PresentedDiagnosticSeverity::Warning, "second line"),
            diagnostic(0..2, PresentedDiagnosticSeverity::Information, "first line"),
        ],
    );
    assert_eq!(commands.len(), 2);
    let DrawCommand::DiagnosticMessage { text: first, .. } = &commands[0] else {
        unreachable!()
    };
    let DrawCommand::DiagnosticMessage { text: second, .. } = &commands[1] else {
        unreachable!()
    };
    assert_eq!(first.as_ref(), "first line");
    assert_eq!(second.as_ref(), "second line");
}
```

- [ ] **Step 2: Write the failing tests (placement and ellipsize) in `presentation_model.rs`**

`presentation_model.rs` has no layout helpers today; give it a self-contained set. Append:

```rust
mod diagnostic_message_emission {
    use std::sync::Arc;

    use makepad_widgets::{dvec2, Rect};
    use waml_markdown_editor::{
        layout::{CaretStop, GeometryElementId, GlyphCluster, LayoutElementId, LayoutSnapshot, VisualLine},
        presentation::{
            draw::{
                build_draw_commands, DrawCommand, PresentationFrame, PresentedDiagnostic,
                PresentedDiagnosticSeverity, MESSAGE_GAP,
            },
            EmbeddedAssetFrame, PresentationPlan, PresentationStyles, TextRole,
        },
        selection::{Affinity, Selection, SelectionSet, TextPosition},
    };
    use waml_syntax::{DocumentRevision, SourceText, TextRange, TextSize};

    use super::{owner, range, t};

    fn snapshot(viewport_width: f64, cluster_x: f64, cluster_width: f64) -> Arc<LayoutSnapshot> {
        let source_range = range(0..4);
        let cluster = GlyphCluster::for_test(
            source_range,
            Rect {
                pos: dvec2(cluster_x, 20.0),
                size: dvec2(cluster_width, 18.0),
            },
            vec![
                CaretStop::new(
                    TextPosition::new(t(0), Affinity::Before),
                    dvec2(cluster_x, 20.0),
                ),
                CaretStop::new(
                    TextPosition::new(t(4), Affinity::Before),
                    dvec2(cluster_x + cluster_width, 20.0),
                ),
            ],
        );
        Arc::new(LayoutSnapshot::from_parts_for_test(
            DocumentRevision::INITIAL,
            dvec2(viewport_width, 60.0),
            vec![VisualLine::for_test(source_range, 20.0, 18.0)],
            vec![cluster],
            Vec::new(),
        ))
    }

    fn messages(
        layout: Arc<LayoutSnapshot>,
        message: &str,
    ) -> Vec<DrawCommand> {
        let source = "abcd";
        let plan = PresentationPlan {
            revision: DocumentRevision::INITIAL,
            source_len: t(source.len()),
            items: Arc::from([super::run(0..source.len(), TextRole::Body, owner(1), 0)]),
            links: Arc::from([]),
            blocks: Arc::from([]),
            diagnostics: Arc::from([]),
        };
        let frame = PresentationFrame {
            revision: DocumentRevision::INITIAL,
            layout,
            active_owners: Arc::from([]),
            diagnostics: Arc::from([PresentedDiagnostic {
                revision: DocumentRevision::INITIAL,
                range: range(1..3),
                severity: PresentedDiagnosticSeverity::Error,
                message: Arc::from(message),
            }]),
            assets: Arc::new(EmbeddedAssetFrame {
                revision: DocumentRevision::INITIAL,
                items: Arc::from([]),
            }),
        };
        let selections = SelectionSet::from_source(
            DocumentRevision::INITIAL,
            &SourceText::new(source.to_owned()).unwrap(),
            vec![Selection::new(
                TextPosition::new(t(0), Affinity::Before),
                TextPosition::new(t(0), Affinity::Before),
            )],
            0,
        )
        .unwrap();
        build_draw_commands(&frame, &plan, &PresentationStyles::balanced(), &selections, None)
            .unwrap()
            .iter()
            .filter(|command| matches!(command, DrawCommand::DiagnosticMessage { .. }))
            .cloned()
            .collect()
    }

    #[test]
    fn placement_sits_message_gap_past_the_last_cluster_right_edge() {
        let commands = messages(snapshot(600.0, 10.0, 40.0), "boom");
        let DrawCommand::DiagnosticMessage { rect, text, .. } = &commands[0] else {
            unreachable!()
        };
        assert_eq!(rect.pos.x, 10.0 + 40.0 + MESSAGE_GAP);
        assert_eq!(text.as_ref(), "boom");
        let advance = PresentationStyles::balanced().diagnostic_message_advance();
        assert_eq!(rect.size.x, 4.0 * advance);
    }

    #[test]
    fn ellipsize_fires_exactly_at_the_viewport_width_boundary() {
        let advance = PresentationStyles::balanced().diagnostic_message_advance();
        let x = 10.0 + 40.0 + MESSAGE_GAP;
        // Budget for exactly 10 characters past the row text.
        let viewport = x + 10.0 * advance;
        // 10 chars fit untouched.
        let fits = messages(snapshot(viewport, 10.0, 40.0), "0123456789");
        let DrawCommand::DiagnosticMessage { text, .. } = &fits[0] else {
            unreachable!()
        };
        assert_eq!(text.as_ref(), "0123456789");
        // 11 chars ellipsize to 9 + '…' (10 glyphs total).
        let clipped = messages(snapshot(viewport, 10.0, 40.0), "0123456789A");
        let DrawCommand::DiagnosticMessage { text, .. } = &clipped[0] else {
            unreachable!()
        };
        assert_eq!(text.as_ref(), "012345678…");
    }

    #[test]
    fn a_row_with_no_room_left_emits_no_message_at_all() {
        // Viewport ends before even one character fits past the gap.
        let commands = messages(snapshot(10.0 + 40.0 + MESSAGE_GAP + 0.5, 10.0, 40.0), "boom");
        assert!(commands.is_empty(), "no wrapping, no row growth, no hard clip");
    }
}
```

Note for the implementer: `GlyphCluster::for_test(source_range, rect, caret_stops)` exists at `layout/geometry.rs:93`. If any helper here needs `pub` visibility adjustments (`run`/`owner`/`range`/`t` are file-local in `presentation_model.rs` — `use super::…` reaches them since the module is inline in the same file), adjust imports rather than duplicating logic. If `TextRange`/`TextSize` imports end up unused, drop them.

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p waml-markdown-editor --test draw_layers diagnostics_bucket` and `cargo test -p waml-markdown-editor --test presentation_model diagnostic_message`
Expected: FAIL — no `MESSAGE_GAP`, no messages emitted.

- [ ] **Step 4: Implement the emission rule**

In `draw.rs`, add near the top (below the imports; add `dvec2` to the `makepad_widgets` import):

```rust
/// Gap between a row's last glyph and its diagnostic message.
pub const MESSAGE_GAP: f64 = 12.0;

/// Error > Warning > Information.
fn severity_rank(severity: PresentedDiagnosticSeverity) -> u8 {
    match severity {
        PresentedDiagnosticSeverity::Error => 2,
        PresentedDiagnosticSeverity::Warning => 1,
        PresentedDiagnosticSeverity::Information => 0,
    }
}
```

In `build_draw_commands`, directly after the diagnostic-underline loop (line ~315) and before the `EmbeddedBlock` loop, insert:

```rust
    // End-of-row diagnostic messages: one per visual line, computed here so
    // placement lives in the pure, unit-tested draw-command layer.
    let mut message_lines: Vec<(usize, Vec<&PresentedDiagnostic>)> = Vec::new();
    for diagnostic in frame.diagnostics.iter() {
        let end = diagnostic.range.end();
        // The first visual line whose source range contains the END offset;
        // an offset on a line boundary belongs to the earlier line.
        let Some(index) = frame.layout.visual_lines().iter().position(|line| {
            line.source_range.start() <= end && end <= line.source_range.end()
        }) else {
            continue; // off-viewport; nothing to say
        };
        match message_lines.iter_mut().find(|(line, _)| *line == index) {
            Some((_, bucket)) => bucket.push(diagnostic),
            None => message_lines.push((index, vec![diagnostic])),
        }
    }
    message_lines.sort_by_key(|(index, _)| *index);
    for (index, bucket) in message_lines {
        // Worst severity wins; ties break on the earliest start, which makes
        // the order total: two diagnostics never contend for the same slot.
        let winner = bucket
            .iter()
            .copied()
            .min_by_key(|diagnostic| {
                (
                    std::cmp::Reverse(severity_rank(diagnostic.severity)),
                    diagnostic.range.start(),
                )
            })
            .expect("a bucket is created non-empty");
        let line = &frame.layout.visual_lines()[index];
        // The maximum right edge of the line's glyph clusters; a line with no
        // clusters (blank line) anchors at its own left edge.
        let text_right = frame
            .layout
            .glyph_clusters()
            .iter()
            .filter(|cluster| {
                cluster.source_range.start() < line.source_range.end()
                    && line.source_range.start() < cluster.source_range.end()
            })
            .map(|cluster| cluster.rect.pos.x + cluster.rect.size.x)
            .fold(None::<f64>, |best, right| {
                Some(best.map_or(right, |best| best.max(right)))
            });
        let x = text_right.unwrap_or(line.rect.pos.x) + MESSAGE_GAP;
        let mut text = winner.message.to_string();
        let others = bucket.len() - 1;
        if others > 0 {
            text.push_str(&format!(" +{others}"));
        }
        // Ellipsize against the viewport: the message renders in the mono
        // face, so width is chars * advance (wide characters overrun
        // slightly; accepted -- diagnostic messages are ASCII parser text).
        let advance = styles.diagnostic_message_advance();
        let budget = ((frame.layout.viewport_width() - x) / advance).floor();
        if budget < 1.0 {
            continue; // no wrapping, no row growth, no hard clip
        }
        let max_chars = budget as usize;
        if text.chars().count() > max_chars {
            text = text.chars().take(max_chars.saturating_sub(1)).collect();
            text.push('…');
        }
        let width = text.chars().count() as f64 * advance;
        commands.push(DrawCommand::DiagnosticMessage {
            line: line.source_range,
            rect: Rect {
                pos: dvec2(x, line.rect.pos.y),
                size: dvec2(width, line.rect.size.y),
            },
            text: text.into(),
            severity: winner.severity,
        });
    }
```

- [ ] **Step 5: Run the new tests, then the full gate**

Run: `cargo test -p waml-markdown-editor --test draw_layers` and `--test presentation_model` (all PASS, including the pre-existing layer-order tests — messages land inside the Decoration span). Then the full gate: `cargo test --workspace`, and in `editors/vscode`: `pnpm install && pnpm build && pnpm lint && pnpm test`.
Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-markdown-editor/src/presentation/draw.rs crates/waml-markdown-editor/tests/draw_layers.rs crates/waml-markdown-editor/tests/presentation_model.rs
git commit -m "feat(markdown-editor): emit end-of-row diagnostic messages from build_draw_commands"
```

---

### Task 4: Widget measures the advance and installs it with the styles

**Files:**
- Modify: `crates/waml-markdown-editor/src/widget.rs:784-790` (`DrawCommandsCache`), `:1038` (call site), `:1110-1160` (`cached_draw_commands`)
- Test: the `layout_pipeline_tests` module inside `widget.rs` (~line 2427)

**Interfaces:**
- Consumes: `PresentationStyles::with_diagnostic_message_advance` (Task 1), `self.gutter_metrics(cx).digit_width` (existing — the digit advance of the mono face at `GUTTER_FONT_SIZE`, exactly the face and size the message renders in).
- Produces: `fn cached_draw_commands(&mut self, cx: &mut Cx, session: &MarkdownDocumentSession, installed: &Arc<InstalledPresentation>, layout: &Arc<LayoutSnapshot>) -> …` (signature gains `cx`), `DrawCommandsCache` gains `message_advance: f64`, and a free function `fn draw_commands_cache_reusable(cache: &DrawCommandsCache, installed: &Arc<InstalledPresentation>, layout: &Arc<LayoutSnapshot>, selections: &crate::selection::SelectionSet, message_advance: f64) -> bool`.

- [ ] **Step 1: Write the failing test**

In the existing `layout_pipeline_tests` module in `widget.rs` (which already has `installed_presentation()` and `layout_snapshot()` helpers):

```rust
    #[test]
    fn a_changed_message_advance_invalidates_the_draw_command_cache() {
        let installed = installed_presentation();
        let layout = layout_snapshot();
        let selections = SelectionSet::empty_for_test();
        let cache = DrawCommandsCache {
            installed: installed.clone(),
            layout: layout.clone(),
            selections: selections.clone(),
            message_advance: 6.6,
            commands: Arc::from([]),
            plan: Arc::new(Default::default()),
        };
        assert!(draw_commands_cache_reusable(
            &cache, &installed, &layout, &selections, 6.6
        ));
        assert!(
            !draw_commands_cache_reusable(&cache, &installed, &layout, &selections, 7.0),
            "a font refresh that changes the mono advance must rebuild the commands"
        );
    }
```

If `SelectionSet::empty_for_test()` does not exist, construct the selection set the way the sibling tests in that module do (mirror their exact pattern; do not invent a new constructor unless none exists, in which case add a `#[doc(hidden)] pub fn empty_for_test()` beside the other `for_test` helpers in `selection.rs`).

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-markdown-editor --lib a_changed_message_advance`
Expected: FAIL to compile — no `message_advance` field, no `draw_commands_cache_reusable`.

- [ ] **Step 3: Implement**

Add `message_advance: f64` to `DrawCommandsCache` (document it: "Mono advance the cached commands were ellipsized against; a font refresh or scale change that moves it must rebuild"). Extract the reuse predicate as a free function next to the struct:

```rust
fn draw_commands_cache_reusable(
    cache: &DrawCommandsCache,
    installed: &Arc<InstalledPresentation>,
    layout: &Arc<LayoutSnapshot>,
    selections: &crate::selection::SelectionSet,
    message_advance: f64,
) -> bool {
    Arc::ptr_eq(&cache.installed, installed)
        && Arc::ptr_eq(&cache.layout, layout)
        && cache.selections == *selections
        && cache.message_advance.to_bits() == message_advance.to_bits()
}
```

Rework `cached_draw_commands`:

- Signature gains `cx: &mut Cx` as the first parameter.
- First line: `let message_advance = self.gutter_metrics(cx).digit_width;`
- The `reusable` check becomes `session.ime().is_none() && self.pipeline.draw_commands_cache.as_ref().is_some_and(|cache| draw_commands_cache_reusable(cache, installed, layout, session.selections(), message_advance))`.
- The build passes measured styles: `let styles = installed.styles.with_diagnostic_message_advance(message_advance);` then `build_draw_commands(&frame, &installed.plan, &styles, session.selections(), session.ime())` (note `installed.styles` is `Arc<PresentationStyles>` and the type is `Copy` — `(*installed.styles).with_diagnostic_message_advance(message_advance)` if deref is needed).
- Store `message_advance` in the new cache entry.

Update the single call site at ~line 1038: `self.cached_draw_commands(cx, session, &installed, &layout)?` (the caller's `cx: &mut Cx2d` deref-coerces to `&mut Cx`).

- [ ] **Step 4: Run the full gate**

Run: `cargo test --workspace`, then in `editors/vscode`: `pnpm install && pnpm build && pnpm lint && pnpm test`.
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-markdown-editor/src/widget.rs crates/waml-markdown-editor/src/selection.rs
git commit -m "feat(markdown-editor): measure the mono message advance and key the draw cache on it"
```

(Drop `selection.rs` from the add if no test helper was needed there.)

---

### Task 5: Three severity theme colours replace `diagnostic_color`

**Files:**
- Modify: `crates/waml-markdown-editor/src/widget.rs:55` (DSL default), `:690` (live field), `:1284`, `:1323`, `:1450` (uses)
- Modify: `crates/waml-editor/src/bin/markdown_presentation_harness.rs:77` (DSL override)

**Interfaces:**
- Consumes: `PresentedDiagnosticSeverity` (add it to the `presentation` import list in `widget.rs`).
- Produces: `#[live] diagnostic_error_color: Vec4`, `#[live] diagnostic_warning_color: Vec4`, `#[live] diagnostic_info_color: Vec4` on `MarkdownEditor`, and `fn severity_color(&self, severity: PresentedDiagnosticSeverity) -> Vec4`. The name `diagnostic_color` no longer exists anywhere in the repo.

- [ ] **Step 1: Replace the field and DSL defaults**

In `widget.rs`, replace the single `#[live] diagnostic_color: Vec4` field with:

```rust
    #[live]
    diagnostic_error_color: Vec4,
    #[live]
    diagnostic_warning_color: Vec4,
    #[live]
    diagnostic_info_color: Vec4,
```

In the `script_mod!` DSL block (line ~55), replace `diagnostic_color: #d64545` with:

```text
        diagnostic_error_color: #d64545
        diagnostic_warning_color: #c98a2d
        diagnostic_info_color: #3f73d8
```

(Error keeps the shipped red; warning is an amber in the same saturation family; info reuses the theme's active-marker blue.)

- [ ] **Step 2: Add the helper and update the three uses**

```rust
    fn severity_color(&self, severity: PresentedDiagnosticSeverity) -> Vec4 {
        match severity {
            PresentedDiagnosticSeverity::Error => self.diagnostic_error_color,
            PresentedDiagnosticSeverity::Warning => self.diagnostic_warning_color,
            PresentedDiagnosticSeverity::Information => self.diagnostic_info_color,
        }
    }
```

- `paint_command` decoration arm (line ~1284): `DecorationRole::DiagnosticUnderline(severity) => self.severity_color(*severity),` (still the flat underline for now; Task 6 swaps the pen).
- `EmbeddedState::Failed` fill (line ~1323): `self.draw_embedded.color = self.diagnostic_error_color;` (a failed embed is an error surface).
- `ColorRole::Recovery` (line ~1450): `ColorRole::Recovery => self.diagnostic_error_color,`.

- [ ] **Step 3: Update the harness DSL**

In `crates/waml-editor/src/bin/markdown_presentation_harness.rs` (line ~77), replace `diagnostic_color: atlas.accent` with:

```text
            diagnostic_error_color: #d64545
            diagnostic_warning_color: #c98a2d
            diagnostic_info_color: #3f73d8
```

Then verify no reference survives: `grep -rn "diagnostic_color" crates editors` must return nothing.

- [ ] **Step 4: Run the full gate**

Run: `cargo test --workspace`, then in `editors/vscode`: `pnpm install && pnpm build && pnpm lint && pnpm test`.
Expected: all PASS (the colour trio is paint-time only; no pure test observes it).

- [ ] **Step 5: Commit**

```bash
git add crates/waml-markdown-editor/src/widget.rs crates/waml-editor/src/bin/markdown_presentation_harness.rs
git commit -m "feat(markdown-editor): split diagnostic_color into per-severity theme colours"
```

---

### Task 6: Squiggle shader and decoration paint split

**Files:**
- Create: `crates/waml-markdown-editor/src/squiggle.rs`
- Modify: `crates/waml-markdown-editor/src/lib.rs` (module declaration), `crates/waml-markdown-editor/src/widget.rs` (`register_script_mod` at ~line 71, live field at ~668, `paint_command` at ~1281, `squiggle_rect` beside `underline_rect` at ~1744)
- Test: `squiggle_rect` unit tests in a `#[cfg(test)]` module in `widget.rs`

**Interfaces:**
- Consumes: `severity_color` (Task 5).
- Produces: `DrawSquiggle` (`pub struct` with `#[deref] draw_super: DrawQuad`, `#[live] color: Vec4f`, `#[live(0.0)] phase_x: f32`), `fn squiggle_rect(rect: Rect) -> Rect`, `#[live] draw_squiggle: DrawSquiggle` on `MarkdownEditor`.

- [ ] **Step 1: Write the failing `squiggle_rect` test**

In `widget.rs`, next to the existing test modules:

```rust
#[cfg(test)]
mod squiggle_rect_tests {
    use super::*;

    #[test]
    fn squiggle_band_hugs_the_bottom_with_four_pixels_of_room() {
        let rect = Rect {
            pos: dvec2(10.0, 20.0),
            size: dvec2(80.0, 18.0),
        };
        let band = squiggle_rect(rect);
        assert_eq!(band.pos, dvec2(10.0, 20.0 + 18.0 - 4.0));
        assert_eq!(band.size, dvec2(80.0, 4.0));
    }

    #[test]
    fn a_short_rect_never_yields_a_band_taller_than_itself() {
        let rect = Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(40.0, 3.0),
        };
        let band = squiggle_rect(rect);
        assert_eq!(band.pos, dvec2(0.0, 0.0));
        assert_eq!(band.size, dvec2(40.0, 3.0));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-markdown-editor --lib squiggle_rect_tests`
Expected: FAIL to compile — no `squiggle_rect`.

- [ ] **Step 3: Create the shader module**

`crates/waml-markdown-editor/src/squiggle.rs`, modelled exactly on `src/reading/bullet.rs`:

```rust
//! The diagnostic squiggle pen.
//!
//! An antialiased sine-wave underline, coloured per severity. The wave phase
//! is locked to absolute document x (`phase_x`, carried on the instance), so
//! the squiggle does not crawl when the viewport scrolls or text reflows —
//! and adjacent rects of one wrapped range continue a single wave seamlessly.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*

    // Analytic distance from the fragment to a sine curve, stroked with
    // smoothstep. `sdf.box(.., 0)` floods the quad in this fork -- no sdf
    // helpers here at all, the distance is computed directly.
    set_type_default() do #(DrawSquiggle::script_shader(vm)){
        ..mod.draw.DrawQuad
        pixel: fn() {
            let px = self.pos * self.rect_size
            let two_pi = 6.2831853
            let period = 4.0
            let amplitude = 1.5
            let angle = (self.phase_x + px.x) * two_pi / period
            let mid = self.rect_size.y * 0.5
            let curve = mid + amplitude * sin(angle)
            let slope = amplitude * cos(angle) * two_pi / period
            let dist = abs(px.y - curve) / sqrt(1.0 + slope * slope)
            let alpha = 1.0 - smoothstep(0.5, 1.5, dist)
            return self.color * alpha
        }
    }
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawSquiggle {
    #[deref]
    pub draw_super: DrawQuad,
    #[live]
    pub color: Vec4f,
    /// Absolute x of the quad's left edge, locking the wave phase.
    #[live(0.0)]
    pub phase_x: f32,
}
```

Declare the module in `lib.rs` alongside the existing module list: `pub mod squiggle;`.

- [ ] **Step 4: Register and wire the widget**

In `widget.rs`:

- `register_script_mod` (line ~71): register the squiggle **before** the consumer, exactly like the bullet (a child shader registered after its consumer is dead and invisible):

```rust
pub(crate) fn register_script_mod(vm: &mut ScriptVm) -> ScriptValue {
    // A child widget is dead and invisible unless its script_mod registers
    // BEFORE its consumer's, so the bullet, the squiggle pen, and the viewer
    // go first.
    crate::squiggle::script_mod(vm);
    crate::reading::script_mod(vm);
    script_mod(vm)
}
```

(`script_mod!` generates a `pub fn script_mod`; mirror however `reading::script_mod` is exported.)

- Add the live field next to `draw_decoration` (line ~672), plus the import `use crate::squiggle::DrawSquiggle;`:

```rust
    #[live]
    draw_squiggle: DrawSquiggle,
```

- Add `squiggle_rect` beside `underline_rect` (line ~1744):

```rust
/// The band the squiggle strokes inside: 4px hugging the text bottom, where
/// the flat underline gets 2px -- the wave needs amplitude plus antialiasing.
fn squiggle_rect(rect: Rect) -> Rect {
    let height = rect.size.y.min(4.0);
    Rect {
        pos: dvec2(rect.pos.x, rect.pos.y + (rect.size.y - height).max(0.0)),
        size: dvec2(rect.size.x, height),
    }
}
```

- Split the decoration arm in `paint_command` (line ~1281):

```rust
            DrawCommand::Decoration { rects, role, .. } => match role {
                DecorationRole::LinkUnderline => {
                    self.draw_decoration.color = self.link_color;
                    for rect in rects.iter() {
                        self.draw_decoration.draw_abs(cx, underline_rect(*rect));
                    }
                }
                DecorationRole::DiagnosticUnderline(severity) => {
                    self.draw_squiggle.color = self.severity_color(*severity);
                    for rect in rects.iter() {
                        let band = squiggle_rect(*rect);
                        self.draw_squiggle.phase_x = band.pos.x as f32;
                        self.draw_squiggle.draw_abs(cx, band);
                    }
                }
            },
```

`draw_decoration` stays a plain `DrawColor` serving link underlines (and text-style underline/strikethrough operations) untouched. If the `color` field types disagree (`Vec4` vs `Vec4f`), convert at the assignment the way the bullet's caller does — do not change the theme field types.

- [ ] **Step 5: Run the full gate**

Run: `cargo test --workspace` (the `squiggle_rect` tests and every existing widget/parity test must pass; the shader itself is exercised only visually — that check is Task 8, never here), then in `editors/vscode`: `pnpm install && pnpm build && pnpm lint && pnpm test`.
Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-markdown-editor/src/squiggle.rs crates/waml-markdown-editor/src/lib.rs crates/waml-markdown-editor/src/widget.rs
git commit -m "feat(markdown-editor): stroke diagnostic underlines as a severity-coloured squiggle"
```

---

### Task 7: Paint the diagnostic message

**Files:**
- Modify: `crates/waml-markdown-editor/src/widget.rs:1259` (`paint_command` — replace the Task 2 no-op arm)

**Interfaces:**
- Consumes: `DrawCommand::DiagnosticMessage` (Task 2, populated since Task 3), `severity_color` (Task 5), `gutter_metrics` + `GUTTER_FONT_SIZE` (existing).
- Produces: the message renders in the mono face at the gutter size convention, severity-coloured, centred in the row band. No new public surface.

- [ ] **Step 1: Replace the placeholder arm**

```rust
            DrawCommand::DiagnosticMessage {
                rect,
                text,
                severity,
                ..
            } => {
                // Mono at the gutter's size convention, coloured by severity;
                // it reads as chrome rather than prose. No leading glyph --
                // colour carries the severity.
                let metrics = self.gutter_metrics(cx);
                self.draw_text_mono.color = self.severity_color(*severity);
                let saved_font_size = self.draw_text_mono.text_style.font_size;
                self.draw_text_mono.text_style.font_size = GUTTER_FONT_SIZE * 0.75;
                // Centre the run in the row band: drop the measured ascent
                // from the band's vertical centre, mirroring paint_gutter's
                // baseline arithmetic.
                let top = rect.pos.y + ((rect.size.y - metrics.ascent) * 0.5).max(0.0);
                self.draw_text_mono
                    .draw_abs(cx, dvec2(rect.pos.x, top), text);
                self.draw_text_mono.text_style.font_size = saved_font_size;
            }
```

(`gutter_metrics(&mut self, cx: &mut Cx)` — `paint_command`'s `cx: &mut Cx2d` deref-coerces. The font size is restored immediately so the pristine mono face stays comparable for the face-refresh loop.)

- [ ] **Step 2: Run the full gate**

Run: `cargo test --workspace`, then in `editors/vscode`: `pnpm install && pnpm build && pnpm lint && pnpm test`.
Expected: all PASS. Do NOT attempt any visual verification here — that is Task 8, human-owned.

- [ ] **Step 3: Commit**

```bash
git add crates/waml-markdown-editor/src/widget.rs
git commit -m "feat(markdown-editor): paint end-of-row diagnostic messages in severity-coloured mono"
```

---

### Task 8: Visual verification (HUMAN-OWNED, deferred)

**This task is not for an automated agent.** No implementation task above blocks on it; the plan's code work is complete after Task 7. A human runs the editor (e.g. via `run.ps1` / the `run` skill, or `cargo run -p waml-editor --bin markdown_presentation_harness`) on a document with syntax and semantic diagnostics of all three severities and signs off:

- [ ] Squiggle replaces the flat 2px bar under diagnostic ranges; ~1.5px amplitude, ~4px period, ~1px stroke, visibly antialiased (no staircase, no flooded quad).
- [ ] Error is red, warning amber, info blue — squiggle and message agree per row.
- [ ] Link underlines and strikethroughs are untouched flat rules.
- [ ] The message sits ~12px past the row's last glyph, vertically centred, small mono, no leading glyph; editing the row moves it with the text.
- [ ] A row with several diagnostics shows one message with the worst severity and a `+N` suffix.
- [ ] A long message ellipsizes with `…` at the viewport edge — no wrap, no row growth, no clipped half-glyph.
- [ ] Scrolling does not make the squiggle crawl; a wrapped diagnostic's segments read as one continuous wave.
- [ ] The message is not selectable and the caret never lands in it.
- [ ] The WAML source view (open a `.waml` bundle source tab) shows the same treatment as the markdown editor.

Findings go back as ordinary follow-up issues; do not amend the landed commits.
