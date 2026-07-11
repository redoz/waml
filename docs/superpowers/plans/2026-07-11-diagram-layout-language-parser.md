# Diagram Layout Language — Parser Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Parse, serialize, and validate the UAML `## Layout` diagram-arrangement language and grouped `## Members`, round-tripping the Markdown text ↔ AST in the Rust `crates/uaml` crate.

**Architecture:** A new `crates/uaml/src/layout.rs` module holds a small tokenizer plus a hand-written recursive-descent parser (and matching renderer) for one `## Layout` bullet. The syntax AST lives in `syntax.rs` next to the existing `ParsedRel`/`MemberLine` types. `## Members` gains a nested-group tree (headings are membership only). `Section` grows a `Layout` variant and a grouped `Members` variant; the old per-node `at x,y` coordinate and the `## Render hints` section are removed (superseded by `## Layout`). The resolved `model::Diagram` carries `groups` + `layout`. Validation adds malformed-layout linting, unknown-operand-reference warnings, and directed-cycle detection over placement constraints.

**Tech Stack:** Rust (edition as in `crates/uaml/Cargo.toml`), `regex` (already a dependency), `pulldown-cmark` (already a dependency). No new crates.

## Global Constraints

- Scope is **parse + serialize + validate only**. NO solver, NO pixel/coordinate computation, NO rendering, NO web/TS editor. `crates/uaml` (Rust) only — do not touch `packages/`.
- **No coordinate is ever stored.** The `MemberLine.position` field and the `at x,y` syntax are removed, not preserved.
- Positions are expressed **relationally**. The canonical grammar is the BNF in `docs/uaml-spec.md` under **Diagram documents → The `## Layout` section → BNF** (lines ~627–659). Every production there must map to a task here.
- **Round-trip = semantic fixpoint**, matching the existing test style: `serialize(parse(x)) == serialize(parse(serialize(parse(x))))`. Byte-identical echo of the author's input is NOT required (whitespace, hint joiners `and`↔`,`, and author parens may normalize), but the result must be a fixpoint.
- Test command (whole crate): `cargo test -p uaml`. Single test: `cargo test -p uaml <substring>`.
- Follow existing crate idioms: `LazyLock<Regex>` for regexes, `filter_map` line parsers, `#[derive(Debug, Clone, PartialEq)]` on AST types, `Diagnostic::new` (error) / `Diagnostic::warn` (warning) with a `DiagCode`.
- Do NOT `git push`. Commit locally per task. Do NOT touch uncommitted `packages/web-svelte` migration files.

---

### Task 1: Layout AST types + `Section::Layout` variant (additive)

Add every AST type the layout language needs, plus a new `Section::Layout` variant wired through the exhaustive matches so the crate keeps compiling. `## Members` grouping and `## Render hints` removal come later — this task is purely additive and leaves existing behavior intact (classify emits an empty `Layout` for now).

**Files:**
- Modify: `crates/uaml/src/syntax.rs` (add AST types + `Section::Layout`)
- Modify: `crates/uaml/src/parse.rs:22-46` (classify: route `"layout"`)
- Modify: `crates/uaml/src/serialize.rs:7-49` (add `Layout` arm to `section_order` + `render_section`)

**Interfaces:**
- Produces (consumed by Tasks 2–13):
  ```rust
  pub enum LayoutStatement {
      Placement { operands: Vec<Operand>, directions: Vec<Direction> }, // operands.len() == directions.len() + 1
      Alignment { left: Anchored, right: Anchored },
      Standalone(Operand),
  }
  pub enum Direction { LeftOf, RightOf, Above, Below }        // Copy
  pub struct Anchored { pub edge: Option<Edge>, pub operand: Operand }
  pub enum Edge { Top, Bottom, Left, Right, Center }          // Copy
  pub struct Operand { pub ref_: OperandRef, pub axis: Option<Axis>, pub hints: Vec<Hint> }
  pub enum Axis { Row, Column }                               // Copy
  pub enum OperandRef { Name(NameRef), InlineGroup { axis: Axis, items: Vec<Operand> }, Paren(Box<Operand>) }
  pub enum NameRef { Link { title: String, slug: String }, Bare(String) }
  pub enum Hint { Shape(Shape), Margin(Margin), Flag(Flag) }  // Copy
  pub enum Shape { Frame, Box, Shrink }                       // Copy
  pub enum Margin { No, Small, Medium, Large }                // Copy
  pub enum Flag { Emphasized, Collapsed }                     // Copy
  ```
- Produces: `Section::Layout(Vec<LayoutStatement>)`.

- [x] **Step 1: Write the failing test**

Append to the `tests` module in `crates/uaml/src/syntax.rs`:

```rust
    #[test]
    fn layout_statement_is_constructible() {
        let stmt = LayoutStatement::Placement {
            operands: vec![
                Operand { ref_: OperandRef::Name(NameRef::Bare("Users".into())), axis: None, hints: vec![] },
                Operand { ref_: OperandRef::Name(NameRef::Bare("Orders".into())), axis: None, hints: vec![] },
            ],
            directions: vec![Direction::LeftOf],
        };
        match stmt {
            LayoutStatement::Placement { operands, directions } => {
                assert_eq!(operands.len(), 2);
                assert_eq!(directions, vec![Direction::LeftOf]);
            }
            _ => panic!("wrong variant"),
        }
    }
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml layout_statement_is_constructible`
Expected: FAIL — compile error, `LayoutStatement`/`Operand`/etc. not found.

- [x] **Step 3: Add the AST types and the `Section` variant**

In `crates/uaml/src/syntax.rs`, add these type definitions (place them after the existing `HintLine` enum, before the `tests` module):

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutStatement {
    /// `A left of B above C` — N operands, N-1 directions.
    Placement { operands: Vec<Operand>, directions: Vec<Direction> },
    /// `top of X aligned with top of Y`
    Alignment { left: Anchored, right: Anchored },
    /// A lone operand — meaningful when it carries `as`/`with` treatment.
    Standalone(Operand),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction { LeftOf, RightOf, Above, Below }

#[derive(Debug, Clone, PartialEq)]
pub struct Anchored { pub edge: Option<Edge>, pub operand: Operand }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Edge { Top, Bottom, Left, Right, Center }

#[derive(Debug, Clone, PartialEq)]
pub struct Operand {
    pub ref_: OperandRef,
    pub axis: Option<Axis>,
    pub hints: Vec<Hint>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Axis { Row, Column }

#[derive(Debug, Clone, PartialEq)]
pub enum OperandRef {
    Name(NameRef),
    InlineGroup { axis: Axis, items: Vec<Operand> },
    Paren(Box<Operand>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum NameRef {
    Link { title: String, slug: String },
    Bare(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hint { Shape(Shape), Margin(Margin), Flag(Flag) }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape { Frame, Box, Shrink }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Margin { No, Small, Medium, Large }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Flag { Emphasized, Collapsed }
```

In the same file, add the variant to the `Section` enum (after the `Members` line):

```rust
    Members(Vec<MemberLine>),
    Layout(Vec<LayoutStatement>),
    RenderHints(Vec<HintLine>),
```

In `crates/uaml/src/parse.rs`, in `classify` (the `match title.to_lowercase().as_str()` block), add an arm before the `_ =>` fallback. For now it produces an empty vec (real parsing arrives in Task 8):

```rust
        "layout" => Section::Layout(Vec::new()),
```

In `crates/uaml/src/serialize.rs`, add a `Layout` arm to `section_order` (between `Members` and `RenderHints`, renumbering the tail):

```rust
        Section::Members(_) => 5,
        Section::Layout(_) => 6,
        Section::RenderHints(_) => 7,
        Section::Unknown { .. } => 8,
```

And add a `Layout` arm to `render_section` (an empty vec renders just the heading; real rendering arrives in Task 8):

```rust
        Section::Layout(stmts) => {
            let body = stmts
                .iter()
                .map(crate::layout::render_layout_line)
                .collect::<Vec<_>>()
                .join("\n");
            if body.is_empty() { "## Layout".to_string() } else { format!("## Layout\n{body}") }
        }
```

`render_layout_line` does not exist yet, so also register the module now: in `crates/uaml/src/lib.rs` add `pub mod layout;` after `pub mod grammar;`, and create a stub `crates/uaml/src/layout.rs`:

```rust
//! Tokenizer, recursive-descent parser, and renderer for the `## Layout`
//! diagram-arrangement language. See docs/uaml-spec.md (Diagram documents).

use crate::syntax::LayoutStatement;

/// Render one `## Layout` statement back to its `- …` bullet text.
pub fn render_layout_line(_stmt: &LayoutStatement) -> String {
    String::new() // implemented in Task 8
}
```

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml`
Expected: PASS (whole crate still compiles and green; `layout_statement_is_constructible` passes).

- [x] **Step 5: Commit**

```bash
git add crates/uaml/src/syntax.rs crates/uaml/src/parse.rs crates/uaml/src/serialize.rs crates/uaml/src/lib.rs crates/uaml/src/layout.rs
git commit -m "feat(uaml): add Layout AST types and Section::Layout variant"
```

---

### Task 2: Layout tokenizer

Tokenize one layout bullet body into atoms: parenthesis, comma, `[title](./slug.md)` link, `"quoted"` string, and bare words. Multi-word keywords (`left of`, `aligned with`, `column of`, `as`, `with`) are recognized later by the parser peeking word sequences — the lexer only splits.

**Files:**
- Modify: `crates/uaml/src/layout.rs`

**Interfaces:**
- Produces (used by Tasks 3–7, module-private): `enum Tok { Word(String), Link { title, slug }, Quoted(String), LParen, RParen, Comma }` and `fn lex_layout(body: &str) -> Option<Vec<Tok>>`.

- [x] **Step 1: Write the failing test**

Add a `tests` module at the bottom of `crates/uaml/src/layout.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_link_word_paren_comma_and_quote() {
        let toks = lex_layout("column of [Order](./order.md), (Users) as \"my group\"").unwrap();
        assert_eq!(toks, vec![
            Tok::Word("column".into()),
            Tok::Word("of".into()),
            Tok::Link { title: "Order".into(), slug: "order".into() },
            Tok::Comma,
            Tok::LParen,
            Tok::Word("Users".into()),
            Tok::RParen,
            Tok::Word("as".into()),
            Tok::Quoted("my group".into()),
        ]);
    }

    #[test]
    fn rejects_unterminated_quote_and_bad_link() {
        assert!(lex_layout("\"oops").is_none());
        assert!(lex_layout("[Order](./order.md").is_none());
    }
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml layout::tests::lexes_link_word_paren_comma_and_quote`
Expected: FAIL — `Tok` / `lex_layout` not found.

- [x] **Step 3: Implement the tokenizer**

In `crates/uaml/src/layout.rs`, add above the `tests` module:

```rust
use std::sync::LazyLock;
use regex::Regex;

// Anchored at the start: `[title](./slug.md)`. Slug may contain a directory prefix.
static LAYOUT_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[([^\]]*)\]\(\./(.+?)\.md\)").unwrap());

/// Strip a directory prefix and the `.md` suffix from a link path.
fn link_slug(path: &str) -> String {
    let seg = path.rsplit(['/', '\\']).next().unwrap_or(path);
    seg.strip_suffix(".md").unwrap_or(seg).to_string()
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Word(String),
    Link { title: String, slug: String },
    Quoted(String),
    LParen,
    RParen,
    Comma,
}

/// Split one layout bullet body into tokens. Returns `None` on a malformed
/// link or an unterminated quote.
fn lex_layout(body: &str) -> Option<Vec<Tok>> {
    let chars: Vec<char> = body.trim().chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '(' => { out.push(Tok::LParen); i += 1; }
            ')' => { out.push(Tok::RParen); i += 1; }
            ',' => { out.push(Tok::Comma); i += 1; }
            '[' => {
                let rest: String = chars[i..].iter().collect();
                let cap = LAYOUT_LINK_RE.captures(&rest)?;
                let whole = cap.get(0).unwrap().as_str();
                out.push(Tok::Link {
                    title: cap[1].to_string(),
                    slug: link_slug(&cap[2]),
                });
                i += whole.chars().count();
            }
            '"' => {
                let mut j = i + 1;
                let mut val = String::new();
                while j < chars.len() && chars[j] != '"' {
                    val.push(chars[j]);
                    j += 1;
                }
                if j >= chars.len() {
                    return None; // unterminated quote
                }
                out.push(Tok::Quoted(val));
                i = j + 1;
            }
            _ => {
                let start = i;
                while i < chars.len()
                    && !chars[i].is_whitespace()
                    && !matches!(chars[i], '(' | ')' | ',' | '[' | '"')
                {
                    i += 1;
                }
                let w: String = chars[start..i].iter().collect();
                out.push(Tok::Word(w));
            }
        }
    }
    Some(out)
}
```

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml layout::tests::`
Expected: PASS (both new tests).

- [x] **Step 5: Commit**

```bash
git add crates/uaml/src/layout.rs
git commit -m "feat(uaml): tokenizer for the Layout language"
```

---

### Task 3: Parse a standalone operand (bare name + link)

Introduce the parser cursor and `parse_layout_line`, handling the simplest statement: a lone operand (bare word, quoted name, or link). This establishes `parse_ref` / `parse_operand` / the `Cur` helper that later tasks extend.

**Files:**
- Modify: `crates/uaml/src/layout.rs`

**Interfaces:**
- Produces (used by Tasks 4–13): `pub fn parse_layout_line(line: &str) -> Option<LayoutStatement>`. The `line` includes the leading `- ` bullet.
- Produces (module-private, extended by later tasks): `struct Cur`, `fn parse_operand(&mut Cur) -> Option<Operand>`, `fn parse_ref(&mut Cur) -> Option<OperandRef>`.

- [x] **Step 1: Write the failing test**

Add to the `tests` module in `crates/uaml/src/layout.rs`:

```rust
    #[test]
    fn parses_standalone_bare_and_link() {
        use crate::syntax::*;
        assert_eq!(
            parse_layout_line("- Orders"),
            Some(LayoutStatement::Standalone(Operand {
                ref_: OperandRef::Name(NameRef::Bare("Orders".into())),
                axis: None,
                hints: vec![],
            }))
        );
        assert_eq!(
            parse_layout_line("- [Order](./order.md)"),
            Some(LayoutStatement::Standalone(Operand {
                ref_: OperandRef::Name(NameRef::Link { title: "Order".into(), slug: "order".into() }),
                axis: None,
                hints: vec![],
            }))
        );
    }

    #[test]
    fn rejects_line_without_bullet_and_trailing_garbage() {
        assert!(parse_layout_line("Orders").is_none());       // no "- " bullet
        assert!(parse_layout_line("- Orders Extra").is_none()); // two bare words, no relation
    }
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml layout::tests::parses_standalone_bare_and_link`
Expected: FAIL — `parse_layout_line` not found.

- [x] **Step 3: Implement the cursor, `parse_ref`, `parse_operand`, and `parse_layout_line`**

In `crates/uaml/src/layout.rs`, add above the `tests` module. Import the AST at the top of the file (replace the existing `use crate::syntax::LayoutStatement;` line with the glob):

```rust
use crate::syntax::*;
```

Then add:

```rust
struct Cur<'a> { toks: &'a [Tok], pos: usize }

impl<'a> Cur<'a> {
    fn peek(&self) -> Option<&Tok> { self.toks.get(self.pos) }
    fn peek_word(&self) -> Option<&str> {
        match self.toks.get(self.pos) {
            Some(Tok::Word(w)) => Some(w.as_str()),
            _ => None,
        }
    }
    fn bump(&mut self) -> Option<&Tok> {
        let t = self.toks.get(self.pos);
        if t.is_some() { self.pos += 1; }
        t
    }
    /// Consume the next token iff it is `Word(w)` (case-insensitive).
    fn eat_word(&mut self, w: &str) -> bool {
        if self.peek_word().map(|x| x.eq_ignore_ascii_case(w)) == Some(true) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn done(&self) -> bool { self.pos >= self.toks.len() }
}

/// Parse one `## Layout` bullet (leading `- ` required). Returns `None` if the
/// bullet is malformed or has unconsumed trailing tokens.
pub fn parse_layout_line(line: &str) -> Option<LayoutStatement> {
    let body = line.trim().strip_prefix("- ")?;
    let toks = lex_layout(body)?;
    if toks.is_empty() {
        return None;
    }
    // Standalone only, for now. Placement/alignment dispatch arrives in Tasks 6–7.
    let mut cur = Cur { toks: &toks, pos: 0 };
    let op = parse_operand(&mut cur)?;
    if !cur.done() {
        return None;
    }
    Some(LayoutStatement::Standalone(op))
}

fn parse_operand(cur: &mut Cur) -> Option<Operand> {
    let ref_ = parse_ref(cur)?;
    // `as` / `with` clauses are added in Task 4.
    Some(Operand { ref_, axis: None, hints: vec![] })
}

fn parse_ref(cur: &mut Cur) -> Option<OperandRef> {
    match cur.peek()? {
        Tok::Link { .. } => {
            if let Some(Tok::Link { title, slug }) = cur.bump() {
                Some(OperandRef::Name(NameRef::Link { title: title.clone(), slug: slug.clone() }))
            } else {
                None
            }
        }
        Tok::Quoted(_) => {
            if let Some(Tok::Quoted(v)) = cur.bump() {
                Some(OperandRef::Name(NameRef::Bare(v.clone())))
            } else {
                None
            }
        }
        Tok::Word(w) => {
            // Inline groups + parens are added in Task 5. For now a lone bare word.
            let name = w.clone();
            cur.bump();
            Some(OperandRef::Name(NameRef::Bare(name)))
        }
        _ => None,
    }
}
```

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml layout::tests::`
Expected: PASS (standalone + reject tests, plus Task 2 lexer tests).

- [x] **Step 5: Commit**

```bash
git add crates/uaml/src/layout.rs
git commit -m "feat(uaml): parse standalone Layout operands"
```

---

### Task 4: Parse `as <axis>` and `with <hints>` treatment clauses

Extend `parse_operand` to consume an optional `as row|column` clause then an optional `with <hints>` clause. Hints are `shape` (`frame`/`box`/`shrink`), `margin` (`no`/`small`/`medium`/`large` + `margin`|`margins`), and `flag` (`emphasized`/`collapsed`), joined by `,` or `and`.

**Files:**
- Modify: `crates/uaml/src/layout.rs`

**Interfaces:**
- Produces (module-private): `fn parse_axis(&mut Cur) -> Option<Axis>`, `fn parse_hints(&mut Cur) -> Option<Vec<Hint>>`, `fn parse_hint(&mut Cur) -> Option<Hint>`.

- [x] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
    #[test]
    fn parses_as_axis_and_with_hints() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- Users as column with frame and large margin").unwrap();
        let LayoutStatement::Standalone(op) = stmt else { panic!("expected standalone") };
        assert_eq!(op.axis, Some(Axis::Column));
        assert_eq!(op.hints, vec![
            Hint::Shape(Shape::Frame),
            Hint::Margin(Margin::Large),
        ]);
    }

    #[test]
    fn parses_all_hint_kinds_and_margins_word() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- Order with box, no margins, emphasized, collapsed").unwrap();
        let LayoutStatement::Standalone(op) = stmt else { panic!("expected standalone") };
        assert_eq!(op.hints, vec![
            Hint::Shape(Shape::Box),
            Hint::Margin(Margin::No),
            Hint::Flag(Flag::Emphasized),
            Hint::Flag(Flag::Collapsed),
        ]);
    }

    #[test]
    fn rejects_margin_level_without_margin_keyword() {
        assert!(parse_layout_line("- Order with large").is_none());
    }
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml layout::tests::parses_as_axis_and_with_hints`
Expected: FAIL — assertion fails (`op.axis` is `None`, `op.hints` is empty).

- [x] **Step 3: Extend `parse_operand` and add hint parsing**

In `crates/uaml/src/layout.rs`, replace the body of `parse_operand` with:

```rust
fn parse_operand(cur: &mut Cur) -> Option<Operand> {
    let ref_ = parse_ref(cur)?;

    let mut axis = None;
    if cur.peek_word().map(|w| w.eq_ignore_ascii_case("as")) == Some(true) {
        cur.bump();
        axis = Some(parse_axis(cur)?);
    }

    let mut hints = vec![];
    if cur.peek_word().map(|w| w.eq_ignore_ascii_case("with")) == Some(true) {
        cur.bump();
        hints = parse_hints(cur)?;
    }

    Some(Operand { ref_, axis, hints })
}

fn parse_axis(cur: &mut Cur) -> Option<Axis> {
    match cur.peek_word()?.to_ascii_lowercase().as_str() {
        "row" => { cur.bump(); Some(Axis::Row) }
        "column" => { cur.bump(); Some(Axis::Column) }
        _ => None,
    }
}

fn parse_hints(cur: &mut Cur) -> Option<Vec<Hint>> {
    let mut hints = vec![parse_hint(cur)?];
    loop {
        if cur.peek() == Some(&Tok::Comma) {
            cur.bump();
        } else if cur.peek_word().map(|w| w.eq_ignore_ascii_case("and")) == Some(true) {
            cur.bump();
        } else {
            break;
        }
        hints.push(parse_hint(cur)?);
    }
    Some(hints)
}

fn parse_hint(cur: &mut Cur) -> Option<Hint> {
    let w = cur.peek_word()?.to_ascii_lowercase();
    match w.as_str() {
        "frame" => { cur.bump(); Some(Hint::Shape(Shape::Frame)) }
        "box" => { cur.bump(); Some(Hint::Shape(Shape::Box)) }
        "shrink" => { cur.bump(); Some(Hint::Shape(Shape::Shrink)) }
        "emphasized" => { cur.bump(); Some(Hint::Flag(Flag::Emphasized)) }
        "collapsed" => { cur.bump(); Some(Hint::Flag(Flag::Collapsed)) }
        "no" | "small" | "medium" | "large" => {
            let m = match w.as_str() {
                "no" => Margin::No,
                "small" => Margin::Small,
                "medium" => Margin::Medium,
                _ => Margin::Large,
            };
            cur.bump();
            if cur.eat_word("margin") || cur.eat_word("margins") {
                Some(Hint::Margin(m))
            } else {
                None // a margin level must be followed by `margin`/`margins`
            }
        }
        _ => None,
    }
}
```

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml layout::tests::`
Expected: PASS (all three new tests + earlier tests).

- [x] **Step 5: Commit**

```bash
git add crates/uaml/src/layout.rs
git commit -m "feat(uaml): parse Layout as/with treatment clauses"
```

---

### Task 5: Parse inline groups (`column of` / `row of`), operand lists, and parens

Extend `parse_ref` to handle inline `column of …` / `row of …` groups (anonymous, comma-separated operand lists) and parenthesized operands. `with` binds greedily to the nearest complete operand on its left; parens override.

**Files:**
- Modify: `crates/uaml/src/layout.rs`

**Interfaces:**
- Produces (module-private): `fn parse_operand_list(&mut Cur) -> Option<Vec<Operand>>`.

- [x] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
    #[test]
    fn parses_inline_group_and_greedy_with_binding() {
        use crate::syntax::*;
        // `with` binds to Account (nearest operand), NOT the whole column.
        let stmt = parse_layout_line("- column of Customer, Account with large margin").unwrap();
        let LayoutStatement::Standalone(op) = stmt else { panic!() };
        let OperandRef::InlineGroup { axis, items } = op.ref_ else { panic!("expected inline group") };
        assert_eq!(axis, Axis::Column);
        assert_eq!(items.len(), 2);
        assert!(op.hints.is_empty(), "outer operand carries no hints");
        assert_eq!(items[1].hints, vec![Hint::Margin(Margin::Large)]);
    }

    #[test]
    fn parens_rebind_with_to_the_whole_group() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- (column of Customer, Account) with large margin").unwrap();
        let LayoutStatement::Standalone(op) = stmt else { panic!() };
        assert_eq!(op.hints, vec![Hint::Margin(Margin::Large)]);
        let OperandRef::Paren(inner) = op.ref_ else { panic!("expected paren") };
        assert!(inner.hints.is_empty());
        assert!(matches!(inner.ref_, OperandRef::InlineGroup { .. }));
    }

    #[test]
    fn parses_nested_inline_groups() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- row of (column of Customer, Account), Orders").unwrap();
        let LayoutStatement::Standalone(op) = stmt else { panic!() };
        let OperandRef::InlineGroup { axis, items } = op.ref_ else { panic!() };
        assert_eq!(axis, Axis::Row);
        assert_eq!(items.len(), 2);
        assert!(matches!(items[0].ref_, OperandRef::Paren(_)));
    }
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml layout::tests::parses_inline_group_and_greedy_with_binding`
Expected: FAIL — current `parse_ref` treats `column` as a bare word, leaving `of Customer …` unconsumed, so `parse_layout_line` returns `None`.

- [x] **Step 3: Extend `parse_ref` and add `parse_operand_list`**

In `crates/uaml/src/layout.rs`, replace the whole `parse_ref` function with:

```rust
fn parse_ref(cur: &mut Cur) -> Option<OperandRef> {
    match cur.peek()? {
        Tok::LParen => {
            cur.bump();
            let inner = parse_operand(cur)?;
            match cur.bump() {
                Some(Tok::RParen) => {}
                _ => return None,
            }
            Some(OperandRef::Paren(Box::new(inner)))
        }
        Tok::Link { .. } => {
            if let Some(Tok::Link { title, slug }) = cur.bump() {
                Some(OperandRef::Name(NameRef::Link { title: title.clone(), slug: slug.clone() }))
            } else {
                None
            }
        }
        Tok::Quoted(_) => {
            if let Some(Tok::Quoted(v)) = cur.bump() {
                Some(OperandRef::Name(NameRef::Bare(v.clone())))
            } else {
                None
            }
        }
        Tok::Word(w) => {
            let lw = w.to_ascii_lowercase();
            if lw == "column" || lw == "row" {
                let axis = if lw == "column" { Axis::Column } else { Axis::Row };
                cur.bump();
                if !cur.eat_word("of") {
                    return None;
                }
                let items = parse_operand_list(cur)?;
                Some(OperandRef::InlineGroup { axis, items })
            } else {
                let name = w.clone();
                cur.bump();
                Some(OperandRef::Name(NameRef::Bare(name)))
            }
        }
        _ => None,
    }
}

fn parse_operand_list(cur: &mut Cur) -> Option<Vec<Operand>> {
    let mut items = vec![parse_operand(cur)?];
    while cur.peek() == Some(&Tok::Comma) {
        cur.bump();
        items.push(parse_operand(cur)?);
    }
    Some(items)
}
```

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml layout::tests::`
Expected: PASS (three new tests + earlier tests).

- [x] **Step 5: Commit**

```bash
git add crates/uaml/src/layout.rs
git commit -m "feat(uaml): parse inline Layout groups, operand lists, and parens"
```

---

### Task 6: Parse placement statements (directions, chained)

Dispatch `parse_layout_line` to a placement branch: an operand followed by one or more `<direction> <operand>` pairs. Directions are `left of`, `right of` (two words), `above`, `below` (one word). Chainable: `A above B above C`.

**Files:**
- Modify: `crates/uaml/src/layout.rs`

**Interfaces:**
- Produces (module-private): `fn eat_direction(&mut Cur) -> Option<Direction>`, `fn try_parse_placement(&[Tok]) -> Option<LayoutStatement>`.

- [x] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
    #[test]
    fn parses_single_and_chained_placement() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- Users left of Orders").unwrap();
        let LayoutStatement::Placement { operands, directions } = stmt else { panic!("expected placement") };
        assert_eq!(operands.len(), 2);
        assert_eq!(directions, vec![Direction::LeftOf]);

        let stmt = parse_layout_line("- Order above OrderLine above Payment").unwrap();
        let LayoutStatement::Placement { operands, directions } = stmt else { panic!() };
        assert_eq!(operands.len(), 3);
        assert_eq!(directions, vec![Direction::Above, Direction::Above]);
    }

    #[test]
    fn parses_all_four_directions() {
        use crate::syntax::*;
        for (text, dir) in [
            ("- A left of B", Direction::LeftOf),
            ("- A right of B", Direction::RightOf),
            ("- A above B", Direction::Above),
            ("- A below B", Direction::Below),
        ] {
            let LayoutStatement::Placement { directions, .. } = parse_layout_line(text).unwrap() else { panic!() };
            assert_eq!(directions, vec![dir]);
        }
    }

    #[test]
    fn placement_binds_with_to_operand_then_direction() {
        use crate::syntax::*;
        // greedy `with` on the first operand, then a trailing relation
        let stmt = parse_layout_line("- Users with frame left of Orders").unwrap();
        let LayoutStatement::Placement { operands, directions } = stmt else { panic!() };
        assert_eq!(directions, vec![Direction::LeftOf]);
        assert_eq!(operands[0].hints, vec![Hint::Shape(Shape::Frame)]);
    }
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml layout::tests::parses_single_and_chained_placement`
Expected: FAIL — `parse_layout_line` currently returns `None` (trailing `left of Orders` unconsumed by the standalone branch).

- [x] **Step 3: Add the placement branch**

In `crates/uaml/src/layout.rs`, add these functions above the `tests` module:

```rust
fn eat_direction(cur: &mut Cur) -> Option<Direction> {
    match cur.peek_word()?.to_ascii_lowercase().as_str() {
        "above" => { cur.bump(); Some(Direction::Above) }
        "below" => { cur.bump(); Some(Direction::Below) }
        "left" => {
            let save = cur.pos;
            cur.bump();
            if cur.eat_word("of") { Some(Direction::LeftOf) } else { cur.pos = save; None }
        }
        "right" => {
            let save = cur.pos;
            cur.bump();
            if cur.eat_word("of") { Some(Direction::RightOf) } else { cur.pos = save; None }
        }
        _ => None,
    }
}

fn try_parse_placement(toks: &[Tok]) -> Option<LayoutStatement> {
    let mut cur = Cur { toks, pos: 0 };
    let first = parse_operand(&mut cur)?;
    let mut operands = vec![first];
    let mut directions = vec![eat_direction(&mut cur)?]; // at least one direction required
    operands.push(parse_operand(&mut cur)?);
    while let Some(d) = eat_direction(&mut cur) {
        directions.push(d);
        operands.push(parse_operand(&mut cur)?);
    }
    if !cur.done() {
        return None;
    }
    Some(LayoutStatement::Placement { operands, directions })
}
```

Then update `parse_layout_line` to try placement before falling back to standalone. Replace its body (after the `if toks.is_empty()` guard) with:

```rust
    if let Some(stmt) = try_parse_placement(&toks) {
        return Some(stmt);
    }
    let mut cur = Cur { toks: &toks, pos: 0 };
    let op = parse_operand(&mut cur)?;
    if !cur.done() {
        return None;
    }
    Some(LayoutStatement::Standalone(op))
```

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml layout::tests::`
Expected: PASS (three new tests + earlier tests, including `rejects_line_without_bullet_and_trailing_garbage` — `- Orders Extra` still has no direction, so placement fails and standalone leaves `Extra` unconsumed → `None`).

- [x] **Step 5: Commit**

```bash
git add crates/uaml/src/layout.rs
git commit -m "feat(uaml): parse Layout placement statements"
```

---

### Task 7: Parse alignment statements (edges + bare center)

Add the alignment branch: `[<edge> of] X aligned with [<edge> of] Y`, edges `top`/`bottom`/`left`/`right`/`center`. Dispatched by a top-level (paren-depth-0) `aligned with` token sequence, checked before placement so `left of X aligned with Y` reads the leading `left` as an edge, not a placement direction.

**Files:**
- Modify: `crates/uaml/src/layout.rs`

**Interfaces:**
- Produces (module-private): `fn parse_alignment(&[Tok]) -> Option<LayoutStatement>`, `fn parse_anchored(&[Tok]) -> Option<Anchored>`, `fn top_level_seq_index(&[Tok], &[&str]) -> Option<usize>`.

- [x] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
    #[test]
    fn parses_anchored_alignment() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- top of VIP aligned with top of Orders").unwrap();
        let LayoutStatement::Alignment { left, right } = stmt else { panic!("expected alignment") };
        assert_eq!(left.edge, Some(Edge::Top));
        assert_eq!(right.edge, Some(Edge::Top));
        assert_eq!(left.operand.ref_, OperandRef::Name(NameRef::Bare("VIP".into())));
    }

    #[test]
    fn parses_bare_center_to_center_alignment() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- X aligned with Y").unwrap();
        let LayoutStatement::Alignment { left, right } = stmt else { panic!() };
        assert_eq!(left.edge, None);
        assert_eq!(right.edge, None);
    }

    #[test]
    fn edge_left_is_not_read_as_placement_direction() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- left of X aligned with right of Y").unwrap();
        let LayoutStatement::Alignment { left, right } = stmt else { panic!("expected alignment, not placement") };
        assert_eq!(left.edge, Some(Edge::Left));
        assert_eq!(right.edge, Some(Edge::Right));
    }
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml layout::tests::parses_anchored_alignment`
Expected: FAIL — no alignment branch; `parse_layout_line` returns `None`.

- [x] **Step 3: Add the alignment branch**

In `crates/uaml/src/layout.rs`, add above the `tests` module:

```rust
/// Index of the first top-level (paren-depth 0) occurrence of the word
/// sequence `seq`, or `None`.
fn top_level_seq_index(toks: &[Tok], seq: &[&str]) -> Option<usize> {
    let mut depth = 0i32;
    for i in 0..toks.len() {
        match &toks[i] {
            Tok::LParen => depth += 1,
            Tok::RParen => depth -= 1,
            Tok::Word(w) if depth == 0 && w.eq_ignore_ascii_case(seq[0]) => {
                let matched = seq.iter().enumerate().all(|(k, s)| {
                    matches!(toks.get(i + k), Some(Tok::Word(x)) if x.eq_ignore_ascii_case(s))
                });
                if matched {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_alignment(toks: &[Tok]) -> Option<LayoutStatement> {
    let idx = top_level_seq_index(toks, &["aligned", "with"])?;
    let left = parse_anchored(&toks[..idx])?;
    let right = parse_anchored(&toks[idx + 2..])?;
    Some(LayoutStatement::Alignment { left, right })
}

fn parse_anchored(toks: &[Tok]) -> Option<Anchored> {
    let mut cur = Cur { toks, pos: 0 };
    let edge = match cur.peek_word() {
        Some(w) => {
            let e = match w.to_ascii_lowercase().as_str() {
                "top" => Some(Edge::Top),
                "bottom" => Some(Edge::Bottom),
                "left" => Some(Edge::Left),
                "right" => Some(Edge::Right),
                "center" => Some(Edge::Center),
                _ => None,
            };
            match e {
                Some(e) => {
                    let save = cur.pos;
                    cur.bump();
                    if cur.eat_word("of") { Some(e) } else { cur.pos = save; None }
                }
                None => None,
            }
        }
        None => None,
    };
    let operand = parse_operand(&mut cur)?;
    if !cur.done() {
        return None;
    }
    Some(Anchored { edge, operand })
}
```

Then, in `parse_layout_line`, add the alignment dispatch **before** the placement attempt (immediately after the `if toks.is_empty()` guard):

```rust
    if top_level_seq_index(&toks, &["aligned", "with"]).is_some() {
        return parse_alignment(&toks);
    }
```

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml layout::tests::`
Expected: PASS (three new tests + all earlier tests).

- [x] **Step 5: Commit**

```bash
git add crates/uaml/src/layout.rs
git commit -m "feat(uaml): parse Layout alignment statements"
```

---

### Task 8: Render Layout statements + wire `## Layout` round-trip

Implement `render_layout_line` (replacing the Task 1 stub) and wire real parsing into `classify` so `## Layout` round-trips as a semantic fixpoint.

**Files:**
- Modify: `crates/uaml/src/layout.rs` (implement renderer)
- Modify: `crates/uaml/src/parse.rs:22-46` (classify: real layout parse)

**Interfaces:**
- Consumes: `parse_layout_line` (Tasks 3–7), the AST (Task 1).
- Produces: working `pub fn render_layout_line(&LayoutStatement) -> String`.

- [x] **Step 1: Write the failing test**

Add to the `tests` module in `crates/uaml/src/layout.rs`:

```rust
    #[test]
    fn layout_lines_round_trip() {
        for line in [
            "- Orders",
            "- Users as column with frame, large margin",
            "- Users left of Orders",
            "- Order above OrderLine above Payment",
            "- top of VIP aligned with top of Orders",
            "- X aligned with Y",
            "- (column of Customer, Account) with large margin",
            "- row of (column of Customer, Account), Orders",
            "- [Money](./money.md) with collapsed",
        ] {
            let parsed = parse_layout_line(line).unwrap_or_else(|| panic!("failed to parse: {line}"));
            let rendered = render_layout_line(&parsed);
            let reparsed = parse_layout_line(&rendered).unwrap_or_else(|| panic!("failed to reparse: {rendered}"));
            assert_eq!(parsed, reparsed, "not a fixpoint: {line} -> {rendered}");
        }
    }

    #[test]
    fn hint_joiner_normalizes_to_comma() {
        let parsed = parse_layout_line("- Users with frame and large margin").unwrap();
        assert_eq!(render_layout_line(&parsed), "- Users with frame, large margin");
    }
```

Also add a `## Layout` round-trip test to `crates/uaml/src/serialize.rs`'s `tests` module:

```rust
    #[test]
    fn serialize_round_trips_layout_section() {
        let src = "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n- Users left of Orders\n- top of Users aligned with top of Orders\n";
        let once = serialize_document(&parse_document(src));
        let twice = serialize_document(&parse_document(&once));
        assert_eq!(once, twice);
        assert!(once.contains("## Layout\n- Users left of Orders"));
    }
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml layout_lines_round_trip`
Expected: FAIL — `render_layout_line` returns `""` (stub).

- [x] **Step 3: Implement the renderer and wire real parsing**

In `crates/uaml/src/layout.rs`, replace the stub `render_layout_line` with:

```rust
pub fn render_layout_line(stmt: &LayoutStatement) -> String {
    let body = match stmt {
        LayoutStatement::Standalone(op) => render_operand(op),
        LayoutStatement::Placement { operands, directions } => {
            let mut s = render_operand(&operands[0]);
            for (d, op) in directions.iter().zip(&operands[1..]) {
                s.push_str(&format!(" {} {}", dir_str(*d), render_operand(op)));
            }
            s
        }
        LayoutStatement::Alignment { left, right } => {
            format!("{} aligned with {}", render_anchored(left), render_anchored(right))
        }
    };
    format!("- {body}")
}

fn render_anchored(a: &Anchored) -> String {
    match a.edge {
        Some(e) => format!("{} of {}", edge_str(e), render_operand(&a.operand)),
        None => render_operand(&a.operand),
    }
}

fn render_operand(op: &Operand) -> String {
    let mut s = render_ref(&op.ref_);
    if let Some(ax) = op.axis {
        s.push_str(&format!(" as {}", axis_str(ax)));
    }
    if !op.hints.is_empty() {
        let hs = op.hints.iter().map(render_hint).collect::<Vec<_>>().join(", ");
        s.push_str(&format!(" with {hs}"));
    }
    s
}

fn render_ref(r: &OperandRef) -> String {
    match r {
        OperandRef::Name(NameRef::Link { title, slug }) => format!("[{title}](./{slug}.md)"),
        OperandRef::Name(NameRef::Bare(name)) => render_bare_name(name),
        OperandRef::InlineGroup { axis, items } => {
            let list = items.iter().map(render_operand).collect::<Vec<_>>().join(", ");
            format!("{} of {}", axis_str(*axis), list)
        }
        OperandRef::Paren(inner) => format!("({})", render_operand(inner)),
    }
}

/// A bare name re-quotes iff it is empty or contains a character the lexer
/// would split on (whitespace or a delimiter).
fn render_bare_name(name: &str) -> String {
    let needs_quote = name.is_empty()
        || name.chars().any(|c| c.is_whitespace() || matches!(c, '(' | ')' | ',' | '[' | ']' | '"'));
    if needs_quote { format!("\"{name}\"") } else { name.to_string() }
}

fn render_hint(h: &Hint) -> String {
    match h {
        Hint::Shape(Shape::Frame) => "frame".to_string(),
        Hint::Shape(Shape::Box) => "box".to_string(),
        Hint::Shape(Shape::Shrink) => "shrink".to_string(),
        Hint::Flag(Flag::Emphasized) => "emphasized".to_string(),
        Hint::Flag(Flag::Collapsed) => "collapsed".to_string(),
        Hint::Margin(m) => {
            let level = match m {
                Margin::No => "no",
                Margin::Small => "small",
                Margin::Medium => "medium",
                Margin::Large => "large",
            };
            format!("{level} margin")
        }
    }
}

fn dir_str(d: Direction) -> &'static str {
    match d {
        Direction::LeftOf => "left of",
        Direction::RightOf => "right of",
        Direction::Above => "above",
        Direction::Below => "below",
    }
}

fn edge_str(e: Edge) -> &'static str {
    match e {
        Edge::Top => "top",
        Edge::Bottom => "bottom",
        Edge::Left => "left",
        Edge::Right => "right",
        Edge::Center => "center",
    }
}

fn axis_str(a: Axis) -> &'static str {
    match a { Axis::Row => "row", Axis::Column => "column" }
}
```

In `crates/uaml/src/parse.rs`, change the `"layout"` arm in `classify` from the Task 1 stub to real parsing:

```rust
        "layout" => Section::Layout(
            lines(content).iter().filter_map(|l| crate::layout::parse_layout_line(l)).collect(),
        ),
```

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml`
Expected: PASS (layout round-trip, joiner normalization, serialize layout round-trip, plus the whole crate).

- [x] **Step 5: Commit**

```bash
git add crates/uaml/src/layout.rs crates/uaml/src/parse.rs crates/uaml/src/serialize.rs
git commit -m "feat(uaml): render Layout statements and round-trip the section"
```

---

### Task 9: Grouped `## Members` (nested heading tree) + drop `at x,y`

Replace the flat `Section::Members(Vec<MemberLine>)` with a nested-group tree (`MembersBlock`). Sub-headings `###`+ under `## Members` declare groups; a flat bullet list is one implicit top-level group. Nesting is heading depth. The `MemberLine.position` field and the `at x,y` syntax are removed.

**Files:**
- Modify: `crates/uaml/src/syntax.rs` (add `MembersBlock`/`MemberGroup`; change `Section::Members`; drop `MemberLine.position`)
- Modify: `crates/uaml/src/grammar.rs` (`MEMBER_RE`, `parse_member_line`, `render_member_line`, `parse_members_block`, `render_members_block`; update member test)
- Modify: `crates/uaml/src/parse.rs` (classify `"members"`; `build_diagrams` members handling — temporary shim)
- Modify: `crates/uaml/src/serialize.rs` (`render_section` Members arm)

**Interfaces:**
- Produces:
  ```rust
  pub struct MembersBlock { pub groups: Vec<MemberGroup> }
  pub struct MemberGroup { pub name: String, pub depth: u8, pub members: Vec<MemberLine>, pub children: Vec<MemberGroup> }
  pub struct MemberLine { pub title: String, pub slug: String }  // position removed
  pub fn parse_members_block(content: &str) -> MembersBlock;      // in grammar.rs
  pub fn render_members_block(block: &MembersBlock) -> String;    // in grammar.rs
  ```

- [x] **Step 1: Write the failing test**

Add to the `tests` module in `crates/uaml/src/grammar.rs`:

```rust
    #[test]
    fn parses_nested_member_groups() {
        let content = "### Users\n- [Customer](./customer.md)\n\n#### VIP\n- [Platinum](./platinum.md)\n\n### Orders\n- [Order](./order.md)";
        let block = parse_members_block(content);
        assert_eq!(block.groups.len(), 2);
        assert_eq!(block.groups[0].name, "Users");
        assert_eq!(block.groups[0].depth, 3);
        assert_eq!(block.groups[0].members[0].slug, "customer");
        assert_eq!(block.groups[0].children[0].name, "VIP");
        assert_eq!(block.groups[0].children[0].depth, 4);
        assert_eq!(block.groups[1].name, "Orders");
    }

    #[test]
    fn flat_list_is_one_implicit_group_and_round_trips() {
        let content = "- [Order](./order.md)\n- [Customer](./customer.md)";
        let block = parse_members_block(content);
        assert_eq!(block.groups.len(), 1);
        assert_eq!(block.groups[0].name, "");
        assert_eq!(block.groups[0].depth, 0);
        assert_eq!(block.groups[0].members.len(), 2);

        let rendered = render_members_block(&block);
        let reparsed = parse_members_block(rendered.strip_prefix("## Members\n").unwrap());
        assert_eq!(block, reparsed);
    }

    #[test]
    fn member_line_has_no_position() {
        let m = parse_member_line("- [Order](./order.md)").unwrap();
        assert_eq!(m.slug, "order");
        assert_eq!(render_member_line(&m), "- [Order](./order.md)");
    }
```

Delete the obsolete `parses_and_renders_member_with_position` test in the same module (lines ~257–263).

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml parses_nested_member_groups`
Expected: FAIL — `parse_members_block` / `MembersBlock` not found; `MemberLine` still has `position`.

- [x] **Step 3: Implement the grouped members**

In `crates/uaml/src/syntax.rs`: drop `position` from `MemberLine`, and add the group types after it:

```rust
/// One `## Members` bullet in a diagram document.
#[derive(Debug, Clone, PartialEq)]
pub struct MemberLine {
    pub title: String,
    pub slug: String,
}

/// The `## Members` section: a forest of groups. A flat bullet list (no
/// sub-headings) is a single implicit top-level group (name `""`, depth 0).
#[derive(Debug, Clone, PartialEq)]
pub struct MembersBlock {
    pub groups: Vec<MemberGroup>,
}

/// A membership group. `name` is the heading text (`""` for the implicit
/// top-level group); `depth` is the heading level (3 for `###`, 0 implicit).
#[derive(Debug, Clone, PartialEq)]
pub struct MemberGroup {
    pub name: String,
    pub depth: u8,
    pub members: Vec<MemberLine>,
    pub children: Vec<MemberGroup>,
}
```

Change the `Section` enum's members arm:

```rust
    Members(MembersBlock),
```

In `crates/uaml/src/grammar.rs`: drop the `at x,y` capture from `MEMBER_RE`, simplify `parse_member_line` and `render_member_line`, and add the block parser/renderer. Replace `MEMBER_RE` (line ~27) with:

```rust
static MEMBER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- \[([^\]]*)\]\(\./(.+?)\.md\)\s*$").unwrap());
```

Replace `parse_member_line` (lines ~116–124) with:

```rust
pub fn parse_member_line(line: &str) -> Option<MemberLine> {
    let line = line.trim_end_matches('\r').trim();
    let m = MEMBER_RE.captures(line)?;
    Some(MemberLine { title: m[1].to_string(), slug: basename(&m[2]).to_string() })
}
```

Replace `render_member_line` (lines ~174–180) with:

```rust
pub fn render_member_line(m: &MemberLine) -> String {
    format!("- [{}](./{}.md)", m.title, m.slug)
}
```

Add (near the other `parse_*`/`render_*` functions) the block parser and renderer, plus the `MembersBlock`/`MemberGroup` imports. Update the `use crate::syntax::{...}` line at the top of `grammar.rs` to include `MemberGroup, MembersBlock`:

```rust
fn heading_depth(line: &str) -> Option<(u8, String)> {
    if !line.starts_with("###") {
        return None; // `##` is the section itself; groups start at `###`
    }
    let hashes = line.chars().take_while(|&c| c == '#').count();
    let name = line[hashes..].trim().to_string();
    Some((hashes as u8, name))
}

/// Parse the raw text under `## Members` into a group forest.
pub fn parse_members_block(content: &str) -> MembersBlock {
    fn close_to(stack: &mut Vec<MemberGroup>, groups: &mut Vec<MemberGroup>, depth: u8) {
        while let Some(top) = stack.last() {
            if top.depth >= depth {
                let g = stack.pop().unwrap();
                match stack.last_mut() {
                    Some(parent) => parent.children.push(g),
                    None => groups.push(g),
                }
            } else {
                break;
            }
        }
    }

    let mut groups: Vec<MemberGroup> = Vec::new();
    let mut implicit = MemberGroup { name: String::new(), depth: 0, members: vec![], children: vec![] };
    let mut stack: Vec<MemberGroup> = Vec::new();

    for raw in content.lines() {
        let line = raw.trim_end_matches('\r');
        let t = line.trim_start();
        if let Some((depth, name)) = heading_depth(t) {
            close_to(&mut stack, &mut groups, depth);
            stack.push(MemberGroup { name, depth, members: vec![], children: vec![] });
        } else if let Some(m) = parse_member_line(t) {
            match stack.last_mut() {
                Some(g) => g.members.push(m),
                None => implicit.members.push(m),
            }
        }
        // blank / unrecognized lines are ignored here (validate flags droppable content)
    }
    close_to(&mut stack, &mut groups, 0);

    if !implicit.members.is_empty() {
        groups.insert(0, implicit);
    } else if groups.is_empty() {
        groups.push(implicit); // empty `## Members` yields one empty implicit group
    }
    MembersBlock { groups }
}

/// Render a members block, heading included, as valid `## Members` Markdown.
pub fn render_members_block(block: &MembersBlock) -> String {
    fn render_group(out: &mut String, g: &MemberGroup) {
        if g.depth > 0 {
            out.push_str(&format!("\n\n{} {}", "#".repeat(g.depth as usize), g.name));
        }
        for m in &g.members {
            out.push('\n');
            out.push_str(&render_member_line(m));
        }
        for c in &g.children {
            render_group(out, c);
        }
    }
    let mut out = String::from("## Members");
    for g in &block.groups {
        render_group(&mut out, g);
    }
    out
}
```

In `crates/uaml/src/parse.rs`, change the `"members"` classify arm to:

```rust
        "members" => Section::Members(crate::grammar::parse_members_block(content)),
```

Remove the now-unused `parse_member_line` import from `grammar` in `parse.rs` if the compiler flags it (it is still used indirectly via `parse_members_block`). In `build_diagrams` (lines ~286–309), replace the `Section::Members(ms) => { … }` arm with a temporary shim that flattens groups so the existing `Diagram.members: Vec<Member>` still compiles (Task 11 replaces this):

```rust
                Section::Members(block) => {
                    fn collect(g: &crate::syntax::MemberGroup, keyset: &HashSet<&str>, out: &mut Vec<Member>) {
                        for m in &g.members {
                            if keyset.contains(m.slug.as_str()) {
                                out.push(Member { key: m.slug.clone() });
                            }
                        }
                        for c in &g.children {
                            collect(c, keyset, out);
                        }
                    }
                    for g in &block.groups {
                        collect(g, keyset, &mut members);
                    }
                }
```

Since `Member` no longer carries a position, update `crates/uaml/src/model.rs` `Member` (lines ~216–221) to drop the field:

```rust
/// A diagram member: a classifier slug drawn in a view.
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub key: String,
}
```

And update the `d.members[0].position` assertion in the `parse.rs` `model_tests` module (line ~498) — delete that one assertion line (position no longer exists); keep the surrounding membership assertions.

In `crates/uaml/src/serialize.rs`, change the Members arm of `render_section` to:

```rust
        Section::Members(block) => crate::grammar::render_members_block(block),
```

In `crates/uaml/src/validate.rs`, the `"members"` line-lint arm (lines ~166–177) calls `parse_member_line`; leave it — it still parses individual bullet lines. (Sub-heading exemption is handled in Task 12.)

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml`
Expected: PASS (grouped-member tests + whole crate; the old position test is gone).

- [x] **Step 5: Commit**

```bash
git add crates/uaml/src/syntax.rs crates/uaml/src/grammar.rs crates/uaml/src/parse.rs crates/uaml/src/serialize.rs crates/uaml/src/model.rs
git commit -m "feat(uaml): grouped Members tree; drop stored coordinates"
```

---

### Task 10: Remove the superseded `## Render hints` section

`## Layout` supersedes `## Render hints`; its per-node `emphasize`/`collapse` flags move to layout operand treatment (`emphasized`/`collapsed`). Remove the `RenderHints` section, `HintLine`, and their parsers/renderers. Any lingering `## Render hints` in a document degrades to an `Unknown` section (preserved verbatim), not dropped.

**Files:**
- Modify: `crates/uaml/src/syntax.rs` (remove `Section::RenderHints`, `HintLine`)
- Modify: `crates/uaml/src/grammar.rs` (remove `parse_hint_line`, `render_hint_line`, `EMPHASIZE_RE`, `COLLAPSE_RE`, their tests)
- Modify: `crates/uaml/src/parse.rs` (remove `"render hints"` classify arm; remove `RenderHints` handling in `build_diagrams`; remove `HintLine` import)
- Modify: `crates/uaml/src/serialize.rs` (remove `RenderHints` arms)
- Modify: `crates/uaml/src/model.rs` (remove `RenderHints`; drop `Diagram.hints` — folded into `groups`/`layout` in Task 11)

**Interfaces:**
- Removes: `Section::RenderHints`, `HintLine`, `RenderHints`, `parse_hint_line`, `render_hint_line`.

- [x] **Step 1: Write the failing test**

Add to the `tests` module in `crates/uaml/src/serialize.rs`:

```rust
    #[test]
    fn render_hints_section_degrades_to_preserved_unknown() {
        let src = "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Render hints\n- emphasize: order\n";
        let out = serialize_document(&parse_document(src));
        // Preserved verbatim as an Unknown section, not silently dropped.
        assert!(out.contains("## Render hints\n- emphasize: order"));
    }
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml render_hints_section_degrades_to_preserved_unknown`
Expected: FAIL — `## Render hints` is currently classified as `Section::RenderHints` and re-rendered through `render_hint_line`, so the assertion may pass by coincidence OR the crate still references removed symbols after Step 3. Run first to confirm current behavior; the meaningful check is that after Step 3 the section flows through `Unknown`.

- [x] **Step 3: Remove RenderHints throughout**

In `crates/uaml/src/syntax.rs`: delete the `RenderHints(Vec<HintLine>)` variant from `Section`, and delete the `HintLine` enum (lines ~50–55).

In `crates/uaml/src/grammar.rs`: delete `EMPHASIZE_RE`, `COLLAPSE_RE` (lines ~30–33), `parse_hint_line` (lines ~126–136), `render_hint_line` (lines ~182–187), the `HintLine` import, and the `parse_hint_line` unit test (lines ~266–270).

In `crates/uaml/src/parse.rs`: delete the `"render hints"` classify arm (lines ~37–39); delete `use crate::syntax::HintLine;` (line ~271); in `build_diagrams`, delete the `Section::RenderHints(hs) => { … }` arm and the `hints` local + its use; delete the `RenderHints` name from the `use crate::model::{…}` import (line ~13); delete the `d.hints.emphasize` / `d.hints.collapse` assertions in `model_tests` (lines ~499–500).

In `crates/uaml/src/serialize.rs`: delete `render_hint_line` from the `use crate::grammar::{…}` import; delete the `Section::RenderHints(_) => 7,` line from `section_order` (renumber `Unknown` to `7`); delete the `Section::RenderHints(hints) => { … }` arm from `render_section`.

Resulting `section_order`:

```rust
        Section::Members(_) => 5,
        Section::Layout(_) => 6,
        Section::Unknown { .. } => 7,
```

In `crates/uaml/src/model.rs`: delete the `RenderHints` struct (lines ~223–227) and the `hints: RenderHints` field from `Diagram` (Task 11 finalizes `Diagram`).

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml`
Expected: PASS (the degrade test + whole crate compiles with no `RenderHints`/`HintLine` references).

- [x] **Step 5: Commit**

```bash
git add crates/uaml/src/syntax.rs crates/uaml/src/grammar.rs crates/uaml/src/parse.rs crates/uaml/src/serialize.rs crates/uaml/src/model.rs
git commit -m "refactor(uaml): remove Render hints section (superseded by Layout)"
```

---

### Task 11: Resolve grouped members + layout into `model::Diagram`

Finalize the resolved model: `Diagram` carries a resolved group forest (`groups`) and the parsed layout statements (`layout`), replacing the flat `members`/`hints`.

**Files:**
- Modify: `crates/uaml/src/model.rs` (`Diagram`, `DiagramGroup`; remove `Member`)
- Modify: `crates/uaml/src/parse.rs` (`build_diagrams`; imports)

**Interfaces:**
- Produces:
  ```rust
  pub struct DiagramGroup { pub name: String, pub members: Vec<String>, pub children: Vec<DiagramGroup> }
  pub struct Diagram {
      pub key: String, pub title: String, pub profile: String,
      pub groups: Vec<DiagramGroup>,
      pub layout: Vec<crate::syntax::LayoutStatement>,
  }
  ```

- [x] **Step 1: Write the failing test**

Add to the `model_tests` module in `crates/uaml/src/parse.rs` (adapt the existing diagram test fixture, or add a new test):

```rust
    #[test]
    fn builds_diagram_groups_and_layout() {
        let diagram = "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n\n### Orders\n- [Order](./order.md)\n\n## Layout\n- Users left of Orders\n";
        let bundle = vec![
            ("customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
            ("order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("orders.md".to_string(), diagram.to_string()),
        ];
        let model = build_model(&bundle);
        let d = model.diagrams.iter().find(|d| d.key == "orders").unwrap();
        assert_eq!(d.groups.len(), 2);
        assert_eq!(d.groups[0].name, "Users");
        assert_eq!(d.groups[0].members, vec!["customer".to_string()]);
        assert_eq!(d.layout.len(), 1);
        assert!(matches!(d.layout[0], crate::syntax::LayoutStatement::Placement { .. }));
    }
```

Delete the old flat-members diagram test in `model_tests` that referenced `d.members` (it no longer compiles).

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml builds_diagram_groups_and_layout`
Expected: FAIL — `Diagram` has no `groups`/`layout` fields.

- [x] **Step 3: Finalize the model and `build_diagrams`**

In `crates/uaml/src/model.rs`, delete the `Member` struct and replace the `Diagram` struct with:

```rust
/// A resolved membership group in a diagram (heading text + resolved keys).
#[derive(Debug, Clone, PartialEq)]
pub struct DiagramGroup {
    pub name: String,
    pub members: Vec<String>,
    pub children: Vec<DiagramGroup>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagram {
    pub key: String,
    pub title: String,
    pub profile: String,
    pub groups: Vec<DiagramGroup>,
    pub layout: Vec<crate::syntax::LayoutStatement>,
}
```

In `crates/uaml/src/parse.rs`, update the `use crate::model::{…}` import (remove `Member`, keep `Diagram`; add `DiagramGroup`), and replace the body of `build_diagrams` members/layout handling with:

```rust
fn resolve_group(g: &crate::syntax::MemberGroup, keyset: &HashSet<&str>) -> DiagramGroup {
    DiagramGroup {
        name: g.name.clone(),
        members: g
            .members
            .iter()
            .filter(|m| keyset.contains(m.slug.as_str()))
            .map(|m| m.slug.clone())
            .collect(),
        children: g.children.iter().map(|c| resolve_group(c, keyset)).collect(),
    }
}

fn build_diagrams(parsed: &[ParsedDoc], keyset: &HashSet<&str>) -> Vec<Diagram> {
    let mut out = Vec::new();
    for p in parsed.iter().filter(|p| p.ty == ClassifierType::Diagram) {
        let fm = &p.doc.frontmatter;
        let title = fm.get_str("title").map(String::from).unwrap_or_else(|| "Untitled diagram".to_string());
        let profile = fm
            .get_str("profile")
            .filter(|s| !s.is_empty())
            .unwrap_or("uml-domain")
            .to_string();

        let mut groups = Vec::new();
        let mut layout = Vec::new();
        for s in &p.doc.sections {
            match s {
                Section::Members(block) => {
                    groups = block.groups.iter().map(|g| resolve_group(g, keyset)).collect();
                }
                Section::Layout(stmts) => {
                    layout = stmts.clone();
                }
                _ => {}
            }
        }
        out.push(Diagram { key: p.slug.clone(), title, profile, groups, layout });
    }
    out
}
```

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml`
Expected: PASS (`builds_diagram_groups_and_layout` + whole crate).

- [x] **Step 5: Commit**

```bash
git add crates/uaml/src/model.rs crates/uaml/src/parse.rs
git commit -m "feat(uaml): resolve grouped members and layout into model::Diagram"
```

---

### Task 12: Validate malformed layout lines + unknown operand references

Add diagnostics: a malformed `## Layout` bullet is an error; a `## Members` sub-heading is not droppable content; a `## Layout` operand referencing neither a diagram member key nor a declared group name is a warning.

**Files:**
- Modify: `crates/uaml/src/diagnostic.rs` (add `MalformedLayout`, `UnresolvedLayoutRef`)
- Modify: `crates/uaml/src/validate.rs` (line-lint arm + exemption; semantic ref pass)

**Interfaces:**
- Consumes: `parse_layout_line`, `parse_members_block`, `parse_document`, the AST.
- Produces (validate.rs, module-private): `fn validate_diagram_refs(path, text, keyset, diags)`.

- [x] **Step 1: Write the failing test**

Add to the `tests` module in `crates/uaml/src/validate.rs`:

```rust
    #[test]
    fn malformed_layout_line_is_an_error() {
        let bundle = vec![(
            "d.md".to_string(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Layout\n- Users nonsense Orders\n".to_string(),
        )];
        let diags = validate(&bundle);
        assert!(diags.iter().any(|d| d.code == DiagCode::MalformedLayout));
    }

    #[test]
    fn member_subheading_is_not_droppable() {
        let bundle = vec![(
            "d.md".to_string(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n".to_string(),
        )];
        let diags = validate(&bundle);
        assert!(!diags.iter().any(|d| d.code == DiagCode::DroppableContent),
            "### group heading must not be flagged droppable");
    }

    #[test]
    fn unknown_layout_ref_is_a_warning() {
        let bundle = vec![
            ("customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
            ("d.md".to_string(),
             "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n\n## Layout\n- Users left of Ghosts\n".to_string()),
        ];
        let diags = validate(&bundle);
        // "Users" is a declared group, "Ghosts" resolves to nothing -> one warning.
        let refs: Vec<_> = diags.iter().filter(|d| d.code == DiagCode::UnresolvedLayoutRef).collect();
        assert_eq!(refs.len(), 1);
        assert!(refs[0].message.contains("Ghosts"));
    }
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml malformed_layout_line_is_an_error`
Expected: FAIL — `DiagCode::MalformedLayout` not found.

- [x] **Step 3: Add codes, the lint arm, the exemption, and the ref pass**

In `crates/uaml/src/diagnostic.rs`, add to the `DiagCode` enum and `as_str`:

```rust
    MalformedLayout,
    UnresolvedLayoutRef,
```
```rust
            DiagCode::MalformedLayout => "malformed-layout",
            DiagCode::UnresolvedLayoutRef => "unresolved-layout-ref",
```

In `severity`, add `UnresolvedLayoutRef` to the warning list (it becomes a warning), leaving `MalformedLayout` as the default error:

```rust
    pub fn severity(self) -> Severity {
        match self {
            DiagCode::UnknownType | DiagCode::UnresolvedLayoutRef => Severity::Warning,
            _ => Severity::Error,
        }
    }
```

In `crates/uaml/src/validate.rs`, update the droppable-content guard (lines ~128–140) to include `"layout"` in the bullet-section list AND exempt `###`+ headings in the members section. Replace that block with:

```rust
        if !trimmed.is_empty() {
            let is_h1 = trimmed.starts_with('#') && !trimmed.starts_with("##");
            let is_member_group_heading = section == "members" && trimmed.starts_with("###");
            if !is_h1 && !is_member_group_heading {
                let in_bullet_section = matches!(
                    section.as_str(),
                    "attributes" | "values" | "relationships" | "members" | "layout"
                );
                if (!seen_section || in_bullet_section) && !trimmed.starts_with("- ") {
                    diags.push(Diagnostic::new(
                        DiagCode::DroppableContent,
                        "content here is outside the recognized document structure and would be silently dropped by fmt",
                        path,
                        n,
                    ));
                }
            }
        }
```

In the `match section.as_str()` block (lines ~147–178), add a `"layout"` arm:

```rust
            "layout" => {
                if crate::layout::parse_layout_line(trimmed).is_none() {
                    diags.push(Diagnostic::new(
                        DiagCode::MalformedLayout,
                        "malformed layout statement",
                        path,
                        n,
                    ));
                }
            }
```

Add the semantic ref pass. At the top of `validate.rs`, add imports:

```rust
use crate::parse::parse_document;
use crate::syntax::{LayoutStatement, MemberGroup, NameRef, Operand, OperandRef, Section};
```

Add the function (module scope):

```rust
/// Collect every group's heading name (recursively) into `names`.
fn collect_group_names(g: &MemberGroup, names: &mut HashSet<String>) {
    if !g.name.is_empty() {
        names.insert(g.name.clone());
    }
    for c in &g.children {
        collect_group_names(c, names);
    }
}

/// Walk an operand, reporting each `Name` ref that resolves to neither a
/// member key nor a declared group name.
fn check_operand_refs(
    op: &Operand,
    keyset: &HashSet<String>,
    group_names: &HashSet<String>,
    path: &str,
    line: usize,
    diags: &mut Vec<Diagnostic>,
) {
    match &op.ref_ {
        OperandRef::Name(name) => {
            let (label, resolved) = match name {
                NameRef::Link { slug, .. } => (slug.clone(), keyset.contains(slug)),
                NameRef::Bare(s) => (s.clone(), keyset.contains(s) || group_names.contains(s)),
            };
            if !resolved {
                diags.push(Diagnostic::warn(
                    DiagCode::UnresolvedLayoutRef,
                    format!("layout operand '{label}' resolves to no member or group"),
                    path,
                    line,
                ));
            }
        }
        OperandRef::InlineGroup { items, .. } => {
            for it in items {
                check_operand_refs(it, keyset, group_names, path, line, diags);
            }
        }
        OperandRef::Paren(inner) => check_operand_refs(inner, keyset, group_names, path, line, diags),
    }
}

fn validate_diagram_refs(path: &str, text: &str, keyset: &HashSet<String>, diags: &mut Vec<Diagnostic>) {
    if doc_type(text) != "Diagram" {
        return;
    }
    let doc = parse_document(text);
    let mut group_names = HashSet::new();
    let mut layout: &[LayoutStatement] = &[];
    for s in &doc.sections {
        match s {
            Section::Members(block) => {
                for g in &block.groups {
                    collect_group_names(g, &mut group_names);
                }
            }
            Section::Layout(stmts) => layout = stmts,
            _ => {}
        }
    }
    // Line number is approximate (the statement's position within the doc is
    // not tracked here); use the `## Layout` heading line as the anchor.
    let layout_line = text.lines().position(|l| l.trim().to_lowercase() == "## layout").map(|i| i + 1).unwrap_or(1);
    for stmt in layout {
        let ops: Vec<&Operand> = match stmt {
            LayoutStatement::Standalone(op) => vec![op],
            LayoutStatement::Placement { operands, .. } => operands.iter().collect(),
            LayoutStatement::Alignment { left, right } => vec![&left.operand, &right.operand],
        };
        for op in ops {
            check_operand_refs(op, keyset, &group_names, path, layout_line, diags);
        }
    }
}
```

Wire the pass into `validate()`. In the per-document loop (after `validate_doc(...)` is called for each `(path, text)`), add:

```rust
        validate_diagram_refs(path, text, &keyset, &mut diags);
```

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml`
Expected: PASS (three new validation tests + whole crate).

- [x] **Step 5: Commit**

```bash
git add crates/uaml/src/diagnostic.rs crates/uaml/src/validate.rs
git commit -m "feat(uaml): validate malformed layout and unknown operand refs"
```

---

### Task 13: Validate layout placement cycles

Detect contradictory placement constraints: `A left of B` and `B left of A` (or the `above`/`below` equivalent) form a directed cycle and are unsatisfiable. Build one directed graph per axis (horizontal from `left of`/`right of`, vertical from `above`/`below`) over named operands and report an error on any cycle.

**Files:**
- Modify: `crates/uaml/src/diagnostic.rs` (add `LayoutCycle`)
- Modify: `crates/uaml/src/validate.rs` (cycle detection in the diagram pass)

**Interfaces:**
- Consumes: the layout AST, `validate_diagram_refs`'s parsed statements.
- Produces (module-private): `fn operand_key(&Operand) -> Option<String>`, `fn has_cycle(&HashMap<String, Vec<String>>) -> bool`.

- [x] **Step 1: Write the failing test**

Add to the `tests` module in `crates/uaml/src/validate.rs`:

```rust
    #[test]
    fn contradictory_placement_is_a_cycle_error() {
        let bundle = vec![(
            "d.md".to_string(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Layout\n- A left of B\n- B left of A\n".to_string(),
        )];
        let diags = validate(&bundle);
        assert!(diags.iter().any(|d| d.code == DiagCode::LayoutCycle));
    }

    #[test]
    fn consistent_placement_has_no_cycle() {
        let bundle = vec![(
            "d.md".to_string(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Layout\n- A left of B left of C\n- A above D\n".to_string(),
        )];
        let diags = validate(&bundle);
        assert!(!diags.iter().any(|d| d.code == DiagCode::LayoutCycle));
    }
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml contradictory_placement_is_a_cycle_error`
Expected: FAIL — `DiagCode::LayoutCycle` not found.

- [x] **Step 3: Add the code and cycle detection**

In `crates/uaml/src/diagnostic.rs`, add `LayoutCycle` to `DiagCode` and `as_str` (it is an error — the default severity, no change needed):

```rust
    LayoutCycle,
```
```rust
            DiagCode::LayoutCycle => "layout-cycle",
```

In `crates/uaml/src/validate.rs`, add these helpers (module scope):

```rust
/// A stable key for a named operand (its slug or bare name); `None` for an
/// anonymous inline group.
fn operand_key(op: &Operand) -> Option<String> {
    match &op.ref_ {
        OperandRef::Name(NameRef::Link { slug, .. }) => Some(slug.clone()),
        OperandRef::Name(NameRef::Bare(s)) => Some(s.clone()),
        OperandRef::Paren(inner) => operand_key(inner),
        OperandRef::InlineGroup { .. } => None,
    }
}

/// Depth-first cycle check over a directed adjacency map.
fn has_cycle(graph: &HashMap<String, Vec<String>>) -> bool {
    // 0 = unvisited, 1 = on stack, 2 = done
    fn dfs(node: &str, graph: &HashMap<String, Vec<String>>, state: &mut HashMap<String, u8>) -> bool {
        state.insert(node.to_string(), 1);
        if let Some(succs) = graph.get(node) {
            for s in succs {
                match state.get(s).copied().unwrap_or(0) {
                    1 => return true,
                    0 => {
                        if dfs(s, graph, state) {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
        }
        state.insert(node.to_string(), 2);
        false
    }
    let mut state: HashMap<String, u8> = HashMap::new();
    for node in graph.keys() {
        if state.get(node).copied().unwrap_or(0) == 0 && dfs(node, graph, &mut state) {
            return true;
        }
    }
    false
}
```

Extend `validate_diagram_refs` to also build the two graphs and report a cycle. Add, before the function's closing brace (after the ref-checking loop), a second pass over `layout`:

```rust
    use crate::syntax::Direction;
    let mut horizontal: HashMap<String, Vec<String>> = HashMap::new();
    let mut vertical: HashMap<String, Vec<String>> = HashMap::new();
    for stmt in layout {
        if let LayoutStatement::Placement { operands, directions } = stmt {
            for (i, dir) in directions.iter().enumerate() {
                let (a, b) = (operand_key(&operands[i]), operand_key(&operands[i + 1]));
                let (Some(a), Some(b)) = (a, b) else { continue };
                // Edge points from the operand that must come first to the one after it.
                let (graph, from, to) = match dir {
                    Direction::LeftOf => (&mut horizontal, a, b),
                    Direction::RightOf => (&mut horizontal, b, a),
                    Direction::Above => (&mut vertical, a, b),
                    Direction::Below => (&mut vertical, b, a),
                };
                graph.entry(from).or_default().push(to);
            }
        }
    }
    if has_cycle(&horizontal) || has_cycle(&vertical) {
        let layout_line = text.lines().position(|l| l.trim().to_lowercase() == "## layout").map(|i| i + 1).unwrap_or(1);
        diags.push(Diagnostic::new(
            DiagCode::LayoutCycle,
            "layout placement constraints form a cycle (contradictory ordering)",
            path,
            layout_line,
        ));
    }
```

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml`
Expected: PASS (both cycle tests + whole crate).

- [x] **Step 5: Commit**

```bash
git add crates/uaml/src/diagnostic.rs crates/uaml/src/validate.rs
git commit -m "feat(uaml): detect contradictory layout placement cycles"
```

---

## Final verification

- [x] Run the full crate suite: `cargo test -p uaml` — all green.
- [x] Run `cargo clippy -p uaml` — no new warnings (in particular, no dead-code warnings from removed `RenderHints`/`position` symbols).
- [x] Confirm the spec's worked example parses and round-trips: add a temporary test (or a `cargo run` scratch) feeding the `docs/uaml-spec.md` "Worked example" diagram (lines ~680+) through `parse_document` → `serialize_document` twice and assert a fixpoint. Remove the scratch after confirming.

## Spec-coverage map (self-review)

| Spec production / rule (docs/uaml-spec.md) | Task |
|---|---|
| `<statement>` = placement \| alignment \| standalone | 3, 6, 7 |
| `<placement>`, `<direction>` (4 directions, chainable) | 6 |
| `<alignment>`, `<anchored>`, `<edge>` (top/bottom/left/right/center; bare = center) | 7 |
| `<standalone>` (lone treated operand) | 3, 4 |
| `<operand>` = ref [`as` axis] [`with` hints] | 3, 4 |
| `<ref>` = name \| inline-group \| `( … )` | 3, 5 |
| `<inline-group>` (`column of`/`row of`), `<operand-list>` | 5 |
| `<hints>`, `<hint>`, `<shape>`, `<margin>`, `<flag>` | 4 |
| `<name>` = ident \| link \| quoted | 2, 3 |
| Greedy `with` binding; parens override | 5, 6 |
| Members = membership only; groups = headings; nesting by depth; flat = implicit group | 9 |
| Default clump (no axis stored unless `as row`/`as column`) | 1, 9 (axis captured, no solver) |
| `## Layout` supersedes `## Render hints`; `at x,y` removed | 9, 10 |
| Round-trip (parse ↔ serialize, fixpoint) | 8, 9 |
| Validation: unknown refs | 12 |
| Validation: malformed layout / bad structure | 12 |
| Validation: cycles | 13 |
