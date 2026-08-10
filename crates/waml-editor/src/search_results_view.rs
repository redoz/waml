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
use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, PopupRequest, RevealTarget, ViewData,
    ViewOutcome,
};
use crate::icons::{Icon, IconSet};
use crate::navigation::{NavigationIntent, NavigationTarget, OpenDisposition};
use crate::popup::base::PopupResult;

/// Tag on the confirm popup a hidden-row activation opens (decision 8, spec
/// §States). Relayed to `on_popup_result` regardless of which tag closed --
/// checked there so a different popup's close event is a no-op here.
fn hidden_row_confirm_tag() -> LiveId {
    live_id!(search_hidden_confirm)
}

/// What a result row's match is shown as, next to the snippet (spec
/// §Results tab row shapes). `Line` carries the 1-based source line a prose
/// hit landed on. Constructed by `label_for`, which `App::open_search_results`
/// (Task 9) calls to build each `ResultRow`.
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
/// §Results tab). Called by `SearchResultsView::new`, itself called by
/// `documents::open_search` (Task 9).
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

/// The bundle-wide session's hit order (Task 14, spec §Search session):
/// exactly `group_hits`' document/rank order, flattened -- so `SearchSession`'s
/// F3/Shift+F3 walk lands on the SAME sequence the results tab shows, hit for
/// hit. Called by `App::open_search_results` alongside `documents::open_search`,
/// over the identical `rows`, so the tab and the session never disagree about
/// what "next" means.
pub fn ordered_hits(rows: Vec<ResultRow>) -> Vec<SearchHit> {
    group_hits(rows)
        .into_iter()
        .flat_map(|group| group.rows.into_iter().map(|row| row.hit))
        .collect()
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
    /// F3 traversal marks the current row here (Task 14): `(group_index,
    /// row_index)`, set by `set_cursor` from a flat `SearchSession` index.
    cursor: Option<(usize, usize)>,
    /// The `(group_index, row_index)` awaiting the hidden-row confirm popup's
    /// answer (decision 8) -- stashed by `handle` when a hidden row is
    /// clicked, consumed by `on_popup_result` once the popup closes.
    pending_hidden_row: Option<(usize, usize)>,
}

impl SearchResultsView {
    pub fn new(query: String, rows: Vec<ResultRow>) -> Self {
        SearchResultsView {
            query,
            groups: group_hits(rows),
            cursor: None,
            pending_hidden_row: None,
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

    /// Convert a flat `SearchSession` index (results-tab order, the same
    /// sequence `ordered_hits` flattens `group_hits` into) to the `(group,
    /// row)` pair the widget addresses. `None` for an out-of-range index
    /// (a stale session over a since-changed result set).
    fn locate(&self, flat: usize) -> Option<(usize, usize)> {
        let mut remaining = flat;
        for (group_index, group) in self.groups.iter().enumerate() {
            if remaining < group.rows.len() {
                return Some((group_index, remaining));
            }
            remaining -= group.rows.len();
        }
        None
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
        body.search_results().set_cursor(cx, self.cursor);
    }

    fn mark_search_cursor(&mut self, cx: &mut Cx, body: &BodyWidgets, index: Option<usize>) {
        self.cursor = index.and_then(|flat| self.locate(flat));
        body.search_results().set_cursor(cx, self.cursor);
    }

    fn handle(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        _data: ViewData<'_>,
    ) -> ViewOutcome {
        let mut outcome = ViewOutcome::default();
        if let Some(index) = body.search_results().header_toggled(actions) {
            if let Some(group) = self.groups.get_mut(index) {
                group.collapsed = !group.collapsed;
                let header = self.header_text();
                body.search_results()
                    .set_groups(cx, &header, self.groups.clone());
            }
        }
        if let Some((group_index, row_index)) = body.search_results().row_opened(actions) {
            if let Some(row) = self
                .groups
                .get(group_index)
                .and_then(|group| group.rows.get(row_index))
            {
                if row.hidden {
                    // Projection-hidden hit (spec §States, decision 8): stash
                    // the row and ask the shell for a one-item confirm
                    // popup; the actual navigation lands from
                    // `on_popup_result` once the user commits it.
                    self.pending_hidden_row = Some((group_index, row_index));
                    outcome.popup = Some(PopupRequest::Confirm {
                        anchor: DVec2::default(),
                        title: "Show hidden match".to_string(),
                        tag: hidden_row_confirm_tag(),
                    });
                } else {
                    let (navigation, reveal) = navigation_for_row(row);
                    outcome.navigation = Some(navigation);
                    outcome.reveal = reveal;
                }
            }
        }
        outcome
    }

    fn on_popup_result(
        &mut self,
        _cx: &mut Cx,
        _body: &BodyWidgets,
        _data: ViewData<'_>,
        tag: LiveId,
        result: PopupResult,
    ) -> ViewOutcome {
        let mut outcome = ViewOutcome::default();
        if tag != hidden_row_confirm_tag() {
            return outcome;
        }
        let Some((group_index, row_index)) = self.pending_hidden_row.take() else {
            return outcome;
        };
        if matches!(result, PopupResult::Invoked(_)) {
            if let Some(row) = self
                .groups
                .get(group_index)
                .and_then(|group| group.rows.get(row_index))
            {
                let (navigation, reveal) = navigation_for_row(row);
                outcome.navigation = Some(navigation);
                outcome.reveal = reveal;
            }
        }
        outcome
    }

    fn chrome(&self) -> BodyChrome {
        BodyChrome::HIDDEN
    }
}

/// The hit's concept id: its own, when the extractor resolved one, else the
/// document path with its `.md` suffix stripped. Shared by `navigation_for_hit`
/// and the palette's `Document`/`Recent` row commits (Task 11), which open a
/// concept without a reveal.
pub(crate) fn concept_id_for_hit(hit: &SearchHit) -> String {
    hit.concept_id.clone().unwrap_or_else(|| {
        hit.document
            .strip_suffix(".md")
            .unwrap_or(&hit.document)
            .to_string()
    })
}

/// The navigation + reveal a hit resolves to (spec §Activation per document
/// kind): a model hit opens on the canvas and reveals its element; a
/// prose/structure hit opens on the markdown surface and reveals its span.
/// `SearchResultsView::handle`/`on_popup_result` and the palette's
/// `Concept`/`Text`/`Structure` row commits (Task 11) all funnel through
/// here -- one reveal mapping, no second copy to drift out of sync.
pub(crate) fn navigation_for_hit(
    hit: &SearchHit,
) -> (NavigationIntent, Option<(String, RevealTarget)>) {
    let concept_id = concept_id_for_hit(hit);
    let (surface, reveal) = match &hit.target {
        HitTarget::ModelElement { key } => (
            waml::view::surface::SurfaceId::canvas(),
            RevealTarget::ModelElement { key: key.clone() },
        ),
        HitTarget::TextSpan { start, end, .. } => (
            waml::view::surface::SurfaceId::markdown(),
            RevealTarget::TextSpan {
                start: *start,
                end: *end,
            },
        ),
    };
    let navigation = NavigationIntent::Resolved {
        target: NavigationTarget::Document {
            concept_id: concept_id.clone(),
            surface: Some(surface),
            fragment: None,
        },
        disposition: OpenDisposition::Preview,
    };
    (navigation, Some((concept_id, reveal)))
}

/// The navigation + reveal a row's hit resolves to; see `navigation_for_hit`.
fn navigation_for_row(row: &ResultRow) -> (NavigationIntent, Option<(String, RevealTarget)>) {
    navigation_for_hit(&row.hit)
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
            // F3 traversal's current-row mark (Task 14): stronger tint than
            // hover, additive so a hovered current row reads brighter still.
            current: uniform(0.0)
            pixel: fn() {
                let a = 0.10 * self.hover + 0.18 * self.current
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
    /// F3 traversal's current-row mark (Task 14), set by `set_row`'s
    /// `current` argument -- `SearchResultsListView` computes it from its
    /// own `cursor` field, the widget-side twin of `SearchResultsView::cursor`.
    #[rust]
    current: bool,
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
        self.view.draw_bg.set_uniform(
            cx,
            live_id!(current),
            &[if self.current { 1.0 } else { 0.0 }],
        );
        self.view.draw_walk(cx, scope, walk)
    }
}

impl SearchResultRow {
    pub fn set_row(&mut self, cx: &mut Cx, row: &ResultRow, current: bool) {
        self.current = current;
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
    pub fn set_row(&self, cx: &mut Cx, row: &ResultRow, current: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_row(cx, row, current);
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
    /// `(group_index, row_index)` of the clicked result row. Routed by
    /// `SearchResultsView::handle` to a navigation outcome (Task 9).
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
    /// F3 traversal's current-row mark (Task 14, set via `set_cursor`),
    /// independent of `groups` -- a header toggle's `set_groups` call leaves
    /// it untouched, so collapsing a group never loses the mark it might be
    /// hiding.
    #[rust]
    cursor: Option<(usize, usize)>,
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
                            let current = self.cursor == Some((group_index, row_index));
                            row.as_search_result_row().set_row(
                                cx,
                                &self.groups[group_index].rows[row_index],
                                current,
                            );
                            row.draw_all(cx, &mut Scope::empty());
                        }
                    }
                }
            }
        }
        DrawStep::done()
    }

    /// One `WamlSearchGroupHeader` item per document group, `text` = the
    /// group's full document path (`directory` + `path`), `value` = its row
    /// count -- Task 16's `expect_results_grouped_by_document` DSL op reads
    /// exactly these two fields. Computed straight from `self.groups`
    /// (unlike `flattened_items`, this ignores `FlatList` pooling/
    /// virtualization entirely), same idiom `PalettePopup::semantic_items`
    /// and `ProjectTree`'s `semantic_items` use for a hand-managed row list.
    fn semantic_items(&self, _cx: &Cx) -> Vec<WidgetSemanticItem> {
        if !self.view.visible() {
            return Vec::new();
        }
        search_results_semantic_groups(&self.groups)
    }
}

/// Pure half of `SearchResultsListView::semantic_items`, unit-testable
/// without a live widget/`Cx`.
fn search_results_semantic_groups(groups: &[DocumentGroup]) -> Vec<WidgetSemanticItem> {
    groups
        .iter()
        .enumerate()
        .map(|(index, group)| WidgetSemanticItem {
            id: format!("search-group-header:{index}"),
            widget_type: "WamlSearchGroupHeader".to_string(),
            rect: Rect::default(),
            visible: true,
            enabled: true,
            text: Some(format!("{}{}", group.directory, group.path)),
            value: Some(group.rows.len().to_string()),
            checked: None,
            selected: None,
        })
        .collect()
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

    /// Mirror the live search session's cursor (Task 14): `cursor` is the
    /// `(group_index, row_index)` of the row F3/Shift+F3 (or an activation)
    /// last landed on, or `None` to clear the mark. Called by
    /// `SearchResultsView::mark_search_cursor`/`sync`.
    pub fn set_cursor(&mut self, cx: &mut Cx, cursor: Option<(usize, usize)>) {
        self.cursor = cursor;
        self.view.redraw(cx);
    }

    /// `(group_index, row_index)` of the row clicked in `actions`, if any.
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
    pub fn set_cursor(&self, cx: &mut Cx, cursor: Option<(usize, usize)>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_cursor(cx, cursor);
        }
    }
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
    use waml::source::SourceBundle;

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
    fn semantic_groups_expose_full_document_paths_and_row_counts() {
        let groups = group_hits(vec![
            row("billing.waml", FieldGroup::Names, model("payment")),
            row("guides/checkout.md", FieldGroup::Names, span(1)),
            row("guides/checkout.md", FieldGroup::Prose, span(4)),
        ]);

        let items = search_results_semantic_groups(&groups);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].widget_type, "WamlSearchGroupHeader");
        assert_eq!(items[0].text.as_deref(), Some("billing.waml"));
        assert_eq!(items[0].value.as_deref(), Some("1"));
        assert_eq!(items[1].text.as_deref(), Some("guides/checkout.md"));
        assert_eq!(items[1].value.as_deref(), Some("2"));
    }

    #[test]
    fn semantic_groups_are_empty_for_no_groups() {
        assert!(search_results_semantic_groups(&[]).is_empty());
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

    #[test]
    fn navigation_for_row_maps_a_model_hit_to_canvas_and_a_model_element_reveal() {
        let row = row("billing/order.md", FieldGroup::Names, model("order"));

        let (navigation, reveal) = navigation_for_row(&row);

        assert_eq!(
            navigation,
            NavigationIntent::Resolved {
                target: NavigationTarget::Document {
                    concept_id: "billing/order".to_string(),
                    surface: Some(waml::view::surface::SurfaceId::canvas()),
                    fragment: None,
                },
                disposition: OpenDisposition::Preview,
            }
        );
        assert_eq!(
            reveal,
            Some((
                "billing/order".to_string(),
                RevealTarget::ModelElement {
                    key: "order".to_string()
                }
            ))
        );
    }

    #[test]
    fn navigation_for_row_maps_a_text_span_hit_to_markdown_and_a_text_span_reveal() {
        let row = row("guides/checkout.md", FieldGroup::Prose, span(12));

        let (navigation, reveal) = navigation_for_row(&row);

        assert_eq!(
            navigation,
            NavigationIntent::Resolved {
                target: NavigationTarget::Document {
                    concept_id: "guides/checkout".to_string(),
                    surface: Some(waml::view::surface::SurfaceId::markdown()),
                    fragment: None,
                },
                disposition: OpenDisposition::Preview,
            }
        );
        assert_eq!(
            reveal,
            Some((
                "guides/checkout".to_string(),
                RevealTarget::TextSpan { start: 0, end: 1 }
            ))
        );
    }

    #[test]
    fn navigation_for_row_prefers_the_hits_own_concept_id_over_the_derived_one() {
        let hit = SearchHit {
            document: "billing/order.md".to_string(),
            concept_id: Some("billing/order-alias".to_string()),
            group: FieldGroup::Names,
            target: model("order"),
            score: 1.0,
        };
        let row = ResultRow {
            label: label_for(&hit),
            hit,
            snippet: Snippet {
                text: String::new(),
                highlights: Vec::new(),
            },
            hidden: false,
        };

        let (navigation, _reveal) = navigation_for_row(&row);

        let NavigationIntent::Resolved {
            target: NavigationTarget::Document { concept_id, .. },
            ..
        } = navigation
        else {
            panic!("expected a resolved document navigation");
        };
        assert_eq!(concept_id, "billing/order-alias");
    }

    fn empty_body(cx: &mut Cx) -> WidgetRef {
        let root = cx.with_vm(View::script_new_with_default);
        WidgetRef::new_with_inner(Box::new(root))
    }

    fn empty_view_data() -> (
        SourceBundle,
        waml::analysis::OkfAnalysis,
        waml::uml::Analysis,
        u64,
    ) {
        waml::analysis::prepare_candidate(
            SourceBundle::try_from_pairs([("a.md", "# A\n")]).unwrap(),
            None,
            1,
        )
        .unwrap()
        .into_parts()
    }

    #[test]
    fn on_popup_result_invoked_completes_the_pending_hidden_row_navigation() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let ui = empty_body(&mut cx);
        let body = BodyWidgets::new(&mut cx, &ui);
        let (source, okf_analysis, uml_analysis, revision) = empty_view_data();
        let data = ViewData {
            source: &source,
            okf_analysis: &okf_analysis,
            uml_analysis: &uml_analysis,
            revision,
        };
        let mut hidden_row = row("a.md", FieldGroup::Names, model("a"));
        hidden_row.hidden = true;
        let mut view = SearchResultsView::new("q".to_string(), vec![hidden_row]);
        view.pending_hidden_row = Some((0, 0));

        let outcome = view.on_popup_result(
            &mut cx,
            &body,
            data,
            hidden_row_confirm_tag(),
            PopupResult::Invoked(live_id!(confirm)),
        );

        assert!(view.pending_hidden_row.is_none());
        assert!(outcome.navigation.is_some());
        assert!(outcome.reveal.is_some());
    }

    #[test]
    fn on_popup_result_dismissed_clears_the_pending_row_without_navigating() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let ui = empty_body(&mut cx);
        let body = BodyWidgets::new(&mut cx, &ui);
        let (source, okf_analysis, uml_analysis, revision) = empty_view_data();
        let data = ViewData {
            source: &source,
            okf_analysis: &okf_analysis,
            uml_analysis: &uml_analysis,
            revision,
        };
        let mut hidden_row = row("a.md", FieldGroup::Names, model("a"));
        hidden_row.hidden = true;
        let mut view = SearchResultsView::new("q".to_string(), vec![hidden_row]);
        view.pending_hidden_row = Some((0, 0));

        let outcome = view.on_popup_result(
            &mut cx,
            &body,
            data,
            hidden_row_confirm_tag(),
            PopupResult::Dismissed,
        );

        assert!(view.pending_hidden_row.is_none());
        assert!(outcome.navigation.is_none());
        assert!(outcome.reveal.is_none());
    }

    #[test]
    fn on_popup_result_ignores_an_unrelated_tag() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let ui = empty_body(&mut cx);
        let body = BodyWidgets::new(&mut cx, &ui);
        let (source, okf_analysis, uml_analysis, revision) = empty_view_data();
        let data = ViewData {
            source: &source,
            okf_analysis: &okf_analysis,
            uml_analysis: &uml_analysis,
            revision,
        };
        let mut hidden_row = row("a.md", FieldGroup::Names, model("a"));
        hidden_row.hidden = true;
        let mut view = SearchResultsView::new("q".to_string(), vec![hidden_row]);
        view.pending_hidden_row = Some((0, 0));

        let outcome = view.on_popup_result(
            &mut cx,
            &body,
            data,
            live_id!(some_unrelated_popup),
            PopupResult::Invoked(live_id!(confirm)),
        );

        assert!(outcome.navigation.is_none());
        assert_eq!(view.pending_hidden_row, Some((0, 0)));
    }
}
