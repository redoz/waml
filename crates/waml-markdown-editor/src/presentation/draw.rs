//! Pure presentation draw-command construction.

use std::sync::Arc;

use makepad_widgets::Rect;
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
        let Some((owner, style)) = plan.items.iter().find_map(|item| match item {
            PresentationItem::TextRun {
                id, range, style, ..
            } if range.start() <= cluster.source_range.start()
                && cluster.source_range.end() <= range.end() =>
            {
                Some((id.owner, *style))
            }
            _ => None,
        }) else {
            continue;
        };
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
