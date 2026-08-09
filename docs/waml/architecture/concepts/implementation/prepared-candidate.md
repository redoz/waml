---
type: uml.Class
title: Prepared Candidate
description: Fully prepared immutable source, OKF, UML, affected, and revision state that can replace a live snapshot.
stereotype: runtime
sources:
  - { id: prepared-candidate, resource: ../../../../../crates/waml/src/analysis.rs, title: crates/waml/src/analysis.rs::PreparedCandidate }
---

# Prepared Candidate

## Relationships
- depends [Source Bundle](./source-bundle.md)
- depends [OKF Analysis](./okf-analysis.md)
- depends [UML Analysis](./uml-analysis.md)
- depends [Affected Analysis](./affected-analysis.md)

## Notes
- `PreparedCandidate` carries owned immutable source, OKF analysis, UML analysis, and the candidate revision.
- Its UML analysis carries the affected closure. A caller installs the candidate only after preparation succeeds.
