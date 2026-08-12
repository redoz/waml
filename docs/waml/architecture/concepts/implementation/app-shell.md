---
type: uml.Class
title: App Shell
description: The editor composition root that coordinates UI state, session changes, documents, navigation, and platform effects.
stereotype: runtime
sources:
  - id: app
    resource: ../../../../../crates/waml-editor/src/app.rs
    title: crates/waml-editor/src/app.rs::App
  - id: navigation-model
    resource: ../../../../../crates/waml-editor/src/navigation.rs
    title: crates/waml-editor/src/navigation.rs
  - id: navigation-controller
    resource: ../../../../../crates/waml-editor/src/app/navigation.rs
    title: crates/waml-editor/src/app/navigation.rs
---

# App Shell

## Relationships
- depends [Editor Session](./editor-session.md)
- depends [Document Host](./document-host.md)
- depends [Platform Adapter](./platform-adapter.md)

## Notes
- `App` is the editor composition root. It coordinates UI actions, responsive shell state, workspace lifecycle, navigation history, documents, and platform effects.
- `navigation.rs` owns semantic navigation targets, link resolution, breadcrumbs, and open dispositions. `app/navigation.rs` applies those results at the app boundary.
- The app shell does not own bundle analysis. It requests changes from `EditorSession` and reads installed snapshots.
