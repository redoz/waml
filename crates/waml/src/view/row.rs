//! Row identity types: `RowPath`, `ViewId`, `RowId`.
//!
//! `RowPath` is a "/"-separated, non-empty-segment path. It is syntactically
//! transparent — anyone may split it — but semantically owned: only the
//! middleware that emitted a row says what its segments mean.

use std::fmt;

use super::chain::Chain;
use super::surface::SurfaceId;

/// Error constructing a [`RowPath`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowPathError {
    /// The path (or a joined segment) was empty.
    Empty,
    /// A segment between two '/' separators (or the whole path) was empty,
    /// e.g. `"a//b"`, `"/a"`, `"a/"`.
    EmptySegment,
}

impl fmt::Display for RowPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RowPathError::Empty => write!(f, "row path must not be empty"),
            RowPathError::EmptySegment => write!(f, "row path must not contain empty segments"),
        }
    }
}

impl std::error::Error for RowPathError {}

/// "/"-separated, non-empty segments. Syntactically transparent, semantically
/// owned: anyone may split it; only the owning middleware says what a segment
/// means.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RowPath(String);

impl RowPath {
    /// Rejects empty paths, empty segments, and leading/trailing '/'.
    pub fn parse(text: &str) -> Result<RowPath, RowPathError> {
        if text.is_empty() {
            return Err(RowPathError::Empty);
        }
        if text.starts_with('/') || text.ends_with('/') || text.split('/').any(str::is_empty) {
            return Err(RowPathError::EmptySegment);
        }
        Ok(RowPath(text.to_string()))
    }

    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    pub fn parent(&self) -> Option<RowPath> {
        let (parent, _last) = self.0.rsplit_once('/')?;
        Some(RowPath(parent.to_string()))
    }

    pub fn starts_with(&self, other: &RowPath) -> bool {
        let mut self_segments = self.segments();
        for other_segment in other.segments() {
            match self_segments.next() {
                Some(seg) if seg == other_segment => continue,
                _ => return false,
            }
        }
        true
    }

    /// Appends a single segment. Rejects an empty or "/"-containing segment.
    pub fn join(&self, segment: &str) -> Result<RowPath, RowPathError> {
        if segment.is_empty() || segment.contains('/') {
            return Err(RowPathError::EmptySegment);
        }
        Ok(RowPath(format!("{}/{}", self.0, segment)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RowPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The emitting middleware's declared name, disambiguated when a name repeats
/// within one chain (`"group-by-tag#2"`). Folder-scoped; NOT chain position —
/// inserting a stage must not invalidate persisted `RowId`s below it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ViewId(String);

impl ViewId {
    pub fn new(name: impl Into<String>) -> ViewId {
        ViewId(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Assigns disambiguated `ViewId`s for a chain's declared stage names,
    /// in order: the first occurrence of a name keeps it bare, later
    /// occurrences of the same name get `#2`, `#3`, ...
    pub fn disambiguate<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<ViewId> {
        let mut seen: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
        names
            .into_iter()
            .map(|name| {
                let count = seen.entry(name).or_insert(0);
                *count += 1;
                if *count == 1 {
                    ViewId::new(name)
                } else {
                    ViewId::new(format!("{name}#{count}"))
                }
            })
            .collect()
    }
}

impl fmt::Display for ViewId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RowId {
    pub owner: ViewId,
    pub path: RowPath,
}

/// What a row points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowTarget {
    /// A real concept document, by href.
    Concept(String),
    /// A real child directory, by address string.
    Folder(String),
    /// No file behind it. The owner interprets the RowPath.
    Virtual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RowCaps {
    pub rename: bool,
    pub delete: bool,
    pub move_out: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChildCaps {
    pub reorder: bool,
    pub insert: bool,
    pub accept_move_in: bool,
}

/// Error constructing a [`Row`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowConstructError {
    /// A `Virtual` target was given without a `surface` — there is no
    /// document to infer a default surface from.
    VirtualWithoutSurface,
}

impl fmt::Display for RowConstructError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RowConstructError::VirtualWithoutSurface => {
                write!(f, "a virtual row must declare an explicit surface")
            }
        }
    }
}

impl std::error::Error for RowConstructError {}

pub struct Row {
    pub id: RowId,
    pub label: String,
    pub blurb: Option<String>,
    pub target: RowTarget,
    /// None ⇒ default resolution by document type. Middleware may override.
    pub surface: Option<SurfaceId>,
    /// Folder rows only: the chain used when this row expands. Lazy.
    pub expand: Option<Chain>,
    pub caps: RowCaps,
    pub child_caps: ChildCaps,
}

impl Row {
    /// The only constructor. Enforces: a Virtual target MUST name a surface —
    /// there is no document to infer one from.
    pub fn new(
        id: RowId,
        label: String,
        target: RowTarget,
        surface: Option<SurfaceId>,
    ) -> Result<Row, RowConstructError> {
        if matches!(target, RowTarget::Virtual) && surface.is_none() {
            return Err(RowConstructError::VirtualWithoutSurface);
        }
        Ok(Row {
            id,
            label,
            blurb: None,
            target,
            surface,
            expand: None,
            caps: RowCaps::default(),
            child_caps: ChildCaps::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_path_parses_segments_and_parent() {
        let path = RowPath::parse("a/b/c").unwrap();
        assert_eq!(path.segments().collect::<Vec<_>>(), vec!["a", "b", "c"]);
        assert_eq!(path.parent().unwrap().as_str(), "a/b");

        let root = RowPath::parse("a").unwrap();
        assert_eq!(root.parent(), None);
    }

    #[test]
    fn row_path_rejects_empty_and_empty_segments() {
        assert!(RowPath::parse("").is_err());
        assert!(RowPath::parse("a//b").is_err());
        assert!(RowPath::parse("/a").is_err());
        assert!(RowPath::parse("a/").is_err());
    }

    #[test]
    fn starts_with_is_segment_wise_not_string_prefix() {
        let ab_c = RowPath::parse("ab/c").unwrap();
        let a = RowPath::parse("a").unwrap();
        assert!(!ab_c.starts_with(&a));

        let a_b = RowPath::parse("a/b").unwrap();
        assert!(a_b.starts_with(&a));
    }

    #[test]
    fn view_id_disambiguates_repeats_within_one_chain() {
        let names = vec!["name", "other", "name", "name"];
        let ids = ViewId::disambiguate(names.clone());
        assert_eq!(
            ids.iter().map(ViewId::as_str).collect::<Vec<_>>(),
            vec!["name", "other", "name#2", "name#3"]
        );

        // Stable across two runs.
        let ids_again = ViewId::disambiguate(names);
        assert_eq!(ids, ids_again);
    }

    fn row_id(name: &str) -> RowId {
        RowId {
            owner: ViewId::new("owner"),
            path: RowPath::parse(name).unwrap(),
        }
    }

    #[test]
    fn a_virtual_row_without_a_surface_is_rejected_at_construction() {
        let result = Row::new(row_id("a"), "A".to_string(), RowTarget::Virtual, None);
        assert_eq!(result.err(), Some(RowConstructError::VirtualWithoutSurface));
    }

    #[test]
    fn a_real_target_with_surface_none_constructs() {
        let result = Row::new(
            row_id("a"),
            "A".to_string(),
            RowTarget::Concept("a.waml".to_string()),
            None,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn caps_default_to_nothing() {
        let row = Row::new(
            row_id("a"),
            "A".to_string(),
            RowTarget::Concept("a.waml".to_string()),
            None,
        )
        .unwrap();
        assert_eq!(row.caps, RowCaps::default());
        assert_eq!(row.child_caps, ChildCaps::default());
        assert!(!row.caps.rename && !row.caps.delete && !row.caps.move_out);
        assert!(
            !row.child_caps.reorder && !row.child_caps.insert && !row.child_caps.accept_move_in
        );
    }
}
