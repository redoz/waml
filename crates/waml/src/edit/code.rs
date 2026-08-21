//! The machine-readable half of an [`EditError`](super::EditError): *why* an
//! operation was rejected, as a stable kebab-case code.
//!
//! This is deliberately NOT [`DiagCode`](crate::diagnostic::DiagCode). A
//! diagnostic describes a *document* -- it has a range, a severity, and it is
//! published by `waml check` and the LSP for content that already exists. An
//! [`EditCode`] describes a *rejected operation* -- it has a batch index, an op
//! name and a target, no range and no severity, because every one of them is
//! fatal to the transaction it belongs to. Folding these into `DiagCode` would
//! put codes into `DiagCode::ALL` that can never appear in a diagnostic list and
//! would force a `severity()` answer for things that have none.
//!
//! What *is* shared is the mechanism: one `macro_rules!` table where each
//! variant declares its wire name exactly once, an [`EditCode::ALL`] that
//! expands from the same table, and a test that pins every name.

/// Declares every edit-rejection code once, next to the kebab-case wire name it
/// is published under.
///
/// The wire name is a literal in a single table, and everything that needs it --
/// the `serde` rename, [`EditCode::as_str`] and [`EditCode::ALL`] -- expands
/// from that one literal. A variant without a name is a syntax error in this
/// macro, so the wire name and the programmatic name cannot drift apart.
macro_rules! edit_codes {
    ($(
        $(#[doc = $doc:literal])*
        $variant:ident => $wire:literal,
    )+) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub enum EditCode {
            $(
                $(#[doc = $doc])*
                #[cfg_attr(feature = "serde", serde(rename = $wire))]
                $variant,
            )+
        }

        impl EditCode {
            /// Every code, in declaration order. Expanded from the same table as
            /// the enum, so it cannot fall behind it.
            pub const ALL: &[EditCode] = &[$(EditCode::$variant,)+];

            /// The stable kebab-case wire name. Identical to the `serde` name by
            /// construction: both expand from the same literal.
            pub fn as_str(self) -> &'static str {
                match self {
                    $(EditCode::$variant => $wire,)+
                }
            }
        }
    };
}

edit_codes! {
    /// The document, package, concept, attribute, value or relationship the op
    /// addresses does not exist.
    NotFound => "not-found",
    /// The name the op would create -- or move/rename onto -- is already taken.
    AlreadyExists => "already-exists",
    /// The target exists, but it is the wrong kind of thing for this op. The
    /// case that named this code: `place.set` against a classifier, which has
    /// no `## Layout` section to write into and used to be written anyway.
    WrongTarget => "wrong-target",
    /// An argument is empty, malformed, or does not apply to the element it was
    /// given for (a relationship kind that takes no ends, a blank title).
    InvalidArgument => "invalid-argument",
    /// An index argument addresses past the end of the list it indexes into.
    OutOfRange => "out-of-range",
    /// The op is not defined for this target at all -- renaming or deleting the
    /// root package, for instance. Distinct from [`EditCode::WrongTarget`]:
    /// there is no target of any kind for which this op would be accepted here.
    Unsupported => "unsupported",
    /// The analysis, revision or undo history the op was built against no
    /// longer describes the source it is being applied to. The caller should
    /// re-read and retry rather than reword the request.
    StaleContext => "stale-context",
    /// The document on disk is missing structure the op needs: no clean
    /// frontmatter, no title heading, an attribute with no type reference.
    MalformedDocument => "malformed-document",
    /// Refused because other documents still point at the target. Carries the
    /// referrers in the message; `--cascade` is the caller's way through.
    ReferencedElsewhere => "referenced-elsewhere",
    /// An invariant this crate is responsible for maintaining was violated.
    /// A bug here, not a bad request -- the caller cannot fix it by asking
    /// differently.
    Internal => "internal",
}

impl std::fmt::Display for EditCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// An error type that already knows *why* it failed and can say so as an
/// [`EditCode`].
///
/// The point is to stop `.map_err(|error| EditError::at(op, error.to_string()))`
/// -- which took an error that had already been classified into variants and
/// threw the classification away, leaving the caller with prose. Every `impl`
/// below is an exhaustive `match`, so a new variant in the source enum is a
/// compile error here rather than a silently mis-coded rejection.
pub(crate) trait EditCoded {
    fn edit_code(&self) -> EditCode;
}

impl EditCoded for crate::source::SourceError {
    fn edit_code(&self) -> EditCode {
        use crate::source::SourceError;
        match self {
            SourceError::InvalidPath(_) | SourceError::InvalidRange { .. } => {
                EditCode::InvalidArgument
            }
            SourceError::DuplicatePath(_) => EditCode::AlreadyExists,
        }
    }
}

impl EditCoded for crate::host::HostIngressError {
    fn edit_code(&self) -> EditCode {
        use crate::host::HostIngressError;
        match self {
            HostIngressError::ExistingDocument { .. } => EditCode::AlreadyExists,
            HostIngressError::MissingDocument { .. } => EditCode::NotFound,
            HostIngressError::Source(error) => error.edit_code(),
        }
    }
}

impl EditCoded for waml_syntax::TextError {
    fn edit_code(&self) -> EditCode {
        use waml_syntax::TextError;
        match self {
            TextError::OutOfBounds { .. } => EditCode::OutOfRange,
            TextError::SourceTooLarge { .. }
            | TextError::WidthOverflow { .. }
            | TextError::ReversedRange { .. }
            | TextError::NonUtf8Boundary { .. } => EditCode::InvalidArgument,
        }
    }
}

impl EditCoded for waml_syntax::ParseError {
    fn edit_code(&self) -> EditCode {
        use waml_syntax::ParseError;
        match self {
            ParseError::SourceTooLarge { .. }
            | ParseError::InvalidRange { .. }
            | ParseError::WidthOverflow => EditCode::InvalidArgument,
            // The text we hand the parser is text this crate just rendered, so
            // a parser that stalls or breaks its own invariants on it is our
            // bug, not a bad request.
            ParseError::StructuralInvariant { .. } | ParseError::ParserStalled { .. } => {
                EditCode::Internal
            }
            ParseError::NonMonotonicRevision { .. } => EditCode::StaleContext,
        }
    }
}

impl EditCoded for crate::analysis::AnalysisError {
    fn edit_code(&self) -> EditCode {
        use crate::analysis::AnalysisError;
        match self {
            AnalysisError::SourceTooLarge { .. } => EditCode::InvalidArgument,
            AnalysisError::Shell { .. } => EditCode::MalformedDocument,
            AnalysisError::Okf(error) => error.edit_code(),
            AnalysisError::InvalidPromotedMarkdownUpdate { .. } => EditCode::StaleContext,
            // The candidate this crate built violates an invariant this crate
            // maintains. The caller cannot fix that by asking differently.
            AnalysisError::CatalogInvariant { .. }
            | AnalysisError::Specialization { .. }
            | AnalysisError::AmbiguousClaim { .. }
            | AnalysisError::StructuralInvariant { .. } => EditCode::Internal,
        }
    }
}

impl From<crate::analysis::AnalysisError> for super::EditError {
    fn from(error: crate::analysis::AnalysisError) -> Self {
        super::EditError::wrap("analysis.prepare", &error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every edit-rejection wire name. This list is the wire contract: the
    /// `code` field of the 422 body `POST /api/ops` returns carries these
    /// strings, so a change here is a change another process can see.
    ///
    /// It fails closed. `EditCode::ALL` expands from the same table as the
    /// enum, so a new variant appears in it automatically and the comparison
    /// below stops matching by length; a renamed variant stops matching by
    /// value. Neither can pass silently -- the name has to be repeated here,
    /// deliberately.
    const WIRE_NAMES: &[&str] = &[
        "not-found",
        "already-exists",
        "wrong-target",
        "invalid-argument",
        "out-of-range",
        "unsupported",
        "stale-context",
        "malformed-document",
        "referenced-elsewhere",
        "internal",
    ];

    #[test]
    fn wire_names_are_exhaustive_and_unchanged() {
        let live: Vec<&str> = EditCode::ALL.iter().map(|code| code.as_str()).collect();
        assert_eq!(live, WIRE_NAMES);

        let mut unique = live.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), live.len(), "two codes share one wire name");
    }

    /// The serde name and `as_str` expand from the same literal, so this
    /// asserts that expansion actually holds for every variant -- and that a
    /// code parses back into itself, which is what a client reading a rejected
    /// op does.
    #[cfg(feature = "serde")]
    #[test]
    fn every_code_round_trips_under_its_wire_name() {
        for &code in EditCode::ALL {
            let json = serde_json::to_value(code).unwrap();
            assert_eq!(
                json,
                serde_json::Value::String(code.as_str().to_owned()),
                "serde name differs from as_str for {code:?}"
            );
            let back: EditCode = serde_json::from_value(json).unwrap();
            assert_eq!(back, code);
        }
    }

    #[test]
    fn display_is_the_wire_name() {
        assert_eq!(EditCode::WrongTarget.to_string(), "wrong-target");
    }
}
