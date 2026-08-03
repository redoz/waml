//! Style roles for presentation text.
//!
//! This module names the style each `TextRole` receives. Concrete metrics,
//! colors, and document insets are resolved by the style task; the compiler
//! only attaches roles.

use waml_syntax::TextRange;

use crate::layout::{EdgeInsets, FontKey, FontWeight, TextMetrics};

use super::{
    highlight::CodeTokenRole, ColorRole, FontRole, FontSizeRole, FontWeightRole, TextRole,
    TextStyle,
};

pub const FONT_SANS: FontKey = FontKey(1);
pub const FONT_MONO: FontKey = FontKey(2);
pub const WEIGHT_REGULAR: FontWeight = FontWeight(400);
pub const WEIGHT_SEMIBOLD: FontWeight = FontWeight(600);

/// Logical-pixel spacing of the balanced document style.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PresentationSpacing {
    pub paragraph_after: f64,
    pub quote_inset: f64,
    pub quote_rule: f64,
    pub quote_gap: f64,
    pub list_marker_gap: f64,
    pub code_padding: f64,
    pub cell_padding_x: f64,
    pub cell_padding_y: f64,
}

impl PresentationSpacing {
    /// `(before, after)` margins of one heading level.
    pub fn heading_margins(&self, level: u8) -> (f64, f64) {
        match level {
            1 => (12.0, 5.0),
            2 => (10.0, 4.0),
            3 => (9.0, 4.0),
            4 => (8.0, 3.0),
            _ => (7.0, 3.0),
        }
    }
}

/// Resolves a presentation text role into its style roles.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PresentationStyles;

impl PresentationStyles {
    /// The balanced document style. Metrics are fixed logical values and do not
    /// depend on device pixel ratio.
    pub fn balanced() -> Self {
        Self
    }

    /// Always 24 logical pixels on all four sides.
    pub fn document_insets(&self) -> EdgeInsets {
        EdgeInsets {
            top: 24.0,
            right: 24.0,
            bottom: 24.0,
            left: 24.0,
        }
    }

    pub fn spacing(&self) -> PresentationSpacing {
        PresentationSpacing {
            paragraph_after: 6.0,
            quote_inset: 12.0,
            quote_rule: 3.0,
            quote_gap: 8.0,
            list_marker_gap: 8.0,
            code_padding: 12.0,
            cell_padding_x: 8.0,
            cell_padding_y: 6.0,
        }
    }

    /// Content width inside the fixed document inset.
    pub fn content_width(&self, viewport_width: f64) -> f64 {
        (viewport_width - self.document_insets().left - self.document_insets().right).max(0.0)
    }

    /// Hanging indent of a list item: the literal marker width plus the gap.
    pub fn marker_indent(&self, marker_range: TextRange) -> f64 {
        let characters = marker_range
            .end()
            .to_usize()
            .saturating_sub(marker_range.start().to_usize()) as f64;
        characters * self.metrics(TextRole::ListMarker).font_size as f64 * 0.5
            + self.spacing().list_marker_gap
    }

    /// Layout metrics of one text role. Color roles never enter metrics, so a
    /// color-only change cannot alter cached geometry.
    pub fn metrics(&self, role: TextRole) -> TextMetrics {
        let sans =
            |font_size: f32, line_spacing: f32, weight: FontWeight, italic: bool| TextMetrics {
                font: FONT_SANS,
                font_size,
                line_spacing,
                weight,
                italic,
            };
        match role {
            TextRole::Heading(level) | TextRole::HeadingMarker(level) => {
                let size = match level {
                    1 => 24.0,
                    2 => 20.0,
                    3 => 17.5,
                    4 => 15.5,
                    5 => 14.5,
                    _ => 13.5,
                };
                sans(size, 1.05, WEIGHT_SEMIBOLD, false)
            }
            TextRole::Emphasis => sans(13.5, 1.12, WEIGHT_REGULAR, true),
            TextRole::Strong => sans(13.5, 1.12, WEIGHT_SEMIBOLD, false),
            TextRole::StrongEmphasis => sans(13.5, 1.12, WEIGHT_SEMIBOLD, true),
            TextRole::InlineCode
            | TextRole::CodeContent
            | TextRole::CodeToken(_)
            | TextRole::CodeFence
            | TextRole::CodeInfo
            | TextRole::Frontmatter => TextMetrics {
                font: FONT_MONO,
                font_size: 12.5,
                line_spacing: 1.08,
                weight: WEIGHT_REGULAR,
                italic: false,
            },
            // Every other role, including roles added later, keeps body metrics
            // so its source range stays visible.
            _ => sans(13.5, 1.12, WEIGHT_REGULAR, false),
        }
    }

    /// The style of one highlighted code token. Font family, size, and line
    /// spacing stay exactly those of code content, so highlighting can never
    /// move a glyph.
    pub fn code_token(&self, role: CodeTokenRole) -> TextStyle {
        let base = self.text_style(TextRole::CodeContent);
        let color = match role {
            CodeTokenRole::Keyword | CodeTokenRole::Type => ColorRole::Link,
            CodeTokenRole::Property => ColorRole::Text,
            CodeTokenRole::String | CodeTokenRole::Number => ColorRole::Code,
            CodeTokenRole::Comment | CodeTokenRole::Punctuation => ColorRole::Muted,
            CodeTokenRole::Invalid => ColorRole::Recovery,
        };
        TextStyle {
            color,
            active_color: color,
            italic: matches!(role, CodeTokenRole::Comment),
            weight: match role {
                CodeTokenRole::Keyword => FontWeightRole::SemiBold,
                _ => base.weight,
            },
            ..base
        }
    }

    pub fn text_style(&self, role: TextRole) -> TextStyle {
        let base = TextStyle {
            font: FontRole::Body,
            size: FontSizeRole::Body,
            weight: FontWeightRole::Regular,
            italic: false,
            color: ColorRole::Text,
            active_color: ColorRole::Text,
            background: None,
            underline: false,
            strikethrough: false,
        };
        match role {
            TextRole::Body | TextRole::Whitespace | TextRole::LineBreak => base,
            // Markers stay dim rather than hidden, and only change color when
            // their construct is active.
            TextRole::SyntaxMarker => TextStyle {
                color: ColorRole::Marker,
                active_color: ColorRole::ActiveMarker,
                ..base
            },
            TextRole::Heading(level) => TextStyle {
                font: FontRole::Heading,
                size: FontSizeRole::Heading(level),
                weight: FontWeightRole::SemiBold,
                ..base
            },
            // Heading markers keep the heading's font and size and differ from
            // the heading text only in color.
            TextRole::HeadingMarker(level) => TextStyle {
                font: FontRole::Heading,
                size: FontSizeRole::Heading(level),
                weight: FontWeightRole::SemiBold,
                color: ColorRole::Marker,
                active_color: ColorRole::ActiveMarker,
                ..base
            },
            TextRole::Emphasis => TextStyle {
                italic: true,
                ..base
            },
            TextRole::Strong => TextStyle {
                weight: FontWeightRole::Bold,
                ..base
            },
            TextRole::StrongEmphasis => TextStyle {
                weight: FontWeightRole::Bold,
                italic: true,
                ..base
            },
            TextRole::Strikethrough => TextStyle {
                strikethrough: true,
                ..base
            },
            TextRole::LinkLabel => TextStyle {
                color: ColorRole::Link,
                active_color: ColorRole::Link,
                underline: true,
                ..base
            },
            TextRole::LinkDestination => TextStyle {
                color: ColorRole::Marker,
                active_color: ColorRole::ActiveMarker,
                ..base
            },
            TextRole::ListMarker | TextRole::TaskMarker | TextRole::QuoteMarker => TextStyle {
                color: ColorRole::Marker,
                active_color: ColorRole::ActiveMarker,
                weight: FontWeightRole::Medium,
                ..base
            },
            TextRole::InlineCode => TextStyle {
                font: FontRole::Monospace,
                size: FontSizeRole::Code,
                color: ColorRole::Code,
                active_color: ColorRole::Code,
                background: Some(ColorRole::CodeSurface),
                ..base
            },
            TextRole::CodeFence | TextRole::CodeInfo => TextStyle {
                font: FontRole::Monospace,
                size: FontSizeRole::Code,
                color: ColorRole::Marker,
                active_color: ColorRole::ActiveMarker,
                ..base
            },
            TextRole::CodeToken(token) => self.code_token(token),
            TextRole::CodeContent => TextStyle {
                font: FontRole::Monospace,
                size: FontSizeRole::Code,
                color: ColorRole::Code,
                active_color: ColorRole::Code,
                ..base
            },
            TextRole::TableDelimiter => TextStyle {
                color: ColorRole::TableRule,
                active_color: ColorRole::ActiveMarker,
                ..base
            },
            TextRole::RawHtml => TextStyle {
                font: FontRole::Monospace,
                size: FontSizeRole::Code,
                color: ColorRole::Muted,
                active_color: ColorRole::Muted,
                ..base
            },
            TextRole::Frontmatter => TextStyle {
                font: FontRole::Monospace,
                size: FontSizeRole::Code,
                color: ColorRole::Muted,
                active_color: ColorRole::Muted,
                ..base
            },
            TextRole::Recovery => TextStyle {
                color: ColorRole::Recovery,
                active_color: ColorRole::Recovery,
                ..base
            },
        }
    }
}
