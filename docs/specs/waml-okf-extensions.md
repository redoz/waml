# WAML OKF extensions

Producer-specific extensions layered on top of the plain OKF substrate
(`docs/specs/OKF_SPEC.md`, untouched). These are WAML-side conventions read
from an `index.md`'s frontmatter; a strict OKF-only consumer that does not
know them degrades to the plain listing — the extension keys are ordinary
unknown frontmatter to it.

## `index.md` frontmatter

A non-root `index.md` may open with a frontmatter block declaring:

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
