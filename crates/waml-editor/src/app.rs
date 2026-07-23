use crate::doc_tabs::{OpenTabs, TabKind};
use crate::fps_meter::FpsMeter;
use crate::icon_button::IconButtonWidgetRefExt;
use crate::inspector::Subject;
use crate::load;
use crate::nav::NavState;
use crate::popup::base::PopupResult;
use crate::popup::root::{MenuOpen, PopupRoot, PopupSpec, RadialOpen};
use crate::popup::select::{SelectItem, SelectLead};
use makepad_widgets::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use waml::model::Model;

script_mod! {
    use mod.prelude.widgets.*
    use mod.atlas
    use mod.widgets.GraphCanvas
    use mod.widgets.ProjectTree
    use mod.widgets.Inspector
    use mod.widgets.DocTabs
    use mod.widgets.DiagramSwitcher
    use mod.widgets.ShortcutsOverlay
    use mod.widgets.ToolDock
    use mod.widgets.SelectionToolbar
    use mod.widgets.Statusbar
    use mod.widgets.SolidView
    use mod.widgets.DesktopButton
    use mod.widgets.DesktopButtonType
    use mod.widgets.StartScreen
    use mod.widgets.PopupRoot
    use mod.widgets.LogoMark
    use mod.widgets.IconButton

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.inner_size: vec2(1280, 840)
                window.title: "WAML"
                window.caption_bar_height_override: 44.0
                caption_bar: SolidView{
                    visible: false
                    flow: Right
                    // Fill the 44px caption slot and center children on its
                    // vertical axis; `Fit` let the shorter content top-sit, which
                    // read as the model name riding too high.
                    height: Fill
                    align: Align{y: 0.5}
                    draw_bg.color: atlas.field_bg
                    // Fixed-width nav cluster (252) holding the logo/sep/name. The
                    // burger button follows it as a direct caption-bar child --
                    // its hit/drag-query path only works there, not nested --
                    // adding 40px (36 + 4 margins) so the burger's right edge and
                    // the first doc tab both land on the tree's right edge
                    // (12 margin + 280 tree = 292).
                    nav := View{
                        width: 252.0
                        height: Fill
                        flow: Right
                        align: Align{y: 0.5}
                    wordmark := View{
                        width: Fit
                        height: Fill
                        align: Align{x: 0.0, y: 0.5}
                        margin: Inset{left: 2.0}
                        padding: Inset{left: 6.0, right: 10.0}
                        // 6-color "W" wordmark, drawn as an anti-aliased SDF (see
                        // `logo.rs`) -- DrawSvg stair-stepped at this size. Now an
                        // interactive `LogoMark` widget: hover plays the shimmer,
                        // a left-click opens the app radial (see the drag-query
                        // override + `logo_action` wiring below). Fixed size holds
                        // the logo's ~1.749 content aspect.
                        logo := LogoMark{
                            width: 52.0
                            height: 29.7
                        }
                    }
                    sep := View{
                        width: Fit
                        height: Fill
                        align: Align{x: 0.0, y: 0.5}
                        Label{
                            text: "/"
                            draw_text +: {
                                color: atlas.text_dim
                                text_style: theme.font_regular{font_size: 16}
                            }
                        }
                    }
                    // `Fill` + `clip_x` bound a long model path to the nav instead
                    // of letting a `Fit` box grow with the path and shove the
                    // burger past 292. Left-aligned; the nav's fixed width holds
                    // the layout regardless of name length.
                    model_name_view := View{
                        width: Fill
                        height: Fill
                        clip_x: true
                        align: Align{x: 0.0, y: 0.5}
                        model_name := Label{
                            text: ""
                            draw_text +: {
                                color: atlas.text_dim
                                // Medium weight for a title a touch heavier than
                                // the tabs. Trim seats the glyphs on the caption's
                                // vertical center: the y:0.5-centered metric box
                                // centers glyph mass when ascender-|descender| ~=
                                // cap height, so a positive `desc` shrinks the box
                                // bottom and drops the glyphs down (negative rode
                                // them high).
                                text_style: TextStyle{
                                    font_size: 13
                                    font_family: FontFamily{
                                        latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Medium.ttf") asc: 0.1 desc: 0.15}
                                    }
                                }
                            }
                        }
                    }
                    }
                    // Burger button right after the nav, as a direct caption-bar
                    // child. Outer margins hold the 292 edge (252 + 40). It
                    // starts hidden -- `App` flips it visible only while a model
                    // is open (see `show_editor` / `show_start_screen`). Save is
                    // gone: the editor autosaves.
                    // The 32px button is seated by the bar's `align: y:0.5`; the
                    // asymmetric `top: 2` margin (vs `bottom: 0`) biases it 1px
                    // BELOW the geometric centre -- at a true centre the burger
                    // reads optically high, so the layout, not the button, carries
                    // the 1px down-nudge. Left/right hold the 292 edge (252 + 40).
                    menu_btn := IconButton{ margin: Inset{left: 1.0, right: 3.0, top: 2.0} visible: false }
                    doc_tabs := DocTabs{
                        width: Fill
                        height: Fill
                    }
                    windows_buttons := View {
                        visible: false
                        width: Fit height: Fit
                        min := DesktopButton {
                            draw_bg.button_type: DesktopButtonType.WindowsMin
                            width: 46 height: 29
                            draw_bg +: {
                                color: #000, color_hover: #000, color_down: #000
                                bg_color_hover: #E9E9E9, bg_color_down: #CCCCCC
                            }
                        }
                        max := DesktopButton {
                            draw_bg.button_type: DesktopButtonType.WindowsMax
                            width: 46 height: 29
                            draw_bg +: {
                                color: #000, color_hover: #000, color_down: #000
                                bg_color_hover: #E9E9E9, bg_color_down: #CCCCCC
                            }
                        }
                        close := DesktopButton {
                            draw_bg.button_type: DesktopButtonType.WindowsClose
                            width: 46 height: 29
                            draw_bg +: {
                                color: #000, color_hover: #FFF, color_down: #FFF
                                bg_color_hover: #E81123, bg_color_down: #F1707A
                            }
                        }
                    }
                }
                body +: {
                    View{
                    width: Fill
                    height: Fill
                    // Overlay flow: `main_column` and `shortcuts_overlay`
                    // both get the full turtle rect (see `Flow::Overlay` in
                    // makepad's turtle.rs); `shortcuts_overlay` is declared
                    // second, so it paints over the whole column when open,
                    // and draws nothing when closed. Plain flow:Down
                    // siblings can't do this -- Fill/Fill would split space
                    // between them instead of overlapping (see U7's paint-
                    // order writeup on `DiagramSwitcher` for the sibling
                    // z-order rules this sidesteps).
                    flow: Overlay
                    main_column := View{
                    width: Fill
                    height: Fill
                    flow: Down
                    // Starts hidden: the start screen (no-arg launch) shows
                    // over this; `App` flips it visible once a project opens.
                    visible: false
                    // Body: a fullscreen canvas base with floating HUD panels
                    // over it. In an Overlay flow every child gets the full body
                    // rect, so each panel is wrapped in a Fill/Fill View whose
                    // `align` parks it in a corner/edge; the panel's own margin
                    // leaves canvas ground showing around it. The wrappers carry
                    // no bg and don't grab pointer events over empty area, so the
                    // canvas keeps its pan/zoom in the gaps between panels.
                    View{
                        width: Fill
                        height: Fill
                        flow: Overlay
                        canvas := GraphCanvas{
                            width: Fill
                            height: Fill
                        }
                        // Tool dock: left edge, vertically centered. Wrapper is
                        // toggled visible only on a diagram tab (hidden while a
                        // classifier/package is previewed) -- see `sync_active_tab`.
                        tool_dock_wrap := View{
                            width: Fill
                            height: Fill
                            align: Align{x: 0.0, y: 0.5}
                            tool_dock := ToolDock{
                                width: 48.0
                                // Hugs its 7 buttons (7 * ITEM_H 44); the widget
                                // draws items manually so Fit collapses to 0 --
                                // an explicit height is required. Vertically
                                // centered, right of the project tree (12+280+12).
                                height: 308.0
                                margin: Inset{left: 304.0}
                            }
                        }
                        // Project tree: far left edge.
                        View{
                            width: Fill
                            height: Fill
                            align: Align{x: 0.0, y: 0.0}
                            project_tree := ProjectTree{
                                width: 280.0
                                height: Fill
                                margin: Inset{left: 12.0, top: 12.0, bottom: 12.0}
                            }
                        }
                        // Inspector: top-right.
                        View{
                            width: Fill
                            height: Fill
                            align: Align{x: 1.0, y: 0.0}
                            inspector := Inspector{
                                width: 320.0
                                height: Fill
                                margin: Inset{right: 12.0, top: 12.0, bottom: 12.0}
                            }
                        }
                        // Selection toolbar: bottom, centered.
                        View{
                            width: Fill
                            height: Fill
                            align: Align{x: 0.5, y: 1.0}
                            selection_toolbar := SelectionToolbar{
                                width: Fit
                                height: 44.0
                                margin: Inset{bottom: 12.0}
                            }
                        }
                    }
                    }
                    shortcuts_overlay := ShortcutsOverlay{
                        width: Fill
                        height: Fill
                    }
                    start_screen := StartScreen{
                        width: Fill
                        height: Fill
                    }
                    // Single-active popup authority: last overlay child so it paints above
                    // the canvas + every panel. Hosts the wedge + linear-card surfaces; each
                    // paints nothing while closed. Replaces the old `radial` + `app_menu`
                    // children.
                    popup_root := PopupRoot{ width: Fill height: Fill }
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    model: Model,
    /// Basename of the currently-open bundle directory. The bundle's display
    /// name falls back to this when the model carries no root name (`model.path`
    /// is empty -- no root `index.md` H1 / frontmatter title), so an unnamed
    /// bundle reads as its folder rather than a bare "bundle". Retained across a
    /// theme live-edit reload (`rehydrate`), which has no `dir` in hand.
    #[rust]
    open_name: String,
    #[rust]
    tabs: OpenTabs,
    /// Recents last rendered into the start screen, so an `OpenRecent(i)`
    /// action resolves to a path without re-reading disk or index drift.
    #[rust]
    start_recents: Vec<crate::config::Recent>,
    /// Which screen is live (editor vs start), so a theme live-edit reload
    /// re-hydrates the right one. See `rehydrate`.
    #[rust]
    editor_shown: bool,
    /// FPS-heat meter for the top-bar wordmark: samples framerate across a user
    /// interaction and maps it to the tint the logo renders. See `fps_meter.rs`.
    #[rust]
    fps_meter: FpsMeter,
    /// Scope / search / type-filter state for the tree panel's header band; the
    /// app owns it and rebuilds `NavView` on every change (see `nav.rs`).
    #[rust]
    nav_state: NavState,
    /// Distinct `TreeKind`s present in the currently open model, in canonical
    /// order; the type-filter dropdown lists these (plus the "All" row).
    /// Recomputed once per model load (`open_dir`), not per keystroke.
    #[rust]
    nav_kinds: Vec<crate::tree::TreeKind>,
    /// Maps each scope-dropdown popup item id back to its `PackageRow.key`, so
    /// the `nav_scope` tag's committed `LiveId` (from `PopupRoot::closed`)
    /// resolves to a scope to apply. Rebuilt every time the dropdown opens.
    #[rust]
    nav_scope_ids: Vec<(LiveId, String)>,
    /// Maps each type-filter dropdown item id back to its filter (`None` = the
    /// "All" row), so the `nav_filter` tag's committed `LiveId` resolves to a
    /// `NavState::filter`. Rebuilt every time the dropdown opens.
    #[rust]
    nav_filter_ids: Vec<(LiveId, Option<crate::tree::TreeKind>)>,
    /// One live view object per open tab, keyed by `DocTab::id`. Populated as
    /// tabs (diagram and classifier-preview alike) activate; pruned when a
    /// tab closes (`relay_outcome`) or its base diagram is swapped
    /// (`switch_diagram`).
    #[rust]
    views: HashMap<LiveId, Box<dyn crate::doc_view::DocView>>,
}

impl App {
    /// Drop view objects for tabs that are no longer open. Keeps per-tab live
    /// state (a diagram's `expanded`, a preview's key) alive across tab
    /// switches but reclaims it when the tab closes.
    fn reconcile_views(&mut self) {
        let open: HashSet<LiveId> = self.tabs.tabs.iter().map(|t| t.id).collect();
        self.views.retain(|id, _| open.contains(id));
    }

    /// Registry-driven: look up (or create) the active tab's view, delegate
    /// `sync`, and let it drive the shared body surface + tool dock
    /// visibility. Both `TabKind`s go through the identical path now --
    /// their differing behavior lives entirely in the view objects.
    fn sync_active_tab(&mut self, cx: &mut Cx) {
        self.reconcile_views();
        let Some(active) = self.tabs.active_tab().cloned() else {
            if let Some(mut panel) = self
                .ui
                .widget(cx, ids!(project_tree))
                .borrow_mut::<crate::tree_panel::ProjectTree>()
            {
                panel.set_selected_key(cx, None);
            }
            return;
        };
        // Mirror the active tab onto the tree row highlight (single choke point
        // for every activation source: tab click, tree click, switcher, keys).
        if let Some(mut panel) = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
        {
            panel.set_selected_key(cx, Some(active.key.clone()));
        }

        let body = crate::doc_view::BodyWidgets::new(cx, &self.ui);
        let view = self
            .views
            .entry(active.id)
            .or_insert_with(|| crate::doc_view::make_view(&active));
        if let Some(v) = view.downcast_diagram() {
            v.set_active(active.key.clone(), active.title.clone());
        }
        view.sync(cx, &body, &self.model);
        body.set_tool_dock_visible(cx, view.wants_tooldock());

        self.sync_statusbar(cx);
    }

    fn refresh_doc_tabs(&mut self, cx: &mut Cx) {
        if let Some(mut doc_tabs) = self
            .ui
            .widget(cx, ids!(doc_tabs))
            .borrow_mut::<crate::doc_tabs::DocTabs>()
        {
            doc_tabs.set_tabs(cx, &self.tabs);
        }
    }

    /// Swap the base (first) tab's diagram and re-activate it. Shared by the
    /// tree panel's diagram row and the caption-area diagram switcher (U7) --
    /// both just need to name a diagram key, everything else is identical.
    fn switch_diagram(&mut self, cx: &mut Cx, key: &str) {
        let Some(diagram) = self.model.diagrams.iter().find(|d| d.key == key) else {
            log!("switch_diagram: no diagram with key {key:?}");
            return;
        };
        let base_id = self
            .tabs
            .set_diagram_base(diagram.key.clone(), diagram.title.clone());
        // A new diagram is being shown in the base tab: drop the cached view
        // so its expansion state (keyed by node key, which may not exist in
        // the new diagram) starts fresh -- `sync_active_tab` below recreates
        // it via `make_view`.
        self.views.remove(&base_id);
        self.tabs.activate(base_id);
        self.refresh_doc_tabs(cx);
        self.sync_active_tab(cx);
        self.sync_diagram_switcher_current(cx);
    }

    /// Push the base tab's current diagram title into the switcher's trigger
    /// chip. Called wherever the base tab's diagram changes.
    fn sync_diagram_switcher_current(&mut self, cx: &mut Cx) {
        let title = self
            .tabs
            .tabs
            .iter()
            .find(|t| t.kind == TabKind::Diagram)
            .map(|t| t.title.clone())
            .unwrap_or_default();
        if let Some(mut switcher) = self
            .ui
            .widget(cx, ids!(diagram_switcher))
            .borrow_mut::<crate::diagram_switcher::DiagramSwitcher>()
        {
            switcher.set_current(cx, &title);
        }
    }

    /// Toggle the keybinding-hint overlay (U8), triggered by the tool
    /// dock's `Shortcuts` button or the `?` hotkey.
    fn toggle_shortcuts_overlay(&mut self, cx: &mut Cx) {
        if let Some(mut overlay) = self
            .ui
            .widget(cx, ids!(shortcuts_overlay))
            .borrow_mut::<crate::shortcuts_overlay::ShortcutsOverlay>()
        {
            let next = !overlay.visible();
            overlay.set_visible(cx, next);
        }
    }

    /// Force the overlay's visibility (used by the `Escape` hotkey, which
    /// should only ever close it, never toggle it open).
    fn set_shortcuts_overlay(&mut self, cx: &mut Cx, visible: bool) {
        if let Some(mut overlay) = self
            .ui
            .widget(cx, ids!(shortcuts_overlay))
            .borrow_mut::<crate::shortcuts_overlay::ShortcutsOverlay>()
        {
            overlay.set_visible(cx, visible);
        }
    }

    /// Push diagram name / node count / zoom / active tool into the bottom
    /// statusbar. Snapshot values -- called at each sync point (tab switch,
    /// startup, tool-dock mode change), not live during a canvas drag.
    fn sync_statusbar(&mut self, cx: &mut Cx) {
        let diagram_name = self
            .tabs
            .tabs
            .first()
            .map(|t| t.title.clone())
            .unwrap_or_default();
        let (node_count, zoom_pct) = self
            .ui
            .widget(cx, ids!(canvas))
            .borrow_mut::<crate::canvas::GraphCanvas>()
            .map(|c| (c.node_count(), c.zoom_pct()))
            .unwrap_or((0, 100));
        let tool_label = self
            .ui
            .widget(cx, ids!(tool_dock))
            .borrow_mut::<crate::tool_dock::ToolDock>()
            .map(|d| d.active().label())
            .unwrap_or("Select");
        if let Some(mut statusbar) = self
            .ui
            .widget(cx, ids!(statusbar))
            .borrow_mut::<crate::statusbar::Statusbar>()
        {
            statusbar.set_state(cx, diagram_name, node_count, zoom_pct, tool_label);
        }
    }

    /// Load `dir` and populate the editor (tree, canvas, tabs, inspector,
    /// statusbar, diagram switcher). A model with zero diagrams still opens --
    /// empty canvas, no diagram tab. Returns `false` (having `log!`d) only when
    /// the model fails to load, so the caller keeps the start screen up.
    fn open_dir(&mut self, cx: &mut Cx, dir: &Path, wanted_diagram: Option<&str>) -> bool {
        let model = match load::load_model(dir) {
            Ok(m) => m,
            Err(e) => {
                log!("failed to load OKF dir {:?}: {e}", dir);
                return false;
            }
        };
        self.model = model;
        // Fresh model: no tab ids (and so no view state, e.g. expansion) carry
        // over -- `open_dir` always rebuilds `self.tabs` from scratch below.
        self.views.clear();
        // Fresh model: recompute the type-filter chip's cycle and reset scope /
        // search / filter to the whole-model browse state.
        self.nav_kinds = crate::nav::kinds_in_model(&self.model);
        self.nav_state = NavState::default();

        // Folder basename backs the display name when the bundle has no root
        // name of its own. `..` / drive-root degenerate to an empty basename;
        // "bundle" is the last-ditch label.
        self.open_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| !n.is_empty())
            .unwrap_or("bundle")
            .to_string();

        let root_name = if self.model.path.is_empty() {
            self.open_name.as_str()
        } else {
            self.model.path.as_str()
        };
        self.ui.label(cx, ids!(model_name)).set_text(cx, root_name);

        // Record this open in the recents store (best-effort; see config.rs).
        crate::config::push_recent(dir, root_name);

        self.refresh_nav(cx, true);

        // A model may carry zero diagrams (a pure classifier/behavior bundle). We
        // still open it -- the tree and inspector are useful on their own -- just
        // with an empty canvas and no active diagram tab.
        match crate::cli::select_diagram(&self.model, wanted_diagram) {
            Some(diagram) => {
                // Seed the base tab; `self.views` was just cleared above, so
                // `sync_active_tab` lazily creates a fresh `ClassDiagramView`
                // (no card expansion carried over) and drives the canvas,
                // inspector, tree-row highlight, and tool dock through the
                // normal registry-driven path.
                self.tabs = OpenTabs::diagram_base(diagram.key.clone(), diagram.title.clone());
                self.refresh_doc_tabs(cx);
                self.sync_active_tab(cx);
            }
            None => {
                log!(
                    "no diagrams in {:?}; opening model with an empty canvas",
                    dir
                );
                // Empty scene draws nothing and `bounding_box` returns `None`, so
                // the fit path leaves the camera untouched (no divide-by-zero). No
                // diagram tab; the tree/inspector below still populate.
                if let Some(mut canvas) = self
                    .ui
                    .widget(cx, ids!(canvas))
                    .borrow_mut::<crate::canvas::GraphCanvas>()
                {
                    canvas.set_scene(cx, crate::scene::Scene::default());
                }
                self.tabs = OpenTabs::default();
                self.refresh_doc_tabs(cx);
                if let Some(mut inspector) = self
                    .ui
                    .widget(cx, ids!(inspector))
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, &self.model, Subject::None);
                    inspector.set_diagram_elements(cx, &self.model, vec![]);
                }
            }
        }
        self.sync_statusbar(cx);

        // Tool dock is diagram-only: show it when the base tab is a diagram,
        // hide it for a diagram-less model. `open_dir` bypasses the tab-switch
        // path (`sync_active_tab`), so set it explicitly here.
        let has_diagram = self
            .tabs
            .active_tab()
            .map(|t| t.kind == TabKind::Diagram)
            .unwrap_or(false);
        crate::doc_view::BodyWidgets::new(cx, &self.ui).set_tool_dock_visible(cx, has_diagram);

        // Diagram switcher (U7): push the base tab's current diagram title into
        // the trigger chip (empty when the model carries no diagram).
        self.sync_diagram_switcher_current(cx);
        true
    }

    /// Reveal the editor, hide the start screen. `main_column` is a `View`
    /// (honors `WidgetRef::set_visible`); `StartScreen` is a custom widget
    /// whose no-op default `Widget::set_visible` means we must toggle its own
    /// `visible` flag via the borrowed inherent method instead.
    fn show_editor(&mut self, cx: &mut Cx) {
        self.editor_shown = true;
        self.ui.widget(cx, ids!(main_column)).set_visible(cx, true);
        // Caption burger + doc-tab strip belong to an open model.
        self.ui.widget(cx, ids!(menu_btn)).set_visible(cx, true);
        self.ui
            .widget(cx, ids!(menu_btn))
            .as_icon_button()
            .set_icon(cx, crate::icons::Icon::Menu);
        if let Some(mut doc_tabs) = self
            .ui
            .widget(cx, ids!(doc_tabs))
            .borrow_mut::<crate::doc_tabs::DocTabs>()
        {
            doc_tabs.set_visible(cx, true);
        }
        if let Some(mut screen) = self
            .ui
            .widget(cx, ids!(start_screen))
            .borrow_mut::<crate::start_screen::StartScreen>()
        {
            screen.set_visible(cx, false);
        }
    }

    /// Re-push every imperatively-set widget content after a theme live-edit
    /// (`Event::LiveEdit` -> `Apply::Reload`) wiped it. Reads from the in-memory
    /// `model`/`tabs`, so the open project and active tab survive the toggle;
    /// the tool-dock mode (back to `Select`) and the inspector element-picker
    /// are the only bits not restored, both cheap to re-touch by hand.
    fn rehydrate(&mut self, cx: &mut Cx) {
        if !self.editor_shown {
            // Start screen: `show_start_screen` re-reads recents and re-shows.
            self.show_start_screen(cx);
            return;
        }
        let root_name = if self.model.path.is_empty() {
            self.open_name.as_str()
        } else {
            self.model.path.as_str()
        };
        self.ui.label(cx, ids!(model_name)).set_text(cx, root_name);

        self.refresh_nav(cx, true);
        self.refresh_doc_tabs(cx);
        self.sync_active_tab(cx);
        self.sync_diagram_switcher_current(cx);
        self.show_editor(cx);
    }

    /// Load recents into the start screen and reveal it, hiding the editor.
    fn show_start_screen(&mut self, cx: &mut Cx) {
        self.start_recents = crate::config::recents();
        let rows: Vec<crate::start_screen::RecentRow> = self
            .start_recents
            .iter()
            .map(|r| crate::start_screen::RecentRow {
                title: r.title().to_string(),
                path: r.path().display().to_string(),
                when: format_opened(r.opened_at()),
            })
            .collect();
        if let Some(mut screen) = self
            .ui
            .widget(cx, ids!(start_screen))
            .borrow_mut::<crate::start_screen::StartScreen>()
        {
            screen.set_recents(cx, rows);
            screen.set_visible(cx, true);
        }
        self.ui.widget(cx, ids!(main_column)).set_visible(cx, false);
        // No open model on the start screen: hide burger + doc-tab strip, and
        // drop the editor's tab state so a re-open starts clean rather than
        // inheriting the closed model's tabs (open_dir rebuilds from scratch).
        self.ui.widget(cx, ids!(menu_btn)).set_visible(cx, false);
        self.tabs = OpenTabs::default();
        self.refresh_doc_tabs(cx);
        if let Some(mut doc_tabs) = self
            .ui
            .widget(cx, ids!(doc_tabs))
            .borrow_mut::<crate::doc_tabs::DocTabs>()
        {
            doc_tabs.set_visible(cx, false);
        }
        self.editor_shown = false;
    }

    /// Prompt for a model directory via the native folder picker and open it.
    /// Shared by the start screen's "Open a model" and the burger's "Open
    /// model". Blocks the window while modal, as OS file dialogs do; Cancel
    /// yields `None` (no-op); a non-model dir makes `open_dir` log + return
    /// false, so we stay put.
    fn open_model_via_picker(&mut self, cx: &mut Cx) {
        if let Some(dir) = rfd::FileDialog::new()
            .set_title("Open a model")
            .pick_folder()
        {
            if self.open_dir(cx, &dir, None) {
                self.show_editor(cx);
            }
        }
    }

    /// The main window's client rect in main-window coords (popup clip bounds).
    fn window_bounds(&mut self, cx: &mut Cx) -> Rect {
        let sz = self.ui.window(cx, ids!(main_window)).get_inner_size(cx);
        Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(sz.x, sz.y),
        }
    }

    /// Rebuild the nav projection from the current `nav_state` and push it to
    /// the tree panel, along with the header's chip label. The single choke
    /// point for every scope/query/filter change (see
    /// `ScopeRequest`/`Query`/`FilterRequest` handling in `handle_actions`).
    ///
    /// `scope_changed` gates the two header bits that only move when the scope
    /// (or model) changes: the scope title -- whose lookup runs a full
    /// `nav::packages` tree build -- and the authoritative search text. Keeping
    /// them off the per-keystroke `Query` path holds a query edit to a single
    /// tree build (the `view` below, not two), and lets `open_dir`/scope-pick
    /// clear the search field when they reset `nav_state.query` (otherwise the
    /// field keeps showing the previous model's text over an unfiltered tree).
    fn refresh_nav(&mut self, cx: &mut Cx, scope_changed: bool) {
        let view = crate::nav::view(&self.model, &self.nav_state);
        let chip = crate::nav::chip_label(self.nav_state.filter).to_string();
        let title = scope_changed.then(|| {
            crate::nav::packages(&self.model)
                .into_iter()
                .find(|r| r.key == self.nav_state.scope)
                .map(|r| r.title)
                .unwrap_or_else(|| "Untitled".to_string())
        });
        if let Some(mut panel) = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
        {
            panel.set_view(cx, view);
            panel.set_chip_filter(cx, self.nav_state.filter, &chip);
            if let Some(title) = title {
                panel.set_scope_title(cx, title);
                panel.set_query_text(cx, &self.nav_state.query);
            }
        }
    }
}

/// The four node-radial commands (Remove = danger). Ids are what `RadialPopup`
/// reports on commit; `crate::canvas::node_command_for` maps them back.
pub fn node_radial_items() -> Vec<crate::popup::base::PopupItem> {
    use crate::icons::Icon;
    use crate::popup::base::PopupItem;
    vec![
        PopupItem {
            id: live_id!(open),
            label: "Open".into(),
            icon: Icon::PackageOpen,
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(style),
            label: "Style".into(),
            icon: Icon::Paintbrush,
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(markdown),
            label: "Markdown".into(),
            icon: Icon::SquareMenu,
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(remove),
            label: "Remove".into(),
            icon: Icon::Trash,
            danger: true,
            enabled: true,
        },
    ]
}

/// The logo (app) drop-down rows, top to bottom: Properties, About, Exit
/// (danger). No Cancel row -- a drop-down dismisses via Esc / outside-click.
/// Ids are what `MenuPopup` reports on commit; `logo_command_for` maps them back.
pub fn logo_menu_items() -> Vec<crate::popup::base::PopupItem> {
    use crate::icons::Icon;
    use crate::popup::base::PopupItem;
    vec![
        PopupItem {
            id: live_id!(properties),
            label: "Properties".into(),
            icon: Icon::SlidersHorizontal,
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(about),
            label: "About".into(),
            icon: Icon::Info,
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(exit),
            label: "Exit".into(),
            icon: Icon::CircleX,
            danger: true,
            enabled: true,
        },
    ]
}

/// The burger (caption `menu_btn`) drop-down rows: Create, Open model, Close
/// model. New/Open mirror the start screen's actions; Close returns to the
/// start screen. Routed through `popup_root`; the committed ids are handled
/// via the tag-filtered `closed` read in `handle_actions`.
pub fn burger_menu_items() -> Vec<crate::popup::base::PopupItem> {
    use crate::icons::Icon;
    use crate::popup::base::PopupItem;
    vec![
        PopupItem {
            id: live_id!(new_model),
            // No model-specific glyph exists, so keep it a generic "Create".
            label: "Create".into(),
            icon: Icon::SquarePlus,
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(open_model),
            label: "Open model".into(),
            // The open-door glyph, pairing with Close model's door-closed.
            icon: Icon::DoorOpen,
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(close_model),
            label: "Close model".into(),
            // The door-closed glyph, drawn directly from the catalog.
            icon: Icon::DoorClosed,
            danger: false,
            enabled: true,
        },
    ]
}

/// The logo-radial commands `App` acts on. `Cancel` is intentionally absent:
/// committing the Cancel wedge just closes the radial (mapped to `None`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogoCommand {
    Properties,
    About,
    Exit,
}

/// Map a radial-committed `LiveId` to a logo command. `None` = not one of ours
/// (Cancel / node ids / unknown).
pub fn logo_command_for(id: LiveId) -> Option<LogoCommand> {
    if id == live_id!(properties) {
        Some(LogoCommand::Properties)
    } else if id == live_id!(about) {
        Some(LogoCommand::About)
    } else if id == live_id!(exit) {
        Some(LogoCommand::Exit)
    } else {
        None
    }
}

/// Humanize a recent's `opened_at` (unix seconds) as a coarse relative stamp
/// ("just now", "yesterday", "3 weeks ago") for the start-screen row -- easier
/// to scan than an absolute date and self-explanatory without a header.
fn format_opened(secs: u64) -> String {
    const MIN: u64 = 60;
    const HOUR: u64 = 60 * MIN;
    const DAY: u64 = 24 * HOUR;
    const WEEK: u64 = 7 * DAY;
    const MONTH: u64 = 30 * DAY;
    const YEAR: u64 = 365 * DAY;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(secs);
    let d = now.saturating_sub(secs);

    // "1 unit ago" reads better as "a unit ago"; "an hour" is special-cased.
    fn ago(n: u64, unit: &str) -> String {
        if n == 1 {
            format!("a {unit} ago")
        } else {
            format!("{n} {unit}s ago")
        }
    }
    match d {
        0..=44 => "just now".to_string(),
        45..=89 => "a minute ago".to_string(),
        _ if d < HOUR => ago(d / MIN, "minute"),
        _ if d < 2 * HOUR => "an hour ago".to_string(),
        _ if d < DAY => ago(d / HOUR, "hour"),
        _ if d < 2 * DAY => "yesterday".to_string(),
        _ if d < WEEK => ago(d / DAY, "day"),
        _ if d < 2 * WEEK => "a week ago".to_string(),
        _ if d < MONTH => ago(d / WEEK, "week"),
        _ if d < 2 * MONTH => "a month ago".to_string(),
        _ if d < YEAR => ago(d / MONTH, "month"),
        _ if d < 2 * YEAR => "a year ago".to_string(),
        _ => ago(d / YEAR, "year"),
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        let argv: Vec<String> = std::env::args().collect();
        let args = match crate::cli::parse(&argv) {
            Ok(a) => a,
            Err(e) => {
                log!("{e}");
                return;
            }
        };
        match args.dir {
            Some(dir) => {
                if self.open_dir(cx, &dir, args.diagram.as_deref()) {
                    self.show_editor(cx);
                } else {
                    // Bad dir -> fall back to the start screen, never a blank window.
                    self.show_start_screen(cx);
                }
            }
            None => self.show_start_screen(cx),
        }
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let body = crate::doc_view::BodyWidgets::new(cx, &self.ui);
        // Caption burger -- placeholder menu wiring this pass.
        if let Some(press) = self
            .ui
            .widget(cx, ids!(menu_btn))
            .as_icon_button()
            .pressed(actions)
        {
            // Burger drop-down: routed through the single `popup_root` authority.
            // Opens on the PRESS so the same gesture can drag straight into a row
            // and release to pick (marking menu); a plain tap latches it open
            // instead.
            //
            // Anchored a touch right of the button's left edge and tucked up
            // under it (negative `MENU_GAP`) so the card hangs off the glyph.
            // No caption clamp: `MenuPopup` draws in the window overlay, so the
            // card renders over the caption band instead of being clipped at the
            // body's top edge.
            //
            // The burger is a 32px `IconButton` centred in the 44px caption bar,
            // so its own bottom edge sits well inside the band. Anchor the card
            // off the CAPTION-BAR bottom instead (button centre + half the bar
            // height), so the drop keeps the same gap regardless of the button's
            // height -- deriving it from `btn.size.y` alone let the shorter box
            // pull the card up into the caption (read as attaching too close).
            let btn = self.ui.widget(cx, ids!(menu_btn)).as_icon_button().rect();
            let anchor = dvec2(
                btn.pos.x + crate::popup::menu::MENU_INDENT_X,
                btn.pos.y
                    + btn.size.y * 0.5
                    + crate::popup::menu::CAPTION_H * 0.5
                    + crate::popup::menu::MENU_GAP,
            );
            let bounds = self.window_bounds(cx);
            if let Some(mut pr) = self
                .ui
                .widget(cx, ids!(popup_root))
                .borrow_mut::<PopupRoot>()
            {
                pr.show_at(
                    cx,
                    PopupSpec::Menu {
                        tag: live_id!(burger),
                        anchor,
                        bounds,
                        items: burger_menu_items(),
                        open: MenuOpen::Press(press),
                    },
                );
            }
            // Caller-local glow: light the burger now; it drops when we see this
            // tag's Closed (dismiss OR commit) in handle_actions (Step 7).
            self.ui
                .widget(cx, ids!(menu_btn))
                .as_icon_button()
                .set_active(cx, true);
        }

        // Popup outcomes (tag-filtered off the single action queue).
        if let Some(pr) = self.ui.widget(cx, ids!(popup_root)).borrow::<PopupRoot>() {
            let logo_closed = pr.closed(actions, live_id!(logo));
            let burger_closed = pr.closed(actions, live_id!(burger));
            let node_closed = pr.closed(actions, live_id!(node_menu));
            let picker_closed = pr.closed(actions, live_id!(element_picker));
            let nav_scope_closed = pr.closed(actions, live_id!(nav_scope));
            let nav_filter_closed = pr.closed(actions, live_id!(nav_filter));
            drop(pr);

            // Burger caller-local glow: any close of the burger tag drops it.
            if burger_closed.is_some() {
                self.ui
                    .widget(cx, ids!(menu_btn))
                    .as_icon_button()
                    .set_active(cx, false);
            }
            if let Some(PopupResult::Invoked(id)) = burger_closed {
                if id == live_id!(new_model) {
                    // Burger "Create new model": same stub as the start screen's
                    // New project until the template picker lands in a later slice.
                    log!("New model: not yet implemented (template picker is a later slice)");
                } else if id == live_id!(open_model) {
                    // Burger "Open model": native folder picker, same as the
                    // start screen's "Open a model".
                    self.open_model_via_picker(cx);
                } else if id == live_id!(close_model) {
                    self.show_start_screen(cx);
                }
            }
            if let Some(PopupResult::Invoked(id)) = logo_closed {
                if let Some(cmd) = logo_command_for(id) {
                    match cmd {
                        LogoCommand::Properties => log!("logo command: Properties (stub)"),
                        LogoCommand::About => {
                            cx.open_url("https://github.com/redoz/waml", OpenUrlInPlace::No)
                        }
                        LogoCommand::Exit => cx.quit(),
                    }
                }
            }
            if let Some(PopupResult::Invoked(id)) = node_closed {
                if let Some(cmd) = crate::canvas::node_command_for(id) {
                    log!("node command: {cmd:?}");
                }
            }
            // Element-picker: any close (commit or dismiss) clears the box's
            // active state; a node commit repoints the inspector only
            // (inspector-local -- no tab open, no canvas move), the same path a
            // canvas/tab selection takes. Routed through the owning view so a
            // document-scoped popup result stays view-local (`popup_root`
            // access itself stays in the shell).
            if let Some(result) = picker_closed {
                if let Some(active) = self.tabs.active_tab().cloned() {
                    if let Some(view) = self.views.get_mut(&active.id) {
                        let outcome = view.on_popup_result(
                            cx,
                            &body,
                            &self.model,
                            live_id!(element_picker),
                            result,
                        );
                        self.relay_outcome(cx, &active, outcome);
                    }
                }
            }
            // Scope dropdown: a pick sets `NavState::scope` and rebuilds the
            // nav projection under the new scope.
            if let Some(PopupResult::Invoked(id)) = nav_scope_closed {
                if let Some((_, key)) = self.nav_scope_ids.iter().find(|(i, _)| *i == id) {
                    self.nav_state.scope = key.clone();
                    self.refresh_nav(cx, true);
                }
            }
            // Type-filter dropdown: a pick sets `NavState::filter` (`None` = the
            // "All" row) and rebuilds the nav projection under the new filter.
            if let Some(PopupResult::Invoked(id)) = nav_filter_closed {
                if let Some((_, filter)) = self.nav_filter_ids.iter().find(|(i, _)| *i == id) {
                    self.nav_state.filter = *filter;
                    self.refresh_nav(cx, false);
                }
            }
        }

        // Scope title trigger: open the dropdown listing every package (row 0
        // is the synthetic whole-model root), anchored under the title. The id
        // map is rebuilt here so the `nav_scope` tag's `closed` result (above)
        // can resolve a committed `LiveId` back to a scope key.
        let scope_anchor = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.scope_request(actions));
        if let Some(anchor_rect) = scope_anchor {
            self.nav_scope_ids.clear();
            let items: Vec<crate::popup::base::PopupItem> = crate::nav::packages(&self.model)
                .into_iter()
                .map(|row| {
                    let id = LiveId::from_str(&format!("scope:{}", row.key));
                    self.nav_scope_ids.push((id, row.key.clone()));
                    crate::popup::base::PopupItem {
                        id,
                        label: format!("{}{}", "  ".repeat(row.depth), row.title),
                        icon: crate::icons::Icon::Folder,
                        danger: false,
                        enabled: true,
                    }
                })
                .collect();
            let anchor = dvec2(
                anchor_rect.pos.x,
                anchor_rect.pos.y + anchor_rect.size.y + crate::popup::menu::MENU_GAP,
            );
            let bounds = self.window_bounds(cx);
            if let Some(mut pr) = self
                .ui
                .widget(cx, ids!(popup_root))
                .borrow_mut::<PopupRoot>()
            {
                pr.show_at(
                    cx,
                    PopupSpec::Menu {
                        tag: live_id!(nav_scope),
                        anchor,
                        bounds,
                        items,
                        open: MenuOpen::Popup,
                    },
                );
            }
            return;
        }

        // Search field: live-filter the tree on every keystroke.
        let query = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.query_changed(actions));
        if let Some(q) = query {
            self.nav_state.query = q;
            self.refresh_nav(cx, false);
            return;
        }

        // Type-filter chip: open the type-filter dropdown, anchored under the
        // chip. Rows are "All" (Funnel) plus every kind present in the model,
        // each with its matching glyph; the active filter is pre-selected. The
        // id map is rebuilt here so the `nav_filter` tag's `closed` result
        // (above) can resolve a committed `LiveId` back to a filter.
        let filter_anchor = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.filter_request(actions));
        if let Some(anchor_rect) = filter_anchor {
            self.nav_filter_ids.clear();
            let mut items: Vec<SelectItem> = Vec::new();
            for filter in std::iter::once(None).chain(self.nav_kinds.iter().copied().map(Some)) {
                let id = match filter {
                    None => live_id!(filter_all),
                    Some(kind) => LiveId::from_str(&format!("filter:{kind:?}")),
                };
                self.nav_filter_ids.push((id, filter));
                let lead = match filter {
                    None => SelectLead::Icon(crate::icons::Icon::Funnel),
                    Some(kind) => crate::icons::IconSet::icon_for(kind)
                        .map(SelectLead::Icon)
                        .unwrap_or(SelectLead::None),
                };
                items.push(SelectItem {
                    id,
                    lead,
                    label: crate::nav::chip_label(filter).to_string(),
                    selected: filter == self.nav_state.filter,
                    enabled: true,
                });
            }
            let anchor = dvec2(
                anchor_rect.pos.x,
                anchor_rect.pos.y + anchor_rect.size.y + crate::popup::select::SELECT_GAP,
            );
            let min_width = anchor_rect.size.x;
            let bounds = self.window_bounds(cx);
            if let Some(mut pr) = self
                .ui
                .widget(cx, ids!(popup_root))
                .borrow_mut::<PopupRoot>()
            {
                pr.show_at(
                    cx,
                    PopupSpec::Select {
                        tag: live_id!(nav_filter),
                        anchor,
                        min_width,
                        bounds,
                        items,
                    },
                );
            }
            return;
        }

        // Classifier focus: single-click a class row -> open/replace the
        // single preview tab, focus-render that node in the canvas, and
        // point the inspector at it.
        let focused = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.focused_classifier(actions));
        if let Some(key) = focused {
            if let Some(node) = self.model.nodes.iter().find(|n| n.key == key) {
                let title = node
                    .concept
                    .title
                    .clone()
                    .unwrap_or_else(|| node.key.clone());
                self.tabs
                    .open_preview(key, title, crate::tree::kind_of(&node.ty));
                self.refresh_doc_tabs(cx);
                self.sync_active_tab(cx);
            }
            return;
        }

        // Diagram row: swap the permanent Diagram tab's content and activate it.
        let selected = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.selected_diagram(actions));
        if let Some(key) = selected {
            self.switch_diagram(cx, &key);
            return;
        }

        // Diagram switcher chip (caption area, U7): click cycles the base
        // tab to the next `Model::diagrams` entry (wrapping), same
        // swap-and-activate path as the tree panel's diagram row.
        let switcher_clicked = self
            .ui
            .widget(cx, ids!(diagram_switcher))
            .borrow_mut::<crate::diagram_switcher::DiagramSwitcher>()
            .and_then(|switcher| switcher.switcher_action(actions));
        if let Some(crate::diagram_switcher::DiagramSwitcherAction::Clicked) = switcher_clicked {
            let keys: Vec<String> = self.model.diagrams.iter().map(|d| d.key.clone()).collect();
            let current = self
                .tabs
                .tabs
                .iter()
                .find(|t| t.kind == TabKind::Diagram)
                .map(|t| t.key.clone())
                .unwrap_or_default();
            if let Some(next) = crate::diagram_switcher::next_diagram_key(&keys, &current) {
                self.switch_diagram(cx, &next);
            }
            return;
        }

        // Doc tab: the active view (`ClassDiagramView`/`ClassifierPreviewView`)
        // fully owns its actions (inline-edit commit, element-picker open,
        // tool dock, canvas pointer actions, selection toolbar) via
        // `DocView::handle`; the shell only relays the returned `ViewOutcome`.
        // No active tab (start screen / diagram-less model) simply skips this.
        if let Some(active) = self.tabs.active_tab().cloned() {
            let view = self
                .views
                .entry(active.id)
                .or_insert_with(|| crate::doc_view::make_view(&active));
            if let Some(v) = view.downcast_diagram() {
                v.set_active(active.key.clone(), active.title.clone());
            }
            let outcome = view.handle(cx, &body, actions, &self.model);
            if self.relay_outcome(cx, &active, outcome) {
                return;
            }
        }

        // Logo drop-down: a left-click on the top-bar wordmark opens a plain
        // vertical menu that drops DOWN-right from the mark (the wordmark sits
        // in the window's top-left corner, so a radial there is always
        // degenerate -- a drop-down stays fully on-screen). (Hover/click only
        // reach the widget because of the drag-query override below.) Routed
        // through the single `popup_root` authority.
        let logo_click = self
            .ui
            .widget(cx, ids!(logo))
            .borrow::<crate::logo::LogoMark>()
            .and_then(|l| l.logo_action(actions).map(|_| l.drawn_rect()));
        if let Some(logo_rect) = logo_click {
            // Anchor the card at the logo's bottom-left so it drops down-right.
            // The wordmark sits INSIDE the 44px caption bar (see
            // `window.caption_bar_height_override`), but `MenuPopup` draws in the
            // window overlay, whose clip rect starts at the caption's bottom --
            // so clamp the top down to the caption bottom, else the card's top
            // frame edge falls in the caption band and gets clipped away.
            let anchor = dvec2(
                logo_rect.pos.x,
                (logo_rect.pos.y + logo_rect.size.y + crate::popup::menu::MENU_GAP)
                    .max(crate::popup::menu::CAPTION_H),
            );
            let bounds = self.window_bounds(cx);
            if let Some(mut pr) = self
                .ui
                .widget(cx, ids!(popup_root))
                .borrow_mut::<PopupRoot>()
            {
                pr.show_at(
                    cx,
                    PopupSpec::Menu {
                        tag: live_id!(logo),
                        anchor,
                        bounds,
                        items: logo_menu_items(),
                        open: MenuOpen::Popup,
                    },
                );
            }
            return;
        }

        // Start screen: recent-project rows open directly; Open project runs the
        // native rfd folder picker; New project stays a stub (`log!` only) until
        // the template picker lands in a later slice.
        if let Some(screen) = self
            .ui
            .widget(cx, ids!(start_screen))
            .borrow_mut::<crate::start_screen::StartScreen>()
        {
            if let Some(action) = screen.screen_action(actions) {
                drop(screen); // release the borrow before opening a project
                match action {
                    crate::start_screen::StartScreenAction::OpenRecent(i) => {
                        if let Some(recent) = self.start_recents.get(i).cloned() {
                            if self.open_dir(cx, recent.path(), None) {
                                self.show_editor(cx);
                            }
                        }
                    }
                    crate::start_screen::StartScreenAction::NewProject => {
                        log!("New project: not yet implemented (template picker is a later slice)");
                    }
                    crate::start_screen::StartScreenAction::OpenProject => {
                        self.open_model_via_picker(cx);
                    }
                    crate::start_screen::StartScreenAction::None => {}
                }
                return;
            }
        }

        // Shortcuts overlay (U8): `?` (dock button or hotkey) or clicking
        // anywhere on the overlay's scrim closes it again.
        let overlay_dismissed = self
            .ui
            .widget(cx, ids!(shortcuts_overlay))
            .borrow_mut::<crate::shortcuts_overlay::ShortcutsOverlay>()
            .and_then(|overlay| overlay.overlay_action(actions));
        if let Some(crate::shortcuts_overlay::ShortcutsOverlayAction::Dismissed) = overlay_dismissed
        {
            self.toggle_shortcuts_overlay(cx);
            return;
        }

        // Doc tab strip: click a tab to activate it, or its close button.
        let tab_action = self
            .ui
            .widget(cx, ids!(doc_tabs))
            .borrow_mut::<crate::doc_tabs::DocTabs>()
            .and_then(|tabs| tabs.tab_action(actions));
        match tab_action {
            Some(crate::doc_tabs::DocTabsAction::Activate(id)) => {
                self.tabs.activate(id);
                self.refresh_doc_tabs(cx);
                self.sync_active_tab(cx);
            }
            Some(crate::doc_tabs::DocTabsAction::Promote(id)) => {
                self.tabs.activate(id);
                self.tabs.promote(id);
                self.refresh_doc_tabs(cx);
                self.sync_active_tab(cx);
            }
            Some(crate::doc_tabs::DocTabsAction::Close(id)) => {
                self.tabs.close(id);
                self.refresh_doc_tabs(cx);
                self.sync_active_tab(cx);
            }
            _ => {}
        }
    }
}

impl App {
    /// Apply a view's `ViewOutcome`: place popups, relay tab-lifecycle intents,
    /// re-snap the statusbar. Returns `true` if the shell consumed the event
    /// (mirrors the old `return`-after-handling flow in `handle_actions`).
    fn relay_outcome(
        &mut self,
        cx: &mut Cx,
        active: &crate::doc_tabs::DocTab,
        outcome: crate::doc_view::ViewOutcome,
    ) -> bool {
        let mut consumed = false;

        // `ops`: forward-looking, empty this migration (spec §2 -- no shell
        // `Op` application exists yet). Applied here when it lands.
        for _op in &outcome.ops {}

        if let Some(req) = outcome.popup {
            let bounds = self.window_bounds(cx);
            if let Some(mut pr) = self
                .ui
                .widget(cx, ids!(popup_root))
                .borrow_mut::<PopupRoot>()
            {
                match req {
                    crate::doc_view::PopupRequest::NodeRadial { center } => {
                        pr.show_at(
                            cx,
                            PopupSpec::Radial {
                                tag: live_id!(node_menu),
                                center,
                                bounds,
                                items: node_radial_items(),
                                open: RadialOpen::Marking,
                            },
                        );
                    }
                    crate::doc_view::PopupRequest::ElementPicker {
                        anchor_rect,
                        min_width,
                        items,
                    } => {
                        let anchor = dvec2(
                            anchor_rect.pos.x,
                            anchor_rect.pos.y
                                + anchor_rect.size.y
                                + crate::popup::select::SELECT_GAP,
                        );
                        pr.show_at(
                            cx,
                            PopupSpec::Select {
                                tag: live_id!(element_picker),
                                anchor,
                                min_width,
                                bounds,
                                items,
                            },
                        );
                    }
                }
            }
            consumed = true;
        }

        if let Some(key) = outcome.promote_subject {
            if let Some(tab) = self.tabs.tabs.iter().find(|t| t.key == key) {
                let id = tab.id;
                self.tabs.promote(id);
                self.refresh_doc_tabs(cx);
            }
            consumed = true;
        }

        if outcome.close_active {
            let id = active.id;
            self.tabs.close(id);
            self.views.remove(&id);
            self.refresh_doc_tabs(cx);
            self.sync_active_tab(cx);
            consumed = true;
        }

        if let Some(key) = outcome.open_preview {
            // Unused this migration (the project tree, shell chrome, still
            // drives previews). Placeholder relay for the forward-looking
            // channel; resolves title/kind off the model.
            if let Some(node) = self.model.nodes.iter().find(|n| n.key == key) {
                let title = node
                    .concept
                    .title
                    .clone()
                    .unwrap_or_else(|| node.key.clone());
                self.tabs
                    .open_preview(key, title, crate::tree::kind_of(&node.ty));
                self.refresh_doc_tabs(cx);
                self.sync_active_tab(cx);
            }
            consumed = true;
        }

        if outcome.statusbar_dirty {
            self.sync_statusbar(cx);
        }

        consumed
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        crate::theme_atlas::script_mod(vm);
        // Repoint `mod.atlas` at the dark block when the persisted theme is
        // Dark. Re-read on every script_mod so a live-edit reload picks up a
        // toggle. `atlas_light` stays the default alias inside theme_atlas.
        if crate::config::theme() == crate::config::ThemeMode::Dark {
            script_eval!(vm, {
                mod.atlas = mod.themes.atlas_dark
            });
        }
        crate::icons::script_mod(vm);
        crate::frame::script_mod(vm);
        crate::popup::menu::script_mod(vm);
        crate::popup::radial::script_mod(vm);
        crate::popup::select::script_mod(vm);
        crate::popup::root::script_mod(vm);
        crate::canvas::script_mod(vm);
        // `IconButton` must register before EVERY consumer that mounts it as a
        // child -- `tree_panel`, `inspector_panel`, `tool_dock` -- because a
        // module's DSL resolves `mod.widgets.*` eagerly at `use`-time, not
        // lazily: an unregistered `IconButton {}` silently instantiates a dead,
        // unqueryable node (invisible glyph, `set_icon`/`clicked` no-op). Its own
        // deps (`icons`, `atlas`) are already registered above.
        crate::icon_button::script_mod(vm);
        crate::tree_panel::script_mod(vm);
        // `select_box` must register before `inspector_panel`: the inspector's
        // `element_bar` mounts `SelectBox` as a child, and the DSL resolves
        // `mod.widgets.*` eagerly at `use`-time, not lazily.
        crate::select_box::script_mod(vm);
        crate::inspector_panel::script_mod(vm);
        crate::doc_tabs::script_mod(vm);
        crate::diagram_switcher::script_mod(vm);
        crate::shortcuts_overlay::script_mod(vm);
        crate::tool_dock::script_mod(vm);
        crate::selection_toolbar::script_mod(vm);
        crate::statusbar::script_mod(vm);
        crate::logo::script_mod(vm);
        crate::action_link::script_mod(vm);
        crate::recent_row::script_mod(vm);
        crate::start_screen::script_mod(vm);
        // Registered so the design surface compiles into the crate, but never
        // mounted in the live UI -- it is viewable only via the
        // `node_editor_harness` bin (see `node_design_editor.rs`).
        crate::node_design_editor::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        // Theme live-edit: the framework has already re-run `script_mod` and
        // `Apply::Reload`ed the widget tree (wiping imperatively-pushed
        // content) *before* this `Event::LiveEdit` lands, so re-hydrate now.
        if let Event::LiveEdit = event {
            self.rehydrate(cx);
        }

        // Wordmark FPS-heat meter: `App` forwards every raw event to the meter,
        // which owns all interaction-span detection (primary press/release plus
        // the mouse-wheel scroll tail) and framerate sampling. When it reports a
        // change, push the fresh colour/strength to the top-bar wordmark. This is
        // app-wide (not hit-tested), so it fires no matter which child widget
        // captures the drag, and is a no-op on the splash instance.
        if self.fps_meter.on_event(cx, event) {
            if let Some(mut logo) = self
                .ui
                .widget(cx, ids!(logo))
                .borrow_mut::<crate::logo::LogoMark>()
            {
                logo.set_heat(cx, self.fps_meter.color(), self.fps_meter.strength());
            }
        }

        // Tool-dock hotkeys (V/N/C): global, visual-only mode switch. Only
        // live while nothing holds key focus, so they don't fight with the
        // inspector's inline-edit text entry.
        if let Event::KeyDown(ke) = event {
            if cx.key_focus() == Area::Empty {
                let letter = match ke.key_code {
                    KeyCode::KeyV => Some('V'),
                    KeyCode::KeyN => Some('N'),
                    KeyCode::KeyC => Some('C'),
                    _ => None,
                };
                if let Some(tool) = letter.and_then(crate::tool_dock::tool_for_hotkey) {
                    if let Some(mut dock) = self
                        .ui
                        .widget(cx, ids!(tool_dock))
                        .borrow_mut::<crate::tool_dock::ToolDock>()
                    {
                        dock.set_active(cx, tool);
                    }
                    self.sync_statusbar(cx);
                }
                // Shortcuts overlay (U8): `?` opens it, `Escape` closes it --
                // same global-hotkey guard (nothing holding key focus) as
                // the tool-dock modes above.
                match ke.key_code {
                    KeyCode::Slash => self.toggle_shortcuts_overlay(cx),
                    KeyCode::Escape => self.set_shortcuts_overlay(cx, false),
                    // Theme toggle: persist the flip, then request a live-edit.
                    // The reload re-runs `script_mod` (repointing `mod.atlas`)
                    // and `Apply::Reload`s the tree; `Event::LiveEdit` then
                    // lands in `rehydrate` to re-push the wiped content.
                    KeyCode::KeyT => {
                        let mode = crate::config::toggle_theme();
                        log!("theme toggled -> {mode:?}");
                        cx.request_live_edit();
                    }
                    _ => {}
                }
            }
        }
        self.match_event(cx, event);

        // Single popup seam: light-dismiss + active-surface routing + emission.
        if let Some(mut pr) = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow_mut::<PopupRoot>()
        {
            pr.route(cx, event);
        }

        self.ui.handle_event(cx, event, &mut Scope::empty());

        // The Window widget marks the entire caption bar (minus the window
        // min/max/close buttons) as an OS window-drag region, which swallows
        // pointer events over the doc-tab strip living there -- tab clicks and
        // hover never reach the widget. Re-answer the drag query as `Client`
        // over the tab strip so it behaves as a normal interactive area. This
        // runs after `ui.handle_event`, so this `set` overrides the Window's
        // `Caption` answer (last write wins before the platform reads it).
        if let Event::WindowDragQuery(dq) = event {
            let over_tab = self
                .ui
                .widget(cx, ids!(doc_tabs))
                .borrow::<crate::doc_tabs::DocTabs>()
                .map(|tabs| tabs.hits_any_tab(dq.abs))
                .unwrap_or(false);
            // The wordmark also lives in the caption drag region; without this
            // the logo never gets hover/click (the whole feature is dead).
            let over_logo = self
                .ui
                .widget(cx, ids!(logo))
                .borrow::<crate::logo::LogoMark>()
                .map(|l| l.drawn_rect().contains(dq.abs))
                .unwrap_or(false);
            // The caption burger lives in the drag region too; treat its
            // rect as client area so clicks reach the widget.
            let over_btn = self
                .ui
                .widget(cx, ids!(menu_btn))
                .as_icon_button()
                .rect()
                .contains(dq.abs);
            // While the drop-down is open, treat the WHOLE caption as client
            // area. The header is otherwise an OS window-drag region, so a press
            // there starts a drag and never reaches the app as a click -- the
            // one spot the menu wouldn't dismiss from. Client-izing it turns a
            // header press into a normal MouseDown, which the menu's
            // outside-click path dismisses on.
            let menu_open = self
                .ui
                .widget(cx, ids!(popup_root))
                .borrow::<PopupRoot>()
                .map(|pr| pr.is_open())
                .unwrap_or(false);
            if over_tab || over_logo || over_btn || menu_open {
                dq.response.set(WindowDragQueryResponse::Client);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{logo_command_for, LogoCommand};
    use makepad_widgets::*;

    #[test]
    fn logo_command_for_maps_ids_and_rejects_others() {
        assert_eq!(
            logo_command_for(live_id!(properties)),
            Some(LogoCommand::Properties)
        );
        assert_eq!(logo_command_for(live_id!(about)), Some(LogoCommand::About));
        assert_eq!(logo_command_for(live_id!(exit)), Some(LogoCommand::Exit));
        // Cancel maps to nothing (the radial just closes on commit).
        assert_eq!(logo_command_for(live_id!(cancel)), None);
        // A node-radial id / unknown id is not ours.
        assert_eq!(logo_command_for(live_id!(remove)), None);
        assert_eq!(logo_command_for(live_id!(nonsense)), None);
    }
}
