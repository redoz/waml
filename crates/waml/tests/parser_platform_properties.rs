use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};

use proptest::prelude::*;
use waml::{
    action::{ActionBasis, CodeAction, SyntaxChangeBatch, TextEdit, VersionedDocumentChange},
    analysis::{prepare_candidate, ClaimSet, PreviousAnalyses},
    source::{BundlePath, SourceBundle},
    uml::DeclaredField,
};
use waml_syntax::{GreenElement, GreenText, SyntaxTree};

struct CountingAllocator;

static DOCUMENT_BYTES: AtomicUsize = AtomicUsize::new(usize::MAX);
static DOCUMENT_SIZED_BYTE_BUFFER_EVENTS: AtomicUsize = AtomicUsize::new(0);
static MEASUREMENT_LOCK: Mutex<()> = Mutex::new(());
thread_local! {
    static COUNT_THIS_THREAD: Cell<bool> = const { Cell::new(false) };
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null()
            && COUNT_THIS_THREAD.try_with(Cell::get).unwrap_or(false)
            && layout.align() == std::mem::align_of::<u8>()
            && layout.size() >= DOCUMENT_BYTES.load(Ordering::Relaxed)
        {
            DOCUMENT_SIZED_BYTE_BUFFER_EVENTS.fetch_add(1, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let pointer = unsafe { System.realloc(pointer, layout, size) };
        if !pointer.is_null()
            && COUNT_THIS_THREAD.try_with(Cell::get).unwrap_or(false)
            && layout.align() == std::mem::align_of::<u8>()
            && size >= DOCUMENT_BYTES.load(Ordering::Relaxed)
        {
            DOCUMENT_SIZED_BYTE_BUFFER_EVENTS.fetch_add(1, Ordering::Relaxed);
        }
        pointer
    }
}

fn measure<R>(document_bytes: usize, f: impl FnOnce() -> R) -> (R, usize) {
    let _lock = MEASUREMENT_LOCK.lock().unwrap();
    DOCUMENT_BYTES.store(document_bytes, Ordering::Relaxed);
    DOCUMENT_SIZED_BYTE_BUFFER_EVENTS.store(0, Ordering::Relaxed);
    COUNT_THIS_THREAD.with(|active| active.set(true));
    let result = f();
    COUNT_THIS_THREAD.with(|active| active.set(false));
    (
        result,
        DOCUMENT_SIZED_BYTE_BUFFER_EVENTS.load(Ordering::Relaxed),
    )
}

#[test]
fn allocator_proxy_calibrates_byte_strings_and_excludes_aligned_vectors() {
    let original = "x".repeat(64 * 1024);
    let (_, empty) = measure(original.len(), || {});
    let (_, structural) = measure(original.len(), || {
        std::hint::black_box(Vec::<u64>::with_capacity(original.len() / 8));
    });
    let (_, cloned_string) = measure(original.len(), || {
        std::hint::black_box(original.clone());
    });
    assert_eq!((empty, structural, cloned_string), (0, 0, 1));
}

#[test]
fn candidate_preparation_does_not_copy_the_whole_document() {
    const LARGE_DOCUMENT: usize = 1_048_576;
    let raw = "a".repeat(LARGE_DOCUMENT);
    let baseline_source = SourceBundle::try_from_pairs([(
        "document.md",
        format!("---\ntype: Playbook\n---\n# Alpha\n\n<div>\n{raw}\n</div>\n"),
    )])
    .unwrap();
    let document_len = baseline_source
        .document(&BundlePath::parse("document.md").unwrap())
        .unwrap()
        .text()
        .len();
    let baseline_pointer = baseline_source
        .document(&BundlePath::parse("document.md").unwrap())
        .unwrap()
        .text()
        .as_ptr();
    let (full, full_large) = measure(document_len, || {
        prepare_candidate(baseline_source.clone(), None, 0).unwrap()
    });
    let baseline = prepare_candidate(baseline_source.clone(), None, 1).unwrap();
    let path = BundlePath::parse("document.md").unwrap();
    assert_eq!(
        full.source().document(&path).unwrap().text().as_ptr(),
        baseline_pointer
    );
    let id = baseline.okf().catalog.id_for_path(&path).unwrap();
    let (unchanged, unchanged_large) = measure(document_len, || {
        prepare_candidate(
            baseline_source.clone(),
            Some(PreviousAnalyses {
                okf: baseline.okf(),
                uml: baseline.uml(),
            }),
            2,
        )
        .unwrap()
    });
    let old_snapshot = baseline.okf().shell.document(id).unwrap();
    let unchanged_snapshot = unchanged.okf().shell.document(id).unwrap();
    assert!(std::sync::Arc::ptr_eq(old_snapshot, unchanged_snapshot));
    assert_eq!(
        unchanged.source().document(&path).unwrap().text().as_ptr(),
        baseline_pointer
    );
    assert_eq!(
        full_large, 0,
        "fixture must not make the full parser allocate a whole source"
    );
    assert_eq!(
        unchanged_large, 0,
        "identical source must reuse its snapshot without whole-document allocation"
    );

    let (touched, touched_large) = measure(document_len, || {
        // The candidate source is the one intentional whole-document
        // allocation for a touched document.  Avoid `replace_range`/formatting
        // so capacity growth cannot masquerade as a parser allocation.
        let mut bytes = baseline_source
            .document(&path)
            .unwrap()
            .text()
            .as_bytes()
            .to_vec();
        let heading = "---\ntype: Playbook\n---\n# Alpha";
        bytes[heading.len() - 1] = b'B';
        let touched_source = SourceBundle::try_from_pairs([(
            "document.md",
            String::from_utf8(bytes).expect("the source remains valid UTF-8"),
        )])
        .unwrap();
        let expected_pointer = touched_source.document(&path).unwrap().text().as_ptr();
        let prepared = prepare_candidate(
            touched_source,
            Some(PreviousAnalyses {
                okf: baseline.okf(),
                uml: baseline.uml(),
            }),
            3,
        )
        .unwrap();
        assert_eq!(
            prepared.source().document(&path).unwrap().text().as_ptr(),
            expected_pointer
        );
        prepared
    });
    assert_eq!(
        touched_large, 1,
        "only the intended replacement source allocation is allowed"
    );
    let touched_document = touched.source().document(&path).unwrap();
    let touched_slice = touched_document
        .slice(0..touched_document.text().len())
        .unwrap();
    assert_eq!(
        touched_slice.as_str().as_ptr(),
        touched_document.text().as_ptr()
    );
}

fn tree_source_weak<L: waml_syntax::SyntaxLanguage>(
    tree: &SyntaxTree<L>,
) -> std::sync::Weak<String> {
    fn find<L: waml_syntax::SyntaxLanguage>(
        element: &GreenElement<L>,
    ) -> Option<std::sync::Weak<String>> {
        match element {
            GreenElement::Node(node) => node.children().iter().find_map(find),
            GreenElement::Token(token) => token
                .leading_trivia()
                .iter()
                .map(|x| &x.text)
                .chain(std::iter::once(token.text()))
                .chain(token.trailing_trivia().iter().map(|x| &x.text))
                .find_map(|text| match text {
                    GreenText::SourceSlice { source, .. } => {
                        Some(std::sync::Arc::downgrade(source.shared()))
                    }
                    GreenText::Static(_) | GreenText::Owned(_) => None,
                }),
        }
    }
    find(&GreenElement::Node(tree.root_green().clone())).expect("parsed tree retains source")
}

#[test]
fn one_thousand_edits_retain_only_baseline_and_current_sources() {
    let path = BundlePath::parse("document.md").unwrap();
    let baseline_source = SourceBundle::try_from_pairs([("document.md", "# A\nbody\n")]).unwrap();
    let baseline = prepare_candidate(baseline_source, None, 1).unwrap();
    let id = baseline.okf().catalog.id_for_path(&path).unwrap();
    let mut weak = vec![tree_source_weak(
        baseline.okf().shell.document(id).unwrap().syntax(),
    )];
    let mut current = prepare_candidate(
        SourceBundle::try_from_pairs([("document.md", "# B\nbody\n")]).unwrap(),
        Some(PreviousAnalyses {
            okf: baseline.okf(),
            uml: baseline.uml(),
        }),
        2,
    )
    .unwrap();
    weak.push(tree_source_weak(
        current.okf().shell.document(id).unwrap().syntax(),
    ));
    for edit in 0..999 {
        let title = if edit % 2 == 0 {
            "# C\nbody\n"
        } else {
            "# B\nbody\n"
        };
        let next_source = SourceBundle::try_from_pairs([("document.md", title)]).unwrap();
        let next = prepare_candidate(
            next_source,
            Some(PreviousAnalyses {
                okf: current.okf(),
                uml: current.uml(),
            }),
            edit + 3,
        )
        .unwrap();
        weak.push(tree_source_weak(
            next.okf().shell.document(id).unwrap().syntax(),
        ));
        current = next;
    }
    assert_eq!(
        weak.iter().filter(|weak| weak.upgrade().is_some()).count(),
        2
    );
    drop(baseline);
    assert_eq!(
        weak.iter().filter(|weak| weak.upgrade().is_some()).count(),
        1
    );
}

proptest! {
    #[test]
    fn only_supported_uml_type_strings_are_claimed(ty in "[^\\r\\n]{0,48}") {
        const SUPPORTED: &[&str] = &["uml.Class", "uml.Package", "uml.DataType", "uml.Enum", "uml.Object", "uml.Activity", "uml.StateMachine", "uml.Sequence", "Diagram"];
        let concept = waml::okf::project("x.md", &format!("---\ntype: {ty}\n---\n# X\n"));
        prop_assert_eq!(waml::uml::recognizes(&concept), SUPPORTED.contains(&ty.as_str()));
    }
}

#[test]
fn reserved_unclaimed_and_declared_states_remain_distinct() {
    let source = SourceBundle::try_from_pairs([
        ("index.md", "# Index\n"),
        ("log.md", "# Log\n"),
        (
            "plain.md",
            "---\ntype: Playbook\n---\n# Plain\n## Attributes\n- hidden: String\n",
        ),
        ("absent.md", "---\ntype: uml.Class\n---\n# Absent\n"),
        (
            "invalid.md",
            "---\ntype: uml.Class\n---\n# Invalid\n## Attributes\n- broken\n",
        ),
    ])
    .unwrap();
    let prepared = prepare_candidate(source, None, 1).unwrap();
    assert!(prepared.okf().bundle.concept("index").is_none());
    assert!(prepared.okf().bundle.concept("log").is_none());
    let plain = prepared
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("plain.md").unwrap())
        .unwrap();
    assert!(prepared.uml().syntax.document(plain).is_none());
    assert!(prepared
        .uml()
        .declared
        .concept("absent")
        .unwrap()
        .attributes
        .is_empty());
    let invalid = &prepared
        .uml()
        .declared
        .concept("invalid")
        .unwrap()
        .attributes[0];
    assert!(!matches!(invalid.name, DeclaredField::Absent));
    assert!(!matches!(invalid.ty, DeclaredField::Absent));
}

#[test]
fn identical_claim_sets_have_stable_order_and_invalid_actions_are_atomic() {
    let left: Vec<_> = ClaimSet::from_concept_ids(["b".to_owned(), "a".to_owned()])
        .iter()
        .map(str::to_owned)
        .collect();
    let right: Vec<_> = ClaimSet::from_concept_ids(["a".to_owned(), "b".to_owned()])
        .iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(left, right);

    let source = SourceBundle::try_from_pairs([("a.md", "# A\n")]).unwrap();
    let prepared = prepare_candidate(source.clone(), None, 7).unwrap();
    let document = prepared
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("a.md").unwrap())
        .unwrap();
    let revision = prepared
        .okf()
        .catalog
        .document(document)
        .unwrap()
        .revision();
    let edit = |start, end, replacement| TextEdit {
        range: waml_syntax::TextRange::new(
            waml_syntax::TextSize::try_from_usize(start).unwrap(),
            waml_syntax::TextSize::try_from_usize(end).unwrap(),
        )
        .unwrap(),
        replacement: std::sync::Arc::from(replacement),
    };
    let action = CodeAction {
        title: "overlap".into(),
        basis: ActionBasis::Bundle {
            session_revision: 7,
        },
        changes: vec![VersionedDocumentChange {
            document,
            base_document_revision: revision,
            edits: vec![edit(0, 2, "X"), edit(1, 3, "Y")].into(),
        }]
        .into(),
    };
    assert!(SyntaxChangeBatch::new(action).is_err());
    assert_eq!(
        source
            .document(&BundlePath::parse("a.md").unwrap())
            .unwrap()
            .text(),
        "# A\n"
    );
}

#[test]
fn known_whole_tree_materializers_stay_out_of_incremental_paths() {
    fn production(source: &str) -> &str {
        source.split("#[cfg(test)]").next().unwrap_or(source)
    }
    let syntax_incremental = include_str!("../../waml-syntax/src/incremental.rs");
    let uml_incremental = include_str!("../src/uml/syntax/mod.rs");
    let okf_shell = include_str!("../src/okf/shell.rs");
    assert!(!production(syntax_incremental).contains("previous.write_to_string()"));
    assert!(!production(uml_incremental).contains("previous.write_to_string()"));
    assert!(!production(okf_shell).contains("snapshot.syntax().write_to_string()"));
}
