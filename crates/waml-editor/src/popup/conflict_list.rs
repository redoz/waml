//! `ConflictList` — the grouped, deletable conflict-error-list card (spec
//! `2026-07-24-conflict-list-grouped-delete-design`). The toolbar conflict
//! badge opens this as the 4th `PopupRoot` surface (wired in `root.rs`, Task
//! 5): one `SceneRelation` per line, `dropped` first then each
//! `conflicts_with`, a hairline divider between conflict groups, and a
//! trailing trash glyph on every line. Mirrors `MenuPopup`'s overlay-draw
//! idiom exactly (`begin_overlay_reuse` + `begin_root_turtle`) so recorded row
//! rects land in window/overlay space and match `MouseMove`/`MouseDown.abs`
//! directly — no translation.

// Not yet instantiated as a `PopupRoot` child (that lands in Task 5) --
// module-wide allow keeps the dead-code gate green in the meantime, matching
// `base.rs`/`node_menu.rs`/`select.rs`.
#![allow(dead_code)]

use crate::icons::{Icon, IconSet};
use crate::popup::base::{Popup, PopupVerdict};
use crate::scene::{relation_statement, SceneConflict};
use makepad_widgets::*;

/// Card width (lpx): wider than `menu::MENU_MAX_W` (320) — relation statements
/// run longer than menu labels.
pub const CONFLICT_MAX_W: f64 = 380.0;
/// Top/bottom padding inside the card (lpx). Matches `menu::PAD_V`.
pub const PAD_V: f64 = 6.0;
/// Left/right padding inside the card (lpx). Matches `menu::PAD_H`.
pub const PAD_H: f64 = 4.0;
/// Row height (lpx). A touch shorter than the menu's 34 — no leading icon
/// gutter, just a statement + a trailing trash cell.
pub const ROW_H: f64 = 30.0;
/// Hairline divider height between conflict groups (lpx).
pub const DIVIDER_H: f64 = 1.0;
/// Trailing trash-glyph cell width (lpx).
pub const TRASH_W: f64 = 22.0;
/// Inset of the trash glyph within its cell (lpx).
pub const TRASH_INSET: f64 = 3.0;

/// One drawn row: a relation statement plus its trailing trash glyph.
/// `body_rect` / `trash_rect` are filled by `ConflictList::draw` every frame
/// (window/overlay space).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConflictRow {
    pub subject: String,
    pub reference: String,
    pub statement: String,
    pub body_rect: Rect,
    pub trash_rect: Rect,
}

/// Flatten every conflict's relations into rows — `dropped` first, then each
/// `conflicts_with`, in order — plus the row index at which a divider
/// precedes a new conflict group (never emitted before the first group, i.e.
/// always `len(conflicts) - 1` entries).
pub fn rows_of(conflicts: &[SceneConflict]) -> (Vec<(String, String, String)>, Vec<usize>) {
    let mut rows = Vec::new();
    let mut dividers = Vec::new();
    for (gi, c) in conflicts.iter().enumerate() {
        if gi > 0 {
            dividers.push(rows.len());
        }
        rows.push((
            c.dropped.subject.clone(),
            c.dropped.reference.clone(),
            relation_statement(&c.dropped),
        ));
        for w in &c.conflicts_with {
            rows.push((
                w.subject.clone(),
                w.reference.clone(),
                relation_statement(w),
            ));
        }
    }
    (rows, dividers)
}

/// Full content height: padding + every row + one hairline per group
/// boundary. Zero conflicts collapses to just the vertical padding.
pub fn content_height(conflicts: &[SceneConflict]) -> f64 {
    let (rows, dividers) = rows_of(conflicts);
    PAD_V * 2.0 + rows.len() as f64 * ROW_H + dividers.len() as f64 * DIVIDER_H
}

/// Fixed-width, content-height card size for `Presenter::place`.
pub fn content_size(conflicts: &[SceneConflict]) -> DVec2 {
    dvec2(CONFLICT_MAX_W, content_height(conflicts))
}

/// What a pointer landed on. `Trash`/`Body` carry the row index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ConflictHit {
    Trash(usize),
    Body(usize),
    #[default]
    None,
}

/// Classify `point` against `rows`: each row's `trash_rect` is tested BEFORE
/// its `body_rect` (the trash cell is nested at the row's right edge, so
/// checking the body first would always win).
pub fn classify(point: DVec2, rows: &[ConflictRow]) -> ConflictHit {
    for (i, row) in rows.iter().enumerate() {
        if row.trash_rect.contains(point) {
            return ConflictHit::Trash(i);
        }
        if row.body_rect.contains(point) {
            return ConflictHit::Body(i);
        }
    }
    ConflictHit::None
}

/// Emitted on a body press (focus the pair on canvas) or a trash press
/// (delete the placement + re-solve). Never closes the surface — `App` reads
/// this alongside (not instead of) `PopupRootAction`.
#[derive(Clone, Debug, Default)]
pub enum ConflictListAction {
    #[default]
    None,
    Focus {
        subject: String,
        reference: String,
    },
    Delete {
        subject: String,
        reference: String,
    },
}

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.fonts
    use mod.widgets.*
    use mod.text.*

    mod.widgets.ConflictListBase = #(ConflictList::register_widget(vm))

    mod.widgets.ConflictList = set_type_default() do mod.widgets.ConflictListBase{
        width: Fill
        height: Fill
        // Card surface: the shared Atlas `AccentFrame` -- same material as
        // `MenuPopup`'s card.
        draw_frame: mod.draw.AccentFrame{ color: atlas.field_bg }
        // Trash-glyph tint holders, mirroring `MenuPopup`'s row-icon tokens:
        // idle rests in `text`, a hovered row lights to `accent`, the armed
        // (about-to-delete) trash glyph goes `danger`.
        draw_icon_idle +: { color: atlas.text }
        draw_icon_accent +: { color: atlas.accent }
        draw_icon_danger +: { color: atlas.danger }
        // Hairline between conflict groups.
        draw_divider: mod.draw.DrawColor{ color: atlas.accent_soft }
        draw_label +: {
            color: atlas.text
            text_style: fonts.text_menu
        }
    }
}

/// Not yet instantiated as a `PopupRoot` child (that lands in Task 5) — keeps
/// the dead-code gate green in the meantime, mirroring `MenuPopup`.
#[allow(dead_code)]
#[derive(Script, ScriptHook, Widget)]
pub struct ConflictList {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    /// Own draw list, drawn into the WINDOW OVERLAY (`begin_overlay_reuse`) --
    /// exactly `MenuPopup`'s idiom -- so the card escapes the body's clip and
    /// its recorded row rects land in the same space as
    /// `MouseMove`/`MouseDown.abs` (no translation needed).
    #[live]
    draw_list: DrawList2d,

    #[redraw]
    #[live]
    draw_frame: DrawColor,
    #[redraw]
    #[live]
    draw_label: DrawText,
    /// Hairline between conflict groups.
    #[redraw]
    #[live]
    draw_divider: DrawColor,
    /// Color-only holders (never drawn): a trash glyph's `color` is copied
    /// from one of these per draw, so the tint RGBA stays in the DSL.
    #[redraw]
    #[live]
    draw_icon_idle: DrawColor,
    #[redraw]
    #[live]
    draw_icon_accent: DrawColor,
    #[redraw]
    #[live]
    draw_icon_danger: DrawColor,
    /// The shared project-tree SDF glyph set; the trash cell tints one field.
    #[live]
    icons: IconSet,

    #[rust]
    rows: Vec<ConflictRow>,
    /// Row indices with a divider drawn immediately above them (from
    /// `rows_of`'s second return value).
    #[rust]
    dividers: Vec<usize>,
    #[rust]
    armed: ConflictHit,
    /// The card's placed rect (origin + size), set by `open`.
    #[rust]
    placed: Rect,
    #[rust]
    open: bool,
}

impl Widget for ConflictList {
    // Event-passive: the parent (`PopupRoot`) drives this through the
    // inherent `Popup` methods below.
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, _walk: Walk) -> DrawStep {
        // Draw nothing when closed (no overlay list to reuse either).
        if !self.open {
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
}

#[allow(dead_code)]
impl ConflictList {
    /// Open the card at `placed` (already clamped by `Presenter::place`
    /// against `content_size`), flattening `conflicts` into rows. Row rects
    /// are (re)filled every frame by `draw`.
    pub fn open(&mut self, cx: &mut Cx, placed: Rect, conflicts: Vec<SceneConflict>) {
        let (raw, dividers) = rows_of(&conflicts);
        self.rows = raw
            .into_iter()
            .map(|(subject, reference, statement)| ConflictRow {
                subject,
                reference,
                statement,
                body_rect: Rect::default(),
                trash_rect: Rect::default(),
            })
            .collect();
        self.dividers = dividers;
        self.placed = placed;
        self.armed = ConflictHit::None;
        self.open = true;
        self.draw_frame.redraw(cx);
    }

    /// Draw the card + rows at the stored `placed` rect. Called from
    /// `draw_walk`.
    pub fn draw(&mut self, cx: &mut Cx2d) {
        if !self.open {
            return;
        }
        // Card surface: source-bright Atlas frame + field-bg fill (see
        // `AccentFrame` in `frame.rs`), same thin hairline weight as the menu.
        self.draw_frame.set_uniform(cx, live_id!(zoom), &[0.6]);
        self.draw_frame.draw_abs(cx, self.placed);

        let left = self.placed.pos.x + PAD_H;
        let right = self.placed.pos.x + self.placed.size.x - PAD_H;
        let label_w = (right - left - TRASH_W).max(0.0);

        // Pass 1: pure geometry. Fills every row's `body_rect`/`trash_rect`
        // without touching any drawable field, so it can index `self.rows`
        // freely alongside `self.dividers`.
        let n = self.rows.len();
        let mut y = self.placed.pos.y + PAD_V;
        for i in 0..n {
            if self.dividers.contains(&i) {
                y += DIVIDER_H;
            }
            self.rows[i].body_rect = Rect {
                pos: dvec2(left, y),
                size: dvec2(label_w, ROW_H),
            };
            self.rows[i].trash_rect = Rect {
                pos: dvec2(left + label_w, y),
                size: dvec2(TRASH_W, ROW_H),
            };
            y += ROW_H;
        }

        // Pass 2: the actual drawing, over an owned clone of the rows so the
        // loop never holds a live borrow of `self.rows` while `self` is
        // reborrowed mutably for `draw_label`/`icons`/`draw_divider`.
        let rows = self.rows.clone();
        for (i, row) in rows.iter().enumerate() {
            if self.dividers.contains(&i) {
                let div = Rect {
                    pos: dvec2(left, row.body_rect.pos.y - DIVIDER_H),
                    size: dvec2(right - left, DIVIDER_H),
                };
                self.draw_divider.draw_abs(cx, div);
            }
            let cy = row.body_rect.pos.y + row.body_rect.size.y * 0.5;
            self.draw_label
                .draw_abs(cx, dvec2(row.body_rect.pos.x, cy - 6.0), &row.statement);

            let tint = match self.armed {
                ConflictHit::Trash(j) if j == i => self.draw_icon_danger.color,
                ConflictHit::Body(j) if j == i => self.draw_icon_accent.color,
                _ => self.draw_icon_idle.color,
            };
            let d = (TRASH_W - TRASH_INSET * 2.0).max(0.0);
            let icon = Rect {
                pos: dvec2(row.trash_rect.pos.x + TRASH_INSET, cy - d * 0.5),
                size: dvec2(d, d),
            };
            self.icons.draw(cx, Icon::Trash, icon, tint);
        }
    }

    /// Reader for `App`: the surface's last-emitted action this frame, if
    /// any (mirrors `ConflictBadge::clicked`).
    pub fn action(&self, actions: &Actions) -> Option<ConflictListAction> {
        actions
            .find_widget_action(self.widget_uid())
            .map(|a| a.cast())
    }
}

impl Popup for ConflictList {
    fn handle(&mut self, cx: &mut Cx, event: &Event) -> PopupVerdict {
        match event {
            Event::MouseMove(e) => {
                self.armed = classify(e.abs, &self.rows);
                self.draw_frame.redraw(cx);
                if self.armed != ConflictHit::None || self.placed.contains(e.abs) {
                    PopupVerdict::Consumed
                } else {
                    PopupVerdict::Ignored
                }
            }
            // The trash and body presses NEVER close the surface -- they emit
            // a separate `ConflictListAction` and stay `Consumed`. Only Esc /
            // outside-click / supersede close it (decided in `PopupRoot`).
            Event::MouseDown(e) if e.button.is_primary() => match classify(e.abs, &self.rows) {
                ConflictHit::Trash(i) => {
                    let (subject, reference) =
                        (self.rows[i].subject.clone(), self.rows[i].reference.clone());
                    cx.widget_action(
                        self.widget_uid(),
                        ConflictListAction::Delete { subject, reference },
                    );
                    PopupVerdict::Consumed
                }
                ConflictHit::Body(i) => {
                    let (subject, reference) =
                        (self.rows[i].subject.clone(), self.rows[i].reference.clone());
                    cx.widget_action(
                        self.widget_uid(),
                        ConflictListAction::Focus { subject, reference },
                    );
                    PopupVerdict::Consumed
                }
                // Outside every row -> Ignored, so a primary press here reads
                // as the outside-click that dismisses (`PopupRoot::decide`).
                ConflictHit::None => PopupVerdict::Ignored,
            },
            _ => PopupVerdict::Consumed,
        }
    }

    fn reset(&mut self) {
        self.open = false;
        self.rows.clear();
        self.dividers.clear();
        self.armed = ConflictHit::None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::SceneRelation;
    use waml::syntax::Direction;

    fn rel(subject: &str, reference: &str) -> SceneRelation {
        SceneRelation {
            subject: subject.to_string(),
            reference: reference.to_string(),
            dir: Direction::LeftOf,
        }
    }

    #[test]
    fn content_height_for_zero_one_and_n_conflicts() {
        assert_eq!(content_height(&[]), PAD_V * 2.0);

        let one = vec![SceneConflict {
            dropped: rel("order", "customer"),
            conflicts_with: vec![rel("customer", "order")],
        }];
        // 2 rows (dropped + 1 conflicts_with), 0 dividers (a single group).
        assert_eq!(content_height(&one), PAD_V * 2.0 + 2.0 * ROW_H);
        assert_eq!(
            content_size(&one),
            dvec2(CONFLICT_MAX_W, content_height(&one))
        );

        let two = vec![
            SceneConflict {
                dropped: rel("a", "b"),
                conflicts_with: vec![rel("b", "a")],
            },
            SceneConflict {
                dropped: rel("c", "d"),
                conflicts_with: vec![rel("d", "c"), rel("e", "c")],
            },
        ];
        // Rows: 2 + 3 = 5; dividers: 1 (between the two groups).
        assert_eq!(content_height(&two), PAD_V * 2.0 + 5.0 * ROW_H + DIVIDER_H);
    }

    #[test]
    fn rows_are_dropped_first_then_conflicts_with_with_group_dividers() {
        let conflicts = vec![
            SceneConflict {
                dropped: rel("a", "b"),
                conflicts_with: vec![rel("b", "a")],
            },
            SceneConflict {
                dropped: rel("c", "d"),
                conflicts_with: vec![rel("d", "c"), rel("e", "c")],
            },
        ];
        let (rows, dividers) = rows_of(&conflicts);
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0].0, "a", "first group's dropped subject");
        assert_eq!(rows[1].0, "b", "first group's conflicts_with subject");
        assert_eq!(rows[2].0, "c", "second group's dropped subject");
        assert_eq!(
            rows[3].0, "d",
            "second group's first conflicts_with subject"
        );
        assert_eq!(
            rows[4].0, "e",
            "second group's second conflicts_with subject"
        );
        // The divider lands at the start of the second group -- never after
        // the last group.
        assert_eq!(dividers, vec![2]);
    }

    #[test]
    fn classify_tests_trash_before_body_and_outside_is_none() {
        let rows = vec![ConflictRow {
            subject: "a".to_string(),
            reference: "b".to_string(),
            statement: "a left of b".to_string(),
            body_rect: Rect {
                pos: dvec2(0.0, 0.0),
                size: dvec2(100.0, ROW_H),
            },
            trash_rect: Rect {
                pos: dvec2(80.0, 0.0),
                size: dvec2(20.0, ROW_H),
            },
        }];
        // Inside the (nested) trash cell -> Trash(0), even though it's also
        // inside the body rect.
        assert_eq!(classify(dvec2(90.0, 5.0), &rows), ConflictHit::Trash(0));
        // Inside the body but not the trash cell -> Body(0).
        assert_eq!(classify(dvec2(10.0, 5.0), &rows), ConflictHit::Body(0));
        // Outside every row -> None.
        assert_eq!(classify(dvec2(500.0, 500.0), &rows), ConflictHit::None);
    }
}
