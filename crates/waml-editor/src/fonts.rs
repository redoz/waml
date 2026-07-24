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

    // One object-literal assignment CREATES the `mod.fonts` namespace (colon
    // fields), mirroring `theme_atlas.rs`'s `mod.themes.atlas_light = { ... }`.
    // A field-by-field `mod.fonts.text_x = ...` instead aborts the VM
    // type-check ("field fonts not found") -- the namespace never exists and
    // every `use mod.fonts` consumer resolves NotFound (no visible chrome
    // text). See app.rs `App::script_mod` for the registration order.
    mod.fonts = {
        // The rare big moment: caption/window title and the shortcuts-overlay
        // title. Condensed SemiBold cut.
        text_title: TextStyle{
            font_size: 16
            font_family: FontFamily{
                latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans_Condensed-SemiBold.ttf") asc: -0.1 desc: 0.0}
            }
            line_spacing: 1.1
        }

        // Panel/section headings, card names.
        text_heading: TextStyle{
            font_size: 13
            font_family: FontFamily{
                latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-SemiBold.ttf") asc: -0.1 desc: 0.0}
            }
            line_spacing: 1.2
        }

        // Default UI body text.
        text_body: TextStyle{
            font_size: 12
            font_family: FontFamily{
                latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
            }
            line_spacing: 1.2
        }

        // Secondary/meta text: labels, dim captions, timestamps/paths.
        text_label: TextStyle{
            font_size: 11
            font_family: FontFamily{
                latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Medium.ttf") asc: -0.1 desc: 0.0}
            }
            line_spacing: 1.2
        }

        // Dense interactive menu/select/tab rows.
        text_menu: TextStyle{
            font_size: 10
            font_family: FontFamily{
                latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
            }
            line_spacing: 1.2
        }

        // Small uppercase section labels (RECENT / START, eyebrow headings).
        text_eyebrow: TextStyle{
            font_size: 10
            font_family: FontFamily{
                latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-SemiBold.ttf") asc: -0.1 desc: 0.0}
            }
            line_spacing: 1.2
        }

        // Monospace attribute-row signatures (column-aligned).
        text_mono: TextStyle{
            font_size: 11
            font_family: FontFamily{
                latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
            }
            line_spacing: 1.2
        }
    }
}

/// Namespace-creation gate: the `mod.fonts` block MUST be created by a single
/// object-literal assignment (`mod.fonts = { text_x: ... }`), the same shape
/// `theme_atlas.rs` uses for `mod.themes.atlas_light`. Assigning field-by-field
/// (`mod.fonts.text_x = ...`) instead aborts the script-VM type-check ("field
/// fonts not found") -- the namespace is never created, every `use mod.fonts`
/// consumer resolves `NotFound`, and ALL chrome text silently vanishes at
/// runtime (icons still render). That outage compiles clean and passes the
/// whole `cargo test`/`pnpm` gate, because the gate never boots the makepad
/// script VM (which needs a `Cx`/GPU context unavailable headless) -- so this
/// class can only be caught by a source-shape assertion here. See
/// `App::script_mod` in `app.rs` for the registration order the runtime relies
/// on.
#[cfg(test)]
mod fonts_namespace_shape_gate {
    /// The token module registers under this script namespace. Any consumer
    /// does `use mod.fonts.*`; if the namespace is never created the widget
    /// falls back to an empty font family and renders no text.
    #[test]
    fn mod_fonts_is_created_by_one_object_literal_not_field_by_field() {
        let src = include_str!("fonts.rs");
        // Keep only the `script_mod! { ... }` block so the doc comments above
        // (which name the broken shape on purpose) don't trip the assertion.
        let block = src
            .split_once("script_mod! {")
            .and_then(|(_, rest)| rest.split_once("\n}\n"))
            .map(|(body, _)| body)
            .expect("fonts.rs must contain a `script_mod! { ... }` block");

        assert!(
            block.contains("mod.fonts = {"),
            "fonts.rs must CREATE the namespace with a single object-literal \
             assignment `mod.fonts = {{ ... }}` (mirrors theme_atlas.rs)"
        );
        // The field-by-field shape `mod.fonts.text_x = ...` is the exact bug
        // that blanks all chrome text -- forbid it inside the script block.
        let offender = block
            .lines()
            .find(|l| l.trim_start().starts_with("mod.fonts.") && l.contains('='));
        assert!(
            offender.is_none(),
            "field-by-field `mod.fonts.<field> = ...` aborts the VM type-check \
             and blanks all chrome text; use `text_<field>:` fields inside the \
             single `mod.fonts = {{ ... }}` object literal instead. Offender: {}",
            offender.unwrap_or("").trim()
        );
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
