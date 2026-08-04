# Folder View as a Middleware Chain — design

Date: 2026-08-05

Supersedes `docs/superpowers/specs/2026-08-02-folder-view-design.md`. That
design's model work (frontmatter on `Index`, profile/view resolution) survives
intact; its view model (`ViewSpec` as a closed enum of four alternatives, rows
derived purely from the OKF model) is replaced. The unimplemented plan
`docs/superpowers/plans/2026-08-02-folder-view.md` is rewritten against this
document.

Nothing from either document has landed: there is no `ViewSpec` in
`crates/waml/src` today. This is a redesign before code, not a migration.

## Problem

The superseded design gives a folder one view, chosen from a fixed set, that
renders that folder's own members. Two things it cannot express:

1. **Encapsulation.** A folder whose contents are internals of a custom view
   has no way to say so. Its children are always listed, always browsable,
   always openable on their own.
2. **Projection.** A view either shows the directory as it is, or is one of
   three hardcoded alternatives. It cannot filter, regroup, rename, merge, or
   synthesize the rows it presents.

Both are the same missing idea: a view does not *own* what appears beneath it.

## The idea

A folder's view is a **middleware chain** over that folder's contents, in the
ASP.NET sense: each stage receives a context and a continuation, and decides
what to do with both.

```
rows  ──▶  [ hide-refs ]  ──▶  [ group-by-tag ]  ──▶  [ root view ]
ops   ◀──                 ◀──                    ◀──
```

Rows travel up the chain; edit operations travel back down. Each stage may pass
through, transform, drop, add, or refuse.

The terminal stage — the **root view** — is the plain OKF directory listing:
`index.md` member order, real files, real hrefs. It is the ground truth every
other stage is a lens over.

A folder with no `view:` declaration has a one-element chain containing only the
root view. That is today's behavior, expressed in the new model rather than
special-cased alongside it.

## What a middleware can do

| Behavior | Implementation |
|---|---|
| pass through | return `next(ctx)` unchanged |
| filter / transform / decorate | call `next`, post-process rows |
| synthesize | call `next`, append rows with no file behind them |
| take over | never call `next`; the folder is opaque |

"Take over" is the encapsulation case. A custom view that emits no rows for its
internals makes them invisible in the tree panel; they remain reachable by path
and by search, but not by browsing.

## The chain resolves surfaces too

A chain answers two questions, not one:

- **what rows exist** under this container
- **which surface renders** a given target

A `DocView` stays exactly what it is today — a renderer. What changes is that it
is no longer chosen by hardcoded wiring; the chain chooses it.

```rust
pub struct Row {
    // ...
    pub target: RowTarget,
    /// None ⇒ default resolution, by document type. Middleware may override.
    pub surface: Option<SurfaceId>,
}

pub trait Projection {
    // ...
    /// Which surface renders this container's own tab.
    fn surface(&self, ctx: &ProjectionCtx<'_>, next: Next<'_>) -> SurfaceId;
}
```

A `SurfaceId` is a name contributed by an extension's editor half (see
Extensions), resolved in the same table the chain already uses. It is not a
second namespace.

This is why `markdown` and `member:` are **not middleware**, though an earlier
draft of this design listed them as such. Neither projects rows:

- `view: markdown` resolves this folder's own target to the markdown surface.
- `view: member:./orders` resolves this folder's target to that member's target
  and that member's resolved surface.

Both are resolution outcomes. Treating them as projection stages made two
different axes share one vocabulary.

The root view is **total on both axes**: it owns every unclaimed row *and* the
default surface resolution. A middleware that declines to answer either question
falls through to it by the same rule.

The encapsulation case is then coherent end to end. A middleware that owns a
subtree also owns how its internals render if you do reach them, rather than
owning the listing while the surface is decided somewhere else.

### Costs

- **Virtual rows must name a surface.** There is no document to infer one from,
  so `surface: None` is legal only for rows with a real target.
- **Default resolution stays total.** An unresolvable surface degrades to the
  document-type default and emits a diagnostic. Never a blank tab.

### Scope

Land the mechanism — surface is chain-resolved, defaulting to document type —
and register exactly the surfaces that exist today: markdown reading, source,
canvas, and the folder listing. No speculative format registry.

"Adding a new format is easy" is a property to demonstrate the first time one is
actually needed, not a claim the design makes about itself. The seam earns its
generality when a second real consumer disagrees with it.

## Extensions

`Projection`, `Row`, `Chain`, and `RowPath` live in `waml`, the headless crate.
That is what keeps row projection unit-testable with no window, and it is not
negotiable. `DocView` lives in `waml-editor` and pulls in makepad, so a
middleware cannot construct one directly.

An **Extension** is the unit that spans the boundary: it contributes middleware
and profiles on the headless side, and the surfaces those middleware resolve to
on the editor side.

```rust
// waml — headless
pub trait CoreExtension {
    fn name(&self) -> &str;
    fn middleware(&self) -> Vec<(&'static str, Box<dyn Projection>)>;
    fn profiles(&self) -> Vec<ProfileDef>;
}

// waml-editor
pub trait EditorExtension {
    fn name(&self) -> &str;                              // matches its core half
    fn surfaces(&self) -> Vec<(&'static str, SurfaceFactory)>;
}
```

Two traits rather than one type, because the crate dependency points one way:
`waml` cannot name `DocView`. The halves are paired by name at startup.

An extension is a **composition** unit, not a runtime seam. Nothing resolves
*through* it. The chain looks middleware up by name exactly as before; the
extension is only what put that name in the table. There is therefore one name
table, not a middleware table plus a separate `SurfaceId` namespace.

`SurfaceFactory` is a factory, not an instance: `fn open(&self, ctx, path) ->
Box<dyn DocView>`, called when a row is opened. A `DocView` per row in a listing
would allocate widgets and fonts for rows nobody opens.

Session restore needs no persisted surface identity. Persist the `RowId`, re-run
`resolve(path)`, hand the row to its owner's editor half. Deterministic, and
`resolve` is required anyway.

### Constraints

- **Declaration only, no behavior.** An extension returns lists. The moment it
  grows `on_open` or `handle_event` it becomes a god-concept and every feature
  gets pushed into it.
- **The core half stands alone.** Tests, the LSP, and any headless consumer load
  `CoreExtension`s with no editor present. An extension's middleware must never
  *require* its surfaces to project rows; it simply cannot open them.
- **Pairing is gate-checked.** A `CoreExtension` whose middleware is reachable
  while its `EditorExtension` half is absent yields rows that cannot be opened —
  the `script_mod!` failure mode exactly. Asserted in
  `crates/waml-editor/src/script_gate.rs`, not discovered at runtime.
- **Compiled-in, not a plugin API.** "Extension" reads as "plugin" to anyone who
  meets the word cold. It is not one. No dynamic loading, no bundle-supplied
  code; the threat model is unchanged.

### Scope

Land the two traits and register exactly **one** extension: core, holding
`index`, `hide`, the `okf` and `uml-domain` profiles, and today's surfaces.

`UmlExtension` is the intended end state and the shape the seam is built for —
UML middleware, the canvas surface, and the `uml-domain` profile rules in one
place. It is deliberately **not** split out in this work. A seam is proven when a
second extension disagrees with it; manufacturing that second extension now
produces a shape fitted to a guess rather than to UML's actual needs.

### Intended second consumer

UML is the expected proof. A `uml-domain` profile would bind three things a
profile is already the hook for: a chain (a package projects its classes as rows,
`references/` hidden), surface resolution (a class concept opens the canvas, the
package opens the projected listing), and rules (legal element types, child
templates — the deferred profile system).

That binding is **not designed here**. It is named so that the seam is built
knowing what will lean on it, and so that the first consumer to disagree can
correct it before a second one calcifies it.

## Descent

Descent is an explicit value in the context, not an implicit walk. A middleware
decides, per child folder, what happens when that row expands.

```rust
pub struct ProjectionCtx<'a> {
    pub dir: &'a okf::Directory,
    pub bundle: &'a Bundle,
    /// This middleware's frontmatter params.
    pub params: &'a Frontmatter,
    /// Default descent: resolve the child's own declared chain.
    pub descend: &'a dyn Fn(&okf::Directory) -> Chain,
}
```

Three descent policies, no extra vocabulary:

- **honor the child** — `expand: Some((ctx.descend)(child))`. The child's own
  `view:` wins. The default.
- **govern the child** — `expand: Some(my_chain.clone())`. The ancestor's chain
  runs for the subtree.
- **reconfigure the child** — `expand: Some(self.wrap((ctx.descend)(child)))`.
  Inheritance becomes an ordinary middleware, written once, rather than a spec
  feature.

`descend` is a closure and `expand` is forced only when a row actually expands,
so even the governing case stays lazy. No eager subtree walk.

An earlier draft of this design put a `scope: self | subtree` key in
frontmatter. It is dropped: it was a declaration standing in for a behavior the
continuation already expresses.

## Rows, identity, and ownership

```rust
/// "/" separated, non-empty segments. Structured, not opaque.
pub struct RowPath(String);

impl RowPath {
    pub fn segments(&self) -> impl Iterator<Item = &str>;
    pub fn parent(&self) -> Option<RowPath>;
    pub fn starts_with(&self, other: &RowPath) -> bool;
}

pub struct RowId {
    /// Which middleware in this folder's chain emitted the row.
    pub owner: ViewId,
    pub path: RowPath,
}

pub struct Row {
    pub id: RowId,
    pub label: String,
    pub blurb: Option<String>,
    pub target: RowTarget,          // Concept | Folder | Virtual
    /// Folder rows only: the chain used when this row expands.
    pub expand: Option<Chain>,
    /// Advisory, for affordances. See Capabilities.
    pub caps: RowCaps,
    pub child_caps: ChildCaps,
}
```

A row's key is `<owner view id>/<path the owner resolves>`. The emitting
middleware owns the row and is the only stage that interprets its path.

### The path is structured, not opaque

A `RowPath` is *syntactically transparent, semantically owned*. Any code may
split it on `/`, take a parent, or test a prefix. Only the owner may say what a
segment means.

That buys **prefix resolution**, which is the point: restoring a saved address, a
deep link, or a bookmark without replaying the walk that produced it. Breadcrumbs
alone would not justify it — the rows already passed through carry their labels —
but session restore and deep links have no such history to draw on.

So the owner gains one method:

```rust
fn resolve(&self, ctx: &ProjectionCtx<'_>, path: &RowPath) -> Result<Vec<Row>, Unresolved>;
```

Given a path, return the rows along it, labels included. Breadcrumbs, deep links,
and session restore become the same call.

This is a real constraint on synthesizing middleware, not a free property: a
middleware that mints a virtual path must be able to resolve that path again on a
later run, from the directory alone. A synthesizing middleware whose paths are
positional or run-dependent cannot satisfy it, and must key its paths on
something stable in the model instead.

`Unresolved` is not a failure of the chain. A path that no longer resolves — the
underlying file was deleted, a filter now excludes it — falls back to the nearest
resolvable prefix, which is at worst the folder itself.

### Capabilities

`apply` is authoritative but answers only after the fact: the sole way to learn
that an op is unsupported is to attempt it. That is fine for correctness and
useless for affordances — the surface must decide whether to draw a drag handle
before a drag begins.

Rows therefore declare capabilities, in two sets:

- `caps` — about the row itself: rename, delete, move out.
- `child_caps` — about the rows beneath it: reorder, insert, accept a move in.

"I can reorder my children" is a container capability on the folder row, distinct
from "I can be reordered", which lives on each child.

Capabilities are **advisory, for rendering only**; `apply` remains the authority.
That creates one invariant, tested rather than trusted: **a declared capability
must not yield `Unsupported`.** A property test over every row of every fixture
chain enforces it. Without it, capabilities drift from behavior and the surface
draws dead affordances.

The converse is allowed: a middleware may under-declare and still accept an op.
Under-declaring hides an affordance; over-declaring breaks one.

**Ownership is total.** Any row not claimed by a middleware is emitted by the
root view with a real member href. No file in a directory can be orphaned by a
badly written middleware — the worst it can do is decline to show one.

`ViewId` is the middleware's declared name, disambiguated when a name repeats
within one chain (`group-by-tag#2`). Chain position is deliberately *not* used:
inserting a stage would invalidate every persisted `RowId` below it. Ids are
folder-scoped; there is no global registry.

Because `RowId` is stable across re-projection, selection, expansion state, and
scroll position survive a chain re-run.

## Editing

Editing is delegated, not gated. Operations travel back down the chain to the
row's owner:

```rust
pub trait Projection {
    fn project(&self, ctx: &ProjectionCtx<'_>, next: Next<'_>) -> Result<Vec<Row>, ProjectionError>;

    fn resolve(&self, ctx: &ProjectionCtx<'_>, path: &RowPath)
        -> Result<Vec<Row>, Unresolved>;

    fn apply(&self, ctx: &ProjectionCtx<'_>, path: &RowPath, op: RowOp, next: Next<'_>)
        -> Result<Vec<OkfOp>, Unsupported>;
}
```

- **root view** — `path` is the real member href; ops become the existing OKF
  ops. An identity chain therefore yields full outline editing (Enter, retitle,
  Tab/Shift-Tab, drag-reorder, bullet-zoom) with no additional work. This is
  exactly the behavior specced on 2026-08-02.
- **a filtering middleware** — its paths map one-to-one onto inner paths, so
  `apply` forwards to `next`. `hide` needs no edit code and does not break
  editing beneath it.
- **a synthesizing middleware** — owns its virtual paths, answers the ops it can
  express, returns `Unsupported` for the rest.

So "is this row editable" is not a stored property. It is whatever the owner
accepts, per operation. A row may be renameable but not reorderable.

Consequently **`outline` is not a middleware**. It produces the same rows as the
root view; editability is a surface capability, on whenever the owner accepts the
operation. The chain decides *what rows exist*; the surface decides *what you can
do to them*.

## Declaration

`view:` accepts a scalar or a sequence. A scalar is a one-element chain. First
entry is outermost.

```yaml
view: outline
view: [hide-refs, group-by-tag]
view: member:./orders
```

Params are ordinary frontmatter keys read by the named middleware via
`ctx.params`. An unrecognized name is a declaration-level failure (below), not a
silent skip.

## Bounds

A middleware that synthesizes folder rows and hands itself back as `expand`
recurses forever.

- **Depth cap, default 20**, overridable in settings (`.waml/settings.json`).
- Counts **chain-descent depth**, not directory depth: synthesized folders can
  exceed real tree depth.
- Enforced by the runner. Middleware cannot opt out.
- A **visited-directory guard** runs alongside, so a cycle trips on first
  revisit rather than after twenty levels.
- On trip: stop descending and emit a diagnostic row naming the folder and the
  chain. Never a silent truncation.

**The override is user/workspace scope, never bundle scope.** A bundle that
could raise its own cap makes the guard decorative; waml opens bundles it did
not write. Any `max_view_depth` appearing in bundle frontmatter is ignored.

## Failure

Failures reuse `waml::diagnostic::Diagnostic` (`crates/waml/src/diagnostic.rs:131`)
and `with_provenance` (`:193`). No new diagnostic channel.

**Granularity is the whole chain, not one stage.** If stage 3 of 4 fails, stages
1 and 2's output is discarded and the root view renders. A half-applied chain
produces rows no declared configuration would yield — unexplainable, and worse
than falling back.

| Failure | Span |
|---|---|
| unknown middleware name | the name in `view:` |
| bad or missing params | the param key |
| middleware returns `Err` | document-level |
| depth cap or cycle guard | document-level, names the folder that tripped it |
| unknown `SurfaceId` | the resolution site; degrades to the type default |

Surfaced in two places, from the same diagnostics:

- **Folder view** — a header strip above the fallback rows, naming the stage and
  the reason. The folder still works; blast radius is one folder.
- **Tree panel** — a marker on the folder row, so a degraded folder inside a
  collapsed subtree is not silent.

The fallback path *is* the default path — the same root view object, not a
parallel safe mode — so it cannot rot from disuse.

**No `catch_unwind`.** Middleware returns `Result`; a panic is a bug caught by
tests. A guard would work on native and lie on web, where a panic poisons the
instance regardless.

## Who writes middleware

Rust built-ins with frontmatter params. Bundle-supplied code is out of scope.

waml's threat model is "open a bundle someone else wrote". Executing code from
that bundle is a sandbox conversation of its own, not a side effect of this
design. The seam above is exactly where that boundary would land if it is ever
wanted, so deferring costs nothing.

### v1 set

Middleware:

- `index` — the root view. Terminal listing, real hrefs, full edit support,
  default surface resolution.
- `hide` — params `hide: [glob, ...]`; drops matching rows, forwards ops.

Surface resolutions (not middleware — see "The chain resolves surfaces too"):

- `markdown` — this folder's target renders in the markdown surface.
- `member:<href>` — this folder's target is that member, at that member's
  resolved surface.

`hide` is the cheapest proof that a non-identity chain works end to end, and is
immediately useful for `references/`. `kanban`, `gallery`, `group-by`, and an
`inherit` middleware stay out until a real folder wants one — the seam is what
makes them cheap to add, so there is no reason to guess now.

## Carried forward unchanged

From the 2026-08-02 design:

- `okf::Index` gains `profile`, `view`, and `extra` (unknown producer keys
  survive round-trip).
- `render_index` (`crates/waml/src/index_md.rs:42`) emits frontmatter, so a write
  path cannot silently erase a folder's declaration. Round-trip test required.
- `ProfileDef` static table with `default_view`; `resolved_profile(dir)` walks to
  the nearest declaring ancestor, self first.
- `resolved_view(dir)`: own `view:`, else inherited profile's `default_view`,
  else the root view. Now returns a `Chain` rather than a `ViewSpec`.
- Tree behavior: chevron folds, row body opens the folder's view as a tab.
- No auto-detection. A folder holding one diagram does not silently become
  `member:`.

## OKF posture

`docs/okf-spec.md` is an external standard. It stays byte-identical.

This design adds **no new deviation**. The single existing one — frontmatter in a
non-root `index.md`, already present in `parse_authored_index`
(`crates/waml/src/okf/shell.rs:434`, which reads `title` today) — is unchanged in
kind. `view:` widening from scalar to scalar-or-sequence is a YAML value shape
within a key that deviation already covers.

Degradation for a strict OKF consumer is unchanged: the frontmatter block renders
as YAML or is skipped. Members, links, and body are untouched. A waml-authored
bundle stays readable by any OKF consumer.

Still not deviating: members stay flat and link-only; no plain-text bullets; no
new reserved filenames; no sidecar files.

Deviations are recorded in `docs/specs/waml-okf-extensions.md`, one entry each,
with its strict-consumer degradation.

## Delivery order

1. **Frontmatter on `Index`** — parse `profile`/`view`/`extra`, emit them in
   `render_index`, round-trip test. No UI. (Unchanged from the superseded plan.)
2. **`Chain`, `Projection`, the runner** — root view only, including `RowPath`,
   `resolve`, and capability declaration, plus depth cap, cycle guard, and the
   failure path. Pure model work, unit-tested with no editor. An identity chain
   must reproduce the plain listing exactly.
3. **Resolution** — `ProfileDef` table, `resolved_profile`, `resolved_view`
   returning a `Chain`. No UI.
4. **Folder surface, read-only** — render the projected rows; tree row-versus-
   chevron split; diagnostics strip and tree marker. Folders open for the first
   time.
5. **Extensions** — `CoreExtension` and `EditorExtension`, name pairing
   gate-checked like `script_mod!`, one registered extension (core), default
   surface resolution by document type. Then `markdown` and `member:` fall out
   as resolutions.
6. **`hide`** — the first non-identity middleware. Proves projection and op
   forwarding on a working surface.
7. **Editing** — `apply` on the root view: Enter, retitle, Tab/Shift-Tab,
   drag-reorder, bullet-zoom. Forwarding through `hide` verified.

Steps 1–3 are headless. Step 4 is useful alone. Editing lands on a surface that
already works rather than arriving with it.

## Testing

Headless, in `waml`:

- `Index` parse: frontmatter keys promoted, unknown keys land in `extra`, an
  index with no frontmatter parses as today.
- Round-trip: parse, render, reparse, assert equality including unknown keys.
- `resolved_profile`: self beats ancestor; nearest ancestor beats further; none
  declared yields `None`.
- `resolved_view`: each resolution step in isolation; explicit local `view` beats
  an inherited profile default.
- Identity chain equals the plain OKF listing, row for row, including order.
- `hide` drops exactly the matching rows and forwards every op unchanged.
- Depth cap trips at the configured value; a self-descending middleware
  terminates and produces the diagnostic row.
- Cycle guard trips on first revisit.
- A failing stage discards earlier stages' output and yields the root view plus a
  spanned diagnostic.
- A bundle-frontmatter `max_view_depth` is ignored.
- `RowId` is stable across a re-projection with unchanged inputs.
- Every path minted by `project` resolves through `resolve` on a later run.
- An unresolvable path falls back to its nearest resolvable prefix.
- Every declared capability is accepted by `apply` — property test over all rows
  of every fixture chain.
- Surface resolution is total: a row with `surface: None` and a real target
  resolves to its document-type default.
- An unknown `SurfaceId` degrades to the type default and emits a diagnostic
  rather than yielding a blank tab.
- A virtual row with `surface: None` is rejected at construction.
- `CoreExtension`s load and project rows with no `EditorExtension` present.
- Gate: every reachable middleware name has an editor half when it resolves a
  surface.
- `apply` on the root view produces the expected OKF op batch and leaves both
  affected `index.md` files consistent.

Editor-side, verified visually and stated as such:

- Chevron folds without opening; row body opens without folding.
- An opaque folder shows no descendants in the tree.
- The diagnostics strip and the tree marker appear for a failing chain.

## Open questions

**Tab on a concept with no preceding sibling directory.** Carried forward
undecided from the 2026-08-02 design. Workflowy indents under the bullet above,
which here means promoting `orders.md` to `orders/index.md` — legal in OKF and
reversible, but a keystroke causing a structural change. The alternative is Tab
refusing unless a real directory precedes. Affects step 6 only.

**Middleware inspectability.** Once a chain runs, "why is this row here" and
"why is this one missing" cannot be answered by reading `index.md`. A per-folder
debug listing — chain resolved, each stage's row delta — is the obvious answer,
but it is not scoped here. It should land before the middleware set grows past
`hide`.
