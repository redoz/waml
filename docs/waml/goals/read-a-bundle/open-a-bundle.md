# Open a Bundle

**Goal:** A reader opens a bundle and keeps a usable workspace when an open
attempt fails.

**Why:** All other reader workflows need an open bundle.

**Done when:** The editor shows a start screen without an open bundle, keeps
recent bundles in promoted order, pins a recent bundle without changing its
identity, replaces the workspace after a valid open, keeps the prior workspace
after a failed open, and opens an in-bundle document link in a preview.

**Status:** done
**MVP:** yes

## Shipped behavior

#### NATIVE-001 — the editor shows the start screen without an open bundle

**Applies to:** native

**Given** no bundle is open
**When** the reader starts the editor
**Then** the editor shows the start screen

**Evidence:** `crates/waml-editor/src/start_screen.rs::StartScreen`

#### NATIVE-002 — recent bundles stay in promoted order

**Applies to:** native

**Given** the start screen has recent bundles
**When** the reader views the recent-bundle list
**Then** promoted bundles appear before the other recent bundles

**Evidence:** `crates/waml-editor/src/config.rs::sort_recents`

#### NATIVE-003 — pinning keeps the recent bundle identity

**Applies to:** native

**Given** the start screen has an unpinned recent bundle
**When** the reader pins that bundle
**Then** the recent bundle keeps its stored identity and becomes pinned

**Evidence:** `crates/waml-editor/src/config.rs::set_pinned`

#### NATIVE-004 — a valid bundle replaces the active workspace

**Applies to:** native

**Given** the editor has an active workspace
**When** the reader opens a valid bundle
**Then** the editor replaces the active workspace with the loaded bundle

**Evidence:** `crates/waml-editor/src/app/workspace.rs:493` `crates/waml-editor/src/app/workspace.rs:595`

#### NATIVE-005 — a failed open keeps the prior workspace

**Applies to:** native

**Given** the editor has an active workspace
**When** the reader tries to open a bundle that the editor cannot load
**Then** the prior workspace remains active

**Evidence:** `crates/waml-editor/src/app/workspace.rs:493`

#### NATIVE-014 — an in-bundle document link opens in the preview

**Applies to:** shared

**Given** a Markdown link refers to another document in the open bundle
**When** the reader follows the link
**Then** the referenced document becomes the active preview document

**Evidence:** `crates/waml-editor/src/app/tests/navigation.rs::navigation_markdown_resolves_only_at_the_app_boundary`

## Verification gaps

- NATIVE-001 — target: native; No native test asserts the visible empty/start screen.
- NATIVE-002 — target: native; No native test asserts rendered recent-item order.
- NATIVE-003 — target: native; No native test asserts pinning from the start screen.
- NATIVE-004 — target: native; The test covers replacement saves, not the full active-workspace open result.
- NATIVE-005 — target: native; The test checks the asset root only, not the complete prior-workspace result.

## Notes

- Browser entry workflows are owned by [Share and Publish](../share-and-publish/).
- Final-save close protection is owned by
  [Save and Undo](../author-in-the-editor/save-and-undo.md).
