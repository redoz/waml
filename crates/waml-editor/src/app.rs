use crate::caption_button::CaptionButtonWidgetRefExt;
use crate::doc_tabs::{OpenTabs, TabKind};
use crate::fps_meter::FpsMeter;
use crate::inspector::{diagram_elements, Subject};
use crate::load;
use crate::nav::NavState;
use crate::popup::base::PopupResult;
use crate::popup::root::{MenuOpen, PopupRoot, PopupSpec, RadialOpen};
use crate::scene::{build_focus_scene, build_scene};
use makepad_widgets::*;
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
    use mod.widgets.CaptionButton

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
                    menu_btn := CaptionButton{ shape: 0.0 margin: Inset{left: 1.0, right: 3.0} visible: false }
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
    /// Ephemeral set of node keys whose card body is expanded (all members
    /// shown) rather than capped at `card::MAX_BODY_ROWS`. Never persisted to the
    /// model; cleared when the open diagram changes, held across same-diagram
    /// rebuilds. See `GraphCanvasAction::ToggleExpand` handling.
    #[rust]
    expanded: HashSet<String>,
    /// Scope / search / type-filter state for the tree panel's header band; the
    /// app owns it and rebuilds `NavView` on every change (see `nav.rs`).
    #[rust]
    nav_state: NavState,
    /// Distinct `TreeKind`s present in the currently open model, in canonical
    /// order; drives the type-filter chip's rotation cycle (`RotateFilter`).
    /// Recomputed once per model load (`open_dir`), not per keystroke.
    #[rust]
    nav_kinds: Vec<crate::tree::TreeKind>,
    /// Maps each scope-dropdown popup item id back to its `PackageRow.key`, so
    /// the `nav_scope` tag's committed `LiveId` (from `PopupRoot::closed`)
    /// resolves to a scope to apply. Rebuilt every time the dropdown opens.
    #[rust]
    nav_scope_ids: Vec<(LiveId, String)>,
}

impl App {
    /// tabs rebuild+fit the full diagram scene (inspector empty state, since
    /// diagram hit-test selection is out of scope); classifier tabs pin the
    /// 1.5x focus render and point the inspector at that classifier.
    fn sync_active_tab(&mut self, cx: &mut Cx) {
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
        match active.kind {
            TabKind::Diagram => {
                let built = self
                    .model
                    .diagrams
                    .iter()
                    .find(|d| d.key == active.key)
                    .map(|d| build_scene(&self.model, d, &self.expanded));
                if let Some((scene, diags)) = built {
                    for d in &diags {
                        log!("diagnostic: {d:?}");
                    }
                    let node_keys: Vec<String> =
                        scene.nodes.iter().map(|n| n.key.clone()).collect();
                    if let Some(mut canvas) = self
                        .ui
                        .widget(cx, ids!(canvas))
                        .borrow_mut::<crate::canvas::GraphCanvas>()
                    {
                        canvas.set_scene(cx, scene);
                    }
                    self.sync_inspector_elements(cx, &active.key, &active.title, &node_keys);
                }
                if let Some(mut inspector) = self
                    .ui
                    .widget(cx, ids!(inspector))
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, &self.model, Subject::None);
                }
                if let Some(mut toolbar) =
                    self.ui
                        .widget(cx, ids!(selection_toolbar))
                        .borrow_mut::<crate::selection_toolbar::SelectionToolbar>()
                {
                    toolbar.set_selection(cx, None);
                }
                // Diagram tab: the tool dock is usable.
                self.set_diagram_toolbars(cx, true);
            }
            TabKind::Classifier => {
                let scene = build_focus_scene(&self.model, &active.key);
                if let Some(mut canvas) = self
                    .ui
                    .widget(cx, ids!(canvas))
                    .borrow_mut::<crate::canvas::GraphCanvas>()
                {
                    canvas.set_focus(cx, scene);
                }
                if let Some(mut inspector) = self
                    .ui
                    .widget(cx, ids!(inspector))
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, &self.model, Subject::Classifier(active.key.clone()));
                    // Previewing a classifier/package (not a diagram): no
                    // diagram element-picker.
                    inspector.set_picker_visible(cx, false);
                }
                if let Some(mut toolbar) =
                    self.ui
                        .widget(cx, ids!(selection_toolbar))
                        .borrow_mut::<crate::selection_toolbar::SelectionToolbar>()
                {
                    // Single-classifier focus only in this mock -- always 1.
                    toolbar.set_selection(cx, Some(1));
                }
                // Previewing a classifier/package: no tool dock.
                self.set_diagram_toolbars(cx, false);
            }
        }
        self.sync_statusbar(cx);
    }

    /// Show/hide the left tool dock. Hidden while a classifier/package is
    /// previewed -- only a diagram tab exposes drawing tools.
    fn set_diagram_toolbars(&mut self, cx: &mut Cx, show: bool) {
        self.ui
            .widget(cx, ids!(tool_dock_wrap))
            .set_visible(cx, show);
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
        // A new diagram is being shown in the base tab: drop stale expansion
        // (keyed by node key, which may not exist in the new diagram).
        self.expanded.clear();
        let base_id = self
            .tabs
            .set_diagram_base(diagram.key.clone(), diagram.title.clone());
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

    /// Feed the inspector's element-picker the current diagram's contents
    /// (diagram, nodes, source-anchored edges). `node_keys` are the diagram's
    /// drawable nodes, in display order (from the built `Scene`).
    fn sync_inspector_elements(
        &mut self,
        cx: &mut Cx,
        diagram_key: &str,
        diagram_title: &str,
        node_keys: &[String],
    ) {
        let rows = diagram_elements(&self.model, diagram_key, diagram_title, node_keys);
        if let Some(mut inspector) = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_diagram_elements(cx, &self.model, rows);
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
        // Fresh model: no node keys carry over, so clear expansion state.
        self.expanded.clear();
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
                let (scene, diags) = build_scene(&self.model, diagram, &self.expanded);
                for d in &diags {
                    log!("diagnostic: {d:?}");
                }
                let diagram_key = diagram.key.clone();
                let diagram_title = diagram.title.clone();
                let node_keys: Vec<String> = scene.nodes.iter().map(|n| n.key.clone()).collect();
                if let Some(mut canvas) = self
                    .ui
                    .widget(cx, ids!(canvas))
                    .borrow_mut::<crate::canvas::GraphCanvas>()
                {
                    canvas.set_scene(cx, scene);
                } else {
                    log!("canvas widget not found / wrong type");
                }

                self.tabs = OpenTabs::diagram_base(diagram.key.clone(), diagram.title.clone());
                self.refresh_doc_tabs(cx);
                // open_dir bypasses sync_active_tab (it built the scene inline),
                // so point the tree row highlight at the base tab by hand.
                if let Some(mut panel) = self
                    .ui
                    .widget(cx, ids!(project_tree))
                    .borrow_mut::<crate::tree_panel::ProjectTree>()
                {
                    panel.set_selected_key(cx, Some(diagram_key.clone()));
                }
                if let Some(mut inspector) = self
                    .ui
                    .widget(cx, ids!(inspector))
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, &self.model, Subject::None);
                }
                self.sync_inspector_elements(cx, &diagram_key, &diagram_title, &node_keys);
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
        self.set_diagram_toolbars(cx, has_diagram);

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
    /// `ScopeRequest`/`Query`/`RotateFilter` handling in `handle_actions`).
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
            panel.set_chip_label(cx, &chip);
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
        // Caption burger -- placeholder menu wiring this pass.
        if let Some(press) = self
            .ui
            .widget(cx, ids!(menu_btn))
            .as_caption_button()
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
            let btn = self
                .ui
                .widget(cx, ids!(menu_btn))
                .as_caption_button()
                .rect();
            let anchor = dvec2(
                btn.pos.x + crate::popup::menu::MENU_INDENT_X,
                btn.pos.y + btn.size.y + crate::popup::menu::MENU_GAP,
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
                .as_caption_button()
                .set_held(cx, true);
        }

        // Popup outcomes (tag-filtered off the single action queue).
        if let Some(pr) = self.ui.widget(cx, ids!(popup_root)).borrow::<PopupRoot>() {
            let logo_closed = pr.closed(actions, live_id!(logo));
            let burger_closed = pr.closed(actions, live_id!(burger));
            let node_closed = pr.closed(actions, live_id!(node_menu));
            let picker_closed = pr.closed(actions, live_id!(element_picker));
            let nav_scope_closed = pr.closed(actions, live_id!(nav_scope));
            drop(pr);

            // Burger caller-local glow: any close of the burger tag drops it.
            if burger_closed.is_some() {
                self.ui
                    .widget(cx, ids!(menu_btn))
                    .as_caption_button()
                    .set_held(cx, false);
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
            // canvas/tab selection takes.
            if let Some(result) = picker_closed {
                if let Some(mut inspector) = self
                    .ui
                    .widget(cx, ids!(inspector))
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.on_picker_closed(cx, &self.model, result);
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

        // Type-filter chip: cycle None -> kinds_in_model[0] -> ... -> last -> None.
        let rotate = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .map(|panel| panel.rotate_filter_clicked(actions))
            .unwrap_or(false);
        if rotate {
            let cycle: Vec<Option<crate::tree::TreeKind>> = std::iter::once(None)
                .chain(self.nav_kinds.iter().copied().map(Some))
                .collect();
            let cur = cycle
                .iter()
                .position(|f| *f == self.nav_state.filter)
                .unwrap_or(0);
            self.nav_state.filter = cycle[(cur + 1) % cycle.len()];
            self.refresh_nav(cx, false);
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

        // Inline-edit commit: the inspector emits `Edited(subject_key)` when a
        // field's value actually changed. Promote the tab pointing at that
        // subject from preview to persisted (title de-dims).
        let edited_key = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow_mut::<crate::inspector_panel::Inspector>()
            .and_then(|inspector| inspector.edited(actions));
        if let Some(key) = edited_key {
            if let Some(tab) = self.tabs.tabs.iter().find(|t| t.key == key) {
                let id = tab.id;
                self.tabs.promote(id);
                self.refresh_doc_tabs(cx);
            }
            return;
        }

        // Element-picker: the SelectBox asked to open its flyout. Only `App`
        // may place a cross-tree popup, so relay through `popup_root` (routed
        // through the single authority, same as burger/logo/node radial).
        let open_request = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow_mut::<crate::inspector_panel::Inspector>()
            .and_then(|inspector| inspector.take_open_request(cx, actions));
        if let Some((anchor_rect, min_width, items)) = open_request {
            let anchor = dvec2(
                anchor_rect.pos.x,
                anchor_rect.pos.y + anchor_rect.size.y + crate::popup::select::SELECT_GAP,
            );
            let bounds = self.window_bounds(cx);
            if let Some(mut pr) = self
                .ui
                .widget(cx, ids!(popup_root))
                .borrow_mut::<PopupRoot>()
            {
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
            return;
        }

        // Tool dock: mode clicks (Select/Add/Connect) update their own
        // highlight already; the one-shot action buttons stay mock no-ops. The
        // keybinding overlay (U8) is reached via the `?` hotkey below.
        let dock_action = self
            .ui
            .widget(cx, ids!(tool_dock))
            .borrow_mut::<crate::tool_dock::ToolDock>()
            .and_then(|dock| dock.dock_action(actions));
        if let Some(action) = dock_action {
            match action {
                crate::tool_dock::ToolDockAction::ModeChanged(_) => self.sync_statusbar(cx),
                other => log!("tool dock: {other:?}"),
            }
            return;
        }

        // Canvas pointer actions. A right-press opens the node radial (Task 4)
        // routed through the single `popup_root` authority; a primary click
        // selects/deselects a node, repointing the inspector only (no tab, no
        // camera move -- the same inspector-local path the element-picker takes).
        let canvas_menu = self
            .ui
            .widget(cx, ids!(canvas))
            .borrow_mut::<crate::canvas::GraphCanvas>()
            .and_then(|c| c.canvas_action(actions));
        match canvas_menu {
            Some(crate::canvas::GraphCanvasAction::NodeMenu { abs, node: _ }) => {
                let bounds = self.window_bounds(cx);
                if let Some(mut pr) = self
                    .ui
                    .widget(cx, ids!(popup_root))
                    .borrow_mut::<PopupRoot>()
                {
                    pr.show_at(
                        cx,
                        PopupSpec::Radial {
                            tag: live_id!(node_menu),
                            center: abs,
                            bounds,
                            items: node_radial_items(),
                            open: RadialOpen::Marking,
                        },
                    );
                }
                return;
            }
            Some(crate::canvas::GraphCanvasAction::NodeSelect { key }) => {
                if let Some(mut inspector) = self
                    .ui
                    .widget(cx, ids!(inspector))
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, &self.model, Subject::Classifier(key));
                }
                return;
            }
            Some(crate::canvas::GraphCanvasAction::NodeDeselect) => {
                if let Some(mut inspector) = self
                    .ui
                    .widget(cx, ids!(inspector))
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, &self.model, Subject::None);
                }
                return;
            }
            Some(crate::canvas::GraphCanvasAction::ToggleExpand { key }) => {
                if !self.expanded.remove(&key) {
                    self.expanded.insert(key);
                }
                // Re-solve the current diagram with the updated set; update_scene
                // holds the camera and re-resolves the selection by key.
                if let Some(active) = self.tabs.active_tab().cloned() {
                    if active.kind == TabKind::Diagram {
                        if let Some(diagram) =
                            self.model.diagrams.iter().find(|d| d.key == active.key)
                        {
                            let (scene, diags) = build_scene(&self.model, diagram, &self.expanded);
                            for d in &diags {
                                log!("diagnostic: {d:?}");
                            }
                            if let Some(mut canvas) = self
                                .ui
                                .widget(cx, ids!(canvas))
                                .borrow_mut::<crate::canvas::GraphCanvas>()
                            {
                                canvas.update_scene(cx, scene);
                            }
                        }
                    }
                }
                return;
            }
            _ => {}
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

        // Selection toolbar: `Delete` closes the focused classifier's doc
        // tab (in-memory only -- the Model is never touched); `New Diagram`
        // is a mock no-op (diagram creation is out of scope for this pass).
        let toolbar_action = self
            .ui
            .widget(cx, ids!(selection_toolbar))
            .borrow_mut::<crate::selection_toolbar::SelectionToolbar>()
            .and_then(|toolbar| toolbar.toolbar_action(actions));
        match toolbar_action {
            Some(crate::selection_toolbar::SelectionToolbarAction::Delete) => {
                if let Some(active) = self.tabs.active_tab() {
                    if active.kind == TabKind::Classifier {
                        let id = active.id;
                        self.tabs.close(id);
                        self.refresh_doc_tabs(cx);
                        self.sync_active_tab(cx);
                    }
                }
                return;
            }
            Some(crate::selection_toolbar::SelectionToolbarAction::NewDiagram) => {
                log!("selection toolbar: New Diagram (mock no-op)");
                return;
            }
            _ => {}
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
        crate::caption_button::script_mod(vm);
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
                .as_caption_button()
                .hits(dq.abs);
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
