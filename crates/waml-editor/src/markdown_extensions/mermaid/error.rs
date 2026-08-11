use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MermaidRenderError {
    SourceTooLarge,
    DiagramNotDetected,
    InvalidDiagram,
    SvgTooLarge,
    UnsafeSvg,
    InvalidSvg,
    InvalidDimensions,
}

impl MermaidRenderError {
    pub(super) fn viewer_message(self) -> Arc<str> {
        Arc::from(match self {
            Self::SourceTooLarge => "diagram source is too large",
            Self::DiagramNotDetected => "diagram type was not detected",
            Self::InvalidDiagram => "diagram syntax is invalid",
            Self::SvgTooLarge => "rendered diagram is too large",
            Self::UnsafeSvg => "rendered diagram is unsafe",
            Self::InvalidSvg => "rendered diagram is invalid",
            Self::InvalidDimensions => "rendered diagram dimensions are invalid",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::MermaidRenderError;

    #[test]
    fn viewer_messages_are_stable_and_short() {
        let cases = [
            (
                MermaidRenderError::SourceTooLarge,
                "diagram source is too large",
            ),
            (
                MermaidRenderError::DiagramNotDetected,
                "diagram type was not detected",
            ),
            (
                MermaidRenderError::InvalidDiagram,
                "diagram syntax is invalid",
            ),
            (
                MermaidRenderError::SvgTooLarge,
                "rendered diagram is too large",
            ),
            (MermaidRenderError::UnsafeSvg, "rendered diagram is unsafe"),
            (
                MermaidRenderError::InvalidSvg,
                "rendered diagram is invalid",
            ),
            (
                MermaidRenderError::InvalidDimensions,
                "rendered diagram dimensions are invalid",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.viewer_message().as_ref(), expected);
        }
    }
}
