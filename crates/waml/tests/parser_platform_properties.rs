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
    analysis::{prepare_candidate, validate_disjoint_claims, AnalysisError, ClaimSet, PreviousAnalyses},
    edit::{EditBatch, EditContext},
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

struct AllocationMeasurement {
    document_bytes: usize,
    events: usize,
    was_active: bool,
}

impl AllocationMeasurement {
    fn start(document_bytes: usize) -> Self {
        let was_active = COUNT_THIS_THREAD.with(|active| {
            let was_active = active.get();
            active.set(true);
            was_active
        });
        Self {
            document_bytes: DOCUMENT_BYTES.swap(document_bytes, Ordering::SeqCst),
            events: DOCUMENT_SIZED_BYTE_BUFFER_EVENTS.swap(0, Ordering::SeqCst),
            was_active,
        }
    }

    fn finish(&self) -> usize {
        DOCUMENT_SIZED_BYTE_BUFFER_EVENTS.load(Ordering::SeqCst)
    }
}

impl Drop for AllocationMeasurement {
    fn drop(&mut self) {
        DOCUMENT_BYTES.store(self.document_bytes, Ordering::SeqCst);
        DOCUMENT_SIZED_BYTE_BUFFER_EVENTS.store(self.events, Ordering::SeqCst);
        COUNT_THIS_THREAD.with(|active| active.set(self.was_active));
    }
}

fn measure<R>(document_bytes: usize, f: impl FnOnce() -> R) -> (R, usize) {
    let _lock = MEASUREMENT_LOCK.lock().unwrap();
    let measurement = AllocationMeasurement::start(document_bytes);
    let result = f();
    (result, measurement.finish())
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
fn allocation_counter_reports_exactly_one_named_document_buffer_with_provenance() {
    const DOCUMENT_BYTES: usize = 64 * 1024;
    let (named_document_buffer, allocations) = measure(DOCUMENT_BYTES, || {
        let named_document_buffer = "x".repeat(DOCUMENT_BYTES);
        assert_eq!(named_document_buffer.len(), DOCUMENT_BYTES);
        named_document_buffer
    });
    assert_eq!(allocations, 1, "the named document buffer is the one allocation");
    assert_eq!(named_document_buffer.as_ptr().is_null(), false);
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

#[test]
fn one_thousand_two_claimed_document_edits_retain_only_baseline_and_current_provenance() {
    let paths = [BundlePath::parse("left.md").unwrap(), BundlePath::parse("right.md").unwrap()];
    let source = SourceBundle::try_from_pairs([
        ("left.md", "---\ntype: uml.Class\n---\n# Left\n"),
        ("right.md", "---\ntype: uml.Class\n---\n# Right\n"),
    ]).unwrap();
    let baseline = prepare_candidate(source, None, 1).unwrap();
    let ids: Vec<_> = paths.iter().map(|path| baseline.okf().catalog.id_for_path(path).unwrap()).collect();
    let mut weak: Vec<_> = ids.iter().map(|&id| tree_source_weak(baseline.okf().shell.document(id).unwrap().syntax())).collect();
    let mut current = prepare_candidate(SourceBundle::try_from_pairs([
        ("left.md", "---\ntype: uml.Class\n---\n# left\n"),
        ("right.md", "---\ntype: uml.Class\n---\n# right\n"),
    ]).unwrap(), Some(PreviousAnalyses { okf: baseline.okf(), uml: baseline.uml() }), 2).unwrap();
    for edit in 1..1_000 {
        let left = if edit % 2 == 0 { "L" } else { "l" };
        let right = if edit % 2 == 0 { "R" } else { "r" };
        let next = prepare_candidate(SourceBundle::try_from_pairs([
            ("left.md", format!("---\ntype: uml.Class\n---\n# {left}eft\n")),
            ("right.md", format!("---\ntype: uml.Class\n---\n# {right}ight\n")),
        ]).unwrap(), Some(PreviousAnalyses { okf: current.okf(), uml: current.uml() }), edit + 2).unwrap();
        for &id in &ids { weak.push(tree_source_weak(next.okf().shell.document(id).unwrap().syntax())); }
        current = next;
    }
    assert_eq!(weak.iter().filter(|source| source.upgrade().is_some()).count(), 4);
    drop(baseline);
    assert_eq!(weak.iter().filter(|source| source.upgrade().is_some()).count(), 2);
    for (&id, path) in ids.iter().zip(&paths) {
        assert_eq!(current.okf().shell.document(id).unwrap().document().path(), path);
        assert!(current.uml().syntax.document(id).is_some());
    }
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
fn literal_claimed_paths_with_matching_titles_remain_distinct_documents() {
    let source = SourceBundle::try_from_pairs([
        ("left.md", "---\ntype: uml.Class\n---\n# Same\n"),
        ("right.md", "---\ntype: uml.Class\n---\n# Same\n"),
    ])
    .unwrap();
    let prepared = prepare_candidate(source, None, 1).unwrap();
    let left = prepared
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("left.md").unwrap())
        .unwrap();
    let right = prepared
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("right.md").unwrap())
        .unwrap();
    assert_ne!(left, right);
    assert!(prepared.uml().syntax.document(left).is_some());
    assert!(prepared.uml().syntax.document(right).is_some());
}

#[test]
fn literal_provider_claim_ambiguity_reports_stable_provenance() {
    let shared = ClaimSet::from_concept_ids(["same".to_owned()]);
    assert!(matches!(
        validate_disjoint_claims([("uml", &shared), ("future", &shared)]),
        Err(AnalysisError::AmbiguousClaim { concept_id, first, second })
            if concept_id == "same" && first == "future" && second == "uml"
    ));
}

proptest! {
    #[test]
    fn literal_claim_reserved_unclaimed_ambiguity_and_declaration_states_are_distinct(
        ty in "[^\\r\\n]{0,48}", body in "[^\\r\\n]{0,24}"
    ) {
        let claimed_source = format!("---\ntype: uml.Class\n---\n# Claimed\n{body}\n");
        let unclaimed_source = format!("---\ntype: {ty}\n---\n# Unclaimed\n");
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Reserved\n"),
            ("claimed.md", claimed_source.as_str()),
            ("unclaimed.md", unclaimed_source.as_str()),
        ]).unwrap();
        let prepared = prepare_candidate(source, None, 1).unwrap();
        let claimed = prepared.okf().catalog.id_for_path(&BundlePath::parse("claimed.md").unwrap()).unwrap();
        let unclaimed = prepared.okf().catalog.id_for_path(&BundlePath::parse("unclaimed.md").unwrap()).unwrap();
        prop_assert!(prepared.okf().bundle.concept("index").is_none());
        prop_assert!(prepared.uml().syntax.document(claimed).is_some());
        prop_assert_eq!(prepared.uml().syntax.document(unclaimed).is_some(), ["uml.Class", "uml.Package", "uml.DataType", "uml.Enum", "uml.Object", "uml.Activity", "uml.StateMachine", "uml.Sequence", "Diagram"].contains(&ty.as_str()));
        prop_assert!(prepared.uml().declared.concept("claimed").is_some());
    }
}

#[test]
fn edit_batch_lowering_has_valid_output_and_all_invalid_cases_are_atomic() {
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
    let context = EditContext { source: &source, okf_analysis: prepared.okf(), session_revision: 7, uml: prepared.uml() };
    let valid = SyntaxChangeBatch::new(CodeAction {
        title: "valid".into(), basis: ActionBasis::Bundle { session_revision: 7 },
        changes: vec![VersionedDocumentChange { document, base_document_revision: revision, edits: vec![edit(2, 3, "Z")].into() }].into(),
    }).unwrap();
    assert_eq!(valid.lower(context).unwrap().document(&BundlePath::parse("a.md").unwrap()).unwrap().text(), "# Z\n");
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
    assert!(SyntaxChangeBatch::new(action).is_err(), "overlap is rejected before mutation");
    let stale = SyntaxChangeBatch::new(CodeAction { title: "stale".into(), basis: ActionBasis::Bundle { session_revision: 8 }, changes: vec![VersionedDocumentChange { document, base_document_revision: revision, edits: vec![edit(2, 3, "Z")].into() }].into() }).unwrap();
    assert!(stale.lower(context).is_err(), "stale basis is rejected atomically");
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
    for (name, source) in [
        ("syntax incremental", syntax_incremental),
        ("UML incremental", uml_incremental),
        ("OKF shell", okf_shell),
    ] {
        assert!(
            !production(source).contains("write_to_string()"),
            "{name} must retain source slices rather than materialize a complete tree"
        );
    }
}
