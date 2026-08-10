#[path = "../src/upgrade.rs"]
mod upgrade;

use upgrade::{
    plan_upgrade, plan_upgrade_with_migrations, Migration, UpgradeError, DIAGRAM_TYPE_MIGRATION_ID,
    MIGRATIONS,
};
use waml::source::SourceBundle;

fn legacy_documents() -> Vec<(String, String)> {
    vec![
        (
            "class.md".into(),
            "---\ntype: Diagram\n---\n# Class diagram\n".into(),
        ),
        (
            "activity.md".into(),
            "---\ntype: uml.Activity\n---\n# Activity diagram\n".into(),
        ),
    ]
}

fn always_detect(_: &SourceBundle) -> Result<bool, UpgradeError> {
    Ok(true)
}

fn replace_text(source: &SourceBundle, from: &str, to: &str) -> Result<SourceBundle, UpgradeError> {
    SourceBundle::try_from_pairs(source.to_pairs().into_iter().map(|(path, text)| {
        let text = text.replace(from, to);
        (path, text)
    }))
    .map_err(|error| UpgradeError::InvalidSource(error.to_string()))
}

fn make_temporarily_invalid(source: &SourceBundle) -> Result<SourceBundle, UpgradeError> {
    replace_text(
        source,
        "---\n# Stable",
        "# Missing frontmatter fence\n# Stable",
    )
}

fn repair_temporary_error(source: &SourceBundle) -> Result<SourceBundle, UpgradeError> {
    replace_text(
        source,
        "# Missing frontmatter fence\n# Stable",
        "---\n# Stable",
    )
}

fn first_change(source: &SourceBundle) -> Result<SourceBundle, UpgradeError> {
    replace_text(source, "# Stable", "# First")
}

fn second_change(source: &SourceBundle) -> Result<SourceBundle, UpgradeError> {
    replace_text(source, "# First", "# Second")
}

#[test]
fn registry_has_the_stable_canonical_diagram_migration_in_order() {
    assert_eq!(MIGRATIONS.len(), 1);
    assert_eq!(MIGRATIONS[0].id, DIAGRAM_TYPE_MIGRATION_ID);
    assert_eq!(MIGRATIONS[0].id, "canonical-uml-diagram-types");
    assert_eq!(
        MIGRATIONS[0].description,
        "Use canonical UML diagram document types"
    );
}

#[test]
fn plan_reports_each_changed_file_once() {
    let plan = plan_upgrade(&legacy_documents()).expect("legacy documents upgrade");

    assert_eq!(
        plan.files,
        vec![
            (
                "class.md".into(),
                "---\ntype: uml.ClassDiagram\n---\n# Class diagram\n".into(),
            ),
            (
                "activity.md".into(),
                "---\ntype: uml.ActivityDiagram\n---\n# Activity diagram\n".into(),
            ),
        ]
    );
    assert_eq!(
        plan.applied
            .iter()
            .map(|applied| (applied.path.as_str(), applied.id, applied.description))
            .collect::<Vec<_>>(),
        vec![
            (
                "activity.md",
                DIAGRAM_TYPE_MIGRATION_ID,
                "Use canonical UML diagram document types",
            ),
            (
                "class.md",
                DIAGRAM_TYPE_MIGRATION_ID,
                "Use canonical UML diagram document types",
            ),
        ]
    );
}

#[test]
fn plan_rejects_a_full_candidate_with_errors_after_migration() {
    let files = vec![
        (
            "legacy.md".into(),
            "---\ntype: Diagram\n---\n# Legacy\n".into(),
        ),
        (
            "broken.md".into(),
            "---\ntype: uml.Class\n# The frontmatter fence is missing.\n".into(),
        ),
    ];

    assert!(plan_upgrade(&files).is_err());
}

#[test]
fn second_plan_keeps_upgraded_bytes_and_has_no_applied_reports() {
    let first = plan_upgrade(&legacy_documents()).expect("first upgrade");
    let second = plan_upgrade(&first.files).expect("second upgrade");

    assert_eq!(second.files, first.files);
    assert!(second.applied.is_empty());
}

#[test]
fn later_migration_repairs_temporary_error_before_full_validation() {
    let migrations = [
        Migration {
            id: "make-invalid",
            description: "Make a temporary invalid candidate",
            detect: always_detect,
            transform: make_temporarily_invalid,
        },
        Migration {
            id: MIGRATIONS[0].id,
            description: MIGRATIONS[0].description,
            detect: MIGRATIONS[0].detect,
            transform: MIGRATIONS[0].transform,
        },
        Migration {
            id: "repair-invalid",
            description: "Repair the temporary invalid candidate",
            detect: always_detect,
            transform: repair_temporary_error,
        },
    ];
    let files = vec![(
        "stable.md".into(),
        "---\ntype: uml.Class\n---\n# Stable\n".into(),
    )];

    let plan = plan_upgrade_with_migrations(&files, &migrations)
        .expect("final repaired candidate must be authoritative");

    assert_eq!(plan.files, files);
}

#[test]
fn two_migrations_touching_one_path_keep_first_report_metadata() {
    let migrations = [
        Migration {
            id: "first-change",
            description: "Apply the first change",
            detect: always_detect,
            transform: first_change,
        },
        Migration {
            id: "second-change",
            description: "Apply the second change",
            detect: always_detect,
            transform: second_change,
        },
    ];
    let files = vec![(
        "stable.md".into(),
        "---\ntype: uml.Class\n---\n# Stable\n".into(),
    )];

    let plan = plan_upgrade_with_migrations(&files, &migrations).expect("ordered changes");

    assert_eq!(
        plan.files,
        vec![(
            "stable.md".into(),
            "---\ntype: uml.Class\n---\n# Second\n".into(),
        )]
    );
    assert_eq!(plan.applied.len(), 1);
    assert_eq!(plan.applied[0].path, "stable.md");
    assert_eq!(plan.applied[0].id, "first-change");
    assert_eq!(plan.applied[0].description, "Apply the first change");
}
