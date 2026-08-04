use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    mem::align_of,
    sync::Mutex,
};

use proptest::prelude::*;
use waml::{
    action::{
        ActionBasis, ActionError, CodeAction, SyntaxChangeBatch, TextEdit, VersionedDocumentChange,
    },
    analysis::{
        prepare_candidate, validate_disjoint_claims, AnalysisError, ClaimSet, PreviousAnalyses,
    },
    edit::{EditBatch, EditContext},
    source::{BundlePath, SourceBundle},
    uml::DeclaredField,
};
use waml_syntax::{GreenElement, GreenText, SyntaxTree};

struct CountingAllocator;

static MEASUREMENT_LOCK: Mutex<()> = Mutex::new(());
thread_local! {
    static COUNT_THIS_THREAD: Cell<bool> = const { Cell::new(false) };
    static DOCUMENT_BYTES: Cell<usize> = const { Cell::new(usize::MAX) };
    static DOCUMENT_SIZED_BYTE_BUFFER_EVENTS: Cell<usize> = const { Cell::new(0) };
}

fn record_byte_buffer_allocation(size: usize, align: usize) {
    let count = COUNT_THIS_THREAD.try_with(Cell::get).unwrap_or(false)
        && align == align_of::<u8>()
        && DOCUMENT_BYTES
            .try_with(Cell::get)
            .is_ok_and(|threshold| size >= threshold);
    if count {
        let _ = DOCUMENT_SIZED_BYTE_BUFFER_EVENTS.try_with(|events| events.set(events.get() + 1));
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_byte_buffer_allocation(layout.size(), layout.align());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record_byte_buffer_allocation(layout.size(), layout.align());
        }
        pointer
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let pointer = unsafe { System.realloc(pointer, layout, size) };
        if !pointer.is_null() {
            record_byte_buffer_allocation(size, layout.align());
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
            document_bytes: DOCUMENT_BYTES.replace(document_bytes),
            events: DOCUMENT_SIZED_BYTE_BUFFER_EVENTS.replace(0),
            was_active,
        }
    }

    fn finish(&self) -> usize {
        DOCUMENT_SIZED_BYTE_BUFFER_EVENTS.with(Cell::get)
    }
}

impl Drop for AllocationMeasurement {
    fn drop(&mut self) {
        DOCUMENT_BYTES.set(self.document_bytes);
        DOCUMENT_SIZED_BYTE_BUFFER_EVENTS.set(self.events);
        COUNT_THIS_THREAD.set(self.was_active);
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
    let (_, below_threshold) = measure(original.len(), || {
        std::hint::black_box("x".repeat(original.len() - 1));
    });
    let (_, zeroed) = measure(original.len(), || unsafe {
        let layout = Layout::array::<u8>(original.len()).unwrap();
        let pointer = std::alloc::alloc_zeroed(layout);
        assert!(!pointer.is_null());
        std::alloc::dealloc(pointer, layout);
    });
    let (_, grown) = measure(original.len(), || {
        let mut bytes = Vec::<u8>::with_capacity(original.len() / 2);
        bytes.reserve_exact(original.len());
        std::hint::black_box(bytes);
    });
    let (_, other_thread) = measure(original.len(), || {
        std::thread::spawn(move || std::hint::black_box("x".repeat(original.len())))
            .join()
            .unwrap();
    });
    assert_eq!(
        (
            empty,
            structural,
            cloned_string,
            below_threshold,
            zeroed,
            grown,
            other_thread
        ),
        (0, 0, 1, 0, 1, 1, 0)
    );
}

#[test]
fn allocation_counter_reports_exactly_one_named_document_buffer_with_provenance() {
    const DOCUMENT_BYTES: usize = 64 * 1024;
    let (named_document_buffer, allocations) = measure(DOCUMENT_BYTES, || {
        let named_document_buffer = "x".repeat(DOCUMENT_BYTES);
        assert_eq!(named_document_buffer.len(), DOCUMENT_BYTES);
        named_document_buffer
    });
    assert_eq!(
        allocations, 1,
        "the named document buffer is the one allocation"
    );
    assert_eq!(named_document_buffer.len(), DOCUMENT_BYTES);
}

#[test]
fn whole_document_allocation_counts_are_exact() {
    const LARGE_DOCUMENT: usize = 1_048_576;
    let raw = "a".repeat(LARGE_DOCUMENT);
    let baseline_source = SourceBundle::try_from_pairs([(
        "document.md",
        format!("---\ntype: uml.Class\n---\n# Alpha\n\n<div>\n{raw}\n</div>\n"),
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
    let baseline_allocation = baseline_source.document(&path).unwrap().text().as_ptr();
    assert_eq!(
        baseline
            .okf()
            .catalog
            .document(id)
            .unwrap()
            .text()
            .shared()
            .as_ptr(),
        baseline_allocation
    );
    assert_every_source_slice_shares(
        baseline.okf().markdown.document(id).unwrap().tree(),
        baseline_allocation,
    );
    assert_every_source_slice_shares(
        baseline.uml().syntax.document(id).unwrap().syntax(),
        baseline_allocation,
    );
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
    let old_snapshot = baseline.okf().markdown.document(id).unwrap();
    let unchanged_snapshot = unchanged.okf().markdown.document(id).unwrap();
    assert!(std::sync::Arc::ptr_eq(old_snapshot, unchanged_snapshot));
    assert!(std::sync::Arc::ptr_eq(
        baseline.okf().markdown.document(id).unwrap().structure(),
        unchanged.okf().markdown.document(id).unwrap().structure()
    ));
    assert!(std::sync::Arc::ptr_eq(
        baseline.uml().syntax.document(id).unwrap(),
        unchanged.uml().syntax.document(id).unwrap()
    ));
    assert_eq!(
        unchanged.source().document(&path).unwrap().text().as_ptr(),
        baseline_pointer
    );
    assert_eq!(
        full_large, 1,
        "the canonical Markdown snapshot owns the one full-parse source buffer"
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
        let heading = "---\ntype: uml.Class\n---\n# Alpha";
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
        touched_large, 2,
        "the replacement source and canonical Markdown snapshot buffers are allowed"
    );
    let touched_document = touched.source().document(&path).unwrap();
    let touched_allocation = touched_document.text().as_ptr();
    assert_eq!(
        touched
            .okf()
            .catalog
            .document(id)
            .unwrap()
            .text()
            .shared()
            .as_ptr(),
        touched_allocation
    );
    assert_every_source_slice_shares(
        touched.okf().markdown.document(id).unwrap().tree(),
        touched_allocation,
    );
    assert_every_source_slice_shares(
        touched.uml().syntax.document(id).unwrap().syntax(),
        touched_allocation,
    );
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

fn assert_every_source_slice_shares<L: waml_syntax::SyntaxLanguage>(
    tree: &SyntaxTree<L>,
    expected: *const u8,
) {
    fn walk<L: waml_syntax::SyntaxLanguage>(
        element: &GreenElement<L>,
        expected: *const u8,
        count: &mut usize,
    ) {
        match element {
            GreenElement::Node(node) => {
                for child in node.children() {
                    walk(child, expected, count);
                }
            }
            GreenElement::Token(token) => {
                for text in token
                    .leading_trivia()
                    .iter()
                    .map(|x| &x.text)
                    .chain(std::iter::once(token.text()))
                    .chain(token.trailing_trivia().iter().map(|x| &x.text))
                {
                    if let GreenText::SourceSlice { source, .. } = text {
                        assert_eq!(source.shared().as_ptr(), expected);
                        *count += 1;
                    }
                }
            }
        }
    }
    let mut count = 0;
    walk(
        &GreenElement::Node(tree.root_green().clone()),
        expected,
        &mut count,
    );
    assert!(count > 0, "tree must retain source-backed leaves");
}

#[test]
fn one_thousand_edits_retain_only_baseline_and_current_sources() {
    let path = BundlePath::parse("document.md").unwrap();
    let baseline_source = SourceBundle::try_from_pairs([("document.md", "# A\nbody\n")]).unwrap();
    let baseline = prepare_candidate(baseline_source, None, 1).unwrap();
    let id = baseline.okf().catalog.id_for_path(&path).unwrap();
    let mut weak = vec![tree_source_weak(
        baseline.okf().markdown.document(id).unwrap().tree(),
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
        current.okf().markdown.document(id).unwrap().tree(),
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
            next.okf().markdown.document(id).unwrap().tree(),
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
fn one_thousand_host_replacements_retain_two_touched_and_one_untouched_allocation() {
    let left_path = BundlePath::parse("left.md").unwrap();
    let right_path = BundlePath::parse("right.md").unwrap();
    let source = SourceBundle::try_from_pairs([
        ("left.md", "---\ntype: uml.Class\n---\n# Left\n"),
        ("right.md", "---\ntype: uml.Class\n---\n# Right\n"),
    ])
    .unwrap();
    let baseline = prepare_candidate(source, None, 1).unwrap();
    let left_id = baseline.okf().catalog.id_for_path(&left_path).unwrap();
    let right_id = baseline.okf().catalog.id_for_path(&right_path).unwrap();
    let untouched_version = baseline.okf().catalog.document(right_id).unwrap().clone();
    let untouched_markdown = baseline.okf().markdown.document(right_id).unwrap().clone();
    let untouched_uml = baseline.uml().syntax.document(right_id).unwrap().clone();
    let untouched_structure = untouched_markdown.structure().clone();
    let untouched_source = tree_source_weak(untouched_markdown.tree());
    let mut touched_sources = vec![tree_source_weak(
        baseline.okf().markdown.document(left_id).unwrap().tree(),
    )];
    let first_source = waml::host::replace_document(
        baseline.source(),
        waml::source::SourceDocument::new(
            left_path.clone(),
            "---\ntype: uml.Class\n---\n# left\n".to_owned(),
        ),
    )
    .unwrap();
    let mut current = prepare_candidate(
        first_source,
        Some(PreviousAnalyses {
            okf: baseline.okf(),
            uml: baseline.uml(),
        }),
        2,
    )
    .unwrap();
    touched_sources.push(tree_source_weak(
        current.okf().markdown.document(left_id).unwrap().tree(),
    ));
    for edit in 1..1_000 {
        let title = if edit % 2 == 0 { "Left" } else { "left" };
        let next_source = waml::host::replace_document(
            current.source(),
            waml::source::SourceDocument::new(
                left_path.clone(),
                format!("---\ntype: uml.Class\n---\n# {title}\n"),
            ),
        )
        .unwrap();
        let next = prepare_candidate(
            next_source,
            Some(PreviousAnalyses {
                okf: current.okf(),
                uml: current.uml(),
            }),
            edit + 2,
        )
        .unwrap();
        touched_sources.push(tree_source_weak(
            next.okf().markdown.document(left_id).unwrap().tree(),
        ));
        assert!(std::sync::Arc::ptr_eq(
            &untouched_version,
            next.okf().catalog.document(right_id).unwrap()
        ));
        assert!(std::sync::Arc::ptr_eq(
            &untouched_markdown,
            next.okf().markdown.document(right_id).unwrap()
        ));
        assert!(std::sync::Arc::ptr_eq(
            &untouched_uml,
            next.uml().syntax.document(right_id).unwrap()
        ));
        assert!(std::sync::Arc::ptr_eq(
            &untouched_structure,
            next.okf().markdown.document(right_id).unwrap().structure()
        ));
        current = next;
    }
    assert_eq!(
        touched_sources
            .iter()
            .filter(|source| source.upgrade().is_some())
            .count(),
        2
    );
    assert!(untouched_source.upgrade().is_some());
    drop(baseline);
    assert_eq!(
        touched_sources
            .iter()
            .filter(|source| source.upgrade().is_some())
            .count(),
        1
    );
    assert!(untouched_source.upgrade().is_some());
    assert_eq!(
        current.okf().catalog.document(right_id).unwrap().path(),
        &right_path
    );
}

proptest! {
    #[test]
    fn only_supported_uml_type_strings_are_claimed(ty in "[^\\r\\n]{0,48}") {
        const SUPPORTED: &[&str] = &["uml.Class", "uml.Interface", "uml.Enum", "uml.DataType", "uml.Package", "uml.Note", "uml.Association", "uml.Actor", "uml.UseCase", "uml.InstanceSpecification", "uml.Activity", "uml.StateMachine", "uml.Sequence", "Diagram"];
        let concept = waml::okf::project("x.md", &format!("---\ntype: {ty}\n---\n# X\n")).unwrap();
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
        (
            "valid.md",
            "---\ntype: uml.Class\n---\n# Valid\n## Attributes\n- name: String\n",
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
    assert!(matches!(
        invalid.ty,
        DeclaredField::Incomplete { .. } | DeclaredField::Invalid { .. }
    ));
    let malformed_range = match &invalid.ty {
        DeclaredField::Incomplete { syntax, .. } | DeclaredField::Invalid { syntax, .. } => {
            syntax.range()
        }
        _ => unreachable!(),
    };
    assert!(malformed_range.start() < malformed_range.end());
    let valid = &prepared.uml().declared.concept("valid").unwrap().attributes[0];
    assert!(matches!(valid.visibility, DeclaredField::Absent));
    assert!(matches!(valid.multiplicity, DeclaredField::Absent));
    assert!(matches!(valid.name, DeclaredField::Valid { .. }));
    assert!(matches!(valid.ty, DeclaredField::Valid { .. }));
}

#[test]
fn every_supported_literal_and_explicit_near_miss_has_exact_claim_state() {
    const SUPPORTED: &[&str] = &[
        "uml.Class",
        "uml.Interface",
        "uml.Enum",
        "uml.DataType",
        "uml.Package",
        "uml.Note",
        "uml.Association",
        "uml.Actor",
        "uml.UseCase",
        "uml.InstanceSpecification",
        "uml.Activity",
        "uml.StateMachine",
        "uml.Sequence",
        "Diagram",
    ];
    const NEAR_MISSES: &[&str] = &[
        "uml.class",
        "uml.Classx",
        "uml.Diagram",
        "diagram",
        "uml.SequenceDiagram",
        "uml.Instance",
    ];
    for ty in SUPPORTED {
        assert!(
            waml::uml::recognizes(
                &waml::okf::project("x.md", &format!("---\ntype: {ty}\n---\n# X\n")).unwrap()
            ),
            "{ty}"
        );
    }
    for ty in NEAR_MISSES {
        assert!(
            !waml::uml::recognizes(
                &waml::okf::project("x.md", &format!("---\ntype: {ty}\n---\n# X\n")).unwrap()
            ),
            "{ty}"
        );
    }
}

proptest! {
    #[test]
    fn reserved_documents_with_arbitrary_bodies_are_never_claimed(body in "(?s).{0,128}") {
        let source = SourceBundle::try_from_pairs([("index.md", body.as_str()), ("log.md", body.as_str())]).unwrap();
        let prepared = prepare_candidate(source, None, 1).unwrap();
        prop_assert!(prepared.okf().bundle.concept("index").is_none());
        prop_assert!(prepared.okf().bundle.concept("log").is_none());
        prop_assert!(prepared.uml().claims.iter().next().is_none());
    }
}

#[test]
fn malformed_reserved_document_bodies_are_unclaimed_and_lossless() {
    let body = "0\n\r\t\u{0800}";
    let source = SourceBundle::try_from_pairs([("index.md", body), ("log.md", body)]).unwrap();
    let prepared = prepare_candidate(source, None, 1).unwrap();

    assert!(prepared.okf().bundle.concept("index").is_none());
    assert!(prepared.okf().bundle.concept("log").is_none());
    assert!(prepared.uml().claims.iter().next().is_none());

    for path in ["index.md", "log.md"] {
        let id = prepared
            .okf()
            .catalog
            .id_for_path(&BundlePath::parse(path).unwrap())
            .unwrap();
        assert_eq!(
            prepared
                .okf()
                .markdown
                .document(id)
                .unwrap()
                .text()
                .shared()
                .as_str(),
            body
        );
    }
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
        prop_assert_eq!(prepared.uml().syntax.document(unclaimed).is_some(), ["uml.Class", "uml.Interface", "uml.Enum", "uml.DataType", "uml.Package", "uml.Note", "uml.Association", "uml.Actor", "uml.UseCase", "uml.InstanceSpecification", "uml.Activity", "uml.StateMachine", "uml.Sequence", "Diagram"].contains(&ty.as_str()));
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

    let source =
        SourceBundle::try_from_pairs([("a.md", "---\ntype: uml.Class\n---\n# é\n")]).unwrap();
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
    let context = EditContext {
        source: &source,
        okf_analysis: prepared.okf(),
        session_revision: 7,
        uml: prepared.uml(),
    };
    let authored = source
        .document(&BundlePath::parse("a.md").unwrap())
        .unwrap()
        .text();
    let heading = authored.find('é').unwrap();
    let valid = SyntaxChangeBatch::new(CodeAction {
        title: "valid".into(),
        basis: ActionBasis::Bundle {
            session_revision: 7,
        },
        changes: vec![VersionedDocumentChange {
            document,
            base_document_revision: revision,
            edits: vec![edit(heading, heading + 2, "Z"), edit(0, 3, "+++\n")].into(),
        }]
        .into(),
    })
    .unwrap();
    let mut oracle = authored.to_owned();
    oracle.replace_range(heading..heading + 2, "Z");
    oracle.replace_range(0..3, "+++\n");
    assert_eq!(
        valid
            .lower(context)
            .unwrap()
            .document(&BundlePath::parse("a.md").unwrap())
            .unwrap()
            .text(),
        oracle
    );
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
    assert_eq!(
        SyntaxChangeBatch::new(action).unwrap_err(),
        ActionError::Overlap {
            document,
            first: edit(0, 2, "").range,
            second: edit(1, 3, "").range
        }
    );
    let cases = [
        (
            "stale session",
            ActionBasis::Bundle {
                session_revision: 8,
            },
            edit(heading, heading + 2, "Z"),
            ActionError::StaleSession {
                expected: 8,
                actual: 7,
            },
        ),
        (
            "non UTF-8 boundary",
            ActionBasis::Bundle {
                session_revision: 7,
            },
            edit(heading + 1, heading + 2, "Z"),
            ActionError::NonUtf8Boundary {
                document,
                offset: waml_syntax::TextSize::try_from_usize(heading + 1).unwrap(),
            },
        ),
        (
            "out of bounds",
            ActionBasis::Bundle {
                session_revision: 7,
            },
            edit(0, authored.len() + 1, "Z"),
            ActionError::InvalidRange {
                document,
                range: edit(0, authored.len() + 1, "").range,
            },
        ),
    ];
    let catalog = prepared.okf().catalog.clone();
    let markdown = prepared.okf().markdown.document(document).unwrap().clone();
    let uml = prepared.uml().syntax.document(document).unwrap().clone();
    let structure = markdown.structure().clone();
    for (name, basis, bad_edit, expected) in cases {
        let batch = SyntaxChangeBatch::new(CodeAction {
            title: name.into(),
            basis,
            changes: vec![VersionedDocumentChange {
                document,
                base_document_revision: revision,
                edits: vec![bad_edit].into(),
            }]
            .into(),
        })
        .unwrap();
        let error = batch.lower(context).unwrap_err();
        assert_eq!(error.reason, format!("syntax action error: {expected:?}"));
        assert!(std::sync::Arc::ptr_eq(&catalog, &prepared.okf().catalog));
        assert!(std::sync::Arc::ptr_eq(
            &markdown,
            prepared.okf().markdown.document(document).unwrap()
        ));
        assert!(std::sync::Arc::ptr_eq(
            &uml,
            prepared.uml().syntax.document(document).unwrap()
        ));
        assert!(std::sync::Arc::ptr_eq(
            &structure,
            prepared
                .okf()
                .markdown
                .document(document)
                .unwrap()
                .structure()
        ));
        assert_eq!(
            prepared
                .okf()
                .catalog
                .document(document)
                .unwrap()
                .revision(),
            revision
        );
        assert_eq!(
            source
                .document(&BundlePath::parse("a.md").unwrap())
                .unwrap()
                .text(),
            authored
        );
    }
    let changed_source = waml::host::replace_document(
        &source,
        waml::source::SourceDocument::new(
            BundlePath::parse("a.md").unwrap(),
            authored.replace('é', "E"),
        ),
    )
    .unwrap();
    let changed = prepare_candidate(
        changed_source.clone(),
        Some(PreviousAnalyses {
            okf: prepared.okf(),
            uml: prepared.uml(),
        }),
        8,
    )
    .unwrap();
    let current_revision = changed.okf().catalog.document(document).unwrap().revision();
    let changed_catalog = changed.okf().catalog.clone();
    let changed_markdown = changed.okf().markdown.document(document).unwrap().clone();
    let changed_uml = changed.uml().syntax.document(document).unwrap().clone();
    let changed_structure = changed_markdown.structure().clone();
    let changed_pointer = changed_source
        .document(&BundlePath::parse("a.md").unwrap())
        .unwrap()
        .text()
        .as_ptr();
    let stale_document = SyntaxChangeBatch::new(CodeAction {
        title: "stale document".into(),
        basis: ActionBasis::Document {
            document,
            document_revision: revision,
            session_revision: 8,
        },
        changes: vec![VersionedDocumentChange {
            document,
            base_document_revision: current_revision,
            edits: vec![edit(heading, heading + 1, "Z")].into(),
        }]
        .into(),
    })
    .unwrap();
    let error = stale_document
        .lower(EditContext {
            source: &changed_source,
            okf_analysis: changed.okf(),
            session_revision: 8,
            uml: changed.uml(),
        })
        .unwrap_err();
    assert_eq!(
        error.reason,
        format!(
            "syntax action error: {:?}",
            ActionError::StaleDocument {
                document,
                expected: revision,
                actual: current_revision
            }
        )
    );
    assert!(std::sync::Arc::ptr_eq(
        &changed_catalog,
        &changed.okf().catalog
    ));
    assert!(std::sync::Arc::ptr_eq(
        &changed_markdown,
        changed.okf().markdown.document(document).unwrap()
    ));
    assert!(std::sync::Arc::ptr_eq(
        &changed_uml,
        changed.uml().syntax.document(document).unwrap()
    ));
    assert!(std::sync::Arc::ptr_eq(
        &changed_structure,
        changed
            .okf()
            .markdown
            .document(document)
            .unwrap()
            .structure()
    ));
    assert_eq!(
        changed.okf().catalog.document(document).unwrap().revision(),
        current_revision
    );
    assert_eq!(
        changed_source
            .document(&BundlePath::parse("a.md").unwrap())
            .unwrap()
            .text()
            .as_ptr(),
        changed_pointer
    );
    assert_eq!(
        source
            .document(&BundlePath::parse("a.md").unwrap())
            .unwrap()
            .text(),
        authored
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
