# Export a Bundle

**Goal:** A reader takes the content out again, as a bundle file, as a static
site, or as files on disk.

**Why:** A format that a user cannot leave is a risk. The ability to export
makes the decision to use WAML a small risk. The static site is how a bundle
comes to a reader who installs no software.

**Done when:** The editor writes its current model as a bundle file, in the
native form and in the web form. The export command writes a complete site that
opens the embedded bundle in a browser. A bundle that a person exports and then
opens has the same content as the original.

**Status:** partial — unverified
**MVP:** no

## Notes

- Bundle export operates in the editor, and the editor opens an exported
  bundle.
- The export command writes raw files. The command contains the assets of the
  editor in compressed form. There is no option to use a different artifact
  directory, because an artifact from a different build is a different product
  from the product that the team tested.
- The browser selects its start source in a fixed order: the URL fragment, then
  an interface query, then a bundle query, then the start screen.
- An edit in an exported site changes the URL fragment in position. The edit
  does not write the bundle file that the site contains.
- The export command in the editor is one menu row, in the two forms. A native
  build opens a save dialog. A browser build starts a download.
- Image export does not exist and no goal contains it. It is the first function
  to add after the MVP. Most readers ask for it first.
- `MVP: no`. The bar is to read and to author. The bar is not to extract.
