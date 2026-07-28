mod ast;
mod kind;
pub mod parser;
pub use ast::{AttributeSyntax, MultiplicitySyntax, TypeReferenceSyntax};
pub use kind::{UmlSyntaxDiagnosticCode, UmlSyntaxKind};
#[derive(Debug)]
pub struct UmlLanguage;
impl waml_syntax::SyntaxLanguage for UmlLanguage {
    type Kind = UmlSyntaxKind;
    type DiagnosticCode = UmlSyntaxDiagnosticCode;
}
