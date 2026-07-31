use std::{collections::HashMap, num::NonZeroU64, sync::Arc};

use pulldown_cmark::{Event, Parser};

use crate::{
    GreenElement, GreenFactory, GreenNode, GreenText, OkfMarkdownLanguage,
    OkfMarkdownSyntaxKind as Kind, ParseError, SourceText, SyntaxAnnotation, SyntaxIdentity,
    TextRange, TextSize,
};

use super::reference::{decode_destination, normalize_label, MarkdownReferenceMap};

const DESTINATION_ANNOTATION: &str = "waml.markdown.link.destination";
const DESTINATION_RANGE_ANNOTATION: &str = "waml.markdown.link.destination_range";
const TITLE_ANNOTATION: &str = "waml.markdown.link.title";
const LINK_KIND_ANNOTATION: &str = "waml.markdown.link.kind";
const REFERENCE_LABEL_ANNOTATION: &str = "waml.markdown.link.reference_label";
const OWNER_ANNOTATION: &str = "waml.markdown.link.owner";
const ENTITY_VALUE_ANNOTATION: &str = "waml.markdown.entity.value";

const DESTINATION_ID: u64 = u64::MAX - 1;
const DESTINATION_RANGE_ID: u64 = u64::MAX - 2;
const TITLE_ID: u64 = u64::MAX - 3;
const LINK_KIND_ID: u64 = u64::MAX - 4;
const REFERENCE_LABEL_ID: u64 = u64::MAX - 5;
const OWNER_ID: u64 = u64::MAX - 6;
const ENTITY_VALUE_ID: u64 = u64::MAX - 7;

pub(crate) struct InlineParse {
    pub root: GreenNode<OkfMarkdownLanguage>,
    pub inline_roots: Arc<[GreenNode<OkfMarkdownLanguage>]>,
    pub references: MarkdownReferenceMap,
}

pub(crate) fn apply(
    text: &SourceText,
    root: &GreenNode<OkfMarkdownLanguage>,
    references: MarkdownReferenceMap,
    start: usize,
) -> Result<InlineParse, ParseError> {
    let (root, inline_roots, backlinks) = {
        let mut context = InlineContext {
            text,
            references: &references,
            inline_roots: Vec::new(),
            backlinks: HashMap::new(),
        };
        let mut at = start;
        let root = rebuild(&mut context, root, &mut at)?;
        (root, context.inline_roots, context.backlinks)
    };
    let references = references.with_backlinks(backlinks);
    Ok(InlineParse {
        root,
        inline_roots: inline_roots.into(),
        references,
    })
}

struct InlineContext<'a> {
    text: &'a SourceText,
    references: &'a MarkdownReferenceMap,
    inline_roots: Vec<GreenNode<OkfMarkdownLanguage>>,
    backlinks: HashMap<Arc<str>, Vec<SyntaxIdentity>>,
}

fn rebuild(
    context: &mut InlineContext<'_>,
    node: &GreenNode<OkfMarkdownLanguage>,
    at: &mut usize,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    let start = *at;
    let end = start + node.width().to_usize();
    if node.kind() == Kind::Paragraph {
        *at = end;
        let owner = SyntaxIdentity::fresh()?;
        let children = parse_inlines(context, start, end, owner, true)?;
        let rebuilt = semantic_with_identity(Kind::Paragraph, children, owner, Vec::new())?;
        context.inline_roots.push(rebuilt.clone());
        return Ok(rebuilt);
    }
    if matches!(
        node.kind(),
        Kind::AtxHeading | Kind::SetextHeading | Kind::TableCell
    ) && has_inline_syntax(&context.text.shared()[start..end])
    {
        let owner = SyntaxIdentity::fresh()?;
        let mut children = Vec::new();
        for child in node.children() {
            match child {
                GreenElement::Token(token) if token.kind() == Kind::TextToken => {
                    let token_start = *at;
                    let token_end = token_start + token.width().to_usize();
                    children.extend(parse_inlines(context, token_start, token_end, owner, true)?);
                    *at = token_end;
                }
                GreenElement::Token(token) => {
                    *at += token.width().to_usize();
                    children.push(GreenElement::Token(token.clone()));
                }
                GreenElement::Node(child) => {
                    children.push(GreenElement::Node(rebuild(context, child, at)?));
                }
            }
        }
        let rebuilt = semantic_with_identity(node.kind(), children, owner, Vec::new())?;
        context.inline_roots.push(rebuilt.clone());
        return Ok(rebuilt);
    }
    let mut children = Vec::new();
    for child in node.children() {
        match child {
            GreenElement::Node(child) => {
                children.push(GreenElement::Node(rebuild(context, child, at)?));
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

fn has_inline_syntax(source: &str) -> bool {
    source
        .bytes()
        .any(|byte| matches!(byte, b'\\' | b'&' | b'`' | b'*' | b'_' | b'[' | b'!' | b'<'))
}

#[derive(Clone, Copy, Debug)]
struct EmphasisPair {
    open: usize,
    close: usize,
    width: usize,
    kind: Kind,
}

#[derive(Clone, Copy, Debug)]
struct Delimiter {
    marker: u8,
    start: usize,
    length: usize,
    remaining: usize,
    can_open: bool,
    can_close: bool,
}

fn parse_inlines(
    context: &mut InlineContext<'_>,
    start: usize,
    end: usize,
    owner: SyntaxIdentity,
    allow_links: bool,
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, ParseError> {
    let source = context.text.shared();
    let emphasis = emphasis_pairs(source, start, end);
    let mut out = Vec::new();
    let mut at = start;
    let mut plain = start;
    while at < end {
        if let Some(pair) = emphasis.iter().find(|pair| pair.open == at) {
            flush(context.text, plain, at, &mut out)?;
            let mut children = vec![tok(
                context.text,
                pair.open,
                pair.open + pair.width,
                Kind::EmphasisDelimiterToken,
            )?];
            children.extend(parse_inlines(
                context,
                pair.open + pair.width,
                pair.close,
                owner,
                allow_links,
            )?);
            children.push(tok(
                context.text,
                pair.close,
                pair.close + pair.width,
                Kind::EmphasisDelimiterToken,
            )?);
            out.push(node(pair.kind, children, Vec::new())?);
            at = pair.close + pair.width;
            plain = at;
            continue;
        }

        let rest = &source[at..end];
        if rest.starts_with("\\\r\n") || rest.starts_with("\\\n") {
            flush(context.text, plain, at, &mut out)?;
            let newline_start = at + 1;
            let newline_end = newline_start + newline_width(&source[newline_start..end]);
            out.push(node(
                Kind::HardLineBreak,
                vec![
                    tok(context.text, at, at + 1, Kind::BackslashToken)?,
                    tok(context.text, newline_start, newline_end, Kind::NewlineToken)?,
                ],
                Vec::new(),
            )?);
            at = newline_end;
            plain = at;
            continue;
        }
        if rest.as_bytes().first() == Some(&b' ') {
            let whitespace_end = at + rest.bytes().take_while(|byte| *byte == b' ').count();
            if whitespace_end - at >= 2
                && matches!(
                    source[whitespace_end..end].as_bytes().first(),
                    Some(b'\r' | b'\n')
                )
            {
                flush(context.text, plain, at, &mut out)?;
                let newline_end = whitespace_end + newline_width(&source[whitespace_end..end]);
                out.push(node(
                    Kind::HardLineBreak,
                    vec![
                        tok(context.text, at, whitespace_end, Kind::WhitespaceToken)?,
                        tok(
                            context.text,
                            whitespace_end,
                            newline_end,
                            Kind::NewlineToken,
                        )?,
                    ],
                    Vec::new(),
                )?);
                at = newline_end;
                plain = at;
                continue;
            }
        }
        if rest.starts_with('`') {
            let run = rest.bytes().take_while(|byte| *byte == b'`').count();
            if let Some(close) = code_span_close(source, at + run, end, run) {
                flush(context.text, plain, at, &mut out)?;
                out.push(node(
                    Kind::CodeSpan,
                    vec![
                        tok(context.text, at, at + run, Kind::CodeDelimiterToken)?,
                        tok(context.text, at + run, close, Kind::TextToken)?,
                        tok(context.text, close, close + run, Kind::CodeDelimiterToken)?,
                    ],
                    Vec::new(),
                )?);
                at = close + run;
                plain = at;
                continue;
            }
        }
        if rest.starts_with('\\') {
            let next_start = at + 1;
            if let Some(next) = source[next_start..end].chars().next() {
                if next.is_ascii_punctuation() {
                    flush(context.text, plain, at, &mut out)?;
                    let next_end = next_start + next.len_utf8();
                    out.push(node(
                        Kind::Escape,
                        vec![
                            tok(context.text, at, next_start, Kind::BackslashToken)?,
                            tok(context.text, next_start, next_end, Kind::TextToken)?,
                        ],
                        Vec::new(),
                    )?);
                    at = next_end;
                    plain = at;
                    continue;
                }
            }
        }
        if rest.starts_with('&') {
            if let Some(relative) = rest.find(';').filter(|relative| *relative > 1) {
                let close = at + relative + 1;
                if let Some(decoded) = decode_entity(&source[at..close]) {
                    flush(context.text, plain, at, &mut out)?;
                    out.push(node(
                        Kind::Entity,
                        vec![tok(context.text, at, close, Kind::EntityToken)?],
                        vec![annotation(
                            ENTITY_VALUE_ID,
                            ENTITY_VALUE_ANNOTATION,
                            Some(decoded.into()),
                        )],
                    )?);
                    at = close;
                    plain = at;
                    continue;
                }
            }
        }
        if rest.starts_with('<') {
            if let Some(relative) = rest.find('>') {
                let close = at + relative + 1;
                let inside = &source[at + 1..close - 1];
                let kind = if is_autolink(inside) {
                    Some(Kind::Autolink)
                } else if is_raw_html(inside) {
                    Some(Kind::RawHtml)
                } else {
                    None
                };
                if let Some(kind) = kind {
                    flush(context.text, plain, at, &mut out)?;
                    out.push(node(
                        kind,
                        vec![
                            tok(context.text, at, at + 1, Kind::AutolinkOpenToken)?,
                            tok(
                                context.text,
                                at + 1,
                                close - 1,
                                if kind == Kind::RawHtml {
                                    Kind::HtmlToken
                                } else {
                                    Kind::TextToken
                                },
                            )?,
                            tok(context.text, close - 1, close, Kind::AutolinkCloseToken)?,
                        ],
                        Vec::new(),
                    )?);
                    at = close;
                    plain = at;
                    continue;
                }
            }
        }
        let image = rest.starts_with("![");
        if image || (allow_links && rest.starts_with('[')) {
            if let Some(parsed) = parse_link(context, at, end, owner, image, allow_links)? {
                flush(context.text, plain, at, &mut out)?;
                out.push(GreenElement::Node(parsed.node));
                at = parsed.end;
                plain = at;
                continue;
            }
        }
        if rest.starts_with('\r') || rest.starts_with('\n') {
            flush(context.text, plain, at, &mut out)?;
            let newline_end = at + newline_width(rest);
            out.push(node(
                Kind::SoftLineBreak,
                vec![tok(context.text, at, newline_end, Kind::NewlineToken)?],
                Vec::new(),
            )?);
            at = newline_end;
            plain = at;
            continue;
        }
        at += source[at..end]
            .chars()
            .next()
            .expect("non-empty range")
            .len_utf8();
    }
    flush(context.text, plain, end, &mut out)?;
    Ok(out)
}

struct ParsedLink {
    node: GreenNode<OkfMarkdownLanguage>,
    end: usize,
}

fn parse_link(
    context: &mut InlineContext<'_>,
    start: usize,
    end: usize,
    owner: SyntaxIdentity,
    image: bool,
    allow_links: bool,
) -> Result<Option<ParsedLink>, ParseError> {
    let source = context.text.shared();
    let open = start + usize::from(image);
    let Some(label_end) = find_unescaped(source, open + 1, end, ']') else {
        return Ok(None);
    };
    let label = &source[open + 1..label_end];
    let after_label = label_end + 1;
    let (
        close,
        semantic_destination,
        semantic_range,
        semantic_title,
        normalized_reference,
        destination_parts,
        link_kind,
    ) = if source[after_label..end].starts_with('(') {
        let Some(parts) = inline_destination(source, after_label, end) else {
            return Ok(None);
        };
        let semantic_title = parts
            .title
            .clone()
            .map(|range| source[range.start + 1..range.end - 1].to_owned());
        (
            parts.close + 1,
            decode_destination(&source[parts.destination.clone()]),
            parts.destination.clone(),
            semantic_title,
            None,
            Some(parts),
            "inline",
        )
    } else {
        let (reference_label, reference_end) = if source[after_label..end].starts_with('[') {
            let Some(reference_end) = find_unescaped(source, after_label + 1, end, ']') else {
                return Ok(None);
            };
            let explicit = &source[after_label + 1..reference_end];
            (
                if explicit.is_empty() { label } else { explicit },
                reference_end + 1,
            )
        } else {
            (label, after_label)
        };
        let Some(normalized) = normalize_label(reference_label) else {
            return Ok(None);
        };
        let Some(definition) = context.references.definitions.get(&normalized) else {
            return Ok(None);
        };
        (
            reference_end,
            definition.destination.to_string(),
            definition.destination_range.start().to_usize()
                ..definition.destination_range.end().to_usize(),
            definition.title.as_ref().map(ToString::to_string),
            Some(normalized),
            None,
            "reference",
        )
    };

    let mut children = Vec::new();
    if image {
        children.push(tok(context.text, start, open, Kind::ImageBangToken)?);
    }
    children.push(tok(context.text, open, open + 1, Kind::LinkLabelOpenToken)?);
    children.extend(parse_inlines(
        context,
        open + 1,
        label_end,
        owner,
        image && allow_links,
    )?);
    children.push(tok(
        context.text,
        label_end,
        label_end + 1,
        Kind::LinkLabelCloseToken,
    )?);
    if let Some(parts) = destination_parts {
        children.push(tok(
            context.text,
            after_label,
            after_label + 1,
            Kind::LinkDestinationOpenToken,
        )?);
        emit_link_gap(
            context.text,
            after_label + 1,
            parts.destination.start,
            &mut children,
        )?;
        children.push(tok(
            context.text,
            parts.destination.start,
            parts.destination.end,
            Kind::LinkDestinationToken,
        )?);
        if let Some(title) = parts.title {
            emit_link_gap(
                context.text,
                parts.destination.end,
                title.start,
                &mut children,
            )?;
            children.push(tok(
                context.text,
                title.start,
                title.end,
                Kind::LinkTitleToken,
            )?);
            emit_link_gap(context.text, title.end, parts.close, &mut children)?;
        } else {
            emit_link_gap(
                context.text,
                parts.destination.end,
                parts.close,
                &mut children,
            )?;
        }
        children.push(tok(
            context.text,
            parts.close,
            parts.close + 1,
            Kind::LinkDestinationCloseToken,
        )?);
    } else if after_label < close {
        children.push(tok(
            context.text,
            after_label,
            after_label + 1,
            Kind::LinkLabelOpenToken,
        )?);
        if after_label + 1 < close - 1 {
            children.push(tok(
                context.text,
                after_label + 1,
                close - 1,
                Kind::TextToken,
            )?);
        }
        children.push(tok(
            context.text,
            close - 1,
            close,
            Kind::LinkLabelCloseToken,
        )?);
    }

    let identity = SyntaxIdentity::fresh()?;
    let mut annotations = vec![
        annotation(
            DESTINATION_ID,
            DESTINATION_ANNOTATION,
            Some(semantic_destination.into()),
        ),
        annotation(
            DESTINATION_RANGE_ID,
            DESTINATION_RANGE_ANNOTATION,
            Some(format!("{}:{}", semantic_range.start, semantic_range.end).into()),
        ),
        annotation(
            LINK_KIND_ID,
            LINK_KIND_ANNOTATION,
            Some(Arc::from(link_kind)),
        ),
        annotation(
            OWNER_ID,
            OWNER_ANNOTATION,
            Some(Arc::from(owner.get().to_string())),
        ),
    ];
    if let Some(title) = semantic_title {
        annotations.push(annotation(TITLE_ID, TITLE_ANNOTATION, Some(title.into())));
    }
    if let Some(label) = normalized_reference {
        annotations.push(annotation(
            REFERENCE_LABEL_ID,
            REFERENCE_LABEL_ANNOTATION,
            Some(label.clone()),
        ));
        let owners = context.backlinks.entry(label).or_default();
        if !owners.contains(&owner) {
            owners.push(owner);
        }
    }
    Ok(Some(ParsedLink {
        node: semantic_with_identity(
            if image { Kind::Image } else { Kind::Link },
            children,
            identity,
            annotations,
        )?,
        end: close,
    }))
}

struct InlineDestination {
    destination: std::ops::Range<usize>,
    title: Option<std::ops::Range<usize>>,
    close: usize,
}

fn inline_destination(source: &str, open: usize, end: usize) -> Option<InlineDestination> {
    let mut at = skip_whitespace(source, open + 1, end);
    let destination = if source[at..end].starts_with('<') {
        let close = find_unescaped(source, at + 1, end, '>')?;
        let result = at + 1..close;
        at = close + 1;
        result
    } else {
        let start = at;
        let mut depth = 0usize;
        while at < end {
            let ch = source[at..end].chars().next()?;
            if ch == '\\' {
                at += ch.len_utf8();
                if at < end {
                    at += source[at..end].chars().next()?.len_utf8();
                }
                continue;
            }
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            } else if ch.is_whitespace() && depth == 0 {
                break;
            }
            at += ch.len_utf8();
        }
        if at == start {
            return None;
        }
        start..at
    };
    at = skip_whitespace(source, at, end);
    let title = if at < end && matches!(source.as_bytes()[at], b'"' | b'\'' | b'(') {
        let close_char = if source.as_bytes()[at] == b'(' {
            ')'
        } else {
            source[at..].chars().next()?
        };
        let close = find_unescaped(source, at + 1, end, close_char)?;
        let result = at..close + close_char.len_utf8();
        at = skip_whitespace(source, close + close_char.len_utf8(), end);
        Some(result)
    } else {
        None
    };
    (source[at..end].starts_with(')')).then_some(InlineDestination {
        destination,
        title,
        close: at,
    })
}

fn emit_link_gap(
    text: &SourceText,
    start: usize,
    end: usize,
    out: &mut Vec<GreenElement<OkfMarkdownLanguage>>,
) -> Result<(), ParseError> {
    if start < end {
        out.push(tok(text, start, end, Kind::WhitespaceToken)?);
    }
    Ok(())
}

fn emphasis_pairs(source: &str, start: usize, end: usize) -> Vec<EmphasisPair> {
    let protected = code_spans(source, start, end);
    let mut delimiters = Vec::<Delimiter>::new();
    let mut pairs = Vec::new();
    let mut at = start;
    while at < end {
        if let Some(range) = protected
            .iter()
            .find(|range| range.start <= at && at < range.end)
        {
            at = range.end;
            continue;
        }
        let byte = source.as_bytes()[at];
        if !matches!(byte, b'*' | b'_') || escaped(source, start, at) {
            at += source[at..end]
                .chars()
                .next()
                .expect("non-empty")
                .len_utf8();
            continue;
        }
        let length = source[at..end]
            .bytes()
            .take_while(|candidate| *candidate == byte)
            .count();
        let (can_open, can_close) = flanking(source, start, end, at, length, byte);
        let mut closer = Delimiter {
            marker: byte,
            start: at,
            length,
            remaining: length,
            can_open,
            can_close,
        };
        if can_close {
            loop {
                let Some(index) = delimiters.iter().rposition(|opener| {
                    opener.marker == byte
                        && opener.can_open
                        && opener.remaining > 0
                        && !rule_of_three(*opener, closer)
                }) else {
                    break;
                };
                let opener = &mut delimiters[index];
                let width = usize::from(opener.remaining >= 2 && closer.remaining >= 2) + 1;
                let open = opener.start + opener.remaining - width;
                let close = closer.start + closer.length - closer.remaining;
                pairs.push(EmphasisPair {
                    open,
                    close,
                    width,
                    kind: if width == 2 {
                        Kind::StrongEmphasis
                    } else {
                        Kind::Emphasis
                    },
                });
                opener.remaining -= width;
                closer.remaining -= width;
                if closer.remaining == 0 {
                    break;
                }
            }
        }
        if can_open && closer.remaining > 0 {
            delimiters.push(closer);
        }
        at += length;
    }
    pairs.sort_by_key(|pair| (pair.open, std::cmp::Reverse(pair.close)));
    pairs
}

fn flanking(
    source: &str,
    start: usize,
    end: usize,
    at: usize,
    length: usize,
    marker: u8,
) -> (bool, bool) {
    let previous = source[start..at].chars().next_back();
    let next = source[at + length..end].chars().next();
    let previous_whitespace = previous.is_none_or(char::is_whitespace);
    let next_whitespace = next.is_none_or(char::is_whitespace);
    let previous_punctuation = previous.is_some_and(is_unicode_punctuation);
    let next_punctuation = next.is_some_and(is_unicode_punctuation);
    let left =
        !next_whitespace && (!next_punctuation || previous_whitespace || previous_punctuation);
    let right =
        !previous_whitespace && (!previous_punctuation || next_whitespace || next_punctuation);
    if marker == b'_' {
        (
            left && (!right || previous_punctuation),
            right && (!left || next_punctuation),
        )
    } else {
        (left, right)
    }
}

fn is_unicode_punctuation(ch: char) -> bool {
    ch.is_ascii_punctuation()
        || matches!(
            ch,
            '\u{00a1}'..='\u{00bf}'
                | '\u{2000}'..='\u{206f}'
                | '\u{2e00}'..='\u{2e7f}'
                | '\u{3000}'..='\u{303f}'
        )
}

fn rule_of_three(opener: Delimiter, closer: Delimiter) -> bool {
    (opener.can_close || closer.can_open)
        && (opener.length + closer.length) % 3 == 0
        && (opener.length % 3 != 0 || closer.length % 3 != 0)
}

fn code_spans(source: &str, start: usize, end: usize) -> Vec<std::ops::Range<usize>> {
    let mut spans = Vec::new();
    let mut at = start;
    while at < end {
        if source.as_bytes()[at] == b'`' {
            let run = source[at..end]
                .bytes()
                .take_while(|byte| *byte == b'`')
                .count();
            if let Some(close) = code_span_close(source, at + run, end, run) {
                spans.push(at..close + run);
                at = close + run;
                continue;
            }
            at += run;
        } else {
            at += source[at..end]
                .chars()
                .next()
                .expect("non-empty")
                .len_utf8();
        }
    }
    spans
}

fn code_span_close(source: &str, mut at: usize, end: usize, wanted: usize) -> Option<usize> {
    while at < end {
        let relative = source[at..end].find('`')?;
        at += relative;
        let run = source[at..end]
            .bytes()
            .take_while(|byte| *byte == b'`')
            .count();
        if run == wanted {
            return Some(at);
        }
        at += run;
    }
    None
}

fn decode_entity(spelling: &str) -> Option<String> {
    let mut text = String::new();
    for event in Parser::new(spelling) {
        if let Event::Text(value) = event {
            text.push_str(&value);
        }
    }
    (text != spelling && !text.is_empty()).then_some(text)
}

fn is_autolink(value: &str) -> bool {
    value.contains('@')
        || value.split_once(':').is_some_and(|(scheme, rest)| {
            !rest.is_empty()
                && scheme.len() >= 2
                && scheme.len() <= 32
                && scheme.bytes().enumerate().all(|(index, byte)| {
                    byte.is_ascii_alphanumeric()
                        || (index > 0 && matches!(byte, b'+' | b'-' | b'.'))
                })
        })
}

fn is_raw_html(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with('!')
        || value.starts_with('?')
        || value
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
}

fn find_unescaped(source: &str, mut at: usize, end: usize, wanted: char) -> Option<usize> {
    while at < end {
        let ch = source[at..end].chars().next()?;
        if ch == wanted && !escaped(source, 0, at) {
            return Some(at);
        }
        at += ch.len_utf8();
    }
    None
}

fn escaped(source: &str, start: usize, at: usize) -> bool {
    source[start..at]
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'\\')
        .count()
        % 2
        == 1
}

fn skip_whitespace(source: &str, mut at: usize, end: usize) -> usize {
    while at < end {
        let ch = source[at..end].chars().next().expect("non-empty");
        if !ch.is_whitespace() {
            break;
        }
        at += ch.len_utf8();
    }
    at
}

fn newline_width(value: &str) -> usize {
    if value.starts_with("\r\n") {
        2
    } else {
        1
    }
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
        let whitespace = source[at..end]
            .chars()
            .next()
            .expect("non-empty")
            .is_whitespace();
        let mut next = at;
        while next < end
            && source[next..end]
                .chars()
                .next()
                .expect("non-empty")
                .is_whitespace()
                == whitespace
        {
            next += source[next..end]
                .chars()
                .next()
                .expect("non-empty")
                .len_utf8();
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
            Vec::new(),
        )?);
        at = next;
    }
    Ok(())
}

fn node(
    kind: Kind,
    children: Vec<GreenElement<OkfMarkdownLanguage>>,
    annotations: Vec<SyntaxAnnotation>,
) -> Result<GreenElement<OkfMarkdownLanguage>, ParseError> {
    let identity = SyntaxIdentity::fresh()?;
    Ok(GreenElement::Node(semantic_with_identity(
        kind,
        children,
        identity,
        annotations,
    )?))
}

fn semantic_with_identity(
    kind: Kind,
    children: Vec<GreenElement<OkfMarkdownLanguage>>,
    identity: SyntaxIdentity,
    mut annotations: Vec<SyntaxAnnotation>,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    annotations.insert(0, identity.annotation());
    GreenFactory::new()
        .node_with_annotations(kind, children, annotations.into())
        .map_err(|_| ParseError::WidthOverflow)
}

fn annotation(id: u64, kind: &'static str, data: Option<Arc<str>>) -> SyntaxAnnotation {
    SyntaxAnnotation::new(NonZeroU64::new(id).expect("non-zero"), kind, data)
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

pub(crate) fn link_annotation<'a>(
    annotations: &'a [SyntaxAnnotation],
    kind: &str,
) -> Option<&'a str> {
    annotations
        .iter()
        .find(|annotation| annotation.kind() == kind)
        .and_then(SyntaxAnnotation::data)
}

pub(crate) fn destination_annotation() -> &'static str {
    DESTINATION_ANNOTATION
}
pub(crate) fn destination_range_annotation() -> &'static str {
    DESTINATION_RANGE_ANNOTATION
}
pub(crate) fn title_annotation() -> &'static str {
    TITLE_ANNOTATION
}
pub(crate) fn link_kind_annotation() -> &'static str {
    LINK_KIND_ANNOTATION
}
pub(crate) fn reference_label_annotation() -> &'static str {
    REFERENCE_LABEL_ANNOTATION
}
pub(crate) fn owner_annotation() -> &'static str {
    OWNER_ANNOTATION
}
pub(crate) fn entity_value_annotation() -> &'static str {
    ENTITY_VALUE_ANNOTATION
}
