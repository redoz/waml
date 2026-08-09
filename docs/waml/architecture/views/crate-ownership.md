---
type: Diagram
title: Crate Ownership
description: The current dependency direction and ownership of the six production WAML crates.
profile: uml-domain
---

# Crate Ownership

## Notes
- A `depends` arrow points from a consuming crate to the local crate named in its Cargo manifest.
- `waml-editor` depends on `waml` and `waml-markdown-editor`.
- `waml-markdown-editor` depends on `waml-syntax`.
- `waml` depends on `waml-syntax`.
- `waml-ops-dto` depends on `waml`.
- `waml-cli` depends on `waml` and `waml-ops-dto`.
- `waml-ui-test` and `waml-ui-test-macros` are workspace test-support crates. They are outside this six-crate product ownership view.
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
