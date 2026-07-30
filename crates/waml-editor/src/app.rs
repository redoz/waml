mod actions;

use crate::doc_tabs::OpenTabs;
use crate::dock::DockState;
use crate::dock::ResponsiveDockLayout;
use crate::document::NavCategory;
use crate::document_host::{DocumentCommand, DocumentHost};
use crate::editor_session::EditorSession;
use crate::fps_meter::FpsMeter;
use crate::icon_button::IconButtonWidgetRefExt;
use crate::load;
use crate::nav::NavState;
use crate::platform_browser::{ExternalUrlAdapter, PlatformBrowser};
use crate::popup::base::PopupResult;
use crate::popup::root::{MenuOpen, PopupRoot, PopupSpec};
use crate::popup::select::{SelectItem, SelectLead};
use crate::view_history::{HistoryDirection, ViewAnchor, ViewHistory, ViewLocation};
use makepad_widgets::*;
use std::path::{Path, PathBuf};

fn open_overlay_contains(
    point: DVec2,
    tree_state: DockState,
    tree_rect: Rect,
    inspector_state: DockState,
    inspector_rect: Rect,
) -> bool {
    (tree_state == DockState::Pinned && tree_rect.contains(point))
        || (inspector_state == DockState::Pinned && inspector_rect.contains(point))
}

fn should_dismiss_narrow_dock(
    point: DVec2,
    canvas_rect: Rect,
    tree_state: DockState,
    tree_rect: Rect,
    inspector_state: DockState,
    inspector_rect: Rect,
) -> bool {
    canvas_rect.contains(point)
        && !open_overlay_contains(
            point,
            tree_state,
            tree_rect,
            inspector_state,
            inspector_rect,
        )
}

fn project_document_header(
    chrome: crate::doc_view::DocumentHeaderChrome,
    breadcrumb: Option<Vec<crate::navigation::BreadcrumbSegment>>,
) -> (
    Vec<crate::navigation::BreadcrumbSegment>,
    Option<crate::icons::Icon>,
) {
    let segments = if chrome.breadcrumb {
        breadcrumb.unwrap_or_default()
    } else {
        Vec::new()
    };
    (segments, chrome.right_dock)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingFragment {
    concept_id: String,
    fragment: String,
}

#[derive(Clone, Debug, PartialEq)]
struct PendingAnchorRestore {
    document: crate::navigation::DocumentLocator,
    anchor: ViewAnchor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransitionCause {
    UserNavigation,
    UndoRedoReveal,
    HistoryTraversal,
    PassiveReconciliation,
}

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
    use mod.widgets.AgentMark
    use mod.widgets.DocumentHeader

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
                            // glyph (`Icon::ListTree`, inherited from the retired tree
                            // flag spine).
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
                                    md := Markdown{
                                        width: Fill
                                        height: Fill
                                        scroll_bars: ScrollBars{ scroll_bar_y: ScrollBar{} }
                                        font_color: atlas.text
                                        draw_text +: { color: atlas.text }
                                        draw_block +: {
                                            line_color: atlas.text
                                            quote_fg_color: atlas.text
                                            quote_bg_color: atlas.group_fill
                                            code_color: atlas.group_fill
                                            sep_color: atlas.text_dim
                                            table_header_bg_color: atlas.group_fill
                                            table_border_color: atlas.text_dim
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
                            tree_host := View{
                                width: 0.0
                                height: Fill
                                project_tree := ProjectTree{ width: Fill height: Fill }
                            }
                        }
                        inspector_layer := View{
                            width: Fill
                            height: Fill
                            align: Align{x: 1.0, y: 0.0}
                            inspector_host := View{
                                width: 0.0
                                height: Fill
                                inspector := Inspector{ width: Fill height: Fill }
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

/// Footprint of the caption's tree-column toggle: the `tree_btn` DSL `width`
/// (30, the burger's size) plus its 2px left margin, which seats it on the
/// burger's centreline one row above (see the `tree_btn` comment). Kept here
/// because `sync_tree_gap` has to subtract it from the tree column's width: the
/// button leads the row, so the spacer after it is short by exactly the button's
/// own footprint.
const TREE_BTN_W: f64 = 32.0;
const NARROW_ENTER_W: f64 = 640.0;
const NARROW_EXIT_W: f64 = 680.0;

fn next_narrow(narrow: bool, viewport_w: f64) -> bool {
    if narrow {
        viewport_w <= NARROW_EXIT_W
    } else {
        viewport_w < NARROW_ENTER_W
    }
}

/// How long the document has to sit unchanged before `mark_dirty` turns into a
/// `save`. Sized for a pause in editing, not for the tail of a single gesture:
/// a save is a full deflate of the bundle, so coalescing a run of related edits
/// into one is worth more than persisting each of them promptly.
const SAVE_DEBOUNCE_SECS: f64 = 3.0;

#[derive(Clone, Debug, PartialEq, Eq)]
enum BackingTransitionError {
    Save(String),
    Load(String),
}

/// Flush the current document before loading its replacement. Keeping this
/// ordering explicit prevents a reopen of the same directory from observing
/// stale pre-save source.
fn replace_after_save<T, S, L>(save: S, load: L) -> Result<T, BackingTransitionError>
where
    S: FnOnce() -> Result<(), String>,
    L: FnOnce() -> Result<T, String>,
{
    save().map_err(BackingTransitionError::Save)?;
    load().map_err(BackingTransitionError::Load)
}

fn close_after_save<T, S>(state: &mut T, save: S) -> Result<(), String>
where
    T: Default,
    S: FnOnce(&T) -> Result<(), String>,
{
    save(state)?;
    *state = T::default();
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct SaveFeedback {
    save_error: Option<String>,
}

impl SaveFeedback {
    fn finish_save(&mut self, result: &Result<(), String>) {
        match result {
            Ok(()) => self.save_error = None,
            Err(error) => self.save_error = Some(error.clone()),
        }
    }

    fn save_error(&self) -> Option<&str> {
        self.save_error.as_deref()
    }

    fn opened_replacement_bundle(&mut self) {
        *self = Self::default();
    }
}

fn should_flush_save(event: &Event) -> bool {
    matches!(event, Event::Shutdown | Event::QuitRequested(_))
}

fn prevent_quit_after_failed_save(event: &Event, result: &Result<(), String>) -> bool {
    if result.is_err() {
        if let Event::QuitRequested(quit) = event {
            quit.handle();
            return true;
        }
    }
    false
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
    /// Last-applied caption `tree_gap` width, same change-guard role as
    /// `dock_layout` (see `sync_tree_gap`). Negative so the first sync always
    /// writes, even when the computed gap is 0 (collapsed tree).
    #[rust(-1.0)]
    tree_gap_w: f64,
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
}

impl App {
    fn set_navigation_message(&mut self, cx: &mut Cx, message: Option<&str>) {
        if let Some(mut statusbar) = self
            .ui
            .widget(cx, ids!(statusbar))
            .borrow_mut::<crate::statusbar::Statusbar>()
        {
            statusbar.set_navigation_message(cx, message);
        }
    }

    fn set_history_problem(&mut self, cx: &mut Cx, message: Option<&str>) {
        if let Some(mut statusbar) = self
            .ui
            .widget(cx, ids!(statusbar))
            .borrow_mut::<crate::statusbar::Statusbar>()
        {
            statusbar.set_history_problem(cx, message);
        }
    }

    fn set_history_success(&mut self, cx: &mut Cx, message: Option<&str>) {
        if let Some(mut statusbar) = self
            .ui
            .widget(cx, ids!(statusbar))
            .borrow_mut::<crate::statusbar::Statusbar>()
        {
            statusbar.set_history_success(cx, message);
        }
    }

    fn clear_history_feedback(&mut self, cx: &mut Cx) {
        if let Some(mut statusbar) = self
            .ui
            .widget(cx, ids!(statusbar))
            .borrow_mut::<crate::statusbar::Statusbar>()
        {
            statusbar.clear_history_feedback(cx);
        }
    }

    fn sync_history_controls(&mut self, cx: &mut Cx) {
        let has_active_document = self.documents.active_tab().is_some();
        let can_back = self
            .view_history
            .can_traverse(HistoryDirection::Back, |location| {
                crate::documents::open_locator(
                    self.session.okf_analysis(),
                    self.session.uml_analysis(),
                    &location.document,
                )
                .is_some()
            });
        let can_forward = self
            .view_history
            .can_traverse(HistoryDirection::Forward, |location| {
                crate::documents::open_locator(
                    self.session.okf_analysis(),
                    self.session.uml_analysis(),
                    &location.document,
                )
                .is_some()
            });
        if let Some(mut header) = self
            .ui
            .widget(cx, ids!(document_header))
            .borrow_mut::<crate::document_header::DocumentHeader>()
        {
            header.set_history_visible(cx, has_active_document);
            header.set_history_enabled(cx, can_back, can_forward);
        }
    }

    fn handle_navigation_intent(
        &mut self,
        cx: &mut Cx,
        intent: crate::navigation::NavigationIntent,
    ) -> bool {
        let (target, disposition) = match intent {
            crate::navigation::NavigationIntent::Resolved {
                target,
                disposition,
            } => (target, disposition),
            crate::navigation::NavigationIntent::MarkdownLink {
                current_concept_id,
                href,
            } => {
                let target = match crate::navigation::resolve_link(
                    self.session.okf(),
                    &current_concept_id,
                    &href,
                ) {
                    Ok(target) => target,
                    Err(error) => {
                        self.set_navigation_message(cx, Some(&error.status_message()));
                        return false;
                    }
                };
                (target, crate::navigation::OpenDisposition::Preview)
            }
        };
        self.navigate_with(cx, target, disposition, &mut PlatformBrowser)
    }

    fn navigate_with<B: ExternalUrlAdapter>(
        &mut self,
        cx: &mut Cx,
        target: crate::navigation::NavigationTarget,
        disposition: crate::navigation::OpenDisposition,
        browser: &mut B,
    ) -> bool {
        match target {
            crate::navigation::NavigationTarget::Document {
                concept_id,
                fragment,
            } => {
                if self.session.okf().concept(&concept_id).is_none() {
                    self.set_navigation_message(
                        cx,
                        Some(&format!("Document not found: {concept_id}")),
                    );
                    return false;
                }
                self.pending_fragment = fragment.map(|fragment| PendingFragment {
                    concept_id: concept_id.clone(),
                    fragment,
                });
                let anchor = self
                    .pending_fragment
                    .as_ref()
                    .map(|pending| ViewAnchor::Markdown {
                        fragment: Some(pending.fragment.clone()),
                        scroll_y: 0.0,
                    })
                    .unwrap_or(ViewAnchor::None);
                let changed = self.transition_to_location(
                    cx,
                    ViewLocation {
                        document: crate::navigation::DocumentLocator::primary(&concept_id),
                        anchor,
                    },
                    TransitionCause::UserNavigation,
                );
                if disposition == crate::navigation::OpenDisposition::Persistent {
                    let id = self.documents.active_id();
                    self.documents.transition(
                        cx,
                        &self.ui,
                        &self.session,
                        DocumentCommand::Promote(id),
                    );
                }
                cx.redraw_all();
                self.set_navigation_message(cx, None);
                changed
            }
            crate::navigation::NavigationTarget::Directory { address } if address == "/" => {
                self.nav_state.scope = "/".into();
                self.nav_state.query.clear();
                self.nav_state.filter = None;
                let (_, inspector) = self.dock_states(cx);
                let inspector = if self.narrow {
                    crate::dock::narrow_entry_states(crate::dock::DockState::Pinned, inspector).1
                } else {
                    inspector
                };
                self.apply_dock_states(cx, crate::dock::DockState::Pinned, inspector);
                self.refresh_nav(cx, true);
                self.set_navigation_message(cx, None);
                true
            }
            crate::navigation::NavigationTarget::Directory { address } => {
                let toggled = self
                    .ui
                    .widget(cx, ids!(project_tree))
                    .borrow_mut::<crate::tree_panel::ProjectTree>()
                    .is_some_and(|mut tree| tree.toggle_directory(cx, &address));
                if toggled {
                    self.set_navigation_message(cx, None);
                }
                toggled
            }
            crate::navigation::NavigationTarget::ExternalUrl(url) => match browser.open(cx, &url) {
                Ok(()) => {
                    self.set_navigation_message(cx, None);
                    true
                }
                Err(error) => {
                    self.set_navigation_message(cx, Some(&format!("Could not open link: {error}")));
                    false
                }
            },
        }
    }

    fn apply_pending_fragment(&mut self, cx: &mut Cx) {
        let Some(pending) = self.pending_fragment.as_ref() else {
            return;
        };
        if self
            .documents
            .active_tab()
            .map_or(true, |tab| tab.concept_id != pending.concept_id)
        {
            return;
        }
        let fragment = pending.fragment.clone();
        let found =
            self.documents
                .scroll_active_to_fragment(cx, &self.ui, &self.session, &fragment);
        self.pending_fragment = None;
        if found {
            if let Some(current) = self.documents.capture_active_location(cx, &self.ui) {
                self.view_history.refresh_current(current);
            }
            self.set_navigation_message(cx, None);
        } else {
            self.set_navigation_message(cx, Some(&format!("Section not found: {fragment}")));
        }
    }

    fn apply_pending_anchor_restore(&mut self, cx: &mut Cx) {
        let Some(pending) = self.pending_anchor_restore.take() else {
            return;
        };
        if self
            .documents
            .active_tab()
            .map_or(true, |tab| tab.locator() != pending.document)
        {
            return;
        }
        let _ = self
            .documents
            .restore_active_anchor(cx, &self.ui, &self.session, &pending.anchor);
        if let Some(current) = self.documents.capture_active_location(cx, &self.ui) {
            self.view_history.refresh_current(current);
        }
    }

    /// Synchronize shell projections after the document host has completed a
    /// transition. Document content and view-specific chrome stay host-owned.
    fn sync_document_shell(&mut self, cx: &mut Cx) {
        let active_concept = self
            .documents
            .active_tab()
            .map(|tab| tab.concept_id.clone());
        let chrome = self.documents.active_chrome().document_header;
        let breadcrumb = if chrome.breadcrumb {
            active_concept.as_deref().and_then(|concept_id| {
                crate::navigation::breadcrumb_for(
                    self.session.okf_analysis(),
                    self.session.uml_analysis(),
                    concept_id,
                )
            })
        } else {
            None
        };
        let (segments, right_dock) = project_document_header(chrome, breadcrumb);
        if let Some(mut header) = self
            .ui
            .widget(cx, ids!(document_header))
            .borrow_mut::<crate::document_header::DocumentHeader>()
        {
            header.set_segments(cx, segments);
            header.set_right_dock(cx, right_dock);
        }
        self.sync_history_controls(cx);
        if let Some(mut tree) = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
        {
            tree.set_selected_key(cx, active_concept);
        }
        self.sync_diagram_switcher_current(cx);
        self.sync_statusbar(cx);
        self.sync_conflict_badge(cx);
    }

    /// Open or focus a document through the shared preview slot. All callers
    /// use this path so replacement cleanup and view/chrome synchronization
    /// stay identical for classifiers and diagrams.
    fn transition_document(&mut self, cx: &mut Cx, concept_id: &str, persistent: bool) -> bool {
        let changed = self.transition_to_location(
            cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::primary(concept_id),
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        );
        if persistent && changed {
            let id = self.documents.active_id();
            self.documents
                .transition(cx, &self.ui, &self.session, DocumentCommand::Promote(id));
        }
        changed
    }

    /// Open `key`'s raw markdown source through the shared history-aware
    /// transition path (spec §5.2). Factored out of the node context menu's
    /// `ViewSource` handler so a read-only surface with no context menu (the
    /// behavior canvas, Task 9) can reach the same code path from its own
    /// selection affordance.
    fn open_view_source(&mut self, cx: &mut Cx, key: &str) {
        self.transition_to_location(
            cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::source(key),
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        );
    }

    fn transition_to_location(
        &mut self,
        cx: &mut Cx,
        location: ViewLocation,
        cause: TransitionCause,
    ) -> bool {
        let departing = self.documents.capture_active_location(cx, &self.ui);
        if matches!(cause, TransitionCause::UserNavigation)
            && matches!(location.anchor, ViewAnchor::None)
            && departing
                .as_ref()
                .is_some_and(|current| current.document == location.document)
        {
            self.session.break_edit_merge_group();
            self.view_history
                .refresh_current(departing.expect("same-document location was checked"));
            self.sync_history_controls(cx);
            return true;
        }
        if matches!(cause, TransitionCause::HistoryTraversal) {
            if let Some(departing) = departing.clone() {
                self.view_history.refresh_current(departing);
            }
        }
        if !self
            .documents
            .restore_location(cx, &self.ui, &self.session, &location)
        {
            return false;
        }
        if matches!(
            cause,
            TransitionCause::HistoryTraversal | TransitionCause::UndoRedoReveal
        ) && !matches!(location.anchor, ViewAnchor::None)
        {
            self.pending_anchor_restore = Some(PendingAnchorRestore {
                document: location.document.clone(),
                anchor: location.anchor.clone(),
            });
            cx.redraw_all();
        }
        self.sync_document_shell(cx);
        // Re-submit the complete composed tree after the selection change.
        // Makepad's immediate-mode `FileTree` otherwise retains only the rows
        // visited before its clicked leaf on that redraw, making a trailing
        // Generic OKF row disappear until the next query/filter event.
        self.refresh_nav(cx, false);
        let Some(mut arriving) = self.documents.capture_active_location(cx, &self.ui) else {
            return false;
        };
        if matches!(
            location.anchor,
            ViewAnchor::Markdown {
                fragment: Some(_),
                ..
            }
        ) {
            arriving.anchor = location.anchor.clone();
        }

        match cause {
            TransitionCause::UserNavigation => {
                self.session.break_edit_merge_group();
                if let Some(departing) = departing {
                    let explicit_fragment = matches!(
                        location.anchor,
                        ViewAnchor::Markdown {
                            fragment: Some(_),
                            ..
                        }
                    );
                    if departing.document == arriving.document && !explicit_fragment {
                        self.view_history.refresh_current(arriving);
                    } else {
                        self.view_history.record_transition(departing, arriving);
                    }
                } else {
                    self.view_history.reset(Some(arriving));
                }
            }
            TransitionCause::UndoRedoReveal => {
                self.session.break_edit_merge_group();
                if let Some(departing) = departing {
                    self.view_history.record_transition(departing, arriving);
                } else {
                    self.view_history.reset(Some(arriving));
                }
            }
            TransitionCause::HistoryTraversal => {}
            TransitionCause::PassiveReconciliation => {
                if self
                    .view_history
                    .current()
                    .is_some_and(|current| current.document == arriving.document)
                {
                    self.view_history.refresh_current(arriving);
                }
            }
        }
        self.sync_history_controls(cx);
        true
    }

    fn traverse_view_history(&mut self, cx: &mut Cx, direction: HistoryDirection) -> bool {
        let Some(target) = self.view_history.target(direction, |location| {
            crate::documents::open_locator(
                self.session.okf_analysis(),
                self.session.uml_analysis(),
                &location.document,
            )
            .is_some()
        }) else {
            return false;
        };
        let location = target.location.clone();
        if !self.transition_to_location(cx, location, TransitionCause::HistoryTraversal) {
            return false;
        }
        self.view_history.commit_traversal(target);
        self.session.break_edit_merge_group();
        self.sync_history_controls(cx);
        true
    }

    fn close_document(&mut self, cx: &mut Cx, id: LiveId) -> bool {
        let was_active = self.documents.active_id() == id;
        let departing = was_active
            .then(|| self.documents.capture_active_location(cx, &self.ui))
            .flatten();
        let changed =
            self.documents
                .transition(cx, &self.ui, &self.session, DocumentCommand::Close(id));
        if !changed {
            return false;
        }
        self.sync_document_shell(cx);
        if was_active {
            self.session.break_edit_merge_group();
            match (
                departing,
                self.documents.capture_active_location(cx, &self.ui),
            ) {
                (Some(departing), Some(arriving)) => {
                    self.view_history.record_transition(departing, arriving);
                }
                (_, None) => self.view_history.reset(None),
                _ => {}
            }
        }
        self.sync_history_controls(cx);
        true
    }

    /// Push the active diagram title into the switcher's trigger chip, falling
    /// back to another open diagram when a classifier is active.
    fn sync_diagram_switcher_current(&mut self, cx: &mut Cx) {
        let title = self
            .documents
            .active_tab()
            .filter(|tab| tab.presentation.category == NavCategory::Diagram)
            .or_else(|| {
                self.documents
                    .tabs()
                    .iter()
                    .find(|tab| tab.presentation.category == NavCategory::Diagram)
            })
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
    /// dock's `Shortcuts` button or the `?` hotkey. Opening it closes every
    /// style-guide page first (one-overlay-open-at-a-time invariant).
    fn toggle_shortcuts_overlay(&mut self, cx: &mut Cx) {
        let now_visible = self
            .ui
            .widget(cx, ids!(shortcuts_overlay))
            .borrow::<crate::shortcuts_overlay::ShortcutsOverlay>()
            .map(|overlay| overlay.visible())
            .unwrap_or(false);
        let next = !now_visible;
        if next {
            self.close_page_overlays(cx);
        }
        self.set_shortcuts_overlay(cx, next);
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

    /// Close the shortcuts overlay AND every style-guide page. Every open path
    /// calls this first, so exactly one overlay is ever visible.
    fn close_page_overlays(&mut self, cx: &mut Cx) {
        self.set_shortcuts_overlay(cx, false);
        if let Some(mut o) = self
            .ui
            .widget(cx, ids!(fonts_overlay))
            .borrow_mut::<crate::fonts_overlay::FontsOverlay>()
        {
            o.set_visible(cx, false);
        }
        if let Some(mut o) = self
            .ui
            .widget(cx, ids!(icons_overlay))
            .borrow_mut::<crate::icons_overlay::IconsOverlay>()
        {
            o.set_visible(cx, false);
        }
        if let Some(mut o) = self
            .ui
            .widget(cx, ids!(colors_overlay))
            .borrow_mut::<crate::colors_overlay::ColorsOverlay>()
        {
            o.set_visible(cx, false);
        }
    }

    /// Close every overlay/page, then show the requested style-guide page.
    fn open_page_overlay(&mut self, cx: &mut Cx, which: LogoCommand) {
        self.close_page_overlays(cx);
        if which == LogoCommand::Fonts {
            if let Some(mut o) = self
                .ui
                .widget(cx, ids!(fonts_overlay))
                .borrow_mut::<crate::fonts_overlay::FontsOverlay>()
            {
                o.set_visible(cx, true);
            }
        } else if which == LogoCommand::Icons {
            if let Some(mut o) = self
                .ui
                .widget(cx, ids!(icons_overlay))
                .borrow_mut::<crate::icons_overlay::IconsOverlay>()
            {
                o.set_visible(cx, true);
            }
        } else if which == LogoCommand::Colors {
            if let Some(mut o) = self
                .ui
                .widget(cx, ids!(colors_overlay))
                .borrow_mut::<crate::colors_overlay::ColorsOverlay>()
            {
                o.set_visible(cx, true);
            }
        }
    }

    fn dock_states(&mut self, cx: &mut Cx) -> (crate::dock::DockState, crate::dock::DockState) {
        let tree = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow::<crate::tree_panel::ProjectTree>()
            .map(|panel| panel.dock_state())
            .unwrap_or(crate::dock::DockState::Flag);
        let inspector = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow::<crate::inspector_panel::Inspector>()
            .map(|panel| panel.dock_state())
            .unwrap_or(crate::dock::DockState::Flag);
        (tree, inspector)
    }

    fn apply_dock_states(
        &mut self,
        cx: &mut Cx,
        tree: crate::dock::DockState,
        inspector: crate::dock::DockState,
    ) {
        if let Some(mut panel) = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
        {
            if panel.dock_state() != tree {
                if tree == crate::dock::DockState::Pinned {
                    panel.open_dock(cx);
                } else {
                    panel.close_dock(cx);
                }
            }
        }
        if let Some(mut panel) = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            if panel.dock_state() != inspector {
                if inspector == crate::dock::DockState::Pinned {
                    panel.open_dock(cx);
                } else {
                    panel.close_dock(cx);
                }
            }
        }
    }

    fn route_narrow_dock_pointer(&mut self, cx: &mut Cx, event: &Event, popup_was_open: bool) {
        if !self.narrow {
            return;
        }
        let (tree_state, inspector_state) = self.dock_states(cx);
        let tree_rect = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow::<crate::tree_panel::ProjectTree>()
            .map(|panel| panel.drawn_rect(cx))
            .unwrap_or_default();
        let inspector_rect = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow::<crate::inspector_panel::Inspector>()
            .map(|panel| panel.drawn_rect(cx))
            .unwrap_or_default();
        let canvas_rect = self.ui.widget(cx, ids!(canvas)).area().rect(cx);
        let contains = |point| {
            open_overlay_contains(
                point,
                tree_state,
                tree_rect,
                inspector_state,
                inspector_rect,
            )
        };
        match event {
            Event::MouseMove(e) => {
                self.pointer_in_narrow_dock = contains(e.abs);
            }
            Event::MouseDown(e) if e.button.is_primary() => {
                let inside = contains(e.abs);
                self.pointer_in_narrow_dock = inside;
                if !popup_was_open
                    && should_dismiss_narrow_dock(
                        e.abs,
                        canvas_rect,
                        tree_state,
                        tree_rect,
                        inspector_state,
                        inspector_rect,
                    )
                {
                    self.apply_dock_states(cx, DockState::Flag, DockState::Flag);
                }
            }
            _ => {}
        }
    }

    /// Reconcile responsive mode and panel state, then update reservation slots
    /// and overlay hosts together so one layout model owns all dock geometry.
    fn sync_dock_slots(&mut self, cx: &mut Cx) {
        let viewport_w = self.window_bounds(cx).size.x;
        let next = next_narrow(self.narrow, viewport_w);
        if next != self.narrow {
            if let Some(mut root) = self
                .ui
                .widget(cx, ids!(popup_root))
                .borrow_mut::<PopupRoot>()
            {
                if root.is_open_for(live_id!(doc_switcher)) {
                    root.close(cx);
                }
            }
            self.narrow = next;
            if self.narrow {
                let (tree, inspector) = self.dock_states(cx);
                let (tree, inspector) = crate::dock::narrow_entry_states(tree, inspector);
                self.apply_dock_states(cx, tree, inspector);
            }
            if let Some(mut tabs) = self
                .ui
                .widget(cx, ids!(doc_tabs))
                .borrow_mut::<crate::doc_tabs::DocTabs>()
            {
                tabs.set_narrow(cx, self.narrow);
            }
            cx.redraw_all();
        }

        let (tree_state, inspector_state) = self.dock_states(cx);
        let layout = crate::dock::responsive_layout(
            self.narrow,
            viewport_w,
            tree_state,
            inspector_state,
            crate::tree_panel::PROJECT_TREE_W,
            crate::inspector_panel::INSPECTOR_W,
        );
        if layout != self.dock_layout {
            self.dock_layout = layout;
            if let Some(mut view) = self.ui.widget(cx, ids!(left_slot)).borrow_mut::<View>() {
                view.walk.width = Size::Fixed(layout.left_slot);
            }
            if let Some(mut view) = self.ui.widget(cx, ids!(right_slot)).borrow_mut::<View>() {
                view.walk.width = Size::Fixed(layout.right_slot);
            }
            if let Some(mut view) = self.ui.widget(cx, ids!(tree_host)).borrow_mut::<View>() {
                view.walk.width = Size::Fixed(layout.tree_body);
            }
            if let Some(mut view) = self
                .ui
                .widget(cx, ids!(inspector_host))
                .borrow_mut::<View>()
            {
                view.walk.width = Size::Fixed(layout.inspector_body);
            }
            cx.redraw_all();
        }
        self.ui
            .widget(cx, ids!(tree_btn))
            .as_icon_button()
            .set_active(cx, tree_state == crate::dock::DockState::Pinned);
        let header_height = self
            .ui
            .widget(cx, ids!(document_header))
            .borrow_mut::<crate::document_header::DocumentHeader>()
            .map(|mut header| {
                header.set_right_dock_active(cx, inspector_state == crate::dock::DockState::Pinned);
                header.visible_height()
            })
            .unwrap_or(0.0);
        let inspector_top = crate::dock::narrow_inspector_top(self.narrow, header_height);
        if let Some(mut view) = self
            .ui
            .widget(cx, ids!(inspector_host))
            .borrow_mut::<View>()
        {
            if (view.walk.margin.top - inspector_top).abs() > 0.5 {
                view.walk.margin.top = inspector_top;
                cx.redraw_all();
            }
        }
        self.sync_tree_gap(cx, layout.left_slot);
    }

    /// Push the launch-flag marks into `AgentMark`. Called at startup AND from
    /// `rehydrate`: the `T` theme toggle goes through `cx.request_live_edit()`
    /// -> `Apply::Reload`, which resets the widget's `#[rust]` state, so without
    /// the second call both marks vanish the first time an agent toggles the
    /// theme and the window silently becomes indistinguishable again.
    fn apply_agent_marks(&mut self, cx: &mut Cx) {
        let badge = self.agent_badge.clone();
        let tint = self.agent_tint;
        if let Some(mut mark) = self
            .ui
            .widget(cx, ids!(agent_mark))
            .borrow_mut::<crate::agent_mark::AgentMark>()
        {
            mark.set_marks(cx, badge, tint);
        }
    }

    /// Measure the title row and push its width to `AgentMark`, which draws
    /// across it with `draw_abs` (it is mounted zero-width, so it cannot learn
    /// the row width from its own turtle). Same measure-and-push shape as
    /// `sync_tree_gap` feeding `DocTabs::set_left_overshoot`.
    ///
    /// The min/max/close cluster shares this row, and the marker is a
    /// RIGHT-floated pill, so the cluster's width is subtracted -- otherwise the
    /// pill floats to the window edge and lands underneath the buttons.
    fn sync_agent_row(&mut self, cx: &mut Cx) {
        if self.agent_badge.is_none() && self.agent_tint.is_none() {
            return;
        }
        let w = (self.ui.widget(cx, ids!(title_row)).area().rect(cx).size.x
            - self
                .ui
                .widget(cx, ids!(windows_buttons))
                .area()
                .rect(cx)
                .size
                .x)
            .max(0.0);
        if (w - self.agent_row_w).abs() <= 0.5 {
            return;
        }
        self.agent_row_w = w;
        if let Some(mut mark) = self
            .ui
            .widget(cx, ids!(agent_mark))
            .borrow_mut::<crate::agent_mark::AgentMark>()
        {
            mark.set_row_width(cx, w);
        }
    }

    /// Size the caption's `tree_gap` so the tab STRIP's left edge lands on the
    /// tree column's right edge -- the invariant that makes the two-row caption
    /// read as one chrome mass: the tabs start exactly where `field_bg` steps in
    /// from full-width to column-width (the first card sits `TAB_LEFT_INSET`
    /// inside that). `[T]` itself leads the row and never moves; only the cards
    /// travel, because the control that moves them must not move itself.
    ///
    /// `tree_w` is the column's width in window coordinates (the body starts at
    /// x=0, so it is also the column's right edge). `tab_row` starts at x=0 and
    /// `[T]` occupies the first `TREE_BTN_W` of it, so the gap is what is left
    /// after the button; clamped at 0 so the collapsed
    /// state collapses the spacer instead of going negative and the strip sits
    /// immediately right of `[T]` (`TAB_LEFT_INSET` supplies the breathing gap).
    ///
    /// The row offset is read from the last-drawn `tab_row` rect rather than
    /// hardcoded, so a reshaped caption can't silently desync the cards from the
    /// column. That makes this a two-frame settle on the very first draw (the
    /// rect is zero until `tab_row` has been laid out once), hence the guard is
    /// on the computed gap rather than on `tree_w` alone.
    ///
    /// Same measured rects also drive the tab strip's top-rule left overshoot:
    /// `doc_tabs` no longer begins at the window's left edge, and the rule must
    /// reach back to `tab_row`'s left edge, which is the window edge in this
    /// full-width caption hierarchy.
    fn sync_tree_gap(&mut self, cx: &mut Cx, tree_w: f64) {
        let row_x = self.ui.widget(cx, ids!(tab_row)).area().rect(cx).pos.x;
        let tabs_x = self.ui.widget(cx, ids!(doc_tabs)).area().rect(cx).pos.x;
        let overshoot = (tabs_x - row_x).max(0.0);
        if (overshoot - self.rule_overshoot).abs() > 0.5 {
            self.rule_overshoot = overshoot;
            if let Some(mut tabs) = self
                .ui
                .widget(cx, ids!(doc_tabs))
                .borrow_mut::<crate::doc_tabs::DocTabs>()
            {
                tabs.set_left_overshoot(cx, overshoot);
            }
        }
        let gap = (tree_w - TREE_BTN_W).max(0.0);
        if (gap - self.tree_gap_w).abs() <= 0.5 {
            return;
        }
        self.tree_gap_w = gap;
        // The first card's lead-in is a pure function of the gap, so it rides
        // the same change guard. Open column (gap > 0): flush, so the card's
        // left flank lands on the tree column's right edge -- the canvas's left
        // edge -- and the chrome has no step in it. Collapsed (gap == 0): the
        // strip butts against `[T]`, which needs the breathing room back.
        if let Some(mut tabs) = self
            .ui
            .widget(cx, ids!(doc_tabs))
            .borrow_mut::<crate::doc_tabs::DocTabs>()
        {
            let inset = if gap > 0.5 {
                0.0
            } else {
                crate::doc_tabs::TAB_LEFT_INSET
            };
            tabs.set_lead_inset(cx, inset);
        }
        // Same seam as the dock slots: no live-DSL setter in this fork, so
        // mutate the public `walk` field and force a full relayout (the
        // `flow: Right` row must reflow, not just this child).
        if let Some(mut spacer) = self.ui.widget(cx, ids!(tree_gap)).borrow_mut::<View>() {
            spacer.walk.width = Size::Fixed(gap);
        }
        cx.redraw_all();
    }

    /// Push diagram name / node count / zoom / active tool into the bottom
    /// statusbar. Snapshot values -- called at each sync point (tab switch,
    /// startup, tool-dock mode change), not live during a canvas drag.
    fn sync_statusbar(&mut self, cx: &mut Cx) {
        let diagram_name = self
            .documents
            .tabs()
            .first()
            .map(|t| t.title.clone())
            .unwrap_or_default();
        // Read the surface the ACTIVE document actually draws on. Reading only
        // `ClassDiagramSurface` left a behavior document reporting whatever the
        // last class diagram had — an 11-node activity showed "1 node", and the
        // zoom percentage was equally stale. The behavior surface is its own
        // widget (`behavior_canvas`), so the active tab's category selects it.
        let behavior_active = matches!(
            self.documents
                .active_tab()
                .map(|t| t.presentation.category),
            Some(NavCategory::Behavior | NavCategory::Sequence)
        );
        let (node_count, zoom_pct) = if behavior_active {
            self.ui
                .widget(cx, ids!(behavior_canvas))
                .borrow_mut::<crate::canvas::BehaviorSurface>()
                .map(|b| (b.node_count(), b.zoom_pct()))
                .unwrap_or((0, 100))
        } else {
            self.ui
                .widget(cx, ids!(canvas))
                .borrow_mut::<crate::canvas::ClassDiagramSurface>()
                .map(|c| (c.node_count(), c.zoom_pct()))
                .unwrap_or((0, 100))
        };
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

    /// The open document changed. Schedules a save; does not perform one.
    ///
    /// Restarts the timer rather than extending it, so a burst of ops -- a drag
    /// that re-authors placement as it moves -- coalesces into a single save
    /// when it settles instead of one per frame.
    fn mark_dirty(&mut self, cx: &mut Cx) {
        self.schedule_save(cx);
    }

    fn schedule_save(&mut self, cx: &mut Cx) {
        cx.stop_timer(self.save_timer);
        self.save_timer = cx.start_timeout(SAVE_DEBOUNCE_SECS);
    }

    /// Persist the open bundle, by whatever means this build has.
    ///
    /// The editor has one document model and two very different backings, so
    /// this is the seam where that difference lives; callers only ever say the
    /// document changed (`mark_dirty`), never how to store it.
    fn save(&mut self, cx: &mut Cx) -> Result<(), String> {
        let revision = self.session.revision();
        let state = self.session.history_state();
        let snapshot = self.session.snapshot();
        if snapshot.dirty_revision.is_none() {
            return Ok(());
        }
        if snapshot.source.is_empty() {
            return Err("cannot save an empty bundle".to_string());
        }
        self.save_backend(cx, snapshot)?;
        self.session.mark_saved(revision, state);
        Ok(())
    }

    fn sync_save_error(&mut self, cx: &mut Cx) {
        let error = self.save_feedback.save_error().map(str::to_owned);
        if let Some(mut statusbar) = self
            .ui
            .widget(cx, ids!(statusbar))
            .borrow_mut::<crate::statusbar::Statusbar>()
        {
            statusbar.set_save_error(cx, error.as_deref());
        }
    }

    fn save_or_retry(&mut self, cx: &mut Cx, retry_on_error: bool) -> Result<(), String> {
        let result = self.save(cx);
        if let Err(error) = &result {
            log!("failed to save open document: {error}");
            if retry_on_error && self.session.is_dirty() {
                self.schedule_save(cx);
            }
        }
        self.save_feedback.finish_save(&result);
        self.sync_save_error(cx);
        result
    }

    /// Browser backing: the URL fragment is the whole filesystem.
    ///
    /// Two things ride on this. A refresh restores the document, because
    /// `handle_startup` decodes exactly this fragment. And the update-check
    /// toast in `index.html` (see `scripts/inject-runtime-shell.mjs`) can build
    /// its reload URL from `location.hash` alone -- it never has to call into
    /// wasm, which makepad gives us no channel for anyway.
    ///
    /// `replace`, not push: an edit is not a navigation, and one history entry
    /// per save would make Back mean "undo some edits, sometimes".
    #[cfg(target_arch = "wasm32")]
    fn save_backend(
        &self,
        cx: &mut Cx,
        snapshot: crate::editor_session::EditorSnapshot<'_>,
    ) -> Result<(), String> {
        cx.browser_update_url(
            &format!("#{}", waml::share::encode_source(snapshot.source)),
            true,
        );
        Ok(())
    }

    /// Native backing: atomically replace each authored file in the opened OKF
    /// directory. The helper validates bundle paths before performing writes.
    #[cfg(not(target_arch = "wasm32"))]
    fn save_backend(
        &self,
        _cx: &mut Cx,
        snapshot: crate::editor_session::EditorSnapshot<'_>,
    ) -> Result<(), String> {
        let Some(root) = self.open_dir.as_deref() else {
            return Err("native bundle has no opened directory".to_string());
        };
        crate::native_save::save_snapshot_atomic(root, snapshot)
            .map_err(|error| format!("failed to save OKF dir {root:?}: {error}"))
    }

    /// Push the canvas's current conflict count onto the toolbar badge.
    fn sync_conflict_badge(&mut self, cx: &mut Cx) {
        let n = self
            .ui
            .widget(cx, ids!(canvas))
            .borrow::<crate::canvas::ClassDiagramSurface>()
            .map(|c| c.conflict_count())
            .unwrap_or(0);
        if let Some(mut badge) = self
            .ui
            .widget(cx, ids!(conflict_badge))
            .borrow_mut::<crate::conflict_badge::ConflictBadge>()
        {
            badge.set_count(cx, n);
        }
    }

    /// Open the grouped, deletable conflict-error-list card, anchored under
    /// the toolbar badge. Shared by the badge click and the delete-refresh
    /// path (which re-anchors the still-open list after a row is removed).
    fn open_conflict_list(&mut self, cx: &mut Cx, conflicts: Vec<crate::scene::SceneConflict>) {
        let btn = self.ui.widget(cx, ids!(conflict_badge)).area().rect(cx);
        let anchor = dvec2(
            btn.pos.x,
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
                PopupSpec::Conflict {
                    tag: live_id!(conflict_list),
                    anchor,
                    bounds,
                    conflicts,
                },
            );
        }
    }

    /// Read `dir` off disk and populate the editor. Returns `false` (having
    /// `log!`d) only when the model fails to load, so the caller keeps the
    /// start screen up.
    #[cfg(not(target_arch = "wasm32"))]
    fn open_dir(&mut self, cx: &mut Cx, dir: &Path, wanted_diagram: Option<&str>) -> bool {
        let next_root = dir.to_path_buf();
        let transition = replace_after_save(
            || self.save(cx),
            || {
                load::read_bundle(&next_root)
                    .map_err(|error| format!("failed to load OKF dir {next_root:?}: {error}"))
            },
        );
        let bundle = match transition {
            Ok(loaded) => loaded,
            Err(BackingTransitionError::Save(error)) => {
                self.save_feedback.finish_save(&Err(error.clone()));
                self.schedule_save(cx);
                self.sync_save_error(cx);
                log!("{error}");
                return false;
            }
            Err(BackingTransitionError::Load(error)) => {
                // Any old edits were successfully flushed before the new load
                // was attempted, so the retained backing is now clean.
                self.save_feedback.finish_save(&Ok(()));
                self.sync_save_error(cx);
                log!("{error}");
                return false;
            }
        };
        // Folder basename backs the display name when the bundle has no root
        // name of its own. `..` / drive-root degenerate to an empty basename;
        // "bundle" is the last-ditch label.
        let display_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| !n.is_empty())
            .unwrap_or("bundle")
            .to_string();
        // Record this open in the recents store (best-effort; see config.rs).
        // Recents are a filesystem affordance: only an open with a path behind
        // it can be reopened later, so this stays on the `open_dir` side.
        if !self.open_bundle(cx, bundle, display_name, wanted_diagram) {
            return false;
        }
        self.open_dir = Some(next_root);
        let root_name = if self.session.uml_projection().path.is_empty() {
            self.open_name.as_str()
        } else {
            self.session.uml_projection().path.as_str()
        };
        crate::config::push_recent(dir, root_name);
        true
    }

    #[cfg(target_arch = "wasm32")]
    fn open_dir(&mut self, _cx: &mut Cx, _dir: &Path, _wanted_diagram: Option<&str>) -> bool {
        false
    }

    /// Populate the editor from an already-read bundle (tree, canvas, tabs,
    /// inspector, statusbar, diagram switcher). A model with zero diagrams
    /// still opens -- empty canvas, no diagram tab.
    ///
    /// Split out of `open_dir` so the web build, which has no filesystem, can
    /// open a model decoded from the URL fragment through exactly this path.
    /// Always returns `true`: the fallible part -- reading and parsing -- has
    /// already happened by the time it is called.
    fn open_bundle(
        &mut self,
        cx: &mut Cx,
        files: waml::source::SourceBundle,
        display_name: String,
        wanted_diagram: Option<&str>,
    ) -> bool {
        cx.stop_timer(self.save_timer);
        let change = match self.session.replace(files) {
            Ok(change) => change,
            Err(error) => {
                log!("failed to analyze replacement bundle: {error}");
                return false;
            }
        };
        debug_assert_eq!(change.revision, self.session.revision());
        self.save_feedback.opened_replacement_bundle();
        self.sync_save_error(cx);
        // Retain the raw bundle so drag-to-place ops can re-author `## Layout`
        // in-memory: the diagram view emits `Op::PlaceSet`, the shell applies it
        // against this bundle and rebuilds the model (see `handle_actions`).
        // Fresh model: recompute the type-filter chip's cycle and reset scope /
        // search / filter to the whole-model browse state.
        self.nav_kinds =
            crate::nav::kinds_in_model(self.session.okf_analysis(), self.session.uml_analysis());
        self.nav_state = NavState::default();

        self.open_name = display_name;

        let root_name = self
            .session
            .okf()
            .index("/")
            .and_then(|index| index.title.as_deref())
            .unwrap_or(self.open_name.as_str());
        self.ui.label(cx, ids!(model_name)).set_text(cx, root_name);

        self.refresh_nav(cx, true);

        // Start with the requested/first supported diagram, otherwise the first
        // indexed Concept. An empty bundle keeps an empty canvas and no tab.
        self.documents
            .replace_for_session(cx, &self.ui, &self.session, OpenTabs::default());
        self.view_history.reset(None);
        match crate::cli::select_initial_document(
            self.session.okf(),
            self.session.uml_projection(),
            wanted_diagram,
        ) {
            crate::cli::InitialDocument::Diagram(concept_id)
            | crate::cli::InitialDocument::Concept(concept_id) => {
                let concept_id = concept_id.to_owned();
                self.transition_document(cx, &concept_id, false);
            }
            crate::cli::InitialDocument::None => {
                log!(
                    "no documents in {:?}; opening bundle with an empty canvas",
                    self.open_name
                );
                // Empty scene draws nothing and `bounding_box` returns `None`, so
                // the fit path leaves the camera untouched (no divide-by-zero). No
                // diagram tab; the project tree was already populated by
                // `refresh_nav` above.
                if let Some(mut canvas) = self
                    .ui
                    .widget(cx, ids!(canvas))
                    .borrow_mut::<crate::canvas::ClassDiagramSurface>()
                {
                    canvas.clear(cx);
                }
            }
        }
        self.sync_document_shell(cx);
        true
    }

    /// The caption bar ships hidden and makepad only unhides it for the
    /// platforms that hand an app its own window chrome -- `sync_caption_bar_state`
    /// has arms for Windows, macOS and Wayland CSD, and an empty one for
    /// `OsType::Web`. That is the right default for a title bar, but ours also
    /// carries the logo, doc tabs, tree toggle and burger, so the browser
    /// build would come up with no navigation at all. Reveal it ourselves.
    #[cfg(target_arch = "wasm32")]
    fn reveal_caption_bar_on_web(&mut self, cx: &mut Cx) {
        self.ui.widget(cx, ids!(caption_bar)).set_visible(cx, true);
    }

    /// Reveal the editor, hide the start screen. `main_column` is a `View`
    /// (honors `WidgetRef::set_visible`); `StartScreen` is a custom widget
    /// whose no-op default `Widget::set_visible` means we must toggle its own
    /// `visible` flag via the borrowed inherent method instead.
    fn show_editor(&mut self, cx: &mut Cx) {
        self.editor_shown = true;
        self.ui.widget(cx, ids!(main_column)).set_visible(cx, true);
        // Caption burger + tree toggle + doc-tab strip belong to an open model.
        self.ui.widget(cx, ids!(menu_btn)).set_visible(cx, true);
        self.ui
            .widget(cx, ids!(menu_btn))
            .as_icon_button()
            .set_icon(cx, crate::icons::Icon::Menu);
        self.ui.widget(cx, ids!(tree_btn)).set_visible(cx, true);
        self.ui
            .widget(cx, ids!(tree_btn))
            .as_icon_button()
            .set_icon(cx, crate::icons::Icon::ListTree);
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
        // First, before the start-screen early return: the marks apply to both
        // screens.
        self.apply_agent_marks(cx);
        self.agent_row_w = 0.0; // force `sync_agent_row` to re-push after reload
        if !self.editor_shown {
            // Start screen: `show_start_screen` re-reads recents and re-shows.
            self.show_start_screen(cx);
            if let Some(mut tabs) = self
                .ui
                .widget(cx, ids!(doc_tabs))
                .borrow_mut::<crate::doc_tabs::DocTabs>()
            {
                tabs.set_narrow(cx, self.narrow);
            }
            self.dock_layout = crate::dock::ResponsiveDockLayout::default();
            self.tree_gap_w = -1.0;
            self.rule_overshoot = -1.0;
            self.sync_dock_slots(cx);
            return;
        }
        let root_name = if self.session.uml_projection().path.is_empty() {
            self.open_name.as_str()
        } else {
            self.session.uml_projection().path.as_str()
        };
        self.ui.label(cx, ids!(model_name)).set_text(cx, root_name);

        self.refresh_nav(cx, true);
        self.documents.sync_active(cx, &self.ui, &self.session);
        self.sync_document_shell(cx);
        self.show_editor(cx);
        if let Some(mut tabs) = self
            .ui
            .widget(cx, ids!(doc_tabs))
            .borrow_mut::<crate::doc_tabs::DocTabs>()
        {
            tabs.set_narrow(cx, self.narrow);
        }
        self.dock_layout = crate::dock::ResponsiveDockLayout::default();
        self.tree_gap_w = -1.0;
        self.rule_overshoot = -1.0;
        self.sync_dock_slots(cx);
    }

    /// Flush the current backing before closing it. A failed flush leaves the
    /// editor, source snapshots, and retry timer intact.
    fn close_model(&mut self, cx: &mut Cx) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        let result = {
            let root = self.open_dir.as_deref();
            close_after_save(&mut self.session, |session| {
                let snapshot = session.snapshot();
                if snapshot.dirty_revision.is_none() {
                    return Ok(());
                }
                let root =
                    root.ok_or_else(|| "native bundle has no opened directory".to_string())?;
                crate::native_save::save_snapshot_atomic(root, snapshot)
                    .map_err(|error| format!("failed to save OKF dir {root:?}: {error}"))
            })
        };

        #[cfg(target_arch = "wasm32")]
        let result = close_after_save(&mut self.session, |session| {
            if session.is_dirty() {
                cx.browser_update_url(
                    &format!("#{}", waml::share::encode_source(session.source())),
                    true,
                );
            }
            Ok(())
        });

        self.save_feedback.finish_save(&result);
        if let Err(error) = &result {
            log!("failed to close open document: {error}");
            self.schedule_save(cx);
            self.sync_save_error(cx);
            return false;
        }

        cx.stop_timer(self.save_timer);
        self.open_dir = None;
        self.sync_save_error(cx);
        self.show_start_screen(cx);
        true
    }

    /// Load recents into the start screen and reveal it, hiding the editor.
    fn show_start_screen(&mut self, cx: &mut Cx) {
        self.start_recents = crate::config::recents();
        let rows: Vec<crate::start_screen::RecentRow> = self
            .start_recents
            .iter()
            // Keep the complete config list in `start_recents`; `StartScreen`
            // caps only its rendered copy to the first five. Screen indices
            // therefore still map 1:1 to this backing list.
            .map(|r| crate::start_screen::RecentRow {
                title: r.title().to_string(),
                path: r.path().display().to_string(),
                when: format_opened(r.opened_at()),
                pinned: r.pinned(),
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
        // No open model on the start screen: hide burger + tree toggle +
        // doc-tab strip, and drop the editor's tab state so a re-open starts
        // clean rather than inheriting the closed model's tabs (open_dir
        // rebuilds from scratch).
        self.ui.widget(cx, ids!(menu_btn)).set_visible(cx, false);
        self.ui.widget(cx, ids!(tree_btn)).set_visible(cx, false);
        // Replacing the host with no tabs applies hidden chrome through the
        // same transition path used when the final document closes.
        // Clear the stale model title: the caption bar keeps drawing (logo +
        // name) even with no model open, so a leftover name reads as if the
        // closed model were still loaded.
        self.ui.label(cx, ids!(model_name)).set_text(cx, "");
        self.documents
            .replace_for_session(cx, &self.ui, &self.session, OpenTabs::default());
        self.sync_document_shell(cx);
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
    #[cfg(not(target_arch = "wasm32"))]
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

    /// The browser has no directory picker and no filesystem to point one at,
    /// so the web build gets its model from the URL fragment instead. Keeping
    /// the method (rather than cfg-ing out every call site) leaves the start
    /// screen and burger wiring identical across targets.
    #[cfg(target_arch = "wasm32")]
    fn open_model_via_picker(&mut self, _cx: &mut Cx) {}

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
        let view = crate::nav::view(
            self.session.okf_analysis(),
            self.session.uml_analysis(),
            &self.nav_state,
        );
        let chip = crate::nav::chip_label(self.nav_state.filter).to_string();
        let title = scope_changed.then(|| {
            crate::nav::packages(self.session.okf_analysis(), self.session.uml_analysis())
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
            panel.set_view_with_fold_reset(cx, view, scope_changed);
            panel.set_chip_filter(cx, self.nav_state.filter, &chip);
            if let Some(title) = title {
                panel.set_scope_title(cx, title);
                panel.set_query_text(cx, &self.nav_state.query);
            }
        }
    }
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
            icon: Some(Icon::SlidersHorizontal),
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(about),
            label: "About".into(),
            icon: Some(Icon::Info),
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(fonts),
            label: "Fonts".into(),
            icon: Some(Icon::Paintbrush),
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(icons),
            label: "Icons".into(),
            icon: Some(Icon::SquareMenu),
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(colors),
            label: "Colors".into(),
            icon: Some(Icon::Squircle),
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(exit),
            label: "Exit".into(),
            icon: Some(Icon::CircleX),
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
            icon: Some(Icon::SquarePlus),
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(open_model),
            label: "Open model".into(),
            // The open-door glyph, pairing with Close model's door-closed.
            icon: Some(Icon::DoorOpen),
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(close_model),
            label: "Close model".into(),
            // The door-closed glyph, drawn directly from the catalog.
            icon: Some(Icon::DoorClosed),
            danger: false,
            enabled: true,
        },
    ]
}

const DOC_SWITCHER_MAX_H: f64 = 360.0;

fn doc_switcher_items(open: &[crate::doc_tabs::DocTab]) -> Vec<crate::popup::base::PopupItem> {
    open.iter()
        .map(|tab| crate::popup::base::PopupItem {
            id: tab.id,
            label: tab.title.clone(),
            icon: Some(tab.presentation.icon),
            danger: false,
            enabled: true,
        })
        .collect()
}

/// The logo-radial commands `App` acts on. `Cancel` is intentionally absent:
/// committing the Cancel wedge just closes the radial (mapped to `None`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogoCommand {
    Properties,
    About,
    Fonts,
    Icons,
    Colors,
    Exit,
}

/// Map a radial-committed `LiveId` to a logo command. `None` = not one of ours
/// (Cancel / node ids / unknown).
pub fn logo_command_for(id: LiveId) -> Option<LogoCommand> {
    if id == live_id!(properties) {
        Some(LogoCommand::Properties)
    } else if id == live_id!(about) {
        Some(LogoCommand::About)
    } else if id == live_id!(fonts) {
        Some(LogoCommand::Fonts)
    } else if id == live_id!(icons) {
        Some(LogoCommand::Icons)
    } else if id == live_id!(colors) {
        Some(LogoCommand::Colors)
    } else if id == live_id!(exit) {
        Some(LogoCommand::Exit)
    } else {
        None
    }
}

/// Map a `ConflictListAction::Delete` to the `Op::PlaceRm` that removes it
/// from `diagram`'s `## Layout` section. Pure so it is unit-testable without
/// a live `Cx`/`App`; `None` for any other action (nothing to remove).
fn place_rm_for(
    diagram: &str,
    action: &crate::popup::conflict_list::ConflictListAction,
) -> Option<waml::uml::Op> {
    match action {
        crate::popup::conflict_list::ConflictListAction::Delete { subject, reference } => {
            Some(waml::uml::Op::PlacementRemove {
                diagram: diagram.to_string(),
                subject_slug: subject.clone(),
                reference_slug: reference.clone(),
            })
        }
        _ => None,
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

/// The current URL fragment, `#` included, or `None` when the page has none.
///
/// makepad hands the browser's `location` to Rust as `OsType::Web`, refreshed
/// on every `hashchange`, so this is a plain read -- no JS interop needed.
#[cfg(target_arch = "wasm32")]
fn web_location_hash(cx: &Cx) -> Option<String> {
    match cx.os_type() {
        makepad_widgets::makepad_platform::OsType::Web(params) if !params.hash.is_empty() => {
            Some(params.hash.clone())
        }
        _ => None,
    }
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
        let Some(fragment) = web_location_hash(cx) else {
            self.show_start_screen(cx);
            return;
        };
        if !waml::share::is_share_link(&fragment) {
            // Some other anchor (or nothing at all): not our business.
            self.show_start_screen(cx);
            return;
        }
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

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        self.handle_action_batch(cx, actions);
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
        // Theme live-edit: the framework has already re-run `script_mod` and
        // `Apply::Reload`ed the widget tree (wiping imperatively-pushed
        // content) *before* this `Event::LiveEdit` lands, so re-hydrate now.
        if let Event::LiveEdit = event {
            self.rehydrate(cx);
        }

        // Logo FPS-heat meter: `App` forwards every raw event to the meter,
        // which owns all interaction-span detection (primary press/release plus
        // the mouse-wheel scroll tail) and framerate sampling. When it reports a
        // change, push the fresh colour/strength to the top-bar logo. This is
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

        // Model history owns the standard platform chords, even while an
        // editor has focus. Returning here keeps focused widgets from also
        // applying a competing local undo/redo when a stack is empty.
        if let Event::KeyDown(ke) = event {
            let macos = matches!(
                cx.os_type(),
                makepad_widgets::makepad_platform::OsType::Macos
            );
            if let Some(command) =
                crate::shortcuts::history_command_for(ke.key_code, ke.modifiers, macos)
            {
                match command {
                    crate::shortcuts::HistoryCommand::Undo => {
                        self.perform_undo(cx);
                    }
                    crate::shortcuts::HistoryCommand::Redo => {
                        self.perform_redo(cx);
                    }
                }
                return;
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
                    KeyCode::Escape => self.close_page_overlays(cx),
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

        // Escape always returns an active diagram tab from its properties page
        // to the canvas, including while one of the property fields has focus.
        if matches!(event, Event::KeyDown(ke) if ke.key_code == KeyCode::Escape) {
            self.documents.on_active_escape(cx, &self.ui);
            self.session.break_edit_merge_group();
        }

        // Debounced save: the document has sat unchanged for a beat, so persist
        // it through whichever backing this build has.
        if should_flush_save(event) {
            // A graceful quit can be cancelled, so keep the app alive and retry
            // after surfacing an error. Shutdown cannot be cancelled and remains
            // a final best-effort write for forced/platform teardown paths.
            let retry_on_error = matches!(event, Event::QuitRequested(_));
            let result = self.save_or_retry(cx, retry_on_error);
            prevent_quit_after_failed_save(event, &result);
        } else if self.save_timer.is_event(event).is_some() {
            let _ = self.save_or_retry(cx, true);
        }

        // Single popup seam: light-dismiss + active-surface routing + emission.
        let popup_was_open = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow::<PopupRoot>()
            .map(|root| root.is_open())
            .unwrap_or(false);
        if let Some(mut pr) = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow_mut::<PopupRoot>()
        {
            pr.route(cx, event);
        }
        self.route_narrow_dock_pointer(cx, event, popup_was_open);

        self.ui.handle_event(cx, event, &mut Scope::empty());
        if matches!(event, Event::Draw(_)) {
            self.apply_pending_fragment(cx);
            self.apply_pending_anchor_restore(cx);
        }
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
            // The logo also lives in the caption drag region; without this
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
                .rect(cx)
                .contains(dq.abs);
            // Same for the tab row's tree-column toggle: it sits in the caption
            // drag region, so without this its clicks become window drags and
            // the toggle is dead.
            let over_tree_btn = self
                .ui
                .widget(cx, ids!(tree_btn))
                .as_icon_button()
                .rect(cx)
                .contains(dq.abs);
            // Breadcrumb segments and the right-dock button share the header's
            // live rect. Keep that interactive row in client space.
            let document_header = self.ui.widget(cx, ids!(document_header));
            let over_document_header = document_header
                .borrow::<crate::document_header::DocumentHeader>()
                .map(|header| {
                    header.visible_height() > 0.0
                        && document_header.area().rect(cx).contains(dq.abs)
                })
                .unwrap_or(false);
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
            if over_tab
                || over_logo
                || over_btn
                || over_tree_btn
                || over_document_header
                || menu_open
            {
                dq.response.set(WindowDragQueryResponse::Client);
            }
        }

        // Push each panel's DockState-driven slot width onto its reservation
        // spacer every frame (including NextFrame, so the peek-timer's own
        // dock transitions are picked up promptly).
        self.sync_dock_slots(cx);
        // Same shape for the marker's row width: it is mounted zero-width, so
        // `App` is the only thing that knows how wide the title row is.
        self.sync_agent_row(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        close_after_save, doc_switcher_items, logo_command_for, next_narrow, open_overlay_contains,
        place_rm_for, prevent_quit_after_failed_save, project_document_header, replace_after_save,
        should_dismiss_narrow_dock, should_flush_save, App, BackingTransitionError, LogoCommand,
        PendingFragment, SaveFeedback, TransitionCause,
    };
    use crate::doc_tabs::{DocTab, OpenTabs};
    use crate::doc_view::{BodyWidgets, DocView, DocumentHeaderChrome, ViewData};
    use crate::dock::DockState;
    use crate::document::{DocumentPresentation, NavCategory, OpenDocument};
    use crate::document_host::DocumentCommand;
    use crate::icons::{Icon, IconSet};
    use crate::nav::NavState;
    use crate::navigation::{
        BreadcrumbSegment, NavigationIntent, NavigationTarget, OpenDisposition,
    };
    use crate::platform_browser::ExternalUrlAdapter;
    use crate::popup::conflict_list::ConflictListAction;
    use crate::tree::TreeKind;
    use crate::view_history::{HistoryDirection, ViewAnchor, ViewLocation};
    use makepad_widgets::*;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    #[derive(Default)]
    struct FakeBrowser {
        opened: Vec<String>,
        error: Option<String>,
    }

    impl ExternalUrlAdapter for FakeBrowser {
        fn open(&mut self, _cx: &mut Cx, url: &str) -> Result<(), String> {
            self.opened.push(url.into());
            self.error.clone().map_or(Ok(()), Err)
        }
    }

    struct ResettingAnchorView(Rc<RefCell<ViewAnchor>>);

    impl DocView for ResettingAnchorView {
        fn sync(&mut self, _: &mut Cx, _: &BodyWidgets, _: ViewData<'_>) {
            *self.0.borrow_mut() = ViewAnchor::None;
        }

        fn handle(
            &mut self,
            _: &mut Cx,
            _: &BodyWidgets,
            _: &Actions,
            _: ViewData<'_>,
        ) -> crate::doc_view::ViewOutcome {
            crate::doc_view::ViewOutcome::default()
        }

        fn chrome(&self) -> crate::doc_view::BodyChrome {
            crate::doc_view::BodyChrome::HIDDEN
        }

        fn capture_anchor(&self, _: &BodyWidgets) -> ViewAnchor {
            self.0.borrow().clone()
        }

        fn restore_anchor(
            &mut self,
            _: &mut Cx,
            _: &BodyWidgets,
            _: ViewData<'_>,
            anchor: &ViewAnchor,
        ) -> bool {
            *self.0.borrow_mut() = anchor.clone();
            true
        }
    }

    fn navigation_app_with_anchor_probe(anchor: ViewAnchor) -> (Cx, App, Rc<RefCell<ViewAnchor>>) {
        let (mut cx, mut app) = navigation_app();
        let state = Rc::new(RefCell::new(anchor.clone()));
        app.documents.transition(
            &mut cx,
            &app.ui,
            &app.session,
            DocumentCommand::Open {
                document: OpenDocument {
                    tab_id: LiveId::from_str("anchor-probe"),
                    concept_id: "sales/order".into(),
                    kind: crate::view_history::DocumentKind::Primary,
                    title: "Order".into(),
                    presentation: DocumentPresentation {
                        icon: Icon::StickyNote,
                        accent: None,
                        category: NavCategory::OkfDocument,
                    },
                    view: Box::new(ResettingAnchorView(state.clone())),
                },
                persistent: true,
            },
        );
        *state.borrow_mut() = anchor;
        app.view_history
            .reset(app.documents.capture_active_location(&mut cx, &app.ui));
        (cx, app, state)
    }

    fn navigation_app() -> (Cx, App) {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.widget_tree_mark_dirty(WidgetUid(0));
        let mut app = cx.with_vm(App::script_new_with_default);
        let source = waml::source::SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            (
                "sales/index.md",
                "# Sales\n\n* [Order](order.md)\n* [Customer](customer.md)\n",
            ),
            (
                "sales/order.md",
                "---\ntype: Runbook\ntitle: Order\n---\n# Order\n",
            ),
            (
                "sales/customer.md",
                "---\ntype: Runbook\ntitle: Customer\n---\n# Customer\n\n## History\nDetails\n",
            ),
            (
                "sales/next.md",
                "---\ntype: Runbook\ntitle: Next\n---\n# Next\n\n## Details\nRecorded\n",
            ),
        ])
        .unwrap();
        app.session.replace(source).unwrap();
        let mut project_tree = cx.with_vm(crate::tree_panel::ProjectTree::script_new_with_default);
        let file_tree =
            WidgetRef::new_with_inner(Box::new(cx.with_vm(FileTree::script_new_with_default)));
        project_tree.children.push((live_id!(file_tree), file_tree));
        project_tree.set_view(
            &mut cx,
            crate::nav::view(
                app.session.okf_analysis(),
                app.session.uml_analysis(),
                &NavState::default(),
            ),
        );
        let project_tree = WidgetRef::new_with_inner(Box::new(project_tree));
        let statusbar = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(crate::statusbar::Statusbar::script_new_with_default),
        ));
        let document_header = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(crate::document_header::DocumentHeader::script_new_with_default),
        ));
        let inspector = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(crate::inspector_panel::Inspector::script_new_with_default),
        ));
        let mut ui = cx.with_vm(View::script_new_with_default);
        ui.children.push((live_id!(project_tree), project_tree));
        ui.children.push((live_id!(statusbar), statusbar));
        ui.children
            .push((live_id!(document_header), document_header));
        ui.children.push((live_id!(inspector), inspector));
        app.ui = WidgetRef::new_with_inner(Box::new(ui));
        (cx, app)
    }

    fn widget_action(uid: WidgetUid, action: impl WidgetActionTrait + 'static) -> Action {
        Box::new(WidgetAction {
            data: None,
            action: Box::new(action),
            widget_uid: uid,
            group: None,
        })
    }

    fn mount_markdown_surface(cx: &mut Cx, app: &mut App) {
        let markdown =
            WidgetRef::new_with_inner(Box::new(cx.with_vm(Markdown::script_new_with_default)));
        let mut surface = cx.with_vm(View::script_new_with_default);
        surface.children.push((live_id!(md), markdown));
        let surface = WidgetRef::new_with_inner(Box::new(surface));
        app.ui
            .borrow_mut::<View>()
            .expect("test root view is mounted")
            .children
            .push((live_id!(markdown_surface), surface));
        cx.widget_tree_mark_dirty(app.ui.widget_uid());
    }

    fn record_markdown_anchors(cx: &mut Cx, app: &App) {
        let draw_event = DrawEvent {
            redraw_all: true,
            ..DrawEvent::default()
        };
        let pass = DrawPass::new_with_name(cx, "markdown-navigation-test");
        let mut draw_list = DrawList2d::new(cx);
        let mut draw_cx = CxDraw::new(cx, &draw_event);
        draw_cx.begin_pass(&pass, None);
        draw_list.begin_always(&mut draw_cx);
        {
            let mut cx_2d = Cx2d::new(&mut draw_cx);
            cx_2d.begin_root_turtle(dvec2(800.0, 600.0), Layout::default());
            app.ui
                .widget(&cx_2d, ids!(markdown_surface.md))
                .draw_walk_all(&mut cx_2d, &mut Scope::empty(), Walk::fill());
            cx_2d.end_turtle();
            draw_list.end(&mut cx_2d);
        }
        draw_cx.end_pass(&pass);
    }

    fn draw_document_header(cx: &mut Cx, app: &App, size: DVec2) -> Rect {
        let draw_event = DrawEvent {
            redraw_all: true,
            ..DrawEvent::default()
        };
        let pass = DrawPass::new_with_name(cx, "document-header-test");
        let mut draw_list = DrawList2d::new(cx);
        let mut draw_cx = CxDraw::new(cx, &draw_event);
        draw_cx.begin_pass(&pass, None);
        draw_list.begin_always(&mut draw_cx);
        {
            let mut cx_2d = Cx2d::new(&mut draw_cx);
            cx_2d.begin_root_turtle(size, Layout::default());
            app.ui.widget(&cx_2d, ids!(document_header)).draw_walk_all(
                &mut cx_2d,
                &mut Scope::empty(),
                Walk::fill(),
            );
            cx_2d.end_turtle();
            draw_list.end(&mut cx_2d);
        }
        draw_cx.end_pass(&pass);
        drop(draw_cx);
        app.ui.widget(cx, ids!(document_header)).area().rect(cx)
    }

    fn mounted_production_shell() -> (Cx, App) {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let app = cx.with_vm(|vm| {
            let value = <App as AppMain>::script_mod(vm);
            let mut app = <App as ScriptNew>::script_from_value(vm, value);
            <App as AppMain>::after_new_from_script(vm, &mut app);
            app
        });
        (cx, app)
    }

    #[derive(Debug)]
    struct DockAreas {
        body: Rect,
        left_slot: Rect,
        right_slot: Rect,
        header: Rect,
        center: Rect,
        inspector: Rect,
    }

    fn draw_mounted_dock(cx: &mut Cx, app: &App, size: DVec2) -> DockAreas {
        let draw_event = DrawEvent {
            redraw_all: true,
            ..DrawEvent::default()
        };
        let pass = DrawPass::new_with_name(cx, "mounted-dock-test");
        let mut draw_list = DrawList2d::new(cx);
        let mut draw_cx = CxDraw::new(cx, &draw_event);
        draw_cx.begin_pass(&pass, None);
        draw_list.begin_always(&mut draw_cx);
        {
            let mut cx_2d = Cx2d::new(&mut draw_cx);
            cx_2d.begin_root_turtle(size, Layout::default());
            app.ui.widget(&cx_2d, ids!(dock_body)).draw_walk_all(
                &mut cx_2d,
                &mut Scope::empty(),
                Walk::fill(),
            );
            cx_2d.end_turtle();
            draw_list.end(&mut cx_2d);
        }
        draw_cx.end_pass(&pass);
        drop(draw_cx);

        let rect = |id_path| app.ui.widget(cx, id_path).area().rect(cx);
        DockAreas {
            body: rect(ids!(dock_body)),
            left_slot: rect(ids!(left_slot)),
            right_slot: rect(ids!(right_slot)),
            header: rect(ids!(document_header)),
            center: rect(ids!(center_stack)),
            inspector: rect(ids!(inspector_host)),
        }
    }

    fn configure_mounted_dock(
        cx: &mut Cx,
        app: &mut App,
        size: DVec2,
        tree: DockState,
        inspector: DockState,
        header_visible: bool,
    ) {
        let window_id = app
            .ui
            .window(cx, ids!(main_window))
            .window_id()
            .expect("production shell mounts main_window");
        cx.windows[window_id].window_geom.inner_size = size;
        cx.windows[window_id].window_geom.outer_size = size;
        app.apply_dock_states(cx, tree, inspector);
        let header_widget = app.ui.widget(cx, ids!(document_header));
        let mut header = header_widget
            .borrow_mut::<crate::document_header::DocumentHeader>()
            .expect("production shell mounts document_header");
        if header_visible {
            header.set_segments(
                cx,
                vec![BreadcrumbSegment {
                    title: "Order".into(),
                    target: NavigationTarget::Document {
                        concept_id: "sales/order".into(),
                        fragment: None,
                    },
                }],
            );
            header.set_right_dock(cx, Some(Icon::SlidersHorizontal));
        } else {
            header.set_segments(cx, Vec::new());
            header.set_right_dock(cx, None);
        }
        drop(header);
        app.sync_dock_slots(cx);
    }

    fn assert_near(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 0.5,
            "expected {actual} to be within 0.5px of {expected}"
        );
    }

    fn drawn_header_right_dock_active(cx: &mut Cx, app: &App) -> bool {
        let header = app.ui.widget(cx, ids!(document_header));
        let active = header
            .borrow::<crate::document_header::DocumentHeader>()
            .expect("production shell mounts document_header")
            .test_right_dock_active(cx)
            .expect("production shell draws the visible right button");
        active
    }

    #[test]
    fn mounted_dock_areas_follow_wide_and_narrow_production_layout() {
        let wide_size = dvec2(1_200.0, 700.0);
        let (mut cx, mut app) = mounted_production_shell();
        configure_mounted_dock(
            &mut cx,
            &mut app,
            wide_size,
            DockState::Pinned,
            DockState::Pinned,
            true,
        );
        let wide = draw_mounted_dock(&mut cx, &app, wide_size);
        assert_near(wide.body.size.x, wide_size.x);
        assert_near(wide.left_slot.size.x, crate::tree_panel::PROJECT_TREE_W);
        assert_near(wide.right_slot.size.x, crate::inspector_panel::INSPECTOR_W);
        assert_near(
            wide.header.pos.x,
            wide.left_slot.pos.x + wide.left_slot.size.x,
        );
        assert_near(
            wide.header.pos.x + wide.header.size.x,
            wide.right_slot.pos.x,
        );
        assert!(drawn_header_right_dock_active(&mut cx, &app));

        let narrow_size = dvec2(560.0, 700.0);
        let (mut cx, mut app) = mounted_production_shell();
        configure_mounted_dock(
            &mut cx,
            &mut app,
            narrow_size,
            DockState::Flag,
            DockState::Pinned,
            true,
        );
        let narrow_visible = draw_mounted_dock(&mut cx, &app, narrow_size);
        assert_near(
            narrow_visible.header.size.y,
            crate::document_header::DOCUMENT_HEADER_H,
        );
        assert!(
            narrow_visible.inspector.pos.y
                >= narrow_visible.header.pos.y + narrow_visible.header.size.y
        );

        let (mut cx, mut app) = mounted_production_shell();
        configure_mounted_dock(
            &mut cx,
            &mut app,
            narrow_size,
            DockState::Flag,
            DockState::Pinned,
            false,
        );
        let narrow_absent = draw_mounted_dock(&mut cx, &app, narrow_size);
        assert_near(narrow_absent.inspector.pos.y, narrow_absent.body.pos.y);
        assert_near(narrow_absent.center.pos.y, narrow_absent.body.pos.y);

        let (mut cx, mut app) = mounted_production_shell();
        configure_mounted_dock(
            &mut cx,
            &mut app,
            narrow_size,
            DockState::Flag,
            DockState::Flag,
            true,
        );
        draw_mounted_dock(&mut cx, &app, narrow_size);
        assert!(!drawn_header_right_dock_active(&mut cx, &app));
    }

    #[test]
    fn mounted_history_buttons_occupy_the_fixed_leading_strip() {
        let size = dvec2(600.0, 30.0);
        let (mut cx, mut app) = mounted_production_shell();
        configure_mounted_dock(
            &mut cx,
            &mut app,
            size,
            DockState::Flag,
            DockState::Flag,
            true,
        );
        app.ui
            .widget(&cx, ids!(document_header))
            .borrow_mut::<crate::document_header::DocumentHeader>()
            .expect("production shell mounts document_header")
            .set_history_visible(&mut cx, true);

        draw_document_header(&mut cx, &app, size);

        let back = app
            .ui
            .widget(&cx, ids!(document_header.back_button))
            .area()
            .rect(&cx);
        let forward = app
            .ui
            .widget(&cx, ids!(document_header.forward_button))
            .area()
            .rect(&cx);
        assert_eq!(back.size.x, crate::document_header::DOCUMENT_HEADER_H);
        assert_eq!(forward.size.x, crate::document_header::DOCUMENT_HEADER_H);
        assert_eq!(
            forward.pos.x,
            back.pos.x + crate::document_header::DOCUMENT_HEADER_H
        );
    }

    fn project_tree_folder_is_open(cx: &mut Cx, app: &App, address: &str) -> bool {
        let project_tree = app.ui.widget(cx, ids!(project_tree));
        let file_tree = project_tree.file_tree(cx, ids!(file_tree));
        let draw_event = DrawEvent::default();
        let mut draw_cx = CxDraw::new(cx, &draw_event);
        let mut cx_2d = Cx2d::new(&mut draw_cx);
        cx_2d.begin_root_turtle(dvec2(0.0, 0.0), Layout::default());
        let mut file_tree = file_tree
            .borrow_mut()
            .expect("mounted ProjectTree has a FileTree");
        let is_open = file_tree
            .begin_folder(&mut cx_2d, LiveId::from_str(address), address)
            .is_ok();
        if is_open {
            file_tree.end_folder();
        }
        drop(file_tree);
        cx_2d.end_turtle();
        is_open
    }

    #[test]
    fn navigation_external_target_invokes_only_the_browser_adapter_once() {
        let (mut cx, mut app) = navigation_app();
        let mut browser = FakeBrowser::default();

        assert!(app.navigate_with(
            &mut cx,
            NavigationTarget::ExternalUrl("https://example.com/docs".into()),
            OpenDisposition::Preview,
            &mut browser,
        ));
        assert_eq!(browser.opened, vec!["https://example.com/docs"]);
        assert!(app.documents.tabs().is_empty());
        assert_eq!(app.nav_state, NavState::default());
    }

    #[test]
    fn navigation_browser_failure_preserves_document_and_directory_state() {
        let (mut cx, mut app) = navigation_app();
        let mut browser = FakeBrowser::default();
        assert!(app.navigate_with(
            &mut cx,
            NavigationTarget::Document {
                concept_id: "sales/order".into(),
                fragment: None,
            },
            OpenDisposition::Persistent,
            &mut browser,
        ));
        app.nav_state.scope = "/sales".into();
        let active = app.documents.active_id();
        let nav_state = app.nav_state.clone();
        browser.error = Some("blocked".into());

        assert!(!app.navigate_with(
            &mut cx,
            NavigationTarget::ExternalUrl("https://example.com/blocked".into()),
            OpenDisposition::Preview,
            &mut browser,
        ));
        assert_eq!(browser.opened, vec!["https://example.com/blocked"]);
        assert_eq!(app.documents.active_id(), active);
        assert_eq!(app.nav_state, nav_state);
        let statusbar = app.ui.widget(&cx, ids!(statusbar));
        let statusbar = statusbar
            .borrow::<crate::statusbar::Statusbar>()
            .expect("test statusbar is mounted");
        assert_eq!(
            crate::statusbar::navigation_message(&statusbar),
            Some("Could not open link: blocked")
        );
        drop(statusbar);
        app.ui
            .widget(&cx, ids!(statusbar))
            .borrow_mut::<crate::statusbar::Statusbar>()
            .expect("test statusbar is mounted")
            .set_save_error(&mut cx, Some("disk full"));
        browser.error = None;
        assert!(app.navigate_with(
            &mut cx,
            NavigationTarget::ExternalUrl("https://example.com/retry".into()),
            OpenDisposition::Preview,
            &mut browser,
        ));
        let statusbar = app.ui.widget(&cx, ids!(statusbar));
        let statusbar = statusbar
            .borrow::<crate::statusbar::Statusbar>()
            .expect("test statusbar is mounted");
        assert_eq!(crate::statusbar::navigation_message(&statusbar), None);
        assert_eq!(crate::statusbar::save_error(&statusbar), Some("disk full"));
    }

    #[test]
    fn navigation_document_preview_persistence_and_repeat_activation_are_stable() {
        let (mut cx, mut app) = navigation_app();
        let mut browser = FakeBrowser::default();
        let order = NavigationTarget::Document {
            concept_id: "sales/order".into(),
            fragment: None,
        };

        assert!(app.navigate_with(
            &mut cx,
            order.clone(),
            OpenDisposition::Preview,
            &mut browser,
        ));
        assert_eq!(app.documents.tabs().len(), 1);
        assert!(app.documents.tabs()[0].preview);

        assert!(app.navigate_with(
            &mut cx,
            order.clone(),
            OpenDisposition::Persistent,
            &mut browser,
        ));
        assert_eq!(app.documents.tabs().len(), 1);
        assert!(!app.documents.tabs()[0].preview);

        assert!(app.navigate_with(&mut cx, order, OpenDisposition::Persistent, &mut browser,));
        assert_eq!(app.documents.tabs().len(), 1);
        assert!(!app.documents.tabs()[0].preview);
    }

    #[test]
    fn navigation_markdown_resolves_only_at_the_app_boundary() {
        let (mut cx, mut app) = navigation_app();

        assert!(app.handle_navigation_intent(
            &mut cx,
            NavigationIntent::MarkdownLink {
                current_concept_id: "sales/order".into(),
                href: "./customer.md".into(),
            },
        ));
        assert_eq!(
            app.documents
                .active_tab()
                .map(|tab| tab.concept_id.as_str()),
            Some("sales/customer")
        );
        assert!(app.documents.tabs()[0].preview);
    }

    fn resolved_target(intent: &NavigationIntent) -> Option<&NavigationTarget> {
        match intent {
            NavigationIntent::Resolved { target, .. } => Some(target),
            NavigationIntent::MarkdownLink { .. } => None,
        }
    }

    fn navigation_app_with_active_order() -> (Cx, App) {
        let (mut cx, mut app) = navigation_app();
        mount_markdown_surface(&mut cx, &mut app);
        assert!(app.transition_to_location(
            &mut cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::primary("sales/order"),
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        ));
        assert_eq!(
            app.documents
                .active_tab()
                .map(|tab| (tab.concept_id.as_str(), tab.preview)),
            Some(("sales/order", true))
        );
        assert!(
            app.ui
                .widget(&cx, ids!(markdown_surface.md))
                .text()
                .contains("# Order"),
            "the mounted Markdown ingress belongs to the active order document"
        );
        (cx, app)
    }

    #[test]
    fn manual_and_preview_transitions_follow_back_and_forward_history() {
        let (mut cx, mut app) = navigation_app_with_active_order();
        assert!(app.transition_to_location(
            &mut cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::primary("sales/customer"),
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        ));
        assert_eq!(app.documents.tabs().len(), 1);
        assert!(app.documents.tabs()[0].preview);
        assert_eq!(app.view_history.len(), 2);

        assert!(app.traverse_view_history(&mut cx, HistoryDirection::Back));
        assert_eq!(
            app.documents.active_tab().unwrap().concept_id,
            "sales/order"
        );
        assert_eq!(app.documents.tabs().len(), 1);
        assert!(app.documents.tabs()[0].preview);

        assert!(app.traverse_view_history(&mut cx, HistoryDirection::Forward));
        assert_eq!(
            app.documents.active_tab().unwrap().concept_id,
            "sales/customer"
        );
        assert_eq!(app.view_history.len(), 2);
    }

    #[test]
    fn header_history_actions_traverse_once_and_report_unavailable_targets() {
        let (mut cx, mut app) = navigation_app_with_active_order();
        assert!(app.transition_document(&mut cx, "sales/customer", false));
        {
            let header = app.ui.widget(&cx, ids!(document_header));
            let header = header
                .borrow::<crate::document_header::DocumentHeader>()
                .expect("test header is mounted");
            assert_eq!(header.test_history_enabled(), (true, false));
        }

        let back_button_uid = app
            .ui
            .widget(&cx, ids!(document_header.back_button))
            .widget_uid();
        app.handle_action_batch(
            &mut cx,
            &[widget_action(
                back_button_uid,
                crate::icon_button::IconButtonAction::TaggedClicked(live_id!(history_back)),
            )],
        );
        assert_eq!(
            app.documents
                .active_tab()
                .map(|tab| tab.concept_id.as_str()),
            Some("sales/order")
        );
        {
            let header = app.ui.widget(&cx, ids!(document_header));
            let header = header
                .borrow::<crate::document_header::DocumentHeader>()
                .expect("test header is mounted");
            assert_eq!(header.test_history_enabled(), (false, true));
        }

        let forward_button_uid = app
            .ui
            .widget(&cx, ids!(document_header.forward_button))
            .widget_uid();
        app.handle_action_batch(
            &mut cx,
            &[widget_action(
                forward_button_uid,
                crate::icon_button::IconButtonAction::TaggedClicked(live_id!(history_forward)),
            )],
        );
        assert_eq!(
            app.documents
                .active_tab()
                .map(|tab| tab.concept_id.as_str()),
            Some("sales/customer")
        );

        app.handle_action_batch(
            &mut cx,
            &[widget_action(
                back_button_uid,
                crate::icon_button::IconButtonAction::TaggedClicked(live_id!(history_back)),
            )],
        );
        app.handle_action_batch(
            &mut cx,
            &[widget_action(
                back_button_uid,
                crate::icon_button::IconButtonAction::TaggedClicked(live_id!(history_back)),
            )],
        );
        let statusbar = app.ui.widget(&cx, ids!(statusbar));
        let statusbar = statusbar
            .borrow::<crate::statusbar::Statusbar>()
            .expect("test statusbar is mounted");
        assert_eq!(
            crate::statusbar::history_feedback(&statusbar),
            (Some("No previous view"), None)
        );
    }

    #[test]
    fn back_then_manual_navigation_clears_forward() {
        let (mut cx, mut app) = navigation_app_with_active_order();
        for concept_id in ["sales/customer", "sales/next"] {
            assert!(app.transition_to_location(
                &mut cx,
                ViewLocation {
                    document: crate::navigation::DocumentLocator::primary(concept_id),
                    anchor: ViewAnchor::None,
                },
                TransitionCause::UserNavigation,
            ));
        }
        assert!(app.traverse_view_history(&mut cx, HistoryDirection::Back));
        assert_eq!(
            app.documents.active_tab().unwrap().concept_id,
            "sales/customer"
        );

        assert!(app.transition_to_location(
            &mut cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::primary("sales/order"),
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        ));

        assert!(!app
            .view_history
            .can_traverse(HistoryDirection::Forward, |_| true));
    }

    #[test]
    fn repeat_current_user_navigation_preserves_the_active_anchor() {
        let anchor = ViewAnchor::Diagram {
            selected_key: Some("sales/customer".into()),
            camera: crate::view_history::DiagramCameraAnchor {
                pan_x: 12.0,
                pan_y: 34.0,
                zoom: 1.5,
            },
        };
        let (mut cx, mut app, state) = navigation_app_with_anchor_probe(anchor.clone());

        assert!(app.transition_to_location(
            &mut cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::primary("sales/order"),
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        ));

        assert_eq!(*state.borrow(), anchor);
    }

    #[test]
    fn same_document_undo_reveal_records_the_departing_anchor_for_back() {
        let departing = ViewAnchor::Diagram {
            selected_key: Some("sales/customer".into()),
            camera: crate::view_history::DiagramCameraAnchor {
                pan_x: 12.0,
                pan_y: 34.0,
                zoom: 1.5,
            },
        };
        let (mut cx, mut app, _) = navigation_app_with_anchor_probe(departing.clone());

        assert!(app.transition_to_location(
            &mut cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::primary("sales/order"),
                anchor: ViewAnchor::Diagram {
                    selected_key: None,
                    camera: crate::view_history::DiagramCameraAnchor {
                        pan_x: 40.0,
                        pan_y: 50.0,
                        zoom: 2.0,
                    },
                },
            },
            TransitionCause::UndoRedoReveal,
        ));

        let back = app
            .view_history
            .target(HistoryDirection::Back, |_| true)
            .expect("Undo reveal must create a Back entry even within one document");
        assert_eq!(back.location.anchor, departing);
    }

    #[test]
    fn active_close_records_fallback_but_promote_and_inactive_close_do_not() {
        let (mut cx, mut app) = navigation_app_with_active_order();
        let order_id = app.documents.active_id();
        app.documents.transition(
            &mut cx,
            &app.ui,
            &app.session,
            DocumentCommand::Promote(order_id),
        );
        let history_after_promote = app.view_history.len();
        assert!(app.transition_to_location(
            &mut cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::primary("sales/customer"),
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        ));
        let customer_id = app.documents.active_id();
        assert_eq!(app.documents.tabs().len(), 2);
        let history_before_inactive_close = app.view_history.len();

        assert!(app.close_document(&mut cx, order_id));
        assert_eq!(app.view_history.len(), history_before_inactive_close);
        assert_eq!(app.documents.active_id(), customer_id);

        app.documents.transition(
            &mut cx,
            &app.ui,
            &app.session,
            DocumentCommand::Promote(customer_id),
        );
        assert!(app.transition_to_location(
            &mut cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::primary("sales/next"),
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        ));
        let before_active_close = app.view_history.len();
        let next_id = app.documents.active_id();
        assert!(app.close_document(&mut cx, next_id));
        assert_eq!(app.view_history.len(), before_active_close + 1);
        assert_eq!(
            app.documents.active_tab().unwrap().concept_id,
            "sales/customer"
        );
        assert!(history_after_promote > 0);
    }

    #[test]
    fn undo_reveals_the_document_where_the_edit_started_and_records_the_move() {
        let (mut cx, mut app) = navigation_app_with_active_order();
        let order = ViewLocation {
            document: crate::navigation::DocumentLocator::primary("sales/order"),
            anchor: ViewAnchor::None,
        };
        let customer = ViewLocation {
            document: crate::navigation::DocumentLocator::primary("sales/customer"),
            anchor: ViewAnchor::None,
        };
        app.session
            .apply_edit(crate::editor_session::EditRequest {
                before_location: order.clone(),
                intent: crate::document::EditIntent {
                    edit: waml::edit::PendingEdit::new(waml::okf::Batch(vec![
                        waml::okf::Op::IndexRetitle {
                            directory: waml::okf::DirectoryAddress::parse("/sales").unwrap(),
                            title: "Commerce".into(),
                        },
                    ])),
                    label: "Rename sales".into(),
                    merge_key: None,
                    after_location: Some(customer),
                },
            })
            .unwrap();
        assert!(app.transition_to_location(
            &mut cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::primary("sales/next"),
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        ));

        assert!(app.perform_undo(&mut cx));

        assert_eq!(
            app.documents.active_tab().unwrap().concept_id,
            "sales/order"
        );
        assert!(app
            .view_history
            .can_traverse(HistoryDirection::Back, |_| true));
        assert!(app.traverse_view_history(&mut cx, HistoryDirection::Back));
        assert_eq!(
            app.documents.active_tab().unwrap().concept_id,
            "sales/next",
            "Back after an Undo reveal returns to the editor that was active"
        );
        let statusbar = app.ui.widget(&cx, ids!(statusbar));
        let statusbar = statusbar
            .borrow::<crate::statusbar::Statusbar>()
            .expect("test statusbar is mounted");
        assert_eq!(
            crate::statusbar::history_feedback(&statusbar),
            (None, Some("Undid: Rename sales"))
        );
    }

    #[test]
    fn global_history_chord_dispatches_before_the_widget_tree_and_consumes_empty_stack() {
        let (mut cx, mut app) = navigation_app_with_active_order();
        app.session
            .apply_edit(crate::editor_session::EditRequest {
                before_location: ViewLocation {
                    document: crate::navigation::DocumentLocator::primary("sales/order"),
                    anchor: ViewAnchor::None,
                },
                intent: crate::document::EditIntent {
                    edit: waml::edit::PendingEdit::new(waml::okf::Batch(vec![
                        waml::okf::Op::IndexRetitle {
                            directory: waml::okf::DirectoryAddress::parse("/sales").unwrap(),
                            title: "Commerce".into(),
                        },
                    ])),
                    label: "Rename sales".into(),
                    merge_key: None,
                    after_location: None,
                },
            })
            .unwrap();
        let undo = Event::KeyDown(KeyEvent {
            key_code: KeyCode::KeyZ,
            modifiers: KeyModifiers {
                control: true,
                ..Default::default()
            },
            ..Default::default()
        });

        app.handle_event(&mut cx, &undo);
        {
            let statusbar = app.ui.widget(&cx, ids!(statusbar));
            let statusbar = statusbar
                .borrow::<crate::statusbar::Statusbar>()
                .expect("test statusbar is mounted");
            assert_eq!(
                crate::statusbar::history_feedback(&statusbar),
                (None, Some("Undid: Rename sales"))
            );
        }

        app.handle_event(&mut cx, &undo);
        let statusbar = app.ui.widget(&cx, ids!(statusbar));
        let statusbar = statusbar
            .borrow::<crate::statusbar::Statusbar>()
            .expect("test statusbar is mounted");
        assert_eq!(
            crate::statusbar::history_feedback(&statusbar),
            (Some("Nothing to undo"), None)
        );
    }

    #[test]
    fn navigation_document_ingresses_share_target_and_preview_command() {
        let target = NavigationTarget::Document {
            concept_id: "sales/customer".into(),
            fragment: None,
        };
        let tree_intent = NavigationIntent::Resolved {
            target: target.clone(),
            disposition: OpenDisposition::Preview,
        };
        let (breadcrumb_intent, markdown_resolved_intent) = {
            let (_cx, fixture_app) = navigation_app();
            let breadcrumb_target = crate::navigation::breadcrumb_for(
                fixture_app.session.okf_analysis(),
                fixture_app.session.uml_analysis(),
                "sales/customer",
            )
            .expect("customer has a canonical breadcrumb")
            .into_iter()
            .last()
            .expect("breadcrumb ends at the document")
            .target;
            (
                NavigationIntent::Resolved {
                    target: breadcrumb_target,
                    disposition: OpenDisposition::Preview,
                },
                NavigationIntent::Resolved {
                    target: crate::navigation::resolve_link(
                        fixture_app.session.okf(),
                        "sales/order",
                        "./customer.md",
                    )
                    .expect("relative customer link resolves"),
                    disposition: OpenDisposition::Preview,
                },
            )
        };

        assert_eq!(
            resolved_target(&tree_intent),
            resolved_target(&breadcrumb_intent)
        );
        assert_eq!(
            resolved_target(&tree_intent),
            resolved_target(&markdown_resolved_intent)
        );

        enum Ingress {
            Tree,
            Header,
            Markdown,
        }
        for ingress in [Ingress::Tree, Ingress::Header, Ingress::Markdown] {
            let (mut cx, mut app) = navigation_app_with_active_order();
            let order_id = app.documents.active_id();
            let action = match ingress {
                Ingress::Tree => widget_action(
                    app.ui.widget(&cx, ids!(project_tree)).widget_uid(),
                    crate::tree_panel::ProjectTreeAction::Navigate(tree_intent.clone()),
                ),
                Ingress::Header => widget_action(
                    app.ui.widget(&cx, ids!(document_header)).widget_uid(),
                    crate::document_header::DocumentHeaderAction::Navigate(target.clone()),
                ),
                Ingress::Markdown => widget_action(
                    app.ui.widget(&cx, ids!(markdown_surface.md)).widget_uid(),
                    MarkdownAction::LinkNavigated("./customer.md".into()),
                ),
            };

            app.handle_action_batch(&mut cx, &[action]);
            assert_ne!(app.documents.active_id(), order_id);
            assert_eq!(
                app.documents
                    .active_tab()
                    .map(|tab| tab.concept_id.as_str()),
                Some("sales/customer")
            );
            assert_eq!(app.documents.tabs().len(), 1);
            assert!(
                app.documents.tabs()[0].preview,
                "all ordinary navigation ingresses must use preview disposition"
            );
            assert!(
                app.ui
                    .widget(&cx, ids!(markdown_surface.md))
                    .text()
                    .contains("# Customer"),
                "each ingress must update the mounted Preview body"
            );
            assert_eq!(app.view_history.len(), 2);
            assert!(app.traverse_view_history(&mut cx, HistoryDirection::Back));
            assert_eq!(
                app.documents.active_tab().unwrap().concept_id,
                "sales/order"
            );
        }

        let (mut cx, mut app) = navigation_app_with_active_order();
        let persistent_tree = NavigationIntent::Resolved {
            target,
            disposition: OpenDisposition::Persistent,
        };
        let project_tree_uid = app.ui.widget(&cx, ids!(project_tree)).widget_uid();
        let persistent_action = || {
            widget_action(
                project_tree_uid,
                crate::tree_panel::ProjectTreeAction::Navigate(persistent_tree.clone()),
            )
        };
        app.handle_action_batch(&mut cx, &[persistent_action()]);
        assert_eq!(app.documents.tabs().len(), 1);
        assert!(!app.documents.tabs()[0].preview);
        let history_after_first = app.view_history.len();
        app.handle_action_batch(&mut cx, &[persistent_action()]);
        assert_eq!(app.documents.tabs().len(), 1);
        assert!(!app.documents.tabs()[0].preview);
        assert_eq!(app.view_history.len(), history_after_first);
    }

    #[test]
    fn navigation_markdown_failures_preserve_document_and_report_exact_status() {
        let (mut cx, mut app) = navigation_app();
        let mut browser = FakeBrowser::default();
        assert!(app.navigate_with(
            &mut cx,
            NavigationTarget::Document {
                concept_id: "sales/order".into(),
                fragment: None,
            },
            OpenDisposition::Persistent,
            &mut browser,
        ));
        let active = app.documents.active_id();
        let cases = [
            ("http://", "Invalid link: http://"),
            ("mailto:a@example.com", "Unsupported link scheme: mailto"),
            ("../../../escape.md", "Link leaves this bundle"),
            ("./missing.md", "Document not found: sales/missing"),
        ];

        for (href, expected) in cases {
            assert!(!app.handle_navigation_intent(
                &mut cx,
                NavigationIntent::MarkdownLink {
                    current_concept_id: "sales/order".into(),
                    href: href.into(),
                },
            ));
            assert_eq!(app.documents.active_id(), active);
            let statusbar = app.ui.widget(&cx, ids!(statusbar));
            let statusbar = statusbar
                .borrow::<crate::statusbar::Statusbar>()
                .expect("test statusbar is mounted");
            assert_eq!(
                crate::statusbar::navigation_message(&statusbar),
                Some(expected),
                "{href}"
            );
        }
    }

    #[test]
    fn navigation_root_restores_scope_and_clears_query_and_filter() {
        let (mut cx, mut app) = navigation_app();
        let mut browser = FakeBrowser::default();
        app.nav_state = NavState {
            scope: "/sales".into(),
            query: "order".into(),
            filter: Some(TreeKind::Class),
        };

        assert!(app.navigate_with(
            &mut cx,
            NavigationTarget::Directory {
                address: "/".into(),
            },
            OpenDisposition::Preview,
            &mut browser,
        ));
        assert_eq!(app.nav_state, NavState::default());
        let project_tree = app.ui.widget(&cx, ids!(project_tree));
        let project_tree = project_tree
            .borrow::<crate::tree_panel::ProjectTree>()
            .expect("test project tree is mounted");
        assert_eq!(project_tree.dock_state(), DockState::Pinned);
    }

    #[test]
    fn navigation_root_uses_narrow_mutual_exclusion_and_preserves_wide_inspector() {
        for (narrow, expected_inspector) in [(true, DockState::Flag), (false, DockState::Pinned)] {
            let (mut cx, mut app) = navigation_app();
            let mut browser = FakeBrowser::default();
            app.narrow = narrow;
            app.ui
                .widget(&cx, ids!(project_tree))
                .borrow_mut::<crate::tree_panel::ProjectTree>()
                .expect("test project tree is mounted")
                .close_dock(&mut cx);
            app.ui
                .widget(&cx, ids!(inspector))
                .borrow_mut::<crate::inspector_panel::Inspector>()
                .expect("test inspector is mounted")
                .open_dock(&mut cx);

            assert!(app.navigate_with(
                &mut cx,
                NavigationTarget::Directory {
                    address: "/".into(),
                },
                OpenDisposition::Preview,
                &mut browser,
            ));

            assert_eq!(
                app.dock_states(&mut cx),
                (DockState::Pinned, expected_inspector)
            );
        }
    }

    #[test]
    fn navigation_directory_intents_share_one_app_owned_toggle_path() {
        enum Ingress {
            Tree,
            Header,
            Markdown,
        }

        for ingress in [Ingress::Tree, Ingress::Header, Ingress::Markdown] {
            let (mut cx, mut app) = navigation_app();
            mount_markdown_surface(&mut cx, &mut app);
            let mut browser = FakeBrowser::default();
            assert!(app.navigate_with(
                &mut cx,
                NavigationTarget::Document {
                    concept_id: "sales/order".into(),
                    fragment: None,
                },
                OpenDisposition::Persistent,
                &mut browser,
            ));
            app.nav_state.scope = "/sales".into();
            let active = app.documents.active_id();
            let markdown = app.ui.widget(&cx, ids!(markdown_surface.md));
            assert!(
                markdown.borrow::<Markdown>().is_some(),
                "Markdown ingress must originate from the mounted renderer"
            );
            assert!(
                markdown.text().contains("# Order"),
                "the mounted renderer must belong to the active document"
            );
            let markdown_uid = markdown.widget_uid();
            assert!(
                project_tree_folder_is_open(&mut cx, &app, "/sales"),
                "the fresh Browse tree starts with its top-level folder open"
            );
            let action = match ingress {
                Ingress::Tree => {
                    let uid = app.ui.widget(&cx, ids!(project_tree)).widget_uid();
                    widget_action(
                        uid,
                        crate::tree_panel::ProjectTreeAction::Navigate(
                            NavigationIntent::Resolved {
                                target: NavigationTarget::Directory {
                                    address: "/sales".into(),
                                },
                                disposition: OpenDisposition::Preview,
                            },
                        ),
                    )
                }
                Ingress::Header => {
                    let uid = app.ui.widget(&cx, ids!(document_header)).widget_uid();
                    widget_action(
                        uid,
                        crate::document_header::DocumentHeaderAction::Navigate(
                            NavigationTarget::Directory {
                                address: "/sales".into(),
                            },
                        ),
                    )
                }
                Ingress::Markdown => {
                    widget_action(markdown_uid, MarkdownAction::LinkNavigated("./".into()))
                }
            };
            let actions: ActionsBuf = vec![action];
            app.handle_action_batch(&mut cx, &actions);

            assert!(
                !project_tree_folder_is_open(&mut cx, &app, "/sales"),
                "each ingress must close the initially-open folder exactly once"
            );
            assert_eq!(app.documents.active_id(), active);
            assert_eq!(app.nav_state.scope, "/sales");
        }
    }

    #[test]
    fn navigation_draw_hook_scrolls_recorded_fragment_after_target_draw() {
        let (mut cx, mut app) = navigation_app();
        mount_markdown_surface(&mut cx, &mut app);
        let mut browser = FakeBrowser::default();

        assert!(app.navigate_with(
            &mut cx,
            NavigationTarget::Document {
                concept_id: "sales/customer".into(),
                fragment: Some("history".into()),
            },
            OpenDisposition::Preview,
            &mut browser,
        ));
        assert!(
            app.ui
                .widget(&cx, ids!(markdown_surface.md))
                .text()
                .contains("## History"),
            "the active document must reach the mounted renderer before draw"
        );
        assert_eq!(
            app.pending_fragment,
            Some(PendingFragment {
                concept_id: "sales/customer".into(),
                fragment: "history".into(),
            })
        );
        record_markdown_anchors(&mut cx, &app);

        AppMain::handle_event(
            &mut app,
            &mut cx,
            &Event::Draw(DrawEvent {
                redraw_all: true,
                ..DrawEvent::default()
            }),
        );

        assert!(
            !app.ui
                .widget(&cx, ids!(markdown_surface.md))
                .area()
                .is_empty(),
            "the real renderer draw must record a mounted area"
        );
        assert_eq!(app.pending_fragment, None);
        let statusbar = app.ui.widget(&cx, ids!(statusbar));
        let statusbar = statusbar
            .borrow::<crate::statusbar::Statusbar>()
            .expect("test statusbar is mounted");
        assert_eq!(crate::statusbar::navigation_message(&statusbar), None);
    }

    #[test]
    fn navigation_draw_hook_keeps_mismatch_then_reports_missing_once() {
        let (mut cx, mut app) = navigation_app();
        mount_markdown_surface(&mut cx, &mut app);
        let mut browser = FakeBrowser::default();
        assert!(app.navigate_with(
            &mut cx,
            NavigationTarget::Document {
                concept_id: "sales/order".into(),
                fragment: None,
            },
            OpenDisposition::Preview,
            &mut browser,
        ));
        app.pending_fragment = Some(PendingFragment {
            concept_id: "sales/customer".into(),
            fragment: "missing".into(),
        });

        AppMain::handle_event(
            &mut app,
            &mut cx,
            &Event::Draw(DrawEvent {
                redraw_all: true,
                ..DrawEvent::default()
            }),
        );

        assert_eq!(
            app.pending_fragment,
            Some(PendingFragment {
                concept_id: "sales/customer".into(),
                fragment: "missing".into(),
            })
        );

        assert!(app.navigate_with(
            &mut cx,
            NavigationTarget::Document {
                concept_id: "sales/customer".into(),
                fragment: Some("missing".into()),
            },
            OpenDisposition::Preview,
            &mut browser,
        ));
        record_markdown_anchors(&mut cx, &app);
        AppMain::handle_event(
            &mut app,
            &mut cx,
            &Event::Draw(DrawEvent {
                redraw_all: true,
                ..DrawEvent::default()
            }),
        );
        assert_eq!(app.pending_fragment, None);
        let statusbar = app.ui.widget(&cx, ids!(statusbar));
        let mut statusbar = statusbar
            .borrow_mut::<crate::statusbar::Statusbar>()
            .expect("test statusbar is mounted");
        assert_eq!(
            crate::statusbar::navigation_message(&statusbar),
            Some("Section not found: missing")
        );
        statusbar.set_navigation_message(&mut cx, None);
        drop(statusbar);

        AppMain::handle_event(
            &mut app,
            &mut cx,
            &Event::Draw(DrawEvent {
                redraw_all: true,
                ..DrawEvent::default()
            }),
        );
        let statusbar = app.ui.widget(&cx, ids!(statusbar));
        let statusbar = statusbar
            .borrow::<crate::statusbar::Statusbar>()
            .expect("test statusbar is mounted");
        assert_eq!(crate::statusbar::navigation_message(&statusbar), None);
    }

    #[test]
    fn non_markdown_active_view_rejects_hidden_stale_fragment_once() {
        struct NonMarkdownView;

        impl DocView for NonMarkdownView {
            fn sync(
                &mut self,
                cx: &mut Cx,
                body: &BodyWidgets,
                _data: crate::doc_view::ViewData<'_>,
            ) {
                body.show_canvas(cx);
            }

            fn handle(
                &mut self,
                _cx: &mut Cx,
                _body: &BodyWidgets,
                _actions: &Actions,
                _data: crate::doc_view::ViewData<'_>,
            ) -> crate::doc_view::ViewOutcome {
                crate::doc_view::ViewOutcome::default()
            }

            fn chrome(&self) -> crate::doc_view::BodyChrome {
                crate::doc_view::BodyChrome::HIDDEN
            }
        }

        let (mut cx, mut app) = navigation_app();
        mount_markdown_surface(&mut cx, &mut app);
        crate::markdown_surface::set_markdown(&app.ui, &mut cx, "# Details\n");
        record_markdown_anchors(&mut cx, &app);

        let tab_id = LiveId::from_str("diagram");
        app.documents.transition(
            &mut cx,
            &app.ui,
            &app.session,
            DocumentCommand::Open {
                document: crate::document::OpenDocument {
                    tab_id,
                    concept_id: "diagram".into(),
                    kind: crate::view_history::DocumentKind::Primary,
                    title: "Diagram".into(),
                    presentation: DocumentPresentation {
                        icon: Icon::Workflow,
                        accent: None,
                        category: crate::document::NavCategory::Diagram,
                    },
                    view: Box::new(NonMarkdownView),
                },
                persistent: true,
            },
        );
        let active_before = app.documents.active_id();
        app.pending_fragment = Some(PendingFragment {
            concept_id: "diagram".into(),
            fragment: "details".into(),
        });

        app.apply_pending_fragment(&mut cx);

        assert_eq!(app.pending_fragment, None);
        assert_eq!(app.documents.active_id(), active_before);
        let statusbar = app.ui.widget(&cx, ids!(statusbar));
        let mut statusbar = statusbar
            .borrow_mut::<crate::statusbar::Statusbar>()
            .expect("test statusbar is mounted");
        assert_eq!(
            crate::statusbar::navigation_message(&statusbar),
            Some("Section not found: details")
        );
        statusbar.set_navigation_message(&mut cx, None);
        drop(statusbar);

        app.apply_pending_fragment(&mut cx);

        let statusbar = app.ui.widget(&cx, ids!(statusbar));
        let statusbar = statusbar
            .borrow::<crate::statusbar::Statusbar>()
            .expect("test statusbar is mounted");
        assert_eq!(crate::statusbar::navigation_message(&statusbar), None);
        assert_eq!(app.documents.active_id(), active_before);
    }

    #[test]
    fn navigation_source_and_generic_views_activate_and_scroll_real_renderer() {
        #[derive(Clone, Copy)]
        enum ViewKind {
            Source,
            Generic,
        }

        for view_kind in [ViewKind::Source, ViewKind::Generic] {
            for (fragment, expected_status) in [
                ("details", None),
                ("missing", Some("Section not found: missing")),
            ] {
                let (mut cx, mut app) = navigation_app();
                mount_markdown_surface(&mut cx, &mut app);
                let markdown = app.ui.widget(&cx, ids!(markdown_surface.md));
                let markdown_uid = markdown.widget_uid();
                let intent = {
                    let body = BodyWidgets::new(&mut cx, &app.ui);
                    let mut view: Box<dyn DocView> = match view_kind {
                        ViewKind::Source => {
                            Box::new(crate::source_view::SourceView::new("sales/order".into()))
                        }
                        ViewKind::Generic => Box::new(
                            crate::generic_okf_view::GenericOkfView::new("sales/order".into()),
                        ),
                    };
                    let data = ViewData {
                        source: app.session.source(),
                        okf_analysis: app.session.okf_analysis(),
                        uml_analysis: app.session.uml_analysis(),
                        revision: app.session.revision(),
                    };
                    view.sync(&mut cx, &body, data);
                    assert!(
                        markdown.text().contains("# Order"),
                        "each view must populate the mounted shared renderer"
                    );
                    let href = format!("./next.md#{fragment}");
                    let actions: ActionsBuf = vec![widget_action(
                        markdown_uid,
                        MarkdownAction::LinkNavigated(href.clone()),
                    )];
                    let outcome = view.handle(&mut cx, &body, &actions, data);
                    assert_eq!(
                        outcome.navigation,
                        Some(NavigationIntent::MarkdownLink {
                            current_concept_id: "sales/order".into(),
                            href,
                        })
                    );
                    outcome.navigation.expect("view emits navigation")
                };

                assert!(app.handle_navigation_intent(&mut cx, intent));
                assert_eq!(
                    app.documents
                        .active_tab()
                        .map(|tab| tab.concept_id.as_str()),
                    Some("sales/next")
                );
                assert_eq!(
                    app.pending_fragment,
                    Some(PendingFragment {
                        concept_id: "sales/next".into(),
                        fragment: fragment.into(),
                    })
                );

                record_markdown_anchors(&mut cx, &app);
                AppMain::handle_event(
                    &mut app,
                    &mut cx,
                    &Event::Draw(DrawEvent {
                        redraw_all: true,
                        ..DrawEvent::default()
                    }),
                );

                assert_eq!(app.pending_fragment, None);
                assert_eq!(
                    app.documents
                        .active_tab()
                        .map(|tab| tab.concept_id.as_str()),
                    Some("sales/next"),
                    "missing anchors must preserve the newly activated target"
                );
                let statusbar = app.ui.widget(&cx, ids!(statusbar));
                let statusbar = statusbar
                    .borrow::<crate::statusbar::Statusbar>()
                    .expect("test statusbar is mounted");
                assert_eq!(
                    crate::statusbar::navigation_message(&statusbar),
                    expected_status,
                    "{fragment}"
                );
            }
        }
    }

    fn tab(id: LiveId, key: &str, title: &str, category: TreeKind, preview: bool) -> DocTab {
        DocTab {
            id,
            concept_id: key.into(),
            kind: crate::view_history::DocumentKind::Primary,
            title: title.into(),
            presentation: DocumentPresentation {
                icon: IconSet::icon_for(category).unwrap(),
                accent: None,
                category,
            },
            preview,
        }
    }

    #[test]
    fn document_header_projection_keeps_icon_when_breadcrumb_is_missing() {
        let chrome = DocumentHeaderChrome {
            breadcrumb: true,
            right_dock: Some(Icon::SlidersHorizontal),
        };
        let (segments, icon) = project_document_header(chrome, None);

        assert!(segments.is_empty());
        assert_eq!(icon, Some(Icon::SlidersHorizontal));
    }

    #[test]
    fn document_header_projection_obeys_breadcrumb_flag_and_hidden_chrome() {
        let segment = BreadcrumbSegment {
            title: "Order".into(),
            target: NavigationTarget::Document {
                concept_id: "sales/order".into(),
                fragment: None,
            },
        };
        let icon_only = DocumentHeaderChrome {
            breadcrumb: false,
            right_dock: Some(Icon::SlidersHorizontal),
        };
        assert_eq!(
            project_document_header(icon_only, Some(vec![segment.clone()])),
            (Vec::new(), Some(Icon::SlidersHorizontal))
        );

        let breadcrumb = DocumentHeaderChrome {
            breadcrumb: true,
            right_dock: None,
        };
        assert_eq!(
            project_document_header(breadcrumb, Some(vec![segment.clone()])),
            (vec![segment], None)
        );
        assert_eq!(
            project_document_header(DocumentHeaderChrome::default(), None),
            (Vec::new(), None)
        );
    }

    fn assert_mounted_header(
        cx: &Cx,
        app: &App,
        expected_titles: &[&str],
        expected_icon: Option<Icon>,
        expected_height: f64,
    ) {
        let header = app.ui.widget(cx, ids!(document_header));
        let header = header
            .borrow::<crate::document_header::DocumentHeader>()
            .expect("test document header is mounted");
        assert_eq!(
            header
                .test_segments()
                .iter()
                .map(|segment| segment.title.as_str())
                .collect::<Vec<_>>(),
            expected_titles
        );
        assert_eq!(header.test_right_dock(), expected_icon);
        assert_eq!(header.visible_height(), expected_height);
    }

    fn mounted_inspector_state(cx: &Cx, app: &App) -> DockState {
        app.ui
            .widget(cx, ids!(inspector))
            .borrow::<crate::inspector_panel::Inspector>()
            .expect("test inspector is mounted")
            .dock_state()
    }

    #[test]
    fn document_header_source_generic_start_source_sequence_has_no_stale_state() {
        let (mut cx, mut app) = navigation_app();
        let source = crate::okf_documents::open_source(app.session.okf_analysis(), "sales/order")
            .expect("source document exists");
        app.documents.transition(
            &mut cx,
            &app.ui,
            &app.session,
            DocumentCommand::Open {
                document: source,
                persistent: false,
            },
        );
        app.sync_document_shell(&mut cx);
        assert_mounted_header(
            &cx,
            &app,
            &["Root", "Sales", "Order"],
            Some(Icon::SlidersHorizontal),
            crate::document_header::DOCUMENT_HEADER_H,
        );

        // The minimal harness has no mounted Window bounds, so keep responsive
        // mode explicitly narrow instead of letting a zero-width query perform
        // the initial wide-to-narrow reconciliation during the style check.
        app.narrow = true;
        draw_document_header(&mut cx, &app, dvec2(480.0, 30.0));
        let right_button_uid = app
            .ui
            .widget(&cx, ids!(document_header.right_button))
            .widget_uid();
        let action = widget_action(
            right_button_uid,
            crate::icon_button::IconButtonAction::Clicked,
        );
        {
            let header = app.ui.widget(&cx, ids!(document_header));
            let header = header
                .borrow::<crate::document_header::DocumentHeader>()
                .expect("test document header is mounted");
            assert_eq!(
                header.action(std::slice::from_ref(&action)),
                Some(crate::document_header::DocumentHeaderAction::ToggleRightDock)
            );
        }
        app.handle_action_batch(&mut cx, &[action]);
        assert_eq!(mounted_inspector_state(&cx, &app), DockState::Pinned);
        app.sync_dock_slots(&mut cx);

        let generic = crate::okf_documents::open(app.session.okf_analysis(), "sales/order")
            .expect("generic document exists");
        app.documents.transition(
            &mut cx,
            &app.ui,
            &app.session,
            DocumentCommand::Open {
                document: generic,
                persistent: false,
            },
        );
        app.sync_document_shell(&mut cx);
        assert_mounted_header(
            &cx,
            &app,
            &["Root", "Sales", "Order"],
            None,
            crate::document_header::DOCUMENT_HEADER_H,
        );
        assert_eq!(mounted_inspector_state(&cx, &app), DockState::Flag);
        app.sync_dock_slots(&mut cx);

        app.show_start_screen(&mut cx);
        assert_mounted_header(&cx, &app, &[], None, 0.0);
        assert_eq!(mounted_inspector_state(&cx, &app), DockState::Flag);
        app.sync_dock_slots(&mut cx);

        let source = crate::okf_documents::open_source(app.session.okf_analysis(), "sales/order")
            .expect("source document still exists");
        app.documents.transition(
            &mut cx,
            &app.ui,
            &app.session,
            DocumentCommand::Open {
                document: source,
                persistent: false,
            },
        );
        app.sync_document_shell(&mut cx);
        assert_mounted_header(
            &cx,
            &app,
            &["Root", "Sales", "Order"],
            Some(Icon::SlidersHorizontal),
            crate::document_header::DOCUMENT_HEADER_H,
        );
        assert_eq!(mounted_inspector_state(&cx, &app), DockState::Flag);
        app.sync_dock_slots(&mut cx);
    }

    #[test]
    fn visible_mounted_document_header_is_client_area_but_collapsed_header_is_not() {
        let (mut cx, mut app) = navigation_app();
        app.narrow = true;
        let segment = BreadcrumbSegment {
            title: "Order".into(),
            target: NavigationTarget::Document {
                concept_id: "sales/order".into(),
                fragment: None,
            },
        };
        {
            let header_widget = app.ui.widget(&cx, ids!(document_header));
            let mut header = header_widget
                .borrow_mut::<crate::document_header::DocumentHeader>()
                .expect("test document header is mounted");
            header.set_segments(&mut cx, vec![segment]);
            header.set_right_dock(&mut cx, Some(Icon::SlidersHorizontal));
        }
        let header_rect = draw_document_header(&mut cx, &app, dvec2(360.0, 30.0));
        assert_eq!(
            header_rect.size.y,
            crate::document_header::DOCUMENT_HEADER_H
        );
        let response = Rc::new(Cell::new(WindowDragQueryResponse::Caption));
        AppMain::handle_event(
            &mut app,
            &mut cx,
            &Event::WindowDragQuery(WindowDragQueryEvent {
                window_id: WindowId(0, 0),
                abs: header_rect.pos + header_rect.size * 0.5,
                response: response.clone(),
            }),
        );
        assert!(matches!(response.get(), WindowDragQueryResponse::Client));

        {
            let header_widget = app.ui.widget(&cx, ids!(document_header));
            let mut header = header_widget
                .borrow_mut::<crate::document_header::DocumentHeader>()
                .expect("test document header is mounted");
            header.set_segments(&mut cx, Vec::new());
            header.set_right_dock(&mut cx, None);
        }
        assert_eq!(
            app.ui
                .widget(&cx, ids!(document_header))
                .borrow::<crate::document_header::DocumentHeader>()
                .expect("test document header is mounted")
                .visible_height(),
            0.0
        );
        response.set(WindowDragQueryResponse::Caption);
        AppMain::handle_event(
            &mut app,
            &mut cx,
            &Event::WindowDragQuery(WindowDragQueryEvent {
                window_id: WindowId(0, 0),
                abs: header_rect.pos + header_rect.size * 0.5,
                response: response.clone(),
            }),
        );
        assert!(matches!(response.get(), WindowDragQueryResponse::Caption));
    }

    #[test]
    fn shutdown_and_quit_request_are_final_save_events() {
        assert!(should_flush_save(&Event::Shutdown));
        assert!(should_flush_save(&Event::QuitRequested(
            QuitRequestedEvent::new(QuitReason::Menu)
        )));
        assert!(!should_flush_save(&Event::Startup));
    }

    #[test]
    fn failed_final_save_retains_dirty_and_prevents_quit() {
        let result = Err("disk full".to_string());

        let quit = Event::QuitRequested(QuitRequestedEvent::new(QuitReason::Menu));
        assert!(prevent_quit_after_failed_save(&quit, &result));
        let Event::QuitRequested(quit) = quit else {
            unreachable!()
        };
        assert!(quit.handled.get());
    }

    #[test]
    fn successful_bundle_open_clears_the_visible_save_error() {
        let mut state = SaveFeedback::default();
        state.finish_save(&Err("disk full".into()));
        assert_eq!(state.save_error(), Some("disk full"));

        state.opened_replacement_bundle();

        assert_eq!(state.save_error(), None);
    }

    #[test]
    fn replacement_saves_old_document_before_loading_new_document() {
        let calls = RefCell::new(Vec::new());

        let loaded = replace_after_save(
            || {
                calls.borrow_mut().push("save");
                Ok(())
            },
            || {
                calls.borrow_mut().push("load");
                Ok("new document")
            },
        )
        .unwrap();

        assert_eq!(calls.into_inner(), vec!["save", "load"]);
        assert_eq!(loaded, "new document");
    }

    #[test]
    fn failed_save_blocks_replacement_load() {
        let error = replace_after_save(
            || Err("external edit conflict".into()),
            || -> Result<(), String> { panic!("replacement must not load after a failed save") },
        )
        .unwrap_err();

        assert_eq!(
            error,
            BackingTransitionError::Save("external edit conflict".into())
        );
    }

    #[test]
    fn failed_save_blocks_close_and_keeps_document_state() {
        let mut state = vec!["edited"];
        let before = state.clone();

        assert_eq!(
            close_after_save(&mut state, |_| Err("disk full".into())),
            Err("disk full".into())
        );
        assert_eq!(state, before);
    }

    #[test]
    fn successful_save_allows_close_and_clears_document_state() {
        let mut state = vec!["edited"];
        let mut saved = false;

        close_after_save(&mut state, |current| {
            assert_eq!(current, &vec!["edited"]);
            saved = true;
            Ok(())
        })
        .unwrap();

        assert!(saved);
        assert!(state.is_empty());
    }

    #[test]
    fn document_switcher_items_preserve_order_and_tab_identity() {
        let diagram = LiveId::from_str("diagram");
        let customer = LiveId::from_str("customer");
        let order = LiveId::from_str("order");
        let tabs = OpenTabs {
            tabs: vec![
                tab(diagram, "d", "Diagram", TreeKind::Diagram, false),
                tab(customer, "customer", "Customer", TreeKind::Class, false),
                tab(order, "order", "Order", TreeKind::Class, true),
            ],
            active: order,
        };

        let items = doc_switcher_items(&tabs.tabs);
        assert_eq!(
            items.iter().map(|item| item.id).collect::<Vec<_>>(),
            tabs.tabs.iter().map(|tab| tab.id).collect::<Vec<_>>()
        );
        assert_eq!(
            items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Diagram", "Customer", "Order"]
        );
        assert!(items.iter().all(|item| item.enabled && !item.danger));
    }

    #[test]
    fn breakpoint_enters_below_640_and_leaves_above_680() {
        assert!(next_narrow(false, 639.9));
        assert!(next_narrow(true, 680.0));
        assert!(!next_narrow(true, 680.1));
    }

    #[test]
    fn breakpoint_preserves_mode_through_the_hysteresis_band() {
        for width in [640.0, 650.0, 680.0] {
            assert!(!next_narrow(false, width));
            assert!(next_narrow(true, width));
        }
    }

    #[test]
    fn only_the_open_narrow_panel_counts_as_inside() {
        let canvas = Rect {
            pos: dvec2(0.0, 66.0),
            size: dvec2(390.0, 700.0),
        };
        let tree = Rect {
            pos: dvec2(0.0, 66.0),
            size: dvec2(280.0, 700.0),
        };
        let inspector = Rect {
            pos: dvec2(70.0, 66.0),
            size: dvec2(320.0, 700.0),
        };
        assert!(open_overlay_contains(
            dvec2(100.0, 200.0),
            DockState::Pinned,
            tree,
            DockState::Flag,
            inspector
        ));
        assert!(!open_overlay_contains(
            dvec2(300.0, 200.0),
            DockState::Pinned,
            tree,
            DockState::Flag,
            inspector
        ));
        assert!(should_dismiss_narrow_dock(
            dvec2(300.0, 200.0),
            canvas,
            DockState::Pinned,
            tree,
            DockState::Flag,
            inspector
        ));
        assert!(!should_dismiss_narrow_dock(
            dvec2(16.0, 50.0),
            canvas,
            DockState::Pinned,
            tree,
            DockState::Flag,
            inspector
        ));
    }

    #[test]
    fn conflict_delete_maps_to_place_rm() {
        let action = ConflictListAction::Delete {
            subject: "order".to_string(),
            reference: "payment-gateway".to_string(),
        };
        let op = place_rm_for("dia", &action);
        assert_eq!(
            op,
            Some(waml::uml::Op::PlacementRemove {
                diagram: "dia".to_string(),
                subject_slug: "order".to_string(),
                reference_slug: "payment-gateway".to_string(),
            })
        );
    }

    #[test]
    fn conflict_focus_never_maps_to_an_op() {
        let action = ConflictListAction::Focus {
            subject: "order".to_string(),
            reference: "payment-gateway".to_string(),
        };
        assert_eq!(place_rm_for("dia", &action), None);
        assert_eq!(place_rm_for("dia", &ConflictListAction::None), None);
    }

    // End-to-end at the ops layer (no live `Cx`/`App` needed): the mapped
    // `Op::PlaceRm` removes ONLY the targeted placement from the re-serialized
    // bundle, leaving an unrelated one intact. The solver's dropped/
    // conflicts_with reporting is already covered by Task 1's `waml::ops`
    // tests and `scene.rs`'s `project_conflicts` tests.
    #[test]
    fn conflict_delete_removes_only_the_targeted_placement() {
        let source = waml::source::SourceBundle::try_from_pairs([(
            "shop/dia.md".to_string(),
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n\
             - [Order](./order.md) left of [PaymentGateway](./payment-gateway.md)\n\
             - [Customer](./customer.md) below [Order](./order.md)\n"
                .to_string(),
        )])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source.clone(), None, 1).unwrap();
        let action = ConflictListAction::Delete {
            subject: "order".to_string(),
            reference: "payment-gateway".to_string(),
        };
        let op = place_rm_for("dia", &action).expect("Delete maps to an Op");
        let out = waml::edit::EditBatch::lower(
            &waml::uml::Batch(vec![op]),
            waml::edit::EditContext {
                source: &source,
                okf_analysis: prepared.okf(),
                session_revision: prepared.revision(),
                uml: prepared.uml(),
            },
        )
        .unwrap();
        let markdown = out.document_by_concept_id("shop/dia").unwrap().text();
        assert!(
            !markdown.contains("left of"),
            "the deleted placement is gone: {markdown}"
        );
        assert!(
            markdown.contains("below"),
            "the OTHER placement survives: {markdown}"
        );
    }

    #[test]
    fn logo_command_for_maps_ids_and_rejects_others() {
        assert_eq!(
            logo_command_for(live_id!(properties)),
            Some(LogoCommand::Properties)
        );
        assert_eq!(logo_command_for(live_id!(about)), Some(LogoCommand::About));
        assert_eq!(logo_command_for(live_id!(fonts)), Some(LogoCommand::Fonts));
        assert_eq!(logo_command_for(live_id!(icons)), Some(LogoCommand::Icons));
        assert_eq!(
            logo_command_for(live_id!(colors)),
            Some(LogoCommand::Colors)
        );
        assert_eq!(logo_command_for(live_id!(exit)), Some(LogoCommand::Exit));
        // Cancel maps to nothing (the radial just closes on commit).
        assert_eq!(logo_command_for(live_id!(cancel)), None);
        // A node-radial id / unknown id is not ours.
        assert_eq!(logo_command_for(live_id!(remove)), None);
        assert_eq!(logo_command_for(live_id!(nonsense)), None);
    }
}
