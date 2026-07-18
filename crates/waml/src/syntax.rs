use crate::diagnostic::DiagCode;
use crate::frontmatter::Frontmatter;
use crate::model::{Attribute, RelEnd, RelationshipKind};

/// A malformed or droppable source line preserved verbatim in the tree.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorNode {
    pub raw: String,          // the original line, byte-for-byte (for serialize)
    pub line: usize,          // 1-based line within the source document
    pub span: (usize, usize), // byte range within `line`
    pub code: DiagCode,       // which syntactic diagnostic this line yields
    pub message: String,      // the derived diagnostic message
}

/// One bullet-section item: a well-formed node, or a preserved error line.
#[derive(Debug, Clone, PartialEq)]
pub enum Line<T> {
    Parsed(T),
    Error(ErrorNode),
}

impl<T> Line<T> {
    pub fn parsed(&self) -> Option<&T> {
        match self {
            Line::Parsed(t) => Some(t),
            Line::Error(_) => None,
        }
    }

    pub fn parsed_mut(&mut self) -> Option<&mut T> {
        match self {
            Line::Parsed(t) => Some(t),
            Line::Error(_) => None,
        }
    }
}

/// One `## Layout` bullet: a parsed statement plus its source line.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutItem {
    pub line: usize,
    pub stmt: LayoutStatement,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub frontmatter: Frontmatter,
    pub title: String,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Section {
    Attributes(Vec<Line<Attribute>>),
    Values(Vec<Line<String>>),
    /// An `InstanceSpecification`'s slot values.
    Slots(Vec<Line<ParsedSlot>>),
    Relationships(Vec<Line<ParsedRel>>),
    Body(String),
    Notes(Vec<Line<String>>),
    /// A flow document's `## Nodes` section (one directed graph).
    Nodes(FlowBlock),
    /// A sequence document's participants.
    Lifelines(Vec<Line<LifelineLine>>),
    /// A sequence document's ordered messages.
    Messages(MessagesBlock),
    Members(MembersBlock),
    Layout(Vec<Line<LayoutItem>>),
    /// An unrecognized `## Section`, preserved verbatim (graceful degradation).
    Unknown {
        title: String,
        raw: String,
    },
}

/// A `## Slots` value's SURFACE form (preserved for byte-identical round-trip),
/// distinct from the resolved `model::Slot`.
#[derive(Debug, Clone, PartialEq)]
pub enum SlotValue {
    /// A `"quoted string"` literal (quotes are part of the surface form).
    Quoted(String),
    /// A bare identifier or number (`PLACED`, `3`).
    Bare(String),
    /// A `[Label](./slug.md)` link (instance-valued slot); resolved downstream.
    Link(LinkRef),
}

/// One `## Slots` bullet: `- name: value`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSlot {
    pub name: String,
    pub value: SlotValue,
    /// 1-based line within the document (0 until filled by `parse`).
    pub line: usize,
    /// Byte range within `line`, if positioned by `parse`.
    pub span: Option<(usize, usize)>,
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
    pub members: Vec<Line<MemberLine>>,
    pub children: Vec<MemberGroup>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LayoutStatement {
    /// `A left of B above C` — N operands, N-1 directions.
    Placement {
        operands: Vec<Operand>,
        directions: Vec<Direction>,
    },
    /// `top of X aligned with top of Y`
    Alignment { left: Anchored, right: Anchored },
    /// A lone operand — meaningful when it carries `as`/`with` treatment.
    Standalone(Operand),
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Direction {
    LeftOf,
    RightOf,
    Above,
    Below,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Anchored {
    pub edge: Option<Edge>,
    pub operand: Operand,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
    Center,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Operand {
    pub ref_: OperandRef,
    pub axis: Option<Axis>,
    pub hints: Vec<Hint>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Axis {
    Row,
    Column,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OperandRef {
    Name(NameRef),
    InlineGroup { axis: Axis, items: Vec<Operand> },
    Paren(Box<Operand>),
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NameRef {
    Link { title: String, slug: String },
    Bare(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Hint {
    Shape(Shape),
    Margin(Margin),
    Flag(Flag),
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Shape {
    Frame,
    Box,
    Shrink,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Margin {
    No,
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Flag {
    Emphasized,
    Collapsed,
}

/// A parsed `[Title](./slug.md)` reference (unresolved slug stem).
#[derive(Debug, Clone, PartialEq)]
pub struct LinkRef {
    pub title: String,
    pub slug: String,
}

/// A flow edge's target: a bare local vertex label, or a cross-document link.
#[derive(Debug, Clone, PartialEq)]
pub enum FlowTargetRef {
    Local(String),
    Link(LinkRef),
}

/// One `transitions` bullet: `[on `t`] [when `g`|else] transitions to <target>
/// [carries <link>] [: `effect`]`.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowTransition {
    pub trigger: Option<String>,
    pub guard: Option<String>,
    pub is_else: bool,
    pub target: FlowTargetRef,
    pub carries: Option<LinkRef>,
    pub effect: Option<String>,
    /// 1-based line within the document (0 until filled by the block parser).
    pub line: usize,
}

/// One bullet under a flow node heading.
#[derive(Debug, Clone, PartialEq)]
pub enum FlowBullet {
    Transition(FlowTransition),
    Entry(String),
    Do(String),
    Exit(String),
    Refines(LinkRef),
    Partition(String),
}

/// One `###` node in a `## Nodes` section. Identity = heading text minus the
/// leading kind keyword (the link title for `object` nodes).
#[derive(Debug, Clone, PartialEq)]
pub struct FlowNodeSyntax {
    pub kind: crate::model::FlowNodeKind,
    pub identity: String,
    pub object_ref: Option<LinkRef>,
    pub bullets: Vec<Line<FlowBullet>>,
    pub notes: Vec<Line<String>>,
    pub line: usize,
}

/// The `## Nodes` section of a flow document: one directed graph.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowBlock {
    pub nodes: Vec<FlowNodeSyntax>,
    /// Non-heading content before the first `###` — preserved, never dropped.
    pub preamble_errors: Vec<ErrorNode>,
}

/// One `## Lifelines` bullet: `- [Title](./slug.md)[ alias]`.
#[derive(Debug, Clone, PartialEq)]
pub struct LifelineLine {
    pub link: LinkRef,
    pub alias: Option<String>,
    pub line: usize,
    pub span: Option<(usize, usize)>,
}

/// One message bullet: `- <sender> <verb> <receiver>[: `signature`]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMessage {
    pub from: String,
    pub verb: crate::model::MessageVerb,
    pub to: String,
    pub signature: Option<String>,
    pub line: usize,
}

/// One operand of an authored fragment (`- when `g`` / `- else`).
#[derive(Debug, Clone, PartialEq)]
pub struct SeqOperandSyntax {
    /// None = the `else` operand.
    pub guard: Option<String>,
    pub items: Vec<Line<SeqItemSyntax>>,
    pub line: usize,
}

/// One `## Messages` item: a message, or a fragment owning operands.
/// `errors` preserves misplaced lines authored directly inside the fragment
/// (outside any operand) so serialization stays lossless.
#[derive(Debug, Clone, PartialEq)]
pub enum SeqItemSyntax {
    Message(ParsedMessage),
    Fragment {
        kind: crate::model::FragmentKind,
        operands: Vec<SeqOperandSyntax>,
        errors: Vec<ErrorNode>,
        line: usize,
    },
}

/// The ordered `## Messages` section. Document order is time order.
#[derive(Debug, Clone, PartialEq)]
pub struct MessagesBlock {
    pub items: Vec<Line<SeqItemSyntax>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_wraps_parsed_and_error_nodes() {
        let item = LayoutItem {
            line: 12,
            stmt: LayoutStatement::Standalone(Operand {
                ref_: OperandRef::Name(NameRef::Bare("Orders".into())),
                axis: None,
                hints: vec![],
            }),
        };
        let good: Line<LayoutItem> = Line::Parsed(item);
        assert!(good.parsed().is_some());
        let bad: Line<LayoutItem> = Line::Error(ErrorNode {
            raw: "- nonsense".into(),
            line: 13,
            span: (0, 10),
            code: crate::diagnostic::DiagCode::MalformedLayout,
            message: "malformed layout statement".into(),
        });
        assert!(bad.parsed().is_none());
        let _s = Section::Layout(vec![good, bad]); // must typecheck
    }

    #[test]
    fn document_is_constructible() {
        let doc = Document {
            frontmatter: Frontmatter::default(),
            title: "Order".to_string(),
            sections: vec![Section::Relationships(vec![Line::Parsed(ParsedRel {
                kind: RelationshipKind::Composes,
                target_title: "OrderLine".to_string(),
                target_slug: "order-line".to_string(),
                name: None,
                from_end: RelEnd::default(),
                to_end: RelEnd::default(),
                line: 0,
                span: None,
            })])],
        };
        assert_eq!(doc.title, "Order");
        assert_eq!(doc.sections.len(), 1);
    }

    #[test]
    fn layout_statement_is_constructible() {
        let stmt = LayoutStatement::Placement {
            operands: vec![
                Operand {
                    ref_: OperandRef::Name(NameRef::Bare("Users".into())),
                    axis: None,
                    hints: vec![],
                },
                Operand {
                    ref_: OperandRef::Name(NameRef::Bare("Orders".into())),
                    axis: None,
                    hints: vec![],
                },
            ],
            directions: vec![Direction::LeftOf],
        };
        match stmt {
            LayoutStatement::Placement {
                operands,
                directions,
            } => {
                assert_eq!(operands.len(), 2);
                assert_eq!(directions, vec![Direction::LeftOf]);
            }
            _ => panic!("wrong variant"),
        }
    }
}
