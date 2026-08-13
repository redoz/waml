use std::sync::{Arc, Mutex};

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use makepad_widgets::{
    looks_like_svg, makepad_draw::svg::parse_svg, makepad_html::parse_html, InternLiveId, LiveId,
};
use merman::svg::{
    HeadlessError, HeadlessRenderer, HostTheme, HostThemePreset, Presentation,
    RenderResourceProfile, RootBackgroundPostprocessor, SvgPipeline,
};
use waml_markdown_editor::reading::{
    BlockExtensionAppearance, BlockExtensionRequest, RenderedBlockSvg,
};

use super::{
    cache::{lock_cache, CacheKey, MermaidCache},
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
        lock_cache(&self.cache).get(&key)
    }

    pub(in crate::markdown_extensions) fn render_and_cache(
        &self,
        request: &BlockExtensionRequest,
    ) -> BlockRenderResult {
        let key = CacheKey::from_request(request);
        if let Some(hit) = lock_cache(&self.cache).get(&key) {
            return hit;
        }

        #[cfg(test)]
        self.uncached_renders.fetch_add(1, Ordering::Relaxed);
        let rendered = render_uncached(request);

        let mut cache = lock_cache(&self.cache);
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
    let diagram_id = CacheKey::from_request(request).diagram_id();
    let renderer = HeadlessRenderer::new()
        .with_presentation(Presentation::new().with_theme(HostTheme::from_preset(preset)))
        .with_resource_profile(RenderResourceProfile::Constrained)
        .with_vendored_text_measurer()
        .with_diagram_id(&diagram_id);

    let mut pipeline = SvgPipeline::resvg_safe();
    pipeline.push_postprocessor(RootBackgroundPostprocessor::new("transparent"));
    let sealed = match renderer
        .render_resvg_compatible_svg_with_pipeline_sync(&request.content, &pipeline)
    {
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

    let normalized = normalize_sealed_svg_with(sealed.into_string(), normalize_for_makepad)
        .map_err(MermaidRenderError::viewer_message)?;
    validate_svg(normalized).map_err(MermaidRenderError::viewer_message)
}

fn normalize_sealed_svg_with(
    svg: String,
    normalizer: impl FnOnce(String) -> Result<String, MermaidRenderError>,
) -> Result<String, MermaidRenderError> {
    if svg.len() > MAX_SVG_BYTES {
        return Err(MermaidRenderError::SvgTooLarge);
    }
    validate_svg_safety(&svg)?;
    normalizer(svg)
}

fn usvg_options() -> usvg::Options<'static> {
    usvg::Options {
        font_family: "IBM Plex Sans".to_string(),
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        font_resolver: usvg::FontResolver {
            select_font: usvg::FontResolver::default_font_selector(),
            select_fallback: usvg::FontResolver::default_fallback_selector(),
        },
        ..usvg::Options::default()
    }
}

fn normalize_for_makepad(svg: String) -> Result<String, MermaidRenderError> {
    let mut options = usvg_options();
    let fontdb = options.fontdb_mut();
    fontdb.load_font_data(
        include_bytes!("../../../resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf").to_vec(),
    );
    fontdb.load_font_data(
        include_bytes!("../../../resources/fonts/IBM_Plex_Sans/IBMPlexSans-SemiBold.ttf").to_vec(),
    );
    fontdb.set_sans_serif_family("IBM Plex Sans");

    let tree = usvg::Tree::from_str(&svg, &options).map_err(|error| {
        tracing::warn!(?error, "could not normalize Mermaid SVG for Makepad");
        MermaidRenderError::InvalidSvg
    })?;
    let normalized = tree.to_string(&usvg::WriteOptions {
        preserve_text: false,
        ..usvg::WriteOptions::default()
    });
    if normalized.contains("<style")
        || normalized.contains("<text")
        || normalized.contains("foreignObject")
    {
        tracing::warn!("normalized Mermaid SVG retained unsupported Makepad content");
        return Err(MermaidRenderError::InvalidSvg);
    }
    Ok(normalized)
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
    validate_decoded_svg_values(svg)?;
    Ok(())
}

fn validate_decoded_svg_values(svg: &str) -> Result<(), MermaidRenderError> {
    let mut errors = None;
    let document = parse_html(svg, &mut errors, InternLiveId::No);
    if errors.as_ref().is_some_and(|errors| !errors.is_empty()) {
        return Err(MermaidRenderError::InvalidSvg);
    }

    let style_tag = LiveId::from_str_lc("style");
    let xml_base = LiveId::from_str_lc("xml:base");
    let uri_attributes = [
        LiveId::from_str_lc("href"),
        LiveId::from_str_lc("xlink:href"),
        LiveId::from_str_lc("src"),
    ];
    let mut walker = document.new_walker();
    while !walker.done() {
        if let Some(tag) = walker.open_tag_lc() {
            let mut attributes = document.new_walker_with_index(walker.index() + 1);
            while let Some((name, value)) = attributes.while_attr_lc() {
                if name == xml_base {
                    return Err(MermaidRenderError::UnsafeSvg);
                }
                validate_css_urls(value)?;
                if uri_attributes.contains(&name) {
                    validate_fragment_reference(value)?;
                }
            }
            if tag == style_tag {
                if let Some(style) = walker.find_text() {
                    validate_css_urls(style)?;
                }
            }
        }
        walker.walk();
    }
    Ok(())
}

fn validate_css_urls(css: &str) -> Result<(), MermaidRenderError> {
    let normalized = normalize_css_escapes(css)?;
    if normalized.contains("@import") {
        return Err(MermaidRenderError::UnsafeSvg);
    }

    let bytes = normalized.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if !is_css_identifier_byte(bytes[cursor]) {
            cursor += 1;
            continue;
        }
        let start = cursor;
        while cursor < bytes.len() && is_css_identifier_byte(bytes[cursor]) {
            cursor += 1;
        }
        if &normalized[start..cursor] != "url" {
            continue;
        }
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'(') {
            continue;
        }
        cursor += 1;
        let value_start = cursor;
        while cursor < bytes.len() && bytes[cursor] != b')' {
            cursor += 1;
        }
        if cursor == bytes.len() {
            return Err(MermaidRenderError::UnsafeSvg);
        }
        validate_fragment_reference(&normalized[value_start..cursor])?;
        cursor += 1;
    }
    Ok(())
}

fn normalize_css_escapes(css: &str) -> Result<String, MermaidRenderError> {
    let mut normalized = String::with_capacity(css.len());
    let mut characters = css.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '\\' {
            normalized.push(character.to_ascii_lowercase());
            continue;
        }

        let Some(next) = characters.peek().copied() else {
            return Err(MermaidRenderError::UnsafeSvg);
        };
        if matches!(next, '\n' | '\r' | '\u{000c}') {
            characters.next();
            if next == '\r' && characters.peek() == Some(&'\n') {
                characters.next();
            }
            continue;
        }
        if next.is_ascii_hexdigit() {
            let mut value = 0_u32;
            let mut digits = 0;
            while digits < 6 {
                let Some(hex) = characters.peek().copied() else {
                    break;
                };
                let Some(digit) = hex.to_digit(16) else {
                    break;
                };
                characters.next();
                value = value * 16 + digit;
                digits += 1;
            }
            if characters
                .peek()
                .is_some_and(|character| character.is_ascii_whitespace())
            {
                let whitespace = characters.next();
                if whitespace == Some('\r') && characters.peek() == Some(&'\n') {
                    characters.next();
                }
            }
            let Some(decoded) = char::from_u32(value).filter(|decoded| *decoded != '\0') else {
                return Err(MermaidRenderError::UnsafeSvg);
            };
            normalized.push(decoded.to_ascii_lowercase());
            continue;
        }

        characters.next();
        normalized.push(next.to_ascii_lowercase());
    }
    Ok(normalized)
}

fn is_css_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
}

fn validate_fragment_reference(value: &str) -> Result<(), MermaidRenderError> {
    let value = value.trim();
    let value = if value.len() >= 2
        && matches!(value.as_bytes()[0], b'\'' | b'"')
        && value.as_bytes()[0] == value.as_bytes()[value.len() - 1]
    {
        value[1..value.len() - 1].trim()
    } else {
        value
    };
    let Some(fragment) = value.strip_prefix('#') else {
        return Err(MermaidRenderError::UnsafeSvg);
    };
    if fragment.is_empty()
        || fragment
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(MermaidRenderError::UnsafeSvg);
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
        normalize_sealed_svg_with, render_uncached, usvg_options, validate_dimensions,
        validate_svg, MermaidRenderer, MAX_LOGICAL_SIDE, MAX_SOURCE_BYTES, MAX_SVG_BYTES,
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
    fn required_families_emit_makepad_self_painted_paths() {
        let fixtures = [
            (
                "flowchart",
                "flowchart TD\nDraft --> Review\nReview -->|change| Draft\nReview -->|approve| Done",
            ),
            ("sequence", "sequenceDiagram\nAlice->>Bob: Hello"),
            ("class", "classDiagram\nclass Animal"),
            ("state", "stateDiagram-v2\n[*] --> Still\nStill --> [*]"),
        ];

        for (name, source) in fixtures {
            let rendered = render_uncached(&request(source, BlockExtensionAppearance::Light))
                .unwrap_or_else(|message| panic!("fixture failed: {name}: {message}"));
            let text = str::from_utf8(&rendered.data).expect("rendered SVG must be UTF-8");
            assert!(!text.contains("<style"), "{name} retained class CSS");
            assert!(!text.contains("<text"), "{name} retained unsupported text");
            assert!(
                !text.contains("foreignObject"),
                "{name} retained unsupported HTML labels"
            );
            assert!(text.contains("<path"), "{name} has no path geometry");
            assert!(
                text.match_indices("<path").any(|(start, _)| {
                    text[start..]
                        .find('>')
                        .is_some_and(|end| text[start..start + end].contains("fill=\"#0f172a\""))
                }),
                "{name} has no dark label path geometry"
            );
            assert!(
                text.contains("#64748b") || text.contains("#94a3b8"),
                "{name} has no explicit visible edge paint"
            );
        }
    }

    #[test]
    fn normalized_svg_has_no_opaque_canvas_background() {
        fn has_canvas_cover(group: &usvg::Group, width: f32, height: f32) -> bool {
            group.children().iter().any(|node| match node {
                usvg::Node::Group(group) => has_canvas_cover(group, width, height),
                usvg::Node::Path(path) => {
                    let rect = path.abs_bounding_box();
                    path.fill().is_some_and(|fill| fill.opacity().get() > 0.0)
                        && rect.x().abs() < 0.01
                        && rect.y().abs() < 0.01
                        && (rect.width() - width).abs() < 0.01
                        && (rect.height() - height).abs() < 0.01
                }
                _ => false,
            })
        }

        let rendered = render_uncached(&request(
            "flowchart TD\nDraft --> Review\nReview --> Done",
            BlockExtensionAppearance::Light,
        ))
        .expect("flowchart renders");
        let svg = str::from_utf8(&rendered.data).expect("rendered SVG is UTF-8");
        let tree = usvg::Tree::from_str(svg, &usvg_options()).expect("normalized SVG parses");

        assert!(
            !has_canvas_cover(tree.root(), tree.size().width(), tree.size().height()),
            "the diagram canvas must be transparent so the reading surface shows through"
        );
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
    fn raw_svg_is_rejected_before_the_normalizer_runs() {
        let cases = [
            (
                "oversized",
                "x".repeat(MAX_SVG_BYTES + 1),
                MermaidRenderError::SvgTooLarge,
            ),
            (
                "external href",
                r#"<svg width="10" height="10"><image href="https://example.invalid/x.png" /></svg>"#
                    .to_owned(),
                MermaidRenderError::UnsafeSvg,
            ),
            (
                "CSS URL",
                r#"<svg width="10" height="10"><style>.x{fill:url(file:///secret)}</style></svg>"#
                    .to_owned(),
                MermaidRenderError::UnsafeSvg,
            ),
            (
                "xml:base",
                r##"<svg width="10" height="10" xml:base="file:///secret"><use href="#x" /></svg>"##
                    .to_owned(),
                MermaidRenderError::UnsafeSvg,
            ),
        ];

        for (name, svg, expected) in cases {
            let mut normalizer_calls = 0;
            let result = normalize_sealed_svg_with(svg, |_| {
                normalizer_calls += 1;
                Ok(String::new())
            });
            assert_eq!(result, Err(expected), "wrong result for {name}");
            assert_eq!(normalizer_calls, 0, "normalizer ran for {name}");
        }
    }

    #[test]
    fn usvg_image_resolvers_reject_data_file_and_network_sources() {
        let options = usvg_options();
        assert!((options.image_href_resolver.resolve_data)(
            "image/png",
            Arc::new(vec![137, 80, 78, 71]),
            &options,
        )
        .is_none());
        for href in [
            "data:image/png;base64,iVBORw0KGgo=",
            "file:///C:/secret.png",
            "C:/secret.png",
            "https://example.invalid/secret.png",
        ] {
            assert!(
                (options.image_href_resolver.resolve_string)(href, &options).is_none(),
                "resolver accepted {href}"
            );
        }
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
    fn entity_encoded_css_url_is_rejected_after_xml_decoding() {
        let svg =
            r#"<svg width="10" height="10"><style>.x{fill:u&#x72;l(https://evil/x)}</style></svg>"#;

        assert_eq!(
            validate_svg(svg.to_owned()),
            Err(MermaidRenderError::UnsafeSvg)
        );
    }

    #[test]
    fn entity_encoded_attribute_url_is_rejected_after_xml_decoding() {
        let svg = r#"<svg width="10" height="10"><rect fill="u&#x72;l(https://evil/x)" /></svg>"#;

        assert_eq!(
            validate_svg(svg.to_owned()),
            Err(MermaidRenderError::UnsafeSvg)
        );
    }

    #[test]
    fn css_escaped_url_function_is_rejected_after_normalization() {
        let svg =
            r#"<svg width="10" height="10"><style>.x{fill:u\72l(https://evil/x)}</style></svg>"#;

        assert_eq!(
            validate_svg(svg.to_owned()),
            Err(MermaidRenderError::UnsafeSvg)
        );
    }

    #[test]
    fn xml_base_cannot_redirect_a_local_fragment() {
        let svg = r##"<svg width="10" height="10" xml:base="https://evil/"><a href="#node"><rect id="node" width="10" height="10" /></a></svg>"##;

        assert_eq!(
            validate_svg(svg.to_owned()),
            Err(MermaidRenderError::UnsafeSvg)
        );
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
    fn identical_keys_emit_identical_uncached_svg_across_items() {
        let first = request("flowchart TD\nA-->B", BlockExtensionAppearance::Light);
        let mut second = first.clone();
        second.item.owner = SyntaxIdentity::from_raw_for_test(500);
        second.item.fragment_ordinal = 91;

        let first_svg = render_uncached(&first).expect("first fixture must render");
        let second_svg = render_uncached(&second).expect("second fixture must render");

        assert_eq!(first_svg, second_svg);
    }

    #[test]
    fn concurrent_identical_keys_emit_the_same_uncached_svg() {
        let first = request("flowchart TD\nA-->B", BlockExtensionAppearance::Dark);
        let mut second = first.clone();
        second.item.owner = SyntaxIdentity::from_raw_for_test(700);
        second.item.fragment_ordinal = 19;

        let first_worker = std::thread::spawn(move || render_uncached(&first));
        let second_worker = std::thread::spawn(move || render_uncached(&second));

        assert_eq!(first_worker.join().unwrap(), second_worker.join().unwrap());
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
