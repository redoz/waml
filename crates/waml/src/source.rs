use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

static BUNDLE_MARKER_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?m)^<!--\s*(.+?\.md)\s*-->[ \t]*\n").expect("valid bundle marker regex")
});

/// Decode the CLI concatenated-document transport envelope.
///
/// This helper performs no WAML parsing and creates no syntax authority.
pub fn split_bundle(text: &str) -> Vec<(String, String)> {
    let marks: Vec<_> = BUNDLE_MARKER_RE
        .captures_iter(text)
        .map(|capture| {
            let whole = capture.get(0).expect("whole match");
            (whole.start(), whole.end(), capture[1].to_owned())
        })
        .collect();
    if marks.is_empty() {
        return vec![("pasted/doc.md".to_owned(), text.to_owned())];
    }
    marks
        .iter()
        .enumerate()
        .map(|(index, (_, content_start, path))| {
            let end = marks
                .get(index + 1)
                .map(|(marker_start, _, _)| *marker_start)
                .unwrap_or(text.len());
            (path.clone(), text[*content_start..end].to_owned())
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceError {
    InvalidPath(String),
    DuplicatePath(String),
    InvalidRange { path: String, range: Range<usize> },
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceError::InvalidPath(path) => write!(f, "invalid bundle path: {path}"),
            SourceError::DuplicatePath(path) => write!(f, "duplicate bundle path: {path}"),
            SourceError::InvalidRange { path, range } => {
                write!(
                    f,
                    "invalid source range {}..{} for {path}",
                    range.start, range.end
                )
            }
        }
    }
}

impl std::error::Error for SourceError {}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BundlePath(String);

impl BundlePath {
    pub fn parse(path: impl Into<String>) -> Result<Self, SourceError> {
        let original = path.into();
        let normalized = original.replace('\\', "/");
        let invalid = normalized.is_empty()
            || normalized.starts_with('/')
            || normalized
                .as_bytes()
                .get(1)
                .is_some_and(|byte| *byte == b':')
            || !normalized.ends_with(".md")
            || normalized
                .split('/')
                .any(|segment| segment.is_empty() || segment == "." || segment == "..");
        if invalid {
            return Err(SourceError::InvalidPath(original));
        }
        Ok(BundlePath(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn concept_id(&self) -> Option<&str> {
        self.0.strip_suffix(".md")
    }
}

impl fmt::Display for BundlePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for BundlePath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceDocument {
    path: BundlePath,
    text: Arc<String>,
}

impl SourceDocument {
    pub fn new(path: BundlePath, text: String) -> Self {
        SourceDocument {
            path,
            text: Arc::new(text),
        }
    }

    pub fn path(&self) -> &BundlePath {
        &self.path
    }

    pub fn text(&self) -> &str {
        self.text.as_str()
    }

    pub fn text_shared(&self) -> &Arc<String> {
        &self.text
    }

    pub(crate) fn text_mut(&mut self) -> &mut String {
        Arc::make_mut(&mut self.text)
    }

    pub fn slice(&self, range: Range<usize>) -> Result<SourceSlice, SourceError> {
        if range.start > range.end
            || range.end > self.text.len()
            || !self.text.is_char_boundary(range.start)
            || !self.text.is_char_boundary(range.end)
        {
            return Err(SourceError::InvalidRange {
                path: self.path.to_string(),
                range,
            });
        }
        Ok(SourceSlice {
            source: self.text.clone(),
            range,
        })
    }

    pub(crate) fn text_arc(&self) -> &Arc<String> {
        &self.text
    }
}

#[cfg(test)]
pub(crate) fn source_text_weak(document: &SourceDocument) -> std::sync::Weak<String> {
    Arc::downgrade(&document.text)
}

#[derive(Clone, Debug)]
pub struct SourceSlice {
    source: Arc<String>,
    range: Range<usize>,
}

impl Default for SourceSlice {
    fn default() -> Self {
        SourceSlice::from(String::new())
    }
}

impl PartialEq for SourceSlice {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for SourceSlice {}

impl From<String> for SourceSlice {
    fn from(text: String) -> Self {
        let end = text.len();
        SourceSlice {
            source: Arc::new(text),
            range: 0..end,
        }
    }
}

impl std::ops::Deref for SourceSlice {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl SourceSlice {
    pub(crate) fn from_shared_range(
        source: Arc<String>,
        range: Range<usize>,
    ) -> Result<Self, SourceError> {
        if range.start > range.end
            || range.end > source.len()
            || !source.is_char_boundary(range.start)
            || !source.is_char_boundary(range.end)
        {
            return Err(SourceError::InvalidRange {
                path: "<analysis>".into(),
                range,
            });
        }
        Ok(Self { source, range })
    }

    pub fn as_str(&self) -> &str {
        &self.source[self.range.clone()]
    }

    #[cfg(test)]
    pub(crate) fn source_arc(&self) -> &Arc<String> {
        &self.source
    }
}

impl AsRef<str> for SourceSlice {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for SourceSlice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for SourceSlice {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let text = <String as serde::Deserialize>::deserialize(deserializer)?;
        let end = text.len();
        Ok(SourceSlice {
            source: Arc::new(text),
            range: 0..end,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceBundle {
    documents: Vec<SourceDocument>,
    by_path: BTreeMap<BundlePath, usize>,
}

impl SourceBundle {
    pub fn try_from_pairs<I, P, T>(pairs: I) -> Result<Self, SourceError>
    where
        I: IntoIterator<Item = (P, T)>,
        P: Into<String>,
        T: Into<String>,
    {
        let mut bundle = SourceBundle::default();
        for (path, text) in pairs {
            let path = BundlePath::parse(path)?;
            if bundle.by_path.contains_key(&path) {
                return Err(SourceError::DuplicatePath(path.to_string()));
            }
            let index = bundle.documents.len();
            bundle.by_path.insert(path.clone(), index);
            bundle
                .documents
                .push(SourceDocument::new(path, text.into()));
        }
        Ok(bundle)
    }

    pub fn documents(&self) -> &[SourceDocument] {
        &self.documents
    }

    pub fn document(&self, path: &BundlePath) -> Option<&SourceDocument> {
        self.by_path
            .get(path)
            .and_then(|index| self.documents.get(*index))
    }

    pub(crate) fn document_mut(&mut self, path: &BundlePath) -> Option<&mut SourceDocument> {
        let index = *self.by_path.get(path)?;
        self.documents.get_mut(index)
    }

    pub(crate) fn document_at(&self, index: usize) -> Option<&SourceDocument> {
        self.documents.get(index)
    }

    pub(crate) fn document_at_mut(&mut self, index: usize) -> Option<&mut SourceDocument> {
        self.documents.get_mut(index)
    }

    pub(crate) fn retain_documents(&mut self, mut keep: impl FnMut(&SourceDocument) -> bool) {
        self.documents.retain(|document| keep(document));
        self.rebuild_index();
    }

    pub(crate) fn upsert(&mut self, path: BundlePath, text: String) {
        if let Some(document) = self.document_mut(&path) {
            if document.text() != text {
                *document.text_mut() = text;
            }
            return;
        }
        let index = self.documents.len();
        self.by_path.insert(path.clone(), index);
        self.documents.push(SourceDocument::new(path, text));
    }

    pub(crate) fn push_document(
        &mut self,
        path: impl Into<String>,
        text: String,
    ) -> Result<(), SourceError> {
        let path = BundlePath::parse(path)?;
        if self.by_path.contains_key(&path) {
            return Err(SourceError::DuplicatePath(path.to_string()));
        }
        self.upsert(path, text);
        Ok(())
    }

    pub(crate) fn push_source_document(
        &mut self,
        document: SourceDocument,
    ) -> Result<(), SourceError> {
        if self.by_path.contains_key(document.path()) {
            return Err(SourceError::DuplicatePath(document.path().to_string()));
        }
        let index = self.documents.len();
        self.by_path.insert(document.path().clone(), index);
        self.documents.push(document);
        Ok(())
    }

    pub(crate) fn remove_document(&mut self, index: usize) -> Option<SourceDocument> {
        if index >= self.documents.len() {
            return None;
        }
        let document = self.documents.remove(index);
        self.rebuild_index();
        Some(document)
    }

    pub(crate) fn rename_document(
        &mut self,
        index: usize,
        path: impl Into<String>,
    ) -> Result<(), SourceError> {
        let path = BundlePath::parse(path)?;
        if self
            .by_path
            .get(&path)
            .is_some_and(|existing| *existing != index)
        {
            return Err(SourceError::DuplicatePath(path.to_string()));
        }
        let document = self
            .documents
            .get_mut(index)
            .ok_or_else(|| SourceError::InvalidPath(path.to_string()))?;
        document.path = path;
        self.rebuild_index();
        Ok(())
    }

    pub fn document_by_concept_id(&self, id: &str) -> Option<&SourceDocument> {
        self.documents
            .iter()
            .find(|document| document.path.concept_id() == Some(id))
    }

    pub fn to_pairs(&self) -> Vec<(String, String)> {
        self.documents
            .iter()
            .map(|document| (document.path.to_string(), document.text().to_owned()))
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    pub fn len(&self) -> usize {
        self.documents.len()
    }

    pub fn shares_text_with(&self, other: &SourceBundle, path: &str) -> bool {
        let Ok(path) = BundlePath::parse(path) else {
            return false;
        };
        match (self.document(&path), other.document(&path)) {
            (Some(left), Some(right)) => Arc::ptr_eq(left.text_arc(), right.text_arc()),
            _ => false,
        }
    }

    fn rebuild_index(&mut self) {
        self.by_path = self
            .documents
            .iter()
            .enumerate()
            .map(|(index, document)| (document.path.clone(), index))
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn bundle_path_is_relative_case_preserving_and_slash_normalized() {
        let path = BundlePath::parse(r"Sales\Orders\Order.md").unwrap();
        assert_eq!(path.as_str(), "Sales/Orders/Order.md");
        assert_eq!(path.concept_id(), Some("Sales/Orders/Order"));
    }

    #[test]
    fn bundle_path_accepts_utf8() {
        let path = BundlePath::parse("Ventes/Café.md").unwrap();
        assert_eq!(path.as_str(), "Ventes/Café.md");
        assert_eq!(path.concept_id(), Some("Ventes/Café"));
    }

    #[test]
    fn invalid_bundle_paths_are_rejected() {
        for invalid in [
            "",
            "../order.md",
            "sales/../../order.md",
            "/order.md",
            r"\order.md",
            "C:/order.md",
            "sales//order.md",
            "sales/./order.md",
            "sales/order",
            "sales/order.MD",
        ] {
            assert!(BundlePath::parse(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn duplicate_normalized_paths_are_rejected() {
        let result = SourceBundle::try_from_pairs([
            (r"sales\order.md", "# Order"),
            ("sales/order.md", "# Duplicate"),
        ]);
        assert!(matches!(result, Err(SourceError::DuplicatePath(_))));
    }

    #[test]
    fn source_slice_shares_allocation_and_validates_utf8_boundaries() {
        let doc = SourceDocument::new(
            BundlePath::parse("notes.md").unwrap(),
            "# Café\nBody".into(),
        );
        let body = doc.slice(8..12).unwrap();
        assert_eq!(body.as_str(), "Body");
        assert!(doc.slice(5..6).is_err());
        assert!(Arc::ptr_eq(doc.text_arc(), body.source_arc()));
    }

    #[test]
    fn test_weak_source_handle_tracks_document_allocation() {
        let document =
            SourceDocument::new(BundlePath::parse("notes.md").unwrap(), "# Notes".into());
        let weak = source_text_weak(&document);
        assert!(weak.upgrade().is_some());
        drop(document);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn clone_and_edit_copy_only_the_touched_document() {
        let current = SourceBundle::try_from_pairs([("a.md", "# A\n"), ("b.md", "# B\n")]).unwrap();
        let candidate = current.clone();
        assert!(current.shares_text_with(&candidate, "a.md"));
        assert!(current.shares_text_with(&candidate, "b.md"));

        let mut edited = candidate;
        let path = BundlePath::parse("a.md").unwrap();
        *edited.document_mut(&path).unwrap().text_mut() = "# Changed\n".into();

        assert!(!current.shares_text_with(&edited, "a.md"));
        assert!(current.shares_text_with(&edited, "b.md"));
    }

    #[test]
    fn lookup_uses_path_or_full_concept_id_without_basename_fallback() {
        let bundle = SourceBundle::try_from_pairs([
            ("sales/order.md", "# Sales"),
            ("support/order.md", "# Support"),
        ])
        .unwrap();
        let sales = BundlePath::parse("sales/order.md").unwrap();

        assert_eq!(bundle.document(&sales).unwrap().text(), "# Sales");
        assert_eq!(
            bundle
                .document_by_concept_id("support/order")
                .unwrap()
                .text(),
            "# Support"
        );
        assert!(bundle.document_by_concept_id("order").is_none());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn source_slice_serializes_as_a_string_without_implementation_fields() {
        let doc = SourceDocument::new(
            BundlePath::parse("notes.md").unwrap(),
            "# Notes\nBody".into(),
        );
        let slice = doc.slice(8..12).unwrap();

        let json = serde_json::to_value(&slice).unwrap();
        assert_eq!(json, serde_json::Value::String("Body".into()));
        let decoded: SourceSlice = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.as_str(), "Body");
    }
}
