# WAML OKF extensions

Producer-specific extensions layered on top of the plain OKF substrate
(`docs/specs/OKF_SPEC.md`, untouched). These are WAML-side conventions read
from an `index.md`'s frontmatter; a strict OKF-only consumer that does not
know them degrades to the plain listing — the extension keys are ordinary
unknown frontmatter to it.

## `index.md` frontmatter

Any `index.md` may open with a frontmatter block declaring the keys below —
the bundle root included. The OKF substrate permits only `okf_version` in a
root `index.md` (`OKF_SPEC.md` §12) and no frontmatter at all elsewhere; these
keys are the WAML extension to that, and a strict OKF consumer sees ordinary
unknown keys either way (see *Strict-consumer degradation*). The root is not
an incidental case: it is where a bundle declares the profile or view chain
every directory under it inherits.

The keys:

- `title` — overrides the H1 as the folder's title (existing OKF behavior,
  documented here for completeness).
- `profile` — the folder's locally declared profile name (e.g.
  `uml-domain`). What is in *effect* for the folder is
  `Bundle::resolved_profile`, which falls back to an ancestor's declaration
  or the built-in default when this key is absent.
- `view` — the folder's locally declared view middleware chain, a plain
  scalar (`view: outline`) for a one-element chain or a flow/blocked
  sequence (`view: [hide-refs, group-by-tag]`) for a multi-element chain.
  The first entry is outermost. What is in effect is
  `Bundle::resolved_view`, a `Chain` — see
  `docs/superpowers/specs/2026-08-05-folder-view-middleware-design.md`.

## Packages: `profile: uml-domain`

A folder declaring `profile: uml-domain` is a **UML package**, and a surface
that lists it draws the package glyph (`box`) rather than the plain folder
glyph — the tree panel and the folder tab both, since both resolve the same
`IconId` against the same icon table.

Two declarations are needed, and getting only one of them is the mistake a
bundle author makes first:

1. **The parent** must resolve the `uml` view stage — by declaring
   `view: uml`, or by declaring `profile: uml-domain` itself and inheriting
   the profile's `["uml"]` default chain. The stage stamps rows while
   projecting a *listing*, so it is the listing's own chain that matters.
2. **The folder itself** must declare `profile: uml-domain`. The check is on
   the folder's *locally declared* profile, not `resolved_profile`: a folder
   that merely inherits `uml-domain` from an ancestor is not marked a package.
   Marking a package is an author's decision, not a consequence of where the
   folder happens to sit.

A folder with (2) but not (1) is still a package by `resolved_profile` — but
no stage is running that would stamp its row, so it draws the folder glyph.
`tests/fixtures/packages` is the worked example; the two rules above are
pinned by tests in `crates/waml-editor/src/tree.rs`.

One thing this cannot reach: the tree's ROOT row takes a fixed presentation
rather than a projected row's icon, so a bundle root declaring
`profile: uml-domain` boxes in the folder view but not as the tree's root row.

Any other key is preserved verbatim (`Index::extra`) and survives a
parse → edit → re-render round-trip unchanged, even keys this version of
WAML does not interpret (forward compatibility with a producer that reads
more than this build does).

## Strict-consumer degradation

`profile` and `view` are WAML conventions, not part of the OKF substrate
itself. A strict OKF consumer that has never heard of them still parses the
document correctly: they are just two more frontmatter keys it does not
recognize, alongside whatever else a producer wrote. The folder still has a
title, a description, and an ordered member list — the plain listing — with
or without a declared view chain. Nothing about the substrate requires a
consumer to run the chain to get a valid projection of the folder.

A malformed `view:` value (for example a nested mapping, which this scalar-
or-sequence declaration does not accept) is not promoted to `Index::view`;
it stays in `Index::extra` so a re-render never erases what the author
wrote, and `Bundle::resolved_view` falls back to the root view alone for
that folder.
