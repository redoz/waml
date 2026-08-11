//! Compatibility validation entry points backed by the parser-platform analysis.
//!
//! These functions do not parse independently. They adapt legacy tuple callers
//! to the authoritative `SourceBundle -> prepare_candidate` pipeline.

use std::collections::BTreeMap;

use crate::{
    analysis::{prepare_candidate, PreparedCandidate},
    diagnostic::Diagnostic,
    source::{BundlePath, SourceBundle},
};

pub fn validate_from_source(bundle: &SourceBundle) -> Vec<Diagnostic> {
    prepare_candidate(bundle.clone(), None, 0)
        .map(|candidate| candidate.uml().diagnostics.iter().cloned().collect())
        .unwrap_or_default()
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
    let Ok(source) = SourceBundle::try_from_pairs(normalized) else {
        return Vec::new();
    };
    let Ok(candidate) = prepare_candidate(source, None, 0) else {
        return Vec::new();
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
        .uml()
        .diagnostics
        .iter()
        .cloned()
        .map(|mut diagnostic| {
            if let Some(display) = display_paths.get(&diagnostic.file) {
                diagnostic.file.clone_from(display);
            }
            diagnostic
        })
        .collect()
}
