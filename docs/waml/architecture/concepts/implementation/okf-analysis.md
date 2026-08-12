---
type: uml.Class
title: OKF Analysis
description: Markdown syntax and catalog analysis plus OKF lowering for one bundle revision.
stereotype: runtime
sources:
  - id: okf-analysis
    resource: ../../../../../crates/waml/src/analysis.rs
    title: crates/waml/src/analysis.rs::OkfAnalysis
---

# OKF Analysis

## Relationships
- depends [Source Bundle](./source-bundle.md)
- depends [Markdown Syntax](./markdown-syntax.md)

## Notes
- `OkfAnalysis` owns the Markdown syntax snapshots, catalog snapshot, lowered OKF bundle, Markdown diagnostics, and revision data for one candidate.
- It can reuse previous Markdown analysis and promoted syntax updates when their identities and revisions are valid.
