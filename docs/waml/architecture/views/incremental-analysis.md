---
type: uml.Activity
title: Incremental Analysis
description: The document-local edit, source promotion, and semantic analysis flow for an incremental Markdown change.
---

# Incremental Analysis

## Notes
- `MarkdownDocumentSession::apply_edit_without_history` validates the local `DocumentRevision` and exact changes, reparses, and then advances its Markdown snapshot.
- `EditorSession::promote_source_edit` later validates the session document revision and accepted source `Arc` identity before it installs a source-only session revision.
- A failed promotion leaves the editor-session snapshot unchanged. The app shell then synchronizes the active Markdown view from that snapshot.
- [Affected Analysis](../concepts/implementation/affected-analysis.md) is sorted and has no duplicate document, island, or diagram identities.
- [UML Analysis](../concepts/implementation/uml-analysis.md) records freshness for each syntax island.
- A quarantined document does not make every projection stale. Unrelated diagram projections stay current.
- A direct semantic edit commits only a fully prepared candidate. An accepted Markdown source edit can install source before its semantic completion.
- Current UML analysis visits every claimed concept. Affected analysis is output metadata, not an affected-only semantic scheduler.
- Affected identities describe semantic impact. The current shell still prepares every open document and updates the active view.

## Nodes

### initial
- transitions to Validate Local Markdown Edit

### Validate Local Markdown Edit
- do: `validate the local document revision, ordered change ranges, change map, result selection, and next revision`
- transitions to Local Edit Valid?

### decision Local Edit Valid?
- when `local revision and changes are valid` transitions to Reparse Local Markdown
- else transitions to Reject Local Edit

### Reparse Local Markdown
- do: `reparse valid changed ranges and preserve reusable syntax identities`
- transitions to Incremental Recovery Applied?

### decision Incremental Recovery Applied?
- when `incremental recovery applies` transitions to Advance Local Markdown State
- else transitions to Full Document Reparse

### Full Document Reparse
- do: `parse the full changed document and record the fallback reason`
- transitions to Advance Local Markdown State

### Advance Local Markdown State
- do: `install the next Markdown snapshot and selections, then return a proposed source edit`
- transitions to Promote Source Edit

### Promote Source Edit
- do: `submit the proposal to the editor session after document-local editing completes`
- transitions to Promotion Guards Match?

### decision Promotion Guards Match?
- when `session document revision and accepted source Arc identity match` transitions to Install Source-Only Session Revision
- else transitions to Reject Promotion

### Install Source-Only Session Revision
- do: `apply the exact source changes and advance the editor session with pending semantic work`
- transitions to Prepare Markdown and Catalog

### Prepare Markdown and Catalog
- do: `build a provisional catalog, reuse or promote Markdown snapshots, quarantine shell-failed documents, and derive OKF from the accepted catalog`
- transitions to Malformed Document Quarantined?

### decision Malformed Document Quarantined?
- when `one or more documents are quarantined` transitions to Continue with Accepted Documents
- else transitions to Analyze Claimed UML Concepts

### Continue with Accepted Documents
- do: `record each quarantine error and exclude that document from downstream analysis`
- transitions to Analyze Claimed UML Concepts

### Analyze Claimed UML Concepts
- do: `visit every claimed concept, reuse syntax identities, validate semantics, and build projections`
- transitions to Compute Affected Closure and Freshness

### Compute Affected Closure and Freshness
- do: `derive affected documents, islands, and diagrams after projection`
- transitions to Retain Unrelated Current Projections

### Retain Unrelated Current Projections
- do: `reuse prior projections for failed dependents and keep unrelated projections current`
- transitions to Prepare Candidate

### Prepare Candidate
- transitions to Candidate Valid?

### decision Candidate Valid?
- when `all required invariants hold` transitions to Commit Candidate
- else transitions to Reject Candidate

### Commit Candidate
- do: `install the prepared state at its revision`
- transitions to Reconcile Open Documents and Active View

### Reconcile Open Documents and Active View
- do: `prepare every open tab, reconcile the document set, and update the active view`
- transitions to final

### Reject Local Edit
- do: `keep the document-local Markdown snapshot and revision`
- transitions to final

### Reject Promotion
- do: `keep the editor-session snapshot unchanged and synchronize the active Markdown view from it`
- transitions to final

### Reject Candidate
- do: `keep the accepted snapshot and revision`
- transitions to final

### final
