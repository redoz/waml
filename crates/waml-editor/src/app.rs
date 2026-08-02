mod actions;
mod event;
mod menus;
mod navigation;
mod shell;
mod workspace;

use self::navigation::{PendingAnchorRestore, PendingFragment, TransitionCause};
#[cfg(target_arch = "wasm32")]
use self::workspace::web_location_query;
use self::workspace::{prevent_quit_after_failed_save, should_flush_save, SaveFeedback};
pub use menus::{burger_menu_items, logo_command_for, logo_menu_items, LogoCommand};
use menus::{doc_switcher_items, DOC_SWITCHER_MAX_H};

use crate::doc_tabs::OpenTabs;
use crate::dock::{DockMotion, DockState, ResponsiveDockLayout};
use crate::document::NavCategory;
use crate::document_host::{DocumentCommand, DocumentHost};
use crate::editor_session::{EditorSession, ExternalReplacement, SaveCompletion, SaveTicket};
use crate::fps_meter::FpsMeter;
use crate::icon_button::IconButtonWidgetRefExt;
use crate::load;
use crate::nav::NavState;
use crate::panel_splitter::PanelSplitterWidgetRefExt;
use crate::platform_browser::{ExternalUrlAdapter, PlatformBrowser};
use crate::popup::base::PopupResult;
use crate::popup::root::{MenuOpen, PopupRoot, PopupSpec};
use crate::project_settings::DockWidths;
use crate::view_history::{HistoryDirection, ViewAnchor, ViewHistory, ViewLocation};
use makepad_widgets::*;
use std::path::{Path, PathBuf};

script_mod! {
    use mod.prelude.widgets.*
    use mod.atlas
    use mod.fonts
    use mod.widgets.ClassDiagramSurface
    use mod.widgets.BehaviorSurface
    use mod.widgets.ProjectTree
    use mod.widgets.Inspector
    use mod.widgets.DocTabs
    use mod.widgets.DiagramSwitcher
    use mod.widgets.ShortcutsOverlay
    use mod.widgets.FontsOverlay
    use mod.widgets.IconsOverlay
    use mod.widgets.ColorsOverlay
    use mod.widgets.ToolDock
    use mod.widgets.ViewBar
    use mod.widgets.DiagramProperties
    use mod.widgets.ConflictBadge
    use mod.widgets.SelectionToolbar
    use mod.widgets.Statusbar
    use mod.widgets.SolidView
    use mod.widgets.DesktopButton
    use mod.widgets.DesktopButtonType
    use mod.widgets.StartScreen
    use mod.widgets.PopupRoot
    use mod.widgets.LogoMark
    use mod.widgets.IconButton
    use mod.widgets.PanelSplitter
    use mod.widgets.AgentMark
    use mod.widgets.DocumentHeader
    use mod.widgets.MarkdownEditor

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.inner_size: vec2(1280, 840)
                window.title: "WAML"
                window.caption_bar_height_override: 66.0
                caption_bar: SolidView{
                    visible: false
                    // Full-width two-row caption. `caption_col` owns the vertical
                    // split: title controls live above the document strip. Every
                    // interactive child is made client area by `WindowDragQuery`,
                    // overriding the surrounding native drag region.
                    width: Fill
                    height: Fill
                    draw_bg.color: atlas.field_bg
                    caption_col := View{
                        width: Fill
                        height: Fill
                        flow: Down
                        // Don't clip the tab band at the column's left edge: the
                        // top rule reaches back over `[T]` and the tree spacer to
                        // `tab_row`'s own start.
                        clip_x: false
                        // Title row: the burger + the open model's name, sitting on
                        // one line above the tabs. `clip_x` bounds a long model path
                        // to the row.
                        title_row := View{
                            width: Fill
                            height: 34.0
                            flow: Right
                            // `align y:0.5` centres the burger + heading metric box
                            // in the row (the proven single-row recipe -- margins
                            // are absorbed by this centring turtle and do nothing
                            // for the label here).
                            align: Align{x: 0.0, y: 0.5}
                            clip_x: true
                            padding: Inset{left: 2.0}
                            // Per-agent window marker (--title / --color). FIRST
                            // child and zero-width: it reserves no space in this
                            // `flow: Right` row, so the burger and model name do
                            // not move, and drawing first puts its wash UNDER
                            // them instead of gelling over them. It draws across
                            // the full row via `draw_abs` and an App-measured
                            // width (`sync_agent_row`), bounded by this row's
                            // `clip_x`.
                            agent_mark := AgentMark{}
                            // Interactive app mark. Its menu drops from this upper
                            // row and its rectangle is excluded from window dragging.
                            logo := LogoMark{ width: 44.0 height: 25.0 }
                            // Burger on the title line, scaled up (30px button, 20px
                            // glyph) so it reads as a peer of the heading and sits on
                            // its centreline. 30 in a 34px row leaves 2px slack top
                            // and bottom, clearing the window top edge. Hidden until
                            // a model opens (`show_editor`/`show_start_screen`); its
                            // drop-down anchors off the caption bottom (see the
                            // burger-menu handler), so its row placement is free.
                            menu_btn := IconButton{ width: 30.0 height: 30.0 icon_size: 18.0 margin: Inset{left: 0.0, right: 2.0, top: 4.0} visible: false }
                            // `Fill`, not `Fit`: the name has to ABSORB the row's
                            // slack so `windows_buttons` behind it stays pinned to
                            // the window's right edge. A `Fit` label long enough to
                            // overflow would shove the button cluster past the row
                            // and `clip_x` would eat it (the caption-child trap).
                            // Overflow is clipped by this row instead.
                            model_name := Label{
                                width: Fill
                                text: ""
                                draw_text +: {
                                    color: atlas.text
                                    // `text_caption` (Regular, 11) -- one px above the
                                    // 10px doc-tab labels so it reads as the heading,
                                    // and quieter than `text_title`, which at
                                    // Condensed SemiBold 16 competes with the 30px
                                    // burger instead of heading the tabs. The
                                    // y:0.5-centred metric box centres glyph mass when
                                    // ascender-|descender| ~= cap; the token's
                                    // `asc:0.1 desc:0.15` trim (proven for the old
                                    // single-row name) seats it on the row centre,
                                    // clear of the window top edge.
                                    text_style: fonts.text_caption
                                }
                            }
                            // Min/max/close live INSIDE the title row, not beside
                            // `caption_col` in the caption's `flow: Right`. As a
                            // sibling of the column they charged their whole 138px
                            // (3 x 46) to BOTH rows, even though the buttons are 29px
                            // tall and hug the top -- the tab row's y is clear of
                            // them. That reserve held the tab row's trailing content
                            // 138px inboard of the window edge. Charged to this row
                            // alone, the tab row now runs the full window width.
                            //
                            // The fork resolves these by id (`ids!(windows_buttons)`,
                            // a descending path search) for its own drag query and
                            // min/max/close handlers, so re-parenting is invisible to
                            // it. `height: Fill` + own top-align still seats them at
                            // y0 of the band, since this row IS the top of the band.
                            windows_buttons := View {
                                visible: false
                                width: Fit height: Fill
                                align: Align{y: 0.0}
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
                        // Tab row: the tree-column toggle then the doc-tab strip fill
                        // the lower band. `DocTabs`' own `draw_bg` repaints
                        // `field_bg`, so it reads seamless with the caption; the
                        // active tab card bleeds down into the body.
                        //
                        // `flow: Right` is what makes the Zed reading cheap: one
                        // runtime-driven spacer between `[T]` and the strip moves
                        // every tab card, so no tab-side offset arithmetic is needed.
                        tab_row := View{
                            width: Fill
                            height: Fill
                            flow: Right
                            // See `caption_col`: the tab strip's top rule reaches
                            // left to this row's own edge, past `[T]` and the spacer.
                            clip_x: false
                            // The tree-column toggle, FIRST child so it is anchored
                            // hard against the full-width row's left edge and never moves: expanding the
                            // tree must slide only the tab cards, not the control that
                            // slides them. 30px button / 18px glyph -- the burger's exact
                            // size, because the two stack in one column and any mismatch
                            // reads as a mistake. That makes this box taller than a tab
                            // card (which insets `TOP_MARGIN` = 8 into the 32px row), so
                            // it deliberately overhangs the cards rather than sitting
                            // flush with them; `top: 1` centres the 30px box in the 32px
                            // row. Hidden until a model opens
                            // (`show_editor`/`show_start_screen`), which also sets the
                            // glyph (`Icon::PanelLeftClose` at first; runtime
                            // synchronization switches between `PanelLeftOpen`
                            // and `PanelLeftClose`).
                            //
                            // `left: 2` stacks this glyph on the burger's centreline one
                            // row above: the burger gets its 2px from `title_row`'s
                            // `padding`, this row has no padding, so the button carries
                            // the same 2 as a margin instead. Both rows start at the same
                            // x=0 and both boxes are now 30px
                            // wide, so equal insets align the columns. Counted into
                            // `TREE_BTN_W`, which `sync_tree_gap` subtracts.
                            tree_btn := IconButton{ width: 30.0 height: 30.0 icon_size: 18.0 margin: Inset{left: 2.0, top: 1.0} visible: false }
                            // Runtime-driven spacer (`sync_dock_slots`) between `[T]`
                            // and the strip, sized so the STRIP's left edge lands on
                            // the tree column's right edge -- where the `field_bg`
                            // chrome mass steps in from full-width to column-width;
                            // the first card then sits `TAB_LEFT_INSET` inside that.
                            // 0 while the tree is collapsed, so the strip falls back
                            // against `[T]` with only that inset between them.
                            tree_gap := View{ width: 0.0, height: Fill }
                            // View history, on the ROW rather than inside the
                            // document header: one pair for the whole shell, so
                            // it does not blink in and out with the per-document
                            // breadcrumb band. Placed AFTER `tree_gap`, so the
                            // pair -- not the first tab card -- starts on the
                            // tree column's right edge and the strip follows it.
                            // Hidden until a document is active
                            // (`sync_history_controls`).
                            history_back_btn := IconButton{ width: 30.0 height: 30.0 icon_size: 18.0 margin: Inset{top: 1.0} visible: false }
                            history_forward_btn := IconButton{ width: 30.0 height: 30.0 icon_size: 18.0 margin: Inset{top: 1.0} visible: false }
                            doc_tabs := DocTabs{
                                width: Fill
                                height: Fill
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
                    // `dock_row` reserves center space while the following overlay
                    // layers paint the panel bodies. Keeping reservation and hosts
                    // separate lets narrow mode float a panel over the full center.
                    dock_body := View{
                        width: Fill
                        height: Fill
                        flow: Overlay
                        dock_row := View{
                            width: Fill
                            height: Fill
                            flow: Right
                            // Empty runtime-sized reservation: it narrows the center
                            // only in wide mode; the tree itself paints in tree_layer.
                            left_slot := View{
                                width: 0.0
                                height: Fill
                            }
                            // The shared header participates in center layout;
                            // the existing body remains an overlay beneath it.
                            center_column := View{
                                width: Fill
                                height: Fill
                                flow: Down
                                document_header := DocumentHeader{
                                    width: Fill
                                    height: 0.0
                                }
                                center_stack := View{
                                width: Fill
                                height: Fill
                                flow: Overlay
                                // Canvas gets its own wrapper View so the shell can
                                // hide the whole diagram render on a Source tab
                                // (ClassDiagramSurface has no `visible` field of its own and
                                // draws every frame unconditionally). Diagram + Preview
                                // tabs keep it shown; the active `DocView` toggles it off
                                // only for a Source tab, mutually exclusive with
                                // `markdown_surface` below.
                                canvas_wrap := View{
                                    width: Fill
                                    height: Fill
                                    flow: Overlay
                                    canvas := ClassDiagramSurface{
                                        width: Fill
                                        height: Fill
                                    }
                                }
                                // Sibling surface for activity/state-machine/sequence tabs
                                // (spec §1.2-1.3): kind-agnostic, so one widget covers all
                                // three. `BehaviorDocView` toggles this and `canvas_wrap`
                                // mutually exclusively.
                                behavior_canvas_wrap := View{
                                    width: Fill
                                    height: Fill
                                    flow: Overlay
                                    visible: false
                                    behavior_canvas := BehaviorSurface{
                                        width: Fill
                                        height: Fill
                                    }
                                }
                                diagram_properties_wrap := View {
                                    width: Fill
                                    height: Fill
                                    visible: false
                                    diagram_properties := DiagramProperties {
                                        width: Fill
                                        height: Fill
                                    }
                                }
                                // View Source tab body: renders the subject's raw markdown via the
                                // upstream Markdown widget, scrolling vertically when it overflows.
                                // The `surface` (paper white) bg reads as a document page, and the
                                // canvas beneath is hidden outright on a Source tab (see above), so
                                // this slot no longer relies on opaque occlusion. The upstream
                                // Markdown default inks text with `theme.color_label_inner`
                                // (near-white) -- unreadable on a light slot -- so `font_color` is
                                // repointed at the Atlas `text` ink for dark-on-light contrast.
                                // The `surface` (paper white) bg reads as a document page, and the
                                // canvas beneath is hidden outright on a Source tab (see above), so
                                // this slot no longer relies on opaque occlusion.
                                markdown_surface := View{
                                    width: Fill
                                    height: Fill
                                    visible: false
                                    show_bg: true
                                    draw_bg +: {
                                        color: atlas.surface
                                        pixel: fn() {
                                            return vec4(self.color.rgb * self.color.a, self.color.a)
                                        }
                                    }
                                    flow: Down
                                    editor := MarkdownEditor{
                                        width: Fill
                                        height: Fill
                                        scroll_bars: ScrollBars {
                                            scroll_bar_y: ScrollBar {
                                                draw_bg +: {
                                                    size: 5.0
                                                    color: atlas.text_dim
                                                    color_hover: atlas.accent
                                                    color_drag: atlas.accent
                                                }
                                            }
                                        }
                                    }
                                }
                                // Tool dock: left edge of the CENTER, vertically
                                // centered. Anchors to the real center rect now,
                                // so it auto-tracks dock state (retired margin:304).
                                tool_dock_wrap := View{
                                    width: Fill
                                    height: Fill
                                    align: Align{x: 0.0, y: 0.5}
                                    tool_dock := ToolDock{
                                        width: 48.0
                                        // Five real `IconButton` children in a
                                        // `flow: Down` turtle since the IconButton
                                        // extraction, so `Fit` measures correctly.
                                        height: Fit
                                        margin: Inset{left: 12.0}
                                    }
                                }
                                // Canvas view bar: bottom-center, ALWAYS visible
                                // over a diagram, so its click targets never move.
                                // It shares the bottom-center slot with the
                                // selection pill below, but the two are never
                                // co-visible: the bar is diagram-only
                                // (`DocView::chrome`) and the pill is only
                                // populated by `ClassifierPreviewView`, so both sit
                                // at the same 12px bottom offset.
                                view_bar_wrap := View{
                                    width: Fill
                                    height: Fill
                                    align: Align{x: 0.5, y: 1.0}
                                    view_bar := ViewBar{
                                        width: Fit
                                        height: 36.0
                                        margin: Inset{bottom: 12.0}
                                    }
                                }
                                // Conflict counter: top-right of center.
                                conflict_badge_wrap := View{
                                    width: Fill
                                    height: Fill
                                    align: Align{x: 1.0, y: 0.0}
                                    conflict_badge := ConflictBadge{
                                        margin: Inset{right: 12.0, top: 14.0}
                                        visible: false
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
                            // Empty runtime-sized reservation for the right host.
                            right_slot := View{
                                width: 0.0
                                height: Fill
                            }
                        }
                        // Paint tree before inspector. Hosts are runtime-sized, so a
                        // narrow pinned panel overlays the unchanged center stack.
                        // These overlay children follow `dock_row`, so Makepad's
                        // reverse child event order (`EventOrder::Up`) lets an
                        // inside panel hit arrive before the earlier canvas. The
                        // empty host area stays transparent: no full-screen scrim.
                        tree_layer := View{
                            width: Fill
                            height: Fill
                            align: Align{x: 0.0, y: 0.0}
                            // The splitter is the LAST child, so it sits on the
                            // column's inner (canvas-facing) edge. The body is
                            // runtime-sized to `host - SPLITTER_W` by
                            // `sync_dock_slots` rather than being `Fill`:
                            // makepad DEFERS `Fill`, and a widget drawn after a
                            // `Fill` sibling caches a pre-shift rect whose hit
                            // test then silently misses.
                            tree_host := View{
                                width: 0.0
                                height: Fill
                                project_tree := ProjectTree{ width: 0.0 height: Fill }
                                tree_splitter := PanelSplitter{ rule_edge: 1.0 }
                            }
                        }
                        inspector_layer := View{
                            width: Fill
                            height: Fill
                            align: Align{x: 1.0, y: 0.0}
                            // Mirror of `tree_host`: the splitter leads, so it
                            // again lands on the inner edge. Same
                            // runtime-Fixed body width, same reason.
                            inspector_host := View{
                                width: 0.0
                                height: Fill
                                inspector_splitter := PanelSplitter{ rule_edge: 0.0 }
                                inspector := Inspector{ width: 0.0 height: Fill }
                            }
                        }
                    }
                    statusbar := Statusbar{}
                    }
                    shortcuts_overlay := ShortcutsOverlay{
                        width: Fill
                        height: Fill
                    }
                    fonts_overlay := FontsOverlay{
                        width: Fill
                        height: Fill
                    }
                    icons_overlay := IconsOverlay{
                        width: Fill
                        height: Fill
                    }
                    colors_overlay := ColorsOverlay{
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
    session: EditorSession,
    /// Filesystem root backing `bundle` in native builds.
    #[rust]
    open_dir: Option<PathBuf>,
    /// Debounce for `mark_dirty` -> `save`; see `SAVE_DEBOUNCE_SECS`.
    #[rust]
    save_timer: Timer,
    #[rust]
    save_feedback: SaveFeedback,
    /// Basename of the currently-open bundle directory. The bundle's display
    /// name falls back to this when the model carries no root name (`model.path`
    /// is empty -- no root `index.md` H1 / frontmatter title), so an unnamed
    /// bundle reads as its folder rather than a bare "bundle". Retained across a
    /// theme live-edit reload (`rehydrate`), which has no `dir` in hand.
    #[rust]
    open_name: String,
    #[rust]
    documents: DocumentHost,
    #[rust]
    markdown_assets: Option<crate::markdown_hosts::SharedMarkdownAssetHost>,
    #[rust]
    view_history: ViewHistory,
    /// Complete recent-config backing list. `StartScreen` renders a capped copy
    /// of its first five entries, so `OpenRecent(i)` and `TogglePin(i)` resolve
    /// here without re-reading disk or introducing index drift.
    #[rust]
    start_recents: Vec<crate::config::Recent>,
    /// Which screen is live (editor vs start), so a theme live-edit reload
    /// re-hydrates the right one. See `rehydrate`.
    #[rust]
    editor_shown: bool,
    /// FPS-heat meter for the top-bar logo: samples framerate across a user
    /// interaction and maps it to the tint the logo renders. See `fps_meter.rs`.
    #[rust]
    fps_meter: FpsMeter,
    /// Scope state for the tree panel; the app owns it and rebuilds `NavView`
    /// on every change (see `nav.rs`).
    #[rust]
    nav_state: NavState,
    /// The key of the node whose context menu is currently open, stashed when
    /// the menu opens so the committed id (which carries no subject) can be
    /// dispatched against it. Read in the `node_closed` branch (Task 4).
    #[rust]
    node_menu_key: Option<String>,
    #[rust]
    narrow: bool,
    #[rust]
    pointer_in_narrow_dock: bool,
    #[rust]
    dock_layout: ResponsiveDockLayout,
    /// User-dragged widths of the two dock columns, seeded from the open
    /// project's `.waml/settings.json` and persisted back on drag release.
    /// Replaces the compile-time `PROJECT_TREE_W` / `INSPECTOR_W` that
    /// `responsive_layout` used to be handed.
    #[rust]
    dock_widths: DockWidths,
    /// Springy give, in px, currently shown by a collapsed-but-still-held
    /// panel: `(tree, inspector)`. Non-zero only for the length of a drag that
    /// has snapped the panel shut, and reset the moment the finger lifts or the
    /// panel reopens. Not persisted -- it is gesture state, not a width.
    #[rust]
    dock_rubber: (f64, f64),
    #[rust(DockMotion::new(1.0))]
    tree_motion: DockMotion,
    #[rust]
    inspector_motion: DockMotion,
    #[rust]
    dock_next_frame: NextFrame,
    /// Last-applied caption `tree_gap` width, same change-guard role as
    /// `dock_layout` (see `sync_tree_gap`). Negative so the first sync always
    /// writes, even when the computed gap is 0 (collapsed tree).
    #[rust(-1.0)]
    tree_gap_w: f64,
    /// Whether `tab_row`'s history pair is mounted (guards the visibility
    /// writes; the pair sits after `tree_gap`, so it does not enter the spacer
    /// arithmetic).
    #[rust]
    history_controls_visible: bool,
    /// Last-applied `DocTabs::left_overshoot` (see `sync_tree_gap`). Guarded
    /// separately from `tree_gap_w` because it is measured off `doc_tabs`' own
    /// rect, which settles one frame AFTER the gap that moved it. Negative so
    /// the first sync always writes.
    #[rust(-1.0)]
    rule_overshoot: f64,
    /// `--title` badge text, retained so a theme live-edit reload can re-push it
    /// (`Apply::Reload` wipes the widget's own `#[rust]` state).
    #[rust]
    agent_badge: Option<String>,
    /// `--color` tint, retained for the same reason as `agent_badge`.
    #[rust]
    agent_tint: Option<Vec4>,
    /// Last-pushed title-row width, so `sync_agent_row` only pushes on a real
    /// change (same guard shape as `dock_layout`).
    #[rust]
    agent_row_w: f64,
    #[rust]
    pending_fragment: Option<PendingFragment>,
    #[rust]
    pending_anchor_restore: Option<PendingAnchorRestore>,
    /// URL of the in-flight `?bundle=` boot fetch, so its response can name it
    /// in a failure message. `None` once the response (or error) is handled.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    #[rust]
    pending_boot_bundle: Option<String>,
}

impl MatchEvent for App {
    #[cfg(not(target_arch = "wasm32"))]
    fn handle_startup(&mut self, cx: &mut Cx) {
        let argv: Vec<String> = std::env::args().collect();
        let args = match crate::cli::parse(&argv) {
            Ok(a) => a,
            Err(e) => {
                // Land on the start screen rather than a blank window: a bad flag
                // should cost you the flag, not the session.
                log!("{e}");
                self.show_start_screen(cx);
                return;
            }
        };
        self.agent_badge = args.badge.clone();
        self.agent_tint = args.tint.map(|[r, g, b]| vec4(r, g, b, 1.0));
        self.apply_agent_marks(cx);
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

    /// The browser has no argv and no filesystem, so the URL fragment is the
    /// document: `#w1.<payload>` carries a whole deflated bundle (see
    /// [`waml::share`]). No fragment, or one we cannot decode, falls back to
    /// the start screen -- never a blank window.
    #[cfg(target_arch = "wasm32")]
    fn handle_startup(&mut self, cx: &mut Cx) {
        self.reveal_caption_bar_on_web(cx);
        // A browser touch device delivers `TouchUpdate` and nothing else (the
        // backend `preventDefault()`s touches, which also kills the browser's
        // compatibility mouse events). Half this chrome -- every popup surface,
        // the panel scrims, the recents rows, the inspector hover -- routes on
        // raw `MouseDown`/`MouseMove`/`MouseUp`, so under touch it is simply
        // dead. Let a lone finger drive the mouse stream; a second finger still
        // arrives as touch, which is what the canvas pinch-zoom reads.
        cx.set_touch_emulates_mouse(true);
        let (search, hash) = web_location_query(cx);
        let source = match crate::browser_boot::select_browser_boot(&search, &hash) {
            Ok(source) => source,
            Err(e) => {
                log!("could not read this page's URL: {e}");
                crate::browser_boot::BrowserBootSource::Start
            }
        };
        match source {
            crate::browser_boot::BrowserBootSource::Share(fragment) => {
                match waml::share::decode_source(&fragment) {
                    Ok(bundle) => {
                        self.open_bundle(cx, bundle, "shared".to_string(), None);
                        self.show_editor(cx);
                    }
                    Err(e) => {
                        log!("could not open the model in this link: {e}");
                        self.show_start_screen(cx);
                    }
                }
            }
            crate::browser_boot::BrowserBootSource::Bundle(url) => {
                // The fetch lands in `handle_http_response`. Until it does the
                // start screen holds the window -- never a blank one -- and it
                // stays put if the fetch fails.
                self.pending_boot_bundle = Some(url.clone());
                self.show_start_screen(cx);
                cx.http_request(
                    live_id!(boot_bundle),
                    HttpRequest::new(url, HttpMethod::GET),
                );
            }
            // `?api=` is selected for, but no live model server exists yet; the
            // URL is honoured as far as "not a bundle, not a share link".
            crate::browser_boot::BrowserBootSource::Api { .. }
            | crate::browser_boot::BrowserBootSource::Start => self.show_start_screen(cx),
        }
    }

    /// The `?bundle=` fetch came back. Anything other than a 2xx carrying a
    /// valid envelope leaves the start screen up with the reason logged.
    #[cfg(target_arch = "wasm32")]
    fn handle_http_response(&mut self, cx: &mut Cx, request_id: LiveId, response: &HttpResponse) {
        if request_id != live_id!(boot_bundle) {
            return;
        }
        let Some(url) = self.pending_boot_bundle.take() else {
            return;
        };
        if response.status_code < 200 || response.status_code >= 300 {
            log!(
                "{}",
                crate::browser_boot::boot_fetch_error(&url, Some(response.status_code))
            );
            return;
        }
        let body = response.get_body().map(Vec::as_slice).unwrap_or(&[]);
        match crate::browser_boot::decode_boot_bundle(body) {
            Ok(bundle) => {
                self.open_bundle(cx, bundle, "exported".to_string(), None);
                self.show_editor(cx);
            }
            Err(e) => log!("could not open {url}: {e}"),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn handle_http_request_error(&mut self, _cx: &mut Cx, request_id: LiveId, _error: &HttpError) {
        if request_id != live_id!(boot_bundle) {
            return;
        }
        if let Some(url) = self.pending_boot_bundle.take() {
            log!("{}", crate::browser_boot::boot_fetch_error(&url, None));
        }
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        self.handle_action_batch(cx, actions);
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        waml_markdown_editor::script_mod(vm);
        crate::theme_atlas::script_mod(vm);
        // Repoint `mod.atlas` at the dark block when the persisted theme is
        // Dark. Re-read on every script_mod so a live-edit reload picks up a
        // toggle. `atlas_light` stays the default alias inside theme_atlas.
        if crate::config::theme() == crate::config::ThemeMode::Dark {
            script_eval!(vm, {
                mod.atlas = mod.themes.atlas_dark
            });
        }
        crate::fonts::script_mod(vm);
        crate::icons::script_mod(vm);
        crate::frame::script_mod(vm);
        crate::popup::menu::script_mod(vm);
        crate::popup::radial::script_mod(vm);
        crate::popup::select::script_mod(vm);
        crate::popup::conflict_list::script_mod(vm);
        crate::popup::root::script_mod(vm);
        crate::canvas::script_mod(vm);
        // `IconButton` must register before EVERY consumer that mounts it as a
        // child -- `tree_panel`, `inspector_panel`, `tool_dock` -- because a
        // module's DSL resolves `mod.widgets.*` eagerly at `use`-time, not
        // lazily: an unregistered `IconButton {}` silently instantiates a dead,
        // unqueryable node (invisible glyph, `set_icon`/`clicked` no-op). Its own
        // deps (`icons`, `atlas`) are already registered above.
        crate::icon_button::script_mod(vm);
        // `PanelSplitter` is mounted directly by App's own live layout (inside
        // `tree_host` / `inspector_host`), so it must register before the App
        // DSL is evaluated by `self::script_mod` -- same eager `mod.widgets.*`
        // resolution as `IconButton` above. Unregistered, the strip silently
        // becomes a dead, invisible, unhittable node.
        crate::panel_splitter::script_mod(vm);
        // `DocumentHeader` mounts `IconButton`, and App's live layout mounts
        // `DocumentHeader`, so register it after its dependency and before the
        // App DSL is evaluated by `self::script_mod`.
        crate::document_header::script_mod(vm);
        crate::property_controls::script_mod(vm);
        // Diagram Properties mounts the shared SelectBox for Max attributes,
        // so register it before the panel's DSL is evaluated.
        crate::select_box::script_mod(vm);
        crate::diagram_properties::script_mod(vm);
        crate::tree_panel::script_mod(vm);
        // `select_box` is already registered above for Diagram Properties; the
        // inspector's element bar reuses the same widget type.
        // The inspector body's declared child widgets must register before
        // `inspector_panel`: it mounts `SectionHeading` (and, in later tasks,
        // `AttrRowView` / `RefCardView`) as DSL children, and the DSL
        // resolves `mod.widgets.*` eagerly at `use`-time, not lazily.
        crate::section_heading::script_mod(vm);
        crate::attr_row::script_mod(vm);
        // `RefCardView` must register before `inspector_panel`: the inspector's
        // MEMBERS and ASSOCIATIONS FlatLists mount it as a DSL child, and the DSL
        // resolves `mod.widgets.*` eagerly at `use`-time, not lazily. An
        // unregistered child is a dead, invisible node (finding survives both
        // green tests and review).
        crate::ref_card::script_mod(vm);
        crate::inspector_panel::script_mod(vm);
        crate::doc_tabs::script_mod(vm);
        crate::diagram_switcher::script_mod(vm);
        // `OverlayShell` must register before `ShortcutsOverlay`: its DSL
        // customizes `shell +: { ... }` (an embedded field merge, not a
        // mounted DSL child, but the same eager `mod.widgets.*` resolution
        // order rule).
        crate::overlay_shell::script_mod(vm);
        crate::shortcuts_overlay::script_mod(vm);
        crate::fonts_overlay::script_mod(vm);
        crate::icons_overlay::script_mod(vm);
        crate::colors_overlay::script_mod(vm);
        crate::tool_dock::script_mod(vm);
        crate::view_bar::script_mod(vm);
        crate::conflict_badge::script_mod(vm);
        // `AgentMark` must register before `App`'s own DSL, which mounts it as a
        // child of `title_row`: a module's DSL resolves `mod.widgets.*` eagerly
        // at `use`-time, not lazily, so an unregistered child silently becomes a
        // dead, invisible node whose setters no-op. Green tests and review both
        // miss it.
        crate::agent_mark::script_mod(vm);
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
        self.rehydrate_for_event(cx, event);
        self.update_fps_meter(cx, event);
        if self.handle_global_shortcuts(cx, event) {
            return;
        }

        self.match_event(cx, event);
        self.handle_escape_event(cx, event);
        self.handle_persistence_event(cx, event);
        self.route_popup_event(cx, event);
        self.documents.route_ui_event(cx, &self.ui, event);
        self.handle_draw_restores(cx, event);
        self.override_caption_drag_query(cx, event);
        self.synchronize_after_event(cx);
    }
}

#[cfg(test)]
mod tests;
