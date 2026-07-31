use std::sync::Arc;

use pulldown_cmark::Options;

use crate::{
    GreenElement, GreenFactory, GreenNode, GreenText, MarkdownDialect, OkfMarkdownLanguage,
    OkfMarkdownSyntaxKind as Kind, OkfSyntaxDiagnosticCode as Diagnostic, ParseError, SourceText,
    SyntaxIdentity, TextRange, TextSize, TreeDiagnostic,
};

#[allow(dead_code)]
pub(crate) struct BlockParse {
    pub root: GreenNode<OkfMarkdownLanguage>,
    pub diagnostics: Arc<[TreeDiagnostic<Diagnostic>]>,
    pub inline_roots: Arc<[GreenNode<OkfMarkdownLanguage>]>,
    pub definitions: Arc<[TextRange]>,
}

/// The only pulldown extensions that the public Markdown profile names.
pub(crate) fn pulldown_options(dialect: MarkdownDialect) -> Options {
    let mut options = Options::empty();
    if dialect.tables() {
        options.insert(Options::ENABLE_TABLES);
    }
    if dialect.strikethrough() {
        options.insert(Options::ENABLE_STRIKETHROUGH);
    }
    if dialect.task_lists() {
        options.insert(Options::ENABLE_TASKLISTS);
    }
    options
}

pub(crate) fn parse(
    text: &SourceText,
    dialect: MarkdownDialect,
    start: usize,
    end: usize,
) -> Result<BlockParse, ParseError> {
    let source = text.shared();
    let _options = pulldown_options(dialect);
    if start > end
        || end > source.len()
        || !source.is_char_boundary(start)
        || !source.is_char_boundary(end)
    {
        return Err(ParseError::StructuralInvariant {
            reason: "invalid Markdown block range".into(),
        });
    }
    let factory = GreenFactory::<OkfMarkdownLanguage>::new();
    let mut out = Vec::new();
    let mut diagnostics = Vec::new();
    let mut definitions = Vec::new();
    let mut at = start;
    while at < end {
        let line_end = next_line(source, at, end);
        let line = &source[at..line_end];
        if at == 0 && source.starts_with('\u{feff}') {
            out.push(element_token(
                &factory,
                text,
                0,
                '\u{feff}'.len_utf8(),
                Kind::BomToken,
            )?);
            at = '\u{feff}'.len_utf8();
        } else if line.trim_matches([' ', '\t', '\r', '\n']).is_empty() {
            out.push(element_token(
                &factory,
                text,
                at,
                line_end,
                Kind::NewlineToken,
            )?);
            at = line_end;
        } else if let Some(marker_end) = quote_marker(source, at, line_end) {
            out.push(GreenElement::Node(block_node(
                &factory,
                Kind::BlockQuote,
                vec![
                    element_token(&factory, text, at, marker_end, Kind::BlockQuoteMarkerToken)?,
                    element_token(&factory, text, marker_end, line_end, Kind::TextToken)?,
                ],
            )?));
            at = line_end;
        } else if let Some((marker_end, ordered)) = list_marker(source, at, line_end) {
            let item = block_node(
                &factory,
                Kind::ListItem,
                vec![
                    element_token(&factory, text, at, marker_end, Kind::ListMarkerToken)?,
                    element_token(&factory, text, marker_end, line_end, Kind::TextToken)?,
                ],
            )?;
            let list_kind = if ordered { Kind::List } else { Kind::List };
            out.push(GreenElement::Node(block_node(
                &factory,
                list_kind,
                vec![GreenElement::Node(item)],
            )?));
            at = line_end;
        } else if let Some((marker_end, text_start, closing_start)) =
            atx_marker(source, at, line_end)
        {
            let mut children = vec![element_token(
                &factory,
                text,
                at,
                marker_end,
                Kind::HeadingMarkerToken,
            )?];
            if marker_end < text_start {
                children.push(element_token(
                    &factory,
                    text,
                    marker_end,
                    text_start,
                    Kind::WhitespaceToken,
                )?);
            }
            if text_start < closing_start {
                children.push(element_token(
                    &factory,
                    text,
                    text_start,
                    closing_start,
                    Kind::TextToken,
                )?);
            }
            if closing_start < line_end {
                children.push(element_token(
                    &factory,
                    text,
                    closing_start,
                    line_end,
                    Kind::HeadingMarkerToken,
                )?);
            }
            out.push(GreenElement::Node(block_node(
                &factory,
                Kind::AtxHeading,
                children,
            )?));
            at = line_end;
        } else if is_thematic(line) {
            let marker_end = trim_line_end(source, at, line_end);
            let mut children = vec![element_token(
                &factory,
                text,
                at,
                marker_end,
                Kind::ThematicBreakToken,
            )?];
            if marker_end < line_end {
                children.push(element_token(
                    &factory,
                    text,
                    marker_end,
                    line_end,
                    Kind::NewlineToken,
                )?);
            }
            out.push(GreenElement::Node(block_node(
                &factory,
                Kind::ThematicBreak,
                children,
            )?));
            at = line_end;
        } else if let Some(fence_end) = fence_marker(source, at, line_end) {
            let fence = &source[at..fence_end];
            let mut close = None;
            let mut cursor = line_end;
            while cursor < end {
                let next = next_line(source, cursor, end);
                if source[cursor..next].trim_start().starts_with(fence) {
                    close = Some((cursor, next));
                    break;
                }
                cursor = next;
            }
            let content_end = close.map_or(end, |(close_start, _)| close_start);
            let mut children = vec![element_token(
                &factory,
                text,
                at,
                fence_end,
                Kind::CodeFenceToken,
            )?];
            if fence_end < line_end {
                let info_end = trim_line_end(source, fence_end, line_end);
                children.push(element_token(
                    &factory,
                    text,
                    fence_end,
                    info_end,
                    Kind::InfoStringToken,
                )?);
                if info_end < line_end {
                    children.push(element_token(
                        &factory,
                        text,
                        info_end,
                        line_end,
                        Kind::NewlineToken,
                    )?);
                }
            }
            if line_end < content_end {
                children.push(element_token(
                    &factory,
                    text,
                    line_end,
                    content_end,
                    Kind::CodeTextToken,
                )?);
            }
            if let Some((close_start, close_end)) = close {
                children.push(element_token(
                    &factory,
                    text,
                    close_start,
                    close_end,
                    Kind::CodeFenceToken,
                )?);
                at = close_end;
            } else {
                diagnostics.push(diagnostic(
                    Diagnostic::UnclosedFence,
                    end,
                    end,
                    "unclosed code fence",
                )?);
                at = end;
            }
            out.push(GreenElement::Node(block_node(
                &factory,
                Kind::FencedCodeBlock,
                children,
            )?));
        } else if indented(line) {
            let prefix = indentation_end(source, at, line_end);
            out.push(GreenElement::Node(block_node(
                &factory,
                Kind::IndentedCodeBlock,
                vec![
                    element_token(&factory, text, at, prefix, Kind::IndentToken)?,
                    element_token(&factory, text, prefix, line_end, Kind::CodeTextToken)?,
                ],
            )?));
            at = line_end;
        } else if html_block(line) {
            out.push(GreenElement::Node(block_node(
                &factory,
                Kind::HtmlBlock,
                vec![element_token(
                    &factory,
                    text,
                    at,
                    line_end,
                    Kind::HtmlToken,
                )?],
            )?));
            at = line_end;
        } else if let Some(parts) = definition_parts(source, at, line_end) {
            let mut children = Vec::new();
            for (from, to, kind) in parts {
                children.push(element_token(&factory, text, from, to, kind)?);
            }
            definitions.push(range(at, line_end)?);
            out.push(GreenElement::Node(block_node(
                &factory,
                Kind::LinkReferenceDefinition,
                children,
            )?));
            at = line_end;
        } else if line_end < end && is_setext(&source[line_end..next_line(source, line_end, end)]) {
            let underline_end = next_line(source, line_end, end);
            let underline_content_end = trim_line_end(source, line_end, underline_end);
            let mut children = vec![
                element_token(&factory, text, at, line_end, Kind::TextToken)?,
                element_token(
                    &factory,
                    text,
                    line_end,
                    underline_content_end,
                    Kind::SetextUnderlineToken,
                )?,
            ];
            if underline_content_end < underline_end {
                children.push(element_token(
                    &factory,
                    text,
                    underline_content_end,
                    underline_end,
                    Kind::NewlineToken,
                )?);
            }
            out.push(GreenElement::Node(block_node(
                &factory,
                Kind::SetextHeading,
                children,
            )?));
            at = underline_end;
        } else {
            let mut until = line_end;
            while until < end {
                let next = next_line(source, until, end);
                if source[until..next]
                    .trim_matches([' ', '\t', '\r', '\n'])
                    .is_empty()
                    || recognises_block(source, until, next)
                {
                    break;
                }
                until = next;
            }
            out.push(GreenElement::Node(block_node(
                &factory,
                Kind::Paragraph,
                vec![element_token(&factory, text, at, until, Kind::TextToken)?],
            )?));
            at = until;
        }
    }
    let root = factory
        .node(Kind::Root, out)
        .map_err(|_| ParseError::WidthOverflow)?;
    Ok(BlockParse {
        root,
        diagnostics: diagnostics.into(),
        inline_roots: Arc::from([]),
        definitions: definitions.into(),
    })
}

fn block_node(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    kind: Kind,
    children: Vec<GreenElement<OkfMarkdownLanguage>>,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    factory
        .node_with_annotations(
            kind,
            children,
            Arc::from([SyntaxIdentity::fresh()?.annotation()]),
        )
        .map_err(|_| ParseError::WidthOverflow)
}
fn element_token(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    start: usize,
    end: usize,
    kind: Kind,
) -> Result<GreenElement<OkfMarkdownLanguage>, ParseError> {
    if start == end {
        return Ok(GreenElement::Token(factory.missing_token(kind)));
    }
    Ok(GreenElement::Token(
        factory
            .token(kind, slice(text, start, end)?, [], [])
            .map_err(|_| ParseError::WidthOverflow)?,
    ))
}
fn slice(text: &SourceText, start: usize, end: usize) -> Result<GreenText, ParseError> {
    let range = range(start, end)?;
    text.slice(range)
        .map_err(|_| ParseError::InvalidRange { range })?;
    Ok(GreenText::SourceSlice {
        source: text.clone(),
        range,
    })
}
fn range(start: usize, end: usize) -> Result<TextRange, ParseError> {
    TextRange::new(size(start)?, size(end)?).map_err(|_| ParseError::StructuralInvariant {
        reason: "reversed block range".into(),
    })
}
fn size(value: usize) -> Result<TextSize, ParseError> {
    TextSize::try_from_usize(value).map_err(|_| ParseError::SourceTooLarge { bytes: value })
}
fn diagnostic(
    code: Diagnostic,
    start: usize,
    end: usize,
    message: &'static str,
) -> Result<TreeDiagnostic<Diagnostic>, ParseError> {
    Ok(TreeDiagnostic {
        code,
        range: range(start, end)?,
        severity: crate::SyntaxSeverity::Error,
        message: Arc::from(message),
    })
}
fn next_line(source: &str, start: usize, cap: usize) -> usize {
    source[start..cap]
        .find('\n')
        .map(|n| start + n + 1)
        .unwrap_or(cap)
}
fn trim_line_end(source: &str, start: usize, end: usize) -> usize {
    source[start..end].trim_end_matches(['\r', '\n']).len() + start
}
fn quote_marker(source: &str, start: usize, end: usize) -> Option<usize> {
    source[start..end]
        .strip_prefix('>')
        .map(|line| start + 1 + usize::from(line.starts_with(' ')))
}
fn list_marker(source: &str, start: usize, end: usize) -> Option<(usize, bool)> {
    let line = &source[start..end];
    if matches!(line.as_bytes().first(), Some(b'-' | b'+' | b'*'))
        && line.as_bytes().get(1).is_some_and(u8::is_ascii_whitespace)
    {
        return Some((start + 1, false));
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    (digits > 0 && matches!(line.as_bytes().get(digits), Some(b'.' | b')')))
        .then_some((start + digits + 1, true))
}
fn atx_marker(source: &str, start: usize, end: usize) -> Option<(usize, usize, usize)> {
    let line = &source[start..end];
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    if hashes == 0
        || hashes > 6
        || !line
            .as_bytes()
            .get(hashes)
            .is_some_and(u8::is_ascii_whitespace)
    {
        return None;
    }
    let body_end = line.trim_end_matches(['\r', '\n', ' ']).len();
    let close = line[..body_end]
        .rfind(' ')
        .filter(|&at| line[at + 1..body_end].bytes().all(|b| b == b'#'))
        .map(|at| start + at + 1)
        .unwrap_or(start + body_end);
    Some((start + hashes, start + hashes + 1, close))
}
fn is_thematic(line: &str) -> bool {
    let chars: Vec<_> = line.chars().filter(|c| !c.is_whitespace()).collect();
    chars.len() >= 3 && chars.iter().all(|c| *c == chars[0]) && matches!(chars[0], '-' | '*' | '_')
}
fn fence_marker(source: &str, start: usize, end: usize) -> Option<usize> {
    let line = &source[start..end];
    let ch = *line.as_bytes().first()?;
    if !matches!(ch, b'`' | b'~') {
        return None;
    }
    let count = line.bytes().take_while(|b| *b == ch).count();
    (count >= 3).then_some(start + count)
}
fn indented(line: &str) -> bool {
    line.starts_with("    ") || line.starts_with('\t')
}
fn indentation_end(source: &str, start: usize, end: usize) -> usize {
    if source[start..end].starts_with('\t') {
        start + 1
    } else {
        start + 4
    }
}
fn html_block(line: &str) -> bool {
    line.trim_start().starts_with('<')
}
fn is_setext(line: &str) -> bool {
    let line = line.trim_matches([' ', '\t', '\r', '\n']);
    !line.is_empty() && line.bytes().all(|b| matches!(b, b'=' | b'-'))
}
fn recognises_block(source: &str, start: usize, end: usize) -> bool {
    quote_marker(source, start, end).is_some()
        || list_marker(source, start, end).is_some()
        || atx_marker(source, start, end).is_some()
        || is_thematic(&source[start..end])
        || fence_marker(source, start, end).is_some()
        || indented(&source[start..end])
        || html_block(&source[start..end])
        || definition_parts(source, start, end).is_some()
}
fn definition_parts(source: &str, start: usize, end: usize) -> Option<Vec<(usize, usize, Kind)>> {
    let line = &source[start..end];
    let close = line.find("]: ").or_else(|| line.find("]:"))?;
    if !line.starts_with('[') {
        return None;
    }
    let colon = start + close + 1;
    let mut parts = vec![
        (start, start + 1, Kind::LinkLabelOpenToken),
        (start + 1, start + close, Kind::TextToken),
        (start + close, start + close + 1, Kind::LinkLabelCloseToken),
        (colon, colon + 1, Kind::ColonToken),
    ];
    let mut at = colon + 1;
    let destination_start = source[at..end]
        .find(|ch: char| !ch.is_whitespace())
        .map(|offset| at + offset)
        .unwrap_or(end);
    if at < destination_start {
        parts.push((at, destination_start, Kind::WhitespaceToken));
    }
    at = destination_start;
    if source.as_bytes().get(at) == Some(&b'<') {
        parts.push((at, at + 1, Kind::LinkDestinationOpenToken));
        let close = source[at + 1..end].find('>').map(|n| at + 1 + n)?;
        parts.push((at + 1, close, Kind::LinkDestinationToken));
        parts.push((close, close + 1, Kind::LinkDestinationCloseToken));
        at = close + 1;
    } else {
        let close = source[at..end]
            .find(char::is_whitespace)
            .map(|n| at + n)
            .unwrap_or(end);
        parts.push((at, close, Kind::LinkDestinationToken));
        at = close;
    }
    let title_start = source[at..end]
        .find(|ch: char| !ch.is_whitespace())
        .map(|offset| at + offset)
        .unwrap_or(end);
    if at < title_start {
        parts.push((at, title_start, Kind::WhitespaceToken));
    }
    at = title_start;
    if let Some(quote) = source[at..end].find('"') {
        let quote = at + quote;
        let close = source[quote + 1..end].find('"').map(|n| quote + 1 + n)?;
        parts.push((quote, quote + 1, Kind::LinkTitleToken));
        parts.push((quote + 1, close, Kind::TextToken));
        parts.push((close, close + 1, Kind::LinkTitleToken));
        at = close + 1;
    }
    if at < end {
        parts.push((at, end, Kind::WhitespaceToken));
    }
    Some(parts)
}
