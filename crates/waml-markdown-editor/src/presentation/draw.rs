//! Pure presentation draw-command construction.

use std::sync::Arc;

use makepad_widgets::{dvec2, DVec2, Rect};
use waml_syntax::{DocumentRevision, SyntaxIdentity, TextRange};

use crate::{
    ime::ImeComposition,
    layout::{GeometryElementId, LayoutDocument, LayoutElementId, LayoutSnapshot, TextMetrics},
    selection::{Affinity, Selection, SelectionSet, TextPosition},
    widget::DrawLayer,
};

use super::{
    BlockDecorationRole, ColorRole, EmbeddedAssetFrame, EmbeddedState, PresentationError,
    PresentationItem, PresentationPlan, PresentationStyles,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentedDiagnosticSeverity {
    Error,
    Warning,
    Information,
}

/// Gap between a row's last glyph and its diagnostic message.
pub const MESSAGE_GAP: f64 = 12.0;

/// Error > Warning > Information.
fn severity_rank(severity: PresentedDiagnosticSeverity) -> u8 {
    match severity {
        PresentedDiagnosticSeverity::Error => 2,
        PresentedDiagnosticSeverity::Warning => 1,
        PresentedDiagnosticSeverity::Information => 0,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentedDiagnostic {
    pub revision: DocumentRevision,
    pub range: TextRange,
    pub severity: PresentedDiagnosticSeverity,
    pub message: Arc<str>,
}

#[derive(Clone)]
pub struct InstalledPresentation {
    pub revision: DocumentRevision,
    pub plan: Arc<PresentationPlan>,
    pub styles: Arc<PresentationStyles>,
    pub layout_document: Arc<LayoutDocument>,
    pub diagnostics: Arc<[PresentedDiagnostic]>,
    pub assets: Arc<EmbeddedAssetFrame>,
}

impl InstalledPresentation {
    pub fn new(
        plan: Arc<PresentationPlan>,
        styles: Arc<PresentationStyles>,
        layout_document: Arc<LayoutDocument>,
        diagnostics: Arc<[PresentedDiagnostic]>,
        assets: Arc<EmbeddedAssetFrame>,
    ) -> Result<Arc<Self>, PresentationError> {
        let presentation = Arc::new(Self {
            revision: plan.revision,
            plan,
            styles,
            layout_document,
            diagnostics,
            assets,
        });
        presentation.validate()?;
        Ok(presentation)
    }

    pub fn validate(&self) -> Result<(), PresentationError> {
        require_revision(self.revision, self.plan.revision, "plan")?;
        require_revision(
            self.revision,
            self.layout_document.revision,
            "layout_document",
        )?;
        for diagnostic in self.diagnostics.iter() {
            require_revision(self.revision, diagnostic.revision, "diagnostic")?;
        }
        require_revision(self.revision, self.assets.revision, "assets")
    }
}

#[derive(Clone)]
pub struct PresentationFrame {
    pub revision: DocumentRevision,
    pub layout: Arc<LayoutSnapshot>,
    pub active_owners: Arc<[SyntaxIdentity]>,
    pub diagnostics: Arc<[PresentedDiagnostic]>,
    pub assets: Arc<EmbeddedAssetFrame>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedTextStyle {
    pub metrics: TextMetrics,
    pub color: ColorRole,
    pub background: Option<ColorRole>,
    pub underline: bool,
    pub strikethrough: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecorationRole {
    LinkUnderline,
    DiagnosticUnderline(PresentedDiagnosticSeverity),
}

#[derive(Clone, Debug, PartialEq)]
pub enum DrawCommand {
    BlockBackground {
        id: LayoutElementId,
        rect: Rect,
        role: BlockDecorationRole,
    },
    Selection {
        rect: Rect,
    },
    Text {
        id: GeometryElementId,
        range: TextRange,
        rect: Rect,
        style: ResolvedTextStyle,
    },
    Decoration {
        range: TextRange,
        rects: Arc<[Rect]>,
        role: DecorationRole,
    },
    EmbeddedBlock {
        id: LayoutElementId,
        rect: Rect,
        state: EmbeddedState,
    },
    CaretAndIme {
        caret: Rect,
        composition: Arc<[Rect]>,
    },
    /// A diagnostic's message drawn at the end of the visual row it ends on.
    /// Pure decoration: never document text, never selectable, invisible to
    /// caret motion and hit-testing.
    DiagnosticMessage {
        line: TextRange,
        rect: Rect,
        text: Arc<str>,
        severity: PresentedDiagnosticSeverity,
    },
}

impl DrawCommand {
    pub fn layer(&self) -> DrawLayer {
        match self {
            Self::BlockBackground { .. } => DrawLayer::BlockBackground,
            Self::Selection { .. } => DrawLayer::Selection,
            Self::Text { .. } => DrawLayer::Text,
            Self::Decoration { .. } => DrawLayer::Decoration,
            Self::EmbeddedBlock { .. } => DrawLayer::EmbeddedBlock,
            Self::CaretAndIme { .. } => DrawLayer::CaretAndIme,
            Self::DiagnosticMessage { .. } => DrawLayer::Decoration,
        }
    }

    pub fn translated(&self, origin: DVec2) -> Self {
        let translate = |rect: Rect| Rect {
            pos: rect.pos + origin,
            ..rect
        };
        match self {
            Self::BlockBackground { id, rect, role } => Self::BlockBackground {
                id: *id,
                rect: translate(*rect),
                role: *role,
            },
            Self::Selection { rect } => Self::Selection {
                rect: translate(*rect),
            },
            Self::Text {
                id,
                range,
                rect,
                style,
            } => Self::Text {
                id: *id,
                range: *range,
                rect: translate(*rect),
                style: *style,
            },
            Self::Decoration { range, rects, role } => Self::Decoration {
                range: *range,
                rects: rects.iter().copied().map(translate).collect(),
                role: *role,
            },
            Self::EmbeddedBlock { id, rect, state } => Self::EmbeddedBlock {
                id: *id,
                rect: translate(*rect),
                state: state.clone(),
            },
            Self::CaretAndIme { caret, composition } => Self::CaretAndIme {
                caret: translate(*caret),
                composition: composition.iter().copied().map(translate).collect(),
            },
            Self::DiagnosticMessage {
                line,
                rect,
                text,
                severity,
            } => Self::DiagnosticMessage {
                line: *line,
                rect: translate(*rect),
                text: text.clone(),
                severity: *severity,
            },
        }
    }
}

pub fn build_draw_commands(
    frame: &PresentationFrame,
    plan: &PresentationPlan,
    styles: &PresentationStyles,
    selection: &SelectionSet,
    ime: Option<&ImeComposition>,
) -> Result<Arc<[DrawCommand]>, PresentationError> {
    require_revision(plan.revision, frame.revision, "frame")?;
    require_revision(plan.revision, frame.layout.revision(), "layout")?;
    require_revision(plan.revision, selection.revision(), "selection")?;
    require_revision(plan.revision, frame.assets.revision, "assets")?;
    for diagnostic in frame.diagnostics.iter() {
        require_revision(plan.revision, diagnostic.revision, "diagnostic")?;
    }
    if let Some(ime) = ime {
        require_revision(plan.revision, ime.base_revision(), "ime")?;
    }

    let mut commands = Vec::new();

    for item in plan.items.iter() {
        let PresentationItem::BlockDecoration { id, kind, .. } = item else {
            continue;
        };
        // The plan records that a marker is a bullet; the editor surface shows
        // raw markdown, so its bullet is the `-` glyph itself. Only the reading
        // view substitutes the decoration for the marker.
        if kind.role() == BlockDecorationRole::ListBullet {
            continue;
        }
        let id = layout_id(id.owner, id.fragment_ordinal);
        if let Some(block) = frame
            .layout
            .visible_blocks()
            .iter()
            .find(|block| block.id == id)
        {
            let mut rect = block.rect;
            match kind.role() {
                BlockDecorationRole::QuoteRule => {
                    rect.size.x = styles.spacing().quote_rule;
                }
                BlockDecorationRole::ThematicRule => {
                    let height = styles.spacing().quote_rule.min(rect.size.y);
                    rect.pos.y += (rect.size.y - height) * 0.5;
                    rect.size.y = height;
                }
                _ => {}
            }
            commands.push(DrawCommand::BlockBackground {
                id,
                rect,
                role: kind.role(),
            });
        }
    }

    for selection in selection.as_slice() {
        if selection.is_empty() {
            continue;
        }
        for rect in frame.layout.selection_rects(*selection).unwrap_or_default() {
            commands.push(DrawCommand::Selection { rect });
        }
    }

    for cluster in frame.layout.glyph_clusters() {
        let Some((owner, style, hidden)) = plan.items.iter().find_map(|item| match item {
            PresentationItem::TextRun {
                id,
                range,
                style,
                hidden,
                ..
            } if range.start() <= cluster.source_range.start()
                && cluster.source_range.end() <= range.end() =>
            {
                Some((id.owner, *style, *hidden))
            }
            _ => None,
        }) else {
            continue;
        };
        // A hidden run's clusters exist for caret and coverage only.
        if hidden {
            continue;
        }
        let active = frame.active_owners.contains(&owner);
        commands.push(DrawCommand::Text {
            id: cluster.id,
            range: cluster.source_range,
            rect: cluster.rect,
            style: ResolvedTextStyle {
                metrics: cluster.metrics,
                color: if active {
                    style.active_color
                } else {
                    style.color
                },
                background: style.background,
                underline: style.underline,
                strikethrough: style.strikethrough,
            },
        });
    }

    for link in plan.links.iter() {
        let rects = rects_for_range(&frame.layout, link.source_range);
        if !rects.is_empty() {
            commands.push(DrawCommand::Decoration {
                range: link.source_range,
                rects,
                role: DecorationRole::LinkUnderline,
            });
        }
    }
    for diagnostic in frame.diagnostics.iter() {
        let rects = rects_for_range(&frame.layout, diagnostic.range);
        if !rects.is_empty() {
            commands.push(DrawCommand::Decoration {
                range: diagnostic.range,
                rects,
                role: DecorationRole::DiagnosticUnderline(diagnostic.severity),
            });
        }
    }

    // End-of-row diagnostic messages: one per visual line, computed here so
    // placement lives in the pure, unit-tested draw-command layer.
    let mut message_lines: Vec<(usize, Vec<&PresentedDiagnostic>)> = Vec::new();
    for diagnostic in frame.diagnostics.iter() {
        let end = diagnostic.range.end();
        // The first visual line whose source range contains the END offset;
        // an offset on a line boundary belongs to the earlier line.
        let Some(index) =
            frame.layout.visual_lines().iter().position(|line| {
                line.source_range.start() <= end && end <= line.source_range.end()
            })
        else {
            continue; // off-viewport; nothing to say
        };
        match message_lines.iter_mut().find(|(line, _)| *line == index) {
            Some((_, bucket)) => bucket.push(diagnostic),
            None => message_lines.push((index, vec![diagnostic])),
        }
    }
    message_lines.sort_by_key(|(index, _)| *index);
    for (index, bucket) in message_lines {
        // Worst severity wins; ties break on the earliest start, which makes
        // the order total: two diagnostics never contend for the same slot.
        let winner = bucket
            .iter()
            .copied()
            .min_by_key(|diagnostic| {
                (
                    std::cmp::Reverse(severity_rank(diagnostic.severity)),
                    diagnostic.range.start(),
                )
            })
            .expect("a bucket is created non-empty");
        let line = &frame.layout.visual_lines()[index];
        // The maximum right edge of the line's glyph clusters; a line with no
        // clusters (blank line) anchors at its own left edge.
        let text_right = frame
            .layout
            .glyph_clusters()
            .iter()
            .filter(|cluster| {
                cluster.source_range.start() < line.source_range.end()
                    && line.source_range.start() < cluster.source_range.end()
            })
            .map(|cluster| cluster.rect.pos.x + cluster.rect.size.x)
            .fold(None::<f64>, |best, right| {
                Some(best.map_or(right, |best| best.max(right)))
            });
        let x = text_right.unwrap_or(line.rect.pos.x) + MESSAGE_GAP;
        let mut text = winner.message.to_string();
        let others = bucket.len() - 1;
        if others > 0 {
            text.push_str(&format!(" +{others}"));
        }
        // Ellipsize against the viewport: the message renders in the mono
        // face, so width is chars * advance (wide characters overrun
        // slightly; accepted -- diagnostic messages are ASCII parser text).
        let advance = styles.diagnostic_message_advance();
        let budget = ((frame.layout.viewport_width() - x) / advance).floor();
        if budget < 1.0 {
            continue; // no wrapping, no row growth, no hard clip
        }
        let max_chars = budget as usize;
        if text.chars().count() > max_chars {
            text = text.chars().take(max_chars.saturating_sub(1)).collect();
            text.push('…');
        }
        let width = text.chars().count() as f64 * advance;
        commands.push(DrawCommand::DiagnosticMessage {
            line: line.source_range,
            rect: Rect {
                pos: dvec2(x, line.rect.pos.y),
                size: dvec2(width, line.rect.size.y),
            },
            text: text.into(),
            severity: winner.severity,
        });
    }

    for item in plan.items.iter() {
        let PresentationItem::EmbeddedBlock { id, .. } = item else {
            continue;
        };
        let layout_id = layout_id(id.owner, id.fragment_ordinal);
        let Some(block) = frame
            .layout
            .visible_blocks()
            .iter()
            .find(|block| block.id == layout_id)
        else {
            continue;
        };
        let state = frame
            .assets
            .items
            .iter()
            .find(|(candidate, _)| candidate == id)
            .map_or(EmbeddedState::Loading, |(_, state)| state.clone());
        commands.push(DrawCommand::EmbeddedBlock {
            id: layout_id,
            rect: block.rect,
            state,
        });
    }

    if let Some(caret) = frame
        .layout
        .source_to_point(selection.primary().cursor)
        .map(|geometry| geometry.rect)
    {
        let composition = ime.map_or_else(
            || Arc::from([]),
            |ime| rects_for_range(&frame.layout, ime.replace_range()),
        );
        commands.push(DrawCommand::CaretAndIme { caret, composition });
    }

    Ok(commands.into())
}

fn require_revision(
    expected: DocumentRevision,
    actual: DocumentRevision,
    component: &'static str,
) -> Result<(), PresentationError> {
    if expected == actual {
        return Ok(());
    }
    Err(PresentationError::RevisionMismatch {
        expected,
        actual,
        component,
    })
}

fn rects_for_range(layout: &LayoutSnapshot, range: TextRange) -> Arc<[Rect]> {
    layout
        .selection_rects(Selection::new(
            TextPosition::new(range.start(), Affinity::Before),
            TextPosition::new(range.end(), Affinity::After),
        ))
        .unwrap_or_default()
        .into()
}

fn layout_id(owner: SyntaxIdentity, fragment_ordinal: u32) -> LayoutElementId {
    LayoutElementId {
        owner,
        fragment_ordinal,
    }
}
