//! Icons style-guide overlay: the glyphs actually wired into UI code, grouped by
//! where they are used (the grouping IS the "used for" answer). Each row draws
//! the live `IconSet` SDF + the Lucide slug (`Icon::label`) + a one-line purpose.
//! Two drift guards keep the table in lockstep with real usage. Rides `OverlayShell`.

use crate::icons::{Icon, IconSet};
use crate::overlay_shell::{OverlayShell, OverlayShellAction};
use makepad_widgets::*;

/// One glyph row: the catalog `Icon` + a hand-authored purpose line.
pub struct IconEntry {
    pub icon: Icon,
    pub purpose: &'static str,
}

macro_rules! ie {
    ($icon:ident, $purpose:literal) => {
        IconEntry {
            icon: Icon::$icon,
            purpose: $purpose,
        }
    };
}

/// Used glyphs grouped by primary usage-area. Each glyph appears exactly once.
pub const ICON_GROUPS: &[(&str, &[IconEntry])] = &[
    (
        "TOOL DOCK",
        &[
            ie!(MousePointer2, "Select tool"),
            ie!(SquarePlus, "Add tool"),
            ie!(Spline, "Connect tool"),
            ie!(SlidersHorizontal, "Properties tool"),
            ie!(CircleX, "Close / clear"),
        ],
    ),
    (
        "WORDMARK / BURGER MENUS",
        &[
            ie!(Menu, "Burger menu button"),
            ie!(Info, "About"),
            ie!(DoorOpen, "Open model"),
            ie!(DoorClosed, "Close model"),
            ie!(Paintbrush, "Fonts reference (this menu family)"),
            ie!(SquareMenu, "Icons reference (this menu family)"),
        ],
    ),
    (
        "NODE MENU",
        &[
            ie!(Braces, "View source"),
            ie!(Search, "Find in diagrams"),
            ie!(PackageOpen, "Open package (radial)"),
        ],
    ),
    (
        "INSPECTOR",
        &[
            ie!(Group, "Group / package section"),
            ie!(PanelTop, "Node / panel section"),
            ie!(Pin, "Pin inspector"),
            ie!(PinOff, "Unpin inspector"),
            ie!(ListCollapse, "Collapse rows"),
            ie!(Frame, "Frame / bounds"),
            ie!(InspectionPanel, "Inspector flag spine"),
        ],
    ),
    (
        "TREE PANEL",
        &[
            ie!(Folder, "Folder / package node"),
            ie!(Funnel, "Filter chip"),
            ie!(SquareDashedTopSolid, "Classifier kind"),
            ie!(List, "Enumeration kind"),
            ie!(Workflow, "Process kind"),
            ie!(Activity, "Event / signal kind"),
            ie!(ArrowLeftRight, "Association kind"),
            ie!(StickyNote, "Note kind"),
            ie!(ChevronsUpDown, "Scope / select dropdown"),
            ie!(ListTree, "Tree panel flag spine"),
        ],
    ),
    ("START SCREEN", &[ie!(Package, "Recent-model row glyph")]),
    (
        "CONFLICT BADGE / LIST",
        &[
            ie!(MessageSquareWarning, "Conflict badge"),
            ie!(Trash, "Delete conflicting placement"),
        ],
    ),
    (
        "VIEW BAR",
        &[
            ie!(ZoomIn, "Zoom in"),
            ie!(ZoomOut, "Zoom out"),
            ie!(Maximize, "Fit to size"),
            ie!(ScanSearch, "Fit to selection"),
            ie!(SquareDashed, "Show hidden borders"),
            ie!(Ruler, "Show constraints"),
        ],
    ),
];

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.fonts

    mod.widgets.IconsOverlayBase = #(IconsOverlay::register_widget(vm))

    mod.widgets.IconsOverlay = set_type_default() do mod.widgets.IconsOverlayBase{
        width: Fill
        height: Fill
        shell +: {
            panel_width: 560.0
            draw_scrim +: { color: atlas.scrim }
            draw_panel +: { color: atlas.surface }
            draw_edge +: { color: atlas.frame_hi }
            draw_thumb +: { color: atlas.frame_lo }
        }
        draw_group   +: { color: atlas.text_dim  text_style: fonts.text_eyebrow }
        draw_slug    +: { color: atlas.text      text_style: fonts.text_body }
        draw_purpose +: { color: atlas.text_dim  text_style: fonts.text_label }
        // Color-only holder: the glyph tint copied per row (no RGBA in Rust).
        draw_glyph_tint +: { color: atlas.text }
    }
}

#[derive(Clone, Debug, Default)]
pub enum IconsOverlayAction {
    #[default]
    None,
    Dismissed,
}

const GROUP_H: f64 = 34.0;
const ICON_ROW_H: f64 = 30.0;
const GLYPH_COL_W: f64 = 34.0;
const SLUG_COL_W: f64 = 220.0;

#[derive(Script, ScriptHook, Widget)]
pub struct IconsOverlay {
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
    draw_slug: DrawText,
    #[redraw]
    #[live]
    draw_purpose: DrawText,
    /// Color-only holder (never drawn); the per-row glyph tint is copied from it.
    #[redraw]
    #[live]
    draw_glyph_tint: DrawColor,
    /// Shared SDF glyph set (same material the tool dock / menu draw).
    #[live]
    icons: IconSet,
}

impl Widget for IconsOverlay {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if let OverlayShellAction::Dismissed = self.shell.handle_event(cx, event) {
            cx.widget_action(self.widget_uid(), IconsOverlayAction::Dismissed);
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
    for (_, rows) in ICON_GROUPS {
        h += GROUP_H + rows.len() as f64 * ICON_ROW_H;
    }
    h
}

impl IconsOverlay {
    // Mirrors `FontsOverlay`'s API shape; not yet consumed here (App drives
    // this page purely through `set_visible`/`overlay_action`), kept public for
    // parity and future toggle-style callers.
    #[allow(dead_code)]
    pub fn visible(&self) -> bool {
        self.shell.is_open()
    }

    pub fn set_visible(&mut self, cx: &mut Cx, visible: bool) {
        self.shell.set_open(cx, visible);
    }

    pub fn overlay_action(&self, actions: &Actions) -> Option<IconsOverlayAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            IconsOverlayAction::None => None,
            a => Some(a),
        }
    }

    fn draw_rows(&mut self, cx: &mut Cx2d, origin: DVec2, _width: f64) {
        let tint = self.draw_glyph_tint.color;
        let mut y = origin.y;
        for (title, rows) in ICON_GROUPS {
            self.draw_group.draw_abs(cx, dvec2(origin.x, y), title);
            y += GROUP_H;
            for entry in *rows {
                let glyph = Rect {
                    pos: dvec2(origin.x, y - 2.0),
                    size: dvec2(18.0, 18.0),
                };
                self.icons.draw(cx, entry.icon, glyph, tint);
                self.draw_slug
                    .draw_abs(cx, dvec2(origin.x + GLYPH_COL_W, y), entry.icon.label());
                self.draw_purpose.draw_abs(
                    cx,
                    dvec2(origin.x + GLYPH_COL_W + SLUG_COL_W, y),
                    entry.purpose,
                );
                y += ICON_ROW_H;
            }
        }
    }
}

#[cfg(test)]
mod drift {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    fn table_icons() -> Vec<Icon> {
        ICON_GROUPS
            .iter()
            .flat_map(|(_, rows)| rows.iter().map(|e| e.icon))
            .collect()
    }

    /// Guard 1: no glyph is listed twice (one row per glyph invariant).
    #[test]
    fn every_table_glyph_is_unique() {
        let all = table_icons();
        let unique: BTreeSet<_> = all.iter().map(|i| i.label()).collect();
        assert_eq!(
            all.len(),
            unique.len(),
            "a glyph is listed in more than one group"
        );
    }

    fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read_dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                if name == "bin" {
                    continue;
                }
                collect_rs_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    /// Guard 2: the table's icon set equals the set of `Icon::<Variant>` used in
    /// UI code (excl. the catalog file, this file, and `bin/`). `Icon::ALL` is
    /// filtered (not a variant).
    #[test]
    fn table_covers_exactly_the_used_icons() {
        let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rs_files(&src_dir, &mut files);

        let re_names: BTreeSet<String> = Icon::ALL.iter().map(|i| variant_name(*i)).collect();
        let mut used: BTreeSet<String> = BTreeSet::new();
        for path in &files {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if name == "icons.rs" || name == "icons_overlay.rs" {
                continue;
            }
            let content = std::fs::read_to_string(path).expect("read source");
            for token in content.split("Icon::").skip(1) {
                let ident: String = token
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if ident == "ALL" || ident.is_empty() {
                    continue;
                }
                if re_names.contains(&ident) {
                    used.insert(ident);
                }
            }
        }

        let table: BTreeSet<String> = table_icons().iter().map(|i| variant_name(*i)).collect();
        assert_eq!(
            table, used,
            "\nICON_GROUPS must cover exactly the used icons.\nmissing from table: {:?}\nstale in table: {:?}",
            used.difference(&table).collect::<Vec<_>>(),
            table.difference(&used).collect::<Vec<_>>(),
        );
    }

    /// The PascalCase variant name (for set comparison against source tokens).
    fn variant_name(icon: Icon) -> String {
        format!("{icon:?}")
    }
}
