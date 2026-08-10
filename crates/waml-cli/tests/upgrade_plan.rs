#[path = "../src/upgrade.rs"]
mod upgrade;

use upgrade::{plan_upgrade, DIAGRAM_TYPE_MIGRATION_ID, MIGRATIONS};

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
