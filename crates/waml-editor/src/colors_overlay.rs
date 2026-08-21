//! Colors style-guide overlay: every Atlas token, grouped by the `theme_atlas.rs`
//! comment sections. Each row draws a LIVE swatch (wired `draw_swatch_X: DrawColor`
//! = the real token) + the token name + a hex read back from the swatch's `Vec4`
//! (so it tracks the `T` theme flip) + a one-line purpose. Rides `OverlayShell`.

use crate::overlay_shell::{OverlayShell, OverlayShellAction};
use makepad_widgets::*;

/// Selects which wired `draw_swatch_X` a row renders + reads its hex back from.
/// One variant per `mod.atlas` token, in `theme_atlas.rs` order.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Swatch {
    Ground,
    CanvasGround,
    GroupFill,
    Surface,
    SurfaceBorder,
    FieldBg,
    Accent,
    AccentSoft,
    Selection,
    Hover,
    FrameHi,
    FrameLo,
    Scrim,
    Shadow,
    Danger,
    Text,
    TextMid,
    TextDim,
    LogoHi,
    LogoMid,
    LogoLo,
    BucketBlue,
    BucketCyan,
    BucketTeal,
    BucketIndigo,
    BucketAmber,
    BucketGreen,
    BucketRose,
    BucketSlate,
}

/// Hand-authored metadata row. `which` picks the live swatch/hex; `name` is the
/// token field name (must match `mod.atlas`); `purpose` is a one-liner.
pub struct ColorRow {
    pub name: &'static str,
    pub which: Swatch,
    pub purpose: &'static str,
}

macro_rules! cr {
    ($name:literal, $which:ident, $purpose:literal) => {
        ColorRow {
            name: $name,
            which: Swatch::$which,
            purpose: $purpose,
        }
    };
}

/// Grouped by `theme_atlas.rs`'s comment sections.
pub const COLOR_GROUPS: &[(&str, &[ColorRow])] = &[
    (
        "BACKDROPS",
        &[
            cr!("ground", Ground, "App / canvas ground"),
            cr!("canvas_ground", CanvasGround, "Diagram canvas field"),
            cr!("group_fill", GroupFill, "Package / group frame fill"),
        ],
    ),
    (
        "SURFACES",
        &[
            cr!("surface", Surface, "Panels, bars, chips, node bodies"),
            cr!("surface_border", SurfaceBorder, "Surface hairline border"),
            cr!("field_bg", FieldBg, "Editable control fill"),
        ],
    ),
    (
        "ACCENT + FRAME",
        &[
            cr!("accent", Accent, "Brand / interaction accent"),
            cr!("accent_soft", AccentSoft, "Faint accent wash / separators"),
            cr!("selection", Selection, "Active / selected row fill"),
            cr!("hover", Hover, "Hovered row fill, beneath the selection"),
            cr!("frame_hi", FrameHi, "Source-bright frame stop"),
            cr!("frame_lo", FrameLo, "Source-dim frame stop"),
        ],
    ),
    (
        "STATE",
        &[
            cr!("scrim", Scrim, "Modal overlay scrim"),
            cr!("shadow", Shadow, "HUD surface depth-shadow tint"),
            cr!("danger", Danger, "Destructive affordance"),
        ],
    ),
    (
        "TEXT",
        &[
            cr!("text", Text, "Primary text"),
            cr!(
                "text_mid",
                TextMid,
                "Small annotation text (canvas edge labels)"
            ),
            cr!("text_dim", TextDim, "Secondary / meta text"),
        ],
    ),
    (
        "LOGO RAMP",
        &[
            cr!("logo_hi", LogoHi, "Wordmark lightest bar"),
            cr!("logo_mid", LogoMid, "Wordmark mid bar"),
            cr!("logo_lo", LogoLo, "Wordmark darkest bar"),
        ],
    ),
    (
        "NODE BUCKETS",
        &[
            cr!("bucket_blue", BucketBlue, "Node-kind accent: blue"),
            cr!("bucket_cyan", BucketCyan, "Node-kind accent: cyan"),
            cr!("bucket_teal", BucketTeal, "Node-kind accent: teal"),
            cr!("bucket_indigo", BucketIndigo, "Node-kind accent: indigo"),
            cr!("bucket_amber", BucketAmber, "Node-kind accent: amber"),
            cr!("bucket_green", BucketGreen, "Node-kind accent: green"),
            cr!("bucket_rose", BucketRose, "Node-kind accent: rose"),
            cr!("bucket_slate", BucketSlate, "Node-kind accent: slate"),
        ],
    ),
];

/// Format a swatch `Vec4` as `#rrggbb` (+ `aa` when alpha < 1). Components are
/// treated as sRGB 0..1. NOTE: if makepad stores `DrawColor.color` in LINEAR
/// space, wrap each component with the sRGB encode before calling this (verify
/// against a known token at visual-verify time); the formatter contract itself
/// is what this pure function guarantees.
pub fn vec4_to_hex(c: Vec4) -> String {
    let to = |f: f32| (f.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, g, b, a) = (to(c.x), to(c.y), to(c.z), to(c.w));
    if a < 255 {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}")
    }
}

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.fonts

    mod.widgets.ColorsOverlayBase = #(ColorsOverlay::register_widget(vm))

    mod.widgets.ColorsOverlay = set_type_default() do mod.widgets.ColorsOverlayBase{
        width: Fill
        height: Fill
        shell +: {
            panel_width: 520.0
            draw_scrim +: { color: atlas.scrim }
            draw_panel +: { color: atlas.surface }
            draw_edge +: { color: atlas.frame_hi }
            draw_thumb +: { color: atlas.frame_lo }
        }
        draw_group   +: { color: atlas.text_dim  text_style: fonts.text_eyebrow }
        draw_token   +: { color: atlas.text      text_style: fonts.text_mono }
        draw_hex     +: { color: atlas.text_dim  text_style: fonts.text_mono }
        draw_purpose +: { color: atlas.text_dim  text_style: fonts.text_label }
        // Live swatches: each wired to the REAL token so it tracks the T flip.
        draw_swatch_ground         +: { color: atlas.ground }
        draw_swatch_canvas_ground  +: { color: atlas.canvas_ground }
        draw_swatch_group_fill     +: { color: atlas.group_fill }
        draw_swatch_surface        +: { color: atlas.surface }
        draw_swatch_surface_border +: { color: atlas.surface_border }
        draw_swatch_field_bg       +: { color: atlas.field_bg }
        draw_swatch_accent         +: { color: atlas.accent }
        draw_swatch_accent_soft    +: { color: atlas.accent_soft }
        draw_swatch_selection      +: { color: atlas.selection }
        draw_swatch_hover          +: { color: atlas.hover }
        draw_swatch_frame_hi       +: { color: atlas.frame_hi }
        draw_swatch_frame_lo       +: { color: atlas.frame_lo }
        draw_swatch_scrim          +: { color: atlas.scrim }
        draw_swatch_shadow         +: { color: atlas.shadow }
        draw_swatch_danger         +: { color: atlas.danger }
        draw_swatch_text           +: { color: atlas.text }
        draw_swatch_text_mid       +: { color: atlas.text_mid }
        draw_swatch_text_dim       +: { color: atlas.text_dim }
        draw_swatch_logo_hi        +: { color: atlas.logo_hi }
        draw_swatch_logo_mid       +: { color: atlas.logo_mid }
        draw_swatch_logo_lo        +: { color: atlas.logo_lo }
        draw_swatch_bucket_blue    +: { color: atlas.bucket_blue }
        draw_swatch_bucket_cyan    +: { color: atlas.bucket_cyan }
        draw_swatch_bucket_teal    +: { color: atlas.bucket_teal }
        draw_swatch_bucket_indigo  +: { color: atlas.bucket_indigo }
        draw_swatch_bucket_amber   +: { color: atlas.bucket_amber }
        draw_swatch_bucket_green   +: { color: atlas.bucket_green }
        draw_swatch_bucket_rose    +: { color: atlas.bucket_rose }
        draw_swatch_bucket_slate   +: { color: atlas.bucket_slate }
    }
}

#[derive(Clone, Debug, Default)]
pub enum ColorsOverlayAction {
    #[default]
    None,
    Dismissed,
}

const GROUP_H: f64 = 34.0;
const COLOR_ROW_H: f64 = 30.0;
const SWATCH_W: f64 = 26.0;
const SWATCH_H: f64 = 18.0;
const TOKEN_COL_W: f64 = 44.0;
const NAME_COL_W: f64 = 150.0;
const HEX_COL_W: f64 = 110.0;

#[derive(Script, ScriptHook, Widget)]
pub struct ColorsOverlay {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[live]
    shell: OverlayShell,

    #[redraw]
    #[live]
    draw_group: DrawText,
    #[redraw]
    #[live]
    draw_token: DrawText,
    #[redraw]
    #[live]
    draw_hex: DrawText,
    #[redraw]
    #[live]
    draw_purpose: DrawText,

    #[redraw]
    #[live]
    draw_swatch_ground: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_canvas_ground: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_group_fill: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_surface: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_surface_border: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_field_bg: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_accent: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_accent_soft: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_selection: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_hover: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_frame_hi: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_frame_lo: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_scrim: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_shadow: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_danger: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_text: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_text_mid: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_text_dim: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_logo_hi: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_logo_mid: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_logo_lo: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_bucket_blue: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_bucket_cyan: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_bucket_teal: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_bucket_indigo: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_bucket_amber: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_bucket_green: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_bucket_rose: DrawColor,
    #[redraw]
    #[live]
    draw_swatch_bucket_slate: DrawColor,
}

impl Widget for ColorsOverlay {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if let OverlayShellAction::Dismissed = self.shell.handle_event(cx, event) {
            cx.widget_action(self.widget_uid(), ColorsOverlayAction::Dismissed);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, _walk: Walk) -> DrawStep {
        let h = content_height();
        if let Some(pass) = self.shell.begin(cx, h) {
            self.draw_rows(cx, pass.origin, pass.width);
            self.shell.end(cx);
        }
        DrawStep::done()
    }
}

fn content_height() -> f64 {
    let mut h = 0.0;
    for (_, rows) in COLOR_GROUPS {
        h += GROUP_H + rows.len() as f64 * COLOR_ROW_H;
    }
    h
}

impl ColorsOverlay {
    pub fn set_visible(&mut self, cx: &mut Cx, visible: bool) {
        self.shell.set_open(cx, visible);
    }

    pub fn overlay_action(&self, actions: &Actions) -> Option<ColorsOverlayAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            ColorsOverlayAction::None => None,
            a => Some(a),
        }
    }

    /// The `DrawColor` for a swatch (the live token value).
    fn swatch(&mut self, which: Swatch) -> &mut DrawColor {
        match which {
            Swatch::Ground => &mut self.draw_swatch_ground,
            Swatch::CanvasGround => &mut self.draw_swatch_canvas_ground,
            Swatch::GroupFill => &mut self.draw_swatch_group_fill,
            Swatch::Surface => &mut self.draw_swatch_surface,
            Swatch::SurfaceBorder => &mut self.draw_swatch_surface_border,
            Swatch::FieldBg => &mut self.draw_swatch_field_bg,
            Swatch::Accent => &mut self.draw_swatch_accent,
            Swatch::AccentSoft => &mut self.draw_swatch_accent_soft,
            Swatch::Selection => &mut self.draw_swatch_selection,
            Swatch::Hover => &mut self.draw_swatch_hover,
            Swatch::FrameHi => &mut self.draw_swatch_frame_hi,
            Swatch::FrameLo => &mut self.draw_swatch_frame_lo,
            Swatch::Scrim => &mut self.draw_swatch_scrim,
            Swatch::Shadow => &mut self.draw_swatch_shadow,
            Swatch::Danger => &mut self.draw_swatch_danger,
            Swatch::Text => &mut self.draw_swatch_text,
            Swatch::TextMid => &mut self.draw_swatch_text_mid,
            Swatch::TextDim => &mut self.draw_swatch_text_dim,
            Swatch::LogoHi => &mut self.draw_swatch_logo_hi,
            Swatch::LogoMid => &mut self.draw_swatch_logo_mid,
            Swatch::LogoLo => &mut self.draw_swatch_logo_lo,
            Swatch::BucketBlue => &mut self.draw_swatch_bucket_blue,
            Swatch::BucketCyan => &mut self.draw_swatch_bucket_cyan,
            Swatch::BucketTeal => &mut self.draw_swatch_bucket_teal,
            Swatch::BucketIndigo => &mut self.draw_swatch_bucket_indigo,
            Swatch::BucketAmber => &mut self.draw_swatch_bucket_amber,
            Swatch::BucketGreen => &mut self.draw_swatch_bucket_green,
            Swatch::BucketRose => &mut self.draw_swatch_bucket_rose,
            Swatch::BucketSlate => &mut self.draw_swatch_bucket_slate,
        }
    }

    fn draw_rows(&mut self, cx: &mut Cx2d, origin: DVec2, _width: f64) {
        let mut y = origin.y;
        for (title, rows) in COLOR_GROUPS {
            self.draw_group.draw_abs(cx, dvec2(origin.x, y), title);
            y += GROUP_H;
            for row in *rows {
                let sw = self.swatch(row.which);
                let hex = vec4_to_hex(sw.color);
                let rect = Rect {
                    pos: dvec2(origin.x, y - 2.0),
                    size: dvec2(SWATCH_W, SWATCH_H),
                };
                sw.draw_abs(cx, rect);
                let x = origin.x + TOKEN_COL_W;
                self.draw_token.draw_abs(cx, dvec2(x, y), row.name);
                self.draw_hex.draw_abs(cx, dvec2(x + NAME_COL_W, y), &hex);
                self.draw_purpose
                    .draw_abs(cx, dvec2(x + NAME_COL_W + HEX_COL_W, y), row.purpose);
                y += COLOR_ROW_H;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_formats_rgb_and_optional_alpha() {
        assert_eq!(vec4_to_hex(vec4(1.0, 1.0, 1.0, 1.0)), "#ffffff");
        assert_eq!(vec4_to_hex(vec4(0.0, 0.0, 0.0, 1.0)), "#000000");
        // #1496dc == (20,150,220)
        assert_eq!(
            vec4_to_hex(vec4(20.0 / 255.0, 150.0 / 255.0, 220.0 / 255.0, 1.0)),
            "#1496dc"
        );
        // alpha < 1 appends two hex digits.
        assert_eq!(vec4_to_hex(vec4(1.0, 1.0, 1.0, 0.5)), "#ffffff80");
    }

    /// The swatch table (`COLOR_GROUPS`) must cover EXACTLY the keys the real
    /// `mod.themes.atlas_light` namespace produces. Boots the actual script VM
    /// (not a source parse), runs `theme_atlas::script_mod`, reads the produced
    /// object back, and compares id sets — so a token added to one side but not
    /// the other, or a namespace that failed to evaluate, is caught
    /// semantically. Replaces the deleted `include_str!` coverage scan.
    #[test]
    fn atlas_light_keys_equal_the_swatch_table() {
        use crate::script_gate::{boot_test_vm, namespace_key_ids};

        let mut vm = boot_test_vm();
        crate::theme_atlas::script_mod(&mut vm);

        let atlas = script_eval!(vm, { mod.themes.atlas_light });
        let vm_keys = namespace_key_ids(&mut vm, atlas).expect(
            "mod.themes.atlas_light must resolve to an object; a malformed \
             block leaves it absent and every `use mod.atlas` consumer breaks",
        );

        // Expected = the swatch table, mapped name -> LiveId (same hash the
        // `atlas_light` block's `name:` fields produce). Drift-proof: no
        // hardcoded token list, just the live table.
        let expected: std::collections::BTreeSet<LiveId> = COLOR_GROUPS
            .iter()
            .flat_map(|(_, rows)| rows.iter().map(|r| LiveId::from_str(r.name)))
            .collect();

        assert_eq!(
            expected.len(),
            29,
            "expected 29 swatch tokens, got {}",
            expected.len()
        );
        assert_eq!(
            vm_keys, expected,
            "mod.themes.atlas_light keys and the COLOR_GROUPS swatch table drifted apart"
        );
    }
}
