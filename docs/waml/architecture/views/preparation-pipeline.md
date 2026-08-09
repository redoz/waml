---
type: uml.Sequence
title: Preparation Pipeline
description: The immutable pipeline that prepares source, Markdown, OKF, UML, affected, and revision state for installation.
---

# Preparation Pipeline

## Notes
- [Source Bundle](../concepts/implementation/source-bundle.md) is immutable candidate input.
- `PreviousAnalyses` supplies borrowed OKF and UML reuse inputs. It does not own a revision clock.
- The provisional document catalog supplies each `DocumentId` and `DocumentRevision` to Markdown analysis. Shell quarantine can cause one accepted-catalog rebuild before OKF lowering.
- [UML Analysis](../concepts/implementation/uml-analysis.md) carries affected analysis and per-island freshness as output metadata.
- Current UML analysis visits every claimed concept. Affected analysis does not schedule an affected-only semantic pass.
- Preparation does not mutate the live editor snapshot. The caller can install the returned candidate after every stage succeeds.

## Lifelines
- [Editor Session](../concepts/implementation/editor-session.md) as caller
- [waml Core Crate](../concepts/implementation/waml-core-crate.md) as preparation
- [Source Bundle](../concepts/implementation/source-bundle.md) as source
- [Markdown Syntax](../concepts/implementation/markdown-syntax.md) as markdown
- [OKF Analysis](../concepts/implementation/okf-analysis.md) as okf
- [UML Analysis](../concepts/implementation/uml-analysis.md) as uml
- [Prepared Candidate](../concepts/implementation/prepared-candidate.md) as candidate

## Messages
- caller calls preparation `prepare_candidate(candidate_source, previous, candidate_revision)`
- preparation calls source `read candidate documents and bundle-relative identities`
- source returns `immutable candidate documents` to preparation
- preparation calls okf `construct a provisional document catalog with stable document identities and revisions`
- okf returns `provisional document catalog` to preparation
- preparation calls markdown `parse, reparse, promote, or reuse syntax with catalog document identities and revisions`
- markdown returns `Markdown syntax snapshots and quarantined document identities` to preparation
- opt
  - when `one or more shell-failed documents are quarantined`
    - preparation calls okf `rebuild the accepted catalog without quarantined documents`
    - okf returns `accepted document catalog` to preparation
- preparation calls okf `derive the OKF bundle from the accepted catalog and Markdown syntax set`
- okf returns `accepted document catalog and OKF analysis` to preparation
- preparation calls uml `analyze every claimed concept and build view projections`
- uml returns `UML analysis with affected closure and freshness metadata` to preparation
- alt
  - when `a preparation stage fails`
    - preparation returns `analysis error; live snapshot unchanged` to caller
  - else
    - preparation calls candidate `assemble source, OKF, UML, affected, and revision state`
    - candidate returns `immutable PreparedCandidate` to preparation
    - preparation returns `PreparedCandidate` to caller
