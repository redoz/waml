# Start from a Template

**Goal:** When an author makes a package, the editor shows a list of templates.
The list goes from an empty package to complete example packages. The selected
template becomes real content that the author can edit.

**Why:** An empty document is difficult to start. An author who has not written
WAML does not know the form of a good class document. An example that operates
correctly is the fastest instruction. The same list is also the fastest path to
a temporary diagram for an author who knows the format.

**Done when:** The flow to make a package shows a list with three tiers: an
empty tier, a tier with one blank diagram for each kind, and one complete
example for each UML kind. The selected tier makes a package that validates and
draws with no more edits. A test parses and draws each template.

**Status:** partial — unverified
**MVP:** no

## Notes

- The empty tier operates. The library makes a document with a title, no
  members, and valid content for each kind. The flow to make a package uses it.
- The list itself does not exist. The commands for a new model and a new
  project write a log message and do nothing more.
- The three tiers are: **Empty**, which makes a package with an index only;
  **Diagram**, which makes one document of the selected kind with a title and
  no members; and **Template**, which makes a complete example.
- There is one example for each diagram kind. There is not one large example
  for all kinds. An example that has behavior documents only must draw
  correctly, because a bundle with no class document is a usual condition.
- Each example must be sufficiently small to read on one screen. Each example
  must use the features that the [feature cut](../uml/) of that kind marks
  `MVP: yes`. Thus the examples also become a test set for the renderer.
- Templates are content, not code. Keep them in the repository as bundle
  source. Then a change to the format causes a visible failure.
- `MVP: no`. The bar has one bundle and that bundle exists. Change the flag to
  `yes` if this list becomes the least expensive method to get example coverage
  of the UML cuts.
