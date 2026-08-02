# Beyond UML

**Goal:** WAML becomes a general documentation tool — an internal wiki whose
source is a reviewable repository — with UML as one projection among several.

**Why:** UML is the first big feature, not the product. Most documentation a
team writes is not a diagram, and a tool that only handles diagrams gets opened
rarely.

**Done when:** Deliberately unspecified. This is a direction, not a
deliverable.

**Status:** horizon
**MVP:** no

## Notes

- The constraint this horizon places on today's work is a boundary rule, not a
  feature: nothing in the core — bundle, syntax, model, editor shell,
  navigation, search, sharing — may assume UML. UML lives in its own layer
  above them.
- Things this direction would eventually want, none of them scheduled:
  full-text search across a bundle, cross-bundle links, a non-UML typed
  projection such as an architecture decision record or a runbook, multi-author
  presence, and comments.
- Anything here that turns out to be needed for the dogfood bar should be
  pulled out into a real goal rather than built from this page.
