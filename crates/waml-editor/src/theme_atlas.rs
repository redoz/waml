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

        // Wordmark logo (`logo.rs`) greyscale ramp: three luminance stops
        // (lightest / mid / darkest) for the folded-W bars. Dark mode flips to
        // a light silver so the mark reads on its ground.
        logo_hi: #x666666
        logo_mid: #x474747
        logo_lo: #x262626

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

    // Dark mode. Same field names as `atlas_light`, so no widget script_mod
    // changes are needed -- `App::script_mod` repoints `mod.atlas` at this
    // block when the persisted theme is Dark (see `config::ThemeMode`).
    //
    // Anchored on five picked colors: two dark grounds (#x23001e plum,
    // #x284b63 slate), one teal accent (#x58a4b0), and two light texts
    // (#xa9bcd0 dim, #xd8dbe2 primary). The remaining tokens are shades
    // derived off those anchors, keeping the light theme's relative
    // luminance ordering (editable field brightest, canvas ground darkest).
    mod.themes.atlas_dark = {
        let atlas = me

        // Backdrops: burgundy `ground` is the app base; the canvas sits a
        // step *lighter* (not darker) so the diagram field reads as a lifted
        // plum surface, and `group_fill` a step lighter still.
        ground: #x23001e
        canvas_ground: #x2d0827
        group_fill: #x3a1233

        // Glass surfaces, all on the burgundy ramp (this is a *dark* theme --
        // no blue-slate panels). `surface` is the muted plum for bars/pills;
        // `field_bg` is the lifted plum the panels + node cards ride on (the
        // most-elevated / "brightest" surface, mirroring light's
        // #xffffff-over-surface relationship). Teal-tinted border throughout.
        surface: #x371433
        surface_border: #x347a8859
        field_bg: #x431a3d

        // Teal accent, deepened from the light `#x58a4b0` anchor so it reads
        // as a dark-theme accent (the light teal washed out on the burgundy
        // ground). Selection tint bumped slightly so it reads on the dark
        // ground.
        accent: #x347a88
        accent_soft: #x347a8824
        selection: #x347a8833
        frame_hi: #x347a88f2
        frame_lo: #x347a8880

        // Modal scrim: dim cool-dark, mode-invariant (same as light).
        scrim: #x1b2836b3

        // Destructive affordance (rose reads on dark unchanged).
        danger: #xeb4678

        // Text.
        text: #xd8dbe2
        text_dim: #xa9bcd0

        // Wordmark logo ramp: light silver, cool-tinted to match the palette,
        // so the folded-W reads on the plum ground.
        logo_hi: #xe4e7ec
        logo_mid: #xb8bfc9
        logo_lo: #x8b95a3

        // Node-kind accent buckets: saturated HUD swatches, unchanged --
        // they read on both grounds.
        bucket_blue: #x1496dc
        bucket_cyan: #x00b4d2
        bucket_teal: #x14bea0
        bucket_indigo: #x5a6ef0
        bucket_amber: #xe69614
        bucket_green: #x3cbe5a
        bucket_rose: #xeb4678
        bucket_slate: #x64748b
    }

    // Default alias points at light; `App::script_mod` repoints it at
    // `atlas_dark` when the persisted `ThemeMode` is Dark.
    mod.atlas = mod.themes.atlas_light
}
