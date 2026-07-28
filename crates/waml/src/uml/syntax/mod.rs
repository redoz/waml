mod ast;
mod kind;
pub mod parser;
pub use ast::{
    AttributeSyntax, InlineInstanceSyntax, MemberGroupSyntax, MemberSyntax, MultiplicitySyntax,
    RelationshipSyntax, SlotSyntax, SlotValueKind, TypeReferenceSyntax, ValueSyntax,
};
pub use kind::{UmlSyntaxDiagnosticCode, UmlSyntaxKind};
#[derive(Debug)]
pub struct UmlLanguage;
impl waml_syntax::SyntaxLanguage for UmlLanguage {
    type Kind = UmlSyntaxKind;
    type DiagnosticCode = UmlSyntaxDiagnosticCode;
}
