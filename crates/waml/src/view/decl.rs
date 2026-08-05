//! Parse of the frontmatter `view:` declaration. Deliberately dumb: it
//! captures what the author wrote and where, and does not validate
//! middleware names — an unknown name is a declaration-level chain-build
//! failure, not a parse-time silent skip and not a parse error.
//!
//! `markdown` and `member:<href>` are surface-resolution entries, not
//! middleware (spec: "The chain resolves surfaces too"); this layer records
//! them as ordinary entries and a later layer interprets them.

use crate::frontmatter::FmValue;

/// One entry of a `view:` declaration, verbatim, with the source line for
/// spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewEntry {
    pub raw: String,
    pub line: usize,
}

/// A parsed `view:` declaration. A scalar is a one-element chain. First
/// entry is outermost.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewDecl {
    pub entries: Vec<ViewEntry>,
}

/// Parse the frontmatter value. Accepts a plain scalar or a flow/blocked
/// sequence of scalars. Returns `None` for a shape that is neither (e.g. a
/// nested mapping) — the caller keeps the key in `extra` so a re-render
/// never erases what the author wrote.
pub fn parse_view_decl(value: &FmValue, line: usize) -> Option<ViewDecl> {
    match value {
        FmValue::Str(s) => Some(ViewDecl {
            entries: vec![ViewEntry {
                raw: s.clone(),
                line,
            }],
        }),
        FmValue::List(items) => {
            let mut entries = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    FmValue::Str(s) => entries.push(ViewEntry {
                        raw: s.clone(),
                        line,
                    }),
                    _ => return None,
                }
            }
            Some(ViewDecl { entries })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_view_is_a_one_element_chain() {
        let decl = parse_view_decl(&FmValue::Str("outline".to_string()), 3).unwrap();
        assert_eq!(decl.entries.len(), 1);
        assert_eq!(decl.entries[0].raw, "outline");
        assert_eq!(decl.entries[0].line, 3);
    }

    #[test]
    fn sequence_view_preserves_order_first_is_outermost() {
        let value = FmValue::List(vec![
            FmValue::Str("hide-refs".to_string()),
            FmValue::Str("group-by-tag".to_string()),
        ]);
        let decl = parse_view_decl(&value, 5).unwrap();
        let raws: Vec<&str> = decl.entries.iter().map(|e| e.raw.as_str()).collect();
        assert_eq!(raws, vec!["hide-refs", "group-by-tag"]);
    }

    #[test]
    fn member_and_markdown_parse_as_ordinary_entries() {
        let decl = parse_view_decl(&FmValue::Str("member:./orders".to_string()), 1).unwrap();
        assert_eq!(decl.entries[0].raw, "member:./orders");

        let decl = parse_view_decl(&FmValue::Str("markdown".to_string()), 1).unwrap();
        assert_eq!(decl.entries[0].raw, "markdown");
    }

    #[test]
    fn a_mapping_value_is_rejected_not_erased() {
        let value = FmValue::Map(vec![("foo".to_string(), FmValue::Str("bar".to_string()))]);
        assert_eq!(parse_view_decl(&value, 1), None);
    }

    #[test]
    fn empty_sequence_parses_to_an_empty_decl() {
        let decl = parse_view_decl(&FmValue::List(vec![]), 1).unwrap();
        assert!(decl.entries.is_empty());
    }

    #[test]
    fn a_non_scalar_item_in_a_sequence_is_rejected() {
        let value = FmValue::List(vec![FmValue::Str("hide".to_string()), FmValue::Map(vec![])]);
        assert_eq!(parse_view_decl(&value, 1), None);
    }
}
