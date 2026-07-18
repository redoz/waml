//! Atlas: the editor's single UI theme. Exactly two modes -- **light** (this
//! task) and **dark** (a later fast-follow) -- no other configurability.
//! Every semantic color lives here, once, as a named live constant; widgets
//! `use mod.atlas` and reference `atlas.<name>` instead of a hardcoded `#x`
//! literal. Adding dark mode later is a second `mod.themes.atlas_dark` block
//! with the same field names, with `mod.atlas` repointed at it -- no widget
//! script_mod changes.
//!
//! Mirrors the fork's own theme wiring: `theme_desktop_dark.rs` defines
//! `mod.themes.dark = { let theme = me ... }`, and `widgets/src/lib.rs` does
//! `mod.theme = mod.themes.dark` plus folds `theme:mod.theme` into
//! `mod.prelude.widgets_internal` so `theme.color_bg_app`/`theme.font_regular`
//! read naturally from any widget's script_mod. `atlas` is the same shape,
//! named separately so it never collides with the fork's own `theme`.
//!
//! Palette source: HUD design mocks (`hud-icons-mock.html`,
//! `hud-inspector-mock.html`, `hud-node-mock.html`, ...) -- light glass
//! surfaces over a cool ground, a single blue accent, a thin
//! source-bright (asymmetric-gradient, simplified here to two flat stops)
//! frame, and an 8-color bucket set for node-kind accent bars / stereotype
//! coloring (see `node_style::AccentBucket`).

use makepad_widgets::*;

script_mod! {
    mod.themes.atlas_light = {
        let atlas = me

        // Backdrops: the app/canvas ground and the muted fill behind a
        // package/group frame on the canvas (a step above ground, a step
        // below a node's own surface).
        ground: #xeef2f7
        canvas_ground: #xe6ecf3
        group_fill: #xe9eff5

        // Glass surfaces: panels, bars, chips, pills, node bodies. `field_bg`
        // is the crisp white used for editable controls sitting on top of a
        // surface (matches the mock's `.ctrl { background: #fff }` over the
        // panel's translucent white).
        surface: #xf6f9fc
        surface_border: #x1496dc59
        field_bg: #xffffff

        // Brand / interaction accent (single blue -- see hud-icons-mock.html
        // swatch #1). `selection` is the accent-tint fill for an
        // active/selected row; `frame_hi`/`frame_lo` are the two stops of the
        // "source-bright" asymmetric frame (bright corner fading to dim).
        accent: #x1496dc
        accent_soft: #x1496dc24
        selection: #x1496dc22
        frame_hi: #x1496dcf2
        frame_lo: #x1496dc80

        // Modal scrim (shortcuts overlay): stays a dim cool-dark regardless
        // of light/dark mode, same as most HUD-style modal scrims.
        scrim: #x1b2836b3

        // Destructive affordance (member-row remove-on-hover).
        danger: #xeb4678

        // Text.
        text: #x26313f
        text_dim: #x8a97a6

        // Node-kind accent bucket colors (`node_style::AccentBucket`), taken
        // verbatim from the HUD swatch set (hud-icons-mock.html /
        // hud-inspector-mock.html / hud-node-mock.html JS `colors` array).
        bucket_blue: #x1496dc
        bucket_cyan: #x00b4d2
        bucket_teal: #x14bea0
        bucket_indigo: #x5a6ef0
        bucket_amber: #xe69614
        bucket_green: #x3cbe5a
        bucket_rose: #xeb4678
        bucket_slate: #x64748b
    }

    // The one theme in play. Dark mode later = add `mod.themes.atlas_dark`
    // with the same field names and repoint this alias -- no widget changes.
    mod.atlas = mod.themes.atlas_light
}
