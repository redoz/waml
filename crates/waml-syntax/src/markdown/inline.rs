use std::sync::Arc;

use crate::{
    GreenElement, GreenFactory, GreenNode, GreenText, OkfMarkdownLanguage,
    OkfMarkdownSyntaxKind as Kind, ParseError, SourceText, SyntaxIdentity, TextRange, TextSize,
};

use super::reference::MarkdownReferenceMap;

pub(crate) fn apply(
    text: &SourceText,
    root: &GreenNode<OkfMarkdownLanguage>,
    _references: &MarkdownReferenceMap,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    let mut at = 0;
    rebuild(text, root, &mut at)
}

fn rebuild(
    text: &SourceText,
    node: &GreenNode<OkfMarkdownLanguage>,
    at: &mut usize,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    let start = *at;
    let end = start + node.width().to_usize();
    if matches!(node.kind(), Kind::Paragraph) {
        *at = end;
        return inline_node(text, start, end);
    }
    let mut children = Vec::new();
    for child in node.children() {
        match child {
            GreenElement::Node(child) => {
                children.push(GreenElement::Node(rebuild(text, child, at)?))
            }
            GreenElement::Token(token) => {
                *at += token.width().to_usize();
                children.push(GreenElement::Token(token.clone()));
            }
        }
    }
    GreenFactory::new()
        .node_with_annotations(node.kind(), children, node.annotations().into())
        .map_err(|_| ParseError::WidthOverflow)
}

fn inline_node(
    text: &SourceText,
    start: usize,
    end: usize,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    semantic(Kind::Paragraph, parse_inlines(text, start, end)?)
}

fn parse_inlines(
    text: &SourceText,
    start: usize,
    end: usize,
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, ParseError> {
    let source = text.shared();
    let mut out = Vec::new();
    let mut at = start;
    let mut plain = start;
    while at < end {
        let rest = &source[at..end];
        if rest.starts_with('\\') && at + 1 < end {
            flush(text, plain, at, &mut out)?;
            out.push(node(
                Kind::Escape,
                vec![
                    tok(text, at, at + 1, Kind::BackslashToken)?,
                    tok(text, at + 1, at + 2, Kind::TextToken)?,
                ],
            )?);
            at += 2;
            plain = at;
            continue;
        }
        if rest.starts_with('&') {
            if let Some(close) = rest.find(';').filter(|n| *n > 1) {
                let close = at + close + 1;
                flush(text, plain, at, &mut out)?;
                out.push(node(
                    Kind::Entity,
                    vec![tok(text, at, close, Kind::EntityToken)?],
                )?);
                at = close;
                plain = at;
                continue;
            }
        }
        if rest.starts_with('`') {
            let run = rest.bytes().take_while(|b| *b == b'`').count();
            let needle = "`".repeat(run);
            if let Some(relative) = source[at + run..end].find(&needle) {
                let close = at + run + relative + run;
                flush(text, plain, at, &mut out)?;
                out.push(node(
                    Kind::CodeSpan,
                    vec![
                        tok(text, at, at + run, Kind::CodeDelimiterToken)?,
                        tok(text, at + run, close - run, Kind::TextToken)?,
                        tok(text, close - run, close, Kind::CodeDelimiterToken)?,
                    ],
                )?);
                at = close;
                plain = at;
                continue;
            }
        }
        if rest.starts_with('<') {
            if let Some(relative) = rest.find('>') {
                let close = at + relative + 1;
                let inside = &source[at + 1..close - 1];
                flush(text, plain, at, &mut out)?;
                let kind = if inside.contains("://") || inside.contains('@') {
                    Kind::Autolink
                } else {
                    Kind::RawHtml
                };
                out.push(node(
                    kind,
                    vec![
                        tok(text, at, at + 1, Kind::AutolinkOpenToken)?,
                        tok(
                            text,
                            at + 1,
                            close - 1,
                            if kind == Kind::RawHtml {
                                Kind::HtmlToken
                            } else {
                                Kind::TextToken
                            },
                        )?,
                        tok(text, close - 1, close, Kind::AutolinkCloseToken)?,
                    ],
                )?);
                at = close;
                plain = at;
                continue;
            }
        }
        let image = rest.starts_with("![");
        if image || rest.starts_with('[') {
            let open = at + usize::from(image);
            if let Some(label_end_rel) = source[open + 1..end].find(']') {
                let label_end = open + 1 + label_end_rel;
                let mut close = label_end + 1;
                let mut children = Vec::new();
                if image {
                    children.push(tok(text, at, at + 1, Kind::ImageBangToken)?);
                }
                children.push(tok(text, open, open + 1, Kind::LinkLabelOpenToken)?);
                children.extend(parse_inlines(text, open + 1, label_end)?);
                children.push(tok(
                    text,
                    label_end,
                    label_end + 1,
                    Kind::LinkLabelCloseToken,
                )?);
                if source[close..end].starts_with('(') {
                    let destination_open = close;
                    let Some(rel) = source[close + 1..end].find(')') else {
                        at += 1;
                        continue;
                    };
                    close += rel + 2;
                    children.push(tok(
                        text,
                        destination_open,
                        destination_open + 1,
                        Kind::LinkDestinationOpenToken,
                    )?);
                    children.push(tok(
                        text,
                        destination_open + 1,
                        close - 1,
                        Kind::LinkDestinationToken,
                    )?);
                    children.push(tok(
                        text,
                        close - 1,
                        close,
                        Kind::LinkDestinationCloseToken,
                    )?);
                } else if source[close..end].starts_with('[') {
                    let Some(rel) = source[close + 1..end].find(']') else {
                        at += 1;
                        continue;
                    };
                    let ref_end = close + rel + 2;
                    children.push(tok(text, close, close + 1, Kind::LinkLabelOpenToken)?);
                    if close + 1 < ref_end - 1 {
                        children.push(tok(text, close + 1, ref_end - 1, Kind::TextToken)?);
                    }
                    children.push(tok(text, ref_end - 1, ref_end, Kind::LinkLabelCloseToken)?);
                    close = ref_end;
                }
                flush(text, plain, at, &mut out)?;
                out.push(node(
                    if image { Kind::Image } else { Kind::Link },
                    children,
                )?);
                at = close;
                plain = at;
                continue;
            }
        }
        if matches!(rest.as_bytes()[0], b'*' | b'_') {
            let marker = rest.as_bytes()[0];
            let count = rest.bytes().take_while(|b| *b == marker).count().min(2);
            let delimiter = &source[at..at + count];
            if let Some(relative) = source[at + count..end].find(delimiter) {
                let close = at + count + relative + count;
                flush(text, plain, at, &mut out)?;
                out.push(node(
                    if count == 2 {
                        Kind::StrongEmphasis
                    } else {
                        Kind::Emphasis
                    },
                    vec![
                        tok(text, at, at + count, Kind::EmphasisDelimiterToken)?,
                        node(Kind::Text, parse_inlines(text, at + count, close - count)?)?,
                        tok(text, close - count, close, Kind::EmphasisDelimiterToken)?,
                    ],
                )?);
                at = close;
                plain = at;
                continue;
            }
        }
        if rest.starts_with("  \\n") || rest.starts_with("\\\\\n") {
            let width = if rest.starts_with("  \\n") { 3 } else { 2 };
            flush(text, plain, at, &mut out)?;
            out.push(node(
                Kind::HardLineBreak,
                vec![tok(text, at, at + width, Kind::NewlineToken)?],
            )?);
            at += width;
            plain = at;
            continue;
        }
        if rest.starts_with('\n') {
            flush(text, plain, at, &mut out)?;
            out.push(node(
                Kind::SoftLineBreak,
                vec![tok(text, at, at + 1, Kind::NewlineToken)?],
            )?);
            at += 1;
            plain = at;
            continue;
        }
        at += source[at..].chars().next().unwrap().len_utf8();
    }
    flush(text, plain, end, &mut out)?;
    Ok(out)
}

fn flush(
    text: &SourceText,
    start: usize,
    end: usize,
    out: &mut Vec<GreenElement<OkfMarkdownLanguage>>,
) -> Result<(), ParseError> {
    if start >= end {
        return Ok(());
    }
    let source = text.shared();
    let mut at = start;
    while at < end {
        let whitespace = source[at..].chars().next().unwrap().is_whitespace();
        let mut next = at;
        while next < end && source[next..].chars().next().unwrap().is_whitespace() == whitespace {
            next += source[next..].chars().next().unwrap().len_utf8();
        }
        out.push(node(
            Kind::Text,
            vec![tok(
                text,
                at,
                next,
                if whitespace {
                    Kind::WhitespaceToken
                } else {
                    Kind::TextToken
                },
            )?],
        )?);
        at = next;
    }
    Ok(())
}

fn node(
    kind: Kind,
    children: Vec<GreenElement<OkfMarkdownLanguage>>,
) -> Result<GreenElement<OkfMarkdownLanguage>, ParseError> {
    Ok(GreenElement::Node(semantic(kind, children)?))
}
fn semantic(
    kind: Kind,
    children: Vec<GreenElement<OkfMarkdownLanguage>>,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    GreenFactory::new()
        .node_with_annotations(
            kind,
            children,
            Arc::from([SyntaxIdentity::fresh()?.annotation()]),
        )
        .map_err(|_| ParseError::WidthOverflow)
}
fn tok(
    text: &SourceText,
    start: usize,
    end: usize,
    kind: Kind,
) -> Result<GreenElement<OkfMarkdownLanguage>, ParseError> {
    let range =
        TextRange::new(size(start)?, size(end)?).map_err(|_| ParseError::StructuralInvariant {
            reason: "reversed inline range".into(),
        })?;
    Ok(GreenElement::Token(
        GreenFactory::new()
            .token(
                kind,
                GreenText::SourceSlice {
                    source: text.clone(),
                    range,
                },
                [],
                [],
            )
            .map_err(|_| ParseError::WidthOverflow)?,
    ))
}
fn size(value: usize) -> Result<TextSize, ParseError> {
    TextSize::try_from_usize(value).map_err(|_| ParseError::SourceTooLarge { bytes: value })
}
