# Inspector Relationships Cards Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the native inspector's flat-text `ASSOCIATIONS` list with a styled `RELATIONSHIPS` section — one bordered rounded-rect card per relationship, orientation shown by a leading accent glyph, far-end name on line 1, and `kind · role · multiplicity` meta on line 2.

**Architecture:** Two seams change. `inspector.rs` is the pure, makepad-free projection: enrich `AssocRow` (add a three-state `AssocDir` enum plus `role`/`multiplicity`, drop the old `direction: &'static str`) and its `build_view` mapping, fully unit-tested. `inspector_panel.rs` is the immediate-mode makepad widget: add `draw_card` (DrawColor SDF), `draw_name`/`draw_glyph` (DrawText) live instances, card geometry consts, and two pure render helpers (`dir_glyph`, `meta_line`) that the card draw loop calls — the helpers carry this task's unit tests, and the draw loop is validated by a build/clippy/test gate plus a pid-safe native visual check.

**Tech Stack:** Rust, makepad (fork at `C:\dev\makepad`, redoz branch), the crate's Atlas theme tokens (`atlas.text`/`atlas.text_dim`/`atlas.field_bg`/`atlas.accent`), IBM Plex Sans embedded font. Web frontend is unaffected — no web tasks.

## Global Constraints

- Gate for every task that touches Rust: `cargo test -p waml-editor` must be green AND `cargo clippy -p waml-editor -- -D warnings` must be clean. Under `-D warnings`, rustc `dead_code` is a HARD error — never add a struct field, const, or fn in one task that is only read in a later task (a fn reachable only from `#[cfg(test)]` code counts as dead in the non-test clippy build). Every task wires in what it introduces.
- Native/makepad only. The web renderer is out of scope; do not touch `waml-wasm` or any web source.
- Read-only breadth (U6): relationship cards are non-interactive — no click-to-navigate, no click-to-edit, no canvas selection. `Subject` stays `Classifier | None`; edges remain non-selectable.
- Fork SDF gotcha: `sdf.box(x, y, w, h, 0.0)` degenerates and floods — use `sdf.rect` for sharp edges, and `sdf.box(...)` with a POSITIVE corner radius for rounded rects (proven idiom, e.g. `tree_panel.rs:53`, `node_design_editor.rs:106`, `select.rs:120`). Numeric `uniform(...)` defaults do work.
- Atlas accent token is `atlas.accent` (`#x1496dc` light / `#x347a88` dark), with `atlas.accent_soft` also available (confirmed in `crates/waml-editor/src/theme_atlas.rs`).
- Row order is model order (edge declaration order). No sorting in v1.

---

### Task 1: Enrich the `AssocRow` projection (`inspector.rs`)

**Files:**
- Modify: `crates/waml-editor/src/inspector.rs` (import ~line 6; `AssocRow` struct ~lines 79-84; `build_view` association loop ~lines 214-229; tests ~lines 304-326, plus new tests appended in the same `mod tests`)

**Interfaces:**
- Produces: `pub enum AssocDir { Out, In, Bi }` (`#[derive(Debug, Clone, Copy, PartialEq, Eq)]`) and the reshaped `pub struct AssocRow { pub kind: String, pub dir: AssocDir, pub other_label: String, pub role: String, pub multiplicity: String }`. Task 2 consumes both. The old `pub direction: &'static str` field is REMOVED.
- Consumes: nothing from other tasks.

**Design notes:**
- `build_view` iterates `model.edges`. Skip `edge.kind == RelationshipKind::Annotates`. `outgoing = &edge.source == key`, `incoming = &edge.target == key`; skip edges touching neither. `dir` = `Bi` when `edge.bidirectional || (edge.from_end.navigable == Some(true) && edge.to_end.navigable == Some(true))`, else `Out` if outgoing, `In` if incoming. The far end is `to_end` if outgoing else `from_end`; `other_label` resolves the far node key via the existing `node_label` closure. `role` = far end's `role.clone().unwrap_or_default()`. `multiplicity` = far end's `Multiplicity::as_str()` string, but `""` when the multiplicity is `None` or the trivial `"1"` (mirrors the attribute-row convention of hiding a bare `1`).
- The mini fixture's `Order→Customer` edge already carries roles (`from_end` = "1 order", `to_end` = "1 customer"), but all multiplicities are the trivial `"1"` (hidden) and there is no bidirectional or annotates edge. The two new-behavior tests therefore push hand-built `Edge`s onto a loaded `mini()` model (deterministic, no resolver/fixture coupling); the fixture files are left unchanged.

- [x] **Step 1: Update the two existing association tests and add three new ones**

In `crates/waml-editor/src/inspector.rs`, replace the existing `classifier_projects_outgoing_association` and `classifier_projects_incoming_association` tests (currently ~lines 304-326) with these two updated tests plus append the three new tests immediately after them, inside the `mod tests` block:

```rust
    #[test]
    fn classifier_projects_outgoing_association() {
        let model = mini();
        let key = key_for(&model, "Order");
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        assert_eq!(view.associations.len(), 1);
        let assoc = &view.associations[0];
        assert_eq!(assoc.kind, "associates");
        assert_eq!(assoc.dir, AssocDir::Out);
        assert_eq!(assoc.other_label, "Customer");
        // Far end (to_end = "1 customer"): role kept, trivial "1" multiplicity hidden.
        assert_eq!(assoc.role, "customer");
        assert_eq!(assoc.multiplicity, "");
    }

    #[test]
    fn classifier_projects_incoming_association() {
        let model = mini();
        let key = key_for(&model, "Customer");
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        assert_eq!(view.associations.len(), 1);
        let assoc = &view.associations[0];
        assert_eq!(assoc.kind, "associates");
        assert_eq!(assoc.dir, AssocDir::In);
        assert_eq!(assoc.other_label, "Order");
        // Far end (from_end = "1 order").
        assert_eq!(assoc.role, "order");
        assert_eq!(assoc.multiplicity, "");
    }

    #[test]
    fn classifier_projects_bidirectional_association() {
        use waml::model::{Edge, RelEnd, RelationshipKind};
        let mut model = mini();
        let order = key_for(&model, "Order");
        let gateway = key_for(&model, "PaymentGateway");
        model.edges.push(Edge {
            source: order.clone(),
            target: gateway,
            kind: RelationshipKind::Associates,
            name: None,
            from_end: RelEnd::default(),
            to_end: RelEnd::default(),
            bidirectional: true,
        });
        let view = build_view(&model, &Subject::Classifier(order)).unwrap();
        let bi = view
            .associations
            .iter()
            .find(|r| r.dir == AssocDir::Bi)
            .expect("a bidirectional row projected");
        assert_eq!(bi.other_label, "PaymentGateway");
        assert_eq!(bi.kind, "associates");
    }

    #[test]
    fn classifier_projects_far_end_role_and_multiplicity() {
        use waml::model::{Edge, RelEnd, RelationshipKind};
        use waml::multiplicity::Multiplicity;
        let mut model = mini();
        let order = key_for(&model, "Order");
        let gateway = key_for(&model, "PaymentGateway");
        model.edges.push(Edge {
            source: order.clone(),
            target: gateway,
            kind: RelationshipKind::Aggregates,
            name: None,
            from_end: RelEnd::default(),
            to_end: RelEnd {
                multiplicity: Multiplicity::parse("0..1"),
                role: Some("buyer".to_string()),
                navigable: None,
            },
            bidirectional: false,
        });
        let view = build_view(&model, &Subject::Classifier(order)).unwrap();
        let agg = view
            .associations
            .iter()
            .find(|r| r.kind == "aggregates")
            .expect("the aggregates row projected");
        assert_eq!(agg.dir, AssocDir::Out);
        assert_eq!(agg.role, "buyer");
        assert_eq!(agg.multiplicity, "0..1");
    }

    #[test]
    fn annotates_edges_are_skipped() {
        use waml::model::{Edge, RelEnd, RelationshipKind};
        let mut model = mini();
        let order = key_for(&model, "Order");
        let gateway = key_for(&model, "PaymentGateway");
        let before = build_view(&model, &Subject::Classifier(order.clone()))
            .unwrap()
            .associations
            .len();
        model.edges.push(Edge {
            source: order.clone(),
            target: gateway,
            kind: RelationshipKind::Annotates,
            name: None,
            from_end: RelEnd::default(),
            to_end: RelEnd::default(),
            bidirectional: false,
        });
        let after = build_view(&model, &Subject::Classifier(order))
            .unwrap()
            .associations
            .len();
        assert_eq!(before, after, "an annotates edge must not project a row");
    }
```

- [x] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor inspector::tests::classifier_projects`
Expected: FAIL — compile errors (`no variant or associated item named 'Out' found for enum` / `no field 'dir' on type '&AssocRow'`), because `AssocDir` and the new fields don't exist yet.

- [x] **Step 3: Add `AssocDir`, reshape `AssocRow`, and import `RelationshipKind`**

In `crates/waml-editor/src/inspector.rs`, change the top import (~line 6):

```rust
use waml::model::{ElementType, Model, RelationshipKind};
```

Then replace the `AssocRow` struct and its doc comment (~lines 76-84) with:

```rust
/// Orientation of a relationship from the *subject node's* point of view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssocDir {
    Out, // subject is the edge's source        -> glyph "\u{2192}"
    In,  // subject is the edge's target        -> glyph "\u{2190}"
    Bi,  // both ends navigable / bidirectional -> glyph "\u{2194}"
}

/// One association row, pre-rendered to display strings. Derived from
/// `Model::edges` where `key` is either endpoint -- read-only breadth (U6),
/// not an editable field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssocRow {
    pub kind: String,         // RelationshipKind::as_str(), e.g. "associates"
    pub dir: AssocDir,        // orientation from the subject's point of view
    pub other_label: String,  // the far endpoint's title, falling back to its key
    pub role: String,         // far end's role, "" when unset
    pub multiplicity: String, // far end's multiplicity, "" when unset or trivial "1"
}
```

- [x] **Step 4: Rewrite the `build_view` association loop**

In `crates/waml-editor/src/inspector.rs`, replace the association loop (~lines 214-229, the `let mut associations = Vec::new();` block through the closing `}` of the `for edge in &model.edges` loop) with:

```rust
    let mut associations = Vec::new();
    for edge in &model.edges {
        // uml.Note anchor, not a real relationship (mirrors the web skip).
        if edge.kind == RelationshipKind::Annotates {
            continue;
        }
        let outgoing = &edge.source == key;
        let incoming = &edge.target == key;
        if !outgoing && !incoming {
            continue;
        }
        let dir = if edge.bidirectional
            || (edge.from_end.navigable == Some(true) && edge.to_end.navigable == Some(true))
        {
            AssocDir::Bi
        } else if outgoing {
            AssocDir::Out
        } else {
            AssocDir::In
        };
        // Role + multiplicity read from the FAR end.
        let far_end = if outgoing { &edge.to_end } else { &edge.from_end };
        let far_key = if outgoing { &edge.target } else { &edge.source };
        let role = far_end.role.clone().unwrap_or_default();
        // Hide a bare "1" like the attribute rows do.
        let multiplicity = match &far_end.multiplicity {
            Some(m) if m.as_str() != "1" => m.as_str().to_string(),
            _ => String::new(),
        };
        associations.push(AssocRow {
            kind: edge.kind.as_str().to_string(),
            dir,
            other_label: node_label(far_key),
            role,
            multiplicity,
        });
    }
```

- [x] **Step 5: Run the projection tests to verify they pass**

Run: `cargo test -p waml-editor inspector::tests::`
Expected: PASS — all `inspector` unit tests green, including the two updated and three new association tests.

- [x] **Step 6: Gate — full crate tests + clippy**

Run: `cargo test -p waml-editor`
Expected: PASS (the panel still reads `assoc.direction` at this point — see note below).
Run: `cargo clippy -p waml-editor -- -D warnings`
Expected: FAIL at `crates/waml-editor/src/inspector_panel.rs` line ~478 — `no field 'direction' on type '&AssocRow'`. This is EXPECTED: the panel still references the removed field. Do NOT try to fix it here; it is fixed wholesale in Task 2. Proceed to commit — Task 1's own deliverable (the projection + its tests) is complete and green.

> If you prefer a fully-green intermediate commit, you may apply Task 2's panel edits before committing and fold the two tasks; otherwise commit now and let Task 2 restore the build. The projection tests (`cargo test -p waml-editor --lib inspector::tests::`) are the authoritative gate for this task.

- [x] **Step 7: Commit**

```bash
git add crates/waml-editor/src/inspector.rs
git commit -m "feat(inspector): enrich AssocRow projection with AssocDir + role/multiplicity"
```

---

### Task 2: Render the RELATIONSHIPS cards (`inspector_panel.rs`)

**Files:**
- Modify: `crates/waml-editor/src/inspector_panel.rs` (import ~lines 29-32; `live_design!`/`script_mod!` pens after `draw_field_bg` ~line 132; struct fields after `draw_field_bg` field ~line 166; geometry consts after `const GAP` ~line 219; new helper fns after `edge_target` ~line 230; associations draw block ~lines 471-483; new unit tests appended in `mod tests`)

**Interfaces:**
- Consumes (from Task 1): `AssocDir { Out, In, Bi }`, `AssocRow { kind: String, dir: AssocDir, other_label: String, role: String, multiplicity: String }`.
- Produces: private `fn dir_glyph(dir: AssocDir) -> &'static str` and `fn meta_line(assoc: &AssocRow) -> String`, both called by `draw_walk` and unit-tested. No later task depends on these.

**Design decisions (both spec-flagged risks resolved here):**
- **Chip pills vs. single meta line — DECISION: single dim meta line (ships as default).** The panel does zero text measurement today, and the only in-crate measurement API (`waml::solve::sizing::text_width`, used by `card/mod.rs`) measures against the sizing crate's embedded metrics rather than the panel's live `DrawText` pen, so pill widths would not reliably wrap the drawn glyphs. Line 2 renders as one dim run `kind \u{b7} role \u{b7} multiplicity` (empty parts elided) inside the card border — still variant C (bordered cards), just without pill fills. This keeps the run unblocked. Pill upgrade is a noted out-of-scope follow-up.
- **Kind badge — DECISION: folded into the meta line's leading token (simpler path taken).** Wiring the edge picker's per-kind `IconSpline` SDF into an immediate-mode `draw_abs` loop is non-trivial (it is a widget-row `SelectLead`, not a draw pen). Line 1 is `direction glyph (accent) + far-end name (bright)`; the kind stays scannable as the first token of the meta line. An `IconSpline` badge is a noted follow-up.
- **Direction glyphs — Unicode arrows with an ASCII fallback wired for Task 3.** `dir_glyph` returns `\u{2192}` / `\u{2190}` / `\u{2194}`. Task 3's native check confirms IBM Plex Sans renders them; the exact ASCII swap (`->` / `<-` / `<>`) is pre-specified there.

- [x] **Step 1: Write the failing helper tests**

In `crates/waml-editor/src/inspector_panel.rs`, append inside the existing `mod tests` block (after `node_lead_falls_back_to_badge_for_unknown_type`, ~line 807):

```rust
    #[test]
    fn dir_glyph_maps_each_orientation() {
        assert_eq!(dir_glyph(AssocDir::Out), "\u{2192}");
        assert_eq!(dir_glyph(AssocDir::In), "\u{2190}");
        assert_eq!(dir_glyph(AssocDir::Bi), "\u{2194}");
    }

    #[test]
    fn meta_line_joins_present_parts_with_middot() {
        let assoc = AssocRow {
            kind: "associates".into(),
            dir: AssocDir::Out,
            other_label: "Customer".into(),
            role: "buyer".into(),
            multiplicity: "0..1".into(),
        };
        assert_eq!(meta_line(&assoc), "associates \u{b7} buyer \u{b7} 0..1");
    }

    #[test]
    fn meta_line_elides_empty_role_and_multiplicity() {
        let assoc = AssocRow {
            kind: "associates".into(),
            dir: AssocDir::In,
            other_label: "Order".into(),
            role: String::new(),
            multiplicity: String::new(),
        };
        assert_eq!(meta_line(&assoc), "associates");
    }
```

- [x] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor inspector_panel::tests::`
Expected: FAIL — compile errors (`cannot find function 'dir_glyph'`, `cannot find function 'meta_line'`, `cannot find type 'AssocRow'`/`'AssocDir'` in this scope).

- [x] **Step 3: Import the new types**

In `crates/waml-editor/src/inspector_panel.rs`, replace the `crate::inspector` import (~lines 29-32) with:

```rust
use crate::inspector::{
    build_view, effective_field, subject_to_index, AssocDir, AssocRow, ElementKind, ElementRow,
    FieldId, InspectorView, Subject,
};
```

- [x] **Step 4: Add the two pure render helpers**

In `crates/waml-editor/src/inspector_panel.rs`, add these two functions immediately after the `edge_target` function (~line 230, before `fn rgb`):

```rust
/// The leading orientation glyph for a relationship card. Unicode arrows
/// (\u{2192} / \u{2190} / \u{2194}); if IBM Plex Sans renders any as tofu on the
/// running native app, swap these three literals for the ASCII forms
/// `->` / `<-` / `<>` (see the verification task).
fn dir_glyph(dir: AssocDir) -> &'static str {
    match dir {
        AssocDir::Out => "\u{2192}",
        AssocDir::In => "\u{2190}",
        AssocDir::Bi => "\u{2194}",
    }
}

/// The card's second line: `kind \u{b7} role \u{b7} multiplicity`, empty parts
/// elided. Always leads with the kind so the relationship type stays scannable
/// (the mock's kind badge, folded into the meta run for v1 -- an IconSpline
/// badge is a noted follow-up).
fn meta_line(assoc: &AssocRow) -> String {
    let mut parts = vec![assoc.kind.clone()];
    if !assoc.role.is_empty() {
        parts.push(assoc.role.clone());
    }
    if !assoc.multiplicity.is_empty() {
        parts.push(assoc.multiplicity.clone());
    }
    parts.join(" \u{b7} ")
}
```

- [x] **Step 5: Add the card geometry consts**

In `crates/waml-editor/src/inspector_panel.rs`, add immediately after `const GAP: f64 = 12.0;` (~line 219):

```rust
// Relationship-card geometry (px). No text measuring; fixed advances.
const CARD_PAD: f64 = 10.0; // inner padding of each card
const CARD_GAP: f64 = 8.0; // vertical gap between cards
const CARD_LINE_H: f64 = 18.0; // name line height (line 1)
const CARD_META_H: f64 = 16.0; // meta line height (line 2)
const CARD_H: f64 = CARD_PAD * 2.0 + CARD_LINE_H + CARD_META_H; // full card height
const GLYPH_W: f64 = 18.0; // fixed x-advance reserved for the direction glyph
```

- [x] **Step 6: Add the three new draw pens to the `script_mod!` block**

In `crates/waml-editor/src/inspector_panel.rs`, add immediately after `draw_field_bg +: { color: atlas.field_bg }` (~line 132, still inside the `mod.widgets.Inspector = ... do ... { }` body):

```rust
        // Relationship-card background: a faint field-bg fill ringed by a
        // low-alpha accent border, rounded corners via the working box-radius
        // idiom (never `sdf.box(.., 0.0)` -- floods in this fork).
        draw_card +: {
            color: atlas.field_bg
            border: uniform(atlas.accent)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.75, 0.75, self.rect_size.x - 1.5, self.rect_size.y - 1.5, 6.0)
                sdf.fill_keep(vec4(self.color.x, self.color.y, self.color.z, 0.5))
                sdf.stroke(vec4(self.border.x, self.border.y, self.border.z, 0.20), 1.0)
                return sdf.result
            }
        }
        // Far-end name (line 1): brighter than the dim meta run.
        draw_name +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 13
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        // Direction glyph (line 1 lead): accent-colored orientation cue.
        draw_glyph +: {
            color: atlas.accent
            text_style: TextStyle{
                font_size: 14
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
```

- [x] **Step 7: Add the three struct fields**

In `crates/waml-editor/src/inspector_panel.rs`, add immediately after the `draw_field_bg: DrawColor,` field (~line 166, inside `struct Inspector`):

```rust
    #[redraw]
    #[live]
    draw_card: DrawColor,
    #[redraw]
    #[live]
    draw_name: DrawText,
    #[redraw]
    #[live]
    draw_glyph: DrawText,
```

- [x] **Step 8: Replace the flat associations draw block with the card renderer**

In `crates/waml-editor/src/inspector_panel.rs`, replace the entire associations block (~lines 471-483):

```rust
        // Associations: read-only, derived from Model::edges (U6 breadth). Not
        // click-to-edit -- there's no single scalar override target for a
        // relationship yet.
        if !view.associations.is_empty() {
            self.draw_dim.draw_abs(cx, dvec2(x, y), "ASSOCIATIONS");
            y += ROW_H;
            for assoc in &view.associations {
                let line = format!("{} {} {}", assoc.direction, assoc.other_label, assoc.kind);
                self.draw_label.draw_abs(cx, dvec2(x, y), &line);
                y += ROW_H;
            }
            y += GAP;
        }
```

with:

```rust
        // Relationships: read-only cards derived from Model::edges (U6 breadth).
        // One bordered rounded-rect card per edge: line 1 = direction glyph
        // (accent) + far-end name (bright); line 2 = a dim meta run
        // "kind \u{b7} role \u{b7} multiplicity" (empty parts elided). Not
        // click-to-edit -- no single scalar override target for a relationship.
        if !view.associations.is_empty() {
            self.draw_dim.draw_abs(cx, dvec2(x, y), "RELATIONSHIPS");
            y += ROW_H;
            for assoc in &view.associations {
                let card = Rect {
                    pos: dvec2(x, y),
                    size: dvec2(field_w, CARD_H),
                };
                self.draw_card.draw_abs(cx, card);
                let gx = x + CARD_PAD;
                let gy = y + CARD_PAD;
                self.draw_glyph.draw_abs(cx, dvec2(gx, gy), dir_glyph(assoc.dir));
                self.draw_name
                    .draw_abs(cx, dvec2(gx + GLYPH_W, gy), &assoc.other_label);
                self.draw_dim
                    .draw_abs(cx, dvec2(gx, gy + CARD_LINE_H), &meta_line(assoc));
                y += CARD_H + CARD_GAP;
            }
            y += GAP;
        }
```

- [x] **Step 9: Run the helper tests to verify they pass**

Run: `cargo test -p waml-editor inspector_panel::tests::`
Expected: PASS — `dir_glyph_maps_each_orientation`, `meta_line_joins_present_parts_with_middot`, `meta_line_elides_empty_role_and_multiplicity`, plus the pre-existing `edge_target`/`node_lead` tests, all green.

- [x] **Step 10: Gate — full crate tests + clippy**

Run: `cargo test -p waml-editor`
Expected: PASS — the whole crate compiles (the `assoc.direction` reference is gone) and every test is green.
Run: `cargo clippy -p waml-editor -- -D warnings`
Expected: clean — no `dead_code` (`draw_card`/`draw_name`/`draw_glyph` are read in `draw_walk`; every card const is referenced; `dir_glyph`/`meta_line` are called by the draw loop, not only by tests), no warnings.

- [x] **Step 11: Commit**

```bash
git add crates/waml-editor/src/inspector_panel.rs
git commit -m "feat(inspector): render RELATIONSHIPS as two-line bordered cards"
```

---

### Task 3: Native visual verification (pid-safe)

**Files:** none (verification only; a single conditional micro-edit to `crates/waml-editor/src/inspector_panel.rs` if arrow tofu is observed).

**Interfaces:** none.

**CRITICAL pid-safety note (standing memory — honor exactly):** The user very likely has their OWN `waml-editor` open. NEVER screenshot-by-window-name and NEVER `Stop-Process`/kill by process name — that captures or kills the user's session. Launch THIS worktree's own build via its own `scripts/run-native.ps1` (the script builds `$PSScriptRoot`, i.e. the checkout the `.ps1` lives in — NOT your cwd, so run the worktree's copy, not main's), capture the launched process's specific pid, screenshot ONLY that pid, and close ONLY that pid.

- [x] **Step 1: Launch this worktree's build and capture its pid**

From the worktree root, run (PowerShell):

```powershell
$p = Start-Process -FilePath "pwsh" -ArgumentList "-File","scripts/run-native.ps1" -PassThru
"launched pid: $($p.Id)"
```

Wait for the window to finish building and appear. The pid to screenshot is the editor process spawned by the build; if `run-native.ps1` compiles then execs a child, resolve the actual `waml-editor` child pid under `$p.Id` (e.g. `Get-CimInstance Win32_Process -Filter "ParentProcessId=$($p.Id)"`) and use THAT pid. Do not fall back to name-based lookup.

- [x] **Step 2: Drive the inspector to a node with relationships and screenshot by pid**

In the launched editor: open the `mini` fixture (or any model with edges), select the `Order` node (outgoing `associates → Customer`) and the `Customer` node (incoming `associates ← Order`) so both orientations render. Capture a screenshot scoped to the captured pid ONLY.

Verify in the screenshot:
- Section label reads `RELATIONSHIPS` (not `ASSOCIATIONS`).
- One bordered rounded-rect card per relationship, faint fill + thin accent border, not colliding with the `DESCRIPTION` section below.
- Line 1: accent direction glyph then the far-end name in the bright text color. `Order` shows a right arrow to `Customer`; `Customer` shows a left arrow to `Order`.
- Line 2: dim meta run leading with the kind (e.g. `associates`, then `· customer` for `Order`'s card; `Order`'s multiplicity is the trivial `1` and is correctly hidden).
- No glyph tofu (no empty boxes) where the direction arrows should be.

- [x] **Step 3: Conditional — if the arrows render as tofu, swap to ASCII**

ONLY if Step 2 shows tofu boxes instead of arrows: in `crates/waml-editor/src/inspector_panel.rs`, edit `dir_glyph` — replace

```rust
        AssocDir::Out => "\u{2192}",
        AssocDir::In => "\u{2190}",
        AssocDir::Bi => "\u{2194}",
```

with

```rust
        AssocDir::Out => "->",
        AssocDir::In => "<-",
        AssocDir::Bi => "<>",
```

Also update the three `dir_glyph_maps_each_orientation` assertions in `mod tests` to the ASCII literals (`"->"`, `"<-"`, `"<>"`). Then re-run `cargo test -p waml-editor` (Expected: PASS), rebuild via Step 1, re-verify Step 2 (arrows now legible ASCII), and commit:

```bash
git add crates/waml-editor/src/inspector_panel.rs
git commit -m "fix(inspector): ASCII direction glyphs (IBM Plex Sans lacks arrow coverage)"
```

If the arrows render correctly, skip this step entirely — no edit, no commit.

- [x] **Step 4: Close only the launched pid**

Close the editor window you launched, or `Stop-Process -Id <captured-pid>` using the SPECIFIC captured pid only. Never `Stop-Process -Name waml-editor`.

---

## Self-Review (completed by plan author)

**Spec coverage:**
- Enrich projection (`AssocRow` + `AssocDir` + role/multiplicity, Annotates skip, Bi rule, far-end role/mult, hide bare "1") → Task 1 (all covered; tests for outgoing/incoming/bidirectional/role+mult/annotates-skip).
- Update existing two tests + add bidirectional + role/multiplicity tests → Task 1 Step 1 (plus an extra annotates-skip test).
- Replace flat loop with card renderer, `RELATIONSHIPS` label, two-line cards, `draw_card`/`draw_name`/`draw_glyph`, card consts, `field_w` sizing → Task 2.
- Chip-pill risk → resolved to single dim meta line (default), pills noted as follow-up (Task 2 design note).
- Glyph tofu risk → Unicode arrows shipped with pre-specified ASCII fallback (Task 3 Step 3).
- Native visual verify with pid-safe capture, `run-native.ps1` from the worktree → Task 3.
- Web unaffected → no web tasks (Global Constraints).
- Fixture extension → deliberately avoided; new tests inject hand-built edges onto `mini()` for deterministic coverage (Task 1 design note), so no fixture files change and no existing count assertion is disturbed.

**Placeholder scan:** No TBD/TODO/"similar to Task N"/vague-error placeholders. Every code step shows complete code; every run step states an exact command and expected pass/fail.

**Type consistency:** `AssocDir` variants (`Out`/`In`/`Bi`) and `AssocRow` field names (`kind`/`dir`/`other_label`/`role`/`multiplicity`) are identical between Task 1 (definition + projection + tests) and Task 2 (import + `dir_glyph`/`meta_line` + draw loop + tests). Helper signatures `dir_glyph(AssocDir) -> &'static str` and `meta_line(&AssocRow) -> String` match their call sites. Const names (`CARD_PAD`/`CARD_GAP`/`CARD_LINE_H`/`CARD_META_H`/`CARD_H`/`GLYPH_W`) are each referenced. Pen field names (`draw_card`/`draw_name`/`draw_glyph`) match between the `script_mod!` block, the struct, and `draw_walk`.
