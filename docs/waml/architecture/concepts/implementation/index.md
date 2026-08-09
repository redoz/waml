# Implementation concepts

## Crates

* [waml Syntax Crate](./waml-syntax-crate.md) - The crate that owns immutable Markdown green and red syntax and incremental reparse.
* [waml Core Crate](./waml-core-crate.md) - The crate that owns source bundles, analysis, semantic edits, projection, layout, and index generation.
* [waml Markdown Editor Crate](./waml-markdown-editor-crate.md) - The crate that owns WAML Markdown reading and editing sessions, input, layout, and the Makepad widget.
* [waml Editor Crate](./waml-editor-crate.md) - The crate that owns the app shell, editor session, document host, navigation, renderers, and platform adapters.
* [waml Operations DTO Crate](./waml-ops-dto-crate.md) - The crate that owns the serde wire contract for command-line semantic operations.
* [waml CLI Crate](./waml-cli-crate.md) - The crate that owns check, format, index, query, mutation, delivery, API, and language-server hosts.
* [Source Bundle](./source-bundle.md) - An immutable candidate set of source documents and bundle-relative identities.
* [Markdown Syntax](./markdown-syntax.md) - A revisioned immutable Markdown syntax tree, structure map, diagnostics, and query surface.
* [OKF Analysis](./okf-analysis.md) - Markdown syntax and catalog analysis plus OKF lowering for one bundle revision.
* [UML Analysis](./uml-analysis.md) - UML syntax, semantic analysis, projection, diagnostics, freshness, and affected closure for one bundle revision.
* [Prepared Candidate](./prepared-candidate.md) - Fully prepared immutable source, OKF, UML, affected, and revision state that can replace a live snapshot.
* [Affected Analysis](./affected-analysis.md) - The sorted affected documents, syntax islands, and diagrams for one analysis.
* [App Shell](./app-shell.md) - The editor composition root that coordinates UI state, session changes, documents, navigation, and platform effects.
* [Editor Session](./editor-session.md) - The owner of the live revisioned source and analysis snapshot and its prepare-then-commit edit transaction.
* [Document Host](./document-host.md) - The owner of open-tab state and the registry and lifecycle of live document views.
* [Markdown Editor](./markdown-editor.md) - The WAML-owned Markdown document session, input controller, layout pipeline, and Makepad widget.
* [Diagram Renderer](./diagram-renderer.md) - The document-view coordinators and surfaces that present analyzed UML as class and behavior diagrams.
* [Platform Adapter](./platform-adapter.md) - The native and browser boundary for bundle saving, browser boot selection, and external URLs.
