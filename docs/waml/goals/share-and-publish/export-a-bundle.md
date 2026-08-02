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
- `waml export site` writes raw files. The editor's own assets are embedded and
  compressed in the command; there is no flag to point it at an artifact
  directory, because an artifact assembled anywhere but the pinned build is a
  different product than the one that was tested.
- The browser picks its startup source in a fixed order: the URL fragment
  first, then an API query, then a bundle query, then the start screen. An edit
  made in an exported site updates the fragment in place and never rewrites the
  bundle file it was served from.
- The export command in the editor is one menu row, in both forms. A native
  build opens a save dialog; a browser build triggers a download.
- Image export does not exist and has no home in any goal. It is the obvious
  first post-MVP feature and the one most readers will ask for first.
- `MVP: no`: the dogfood bar is reading and authoring, not extraction.
