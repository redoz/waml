---
type: Diagram
title: Crate Ownership
description: The current dependency direction and ownership of the six production WAML crates.
profile: uml-domain
---

# Crate Ownership

## Notes
- The root workspace lists exactly these six crates.
- A `depends` arrow shows a production path dependency under `[dependencies]`. It points from the consumer to the dependency.
- `waml-editor` depends on `waml` and `waml-markdown-editor`.
- `waml-markdown-editor` depends on `waml-syntax`.
- `waml` depends on `waml-syntax`.
- `waml-ops-dto` depends on `waml`.
- `waml-cli` depends on `waml` and `waml-ops-dto`.
- `waml-editor` also has a dev-only path dependency on `waml-syntax`. This production view does not show dependencies under `[dev-dependencies]`.
- [FG-009 — component ports and transactions](../../waml-feature-gaps.md#fg-009-—-component-ports-and-transactions) records why this view uses dependencies and notes instead of typed component ports.

## Members

### Syntax
- [waml Syntax Crate](../concepts/implementation/waml-syntax-crate.md)

### Core
- [waml Core Crate](../concepts/implementation/waml-core-crate.md)

### Presentation
- [waml Markdown Editor Crate](../concepts/implementation/waml-markdown-editor-crate.md)

### Product surfaces
- [waml Editor Crate](../concepts/implementation/waml-editor-crate.md)
- [waml Operations DTO Crate](../concepts/implementation/waml-ops-dto-crate.md)
- [waml CLI Crate](../concepts/implementation/waml-cli-crate.md)
