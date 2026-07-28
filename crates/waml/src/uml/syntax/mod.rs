mod ast;
mod kind;
pub mod parser;
pub use ast::{
    AnchoredSyntax, AttributeSyntax, AxisSyntax, DiagramMembersSyntax, DirectionClauseSyntax,
    EdgeSyntax, FlagSyntax, HintClauseSyntax, HintSyntax, InlineInstanceSyntax,
    LayoutAlignmentSyntax, LayoutAtomSyntax, LayoutPlacementSyntax, LayoutSectionSyntax,
    LayoutStandaloneSyntax, LayoutStatementSyntax, MarginSyntax, MemberGroupSyntax,
    MemberLineSyntax, MemberSyntax, MultiplicitySyntax, NameRefSyntax, OperandRefSyntax,
    OperandSyntax, RelationshipEndSyntax, RelationshipSyntax, ShapeSyntax, SlotSyntax,
    SlotValueKind, TypeReferenceSyntax, ValueSyntax,
};
pub use kind::{UmlSyntaxDiagnosticCode, UmlSyntaxKind};
#[derive(Debug)]
pub struct UmlLanguage;
impl waml_syntax::SyntaxLanguage for UmlLanguage {
    type Kind = UmlSyntaxKind;
    type DiagnosticCode = UmlSyntaxDiagnosticCode;
}
