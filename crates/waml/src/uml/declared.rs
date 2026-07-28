use super::syntax::{AttributeSyntax, UmlLanguage};
use crate::{
    model::{TypeRef, Visibility},
    multiplicity::Multiplicity,
};
use std::{collections::BTreeMap, sync::Arc};
use waml_syntax::{SyntaxLanguage, SyntaxNode};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedSyntax {
    ColonToken,
    TypeReference,
    ValidMultiplicity,
    LinkTarget,
    RelationshipTarget,
    LayoutOperand,
    FlowTarget,
    MessageTarget,
}
pub enum DeclaredField<L: SyntaxLanguage, T> {
    Absent,
    Valid {
        value: T,
        syntax: SyntaxNode<L>,
    },
    Incomplete {
        syntax: SyntaxNode<L>,
        expected: ExpectedSyntax,
    },
    Invalid {
        syntax: SyntaxNode<L>,
        diagnostics: Arc<[crate::diagnostic::DiagCode]>,
    },
}
pub struct DeclaredAttribute {
    pub syntax: AttributeSyntax,
    pub visibility: DeclaredField<UmlLanguage, Visibility>,
    pub name: DeclaredField<UmlLanguage, String>,
    pub ty: DeclaredField<UmlLanguage, TypeRef>,
    pub multiplicity: DeclaredField<UmlLanguage, Multiplicity>,
}
pub struct DeclaredConcept {
    pub concept_id: String,
    pub attributes: Arc<[DeclaredAttribute]>,
}
#[derive(Default)]
pub struct DeclaredBundle {
    pub(crate) concepts: BTreeMap<String, DeclaredConcept>,
}
impl DeclaredBundle {
    pub fn concept(&self, id: &str) -> Option<&DeclaredConcept> {
        self.concepts.get(id)
    }
    pub fn concepts(&self) -> impl Iterator<Item = &DeclaredConcept> {
        self.concepts.values()
    }
}

impl DeclaredAttribute {
    /// Produce the semantic attribute only when every required authored slot is
    /// valid and each optional slot is absent or valid.
    pub(crate) fn validated(&self) -> Option<crate::model::Attribute> {
        if self.syntax.colon_token().flags().is_missing() {
            return None;
        }
        let DeclaredField::Valid { value: name, .. } = &self.name else {
            return None;
        };
        let DeclaredField::Valid { value: ty, .. } = &self.ty else {
            return None;
        };
        let visibility = match &self.visibility {
            DeclaredField::Absent => None,
            DeclaredField::Valid { value, .. } => Some(*value),
            DeclaredField::Incomplete { .. } | DeclaredField::Invalid { .. } => return None,
        };
        let multiplicity = match &self.multiplicity {
            DeclaredField::Absent => None,
            DeclaredField::Valid { value, .. } => Some(value.clone()),
            DeclaredField::Incomplete { .. } | DeclaredField::Invalid { .. } => return None,
        };
        Some(crate::model::Attribute {
            name: name.clone(),
            ty: ty.clone(),
            multiplicity,
            visibility,
            description: None,
        })
    }
}
