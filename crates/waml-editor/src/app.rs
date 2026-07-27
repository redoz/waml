mod actions;

use crate::doc_tabs::{OpenTabs, TabKind};
use crate::dock::DockState;
use crate::dock::ResponsiveDockLayout;
use crate::document_host::{DocumentCommand, DocumentHost};
use crate::editor_session::EditorSession;
use crate::fps_meter::FpsMeter;
use crate::icon_button::IconButtonWidgetRefExt;
use crate::load;
use crate::nav::NavState;
use crate::popup::base::PopupResult;
use crate::popup::root::{MenuOpen, PopupRoot, PopupSpec};
use crate::popup::select::{SelectItem, SelectLead};
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

script_mod! {
    use mod.prelude.widgets.*
    use mod.atlas
    use mod.fonts
    use mod.widgets.ClassDiagramSurface
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
                        // Don't clip the tab band at the column's right edge: the
                        // top rule (drawn by `DocTabs`) still extends past this
                        // column's LEFT edge, back over `[T]` and the tree spacer to
                        // `tab_row`'s own start. `title_row` keeps its own `clip_x`
                        // for the model name; only the tab band needs the overshoot.
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
                            // them. That reserve was what held `[I]` 138px inboard of
                            // the window edge instead of over the column it toggles.
                            // Charged to this row alone, the tab row now runs the full
                            // window width and `[I]` lands flush right.
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
                            // See `caption_col`: the tab strip's top rule overshoots
                            // this row on BOTH sides -- right to the window edge, left
                            // back to this row's own left edge (x=0, past `[T]` and
                            // the spacer).
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
                            // The right-hand twin of `[T]`: the ACTIVE VIEW's
                            // right-dock toggle. LAST child, so it is anchored
                            // hard against the row's right edge and never
                            // moves -- the `Fill` tab strip absorbs every bit
                            // of slack between the two, so opening tabs or
                            // expanding the tree column slides only the cards.
                            // The same 30px box / 18px glyph as `menu_btn` and
                            // `tree_btn`, so all three caption glyphs read as
                            // one set, with `[T]`'s 2px inset mirrored to the
                            // right edge.
                            //
                            // Visibility AND glyph come from
                            // `BodyChrome.right_dock` (see
                            // `sync_right_dock_btn`), NOT from
                            // `show_editor`/`show_start_screen` the way
                            // `tree_btn` is: the button exists because the
                            // active view says it does (and when it does not,
                            // the same seam forces the column shut -- this is
                            // its only close affordance). Counted into
                            // `INSPECTOR_BTN_W`, which `DocTabs` adds back to
                            // its top rule's right overshoot while the button
                            // is mounted (`doc_tabs::rule_x_end`).
                            //
                            // This row now runs the FULL window width -- the
                            // min/max/close reserve is charged to `title_row`
                            // alone -- so `[I]` sits flush against the window's
                            // right edge, over the dock column it toggles.
                            inspector_btn := IconButton{ width: 30.0 height: 30.0 icon_size: 18.0 margin: Inset{right: 2.0, top: 1.0} visible: false }
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
                            // Center: canvas base + aux HUD floaters. Fill, so it
                            // takes whatever the slots leave. Overlay so each
                            // floater wrapper gets the full center rect and parks
                            // itself by `align`; wrappers carry no bg and grab no
                            // pointer events over empty area, so the canvas keeps
                            // pan/zoom in the gaps.
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
                                // `source_view` below.
                                canvas_wrap := View{
                                    width: Fill
                                    height: Fill
                                    flow: Overlay
                                    canvas := ClassDiagramSurface{
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
                                source_view := View{
                                    width: Fill
                                    height: Fill
                                    visible: false
                                    show_bg: true
                                    draw_bg.color: atlas.surface
                                    flow: Down
                                    scroll_bars: ScrollBars{ scroll_bar_y: ScrollBar{} }
                                    md := Markdown{
                                        width: Fill
                                        height: Fit
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

/// Footprint of the caption's right-dock toggle `[I]`: the `inspector_btn` DSL
/// `width` (30, the burger's size) plus its 2px right margin. The right-hand
/// twin of `TREE_BTN_W`. `pub(crate)` because `DocTabs` has the other consumer:
/// the tab strip's turtle is now shorter by exactly this, so its top rule has to
/// overshoot by this much more to still reach the window's right edge.
pub(crate) const INSPECTOR_BTN_W: f64 = 32.0;

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
}

impl App {
    /// Synchronize shell projections after the document host has completed a
    /// transition. Document content and view-specific chrome stay host-owned.
    fn sync_document_shell(&mut self, cx: &mut Cx) {
        let selected = self.documents.active_tab().map(|tab| tab.key.clone());
        if let Some(mut tree) = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
        {
            tree.set_selected_key(cx, selected);
        }
        self.sync_diagram_switcher_current(cx);
        self.sync_statusbar(cx);
        self.sync_conflict_badge(cx);
    }

    /// Open or focus a document through the shared preview slot. All callers
    /// use this path so replacement cleanup and view/chrome synchronization
    /// stay identical for classifiers and diagrams.
    fn transition_document(
        &mut self,
        cx: &mut Cx,
        key: &str,
        node_kind: crate::tree::TreeKind,
        persistent: bool,
    ) -> bool {
        let title = if node_kind == crate::tree::TreeKind::Diagram {
            self.session
                .model()
                .diagrams
                .iter()
                .find(|diagram| diagram.key == key)
                .map(|diagram| diagram.title.clone())
        } else {
            self.session
                .model()
                .nodes
                .iter()
                .find(|node| node.key == key)
                .map(|node| {
                    node.concept
                        .title
                        .clone()
                        .unwrap_or_else(|| node.key.clone())
                })
        };
        let Some(title) = title else {
            return false;
        };

        let changed = self.documents.transition(
            cx,
            &self.ui,
            &self.session,
            DocumentCommand::Open {
                key: key.to_owned(),
                title,
                node_kind,
                persistent,
            },
        );
        self.sync_document_shell(cx);
        changed
    }

    /// Push the active diagram title into the switcher's trigger chip, falling
    /// back to another open diagram when a classifier is active.
    fn sync_diagram_switcher_current(&mut self, cx: &mut Cx) {
        let title = self
            .documents
            .active_tab()
            .filter(|tab| tab.kind == TabKind::Diagram)
            .or_else(|| {
                self.documents
                    .tabs()
                    .iter()
                    .find(|tab| tab.kind == TabKind::Diagram)
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
        self.ui
            .widget(cx, ids!(inspector_btn))
            .as_icon_button()
            .set_active(cx, inspector_state == crate::dock::DockState::Pinned);
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
        let (node_count, zoom_pct) = self
            .ui
            .widget(cx, ids!(canvas))
            .borrow_mut::<crate::canvas::ClassDiagramSurface>()
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
        if !self.session.is_dirty() {
            return Ok(());
        }
        if self.session.bundle().is_empty() {
            return Err("cannot save an empty bundle".to_string());
        }
        let revision = self.session.revision();
        self.save_backend(cx)?;
        self.session.mark_saved(revision);
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
    fn save_backend(&mut self, cx: &mut Cx) -> Result<(), String> {
        cx.browser_update_url(
            &format!("#{}", waml::share::encode(self.session.bundle())),
            true,
        );
        Ok(())
    }

    /// Native backing: atomically replace each authored file in the opened OKF
    /// directory. The helper validates bundle paths before performing writes.
    #[cfg(not(target_arch = "wasm32"))]
    fn save_backend(&mut self, _cx: &mut Cx) -> Result<(), String> {
        let Some(root) = self.open_dir.as_deref() else {
            return Err("native bundle has no opened directory".to_string());
        };
        crate::native_save::save_bundle_atomic(
            root,
            self.session.persisted_bundle(),
            self.session.bundle(),
        )
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
                load::load_bundle_and_model(&next_root)
                    .map_err(|error| format!("failed to load OKF dir {next_root:?}: {error}"))
            },
        );
        let (bundle, model) = match transition {
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
        self.open_dir = Some(next_root);
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
        let root_name = if model.path.is_empty() {
            display_name.as_str()
        } else {
            model.path.as_str()
        };
        crate::config::push_recent(dir, root_name);
        self.open_bundle(cx, bundle, model, display_name, wanted_diagram)
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
        files: Vec<(String, String)>,
        model: waml::model::Model,
        display_name: String,
        wanted_diagram: Option<&str>,
    ) -> bool {
        cx.stop_timer(self.save_timer);
        let change = self.session.replace(files, model);
        debug_assert_eq!(change.revision, self.session.revision());
        self.save_feedback.opened_replacement_bundle();
        self.sync_save_error(cx);
        // Retain the raw bundle so drag-to-place ops can re-author `## Layout`
        // in-memory: the diagram view emits `Op::PlaceSet`, the shell applies it
        // against this bundle and rebuilds the model (see `handle_actions`).
        // Fresh model: recompute the type-filter chip's cycle and reset scope /
        // search / filter to the whole-model browse state.
        self.nav_kinds = crate::nav::kinds_in_model(self.session.model());
        self.nav_state = NavState::default();

        self.open_name = display_name;

        let root_name = if self.session.model().path.is_empty() {
            self.open_name.as_str()
        } else {
            self.session.model().path.as_str()
        };
        self.ui.label(cx, ids!(model_name)).set_text(cx, root_name);

        self.refresh_nav(cx, true);

        // A model may carry zero diagrams (a pure classifier/behavior bundle). We
        // still open it -- the project tree is useful on its own -- just with an
        // empty canvas and no active diagram tab. With no tab there is no view to
        // declare a right dock, so the inspector stays closed and its `[I]`
        // toggle stays hidden.
        let initial_tabs = match crate::cli::select_diagram(self.session.model(), wanted_diagram) {
            Some(diagram) => {
                // Seed a fresh diagram preview through the document host.
                OpenTabs::diagram_preview(diagram.key.clone(), diagram.title.clone())
            }
            None => {
                log!(
                    "no diagrams in {:?}; opening model with an empty canvas",
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
                OpenTabs::default()
            }
        };
        self.documents
            .replace_for_session(cx, &self.ui, &self.session, initial_tabs);
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
        let root_name = if self.session.model().path.is_empty() {
            self.open_name.as_str()
        } else {
            self.session.model().path.as_str()
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
                if !session.is_dirty() {
                    return Ok(());
                }
                let root =
                    root.ok_or_else(|| "native bundle has no opened directory".to_string())?;
                crate::native_save::save_bundle_atomic(
                    root,
                    session.persisted_bundle(),
                    session.bundle(),
                )
                .map_err(|error| format!("failed to save OKF dir {root:?}: {error}"))
            })
        };

        #[cfg(target_arch = "wasm32")]
        let result = close_after_save(&mut self.session, |session| {
            if session.is_dirty() {
                cx.browser_update_url(&format!("#{}", waml::share::encode(session.bundle())), true);
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
        let view = crate::nav::view(self.session.model(), &self.nav_state);
        let chip = crate::nav::chip_label(self.nav_state.filter).to_string();
        let title = scope_changed.then(|| {
            crate::nav::packages(self.session.model())
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
            icon: crate::icons::IconSet::icon_for(tab.node_kind),
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
) -> Option<waml::ops::Op> {
    match action {
        crate::popup::conflict_list::ConflictListAction::Delete { subject, reference } => {
            Some(waml::ops::Op::PlaceRm {
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
        match waml::share::decode(&fragment) {
            Ok(bundle) => {
                let model = waml::parse::build_model(&bundle);
                self.open_bundle(cx, bundle, model, "shared".to_string(), None);
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
        crate::property_controls::script_mod(vm);
        crate::diagram_properties::script_mod(vm);
        crate::tree_panel::script_mod(vm);
        // `select_box` must register before `inspector_panel`: the inspector's
        // `element_bar` mounts `SelectBox` as a child, and the DSL resolves
        // `mod.widgets.*` eagerly at `use`-time, not lazily.
        crate::select_box::script_mod(vm);
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
            // Same for the tab row's right-dock toggle: it sits in the caption
            // drag region, so without this every press becomes a window drag
            // and the toggle is silently dead.
            //
            // This one is why `IconButton::rect` reads the LIVE area instead of
            // a rect cached in `draw_walk`: `[I]` TRAILS the `Fill` tab strip,
            // whose deferred walk shifts the button right only after the row's
            // turtle closes. The cached rect named the pre-shift x (the tree
            // column's right edge), so this test was false everywhere the
            // button actually is.
            let over_inspector_btn = self
                .ui
                .widget(cx, ids!(inspector_btn))
                .as_icon_button()
                .rect(cx)
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
            if over_tab || over_logo || over_btn || over_tree_btn || over_inspector_btn || menu_open
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
        place_rm_for, prevent_quit_after_failed_save, replace_after_save,
        should_dismiss_narrow_dock, should_flush_save, BackingTransitionError, LogoCommand,
        SaveFeedback,
    };
    use crate::doc_tabs::{DocTab, OpenTabs, TabKind};
    use crate::dock::DockState;
    use crate::popup::conflict_list::ConflictListAction;
    use crate::tree::TreeKind;
    use makepad_widgets::*;
    use std::cell::RefCell;

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
                DocTab {
                    id: diagram,
                    key: "d".into(),
                    title: "Diagram".into(),
                    kind: TabKind::Diagram,
                    node_kind: TreeKind::Diagram,
                    preview: false,
                },
                DocTab {
                    id: customer,
                    key: "customer".into(),
                    title: "Customer".into(),
                    kind: TabKind::Classifier,
                    node_kind: TreeKind::Class,
                    preview: false,
                },
                DocTab {
                    id: order,
                    key: "order".into(),
                    title: "Order".into(),
                    kind: TabKind::Classifier,
                    node_kind: TreeKind::Class,
                    preview: true,
                },
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
            Some(waml::ops::Op::PlaceRm {
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
        let bundle = vec![(
            "shop/dia.md".to_string(),
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n\
             - [Order](./order.md) left of [PaymentGateway](./payment-gateway.md)\n\
             - [Customer](./customer.md) below [Order](./order.md)\n"
                .to_string(),
        )];
        let action = ConflictListAction::Delete {
            subject: "order".to_string(),
            reference: "payment-gateway".to_string(),
        };
        let op = place_rm_for("dia", &action).expect("Delete maps to an Op");
        let out = waml::ops::apply(&bundle, &[op]).unwrap();
        assert!(
            !out[0].1.contains("left of"),
            "the deleted placement is gone: {}",
            out[0].1
        );
        assert!(
            out[0].1.contains("below"),
            "the OTHER placement survives: {}",
            out[0].1
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
