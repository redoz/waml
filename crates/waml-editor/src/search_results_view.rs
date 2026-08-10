//! Results tab: the grouped/collapsible view of a search's hits
//! (Task 8, spec 2026-08-09-bundle-search-design.md §Results tab).
//!
//! `group_hits` is the pure grouping model, unit-tested without a `Cx`.
//! `SearchResultsView` is the `DocView` a results tab opens with (wired to
//! the open path, activation, and reveal in Task 9). `SearchResultsListView`
//! (declared below, in the same file per the plan's file structure) is the
//! body-surface widget it draws through -- modeled on `folder_list.rs`'s
//! `FolderListView`, but with two row templates (a collapsible group header
//! plus a result row) instead of one.

use makepad_widgets::*;

use waml::search::{FieldGroup, Hit as SearchHit, HitTarget, Snippet};

use crate::cursor;
use crate::doc_view::{BodyChrome, BodyWidgets, DocView, DocViewIdentity, ViewData, ViewOutcome};
use crate::icons::{Icon, IconSet};

/// What a result row's match is shown as, next to the snippet (spec
/// §Results tab row shapes). `Line` carries the 1-based source line a prose
/// hit landed on.
///
/// Constructed by `label_for`, which `ResultRow`-building code calls once
/// `SearchResultsView::new` has a producer (Task 9, `App::open_search_results`).
/// This task's own unit tests already exercise every variant.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowLabel {
    Name,
    Rel,
    Doc,
    Id,
    Link,
    Line(u32),
}

/// One search hit, ready for display in a document group.
#[derive(Clone, Debug, PartialEq)]
pub struct ResultRow {
    pub hit: SearchHit,
    pub label: RowLabel,
    pub snippet: Snippet,
    /// Hidden by the active projection -- muted + badge (Task 9 fills this
    /// in from `SearchState::hidden_documents`; `Task 8` carries the field).
    pub hidden: bool,
}

/// Every hit for one document, in rank order.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentGroup {
    /// The document's filename, e.g. "billing.waml".
    pub path: String,
    /// The document's containing directory, e.g. "domain/" (empty at the
    /// bundle root).
    pub directory: String,
    pub collapsed: bool,
    pub rows: Vec<ResultRow>,
}

/// Unused outside tests until `group_hits` gets a non-test caller (Task 9).
#[allow(dead_code)]
fn split_document_path(document: &str) -> (String, String) {
    match document.rfind('/') {
        Some(i) => (document[i + 1..].to_string(), document[..=i].to_string()),
        None => (document.to_string(), String::new()),
    }
}

/// `FieldGroup`/`HitTarget` -> the badge a result row shows next to its
/// snippet (spec §Results tab row shapes, tested by Task 8's Step 1 suite).
/// Names land on a model element (a classifier/document name) -> `Name`;
/// a `Names` hit on a raw span (a heading) is the document's own title ->
/// `Doc`. `Model` hits (kind/relationship/tag) -> `Rel`. `Prose` hits always
/// carry a `TextSpan` (body text is never a model element) -> `Line`.
/// `Structure` hits split on target shape too: an `id:` value resolves to
/// the element it names (`ModelElement`) -> `Id`; a frontmatter key or link
/// target is raw source text (`TextSpan`) -> `Link`.
#[allow(dead_code)]
pub fn label_for(hit: &SearchHit) -> RowLabel {
    match hit.group {
        FieldGroup::Names => match hit.target {
            HitTarget::ModelElement { .. } => RowLabel::Name,
            HitTarget::TextSpan { .. } => RowLabel::Doc,
        },
        FieldGroup::Model => RowLabel::Rel,
        FieldGroup::Prose => match hit.target {
            HitTarget::TextSpan { line, .. } => RowLabel::Line(line),
            HitTarget::ModelElement { .. } => RowLabel::Name,
        },
        FieldGroup::Structure => match hit.target {
            HitTarget::ModelElement { .. } => RowLabel::Id,
            HitTarget::TextSpan { .. } => RowLabel::Link,
        },
    }
}

/// Group `rows` by document, groups ordered by their best hit (the order the
/// first row for that document appears in `rows`, which callers hand in
/// already rank-ordered), rows within a group kept in rank order (spec
/// §Results tab). Unused outside tests until `SearchResultsView::new` gets a
/// non-test caller (Task 9, `App::open_search_results`).
#[allow(dead_code)]
pub fn group_hits(rows: Vec<ResultRow>) -> Vec<DocumentGroup> {
    let mut groups: Vec<DocumentGroup> = Vec::new();
    let mut index_by_document: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for row in rows {
        let document = row.hit.document.clone();
        let index = *index_by_document
            .entry(document.clone())
            .or_insert_with(|| {
                let (path, directory) = split_document_path(&document);
                groups.push(DocumentGroup {
                    path,
                    directory,
                    collapsed: false,
                    rows: Vec::new(),
                });
                groups.len() - 1
            });
        groups[index].rows.push(row);
    }
    groups
}

fn tag_text(label: RowLabel) -> String {
    match label {
        RowLabel::Name => "NAME".to_string(),
        RowLabel::Rel => "REL".to_string(),
        RowLabel::Doc => "DOC".to_string(),
        RowLabel::Id => "ID".to_string(),
        RowLabel::Link => "LINK".to_string(),
        RowLabel::Line(n) => format!("L{n}"),
    }
}

/// The results-tab `DocView`: a query plus its grouped hits. Opened by
/// `App::open_search_results` (Task 9); `Task 8` only builds and unit-tests
/// the type itself, and registers its body surface in the live design.
pub struct SearchResultsView {
    pub query: String,
    groups: Vec<DocumentGroup>,
    /// F3 traversal marks the current row here (Task 14).
    #[allow(dead_code)]
    cursor: Option<(usize, usize)>,
}

impl SearchResultsView {
    /// Unused outside tests until `App::open_search_results` calls it
    /// (Task 9).
    #[allow(dead_code)]
    pub fn new(query: String, rows: Vec<ResultRow>) -> Self {
        SearchResultsView {
            query,
            groups: group_hits(rows),
            cursor: None,
        }
    }

    /// `(hits, documents, hidden)` -- counts every row, including rows inside
    /// a collapsed group (a collapsed group still counts, spec §States).
    pub fn counts(&self) -> (usize, usize, usize) {
        let hits = self.groups.iter().map(|group| group.rows.len()).sum();
        let documents = self.groups.len();
        let hidden = self
            .groups
            .iter()
            .flat_map(|group| &group.rows)
            .filter(|row| row.hidden)
            .count();
        (hits, documents, hidden)
    }

    /// `🔍 {query}    {hits} in {documents} documents[, {hidden} hidden]`
    /// (spec §States, hidden-only results).
    pub fn header_text(&self) -> String {
        let (hits, documents, hidden) = self.counts();
        let mut text = format!(
            "\u{1F50D} {}    {hits} in {documents} documents",
            self.query
        );
        if hidden > 0 {
            text.push_str(&format!(", {hidden} hidden"));
        }
        text
    }
}

impl DocView for SearchResultsView {
    fn identity(&self) -> DocViewIdentity {
        DocViewIdentity::SearchResults
    }

    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, _data: ViewData<'_>) {
        body.show_search_results(cx);
        let header = self.header_text();
        body.search_results()
            .set_groups(cx, &header, self.groups.clone());
    }

    fn handle(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        _data: ViewData<'_>,
    ) -> ViewOutcome {
        let outcome = ViewOutcome::default();
        if let Some(index) = body.search_results().header_toggled(actions) {
            if let Some(group) = self.groups.get_mut(index) {
                group.collapsed = !group.collapsed;
                let header = self.header_text();
                body.search_results()
                    .set_groups(cx, &header, self.groups.clone());
            }
        }
        // Row activation -> `ViewOutcome.navigation` (plus the `reveal` field
        // it needs to carry a hit's span/model element) lands in Task 9,
        // which is also what wires this view into the document open path.
        outcome
    }

    fn chrome(&self) -> BodyChrome {
        BodyChrome::HIDDEN
    }
}

// ---------------------------------------------------------------------
// Widget: the body surface `SearchResultsView::sync` draws through.
// ---------------------------------------------------------------------

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.SearchGroupHeaderBase = #(SearchGroupHeader::register_widget(vm))

    mod.widgets.SearchGroupHeader = set_type_default() do mod.widgets.SearchGroupHeaderBase{
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        padding: Inset{left: 4.0, right: 4.0, top: 8.0, bottom: 4.0}
        spacing: 8.0
        show_bg: true
        draw_bg +: {
            color: atlas.accent
            hover: uniform(0.0)
            pixel: fn() {
                let a = 0.06 + 0.06 * self.hover
                return vec4(self.color.x * a, self.color.y * a, self.color.z * a, a)
            }
        }
        // Colour-only holder (never drawn): the immediate-mode chevron glyph
        // copies `color` from this per draw, the `FolderRow` pattern.
        draw_icon +: { color: atlas.text_dim }
        icon_anchor := View {
            width: 16.0
            height: 16.0
        }
        textcol := View {
            width: Fill
            height: Fit
            flow: Down
            spacing: 1.0
            path_label := Label {
                width: Fill
                text: ""
                draw_text +: {
                    color: atlas.text
                    text_style: fonts.text_label
                }
            }
            dir_label := Label {
                width: Fill
                text: ""
                draw_text +: {
                    color: atlas.text_dim
                    text_style: fonts.text_micro
                }
            }
        }
        count_label := Label {
            text: ""
            draw_text +: {
                color: atlas.text_dim
                text_style: fonts.text_micro
            }
        }
    }

    mod.widgets.SearchResultRowBase = #(SearchResultRow::register_widget(vm))

    mod.widgets.SearchResultRow = set_type_default() do mod.widgets.SearchResultRowBase{
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.0}
        padding: Inset{left: 20.0, right: 4.0, top: 4.0, bottom: 4.0}
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
        tag_label := Label {
            width: 48.0
            text: ""
            draw_text +: {
                color: atlas.text_dim
                text_style: fonts.text_micro
            }
        }
        textcol := View {
            width: Fill
            height: Fit
            flow: Down
            spacing: 2.0
            snippet_label := Label {
                width: Fill
                text: ""
                draw_text +: {
                    color: atlas.text
                    text_style: fonts.text_label
                }
            }
            detail_label := Label {
                width: Fill
                text: ""
                draw_text +: {
                    color: atlas.text_dim
                    text_style: fonts.text_micro
                }
            }
        }
        hidden_badge := Label {
            text: "hidden"
            visible: false
            draw_text +: {
                color: atlas.danger
                text_style: fonts.text_micro
            }
        }
    }

    mod.widgets.SearchResultsListViewBase = #(SearchResultsListView::register_widget(vm))

    mod.widgets.SearchResultsListView = set_type_default() do mod.widgets.SearchResultsListViewBase{
        width: Fill
        height: Fill
        flow: Down
        header_label := Label {
            width: Fill
            text: ""
            padding: Inset{left: 24.0, right: 24.0, top: 12.0, bottom: 8.0}
            draw_text +: {
                color: atlas.text
                text_style: fonts.text_label
            }
        }
        rows_scroll := ScrollYView {
            width: Fill
            height: Fill
            padding: Inset{left: 24.0, right: 24.0, top: 0.0, bottom: 24.0}
            rows_list := FlatList {
                width: Fill
                height: Fill
                flow: Down
                spacing: 2.0
                Header := mod.widgets.SearchGroupHeader { }
                Row := mod.widgets.SearchResultRow { }
            }
        }
    }
}

/// Emitted when a group header is clicked.
#[derive(Clone, Debug, Default)]
pub enum SearchGroupHeaderClickAction {
    #[default]
    None,
    Clicked,
}

#[derive(Script, ScriptHook, Widget)]
pub struct SearchGroupHeader {
    #[deref]
    view: View,
    #[live]
    icons: IconSet,
    #[live]
    draw_icon: DrawColor,
    #[rust]
    icon: Icon,
    #[rust]
    icon_rect: Rect,
    #[rust]
    hovered: bool,
}

impl Widget for SearchGroupHeader {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        let uid = self.widget_uid();
        match event.hits(cx, self.view.area()) {
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over => {
                cx.widget_action(uid, SearchGroupHeaderClickAction::Clicked);
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
        self.view
            .draw_bg
            .set_uniform(cx, live_id!(hover), &[if self.hovered { 1.0 } else { 0.0 }]);
        let step = self.view.draw_walk(cx, scope, walk);
        self.icon_rect = self.view.view(cx, ids!(icon_anchor)).area().rect(cx);
        self.icons
            .draw(cx, self.icon, self.icon_rect, self.draw_icon.color);
        step
    }
}

impl SearchGroupHeader {
    pub fn set_group(&mut self, cx: &mut Cx, group: &DocumentGroup) {
        self.icon = if group.collapsed {
            Icon::ArrowRight
        } else {
            Icon::ArrowDown
        };
        self.view
            .label(cx, ids!(textcol.path_label))
            .set_text(cx, &group.path);
        self.view
            .label(cx, ids!(textcol.dir_label))
            .set_text(cx, &group.directory);
        self.view
            .label(cx, ids!(count_label))
            .set_text(cx, &group.rows.len().to_string());
    }

    pub fn clicked(&self, actions: &Actions) -> bool {
        actions
            .find_widget_action(self.widget_uid())
            .is_some_and(|a| matches!(a.cast(), SearchGroupHeaderClickAction::Clicked))
    }
}

impl SearchGroupHeaderRef {
    pub fn set_group(&self, cx: &mut Cx, group: &DocumentGroup) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_group(cx, group);
        }
    }
    pub fn clicked(&self, actions: &Actions) -> bool {
        self.borrow().is_some_and(|inner| inner.clicked(actions))
    }
}

/// Emitted when a result row is clicked.
#[derive(Clone, Debug, Default)]
pub enum SearchResultRowClickAction {
    #[default]
    None,
    Clicked,
}

#[derive(Script, ScriptHook, Widget)]
pub struct SearchResultRow {
    #[deref]
    view: View,
    #[rust]
    hovered: bool,
}

impl Widget for SearchResultRow {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        let uid = self.widget_uid();
        match event.hits(cx, self.view.area()) {
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over => {
                cx.widget_action(uid, SearchResultRowClickAction::Clicked);
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
        self.view
            .draw_bg
            .set_uniform(cx, live_id!(hover), &[if self.hovered { 1.0 } else { 0.0 }]);
        self.view.draw_walk(cx, scope, walk)
    }
}

impl SearchResultRow {
    pub fn set_row(&mut self, cx: &mut Cx, row: &ResultRow) {
        self.view
            .label(cx, ids!(tag_label))
            .set_text(cx, &tag_text(row.label));
        self.view
            .label(cx, ids!(textcol.snippet_label))
            .set_text(cx, &row.snippet.text);
        self.view
            .label(cx, ids!(textcol.detail_label))
            .set_text(cx, &row.hit.document);
        self.view
            .label(cx, ids!(hidden_badge))
            .set_visible(cx, row.hidden);
    }

    pub fn clicked(&self, actions: &Actions) -> bool {
        actions
            .find_widget_action(self.widget_uid())
            .is_some_and(|a| matches!(a.cast(), SearchResultRowClickAction::Clicked))
    }
}

impl SearchResultRowRef {
    pub fn set_row(&self, cx: &mut Cx, row: &ResultRow) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_row(cx, row);
        }
    }
    pub fn clicked(&self, actions: &Actions) -> bool {
        self.borrow().is_some_and(|inner| inner.clicked(actions))
    }
}

/// One flattened list item: either a group's header row, or one of its
/// result rows (only emitted for an expanded group).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListItemKind {
    Header(usize),
    Row(usize, usize),
}

fn flattened_items(groups: &[DocumentGroup]) -> Vec<(LiveId, ListItemKind)> {
    let mut items = Vec::new();
    for (group_index, group) in groups.iter().enumerate() {
        items.push((
            LiveId::from_str(&format!("h{group_index}")),
            ListItemKind::Header(group_index),
        ));
        if !group.collapsed {
            for row_index in 0..group.rows.len() {
                items.push((
                    LiveId::from_str(&format!("r{group_index}:{row_index}")),
                    ListItemKind::Row(group_index, row_index),
                ));
            }
        }
    }
    items
}

fn header_index_for(groups: &[DocumentGroup], item_id: LiveId) -> Option<usize> {
    (0..groups.len()).find(|&index| LiveId::from_str(&format!("h{index}")) == item_id)
}

fn row_index_for(groups: &[DocumentGroup], item_id: LiveId) -> Option<(usize, usize)> {
    for (group_index, group) in groups.iter().enumerate() {
        for row_index in 0..group.rows.len() {
            if LiveId::from_str(&format!("r{group_index}:{row_index}")) == item_id {
                return Some((group_index, row_index));
            }
        }
    }
    None
}

/// Emitted by `SearchResultsListView`, indexing the groups last passed to
/// `set_groups`.
#[derive(Clone, Debug, Default)]
pub enum SearchResultsListAction {
    #[default]
    None,
    /// `(group_index, row_index)` of the clicked result row. Unused outside
    /// tests until `SearchResultsView::handle` routes it to a navigation
    /// outcome (Task 9).
    #[allow(dead_code)]
    RowOpened(usize, usize),
    /// The group index whose header was clicked.
    HeaderToggled(usize),
}

#[derive(Script, ScriptHook, Widget)]
pub struct SearchResultsListView {
    #[deref]
    view: View,
    #[rust]
    groups: Vec<DocumentGroup>,
}

impl Widget for SearchResultsListView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let items = flattened_items(&self.groups);
        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut list) = item.as_flat_list().borrow_mut() {
                for (item_id, kind) in &items {
                    match *kind {
                        ListItemKind::Header(group_index) => {
                            let row = list.item(cx, *item_id, id!(Header)).unwrap();
                            row.as_search_group_header()
                                .set_group(cx, &self.groups[group_index]);
                            row.draw_all(cx, &mut Scope::empty());
                        }
                        ListItemKind::Row(group_index, row_index) => {
                            let row = list.item(cx, *item_id, id!(Row)).unwrap();
                            row.as_search_result_row()
                                .set_row(cx, &self.groups[group_index].rows[row_index]);
                            row.draw_all(cx, &mut Scope::empty());
                        }
                    }
                }
            }
        }
        DrawStep::done()
    }
}

impl WidgetMatchEvent for SearchResultsListView {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let uid = self.widget_uid();
        let list = self.view.flat_list(cx, ids!(rows_list));
        for (item_id, item) in list.items_with_actions(actions) {
            if item.as_search_group_header().clicked(actions) {
                if let Some(index) = header_index_for(&self.groups, item_id) {
                    cx.widget_action(uid, SearchResultsListAction::HeaderToggled(index));
                }
            } else if item.as_search_result_row().clicked(actions) {
                if let Some((group_index, row_index)) = row_index_for(&self.groups, item_id) {
                    cx.widget_action(
                        uid,
                        SearchResultsListAction::RowOpened(group_index, row_index),
                    );
                }
            }
        }
    }
}

impl SearchResultsListView {
    /// Replace the rendered groups and header line. `SearchResultsView::sync`
    /// calls this with the query's resolved groups, in order.
    pub fn set_groups(&mut self, cx: &mut Cx, header_text: &str, groups: Vec<DocumentGroup>) {
        self.groups = groups;
        self.view
            .label(cx, ids!(header_label))
            .set_text(cx, header_text);
        self.view.redraw(cx);
    }

    /// `(group_index, row_index)` of the row clicked in `actions`, if any.
    /// Unused outside tests until `SearchResultsView::handle` calls it
    /// (Task 9).
    #[allow(dead_code)]
    pub fn row_opened(&self, actions: &Actions) -> Option<(usize, usize)> {
        match self.actions_action(actions) {
            SearchResultsListAction::RowOpened(group_index, row_index) => {
                Some((group_index, row_index))
            }
            SearchResultsListAction::None | SearchResultsListAction::HeaderToggled(_) => None,
        }
    }

    /// The group index whose header was toggled in `actions`, if any.
    pub fn header_toggled(&self, actions: &Actions) -> Option<usize> {
        match self.actions_action(actions) {
            SearchResultsListAction::HeaderToggled(index) => Some(index),
            _ => None,
        }
    }

    fn actions_action(&self, actions: &Actions) -> SearchResultsListAction {
        actions
            .find_widget_action(self.widget_uid())
            .map(|item| item.cast())
            .unwrap_or_default()
    }
}

impl SearchResultsListViewRef {
    pub fn set_groups(&self, cx: &mut Cx, header_text: &str, groups: Vec<DocumentGroup>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_groups(cx, header_text, groups);
        }
    }
    /// Unused outside tests until `SearchResultsView::handle` calls it
    /// (Task 9).
    #[allow(dead_code)]
    pub fn row_opened(&self, actions: &Actions) -> Option<(usize, usize)> {
        self.borrow().and_then(|inner| inner.row_opened(actions))
    }
    pub fn header_toggled(&self, actions: &Actions) -> Option<usize> {
        self.borrow()
            .and_then(|inner| inner.header_toggled(actions))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(document: &str, group: FieldGroup, target: HitTarget) -> SearchHit {
        SearchHit {
            document: document.to_string(),
            concept_id: None,
            group,
            target,
            score: 1.0,
        }
    }

    fn row(document: &str, group: FieldGroup, target: HitTarget) -> ResultRow {
        let hit = hit(document, group, target.clone());
        ResultRow {
            label: label_for(&hit),
            hit,
            snippet: Snippet {
                text: String::new(),
                highlights: Vec::new(),
            },
            hidden: false,
        }
    }

    fn model(key: &str) -> HitTarget {
        HitTarget::ModelElement {
            key: key.to_string(),
        }
    }

    fn span(line: u32) -> HitTarget {
        HitTarget::TextSpan {
            start: 0,
            end: 1,
            line,
        }
    }

    #[test]
    fn label_for_maps_every_field_group_and_target_shape() {
        assert_eq!(
            label_for(&hit("a.md", FieldGroup::Names, model("payment"))),
            RowLabel::Name
        );
        assert_eq!(
            label_for(&hit("a.md", FieldGroup::Names, span(1))),
            RowLabel::Doc
        );
        assert_eq!(
            label_for(&hit("a.md", FieldGroup::Model, model("payment"))),
            RowLabel::Rel
        );
        assert_eq!(
            label_for(&hit("a.md", FieldGroup::Prose, span(12))),
            RowLabel::Line(12)
        );
        assert_eq!(
            label_for(&hit("a.md", FieldGroup::Structure, model("payment"))),
            RowLabel::Id
        );
        assert_eq!(
            label_for(&hit("a.md", FieldGroup::Structure, span(3))),
            RowLabel::Link
        );
    }

    #[test]
    fn group_hits_groups_by_document_and_orders_groups_by_first_appearance() {
        let rows = vec![
            row("b/two.md", FieldGroup::Names, model("two")),
            row("a/one.md", FieldGroup::Names, model("one")),
            row("b/two.md", FieldGroup::Prose, span(4)),
            row("a/one.md", FieldGroup::Prose, span(9)),
        ];
        let groups = group_hits(rows);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].path, "two.md");
        assert_eq!(groups[0].directory, "b/");
        assert_eq!(groups[1].path, "one.md");
        assert_eq!(groups[1].directory, "a/");
        // Rank order within a group is preserved (Names hit before the Prose
        // hit for the same document, as handed in).
        assert_eq!(groups[0].rows.len(), 2);
        assert_eq!(groups[0].rows[0].label, RowLabel::Name);
        assert_eq!(groups[0].rows[1].label, RowLabel::Line(4));
    }

    #[test]
    fn group_hits_at_the_bundle_root_has_no_directory() {
        let rows = vec![row("root.md", FieldGroup::Names, model("root"))];
        let groups = group_hits(rows);
        assert_eq!(groups[0].path, "root.md");
        assert_eq!(groups[0].directory, "");
    }

    #[test]
    fn counts_reports_hits_documents_and_hidden_including_collapsed_groups() {
        let mut rows = vec![
            row("a.md", FieldGroup::Names, model("a")),
            row("a.md", FieldGroup::Prose, span(1)),
            row("b.md", FieldGroup::Names, model("b")),
        ];
        rows[1].hidden = true;
        let mut view = SearchResultsView::new("q".to_string(), rows);
        assert_eq!(view.counts(), (3, 2, 1));
        // Collapsing a group still counts its rows.
        view.groups[0].collapsed = true;
        assert_eq!(view.counts(), (3, 2, 1));
    }

    #[test]
    fn header_text_reports_the_query_and_appends_hidden_only_when_nonzero() {
        let rows = vec![row("a.md", FieldGroup::Names, model("a"))];
        let view = SearchResultsView::new("payment".to_string(), rows);
        assert_eq!(view.header_text(), "\u{1F50D} payment    1 in 1 documents");

        let mut hidden_rows = vec![row("a.md", FieldGroup::Names, model("a"))];
        hidden_rows[0].hidden = true;
        let view = SearchResultsView::new("payment".to_string(), hidden_rows);
        assert_eq!(
            view.header_text(),
            "\u{1F50D} payment    1 in 1 documents, 1 hidden"
        );
    }

    #[test]
    fn identity_is_search_results() {
        let view = SearchResultsView::new("q".to_string(), Vec::new());
        assert_eq!(view.identity(), DocViewIdentity::SearchResults);
    }

    #[test]
    fn chrome_hides_diagram_only_chrome_and_the_breadcrumb() {
        let view = SearchResultsView::new("q".to_string(), Vec::new());
        assert_eq!(view.chrome(), BodyChrome::HIDDEN);
    }

    #[test]
    fn flattened_items_skips_rows_of_a_collapsed_group() {
        let mut groups = group_hits(vec![
            row("a.md", FieldGroup::Names, model("a")),
            row("a.md", FieldGroup::Prose, span(2)),
            row("b.md", FieldGroup::Names, model("b")),
        ]);
        groups[0].collapsed = true;
        let items = flattened_items(&groups);
        // Group a: header only (collapsed). Group b: header + one row.
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].1, ListItemKind::Header(0));
        assert_eq!(items[1].1, ListItemKind::Header(1));
        assert_eq!(items[2].1, ListItemKind::Row(1, 0));
    }

    #[test]
    fn header_and_row_index_lookups_round_trip_through_item_ids() {
        let groups = group_hits(vec![
            row("a.md", FieldGroup::Names, model("a")),
            row("a.md", FieldGroup::Prose, span(2)),
            row("b.md", FieldGroup::Names, model("b")),
        ]);
        for (group_index, group) in groups.iter().enumerate() {
            let header_id = LiveId::from_str(&format!("h{group_index}"));
            assert_eq!(header_index_for(&groups, header_id), Some(group_index));
            for row_index in 0..group.rows.len() {
                let row_id = LiveId::from_str(&format!("r{group_index}:{row_index}"));
                assert_eq!(
                    row_index_for(&groups, row_id),
                    Some((group_index, row_index))
                );
            }
        }
        assert_eq!(header_index_for(&groups, LiveId::from_str("unknown")), None);
        assert_eq!(row_index_for(&groups, LiveId::from_str("unknown")), None);
    }
}
