#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagCode {
    DuplicateSlug,
    FrontmatterNotClean,
    UnknownType,
    MalformedAttribute,
    MalformedRelationship,
    UnresolvedTarget,
}

impl DiagCode {
    pub fn as_str(self) -> &'static str {
        match self {
            DiagCode::DuplicateSlug => "duplicate-slug",
            DiagCode::FrontmatterNotClean => "frontmatter-not-clean",
            DiagCode::UnknownType => "unknown-type",
            DiagCode::MalformedAttribute => "malformed-attribute",
            DiagCode::MalformedRelationship => "malformed-relationship",
            DiagCode::UnresolvedTarget => "unresolved-target",
        }
    }
    /// Default severity for this code (a specific site may downgrade to a warning).
    pub fn severity(self) -> Severity {
        match self {
            DiagCode::UnknownType => Severity::Warning,
            _ => Severity::Error,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagCode,
    pub message: String,
    pub file: String,
    pub line: usize,
}

impl Diagnostic {
    pub fn new(code: DiagCode, message: impl Into<String>, file: impl Into<String>, line: usize) -> Diagnostic {
        Diagnostic { severity: code.severity(), code, message: message.into(), file: file.into(), line }
    }
    pub fn warn(code: DiagCode, message: impl Into<String>, file: impl Into<String>, line: usize) -> Diagnostic {
        Diagnostic { severity: Severity::Warning, code, message: message.into(), file: file.into(), line }
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
}
