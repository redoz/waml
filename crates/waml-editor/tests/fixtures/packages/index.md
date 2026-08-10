---
profile: uml-domain
---

# Packages

A bundle with real nested directories: one declared UML package and one plain
folder, so a reader can tell the package glyph from the folder glyph.

Each folder's glyph comes from the profile it declares in its own frontmatter:
`profile: uml-domain` draws a box, `profile: okf` draws a book, and a folder
declaring nothing draws the plain folder glyph. The root declares
`profile: uml-domain`, so the opened top draws a box.

* [Billing](billing/) - A declared `uml-domain` package.
* [Notes](notes/) - A plain folder, declaring no profile.
