---
type: uml.Class
title: Diagram Renderer
description: The document views that project analyzed UML into interactive class and behavior diagrams.
stereotype: runtime
sources:
  - { id: class-diagram-view, resource: ../../../../../crates/waml-editor/src/class_diagram_view.rs, title: crates/waml-editor/src/class_diagram_view.rs::ClassDiagramView }
  - { id: behavior-document-view, resource: ../../../../../crates/waml-editor/src/behavior_doc_view.rs, title: crates/waml-editor/src/behavior_doc_view.rs::BehaviorDocView }
---

# Diagram Renderer

## Relationships
- depends [Editor Session](./editor-session.md)
- depends [UML Analysis](./uml-analysis.md)

## Notes
- `ClassDiagramView` owns class-diagram scene synchronization, selection, camera, tools, properties, and document-view lifecycle.
- `BehaviorDocView` owns activity and sequence scene projection, selection, camera, and document-view lifecycle.
- Both renderers consume installed analysis through document-view data. They do not own or commit the editor snapshot.
