#[cfg(test)]
mod tests {
    use merman::svg::{
        HeadlessRenderer, HostTheme, HostThemePreset, Presentation, RenderResourceProfile,
    };

    #[test]
    fn pinned_merman_api_renders_resvg_compatible_svg() {
        let renderer = HeadlessRenderer::new()
            .with_presentation(
                Presentation::new()
                    .with_theme(HostTheme::from_preset(HostThemePreset::EditorLight)),
            )
            .with_resource_profile(RenderResourceProfile::Constrained)
            .with_vendored_text_measurer()
            .with_diagram_id("waml-mermaid-spike");
        let sealed = renderer
            .render_resvg_compatible_svg_sync("flowchart TD\nA-->B")
            .unwrap()
            .expect("diagram detected");
        assert!(sealed.as_str().trim_start().starts_with("<svg"));
    }
}
