---
type: uml.Class
title: Markdown Syntax
description: A revisioned immutable Markdown syntax tree, structure map, diagnostics, and query surface.
stereotype: runtime
sources:
  - id: markdown-snapshot
    resource: ../../../../../crates/waml-syntax/src/markdown/snapshot.rs
    title: crates/waml-syntax/src/markdown/snapshot.rs::MarkdownSyntaxSnapshot
---

# Markdown Syntax

## Notes
- `MarkdownSyntaxSnapshot` carries one document revision, immutable source text, its green and red syntax tree, a structure map, diagnostics, and syntax queries.
- `parse_markdown` builds a full snapshot. `reparse_markdown` applies validated text changes to a prior snapshot and can return an incremental update.
