mod actions;
mod navigation;
mod shell;

use self::navigation::{PendingAnchorRestore, PendingFragment, TransitionCause};

use crate::doc_tabs::OpenTabs;
use crate::dock::{DockMotion, DockState, ResponsiveDockLayout};
use crate::document::NavCategory;
use crate::document_host::{DocumentCommand, DocumentHost};
use crate::editor_session::{EditorSession, ExternalReplacement, SaveCompletion, SaveTicket};
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

fn restore_markdown_asset_host_after_open(
    installed: &mut Option<crate::markdown_hosts::SharedMarkdownAssetHost>,
    previous: Option<crate::markdown_hosts::SharedMarkdownAssetHost>,
    opened: bool,
) -> bool {
    if !opened {
        *installed = previous;
    }
    opened
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn browser_save_fragment(ticket: &SaveTicket) -> (String, SaveCompletion) {
    (
        format!("#{}", waml::share::encode_source(&ticket.snapshot.source)),
        SaveCompletion {
            revision: ticket.revision,
            history_state: ticket.history_state,
            result: Ok(()),
        },
    )
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
    /// Scope / search / type-filter state for the tree panel's header band; the
    /// app owns it and rebuilds `NavView` on every change (see `nav.rs`).
    #[rust]
    nav_state: NavState,
    /// Distinct `TreeKind`s present in the currently open model, in canonical
    /// order; the type-filter dropdown lists these (plus the "All" row).
    /// Recomputed once per model load (`open_dir`), not per keystroke.
    #[rust]
    nav_kinds: Vec<crate::tree::TreeKind>,
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
}

impl App {
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
        let Some(ticket) = self.session.save_ticket() else {
            return Ok(());
        };
        if ticket.snapshot.source.is_empty() {
            return Err("cannot save an empty bundle".to_string());
        }
        let completion = self.save_backend(cx, &ticket)?;
        let result = completion.result.clone();
        self.session.finish_save(completion);
        result
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

    #[allow(dead_code)] // Native file watching calls this ingress when enabled.
    fn replace_external_document(
        &mut self,
        cx: &mut Cx,
        document: waml::analysis::DocumentId,
        base_revision: waml_markdown_editor::syntax::DocumentRevision,
        text: String,
    ) -> Result<ExternalReplacement, String> {
        let mut replacement = self
            .session
            .replace_external(document, base_revision, text.clone())
            .map_err(|error| error.to_string())?;
        if let ExternalReplacement::Conflict { dirty_revision } = &replacement {
            debug_assert_eq!(
                self.session.snapshot().dirty_revision,
                Some(*dirty_revision)
            );
            self.save_or_retry(cx, false)?;
            replacement = self
                .session
                .replace_external(document, base_revision, text)
                .map_err(|error| error.to_string())?;
        }
        let ExternalReplacement::Installed(change) = &replacement else {
            return Ok(replacement);
        };
        let assets = self
            .markdown_assets
            .as_ref()
            .ok_or_else(|| "Markdown asset host is not initialized".to_string())?;

        let prepared = self
            .documents
            .tabs()
            .iter()
            .map(|tab| {
                crate::documents::reopen_with_asset_host(
                    self.session.okf_analysis(),
                    self.session.uml_analysis(),
                    tab,
                    assets,
                )
            })
            .collect();
        self.documents.after_external_replacement(
            cx,
            &self.ui,
            &self.session,
            change.clone(),
            prepared,
        );
        if change.uml_changed {
            self.sync_document_shell(cx);
        }
        if change.navigation_changed {
            self.nav_kinds = crate::nav::kinds_in_model(
                self.session.okf_analysis(),
                self.session.uml_analysis(),
            );
            self.refresh_nav(cx, false);
        }
        if change.conflicts_changed {
            self.sync_conflict_badge(cx);
        }
        self.sync_history_controls(cx);
        Ok(replacement)
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
    fn save_backend(&self, cx: &mut Cx, ticket: &SaveTicket) -> Result<SaveCompletion, String> {
        let (fragment, completion) = browser_save_fragment(ticket);
        cx.browser_update_url(&fragment, true);
        Ok(completion)
    }

    /// Native backing: atomically replace each authored file in the opened OKF
    /// directory. The helper validates bundle paths before performing writes.
    #[cfg(not(target_arch = "wasm32"))]
    fn save_backend(&self, _cx: &mut Cx, ticket: &SaveTicket) -> Result<SaveCompletion, String> {
        let Some(root) = self.open_dir.as_deref() else {
            return Err("native bundle has no opened directory".to_string());
        };
        let mut completion = crate::native_save::save_ticket_atomic(root, ticket)
            .map_err(|error| format!("failed to save OKF dir {root:?}: {error}"))?;
        completion.result = completion
            .result
            .map_err(|error| format!("failed to save OKF dir {root:?}: {error}"));
        Ok(completion)
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
        let asset_policy = match crate::markdown_hosts::MarkdownAssetPolicy::native(&next_root) {
            Ok(policy) => policy,
            Err(error) => {
                log!("failed to canonicalize Markdown asset root {next_root:?}: {error}");
                return false;
            }
        };
        let previous_assets =
            self.markdown_assets
                .replace(crate::markdown_hosts::EditorMarkdownAssetHost::shared(
                    asset_policy,
                ));
        let opened = self.open_bundle(cx, bundle, display_name, wanted_diagram);
        if !restore_markdown_asset_host_after_open(
            &mut self.markdown_assets,
            previous_assets,
            opened,
        ) {
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
        if self.markdown_assets.is_none() {
            self.markdown_assets = Some(crate::markdown_hosts::EditorMarkdownAssetHost::shared(
                crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
            ));
        }
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
            .set_icon(cx, crate::icons::Icon::PanelLeftClose);
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
            self.dock_layout = ResponsiveDockLayout::default();
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
        self.dock_layout = ResponsiveDockLayout::default();
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
                let Some(ticket) = session.save_ticket() else {
                    return Ok(());
                };
                let root =
                    root.ok_or_else(|| "native bundle has no opened directory".to_string())?;
                crate::native_save::save_ticket_atomic(root, &ticket)
                    .map_err(|error| format!("failed to save OKF dir {root:?}: {error}"))?
                    .result
                    .map_err(|error| format!("failed to save OKF dir {root:?}: {error}"))
            })
        };

        #[cfg(target_arch = "wasm32")]
        let result = close_after_save(&mut self.session, |session| {
            if let Some(ticket) = session.save_ticket() {
                cx.browser_update_url(
                    &format!("#{}", waml::share::encode_source(&ticket.snapshot.source)),
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
        self.markdown_assets = None;
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
            let macos = matches!(cx.os_type(), OsType::Macos);
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

        self.documents.route_ui_event(cx, &self.ui, event);
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

        // Push each panel's sampled motion slot width to its reservation
        // spacer and body width to its host every frame. NextFrame samples
        // active motion.
        self.sync_dock_slots(cx);
        // Same shape for the marker's row width: it is mounted zero-width, so
        // `App` is the only thing that knows how wide the title row is.
        self.sync_agent_row(cx);
    }
}

#[cfg(test)]
mod tests;
