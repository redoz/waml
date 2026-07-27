//! Deprecated mixed-domain adapter retained for DTO, CLI, and LSP callers.

use crate::edit::{EditContext, EditError};
use crate::source::SourceBundle;
use crate::{okf, uml};

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Okf(okf::Op),
    Uml(uml::Op),
}

#[doc(hidden)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Batch {
    steps: Vec<Step>,
}

impl Batch {
    pub fn new(steps: Vec<Step>) -> Self {
        Self { steps }
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }
}

fn directory(path: &str) -> Result<okf::DirectoryAddress, EditError> {
    let path = path.trim_matches('/');
    okf::DirectoryAddress::parse(if path.is_empty() {
        "/".to_string()
    } else {
        format!("/{path}")
    })
    .map_err(|error| EditError::at("directory", error.to_string()))
}

impl TryFrom<crate::ops::Op> for Step {
    type Error = EditError;

    fn try_from(op: crate::ops::Op) -> Result<Self, Self::Error> {
        use crate::ops::Op as Legacy;
        Ok(match op {
            Legacy::AttrAdd {
                node,
                name,
                ty_token,
                multiplicity,
                visibility,
            } => Step::Uml(uml::Op::AttributeAdd {
                node,
                name,
                ty_token,
                multiplicity,
                visibility,
            }),
            Legacy::AttrSet {
                node,
                name,
                ty_token,
                multiplicity,
                visibility,
                rename,
            } => Step::Uml(uml::Op::AttributeSet {
                node,
                name,
                ty_token,
                multiplicity,
                visibility,
                rename,
            }),
            Legacy::AttrRm { node, name } => Step::Uml(uml::Op::AttributeRemove { node, name }),
            Legacy::ValueAdd { node, literal } => Step::Uml(uml::Op::ValueAdd { node, literal }),
            Legacy::ValueRm { node, literal } => Step::Uml(uml::Op::ValueRemove { node, literal }),
            Legacy::RelAdd {
                source,
                kind,
                target,
                name,
                ends,
            } => Step::Uml(uml::Op::RelationshipAdd {
                source,
                kind,
                target,
                name,
                ends,
            }),
            Legacy::RelSet {
                selector,
                ends,
                name,
            } => Step::Uml(uml::Op::RelationshipSet {
                selector,
                ends,
                name,
            }),
            Legacy::RelRm { selector } => Step::Uml(uml::Op::RelationshipRemove { selector }),
            Legacy::NodeNew {
                slug,
                dir,
                ty,
                title,
                stereotype,
                description,
                abstract_,
            } => Step::Uml(uml::Op::ClassifierNew {
                slug,
                directory: directory(&dir)?,
                ty,
                title,
                stereotype,
                description,
                abstract_,
            }),
            Legacy::NodeSet {
                slug,
                title,
                description,
                stereotype,
                abstract_,
                ty,
            } => Step::Uml(uml::Op::ClassifierSet {
                id: slug,
                title,
                description,
                stereotype,
                abstract_,
                ty,
            }),
            Legacy::NodeRm { slug, cascade } => {
                Step::Uml(uml::Op::ClassifierRemove { id: slug, cascade })
            }
            Legacy::NodeRename { from, to } => Step::Uml(uml::Op::ClassifierRename { from, to }),
            Legacy::PkgMove { slug, to_dir } => Step::Okf(okf::Op::ConceptMove {
                id: slug,
                to_directory: directory(&to_dir)?,
            }),
            Legacy::PkgRename { from, to } => Step::Okf(okf::Op::DirectoryRename {
                directory: directory(&from)?,
                name: to.rsplit('/').next().unwrap_or(&to).to_string(),
            }),
            Legacy::PkgDelete { path, cascade } => Step::Okf(okf::Op::DirectoryDelete {
                directory: directory(&path)?,
                cascade,
            }),
            Legacy::PkgReorder { path, order } => Step::Okf(okf::Op::IndexReorder {
                directory: directory(&path)?,
                order,
            }),
            Legacy::PkgSort { path } => Step::Okf(okf::Op::IndexSort {
                directory: directory(&path)?,
            }),
            Legacy::PkgRetitle { path, title } => Step::Okf(okf::Op::IndexRetitle {
                directory: directory(&path)?,
                title,
            }),
            Legacy::PkgInsert {
                parent_path,
                name,
                docs,
            } => Step::Okf(okf::Op::BundleImport {
                parent: directory(&parent_path)?,
                name,
                bundle: SourceBundle::try_from_pairs(docs)
                    .map_err(|error| EditError::at("pkg.insert", error.to_string()))?,
            }),
            Legacy::DiagramSet {
                key,
                title,
                description,
                clear_description,
                display,
            } => Step::Uml(uml::Op::DiagramSet {
                key,
                title,
                description,
                clear_description,
                display,
            }),
            Legacy::PlaceSet {
                diagram,
                subject_title,
                subject_slug,
                reference_title,
                reference_slug,
                directions,
            } => Step::Uml(uml::Op::PlacementSet {
                diagram,
                subject_title,
                subject_slug,
                reference_title,
                reference_slug,
                directions,
            }),
            Legacy::PlaceRm {
                diagram,
                subject_slug,
                reference_slug,
            } => Step::Uml(uml::Op::PlacementRemove {
                diagram,
                subject_slug,
                reference_slug,
            }),
        })
    }
}

pub fn apply(source: &SourceBundle, batch: &Batch) -> Result<SourceBundle, EditError> {
    let mut candidate = source.clone();
    for (index, step) in batch.steps().iter().enumerate() {
        let result = match step {
            Step::Okf(op) => crate::okf::ops::lower_one(&mut candidate, op),
            Step::Uml(op) => crate::uml::ops::lower_one(&mut candidate, op),
        };
        result.map_err(|mut error| {
            error.index = index;
            error
        })?;
    }
    Ok(candidate)
}

impl crate::edit::sealed::Sealed for Batch {}

impl crate::edit::EditBatch for Batch {
    fn lower(&self, context: EditContext<'_>) -> Result<SourceBundle, EditError> {
        apply(context.source, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_legacy_directories_return_errors_instead_of_panicking() {
        let source = SourceBundle::default();
        let result = crate::ops::apply_source(
            &source,
            &[crate::ops::Op::PkgRetitle {
                path: "../escape".into(),
                title: "Nope".into(),
            }],
        );
        assert!(result.is_err());
    }

    #[test]
    fn malformed_legacy_import_bundle_returns_error() {
        let source = SourceBundle::default();
        let result = crate::ops::apply_source(
            &source,
            &[crate::ops::Op::PkgInsert {
                parent_path: String::new(),
                name: "x".into(),
                docs: vec![
                    ("a.md".into(), "# A".into()),
                    ("a.md".into(), "# Duplicate".into()),
                ],
            }],
        );
        assert!(result.is_err());
    }
}
