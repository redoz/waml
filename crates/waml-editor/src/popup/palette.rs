//! `PalettePopup` — the Ctrl+K command palette, a fifth `PopupRoot` surface
//! beside `MenuPopup`/`RadialPopup`/`SelectFlyout`/`ConflictList` (Task 10,
//! spec `2026-08-09-bundle-search-design.md` §Palette). `build_palette_model`
//! is the pure, unit-tested blended-section model; `PalettePopup` is the
//! widget that renders it and drives keyboard/mouse row selection. Wired into
//! `PopupRoot` here (route + geometry); the hotkey, live re-query and
//! activation routing land in Task 11 -- this task only creates the surface
//! and proves it compiles + is reachable (mirrors `SelectFlyout`'s Task 5
//! creation-before-wiring shape).

// The shell doesn't call `show_at(PopupSpec::Palette)` until Task 11, so
// several `pub` methods here have no in-crate caller yet -- matches
// `select.rs`/`conflict_list.rs`'s module-wide allow for the same reason.
#![allow(dead_code)]

use std::collections::HashSet;

use crate::frame::SurfaceExt;
use crate::popup::base::{Popup, PopupResult, PopupVerdict};
use crate::search_results_view::{label_for, RowLabel};
use crate::search_state::TextIndexStatus;
use makepad_widgets::*;
use waml::search::query::{parse_query, QueryFilter};
// `waml::search::Hit` collides with `makepad_widgets::Hit` (the event-hits
// enum) once this file glob-imports `makepad_widgets::*` -- alias it, same
// as `search_results_view.rs`.
use waml::search::{FieldGroup, Hit as SearchHit, HitTarget, Snippet};

/// Height of the query input row (lpx).
pub const PALETTE_QUERY_H: f64 = 40.0;
/// Height of a section title row (lpx).
pub const PALETTE_HEADER_H: f64 = 22.0;
/// Height of a selectable/informational row (lpx).
pub const PALETTE_ROW_H: f64 = 30.0;
/// Top/bottom padding around the row list, below the query row (lpx).
pub const PALETTE_PAD_V: f64 = 8.0;
/// TEXT section cap: at most this many real snippet rows before a
/// `MoreText` row (spec §Palette).
const TEXT_ROW_CAP: usize = 2;
/// `Recent` section row cap (decision 2).
const RECENT_ROW_CAP: usize = 8;

/// One palette row's shape. `Concept`/`Document`/`Text`/`Structure` carry a
/// live `Hit`; `MoreText`/`Escalate`/`Recent`/`NoResults`/`Indexing` are
/// synthesized rows the model builds without one.
#[derive(Clone, Debug, PartialEq)]
pub enum PaletteRowKind {
    /// `"class Payment    domain/billing.waml"` -- a `Names`/`ModelElement`
    /// or `Model` hit. `kind` is left empty here (this pure model has no
    /// `uml::Analysis` handle to resolve it); the opener may enrich it later.
    Concept { kind: String },
    /// `"md payment-flow.md   guides/"` -- a `Names` hit on a document title.
    Document,
    /// A prose snippet row, capped at `TEXT_ROW_CAP` per section.
    Text { line: u32 },
    /// An `id:`/link/frontmatter-key hit.
    Structure { label: RowLabel },
    /// `"+ N more"` -- the TEXT section's overflow row.
    MoreText { omitted: usize },
    /// `"Search all text for \"q\" -- N results"` -- always the last row for
    /// a non-empty query (spec §Palette).
    Escalate { query: String, total: usize },
    /// Empty-query state: the most recent distinct documents (decision 2).
    Recent { concept_id: String },
    /// No hits at all: names the query and its active filters.
    NoResults { query: String, filters: Vec<String> },
    /// `TextIndexStatus::Building` note under TEXT (decision 6).
    Indexing,
}

/// One rendered palette row.
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteRow {
    pub kind: PaletteRowKind,
    pub title: String,
    /// Path / directory column.
    pub detail: String,
    pub hit: Option<SearchHit>,
    /// Hidden by the active projection -- muted + badge, same rule as the
    /// results tab (`SearchState::hidden_documents`).
    pub hidden: bool,
}

/// One section of the blended list (CONCEPTS / DOCUMENTS / TEXT / STRUCTURE
/// / RECENT), or a title-less single-row section (the trailing escalate row,
/// or the sole no-results row).
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteSectionModel {
    pub title: String,
    pub count: usize,
    pub rows: Vec<PaletteRow>,
}

fn active_filter_strings(query: &str) -> Vec<String> {
    parse_query(query)
        .filters
        .into_iter()
        .map(|f| match f {
            QueryFilter::Kind(k) => format!("kind:{k}"),
            QueryFilter::InPath(p) => format!("in:{p}"),
        })
        .collect()
}

fn palette_row(
    kind: PaletteRowKind,
    title: String,
    detail: String,
    hit: &SearchHit,
    hidden: bool,
) -> PaletteRow {
    PaletteRow {
        kind,
        title,
        detail,
        hit: Some(hit.clone()),
        hidden,
    }
}

/// Build the blended, sectioned palette list for one query (spec §Palette):
/// CONCEPTS, DOCUMENTS, TEXT (max `TEXT_ROW_CAP` rows + `MoreText`),
/// STRUCTURE, then ALWAYS a trailing single-row escalate section for a
/// non-empty query. An empty query yields a single RECENT section instead; a
/// non-empty query with no hits yields a single no-title section naming the
/// query and its active filters.
pub fn build_palette_model(
    query: &str,
    hits: &[SearchHit],
    snippets: &dyn Fn(&SearchHit) -> Snippet,
    hidden: &HashSet<String>,
    recents: &[(String, String)],
    text_status: TextIndexStatus,
) -> Vec<PaletteSectionModel> {
    if query.trim().is_empty() {
        let rows: Vec<PaletteRow> = recents
            .iter()
            .take(RECENT_ROW_CAP)
            .map(|(concept_id, title)| PaletteRow {
                kind: PaletteRowKind::Recent {
                    concept_id: concept_id.clone(),
                },
                title: title.clone(),
                detail: String::new(),
                hit: None,
                hidden: false,
            })
            .collect();
        let count = rows.len();
        return vec![PaletteSectionModel {
            title: "RECENT".to_string(),
            count,
            rows,
        }];
    }

    if hits.is_empty() {
        return vec![PaletteSectionModel {
            title: String::new(),
            count: 0,
            rows: vec![PaletteRow {
                kind: PaletteRowKind::NoResults {
                    query: query.to_string(),
                    filters: active_filter_strings(query),
                },
                title: format!("No results for \"{query}\""),
                detail: String::new(),
                hit: None,
                hidden: false,
            }],
        }];
    }

    let mut concepts = Vec::new();
    let mut documents = Vec::new();
    let mut text_rows = Vec::new();
    let mut structure_rows = Vec::new();

    for hit in hits {
        let is_hidden = hidden.contains(&hit.document);
        let snippet = snippets(hit);
        match (hit.group, &hit.target) {
            (FieldGroup::Names, HitTarget::ModelElement { .. }) | (FieldGroup::Model, _) => {
                concepts.push(palette_row(
                    PaletteRowKind::Concept {
                        kind: String::new(),
                    },
                    snippet.text,
                    hit.document.clone(),
                    hit,
                    is_hidden,
                ));
            }
            (FieldGroup::Names, HitTarget::TextSpan { .. }) => {
                documents.push(palette_row(
                    PaletteRowKind::Document,
                    snippet.text,
                    hit.document.clone(),
                    hit,
                    is_hidden,
                ));
            }
            (FieldGroup::Prose, _) => {
                let line = match hit.target {
                    HitTarget::TextSpan { line, .. } => line,
                    HitTarget::ModelElement { .. } => 0,
                };
                text_rows.push(palette_row(
                    PaletteRowKind::Text { line },
                    snippet.text,
                    hit.document.clone(),
                    hit,
                    is_hidden,
                ));
            }
            (FieldGroup::Structure, _) => {
                let label = label_for(hit);
                structure_rows.push(palette_row(
                    PaletteRowKind::Structure { label },
                    snippet.text,
                    hit.document.clone(),
                    hit,
                    is_hidden,
                ));
            }
        }
    }

    let mut sections = Vec::new();
    if !concepts.is_empty() {
        sections.push(PaletteSectionModel {
            title: "CONCEPTS".to_string(),
            count: concepts.len(),
            rows: concepts,
        });
    }
    if !documents.is_empty() {
        sections.push(PaletteSectionModel {
            title: "DOCUMENTS".to_string(),
            count: documents.len(),
            rows: documents,
        });
    }
    {
        let total = text_rows.len();
        let mut rows: Vec<PaletteRow> = text_rows.into_iter().take(TEXT_ROW_CAP).collect();
        if total > TEXT_ROW_CAP {
            let omitted = total - TEXT_ROW_CAP;
            rows.push(PaletteRow {
                kind: PaletteRowKind::MoreText { omitted },
                title: format!("+ {omitted} more"),
                detail: String::new(),
                hit: None,
                hidden: false,
            });
        }
        if text_status == TextIndexStatus::Building {
            rows.push(PaletteRow {
                kind: PaletteRowKind::Indexing,
                title: "indexing…".to_string(),
                detail: String::new(),
                hit: None,
                hidden: false,
            });
        }
        if !rows.is_empty() {
            sections.push(PaletteSectionModel {
                title: "TEXT".to_string(),
                count: total,
                rows,
            });
        }
    }
    if !structure_rows.is_empty() {
        sections.push(PaletteSectionModel {
            title: "STRUCTURE".to_string(),
            count: structure_rows.len(),
            rows: structure_rows,
        });
    }

    sections.push(PaletteSectionModel {
        title: String::new(),
        count: 1,
        rows: vec![PaletteRow {
            kind: PaletteRowKind::Escalate {
                query: query.to_string(),
                total: hits.len(),
            },
            title: format!(
                "Search all text for \"{query}\" \u{2014} {} results",
                hits.len()
            ),
            detail: String::new(),
            hit: None,
            hidden: false,
        }],
    });

    sections
}

/// A row a keyboard/mouse selection can land on. Informational rows
/// (`NoResults`, `Indexing`) are not.
fn is_activatable(kind: &PaletteRowKind) -> bool {
    !matches!(
        kind,
        PaletteRowKind::NoResults { .. } | PaletteRowKind::Indexing
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaletteLayoutKind {
    Header(usize),
    Row(usize, usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PaletteLayoutRow {
    top: f64,
    height: f64,
    kind: PaletteLayoutKind,
}

/// Pure vertical layout of `sections` (row-list-relative `top`, i.e. `0` is
/// the first pixel below the query row's padding). Recomputed on every
/// `set_sections`/`open_palette` call and reused by both `draw` and hit
/// testing, so the two can never disagree about where a row is.
fn palette_layout(sections: &[PaletteSectionModel]) -> Vec<PaletteLayoutRow> {
    let mut rows = Vec::new();
    let mut y = 0.0_f64;
    for (section_index, section) in sections.iter().enumerate() {
        if !section.title.is_empty() {
            rows.push(PaletteLayoutRow {
                top: y,
                height: PALETTE_HEADER_H,
                kind: PaletteLayoutKind::Header(section_index),
            });
            y += PALETTE_HEADER_H;
        }
        for row_index in 0..section.rows.len() {
            rows.push(PaletteLayoutRow {
                top: y,
                height: PALETTE_ROW_H,
                kind: PaletteLayoutKind::Row(section_index, row_index),
            });
            y += PALETTE_ROW_H;
        }
    }
    rows
}

fn activatable_positions(sections: &[PaletteSectionModel]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (section_index, section) in sections.iter().enumerate() {
        for (row_index, row) in section.rows.iter().enumerate() {
            if is_activatable(&row.kind) {
                out.push((section_index, row_index));
            }
        }
    }
    out
}

/// The full card height `sections` would take: the query row, top/bottom
/// list padding, and every header/row's height. `PopupRoot::show_at` calls
/// this to size the card before `open_palette` (mirrors
/// `conflict_list::content_size`).
pub fn content_height(sections: &[PaletteSectionModel]) -> f64 {
    let rows_height = palette_layout(sections)
        .last()
        .map(|r| r.top + r.height)
        .unwrap_or(0.0);
    PALETTE_QUERY_H + PALETTE_PAD_V * 2.0 + rows_height
}

/// Emitted while typing in the query input; the opener re-queries
/// `SearchState` and pushes fresh sections back via `set_sections` (Task 11).
#[derive(Clone, Debug)]
pub enum PaletteAction {
    QueryChanged(String),
}

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.fonts
    use mod.widgets.*
    use mod.text.*

    mod.widgets.PalettePopupBase = #(PalettePopup::register_widget(vm))

    mod.widgets.PalettePopup = set_type_default() do mod.widgets.PalettePopupBase{
        width: Fill
        height: Fill
        draw_frame: mod.draw.PanelSurface{ color: atlas.field_bg }
        draw_query_bg: mod.draw.DrawColor{ color: atlas.surface_border }
        draw_hover: mod.draw.DrawColor{ color: atlas.selection }
        draw_query +: {
            color: atlas.text
            text_style: fonts.text_menu
        }
        draw_section_label +: {
            color: atlas.text_dim
            text_style: fonts.text_micro
        }
        draw_row_title +: {
            color: atlas.text
            text_style: fonts.text_label
        }
        draw_row_detail +: {
            color: atlas.text_dim
            text_style: fonts.text_micro
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct PalettePopup {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    /// Own draw list into the WINDOW OVERLAY (`begin_overlay_reuse`), same
    /// idiom as `MenuPopup`/`SelectFlyout`.
    #[live]
    draw_list: DrawList2d,

    #[redraw]
    #[live]
    draw_frame: DrawColor,
    #[redraw]
    #[live]
    draw_query_bg: DrawColor,
    #[redraw]
    #[live]
    draw_hover: DrawColor,
    #[redraw]
    #[live]
    draw_query: DrawText,
    #[redraw]
    #[live]
    draw_section_label: DrawText,
    #[redraw]
    #[live]
    draw_row_title: DrawText,
    #[redraw]
    #[live]
    draw_row_detail: DrawText,

    #[rust]
    open: bool,
    /// Card rect, computed by the opener (`PopupRoot::show_at`) and handed in
    /// whole -- unlike `MenuPopup`/`SelectFlyout` this surface has no
    /// clamp-on-measured-width step (decision: fixed max width).
    #[rust]
    rect: Rect,
    #[rust]
    query: String,
    #[rust]
    sections: Vec<PaletteSectionModel>,
    #[rust]
    layout_rows: Vec<PaletteLayoutRow>,
    /// `(section, row)` pairs, parallel to `armed`'s index space.
    #[rust]
    activatable: Vec<(usize, usize)>,
    /// Index into `activatable` -- the keyboard/hover-armed row.
    #[rust]
    armed: Option<usize>,
    /// Index into `activatable` a `MouseDown` landed on, awaiting a matching
    /// `MouseUp` over the same row to commit (press/release, not press-alone).
    #[rust]
    pressed: Option<usize>,
}

impl Widget for PalettePopup {
    // Event-passive: `PopupRoot` drives this through `Popup::handle`, not
    // tree routing.
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, _walk: Walk) -> DrawStep {
        if !self.is_open() {
            return DrawStep::done();
        }
        self.draw_list.begin_overlay_reuse(cx);
        let size = cx.current_pass_size();
        cx.begin_root_turtle(size, Layout::flow_overlay());
        self.draw(cx);
        cx.end_pass_sized_turtle();
        self.draw_list.end(cx);
        DrawStep::done()
    }

    /// One `WamlPaletteSection` item per titled section header (CONCEPTS,
    /// DOCUMENTS, TEXT, STRUCTURE, RECENT -- the title-less escalate/
    /// no-results section never gets a `Header` layout row, see
    /// `palette_layout`, so it never appears here). This is the only way a
    /// UI test can read the blended section list: the row list itself is
    /// hand-drawn (`draw`), not a tree of child widgets, the same reason
    /// `ProjectTree` implements this hook (see the typed UI regression
    /// testing design's "second framework gap").
    fn semantic_items(&self, _cx: &Cx) -> Vec<WidgetSemanticItem> {
        if !self.open {
            return Vec::new();
        }
        palette_semantic_sections(&self.sections, &self.layout_rows, self.rect)
    }
}

/// Pure half of `PalettePopup::semantic_items` (kept free so it is
/// unit-testable without a live widget/`Cx`, the `project_tree_semantic_items`
/// idiom). One item per titled section header, `text` = the section title,
/// `value` = its row count -- `Task 16`'s `expect_palette_sections` DSL op
/// reads exactly these two fields.
fn palette_semantic_sections(
    sections: &[PaletteSectionModel],
    layout_rows: &[PaletteLayoutRow],
    rect: Rect,
) -> Vec<WidgetSemanticItem> {
    let list_top = rect.pos.y + PALETTE_QUERY_H + PALETTE_PAD_V;
    layout_rows
        .iter()
        .filter_map(|entry| {
            let PaletteLayoutKind::Header(section_index) = entry.kind else {
                return None;
            };
            let section = sections.get(section_index)?;
            Some(WidgetSemanticItem {
                id: format!("palette-section:{section_index}"),
                widget_type: "WamlPaletteSection".to_string(),
                rect: Rect {
                    pos: dvec2(rect.pos.x, list_top + entry.top),
                    size: dvec2(rect.size.x, entry.height),
                },
                visible: true,
                enabled: true,
                text: Some(section.title.clone()),
                value: Some(section.count.to_string()),
                checked: None,
                selected: None,
            })
        })
        .collect()
}

impl PalettePopup {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    /// Every row title currently rendered, in draw order -- for App-level
    /// tests asserting a live re-query reached the widget (Task 11), the
    /// same shape as `tree_panel.rs`'s `test_row_titles`.
    #[cfg(test)]
    pub(crate) fn test_row_titles(&self) -> Vec<String> {
        self.sections
            .iter()
            .flat_map(|section| section.rows.iter().map(|row| row.title.clone()))
            .collect()
    }

    /// Open at `rect` (already placed + sized by the opener) with `sections`.
    pub fn open_palette(&mut self, cx: &mut Cx, rect: Rect, sections: Vec<PaletteSectionModel>) {
        self.open = true;
        self.rect = rect;
        self.query.clear();
        self.pressed = None;
        self.set_sections_inner(sections);
        self.draw_frame.redraw(cx);
    }

    /// Re-seed the row list in place (a live-query re-run); a no-op while
    /// closed.
    pub fn set_sections(&mut self, cx: &mut Cx, sections: Vec<PaletteSectionModel>) {
        if !self.open {
            return;
        }
        self.set_sections_inner(sections);
        self.draw_frame.redraw(cx);
    }

    fn set_sections_inner(&mut self, sections: Vec<PaletteSectionModel>) {
        self.layout_rows = palette_layout(&sections);
        self.activatable = activatable_positions(&sections);
        self.armed = if self.activatable.is_empty() {
            None
        } else {
            Some(0)
        };
        self.sections = sections;
    }

    /// The query text as last reported via `PaletteAction::QueryChanged`.
    pub fn query_changed(&self, actions: &Actions) -> Option<String> {
        let action = actions.find_widget_action(self.widget_uid())?;
        match action.action.downcast_ref::<PaletteAction>()? {
            PaletteAction::QueryChanged(q) => Some(q.clone()),
        }
    }

    /// The row at window position `abs`, as an index into `activatable` (row
    /// list only -- the query row and section headers never resolve here).
    fn row_at(&self, abs: DVec2) -> Option<usize> {
        if !self.rect.contains(abs) {
            return None;
        }
        let local_y = abs.y - self.rect.pos.y - PALETTE_QUERY_H - PALETTE_PAD_V;
        if local_y < 0.0 {
            return None;
        }
        self.layout_rows.iter().find_map(|entry| {
            if local_y < entry.top || local_y >= entry.top + entry.height {
                return None;
            }
            match entry.kind {
                PaletteLayoutKind::Row(section, row) => self
                    .activatable
                    .iter()
                    .position(|&(s, r)| s == section && r == row),
                PaletteLayoutKind::Header(_) => None,
            }
        })
    }

    fn move_armed(&mut self, delta: isize) {
        if self.activatable.is_empty() {
            self.armed = None;
            return;
        }
        let len = self.activatable.len() as isize;
        let current = self.armed.map(|i| i as isize).unwrap_or(-1);
        let next = (current + delta).rem_euclid(len);
        self.armed = Some(next as usize);
    }

    /// Commit the row at `index` (an `activatable` index): close and report
    /// it. The opener resolves `p:{section}:{row}` back to the `PaletteRow`
    /// it built the same `sections` from (the widget itself is reset right
    /// after this returns, so the id -- not the widget's own state -- is the
    /// only thing that survives the close).
    fn commit(&mut self, cx: &mut Cx, index: usize) -> PopupVerdict {
        let Some(&(section, row)) = self.activatable.get(index) else {
            return PopupVerdict::Consumed;
        };
        let id = LiveId::from_str(&format!("p:{section}:{row}"));
        self.open = false;
        self.draw_frame.redraw(cx);
        PopupVerdict::Closed(PopupResult::Invoked(id))
    }

    fn emit_query_changed(&mut self, cx: &mut Cx) {
        cx.widget_action(
            self.widget_uid(),
            PaletteAction::QueryChanged(self.query.clone()),
        );
        self.draw_frame.redraw(cx);
    }

    pub fn draw(&mut self, cx: &mut Cx2d) {
        if !self.open {
            return;
        }
        self.draw_frame.draw_surface_abs(cx, self.rect);

        let query_rect = Rect {
            pos: self.rect.pos,
            size: dvec2(self.rect.size.x, PALETTE_QUERY_H),
        };
        self.draw_query_bg.draw_abs(cx, query_rect);
        let placeholder = "Search…";
        let text = if self.query.is_empty() {
            placeholder
        } else {
            self.query.as_str()
        };
        self.draw_query.draw_abs(
            cx,
            dvec2(query_rect.pos.x + 14.0, query_rect.pos.y + 12.0),
            text,
        );

        let list_top = self.rect.pos.y + PALETTE_QUERY_H + PALETTE_PAD_V;
        for entry in self.layout_rows.clone() {
            let row_rect = Rect {
                pos: dvec2(self.rect.pos.x, list_top + entry.top),
                size: dvec2(self.rect.size.x, entry.height),
            };
            match entry.kind {
                PaletteLayoutKind::Header(section) => {
                    if let Some(model) = self.sections.get(section) {
                        self.draw_section_label.draw_abs(
                            cx,
                            dvec2(row_rect.pos.x + 12.0, row_rect.pos.y + 5.0),
                            &model.title,
                        );
                    }
                }
                PaletteLayoutKind::Row(section, row) => {
                    let activatable_index = self
                        .activatable
                        .iter()
                        .position(|&(s, r)| s == section && r == row);
                    if activatable_index.is_some() && activatable_index == self.armed {
                        self.draw_hover.draw_abs(cx, row_rect);
                    }
                    if let Some(model) = self.sections.get(section).and_then(|s| s.rows.get(row)) {
                        self.draw_row_title.draw_abs(
                            cx,
                            dvec2(row_rect.pos.x + 12.0, row_rect.pos.y + 7.0),
                            &model.title,
                        );
                        if !model.detail.is_empty() {
                            self.draw_row_detail.draw_abs(
                                cx,
                                dvec2(row_rect.pos.x + 240.0, row_rect.pos.y + 8.0),
                                &model.detail,
                            );
                        }
                    }
                }
            }
        }
    }
}

impl Popup for PalettePopup {
    fn handle(&mut self, cx: &mut Cx, event: &Event) -> PopupVerdict {
        if !self.open {
            return PopupVerdict::Consumed;
        }
        match event {
            Event::MouseMove(e) => {
                let hit = self.row_at(e.abs);
                if hit != self.armed {
                    self.armed = hit;
                    self.draw_frame.redraw(cx);
                }
                PopupVerdict::Consumed
            }
            Event::MouseDown(e) if e.button.is_primary() => {
                if self.rect.contains(e.abs) {
                    self.pressed = self.row_at(e.abs);
                    e.handled.set(self.draw_frame.area());
                    PopupVerdict::Consumed
                } else {
                    // Outside press: leave `handled` empty so `PopupRoot`
                    // still sees `Ignored` and light-dismisses.
                    PopupVerdict::Ignored
                }
            }
            Event::MouseUp(e) if e.button.is_primary() => {
                if let Some(pressed) = self.pressed.take() {
                    if self.row_at(e.abs) == Some(pressed) {
                        return self.commit(cx, pressed);
                    }
                }
                PopupVerdict::Consumed
            }
            Event::KeyDown(ke) => match ke.key_code {
                KeyCode::ArrowDown => {
                    self.move_armed(1);
                    self.draw_frame.redraw(cx);
                    PopupVerdict::Consumed
                }
                KeyCode::ArrowUp => {
                    self.move_armed(-1);
                    self.draw_frame.redraw(cx);
                    PopupVerdict::Consumed
                }
                KeyCode::ReturnKey => match self.armed {
                    Some(index) => self.commit(cx, index),
                    None => PopupVerdict::Consumed,
                },
                KeyCode::Backspace => {
                    if self.query.pop().is_some() {
                        self.emit_query_changed(cx);
                    }
                    PopupVerdict::Consumed
                }
                _ => PopupVerdict::Consumed,
            },
            Event::TextInput(ti) => {
                let mut changed = false;
                for ch in ti.input.chars() {
                    if !ch.is_control() {
                        self.query.push(ch);
                        changed = true;
                    }
                }
                if changed {
                    self.emit_query_changed(cx);
                }
                PopupVerdict::Consumed
            }
            _ => PopupVerdict::Consumed,
        }
    }

    fn reset(&mut self) {
        self.open = false;
        self.pressed = None;
    }

    fn hovers_item(&self) -> bool {
        self.armed.is_some()
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

    fn model(key: &str) -> HitTarget {
        HitTarget::ModelElement {
            key: key.to_string(),
        }
    }

    fn span(line: u32) -> HitTarget {
        HitTarget::TextSpan {
            start: 0,
            end: 4,
            line,
        }
    }

    fn no_snippet(_hit: &SearchHit) -> Snippet {
        Snippet {
            text: String::new(),
            highlights: Vec::new(),
        }
    }

    fn section_titles(sections: &[PaletteSectionModel]) -> Vec<&str> {
        sections.iter().map(|s| s.title.as_str()).collect()
    }

    #[test]
    fn sections_appear_in_spec_order_with_counts() {
        let hits = vec![
            hit("billing.waml", FieldGroup::Names, model("payment")),
            hit("billing.waml", FieldGroup::Model, model("payment")),
            hit("guides/checkout.md", FieldGroup::Names, span(1)),
            hit("guides/checkout.md", FieldGroup::Prose, span(4)),
            hit(
                "guides/checkout.md",
                FieldGroup::Structure,
                model("payment"),
            ),
        ];
        let hidden = HashSet::new();
        let sections = build_palette_model(
            "payment",
            &hits,
            &no_snippet,
            &hidden,
            &[],
            TextIndexStatus::Ready,
        );
        assert_eq!(
            section_titles(&sections),
            vec!["CONCEPTS", "DOCUMENTS", "TEXT", "STRUCTURE", ""]
        );
        assert_eq!(sections[0].count, 2); // Names+ModelElement and Model both fold into CONCEPTS
        assert_eq!(sections[1].count, 1);
        assert_eq!(sections[2].count, 1);
        assert_eq!(sections[3].count, 1);
        assert_eq!(sections[4].count, 1);
        assert!(matches!(
            sections[4].rows[0].kind,
            PaletteRowKind::Escalate { .. }
        ));
    }

    #[test]
    fn text_caps_at_two_rows_plus_more_text() {
        let hits: Vec<SearchHit> = (0..5)
            .map(|i| hit("a.md", FieldGroup::Prose, span(i)))
            .collect();
        let hidden = HashSet::new();
        let sections = build_palette_model(
            "term",
            &hits,
            &no_snippet,
            &hidden,
            &[],
            TextIndexStatus::Ready,
        );
        let text_section = sections
            .iter()
            .find(|s| s.title == "TEXT")
            .expect("a TEXT section");
        assert_eq!(text_section.count, 5);
        assert_eq!(text_section.rows.len(), 3); // 2 snippet rows + MoreText
        assert!(matches!(
            text_section.rows[2].kind,
            PaletteRowKind::MoreText { omitted: 3 }
        ));
    }

    #[test]
    fn escalate_row_always_present_for_a_non_empty_query_with_the_total() {
        let hits = vec![hit("a.md", FieldGroup::Names, model("a"))];
        let hidden = HashSet::new();
        let sections = build_palette_model(
            "a",
            &hits,
            &no_snippet,
            &hidden,
            &[],
            TextIndexStatus::Ready,
        );
        let escalate = sections.last().expect("a trailing section");
        match &escalate.rows[0].kind {
            PaletteRowKind::Escalate { query, total } => {
                assert_eq!(query, "a");
                assert_eq!(*total, 1);
            }
            other => panic!("expected an Escalate row, got {other:?}"),
        }
    }

    #[test]
    fn empty_query_yields_recent_rows() {
        let recents = vec![
            ("billing/order".to_string(), "Order".to_string()),
            ("billing/invoice".to_string(), "Invoice".to_string()),
        ];
        let hidden = HashSet::new();
        let sections = build_palette_model(
            "",
            &[],
            &no_snippet,
            &hidden,
            &recents,
            TextIndexStatus::Ready,
        );
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "RECENT");
        assert_eq!(sections[0].rows.len(), 2);
        assert!(matches!(
            sections[0].rows[0].kind,
            PaletteRowKind::Recent { .. }
        ));

        // Whitespace-only counts as empty too.
        let whitespace_sections = build_palette_model(
            "   ",
            &[],
            &no_snippet,
            &hidden,
            &recents,
            TextIndexStatus::Ready,
        );
        assert_eq!(whitespace_sections[0].title, "RECENT");
    }

    #[test]
    fn no_hits_yields_no_results_naming_the_query_and_its_filters() {
        let hidden = HashSet::new();
        let sections = build_palette_model(
            "kind:actor payment",
            &[],
            &no_snippet,
            &hidden,
            &[],
            TextIndexStatus::Ready,
        );
        assert_eq!(sections.len(), 1);
        match &sections[0].rows[0].kind {
            PaletteRowKind::NoResults { query, filters } => {
                assert_eq!(query, "kind:actor payment");
                assert_eq!(filters, &vec!["kind:actor".to_string()]);
            }
            other => panic!("expected a NoResults row, got {other:?}"),
        }
    }

    #[test]
    fn building_status_yields_the_indexing_row_in_text_while_other_sections_populate() {
        let hits = vec![
            hit("a.md", FieldGroup::Names, model("a")),
            hit("a.md", FieldGroup::Prose, span(2)),
        ];
        let hidden = HashSet::new();
        let sections = build_palette_model(
            "a",
            &hits,
            &no_snippet,
            &hidden,
            &[],
            TextIndexStatus::Building,
        );
        assert_eq!(
            section_titles(&sections),
            vec!["CONCEPTS", "TEXT", ""],
            "CONCEPTS still populates while the index is building"
        );
        let text_section = &sections[1];
        assert!(text_section
            .rows
            .iter()
            .any(|row| matches!(row.kind, PaletteRowKind::Indexing)));
    }

    #[test]
    fn palette_layout_stacks_headers_and_rows_and_content_height_matches_its_end() {
        let sections = vec![
            PaletteSectionModel {
                title: "CONCEPTS".to_string(),
                count: 2,
                rows: vec![
                    PaletteRow {
                        kind: PaletteRowKind::Concept {
                            kind: String::new(),
                        },
                        title: "A".to_string(),
                        detail: String::new(),
                        hit: None,
                        hidden: false,
                    },
                    PaletteRow {
                        kind: PaletteRowKind::Concept {
                            kind: String::new(),
                        },
                        title: "B".to_string(),
                        detail: String::new(),
                        hit: None,
                        hidden: false,
                    },
                ],
            },
            PaletteSectionModel {
                title: String::new(),
                count: 1,
                rows: vec![PaletteRow {
                    kind: PaletteRowKind::Escalate {
                        query: "q".to_string(),
                        total: 2,
                    },
                    title: "Search all".to_string(),
                    detail: String::new(),
                    hit: None,
                    hidden: false,
                }],
            },
        ];
        let layout = palette_layout(&sections);
        // Header, row, row, row (no header for the title-less section).
        assert_eq!(layout.len(), 4);
        assert_eq!(layout[0].kind, PaletteLayoutKind::Header(0));
        assert_eq!(layout[0].top, 0.0);
        assert_eq!(layout[1].kind, PaletteLayoutKind::Row(0, 0));
        assert_eq!(layout[1].top, PALETTE_HEADER_H);
        assert_eq!(layout[3].kind, PaletteLayoutKind::Row(1, 0));

        let last = layout.last().unwrap();
        assert_eq!(
            content_height(&sections),
            PALETTE_QUERY_H + PALETTE_PAD_V * 2.0 + last.top + last.height
        );
    }

    #[test]
    fn semantic_sections_expose_titled_headers_with_text_and_count() {
        let hits = vec![
            hit("billing.waml", FieldGroup::Names, model("payment")),
            hit("guides/checkout.md", FieldGroup::Prose, span(4)),
        ];
        let hidden = HashSet::new();
        let sections = build_palette_model(
            "payment",
            &hits,
            &no_snippet,
            &hidden,
            &[],
            TextIndexStatus::Ready,
        );
        let layout_rows = palette_layout(&sections);
        let rect = Rect {
            pos: dvec2(100.0, 40.0),
            size: dvec2(360.0, content_height(&sections)),
        };

        let items = palette_semantic_sections(&sections, &layout_rows, rect);

        // Two titled headers (CONCEPTS, TEXT) -- the trailing escalate
        // section has no title and so no `Header` layout row.
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].widget_type, "WamlPaletteSection");
        assert_eq!(items[0].text.as_deref(), Some("CONCEPTS"));
        assert_eq!(items[0].value.as_deref(), Some("1"));
        assert_eq!(items[1].text.as_deref(), Some("TEXT"));
        assert_eq!(items[1].value.as_deref(), Some("1"));
        // The first header sits right below the query row.
        assert_eq!(items[0].rect.pos.x, rect.pos.x);
        assert_eq!(
            items[0].rect.pos.y,
            rect.pos.y + PALETTE_QUERY_H + PALETTE_PAD_V
        );
    }

    #[test]
    fn semantic_sections_are_empty_for_an_empty_query_recent_only_when_no_recents() {
        let hidden = HashSet::new();
        let sections =
            build_palette_model("", &[], &no_snippet, &hidden, &[], TextIndexStatus::Ready);
        let layout_rows = palette_layout(&sections);
        let rect = Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(360.0, content_height(&sections)),
        };

        let items = palette_semantic_sections(&sections, &layout_rows, rect);

        // RECENT is titled, so it still reports a header even with zero rows.
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text.as_deref(), Some("RECENT"));
        assert_eq!(items[0].value.as_deref(), Some("0"));
    }

    #[test]
    fn activatable_positions_skip_no_results_and_indexing_rows() {
        let sections = vec![PaletteSectionModel {
            title: "TEXT".to_string(),
            count: 1,
            rows: vec![
                PaletteRow {
                    kind: PaletteRowKind::Text { line: 1 },
                    title: "snippet".to_string(),
                    detail: String::new(),
                    hit: None,
                    hidden: false,
                },
                PaletteRow {
                    kind: PaletteRowKind::Indexing,
                    title: "indexing…".to_string(),
                    detail: String::new(),
                    hit: None,
                    hidden: false,
                },
            ],
        }];
        assert_eq!(activatable_positions(&sections), vec![(0, 0)]);
    }
}
