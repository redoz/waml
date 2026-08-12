---
type: uml.Class
title: Document Host
description: The owner of open-tab state and the registry and lifecycle of live document views.
stereotype: runtime
sources:
  - id: document-host
    resource: ../../../../../crates/waml-editor/src/document_host.rs
    title: crates/waml-editor/src/document_host.rs::DocumentHost
  - id: document-tabs
    resource: ../../../../../crates/waml-editor/src/doc_tabs.rs
    title: crates/waml-editor/src/doc_tabs.rs::OpenTabs and DocTabs
---

# Document Host

## Relationships
- depends [Editor Session](./editor-session.md)

## Notes
- `OpenTabs` owns preview, permanent, active, close, and fallback tab state. `DocTabs` draws that state and emits tab actions.
- `DocumentHost` owns `OpenTabs` and the live `DocView` registry. It activates, reconciles, and removes views without choosing a concrete document family.
- `DocumentHost::anchors` stores the selection, camera, or scroll anchor of each inactive tab. The host restores that anchor when the tab becomes active.
- A removed tab cannot retain a live view.
- When preview replacement removes the old tab, `reconcile_registry` removes its view and returns it for lifecycle cleanup.
- The test `prepared_preview_replacement_drops_the_old_live_view` fixes this architecture invariant.
