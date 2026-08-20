//! Compatibility validation entry points backed by the parser-platform analysis.
//!
//! These functions do not parse independently. They adapt legacy tuple callers
//! to the authoritative `SourceBundle -> prepare_candidate` pipeline.
//!
//! Every entry point here answers "what is wrong with this bundle?", so failing
//! to analyse it must never be reported the same way as analysing it and
//! finding nothing wrong. An empty result means clean; a failure produces an
//! `analysis-failed` diagnostic.

use std::collections::BTreeMap;

use crate::{
    analysis::{prepare_candidate, PreparedCandidate},
    diagnostic::{DiagCode, Diagnostic},
    source::{BundlePath, SourceBundle},
};

/// Renders a bundle-level failure as a diagnostic, so callers that can only
/// see a `Vec<Diagnostic>` still learn that analysis did not happen.
fn analysis_failed(error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::new(
        DiagCode::AnalysisFailed,
        format!("bundle analysis failed: {error}"),
        String::new(),
        1,
    )]
}

pub fn validate_from_source(bundle: &SourceBundle) -> Vec<Diagnostic> {
    match prepare_candidate(bundle.clone(), None, 0) {
        Ok(candidate) => candidate.diagnostics(),
        Err(error) => analysis_failed(error),
    }
}

pub fn validate(bundle: &[(String, String)]) -> Vec<Diagnostic> {
    let mut display_paths = BTreeMap::new();
    let normalized = bundle.iter().enumerate().map(|(index, (path, text))| {
        let normalized = BundlePath::parse(path.clone())
            .map(|path| path.to_string())
            .unwrap_or_else(|_| {
                let basename = path
                    .replace('\\', "/")
                    .rsplit('/')
                    .next()
                    .unwrap_or("document")
                    .to_owned();
                format!("__compat/{index}/{basename}")
            });
        display_paths.insert(normalized.clone(), path.clone());
        (normalized, text.clone())
    });
    let source = match SourceBundle::try_from_pairs(normalized) {
        Ok(source) => source,
        Err(error) => return analysis_failed(error),
    };
    let candidate = match prepare_candidate(source, None, 0) {
        Ok(candidate) => candidate,
        Err(error) => return analysis_failed(error),
    };
    diagnostics(&candidate, &display_paths)
}

pub fn prepare(files: &[(String, String)]) -> Result<PreparedCandidate, String> {
    let source = SourceBundle::try_from_pairs(files.iter().cloned()).map_err(|e| e.to_string())?;
    prepare_candidate(source, None, 0).map_err(|e| e.to_string())
}

pub fn diagnostics(
    candidate: &PreparedCandidate,
    display_paths: &BTreeMap<String, String>,
) -> Vec<Diagnostic> {
    candidate
        .diagnostics()
        .into_iter()
        .map(|mut diagnostic| {
            if let Some(display) = display_paths.get(&diagnostic.file) {
                diagnostic.file.clone_from(display);
            }
            diagnostic
        })
        .collect()
}
