use std::sync::Arc;

use waml::{
    analysis::{prepare_candidate, PreviousAnalyses},
    source::SourceBundle,
};

const DRIVER: &str =
    "---\ntype: uml.Class\ntitle: Driver\n---\n# Driver\n\n## Attributes\n- id: String {1}\n";

#[test]
fn unrelated_edit_reuses_the_island_tree() {
    let source =
        SourceBundle::try_from_pairs([("driver.md".to_owned(), DRIVER.to_owned())]).unwrap();
    let first = prepare_candidate(source, None, 1).unwrap();
    let id = first
        .okf()
        .catalog
        .id_for_path(first.source().documents()[0].path())
        .unwrap();
    let first_islands = first.uml().island_syntax.document(id).unwrap().clone();

    // Edit the title, which lives outside the Attributes island's source
    // range, so the island's syntax tree should be reused by identity.
    let edited = "---\ntype: uml.Class\ntitle: Driver Two\n---\n# Driver\n\n## Attributes\n- id: String {1}\n";
    let edited_source =
        SourceBundle::try_from_pairs([("driver.md".to_owned(), edited.to_owned())]).unwrap();
    let second = prepare_candidate(
        edited_source,
        Some(PreviousAnalyses {
            okf: first.okf(),
            uml: first.uml(),
        }),
        2,
    )
    .unwrap();
    let second_islands = second.uml().island_syntax.document(id).unwrap().clone();

    assert_eq!(first_islands.len(), 1);
    assert_eq!(second_islands.len(), 1);
    let first_tree = first_islands.values().next().unwrap();
    let second_tree = second_islands.values().next().unwrap();
    assert!(Arc::ptr_eq(first_tree.syntax(), second_tree.syntax()));
}

#[test]
fn edit_inside_the_island_reparses_it() {
    let source =
        SourceBundle::try_from_pairs([("driver.md".to_owned(), DRIVER.to_owned())]).unwrap();
    let first = prepare_candidate(source, None, 1).unwrap();
    let id = first
        .okf()
        .catalog
        .id_for_path(first.source().documents()[0].path())
        .unwrap();
    let first_islands = first.uml().island_syntax.document(id).unwrap().clone();

    let edited =
        "---\ntype: uml.Class\ntitle: Driver\n---\n# Driver\n\n## Attributes\n- name: String {1}\n";
    let edited_source =
        SourceBundle::try_from_pairs([("driver.md".to_owned(), edited.to_owned())]).unwrap();
    let second = prepare_candidate(
        edited_source,
        Some(PreviousAnalyses {
            okf: first.okf(),
            uml: first.uml(),
        }),
        2,
    )
    .unwrap();
    let second_islands = second.uml().island_syntax.document(id).unwrap().clone();

    let first_tree = first_islands.values().next().unwrap();
    let second_tree = second_islands.values().next().unwrap();
    assert!(!Arc::ptr_eq(first_tree.syntax(), second_tree.syntax()));
}
