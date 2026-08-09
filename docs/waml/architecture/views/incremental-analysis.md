---
type: uml.Activity
title: Incremental Analysis
description: The revisioned analysis flow from exact text changes to a committed or rejected candidate.
---

# Incremental Analysis

## Notes
- [Markdown Syntax](../concepts/implementation/markdown-syntax.md) validates exact changes and reports whether reparse is incremental or full.
- [Affected Analysis](../concepts/implementation/affected-analysis.md) is sorted and has no duplicate document, island, or diagram identities.
- [UML Analysis](../concepts/implementation/uml-analysis.md) records freshness for each syntax island.
- A quarantined document does not make every projection stale. Unrelated diagram projections stay current.
- A direct semantic edit commits only a fully prepared candidate. An accepted Markdown source edit can install source before its semantic completion.
- Current UML analysis visits every claimed concept. Affected analysis is output metadata, not an affected-only semantic scheduler.
- Affected identities describe semantic impact. The current shell still prepares every open document and updates the active view.

## Nodes

### initial
- transitions to Exact Text Changes

### Exact Text Changes
- do: `apply ordered changes to the accepted source-text identity`
- transitions to Validate Base Identity

### Validate Base Identity
- transitions to Base Identity Matches?

### decision Base Identity Matches?
- when `revision and source identity match` transitions to Incremental Reparse
- else transitions to Reject Edit

### Incremental Reparse
- do: `reparse valid changed ranges and preserve reusable syntax identities`
- transitions to Incremental Recovery Applied?

### decision Incremental Recovery Applied?
- when `incremental recovery applies` transitions to Prepare Markdown and Catalog
- else transitions to Full Document Reparse

### Full Document Reparse
- do: `parse the full changed document and record the fallback reason`
- transitions to Prepare Markdown and Catalog

### Prepare Markdown and Catalog
- do: `reuse accepted snapshots and quarantine shell-failed documents`
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

### Reject Edit
- do: `keep the accepted snapshot and revision`
- transitions to final

### Reject Candidate
- do: `keep the accepted snapshot and revision`
- transitions to final

### final
