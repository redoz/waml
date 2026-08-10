# Beyond UML

**Goal:** WAML becomes a general documentation tool. It becomes an internal
wiki with a source that a team can review. UML is one projection of several.

**Why:** UML is the first large feature. It is not the product. Most
documentation that a team writes is not a diagram. A tool for diagrams only
gets little use.

**Done when:** This goal has no completion condition. It is a direction. It is
not a deliverable.

**Status:** horizon
**MVP:** no

## Notes

- This direction gives one rule to the current work: no part of the core can
  assume UML. The core includes the bundle, the syntax, the model, the editor
  shell, the navigation, the search, and the sharing. UML is a layer above
  them.
- Bundle search has landed: a command palette, a search results tab, and
  in-document find, scoped to one bundle and identical on the exported static
  site. It is no longer future work.
- This direction can need these functions later. None of them is scheduled:
  search across bundles, links between bundles, a typed projection that is
  not UML such as a decision record or a procedure, more than one author at
  the same time, and comments.
- If the MVP bar needs one of these functions, make it a separate goal. Do not
  start work from this page.
