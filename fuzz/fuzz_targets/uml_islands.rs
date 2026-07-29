#![no_main]

mod support;

use libfuzzer_sys::fuzz_target;
use waml::{analysis::prepare_candidate, source::SourceBundle};

fuzz_target!(|data: &[u8]| {
    let Some(body) = support::valid_utf8(data) else {
        return;
    };
    let value = format!("---\ntype: uml.Class\n---\n# fuzz\n\n{body}");
    let source =
        SourceBundle::try_from_pairs([("fuzz.md", value.clone())]).expect("fixed path is valid");
    let prepared = prepare_candidate(source, None, 1).expect("claimed UML input prepares");
    assert!(prepared.uml().claims.contains("fuzz"));
    assert_eq!(prepared.uml().syntax.len(), 1);
    assert_eq!(prepared.uml().declared.concepts().count(), 1);
    assert!(prepared.uml().declared.concept("fuzz").is_some());

    let document = prepared
        .okf()
        .catalog
        .id_for_path(&waml::source::BundlePath::parse("fuzz.md").unwrap())
        .expect("cataloged fuzz document");
    let snapshot = prepared
        .uml()
        .syntax
        .document(document)
        .expect("claimed document has UML syntax");
    assert_eq!(snapshot.syntax().write_to_string(), value);
    support::assert_tree_ranges(snapshot.syntax(), &value);
});
