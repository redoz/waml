# Start from a Template

**Goal:** Creating a package offers a list of templates, from an empty package
to worked example packages, and the chosen one lands as real editable content.

**Why:** A blank document is the hardest thing to write. An author who has
never written WAML has no idea what a good class document looks like, and the
fastest way to teach the format is to hand them one that already works. The
same picker is also the fastest route to a scratch diagram for someone who does
know the format.

**Done when:** The new-package flow shows a picker with at least an empty tier,
a per-kind blank diagram tier, and one worked example per UML kind; the choice
produces a package that validates and renders with no further edits; and every
shipped template is covered by a test that parses and renders it.

**Status:** partial — unverified
**MVP:** no

## Notes

- The empty tier is real: `waml::seed::new_diagram_doc` emits a titled,
  memberless, valid document per kind, and the new-package flow uses it.
- The picker itself is not built. `New model` and `New project` both log
  `"template picker is a later slice"` (`crates/waml-editor/src/app/actions.rs`
  lines 248 and 616) and do nothing else.
- The picker presents three tiers. **Empty** seeds a package with nothing but
  its index. **Diagram** seeds one titled, memberless, valid document of a
  chosen kind. **Template** seeds a worked example.
- Worked examples are one per diagram kind, not one omnibus bundle. A template
  that carries only behavior documents must open on the canvas correctly — a
  bundle with no class document is a normal case, not a degenerate one.
- Worked examples should be small enough to read in one screen and should each
  exercise the features that kind's [feature cut](../uml/) marks `MVP: yes` —
  which makes them a rendering regression suite as well as a teaching aid.
- Templates are content, not code. They belong in the repository as bundle
  source so that a change to the format breaks them visibly.
- `MVP: no`: the dogfood bar has one bundle and it already exists. Promote if
  the picker turns out to be the cheapest way to get example coverage of the
  UML cuts.
