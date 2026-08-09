use crate::analysis::DocumentId;
use waml_syntax::DocumentRevision;
use waml_syntax::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
pub enum DiagCode {
    DuplicateSlug,
    FrontmatterNotClean,
    UnknownType,
    ObsoleteDiagramType,
    MalformedAttribute,
    MalformedRelationship,
    MalformedFlowBullet,
    DuplicateFlowNode,
    UnresolvedTarget,
    MissingTraceDocument,
    UnresolvedTraceFragment,
    MalformedTraceTarget,
    UnsupportedTraceScheme,
    DroppableContent,
    MalformedLayout,
    UnresolvedLayoutRef,
    LayoutCycle,
    LayoutConflict,
    MalformedMessage,
    MalformedLifeline,
    SlotUnknownAttribute,
    InstanceOfNonClassifier,
    InstanceOfUnresolved,
    UnreachableFlowNode,
    DecisionWithoutGuard,
    EmptyFlowDocument,
    UnknownFlowTarget,
    UnknownLifelineHandle,
    UninvolvedLifeline,
    FragmentZeroOperands,
    EmptyOperandStream,
    FragmentNestingTooDeep,
    DuplicateSequenceName,
    ReservedSequenceName,
    UnknownSequenceEndpoint,
    InvalidSequenceEndpoint,
    InvalidLifelineLifetime,
    DuplicateCallIdentity,
    UnknownCallIdentity,
    UnmatchedReturn,
    AmbiguousReturn,
    CompletedReturn,
    ConflictingReturn,
    InvalidFragmentOperands,
    DuplicateGate,
    InvalidInteractionUse,
    InteractionUseCycle,
    UnsupportedSequenceForm,
    UnknownViewMiddleware,
    InvalidViewParams,
    ViewStageFailed,
    ViewDepthExceeded,
    ViewCycle,
    UnknownSurface,
    UnknownIcon,
    /// Two groups list the same element without one nesting the other, so
    /// their clusters cannot be pulled apart in the default layout.
    EntangledGroups,
}

impl DiagCode {
    pub fn as_str(self) -> &'static str {
        match self {
            DiagCode::DuplicateSlug => "duplicate-slug",
            DiagCode::FrontmatterNotClean => "frontmatter-not-clean",
            DiagCode::UnknownType => "unknown-type",
            DiagCode::ObsoleteDiagramType => "obsolete-diagram-type",
            DiagCode::MalformedAttribute => "malformed-attribute",
            DiagCode::MalformedRelationship => "malformed-relationship",
            DiagCode::MalformedFlowBullet => "malformed-flow-bullet",
            DiagCode::DuplicateFlowNode => "duplicate-flow-node",
            DiagCode::UnresolvedTarget => "unresolved-target",
            DiagCode::MissingTraceDocument => "missing-trace-document",
            DiagCode::UnresolvedTraceFragment => "unresolved-trace-fragment",
            DiagCode::MalformedTraceTarget => "malformed-trace-target",
            DiagCode::UnsupportedTraceScheme => "unsupported-trace-scheme",
            DiagCode::DroppableContent => "droppable-content",
            DiagCode::MalformedLayout => "malformed-layout",
            DiagCode::UnresolvedLayoutRef => "unresolved-layout-ref",
            DiagCode::LayoutCycle => "layout-cycle",
            DiagCode::LayoutConflict => "layout-conflict",
            DiagCode::MalformedMessage => "malformed-message",
            DiagCode::MalformedLifeline => "malformed-lifeline",
            DiagCode::SlotUnknownAttribute => "slot-unknown-attribute",
            DiagCode::InstanceOfNonClassifier => "instance-of-non-classifier",
            DiagCode::InstanceOfUnresolved => "instance-of-unresolved",
            DiagCode::UnreachableFlowNode => "unreachable-flow-node",
            DiagCode::DecisionWithoutGuard => "decision-without-guard",
            DiagCode::EmptyFlowDocument => "empty-flow-document",
            DiagCode::UnknownFlowTarget => "unknown-flow-target",
            DiagCode::UnknownLifelineHandle => "unknown-lifeline-handle",
            DiagCode::UninvolvedLifeline => "uninvolved-lifeline",
            DiagCode::FragmentZeroOperands => "fragment-zero-operands",
            DiagCode::EmptyOperandStream => "empty-operand-stream",
            DiagCode::FragmentNestingTooDeep => "fragment-nesting-too-deep",
            DiagCode::DuplicateSequenceName => "duplicate-sequence-name",
            DiagCode::ReservedSequenceName => "reserved-sequence-name",
            DiagCode::UnknownSequenceEndpoint => "unknown-sequence-endpoint",
            DiagCode::InvalidSequenceEndpoint => "invalid-sequence-endpoint",
            DiagCode::InvalidLifelineLifetime => "invalid-lifeline-lifetime",
            DiagCode::DuplicateCallIdentity => "duplicate-call-identity",
            DiagCode::UnknownCallIdentity => "unknown-call-identity",
            DiagCode::UnmatchedReturn => "unmatched-return",
            DiagCode::AmbiguousReturn => "ambiguous-return",
            DiagCode::CompletedReturn => "completed-return",
            DiagCode::ConflictingReturn => "conflicting-return",
            DiagCode::InvalidFragmentOperands => "invalid-fragment-operands",
            DiagCode::DuplicateGate => "duplicate-gate",
            DiagCode::InvalidInteractionUse => "invalid-interaction-use",
            DiagCode::InteractionUseCycle => "interaction-use-cycle",
            DiagCode::UnsupportedSequenceForm => "unsupported-sequence-form",
            DiagCode::UnknownViewMiddleware => "unknown-view-middleware",
            DiagCode::InvalidViewParams => "invalid-view-params",
            DiagCode::ViewStageFailed => "view-stage-failed",
            DiagCode::ViewDepthExceeded => "view-depth-exceeded",
            DiagCode::ViewCycle => "view-cycle",
            DiagCode::UnknownSurface => "unknown-surface",
            DiagCode::UnknownIcon => "unknown-icon",
            DiagCode::EntangledGroups => "entangled-groups",
        }
    }
    /// Default severity for this code (a specific site may downgrade to a warning).
    pub fn severity(self) -> Severity {
        match self {
            DiagCode::UnknownType
            | DiagCode::UnresolvedLayoutRef
            | DiagCode::SlotUnknownAttribute
            | DiagCode::InstanceOfNonClassifier
            | DiagCode::InstanceOfUnresolved
            | DiagCode::UnreachableFlowNode
            | DiagCode::UnknownLifelineHandle
            | DiagCode::UninvolvedLifeline
            | DiagCode::FragmentZeroOperands
            | DiagCode::EmptyOperandStream
            | DiagCode::FragmentNestingTooDeep
            | DiagCode::UnknownSurface
            | DiagCode::UnknownIcon
            | DiagCode::EntangledGroups => Severity::Warning,
            _ => Severity::Error,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagCode,
    pub message: String,
    pub file: String,
    pub line: usize,
    /// Byte range within `line`, if the diagnostic pins a precise column span.
    pub span: Option<(usize, usize)>,
    /// Stable source identity for revision-scoped parser diagnostics.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub document: Option<DocumentId>,
    /// Document revision against which `range` was produced.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub document_revision: Option<DocumentRevision>,
    /// Absolute UTF-8 byte range in the revision-scoped document.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub range: Option<TextRange>,
}

impl Diagnostic {
    pub fn new(
        code: DiagCode,
        message: impl Into<String>,
        file: impl Into<String>,
        line: usize,
    ) -> Diagnostic {
        Diagnostic {
            severity: code.severity(),
            code,
            message: message.into(),
            file: file.into(),
            line,
            span: None,
            document: None,
            document_revision: None,
            range: None,
        }
    }
    pub fn warn(
        code: DiagCode,
        message: impl Into<String>,
        file: impl Into<String>,
        line: usize,
    ) -> Diagnostic {
        Diagnostic {
            severity: Severity::Warning,
            code,
            message: message.into(),
            file: file.into(),
            line,
            span: None,
            document: None,
            document_revision: None,
            range: None,
        }
    }
    /// Attach a byte range (relative to the diagnostic's line) to this diagnostic.
    pub fn with_span(mut self, span: (usize, usize)) -> Diagnostic {
        self.span = Some(span);
        self
    }

    pub fn with_provenance(
        mut self,
        document: DocumentId,
        document_revision: DocumentRevision,
        range: TextRange,
    ) -> Diagnostic {
        self.document = Some(document);
        self.document_revision = Some(document_revision);
        self.range = Some(range);
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
        assert_eq!(
            DiagCode::ObsoleteDiagramType.as_str(),
            "obsolete-diagram-type"
        );
        assert_eq!(DiagCode::ObsoleteDiagramType.severity(), Severity::Error);
        assert_eq!(DiagCode::MalformedAttribute.severity(), Severity::Error);
        assert_eq!(
            DiagCode::SlotUnknownAttribute.as_str(),
            "slot-unknown-attribute"
        );
        assert_eq!(
            DiagCode::InstanceOfNonClassifier.as_str(),
            "instance-of-non-classifier"
        );
        assert_eq!(
            DiagCode::InstanceOfUnresolved.as_str(),
            "instance-of-unresolved"
        );
        assert_eq!(DiagCode::SlotUnknownAttribute.severity(), Severity::Warning);
        assert_eq!(
            DiagCode::InstanceOfNonClassifier.severity(),
            Severity::Warning
        );
        assert_eq!(DiagCode::InstanceOfUnresolved.severity(), Severity::Warning);

        assert_eq!(
            DiagCode::UnknownViewMiddleware.as_str(),
            "unknown-view-middleware"
        );
        assert_eq!(DiagCode::UnknownViewMiddleware.severity(), Severity::Error);
        assert_eq!(DiagCode::InvalidViewParams.as_str(), "invalid-view-params");
        assert_eq!(DiagCode::InvalidViewParams.severity(), Severity::Error);
        assert_eq!(DiagCode::ViewStageFailed.as_str(), "view-stage-failed");
        assert_eq!(DiagCode::ViewStageFailed.severity(), Severity::Error);
        assert_eq!(DiagCode::ViewDepthExceeded.as_str(), "view-depth-exceeded");
        assert_eq!(DiagCode::ViewDepthExceeded.severity(), Severity::Error);
        assert_eq!(DiagCode::ViewCycle.as_str(), "view-cycle");
        assert_eq!(DiagCode::ViewCycle.severity(), Severity::Error);
        assert_eq!(DiagCode::UnknownSurface.as_str(), "unknown-surface");
        assert_eq!(DiagCode::UnknownSurface.severity(), Severity::Warning);
        assert_eq!(DiagCode::UnknownIcon.as_str(), "unknown-icon");
        assert_eq!(DiagCode::UnknownIcon.severity(), Severity::Warning);
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
