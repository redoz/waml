use std::fmt;

use crate::source::{BundlePath, SourceBundle, SourceDocument, SourceError};

#[cfg(not(target_arch = "wasm32"))]
pub mod confine;

#[cfg(not(target_arch = "wasm32"))]
pub mod ingest;

#[cfg(not(target_arch = "wasm32"))]
pub mod persist;

#[derive(Debug)]
pub enum HostIngressError {
    ExistingDocument { path: BundlePath },
    MissingDocument { path: BundlePath },
    Source(SourceError),
}

impl fmt::Display for HostIngressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExistingDocument { path } => {
                write!(formatter, "document already exists: {path}")
            }
            Self::MissingDocument { path } => write!(formatter, "document does not exist: {path}"),
            Self::Source(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for HostIngressError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SourceError> for HostIngressError {
    fn from(value: SourceError) -> Self {
        Self::Source(value)
    }
}

#[doc(hidden)]
pub fn add_document(
    current: &SourceBundle,
    document: SourceDocument,
) -> Result<SourceBundle, HostIngressError> {
    if current.document(document.path()).is_some() {
        return Err(HostIngressError::ExistingDocument {
            path: document.path().clone(),
        });
    }
    let mut candidate = current.clone();
    candidate.push_source_document(document)?;
    Ok(candidate)
}

#[doc(hidden)]
pub fn replace_document(
    current: &SourceBundle,
    document: SourceDocument,
) -> Result<SourceBundle, HostIngressError> {
    let path = document.path().clone();
    if current.document(&path).is_none() {
        return Err(HostIngressError::MissingDocument { path });
    }
    let mut candidate = current.clone();
    *candidate
        .document_mut(&path)
        .expect("presence checked before candidate clone") = document;
    Ok(candidate)
}

#[doc(hidden)]
pub fn remove_document(
    current: &SourceBundle,
    path: &BundlePath,
) -> Result<SourceBundle, HostIngressError> {
    let Some(index) = current
        .documents()
        .iter()
        .position(|document| document.path() == path)
    else {
        return Err(HostIngressError::MissingDocument { path: path.clone() });
    };
    let mut candidate = current.clone();
    candidate
        .remove_document(index)
        .expect("presence checked before candidate clone");
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::source::{BundlePath, SourceBundle, SourceDocument};

    fn path(value: &str) -> BundlePath {
        BundlePath::parse(value).unwrap()
    }

    #[test]
    fn add_replace_remove_are_pure_and_preserve_untouched_text_allocations() {
        let original =
            SourceBundle::try_from_pairs([("a.md", "# A\n"), ("nested/b.md", "# B\n")]).unwrap();
        let untouched = original
            .document(&path("nested/b.md"))
            .unwrap()
            .text_arc()
            .clone();

        let new_document = SourceDocument::new(path("c.md"), "# C\n".into());
        let new_allocation = new_document.text_arc().clone();
        let added = super::add_document(&original, new_document).unwrap();
        assert!(original.document(&path("c.md")).is_none());
        assert!(Arc::ptr_eq(
            added.document(&path("c.md")).unwrap().text_arc(),
            &new_allocation
        ));
        assert!(Arc::ptr_eq(
            added.document(&path("nested/b.md")).unwrap().text_arc(),
            &untouched
        ));

        let replaced = super::replace_document(
            &added,
            SourceDocument::new(path("a.md"), "# Changed\n".into()),
        )
        .unwrap();
        assert_eq!(original.document(&path("a.md")).unwrap().text(), "# A\n");
        assert!(Arc::ptr_eq(
            replaced.document(&path("nested/b.md")).unwrap().text_arc(),
            &untouched
        ));

        let removed = super::remove_document(&replaced, &path("c.md")).unwrap();
        assert!(removed.document(&path("c.md")).is_none());
        assert!(replaced.document(&path("c.md")).is_some());
        assert!(Arc::ptr_eq(
            removed.document(&path("nested/b.md")).unwrap().text_arc(),
            &untouched
        ));
    }

    #[test]
    fn ingress_rejects_wrong_lifecycle_operation_without_changing_input() {
        let original = SourceBundle::try_from_pairs([("a.md", "# A\n")]).unwrap();
        let allocation = original.document(&path("a.md")).unwrap().text_arc().clone();

        assert!(super::add_document(
            &original,
            SourceDocument::new(path("a.md"), "# Other\n".into())
        )
        .is_err());
        assert!(super::replace_document(
            &original,
            SourceDocument::new(path("missing.md"), "# Missing\n".into())
        )
        .is_err());
        assert!(super::remove_document(&original, &path("missing.md")).is_err());
        assert_eq!(original.document(&path("a.md")).unwrap().text(), "# A\n");
        assert!(Arc::ptr_eq(
            original.document(&path("a.md")).unwrap().text_arc(),
            &allocation
        ));
    }
}
