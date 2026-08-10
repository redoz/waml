use std::collections::{BTreeMap, BTreeSet};

use waml::{
    analysis::prepare_candidate,
    diagnostic::{Diagnostic, Severity},
    frontmatter::{replace_frontmatter_string_scalar, FrontmatterRewriteError},
    source::SourceBundle,
    upgrade::{
        detect_legacy_diagram_types, inspect_legacy_diagram_types, LegacyDiagramType,
        UpgradeInspectionError,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub enum UpgradeError {
    InvalidSource(String),
    Inspection(UpgradeInspectionError),
    Rewrite {
        path: String,
        error: FrontmatterRewriteError,
    },
    Preparation(String),
    InvalidCandidate(Vec<Diagnostic>),
}

pub struct Migration {
    pub id: &'static str,
    pub description: &'static str,
    pub detect: fn(&SourceBundle) -> Result<bool, UpgradeError>,
    pub transform: fn(&SourceBundle) -> Result<SourceBundle, UpgradeError>,
}

pub const DIAGRAM_TYPE_MIGRATION_ID: &str = "canonical-uml-diagram-types";
pub static MIGRATIONS: &[Migration] = &[Migration {
    id: DIAGRAM_TYPE_MIGRATION_ID,
    description: "Use canonical UML diagram document types",
    detect: detect_canonical_uml_diagram_types,
    transform: transform_canonical_uml_diagram_types,
}];

pub struct AppliedMigration {
    pub path: String,
    pub id: &'static str,
    pub description: &'static str,
}

pub struct UpgradePlan {
    pub files: Vec<(String, String)>,
    /// One report per path whose final bytes differ from its original bytes.
    /// The first migration that changes the path supplies its stable ID and
    /// description.
    pub applied: Vec<AppliedMigration>,
}

pub fn plan_upgrade(files: &[(String, String)]) -> Result<UpgradePlan, UpgradeError> {
    plan_upgrade_with_migrations(files, MIGRATIONS)
}

pub(crate) fn plan_upgrade_with_migrations(
    files: &[(String, String)],
    migrations: &[Migration],
) -> Result<UpgradePlan, UpgradeError> {
    let mut candidate = SourceBundle::try_from_pairs(files.iter().cloned())
        .map_err(|error| UpgradeError::InvalidSource(error.to_string()))?;
    let original = candidate.to_pairs().into_iter().collect::<BTreeMap<_, _>>();
    let mut applied = BTreeMap::new();
    let mut migration_ran = false;

    for migration in migrations {
        if !(migration.detect)(&candidate)? {
            continue;
        }
        migration_ran = true;
        let before = candidate.to_pairs().into_iter().collect::<BTreeMap<_, _>>();
        candidate = (migration.transform)(&candidate)?;
        let after = candidate.to_pairs().into_iter().collect::<BTreeMap<_, _>>();
        let paths = before.keys().chain(after.keys()).collect::<BTreeSet<_>>();
        for path in paths {
            if before.get(path) != after.get(path) {
                applied
                    .entry(path.clone())
                    .or_insert_with(|| AppliedMigration {
                        path: path.clone(),
                        id: migration.id,
                        description: migration.description,
                    });
            }
        }
    }

    if !migration_ran {
        return Ok(UpgradePlan {
            files: candidate.to_pairs(),
            applied: Vec::new(),
        });
    }

    let prepared = prepare_candidate(candidate, None, 0)
        .map_err(|error| UpgradeError::Preparation(error.to_string()))?;
    let diagnostics = prepared
        .uml()
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .cloned()
        .collect::<Vec<_>>();
    if !diagnostics.is_empty() {
        return Err(UpgradeError::InvalidCandidate(diagnostics));
    }
    let files = prepared.source().to_pairs();
    let final_files = files.iter().cloned().collect::<BTreeMap<_, _>>();
    applied.retain(|path, _| original.get(path) != final_files.get(path));

    Ok(UpgradePlan {
        files,
        applied: applied.into_values().collect(),
    })
}

fn detect_canonical_uml_diagram_types(source: &SourceBundle) -> Result<bool, UpgradeError> {
    detect_legacy_diagram_types(source).map_err(UpgradeError::Inspection)
}

fn transform_canonical_uml_diagram_types(
    source: &SourceBundle,
) -> Result<SourceBundle, UpgradeError> {
    let uses = inspect_legacy_diagram_types(source).map_err(UpgradeError::Inspection)?;
    let replacements = uses
        .into_iter()
        .map(|legacy| {
            (
                legacy.path,
                (legacy_type_name(legacy.legacy), legacy.replacement.as_str()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut files = Vec::with_capacity(source.len());

    for document in source.documents() {
        let path = document.path().as_str();
        let text = match replacements.get(path) {
            Some((expected, replacement)) => {
                replace_frontmatter_string_scalar(document.text(), "type", expected, replacement)
                    .map_err(|error| UpgradeError::Rewrite {
                        path: path.to_string(),
                        error,
                    })?
                    .ok_or_else(|| UpgradeError::Rewrite {
                        path: path.to_string(),
                        error: FrontmatterRewriteError::InvalidFrontmatter,
                    })?
            }
            None => document.text().to_string(),
        };
        files.push((path.to_string(), text));
    }

    SourceBundle::try_from_pairs(files)
        .map_err(|error| UpgradeError::InvalidSource(error.to_string()))
}

fn legacy_type_name(legacy: LegacyDiagramType) -> &'static str {
    match legacy {
        LegacyDiagramType::Diagram => "Diagram",
        LegacyDiagramType::Activity => "uml.Activity",
        LegacyDiagramType::StateMachine => "uml.StateMachine",
        LegacyDiagramType::Sequence => "uml.Sequence",
    }
}
