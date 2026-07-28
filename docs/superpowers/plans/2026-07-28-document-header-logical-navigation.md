# Document Header and Logical Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a shared, collapsible document header and route tree rows, breadcrumb segments, and rendered Markdown links through one logical navigation policy supporting documents, directories, fragments, and external HTTP(S) URLs.

**Architecture:** Phase 1 adds heading-anchor ownership and fragment scrolling to the sibling Makepad `Markdown` widget, because waml-editor must not reconstruct renderer geometry. Phase 2 adds pure logical targets/resolution, a typed `BodyChrome` header contribution, one shared `DocumentHeader`, and one application navigation handler; views and widgets emit intent while `App` alone mutates tabs, navigator state, docks, status, and the platform browser adapter.

**Tech Stack:** Rust 1.80, Makepad widgets/live DSL, pulldown-cmark 0.12, waml OKF/UML projections, inline Rust unit tests, `cargo test`, `cargo clippy`, RTK command proxy.

## Global Constraints

- Breadcrumbs use authored logical hierarchy and titles only; never split concept IDs, inspect `NavView`, or expose backing filesystem paths.
- `DocumentHeader` is one shell-owned widget; document views contribute only `DocumentHeaderChrome { breadcrumb: bool, right_dock: Option<Icon> }`.
- The header height is exactly `30.0` px when it has breadcrumb segments or a right-dock toggle, and `0.0` otherwise.
- Tree single-click, breadcrumb, and Markdown document activation use preview disposition; tree double-click alone requests persistent disposition.
- Directory activation always reaches `App`, which toggles the matching logical tree folder exactly once without changing the active document or navigator scope; root `/` opens the tree dock and restores root scope.
- Logical resolution never falls back to the filesystem and rejects unsupported schemes, malformed targets, missing logical nodes, and traversal above bundle root.
- Only `http:` and `https:` are external navigation targets.
- External URL tests use a fake `ExternalUrlAdapter` and never launch a real browser.
- A narrow floating inspector starts below a visible document header and reclaims the full height when the header is absent.
- The start screen has no document header.
- Keep both repository worktrees cleanly reviewable: commit Makepad changes in `C:\dev\makepad`, then pin that exact commit in waml-editor before relying on the new API.

---

## File Map and Phase Boundary

The cross-repository Makepad work contains two independently reviewable prerequisites for the same vertical feature: fragment navigation needs renderer-owned heading geometry, while one directory mutation path needs FileTree to emit folder clicks without pre-mutating fold state. They stay in this plan as separate Makepad tasks/commits; the resulting Makepad HEAD is published (with execution-time authorization) and pinned before the first waml compile.

**Phase 1 — Makepad widget prerequisites (`C:\dev\makepad`)**

- Modify `widgets/src/markdown.rs` — derive GitHub-style heading slugs, retain drawn heading anchors, own optional Markdown scrolling, expose `MarkdownRef::scroll_to_fragment`, and test original href actions plus anchor behavior.
- Modify `widgets/src/file_tree.rs` — add a default-preserving `auto_toggle_folders` live option so waml can emit folder intent without Makepad mutating fold state first.

**Phase 2 — waml-editor vertical feature (`C:\dev\waml`)**

- Modify `Cargo.toml` and `crates/waml-editor/Cargo.toml` — add the `url` parser and pin the published Phase 1 Makepad HEAD before any waml code consumes the new widget APIs.
- Create `crates/waml-editor/src/navigation.rs` — resolved target types, dispositions, raw/resolved intent, typed errors, pure breadcrumb query, pure href resolver, and navigation-policy test helpers.
- Create `crates/waml-editor/src/platform_browser.rs` — injectable external URL adapter and production platform launcher.
- Create `crates/waml-editor/src/document_header.rs` — shared header widget, breadcrumb elision/hit geometry, right-dock button, and typed actions.
- Modify `crates/waml-editor/src/main.rs` — declare and register the new modules/widgets.
- Modify `crates/waml-editor/src/doc_view.rs` — add `DocumentHeaderChrome`, add navigation to `ViewOutcome`, expose shared Markdown link/fragment operations, and apply typed header chrome.
- Modify `crates/waml-editor/src/markdown_surface.rs` — consume `MarkdownAction::LinkNavigated` and delegate fragment scrolling.
- Modify `crates/waml-editor/src/source_view.rs` — opt into breadcrumbs and emit raw Markdown-link intent with its current logical concept ID.
- Modify `crates/waml-editor/src/generic_okf_view.rs` — opt into breadcrumbs and emit the same shared Markdown-link intent.
- Modify `crates/waml-editor/src/tree_panel.rs` — emit resolved directory/document intents and expose one directory command that can toggle a folder by logical address.
- Modify `crates/waml-editor/src/app/actions.rs` — route tree/header/view intents into one application-owned navigation entry point and move the right-dock click observer to the header.
- Modify `crates/waml-editor/src/app.rs` — mount/synchronize the header, execute resolved navigation, apply pending fragments after draw, manage root semantics and status, and enforce narrow overlay geometry.
- Modify `crates/waml-editor/src/dock.rs` — add the pure inspector overlay-top geometry calculation and tests.
- Modify `crates/waml-editor/src/doc_tabs.rs` — remove all right-dock-button presence/rule-overshoot coupling.
- Modify `crates/waml-editor/src/statusbar.rs` — display concise navigation/browser/fragment failure messages.

---

### Task 1: Add Renderer-Owned Heading Anchors and Fragment Scrolling

**Files:**

- Modify: `C:\dev\makepad\widgets\src\markdown.rs`

**Interfaces:**

- Consumes: existing `MarkdownAction::LinkNavigated(String)`, `pulldown_cmark::Event`, `ScrollBars`, and `TextFlow`.
- Produces:

```rust
fn heading_slug(text: &str, prior: &mut HashMap<String, usize>) -> String;
fn fragment_scroll_y(anchors: &[(String, f64)], fragment: &str) -> Option<f64>;

impl MarkdownRef {
    pub fn scroll_to_fragment(&self, cx: &mut Cx, fragment: &str) -> bool;
}
```

`Markdown` additionally owns `heading_anchors: Vec<(String, f64)>` and a vertical `ScrollBars`. Anchor `f64` values are content-local y positions captured from `cx.turtle().pos().y` when each heading starts.

- [ ] **Step 1: Write slug and duplicate-heading tests**

Add tests to `widgets/src/markdown.rs`:

```rust
#[test]
fn github_heading_slugs_are_stable_and_suffix_duplicates() {
    let mut prior = HashMap::new();
    assert_eq!(heading_slug("Customer Overview!", &mut prior), "customer-overview");
    assert_eq!(heading_slug("Customer   Overview", &mut prior), "customer-overview-1");
    assert_eq!(heading_slug("API: `create()`", &mut prior), "api-create");
}

#[test]
fn fragment_lookup_uses_the_recorded_slug() {
    let anchors = vec![
        ("overview".into(), 12.0),
        ("overview-1".into(), 84.0),
    ];
    assert_eq!(fragment_scroll_y(&anchors, "overview-1"), Some(84.0));
    assert_eq!(fragment_scroll_y(&anchors, "missing"), None);
}
```

- [ ] **Step 2: Run the focused tests and verify they fail**

Run:

```powershell
cd C:\dev\makepad
rtk cargo test -p makepad-widgets markdown::tests::github_heading_slugs_are_stable_and_suffix_duplicates --lib
```

Expected: FAIL because `heading_slug`, `heading_anchors`, and `fragment_y` do not exist.

- [ ] **Step 3: Implement deterministic slug derivation and anchor collection**

Add `use std::collections::HashMap;`, define `heading_slug`, and update `process_markdown_doc` with explicit heading state:

```rust
fn heading_slug(text: &str, prior: &mut HashMap<String, usize>) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;
    for ch in text.chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            if pending_dash && !slug.is_empty() && !slug.ends_with('-') {
                slug.push('-');
            }
            pending_dash = false;
            slug.push(ch);
        } else if ch.is_whitespace() {
            pending_dash = true;
        }
    }
    let count = prior.entry(slug.clone()).or_insert(0);
    let resolved = if *count == 0 {
        slug
    } else {
        format!("{slug}-{count}")
    };
    *count += 1;
    resolved
}
```

At the start of each render clear `self.heading_anchors`; on `Start(Tag::Heading)` record the content y and begin a `String`; append `Text`, `Code`, `SoftBreak`, and `HardBreak` content while a heading is active; on `End(TagEnd::Heading(_))`, derive the slug and push `(slug, y)`. Keep all existing rendering branches unchanged.

- [ ] **Step 4: Run the slug tests**

Run:

```powershell
cd C:\dev\makepad
rtk cargo test -p makepad-widgets markdown::tests --lib
```

Expected: PASS, including punctuation removal, whitespace collapse, and duplicate suffix ordering.

- [ ] **Step 5: Add the failing scroll and href-action integration tests**

Add a test-only helper that applies the same scroll decision used by the widget:

```rust
fn fragment_scroll_y(anchors: &[(String, f64)], fragment: &str) -> Option<f64> {
    anchors
        .iter()
        .find(|(slug, _)| slug == fragment)
        .map(|(_, y)| *y)
}

#[test]
fn fragment_scroll_finds_anchor_and_missing_anchor_is_harmless() {
    let anchors = vec![("intro".into(), 0.0), ("details".into(), 140.0)];
    assert_eq!(fragment_scroll_y(&anchors, "details"), Some(140.0));
    assert_eq!(fragment_scroll_y(&anchors, "unknown"), None);
}

#[test]
fn markdown_link_action_preserves_the_original_href() {
    let action = MarkdownAction::LinkNavigated("../customer.md#orders".into());
    assert!(matches!(
        action,
        MarkdownAction::LinkNavigated(href) if href == "../customer.md#orders"
    ));
}
```

- [ ] **Step 6: Make `Markdown` own optional scrolling and expose the ref method**

Add `#[live] scroll_bars: ScrollBars`, wrap the existing `TextFlow::begin/process/end` draw in `scroll_bars.begin/end`, and forward events to `scroll_bars.handle_event` before `text_flow.handle_event`. Configure the widget so an unconfigured scrollbar preserves current Fit behavior, while `scroll_bar_y: ScrollBar{}` enables a bounded vertical viewport.

Implement:

```rust
impl Markdown {
    fn fragment_y(&self, fragment: &str) -> Option<f64> {
        fragment_scroll_y(&self.heading_anchors, fragment)
    }
}

impl MarkdownRef {
    pub fn scroll_to_fragment(&self, cx: &mut Cx, fragment: &str) -> bool {
        let Some(mut inner) = self.borrow_mut() else {
            return false;
        };
        let Some(y) = inner.fragment_y(fragment) else {
            return false;
        };
        let x = inner.scroll_bars.get_scroll_pos().x;
        inner.scroll_bars.set_scroll_pos(cx, dvec2(x, y));
        inner.redraw(cx);
        true
    }
}
```

- [ ] **Step 7: Run Makepad verification**

Run:

```powershell
cd C:\dev\makepad
rtk cargo test -p makepad-widgets markdown::tests --lib
rtk cargo clippy -p makepad-widgets --lib -- -D warnings
```

Expected: both commands PASS.

- [ ] **Step 8: Commit the Makepad prerequisite**

```powershell
cd C:\dev\makepad
rtk git add widgets/src/markdown.rs
rtk git commit -m "feat(markdown): add fragment anchors"
rtk git rev-parse HEAD
```

Expected: a reviewable Markdown commit that Task 3 publishes and pins together with Task 2.

---

### Task 2: Add a Default-Preserving FileTree Folder-Toggle Opt-Out

**Files:**

- Modify: `C:\dev\makepad\widgets\src\file_tree.rs`

**Interfaces:**

- Consumes: existing `FileTreeNodeAction::{Opening, Closing, WasClicked}` and `FileTreeAction::FolderClicked(LiveId)`.
- Produces:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FolderPressTransition {
    Opening,
    Closing,
}

fn folder_press_transition(
    auto_toggle_folders: bool,
    currently_open: bool,
) -> Option<FolderPressTransition>;

fn apply_folder_transition(
    open_nodes: &mut HashSet<LiveId>,
    node_id: LiveId,
    transition: Option<FolderPressTransition>,
);
```

`FileTree` gains:

```rust
#[live(true)]
auto_toggle_folders: bool,
```

and passes this value into `FileTreeNode::handle_event`. The default `true` preserves every existing Makepad consumer. With `false`, a folder press still emits `WasClicked`, which becomes `FileTreeAction::FolderClicked`, but emits neither `Opening` nor `Closing` and does not play the folder-open animator.

- [ ] **Step 1: Write focused default and opt-out tests**

Add to `widgets/src/file_tree.rs`:

```rust
#[test]
fn folder_press_defaults_to_existing_toggle_transition() {
    let id = LiveId::from_str("/sales");
    let mut open_nodes = HashSet::new();
    apply_folder_transition(
        &mut open_nodes,
        id,
        folder_press_transition(true, false),
    );
    assert!(open_nodes.contains(&id));
    apply_folder_transition(
        &mut open_nodes,
        id,
        folder_press_transition(true, true),
    );
    assert!(!open_nodes.contains(&id));
}

#[test]
fn folder_press_opt_out_emits_no_fold_transition() {
    let id = LiveId::from_str("/sales");
    let mut open_nodes = HashSet::new();
    let transition = folder_press_transition(false, false);
    apply_folder_transition(&mut open_nodes, id, transition);
    let emitted = FileTreeAction::FolderClicked(id);
    assert!(open_nodes.is_empty());
    assert!(matches!(
        emitted,
        FileTreeAction::FolderClicked(clicked) if clicked == id
    ));
}

#[test]
fn folder_clicked_is_independent_of_fold_transition() {
    let node_id = LiveId::from_str("/sales");
    let emitted = FileTreeAction::FolderClicked(node_id);
    assert!(matches!(
        emitted,
        FileTreeAction::FolderClicked(id) if id == node_id
    ));
    assert_eq!(folder_press_transition(false, false), None);
}
```

- [ ] **Step 2: Run the focused tests and verify failure**

Run:

```powershell
cd C:\dev\makepad
rtk cargo test -p makepad-widgets file_tree::tests::folder_press --lib
```

Expected: FAIL because `FolderPressTransition` and `folder_press_transition` do not exist.

- [ ] **Step 3: Implement the pure transition decision**

```rust
fn folder_press_transition(
    auto_toggle_folders: bool,
    currently_open: bool,
) -> Option<FolderPressTransition> {
    auto_toggle_folders.then_some(if currently_open {
        FolderPressTransition::Closing
    } else {
        FolderPressTransition::Opening
    })
}

fn apply_folder_transition(
    open_nodes: &mut HashSet<LiveId>,
    node_id: LiveId,
    transition: Option<FolderPressTransition>,
) {
    match transition {
        Some(FolderPressTransition::Opening) => {
            open_nodes.insert(node_id);
        }
        Some(FolderPressTransition::Closing) => {
            open_nodes.remove(&node_id);
        }
        None => {}
    }
}
```

- [ ] **Step 4: Gate FileTree’s internal fold mutation**

Add `auto_toggle_folders: bool` to `FileTreeNode::handle_event`. In the folder `Hit::FingerDown` branch, call `folder_press_transition`; only when it returns `Some` play `ids!(open.on/off)` and push `FileTreeNodeAction::Opening/Closing`. Always push `FileTreeNodeAction::WasClicked`. Pass `self.auto_toggle_folders` from `FileTree::handle_event` into every node call. Use `apply_folder_transition` in the existing `Opening`/`Closing` action branches, then keep the mapping from `WasClicked` to `FolderClicked`.

- [ ] **Step 5: Run Makepad FileTree verification**

Run:

```powershell
cd C:\dev\makepad
rtk cargo test -p makepad-widgets file_tree::tests --lib
rtk cargo clippy -p makepad-widgets --lib -- -D warnings
```

Expected: PASS; default behavior still yields `Opening`/`Closing`, while opt-out yields only the click action.

- [ ] **Step 6: Commit the independent FileTree change**

```powershell
cd C:\dev\makepad
rtk git add widgets/src/file_tree.rs
rtk git commit -m "feat(file-tree): allow app-owned folder toggles"
```

Expected: a second reviewable Makepad commit on top of Task 1.

---

### Task 3: Publish and Pin Makepad, Then Define Canonical Breadcrumbs and Logical Link Resolution

**Files:**

- Create: `crates/waml-editor/src/navigation.rs`
- Modify: `crates/waml-editor/src/main.rs`
- Modify: `Cargo.toml`
- Modify: `crates/waml-editor/Cargo.toml`
- Modify: `Cargo.lock`

**Interfaces:**

- Consumes: `crate::tree::build_tree`, `waml::okf::{Bundle, DirectoryAddress}`, `waml::uml::Projection`, and `url::Url`.
- Produces:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavigationTarget {
    Document { concept_id: String, fragment: Option<String> },
    Directory { address: String },
    ExternalUrl(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenDisposition { Preview, Persistent }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BreadcrumbSegment {
    pub title: String,
    pub target: NavigationTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavigationIntent {
    Resolved {
        target: NavigationTarget,
        disposition: OpenDisposition,
    },
    MarkdownLink { current_concept_id: String, href: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavigationError {
    MalformedTarget(String),
    UnsupportedScheme(String),
    EscapesBundle,
    MissingDocument(String),
    MissingDirectory(String),
}

pub fn breadcrumb_for(
    bundle: &waml::okf::Bundle,
    uml: &waml::uml::Projection,
    concept_id: &str,
) -> Option<Vec<BreadcrumbSegment>>;

pub fn resolve_link(
    bundle: &waml::okf::Bundle,
    current_concept_id: &str,
    href: &str,
) -> Result<NavigationTarget, NavigationError>;

impl NavigationError {
    pub fn status_message(&self) -> String;
}
```

- [ ] **Step 1: Publish the two Makepad prerequisite commits**

This external push requires execution-time authorization. After approval, run:

```powershell
cd C:\dev\makepad
rtk git log --oneline -2
rtk git push origin HEAD:refs/heads/waml-document-navigation-prereqs
rtk git rev-parse HEAD
```

Expected: the log shows Task 2’s FileTree commit immediately above Task 1’s Markdown commit, the push succeeds, and `rev-parse` prints the full published Makepad HEAD SHA.

- [ ] **Step 2: Pin the published Makepad HEAD before compiling waml**

Using `apply_patch`, replace `rev = "ec009e50"` in `crates/waml-editor/Cargo.toml` with the exact full SHA printed in Step 1. Update the adjacent fork comment to include “Markdown heading anchors, fragment scrolling, and app-owned FileTree folder toggles”. Add `url = "2.5"` under workspace dependencies, `url = { workspace = true }` to waml-editor, and `mod navigation;` in `main.rs`.

Run:

```powershell
cd C:\dev\waml
rtk cargo update -p makepad-widgets
```

Expected: `Cargo.lock` records a Makepad Git source whose requested revision and resolved commit both equal the Step 1 SHA. Every later waml compile now sees both prerequisite APIs.

- [ ] **Step 3: Write canonical breadcrumb tests**

Add these exact test helpers and an authored root/sales/archive fixture:

```rust
fn crumb(title: &str, target: NavigationTarget) -> BreadcrumbSegment {
    BreadcrumbSegment { title: title.into(), target }
}

fn fixture() -> (waml::okf::Bundle, waml::uml::Projection) {
    let source = waml::source::SourceBundle::try_from_pairs([
        ("index.md", "# Root\n\n* [Sales](sales/)\n"),
        ("sales/index.md", "# Sales\n\n* [Archive](archive/)\n"),
        ("sales/archive/index.md", "# Archive\n\n* [Order](order.md)\n"),
        (
            "sales/archive/order.md",
            "---\ntype: uml.Class\ntitle: Purchase Order\n---\n# Order\n",
        ),
    ]).unwrap();
    let bundle = waml::okf::Bundle::parse(&source).unwrap();
    let uml = waml::uml::project(&bundle);
    (bundle, uml)
}

#[test]
fn breadcrumb_uses_authored_titles_and_full_tree_hierarchy() {
    let (bundle, uml) = fixture();
    let segments = breadcrumb_for(&bundle, &uml, "sales/archive/order").unwrap();
    assert_eq!(
        segments,
        vec![
            crumb("Root", NavigationTarget::Directory { address: "/".into() }),
            crumb("Sales", NavigationTarget::Directory { address: "/sales".into() }),
            crumb("Archive", NavigationTarget::Directory { address: "/sales/archive".into() }),
            crumb("Purchase Order", NavigationTarget::Document {
                concept_id: "sales/archive/order".into(),
                fragment: None,
            }),
        ]
    );
}

#[test]
fn filtered_nav_state_cannot_change_canonical_breadcrumb() {
    let (bundle, uml) = fixture();
    let before = breadcrumb_for(&bundle, &uml, "sales/archive/order");
    let states = [
        crate::nav::NavState {
            scope: "/sales".into(),
            query: String::new(),
            filter: None,
        },
        crate::nav::NavState {
            scope: "/".into(),
            query: "purchase".into(),
            filter: None,
        },
        crate::nav::NavState {
            scope: "/".into(),
            query: String::new(),
            filter: Some(crate::tree::TreeKind::Class),
        },
    ];
    for state in states {
        let _projected = crate::nav::view(&bundle, &uml, &state);
        assert_eq!(breadcrumb_for(&bundle, &uml, "sales/archive/order"), before);
    }
}
```

- [ ] **Step 4: Run breadcrumb tests and verify failure**

Run:

```powershell
cd C:\dev\waml
rtk cargo test -p waml-editor navigation::tests::breadcrumb --bin waml-editor
```

Expected: FAIL because `navigation.rs` and its public types/functions are not implemented.

- [ ] **Step 5: Implement breadcrumb DFS over `build_tree`**

Call `build_tree(bundle, uml, "Untitled")`, recursively push directory targets and the matching document target, pop on a dead branch, and return `None` if no canonical path contains `concept_id`. Do not accept `NavState` or `NavView` in this API.

- [ ] **Step 6: Write the complete resolver matrix**

Add table-driven tests covering:

```rust
fn doc(concept_id: &str, fragment: Option<&str>) -> NavigationTarget {
    NavigationTarget::Document {
        concept_id: concept_id.into(),
        fragment: fragment.map(str::to_owned),
    }
}

fn dir(address: &str) -> NavigationTarget {
    NavigationTarget::Directory { address: address.into() }
}

let cases = [
    ("./customer.md", doc("sales/customer", None)),
    ("../shared.md", doc("shared", None)),
    ("/sales/customer.md", doc("sales/customer", None)),
    ("#orders", doc("sales/order", Some("orders"))),
    ("./customer.md#history", doc("sales/customer", Some("history"))),
    ("./archive/", dir("/sales/archive")),
    ("/", dir("/")),
    ("https://example.com/a?q=1#b", NavigationTarget::ExternalUrl(
        "https://example.com/a?q=1#b".into()
    )),
];
for (href, expected) in cases {
    assert_eq!(resolve_link(&bundle, "sales/order", href), Ok(expected));
}
```

Also assert exact errors for `../../../escape.md`, `mailto:a@example.com`, `http://`, `./missing.md`, `./missing/`, `./customer.md?mode=1`, an empty href, and `#`.

- [ ] **Step 7: Run resolver tests and verify failure**

Run:

```powershell
rtk cargo test -p waml-editor navigation::tests::resolve --bin waml-editor
```

Expected: FAIL because `resolve_link` has no normalization/classification implementation.

- [ ] **Step 8: Implement resolver normalization and typed errors**

Parse absolute URLs with `Url::parse`; accept only `http`/`https` with a host. Detect any other syntactic scheme before logical parsing and return `UnsupportedScheme`. Split a logical fragment once, reject empty fragments and query strings, start relative traversal from `DirectoryAddress::concept_parent(current_concept_id)`, normalize `.`/`..`, and return `EscapesBundle` if `..` consumes past root. A trailing `/` resolves to a normalized leading-slash directory address; a `.md` suffix resolves to a slashless concept ID. Validate existence through `bundle.directory(address)` or `bundle.concept(concept_id)`.

Implement exact status copy:

```rust
impl NavigationError {
    pub fn status_message(&self) -> String {
        match self {
            NavigationError::MalformedTarget(value) => format!("Invalid link: {value}"),
            NavigationError::UnsupportedScheme(value) => {
                format!("Unsupported link scheme: {value}")
            }
            NavigationError::EscapesBundle => "Link leaves this bundle".into(),
            NavigationError::MissingDocument(value) => {
                format!("Document not found: {value}")
            }
            NavigationError::MissingDirectory(value) => {
                format!("Directory not found: {value}")
            }
        }
    }
}
```

- [ ] **Step 9: Run pure model verification**

Run:

```powershell
rtk cargo test -p waml-editor navigation::tests --bin waml-editor
rtk cargo clippy -p waml-editor --bin waml-editor -- -D warnings
```

Expected: PASS.

- [ ] **Step 10: Commit**

```powershell
rtk git add Cargo.toml Cargo.lock crates/waml-editor/Cargo.toml crates/waml-editor/src/main.rs crates/waml-editor/src/navigation.rs
rtk git commit -m "feat(editor): resolve logical navigation"
```

---

### Task 4: Build the Shared Document Header Widget

**Files:**

- Create: `crates/waml-editor/src/document_header.rs`
- Modify: `crates/waml-editor/src/main.rs`

**Interfaces:**

- Consumes: `BreadcrumbSegment`, `NavigationTarget`, `Icon`, and the existing `IconButton`.
- Produces:

```rust
pub const DOCUMENT_HEADER_H: f64 = 30.0;

#[derive(Clone, Debug, PartialEq)]
pub enum DocumentHeaderAction {
    Navigate(NavigationTarget),
    ToggleRightDock,
}

pub struct DocumentHeaderLayout {
    pub visible_indices: Vec<usize>,
    pub segment_rects: Vec<(usize, Rect)>,
    pub height: f64,
}

pub fn header_height(has_breadcrumb: bool, has_right_dock: bool) -> f64;

pub fn layout_header(
    available_width: f64,
    label_widths: &[f64],
    right_button_width: f64,
) -> DocumentHeaderLayout;

impl DocumentHeader {
    pub fn set_segments(&mut self, cx: &mut Cx, segments: Vec<BreadcrumbSegment>);
    pub fn set_right_dock(&mut self, cx: &mut Cx, icon: Option<Icon>);
    pub fn set_right_dock_active(&mut self, cx: &mut Cx, active: bool);
    pub fn visible_height(&self) -> f64;
    pub fn action(&self, actions: &Actions) -> Option<DocumentHeaderAction>;
}
```

- [ ] **Step 1: Write pure layout tests**

```rust
#[test]
fn header_height_tracks_its_two_content_sources() {
    assert_eq!(header_height(false, false), 0.0);
    assert_eq!(header_height(true, false), DOCUMENT_HEADER_H);
    assert_eq!(header_height(false, true), DOCUMENT_HEADER_H);
    assert_eq!(header_height(true, true), DOCUMENT_HEADER_H);
}

#[test]
fn narrow_elision_preserves_the_current_segment() {
    let layout = layout_header(90.0, &[44.0, 52.0, 58.0], 0.0);
    assert_eq!(layout.visible_indices.last(), Some(&2));
    assert!(!layout.visible_indices.contains(&0));
}

#[test]
fn hit_rects_retain_original_segment_indices() {
    let layout = layout_header(300.0, &[40.0, 50.0, 60.0], 30.0);
    assert_eq!(
        layout.segment_rects.iter().map(|(index, _)| *index).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```powershell
rtk cargo test -p waml-editor document_header::tests --bin waml-editor
```

Expected: FAIL because the module, constants, and geometry functions do not exist.

- [ ] **Step 3: Implement KISS geometry**

Reserve `30.0` px for the right button only when present. Always keep the final segment. Walk ancestors from newest to oldest while each label plus a `14.0` px chevron fits; reverse the chosen indices for drawing. Produce one independent rectangle per visible segment and no rectangle for elided ancestors.

- [ ] **Step 4: Implement the widget and actions**

Register `DocumentHeader` with height initially `0.0`, left-aligned subdued ancestor labels, emphasized final label, existing chevron glyph separators, and a right-aligned `IconButton`. In pointer handling, test segment rectangles from newest to oldest and emit the stored target. Relay the button click as `ToggleRightDock`. `set_segments` and `set_right_dock` recompute visibility and force relayout only when content changes; `set_right_dock_active` delegates to the embedded button's existing active-state setter.

- [ ] **Step 5: Add widget-state tests**

Use a test-only state constructor to assert breadcrumb-only, button-only, combined, and empty transitions clear stale segments/icons. Assert clicking the current segment emits its document target.

- [ ] **Step 6: Run header verification**

Run:

```powershell
rtk cargo test -p waml-editor document_header::tests --bin waml-editor
rtk cargo clippy -p waml-editor --bin waml-editor -- -D warnings
```

Expected: PASS.

- [ ] **Step 7: Commit**

```powershell
rtk git add crates/waml-editor/src/document_header.rs crates/waml-editor/src/main.rs
rtk git commit -m "feat(editor): add shared document header"
```

---

### Task 5: Add Typed Header Chrome and Shared Markdown Intent

**Files:**

- Modify: `crates/waml-editor/src/doc_view.rs`
- Modify: `crates/waml-editor/src/markdown_surface.rs`
- Modify: `crates/waml-editor/src/source_view.rs`
- Modify: `crates/waml-editor/src/generic_okf_view.rs`
- Modify: `crates/waml-editor/src/class_diagram_view.rs`
- Modify: `crates/waml-editor/src/classifier_preview_view.rs`
- Modify: `crates/waml-editor/src/document_host.rs`

**Interfaces:**

- Consumes: Task 1 `MarkdownRef::scroll_to_fragment`, Task 3 `NavigationIntent`, and Task 4 `DocumentHeader`.
- Produces:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DocumentHeaderChrome {
    pub breadcrumb: bool,
    pub right_dock: Option<Icon>,
}

pub struct BodyChrome {
    pub tool_dock: bool,
    pub view_bar: bool,
    pub canvas_overlays: bool,
    pub document_header: DocumentHeaderChrome,
}

pub struct ViewOutcome {
    pub edit: Option<PendingEdit>,
    pub popup: Option<PopupRequest>,
    pub promote_subject: Option<String>,
    pub close_active: bool,
    pub statusbar_dirty: bool,
    pub navigation: Option<NavigationIntent>,
}

impl BodyWidgets {
    pub fn markdown_link(&self, actions: &Actions) -> Option<String>;
    pub fn scroll_markdown_to_fragment(&self, cx: &mut Cx, fragment: &str) -> bool;
}

pub fn link_navigated(actions: &Actions) -> Option<String>;
pub fn scroll_to_fragment(ui: &WidgetRef, cx: &mut Cx, fragment: &str) -> bool;
```

- [ ] **Step 1: Write failing chrome contract tests**

Update `doc_view.rs` tests so diagram, classifier preview, and source use:

```rust
DocumentHeaderChrome {
    breadcrumb: true,
    right_dock: Some(Icon::SlidersHorizontal),
}
```

Generic OKF uses:

```rust
DocumentHeaderChrome {
    breadcrumb: true,
    right_dock: None,
}
```

`BodyChrome::HIDDEN` uses `DocumentHeaderChrome::default()`. Add `assert!(ViewOutcome::default().navigation.is_none())`.

- [ ] **Step 2: Run and verify failure**

Run:

```powershell
rtk cargo test -p waml-editor doc_view::tests --bin waml-editor
```

Expected: FAIL because `DocumentHeaderChrome` and `ViewOutcome.navigation` do not exist.

- [ ] **Step 3: Implement the typed chrome contribution**

Replace the top-level `BodyChrome.right_dock` field with `BodyChrome.document_header`. In `BodyWidgets::apply_chrome`, call `DocumentHeader::set_right_dock`; close the inspector when `document_header.right_dock` is `None`; stop looking up `inspector_btn` and stop calling `DocTabs::set_right_dock_btn`.

- [ ] **Step 4: Write shared Markdown-action tests**

Construct `Actions` containing a `WidgetAction` whose boxed action is:

```rust
MarkdownAction::LinkNavigated("../customer.md#history".into())
```

Assert `link_navigated(&actions) == Some("../customer.md#history".into())`. Also assert unrelated widget actions return `None`.

- [ ] **Step 5: Implement shared extraction and fragment delegation**

`link_navigated` iterates all `Actions`, downcasts to `WidgetAction`, then downcasts `widget_action.action` to `MarkdownAction`; it returns the original string without parsing. `scroll_to_fragment` looks up `ids!(markdown_surface.md)`, calls `.as_markdown().scroll_to_fragment(cx, fragment)`, and returns the renderer result.

- [ ] **Step 6: Make Source and Generic OKF views emit identical raw-link intent**

In each view's `handle`, use:

```rust
let Some(href) = body.markdown_link(actions) else {
    return ViewOutcome::default();
};
ViewOutcome {
    navigation: Some(NavigationIntent::MarkdownLink {
        current_concept_id: self.key.clone(), // GenericOkfView uses self.concept_id.clone()
        href,
    }),
    ..ViewOutcome::default()
}
```

Add one unit test per view asserting the current logical ID and raw href. Do not call `resolve_link` in either view.

- [ ] **Step 7: Run view and host tests**

Run:

```powershell
rtk cargo test -p waml-editor markdown_surface::tests --bin waml-editor
rtk cargo test -p waml-editor source_view::tests --bin waml-editor
rtk cargo test -p waml-editor generic_okf_view::tests --bin waml-editor
rtk cargo test -p waml-editor document_host::tests --bin waml-editor
```

Expected: PASS.

- [ ] **Step 8: Commit**

```powershell
rtk git add crates/waml-editor/src/doc_view.rs crates/waml-editor/src/markdown_surface.rs crates/waml-editor/src/source_view.rs crates/waml-editor/src/generic_okf_view.rs crates/waml-editor/src/class_diagram_view.rs crates/waml-editor/src/classifier_preview_view.rs crates/waml-editor/src/document_host.rs
rtk git commit -m "feat(editor): declare document header chrome"
```

---

### Task 6: Unify Tree Document and Directory Intent

**Files:**

- Modify: `crates/waml-editor/src/tree_panel.rs`
- Modify: `crates/waml-editor/src/app/actions.rs`

**Interfaces:**

- Consumes: `NavigationIntent::{Resolved}`, `NavigationTarget::{Document, Directory}`, and `OpenDisposition`.
- Produces:

```rust
pub enum ProjectTreeAction {
    None,
    Navigate(NavigationIntent),
    ScopeRequest { anchor: Rect },
    Query(String),
    FilterRequest { anchor: Rect },
    ContextMenu { key: String, anchor: DVec2 },
}

impl ProjectTree {
    pub fn navigation(&self, actions: &Actions) -> Option<NavigationIntent>;
    pub fn toggle_directory(&mut self, cx: &mut Cx, address: &str) -> bool;
}

fn row_navigation(
    key: &str,
    concept_id: Option<&str>,
    is_directory: bool,
    openable: bool,
    tap_count: u32,
) -> Option<NavigationIntent>;
```

- [ ] **Step 1: Write tree action policy tests**

Replace `document_action` tests with:

```rust
fn resolved_document(
    concept_id: &str,
    disposition: OpenDisposition,
) -> NavigationIntent {
    NavigationIntent::Resolved {
        target: NavigationTarget::Document {
            concept_id: concept_id.into(),
            fragment: None,
        },
        disposition,
    }
}

assert_eq!(
    row_navigation("sales/order", Some("sales/order"), false, true, 1),
    Some(resolved_document("sales/order", OpenDisposition::Preview))
);
assert_eq!(
    row_navigation("sales/order", Some("sales/order"), false, true, 2),
    Some(resolved_document("sales/order", OpenDisposition::Persistent))
);
assert_eq!(
    row_navigation("/sales", None, true, false, 1),
    Some(NavigationIntent::Resolved {
        target: NavigationTarget::Directory { address: "/sales".into() },
        disposition: OpenDisposition::Preview,
    })
);
```

Use the row key as the directory address. A non-openable, non-directory row returns `None`.

- [ ] **Step 2: Run and verify failure**

Run:

```powershell
rtk cargo test -p waml-editor tree_panel::tests --bin waml-editor
```

Expected: FAIL because rows do not emit unified navigation and no public directory command exists.

- [ ] **Step 3: Disable FileTree mutation and emit folder/file clicks through one action**

Configure the `FileTree` mounted inside `ProjectTree` with `auto_toggle_folders: false`. Handle both `file_tree.file_clicked(actions)` and `file_tree.folder_clicked(actions)` and emit `ProjectTreeAction::Navigate`. A folder click now emits intent without changing Makepad `open_nodes`; its `NavigationIntent::Resolved { target, disposition }` is byte-for-byte the same shape emitted by a breadcrumb or resolved Markdown directory link.

- [ ] **Step 4: Implement `toggle_directory`**

Keep a `HashSet<String>` mirror of folder-open state in `ProjectTree`, initialized by `set_view`. `toggle_directory` returns `false` for unknown addresses; otherwise flips the mirror and calls:

```rust
file_tree.set_folder_is_open(
    cx,
    LiveId::from_str(address),
    now_open,
    Animate::Yes,
);
```

Redraw the tree and return `true`. Because `auto_toggle_folders` is false, this method is the only path that updates both the mirror and Makepad `open_nodes`.

- [ ] **Step 5: Add directory parity tests**

Assert tree, breadcrumb, and resolved Markdown construction of `Directory { address: "/sales" }` produce equal `NavigationIntent` values. Assert the folder remains unchanged immediately after `FolderClicked`, changes exactly once after `ProjectTree::toggle_directory`, and toggling an unknown address returns `false` without changing recorded open state.

- [ ] **Step 6: Run tree tests**

Run:

```powershell
rtk cargo test -p waml-editor tree_panel::tests --bin waml-editor
```

Expected: PASS.

- [ ] **Step 7: Commit**

```powershell
rtk git add crates/waml-editor/src/navigation.rs crates/waml-editor/src/tree_panel.rs crates/waml-editor/src/app/actions.rs
rtk git commit -m "feat(editor): unify tree navigation intent"
```

---

### Task 7: Mount the Header, Remove Caption Coupling, and Protect the Narrow Toggle

**Files:**

- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/src/app/actions.rs`
- Modify: `crates/waml-editor/src/dock.rs`
- Modify: `crates/waml-editor/src/doc_tabs.rs`

**Interfaces:**

- Consumes: `DocumentHeader`, `DocumentHeaderChrome`, `DOCUMENT_HEADER_H`, and existing dock state.
- Produces:

```rust
pub fn narrow_inspector_top(narrow: bool, header_height: f64) -> f64;
```

The live widget id for the shared header is `document_header`; the caption-level `inspector_btn` id is removed.

- [ ] **Step 1: Write geometry and stale-state tests**

In `dock.rs`:

```rust
#[test]
fn narrow_inspector_starts_below_only_a_visible_header() {
    assert_eq!(narrow_inspector_top(true, 30.0), 30.0);
    assert_eq!(narrow_inspector_top(true, 0.0), 0.0);
    assert_eq!(narrow_inspector_top(false, 30.0), 0.0);
}
```

In `doc_tabs.rs`, replace `top_rule_overshoot_tracks_the_right_dock_toggle` with a test that `rule_x_end(strip_right) == strip_right`; no header state may affect tab geometry.

- [ ] **Step 2: Run and verify failure**

Run:

```powershell
rtk cargo test -p waml-editor dock::tests::narrow_inspector_starts_below_only_a_visible_header --bin waml-editor
rtk cargo test -p waml-editor doc_tabs::tests::top_rule --bin waml-editor
```

Expected: FAIL because the helper does not exist and `DocTabs` still carries `right_dock_btn`.

- [ ] **Step 3: Reshape the central live layout**

Remove `inspector_btn` from `tab_row`. Replace `center_stack` in `dock_row` with a `center_column` (`flow: Down`) whose first child is:

```text
document_header := DocumentHeader{ width: Fill height: 0.0 }
```

and whose second child is the existing `center_stack` with `height: Fill`. Configure `markdown_surface.md` to use Task 1's internal vertical scrollbar and remove the outer `View.scroll_bars`, so renderer-owned anchors and scroll state are the same surface.

- [ ] **Step 4: Delete all DocTabs/right-button coupling**

Remove `INSPECTOR_BTN_W`, `DocTabs.right_dock_btn`, `DocTabs::set_right_dock_btn`, the conditional `rule_x_end` argument, associated comments, and its old test. The top rule ends at the tab strip's own right edge.

- [ ] **Step 5: Move the dock toggle observer to `DocumentHeaderAction`**

In `observe_caption_and_docks`, remove the caption `inspector_btn` click branch. Read `DocumentHeader::action(actions)`; on `ToggleRightDock`, execute the existing narrow/wide inspector toggle logic unchanged. Continue updating active styling from `sync_dock_slots`, but call `DocumentHeader::set_right_dock_active(cx, inspector_state == DockState::Pinned)`.

- [ ] **Step 6: Enforce narrow overlay accessibility**

In `sync_dock_slots`, read `DocumentHeader::visible_height()`, compute `narrow_inspector_top`, and assign it to `inspector_host.walk.margin.top`. Reduce `inspector_host.walk.height` only through the margin; leave wide geometry unchanged. Update `WindowDragQuery` client-area checks to include header segments/button and remove `over_inspector_btn`.

- [ ] **Step 7: Synchronize header content and start-screen collapse**

In `sync_document_shell`, read `active_tab`, then `documents.active_chrome().document_header`. If `breadcrumb` is true, call `breadcrumb_for`; otherwise pass an empty vector. Missing paths also pass an empty vector without clearing the right-dock icon. `show_start_screen` pushes no segments and no icon, yielding zero height.

- [ ] **Step 8: Run layout and chrome tests**

Run:

```powershell
rtk cargo test -p waml-editor dock::tests --bin waml-editor
rtk cargo test -p waml-editor doc_tabs::tests --bin waml-editor
rtk cargo test -p waml-editor document_header::tests --bin waml-editor
rtk cargo test -p waml-editor doc_view::tests --bin waml-editor
```

Expected: PASS, including empty/breadcrumb/button/combined heights, current-segment elision, active right-dock styling, start-screen collapse, and narrow inspector offset.

- [ ] **Step 9: Commit**

```powershell
rtk git add crates/waml-editor/src/app.rs crates/waml-editor/src/app/actions.rs crates/waml-editor/src/dock.rs crates/waml-editor/src/doc_tabs.rs
rtk git commit -m "feat(editor): mount document header"
```

---

### Task 8: Execute All Navigation Through One Application Handler

**Files:**

- Create: `crates/waml-editor/src/platform_browser.rs`
- Modify: `crates/waml-editor/src/main.rs`
- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/src/app/actions.rs`
- Modify: `crates/waml-editor/src/statusbar.rs`

**Interfaces:**

- Consumes: `NavigationIntent`, `NavigationTarget`, `resolve_link`, `DocumentHost::transition`, `ProjectTree::toggle_directory`, and `BodyWidgets::scroll_markdown_to_fragment`.
- Produces:

```rust
pub trait ExternalUrlAdapter {
    fn open(&mut self, cx: &mut Cx, url: &str) -> Result<(), String>;
}

pub struct PlatformBrowser;

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingFragment {
    concept_id: String,
    fragment: String,
}

impl App {
    fn handle_navigation_intent(
        &mut self,
        cx: &mut Cx,
        intent: NavigationIntent,
    ) -> bool;

    fn navigate_with<B: ExternalUrlAdapter>(
        &mut self,
        cx: &mut Cx,
        target: NavigationTarget,
        disposition: OpenDisposition,
        browser: &mut B,
    ) -> bool;

    fn apply_pending_fragment(&mut self, cx: &mut Cx);
}
```

- [ ] **Step 1: Write policy tests with a fake browser**

Create:

```rust
#[derive(Default)]
struct FakeBrowser {
    opened: Vec<String>,
    error: Option<String>,
}

impl ExternalUrlAdapter for FakeBrowser {
    fn open(&mut self, _cx: &mut Cx, url: &str) -> Result<(), String> {
        self.opened.push(url.into());
        self.error.clone().map_or(Ok(()), Err)
    }
}
```

Test that external targets append once and produce no document/directory command; fake failure preserves active document and records `"Could not open link: blocked"`. Test document preview versus persistent disposition and repeat activation tab count. Feed equal tree, breadcrumb, and resolved Markdown directory intents through the handler, assert each independently performs one toggle, assert non-root directory activation preserves both `documents.active_id()` and `nav_state.scope`, and test root semantics separately.

- [ ] **Step 2: Run policy tests and verify failure**

Run:

```powershell
rtk cargo test -p waml-editor app::tests::navigation --bin waml-editor
```

Expected: FAIL because the adapter and unified executor do not exist.

- [ ] **Step 3: Implement the production platform adapter**

On Windows spawn `rundll32.exe url.dll,FileProtocolHandler <url>`; on macOS spawn `open <url>`; on other native targets spawn `xdg-open <url>`. Convert `std::io::Error` to a concise string. On wasm call `cx.open_url(url, OpenUrlInPlace::No)` and return `Ok(())`. Tests instantiate only `FakeBrowser`.

- [ ] **Step 4: Implement resolved target execution**

`Document` calls the existing `transition_document` with `persistent == OpenDisposition::Persistent`. If a fragment exists, set `pending_fragment` before transition, including for the already-active document, then request redraw. Every non-root `Directory` target calls `ProjectTree::toggle_directory` exactly once; `/` instead sets `nav_state.scope = "/"`, clears query/filter, opens the tree dock, and calls `refresh_nav(cx, true)`. `ExternalUrl` calls only the adapter. Clear navigation status on success.

- [ ] **Step 5: Resolve raw Markdown only at the App boundary**

`handle_navigation_intent` matches `MarkdownLink`, calls `resolve_link(self.session.okf(), &current_concept_id, &href)`, reports `NavigationError::status_message()` on failure, and otherwise calls the same resolved executor with preview disposition. Header and tree actions enter the same method with resolved intents.

- [ ] **Step 6: Add pending-fragment tests**

Assert a cross-document fragment remains pending through document activation, succeeds only after `scroll_markdown_to_fragment` returns true, and clears afterward. Assert a missing anchor keeps the activated document, clears the pending request after the first completed draw, and reports `"Section not found: missing"`. Assert applying an already-cleared request is idempotent.

- [ ] **Step 7: Apply fragments after renderer draw**

After `self.ui.handle_event(cx, event, &mut Scope::empty())`, when `event` is `Event::Draw(_)`, call `apply_pending_fragment`. Verify the active tab concept ID matches the pending concept before asking `BodyWidgets` to scroll. A mismatch keeps the request pending for the activation draw; a matching missing anchor reports failure and clears it.

- [ ] **Step 8: Add concise status feedback**

Extend `Statusbar` with `navigation_message: Option<String>` and:

```rust
pub fn set_navigation_message(&mut self, cx: &mut Cx, message: Option<&str>);
```

Render save errors first, navigation messages second, normal diagram status last. Add tests for unresolved/malformed/unsupported/out-of-bundle, missing fragment, and browser-launch copy. Successful navigation clears only the navigation message, never save feedback.

- [ ] **Step 9: Route all three action sources**

Add a `DocumentHeader` exclusive handler next to tree navigation. Replace `handle_tree_document_open` with `handle_tree_navigation`; update `apply_view_outcome` to consume `outcome.navigation`; both call `handle_navigation_intent`. No widget directly calls `transition_document`, mutates `nav_state`, toggles a directory, or opens a URL.

- [ ] **Step 10: Run navigation and status verification**

Run:

```powershell
rtk cargo test -p waml-editor app::tests::navigation --bin waml-editor
rtk cargo test -p waml-editor statusbar::tests --bin waml-editor
rtk cargo test -p waml-editor document_host::tests --bin waml-editor
```

Expected: PASS.

- [ ] **Step 11: Commit**

```powershell
rtk git add crates/waml-editor/src/platform_browser.rs crates/waml-editor/src/main.rs crates/waml-editor/src/app.rs crates/waml-editor/src/app/actions.rs crates/waml-editor/src/statusbar.rs
rtk git commit -m "feat(editor): execute unified navigation"
```

---

### Task 9: Add End-to-End Source, Generic OKF, Header, and Geometry Regression Coverage

**Files:**

- Modify: `crates/waml-editor/src/navigation.rs`
- Modify: `crates/waml-editor/src/document_header.rs`
- Modify: `crates/waml-editor/src/tree_panel.rs`
- Modify: `crates/waml-editor/src/source_view.rs`
- Modify: `crates/waml-editor/src/generic_okf_view.rs`
- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/src/dock.rs`

**Interfaces:**

- Consumes: all prior task interfaces.
- Produces: the approved spec’s complete unit/integration/geometry regression suite; no new production interface.

- [ ] **Step 1: Add a navigation-entry-point equivalence table**

Build three intents for the same document and three for the same directory:

```rust
fn resolved_target(intent: &NavigationIntent) -> Option<&NavigationTarget> {
    match intent {
        NavigationIntent::Resolved { target, .. } => Some(target),
        NavigationIntent::MarkdownLink { .. } => None,
    }
}

assert_eq!(resolved_target(&tree_intent), resolved_target(&breadcrumb_intent));
assert_eq!(resolved_target(&tree_intent), resolved_target(&markdown_resolved_intent));
```

Assert the resulting `DocumentCommand::Open` differs only when the tree supplies `Persistent`. Assert repeated active-document activation leaves tab count unchanged.

- [ ] **Step 2: Add document-type switch coverage**

Drive chrome state in this order: source (breadcrumb + button), generic OKF (breadcrumb only), start screen (empty), source again. Assert there is no stale right icon, breadcrumb, height, or inspector dock state after each switch.

- [ ] **Step 3: Add renderer/view integration coverage**

For both `SourceView` and `GenericOkfView`, inject `MarkdownAction::LinkNavigated("./next.md#details")`, assert the raw href and correct current concept ID, resolve it, activate the document, and apply a recorded `details` anchor. Assert missing anchors leave the newly activated document selected.

- [ ] **Step 4: Add header pointer and narrow overlay geometry coverage**

Assert every visible segment hit rectangle maps to its original `NavigationTarget`, the current segment survives all positive widths, and the right-dock active state reflects the same `Inspector::dock_state`. For narrow layouts assert:

```rust
assert!(inspector_rect.pos.y >= header_rect.pos.y + header_rect.size.y);
```

when the header is visible, and equality with the body top when absent.

In wide mode also assert `header_rect.pos.x == left_slot_rect.pos.x + left_slot_rect.size.x` and `header_rect.pos.x + header_rect.size.x == right_slot_rect.pos.x`, proving the header spans only the center between dock reservations.

- [ ] **Step 5: Run the complete waml-editor suite**

Run:

```powershell
cd C:\dev\waml
rtk cargo test -p waml-editor --bin waml-editor
rtk cargo clippy -p waml-editor --all-targets -- -D warnings
rtk cargo fmt --all -- --check
```

Expected: all commands PASS.

- [ ] **Step 6: Run the full workspace regression suite**

Run:

```powershell
rtk cargo test --workspace
```

Expected: PASS with no regressions in `waml`, `waml-cli`, or `waml-ops-dto`.

- [ ] **Step 7: Commit regression coverage**

```powershell
rtk git add crates/waml-editor/src/navigation.rs crates/waml-editor/src/document_header.rs crates/waml-editor/src/tree_panel.rs crates/waml-editor/src/source_view.rs crates/waml-editor/src/generic_okf_view.rs crates/waml-editor/src/app.rs crates/waml-editor/src/dock.rs
rtk git commit -m "test(editor): cover logical navigation flow"
```

---

### Task 10: Perform Final Cross-Repository Verification

**Files:**

- Test: `C:\dev\makepad\widgets\src\markdown.rs`
- Test: `C:\dev\makepad\widgets\src\file_tree.rs`
- Test: `C:\dev\waml\Cargo.lock`
- Test: `C:\dev\waml\crates\waml-editor\Cargo.toml`

**Interfaces:**

- Consumes: Task 3’s already-published and already-pinned Makepad HEAD, `MarkdownRef::scroll_to_fragment`, and `FileTree.auto_toggle_folders`.
- Produces: verification evidence only; this task changes and commits no files.

- [ ] **Step 1: Verify the existing pin equals Makepad HEAD**

Run:

```powershell
$makepadHead = (rtk git -C C:\dev\makepad rev-parse HEAD).Trim()
rtk rg -n $makepadHead C:\dev\waml\crates\waml-editor\Cargo.toml C:\dev\waml\Cargo.lock
```

Expected: the same full SHA appears in the manifest revision and lockfile Git source. If it does not, stop: Task 3’s prerequisite pin was not completed and later waml test evidence is invalid.

- [ ] **Step 2: Verify both Makepad prerequisite commits**

Run:

```powershell
cd C:\dev\makepad
rtk cargo test -p makepad-widgets markdown::tests --lib
rtk cargo test -p makepad-widgets file_tree::tests --lib
rtk cargo clippy -p makepad-widgets --lib -- -D warnings
```

Expected: Markdown anchors/scrolling and both FileTree default/opt-out modes PASS.

- [ ] **Step 3: Verify the pinned waml workspace**

Run:

```powershell
cd C:\dev\waml
rtk cargo test --workspace
rtk cargo clippy -p waml-editor --all-targets -- -D warnings
rtk cargo fmt --all -- --check
```

Expected: all commands PASS against the Task 3 pin and no test launches an external browser.

- [ ] **Step 4: Inspect final scope**

Run:

```powershell
rtk git -C C:\dev\makepad status --short
rtk git -C C:\dev\makepad log --oneline -2
rtk git -C C:\dev\waml status --short
rtk git -C C:\dev\waml log --oneline -8
```

Expected: neither repository has uncommitted files from this plan. Makepad shows separate Markdown and FileTree commits; waml shows the pinned resolver commit before header, chrome, tree intent, layout, execution, and regression commits.
