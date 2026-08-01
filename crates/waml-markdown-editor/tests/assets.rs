use std::sync::Arc;

use waml_markdown_editor::{
    layout::LayoutInvalidation,
    presentation::{
        compile_presentation, ApprovedImageSource, AssetEventOutcome, AssetRequestId,
        EmbeddedAssets, EmbeddedState, HighlighterRegistry, ImageAssetEvent, ImageAssetRequest,
        ImageMediaType, MarkdownAssetHost, PresentationItem, PresentationItemId, PresentationPlan,
        PresentationStyles,
    },
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, MarkdownSyntaxSnapshot, SourceText,
};

const SOURCE: &str = "![checker](checker.svg)\n";

#[derive(Default)]
struct FakeHost {
    requested: Vec<ImageAssetRequest>,
    cancelled: Vec<AssetRequestId>,
    events: Vec<ImageAssetEvent>,
}

impl MarkdownAssetHost for FakeHost {
    fn request_image(&mut self, request: ImageAssetRequest) {
        self.requested.push(request);
    }

    fn cancel_image(&mut self, request_id: AssetRequestId) {
        self.cancelled.push(request_id);
    }

    fn drain_events(&mut self) -> Vec<ImageAssetEvent> {
        std::mem::take(&mut self.events)
    }
}

fn plan_for(
    source: &str,
    revision: DocumentRevision,
) -> (Arc<MarkdownSyntaxSnapshot>, Arc<PresentationPlan>) {
    let snapshot = parse_markdown(
        revision,
        SourceText::new(source.to_owned()).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let plan = compile_presentation(
        &snapshot,
        &PresentationStyles::balanced(),
        &HighlighterRegistry::default(),
    )
    .unwrap();
    (snapshot, plan)
}

fn image_item(plan: &PresentationPlan) -> PresentationItemId {
    plan.items
        .iter()
        .find_map(|item| match item {
            PresentationItem::EmbeddedBlock { id, .. } => Some(*id),
            _ => None,
        })
        .expect("the fixture has one image")
}

fn checker() -> ApprovedImageSource {
    ApprovedImageSource::Bytes {
        cache_key: Arc::from("checker.svg"),
        media_type: ImageMediaType::Svg,
        data: Arc::from(std::fs::read("tests/fixtures/checker.svg").unwrap()),
        pixel_size: (96, 48),
    }
}

#[test]
fn reconciliation_requests_each_parsed_image_once_and_cancels_removed_items() {
    let (_, plan) = plan_for(SOURCE, DocumentRevision::INITIAL);
    let mut host = FakeHost::default();
    let mut assets = EmbeddedAssets::default();

    assets.reconcile(&mut host, &plan);
    assets.reconcile(&mut host, &plan);
    assert_eq!(
        host.requested.len(),
        1,
        "one request per image, not per pass"
    );
    assert_eq!(host.requested[0].destination.as_ref(), "checker.svg");
    assert_eq!(host.requested[0].revision, DocumentRevision::INITIAL);
    assert!(host.cancelled.is_empty());

    // The image disappears at the same revision: its request is cancelled.
    let (_, without_image) = plan_for(SOURCE, DocumentRevision::INITIAL);
    let mut stripped = (*without_image).clone();
    stripped.items = without_image
        .items
        .iter()
        .filter(|item| !matches!(item, PresentationItem::EmbeddedBlock { .. }))
        .cloned()
        .collect::<Vec<_>>()
        .into();
    assets.reconcile(&mut host, &stripped);
    assert_eq!(host.cancelled.len(), 1);
}

#[test]
fn a_completion_for_an_older_revision_or_another_item_is_ignored() {
    let (_, plan) = plan_for(SOURCE, DocumentRevision::new(8));
    let mut host = FakeHost::default();
    let mut assets = EmbeddedAssets::default();
    assets.reconcile(&mut host, &plan);
    let item = image_item(&plan);
    let request_id = host.requested[0].request_id;

    let stale = assets.apply_event(ImageAssetEvent::Ready {
        request_id,
        revision: DocumentRevision::new(7),
        item,
        source: checker(),
    });
    assert!(matches!(stale, AssetEventOutcome::IgnoredStale));

    let wrong_item = assets.apply_event(ImageAssetEvent::Ready {
        request_id,
        revision: DocumentRevision::new(8),
        item: PresentationItemId {
            fragment_ordinal: item.fragment_ordinal + 7,
            ..item
        },
        source: checker(),
    });
    assert!(matches!(wrong_item, AssetEventOutcome::IgnoredStale));

    let wrong_request = assets.apply_event(ImageAssetEvent::Ready {
        request_id: AssetRequestId(request_id.0 + 100),
        revision: DocumentRevision::new(8),
        item,
        source: checker(),
    });
    assert!(matches!(wrong_request, AssetEventOutcome::IgnoredStale));

    assert_eq!(assets.state(item), Some(&EmbeddedState::Loading));
}

#[test]
fn loading_failure_and_ready_measurements_are_exact() {
    let (_, plan) = plan_for(SOURCE, DocumentRevision::INITIAL);
    let mut host = FakeHost::default();
    let mut assets = EmbeddedAssets::default();
    assets.reconcile(&mut host, &plan);
    let item = image_item(&plan);
    let request_id = host.requested[0].request_id;

    // Loading: min(240, available) x 72.
    let loading = assets.measurements(600.0);
    assert_eq!(loading.blocks[0].size.x, 240.0);
    assert_eq!(loading.blocks[0].size.y, 72.0);
    assert_eq!(assets.measurements(120.0).blocks[0].size.x, 120.0);

    // Ready: intrinsic pixels as logical pixels, clamped to the content width.
    let outcome = assets.apply_event(ImageAssetEvent::Ready {
        request_id,
        revision: DocumentRevision::INITIAL,
        item,
        source: checker(),
    });
    assert!(matches!(
        outcome,
        AssetEventOutcome::Applied {
            invalidation: Some(LayoutInvalidation::BlockMeasurement(_))
        }
    ));
    let ready = assets.measurements(600.0);
    assert_eq!(ready.blocks[0].size.x, 96.0);
    assert_eq!(ready.blocks[0].size.y, 48.0);
    // Aspect ratio survives the width clamp.
    let narrow = assets.measurements(48.0);
    assert_eq!(narrow.blocks[0].size.x, 48.0);
    assert_eq!(narrow.blocks[0].size.y, 24.0);

    // A very tall image is clamped to 480 logical pixels, keeping its ratio.
    let mut tall_assets = EmbeddedAssets::default();
    let mut tall_host = FakeHost::default();
    tall_assets.reconcile(&mut tall_host, &plan);
    tall_assets.apply_event(ImageAssetEvent::Ready {
        request_id: tall_host.requested[0].request_id,
        revision: DocumentRevision::INITIAL,
        item,
        source: ApprovedImageSource::Bytes {
            cache_key: Arc::from("tall"),
            media_type: ImageMediaType::Png,
            data: Arc::from(&b""[..]),
            pixel_size: (100, 1000),
        },
    });
    let tall = tall_assets.measurements(600.0);
    assert_eq!(tall.blocks[0].size.y, 480.0);
    assert_eq!(tall.blocks[0].size.x, 48.0);

    // Failure: min(320, available) x 48.
    let mut failed_assets = EmbeddedAssets::default();
    let mut failed_host = FakeHost::default();
    failed_assets.reconcile(&mut failed_host, &plan);
    failed_assets.apply_event(ImageAssetEvent::Failed {
        request_id: failed_host.requested[0].request_id,
        revision: DocumentRevision::INITIAL,
        item,
        message: Arc::from("denied"),
    });
    let failed = failed_assets.measurements(600.0);
    assert_eq!(failed.blocks[0].size.x, 320.0);
    assert_eq!(failed.blocks[0].size.y, 48.0);
}

#[test]
fn only_a_failed_item_retries() {
    let (_, plan) = plan_for(SOURCE, DocumentRevision::INITIAL);
    let mut host = FakeHost::default();
    let mut assets = EmbeddedAssets::default();
    assets.reconcile(&mut host, &plan);
    let item = image_item(&plan);

    // Loading does not retry.
    assert!(!assets.retry(&mut host, &plan, item));
    assert_eq!(host.requested.len(), 1);

    assets.apply_event(ImageAssetEvent::Failed {
        request_id: host.requested[0].request_id,
        revision: DocumentRevision::INITIAL,
        item,
        message: Arc::from("denied"),
    });
    assert!(assets.retry(&mut host, &plan, item));
    assert_eq!(host.requested.len(), 2, "one new request");
    assert_eq!(assets.state(item), Some(&EmbeddedState::Loading));

    // Ready does not retry either.
    assets.apply_event(ImageAssetEvent::Ready {
        request_id: host.requested[1].request_id,
        revision: DocumentRevision::INITIAL,
        item,
        source: checker(),
    });
    assert!(!assets.retry(&mut host, &plan, item));
    assert_eq!(host.requested.len(), 2);
}

#[test]
fn the_literal_image_source_is_unchanged_in_every_asset_state() {
    let (snapshot, plan) = plan_for(SOURCE, DocumentRevision::INITIAL);
    let text_of_runs = |plan: &PresentationPlan| {
        plan.items
            .iter()
            .filter_map(|item| match item {
                PresentationItem::TextRun { range, .. } => {
                    Some(snapshot.text().slice(*range).unwrap_or_default().to_owned())
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    };
    let before = text_of_runs(&plan);
    assert_eq!(before, SOURCE);

    let mut host = FakeHost::default();
    let mut assets = EmbeddedAssets::default();
    assets.reconcile(&mut host, &plan);
    let item = image_item(&plan);
    assets.apply_event(ImageAssetEvent::Failed {
        request_id: host.requested[0].request_id,
        revision: DocumentRevision::INITIAL,
        item,
        message: Arc::from("denied"),
    });
    // Asset state lives beside the plan; the plan itself never changes.
    assert_eq!(text_of_runs(&plan), SOURCE);
    assert_eq!(assets.frame(&plan).items.len(), 1);
}

#[test]
fn a_new_revision_restarts_every_request() {
    let (_, first) = plan_for(SOURCE, DocumentRevision::new(1));
    let (_, second) = plan_for(SOURCE, DocumentRevision::new(2));
    let mut host = FakeHost::default();
    let mut assets = EmbeddedAssets::default();
    assets.reconcile(&mut host, &first);
    assets.reconcile(&mut host, &second);
    assert_eq!(host.cancelled.len(), 1);
    assert_eq!(host.requested.len(), 2);
    assert_eq!(host.requested[1].revision, DocumentRevision::new(2));
}
