#![no_main]

mod support;

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use waml::{
    action::{ActionBasis, CodeAction, SyntaxChangeBatch, TextEdit, VersionedDocumentChange},
    analysis::{prepare_candidate, PreviousAnalyses},
    edit::{EditBatch, EditContext},
    source::{BundlePath, SourceBundle, SourceDocument},
};

fn edit(start: usize, end: usize, replacement: impl Into<Arc<str>>) -> TextEdit {
    TextEdit {
        range: support::range(start, end),
        replacement: replacement.into(),
    }
}

fuzz_target!(|data: &[u8]| {
    let Some(body) = support::valid_utf8(data) else {
        return;
    };
    let path = BundlePath::parse("fuzz.md").unwrap();
    let authored = format!("---\ntype: uml.Class\n---\n# fuzz é\n\n{body}");
    let source =
        SourceBundle::try_from_pairs([("fuzz.md", authored.clone())]).expect("fixed source");
    let prepared = prepare_candidate(source.clone(), None, 1).expect("baseline prepares");
    let document = prepared
        .okf()
        .catalog
        .id_for_path(&path)
        .expect("cataloged document");
    let revision = prepared
        .okf()
        .catalog
        .document(document)
        .expect("versioned document")
        .revision();
    let context = EditContext {
        source: &source,
        okf_analysis: prepared.okf(),
        session_revision: 1,
        uml: prepared.uml(),
    };

    let (start, end, replacement) = support::derived_valid_edit(data, &authored);
    let batch = SyntaxChangeBatch::new(CodeAction {
        title: "fuzz valid edit".into(),
        basis: ActionBasis::Bundle {
            session_revision: 1,
        },
        changes: vec![VersionedDocumentChange {
            document,
            base_document_revision: revision,
            edits: vec![edit(start, end, replacement.clone())].into(),
        }]
        .into(),
    })
    .expect("one valid edit is structurally valid");
    let candidate = batch.lower(context).expect("valid versioned edit lowers");
    let mut oracle = authored.clone();
    oracle.replace_range(start..end, &replacement);
    assert_eq!(candidate.document(&path).unwrap().text(), oracle);
    let changed = prepare_candidate(
        candidate,
        Some(PreviousAnalyses {
            okf: prepared.okf(),
            uml: prepared.uml(),
        }),
        2,
    )
    .expect("edited candidate prepares");
    assert_eq!(
        changed
            .okf()
            .shell
            .document(document)
            .unwrap()
            .syntax()
            .write_to_string(),
        oracle
    );

    let selector = data.first().copied().unwrap_or(0) % 5;
    let unicode = authored.find('é').unwrap();
    let (basis, edits) = match selector {
        0 => (
            ActionBasis::Bundle {
                session_revision: 1,
            },
            vec![edit(0, 2, "x"), edit(1, 3, "y")],
        ),
        1 => (
            ActionBasis::Bundle {
                session_revision: 2,
            },
            vec![edit(0, 1, "x")],
        ),
        2 => {
            let stale_source = waml::host::replace_document(
                &source,
                SourceDocument::new(path.clone(), format!("{authored}\n")),
            )
            .unwrap();
            let stale_prepared = prepare_candidate(
                stale_source,
                Some(PreviousAnalyses {
                    okf: prepared.okf(),
                    uml: prepared.uml(),
                }),
                2,
            )
            .expect("changed source prepares");
            let changed_revision = stale_prepared
                .okf()
                .catalog
                .document(document)
                .unwrap()
                .revision();
            (
                ActionBasis::Document {
                    document,
                    document_revision: changed_revision,
                    session_revision: 1,
                },
                vec![edit(0, 1, "x")],
            )
        }
        3 => (
            ActionBasis::Bundle {
                session_revision: 1,
            },
            vec![edit(authored.len(), authored.len() + 1, "x")],
        ),
        _ => (
            ActionBasis::Bundle {
                session_revision: 1,
            },
            vec![edit(unicode + 1, unicode + 2, "x")],
        ),
    };
    let invalid = SyntaxChangeBatch::new(CodeAction {
        title: "fuzz invalid edit".into(),
        basis,
        changes: vec![VersionedDocumentChange {
            document,
            base_document_revision: revision,
            edits: edits.into(),
        }]
        .into(),
    });
    match selector {
        0 => assert!(invalid.is_err()),
        _ => assert!(invalid.unwrap().lower(context).is_err()),
    }
    assert_eq!(source.document(&path).unwrap().text(), authored);
});
