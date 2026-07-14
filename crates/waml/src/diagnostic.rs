#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum DiagCode {
    DuplicateSlug,
    FrontmatterNotClean,
    UnknownType,
    MalformedAttribute,
    MalformedRelationship,
    MalformedFlowBullet,
    UnresolvedTarget,
    DroppableContent,
    MalformedLayout,
    UnresolvedLayoutRef,
    LayoutCycle,
    LayoutConflict,
}

impl DiagCode {
    pub fn as_str(self) -> &'static str {
        match self {
            DiagCode::DuplicateSlug => "duplicate-slug",
            DiagCode::FrontmatterNotClean => "frontmatter-not-clean",
            DiagCode::UnknownType => "unknown-type",
            DiagCode::MalformedAttribute => "malformed-attribute",
            DiagCode::MalformedRelationship => "malformed-relationship",
            DiagCode::MalformedFlowBullet => "malformed-flow-bullet",
            DiagCode::UnresolvedTarget => "unresolved-target",
            DiagCode::DroppableContent => "droppable-content",
            DiagCode::MalformedLayout => "malformed-layout",
            DiagCode::UnresolvedLayoutRef => "unresolved-layout-ref",
            DiagCode::LayoutCycle => "layout-cycle",
            DiagCode::LayoutConflict => "layout-conflict",
        }
    }
    /// Default severity for this code (a specific site may downgrade to a warning).
    pub fn severity(self) -> Severity {
        match self {
            DiagCode::UnknownType | DiagCode::UnresolvedLayoutRef => Severity::Warning,
            _ => Severity::Error,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagCode,
    pub message: String,
    pub file: String,
    pub line: usize,
    /// Byte range within `line`, if the diagnostic pins a precise column span.
    pub span: Option<(usize, usize)>,
}

impl Diagnostic {
    pub fn new(code: DiagCode, message: impl Into<String>, file: impl Into<String>, line: usize) -> Diagnostic {
        Diagnostic { severity: code.severity(), code, message: message.into(), file: file.into(), line, span: None }
    }
    pub fn warn(code: DiagCode, message: impl Into<String>, file: impl Into<String>, line: usize) -> Diagnostic {
        Diagnostic { severity: Severity::Warning, code, message: message.into(), file: file.into(), line, span: None }
    }
    /// Attach a byte range (relative to the diagnostic's line) to this diagnostic.
    pub fn with_span(mut self, span: (usize, usize)) -> Diagnostic {
        self.span = Some(span);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_has_stable_slug_and_severity() {
        assert_eq!(DiagCode::UnresolvedTarget.as_str(), "unresolved-target");
        assert_eq!(DiagCode::UnknownType.severity(), Severity::Warning);
        assert_eq!(DiagCode::MalformedAttribute.severity(), Severity::Error);
    }

    #[test]
    fn constructors_set_severity() {
        let e = Diagnostic::new(DiagCode::DuplicateSlug, "dup", "a.md", 1);
        assert_eq!(e.severity, Severity::Error);
        let w = Diagnostic::warn(DiagCode::UnresolvedTarget, "member", "a.md", 3);
        assert_eq!(w.severity, Severity::Warning);
    }

    #[test]
    fn span_defaults_to_none_and_with_span_sets_it() {
        let d = Diagnostic::new(DiagCode::MalformedAttribute, "bad", "a.md", 5);
        assert_eq!(d.span, None);
        let d = d.with_span((2, 20));
        assert_eq!(d.span, Some((2, 20)));
    }
}
