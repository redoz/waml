use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    rc::Rc,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
};

use makepad_widgets::{
    decode_image_from_data, image_size_by_data, looks_like_svg, makepad_draw::svg::parse_svg,
    SignalToUI,
};
use waml::{analysis::WamlCodeRole, source::BundlePath};
use waml_markdown_editor::presentation::{
    ApprovedImageSource, AssetRequestId, CodeHighlightError, CodeHighlightHost,
    CodeHighlightRequest, CodeHighlightResult, CodeHighlightSpan, CodeTokenRole,
    HighlighterRegistry, ImageAssetEvent, ImageAssetRequest, MarkdownAssetHost, PresentationItemId,
    PresentationPlan,
};

use crate::editor_session::EditorSessionSnapshot;

pub struct WamlCodeHighlightHost {
    snapshot: Arc<EditorSessionSnapshot>,
}

impl WamlCodeHighlightHost {
    pub fn new(snapshot: Arc<EditorSessionSnapshot>) -> Self {
        Self { snapshot }
    }

    pub fn registry(snapshot: Arc<EditorSessionSnapshot>) -> HighlighterRegistry {
        let mut registry = HighlighterRegistry::default();
        registry.register("waml", Arc::new(Self::new(snapshot)));
        registry
    }
}

impl CodeHighlightHost for WamlCodeHighlightHost {
    fn highlight(
        &self,
        request: &CodeHighlightRequest,
    ) -> Result<CodeHighlightResult, CodeHighlightError> {
        let (document, markdown) = self
            .snapshot
            .markdown_snapshots
            .iter()
            .find(|(_, markdown)| {
                let queries = markdown.queries();
                queries
                    .island(request.owner)
                    .is_some_and(|island| island.content_range == request.content_range)
                    || queries
                        .fenced_code(request.owner)
                        .is_some_and(|fence| fence.content_range == request.content_range)
            })
            .ok_or_else(|| {
                CodeHighlightError::Host("WAML code island is not in the snapshot".into())
            })?;

        if request.revision != markdown.revision() {
            return Err(CodeHighlightError::StaleRevision {
                expected: markdown.revision(),
                actual: request.revision,
            });
        }

        let committed = self
            .snapshot
            .okf_analysis
            .markdown_snapshot(*document)
            .filter(|committed| Arc::ptr_eq(committed, markdown))
            .ok_or_else(|| {
                CodeHighlightError::Host(
                    "WAML code analysis is not available for this document revision".into(),
                )
            })?;
        debug_assert_eq!(committed.revision(), request.revision);

        let spans = self
            .snapshot
            .okf_analysis
            .code_spans(request.owner, request.content_range)
            .ok_or_else(|| {
                CodeHighlightError::Host("WAML code analysis rejected the island".into())
            })?
            .iter()
            .map(|span| CodeHighlightSpan {
                range: span.range,
                role: code_token_role(span.role),
            })
            .collect::<Vec<_>>();

        Ok(CodeHighlightResult {
            revision: request.revision,
            owner: request.owner,
            spans: spans.into(),
        })
    }
}

pub(crate) fn code_token_role(role: WamlCodeRole) -> CodeTokenRole {
    match role {
        WamlCodeRole::Keyword => CodeTokenRole::Keyword,
        WamlCodeRole::Type => CodeTokenRole::Type,
        WamlCodeRole::Property => CodeTokenRole::Property,
        WamlCodeRole::String => CodeTokenRole::String,
        WamlCodeRole::Number => CodeTokenRole::Number,
        WamlCodeRole::Comment => CodeTokenRole::Comment,
        WamlCodeRole::Punctuation => CodeTokenRole::Punctuation,
        WamlCodeRole::Invalid => CodeTokenRole::Invalid,
    }
}

pub type SharedMarkdownAssetHost = Rc<RefCell<EditorMarkdownAssetHost>>;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MarkdownAssetLeaseId(u64);

const DIRECT_LEASE: MarkdownAssetLeaseId = MarkdownAssetLeaseId(0);

pub struct MarkdownAssetLease {
    shared: SharedMarkdownAssetHost,
    id: MarkdownAssetLeaseId,
}

#[derive(Clone, Debug)]
pub enum MarkdownAssetPolicy {
    Native { canonical_root: Arc<PathBuf> },
    BrowserBundle,
}

impl MarkdownAssetPolicy {
    pub fn native(root: impl AsRef<Path>) -> std::io::Result<Self> {
        Ok(Self::Native {
            canonical_root: Arc::new(fs::canonicalize(root)?),
        })
    }
}

pub struct EditorMarkdownAssetHost {
    policy: MarkdownAssetPolicy,
    completed: BTreeMap<MarkdownAssetLeaseId, Vec<ImageAssetEvent>>,
    canceled: BTreeSet<(MarkdownAssetLeaseId, AssetRequestId)>,
    pending: BTreeMap<(MarkdownAssetLeaseId, AssetRequestId), PresentationItemId>,
    documents: BTreeMap<MarkdownAssetLeaseId, BTreeMap<PresentationItemId, BundlePath>>,
    completion_tx: Sender<(MarkdownAssetLeaseId, ImageAssetEvent)>,
    completion_rx: Receiver<(MarkdownAssetLeaseId, ImageAssetEvent)>,
    next_lease: u64,
}

impl EditorMarkdownAssetHost {
    pub fn new(policy: MarkdownAssetPolicy) -> Self {
        let (completion_tx, completion_rx) = mpsc::channel();
        Self {
            policy,
            completed: BTreeMap::new(),
            canceled: BTreeSet::new(),
            pending: BTreeMap::new(),
            documents: BTreeMap::new(),
            completion_tx,
            completion_rx,
            next_lease: 0,
        }
    }

    pub fn shared(policy: MarkdownAssetPolicy) -> SharedMarkdownAssetHost {
        Rc::new(RefCell::new(Self::new(policy)))
    }

    pub fn open_lease(shared: &SharedMarkdownAssetHost) -> MarkdownAssetLease {
        let id = {
            let mut host = shared.borrow_mut();
            host.next_lease = host.next_lease.wrapping_add(1);
            let id = MarkdownAssetLeaseId(host.next_lease);
            host.documents.insert(id, BTreeMap::new());
            id
        };
        MarkdownAssetLease {
            shared: shared.clone(),
            id,
        }
    }

    pub fn bind_item(&mut self, item: PresentationItemId, document: BundlePath) {
        self.documents
            .entry(DIRECT_LEASE)
            .or_default()
            .insert(item, document);
    }

    pub fn bind_presentation(&mut self, plan: &PresentationPlan, document: BundlePath) {
        self.reconcile_presentation(plan, document);
    }

    pub fn reconcile_presentation(&mut self, plan: &PresentationPlan, document: BundlePath) {
        self.reconcile_lease(DIRECT_LEASE, plan, document);
    }

    pub fn unbind_document(&mut self, document: &BundlePath) {
        self.unbind_lease_document(DIRECT_LEASE, document);
    }

    fn reconcile_lease(
        &mut self,
        lease: MarkdownAssetLeaseId,
        plan: &PresentationPlan,
        document: BundlePath,
    ) {
        let documents = self.documents.entry(lease).or_default();
        documents.retain(|_, bound| bound != &document);
        for item in plan.items.iter() {
            documents.insert(item.id(), document.clone());
        }
        self.cancel_retired_requests(lease);
    }

    fn unbind_lease_document(&mut self, lease: MarkdownAssetLeaseId, document: &BundlePath) {
        if let Some(documents) = self.documents.get_mut(&lease) {
            documents.retain(|_, bound| bound != document);
        }
        self.cancel_retired_requests(lease);
    }

    fn close_lease(&mut self, lease: MarkdownAssetLeaseId) {
        self.documents.remove(&lease);
        for event in self.completed.remove(&lease).unwrap_or_default() {
            let request_id = event_request_id(&event);
            self.pending.remove(&(lease, request_id));
            self.canceled.remove(&(lease, request_id));
        }
        for (pending_lease, request_id) in self.pending.keys() {
            if *pending_lease == lease {
                self.canceled.insert((lease, *request_id));
            }
        }
    }

    fn cancel_retired_requests(&mut self, lease: MarkdownAssetLeaseId) {
        let documents = self.documents.get(&lease);
        for ((pending_lease, request_id), item) in self.pending.iter() {
            if *pending_lease == lease && documents.map_or(true, |items| !items.contains_key(item))
            {
                self.canceled.insert((lease, *request_id));
            }
        }
    }
}

impl MarkdownAssetHost for EditorMarkdownAssetHost {
    fn request_image(&mut self, request: ImageAssetRequest) {
        self.request_image_for_lease(DIRECT_LEASE, request);
    }

    fn cancel_image(&mut self, request_id: AssetRequestId) {
        self.cancel_image_for_lease(DIRECT_LEASE, request_id);
    }

    fn drain_events(&mut self) -> Vec<ImageAssetEvent> {
        self.drain_events_for_lease(DIRECT_LEASE)
    }
}

impl EditorMarkdownAssetHost {
    fn request_image_for_lease(&mut self, lease: MarkdownAssetLeaseId, request: ImageAssetRequest) {
        self.canceled.remove(&(lease, request.request_id));
        self.pending
            .insert((lease, request.request_id), request.item);
        let document = self
            .documents
            .get(&lease)
            .and_then(|documents| documents.get(&request.item))
            .cloned();
        match &self.policy {
            MarkdownAssetPolicy::BrowserBundle => {
                self.completed.entry(lease).or_default().push(failed_event(
                    &request,
                    "local Markdown images are unavailable in browser bundles".into(),
                ))
            }
            MarkdownAssetPolicy::Native { canonical_root } => {
                let root = canonical_root.clone();
                let completion = self.completion_tx.clone();
                #[cfg(not(target_arch = "wasm32"))]
                std::thread::spawn(move || {
                    let event = complete_native_request(&root, document.as_ref(), &request);
                    if completion.send((lease, event)).is_ok() {
                        SignalToUI::set_ui_signal();
                    }
                });
                #[cfg(target_arch = "wasm32")]
                {
                    let _ = (root, document);
                    if completion
                        .send((
                            lease,
                            failed_event(
                                &request,
                                "native Markdown images are unavailable in browser bundles".into(),
                            ),
                        ))
                        .is_ok()
                    {
                        SignalToUI::set_ui_signal();
                    }
                }
            }
        }
    }

    fn cancel_image_for_lease(&mut self, lease: MarkdownAssetLeaseId, request_id: AssetRequestId) {
        if self.pending.contains_key(&(lease, request_id)) {
            self.canceled.insert((lease, request_id));
        }
    }

    fn drain_events_for_lease(&mut self, lease: MarkdownAssetLeaseId) -> Vec<ImageAssetEvent> {
        for (event_lease, event) in self.completion_rx.try_iter() {
            let request_id = event_request_id(&event);
            self.pending.remove(&(event_lease, request_id));
            if self.canceled.remove(&(event_lease, request_id)) {
                continue;
            }
            if event_lease == DIRECT_LEASE || self.documents.contains_key(&event_lease) {
                self.completed.entry(event_lease).or_default().push(event);
            }
        }
        let mut ready = Vec::new();
        for event in self.completed.remove(&lease).unwrap_or_default() {
            let request_id = event_request_id(&event);
            self.pending.remove(&(lease, request_id));
            if !self.canceled.remove(&(lease, request_id)) {
                ready.push(event);
            }
        }
        ready
    }
}

impl MarkdownAssetLease {
    pub fn id(&self) -> MarkdownAssetLeaseId {
        self.id
    }

    pub fn reconcile_presentation(&mut self, plan: &PresentationPlan, document: BundlePath) {
        self.shared
            .borrow_mut()
            .reconcile_lease(self.id, plan, document);
    }

    pub fn unbind_document(&mut self, document: &BundlePath) {
        self.shared
            .borrow_mut()
            .unbind_lease_document(self.id, document);
    }

    pub fn close(self) {
        // Consuming the lease runs `Drop`, which retires only this view.
    }
}

impl Drop for MarkdownAssetLease {
    fn drop(&mut self) {
        self.shared.borrow_mut().close_lease(self.id);
    }
}

impl MarkdownAssetHost for MarkdownAssetLease {
    fn request_image(&mut self, request: ImageAssetRequest) {
        self.shared
            .borrow_mut()
            .request_image_for_lease(self.id, request);
    }

    fn cancel_image(&mut self, request_id: AssetRequestId) {
        self.shared
            .borrow_mut()
            .cancel_image_for_lease(self.id, request_id);
    }

    fn drain_events(&mut self) -> Vec<ImageAssetEvent> {
        self.shared.borrow_mut().drain_events_for_lease(self.id)
    }
}

fn complete_native_request(
    canonical_root: &Path,
    document: Option<&BundlePath>,
    request: &ImageAssetRequest,
) -> ImageAssetEvent {
    let result = (|| {
        let document = document
            .ok_or_else(|| Arc::<str>::from("image item is not bound to a Markdown document"))?;
        let path = approved_path(canonical_root, document, &request.destination)?;
        let data = fs::read(&path).map_err(|error| stable_file_error("read", error))?;
        let pixel_size = image_dimensions(&path, &data)?;
        Ok(ApprovedImageSource::CanonicalFile {
            path: Arc::new(path),
            pixel_size,
        })
    })();
    match result {
        Ok(source) => ImageAssetEvent::Ready {
            request_id: request.request_id,
            revision: request.revision,
            item: request.item,
            source,
        },
        Err(message) => failed_event(request, message),
    }
}

fn failed_event(request: &ImageAssetRequest, message: Arc<str>) -> ImageAssetEvent {
    ImageAssetEvent::Failed {
        request_id: request.request_id,
        revision: request.revision,
        item: request.item,
        message,
    }
}

fn event_request_id(event: &ImageAssetEvent) -> AssetRequestId {
    match event {
        ImageAssetEvent::Ready { request_id, .. } | ImageAssetEvent::Failed { request_id, .. } => {
            *request_id
        }
    }
}

fn approved_path(
    canonical_root: &Path,
    document: &BundlePath,
    destination: &str,
) -> Result<PathBuf, Arc<str>> {
    if destination.is_empty()
        || destination.starts_with("//")
        || destination.starts_with(r"\\")
        || has_uri_scheme(destination)
    {
        return Err("Markdown image destination is not an approved local relative path".into());
    }
    let relative = Path::new(destination);
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| matches!(part, Component::Prefix(_) | Component::RootDir))
    {
        return Err("Markdown image destination is not an approved local relative path".into());
    }

    let document_path = Path::new(document.as_str());
    let document_dir = document_path.parent().unwrap_or_else(|| Path::new(""));
    let candidate = lexically_normalized(&canonical_root.join(document_dir).join(relative));
    let parent = candidate
        .parent()
        .ok_or_else(|| Arc::<str>::from("Markdown image destination has no parent directory"))?;
    let canonical_parent =
        fs::canonicalize(parent).map_err(|error| stable_file_error("resolve parent", error))?;
    if !canonical_parent.starts_with(canonical_root) {
        return Err("Markdown image destination leaves the bundle root".into());
    }
    let file_name = candidate
        .file_name()
        .ok_or_else(|| Arc::<str>::from("Markdown image destination has no file name"))?;
    let canonical = fs::canonicalize(canonical_parent.join(file_name))
        .map_err(|error| stable_file_error("resolve file", error))?;
    if !canonical.starts_with(canonical_root) {
        return Err("Markdown image destination leaves the bundle root".into());
    }
    Ok(canonical)
}

/// Collapse `.` and `..` textually, without consulting the filesystem.
///
/// `fs::canonicalize` is called below on this path's parent, and the two
/// platforms disagree about what that means for a `..` segment. Windows
/// normalises `..` before the syscall, so `<root>/docs/..` resolves to `<root>`
/// whether or not `docs` exists. Linux uses `realpath`, which requires every
/// component to exist and so fails with `NotFound` on the same input. Without
/// this, a Markdown image reaching upward with `../` resolves on Windows and
/// breaks on Linux whenever an intermediate directory is absent.
///
/// Collapsing here rather than trusting the OS makes both platforms agree.
/// It is safe against traversal because the caller still canonicalises the
/// result and rejects anything that escapes the bundle root -- this only
/// decides which path gets that check, never whether it happens. A `..` with
/// nothing to pop is kept, so the containment check still sees the escape.
fn lexically_normalized(path: &Path) -> PathBuf {
    let mut parts: Vec<Component<'_>> = Vec::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => match parts.last() {
                Some(Component::Normal(_)) => {
                    parts.pop();
                }
                _ => parts.push(part),
            },
            other => parts.push(other),
        }
    }
    parts.iter().collect()
}

fn has_uri_scheme(destination: &str) -> bool {
    let Some((scheme, _)) = destination.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme.as_bytes()[0].is_ascii_alphabetic()
        && scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

fn image_dimensions(path: &Path, data: &[u8]) -> Result<(u32, u32), Arc<str>> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| Arc::<str>::from("Markdown image has no supported file extension"))?;
    let dimensions = match extension.as_str() {
        "png" => {
            let (width, height) = image_size_by_data(data, path)
                .map_err(|_| Arc::<str>::from("PNG image dimensions are invalid"))?;
            checked_dimensions(width, height)?
        }
        "jpg" | "jpeg" => {
            let image = decode_image_from_data(data)
                .map_err(|_| Arc::<str>::from("JPEG image data is invalid"))?;
            checked_dimensions(image.width, image.height)?
        }
        "svg" => {
            if !looks_like_svg(data) {
                return Err("SVG image data has no SVG root".into());
            }
            let text = std::str::from_utf8(data)
                .map_err(|_| Arc::<str>::from("SVG image data is not UTF-8"))?;
            let (width, height) = parse_svg(text).logical_size();
            if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
                return Err("SVG image logical size is invalid".into());
            }
            checked_dimensions(width.round() as usize, height.round() as usize)?
        }
        _ => return Err("Markdown image file type is not supported".into()),
    };
    Ok(dimensions)
}

fn checked_dimensions(width: usize, height: usize) -> Result<(u32, u32), Arc<str>> {
    if width == 0 || height == 0 {
        return Err("Markdown image dimensions must be positive".into());
    }
    Ok((
        u32::try_from(width).map_err(|_| Arc::<str>::from("Markdown image width is too large"))?,
        u32::try_from(height)
            .map_err(|_| Arc::<str>::from("Markdown image height is too large"))?,
    ))
}

fn stable_file_error(operation: &str, error: std::io::Error) -> Arc<str> {
    Arc::from(format!(
        "could not {operation} Markdown image ({:?})",
        error.kind()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::source::SourceBundle;
    use waml_markdown_editor::{
        presentation::{
            compile_presentation, PresentationItem, PresentationRole, PresentationStyles, TextRole,
        },
        syntax::DocumentRevision,
    };
    use waml_syntax::SyntaxIdentity;

    fn item(value: u64) -> PresentationItemId {
        PresentationItemId {
            owner: SyntaxIdentity::from_raw_for_test(value),
            role: PresentationRole::Text(TextRole::Body),
            fragment_ordinal: 0,
        }
    }

    fn request(destination: &str) -> ImageAssetRequest {
        request_for(item(3), destination)
    }

    fn request_for(item: PresentationItemId, destination: &str) -> ImageAssetRequest {
        ImageAssetRequest {
            request_id: AssetRequestId(17),
            revision: DocumentRevision::new(9),
            item,
            source_range: waml_markdown_editor::syntax::TextRange::new(
                waml_markdown_editor::syntax::TextSize::new(0),
                waml_markdown_editor::syntax::TextSize::new(0),
            )
            .unwrap(),
            destination: Arc::from(destination),
        }
    }

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/markdown-assets")
    }

    fn wait_for_events(host: &mut EditorMarkdownAssetHost) -> Vec<ImageAssetEvent> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let events = host.drain_events();
            if !events.is_empty() || std::time::Instant::now() >= deadline {
                return events;
            }
            std::thread::yield_now();
        }
    }

    fn wait_for_lease_events(lease: &mut MarkdownAssetLease) -> Vec<ImageAssetEvent> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let events = lease.drain_events();
            if !events.is_empty() || std::time::Instant::now() >= deadline {
                return events;
            }
            std::thread::yield_now();
        }
    }

    #[test]
    fn lexical_normalization_collapses_dot_segments_without_touching_the_filesystem() {
        let root = fixture_root();
        // The case that broke CI: `docs` does not exist on disk, so on Linux
        // `canonicalize` of the un-normalised parent fails where Windows had
        // silently collapsed it. This assertion runs identically on both.
        assert_eq!(
            lexically_normalized(&root.join("docs").join("../tiny.svg")),
            root.join("tiny.svg")
        );
        assert_eq!(
            lexically_normalized(&root.join("./a/./b/../c.svg")),
            root.join("a").join("c.svg")
        );
        // A `..` with nothing left to pop must survive, so the caller's
        // containment check still sees the escape rather than a tidied path.
        let escaped = lexically_normalized(&root.join("../../outside.svg"));
        assert!(!escaped.starts_with(&root), "{escaped:?} must escape root");
    }

    #[test]
    fn host_native_asset_preserves_identity_and_reads_svg_dimensions() {
        let mut host = EditorMarkdownAssetHost::new(
            MarkdownAssetPolicy::native(fixture_root()).expect("fixture root must exist"),
        );
        host.bind_item(item(3), BundlePath::parse("docs/page.md").unwrap());
        host.request_image(request("../tiny.svg"));

        let events = wait_for_events(&mut host);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ImageAssetEvent::Ready {
                request_id,
                revision,
                item: completed_item,
                source,
            } => {
                assert_eq!(*request_id, AssetRequestId(17));
                assert_eq!(*revision, DocumentRevision::new(9));
                assert_eq!(*completed_item, item(3));
                assert_eq!(source.pixel_size(), (7, 5));
            }
            ImageAssetEvent::Failed { message, .. } => panic!("unexpected failure: {message}"),
        }
    }

    #[test]
    fn host_native_asset_rejects_remote_absolute_and_traversal_destinations() {
        let policy = MarkdownAssetPolicy::native(fixture_root()).unwrap();
        for destination in [
            "https://example.test/image.png",
            "http://example.test/image.png",
            "//example.test/image.png",
            r"C:\image.png",
            "../../../outside.svg",
        ] {
            let mut host = EditorMarkdownAssetHost::new(policy.clone());
            host.bind_item(item(3), BundlePath::parse("docs/page.md").unwrap());
            host.request_image(request(destination));
            assert!(matches!(
                wait_for_events(&mut host).as_slice(),
                [ImageAssetEvent::Failed { .. }]
            ));
        }
    }

    #[test]
    fn host_native_asset_rejects_invalid_svg_logical_size() {
        let mut host = EditorMarkdownAssetHost::new(
            MarkdownAssetPolicy::native(fixture_root()).expect("fixture root must exist"),
        );
        host.bind_item(item(3), BundlePath::parse("page.md").unwrap());
        host.request_image(request("invalid-size.svg"));

        assert!(matches!(
            wait_for_events(&mut host).as_slice(),
            [ImageAssetEvent::Failed { message, .. }]
                if message.as_ref() == "SVG image logical size is invalid"
        ));
    }

    #[test]
    fn host_browser_bundle_fails_locally_with_stable_identity() {
        let mut host = EditorMarkdownAssetHost::new(MarkdownAssetPolicy::BrowserBundle);
        host.request_image(request("tiny.svg"));
        let events = host.drain_events();
        assert!(matches!(
            events.as_slice(),
            [ImageAssetEvent::Failed {
                request_id: AssetRequestId(17),
                revision,
                item: failed_item,
                message,
            }] if *revision == DocumentRevision::new(9)
                && *failed_item == item(3)
                && message.as_ref() == "local Markdown images are unavailable in browser bundles"
        ));
    }

    #[test]
    fn host_cancellation_removes_queued_completion() {
        let mut host = EditorMarkdownAssetHost::new(MarkdownAssetPolicy::BrowserBundle);
        host.request_image(request("tiny.svg"));
        host.cancel_image(AssetRequestId(17));
        assert!(host.drain_events().is_empty());
        assert!(host.pending.is_empty());
        assert!(host.canceled.is_empty());
    }

    #[test]
    fn host_unbind_cancels_async_work_and_reclaims_lifecycle_state() {
        let document = BundlePath::parse("page.md").unwrap();
        let mut host = EditorMarkdownAssetHost::new(
            MarkdownAssetPolicy::native(fixture_root()).expect("fixture root must exist"),
        );
        host.bind_item(item(3), document.clone());
        host.request_image(request("tiny.svg"));
        host.unbind_document(&document);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !host.pending.is_empty() && std::time::Instant::now() < deadline {
            assert!(host.drain_events().is_empty());
            std::thread::yield_now();
        }
        assert!(host
            .documents
            .get(&DIRECT_LEASE)
            .map_or(true, BTreeMap::is_empty));
        assert!(host.pending.is_empty());
        assert!(host.canceled.is_empty());
    }

    #[test]
    fn host_leases_isolate_two_live_views_of_the_same_document() {
        let source = "# Page\n\n![tiny](tiny.svg)\n";
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(SourceBundle::try_from_pairs([("page.md", source)]).unwrap())
            .unwrap();
        let snapshot = session.snapshot();
        let markdown = snapshot.markdown_snapshots.values().next().unwrap();
        let plan = compile_presentation(
            markdown,
            &PresentationStyles,
            &HighlighterRegistry::default(),
        )
        .unwrap();
        let image = plan
            .items
            .iter()
            .find_map(|item| match item {
                PresentationItem::EmbeddedBlock { id, .. } => Some(*id),
                _ => None,
            })
            .expect("the fixture must contain an image item");
        let document = BundlePath::parse("page.md").unwrap();
        let shared = EditorMarkdownAssetHost::shared(
            MarkdownAssetPolicy::native(fixture_root()).expect("fixture root must exist"),
        );
        let mut first = EditorMarkdownAssetHost::open_lease(&shared);
        let mut second = EditorMarkdownAssetHost::open_lease(&shared);
        first.reconcile_presentation(&plan, document.clone());
        second.reconcile_presentation(&plan, document.clone());
        first.request_image(request_for(image, "tiny.svg"));
        second.request_image(request_for(image, "tiny.svg"));

        first.unbind_document(&document);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while shared
            .borrow()
            .pending
            .contains_key(&(first.id(), AssetRequestId(17)))
            && std::time::Instant::now() < deadline
        {
            assert!(first.drain_events().is_empty());
            std::thread::yield_now();
        }
        let second_events = wait_for_lease_events(&mut second);
        assert!(matches!(
            second_events.as_slice(),
            [ImageAssetEvent::Ready { request_id: AssetRequestId(17), item, .. }]
                if *item == image
        ));
        let host = shared.borrow();
        assert!(!host
            .documents
            .get(&first.id())
            .unwrap()
            .contains_key(&image));
        assert_eq!(
            host.documents
                .get(&second.id())
                .and_then(|items| items.get(&image)),
            Some(&document)
        );
        assert!(!host.pending.contains_key(&(first.id(), AssetRequestId(17))));
        assert!(!host.canceled.contains(&(first.id(), AssetRequestId(17))));
    }

    #[test]
    fn host_highlighter_uses_matching_committed_snapshot() {
        let source =
            "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- count: Number {0..42}\n";
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(SourceBundle::try_from_pairs([("order.md", source)]).unwrap())
            .unwrap();
        let snapshot = session.snapshot();
        let markdown = snapshot.markdown_snapshots.values().next().unwrap();
        let island = markdown.structure().islands.first().unwrap();
        let request = CodeHighlightRequest {
            revision: markdown.revision(),
            owner: island.owner,
            language: Arc::from("waml"),
            content_range: island.content_range,
        };

        let result = WamlCodeHighlightHost::new(snapshot)
            .highlight(&request)
            .expect("the committed island must highlight");
        assert_eq!(result.revision, request.revision);
        assert_eq!(result.owner, request.owner);
        assert!(result.spans.iter().all(|span| {
            request.content_range.start() <= span.range.start()
                && span.range.end() <= request.content_range.end()
        }));
        assert!(result
            .spans
            .iter()
            .any(|span| span.role == CodeTokenRole::Property));
    }

    #[test]
    fn host_highlighter_classifies_waml_fenced_content() {
        let source = "# Example\n\n```waml\n## Attributes\n- count: Number {0..42}\n```\n";
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(SourceBundle::try_from_pairs([("example.md", source)]).unwrap())
            .unwrap();
        let snapshot = session.snapshot();
        let markdown = snapshot.markdown_snapshots.values().next().unwrap();
        let plan = compile_presentation(
            markdown,
            &PresentationStyles,
            &WamlCodeHighlightHost::registry(snapshot.clone()),
        )
        .unwrap();

        assert!(plan.items.iter().any(|item| matches!(
            item,
            PresentationItem::TextRun {
                id: PresentationItemId {
                    role: PresentationRole::Text(TextRole::CodeToken(CodeTokenRole::Property)),
                    ..
                },
                ..
            }
        )));
        assert!(plan.diagnostics.is_empty(), "{:#?}", plan.diagnostics);
    }

    #[test]
    fn host_highlighter_rejects_stale_revision() {
        let source = "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- count: Number\n";
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(SourceBundle::try_from_pairs([("order.md", source)]).unwrap())
            .unwrap();
        let snapshot = session.snapshot();
        let markdown = snapshot.markdown_snapshots.values().next().unwrap();
        let island = markdown.structure().islands.first().unwrap();
        let request = CodeHighlightRequest {
            revision: DocumentRevision::new(markdown.revision().get() + 1),
            owner: island.owner,
            language: Arc::from("waml"),
            content_range: island.content_range,
        };

        assert!(matches!(
            WamlCodeHighlightHost::new(snapshot).highlight(&request),
            Err(CodeHighlightError::StaleRevision { .. })
        ));
    }
}
