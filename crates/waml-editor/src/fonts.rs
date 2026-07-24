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

/// chrome-typography-scale Task 11: locks in "no ad-hoc chrome type" so a
/// future edit can't silently reintroduce a bare `font_size:`/`FontMember`
/// instead of a `mod.fonts` role token. Mirrors the plan's verification grep:
///   rg -n 'font_size:|FontMember' crates/waml-editor/src \
///     --glob '!**/canvas.rs' --glob '!**/node_design_editor.rs' \
///     --glob '!**/bin/**' --glob '!**/card/**'
#[cfg(test)]
mod chrome_typography_gate {
    use std::path::{Path, PathBuf};

    /// Task 11 "Excluded files (not chrome)": a full-screen mock render
    /// surface, the standalone node-design mock harness, dev/debug bin
    /// targets, and the card-preview widget -- none was migrated by this
    /// plan.
    const EXCLUDED_FILES: &[&str] = &["canvas.rs", "node_design_editor.rs"];
    const EXCLUDED_DIRS: &[&str] = &["bin", "card"];

    /// This file (`fonts.rs`) IS the `mod.fonts` token-definition module --
    /// its `font_size:`/`FontMember` lines are the scale's canonical source,
    /// not ad-hoc chrome usage, so they are not "residual" hits.
    const DEFINITION_FILE: &str = "fonts.rs";

    /// `doc_tabs.rs` carries the Task 5 / Task 11 DOCUMENTED exceptions:
    /// the three tab STATE-font `TextStyle`s (`draw_text_active` /
    /// `draw_text_preview` / `draw_text_preview_active`, which the 7-role set
    /// cannot express without destroying the provisional/active tab device)
    /// plus the `draw_close` 18pt glyph metric. Every remaining
    /// `font_size:`/`FontMember` hit in this file is one of those four -- see
    /// the call-site comments in `doc_tabs.rs` itself.
    const STATE_FONT_EXCEPTION_FILE: &str = "doc_tabs.rs";

    fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read_dir") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                if EXCLUDED_DIRS.contains(&name.as_str()) {
                    continue;
                }
                collect_rs_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    #[test]
    fn no_residual_font_size_or_font_member_outside_documented_exceptions() {
        let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rs_files(&src_dir, &mut files);
        assert!(
            !files.is_empty(),
            "expected to find chrome source files under {src_dir:?}"
        );

        let mut offenders = Vec::new();
        for path in &files {
            let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
            if EXCLUDED_FILES.contains(&file_name.as_str())
                || file_name == DEFINITION_FILE
                || file_name == STATE_FONT_EXCEPTION_FILE
            {
                continue;
            }
            let content = std::fs::read_to_string(path).expect("read chrome source file");
            for (i, line) in content.lines().enumerate() {
                if line.contains("font_size:") || line.contains("FontMember") {
                    offenders.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "residual ad-hoc font_size:/FontMember outside documented mod.fonts exceptions:\n{}",
            offenders.join("\n")
        );
    }
}
