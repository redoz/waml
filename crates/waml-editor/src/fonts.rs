//! Mode-independent chrome typography scale: 7 semantic `TextStyle` role
//! tokens (family + size + weight + line-spacing) that every chrome widget
//! references instead of an ad-hoc inline `font_size:`/`FontMember`. Mirrors
//! `theme_atlas.rs`'s shape (a single top-level `script_mod!` block of named
//! constants) but the values are `TextStyle`s, not scalar colors, so this
//! block imports the same script namespaces an existing inline-`FontMember`
//! widget imports (see `statusbar.rs`).
//!
//! Unlike `atlas`, these tokens are mode-independent -- there is no
//! light/dark variant to repoint.
//!
//! Registration order matters (the dead-token trap): this module's
//! `script_mod(vm)` must run BEFORE any consumer resolves `mod.fonts` --
//! see `App::script_mod` in `app.rs`, where it is registered right after
//! `theme_atlas` and before every widget registration.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.text.*

    // The rare big moment: caption/window title and the shortcuts-overlay
    // title. Condensed SemiBold cut.
    mod.fonts.text_title = TextStyle{
        font_size: 16
        font_family: FontFamily{
            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans_Condensed-SemiBold.ttf") asc: -0.1 desc: 0.0}
        }
        line_spacing: 1.1
    }

    // Panel/section headings, card names.
    mod.fonts.text_heading = TextStyle{
        font_size: 13
        font_family: FontFamily{
            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-SemiBold.ttf") asc: -0.1 desc: 0.0}
        }
        line_spacing: 1.2
    }

    // Default UI body text.
    mod.fonts.text_body = TextStyle{
        font_size: 12
        font_family: FontFamily{
            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
        }
        line_spacing: 1.2
    }

    // Secondary/meta text: labels, dim captions, timestamps/paths.
    mod.fonts.text_label = TextStyle{
        font_size: 11
        font_family: FontFamily{
            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Medium.ttf") asc: -0.1 desc: 0.0}
        }
        line_spacing: 1.2
    }

    // Dense interactive menu/select/tab rows.
    mod.fonts.text_menu = TextStyle{
        font_size: 10
        font_family: FontFamily{
            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
        }
        line_spacing: 1.2
    }

    // Small uppercase section labels (RECENT / START, eyebrow headings).
    mod.fonts.text_eyebrow = TextStyle{
        font_size: 10
        font_family: FontFamily{
            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-SemiBold.ttf") asc: -0.1 desc: 0.0}
        }
        line_spacing: 1.2
    }

    // Monospace attribute-row signatures (column-aligned).
    mod.fonts.text_mono = TextStyle{
        font_size: 11
        font_family: FontFamily{
            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
        }
        line_spacing: 1.2
    }
}
