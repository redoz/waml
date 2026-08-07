---
profile: uml-domain
---

# Packages

A bundle with real nested directories: one declared UML package and one plain
folder, so a reader can tell the package glyph from the folder glyph.

The root declares `profile: uml-domain` for its `["uml"]` default view chain --
that chain is what stamps the package glyph, and it runs on the LISTING, so it
is the PARENT that must have it. A child declaring `profile: uml-domain` with a
parent that resolves no `uml` stage draws the plain folder glyph.

* [Billing](billing/) - A declared `uml-domain` package.
* [Notes](notes/) - A plain folder, declaring no profile.
