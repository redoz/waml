# Diagram Layout Solver Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a headless, deterministic Rust solver that turns a resolved `model::Diagram` (`## Members` groups + `## Layout` relations) plus per-node sizes into absolute pixel rectangles.

**Architecture:** A new `crates/uaml/src/solve/` module with two layers. Layer A (`resolve.rs`) turns the syntax `LayoutStatement` AST + `DiagramGroup` forest into a syntax-free `Scene` (a `Box` tree + `Constraint` list over `BoxId`s), warning-and-dropping unresolved operand references. Layer B (`geometry.rs`) solves the `Scene` into absolute `Rect`s using a **weighted union-find per axis** (both placement adjacency and alignment become coordinate equalities), packs disconnected clusters deterministically, then sizes/nests composite group boxes. Conflicts (cycles, contradictions, unknown refs) warn and drop so a diagram always renders.

**Tech Stack:** Rust (edition per `crates/uaml/Cargo.toml`). No new crates — reuses `syntax` AST enums (`Shape`, `Margin`, `Axis`, `Direction`, `Edge`, `Flag`), `model::Diagram`, and the existing `Diagnostic`/`DiagCode` infra.

## Global Constraints

- Scope is **Phase 1 only**: the headless solver in `crates/uaml` (Rust). NO WASM, NO web/TS, NO drag-to-relation inference. Do not touch `packages/`.
- **Deterministic output**: same input → byte-identical `Solved`. Use `BTreeMap` (never `HashMap`) for any slug-keyed output. No `HashMap` iteration in output paths.
- **Always renders**: `solve_diagram` never panics and never returns a partial result — every diagram node gets a `Rect`. Conflicting/unresolved constraints emit a `Diagnostic::warn` and are dropped.
- **No numeric constraint solver** (no Cassowary/kiwi/LP crate). Adjacency is a tight *equality*; alignment is an *equality*; both are handled by weighted union-find.
- **No concave shrink-hull** and **no stored coordinates** — `shrink` reserves a bounding rect like `box` in Phase 1; solved pixels are returned, never persisted.
- Reuse crate idioms: `#[derive(Debug, Clone, PartialEq)]` on data types, `Diagnostic::warn(code, msg, file, line)` for warnings, existing `DiagCode` variants where they fit.
- Coordinates: absolute, origin top-left, y-down. All coordinates are `f64`.
- Test command (whole crate): `cargo test -p uaml`. Single test: `cargo test -p uaml <substring>`.
- Do NOT `git push`. Commit locally per task. Do NOT add a `Co-Authored-By` trailer.

**Reference spec:** `docs/superpowers/specs/2026-07-12-diagram-layout-solver-design.md`.

## File Structure

- Create `crates/uaml/src/solve/mod.rs` — public types (`Size`, `SizeMap`, `SolveConfig`, `Rect`, `FlagSet`, `Solved`, `SolvedGroup`, `BoxId`, `BoxKind`, `Box`, `Scene`, `Constraint`), the `pretty` debug renderer, and the top-level `solve_diagram` entry point.
- Create `crates/uaml/src/solve/resolve.rs` — Layer A: `resolve(diagram, sizes) -> (Scene, Vec<Diagnostic>)`.
- Create `crates/uaml/src/solve/potentials.rs` — the weighted union-find (`Potentials`) used per axis.
- Create `crates/uaml/src/solve/geometry.rs` — Layer B: `solve(scene, sizes, cfg) -> (Solved, Vec<Diagnostic>)`.
- Modify `crates/uaml/src/lib.rs` — register `pub mod solve;`.
- Modify `crates/uaml/src/diagnostic.rs` — add `DiagCode::LayoutConflict`.
- Create `crates/uaml/tests/solver_golden.rs` — end-to-end golden test.

---

### Task 1: Module scaffold, public types, and `DiagCode::LayoutConflict`

Stand up the `solve` module with all public data types and a deterministic `pretty` renderer used by every later test. Add the one new diagnostic code. Everything compiles; the entry point is a stub filled in by Task 8.

**Files:**
- Create: `crates/uaml/src/solve/mod.rs`
- Modify: `crates/uaml/src/lib.rs` (register module)
- Modify: `crates/uaml/src/diagnostic.rs:8-43` (add `LayoutConflict`)
- Create: `crates/uaml/src/solve/potentials.rs` (empty placeholder so `mod potentials;` compiles — real content in Task 4)
- Create: `crates/uaml/src/solve/resolve.rs` (empty placeholder — real content in Task 2)
- Create: `crates/uaml/src/solve/geometry.rs` (empty placeholder — real content in Task 5)

**Interfaces:**
- Produces (consumed by Tasks 2–8):
  ```rust
  pub type SizeMap = std::collections::BTreeMap<String, Size>;
  pub struct Size { pub w: f64, pub h: f64 }
  pub struct SolveConfig { pub margin_px: [f64; 4], pub chip: Size }   // Default: [0,8,16,32], chip 96x28
  pub struct Rect { pub x: f64, pub y: f64, pub w: f64, pub h: f64 }
  pub struct FlagSet { pub emphasized: bool, pub collapsed: bool }
  pub struct SolvedGroup { pub rect: Rect, pub shape: Shape, pub title: Option<String>, pub depth: u8 }
  pub struct Solved { pub nodes: BTreeMap<String, Rect>, pub groups: Vec<SolvedGroup>, pub flags: BTreeMap<String, FlagSet> }
  pub enum BoxId { Node(String), Group(u32), Inline(u32) }           // Ord for deterministic keys
  pub enum BoxKind { Leaf, Group }
  pub struct Box { pub id: BoxId, pub kind: BoxKind, pub children: Vec<BoxId>, pub axis: Option<Axis>, pub shape: Shape, pub margin: Margin, pub flags: FlagSet, pub title: Option<String>, pub depth: u8 }
  pub struct Scene { pub boxes: Vec<Box>, pub constraints: Vec<Constraint> }
  pub enum Constraint { Place { a: BoxId, b: BoxId, dir: Direction }, Align { a: BoxId, a_edge: Edge, b: BoxId, b_edge: Edge } }
  pub fn pretty(solved: &Solved) -> String;
  pub fn solve_diagram(diagram: &crate::model::Diagram, sizes: &SizeMap, cfg: &SolveConfig) -> (Solved, Vec<Diagnostic>);  // stub in Task 1
  ```

- [ ] **Step 1: Add the new diagnostic code**

In `crates/uaml/src/diagnostic.rs`, add `LayoutConflict` to the `DiagCode` enum (after `LayoutCycle`):

```rust
    LayoutCycle,
    LayoutConflict,
```

And its slug arm in `as_str` (after the `LayoutCycle` arm):

```rust
            DiagCode::LayoutCycle => "layout-cycle",
            DiagCode::LayoutConflict => "layout-conflict",
```

`LayoutConflict` needs no `severity` arm — it falls through to the `_ => Severity::Error` default, and every solver site constructs it via `Diagnostic::warn` anyway.

- [ ] **Step 2: Register the module**

In `crates/uaml/src/lib.rs`, add after `pub mod slug;`:

```rust
pub mod solve;
```

- [ ] **Step 3: Create the placeholder submodule files**

Create three files so the module tree compiles. `crates/uaml/src/solve/potentials.rs`:

```rust
//! Weighted union-find over per-axis coordinates. Implemented in Task 4.
```

`crates/uaml/src/solve/resolve.rs`:

```rust
//! Layer A: resolve a `model::Diagram` into a syntax-free `Scene`. Implemented in Task 2.
```

`crates/uaml/src/solve/geometry.rs`:

```rust
//! Layer B: solve a `Scene` into absolute rectangles. Implemented in Task 5.
```

- [ ] **Step 4: Write the failing test**

Create `crates/uaml/src/solve/mod.rs` with the test at the bottom first (the types come in Step 6). For now, write the whole file including the test:

```rust
//! Diagram layout solver: resolve a `model::Diagram` into absolute pixel rects.
//! See docs/superpowers/specs/2026-07-12-diagram-layout-solver-design.md.

use std::collections::BTreeMap;
use crate::diagnostic::Diagnostic;
use crate::syntax::{Axis, Shape, Margin, Direction, Edge};

pub mod resolve;
pub mod potentials;
pub mod geometry;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size { pub w: f64, pub h: f64 }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect { pub x: f64, pub y: f64, pub w: f64, pub h: f64 }

pub type SizeMap = BTreeMap<String, Size>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolveConfig { pub margin_px: [f64; 4], pub chip: Size }

impl Default for SolveConfig {
    fn default() -> Self {
        SolveConfig { margin_px: [0.0, 8.0, 16.0, 32.0], chip: Size { w: 96.0, h: 28.0 } }
    }
}

impl SolveConfig {
    /// Pixel gap for a margin level.
    pub fn margin(&self, m: Margin) -> f64 {
        match m {
            Margin::No => self.margin_px[0],
            Margin::Small => self.margin_px[1],
            Margin::Medium => self.margin_px[2],
            Margin::Large => self.margin_px[3],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FlagSet { pub emphasized: bool, pub collapsed: bool }

#[derive(Debug, Clone, PartialEq)]
pub struct SolvedGroup { pub rect: Rect, pub shape: Shape, pub title: Option<String>, pub depth: u8 }

#[derive(Debug, Clone, PartialEq)]
pub struct Solved {
    pub nodes: BTreeMap<String, Rect>,
    pub groups: Vec<SolvedGroup>,
    pub flags: BTreeMap<String, FlagSet>,
}

/// Stable identity of a box in the scene.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoxId {
    Node(String),
    Group(u32),
    Inline(u32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoxKind { Leaf, Group }

#[derive(Debug, Clone, PartialEq)]
pub struct Box {
    pub id: BoxId,
    pub kind: BoxKind,
    pub children: Vec<BoxId>,
    pub axis: Option<Axis>,
    pub shape: Shape,
    pub margin: Margin,
    pub flags: FlagSet,
    pub title: Option<String>,
    pub depth: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    Place { a: BoxId, b: BoxId, dir: Direction },
    Align { a: BoxId, a_edge: Edge, b: BoxId, b_edge: Edge },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub boxes: Vec<Box>,
    pub constraints: Vec<Constraint>,
}

/// Deterministic, human-readable dump of a solved layout. Used by tests.
pub fn pretty(solved: &Solved) -> String {
    let mut out = String::new();
    for (k, r) in &solved.nodes {
        out.push_str(&format!("node {k} @ {:.0},{:.0} {:.0}x{:.0}\n", r.x, r.y, r.w, r.h));
    }
    for g in &solved.groups {
        let title = g.title.as_deref().unwrap_or("");
        out.push_str(&format!(
            "group {:?} \"{}\" d{} @ {:.0},{:.0} {:.0}x{:.0}\n",
            g.shape, title, g.depth, g.rect.x, g.rect.y, g.rect.w, g.rect.h
        ));
    }
    for (k, f) in &solved.flags {
        if f.emphasized || f.collapsed {
            out.push_str(&format!("flags {k} emphasized={} collapsed={}\n", f.emphasized, f.collapsed));
        }
    }
    out
}

/// Top-level entry: resolve then solve. Filled in by Task 8.
pub fn solve_diagram(
    _diagram: &crate::model::Diagram,
    _sizes: &SizeMap,
    _cfg: &SolveConfig,
) -> (Solved, Vec<Diagnostic>) {
    (Solved { nodes: BTreeMap::new(), groups: vec![], flags: BTreeMap::new() }, vec![])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_dumps_nodes_deterministically() {
        let mut nodes = BTreeMap::new();
        nodes.insert("b".to_string(), Rect { x: 10.0, y: 0.0, w: 200.0, h: 90.0 });
        nodes.insert("a".to_string(), Rect { x: 0.0, y: 0.0, w: 200.0, h: 90.0 });
        let solved = Solved { nodes, groups: vec![], flags: BTreeMap::new() };
        // BTreeMap orders keys: a before b.
        assert_eq!(pretty(&solved), "node a @ 0,0 200x90\nnode b @ 10,0 200x90\n");
    }

    #[test]
    fn solve_config_maps_margin_levels() {
        let cfg = SolveConfig::default();
        assert_eq!(cfg.margin(Margin::No), 0.0);
        assert_eq!(cfg.margin(Margin::Large), 32.0);
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p uaml solve::`
Expected: PASS (both tests; whole crate compiles).

- [ ] **Step 6: Commit**

```bash
git add crates/uaml/src/solve crates/uaml/src/lib.rs crates/uaml/src/diagnostic.rs
git commit -m "feat(uaml): scaffold layout solver module, types, and pretty renderer"
```

---

### Task 2: Resolve layer — box forest from `## Members` groups

Build the `Box` tree from the `DiagramGroup` forest: a leaf per member key, a group box per heading (implicit `""` group included), depth from nesting. Defaults: `shape = Shrink`, `margin = Medium`, `axis = None`, no flags, `title = Some(name)` for named groups. Constraints stay empty until Task 3.

**Files:**
- Modify: `crates/uaml/src/solve/resolve.rs` (replace placeholder)

**Interfaces:**
- Consumes: `model::Diagram`, `model::DiagramGroup`, the `Scene`/`Box`/`BoxId` types (Task 1).
- Produces (extended in Task 3): `pub fn resolve(diagram: &Diagram) -> (Scene, Vec<Diagnostic>)`, and the module-private `Builder` with `group_by_name: BTreeMap<String, BoxId>` and `node_keys: BTreeSet<String>` used by Task 3.

- [ ] **Step 1: Write the file (types + failing test)**

Put this whole file in place. If working strictly red-green, stub `add_group`'s body with `todo!()` first, run the test to see it panic, then paste the real body. `crates/uaml/src/solve/resolve.rs`:

```rust
//! Layer A: resolve a `model::Diagram` into a syntax-free `Scene`.

use std::collections::{BTreeMap, BTreeSet};
use crate::diagnostic::Diagnostic;
use crate::model::{Diagram, DiagramGroup};
use crate::syntax::{Margin, Shape};
use super::{Box, BoxId, BoxKind, Constraint, FlagSet, Scene};

struct Builder {
    boxes: Vec<Box>,
    constraints: Vec<Constraint>,
    group_by_name: BTreeMap<String, BoxId>,
    node_keys: BTreeSet<String>,
    next_group: u32,
    next_inline: u32,
}

impl Builder {
    fn new() -> Builder {
        Builder {
            boxes: vec![],
            constraints: vec![],
            group_by_name: BTreeMap::new(),
            node_keys: BTreeSet::new(),
            next_group: 0,
            next_inline: 0,
        }
    }

    fn add_group(&mut self, g: &DiagramGroup, depth: u8) -> BoxId {
        let gid = self.next_group;
        self.next_group += 1;
        let id = BoxId::Group(gid);
        if !g.name.is_empty() {
            self.group_by_name.insert(g.name.clone(), id.clone());
        }
        let mut children = Vec::new();
        for key in &g.members {
            self.node_keys.insert(key.clone());
            let leaf = BoxId::Node(key.clone());
            if !self.boxes.iter().any(|b| b.id == leaf) {
                self.boxes.push(Box {
                    id: leaf.clone(),
                    kind: BoxKind::Leaf,
                    children: vec![],
                    axis: None,
                    shape: Shape::Shrink,
                    margin: Margin::Medium,
                    flags: FlagSet::default(),
                    title: None,
                    depth,
                });
            }
            children.push(leaf);
        }
        for child in &g.children {
            let cid = self.add_group(child, depth + 1);
            children.push(cid);
        }
        let title = if g.name.is_empty() { None } else { Some(g.name.clone()) };
        self.boxes.push(Box {
            id: id.clone(),
            kind: BoxKind::Group,
            children,
            axis: None,
            shape: Shape::Shrink,
            margin: Margin::Medium,
            flags: FlagSet::default(),
            title,
            depth,
        });
        id
    }
}

pub fn resolve(diagram: &Diagram) -> (Scene, Vec<Diagnostic>) {
    let mut b = Builder::new();
    for g in &diagram.groups {
        b.add_group(g, 0);
    }
    // Layout walk (constraints + treatment + warnings) arrives in Task 3.
    let diags = vec![];
    (Scene { boxes: b.boxes, constraints: b.constraints }, diags)
}

#[cfg(test)]
mod tests {
    use super::resolve;
    use crate::model::{Diagram, DiagramGroup};
    use crate::solve::{BoxId, BoxKind};

    fn diagram(groups: Vec<DiagramGroup>, layout: Vec<crate::syntax::LayoutStatement>) -> Diagram {
        Diagram { key: "orders".into(), title: "Orders".into(), profile: "uml-domain".into(), groups, layout }
    }

    #[test]
    fn builds_box_forest_from_groups() {
        let d = diagram(
            vec![DiagramGroup {
                name: "Users".into(),
                members: vec!["customer".into(), "account".into()],
                children: vec![DiagramGroup { name: "VIP".into(), members: vec!["platinum".into()], children: vec![] }],
            }],
            vec![],
        );
        let (scene, diags) = resolve(&d);
        assert!(diags.is_empty());

        let users = scene.boxes.iter().find(|b| b.title.as_deref() == Some("Users")).unwrap();
        assert_eq!(users.kind, BoxKind::Group);
        assert_eq!(users.depth, 0);
        assert_eq!(
            users.children,
            vec![BoxId::Node("customer".into()), BoxId::Node("account".into()), BoxId::Group(1)]
        );

        let vip = scene.boxes.iter().find(|b| b.title.as_deref() == Some("VIP")).unwrap();
        assert_eq!(vip.depth, 1);
        assert!(scene.boxes.iter().any(|b| b.id == BoxId::Node("platinum".into()) && b.kind == BoxKind::Leaf));
    }
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p uaml solve::resolve`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/uaml/src/solve/resolve.rs
git commit -m "feat(uaml): resolve DiagramGroup forest into solver box tree"
```

---

### Task 3: Resolve layer — operands, treatment, and constraints

Walk `diagram.layout`: resolve every operand to a `BoxId` (bare name → group name, else slugified node key; link → slug; inline group → a fresh `Inline` box; paren → inner), apply `as`/`with` treatment onto the referenced box, and emit `Place`/`Align` constraints. An operand that resolves to nothing emits `Diagnostic::warn(UnresolvedLayoutRef, …)` and any constraint touching it is dropped.

**Files:**
- Modify: `crates/uaml/src/solve/resolve.rs`

**Interfaces:**
- Consumes: `Builder` (Task 2), syntax AST (`LayoutStatement`, `Operand`, `OperandRef`, `NameRef`, `Hint`, `Flag`, `Edge`), `slug::slugify`, `DiagCode::UnresolvedLayoutRef`.
- Produces: fully-populated `Scene.constraints` and treatment-annotated boxes.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/uaml/src/solve/resolve.rs`:

```rust
    #[test]
    fn resolves_refs_treatment_and_warns_unknown() {
        use crate::solve::Constraint;
        use crate::syntax::*;

        fn bare(name: &str) -> Operand {
            Operand { ref_: OperandRef::Name(NameRef::Bare(name.into())), axis: None, hints: vec![] }
        }
        let users_treated = Operand {
            ref_: OperandRef::Name(NameRef::Bare("Users".into())),
            axis: Some(Axis::Column),
            hints: vec![Hint::Shape(Shape::Frame)],
        };

        let d = diagram(
            vec![
                DiagramGroup { name: "Users".into(), members: vec!["customer".into(), "account".into()], children: vec![] },
                DiagramGroup { name: "Orders".into(), members: vec!["order".into()], children: vec![] },
            ],
            vec![
                LayoutStatement::Standalone(users_treated),
                LayoutStatement::Placement { operands: vec![bare("Users"), bare("Orders")], directions: vec![Direction::LeftOf] },
                LayoutStatement::Placement { operands: vec![bare("Nope"), bare("Orders")], directions: vec![Direction::LeftOf] },
            ],
        );
        let (scene, diags) = resolve(&d);

        let users = scene.boxes.iter().find(|b| b.title.as_deref() == Some("Users")).unwrap();
        assert_eq!(users.axis, Some(Axis::Column));
        assert_eq!(users.shape, Shape::Frame);

        assert_eq!(scene.constraints.len(), 1);
        assert_eq!(
            scene.constraints[0],
            Constraint::Place { a: BoxId::Group(0), b: BoxId::Group(1), dir: Direction::LeftOf }
        );

        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, crate::diagnostic::DiagCode::UnresolvedLayoutRef);
    }

    #[test]
    fn resolves_bare_node_by_slug_and_inline_group() {
        use crate::syntax::*;
        let inline = Operand {
            ref_: OperandRef::InlineGroup {
                axis: Axis::Column,
                items: vec![Operand { ref_: OperandRef::Name(NameRef::Bare("Order Line".into())), axis: None, hints: vec![] }],
            },
            axis: None,
            hints: vec![Hint::Margin(Margin::Large)],
        };
        let d = diagram(
            vec![DiagramGroup { name: "".into(), members: vec!["order-line".into()], children: vec![] }],
            vec![LayoutStatement::Standalone(inline)],
        );
        let (scene, diags) = resolve(&d);
        assert!(diags.is_empty(), "`Order Line` slugifies to order-line and resolves");
        let ig = scene.boxes.iter().find(|b| b.id == BoxId::Inline(0)).unwrap();
        assert_eq!(ig.axis, Some(Axis::Column));
        assert_eq!(ig.margin, Margin::Large);
        assert_eq!(ig.children, vec![BoxId::Node("order-line".into())]);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml solve::resolve::tests::resolves_refs`
Expected: FAIL — `resolve` ignores `diagram.layout`, so `constraints` is empty and no treatment is applied.

- [ ] **Step 3: Implement the layout walk**

In `crates/uaml/src/solve/resolve.rs`, expand imports. Replace `use crate::syntax::{Margin, Shape};` with:

```rust
use crate::diagnostic::DiagCode;
use crate::slug::slugify;
use crate::syntax::{Axis, Edge, Flag, Hint, LayoutStatement, Margin, NameRef, Operand, OperandRef, Shape};
```

Add these methods to `impl Builder` (after `add_group`):

```rust
    fn box_mut(&mut self, id: &BoxId) -> Option<&mut Box> {
        self.boxes.iter_mut().find(|b| &b.id == id)
    }

    fn apply_treatment(&mut self, id: &BoxId, op: &Operand) {
        if let Some(bx) = self.box_mut(id) {
            if let Some(ax) = op.axis {
                bx.axis = Some(ax);
            }
            for h in &op.hints {
                match h {
                    Hint::Shape(s) => bx.shape = *s,
                    Hint::Margin(m) => bx.margin = *m,
                    Hint::Flag(Flag::Emphasized) => bx.flags.emphasized = true,
                    Hint::Flag(Flag::Collapsed) => bx.flags.collapsed = true,
                }
            }
        }
    }

    fn resolve_ref(&mut self, r: &OperandRef, file: &str, diags: &mut Vec<Diagnostic>) -> Option<BoxId> {
        match r {
            OperandRef::Name(NameRef::Link { slug, .. }) => {
                if self.node_keys.contains(slug) {
                    Some(BoxId::Node(slug.clone()))
                } else {
                    self.warn_unknown(slug, file, diags);
                    None
                }
            }
            OperandRef::Name(NameRef::Bare(name)) => {
                if let Some(id) = self.group_by_name.get(name) {
                    return Some(id.clone());
                }
                let slug = slugify(name, "");
                if self.node_keys.contains(&slug) {
                    Some(BoxId::Node(slug))
                } else {
                    self.warn_unknown(name, file, diags);
                    None
                }
            }
            OperandRef::InlineGroup { axis, items } => {
                let iid = self.next_inline;
                self.next_inline += 1;
                let id = BoxId::Inline(iid);
                let mut children = Vec::new();
                for it in items {
                    if let Some(cid) = self.resolve_operand(it, file, diags) {
                        children.push(cid);
                    }
                }
                self.boxes.push(Box {
                    id: id.clone(),
                    kind: BoxKind::Group,
                    children,
                    axis: Some(*axis),
                    shape: Shape::Shrink,
                    margin: Margin::Medium,
                    flags: FlagSet::default(),
                    title: None,
                    depth: 0,
                });
                Some(id)
            }
            OperandRef::Paren(inner) => self.resolve_operand(inner, file, diags),
        }
    }

    fn resolve_operand(&mut self, op: &Operand, file: &str, diags: &mut Vec<Diagnostic>) -> Option<BoxId> {
        let id = self.resolve_ref(&op.ref_, file, diags)?;
        self.apply_treatment(&id, op);
        Some(id)
    }

    fn warn_unknown(&self, name: &str, file: &str, diags: &mut Vec<Diagnostic>) {
        diags.push(Diagnostic::warn(
            DiagCode::UnresolvedLayoutRef,
            format!("layout references unknown operand `{name}`"),
            file,
            0,
        ));
    }

    fn add_statement(&mut self, stmt: &LayoutStatement, file: &str, diags: &mut Vec<Diagnostic>) {
        match stmt {
            LayoutStatement::Standalone(op) => {
                self.resolve_operand(op, file, diags);
            }
            LayoutStatement::Placement { operands, directions } => {
                let ids: Vec<Option<BoxId>> =
                    operands.iter().map(|o| self.resolve_operand(o, file, diags)).collect();
                for (i, dir) in directions.iter().enumerate() {
                    if let (Some(a), Some(b)) = (&ids[i], &ids[i + 1]) {
                        self.constraints.push(Constraint::Place { a: a.clone(), b: b.clone(), dir: *dir });
                    }
                }
            }
            LayoutStatement::Alignment { left, right } => {
                let a = self.resolve_operand(&left.operand, file, diags);
                let b = self.resolve_operand(&right.operand, file, diags);
                if let (Some(a), Some(b)) = (a, b) {
                    self.constraints.push(Constraint::Align {
                        a,
                        a_edge: left.edge.unwrap_or(Edge::Center),
                        b,
                        b_edge: right.edge.unwrap_or(Edge::Center),
                    });
                }
            }
        }
    }
```

Then wire the walk into `resolve` (replace the `let diags = vec![];` line and the comment above it):

```rust
    let mut diags = vec![];
    for stmt in &diagram.layout {
        b.add_statement(stmt, &diagram.key, &mut diags);
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml solve::resolve`
Expected: PASS (both new tests + Task 2 test).

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/solve/resolve.rs
git commit -m "feat(uaml): resolve Layout operands, treatment, and constraints"
```

---

### Task 4: Weighted union-find (`Potentials`)

A union-find where each element carries a `f64` potential = its coordinate relative to the component root. `union(a, b, delta)` enforces `coord[b] - coord[a] = delta`; a contradictory union returns `Err(existing_delta)`. This is the per-axis engine both placement adjacency and alignment feed into.

**Files:**
- Modify: `crates/uaml/src/solve/potentials.rs` (replace placeholder)

**Interfaces:**
- Produces (used by Task 5): `pub struct Potentials`; `Potentials::new(n)`, `find(&mut self, i) -> (usize, f64)` (root + `coord_i - coord_root`), `union(&mut self, a, b, delta) -> Result<(), f64>`.

- [ ] **Step 1: Write the failing test**

Write the whole file with the test; the impl is Step 3. `crates/uaml/src/solve/potentials.rs`:

```rust
//! Weighted union-find over per-axis coordinates. `pot[i]` is `coord[i]`
//! relative to the component root, so alignment/adjacency equalities compose.

pub struct Potentials {
    parent: Vec<usize>,
    pot: Vec<f64>,
}

impl Potentials {
    pub fn new(n: usize) -> Potentials {
        Potentials { parent: (0..n).collect(), pot: vec![0.0; n] }
    }

    /// Root of `i` and `coord[i] - coord[root]`, with path compression.
    pub fn find(&mut self, i: usize) -> (usize, f64) {
        let p = self.parent[i];
        if p == i {
            return (i, 0.0);
        }
        let (root, pr) = self.find(p);
        self.pot[i] += pr;
        self.parent[i] = root;
        (root, self.pot[i])
    }

    /// Enforce `coord[b] - coord[a] = delta`. `Err(existing)` if `a` and `b`
    /// are already related with a different delta (a contradiction).
    pub fn union(&mut self, a: usize, b: usize, delta: f64) -> Result<(), f64> {
        let (ra, da) = self.find(a);
        let (rb, db) = self.find(b);
        if ra == rb {
            let existing = db - da;
            if (existing - delta).abs() > 1e-6 {
                return Err(existing);
            }
            return Ok(());
        }
        self.parent[rb] = ra;
        self.pot[rb] = delta + da - db;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_composed_offsets() {
        let mut p = Potentials::new(3);
        p.union(0, 1, 10.0).unwrap(); // coord1 = coord0 + 10
        p.union(1, 2, 5.0).unwrap();  // coord2 = coord1 + 5
        let (r0, d0) = p.find(0);
        let (r2, d2) = p.find(2);
        assert_eq!(r0, r2);
        assert!((d2 - d0 - 15.0).abs() < 1e-9, "coord2 - coord0 == 15");
    }

    #[test]
    fn detects_contradiction() {
        let mut p = Potentials::new(2);
        p.union(0, 1, 10.0).unwrap();
        assert!(p.union(0, 1, 12.0).is_err());
        assert!(p.union(0, 1, 10.0).is_ok(), "consistent re-union is fine");
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p uaml solve::potentials`
Expected: PASS (both tests).

- [ ] **Step 3: Commit**

```bash
git add crates/uaml/src/solve/potentials.rs
git commit -m "feat(uaml): weighted union-find for the layout solver"
```

---

### Task 5: Geometry core — flat cluster solve (leaves)

Solve a flat set of leaf boxes into absolute rects. Every `Place` contributes two equalities (along-axis gap + cross-axis center); every `Align` contributes an equality on each axis its edges share. Contradictions warn (`LayoutConflict`) and drop. Components are packed X left-to-right in list order, Y normalized to a shared top band. Groups/shapes/nesting arrive in Task 6.

**Files:**
- Modify: `crates/uaml/src/solve/geometry.rs` (replace placeholder)
- Modify: `crates/uaml/src/solve/mod.rs` (route `solve_diagram`? No — leave stub; `solve` is called directly by tests here)

**Interfaces:**
- Consumes: `Potentials` (Task 4); `Scene`, `Constraint`, `Box`, `BoxId`, `Rect`, `Size`, `SizeMap`, `SolveConfig`, `Solved` (Task 1); `syntax::{Direction, Edge, Margin}`; `DiagCode::LayoutConflict`.
- Produces (extended in Task 6): `pub fn solve(scene: &Scene, sizes: &SizeMap, cfg: &SolveConfig) -> (Solved, Vec<Diagnostic>)`, and the module-private `solve_cluster` reused by Task 6's recursion.

- [ ] **Step 1: Write the failing test**

Write the whole file with the tests; the impl is Step 3. `crates/uaml/src/solve/geometry.rs`:

```rust
//! Layer B: solve a `Scene` into absolute rectangles via weighted union-find.

use std::collections::BTreeMap;
use crate::diagnostic::{DiagCode, Diagnostic};
use crate::syntax::{Direction, Edge, Margin};
use super::potentials::Potentials;
use super::{Box, BoxId, BoxKind, Constraint, Rect, Scene, Size, SizeMap, SolveConfig, Solved};

fn margin_rank(m: Margin) -> u8 {
    match m { Margin::No => 0, Margin::Small => 1, Margin::Medium => 2, Margin::Large => 3 }
}
fn max_margin(a: Margin, b: Margin) -> Margin {
    if margin_rank(a) >= margin_rank(b) { a } else { b }
}

/// Which axes an alignment edge constrains: (x, y).
fn edge_axes(e: Edge) -> (bool, bool) {
    match e {
        Edge::Left | Edge::Right => (true, false),
        Edge::Top | Edge::Bottom => (false, true),
        Edge::Center => (true, true),
    }
}
fn off_x(e: Edge, w: f64) -> f64 {
    match e { Edge::Left => 0.0, Edge::Right => w, Edge::Center => w / 2.0, _ => 0.0 }
}
fn off_y(e: Edge, h: f64) -> f64 {
    match e { Edge::Top => 0.0, Edge::Bottom => h, Edge::Center => h / 2.0, _ => 0.0 }
}

fn eq(p: &mut Potentials, a: usize, b: usize, delta: f64, diags: &mut Vec<Diagnostic>) {
    if p.union(a, b, delta).is_err() {
        diags.push(Diagnostic::warn(
            DiagCode::LayoutConflict,
            "conflicting layout constraint dropped",
            "",
            0,
        ));
    }
}

/// Position a flat set of boxes (given size + margin per id) under a constraint
/// list. Returns one absolute `Rect` per input id.
pub(super) fn solve_cluster(
    ids: &[BoxId],
    dims: &BTreeMap<BoxId, (Size, Margin)>,
    constraints: &[Constraint],
    cfg: &SolveConfig,
    diags: &mut Vec<Diagnostic>,
) -> BTreeMap<BoxId, Rect> {
    let n = ids.len();
    let index: BTreeMap<BoxId, usize> =
        ids.iter().enumerate().map(|(i, id)| (id.clone(), i)).collect();
    let mut px = Potentials::new(n);
    let mut py = Potentials::new(n);

    for c in constraints {
        match c {
            Constraint::Place { a, b, dir } => {
                let (Some(&ia), Some(&ib)) = (index.get(a), index.get(b)) else { continue };
                let (sa, ma) = dims[a];
                let (sb, mb) = dims[b];
                let gap = cfg.margin(max_margin(ma, mb));
                match dir {
                    Direction::LeftOf => {
                        eq(&mut px, ia, ib, sa.w + gap, diags);
                        eq(&mut py, ia, ib, (sa.h - sb.h) / 2.0, diags);
                    }
                    Direction::RightOf => {
                        eq(&mut px, ia, ib, -(sb.w + gap), diags);
                        eq(&mut py, ia, ib, (sa.h - sb.h) / 2.0, diags);
                    }
                    Direction::Above => {
                        eq(&mut py, ia, ib, sa.h + gap, diags);
                        eq(&mut px, ia, ib, (sa.w - sb.w) / 2.0, diags);
                    }
                    Direction::Below => {
                        eq(&mut py, ia, ib, -(sb.h + gap), diags);
                        eq(&mut px, ia, ib, (sa.w - sb.w) / 2.0, diags);
                    }
                }
            }
            Constraint::Align { a, a_edge, b, b_edge } => {
                let (Some(&ia), Some(&ib)) = (index.get(a), index.get(b)) else { continue };
                let (sa, _) = dims[a];
                let (sb, _) = dims[b];
                let (ax, ay) = edge_axes(*a_edge);
                let (bx, by) = edge_axes(*b_edge);
                let mut shared = false;
                if ax && bx {
                    eq(&mut px, ia, ib, off_x(*a_edge, sa.w) - off_x(*b_edge, sb.w), diags);
                    shared = true;
                }
                if ay && by {
                    eq(&mut py, ia, ib, off_y(*a_edge, sa.h) - off_y(*b_edge, sb.h), diags);
                    shared = true;
                }
                if !shared {
                    diags.push(Diagnostic::warn(
                        DiagCode::LayoutConflict,
                        "alignment edges share no axis",
                        "",
                        0,
                    ));
                }
            }
        }
    }

    // Resolve relative coordinates + roots per axis.
    let mut relx = vec![0.0; n];
    let mut rootx = vec![0usize; n];
    let mut rely = vec![0.0; n];
    let mut rooty = vec![0usize; n];
    for i in 0..n {
        let (rx, dx) = px.find(i);
        let (ry, dy) = py.find(i);
        rootx[i] = rx;
        relx[i] = dx;
        rooty[i] = ry;
        rely[i] = dy;
    }
    let w_of = |i: usize| dims[&ids[i]].0.w;

    // X components packed left-to-right by first-member list order.
    let mut xcomps: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..n {
        xcomps.entry(rootx[i]).or_default().push(i);
    }
    let mut order: Vec<(usize, Vec<usize>)> = xcomps.into_iter().collect();
    order.sort_by_key(|(_, v)| *v.iter().min().unwrap());
    let gap = cfg.margin(Margin::Medium);
    let mut originx: BTreeMap<usize, f64> = BTreeMap::new();
    let mut cursor = 0.0;
    for (root, members) in &order {
        let minrel = members.iter().map(|&i| relx[i]).fold(f64::INFINITY, f64::min);
        let maxend = members
            .iter()
            .map(|&i| relx[i] + w_of(i))
            .fold(f64::NEG_INFINITY, f64::max);
        originx.insert(*root, cursor - minrel);
        cursor += (maxend - minrel) + gap;
    }

    // Y components normalized so each top sits at 0 (shared band).
    let mut ycomps: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..n {
        ycomps.entry(rooty[i]).or_default().push(i);
    }
    let mut originy: BTreeMap<usize, f64> = BTreeMap::new();
    for (root, members) in &ycomps {
        let minrel = members.iter().map(|&i| rely[i]).fold(f64::INFINITY, f64::min);
        originy.insert(*root, -minrel);
    }

    let mut out = BTreeMap::new();
    for i in 0..n {
        let (sz, _) = dims[&ids[i]];
        let x = originx[&rootx[i]] + relx[i];
        let y = originy[&rooty[i]] + rely[i];
        out.insert(ids[i].clone(), Rect { x, y, w: sz.w, h: sz.h });
    }
    out
}

pub fn solve(scene: &Scene, sizes: &SizeMap, cfg: &SolveConfig) -> (Solved, Vec<Diagnostic>) {
    let mut diags = vec![];
    // Task 5: flat — position leaf boxes only. Groups/shapes arrive in Task 6.
    let mut dims: BTreeMap<BoxId, (Size, Margin)> = BTreeMap::new();
    let mut ids = vec![];
    for b in &scene.boxes {
        if b.kind == BoxKind::Leaf {
            if let BoxId::Node(key) = &b.id {
                let sz = sizes.get(key).copied().unwrap_or(Size { w: 100.0, h: 40.0 });
                dims.insert(b.id.clone(), (sz, b.margin));
                ids.push(b.id.clone());
            }
        }
    }
    let rects = solve_cluster(&ids, &dims, &scene.constraints, cfg, &mut diags);
    let mut nodes = BTreeMap::new();
    for (id, r) in rects {
        if let BoxId::Node(key) = id {
            nodes.insert(key, r);
        }
    }
    (Solved { nodes, groups: vec![], flags: BTreeMap::new() }, diags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{Margin, Shape};
    use super::super::{FlagSet, pretty};

    fn leaf(k: &str) -> Box {
        Box {
            id: BoxId::Node(k.into()),
            kind: BoxKind::Leaf,
            children: vec![],
            axis: None,
            shape: Shape::Shrink,
            margin: Margin::Medium,
            flags: FlagSet::default(),
            title: None,
            depth: 0,
        }
    }

    fn sizes(keys: &[&str], w: f64, h: f64) -> SizeMap {
        let mut m = SizeMap::new();
        for k in keys {
            m.insert((*k).into(), Size { w, h });
        }
        m
    }

    #[test]
    fn solves_a_row_of_three() {
        let scene = Scene {
            boxes: vec![leaf("a"), leaf("b"), leaf("c")],
            constraints: vec![
                Constraint::Place { a: BoxId::Node("a".into()), b: BoxId::Node("b".into()), dir: Direction::LeftOf },
                Constraint::Place { a: BoxId::Node("b".into()), b: BoxId::Node("c".into()), dir: Direction::LeftOf },
            ],
        };
        let (solved, diags) = solve(&scene, &sizes(&["a", "b", "c"], 200.0, 90.0), &SolveConfig::default());
        assert!(diags.is_empty());
        assert_eq!(
            pretty(&solved),
            "node a @ 0,0 200x90\nnode b @ 216,0 200x90\nnode c @ 432,0 200x90\n"
        );
    }

    #[test]
    fn contradiction_warns_and_still_renders() {
        let scene = Scene {
            boxes: vec![leaf("a"), leaf("b")],
            constraints: vec![
                Constraint::Place { a: BoxId::Node("a".into()), b: BoxId::Node("b".into()), dir: Direction::LeftOf },
                Constraint::Place { a: BoxId::Node("b".into()), b: BoxId::Node("a".into()), dir: Direction::LeftOf },
            ],
        };
        let (solved, diags) = solve(&scene, &sizes(&["a", "b"], 200.0, 90.0), &SolveConfig::default());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, DiagCode::LayoutConflict);
        assert_eq!(solved.nodes.len(), 2, "always renders every node");
    }
}
```

- [ ] **Step 2: Run test to verify it fails, then implement**

Run: `cargo test -p uaml solve::geometry`
Expected: FAIL first if you stub `solve_cluster`/`solve` with `todo!()`. With the full file above, it PASSES. (`a left of b left of c` → x deltas 200+16=216; equal heights → y centers coincide at 0.)

- [ ] **Step 3: Commit**

```bash
git add crates/uaml/src/solve/geometry.rs
git commit -m "feat(uaml): flat geometry solve via per-axis union-find"
```

---

### Task 6: Groups — recursion, internal arrangement, shapes, margins, nesting

Rewrite `solve` to recurse over the box tree. Each group solves its children (post-order), arranges them (`as row`/`as column` → adjacency in list order; no axis → clump = `solve_cluster` with no internal constraints, i.e. an X-flow row), then wraps them: content bounds + `margin` inset → the group's outer size, emitted as a `SolvedGroup` hull (`frame`/`box`/`shrink` all reserve the rect in Phase 1). Constraints are assigned to the cluster whose direct children are both endpoints (siblings); a non-sibling constraint warns and drops.

**Files:**
- Modify: `crates/uaml/src/solve/geometry.rs` (add recursion + rewrite `solve`; keep `solve_cluster`)

**Interfaces:**
- Consumes: `solve_cluster` (Task 5), `syntax::Axis`, `SolvedGroup` (Task 1).
- Produces: recursive `solve` that fills `Solved.groups`; module-private `Laid`, `solve_box`, `axis_constraints`, `endpoints`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/uaml/src/solve/geometry.rs`:

```rust
    fn group(id: u32, children: Vec<BoxId>, axis: Option<crate::syntax::Axis>, shape: Shape, title: &str) -> Box {
        Box {
            id: BoxId::Group(id),
            kind: BoxKind::Group,
            children,
            axis,
            shape,
            margin: Margin::Medium,
            flags: FlagSet::default(),
            title: Some(title.into()),
            depth: 0,
        }
    }

    #[test]
    fn column_group_with_frame_wraps_members_with_margin() {
        use crate::syntax::Axis;
        let scene = Scene {
            boxes: vec![
                leaf("a"),
                leaf("b"),
                group(0, vec![BoxId::Node("a".into()), BoxId::Node("b".into())], Some(Axis::Column), Shape::Frame, "Users"),
            ],
            constraints: vec![],
        };
        let (solved, diags) = solve(&scene, &sizes(&["a", "b"], 200.0, 90.0), &SolveConfig::default());
        assert!(diags.is_empty());
        assert_eq!(
            pretty(&solved),
            "node a @ 16,16 200x90\n\
             node b @ 16,122 200x90\n\
             group Frame \"Users\" d0 @ 0,0 232x228\n"
        );
    }

    #[test]
    fn non_sibling_constraint_warns() {
        use crate::syntax::Axis;
        // `a` lives inside group Users; relating it to top-level `c` is not a sibling relation.
        let scene = Scene {
            boxes: vec![
                leaf("a"),
                leaf("c"),
                group(0, vec![BoxId::Node("a".into())], Some(Axis::Column), Shape::Shrink, "Users"),
            ],
            constraints: vec![Constraint::Place {
                a: BoxId::Node("a".into()),
                b: BoxId::Node("c".into()),
                dir: Direction::LeftOf,
            }],
        };
        let (solved, diags) = solve(&scene, &sizes(&["a", "c"], 200.0, 90.0), &SolveConfig::default());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, DiagCode::LayoutConflict);
        assert_eq!(solved.nodes.len(), 2, "still renders both nodes");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml solve::geometry::tests::column_group`
Expected: FAIL — the Task 5 `solve` ignores group boxes, so `groups` is empty and `a`/`b` are placed as bare leaves.

- [ ] **Step 3: Implement recursion**

In `crates/uaml/src/solve/geometry.rs`, add `Axis` and `SolvedGroup` to imports:

```rust
use crate::syntax::{Axis, Direction, Edge, Margin, Shape};
use super::{Box, BoxId, BoxKind, Constraint, Rect, Scene, Size, SizeMap, SolveConfig, Solved, SolvedGroup};
```

Add these items above the old `solve` function:

```rust
/// A solved subtree in local coordinates: the box's outer size, every
/// descendant leaf rect, and every descendant group hull.
struct Laid {
    size: Size,
    rects: BTreeMap<BoxId, Rect>,
    groups: Vec<SolvedGroup>,
}

fn endpoints(c: &Constraint) -> (&BoxId, &BoxId) {
    match c {
        Constraint::Place { a, b, .. } => (a, b),
        Constraint::Align { a, b, .. } => (a, b),
    }
}

fn axis_constraints(b: &Box) -> Vec<Constraint> {
    let dir = match b.axis {
        Some(Axis::Row) => Direction::LeftOf,
        Some(Axis::Column) => Direction::Above,
        None => return vec![],
    };
    b.children
        .windows(2)
        .map(|w| Constraint::Place { a: w[0].clone(), b: w[1].clone(), dir })
        .collect()
}

/// Union of child rects → (minX, minY, maxX, maxY). Empty → all zero.
fn bounds(rects: &BTreeMap<BoxId, Rect>, ids: &[BoxId]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for id in ids {
        let r = rects[id];
        min_x = min_x.min(r.x);
        min_y = min_y.min(r.y);
        max_x = max_x.max(r.x + r.w);
        max_y = max_y.max(r.y + r.h);
    }
    if ids.is_empty() {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        (min_x, min_y, max_x, max_y)
    }
}

/// Place a set of already-sized children under `cons`, then translate each
/// child's own subtree into this frame. `inset` pads the content; `hull`, if
/// given, appends this box's own group rectangle.
#[allow(clippy::too_many_arguments)]
fn assemble(
    children: &[BoxId],
    child_laid: &BTreeMap<BoxId, Laid>,
    child_margins: &BTreeMap<BoxId, Margin>,
    cons: &[Constraint],
    inset: f64,
    hull: Option<(Shape, Option<String>, u8)>,
    cfg: &SolveConfig,
    diags: &mut Vec<Diagnostic>,
) -> Laid {
    let mut dims: BTreeMap<BoxId, (Size, Margin)> = BTreeMap::new();
    for c in children {
        dims.insert(c.clone(), (child_laid[c].size, child_margins[c]));
    }
    let placed = solve_cluster(children, &dims, cons, cfg, diags);
    let (min_x, min_y, max_x, max_y) = bounds(&placed, children);
    let dx = inset - min_x;
    let dy = inset - min_y;

    let mut rects = BTreeMap::new();
    let mut groups = Vec::new();
    for c in children {
        let pr = placed[c];
        let ox = pr.x + dx;
        let oy = pr.y + dy;
        let cl = &child_laid[c];
        for (k, r) in &cl.rects {
            rects.insert(k.clone(), Rect { x: r.x + ox, y: r.y + oy, w: r.w, h: r.h });
        }
        for g in &cl.groups {
            groups.push(SolvedGroup {
                rect: Rect { x: g.rect.x + ox, y: g.rect.y + oy, w: g.rect.w, h: g.rect.h },
                shape: g.shape,
                title: g.title.clone(),
                depth: g.depth,
            });
        }
    }

    let outer = Size { w: (max_x - min_x) + 2.0 * inset, h: (max_y - min_y) + 2.0 * inset };
    if let Some((shape, title, depth)) = hull {
        groups.push(SolvedGroup { rect: Rect { x: 0.0, y: 0.0, w: outer.w, h: outer.h }, shape, title, depth });
    }
    Laid { size: outer, rects, groups }
}

fn solve_box(
    id: &BoxId,
    boxes: &BTreeMap<BoxId, &Box>,
    sizes: &SizeMap,
    cfg: &SolveConfig,
    cfor: &BTreeMap<Option<BoxId>, Vec<Constraint>>,
    diags: &mut Vec<Diagnostic>,
) -> Laid {
    let b = boxes[id];
    if b.kind == BoxKind::Leaf {
        let key = match id { BoxId::Node(k) => k.clone(), _ => String::new() };
        let sz = sizes.get(&key).copied().unwrap_or(Size { w: 100.0, h: 40.0 });
        let mut rects = BTreeMap::new();
        rects.insert(id.clone(), Rect { x: 0.0, y: 0.0, w: sz.w, h: sz.h });
        return Laid { size: sz, rects, groups: vec![] };
    }

    let mut child_laid = BTreeMap::new();
    let mut child_margins = BTreeMap::new();
    for c in &b.children {
        child_laid.insert(c.clone(), solve_box(c, boxes, sizes, cfg, cfor, diags));
        child_margins.insert(c.clone(), boxes[c].margin);
    }
    let mut cons = axis_constraints(b);
    if let Some(list) = cfor.get(&Some(id.clone())) {
        cons.extend(list.iter().cloned());
    }
    let inset = cfg.margin(b.margin);
    assemble(
        &b.children,
        &child_laid,
        &child_margins,
        &cons,
        inset,
        Some((b.shape, b.title.clone(), b.depth)),
        cfg,
        diags,
    )
}
```

Now replace the entire existing `pub fn solve(…)` with the recursive version:

```rust
pub fn solve(scene: &Scene, sizes: &SizeMap, cfg: &SolveConfig) -> (Solved, Vec<Diagnostic>) {
    let mut diags = vec![];
    let boxes: BTreeMap<BoxId, &Box> = scene.boxes.iter().map(|b| (b.id.clone(), b)).collect();

    // parent[child] = its group; roots have no parent.
    let mut parent: BTreeMap<BoxId, BoxId> = BTreeMap::new();
    for b in &scene.boxes {
        for c in &b.children {
            parent.insert(c.clone(), b.id.clone());
        }
    }
    let roots: Vec<BoxId> =
        scene.boxes.iter().filter(|b| !parent.contains_key(&b.id)).map(|b| b.id.clone()).collect();

    // Assign each constraint to the cluster whose direct children are both
    // endpoints. Non-siblings warn and drop.
    let mut cfor: BTreeMap<Option<BoxId>, Vec<Constraint>> = BTreeMap::new();
    for c in &scene.constraints {
        let (a, b) = endpoints(c);
        let pa = parent.get(a).cloned();
        let pb = parent.get(b).cloned();
        if pa == pb {
            cfor.entry(pa).or_default().push(c.clone());
        } else {
            diags.push(Diagnostic::warn(
                DiagCode::LayoutConflict,
                "layout relates operands that are not siblings; dropped",
                "",
                0,
            ));
        }
    }

    // Solve every root subtree, then assemble the roots as a top-level clump
    // (no hull, no inset).
    let mut child_laid = BTreeMap::new();
    let mut child_margins = BTreeMap::new();
    for r in &roots {
        child_laid.insert(r.clone(), solve_box(r, &boxes, sizes, cfg, &cfor, &mut diags));
        child_margins.insert(r.clone(), boxes[r].margin);
    }
    let root_cons = cfor.get(&None).cloned().unwrap_or_default();
    let laid = assemble(&roots, &child_laid, &child_margins, &root_cons, 0.0, None, cfg, &mut diags);

    let mut nodes = BTreeMap::new();
    for (id, r) in laid.rects {
        if let BoxId::Node(key) = id {
            nodes.insert(key, r);
        }
    }
    let mut groups = laid.groups;
    groups.sort_by(|a, b| {
        a.depth
            .cmp(&b.depth)
            .then(a.rect.x.total_cmp(&b.rect.x))
            .then(a.rect.y.total_cmp(&b.rect.y))
    });

    (Solved { nodes, groups, flags: BTreeMap::new() }, diags)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml solve::geometry`
Expected: PASS (new group tests + the Task 5 row/contradiction tests, which have no group boxes so roots are the bare leaves).

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/solve/geometry.rs
git commit -m "feat(uaml): recursive group solve with shapes, margins, nesting"
```

---

### Task 7: Flags — `collapsed` chip size and `emphasized` passthrough

A `collapsed` node uses `cfg.chip` as its size (rendered as a reference chip) instead of its `SizeMap` entry. Both `emphasized` and `collapsed` are reported per node in `Solved.flags` (only when set, for a compact deterministic map).

**Files:**
- Modify: `crates/uaml/src/solve/geometry.rs` (leaf sizing + flags collection in `solve`)

**Interfaces:**
- Consumes: `SolveConfig.chip` (Task 1), `Box.flags` (Task 1).
- Produces: populated `Solved.flags`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/uaml/src/solve/geometry.rs`:

```rust
    #[test]
    fn collapsed_uses_chip_size_and_flags_reported() {
        let mut a = leaf("a");
        a.flags.collapsed = true;
        let mut b = leaf("b");
        b.flags.emphasized = true;
        let scene = Scene {
            boxes: vec![a, b],
            constraints: vec![Constraint::Place {
                a: BoxId::Node("a".into()),
                b: BoxId::Node("b".into()),
                dir: Direction::LeftOf,
            }],
        };
        let (solved, diags) = solve(&scene, &sizes(&["a", "b"], 200.0, 90.0), &SolveConfig::default());
        assert!(diags.is_empty());
        // a collapses to the 96x28 chip; `a left of b` gaps 96+16=112 in x,
        // centers align in y: (28-90)/2 = -31, normalized so the band top is 0.
        assert_eq!(
            pretty(&solved),
            "node a @ 0,31 96x28\n\
             node b @ 112,0 200x90\n\
             flags a emphasized=false collapsed=true\n\
             flags b emphasized=true collapsed=false\n"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml solve::geometry::tests::collapsed_uses_chip`
Expected: FAIL — `a` is sized 200x90 (chip not applied) and `Solved.flags` is empty.

- [ ] **Step 3: Apply chip sizing and collect flags**

In `solve_box`'s leaf branch, replace the `let sz = …` line with:

```rust
        let sz = if b.flags.collapsed {
            cfg.chip
        } else {
            sizes.get(&key).copied().unwrap_or(Size { w: 100.0, h: 40.0 })
        };
```

In `solve`, just before building the returned `Solved`, add the flags collection and use it. Replace:

```rust
    (Solved { nodes, groups, flags: BTreeMap::new() }, diags)
```

with:

```rust
    let mut flags = BTreeMap::new();
    for b in &scene.boxes {
        if let BoxId::Node(key) = &b.id {
            if b.flags.emphasized || b.flags.collapsed {
                flags.insert(key.clone(), b.flags);
            }
        }
    }

    (Solved { nodes, groups, flags }, diags)
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml solve::geometry`
Expected: PASS (flags test + all earlier geometry tests).

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/solve/geometry.rs
git commit -m "feat(uaml): collapsed chip sizing and flag passthrough"
```

---

### Task 8: Top-level `solve_diagram` + end-to-end golden test

Wire `resolve` → `solve` in the real `solve_diagram` (replacing the Task 1 stub) and lock the whole pipeline with a golden test built from a `model::Diagram`.

**Files:**
- Modify: `crates/uaml/src/solve/mod.rs` (implement `solve_diagram`)
- Create: `crates/uaml/tests/solver_golden.rs`

**Interfaces:**
- Consumes: `resolve::resolve` (Tasks 2–3), `geometry::solve` (Tasks 5–7).
- Produces: working `pub fn solve_diagram(&Diagram, &SizeMap, &SolveConfig) -> (Solved, Vec<Diagnostic>)`.

- [ ] **Step 1: Write the failing test**

Create `crates/uaml/tests/solver_golden.rs`:

```rust
use std::collections::BTreeMap;
use uaml::model::{Diagram, DiagramGroup};
use uaml::solve::{pretty, solve_diagram, Size, SizeMap, SolveConfig};
use uaml::syntax::*;

fn bare(name: &str) -> Operand {
    Operand { ref_: OperandRef::Name(NameRef::Bare(name.into())), axis: None, hints: vec![] }
}

#[test]
fn orders_domain_diagram_solves_to_expected_layout() {
    // Users (as column, framed) left of Orders. All nodes 200x90.
    let users_treated = Operand {
        ref_: OperandRef::Name(NameRef::Bare("Users".into())),
        axis: Some(Axis::Column),
        hints: vec![Hint::Shape(Shape::Frame)],
    };
    let diagram = Diagram {
        key: "orders".into(),
        title: "Orders".into(),
        profile: "uml-domain".into(),
        groups: vec![
            DiagramGroup { name: "Users".into(), members: vec!["customer".into(), "account".into()], children: vec![] },
            DiagramGroup { name: "Orders".into(), members: vec!["order".into()], children: vec![] },
        ],
        layout: vec![
            LayoutStatement::Standalone(users_treated),
            LayoutStatement::Placement { operands: vec![bare("Users"), bare("Orders")], directions: vec![Direction::LeftOf] },
        ],
    };

    let mut sizes: SizeMap = BTreeMap::new();
    for k in ["customer", "account", "order"] {
        sizes.insert(k.into(), Size { w: 200.0, h: 90.0 });
    }

    let (solved, diags) = solve_diagram(&diagram, &sizes, &SolveConfig::default());
    assert!(diags.is_empty(), "no diagnostics: {diags:?}");
    assert_eq!(
        pretty(&solved),
        "node account @ 16,122 200x90\n\
         node customer @ 16,16 200x90\n\
         node order @ 264,69 200x90\n\
         group Frame \"Users\" d0 @ 0,0 232x228\n\
         group Shrink \"Orders\" d0 @ 248,53 232x122\n"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml --test solver_golden`
Expected: FAIL — `solve_diagram` is the Task 1 stub returning an empty `Solved`.

- [ ] **Step 3: Implement `solve_diagram`**

In `crates/uaml/src/solve/mod.rs`, replace the stub `solve_diagram` with:

```rust
/// Top-level entry: resolve the diagram to a `Scene`, then solve it.
pub fn solve_diagram(
    diagram: &crate::model::Diagram,
    sizes: &SizeMap,
    cfg: &SolveConfig,
) -> (Solved, Vec<Diagnostic>) {
    let (scene, mut diags) = resolve::resolve(diagram);
    let (solved, mut geo_diags) = geometry::solve(&scene, sizes, cfg);
    diags.append(&mut geo_diags);
    (solved, diags)
}
```

- [ ] **Step 4: Run the whole crate's tests**

Run: `cargo test -p uaml`
Expected: PASS (the golden test + every unit test across the module + the pre-existing crate tests).

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/solve/mod.rs crates/uaml/tests/solver_golden.rs
git commit -m "feat(uaml): wire solve_diagram end-to-end with golden test"
```

---

## Notes and deferred work

- **Clump / independent-cluster packing** is intentionally simple: X-components pack left-to-right in list order; Y-components share a top band. This is deterministic and always renders, but two fully-unrelated clusters sit side by side rather than in a compact grid. Refinement is a follow-on (the design doc's "clump packing exact shape" open question).
- **Non-sibling constraints are dropped** (warned). A relation between a nested node and a top-level box is not supported in Phase 1; authors relate siblings (top-level groups, or members within one group). Lifting an operand to a common-ancestor proxy is future work.
- **Cycle vs contradiction** both surface as `LayoutConflict` (an inconsistent union). The distinct `LayoutCycle` code exists but Phase 1 does not separate the two cheaply; revisit if authors need the finer signal.
- **`shrink` reserves a bounding rect** like `box`; concave-hull keep-out is deferred (design non-goal for Phase 1).
- **Phases 2 (WASM) and 3 (web)** are separate spec → plan cycles per the design doc.
