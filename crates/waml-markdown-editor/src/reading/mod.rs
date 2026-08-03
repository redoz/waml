//! The markdown **reading view**: a block-rendered presentation of a document.
//!
//! The reading view is a different surface from the editor. The editor styles
//! RAW markdown, so a source line is a visual row. The reading view RENDERS
//! markdown, so there is no source-line correspondence at all. The split seam
//! is `PresentationPlan`: both surfaces consume the same compiled plan, the
//! same styles, the same decorations, highlighters and assets. Neither shares
//! the other's layout engine, motion, selection, input or IME.

pub mod model;

pub use model::{
    build_reading_document, ReadingBlock, ReadingBlockKind, ReadingDocument, ReadingError,
    ReadingPiece,
};
