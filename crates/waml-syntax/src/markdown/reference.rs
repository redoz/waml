use std::{collections::HashMap, sync::Arc};

use crate::{SyntaxIdentity, TextRange};

#[derive(Clone, Debug)]
pub struct MarkdownReferenceDefinition {
    pub label: Arc<str>,
    /// The block node the definition was written in, for a
    /// jump-to-definition that has not landed.
    #[allow(dead_code)] // consumer: link-reference resolution
    pub owner: SyntaxIdentity,
    pub source_range: TextRange,
    pub destination: Arc<str>,
    pub destination_range: TextRange,
    pub title: Option<Arc<str>>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MarkdownReferenceMap {
    pub(crate) definitions: Arc<HashMap<Arc<str>, MarkdownReferenceDefinition>>,
    pub(crate) backlinks: Arc<HashMap<Arc<str>, Arc<[SyntaxIdentity]>>>,
}

impl MarkdownReferenceMap {
    pub(crate) fn from_tree(
        source: &str,
        root: &crate::GreenNode<crate::OkfMarkdownLanguage>,
        start: usize,
    ) -> Result<Self, crate::ParseError> {
        let mut definitions = HashMap::new();
        let mut at = start;
        collect_definitions(root, source, &mut at, &mut definitions)?;
        Ok(Self {
            definitions: Arc::new(definitions),
            backlinks: Arc::new(HashMap::new()),
        })
    }

    /// The label and span of *every* definition the tree holds, repeats
    /// included, in source order.
    ///
    /// Resolution keeps only the first definition of each label, which is what
    /// CommonMark resolves against. A guard asking whether an edit could
    /// change *which* definition that is has to see the ones resolution drops:
    /// unmaking the first definition of a label promotes the next, and a guard
    /// that only knows the first cannot tell that there is a next.
    ///
    /// Reads the label straight off the node rather than through
    /// [`parse_definition`], so a node whose destination the parse rejects
    /// still names its label. A guard has to over-report, and a definition
    /// that resolves to nothing still occupies the place a later edit could
    /// make resolve.
    pub(crate) fn definition_sites(
        source: &str,
        root: &crate::GreenNode<crate::OkfMarkdownLanguage>,
        start: usize,
    ) -> Result<Vec<(Arc<str>, TextRange)>, crate::ParseError> {
        let mut sites = Vec::new();
        let mut at = start;
        collect_sites(root, source, &mut at, &mut sites)?;
        Ok(sites)
    }

    pub(crate) fn with_backlinks(
        mut self,
        backlinks: HashMap<Arc<str>, Vec<SyntaxIdentity>>,
    ) -> Self {
        self.backlinks = Arc::new(
            backlinks
                .into_iter()
                .map(|(label, owners)| (label, owners.into()))
                .collect(),
        );
        self
    }
}

/// Whether any label in `source` is defined twice, by the loosest reading of
/// "defined".
///
/// A definition's label is the text between a `[` and the `]` that a `:`
/// follows, and CommonMark forbids an unescaped `[` inside a label, so the
/// nearest `[` before that `]` is the one that opens it. Nothing else is
/// checked: a container prefix, an indent, or a whole enclosing code fence all
/// leave the label where this finds it. Naming a bracket pair that is no
/// definition at all costs the caller one wasted rescan; missing a real repeat
/// would leave that repeat without a span.
///
/// One forward pass, carrying the most recent `[` rather than searching back
/// from every `]:` — a document of nothing but `]:` would otherwise cost a
/// backward scan per pair.
pub(crate) fn repeats_a_definition_label(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut seen = std::collections::HashSet::new();
    let mut open: Option<usize> = None;
    for at in 0..bytes.len() {
        match bytes[at] {
            b'[' => open = Some(at),
            // `[`, `]` and `:` are ASCII, so these are all char boundaries.
            b']' if bytes.get(at + 1) == Some(&b':') => {
                if let Some(label) = open.and_then(|open| normalize_label(&source[open + 1..at])) {
                    if !seen.insert(label) {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

pub(crate) fn normalize_label(label: &str) -> Option<Arc<str>> {
    let normalized = label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
        .replace('ß', "ss");
    (!normalized.is_empty() && normalized.chars().count() <= 999).then(|| normalized.into())
}

fn range(start: usize, end: usize) -> Result<TextRange, crate::ParseError> {
    let start = crate::TextSize::try_from_usize(start)
        .map_err(|_| crate::ParseError::SourceTooLarge { bytes: start })?;
    let end = crate::TextSize::try_from_usize(end)
        .map_err(|_| crate::ParseError::SourceTooLarge { bytes: end })?;
    TextRange::new(start, end).map_err(|_| crate::ParseError::StructuralInvariant {
        reason: "reversed reference range".into(),
    })
}

fn collect_definitions(
    node: &crate::GreenNode<crate::OkfMarkdownLanguage>,
    source: &str,
    at: &mut usize,
    definitions: &mut HashMap<Arc<str>, MarkdownReferenceDefinition>,
) -> Result<(), crate::ParseError> {
    let start = *at;
    let end = start + node.width().to_usize();
    if node.kind() == crate::OkfMarkdownSyntaxKind::LinkReferenceDefinition {
        if let Some(definition) = parse_definition(source, start, end, identity(node)?)? {
            definitions
                .entry(definition.label.clone())
                .or_insert(definition);
        }
        *at = end;
        return Ok(());
    }
    for child in node.children() {
        match child {
            crate::GreenElement::Node(child) => {
                collect_definitions(child, source, at, definitions)?;
            }
            crate::GreenElement::Token(token) => *at += token.width().to_usize(),
        }
    }
    Ok(())
}

fn collect_sites(
    node: &crate::GreenNode<crate::OkfMarkdownLanguage>,
    source: &str,
    at: &mut usize,
    sites: &mut Vec<(Arc<str>, TextRange)>,
) -> Result<(), crate::ParseError> {
    let start = *at;
    let end = start + node.width().to_usize();
    if node.kind() == crate::OkfMarkdownSyntaxKind::LinkReferenceDefinition {
        if let Some(label) = definition_label(&source[start..end]) {
            sites.push((label, range(start, end)?));
        }
        *at = end;
        return Ok(());
    }
    for child in node.children() {
        match child {
            crate::GreenElement::Node(child) => collect_sites(child, source, at, sites)?,
            crate::GreenElement::Token(token) => *at += token.width().to_usize(),
        }
    }
    Ok(())
}

/// The normalised label a definition's spelling opens with.
fn definition_label(spelling: &str) -> Option<Arc<str>> {
    let spelling = spelling.trim_end_matches(['\r', '\n']);
    let close = spelling.find("]:")?;
    spelling.strip_prefix('[')?;
    normalize_label(&spelling[1..close])
}

fn identity(
    node: &crate::GreenNode<crate::OkfMarkdownLanguage>,
) -> Result<SyntaxIdentity, crate::ParseError> {
    node.annotations()
        .iter()
        .find(|annotation| annotation.kind() == "waml.markdown.identity")
        .and_then(|annotation| annotation.data())
        .and_then(SyntaxIdentity::from_annotation_data)
        .ok_or_else(|| crate::ParseError::StructuralInvariant {
            reason: "reference definition has no syntax identity".into(),
        })
}

fn parse_definition(
    source: &str,
    start: usize,
    end: usize,
    owner: SyntaxIdentity,
) -> Result<Option<MarkdownReferenceDefinition>, crate::ParseError> {
    let spelling = source[start..end].trim_end_matches(['\r', '\n']);
    let Some(close) = spelling.find("]:") else {
        return Ok(None);
    };
    let Some(label) = definition_label(&source[start..end]) else {
        return Ok(None);
    };
    let after_colon = close + 2;
    let value = spelling[after_colon..].trim_start();
    let value_start = after_colon + spelling[after_colon..].len() - value.len();
    let (destination, destination_start, rest, angle_destination) =
        if let Some(value) = value.strip_prefix('<') {
            let Some(close) = value.find('>') else {
                return Ok(None);
            };
            (&value[..close], value_start + 1, &value[close + 1..], true)
        } else {
            let destination_end = value.find(char::is_whitespace).unwrap_or(value.len());
            (
                &value[..destination_end],
                value_start,
                &value[destination_end..],
                false,
            )
        };
    if destination.is_empty() && !angle_destination {
        return Ok(None);
    }
    let title = parse_title(rest);
    let destination_start = start + destination_start;
    Ok(Some(MarkdownReferenceDefinition {
        label,
        owner,
        source_range: range(start, end)?,
        destination: decode_destination(destination).into(),
        destination_range: range(destination_start, destination_start + destination.len())?,
        title: title.map(decode_destination).map(Arc::from),
    }))
}

fn parse_title(rest: &str) -> Option<&str> {
    let rest = rest.trim();
    let (open, close) = match rest.as_bytes().first().copied()? {
        b'"' => ('"', '"'),
        b'\'' => ('\'', '\''),
        b'(' => ('(', ')'),
        _ => return None,
    };
    rest.strip_prefix(open)?.strip_suffix(close)
}

pub(crate) fn decode_destination(value: &str) -> String {
    let mut out = String::new();
    let mut at = 0;
    while at < value.len() {
        let ch = value[at..].chars().next().expect("valid UTF-8 offset");
        if ch == '\\' {
            let next = value[at + 1..].chars().next();
            if next.is_some_and(|next| next.is_ascii_punctuation()) {
                let next = next.expect("checked");
                out.push(next);
                at += 1 + next.len_utf8();
                continue;
            }
        } else if ch == '&' {
            if let Some(relative_end) = value[at..].find(';') {
                let end = at + relative_end + 1;
                if let Some(decoded) = super::inline::decode_entity(&value[at..end]) {
                    out.push_str(&decoded);
                    at = end;
                    continue;
                }
            }
        }
        out.push(ch);
        at += ch.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_definition_scan_names_a_label_defined_twice() {
        // Gates the rescan that hands a repeated definition the same span
        // treatment as the first of its label.
        assert!(repeats_a_definition_label("[id]: /a\n\n[id]: /b\n"));
        // Case and inner whitespace fold, exactly as the parser folds them.
        assert!(repeats_a_definition_label("[ID]: /a\n\n[id]: /b\n"));
        assert!(repeats_a_definition_label("[i\nd]: /a\n\n[i d]: /b\n"));
        // A label spelled across a line break repeats one spelled on a line.
        assert!(repeats_a_definition_label("[\nid]: /a\n\n[id]: /b\n"));
        // Distinct labels are no repeat, however many there are.
        assert!(!repeats_a_definition_label("[a]: /a\n\n[b]: /b\n"));
        // A `]:` with no `[` in front of it names nothing.
        assert!(!repeats_a_definition_label("x]: y\n\nz]: w\n"));
    }
}
