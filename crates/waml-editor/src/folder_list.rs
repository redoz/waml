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

        bullet := Label {
            width: Fit
            text: "\u{2022}"
            draw_text +: {
                color: atlas.text_dim
                text_style: fonts.text_label
            }
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
        // Raw-mode banner (Task D3, "The raw OKF layer"): shown instead of
        // `raw_link` once the folder is actually open in raw mode, so the
        // user cannot mistake the raw listing for the folder's configured
        // view. Presentational only -- no access check backs it.
        raw_banner := View {
            width: Fill
            height: Fit
            visible: false
            padding: Inset{left: 24.0, right: 24.0, top: 10.0, bottom: 10.0}
            show_bg: true
            draw_bg +: {
                color: atlas.text_dim
                pixel: fn() {
                    return vec4(self.color.x, self.color.y, self.color.z, 0.12)
                }
            }
            raw_banner_label := Label {
                width: Fill
                text: "Raw listing \u{2014} the folder's declared view is bypassed"
                draw_text +: {
                    color: atlas.text
                    text_style: fonts.text_label
                }
            }
        }
        // The open-raw affordance: always available on a folder's declared
        // view, hidden once that view IS the raw listing (`raw_banner`
        // covers that case instead).
        raw_link := View {
            width: Fill
            height: Fit
            padding: Inset{left: 24.0, right: 24.0, top: 4.0, bottom: 4.0}
            raw_link_label := Label {
                width: Fit
                text: "View raw"
                draw_text +: {
                    color: atlas.text_dim
                    text_style: fonts.text_micro
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
        self.view.draw_walk(cx, scope, walk)
    }
}

impl FolderRow {
    /// Bind one projected row's view-model onto this widget instance.
    /// `focused` is `FolderListView`'s keyboard-focused row index (Task G3),
    /// compared by the caller -- presentational only, mirrors `hovered`'s
    /// wash so the focused row stays visibly distinct without a click.
    pub fn set_row(&mut self, cx: &mut Cx, row: &FolderRowView, focused: bool) {
        self.view.label(cx, ids!(bullet)).set_text(cx, row.bullet);
        self.view
            .label(cx, ids!(textcol.label))
            .set_text(cx, &row.label);
        let blurb = row.blurb.as_deref().unwrap_or("");
        self.view.label(cx, ids!(textcol.blurb)).set_text(cx, blurb);
        self.view
            .view(cx, ids!(textcol.blurb))
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
    pub fn set_row(&self, cx: &mut Cx, row: &FolderRowView, focused: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_row(cx, row, focused);
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
    /// The "View raw" affordance was clicked (Task D3). Never fired while
    /// the view is already raw -- `raw_link` is hidden in that state.
    RawRequested,
    /// `KeyCode::ReturnKey` fired while `index` held keyboard focus (Task
    /// G3). `FolderView::handle` maps this through `enter_row_op`.
    EnterPressed(usize),
    /// `KeyCode::Tab` (no shift) fired while `index` held keyboard focus.
    /// Maps through `tab_row_op`.
    TabPressed(usize),
    /// `KeyCode::Tab` with shift held. Maps through `shift_tab_row_op`.
    ShiftTabPressed(usize),
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
}

impl Widget for FolderListView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let uid = self.widget_uid();
        let raw_link_area = self.view.view(cx, ids!(raw_link)).area();
        match event.hits(cx, raw_link_area) {
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over => {
                cx.widget_action(uid, FolderListViewAction::RawRequested);
            }
            Hit::FingerHoverIn(_) => {
                cursor::hover_in(cx, MouseCursor::Hand);
            }
            Hit::FingerHoverOut(_) => {
                cursor::hover_out(cx);
            }
            _ => {}
        }
        match event.hits(cx, self.view.area()) {
            Hit::FingerDown(_) => {
                cx.set_key_focus(self.view.area());
            }
            Hit::KeyFocus(_) => {
                self.has_key_focus = true;
            }
            Hit::KeyFocusLost(_) => {
                self.has_key_focus = false;
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
                    _ => {}
                }
            }
            _ => {}
        }
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut list) = item.as_flat_list().borrow_mut() {
                for (i, row_data) in self.rows.iter().enumerate() {
                    let item_id = LiveId::from_str(&format!("{i}:{}", row_data.label));
                    let row = list.item(cx, item_id, id!(Row)).unwrap();
                    row.as_folder_row()
                        .set_row(cx, row_data, self.focused == Some(i));
                    row.draw_all(cx, &mut Scope::empty());
                }
            }
        }
        DrawStep::done()
    }
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
            | FolderListViewAction::RawRequested
            | FolderListViewAction::EnterPressed(_)
            | FolderListViewAction::TabPressed(_)
            | FolderListViewAction::ShiftTabPressed(_) => None,
        }
    }

    /// Whether the "View raw" affordance was clicked (Task D3). `FolderView::handle`
    /// maps a hit into a `DirectoryRaw` navigation target for its own directory.
    pub fn raw_requested(&self, actions: &Actions) -> bool {
        matches!(
            self.actions_action(actions),
            FolderListViewAction::RawRequested
        )
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

    /// Toggle between the "View raw" affordance and the raw-mode banner:
    /// `raw == true` means this view IS the raw listing already, so the
    /// affordance to open it again is hidden and the banner shown instead.
    pub fn set_raw(&mut self, cx: &mut Cx, raw: bool) {
        self.view.view(cx, ids!(raw_banner)).set_visible(cx, raw);
        self.view.view(cx, ids!(raw_link)).set_visible(cx, !raw);
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
    pub fn raw_requested(&self, actions: &Actions) -> bool {
        self.borrow()
            .is_some_and(|inner| inner.raw_requested(actions))
    }
    pub fn set_raw(&self, cx: &mut Cx, raw: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_raw(cx, raw);
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(label: &str, action: FolderRowAction) -> FolderRowView {
        FolderRowView {
            bullet: "\u{2022}",
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
}
