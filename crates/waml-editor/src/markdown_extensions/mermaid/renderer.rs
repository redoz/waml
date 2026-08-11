use std::sync::{Arc, Mutex};

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use makepad_widgets::{looks_like_svg, makepad_draw::svg::parse_svg};
use merman::svg::{
    HeadlessError, HeadlessRenderer, HostTheme, HostThemePreset, Presentation,
    RenderResourceProfile,
};
use waml_markdown_editor::reading::{
    BlockExtensionAppearance, BlockExtensionRequest, RenderedBlockSvg,
};

use super::{
    cache::{CacheKey, MermaidCache},
    error::MermaidRenderError,
    BlockRenderResult,
};

pub(super) const MAX_SOURCE_BYTES: usize = 256 * 1024;
pub(super) const MAX_SVG_BYTES: usize = 8 * 1024 * 1024;
pub(super) const MAX_LOGICAL_SIDE: f64 = 16_384.0;

#[derive(Debug, Default)]
pub(in crate::markdown_extensions) struct MermaidRenderer {
    cache: Mutex<MermaidCache>,
    #[cfg(test)]
    uncached_renders: AtomicUsize,
}

impl MermaidRenderer {
    pub(in crate::markdown_extensions) fn language(&self) -> &'static str {
        "mermaid"
    }

    pub(in crate::markdown_extensions) fn cached(
        &self,
        request: &BlockExtensionRequest,
    ) -> Option<BlockRenderResult> {
        let key = CacheKey::from_request(request);
        self.cache.lock().expect("Mermaid cache poisoned").get(&key)
    }

    pub(in crate::markdown_extensions) fn render_and_cache(
        &self,
        request: &BlockExtensionRequest,
    ) -> BlockRenderResult {
        let key = CacheKey::from_request(request);
        if let Some(hit) = self.cache.lock().expect("Mermaid cache poisoned").get(&key) {
            return hit;
        }

        #[cfg(test)]
        self.uncached_renders.fetch_add(1, Ordering::Relaxed);
        let rendered = render_uncached(request);

        let mut cache = self.cache.lock().expect("Mermaid cache poisoned");
        if let Some(winner) = cache.get(&key) {
            return winner;
        }
        cache.insert(key, rendered.clone());
        rendered
    }

    #[cfg(test)]
    fn uncached_render_count(&self) -> usize {
        self.uncached_renders.load(Ordering::Relaxed)
    }
}

pub(super) fn render_uncached(request: &BlockExtensionRequest) -> BlockRenderResult {
    if request.content.len() > MAX_SOURCE_BYTES {
        return Err(MermaidRenderError::SourceTooLarge.viewer_message());
    }

    let preset = match request.appearance {
        BlockExtensionAppearance::Light => HostThemePreset::EditorLight,
        BlockExtensionAppearance::Dark => HostThemePreset::EditorDark,
    };
    let diagram_id = format!(
        "waml-mermaid-{}-{}",
        request.item.owner.get(),
        request.item.fragment_ordinal
    );
    let renderer = HeadlessRenderer::new()
        .with_presentation(Presentation::new().with_theme(HostTheme::from_preset(preset)))
        .with_resource_profile(RenderResourceProfile::Constrained)
        .with_vendored_text_measurer()
        .with_diagram_id(&diagram_id);

    let sealed = match renderer.render_resvg_compatible_svg_sync(&request.content) {
        Ok(Some(sealed)) => sealed,
        Ok(None) => return Err(MermaidRenderError::DiagramNotDetected.viewer_message()),
        Err(error @ HeadlessError::Parse(merman::Error::DetectType(_))) => {
            tracing::warn!(?error, "Merman did not detect a Mermaid diagram");
            return Err(MermaidRenderError::DiagramNotDetected.viewer_message());
        }
        Err(error) => {
            tracing::warn!(?error, "Merman could not render Mermaid source");
            return Err(MermaidRenderError::InvalidDiagram.viewer_message());
        }
    };

    validate_svg(sealed.into_string()).map_err(MermaidRenderError::viewer_message)
}

fn validate_svg(svg: String) -> Result<RenderedBlockSvg, MermaidRenderError> {
    if svg.len() > MAX_SVG_BYTES {
        return Err(MermaidRenderError::SvgTooLarge);
    }
    validate_svg_safety(&svg)?;
    if !looks_like_svg(svg.as_bytes()) {
        return Err(MermaidRenderError::InvalidSvg);
    }

    let (width, height) = parse_svg(&svg).logical_size();
    let logical_size = (f64::from(width), f64::from(height));
    validate_dimensions(logical_size)?;
    Ok(RenderedBlockSvg {
        data: Arc::<[u8]>::from(svg.into_bytes()),
        logical_size,
    })
}

fn validate_dimensions(size: (f64, f64)) -> Result<(), MermaidRenderError> {
    let (width, height) = size;
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || width > MAX_LOGICAL_SIDE
        || height > MAX_LOGICAL_SIDE
    {
        return Err(MermaidRenderError::InvalidDimensions);
    }
    Ok(())
}

fn validate_svg_safety(svg: &str) -> Result<(), MermaidRenderError> {
    let lower = svg.to_ascii_lowercase();
    for forbidden in [
        "<!doctype",
        "<!entity",
        "<?xml-stylesheet",
        "javascript:",
        "vbscript:",
        "data:text/html",
        "@import",
    ] {
        if lower.contains(forbidden) {
            return Err(MermaidRenderError::UnsafeSvg);
        }
    }

    validate_css_urls(&lower)?;
    let mut cursor = 0;
    while let Some(relative_start) = lower[cursor..].find('<') {
        let start = cursor + relative_start;
        let Some(relative_end) = lower[start..].find('>') else {
            return Err(MermaidRenderError::InvalidSvg);
        };
        let end = start + relative_end;
        validate_tag(&lower[start + 1..end])?;
        cursor = end + 1;
    }
    Ok(())
}

fn validate_css_urls(lower: &str) -> Result<(), MermaidRenderError> {
    let mut remainder = lower;
    while let Some(start) = remainder.find("url(") {
        let value_start = start + "url(".len();
        let Some(relative_end) = remainder[value_start..].find(')') else {
            return Err(MermaidRenderError::UnsafeSvg);
        };
        let value = remainder[value_start..value_start + relative_end]
            .trim()
            .trim_matches(['\'', '"']);
        if !value.starts_with('#') {
            return Err(MermaidRenderError::UnsafeSvg);
        }
        remainder = &remainder[value_start + relative_end + 1..];
    }
    Ok(())
}

fn validate_tag(tag: &str) -> Result<(), MermaidRenderError> {
    let tag = tag.trim_start();
    if tag.starts_with('/') || tag.starts_with('?') || tag.starts_with('!') {
        return Ok(());
    }

    let name_end = tag
        .find(|character: char| character.is_ascii_whitespace() || character == '/')
        .unwrap_or(tag.len());
    let name = &tag[..name_end];
    let local_name = name.rsplit(':').next().unwrap_or(name);
    if matches!(
        local_name,
        "script" | "foreignobject" | "iframe" | "object" | "embed" | "audio" | "video"
    ) {
        return Err(MermaidRenderError::UnsafeSvg);
    }

    validate_attributes(&tag[name_end..])
}

fn validate_attributes(mut attributes: &str) -> Result<(), MermaidRenderError> {
    while !attributes.trim_start().is_empty() {
        attributes = attributes.trim_start();
        if attributes.starts_with('/') {
            return Ok(());
        }
        let name_end = attributes
            .find(|character: char| {
                character.is_ascii_whitespace() || character == '=' || character == '/'
            })
            .unwrap_or(attributes.len());
        if name_end == 0 {
            return Err(MermaidRenderError::InvalidSvg);
        }
        let name = &attributes[..name_end];
        attributes = attributes[name_end..].trim_start();
        let local_name = name.rsplit(':').next().unwrap_or(name);
        if local_name.starts_with("on") {
            return Err(MermaidRenderError::UnsafeSvg);
        }
        if !attributes.starts_with('=') {
            continue;
        }
        attributes = attributes[1..].trim_start();
        let (value, remainder) = take_attribute_value(attributes)?;
        if matches!(local_name, "href" | "src") && !value.is_empty() && !value.starts_with('#') {
            return Err(MermaidRenderError::UnsafeSvg);
        }
        attributes = remainder;
    }
    Ok(())
}

fn take_attribute_value(attributes: &str) -> Result<(&str, &str), MermaidRenderError> {
    let Some(first) = attributes.as_bytes().first().copied() else {
        return Err(MermaidRenderError::InvalidSvg);
    };
    if matches!(first, b'\'' | b'"') {
        let quote = first as char;
        let Some(relative_end) = attributes[1..].find(quote) else {
            return Err(MermaidRenderError::InvalidSvg);
        };
        let end = relative_end + 1;
        Ok((&attributes[1..end], &attributes[end + 1..]))
    } else {
        let end = attributes
            .find(|character: char| character.is_ascii_whitespace() || character == '/')
            .unwrap_or(attributes.len());
        Ok((&attributes[..end], &attributes[end..]))
    }
}

#[cfg(test)]
mod tests {
    use std::{str, sync::Arc};

    use makepad_widgets::{looks_like_svg, makepad_draw::svg::parse_svg};
    use waml_markdown_editor::{
        presentation::{EmbeddedBlockRole, PresentationItemId, PresentationRole},
        reading::{BlockExtensionAppearance, BlockExtensionRequest, BlockExtensionRequestId},
        syntax::{DocumentRevision, TextRange, TextSize},
    };
    use waml_syntax::SyntaxIdentity;

    use super::{
        render_uncached, validate_dimensions, validate_svg, MermaidRenderer, MAX_LOGICAL_SIDE,
        MAX_SOURCE_BYTES, MAX_SVG_BYTES,
    };
    use crate::markdown_extensions::mermaid::error::MermaidRenderError;

    fn request(
        content: impl Into<Arc<str>>,
        appearance: BlockExtensionAppearance,
    ) -> BlockExtensionRequest {
        BlockExtensionRequest {
            request_id: BlockExtensionRequestId(1),
            revision: DocumentRevision::new(3),
            item: PresentationItemId {
                owner: SyntaxIdentity::from_raw_for_test(5),
                role: PresentationRole::Embedded(EmbeddedBlockRole::FencedExtension),
                fragment_ordinal: 7,
            },
            source_range: TextRange::new(TextSize::new(0), TextSize::new(0)).unwrap(),
            content_range: TextRange::new(TextSize::new(0), TextSize::new(0)).unwrap(),
            language: Arc::from("mermaid"),
            content: content.into(),
            appearance,
        }
    }

    #[test]
    fn supported_mermaid_families_render_makepad_parseable_svg() {
        let fixtures = [
            "flowchart TD\nA[Start] --> B[Done]",
            "sequenceDiagram\nAlice->>Bob: Hello",
            "classDiagram\nclass Animal",
            "stateDiagram-v2\n[*] --> Still\nStill --> [*]",
        ];

        for source in fixtures {
            let rendered = render_uncached(&request(source, BlockExtensionAppearance::Light))
                .unwrap_or_else(|message| panic!("fixture failed: {source}: {message}"));
            assert!(rendered.logical_size.0 > 0.0);
            assert!(rendered.logical_size.1 > 0.0);
            assert!(looks_like_svg(&rendered.data));
            let text = str::from_utf8(&rendered.data).expect("rendered SVG must be UTF-8");
            let (width, height) = parse_svg(text).logical_size();
            let size = (f64::from(width), f64::from(height));
            assert_eq!(size, rendered.logical_size);
        }
    }

    #[test]
    fn invalid_diagram_uses_a_stable_error() {
        let result = render_uncached(&request(
            "flowchart TD\nA -->",
            BlockExtensionAppearance::Light,
        ));

        assert_eq!(result, Err(Arc::from("diagram syntax is invalid")));
    }

    #[test]
    fn undetected_diagram_uses_a_stable_error() {
        let result = render_uncached(&request("", BlockExtensionAppearance::Light));

        assert_eq!(result, Err(Arc::from("diagram type was not detected")));
    }

    #[test]
    fn source_larger_than_256_kib_is_rejected_before_rendering() {
        let oversized: Arc<str> = Arc::from("x".repeat(MAX_SOURCE_BYTES + 1));

        let result = render_uncached(&request(oversized, BlockExtensionAppearance::Light));

        assert_eq!(result, Err(Arc::from("diagram source is too large")));
    }

    #[test]
    fn synthetic_output_larger_than_8_mib_is_rejected() {
        let oversized = "x".repeat(MAX_SVG_BYTES + 1);

        assert_eq!(
            validate_svg(oversized),
            Err(MermaidRenderError::SvgTooLarge)
        );
    }

    #[test]
    fn invalid_logical_dimensions_are_rejected() {
        for size in [
            (0.0, 1.0),
            (1.0, 0.0),
            (f64::NAN, 1.0),
            (1.0, f64::NAN),
            (f64::INFINITY, 1.0),
            (1.0, f64::NEG_INFINITY),
            (MAX_LOGICAL_SIDE + 1.0, 1.0),
            (1.0, MAX_LOGICAL_SIDE + 1.0),
        ] {
            assert_eq!(
                validate_dimensions(size),
                Err(MermaidRenderError::InvalidDimensions),
                "accepted {size:?}"
            );
        }
    }

    #[test]
    fn active_svg_and_remote_resources_are_rejected() {
        let unsafe_svgs = [
            r#"<svg width="10" height="10"><foreignObject /></svg>"#,
            r#"<svg width="10" height="10"><script /></svg>"#,
            r#"<svg width="10" height="10" onload="alert(1)" />"#,
            r#"<svg width="10" height="10"><a href="https://example.invalid/">x</a></svg>"#,
            r#"<svg width="10" height="10"><style>.x{fill:url(https://example.invalid/p)}</style></svg>"#,
        ];

        for svg in unsafe_svgs {
            assert_eq!(
                validate_svg(svg.to_owned()),
                Err(MermaidRenderError::UnsafeSvg),
                "accepted {svg}"
            );
        }
    }

    #[test]
    fn same_document_fragment_references_remain_valid() {
        let svg = r##"<svg width="10" height="10" xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="paint" /></defs><a href="#node"><rect id="node" width="10" height="10" fill="url(#paint)" /></a></svg>"##;

        let rendered = validate_svg(svg.to_owned()).expect("fragment references must be valid");

        assert_eq!(rendered.logical_size, (10.0, 10.0));
    }

    #[test]
    fn source_css_cannot_install_an_external_resource() {
        let source = r#"%%{init: {"themeCSS": ".node { fill: url(https://example.invalid/tracker.svg); }"}}%%
flowchart TD
A-->B"#;

        let result = render_uncached(&request(source, BlockExtensionAppearance::Dark));

        if let Ok(rendered) = result {
            let installed = str::from_utf8(&rendered.data).unwrap().to_ascii_lowercase();
            assert!(!installed.contains("example.invalid"));
            assert!(!installed.contains("url(http"));
        }
    }

    #[test]
    fn installed_svg_has_no_active_or_external_content() {
        let rendered = render_uncached(&request(
            "flowchart TD\nA[Start] --> B[Done]",
            BlockExtensionAppearance::Dark,
        ))
        .expect("fixture must render");
        let text = str::from_utf8(&rendered.data).unwrap().to_ascii_lowercase();

        for forbidden in [
            "<script",
            "<foreignobject",
            " onload=",
            "href=\"http",
            "href='http",
            "url(http",
            "javascript:",
        ] {
            assert!(
                !text.contains(forbidden),
                "installed unsafe bytes: {forbidden}"
            );
        }
    }

    #[test]
    fn renderer_reuses_content_across_request_and_document_identity() {
        let renderer = MermaidRenderer::default();
        let first = request("flowchart TD\nA-->B", BlockExtensionAppearance::Light);
        let mut second = first.clone();
        second.request_id = BlockExtensionRequestId(77);
        second.revision = DocumentRevision::new(88);
        second.item.owner = SyntaxIdentity::from_raw_for_test(99);
        second.item.fragment_ordinal = 12;

        let first_result = renderer.render_and_cache(&first);
        let second_result = renderer.render_and_cache(&second);

        assert_eq!(first_result, second_result);
        assert_eq!(renderer.uncached_render_count(), 1);
    }

    #[test]
    fn renderer_does_not_mix_light_and_dark_results() {
        let renderer = MermaidRenderer::default();
        let light = request("flowchart TD\nA-->B", BlockExtensionAppearance::Light);
        let dark = request("flowchart TD\nA-->B", BlockExtensionAppearance::Dark);

        assert!(renderer.render_and_cache(&light).is_ok());
        assert!(renderer.render_and_cache(&dark).is_ok());

        assert_eq!(renderer.uncached_render_count(), 2);
    }

    #[test]
    fn renderer_reuses_a_deterministic_failure() {
        let renderer = MermaidRenderer::default();
        let invalid = request("plain text", BlockExtensionAppearance::Light);

        assert!(renderer.render_and_cache(&invalid).is_err());
        assert!(renderer.render_and_cache(&invalid).is_err());

        assert_eq!(renderer.uncached_render_count(), 1);
    }
}
