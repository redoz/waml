use crate::{
    diagnostic::{Diagnostic, Severity},
    frontmatter::{parse_frontmatter_source, replace_frontmatter_string_scalar},
    model::{DiagramKind, ElementType, UmlMetaclass},
    source::SourceBundle,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyDiagramType {
    Diagram,
    Activity,
    StateMachine,
    Sequence,
}

impl LegacyDiagramType {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "Diagram" => Some(Self::Diagram),
            "uml.Activity" => Some(Self::Activity),
            "uml.StateMachine" => Some(Self::StateMachine),
            "uml.Sequence" => Some(Self::Sequence),
            _ => None,
        }
    }

    fn provisional_type(self) -> &'static str {
        match self {
            Self::Diagram => "uml.ClassDiagram",
            Self::Activity => "uml.ActivityDiagram",
            Self::StateMachine => "uml.StateMachineDiagram",
            Self::Sequence => "uml.SequenceDiagram",
        }
    }

    fn direct_replacement(self) -> Option<DiagramKind> {
        match self {
            Self::Diagram => None,
            Self::Activity => Some(DiagramKind::Activity),
            Self::StateMachine => Some(DiagramKind::StateMachine),
            Self::Sequence => Some(DiagramKind::Sequence),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyDiagramTypeUse {
    pub path: String,
    pub legacy: LegacyDiagramType,
    pub replacement: DiagramKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpgradeInspectionError {
    AmbiguousLegacyDiagram {
        path: String,
        incompatible_members: Vec<String>,
    },
    InvalidLegacyBundle(Vec<Diagnostic>),
}

impl Eq for UpgradeInspectionError {}

struct LegacyDocument {
    path: String,
    concept_id: String,
    legacy: LegacyDiagramType,
}

pub fn inspect_legacy_diagram_types(
    source: &SourceBundle,
) -> Result<Vec<LegacyDiagramTypeUse>, UpgradeInspectionError> {
    let mut legacy_documents = source
        .documents()
        .iter()
        .filter_map(|document| {
            let legacy = parse_frontmatter_source(document.text())?
                .get_str("type")
                .and_then(LegacyDiagramType::parse)?;
            Some(LegacyDocument {
                path: document.path().as_str().to_string(),
                concept_id: crate::okf::id_of(document.path().as_str()),
                legacy,
            })
        })
        .collect::<Vec<_>>();
    legacy_documents.sort_by(|left, right| left.path.cmp(&right.path));

    let migrated = SourceBundle::try_from_pairs(source.documents().iter().map(|document| {
        let text = legacy_documents
            .iter()
            .find(|legacy| legacy.path == document.path().as_str())
            .and_then(|legacy| {
                replace_frontmatter_string_scalar(
                    document.text(),
                    "type",
                    match legacy.legacy {
                        LegacyDiagramType::Diagram => "Diagram",
                        LegacyDiagramType::Activity => "uml.Activity",
                        LegacyDiagramType::StateMachine => "uml.StateMachine",
                        LegacyDiagramType::Sequence => "uml.Sequence",
                    },
                    legacy.legacy.provisional_type(),
                )
                .ok()
                .flatten()
            })
            .unwrap_or_else(|| document.text().to_string());
        (document.path().as_str().to_string(), text)
    }))
    .map_err(|_| UpgradeInspectionError::InvalidLegacyBundle(Vec::new()))?;
    let prepared = crate::analysis::prepare_candidate(migrated, None, 0)
        .map_err(|_| UpgradeInspectionError::InvalidLegacyBundle(Vec::new()))?;
    let diagnostics = prepared
        .uml()
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .cloned()
        .collect::<Vec<_>>();
    if !diagnostics.is_empty() {
        return Err(UpgradeInspectionError::InvalidLegacyBundle(diagnostics));
    }

    let mut uses = Vec::with_capacity(legacy_documents.len());
    for legacy_document in legacy_documents {
        let replacement = match legacy_document.legacy.direct_replacement() {
            Some(replacement) => replacement,
            None => classify_legacy_diagram(&legacy_document, &prepared)?,
        };
        uses.push(LegacyDiagramTypeUse {
            path: legacy_document.path,
            legacy: legacy_document.legacy,
            replacement,
        });
    }
    Ok(uses)
}

fn classify_legacy_diagram(
    document: &LegacyDocument,
    prepared: &crate::analysis::PreparedCandidate,
) -> Result<DiagramKind, UpgradeInspectionError> {
    let mut has_use_case = false;
    let mut incompatible = BTreeSet::new();
    for member in prepared
        .uml()
        .resolved_diagram_member_keys(&document.concept_id)
    {
        let Some(concept) = prepared.okf().bundle.concept(&member) else {
            continue;
        };
        match ElementType::parse(&concept.ty) {
            ElementType::Uml(UmlMetaclass::UseCase) => has_use_case = true,
            ElementType::Uml(
                UmlMetaclass::Class
                | UmlMetaclass::Interface
                | UmlMetaclass::Enum
                | UmlMetaclass::DataType,
            )
            | ElementType::Diagram(
                DiagramKind::Activity | DiagramKind::StateMachine | DiagramKind::Sequence,
            ) => {
                incompatible.insert(member);
            }
            _ => {}
        }
    }
    if has_use_case && !incompatible.is_empty() {
        return Err(UpgradeInspectionError::AmbiguousLegacyDiagram {
            path: document.path.clone(),
            incompatible_members: incompatible.into_iter().collect(),
        });
    }
    Ok(if has_use_case {
        DiagramKind::UseCase
    } else {
        DiagramKind::Class
    })
}
