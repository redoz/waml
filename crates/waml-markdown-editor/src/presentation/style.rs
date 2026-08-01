//! Style roles for presentation text.
//!
//! This module names the style each `TextRole` receives. Concrete metrics,
//! colors, and document insets are resolved by the style task; the compiler
//! only attaches roles.

use super::{ColorRole, FontRole, FontSizeRole, FontWeightRole, TextRole, TextStyle};

/// Resolves a presentation text role into its style roles.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PresentationStyles;

impl PresentationStyles {
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
