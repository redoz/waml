use std::sync::Arc;

use waml::{analysis::analyze_okf, source::SourceBundle};

fn source(pairs: &[(&str, &str)]) -> SourceBundle {
    SourceBundle::try_from_pairs(
        pairs
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned())),
    )
    .unwrap()
}

#[test]
fn catalog_reuses_identity_for_unchanged_documents() {
    let source = source(&[("order.md", "---\ntype: class\n---\n# Order\n")]);
    let first = analyze_okf(&source, None, 1).unwrap();
    let second = analyze_okf(&source, Some(&first), 2).unwrap();

    let path = source.documents()[0].path();
    let id = first.catalog.id_for_path(path).unwrap();
    assert_eq!(first.catalog.session_revision(), 1);
    assert_eq!(second.catalog.session_revision(), 2);
    assert!(Arc::ptr_eq(
        first.catalog.document(id).unwrap(),
        second.catalog.document(id).unwrap(),
    ));
    assert!(Arc::ptr_eq(
        first.shell.document(id).unwrap(),
        second.shell.document(id).unwrap(),
    ));
}

#[test]
fn catalog_tracks_changes_and_never_reuses_removed_paths() {
    let first_source = source(&[("one.md", "---\ntype: note\n---\n# One\n")]);
    let first = analyze_okf(&first_source, None, 1).unwrap();
    let one = first
        .catalog
        .id_for_path(first_source.documents()[0].path())
        .unwrap();

    let changed_source = source(&[("one.md", "---\ntype: note\n---\n# Changed\n")]);
    let changed = analyze_okf(&changed_source, Some(&first), 2).unwrap();
    let changed_one = changed
        .catalog
        .id_for_path(changed_source.documents()[0].path())
        .unwrap();
    assert_eq!(one, changed_one);
    assert_ne!(
        changed.catalog.document(one).unwrap().revision(),
        first.catalog.document(one).unwrap().revision()
    );

    let added_source = source(&[
        ("one.md", "---\ntype: note\n---\n# Changed\n"),
        ("two.md", "# Two\n"),
    ]);
    let added = analyze_okf(&added_source, Some(&changed), 3).unwrap();
    let two = added
        .catalog
        .id_for_path(added_source.documents()[1].path())
        .unwrap();
    assert!(two > one);

    let renamed_source = source(&[("renamed.md", "---\ntype: note\n---\n# Changed\n")]);
    let renamed = analyze_okf(&renamed_source, Some(&added), 4).unwrap();
    let renamed_id = renamed
        .catalog
        .id_for_path(renamed_source.documents()[0].path())
        .unwrap();
    assert_ne!(renamed_id, one);
    assert!(renamed_id > two);
    assert!(renamed.catalog.document(one).is_none());
}
