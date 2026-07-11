//! Tokenizer, recursive-descent parser, and renderer for the `## Layout`
//! diagram-arrangement language. See docs/uaml-spec.md (Diagram documents).

use std::sync::LazyLock;
use regex::Regex;
use crate::syntax::LayoutStatement;

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
