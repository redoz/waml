use std::{ops::Range, sync::Arc};

use makepad_widgets::{dvec2, DVec2};

use crate::{
    edit::{EditCommand, HistoryGroup, MarkdownEditError, ProposedMarkdownEdit},
    layout::{CaretGeometry, LayoutError, LayoutSnapshot},
    selection::{Affinity, Selection, TextPosition},
    session::MarkdownDocumentSession,
    unicode::word_range_at,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionModifier {
    Replace,
    Extend,
    Add,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorKey {
    Enter,
    Tab,
    BackTab,
    Delete,
    Backspace,
    Undo,
    Redo,
    Left { extend: bool },
    Right { extend: bool },
    Up { extend: bool },
    Down { extend: bool },
    SelectAll,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerGesture {
    pub point: DVec2,
    pub clicks: u8,
    pub modifier: SelectionModifier,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EditorInput {
    Text(Arc<str>),
    Paste(Arc<str>),
    Copy,
    Cut,
    Key(EditorKey),
    PointerDown(PointerGesture),
    PointerMove {
        point: DVec2,
    },
    PointerUp,
    ImeStart,
    ImeUpdate {
        preedit: String,
        selection: Range<u32>,
    },
    ImeCommit,
    ImeCancel,
}

#[derive(Default)]
pub struct EditorResponse {
    pub proposals: Vec<ProposedMarkdownEdit>,
    pub clipboard: Option<String>,
    pub request_redraw: bool,
    pub request_ime_at: Option<DVec2>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollState {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollAnchor {
    pub position: TextPosition,
    pub viewport_y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollAdjustment {
    pub scroll_y: f64,
}

#[derive(Default)]
pub struct MarkdownEditorController {
    drag_anchor: Option<TextPosition>,
}

impl MarkdownEditorController {
    pub fn handle(
        &mut self,
        session: &mut MarkdownDocumentSession,
        layout: &LayoutSnapshot,
        input: EditorInput,
    ) -> Result<EditorResponse, MarkdownEditError> {
        let mut response = EditorResponse::default();
        match input {
            EditorInput::Text(text) => {
                self.execute(
                    session,
                    EditCommand::Insert(text),
                    HistoryGroup::named(1),
                    &mut response,
                )?;
            }
            EditorInput::Paste(text) => {
                self.execute(
                    session,
                    EditCommand::Paste(text),
                    HistoryGroup::isolated(),
                    &mut response,
                )?;
            }
            EditorInput::Copy => response.clipboard = Some(copy_selections(session)?),
            EditorInput::Cut => {
                if session.is_read_only() {
                    response.clipboard = Some(copy_selections(session)?);
                } else {
                    let outcome = session.execute(EditCommand::Cut, HistoryGroup::isolated())?;
                    response.clipboard = outcome.clipboard;
                    if let Some(proposal) = outcome.proposal {
                        response.proposals.push(proposal);
                    }
                }
            }
            EditorInput::Key(key) => self.handle_key(session, layout, key, &mut response)?,
            EditorInput::PointerDown(gesture) => {
                let position = layout.point_to_source(gesture.point);
                match gesture.clicks {
                    3.. => {
                        session.select_line_at(position.offset)?;
                        self.drag_anchor = None;
                    }
                    2 => {
                        session.select_word_at(position.offset)?;
                        self.drag_anchor = None;
                    }
                    _ => {
                        let selection = match gesture.modifier {
                            SelectionModifier::Replace => Selection::caret(position),
                            SelectionModifier::Extend => {
                                let primary = session.selections().primary();
                                let anchor = word_range_at(
                                    session.snapshot().text().shared().as_str(),
                                    primary.anchor.offset.to_usize(),
                                )
                                .filter(|(start, _)| position.offset.to_usize() >= *start)
                                .and_then(|(start, _)| {
                                    waml_syntax::TextSize::try_from_usize(start).ok()
                                })
                                .map_or(primary.anchor, |offset| {
                                    TextPosition::new(offset, crate::selection::Affinity::Before)
                                });
                                Selection::new(anchor, position)
                            }
                            SelectionModifier::Add => Selection::caret(position),
                        };
                        match gesture.modifier {
                            SelectionModifier::Add => session.add_selection(selection)?,
                            _ => session.replace_primary_selection(selection)?,
                        }
                        self.drag_anchor = Some(selection.anchor);
                    }
                }
                response.request_redraw = true;
            }
            EditorInput::PointerMove { point } => {
                if let Some(anchor) = self.drag_anchor {
                    session.replace_primary_selection(Selection::new(
                        anchor,
                        layout.point_to_source(point),
                    ))?;
                    response.request_redraw = true;
                }
            }
            EditorInput::PointerUp => self.drag_anchor = None,
            EditorInput::ImeStart => {
                if !session.is_read_only() {
                    session.begin_ime()?;
                }
            }
            EditorInput::ImeUpdate { preedit, selection } => {
                if !session.is_read_only() {
                    session.update_ime(&preedit, selection)?;
                    response.request_redraw = true;
                }
            }
            EditorInput::ImeCommit => {
                if !session.is_read_only() {
                    if let Some(proposal) = session.commit_ime(HistoryGroup::named(2))? {
                        response.proposals.push(proposal);
                    }
                }
            }
            EditorInput::ImeCancel => {
                session.cancel_ime();
                response.request_redraw = true;
            }
        }
        response.request_ime_at = caret_geometry(layout, session.selections().primary().cursor)
            .map(|(_, caret)| caret.rect.pos - dvec2(session.scroll().x, session.scroll().y));
        Ok(response)
    }

    pub fn ensure_primary_caret_visible(
        &self,
        session: &mut MarkdownDocumentSession,
        layout: &LayoutSnapshot,
        viewport_height: f64,
    ) -> Result<ScrollAdjustment, LayoutError> {
        verify_revision(session, layout)?;
        let (_, caret) = caret_geometry(layout, session.selections().primary().cursor)
            .ok_or(LayoutError::GeometryUnavailable)?;
        let height = caret.rect.size.y;
        let mut scroll_y = session.scroll().y;
        let caret_top = caret.rect.pos.y;
        let caret_bottom = caret_top + height;
        if caret_top < scroll_y {
            scroll_y = (caret_top - height).max(0.0);
        } else if caret_bottom > scroll_y + viewport_height {
            scroll_y = (caret_bottom + height * 2.0 - viewport_height).min(caret_top);
        }
        scroll_y = clamp_scroll_y(scroll_y, layout, viewport_height);
        session.set_scroll(ScrollState {
            x: session.scroll().x.max(0.0),
            y: scroll_y,
        });
        Ok(ScrollAdjustment { scroll_y })
    }

    pub fn capture_scroll_anchor(
        &self,
        session: &MarkdownDocumentSession,
        layout: &LayoutSnapshot,
    ) -> Result<ScrollAnchor, LayoutError> {
        verify_revision(session, layout)?;
        let (position, caret) = caret_geometry(layout, session.selections().primary().cursor)
            .ok_or(LayoutError::GeometryUnavailable)?;
        Ok(ScrollAnchor {
            position,
            viewport_y: caret.rect.pos.y - session.scroll().y,
        })
    }

    pub fn restore_scroll_anchor(
        &self,
        session: &mut MarkdownDocumentSession,
        layout: &LayoutSnapshot,
        anchor: ScrollAnchor,
    ) -> Result<ScrollAdjustment, LayoutError> {
        verify_revision(session, layout)?;
        let caret = layout
            .source_to_point(anchor.position)
            .ok_or(LayoutError::GeometryUnavailable)?;
        let scroll_y = (caret.rect.pos.y - anchor.viewport_y).max(0.0);
        session.set_scroll(ScrollState {
            x: session.scroll().x.max(0.0),
            y: scroll_y,
        });
        Ok(ScrollAdjustment { scroll_y })
    }

    fn execute(
        &self,
        session: &mut MarkdownDocumentSession,
        command: EditCommand,
        group: HistoryGroup,
        response: &mut EditorResponse,
    ) -> Result<(), MarkdownEditError> {
        if session.is_read_only() {
            return Ok(());
        }
        let outcome = session.execute(command, group)?;
        if let Some(proposal) = outcome.proposal {
            response.proposals.push(proposal);
        }
        response.clipboard = outcome.clipboard;
        response.request_redraw = true;
        Ok(())
    }

    fn handle_key(
        &self,
        session: &mut MarkdownDocumentSession,
        layout: &LayoutSnapshot,
        key: EditorKey,
        response: &mut EditorResponse,
    ) -> Result<(), MarkdownEditError> {
        match key {
            EditorKey::Enter => self.execute(
                session,
                EditCommand::Insert(Arc::from("\n")),
                HistoryGroup::isolated(),
                response,
            )?,
            EditorKey::Tab => self.execute(
                session,
                EditCommand::Indent { spaces: 4 },
                HistoryGroup::isolated(),
                response,
            )?,
            EditorKey::BackTab => self.execute(
                session,
                EditCommand::Outdent { spaces: 4 },
                HistoryGroup::isolated(),
                response,
            )?,
            EditorKey::Delete => self.execute(
                session,
                EditCommand::DeleteForward,
                HistoryGroup::named(3),
                response,
            )?,
            EditorKey::Backspace => self.execute(
                session,
                EditCommand::DeleteBackward,
                HistoryGroup::named(3),
                response,
            )?,
            EditorKey::Undo => {
                if !session.is_read_only() {
                    if let Some(proposal) = session.undo()? {
                        response.proposals.push(proposal);
                    }
                }
            }
            EditorKey::Redo => {
                if !session.is_read_only() {
                    if let Some(proposal) = session.redo()? {
                        response.proposals.push(proposal);
                    }
                }
            }
            EditorKey::Left { extend } => session.move_left(extend)?,
            EditorKey::Right { extend } => session.move_right(extend)?,
            EditorKey::Up { extend } => {
                session.move_vertical(layout, -1, extend).map_err(|_| {
                    MarkdownEditError::InvalidBoundary {
                        offset: session.selections().primary().cursor.offset,
                    }
                })?
            }
            EditorKey::Down { extend } => {
                session.move_vertical(layout, 1, extend).map_err(|_| {
                    MarkdownEditError::InvalidBoundary {
                        offset: session.selections().primary().cursor.offset,
                    }
                })?
            }
            EditorKey::SelectAll => session.select_all()?,
        }
        response.request_redraw = true;
        Ok(())
    }
}

fn copy_selections(session: &MarkdownDocumentSession) -> Result<String, MarkdownEditError> {
    session
        .selections()
        .as_slice()
        .iter()
        .map(|selection| {
            session
                .snapshot()
                .text()
                .slice(selection.range())
                .map(str::to_owned)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.concat())
        .map_err(MarkdownEditError::from)
}

fn caret_geometry(
    layout: &LayoutSnapshot,
    position: TextPosition,
) -> Option<(TextPosition, CaretGeometry)> {
    layout
        .source_to_point(position)
        .map(|caret| (position, caret))
        .or_else(|| {
            let alternate = TextPosition::new(
                position.offset,
                match position.affinity {
                    Affinity::Before => Affinity::After,
                    Affinity::After => Affinity::Before,
                },
            );
            layout
                .source_to_point(alternate)
                .map(|caret| (alternate, caret))
        })
}

fn verify_revision(
    session: &MarkdownDocumentSession,
    layout: &LayoutSnapshot,
) -> Result<(), LayoutError> {
    if session.local_revision() == layout.revision() {
        Ok(())
    } else {
        Err(LayoutError::RevisionMismatch {
            document: session.local_revision(),
            layout: layout.revision(),
        })
    }
}

fn clamp_scroll_y(scroll_y: f64, layout: &LayoutSnapshot, viewport_height: f64) -> f64 {
    scroll_y.clamp(0.0, (layout.content_size().y - viewport_height).max(0.0))
}
