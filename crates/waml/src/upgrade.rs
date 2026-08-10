use crate::{
    diagnostic::{DiagCode, Diagnostic, Severity},
    frontmatter::{
        inspect_frontmatter_string_scalar, parse_frontmatter_source,
        replace_frontmatter_string_scalar, FmValue, FrontmatterStringScalar,
    },
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

    fn authored_type(self) -> &'static str {
        match self {
            Self::Diagram => "Diagram",
            Self::Activity => "uml.Activity",
            Self::StateMachine => "uml.StateMachine",
            Self::Sequence => "uml.Sequence",
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

enum LegacyTypeInspection {
    NoLegacy,
    Legacy(LegacyDiagramType),
    Malformed {
        legacy: Option<LegacyDiagramType>,
        error: UpgradeInspectionError,
    },
}

fn inspect_document_legacy_type(path: &str, source: &str) -> LegacyTypeInspection {
    let tolerant_legacy =
        parse_frontmatter_source(source).and_then(|frontmatter| match frontmatter.get("type") {
            Some(FmValue::Str(authored_type)) => LegacyDiagramType::parse(authored_type),
            _ => None,
        });
    match inspect_frontmatter_string_scalar(source, "type") {
        Ok(FrontmatterStringScalar::String(authored_type)) => {
            LegacyDiagramType::parse(&authored_type)
                .map_or(LegacyTypeInspection::NoLegacy, LegacyTypeInspection::Legacy)
        }
        Ok(FrontmatterStringScalar::NoFrontmatter | FrontmatterStringScalar::NoScalar) => {
            LegacyTypeInspection::NoLegacy
        }
        Err(error) => LegacyTypeInspection::Malformed {
            legacy: tolerant_legacy,
            error: invalid_legacy_bundle(path, error.to_string()),
        },
    }
}

pub fn detect_legacy_diagram_types(source: &SourceBundle) -> Result<bool, UpgradeInspectionError> {
    let inspections = source
        .documents()
        .iter()
        .map(|document| inspect_document_legacy_type(document.path().as_str(), document.text()))
        .collect::<Vec<_>>();
    let legacy_detected = inspections.iter().any(|inspection| {
        matches!(
            inspection,
            LegacyTypeInspection::Legacy(_)
                | LegacyTypeInspection::Malformed {
                    legacy: Some(_),
                    ..
                }
        )
    });
    if !legacy_detected {
        return Ok(false);
    }
    if let Some(error) = inspections
        .into_iter()
        .find_map(|inspection| match inspection {
            LegacyTypeInspection::Malformed { error, .. } => Some(error),
            LegacyTypeInspection::NoLegacy | LegacyTypeInspection::Legacy(_) => None,
        })
    {
        return Err(error);
    }
    Ok(true)
}

pub fn inspect_legacy_diagram_types(
    source: &SourceBundle,
) -> Result<Vec<LegacyDiagramTypeUse>, UpgradeInspectionError> {
    let mut legacy_documents = Vec::new();
    for document in source.documents() {
        match inspect_document_legacy_type(document.path().as_str(), document.text()) {
            LegacyTypeInspection::NoLegacy => {}
            LegacyTypeInspection::Legacy(legacy) => {
                legacy_documents.push(LegacyDocument {
                    path: document.path().as_str().to_string(),
                    concept_id: crate::okf::id_of(document.path().as_str()),
                    legacy,
                });
            }
            LegacyTypeInspection::Malformed { error, .. } => return Err(error),
        }
    }
    legacy_documents.sort_by(|left, right| left.path.cmp(&right.path));

    let mut migrated_documents = Vec::with_capacity(source.len());
    for document in source.documents() {
        let path = document.path().as_str();
        let text = match legacy_documents.iter().find(|legacy| legacy.path == path) {
            Some(legacy) => replace_frontmatter_string_scalar(
                document.text(),
                "type",
                legacy.legacy.authored_type(),
                legacy.legacy.provisional_type(),
            )
            .map_err(|error| invalid_legacy_bundle(path, error.to_string()))?
            .ok_or_else(|| {
                invalid_legacy_bundle(path, "detected legacy type could not be rewritten")
            })?,
            None => document.text().to_string(),
        };
        migrated_documents.push((path.to_string(), text));
    }
    let failure_path = legacy_documents
        .first()
        .map(|document| document.path.as_str())
        .or_else(|| {
            source
                .documents()
                .first()
                .map(|document| document.path().as_str())
        })
        .unwrap_or("<bundle>");
    let migrated = SourceBundle::try_from_pairs(migrated_documents)
        .map_err(|error| invalid_legacy_bundle(failure_path, error.to_string()))?;
    let prepared = crate::analysis::prepare_candidate(migrated, None, 0)
        .map_err(|error| invalid_legacy_bundle(failure_path, error.to_string()))?;
    let diagnostics = prepared
        .uml()
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.severity == Severity::Error
                || (diagnostic.code == crate::diagnostic::DiagCode::UnresolvedTarget
                    && diagnostic.message.starts_with("unresolved UML member '"))
        })
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

fn invalid_legacy_bundle(
    path: impl Into<String>,
    cause: impl Into<String>,
) -> UpgradeInspectionError {
    UpgradeInspectionError::InvalidLegacyBundle(vec![Diagnostic::new(
        DiagCode::FrontmatterNotClean,
        format!("cannot inspect legacy diagram types: {}", cause.into()),
        path,
        1,
    )])
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
