mod engine;
mod geometry;
mod makepad;

use std::sync::Arc;

use makepad_widgets::DVec2;
use waml_syntax::{DocumentRevision, SyntaxIdentity, TextRange};

pub use crate::selection::Affinity;
pub use engine::{
    BlockSummary, LayoutEngine, LayoutInvalidation, LayoutViewport, ShapedCluster, ShapedRun,
    ShapedGlyph, TextShaper,
};
pub use geometry::{
    BlockGeometry, BlockLayoutData, CaretGeometry, CaretStop, GlyphCluster, LayoutSnapshot,
    LayoutSnapshotMetadata, VisualLine,
};
pub use makepad::{FontResolver, MakepadTextShaper};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LayoutElementId {
    pub owner: SyntaxIdentity,
    pub fragment_ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GeometryElementId {
    pub layout: LayoutElementId,
    pub cluster_ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FontKey(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FontWeight(pub u16);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextMetrics {
    pub font: FontKey,
    pub font_size: f32,
    pub line_spacing: f32,
    pub weight: FontWeight,
    pub italic: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutTextRun {
    pub id: LayoutElementId,
    pub range: TextRange,
    pub metrics: TextMetrics,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeasuredBlock {
    pub id: LayoutElementId,
    pub source_range: TextRange,
    pub size: DVec2,
    pub baseline: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EdgeInsets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColumnAlignment {
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnConstraint {
    pub min_width: f64,
    pub max_width: Option<f64>,
    pub alignment: ColumnAlignment,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlockFlow {
    Paragraph,
    Hanging {
        marker_range: TextRange,
        content_indent: f64,
    },
    Quote,
    Code,
    Table,
    TableRow,
    TableCell {
        column: u32,
    },
    Embedded,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockLayoutSpec {
    pub flow: BlockFlow,
    pub insets: EdgeInsets,
    pub space_before: f64,
    pub space_after: f64,
    pub columns: Arc<[ColumnConstraint]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutBlock {
    pub id: LayoutElementId,
    pub source_range: TextRange,
    pub parent: Option<LayoutElementId>,
    pub spec: BlockLayoutSpec,
}

#[derive(Clone, Debug)]
pub struct LayoutDocument {
    pub revision: DocumentRevision,
    pub content_insets: EdgeInsets,
    pub blocks: Arc<[LayoutBlock]>,
    pub text_runs: Arc<[LayoutTextRun]>,
    pub embedded_blocks: Arc<[MeasuredBlock]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutError {
    RevisionMismatch {
        document: DocumentRevision,
        layout: DocumentRevision,
    },
    GeometryUnavailable,
    InvalidSelection,
    ShapingFailed {
        run: LayoutElementId,
    },
}
