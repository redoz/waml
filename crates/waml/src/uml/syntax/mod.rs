use std::sync::Arc;

use waml_syntax::{MarkdownStructureMap, SourceText, SyntaxTree};

mod ast;
mod kind;
mod parser;

pub(in crate::uml) fn parse_full(
    text: SourceText,
    structure: &MarkdownStructureMap,
) -> Arc<SyntaxTree<UmlLanguage>> {
    parser::parse(text, structure)
}

pub use ast::{
    AnchoredSyntax, AttributeSyntax, AxisSyntax, DiagramMembersSyntax, DirectionClauseSyntax,
    EdgeSyntax, FlagSyntax, FlowBlockSyntax, FlowInternalSyntax, FlowNodeSyntax,
    FlowTransitionSyntax, HintClauseSyntax, HintSyntax, InlineInstanceSyntax,
    LayoutAlignmentSyntax, LayoutAtomSyntax, LayoutPlacementSyntax, LayoutSectionSyntax,
    LayoutStandaloneSyntax, LayoutStatementSyntax, LifelineSyntax, MarginSyntax, MemberGroupSyntax,
    MemberLineSyntax, MemberSyntax, MessageSyntax, MessagesBlockSyntax, MultiplicitySyntax,
    NameRefSyntax, OperandRefSyntax, OperandSyntax, RelationshipEndSyntax, RelationshipSyntax,
    SequenceFragmentSyntax, SequenceOperandSyntax, ShapeSyntax, SlotSyntax, SlotValueKind,
    TypeReferenceSyntax, ValueSyntax,
};
pub use kind::{UmlSyntaxDiagnosticCode, UmlSyntaxKind};
#[derive(Debug)]
pub struct UmlLanguage;
impl waml_syntax::SyntaxLanguage for UmlLanguage {
    type Kind = UmlSyntaxKind;
    type DiagnosticCode = UmlSyntaxDiagnosticCode;
}
