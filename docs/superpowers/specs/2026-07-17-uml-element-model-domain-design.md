# UML Element Model — Target Domain Model

**Status:** Direction-setting design. Names the target object model; concrete
changes ship as sequel specs (see §7). No implementation follows directly from
this document.

**Date:** 2026-07-17

---

## 1. Motivation

waml's runtime model already treats structural diagrams the right way: a
classifier (`Node`) lives in a shared, model-level pool, and a diagram is a
*view* that references pool members by `members`. The same element can appear on
many diagrams; deleting it from a diagram leaves it in the model.

Behavior diagrams do **not** follow this pattern. `FlowDoc` and `SequenceDoc`
are each documented as *"model AND view"* — the behavior document owns its
nodes/edges/lifelines inline, and `BehaviorKind`'s doc comment states behavior
docs "are never classifier nodes." So activity actions and interaction elements
are diagram-local, not reusable model-level elements.

We want to close that asymmetry and add instances, staying faithful to UML while
being pragmatically opinionated. This document fixes the *target* domain model
so the sequel work has one shape to build toward.

Reference point throughout: **Sparx Enterprise Architect**, which stores every
element in one generic `t_object` table (discriminated by `Object_Type`) and
every relationship in one `t_connector` table (discriminated by
`Connector_Type`), with diagrams as pure views (`t_diagramobjects` placement).

---

## 2. Guiding principle

**Loosen and be opinionated, but do not violate UML — and do not call things
classifiers if they are not classifiers.**

Concretely:

- The shared pool holds **`Element`s**, not "classifiers." `Element` (really
  `Element` / `NamedElement` / `PackageableElement`) is UML's actual root; every
  metaclass is an Element.
- **"Classifier" is a subset predicate over element kinds**, not the name of the
  pool. Some pool members are genuine classifiers; many are not.
- UML fidelity lives in the **metaclass discriminator + per-kind rules**, not in
  separate storage buckets. One pool, honestly typed, is both pragmatic and
  UML-valid — Sparx has one element table yet still knows an Object is not a
  Class.

This corrects a current misnomer: `ClassifierType` today already holds
`Diagram`, `Behavior(BehaviorKind)`, and `Unknown` — none of which is a strict
UML Classifier. The target name is an *element/metaclass kind*, with
`is_classifier()` as a derived predicate.

---

## 3. Substrate: one Element pool + one typed edge pool

**Nodes — every entity is an `Element` in a shared, reusable, model-level pool**,
discriminated by its **metaclass**:

- structural classifiers: `Class`, `Actor`, `UseCase`
- behavior classifiers (Behavior ⊂ Class): `Activity`, `Interaction`,
  `StateMachine`
- behavior *elements* (NOT classifiers): `Action`, `Decision`, `Merge`, `Fork`,
  `Join`, `ObjectNode`
- `InstanceSpecification` (NOT a classifier)
- `Comment`/`Note` (NOT a classifier)
- `Association` is an element too — it is a Classifier *and* a Relationship

**Edges — every relationship is a typed edge, discriminated by kind**, each kind
keeping its own semantics and fields; **not** flattened into `Association`:

- structural: `Association`, `Generalization`, `Include`, `Extend`, `Dependency`
- activity: `ControlFlow`, `ObjectFlow` (carry guard / trigger / effect / carried
  type)

### 3.1 "Classifier" is a predicate

| pool member                                   | classifier? |
|-----------------------------------------------|-------------|
| Class, Actor, UseCase                         | ✓           |
| Activity / Interaction / StateMachine         | ✓ (Behavior ⊂ Class) |
| Association                                   | ✓ (also a Relationship) |
| Note / Comment                                | ✗ (Comment) |
| Action / Decision / Fork / Join / ObjectNode  | ✗ (ActivityNode) |
| InstanceSpecification                         | ✗           |
| Diagram                                       | ✗ (notation, not in the metamodel) |

Rules that require "a classifier" test this predicate rather than assuming every
pool member qualifies.

---

## 4. Behavior: model + view split

This **reverses today's "behavior doc IS model AND view."**

Activity and interaction *elements* live in the Element pool like any other
element. A behavior diagram/document becomes a **view** that places references to
pool elements — exactly how class diagrams already reference `members`. Activity
actions, decisions, etc. become reusable model-level elements; a control/object
flow is a model-level typed edge.

---

## 5. Instances

`InstanceSpecification` is **one more element kind in the same pool** — not a
separate category, not a separate store (Sparx keeps Objects in `t_object`
alongside Classes). But it is a **distinct metaclass**, not a subtype of
Classifier:

- carries a `classifier` **reference** (what it is an instance of) + **slots**
  (attribute values), rather than owning attributes/operations
- slots **conform** to the referenced classifier's attributes — a validation
  instances have that classifiers do not
- an instance's `classifier`-ref MUST point at an element that *is* a classifier
  (per §3.1) — never at another instance; you do not instantiate an instance

A **link** is the object-diagram counterpart to an Association: an edge whose
kind is "instance of an Association," connecting two instances.

---

## 6. Sequence specials

- **Lifeline** references a pool element that is a classifier (*types-by*) **or**
  selects an `InstanceSpecification` (*selects*). Because the pool is
  homogeneous, this is `ref_` → a pool element with a per-kind constraint; no
  special-case type is needed. (`Lifeline` already carries `ref_` today — this
  widens the allowed target, not the mechanism.)
- **Message** is its own edge kind and stays **interaction-local** — it is
  **ordered** (document order = time order), so its identity is bound to the
  interaction's time axis. It is **not** a reusable pool edge and **not** an
  Association. This is the deliberate "not always 1:1 between stored and runtime"
  case: activity control-flows go model-level; messages stay local.

---

## 7. Scope: sequel slices

This document is direction only. Each concrete change is its own spec → plan →
build:

1. **Element-pool rename** — `ClassifierType` → element/metaclass kind, with
   `is_classifier()` predicate. Wide blast radius; foundational for the rest.
2. **Behavior model/view split** — activity/interaction elements become pool
   elements; behavior docs become views.
3. **Instances + object diagrams** — `InstanceSpecification` element kind, slots,
   links (association instances).
4. **Instances-as-lifelines** — widen `Lifeline.ref_` targets to include
   instances, with the per-kind constraint.

Order is roughly the dependency order: (1) underpins all; (4) depends on (3).

---

## 8. Explicitly deferred

The **nodes/edges-over-generic-OKF-floor** unification is out of scope here and
built later. waml is already an OKF frontend: an OKF concept is a node, an OKF
link is an untyped edge (OKF §5.3), and `ClassifierType::Unknown` already
implements OKF §9's "tolerate unknown types gracefully." Generalizing the model
so raw OKF is an explicit degenerate frontend beneath the UML recognition layer
is a separate, later effort. This design assumes the UML recognition layer only.

---

## 9. Non-goals

- No storage-format redesign. This is the runtime/object model; on-disk
  representation is decided per sequel slice (storage and runtime are mostly, but
  not always, 1:1).
- No change to rendering pipelines beyond what a sequel slice requires.
- No new source frontends (OKF-raw, etc.) — see §8.
