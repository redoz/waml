//! Tokenizer, recursive-descent parser, and renderer for the `## Layout`
//! diagram-arrangement language. See docs/uaml-spec.md (Diagram documents).

use std::sync::LazyLock;
use regex::Regex;
use crate::syntax::*;

/// Render one `## Layout` statement back to its `- …` bullet text.
pub fn render_layout_line(_stmt: &LayoutStatement) -> String {
    String::new() // implemented in Task 8
}

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
                None // margin level must be followed by `margin`/`margins`
            }
        }
        _ => None,
    }
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

    #[test]
    fn parses_quoted_standalone_operand() {
        use crate::syntax::*;
        let stmt = parse_layout_line("- \"My Group\"").unwrap();
        let LayoutStatement::Standalone(op) = stmt else { panic!("expected standalone") };
        assert_eq!(op.ref_, OperandRef::Name(NameRef::Bare("My Group".into())));
        assert_eq!(op.axis, None);
        assert_eq!(op.hints, vec![]);
    }

    #[test]
    fn rejects_dangling_as_without_axis() {
        assert!(parse_layout_line("- Users as").is_none());
        assert!(parse_layout_line("- Users as with frame").is_none());
    }
}
