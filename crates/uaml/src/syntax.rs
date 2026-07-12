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
    Members(MembersBlock),
    Layout(Vec<LayoutStatement>),
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
    /// 1-based line within the document (0 until filled by `parse`).
    pub line: usize,
    /// Byte range within `line`, if positioned by `parse`.
    pub span: Option<(usize, usize)>,
}

/// One `## Members` bullet in a diagram document.
#[derive(Debug, Clone, PartialEq)]
pub struct MemberLine {
    pub title: String,
    pub slug: String,
    /// 1-based line within the document (0 until filled by `parse`).
    pub line: usize,
    /// Byte range within `line`, if positioned by `parse`.
    pub span: Option<(usize, usize)>,
}

/// The `## Members` section: a forest of groups. A flat bullet list (no
/// sub-headings) is a single implicit top-level group (name `""`, depth 0).
#[derive(Debug, Clone, PartialEq)]
pub struct MembersBlock {
    pub groups: Vec<MemberGroup>,
}

/// A membership group. `name` is the heading text (`""` for the implicit
/// top-level group); `depth` is the heading level (3 for `###`, 0 implicit).
#[derive(Debug, Clone, PartialEq)]
pub struct MemberGroup {
    pub name: String,
    pub depth: u8,
    pub members: Vec<MemberLine>,
    pub children: Vec<MemberGroup>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LayoutStatement {
    /// `A left of B above C` — N operands, N-1 directions.
    Placement { operands: Vec<Operand>, directions: Vec<Direction> },
    /// `top of X aligned with top of Y`
    Alignment { left: Anchored, right: Anchored },
    /// A lone operand — meaningful when it carries `as`/`with` treatment.
    Standalone(Operand),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction { LeftOf, RightOf, Above, Below }

#[derive(Debug, Clone, PartialEq)]
pub struct Anchored { pub edge: Option<Edge>, pub operand: Operand }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Edge { Top, Bottom, Left, Right, Center }

#[derive(Debug, Clone, PartialEq)]
pub struct Operand {
    pub ref_: OperandRef,
    pub axis: Option<Axis>,
    pub hints: Vec<Hint>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Axis { Row, Column }

#[derive(Debug, Clone, PartialEq)]
pub enum OperandRef {
    Name(NameRef),
    InlineGroup { axis: Axis, items: Vec<Operand> },
    Paren(Box<Operand>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum NameRef {
    Link { title: String, slug: String },
    Bare(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hint { Shape(Shape), Margin(Margin), Flag(Flag) }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape { Frame, Box, Shrink }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Margin { No, Small, Medium, Large }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Flag { Emphasized, Collapsed }

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
                line: 0,
                span: None,
            }])],
        };
        assert_eq!(doc.title, "Order");
        assert_eq!(doc.sections.len(), 1);
    }

    #[test]
    fn layout_statement_is_constructible() {
        let stmt = LayoutStatement::Placement {
            operands: vec![
                Operand { ref_: OperandRef::Name(NameRef::Bare("Users".into())), axis: None, hints: vec![] },
                Operand { ref_: OperandRef::Name(NameRef::Bare("Orders".into())), axis: None, hints: vec![] },
            ],
            directions: vec![Direction::LeftOf],
        };
        match stmt {
            LayoutStatement::Placement { operands, directions } => {
                assert_eq!(operands.len(), 2);
                assert_eq!(directions, vec![Direction::LeftOf]);
            }
            _ => panic!("wrong variant"),
        }
    }
}
