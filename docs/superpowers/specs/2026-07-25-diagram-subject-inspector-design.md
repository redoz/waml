# Diagram subject — an inspector view for the diagram itself

## Problem

The inspector's element picker lists a diagram's contents, and the diagram
itself is row 1 — but it is drawn **disabled** and selecting it is a no-op.
There is no `Subject::Diagram`, so a diagram's own identity (title, profile,
description) is unreachable from the inspector. `Subject::Diagram` was named as a
fast-follow in `2026-07-24-inspector-groups-edges-selectable-design.md`; this is
that follow-up.

Separately, the picker's row 0 is a `Placeholder` sentinel
(`"Select an element…"`) selected whenever the subject is `Subject::None` —
which is what a class-diagram view sets on tab activation, on model reload, and
on canvas deselect.

## Scope

1. A **diagram-level projection**: `Subject::Diagram(key)` renders the diagram's
   title, kind label, profile, and description.
2. The diagram is the **fallback subject** — when nothing else is selected, the
   diagram itself is selected. The placeholder sentinel retires.

**Identity only.** No contents section (groups/nodes as nav cards), no
`DiagramDisplay` toggles, no `layout` statement list. Those are deferred below.

## Data seam (`inspector.rs`, pure)

`Subject` gains a variant:

```rust
pub enum Subject {
    #[default]
    None,
    Diagram(String),   // Diagram.key
    Classifier(String),
    Group(String),
    Edge(String),
}
```

The `String` is `Diagram.key` (`model.diagrams[].key`), matching how
`Classifier` carries a node key. Unlike `Group` — which carries a human-authored
*name* resolved across all diagrams' group trees — diagram keys are unique across
the model, so `Subject::Diagram` has no cross-resolve caveat.

`InspectorView` gains one field:

```rust
pub profile: String,   // "" for every non-diagram subject
```

`build_view` gains a `Subject::Diagram(key) => build_diagram_view(model, key)`
arm. `build_diagram_view` finds the diagram by key (returning `None` when it
resolves to nothing, like every other builder) and emits:

| `InspectorView` field | Value |
| --- | --- |
| `title` | `diagram.title` |
| `kind_label` | `"Diagram"` |
| `profile` | `diagram.profile` |
| `description` | `diagram.description` |
| `abstract_flag` | `false` |
| `stereotypes`, `attributes`, `members`, `associations` | empty |

Every other `build_*_view` sets `profile: String::new()`.

### Picker rows

`diagram_elements` no longer emits the sentinel. `PICKER_PLACEHOLDER` and
`ElementKind::Placeholder` are **deleted** — the constant has three uses, all
inside `inspector.rs`, plus one match arm in `inspector_panel.rs`.

Row order becomes `[Diagram, Group*, (Node (+ its source Edges))*]` — previously
`[Placeholder, Diagram, Group*, …]`.

`subject_to_index` gains the `Subject::Diagram(k) → (ElementKind::Diagram, k)`
arm. Its existing `unwrap_or(0)` fallback — for `Subject::None` and for any key
with no matching row — now resolves to the **diagram row** rather than the
sentinel. That is the intended reading: an unresolvable subject falls back to
the diagram, matching the fallback rule. `subject_from` returns
`Some(Subject::Diagram(key))` for
`ElementKind::Diagram` instead of `None`, so a nav card or picker row pointed at
the diagram now repoints the inspector.

## Panel (`inspector_panel.rs`)

- **`build_select_items`** — the loop starts at index `0` instead of `1` (no
  sentinel to skip), and `set_diagram_elements`'s `sel_in_items` becomes
  `Some(sel)` instead of the `sel - 1` shift. The `ElementKind::Diagram` arm
  keeps its `Icon::Frame` lead and flips `enabled: false → true`.
- **`apply_pick`** — `ElementKind::Diagram => Subject::Diagram(row.key.clone())`;
  the `Placeholder` arm goes away.
- **`subject_key`** — `Subject::Diagram(key)` returns the key, so the existing
  title/description click-to-edit override map (keyed `(subject_key, FieldId)`)
  covers the diagram subject with no new machinery. Diagram edits are as
  non-persistent as every other subject's today.
- **`fill_body_column`** — a new `body.profile` row after `body.kind`, gated
  `set_visible(!view.profile.is_empty())` exactly like the stereotype row. It
  reads as a labeled inline row: a dim `Profile` label and a brighter value on
  one line. The DSL gains that one row.
- **The `collapsed` branch stays.** The panel's default subject is
  `Subject::None` before the first sync, and the non-picker hosts
  (`classifier_preview_view`, `source_view`) still set `None`. The branch is
  simply no longer reachable in a settled class-diagram view.

Body order: title, kind, profile, stereotypes, [attributes / members /
relationships — all empty for a diagram], description.

## Host (`class_diagram_view.rs`)

Both `Subject::None` sites become `Subject::Diagram(self.active_key.clone())`:

- `sync` (~line 144) — tab activation and model reload.
- the canvas-deselect handler (~line 346) — clicking empty canvas.

That is the whole of "nothing else selected ⇒ the diagram is selected". No other
host changes: `sync_inspector_elements` already passes `model` + `diagram_key`,
and the non-picker hosts keep setting `None`.

## Known limitation

The override map is keyed by the bare subject string with no kind
discriminator, so a diagram key byte-identical to a node key would share
title/description edits. Deliberately not fixed here: the map is ephemeral and
non-persisted, so the worst case is a stale text override that a reload clears —
no data loss, no disk write.

The trigger to fix it is **persistence**: once title/description edits land
through `diagram.set` / `node.set` (`waml-ops-dto`), a mis-keyed override writes
to the wrong element's markdown. The fix at that point is a serialization
boundary on `Subject` itself — `Subject::urn()` / `Subject::parse()`, with
percent-encoding, since edge ids carry `#` (`a->b#1`) and group names are free
text. Not a separate `Selector` type: the ops DTO addresses elements with typed
per-op fields (`NodeSet { slug }`, `DiagramSet { key }`), so a string address
would be a competing scheme on the wire, not a unifying one.

## Deferred (fast-follow)

- **Contents section** — top-level groups and nodes as `MEMBERS`-style nav cards
  on the diagram subject, plus counts.
- **Display settings** — a `DISPLAY` section surfacing `DiagramDisplay`'s
  toggles (`show_attributes`, `show_roles`, `max_attributes`, …), read-only
  first, editable once `diagram.set { display }` is wired.
- **Layout statements** — listing the authored `Diagram.layout` AST.
- **Persist edits** — diagram title/description through `diagram.set`, which
  already exists on the wire with exactly those fields.

## Tests (pure, in `inspector.rs`'s `tests` module)

- `diagram_elements` row 0 is `ElementKind::Diagram`, carrying the diagram's key
  and title; no placeholder row is emitted.
- `build_view(Subject::Diagram(key))` → `title` / `profile` / `description` from
  the model, `kind_label == "Diagram"`, and `attributes` / `members` /
  `associations` / `stereotypes` all empty.
- `build_view(Subject::Diagram("nope"))` → `None`.
- Every non-diagram `build_view` leaves `profile` empty.
- `subject_to_index` resolves a `Diagram` row by key.
- `subject_from("k", ElementKind::Diagram)` → `Some(Subject::Diagram("k"))`.
- Existing placeholder assertions (`inspector.rs:722-723`, `:953`) updated for
  the retired sentinel.

## Interactive verification

The pure tests cannot see the picker's index shift or the new row. After the
change, launch the native editor on a fixture and confirm, per the usual
per-pid screenshot discipline:

- A freshly activated diagram tab shows the diagram title in the picker box and
  the diagram's identity in the body — not an empty panel.
- Selecting a node, then clicking empty canvas, returns the inspector to the
  diagram rather than blanking it.
- Picking the diagram row from the open picker works (it was disabled before)
  and the selected-row highlight lands on the right row — the off-by-one that
  the `sel - 1` removal is guarding against.
- The `Profile` row is present for a fixture whose diagram declares one, and
  absent (no stray gap) for one that does not.
