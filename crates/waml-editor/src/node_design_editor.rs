//! `NodeDesignEditor`: a hand-drawn immediate-mode port of the HUD "node design
//! editor" mock (`docs/design/hud-node-design-mock.html`). A frosted Atlas HUD
//! pane (AccentFrame material) carrying an identity header and a two-pane body:
//! a LIVE node preview on an inset canvas (left) that reacts to every control on
//! the right. Compiled into the crate but NOT mounted in the live app -- viewable
//! only via `bin/node_editor_harness.rs`.
//!
//! Structure mirrors `inspector_panel.rs`: a `View` deref owns the event area and
//! (nothing else -- `show_bg` is off); the pane frame and all body content are
//! drawn manually in `draw_walk` with `DrawColor`/`DrawText`/`DrawQuad`, and hit
//! rects are captured there for `handle_event`. All state is widget-local
//! (`#[rust]`), no Model wiring: this is a standalone design surface.
//!
//! The single runtime variable that colours everything is the accent (one of the
//! 8 Atlas `bucket_*` swatches, chosen in the Appearance row). Rather than the
//! theme's fixed blue, each accent-tinted surface is recoloured per draw: solid
//! fills via `DrawColor.color`, and the AccentFrame border via a per-draw
//! `set_uniform` of `border_hi`/`border_lo` (the shared frame material reads
//! those two stops -- see `frame.rs`).

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.draw
    use mod.widgets.*

    mod.widgets.NodeDesignEditorBase = #(NodeDesignEditor::register_widget(vm))

    mod.widgets.NodeDesignEditor = set_type_default() do mod.widgets.NodeDesignEditorBase{
        width: Fill
        height: Fill
        show_bg: false
        flow: Down

        // The pane / inset-canvas / node-card frame. One shared AccentFrame
        // instance reused for all three surfaces: `.color` set per surface and
        // `border_hi`/`border_lo` pushed per draw so the frame follows the chosen
        // accent (default blue lives in the atlas tokens).
        draw_pane: mod.draw.AccentFrame{ color: atlas.field_bg }

        // A solid fill (accent or a token colour set from Rust): swatches,
        // toggle-on knob track, segmented-on cell, port nubs.
        draw_fill: mod.draw.DrawColor{ color: atlas.accent }
        // A translucent accent wash (chip bg, header Fill band, hover): `.color`
        // set to vec4(accent.rgb, a) per draw.
        draw_tint: mod.draw.DrawColor{ color: atlas.selection }
        // Accent hairline divider between sections / header / compartments.
        draw_hairline: mod.draw.DrawColor{ color: atlas.accent_soft }

        // Identity palette glyph (rounded square outline + 3 dots), accent-tinted.
        draw_ico: mod.draw.DrawQuad{
            color: uniform(atlas.accent)
            hollow: uniform(atlas.field_bg)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let s = self.rect_size
                sdf.box(s.x * 0.18, s.y * 0.18, s.x * 0.64, s.y * 0.64, 3.0)
                sdf.fill_keep(self.hollow)
                sdf.stroke(self.color, 1.6)
                sdf.circle(s.x * 0.38, s.y * 0.38, 1.4)
                sdf.fill(self.color)
                sdf.circle(s.x * 0.62, s.y * 0.38, 1.4)
                sdf.fill(self.color)
                sdf.circle(s.x * 0.38, s.y * 0.62, 1.4)
                sdf.fill(self.color)
                return sdf.result
            }
        }

        // Compartment drag grip: two stacked horizontal bars (accent-tinted).
        draw_grip: mod.draw.DrawQuad{
            color: uniform(atlas.text_dim)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let s = self.rect_size
                sdf.move_to(s.x * 0.25, s.y * 0.38)
                sdf.line_to(s.x * 0.75, s.y * 0.38)
                sdf.stroke(self.color, 1.8)
                sdf.move_to(s.x * 0.25, s.y * 0.62)
                sdf.line_to(s.x * 0.75, s.y * 0.62)
                sdf.stroke(self.color, 1.8)
                return sdf.result
            }
        }

        // Eyebrow / stereotype: small mono, accent-coloured («node design», «entity»).
        draw_eyebrow +: {
            color: atlas.accent
            text_style: TextStyle{
                font_size: 9
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        // Type name: bold sans ("Entity").
        draw_name +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 15
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Bold.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        // Node-card member rows: mono regular. `.color` is set per segment from
        // Rust (accent visibility marker, dim type/return, amber cardinality).
        draw_mono +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        // Node-card type name: mono bold ("ORDER").
        draw_node_name +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 12
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Bold.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        // Controls: uppercase section headers + sub-headings (semibold sans).
        draw_section +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 10
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-SemiBold.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        // Controls: field labels, segmented/chip text, notes (regular sans).
        // `.color` set per use (dim label, white on-cell, dark chip).
        draw_ctrl +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
    }
}

/// RGB hex (no alpha) -> opaque `Vec4`, matching how the DSL decodes `#xrrggbb`.
/// (Local copy of `inspector_panel::rgb`; the editor crate has no lib target to
/// share it through.)
fn rgb(hex: u32) -> Vec4 {
    Vec4 {
        x: ((hex >> 16) & 0xff) as f32 / 255.0,
        y: ((hex >> 8) & 0xff) as f32 / 255.0,
        z: (hex & 0xff) as f32 / 255.0,
        w: 1.0,
    }
}

/// The 8 accent swatches, in the mock's order (Appearance row / node_style
/// buckets). Values are the Atlas `bucket_*` hexes (see `theme_atlas.rs`); kept
/// as Rust consts so the accent can be recoloured per draw without reading the
/// live theme back out.
const ACCENTS: [(&str, u32); 8] = [
    ("Blue", 0x1496dc),
    ("Cyan", 0x00b4d2),
    ("Teal", 0x14bea0),
    ("Indigo", 0x5a6ef0),
    ("Amber", 0xe69614),
    ("Green", 0x3cbe5a),
    ("Rose", 0xeb4678),
    ("Slate", 0x64748b),
];

#[derive(Script, ScriptHook, Widget)]
pub struct NodeDesignEditor {
    /// Container: owns the event area only (`show_bg` off). The pane frame and
    /// all body content are drawn manually over its rect in `draw_walk`.
    #[deref]
    view: View,

    #[redraw]
    #[live]
    draw_pane: DrawColor,
    #[redraw]
    #[live]
    draw_fill: DrawColor,
    #[live]
    draw_tint: DrawColor,
    #[live]
    draw_hairline: DrawColor,
    #[live]
    draw_ico: DrawQuad,
    #[live]
    draw_grip: DrawQuad,
    #[live]
    draw_eyebrow: DrawText,
    #[live]
    draw_name: DrawText,
    #[live]
    draw_mono: DrawText,
    #[live]
    draw_node_name: DrawText,
    #[live]
    draw_section: DrawText,
    #[live]
    draw_ctrl: DrawText,

    /// Selected accent (index into `ACCENTS`). Recolours the whole preview + pane.
    #[rust]
    accent_idx: usize,
    #[rust]
    view_rect: Rect,
    /// Interactive hit rects captured during `draw_walk`, matched in
    /// `handle_event` (FingerUp). Cleared and rebuilt every draw.
    #[rust]
    regions: Vec<(Region, Rect)>,

    /// Preview state -- every control mutates one of these and the preview
    /// re-reads them each draw. Seeded to the mock's defaults; the control
    /// sections (later steps) flip them.
    #[rust(PreviewState::seed())]
    state: PreviewState,
}

/// Which member compartment a body row is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Compartment {
    Attributes,
    Operations,
}

/// An interactive region recorded in `draw_walk` and resolved on FingerUp.
#[derive(Clone, Copy, PartialEq)]
enum Region {
    /// Accent swatch `n` (0..8).
    Swatch(usize),
    /// Header "Show" toggle.
    HeaderShow,
    /// Ports "Show" toggle.
    PortsShow,
    /// A body compartment's on/off toggle (index into `state.comps`).
    CompToggle(usize),
    /// Header Style segmented: `true` = Fill, `false` = Band.
    HeaderStyle(bool),
    /// Render cap segmented: 0 = All, else 1..=5.
    Render(usize),
    /// A compartment column chip: (comp index, column index 0..3; 0 = locked).
    Column(usize, usize),
    /// Remove stereotype chip `n`.
    ChipRemove(usize),
}

/// One stacked body compartment: its kind and whether it renders.
#[derive(Clone)]
struct CompRow {
    kind: Compartment,
    on: bool,
}

/// The full live-preview model. Mirrors the mock's node classes/dataset. All
/// widget-local; no Model.
#[derive(Clone)]
struct PreviewState {
    header_show: bool,
    /// false = Band (hairline under header), true = Fill (accent-washed band).
    header_fill: bool,
    /// Allowed stereotypes (guillemet chips). The node band shows the first.
    stereotypes: Vec<String>,
    /// Render cap: 0 = All, else 1..=5 (how many allowed stereotypes to show).
    render_cap: usize,
    /// Body compartments, in stack order (drag-reorder mutates this vec).
    comps: Vec<CompRow>,
    /// Attribute columns: [visibility, type, cardinality] (Name is locked on).
    at_cols: [bool; 3],
    /// Operation columns: [visibility, params, return] (Name is locked on).
    op_cols: [bool; 3],
    ports_show: bool,
}

impl PreviewState {
    fn seed() -> Self {
        Self {
            header_show: true,
            header_fill: false,
            stereotypes: vec!["entity".into(), "aggregate".into()],
            render_cap: 0,
            comps: vec![
                CompRow { kind: Compartment::Attributes, on: true },
                CompRow { kind: Compartment::Operations, on: false },
            ],
            at_cols: [true, true, false],
            op_cols: [true, true, true],
            ports_show: false,
        }
    }
}

/// A member row's fixed sample cells (from the mock dataset): visibility marker,
/// name, type-or-params, cardinality-or-return. Empty cells are skipped.
struct MemberRow {
    vis: &'static str,
    name: &'static str,
    ty: &'static str,
    tail: &'static str,
}

const ATTRIBUTES: [MemberRow; 3] = [
    MemberRow { vis: "+", name: "id", ty: " : UUID", tail: " [1]" },
    MemberRow { vis: "+", name: "total", ty: " : Money", tail: " [1]" },
    MemberRow { vis: "-", name: "items", ty: " : Line", tail: " [1..*]" },
];
const OPERATIONS: [MemberRow; 2] = [
    MemberRow { vis: "+", name: "place", ty: "(pay)", tail: " : void" },
    MemberRow { vis: "+", name: "cancel", ty: "()", tail: " : void" },
];

// Pane geometry (px), read off the mock.
const PANE_W: f64 = 660.0;
const PANE_H: f64 = 560.0;
const HEAD_H: f64 = 50.0;
const PAD: f64 = 15.0;
// Left inset-canvas cell (holds the live preview) and the node card inside it.
const STAGE_W: f64 = 258.0;
const NODE_W: f64 = 200.0;
const NODE_HDR_H: f64 = 34.0;
const NODE_ROW_H: f64 = 17.0;
const NODE_COMP_PAD: f64 = 7.0;
// Mono advance at font_size 11 (IBM Plex Mono is 600/1000 em). Approximate --
// used only to place the coloured row segments; tune by eye in the harness.
const MONO_CW: f64 = 6.4;
// Controls column.
const CTRL_PAD_X: f64 = 16.0;
const CTRL_PAD_TOP: f64 = 12.0;
const CTRL_PAD_BOT: f64 = 14.0;
const SEC_H_GAP: f64 = 18.0; // section header baseline -> first field
const FIELD_LABEL_W: f64 = 66.0;
const FIELD_H: f64 = 27.0;
const SWATCH: f64 = 20.0;
const SWATCH_GAP: f64 = 6.0;
// Approx sans advance at font_size 11 (for chip / seg-cell width estimates).
const SANS_CW: f64 = 5.6;
const TOGGLE_W: f64 = 28.0;
const TOGGLE_H: f64 = 18.0;
const KNOB: f64 = 14.0;
const CHIP_H: f64 = 22.0;
const TAGFIELD_H: f64 = 34.0;

impl NodeDesignEditor {
    /// Mutate widget state for a clicked region. Every branch mirrors the mock's
    /// JS handler for that control.
    fn apply_region(&mut self, region: Region) {
        match region {
            Region::Swatch(i) => self.accent_idx = i,
            Region::HeaderShow => self.state.header_show = !self.state.header_show,
            Region::PortsShow => self.state.ports_show = !self.state.ports_show,
            Region::CompToggle(i) => {
                if let Some(c) = self.state.comps.get_mut(i) {
                    c.on = !c.on;
                }
            }
            Region::HeaderStyle(fill) => self.state.header_fill = fill,
            Region::Render(n) => self.state.render_cap = n,
            Region::Column(ci, col) => {
                // Column 0 (Name) is locked on.
                if col == 0 {
                    return;
                }
                let idx = col - 1;
                if let Some(c) = self.state.comps.get(ci) {
                    let cols = match c.kind {
                        Compartment::Attributes => &mut self.state.at_cols,
                        Compartment::Operations => &mut self.state.op_cols,
                    };
                    cols[idx] = !cols[idx];
                }
            }
            Region::ChipRemove(n) => {
                if n < self.state.stereotypes.len() {
                    self.state.stereotypes.remove(n);
                }
            }
        }
    }

    /// Current accent colour (opaque).
    fn accent(&self) -> Vec4 {
        rgb(ACCENTS[self.accent_idx].1)
    }

    /// Same accent at a given alpha (for translucent washes / border stops).
    fn accent_a(&self, a: f32) -> Vec4 {
        let mut c = self.accent();
        c.w = a;
        c
    }

    /// Draw an AccentFrame surface: fill `color`, border recoloured to the
    /// current accent (bright top-left stop -> dim bottom-right, matching the
    /// atlas frame_hi/frame_lo alphas).
    fn draw_frame(&mut self, cx: &mut Cx2d, rect: Rect, fill: Vec4) {
        let hi = self.accent_a(0.95);
        let lo = self.accent_a(0.50);
        self.draw_pane
            .set_uniform(cx, live_id!(border_hi), &[hi.x, hi.y, hi.z, hi.w]);
        self.draw_pane
            .set_uniform(cx, live_id!(border_lo), &[lo.x, lo.y, lo.z, lo.w]);
        self.draw_pane.color = fill;
        self.draw_pane.draw_abs(cx, rect);
    }
}

impl Widget for NodeDesignEditor {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if let Hit::FingerUp(fe) = event.hits(cx, self.view.area()) {
            if !fe.is_primary_hit() {
                return;
            }
            let Some(region) = self
                .regions
                .iter()
                .find(|(_, r)| r.contains(fe.abs))
                .map(|(reg, _)| *reg)
            else {
                return;
            };
            self.apply_region(region);
            self.view.redraw(cx);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}
        let rect = self.view.area().rect(cx);
        self.view_rect = rect;
        self.regions.clear();

        // The pane is centred at its fixed mock width; the harness window is the
        // stage ground. Height fits the content (grows as sections land).
        let pane = Rect {
            pos: dvec2(
                rect.pos.x + (rect.size.x - PANE_W).max(0.0) * 0.5,
                rect.pos.y + 40.0,
            ),
            size: dvec2(PANE_W.min(rect.size.x), PANE_H),
        };
        let field_bg = rgb(0xffffff);
        self.draw_frame(cx, pane, field_bg);

        self.draw_identity_header(cx, pane);
        self.draw_preview(cx, pane);
        self.draw_controls(cx, pane);

        DrawStep::done()
    }
}

impl NodeDesignEditor {
    /// Identity header strip: palette glyph + «node design» eyebrow + type name
    /// "Entity", closed by an accent hairline.
    fn draw_identity_header(&mut self, cx: &mut Cx2d, pane: Rect) {
        let x = pane.pos.x + PAD;
        let cy = pane.pos.y + HEAD_H * 0.5;

        let ico = Rect {
            pos: dvec2(x, cy - 10.0),
            size: dvec2(20.0, 20.0),
        };
        let acc = self.accent();
        self.draw_ico
            .set_uniform(cx, live_id!(color), &[acc.x, acc.y, acc.z, acc.w]);
        self.draw_ico.draw_abs(cx, ico);

        let tx = ico.pos.x + ico.size.x + 11.0;
        self.draw_eyebrow.color = self.accent();
        self.draw_eyebrow
            .draw_abs(cx, dvec2(tx, cy - 13.0), "\u{ab}NODE DESIGN\u{bb}");
        self.draw_name.draw_abs(cx, dvec2(tx, cy - 1.0), "Entity");

        // Accent hairline under the header.
        self.draw_hairline.color = self.accent_a(0.22);
        self.draw_hairline.draw_abs(
            cx,
            Rect {
                pos: dvec2(pane.pos.x, pane.pos.y + HEAD_H),
                size: dvec2(pane.size.x, 1.0),
            },
        );
    }

    /// Left body pane: the inset canvas cell holding the live node preview,
    /// closed on the right by the accent divider to the controls column.
    fn draw_preview(&mut self, cx: &mut Cx2d, pane: Rect) {
        let body_top = pane.pos.y + HEAD_H + 1.0;
        let body_h = pane.size.y - HEAD_H - 1.0;
        let stage = Rect {
            pos: dvec2(pane.pos.x, body_top),
            size: dvec2(STAGE_W, body_h),
        };
        // Inset canvas ground (a step below the pane's white surface).
        self.draw_fill.color = rgb(0xe9eef4);
        self.draw_fill.draw_abs(cx, stage);
        // Divider to the controls column.
        self.draw_hairline.color = self.accent_a(0.14);
        self.draw_hairline.draw_abs(
            cx,
            Rect {
                pos: dvec2(stage.pos.x + stage.size.x, body_top),
                size: dvec2(1.0, body_h),
            },
        );

        // The node card, centred in the stage cell.
        let card_h = self.node_card_height();
        let card = Rect {
            pos: dvec2(
                stage.pos.x + (stage.size.x - NODE_W) * 0.5,
                stage.pos.y + (stage.size.y - card_h) * 0.5,
            ),
            size: dvec2(NODE_W, card_h),
        };
        self.draw_node_card(cx, card);
    }

    /// Total height of the node card for the current state: header (if shown) +
    /// each enabled compartment.
    fn node_card_height(&self) -> f64 {
        let mut h = 0.0;
        if self.state.header_show {
            h += NODE_HDR_H;
        }
        for comp in &self.state.comps {
            if comp.on {
                let n = self.member_rows(comp.kind).len() as f64;
                h += NODE_COMP_PAD * 2.0 + n * NODE_ROW_H;
            }
        }
        h.max(NODE_HDR_H)
    }

    /// Allowed stereotypes capped by Render (0 = All).
    fn shown_stereotypes(&self) -> Vec<String> {
        let cap = if self.state.render_cap == 0 {
            self.state.stereotypes.len()
        } else {
            self.state.render_cap
        };
        self.state.stereotypes.iter().take(cap).cloned().collect()
    }

    fn member_rows(&self, kind: Compartment) -> &'static [MemberRow] {
        match kind {
            Compartment::Attributes => &ATTRIBUTES,
            Compartment::Operations => &OPERATIONS,
        }
    }

    /// The live node card: an accent-framed white surface with the header band
    /// and each enabled compartment stacked top->bottom, an accent hairline
    /// between blocks, plus edge port nubs when ports are shown.
    fn draw_node_card(&mut self, cx: &mut Cx2d, card: Rect) {
        self.draw_frame(cx, card, rgb(0xffffff));

        let mut y = card.pos.y;
        let mut first = true;

        if self.state.header_show {
            // Fill treatment washes the band in accent; Band leaves it clear.
            if self.state.header_fill {
                self.draw_tint.color = self.accent_a(0.12);
                self.draw_tint.draw_abs(
                    cx,
                    Rect {
                        pos: dvec2(card.pos.x, y),
                        size: dvec2(card.size.x, NODE_HDR_H),
                    },
                );
            }
            // The band shows the allowed stereotypes in order, capped by Render
            // (0 = All); empty falls back to «entity». Each is guillemet-wrapped.
            let shown = self.shown_stereotypes();
            let band = if shown.is_empty() {
                "\u{ab}entity\u{bb}".to_string()
            } else {
                shown
                    .iter()
                    .map(|s| format!("\u{ab}{s}\u{bb}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            self.draw_eyebrow.color = self.accent();
            self.draw_eyebrow
                .draw_abs(cx, dvec2(card.pos.x + 11.0, y + 8.0), &band);
            self.draw_node_name
                .draw_abs(cx, dvec2(card.pos.x + 11.0, y + 18.0), "ORDER");
            y += NODE_HDR_H;
            first = false;
        }

        for comp in self.state.comps.clone() {
            if !comp.on {
                continue;
            }
            if !first {
                self.draw_hairline.color = self.accent_a(0.14);
                self.draw_hairline.draw_abs(
                    cx,
                    Rect {
                        pos: dvec2(card.pos.x, y),
                        size: dvec2(card.size.x, 1.0),
                    },
                );
            }
            first = false;
            y += NODE_COMP_PAD;
            for row in self.member_rows(comp.kind) {
                self.draw_member_row(cx, card.pos.x + 11.0, y, comp.kind, row);
                y += NODE_ROW_H;
            }
            y += NODE_COMP_PAD;
        }

        if self.state.ports_show {
            self.draw_port_nubs(cx, card);
        }
    }

    /// One coloured member row, laid out left->right by mono char advance. Cells
    /// are gated by the compartment's column toggles; Name is always shown.
    fn draw_member_row(
        &mut self,
        cx: &mut Cx2d,
        x: f64,
        y: f64,
        kind: Compartment,
        row: &MemberRow,
    ) {
        let cols = match kind {
            Compartment::Attributes => self.state.at_cols,
            Compartment::Operations => self.state.op_cols,
        };
        let mut cx_x = x;
        // [0] visibility marker (accent).
        if cols[0] {
            self.draw_mono.color = self.accent();
            self.draw_mono.draw_abs(cx, dvec2(cx_x, y), row.vis);
            cx_x += (row.vis.chars().count() as f64 + 1.0) * MONO_CW;
        }
        // Name (always, text colour).
        self.draw_mono.color = rgb(0x3a4552);
        self.draw_mono.draw_abs(cx, dvec2(cx_x, y), row.name);
        cx_x += row.name.chars().count() as f64 * MONO_CW;
        // [1] type / params (dim).
        if cols[1] {
            self.draw_mono.color = rgb(0x7b8797);
            self.draw_mono.draw_abs(cx, dvec2(cx_x, y), row.ty);
            cx_x += row.ty.chars().count() as f64 * MONO_CW;
        }
        // [2] cardinality (amber) / return (dim).
        if cols[2] {
            let tail_color = match kind {
                Compartment::Attributes => rgb(0xa58a2a),
                Compartment::Operations => rgb(0x7b8797),
            };
            self.draw_mono.color = tail_color;
            self.draw_mono.draw_abs(cx, dvec2(cx_x, y), row.tail);
        }
    }

    /// Port nubs: small accent-framed white squares straddling the card edges
    /// (two left, one right), matching the mock's `.ports i` positions.
    fn draw_port_nubs(&mut self, cx: &mut Cx2d, card: Rect) {
        let sz = 8.0;
        let nubs = [
            dvec2(card.pos.x - 4.0, card.pos.y + card.size.y * 0.44),
            dvec2(card.pos.x - 4.0, card.pos.y + card.size.y * 0.66),
            dvec2(card.pos.x + card.size.x - 4.0, card.pos.y + card.size.y * 0.52),
        ];
        for p in nubs {
            self.draw_frame(
                cx,
                Rect {
                    pos: p,
                    size: dvec2(sz, sz),
                },
                rgb(0xffffff),
            );
        }
    }

    // ---- controls column -------------------------------------------------

    /// Right controls column: accent-hairline-split sections, each drawn by its
    /// own method. A running `y` cursor threads through; each section returns the
    /// `y` below it and draws its closing hairline.
    fn draw_controls(&mut self, cx: &mut Cx2d, pane: Rect) {
        let x = pane.pos.x + STAGE_W + 1.0;
        let w = pane.size.x - STAGE_W - 1.0;
        let mut y = pane.pos.y + HEAD_H + 1.0;

        y = self.section_appearance(cx, x, w, y);
        y = self.section_header_controls(cx, x, w, y);
        y = self.section_body(cx, x, w, y);
        self.section_ports(cx, x, y);
    }

    /// Draw a section's uppercase header at (x, y); returns the y of the first
    /// field below it.
    fn section_header(&mut self, cx: &mut Cx2d, x: f64, y: f64, text: &str) -> f64 {
        self.draw_section.color = rgb(0x8a97a6);
        self.draw_section
            .draw_abs(cx, dvec2(x + CTRL_PAD_X, y + CTRL_PAD_TOP), text);
        y + CTRL_PAD_TOP + SEC_H_GAP
    }

    /// Close a section with the full-width accent hairline; returns y below it.
    fn section_close(&mut self, cx: &mut Cx2d, x: f64, w: f64, y: f64) -> f64 {
        let yy = y + CTRL_PAD_BOT;
        self.draw_hairline.color = self.accent_a(0.14);
        self.draw_hairline.draw_abs(
            cx,
            Rect {
                pos: dvec2(x, yy),
                size: dvec2(w, 1.0),
            },
        );
        yy + 1.0
    }

    /// A field-row label (dim, in the fixed left label column).
    fn field_label(&mut self, cx: &mut Cx2d, x: f64, y: f64, text: &str) {
        self.draw_ctrl.color = rgb(0x6a7686);
        self.draw_ctrl
            .draw_abs(cx, dvec2(x + CTRL_PAD_X, y + (FIELD_H - 13.0) * 0.5), text);
    }

    // ---- shared control primitives --------------------------------------

    /// HUD toggle: a squared accent-framed track with a knob that slides right
    /// and the track fills accent when `on`. Records `region` as the hit target.
    fn draw_toggle(&mut self, cx: &mut Cx2d, rect: Rect, on: bool, region: Region) {
        if on {
            // Filled accent track, knob (white) to the right.
            self.draw_fill.color = self.accent();
            self.draw_fill.draw_abs(cx, rect);
            let knob = Rect {
                pos: dvec2(rect.pos.x + rect.size.x - KNOB - 2.0, rect.pos.y + 2.0),
                size: dvec2(KNOB, KNOB),
            };
            self.draw_fill.color = rgb(0xffffff);
            self.draw_fill.draw_abs(cx, knob);
        } else {
            // Empty white track (accent border), knob (accent-outlined) to the left.
            self.draw_frame(cx, rect, rgb(0xffffff));
            let knob = Rect {
                pos: dvec2(rect.pos.x + 2.0, rect.pos.y + 2.0),
                size: dvec2(KNOB, KNOB),
            };
            self.draw_frame(cx, knob, rgb(0xffffff));
        }
        self.regions.push((region, rect));
    }

    /// Segmented control: cells left->right, the selected one accent-filled with
    /// white text, the rest white with dim text; the whole ringed by the accent
    /// frame. `stretch` splits `total_w` equally (Render cap); otherwise cells are
    /// content-sized (Band/Fill). Returns each cell's rect (caller maps regions).
    fn draw_segmented(
        &mut self,
        cx: &mut Cx2d,
        x: f64,
        y: f64,
        total_w: f64,
        labels: &[&str],
        selected: usize,
        stretch: bool,
        active: bool,
    ) -> Vec<Rect> {
        let n = labels.len();
        // Cell widths.
        let widths: Vec<f64> = if stretch {
            vec![total_w / n as f64; n]
        } else {
            labels
                .iter()
                .map(|l| l.chars().count() as f64 * SANS_CW + 26.0)
                .collect()
        };
        let mut rects = Vec::with_capacity(n);
        let mut cx_x = x;
        for (i, (label, cw)) in labels.iter().zip(widths.iter()).enumerate() {
            let cell = Rect {
                pos: dvec2(cx_x, y),
                size: dvec2(*cw, FIELD_H),
            };
            if i == selected && active {
                self.draw_fill.color = self.accent();
                self.draw_fill.draw_abs(cx, cell);
                self.draw_ctrl.color = rgb(0xffffff);
            } else {
                self.draw_fill.color = rgb(0xffffff);
                self.draw_fill.draw_abs(cx, cell);
                self.draw_ctrl.color = rgb(0x7b8797);
                // Divider hairline on the left edge of non-first cells.
                if i != 0 {
                    self.draw_hairline.color = self.accent_a(0.18);
                    self.draw_hairline.draw_abs(
                        cx,
                        Rect {
                            pos: dvec2(cx_x, y),
                            size: dvec2(1.0, FIELD_H),
                        },
                    );
                }
            }
            let tw = label.chars().count() as f64 * SANS_CW;
            self.draw_ctrl.draw_abs(
                cx,
                dvec2(cx_x + (cw - tw) * 0.5, y + (FIELD_H - 13.0) * 0.5),
                label,
            );
            rects.push(cell);
            cx_x += cw;
        }
        // Whole-control accent ring (stroke only, transparent fill).
        self.draw_frame(
            cx,
            Rect {
                pos: dvec2(x, y),
                size: dvec2(cx_x - x, FIELD_H),
            },
            Vec4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 },
        );
        rects
    }

    // ---- sections --------------------------------------------------------

    /// Appearance: "Accent" label + the 8-swatch row. Selecting a swatch
    /// recolours everything.
    fn section_appearance(&mut self, cx: &mut Cx2d, x: f64, w: f64, y: f64) -> f64 {
        let mut y = self.section_header(cx, x, y, "APPEARANCE");
        self.field_label(cx, x, y, "Accent");

        let sx = x + CTRL_PAD_X + FIELD_LABEL_W + 12.0;
        let sy = y + (FIELD_H - SWATCH) * 0.5;
        for i in 0..ACCENTS.len() {
            let r = Rect {
                pos: dvec2(sx + (i as f64) * (SWATCH + SWATCH_GAP), sy),
                size: dvec2(SWATCH, SWATCH),
            };
            // Solid swatch in that bucket colour.
            self.draw_fill.color = rgb(ACCENTS[i].1);
            self.draw_fill.draw_abs(cx, r);
            // Selected: accent outline (drawn as a slightly larger frame ring).
            if i == self.accent_idx {
                let ring = Rect {
                    pos: dvec2(r.pos.x - 2.0, r.pos.y - 2.0),
                    size: dvec2(SWATCH + 4.0, SWATCH + 4.0),
                };
                self.draw_frame(cx, ring, Vec4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 });
            }
            self.regions.push((Region::Swatch(i), r));
        }
        y += FIELD_H;
        self.section_close(cx, x, w, y)
    }

    /// Header: Show toggle, Band/Fill Style, Allowed-stereotypes tag field +
    /// note, Render cap. When Show is off the sub-controls read inert (regions
    /// withheld). PLACEHOLDER: the mock also visually fades those rows when off;
    /// opaque hand-drawn pens can't cheaply alpha-dim, so they stay full-strength.
    fn section_header_controls(&mut self, cx: &mut Cx2d, x: f64, w: f64, y: f64) -> f64 {
        let mut y = self.section_header(cx, x, y, "HEADER");
        let ctrl_x = x + CTRL_PAD_X + FIELD_LABEL_W + 12.0;
        let active = self.state.header_show;

        // Show toggle.
        self.field_label(cx, x, y, "Show");
        let tog = Rect {
            pos: dvec2(ctrl_x, y + (FIELD_H - TOGGLE_H) * 0.5),
            size: dvec2(TOGGLE_W, TOGGLE_H),
        };
        self.draw_toggle(cx, tog, self.state.header_show, Region::HeaderShow);
        y += FIELD_H;

        // Style segmented [Band|Fill].
        self.field_label(cx, x, y, "Style");
        let sel = if self.state.header_fill { 1 } else { 0 };
        let cells = self.draw_segmented(cx, ctrl_x, y, 0.0, &["Band", "Fill"], sel, false, active);
        if active {
            for (i, cell) in cells.into_iter().enumerate() {
                self.regions.push((Region::HeaderStyle(i == 1), cell));
            }
        }
        y += FIELD_H + 6.0;

        // Allowed-stereotypes sub-heading + tag field + note.
        self.draw_section.color = rgb(0x9aa6b4);
        self.draw_section
            .draw_abs(cx, dvec2(x + CTRL_PAD_X, y), "ALLOWED STEREOTYPES");
        y += 16.0;
        y = self.draw_tagfield(cx, x, w, y, active);
        self.draw_ctrl.color = rgb(0x9aa6b4);
        self.draw_ctrl.draw_abs(
            cx,
            dvec2(x + CTRL_PAD_X, y + 7.0),
            "Shown in order. Empty will show all.",
        );
        y += 22.0;

        // Render cap [All|1|2|3|4|5], stretched full width.
        self.field_label(cx, x, y, "Render");
        let render_w = w - (CTRL_PAD_X + FIELD_LABEL_W + 12.0) - CTRL_PAD_X;
        let sel = self.state.render_cap; // 0 = All
        let labels = ["All", "1", "2", "3", "4", "5"];
        let cells = self.draw_segmented(cx, ctrl_x, y, render_w, &labels, sel, true, active);
        if active {
            for (i, cell) in cells.into_iter().enumerate() {
                self.regions.push((Region::Render(i), cell));
            }
        }
        y += FIELD_H;
        self.section_close(cx, x, w, y)
    }

    /// The stereotype tag field: an accent-framed white box holding the guillemet
    /// chips (accent-tinted, «label») and a placeholder entry. Returns y below
    /// the box. PLACEHOLDER: adding stereotypes needs a hand-rolled text entry
    /// (see inspector_panel's edit path) -- deferred; the entry is display-only.
    /// Clicking a chip removes it (the mock's hover-✕; simplified to whole-chip).
    fn draw_tagfield(&mut self, cx: &mut Cx2d, x: f64, w: f64, y: f64, active: bool) -> f64 {
        let box_rect = Rect {
            pos: dvec2(x + CTRL_PAD_X, y),
            size: dvec2(w - CTRL_PAD_X * 2.0, TAGFIELD_H),
        };
        self.draw_frame(cx, box_rect, rgb(0xffffff));

        let mut cxp = box_rect.pos.x + 8.0;
        let chip_y = box_rect.pos.y + (TAGFIELD_H - CHIP_H) * 0.5;
        let stereos = self.state.stereotypes.clone();
        for (i, s) in stereos.iter().enumerate() {
            let label = format!("\u{ab}{s}\u{bb}");
            let cw = label.chars().count() as f64 * SANS_CW + 16.0;
            let chip = Rect {
                pos: dvec2(cxp, chip_y),
                size: dvec2(cw, CHIP_H),
            };
            self.draw_tint.color = self.accent_a(0.20);
            self.draw_tint.draw_abs(cx, chip);
            // Guillemets accent, label dark (drawn as one accent-guillemet string
            // plus the dark inner label offset by one glyph).
            self.draw_ctrl.color = self.accent();
            self.draw_ctrl
                .draw_abs(cx, dvec2(cxp + 8.0, chip_y + 5.0), "\u{ab}");
            self.draw_ctrl.color = rgb(0x22303c);
            self.draw_ctrl
                .draw_abs(cx, dvec2(cxp + 8.0 + SANS_CW, chip_y + 5.0), s);
            self.draw_ctrl.color = self.accent();
            self.draw_ctrl.draw_abs(
                cx,
                dvec2(cxp + 8.0 + SANS_CW + s.chars().count() as f64 * SANS_CW, chip_y + 5.0),
                "\u{bb}",
            );
            if active {
                self.regions.push((Region::ChipRemove(i), chip));
            }
            cxp += cw + 4.0;
        }
        // Entry placeholder.
        self.draw_ctrl.color = rgb(0xa4b0bd);
        self.draw_ctrl
            .draw_abs(cx, dvec2(cxp + 4.0, chip_y + 5.0), "Add stereotype\u{2026}");

        box_rect.pos.y + TAGFIELD_H
    }

    /// A column chip (which member field a compartment renders). `on` fills the
    /// accent tint; `lock` is the dashed non-interactive Name chip; a disabled
    /// (`!enabled`) compartment dims + inerts its chips. Returns the chip width.
    fn draw_col_chip(
        &mut self,
        cx: &mut Cx2d,
        x: f64,
        y: f64,
        label: &str,
        on: bool,
        lock: bool,
        enabled: bool,
    ) -> f64 {
        let cw = label.chars().count() as f64 * SANS_CW + 18.0;
        let chip = Rect {
            pos: dvec2(x, y),
            size: dvec2(cw, 20.0),
        };
        if on && !lock {
            self.draw_tint.color = self.accent_a(if enabled { 0.15 } else { 0.06 });
            self.draw_tint.draw_abs(cx, chip);
        } else {
            self.draw_fill.color = rgb(0xffffff);
            self.draw_fill.draw_abs(cx, chip);
        }
        // Accent border ring. PLACEHOLDER: the locked Name chip's dashed border
        // isn't expressible with the flat AccentFrame stroke -- drawn solid.
        self.draw_frame(cx, chip, Vec4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 });
        let color = if !enabled {
            rgb(0xaab4c1)
        } else if lock {
            rgb(0x7b8797)
        } else if on {
            rgb(0x22303c)
        } else {
            rgb(0x8a97a6)
        };
        self.draw_ctrl.color = color;
        self.draw_ctrl.draw_abs(cx, dvec2(x + 9.0, y + 4.0), label);
        cw
    }

    /// The four column chips for a compartment: (label, column index, locked).
    /// Column index 0 = Name (locked); 1..3 map to the compartment's `*_cols`.
    fn comp_columns(kind: Compartment) -> [(&'static str, usize, bool); 4] {
        match kind {
            Compartment::Attributes => [
                ("Name", 0, true),
                ("Visibility", 1, false),
                ("Type", 2, false),
                ("Cardinality", 3, false),
            ],
            Compartment::Operations => [
                ("Name", 0, true),
                ("Visibility", 1, false),
                ("Params", 2, false),
                ("Return", 3, false),
            ],
        }
    }

    /// Body: the stacked compartments, each a grip + label + on/off toggle over a
    /// row of column chips. A disabled compartment dims its label + chips.
    /// PLACEHOLDER: drag-to-reorder is not wired (the grip is inert) -- the order
    /// is static; reordering would just permute `state.comps` (the preview
    /// already mirrors that vec's order).
    fn section_body(&mut self, cx: &mut Cx2d, x: f64, w: f64, y: f64) -> f64 {
        let mut y = self.section_header(cx, x, y, "BODY \u{b7} DRAG TO REORDER");
        y -= 4.0;

        let comps = self.state.comps.clone();
        for (ci, comp) in comps.iter().enumerate() {
            // Item top hairline.
            self.draw_hairline.color = self.accent_a(0.16);
            self.draw_hairline.draw_abs(
                cx,
                Rect {
                    pos: dvec2(x + CTRL_PAD_X, y),
                    size: dvec2(w - CTRL_PAD_X * 2.0, 1.0),
                },
            );

            // Compartment row: grip + label + toggle.
            let crow_h = 30.0;
            let cy = y + crow_h * 0.5;
            let grip = Rect {
                pos: dvec2(x + CTRL_PAD_X + 2.0, cy - 8.0),
                size: dvec2(16.0, 16.0),
            };
            let grip_col = if comp.on { self.accent() } else { rgb(0xb3bdca) };
            self.draw_grip
                .set_uniform(cx, live_id!(color), &[grip_col.x, grip_col.y, grip_col.z, grip_col.w]);
            self.draw_grip.draw_abs(cx, grip);

            let label = match comp.kind {
                Compartment::Attributes => "Attributes",
                Compartment::Operations => "Operations",
            };
            self.draw_ctrl.color = if comp.on { rgb(0x26313f) } else { rgb(0xaab4c1) };
            self.draw_ctrl
                .draw_abs(cx, dvec2(grip.pos.x + 26.0, cy - 6.0), label);

            let tog = Rect {
                pos: dvec2(x + w - CTRL_PAD_X - TOGGLE_W, cy - TOGGLE_H * 0.5),
                size: dvec2(TOGGLE_W, TOGGLE_H),
            };
            self.draw_toggle(cx, tog, comp.on, Region::CompToggle(ci));
            y += crow_h;

            // Column chips, indented under the label.
            let cols = Self::comp_columns(comp.kind);
            let on_flags = match comp.kind {
                Compartment::Attributes => self.state.at_cols,
                Compartment::Operations => self.state.op_cols,
            };
            let mut chx = grip.pos.x + 26.0;
            for (label, col, lock) in cols {
                let on = if lock { true } else { on_flags[col - 1] };
                let cw = self.draw_col_chip(cx, chx, y, label, on, lock, comp.on);
                // Interactive only when enabled and not the locked Name chip.
                if comp.on && !lock {
                    self.regions.push((
                        Region::Column(ci, col),
                        Rect {
                            pos: dvec2(chx, y),
                            size: dvec2(cw, 20.0),
                        },
                    ));
                }
                chx += cw + 5.0;
            }
            y += 20.0 + 8.0;
        }
        // Final bottom hairline for the last item.
        self.draw_hairline.color = self.accent_a(0.16);
        self.draw_hairline.draw_abs(
            cx,
            Rect {
                pos: dvec2(x + CTRL_PAD_X, y),
                size: dvec2(w - CTRL_PAD_X * 2.0, 1.0),
            },
        );
        self.section_close(cx, x, w, y)
    }

    /// Ports: a single Show toggle. Ports sit on the node border, not in the
    /// body stack, so they get a plain on/off (no drag row). Toggling it shows /
    /// hides the preview's edge nubs. Last section -> no closing hairline (mock
    /// `.sect:last-child`).
    fn section_ports(&mut self, cx: &mut Cx2d, x: f64, y: f64) {
        let y = self.section_header(cx, x, y, "PORTS");
        self.field_label(cx, x, y, "Show");
        let tog = Rect {
            pos: dvec2(
                x + CTRL_PAD_X + FIELD_LABEL_W + 12.0,
                y + (FIELD_H - TOGGLE_H) * 0.5,
            ),
            size: dvec2(TOGGLE_W, TOGGLE_H),
        };
        self.draw_toggle(cx, tog, self.state.ports_show, Region::PortsShow);
    }
}
