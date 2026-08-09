---
type: uml.Class
title: Diagram Renderer
description: The document-view coordinators and surfaces that present analyzed UML as class and behavior diagrams.
stereotype: runtime
sources:
  - { id: class-diagram-view, resource: ../../../../../crates/waml-editor/src/class_diagram_view.rs, title: crates/waml-editor/src/class_diagram_view.rs::ClassDiagramView }
  - { id: class-diagram-surface, resource: ../../../../../crates/waml-editor/src/canvas/class/widget.rs, title: crates/waml-editor/src/canvas/class/widget.rs::ClassDiagramSurface }
  - { id: behavior-document-view, resource: ../../../../../crates/waml-editor/src/behavior_doc_view.rs, title: crates/waml-editor/src/behavior_doc_view.rs::BehaviorDocView }
  - { id: behavior-surface, resource: ../../../../../crates/waml-editor/src/canvas/behavior/mod.rs, title: crates/waml-editor/src/canvas/behavior/mod.rs::BehaviorSurface }
  - { id: document-host, resource: ../../../../../crates/waml-editor/src/document_host.rs, title: crates/waml-editor/src/document_host.rs::DocumentHost }
  - { id: body-widgets, resource: ../../../../../crates/waml-editor/src/doc_view.rs, title: crates/waml-editor/src/doc_view.rs::BodyWidgets }
---

# Diagram Renderer

## Relationships
- depends [Editor Session](./editor-session.md)
- depends [UML Analysis](./uml-analysis.md)

## Notes
- `ClassDiagramView` projects installed UML into a class scene. It installs the scene, routes surface and shell actions, and implements the `DocView` lifecycle.
- `BehaviorDocView` projects installed flow or interaction data into a behavior scene.
- It installs the behavior scene, routes surface and shell actions, and implements the `DocView` lifecycle.
- `ClassDiagramSurface` owns the class scene, viewport, interaction, placement, and selection state.
- `BehaviorSurface` owns the behavior scene, viewport, and selected behavior target.
- `DocumentHost` owns inactive per-tab selection, camera, and scroll anchors.
- `BodyWidgets` provides access to the shared class and behavior canvases, inspector, tool dock, selection toolbar, and view toolbar.
- The views and surfaces consume installed analysis. They do not own or commit the editor snapshot.
