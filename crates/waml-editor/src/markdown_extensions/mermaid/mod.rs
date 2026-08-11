use std::sync::Arc;

use waml_markdown_editor::reading::RenderedBlockSvg;

mod cache;
mod error;
mod renderer;

pub(super) use renderer::MermaidRenderer;

pub(super) type BlockRenderResult = Result<RenderedBlockSvg, Arc<str>>;

pub(super) fn renderer() -> Arc<MermaidRenderer> {
    Arc::new(MermaidRenderer::default())
}
