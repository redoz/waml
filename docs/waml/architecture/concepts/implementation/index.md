# Implementation concepts

## Crates

* [waml Syntax Crate](./waml-syntax-crate.md) - The crate that owns immutable Markdown syntax and incremental reparse.
* [waml Core Crate](./waml-core-crate.md) - The crate that owns bundle analysis, semantic edits, projections, layout, and index generation.
* [waml Markdown Editor Crate](./waml-markdown-editor-crate.md) - The crate that owns Markdown reading and editing.
* [waml Editor Crate](./waml-editor-crate.md) - The crate that owns the editor product and its platform adapters.
* [waml Operations DTO Crate](./waml-ops-dto-crate.md) - The crate that owns the semantic-operation wire contract.
* [waml CLI Crate](./waml-cli-crate.md) - The crate that owns command-line and language-server hosts.

## Analysis state

* [Source Bundle](./source-bundle.md) - An immutable candidate set of source documents.
* [Markdown Syntax](./markdown-syntax.md) - A revisioned immutable Markdown syntax snapshot.
* [OKF Analysis](./okf-analysis.md) - Markdown catalog analysis and OKF lowering for one bundle revision.
* [UML Analysis](./uml-analysis.md) - UML syntax, semantic analysis, and projection for one bundle revision.
* [Prepared Candidate](./prepared-candidate.md) - Fully prepared immutable state that can replace the live snapshot.
* [Affected Analysis](./affected-analysis.md) - The affected documents, syntax islands, and diagrams for one analysis.

## Editor runtime

* [App Shell](./app-shell.md) - The editor composition root and user-action coordinator.
* [Editor Session](./editor-session.md) - The owner of the live revisioned editor snapshot and edit transaction.
* [Document Host](./document-host.md) - The owner of open tabs and live document views.
* [Markdown Editor](./markdown-editor.md) - The WAML-owned Markdown session, input, layout, and widget runtime.
* [Diagram Renderer](./diagram-renderer.md) - The owner of class and behavior diagram presentation.
* [Platform Adapter](./platform-adapter.md) - The native and browser boundary for loading, saving, and external URLs.
