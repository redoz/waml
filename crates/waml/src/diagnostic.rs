use crate::source::DocumentId;
use waml_syntax::DocumentRevision;
use waml_syntax::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Severity {
    Error,
    Warning,
}

/// Declares every diagnostic code once, next to the kebab-case wire name it is
/// published under.
///
/// The wire name is a literal in a single table, and everything that needs it --
/// the `serde` rename, [`DiagCode::as_str`] and [`DiagCode::ALL`] -- expands from
/// that one literal. A variant without a name is a syntax error in this macro,
/// so the wire name and the programmatic name can no longer drift apart.
macro_rules! diag_codes {
    ($(
        $(#[doc = $doc:literal])*
        $variant:ident => $wire:literal,
    )+) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub enum DiagCode {
            $(
                $(#[doc = $doc])*
                #[cfg_attr(feature = "serde", serde(rename = $wire))]
                $variant,
            )+
        }

        impl DiagCode {
            /// Every code, in declaration order. Expanded from the same table as
            /// the enum, so it cannot fall behind it.
            pub const ALL: &[DiagCode] = &[$(DiagCode::$variant,)+];

            /// The stable kebab-case wire name. Identical to the `serde` name by
            /// construction: both expand from the same literal.
            pub fn as_str(self) -> &'static str {
                match self {
                    $(DiagCode::$variant => $wire,)+
                }
            }
        }
    };
}

diag_codes! {
    DuplicateSlug => "duplicate-slug",
    FrontmatterNotClean => "frontmatter-not-clean",
    UnknownType => "unknown-type",
    ObsoleteDiagramType => "obsolete-diagram-type",
    MalformedAttribute => "malformed-attribute",
    MalformedRelationship => "malformed-relationship",
    MalformedFlowBullet => "malformed-flow-bullet",
    DuplicateFlowNode => "duplicate-flow-node",
    UnresolvedTarget => "unresolved-target",
    MissingTraceDocument => "missing-trace-document",
    UnresolvedTraceFragment => "unresolved-trace-fragment",
    MalformedTraceTarget => "malformed-trace-target",
    UnsupportedTraceScheme => "unsupported-trace-scheme",
    DroppableContent => "droppable-content",
    MalformedLayout => "malformed-layout",
    UnresolvedLayoutRef => "unresolved-layout-ref",
    LayoutCycle => "layout-cycle",
    LayoutConflict => "layout-conflict",
    InvalidUseCaseGroup => "invalid-use-case-group",
    ActorInsideSystemBoundary => "actor-inside-system-boundary",
    UseCaseOutsideSystemBoundary => "use-case-outside-system-boundary",
    UseCaseInMultipleSystemBoundaries => "use-case-in-multiple-system-boundaries",
    EmptyUseCaseBand => "empty-use-case-band",
    MalformedMessage => "malformed-message",
    MalformedLifeline => "malformed-lifeline",
    SlotUnknownAttribute => "slot-unknown-attribute",
    InstanceOfNonClassifier => "instance-of-non-classifier",
    InstanceOfUnresolved => "instance-of-unresolved",
    UnreachableFlowNode => "unreachable-flow-node",
    DecisionWithoutGuard => "decision-without-guard",
    EmptyFlowDocument => "empty-flow-document",
    UnknownFlowTarget => "unknown-flow-target",
    UnknownLifelineHandle => "unknown-lifeline-handle",
    UninvolvedLifeline => "uninvolved-lifeline",
    FragmentZeroOperands => "fragment-zero-operands",
    EmptyOperandStream => "empty-operand-stream",
    FragmentNestingTooDeep => "fragment-nesting-too-deep",
    DuplicateSequenceName => "duplicate-sequence-name",
    ReservedSequenceName => "reserved-sequence-name",
    UnknownSequenceEndpoint => "unknown-sequence-endpoint",
    InvalidSequenceEndpoint => "invalid-sequence-endpoint",
    InvalidLifelineLifetime => "invalid-lifeline-lifetime",
    DuplicateCallIdentity => "duplicate-call-identity",
    UnknownCallIdentity => "unknown-call-identity",
    UnmatchedReturn => "unmatched-return",
    AmbiguousReturn => "ambiguous-return",
    CompletedReturn => "completed-return",
    ConflictingReturn => "conflicting-return",
    InvalidFragmentOperands => "invalid-fragment-operands",
    DuplicateGate => "duplicate-gate",
    InvalidInteractionUse => "invalid-interaction-use",
    InteractionUseCycle => "interaction-use-cycle",
    UnsupportedSequenceForm => "unsupported-sequence-form",
    UnknownViewMiddleware => "unknown-view-middleware",
    InvalidViewParams => "invalid-view-params",
    ViewStageFailed => "view-stage-failed",
    ViewDepthExceeded => "view-depth-exceeded",
    ViewCycle => "view-cycle",
    UnknownSurface => "unknown-surface",
    UnknownIcon => "unknown-icon",
    /// Two groups list the same element without one nesting the other, so
    /// their clusters cannot be pulled apart in the default layout.
    EntangledGroups => "entangled-groups",
    /// Analysis of the whole bundle failed, so nothing could be checked. A
    /// failure to analyse is never the same answer as "analysed, found
    /// nothing" — this code keeps the two distinguishable for callers that
    /// only see a diagnostic list.
    AnalysisFailed => "analysis-failed",
    /// A document whose shell could not be parsed, so it was excluded from
    /// analysis entirely. Reported per document so the exclusion is visible
    /// rather than silent.
    DocumentQuarantined => "document-quarantined",
}

impl DiagCode {
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

    /// Every diagnostic wire name, exactly as it was published before the codes
    /// moved into one table. This list is the wire contract: `waml check --json`,
    /// the LSP `code` field and every serialized `Diagnostic` carry these strings,
    /// so a change here is a change other processes can see.
    ///
    /// It fails closed. `DiagCode::ALL` expands from the same table as the enum,
    /// so a new variant appears in it automatically and the comparison below stops
    /// matching by length; a renamed variant stops matching by value. Neither can
    /// pass silently -- the name has to be repeated here, deliberately.
    const WIRE_NAMES: &[&str] = &[
        "duplicate-slug",
        "frontmatter-not-clean",
        "unknown-type",
        "obsolete-diagram-type",
        "malformed-attribute",
        "malformed-relationship",
        "malformed-flow-bullet",
        "duplicate-flow-node",
        "unresolved-target",
        "missing-trace-document",
        "unresolved-trace-fragment",
        "malformed-trace-target",
        "unsupported-trace-scheme",
        "droppable-content",
        "malformed-layout",
        "unresolved-layout-ref",
        "layout-cycle",
        "layout-conflict",
        "invalid-use-case-group",
        "actor-inside-system-boundary",
        "use-case-outside-system-boundary",
        "use-case-in-multiple-system-boundaries",
        "empty-use-case-band",
        "malformed-message",
        "malformed-lifeline",
        "slot-unknown-attribute",
        "instance-of-non-classifier",
        "instance-of-unresolved",
        "unreachable-flow-node",
        "decision-without-guard",
        "empty-flow-document",
        "unknown-flow-target",
        "unknown-lifeline-handle",
        "uninvolved-lifeline",
        "fragment-zero-operands",
        "empty-operand-stream",
        "fragment-nesting-too-deep",
        "duplicate-sequence-name",
        "reserved-sequence-name",
        "unknown-sequence-endpoint",
        "invalid-sequence-endpoint",
        "invalid-lifeline-lifetime",
        "duplicate-call-identity",
        "unknown-call-identity",
        "unmatched-return",
        "ambiguous-return",
        "completed-return",
        "conflicting-return",
        "invalid-fragment-operands",
        "duplicate-gate",
        "invalid-interaction-use",
        "interaction-use-cycle",
        "unsupported-sequence-form",
        "unknown-view-middleware",
        "invalid-view-params",
        "view-stage-failed",
        "view-depth-exceeded",
        "view-cycle",
        "unknown-surface",
        "unknown-icon",
        "entangled-groups",
        "analysis-failed",
        "document-quarantined",
    ];

    #[test]
    fn wire_names_are_exhaustive_and_unchanged() {
        let live: Vec<&str> = DiagCode::ALL.iter().map(|c| c.as_str()).collect();
        assert_eq!(live, WIRE_NAMES);

        let mut unique = live.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), live.len(), "two codes share one wire name");
    }

    /// The serde name and `as_str` expand from the same literal, so this asserts
    /// that expansion actually holds for every variant -- and that a code parses
    /// back into itself, which is what a consumer reading a saved diagnostic does.
    #[cfg(feature = "serde")]
    #[test]
    fn every_code_round_trips_under_its_wire_name() {
        for &code in DiagCode::ALL {
            let json = serde_json::to_value(code).unwrap();
            assert_eq!(
                json,
                serde_json::Value::String(code.as_str().to_owned()),
                "serde name differs from as_str for {code:?}"
            );
            let back: DiagCode = serde_json::from_value(json).unwrap();
            assert_eq!(back, code);
        }
    }

    /// Wire names are pinned by `wire_names_are_exhaustive_and_unchanged`;
    /// this covers the other half of a code's contract, the severity it
    /// defaults to.
    #[test]
    fn code_severity_defaults_are_stable() {
        assert_eq!(DiagCode::UnknownType.severity(), Severity::Warning);
        assert_eq!(DiagCode::ObsoleteDiagramType.severity(), Severity::Error);
        assert_eq!(DiagCode::MalformedAttribute.severity(), Severity::Error);
        assert_eq!(DiagCode::SlotUnknownAttribute.severity(), Severity::Warning);
        assert_eq!(
            DiagCode::InstanceOfNonClassifier.severity(),
            Severity::Warning
        );
        assert_eq!(DiagCode::InstanceOfUnresolved.severity(), Severity::Warning);
        assert_eq!(DiagCode::UnknownViewMiddleware.severity(), Severity::Error);
        assert_eq!(DiagCode::InvalidViewParams.severity(), Severity::Error);
        assert_eq!(DiagCode::ViewStageFailed.severity(), Severity::Error);
        assert_eq!(DiagCode::ViewDepthExceeded.severity(), Severity::Error);
        assert_eq!(DiagCode::ViewCycle.severity(), Severity::Error);
        assert_eq!(DiagCode::UnknownSurface.severity(), Severity::Warning);
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
