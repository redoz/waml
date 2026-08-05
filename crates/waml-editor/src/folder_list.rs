//! `FolderListView`: the widget surface for a folder's projected chain
//! (Task D1b, spec 2026-08-05-folder-view-middleware-design.md). Renders the
//! `FolderRowView` view-model built in `folder_view.rs`: bullet, label,
//! optional blurb, one row per projected `Row`, in order. Modeled on
//! `start_screen.rs`'s capped `FlatList` of real row widgets -- here the list
//! is uncapped (a folder's declared view controls its own row count) and
//! scrolls instead of clamping.
//!
//! `FolderRow` is the per-row widget (declared in this same file: a widget
//! must register before any consumer that embeds it, and `FolderListView`'s
//! `FlatList` template is that consumer, so `FolderRow` is registered first in
//! the `script_mod!` block below). It only fires a click for a row whose
//! `action` carries a navigation target -- a `Virtual` row (no file behind it)
//! draws inert, matching `action_for`'s `FolderRowAction::None`.

use makepad_widgets::*;

use crate::cursor;
use crate::folder_view::{FolderRowAction, FolderRowView};
use crate::icons::{Icon, IconSet};

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.FolderRowBase = #(FolderRow::register_widget(vm))

    mod.widgets.FolderRow = set_type_default() do mod.widgets.FolderRowBase{
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.0}
        padding: Inset{left: 4.0, right: 4.0, top: 6.0, bottom: 6.0}
        spacing: 8.0
        show_bg: true
        draw_bg +: {
            color: atlas.accent
            hover: uniform(0.0)
            pixel: fn() {
                let a = 0.10 * self.hover
                return vec4(self.color.x * a, self.color.y * a, self.color.z * a, a)
            }
        }

        // Colour-only holder (never drawn): the immediate-mode glyph copies
        // `color` from this per draw, matching `recent_row.rs`'s `draw_pkg`
        // pattern -- no RGBA crosses Rust.
        draw_icon +: { color: atlas.text_dim }

        // Icon anchor: a 16x16 spacer reserving the flow slot the bullet used
        // to occupy. `Icon::draw` (Task 12) draws the row's resolved `Icon`
        // immediate-mode over this rect in `draw_walk`.
        icon_anchor := View {
            width: 16.0
            height: 16.0
        }

        textcol := View {
            width: Fill
            height: Fit
            flow: Down
            spacing: 2.0
            label := Label {
                width: Fill
                text: ""
                draw_text +: {
                    color: atlas.text
                    text_style: fonts.text_label
                }
            }
            blurb := Label {
                width: Fill
                text: ""
                visible: false
                draw_text +: {
                    color: atlas.text_dim
                    text_style: fonts.text_micro
                }
            }
        }
    }

    mod.widgets.FolderListViewBase = #(FolderListView::register_widget(vm))

    mod.widgets.FolderListView = set_type_default() do mod.widgets.FolderListViewBase{
        width: Fill
        height: Fill
        flow: Down
        // Chain-diagnostics strip: hidden when the chain built and ran
        // clean, shown above the rows when it degraded (whole-chain fallback
        // to the root view). Named the stage and the reason -- written for
        // the document author, not the runner -- so a folder's `view:`
        // mistake is visible the moment its tab opens, not just as the tree
        // marker on a collapsed row.
        diag_strip := View {
            width: Fill
            height: Fit
            visible: false
            padding: Inset{left: 24.0, right: 24.0, top: 10.0, bottom: 10.0}
            show_bg: true
            draw_bg +: {
                color: atlas.danger
                pixel: fn() {
                    return vec4(self.color.x, self.color.y, self.color.z, 0.12)
                }
            }
            diag_label := Label {
                width: Fill
                text: ""
                draw_text +: {
                    color: atlas.text
                    text_style: fonts.text_label
                }
            }
        }
        rows_scroll := ScrollYView {
            width: Fill
            height: Fill
            padding: Inset{left: 24.0, right: 24.0, top: 16.0, bottom: 24.0}
            rows_list := FlatList {
                width: Fill
                height: Fill
                flow: Down
                spacing: 2.0
                Row := mod.widgets.FolderRow { }
            }
        }
    }
}

/// Emitted (grouped through the parent `FlatList`) when a row with a
/// navigation target is clicked. `Virtual` rows never fire this.
#[derive(Clone, Debug, Default)]
pub enum FolderRowClickAction {
    #[default]
    None,
    Clicked,
}

#[derive(Script, ScriptHook, Widget)]
pub struct FolderRow {
    #[deref]
    view: View,
    /// SDF icon set (shared Atlas material), drawn via `IconSet::draw` --
    /// the `recent_row.rs` pattern (Task 12).
    #[live]
    icons: IconSet,
    /// Colour-only holder, copied into the glyph tint per draw.
    #[live]
    draw_icon: DrawColor,
    /// This row's resolved icon, bound in `set_row`.
    #[rust]
    icon: Icon,
    /// The icon anchor's absolute rect, captured during `draw_walk` for the
    /// immediate-mode glyph drawn over it.
    #[rust]
    icon_rect: Rect,
    /// Whether this row has a navigation target -- gates both the hover wash
    /// and the click. A `Virtual` row (`FolderRowAction::None`) is drawn but
    /// never clickable, matching `action_for`.
    #[rust]
    clickable: bool,
    #[rust]
    hovered: bool,
    /// Whether this row holds `FolderListView`'s keyboard focus (Task G3).
    /// Presentational only -- the same tint uniform as hover, distinct
    /// state so focus survives the pointer moving off the row.
    #[rust]
    focused: bool,
}

impl Widget for FolderRow {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if !self.clickable {
            return;
        }
        let uid = self.widget_uid();
        match event.hits(cx, self.view.area()) {
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over => {
                cx.widget_action(uid, FolderRowClickAction::Clicked);
            }
            Hit::FingerHoverIn(_) => {
                self.hovered = true;
                cursor::hover_in(cx, MouseCursor::Hand);
                self.view.redraw(cx);
            }
            Hit::FingerHoverOut(_) => {
                self.hovered = false;
                cursor::hover_out(cx);
                self.view.redraw(cx);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let tinted = self.hovered || self.focused;
        self.view
            .draw_bg
            .set_uniform(cx, live_id!(hover), &[if tinted { 1.0 } else { 0.0 }]);
        let step = self.view.draw_walk(cx, scope, walk);
        self.icon_rect = self.view.view(cx, ids!(icon_anchor)).area().rect(cx);
        self.icons
            .draw(cx, self.icon, self.icon_rect, self.draw_icon.color);
        step
    }
}

impl FolderRow {
    /// Bind one projected row's view-model onto this widget instance.
    /// `focused` is `FolderListView`'s keyboard-focused row index (Task G3),
    /// compared by the caller -- presentational only, mirrors `hovered`'s
    /// wash so the focused row stays visibly distinct without a click.
    /// `label_override`, when `Some`, is the live rename edit buffer drawn
    /// in place of `row.label` -- "typing retitles live" without touching
    /// `row` (the projected view-model is not re-run on every keystroke).
    pub fn set_row(
        &mut self,
        cx: &mut Cx,
        row: &FolderRowView,
        focused: bool,
        label_override: Option<&str>,
    ) {
        self.icon = row.icon;
        self.view
            .label(cx, ids!(textcol.label))
            .set_text(cx, label_override.unwrap_or(&row.label));
        let blurb = row.blurb.as_deref().unwrap_or("");
        self.view.label(cx, ids!(textcol.blurb)).set_text(cx, blurb);
        self.view
            .label(cx, ids!(textcol.blurb))
            .set_visible(cx, !blurb.is_empty());
        let clickable = !matches!(row.action, FolderRowAction::None);
        if self.clickable != clickable {
            self.clickable = clickable;
            self.hovered = false;
        }
        self.focused = focused;
    }

    pub fn clicked(&self, actions: &Actions) -> bool {
        actions
            .find_widget_action(self.widget_uid())
            .is_some_and(|a| matches!(a.cast(), FolderRowClickAction::Clicked))
    }
}

impl FolderRowRef {
    pub fn set_row(
        &self,
        cx: &mut Cx,
        row: &FolderRowView,
        focused: bool,
        label_override: Option<&str>,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_row(cx, row, focused, label_override);
        }
    }
    pub fn clicked(&self, actions: &Actions) -> bool {
        self.borrow().is_some_and(|inner| inner.clicked(actions))
    }
}

/// Emitted when a row with a navigation target is clicked, indexing the rows
/// last passed to `set_rows`.
#[derive(Clone, Debug, Default)]
pub enum FolderListViewAction {
    #[default]
    None,
    RowOpened(usize),
    /// `KeyCode::ReturnKey` fired while `index` held keyboard focus (Task
    /// G3). `FolderView::handle` maps this through `enter_row_op`.
    EnterPressed(usize),
    /// `KeyCode::Tab` (no shift) fired while `index` held keyboard focus.
    /// Maps through `tab_row_op`.
    TabPressed(usize),
    /// `KeyCode::Tab` with shift held. Maps through `shift_tab_row_op`.
    ShiftTabPressed(usize),
    /// `KeyCode::ReturnKey` fired while `index` was mid-rename, carrying the
    /// accumulated edit buffer. `FolderView::handle` maps this through
    /// `rename_row_op`. A cancelled rename (`Escape`, focus loss, or a
    /// resync out from under it) never fires this -- the buffer is dropped
    /// instead.
    RenameCommitted(usize, String),
    /// A drag armed on row `from_index` released with the pointer over
    /// `drop_index` (Task G4), both indexing the rows last passed to
    /// `set_rows` in their pre-drag order. `FolderView::handle` maps this
    /// through `reorder_row_op`, which treats an on-self or
    /// immediately-after-self drop as a no-op.
    RowDropped(usize, usize),
}

#[derive(Script, ScriptHook, Widget)]
pub struct FolderListView {
    #[deref]
    view: View,
    #[rust]
    rows: Vec<FolderRowView>,
    /// Keyboard-focused row index (Task G3), independent of any click --
    /// a row click still opens its target immediately (unchanged), so focus
    /// only drives Up/Down/Return/Tab. Re-clamped in `set_rows` so a resync
    /// after an edit never points past the new row count.
    #[rust]
    focused: Option<usize>,
    /// Whether this list currently holds keyboard focus, tracked off
    /// `Hit::KeyFocus`/`Hit::KeyFocusLost` so arrow/Return/Tab keys are
    /// ignored while some other widget (e.g. a text field) has focus.
    #[rust]
    has_key_focus: bool,
    /// The row index mid-rename (Task G3's "typing retitles live"), if any.
    /// `Some` while `F2` is held on the focused row; every path that could
    /// leave the list in a stale edit state (`Escape`, `Hit::KeyFocusLost`,
    /// a `set_rows` resync out from under the edit) clears it, mirroring
    /// `inspector_panel.rs`'s `editing` field.
    #[rust]
    renaming: Option<usize>,
    /// The accumulated edit buffer while `renaming.is_some()`, seeded from
    /// the row's current label on `F2` and drawn in the row's place instead
    /// of `FolderRowView::label` -- the live part of "typing retitles live".
    /// Discarded (never committed) on cancel.
    #[rust]
    rename_buffer: String,
    /// Each row's absolute draw-time rect, recorded in `draw_walk` in the
    /// same order as `rows` -- the coordinate space `drop_index_from_pointer_y`
    /// needs to turn a `FingerUp` position into a drop index (Task G4).
    /// Reset to the current row count's length every draw pass, so a resync
    /// mid-drag (a row inserted/removed underneath) never reads a stale rect.
    #[rust]
    row_rects: Vec<Rect>,
    /// The row index a reorder drag armed on (Task G4), if a drag is live.
    /// Cleared on EVERY `FingerUp`, including a drop with no rect recorded
    /// for its landing position (`row_rects` empty or stale) -- an armed
    /// drag must never survive past its own release (correctness.md).
    #[rust]
    dragging: Option<usize>,
}

impl Widget for FolderListView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let uid = self.widget_uid();
        match event.hits(cx, self.view.area()) {
            Hit::FingerDown(_) => {
                cx.set_key_focus(self.view.area());
            }
            Hit::KeyFocus(_) => {
                self.has_key_focus = true;
            }
            Hit::KeyFocusLost(_) => {
                self.has_key_focus = false;
                // Every armed edit clears on focus loss (correctness.md) --
                // an unfinished rename must never survive to the next click.
                self.renaming = None;
            }
            Hit::KeyDown(ke) if self.has_key_focus && self.renaming.is_some() => {
                let index = self.renaming.expect("checked by the guard above");
                match ke.key_code {
                    KeyCode::ReturnKey => {
                        let title = std::mem::take(&mut self.rename_buffer);
                        self.renaming = None;
                        cx.widget_action(uid, FolderListViewAction::RenameCommitted(index, title));
                        self.view.redraw(cx);
                    }
                    KeyCode::Escape => {
                        self.renaming = None;
                        self.rename_buffer.clear();
                        self.view.redraw(cx);
                    }
                    KeyCode::Backspace => {
                        self.rename_buffer.pop();
                        self.view.redraw(cx);
                    }
                    _ => {}
                }
            }
            Hit::TextInput(ti) if self.has_key_focus && self.renaming.is_some() => {
                for ch in ti.input.chars() {
                    if !ch.is_control() {
                        self.rename_buffer.push(ch);
                    }
                }
                self.view.redraw(cx);
            }
            Hit::KeyDown(ke) if self.has_key_focus => {
                let Some(index) = self.focused else {
                    return;
                };
                match ke.key_code {
                    KeyCode::ArrowDown => {
                        if index + 1 < self.rows.len() {
                            self.focused = Some(index + 1);
                            self.view.redraw(cx);
                        }
                    }
                    KeyCode::ArrowUp => {
                        if index > 0 {
                            self.focused = Some(index - 1);
                            self.view.redraw(cx);
                        }
                    }
                    KeyCode::ReturnKey => {
                        cx.widget_action(uid, FolderListViewAction::EnterPressed(index));
                    }
                    KeyCode::Tab => {
                        let action = if ke.modifiers.shift {
                            FolderListViewAction::ShiftTabPressed(index)
                        } else {
                            FolderListViewAction::TabPressed(index)
                        };
                        cx.widget_action(uid, action);
                    }
                    KeyCode::F2 => {
                        if let Some(row) = self.rows.get(index) {
                            self.renaming = Some(index);
                            self.rename_buffer = row.label.clone();
                            self.view.redraw(cx);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        // Reorder drag (Task G4). A separate hit-test against the row list's
        // own area (not `self.view.area()` above) -- both can match the same
        // physical event, which is fine: a plain click (no move) both opens
        // via `FolderRow`'s own `Clicked` action AND arms/releases a drag
        // whose `drop_index` lands on-self, a no-op `reorder_row_op` refuses.
        // Every armed drag clears on `FingerUp` unconditionally
        // (correctness.md) -- an aborted drag (dropped past the last row, or
        // with a stale `row_rects`) still releases, it just computes a
        // `drop_index` that `reorder_row_op` then no-ops on.
        let rows_area = self.view.view(cx, ids!(rows_scroll.rows_list)).area();
        match event.hits(cx, rows_area) {
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                if let Some(index) = row_index_at(&self.row_rects, fe.abs.y) {
                    if self
                        .rows
                        .get(index)
                        .is_some_and(|row| !matches!(row.action, FolderRowAction::None))
                    {
                        self.dragging = Some(index);
                    }
                }
            }
            Hit::FingerMove(_) => {
                if self.dragging.is_some() {
                    // Ghost tracking is presentational only -- redraw so a
                    // future ghost overlay can read the live pointer
                    // position; no ghost is drawn yet (visual verification
                    // owed, see the plan's outstanding visual-verification
                    // table).
                    self.view.redraw(cx);
                }
            }
            Hit::FingerUp(fe) => {
                if let Some(from_index) = self.dragging.take() {
                    let drop_index = drop_index_from_pointer_y(&self.row_rects, fe.abs.y);
                    cx.widget_action(
                        uid,
                        FolderListViewAction::RowDropped(from_index, drop_index),
                    );
                    self.view.redraw(cx);
                }
            }
            _ => {}
        }
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.row_rects.clear();
        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut list) = item.as_flat_list().borrow_mut() {
                for (i, row_data) in self.rows.iter().enumerate() {
                    let item_id = LiveId::from_str(&format!("{i}:{}", row_data.label));
                    let row = list.item(cx, item_id, id!(Row)).unwrap();
                    let label_override =
                        (self.renaming == Some(i)).then_some(self.rename_buffer.as_str());
                    row.as_folder_row().set_row(
                        cx,
                        row_data,
                        self.focused == Some(i),
                        label_override,
                    );
                    row.draw_all(cx, &mut Scope::empty());
                    debug_assert_eq!(self.row_rects.len(), i, "rows draw in index order");
                    self.row_rects.push(row.area().rect(cx));
                }
            }
        }
        DrawStep::done()
    }
}

/// The row index whose recorded draw-time rect contains `y`, if any --
/// `Hit::FingerDown`'s arm-the-drag lookup. `None` when `y` falls in the
/// list's padding/gaps between rows, or `row_rects` is stale/empty (nothing
/// drawn yet this pass), which correctly refuses to arm a drag rather than
/// guessing a row.
fn row_index_at(row_rects: &[Rect], y: f64) -> Option<usize> {
    row_rects
        .iter()
        .position(|rect| y >= rect.pos.y && y < rect.pos.y + rect.size.y)
}

/// Turn a drop `y` position into a row index, in the SAME pre-drag `rows`
/// indexing `row_rects` was recorded in (Task G4). Compares against each
/// row's vertical MIDPOINT, not its top edge, so a drop anywhere in a row's
/// upper half lands before it and the lower half lands after -- the usual
/// drag-reorder feel. A `y` past every recorded rect's midpoint (including
/// an empty `row_rects`) lands at the end (`rects.len()`); `reorder_row_op`
/// is the one that turns an on-self or stale index into a no-op, so this
/// function never needs to special-case `from_index` itself.
fn drop_index_from_pointer_y(row_rects: &[Rect], y: f64) -> usize {
    for (i, rect) in row_rects.iter().enumerate() {
        let midpoint = rect.pos.y + rect.size.y * 0.5;
        if y < midpoint {
            return i;
        }
    }
    row_rects.len()
}

/// Map a `FlatList` row `item_id` back to its index in `rows`. Rows are keyed
/// `"{index}:{label}"` in the draw loop, so re-derive each candidate and
/// match. Pure, so the round-trip is unit-tested without a `Cx`.
fn row_index_for(rows: &[FolderRowView], item_id: LiveId) -> Option<usize> {
    rows.iter()
        .enumerate()
        .position(|(i, row)| LiveId::from_str(&format!("{i}:{}", row.label)) == item_id)
}

impl WidgetMatchEvent for FolderListView {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let uid = self.widget_uid();
        let list = self.view.flat_list(cx, ids!(rows_list));
        for (item_id, item) in list.items_with_actions(actions) {
            if item.as_folder_row().clicked(actions) {
                if let Some(i) = row_index_for(&self.rows, item_id) {
                    cx.widget_action(uid, FolderListViewAction::RowOpened(i));
                }
            }
        }
    }
}

impl FolderListView {
    /// Replace the rendered rows. `FolderView::sync` calls this with the
    /// resolved chain's projected rows, in order.
    pub fn set_rows(&mut self, cx: &mut Cx, rows: Vec<FolderRowView>) {
        self.rows = rows;
        self.focused = match self.focused {
            Some(index) if index < self.rows.len() => Some(index),
            _ if self.rows.is_empty() => None,
            _ => Some(0),
        };
        // A resync (e.g. after this same rename committed and reprojected
        // the chain) always drops an in-flight rename -- the row it was
        // editing may no longer exist at that index, or may no longer be
        // the same row, so there is nothing safe to resume.
        self.renaming = None;
        self.rename_buffer.clear();
        // Same reasoning as the rename buffer above: a resync may retarget
        // or remove the row a drag was armed on, so there is nothing safe to
        // continue dragging.
        self.dragging = None;
        self.view.redraw(cx);
    }

    /// Show or hide the chain-diagnostics strip. Empty `diagnostics` hides
    /// it -- the common case, a chain that built and ran clean -- otherwise
    /// every message joins into one line naming what degraded.
    pub fn set_diagnostics(&mut self, cx: &mut Cx, diagnostics: &[waml::diagnostic::Diagnostic]) {
        let degraded = !diagnostics.is_empty();
        self.view
            .view(cx, ids!(diag_strip))
            .set_visible(cx, degraded);
        if degraded {
            let text = diagnostics
                .iter()
                .map(|diag| diag.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            self.view
                .label(cx, ids!(diag_strip.diag_label))
                .set_text(cx, &text);
        }
    }

    /// The row index that was clicked in `actions`, if any. `FolderView::handle`
    /// maps it back to a navigation target via `action_for`/`navigation_for`.
    pub fn row_opened(&self, actions: &Actions) -> Option<usize> {
        match self.actions_action(actions) {
            FolderListViewAction::RowOpened(i) => Some(i),
            FolderListViewAction::None
            | FolderListViewAction::EnterPressed(_)
            | FolderListViewAction::TabPressed(_)
            | FolderListViewAction::ShiftTabPressed(_)
            | FolderListViewAction::RenameCommitted(_, _)
            | FolderListViewAction::RowDropped(_, _) => None,
        }
    }

    /// The row index Enter was pressed on (Task G3), if any this pass.
    /// `FolderView::handle` maps it through `enter_row_op`.
    pub fn enter_pressed(&self, actions: &Actions) -> Option<usize> {
        match self.actions_action(actions) {
            FolderListViewAction::EnterPressed(i) => Some(i),
            _ => None,
        }
    }

    /// The row index Tab (no shift) was pressed on, if any this pass.
    /// `FolderView::handle` maps it through `tab_row_op`.
    pub fn tab_pressed(&self, actions: &Actions) -> Option<usize> {
        match self.actions_action(actions) {
            FolderListViewAction::TabPressed(i) => Some(i),
            _ => None,
        }
    }

    /// The row index Shift-Tab was pressed on, if any this pass.
    /// `FolderView::handle` maps it through `shift_tab_row_op`.
    pub fn shift_tab_pressed(&self, actions: &Actions) -> Option<usize> {
        match self.actions_action(actions) {
            FolderListViewAction::ShiftTabPressed(i) => Some(i),
            _ => None,
        }
    }

    /// The `(index, title)` a rename committed on, if any this pass.
    /// `FolderView::handle` maps it through `rename_row_op`.
    pub fn rename_committed(&self, actions: &Actions) -> Option<(usize, String)> {
        match self.actions_action(actions) {
            FolderListViewAction::RenameCommitted(i, title) => Some((i, title)),
            _ => None,
        }
    }

    /// The `(from_index, drop_index)` a reorder drag released on, if any
    /// this pass (Task G4). `FolderView::handle` maps it through
    /// `reorder_row_op`.
    pub fn row_dropped(&self, actions: &Actions) -> Option<(usize, usize)> {
        match self.actions_action(actions) {
            FolderListViewAction::RowDropped(from_index, drop_index) => {
                Some((from_index, drop_index))
            }
            _ => None,
        }
    }

    fn actions_action(&self, actions: &Actions) -> FolderListViewAction {
        actions
            .find_widget_action(self.widget_uid())
            .map(|item| item.cast())
            .unwrap_or_default()
    }
}

impl FolderListViewRef {
    pub fn set_rows(&self, cx: &mut Cx, rows: Vec<FolderRowView>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_rows(cx, rows);
        }
    }
    pub fn set_diagnostics(&self, cx: &mut Cx, diagnostics: &[waml::diagnostic::Diagnostic]) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_diagnostics(cx, diagnostics);
        }
    }
    pub fn row_opened(&self, actions: &Actions) -> Option<usize> {
        self.borrow().and_then(|inner| inner.row_opened(actions))
    }
    pub fn enter_pressed(&self, actions: &Actions) -> Option<usize> {
        self.borrow().and_then(|inner| inner.enter_pressed(actions))
    }
    pub fn tab_pressed(&self, actions: &Actions) -> Option<usize> {
        self.borrow().and_then(|inner| inner.tab_pressed(actions))
    }
    pub fn shift_tab_pressed(&self, actions: &Actions) -> Option<usize> {
        self.borrow()
            .and_then(|inner| inner.shift_tab_pressed(actions))
    }
    pub fn rename_committed(&self, actions: &Actions) -> Option<(usize, String)> {
        self.borrow()
            .and_then(|inner| inner.rename_committed(actions))
    }
    pub fn row_dropped(&self, actions: &Actions) -> Option<(usize, usize)> {
        self.borrow().and_then(|inner| inner.row_dropped(actions))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(label: &str, action: FolderRowAction) -> FolderRowView {
        FolderRowView {
            icon: crate::icons::Icon::FileText,
            label: label.to_string(),
            blurb: None,
            action,
        }
    }

    #[test]
    fn folder_row_action_default_is_none() {
        assert!(matches!(
            FolderRowClickAction::default(),
            FolderRowClickAction::None
        ));
    }

    #[test]
    fn folder_list_action_default_is_none() {
        assert!(matches!(
            FolderListViewAction::default(),
            FolderListViewAction::None
        ));
    }

    #[test]
    fn row_index_round_trips_through_item_id() {
        let rows = vec![
            row("Orders", FolderRowAction::OpenConcept("orders".into())),
            row("Sales", FolderRowAction::OpenFolder("/sales".into())),
        ];
        for (i, r) in rows.iter().enumerate() {
            let item_id = LiveId::from_str(&format!("{i}:{}", r.label));
            assert_eq!(row_index_for(&rows, item_id), Some(i));
        }
        assert_eq!(row_index_for(&rows, LiveId::from_str("unknown")), None);
    }

    /// Three 20px-tall rows stacked with no gaps, starting at y = 0.
    fn three_rows() -> Vec<Rect> {
        vec![
            Rect {
                pos: dvec2(0.0, 0.0),
                size: dvec2(100.0, 20.0),
            },
            Rect {
                pos: dvec2(0.0, 20.0),
                size: dvec2(100.0, 20.0),
            },
            Rect {
                pos: dvec2(0.0, 40.0),
                size: dvec2(100.0, 20.0),
            },
        ]
    }

    #[test]
    fn drop_index_from_pointer_y_is_correct_at_boundaries() {
        let rects = three_rows();
        // First row's upper half: before row 0.
        assert_eq!(drop_index_from_pointer_y(&rects, 0.0), 0);
        assert_eq!(drop_index_from_pointer_y(&rects, 9.0), 0);
        // Crossing row 0's midpoint (y = 10): lands before row 1.
        assert_eq!(drop_index_from_pointer_y(&rects, 10.0), 1);
        // Between rows, at row 1's midpoint (y = 30): before row 2.
        assert_eq!(drop_index_from_pointer_y(&rects, 30.0), 2);
        // Past every row's midpoint, including past the last rect entirely:
        // lands at the end.
        assert_eq!(drop_index_from_pointer_y(&rects, 50.0), 3);
        assert_eq!(drop_index_from_pointer_y(&rects, 1_000.0), 3);
        // Empty rects: nothing recorded yet, lands at the (empty) end.
        assert_eq!(drop_index_from_pointer_y(&[], 5.0), 0);
    }

    #[test]
    fn row_index_at_finds_the_containing_row_and_refuses_the_gaps() {
        let rects = three_rows();
        assert_eq!(row_index_at(&rects, 0.0), Some(0));
        assert_eq!(row_index_at(&rects, 19.9), Some(0));
        assert_eq!(row_index_at(&rects, 20.0), Some(1));
        assert_eq!(row_index_at(&rects, 45.0), Some(2));
        // Past the last row, and an empty list: neither is inside any row.
        assert_eq!(row_index_at(&rects, 60.0), None);
        assert_eq!(row_index_at(&[], 0.0), None);
    }
}
