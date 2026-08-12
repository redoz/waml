---
type: uml.Class
title: Editor Session
description: The owner of the live revisioned source and analysis snapshot and its prepare-then-commit edit transaction.
stereotype: runtime
sources:
  - id: editor-session
    resource: ../../../../../crates/waml-editor/src/editor_session.rs
    title: crates/waml-editor/src/editor_session.rs::EditorSession
  - id: editor-session-snapshot
    resource: ../../../../../crates/waml-editor/src/editor_session.rs
    title: crates/waml-editor/src/editor_session.rs::EditorSessionSnapshot
---

# Editor Session

## Relationships
- depends [Source Bundle](./source-bundle.md)
- depends [OKF Analysis](./okf-analysis.md)
- depends [UML Analysis](./uml-analysis.md)
- depends [Prepared Candidate](./prepared-candidate.md)

## Notes
- `EditorSession` owns the current immutable snapshot, edit history, saved state, and monotonically increasing session revision.
- An edit is lowered against its base revision. The session prepares a candidate and installs it atomically only if preparation succeeds.
- A failed or stale edit leaves the prior snapshot and history unchanged.
