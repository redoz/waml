use waml_markdown_editor::{
    layout::{EdgeInsets, FontKey, FontWeight, TextMetrics},
    presentation::{
        style::{FONT_MONO, FONT_SANS, WEIGHT_REGULAR, WEIGHT_SEMIBOLD},
        PresentationStyles, TextRole,
    },
};

fn metrics(
    font: FontKey,
    size: f32,
    spacing: f32,
    weight: FontWeight,
    italic: bool,
) -> TextMetrics {
    TextMetrics {
        font,
        font_size: size,
        line_spacing: spacing,
        weight,
        italic,
    }
}

#[test]
fn balanced_metrics_and_document_inset_are_fixed() {
    let styles = PresentationStyles::balanced();
    assert_eq!(
        styles.document_insets(),
        EdgeInsets {
            top: 24.0,
            right: 24.0,
            bottom: 24.0,
            left: 24.0,
        }
    );
    assert_eq!(
        styles.metrics(TextRole::Body),
        metrics(FONT_SANS, 12.5, 1.20, WEIGHT_REGULAR, false)
    );
    assert_eq!(
        styles.metrics(TextRole::Heading(1)),
        metrics(FONT_SANS, 19.0, 1.10, WEIGHT_SEMIBOLD, false)
    );
    assert_eq!(
        styles.metrics(TextRole::Heading(6)),
        metrics(FONT_SANS, 12.5, 1.10, WEIGHT_SEMIBOLD, false)
    );
    assert_eq!(
        styles.metrics(TextRole::CodeContent),
        metrics(FONT_MONO, 11.5, 1.15, WEIGHT_REGULAR, false)
    );
    assert_eq!(
        styles.metrics(TextRole::Emphasis),
        metrics(FONT_SANS, 12.5, 1.20, WEIGHT_REGULAR, true)
    );
    assert_eq!(
        styles.metrics(TextRole::StrongEmphasis),
        metrics(FONT_SANS, 12.5, 1.20, WEIGHT_SEMIBOLD, true)
    );
}

#[test]
fn a_heading_marker_matches_its_heading_metrics_and_differs_only_in_color() {
    let styles = PresentationStyles::balanced();
    for level in 1..=6 {
        assert_eq!(
            styles.metrics(TextRole::HeadingMarker(level)),
            styles.metrics(TextRole::Heading(level))
        );
        let marker = styles.text_style(TextRole::HeadingMarker(level));
        let heading = styles.text_style(TextRole::Heading(level));
        assert_eq!(
            (marker.font, marker.size, marker.weight),
            (heading.font, heading.size, heading.weight)
        );
        assert_ne!(marker.color, heading.color);
    }
}

#[test]
fn heading_margins_and_block_spacing_follow_the_balanced_table() {
    let spacing = PresentationStyles::balanced().spacing();
    assert_eq!(spacing.heading_margins(1), (12.0, 5.0));
    assert_eq!(spacing.heading_margins(2), (10.0, 4.0));
    assert_eq!(spacing.heading_margins(3), (9.0, 4.0));
    assert_eq!(spacing.heading_margins(4), (8.0, 3.0));
    assert_eq!(spacing.heading_margins(5), (7.0, 3.0));
    assert_eq!(spacing.heading_margins(6), (7.0, 3.0));
    assert_eq!(spacing.paragraph_after, 6.0);
    assert_eq!(spacing.quote_inset, 12.0);
    assert_eq!(spacing.quote_rule, 3.0);
    assert_eq!(spacing.quote_gap, 8.0);
    assert_eq!(spacing.list_marker_gap, 8.0);
    assert_eq!(spacing.code_padding, 12.0);
    assert_eq!((spacing.cell_padding_x, spacing.cell_padding_y), (8.0, 6.0));
}

#[test]
fn the_logical_inset_does_not_depend_on_device_pixel_ratio() {
    let styles = PresentationStyles::balanced();
    // Device pixel ratio never reaches the style sheet, so the same logical
    // content width holds for every ratio the shell can report.
    for _dpi in [1.0_f64, 1.25, 1.5, 2.0] {
        assert_eq!(styles.document_insets().left, 24.0);
        assert_eq!(styles.content_width(800.0), 752.0);
    }
    assert_eq!(styles.content_width(10.0), 0.0);
}

#[test]
fn a_role_without_its_own_entry_stays_visible_with_body_metrics() {
    let styles = PresentationStyles::balanced();
    // Recovery has no configured metric entry; it must not vanish or panic.
    assert_eq!(
        styles.metrics(TextRole::Recovery),
        styles.metrics(TextRole::Body)
    );
    assert_eq!(
        styles.metrics(TextRole::RawHtml),
        styles.metrics(TextRole::Body)
    );
}

#[test]
fn color_roles_stay_out_of_layout_metrics() {
    let styles = PresentationStyles::balanced();
    let marker = styles.text_style(TextRole::SyntaxMarker);
    // A marker differs from body only in color, so geometry cannot change with
    // its active state.
    assert_ne!(marker.color, marker.active_color);
    assert_eq!(
        styles.metrics(TextRole::SyntaxMarker),
        styles.metrics(TextRole::Body)
    );
}
