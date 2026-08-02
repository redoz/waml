# Export a Bundle

**Goal:** A reader takes the content out again — as a `.waml` file, as a
static site, or as files on disk.

**Why:** A format nobody can leave is a trap. Exportability is what makes
adopting WAML a low-risk decision, and the static site is how a bundle reaches
readers who will never install anything.

**Done when:** The editor exports its current model as a `.waml` file in both
the native and web forms; `waml export site` writes a self-contained site that
opens the embedded bundle in a browser; and a bundle exported and reopened is
identical to the original.

**Status:** partial — unverified
**MVP:** no

## Notes

- Bundle export exists in the editor, and the editor loads exported bundles
  back.
- `waml export site` is specified in
  `docs/superpowers/plans/2026-08-02-waml-export-site.md`, which also covers
  embedded brotli-compressed assets, browser boot precedence
  (`#w1.` then `?api=` then `?bundle=` then the start screen), and the
  burger-menu **Export WAML bundle…** command. It depends on
  [Bundle Envelope v1](./share-a-link.md).
- That plan explicitly defers `export svg`, `export json`, Mermaid output,
  IndexedDB, service workers, and `file://` support. Image export therefore has
  no home yet and remains the obvious first post-MVP feature.
- `MVP: no`: the dogfood bar is reading and authoring, not extraction.
