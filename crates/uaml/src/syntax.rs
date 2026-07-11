use crate::frontmatter::Frontmatter;
use crate::model::{Attribute, RelEnd, RelationshipKind};

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub frontmatter: Frontmatter,
    pub title: String,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Section {
    Attributes(Vec<Attribute>),
    Values(Vec<String>),
    Relationships(Vec<ParsedRel>),
    Body(String),
    Notes(Vec<String>),
    Members(Vec<MemberLine>),
    RenderHints(Vec<HintLine>),
    /// An unrecognized `## Section`, preserved verbatim (graceful degradation).
    Unknown { title: String, raw: String },
}

/// A relationship's optional `as …` name, as written in one document.
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedName {
    Label(String),
    Ref { title: String, slug: String },
}

/// One `## Relationships` bullet, parsed but not yet resolved against the bundle.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRel {
    pub kind: RelationshipKind,
    pub target_title: String,
    pub target_slug: String,
    pub name: Option<ParsedName>,
    pub from_end: RelEnd,
    pub to_end: RelEnd,
}

/// One `## Members` bullet in a diagram document.
#[derive(Debug, Clone, PartialEq)]
pub struct MemberLine {
    pub title: String,
    pub slug: String,
    pub position: Option<(f64, f64)>,
}

/// One `## Render hints` bullet.
#[derive(Debug, Clone, PartialEq)]
pub enum HintLine {
    Emphasize(Vec<String>),
    Collapse { title: String, slug: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_is_constructible() {
        let doc = Document {
            frontmatter: Frontmatter::default(),
            title: "Order".to_string(),
            sections: vec![Section::Relationships(vec![ParsedRel {
                kind: RelationshipKind::Composes,
                target_title: "OrderLine".to_string(),
                target_slug: "order-line".to_string(),
                name: None,
                from_end: RelEnd::default(),
                to_end: RelEnd::default(),
            }])],
        };
        assert_eq!(doc.title, "Order");
        assert_eq!(doc.sections.len(), 1);
    }
}
