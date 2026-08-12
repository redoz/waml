---
type: uml.Class
title: Affected Analysis
description: The sorted affected documents, syntax islands, and diagrams for one analysis.
stereotype: runtime
sources:
  - id: affected-analysis
    resource: ../../../../../crates/waml/src/analysis.rs
    title: crates/waml/src/analysis.rs::AffectedAnalysis
---

# Affected Analysis

## Attributes
- documents: DocumentId {0..*}
- islands: SyntaxIdentity {0..*}
- diagrams: Diagram Key {0..*}

## Notes
- `AffectedAnalysis` names the documents, syntax islands, and diagrams that one candidate analysis affects.
- Its collections are sorted and do not contain duplicates.
