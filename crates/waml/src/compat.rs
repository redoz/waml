//! Deprecated legacy `Op` → `Step` bridge retained for `waml::ops` callers.

use crate::edit::EditError;
use crate::source::SourceBundle;
use crate::{okf, uml};

pub use crate::edit::{apply, Batch, Step};

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
                selector: uml::RelationshipSelector::try_from(selector).map_err(|_| {
                    EditError::at(
                        "rel.set",
                        "relationship operation requires a relationship selector",
                    )
                })?,
                ends,
                name,
            }),
            Legacy::RelRm { selector } => Step::Uml(uml::Op::RelationshipRemove {
                selector: uml::RelationshipSelector::try_from(selector).map_err(|_| {
                    EditError::at(
                        "rel.rm",
                        "relationship operation requires a relationship selector",
                    )
                })?,
            }),
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
            Legacy::PkgRename { from, to } => {
                let from_parent = from.rsplit_once('/').map_or("", |(parent, _)| parent);
                let to_parent = to.rsplit_once('/').map_or("", |(parent, _)| parent);
                if from_parent != to_parent {
                    Step::Okf(okf::Op::DirectoryMove {
                        directory: directory(&from)?,
                        to_parent: directory(to_parent)?,
                        name: Some(to.rsplit('/').next().unwrap_or(&to).to_string()),
                    })
                } else {
                    Step::Okf(okf::Op::DirectoryRename {
                        directory: directory(&from)?,
                        name: to.rsplit('/').next().unwrap_or(&to).to_string(),
                    })
                }
            }
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

pub(crate) fn step_from_legacy(op: crate::ops::Op) -> Result<Step, EditError> {
    Step::try_from(op)
}
