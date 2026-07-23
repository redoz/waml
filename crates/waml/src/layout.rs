//! Tokenizer, recursive-descent parser, and renderer for the `## Layout`
//! diagram-arrangement language. See docs/waml-spec.md (Diagram documents).

use crate::syntax::*;
use regex::Regex;
use std::sync::LazyLock;

/// Render one `## Layout` statement back to its `- …` bullet text.
pub fn render_layout_line(stmt: &LayoutStatement) -> String {
    let body = match stmt {
        LayoutStatement::Standalone(op) => render_operand(op),
        LayoutStatement::Placement {
            operands,
            directions,
        } => {
            let mut s = render_operand(&operands[0]);
            for (d, op) in directions.iter().zip(&operands[1..]) {
                s.push_str(&format!(" {} {}", dir_str(*d), render_operand(op)));
            }
            s
        }
        LayoutStatement::Alignment { left, right } => {
            format!(
                "{} aligned with {}",
                render_anchored(left),
                render_anchored(right)
            )
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
        let hs = op
            .hints
            .iter()
            .map(render_hint)
            .collect::<Vec<_>>()
            .join(", ");
        s.push_str(&format!(" with {hs}"));
    }
    s
}

fn render_ref(r: &OperandRef) -> String {
    match r {
        OperandRef::Name(NameRef::Link { title, slug }) => format!("[{title}](./{slug}.md)"),
        OperandRef::Name(NameRef::Bare(name)) => render_bare_name(name),
        OperandRef::InlineGroup { axis, items } => {
            let list = items
                .iter()
                .map(render_operand)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{} of {list}", axis_str(*axis))
        }
        OperandRef::Paren(inner) => format!("({})", render_operand(inner)),
    }
}

/// A bare name re-quotes iff it is empty or contains a character the lexer
/// would split on (whitespace or a delimiter).
fn render_bare_name(name: &str) -> String {
    let needs_quote = name.is_empty()
        || name
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '(' | ')' | ',' | '[' | ']' | '"'))
        || name.eq_ignore_ascii_case("column")
        || name.eq_ignore_ascii_case("row");
    if needs_quote {
        format!("\"{name}\"")
    } else {
        name.to_string()
    }
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
        Direction::AboveLeft => "above left of",
        Direction::AboveRight => "above right of",
        Direction::BelowLeft => "below left of",
        Direction::BelowRight => "below right of",
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
    match a {
        Axis::Row => "row",
        Axis::Column => "column",
    }
}

// Anchored at the start: `[title](./slug.md)`. Slug may contain a directory prefix.
static LAYOUT_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[([^\]]*)\]\(\./(.+?)\.md\)").unwrap());

/// Raw captured href stem (dir prefix intact, `.md` already excluded by
/// `LAYOUT_LINK_RE`'s capture group) — resolved against the referring
/// document's directory downstream, mirroring `grammar.rs`'s relationship/
/// member captures. NOT basename-stripped: two docs sharing a basename in
/// different directories must stay distinguishable.
fn link_slug(path: &str) -> String {
    path.to_string()
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
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
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

struct Cur<'a> {
    toks: &'a [Tok],
    pos: usize,
}

impl<'a> Cur<'a> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn peek_word(&self) -> Option<&str> {
        match self.toks.get(self.pos) {
            Some(Tok::Word(w)) => Some(w.as_str()),
            _ => None,
        }
    }
    fn bump(&mut self) -> Option<&Tok> {
        let t = self.toks.get(self.pos);
        if t.is_some() {
            self.pos += 1;
        }
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
    fn done(&self) -> bool {
        self.pos >= self.toks.len()
    }
}

fn eat_direction(cur: &mut Cur) -> Option<Direction> {
    match cur.peek_word()?.to_ascii_lowercase().as_str() {
        "above" => {
            cur.bump(); // consume "above"
            let save = cur.pos;
            match cur.peek_word().map(|w| w.to_ascii_lowercase()).as_deref() {
                Some("left") => {
                    cur.bump();
                    if cur.eat_word("of") {
                        Some(Direction::AboveLeft)
                    } else {
                        cur.pos = save;
                        Some(Direction::Above)
                    }
                }
                Some("right") => {
                    cur.bump();
                    if cur.eat_word("of") {
                        Some(Direction::AboveRight)
                    } else {
                        cur.pos = save;
                        Some(Direction::Above)
                    }
                }
                _ => Some(Direction::Above),
            }
        }
        "below" => {
            cur.bump(); // consume "below"
            let save = cur.pos;
            match cur.peek_word().map(|w| w.to_ascii_lowercase()).as_deref() {
                Some("left") => {
                    cur.bump();
                    if cur.eat_word("of") {
                        Some(Direction::BelowLeft)
                    } else {
                        cur.pos = save;
                        Some(Direction::Below)
                    }
                }
                Some("right") => {
                    cur.bump();
                    if cur.eat_word("of") {
                        Some(Direction::BelowRight)
                    } else {
                        cur.pos = save;
                        Some(Direction::Below)
                    }
                }
                _ => Some(Direction::Below),
            }
        }
        "left" => {
            let save = cur.pos;
            cur.bump();
            if cur.eat_word("of") {
                Some(Direction::LeftOf)
            } else {
                cur.pos = save;
                None
            }
        }
        "right" => {
            let save = cur.pos;
            cur.bump();
            if cur.eat_word("of") {
                Some(Direction::RightOf)
            } else {
                cur.pos = save;
                None
            }
        }
        _ => None,
    }
}

fn try_parse_placement(toks: &[Tok]) -> Option<LayoutStatement> {
    let mut cur = Cur { toks, pos: 0 };
    let first = parse_operand(&mut cur)?;
    let mut operands = vec![first];
    let mut directions = vec![eat_direction(&mut cur)?]; // one direction required
    operands.push(parse_operand(&mut cur)?);
    while let Some(d) = eat_direction(&mut cur) {
        directions.push(d);
        operands.push(parse_operand(&mut cur)?);
    }
    if !cur.done() {
        return None;
    }
    Some(LayoutStatement::Placement {
        operands,
        directions,
    })
}

/// Parse one `## Layout` bullet (leading `- ` required). Returns `Err` if the
/// bullet is malformed or has unconsumed trailing tokens.
pub fn parse_layout_line(line: &str) -> Result<LayoutStatement, crate::grammar::LineError> {
    parse_layout_line_opt(line).ok_or_else(|| crate::grammar::LineError {
        range: crate::grammar::bullet_range(line),
        message: "malformed layout statement".to_string(),
    })
}

/// The recursive-descent core (unchanged body of the former `parse_layout_line`).
fn parse_layout_line_opt(line: &str) -> Option<LayoutStatement> {
    let body = line.trim().strip_prefix("- ")?;
    let toks = lex_layout(body)?;
    if toks.is_empty() {
        return None;
    }
    if top_level_seq_index(&toks, &["aligned", "with"]).is_some() {
        return parse_alignment(&toks);
    }
    if let Some(stmt) = try_parse_placement(&toks) {
        return Some(stmt);
    }
    let mut cur = Cur {
        toks: &toks,
        pos: 0,
    };
    let op = parse_operand(&mut cur)?;
    if !cur.done() {
        return None;
    }
    Some(LayoutStatement::Standalone(op))
}

fn parse_operand(cur: &mut Cur) -> Option<Operand> {
    let ref_ = parse_ref(cur)?;
    let mut axis = None;
    if cur.eat_word("as") {
        axis = Some(parse_axis(cur)?);
    }
    let hints = if cur.eat_word("with") {
        parse_hints(cur)?
    } else {
        vec![]
    };
    Some(Operand { ref_, axis, hints })
}

fn parse_axis(cur: &mut Cur) -> Option<Axis> {
    let w = cur.peek_word()?.to_ascii_lowercase();
    match w.as_str() {
        "row" => {
            cur.bump();
            Some(Axis::Row)
        }
        "column" => {
            cur.bump();
            Some(Axis::Column)
        }
        _ => None,
    }
}

fn parse_hints(cur: &mut Cur) -> Option<Vec<Hint>> {
    let mut hints = vec![parse_hint(cur)?];
    loop {
        let is_sep = cur.peek() == Some(&Tok::Comma)
            || cur.peek_word().map(|w| w.eq_ignore_ascii_case("and")) == Some(true);
        if is_sep {
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
        "frame" => {
            cur.bump();
            Some(Hint::Shape(Shape::Frame))
        }
        "box" => {
            cur.bump();
            Some(Hint::Shape(Shape::Box))
        }
        "shrink" => {
            cur.bump();
            Some(Hint::Shape(Shape::Shrink))
        }
        "emphasized" => {
            cur.bump();
            Some(Hint::Flag(Flag::Emphasized))
        }
        "collapsed" => {
            cur.bump();
            Some(Hint::Flag(Flag::Collapsed))
        }
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
                None // margin level must be followed by `margin`/`margins`
            }
        }
        _ => None,
    }
}

fn parse_ref(cur: &mut Cur) -> Option<OperandRef> {
    match cur.peek()? {
        Tok::LParen => {
            cur.bump();
            let inner = parse_operand(cur)?;
            match cur.bump() {
                Some(Tok::RParen) => Some(OperandRef::Paren(Box::new(inner))),
                _ => None,
            }
        }
        Tok::Link { .. } => {
            if let Some(Tok::Link { title, slug }) = cur.bump() {
                Some(OperandRef::Name(NameRef::Link {
                    title: title.clone(),
                    slug: slug.clone(),
                }))
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
                let axis = if lw == "column" {
                    Axis::Column
                } else {
                    Axis::Row
                };
                cur.bump();
                if !cur.eat_word("of") {
                    // `column`/`row` are reserved keywords: a bare occurrence not
                    // followed by `of` is not a valid operand (no backtrack to a
                    // bare-word reading — see render_bare_name, which always quotes them).
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

fn top_level_seq_index(toks: &[Tok], seq: &[&str]) -> Option<usize> {
    let mut depth = 0;
    for (i, tok) in toks.iter().enumerate() {
        match tok {
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
                    if cur.eat_word("of") {
                        Some(e)
                    } else {
                        cur.pos = save;
                        None
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_link_word_paren_comma_and_quote() {
        let toks = lex_layout("column of [Order](./order.md), (Users) as \"my group\"").unwrap();
        assert_eq!(
            toks,
            vec![
                Tok::Word("column".into()),
                Tok::Word("of".into()),
                Tok::Link {
                    title: "Order".into(),
                    slug: "order".into()
                },
                Tok::Comma,
                Tok::LParen,
                Tok::Word("Users".into()),
                Tok::RParen,
                Tok::Word("as".into()),
                Tok::Quoted("my group".into()),
            ]
        );
    }

    #[test]
    fn rejects_unterminated_quote_and_bad_link() {
        assert!(lex_layout("\"oops").is_none());
        assert!(lex_layout("[Order](./order.md").is_none());
    }

    #[test]
    fn parses_standalone_bare_and_link() {
        use crate::syntax::*;
        assert_eq!(
            parse_layout_line("- Orders"),
            Ok(LayoutStatement::Standalone(Operand {
                ref_: OperandRef::Name(NameRef::Bare("Orders".into())),
                axis: None,
                hints: vec![],
            }))
        );
        assert_eq!(
            parse_layout_line("- [Order](./order.md)"),
            Ok(LayoutStatement::Standalone(Operand {
                ref_: OperandRef::Name(NameRef::Link {
                    title: "Order".into(),
                    slug: "order".into()
                }),
                axis: None,
                hints: vec![],
            }))
        );
    }

    #[test]
    fn malformed_layout_line_is_an_err_with_range() {
        let e = parse_layout_line("- Users nonsense Orders").unwrap_err();
        assert!(e.range.0 < e.range.1);
        assert!(e.message.contains("layout"));
    }

    #[test]
    fn rejects_line_without_bullet_and_trailing_garbage() {
        assert!(parse_layout_line("Orders").is_err()); // no "- " bullet
        assert!(parse_layout_line("- Orders Extra").is_err()); // two bare words, no relation
    }

    #[test]
    fn parses_as_axis_and_with_hints() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- Users as column with frame and large margin").unwrap();
        let LayoutStatement::Standalone(op) = stmt else {
            panic!("expected standalone")
        };
        assert_eq!(op.axis, Some(Axis::Column));
        assert_eq!(
            op.hints,
            vec![Hint::Shape(Shape::Frame), Hint::Margin(Margin::Large),]
        );
    }

    #[test]
    fn parses_all_hint_kinds_and_margins_word() {
        use crate::syntax::*;
        let stmt =
            parse_layout_line("- Order with box, no margins, emphasized, collapsed").unwrap();
        let LayoutStatement::Standalone(op) = stmt else {
            panic!("expected standalone")
        };
        assert_eq!(
            op.hints,
            vec![
                Hint::Shape(Shape::Box),
                Hint::Margin(Margin::No),
                Hint::Flag(Flag::Emphasized),
                Hint::Flag(Flag::Collapsed),
            ]
        );
    }

    #[test]
    fn rejects_margin_level_without_margin_keyword() {
        assert!(parse_layout_line("- Order with large").is_err());
    }

    #[test]
    fn parses_quoted_standalone_operand() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- \"My Group\"").unwrap();
        let LayoutStatement::Standalone(op) = stmt else {
            panic!("expected standalone")
        };
        assert_eq!(op.ref_, OperandRef::Name(NameRef::Bare("My Group".into())));
        assert_eq!(op.axis, None);
        assert_eq!(op.hints, vec![]);
    }

    #[test]
    fn rejects_dangling_as_without_axis() {
        assert!(parse_layout_line("- Users as").is_err());
        assert!(parse_layout_line("- Users as with frame").is_err());
    }

    #[test]
    fn parses_inline_group_and_greedy_with_binding() {
        use crate::syntax::*;
        // `with` binds to Account (nearest operand), NOT the whole column.
        let stmt = parse_layout_line("- column of Customer, Account with large margin").unwrap();
        let LayoutStatement::Standalone(op) = stmt else {
            panic!()
        };
        let OperandRef::InlineGroup { axis, items } = op.ref_ else {
            panic!("expected inline group")
        };
        assert_eq!(axis, Axis::Column);
        assert_eq!(items.len(), 2);
        assert!(op.hints.is_empty(), "outer operand carries no hints");
        assert_eq!(items[1].hints, vec![Hint::Margin(Margin::Large)]);
    }

    #[test]
    fn parens_rebind_with_to_the_whole_group() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- (column of Customer, Account) with large margin").unwrap();
        let LayoutStatement::Standalone(op) = stmt else {
            panic!()
        };
        assert_eq!(op.hints, vec![Hint::Margin(Margin::Large)]);
        let OperandRef::Paren(inner) = op.ref_ else {
            panic!("expected paren")
        };
        assert!(inner.hints.is_empty());
        assert!(matches!(inner.ref_, OperandRef::InlineGroup { .. }));
    }

    #[test]
    fn parses_nested_inline_groups() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- row of (column of Customer, Account), Orders").unwrap();
        let LayoutStatement::Standalone(op) = stmt else {
            panic!()
        };
        let OperandRef::InlineGroup { axis, items } = op.ref_ else {
            panic!()
        };
        assert_eq!(axis, Axis::Row);
        assert_eq!(items.len(), 2);
        assert!(matches!(items[0].ref_, OperandRef::Paren(_)));
    }

    #[test]
    fn parses_single_and_chained_placement() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- Users left of Orders").unwrap();
        let LayoutStatement::Placement {
            operands,
            directions,
        } = stmt
        else {
            panic!("expected placement")
        };
        assert_eq!(operands.len(), 2);
        assert_eq!(directions, vec![Direction::LeftOf]);

        let stmt = parse_layout_line("- Order above OrderLine above Payment").unwrap();
        let LayoutStatement::Placement {
            operands,
            directions,
        } = stmt
        else {
            panic!()
        };
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
            let LayoutStatement::Placement { directions, .. } = parse_layout_line(text).unwrap()
            else {
                panic!()
            };
            assert_eq!(directions, vec![dir]);
        }
    }

    #[test]
    fn diagonals_parse_and_serialize_round_trip() {
        use crate::syntax::{Direction, LayoutStatement};
        for (text, dir) in [
            ("- A above left of B", Direction::AboveLeft),
            ("- A above right of B", Direction::AboveRight),
            ("- A below left of B", Direction::BelowLeft),
            ("- A below right of B", Direction::BelowRight),
        ] {
            let stmt = parse_layout_line(text).unwrap();
            let LayoutStatement::Placement {
                operands,
                directions,
            } = &stmt
            else {
                panic!("expected a placement for {text}");
            };
            assert_eq!(operands.len(), 2, "{text}");
            assert_eq!(directions.as_slice(), &[dir], "{text}");
            let rendered = render_layout_line(&stmt);
            let phrase = match dir {
                Direction::AboveLeft => "above left of",
                Direction::AboveRight => "above right of",
                Direction::BelowLeft => "below left of",
                Direction::BelowRight => "below right of",
                _ => unreachable!(),
            };
            assert!(
                rendered.contains(phrase),
                "serialize lost the diagonal: {rendered}"
            );
        }
    }

    #[test]
    fn bare_above_below_still_parse_as_cardinals() {
        use crate::syntax::{Direction, LayoutStatement};
        for (text, dir) in [
            ("- A above B", Direction::Above),
            ("- A below B", Direction::Below),
        ] {
            let LayoutStatement::Placement { directions, .. } = parse_layout_line(text).unwrap()
            else {
                panic!()
            };
            assert_eq!(directions.as_slice(), &[dir], "{text}");
        }
    }

    #[test]
    fn placement_binds_with_to_operand_then_direction() {
        use crate::syntax::*;
        // greedy `with` on the first operand, then a trailing relation
        let stmt = parse_layout_line("- Users with frame left of Orders").unwrap();
        let LayoutStatement::Placement {
            operands,
            directions,
        } = stmt
        else {
            panic!()
        };
        assert_eq!(directions, vec![Direction::LeftOf]);
        assert_eq!(operands[0].hints, vec![Hint::Shape(Shape::Frame)]);
    }

    #[test]
    fn parses_anchored_alignment() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- top of VIP aligned with top of Orders").unwrap();
        let LayoutStatement::Alignment { left, right } = stmt else {
            panic!("expected alignment")
        };
        assert_eq!(left.edge, Some(Edge::Top));
        assert_eq!(right.edge, Some(Edge::Top));
        assert_eq!(
            left.operand.ref_,
            OperandRef::Name(NameRef::Bare("VIP".into()))
        );
    }

    #[test]
    fn parses_bare_center_to_center_alignment() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- X aligned with Y").unwrap();
        let LayoutStatement::Alignment { left, right } = stmt else {
            panic!()
        };
        assert_eq!(left.edge, None);
        assert_eq!(right.edge, None);
    }

    #[test]
    fn edge_left_is_not_read_as_placement_direction() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- left of X aligned with right of Y").unwrap();
        let LayoutStatement::Alignment { left, right } = stmt else {
            panic!("expected alignment, not placement")
        };
        assert_eq!(left.edge, Some(Edge::Left));
        assert_eq!(right.edge, Some(Edge::Right));
    }

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
            let parsed =
                parse_layout_line(line).unwrap_or_else(|_| panic!("failed to parse: {line}"));
            let rendered = render_layout_line(&parsed);
            let reparsed = parse_layout_line(&rendered)
                .unwrap_or_else(|_| panic!("failed to reparse: {rendered}"));
            assert_eq!(parsed, reparsed, "not a fixpoint: {line} -> {rendered}");
        }
    }

    #[test]
    fn reserved_keyword_bare_name_round_trips_quoted() {
        // A quoted operand whose name equals a reserved axis keyword must
        // render back in quoted form so it re-parses to the same AST.
        for line in ["- \"column\"", "- \"row\""] {
            let parsed = parse_layout_line(line).unwrap();
            let rendered = render_layout_line(&parsed);
            let reparsed = parse_layout_line(&rendered)
                .unwrap_or_else(|_| panic!("reparse failed: {rendered}"));
            assert_eq!(parsed, reparsed, "not a fixpoint: {line} -> {rendered}");
        }
    }

    #[test]
    fn hint_joiner_normalizes_to_comma() {
        let parsed = parse_layout_line("- Users with frame and large margin").unwrap();
        assert_eq!(
            render_layout_line(&parsed),
            "- Users with frame, large margin"
        );
    }
}
