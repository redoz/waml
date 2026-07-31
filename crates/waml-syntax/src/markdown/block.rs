use std::{ops::Range, sync::Arc};

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

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

#[derive(Debug)]
struct BlockFrame {
    kind: Kind,
    source_range: Range<usize>,
    children: Vec<BlockFrame>,
    cursor: usize,
}

pub(crate) fn parse(
    text: &SourceText,
    dialect: MarkdownDialect,
    start: usize,
    end: usize,
) -> Result<BlockParse, ParseError> {
    let source = text.shared();
    validate_range(source, start, end)?;
    let event_start = if start == 0 && source.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        start
    };
    let mut stack = Vec::<BlockFrame>::new();
    let mut blocks = Vec::<BlockFrame>::new();
    for (event, offsets) in
        Parser::new_ext(&source[event_start..end], pulldown_options(dialect)).into_offset_iter()
    {
        let mut offsets = (event_start + offsets.start)..(event_start + offsets.end);
        validate_event_range(source, event_start, end, &offsets)?;
        match event {
            Event::Start(tag) => {
                if let Some(kind) = start_kind(&tag, source, &offsets) {
                    if matches!(kind, Kind::ListItem | Kind::IndentedCodeBlock) {
                        let line_start = source[..offsets.start]
                            .rfind('\n')
                            .map(|newline| newline + 1)
                            .unwrap_or(event_start)
                            .max(event_start);
                        let parent_start = stack
                            .last()
                            .map(|parent| parent.source_range.start)
                            .unwrap_or(event_start);
                        offsets.start = line_start.max(parent_start);
                    }
                    stack.push(BlockFrame {
                        kind,
                        source_range: offsets.clone(),
                        children: Vec::new(),
                        cursor: offsets.start,
                    });
                }
            }
            Event::End(tag) => {
                if end_kind(tag).is_some() {
                    let Some(mut frame) = stack.pop() else {
                        return Err(ParseError::StructuralInvariant {
                            reason: "CommonMark closed a block without an open frame".into(),
                        });
                    };
                    frame.source_range.end = frame.source_range.end.max(offsets.end);
                    frame.cursor = frame.source_range.start;
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(frame);
                    } else {
                        blocks.push(frame);
                    }
                }
            }
            Event::Rule => {
                let frame = BlockFrame {
                    kind: Kind::ThematicBreak,
                    source_range: offsets.clone(),
                    children: Vec::new(),
                    cursor: offsets.start,
                };
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(frame);
                } else {
                    blocks.push(frame);
                }
            }
            _ => {}
        }
    }
    while let Some(mut frame) = stack.pop() {
        frame.source_range.end = end;
        if let Some(parent) = stack.last_mut() {
            parent.children.push(frame);
        } else {
            blocks.push(frame);
        }
    }
    blocks.sort_by_key(|frame| (frame.source_range.start, frame.source_range.end));

    let factory = GreenFactory::<OkfMarkdownLanguage>::new();
    let mut diagnostics = Vec::new();
    let mut definitions = Vec::new();
    let mut children = Vec::new();
    let mut cursor = start;
    for frame in blocks {
        if frame.source_range.start < cursor {
            return Err(ParseError::StructuralInvariant {
                reason: "CommonMark block events overlap out of order".into(),
            });
        }
        emit_uncovered(
            &factory,
            text,
            source,
            cursor,
            frame.source_range.start,
            &mut children,
            &mut definitions,
        )?;
        cursor = frame.source_range.end;
        children.push(GreenElement::Node(build_frame(
            &factory,
            text,
            source,
            frame,
            &mut diagnostics,
        )?));
    }
    emit_uncovered(
        &factory,
        text,
        source,
        cursor,
        end,
        &mut children,
        &mut definitions,
    )?;
    let root = factory
        .node(Kind::Root, children)
        .map_err(|_| ParseError::WidthOverflow)?;
    if root.width().to_usize() != end - start {
        return Err(ParseError::StructuralInvariant {
            reason: "block children do not cover their source range".into(),
        });
    }
    let references = super::reference::MarkdownReferenceMap::from_source(source)?;
    let root = super::inline::apply(text, &root, &references)?;
    Ok(BlockParse {
        root,
        diagnostics: diagnostics.into(),
        inline_roots: Arc::from([]),
        definitions: definitions.into(),
    })
}

fn build_frame(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    mut frame: BlockFrame,
    diagnostics: &mut Vec<TreeDiagnostic<Diagnostic>>,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    frame
        .children
        .sort_by_key(|child| (child.source_range.start, child.source_range.end));
    if frame.children.is_empty() {
        let children = leaf_tokens(
            factory,
            text,
            source,
            frame.kind,
            frame.source_range.clone(),
            diagnostics,
        )?;
        return semantic_node(factory, frame.kind, children);
    }
    let mut children = Vec::new();
    frame.cursor = frame.source_range.start;
    for child in frame.children {
        if child.source_range.start < frame.cursor
            || child.source_range.end > frame.source_range.end
        {
            return Err(ParseError::StructuralInvariant {
                reason: "nested CommonMark block escaped its parent frame".into(),
            });
        }
        emit_frame_gap(
            factory,
            text,
            source,
            frame.kind,
            frame.cursor,
            child.source_range.start,
            &mut children,
        )?;
        frame.cursor = child.source_range.end;
        children.push(GreenElement::Node(build_frame(
            factory,
            text,
            source,
            child,
            diagnostics,
        )?));
    }
    emit_frame_gap(
        factory,
        text,
        source,
        frame.kind,
        frame.cursor,
        frame.source_range.end,
        &mut children,
    )?;
    semantic_node(factory, frame.kind, children)
}

fn start_kind(tag: &Tag<'_>, source: &str, range: &Range<usize>) -> Option<Kind> {
    Some(match tag {
        Tag::Paragraph => Kind::Paragraph,
        Tag::Heading { level, .. } => {
            if heading_is_setext(source, range, *level) {
                Kind::SetextHeading
            } else {
                Kind::AtxHeading
            }
        }
        Tag::BlockQuote(_) => Kind::BlockQuote,
        Tag::CodeBlock(CodeBlockKind::Indented) => Kind::IndentedCodeBlock,
        Tag::CodeBlock(CodeBlockKind::Fenced(_)) => Kind::FencedCodeBlock,
        Tag::HtmlBlock => Kind::HtmlBlock,
        Tag::List(_) => Kind::List,
        Tag::Item => Kind::ListItem,
        Tag::Table(_) => Kind::Table,
        Tag::TableHead => Kind::TableHead,
        Tag::TableRow => Kind::TableRow,
        Tag::TableCell => Kind::TableCell,
        _ => return None,
    })
}

fn end_kind(tag: TagEnd) -> Option<Kind> {
    Some(match tag {
        TagEnd::Paragraph => Kind::Paragraph,
        TagEnd::Heading(_) => Kind::AtxHeading,
        TagEnd::BlockQuote(_) => Kind::BlockQuote,
        TagEnd::CodeBlock => Kind::FencedCodeBlock,
        TagEnd::HtmlBlock => Kind::HtmlBlock,
        TagEnd::List(_) => Kind::List,
        TagEnd::Item => Kind::ListItem,
        TagEnd::Table => Kind::Table,
        TagEnd::TableHead => Kind::TableHead,
        TagEnd::TableRow => Kind::TableRow,
        TagEnd::TableCell => Kind::TableCell,
        _ => return None,
    })
}

fn leaf_tokens(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    kind: Kind,
    range: Range<usize>,
    diagnostics: &mut Vec<TreeDiagnostic<Diagnostic>>,
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, ParseError> {
    match kind {
        Kind::AtxHeading => heading_tokens(factory, text, source, range),
        Kind::SetextHeading => setext_tokens(factory, text, source, range),
        Kind::ThematicBreak => {
            line_marker_tokens(factory, text, source, range, Kind::ThematicBreakToken)
        }
        Kind::IndentedCodeBlock => indented_tokens(factory, text, source, range),
        Kind::FencedCodeBlock => fence_tokens(factory, text, source, range, diagnostics),
        Kind::HtmlBlock => Ok(vec![token(
            factory,
            text,
            range.start,
            range.end,
            Kind::HtmlToken,
        )?]),
        Kind::Paragraph => Ok(vec![token(
            factory,
            text,
            range.start,
            range.end,
            Kind::TextToken,
        )?]),
        Kind::ListItem => {
            let marker_end = list_prefix_end(source, range.start, range.end);
            let mut out = vec![token(
                factory,
                text,
                range.start,
                marker_end,
                Kind::ListMarkerToken,
            )?];
            if marker_end < range.end {
                out.push(token(
                    factory,
                    text,
                    marker_end,
                    range.end,
                    Kind::TextToken,
                )?);
            }
            Ok(out)
        }
        _ => Ok(vec![token(
            factory,
            text,
            range.start,
            range.end,
            Kind::TextToken,
        )?]),
    }
}

fn emit_frame_gap(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    parent: Kind,
    start: usize,
    end: usize,
    out: &mut Vec<GreenElement<OkfMarkdownLanguage>>,
) -> Result<(), ParseError> {
    if start == end {
        return Ok(());
    }
    let marker_kind = match parent {
        Kind::BlockQuote => Kind::BlockQuoteMarkerToken,
        Kind::ListItem => Kind::ListMarkerToken,
        _ => Kind::TextToken,
    };
    if matches!(parent, Kind::BlockQuote | Kind::ListItem) {
        let marker_end = if parent == Kind::BlockQuote {
            quote_prefix_end(source, start, end)
        } else {
            list_prefix_end(source, start, end)
        };
        if marker_end > start {
            out.push(token(factory, text, start, marker_end, marker_kind)?);
        }
        if marker_end < end {
            out.push(token(factory, text, marker_end, end, Kind::TextToken)?);
        }
    } else {
        out.push(token(factory, text, start, end, Kind::TextToken)?);
    }
    Ok(())
}

fn emit_uncovered(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    mut start: usize,
    end: usize,
    out: &mut Vec<GreenElement<OkfMarkdownLanguage>>,
    definitions: &mut Vec<TextRange>,
) -> Result<(), ParseError> {
    while start < end {
        if start == 0 && source.starts_with('\u{feff}') {
            out.push(token(
                factory,
                text,
                0,
                '\u{feff}'.len_utf8(),
                Kind::BomToken,
            )?);
            start = '\u{feff}'.len_utf8();
            continue;
        }
        let line_end = next_line(source, start, end);
        if is_link_definition(&source[start..line_end]) {
            definitions.push(range(start, line_end)?);
            out.push(GreenElement::Node(semantic_node(
                factory,
                Kind::LinkReferenceDefinition,
                definition_tokens(factory, text, source, start, line_end)?,
            )?));
        } else {
            out.push(token(
                factory,
                text,
                start,
                line_end,
                if source[start..line_end]
                    .trim_matches([' ', '\t', '\r', '\n'])
                    .is_empty()
                {
                    Kind::NewlineToken
                } else {
                    Kind::BadToken
                },
            )?);
        }
        start = line_end;
    }
    Ok(())
}

fn heading_tokens(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    range: Range<usize>,
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, ParseError> {
    let content_end = trim_newline(source, range.start, range.end);
    let open_end = range.start
        + source[range.clone()]
            .bytes()
            .take_while(|b| *b == b'#')
            .count();
    let text_start = open_end + usize::from(source.as_bytes().get(open_end) == Some(&b' '));
    let trimmed = source[text_start..content_end].trim_end_matches(' ');
    let candidate_end = text_start + trimmed.len();
    let closing_start = source[text_start..candidate_end]
        .rfind(' ')
        .filter(|at| {
            source[text_start + at + 1..candidate_end]
                .bytes()
                .all(|b| b == b'#')
        })
        .map(|at| text_start + at + 1);
    let text_end = closing_start.unwrap_or(candidate_end);
    let mut out = vec![token(
        factory,
        text,
        range.start,
        open_end,
        Kind::HeadingMarkerToken,
    )?];
    if open_end < text_start {
        out.push(token(
            factory,
            text,
            open_end,
            text_start,
            Kind::WhitespaceToken,
        )?);
    }
    if text_start < text_end {
        out.push(token(factory, text, text_start, text_end, Kind::TextToken)?);
    }
    if let Some(closing_start) = closing_start {
        out.push(token(
            factory,
            text,
            closing_start,
            candidate_end,
            Kind::HeadingMarkerToken,
        )?);
    }
    if candidate_end < content_end {
        out.push(token(
            factory,
            text,
            candidate_end,
            content_end,
            Kind::WhitespaceToken,
        )?);
    }
    if content_end < range.end {
        out.push(token(
            factory,
            text,
            content_end,
            range.end,
            Kind::NewlineToken,
        )?);
    }
    Ok(out)
}

fn setext_tokens(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    range: Range<usize>,
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, ParseError> {
    let content_end = trim_newline(source, range.start, range.end);
    let underline_start = source[range.start..content_end]
        .rfind('\n')
        .map(|at| range.start + at + 1)
        .unwrap_or(range.start);
    let underline_end = content_end;
    let mut out = vec![token(
        factory,
        text,
        range.start,
        underline_start,
        Kind::TextToken,
    )?];
    out.push(token(
        factory,
        text,
        underline_start,
        underline_end,
        Kind::SetextUnderlineToken,
    )?);
    if underline_end < range.end {
        out.push(token(
            factory,
            text,
            underline_end,
            range.end,
            Kind::NewlineToken,
        )?);
    }
    Ok(out)
}

fn line_marker_tokens(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    range: Range<usize>,
    kind: Kind,
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, ParseError> {
    let marker_end = trim_newline(source, range.start, range.end);
    let mut out = vec![token(factory, text, range.start, marker_end, kind)?];
    if marker_end < range.end {
        out.push(token(
            factory,
            text,
            marker_end,
            range.end,
            Kind::NewlineToken,
        )?);
    }
    Ok(out)
}

fn indented_tokens(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    range: Range<usize>,
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, ParseError> {
    let mut out = Vec::new();
    let mut at = range.start;
    while at < range.end {
        let end = next_line(source, at, range.end);
        let indent_end = if source[at..end].starts_with('\t') {
            at + 1
        } else {
            (at + 4).min(end)
        };
        out.push(token(factory, text, at, indent_end, Kind::IndentToken)?);
        if indent_end < end {
            out.push(token(factory, text, indent_end, end, Kind::CodeTextToken)?);
        }
        at = end;
    }
    Ok(out)
}

fn fence_tokens(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    range: Range<usize>,
    diagnostics: &mut Vec<TreeDiagnostic<Diagnostic>>,
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, ParseError> {
    let first_end = next_line(source, range.start, range.end);
    let indent = source[range.start..first_end]
        .bytes()
        .take_while(|b| *b == b' ')
        .count()
        .min(3);
    let fence_start = range.start + indent;
    let fence_byte = source.as_bytes()[fence_start];
    let fence_len = source[fence_start..first_end]
        .bytes()
        .take_while(|b| *b == fence_byte)
        .count();
    let open_end = fence_start + fence_len;
    let info_end = trim_newline(source, open_end, first_end);
    let mut close = None;
    let mut at = first_end;
    while at < range.end {
        let line_end = next_line(source, at, range.end);
        let content_end = trim_newline(source, at, line_end);
        let spaces = source[at..content_end]
            .bytes()
            .take_while(|b| *b == b' ')
            .count();
        if spaces <= 3 {
            let marker_start = at + spaces;
            let count = source[marker_start..content_end]
                .bytes()
                .take_while(|b| *b == fence_byte)
                .count();
            if count >= fence_len && source[marker_start + count..content_end].trim().is_empty() {
                close = Some((
                    at,
                    marker_start,
                    marker_start + count,
                    content_end,
                    line_end,
                ));
            }
        }
        at = line_end;
    }
    let mut out = Vec::new();
    if range.start < fence_start {
        out.push(token(
            factory,
            text,
            range.start,
            fence_start,
            Kind::WhitespaceToken,
        )?);
    }
    out.push(token(
        factory,
        text,
        fence_start,
        open_end,
        Kind::CodeFenceToken,
    )?);
    if open_end < info_end {
        out.push(token(
            factory,
            text,
            open_end,
            info_end,
            Kind::InfoStringToken,
        )?);
    }
    if info_end < first_end {
        out.push(token(
            factory,
            text,
            info_end,
            first_end,
            Kind::NewlineToken,
        )?);
    }
    let content_end = close.map_or(range.end, |value| value.0);
    if first_end < content_end {
        out.push(token(
            factory,
            text,
            first_end,
            content_end,
            Kind::CodeTextToken,
        )?);
    }
    if let Some((line_start, marker_start, marker_end, content_end, line_end)) = close {
        if line_start < marker_start {
            out.push(token(
                factory,
                text,
                line_start,
                marker_start,
                Kind::WhitespaceToken,
            )?);
        }
        out.push(token(
            factory,
            text,
            marker_start,
            marker_end,
            Kind::CodeFenceToken,
        )?);
        if marker_end < content_end {
            out.push(token(
                factory,
                text,
                marker_end,
                content_end,
                Kind::WhitespaceToken,
            )?);
        }
        if content_end < line_end {
            out.push(token(
                factory,
                text,
                content_end,
                line_end,
                Kind::NewlineToken,
            )?);
        }
    } else {
        diagnostics.push(diagnostic(
            Diagnostic::UnclosedFence,
            range.end,
            range.end,
            "unclosed code fence",
        )?);
    }
    Ok(out)
}

fn definition_tokens(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, ParseError> {
    let close = source[start..end]
        .find("]:")
        .ok_or_else(|| ParseError::StructuralInvariant {
            reason: "confirmed link definition lost its label delimiter".into(),
        })?
        + start;
    let mut out = vec![
        token(factory, text, start, start + 1, Kind::LinkLabelOpenToken)?,
        token(factory, text, start + 1, close, Kind::TextToken)?,
        token(factory, text, close, close + 1, Kind::LinkLabelCloseToken)?,
        token(factory, text, close + 1, close + 2, Kind::ColonToken)?,
    ];
    let content_end = trim_newline(source, close + 2, end);
    let mut at = close + 2;
    let destination_start = at
        + source[at..content_end]
            .bytes()
            .take_while(u8::is_ascii_whitespace)
            .count();
    if at < destination_start {
        out.push(token(
            factory,
            text,
            at,
            destination_start,
            Kind::WhitespaceToken,
        )?);
    }
    at = destination_start;
    if source.as_bytes().get(at) == Some(&b'<') {
        out.push(token(
            factory,
            text,
            at,
            at + 1,
            Kind::LinkDestinationOpenToken,
        )?);
        let destination_end = at
            + 1
            + source[at + 1..content_end].find('>').ok_or_else(|| {
                ParseError::StructuralInvariant {
                    reason: "confirmed link destination lost its close delimiter".into(),
                }
            })?;
        out.push(token(
            factory,
            text,
            at + 1,
            destination_end,
            Kind::LinkDestinationToken,
        )?);
        out.push(token(
            factory,
            text,
            destination_end,
            destination_end + 1,
            Kind::LinkDestinationCloseToken,
        )?);
        at = destination_end + 1;
    } else {
        let destination_end = source[at..content_end]
            .find(char::is_whitespace)
            .map(|offset| at + offset)
            .unwrap_or(content_end);
        out.push(token(
            factory,
            text,
            at,
            destination_end,
            Kind::LinkDestinationToken,
        )?);
        at = destination_end;
    }
    let title_start = at
        + source[at..content_end]
            .bytes()
            .take_while(u8::is_ascii_whitespace)
            .count();
    if at < title_start {
        out.push(token(
            factory,
            text,
            at,
            title_start,
            Kind::WhitespaceToken,
        )?);
    }
    at = title_start;
    if source.as_bytes().get(at) == Some(&b'"') {
        let title_end = at
            + 1
            + source[at + 1..content_end].find('"').ok_or_else(|| {
                ParseError::StructuralInvariant {
                    reason: "confirmed link title lost its close delimiter".into(),
                }
            })?;
        out.push(token(factory, text, at, at + 1, Kind::LinkTitleToken)?);
        out.push(token(factory, text, at + 1, title_end, Kind::TextToken)?);
        out.push(token(
            factory,
            text,
            title_end,
            title_end + 1,
            Kind::LinkTitleToken,
        )?);
        at = title_end + 1;
    }
    if at < content_end {
        out.push(token(
            factory,
            text,
            at,
            content_end,
            Kind::WhitespaceToken,
        )?);
    }
    if content_end < end {
        out.push(token(factory, text, content_end, end, Kind::NewlineToken)?);
    }
    Ok(out)
}

fn semantic_node(
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

fn token(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    start: usize,
    end: usize,
    kind: Kind,
) -> Result<GreenElement<OkfMarkdownLanguage>, ParseError> {
    Ok(GreenElement::Token(
        factory
            .token(kind, slice(text, start, end)?, [], [])
            .map_err(|_| ParseError::WidthOverflow)?,
    ))
}

fn validate_range(source: &str, start: usize, end: usize) -> Result<(), ParseError> {
    if start > end
        || end > source.len()
        || !source.is_char_boundary(start)
        || !source.is_char_boundary(end)
    {
        return Err(ParseError::StructuralInvariant {
            reason: "invalid Markdown block range".into(),
        });
    }
    Ok(())
}
fn validate_event_range(
    source: &str,
    start: usize,
    end: usize,
    event: &Range<usize>,
) -> Result<(), ParseError> {
    if event.start > event.end
        || event.start < start
        || event.end > end
        || !source.is_char_boundary(event.start)
        || !source.is_char_boundary(event.end)
    {
        return Err(ParseError::StructuralInvariant {
            reason: "invalid CommonMark event range".into(),
        });
    }
    Ok(())
}
fn heading_is_setext(source: &str, range: &Range<usize>, _level: HeadingLevel) -> bool {
    source[range.clone()].lines().nth(1).is_some_and(|line| {
        let line = line.trim();
        !line.is_empty() && line.bytes().all(|byte| matches!(byte, b'=' | b'-'))
    })
}
fn quote_prefix_end(source: &str, start: usize, end: usize) -> usize {
    if source.as_bytes().get(start) == Some(&b'>') {
        (start + 1 + usize::from(source.as_bytes().get(start + 1) == Some(&b' '))).min(end)
    } else {
        start
    }
}
fn list_prefix_end(source: &str, start: usize, end: usize) -> usize {
    let bytes = source.as_bytes();
    if matches!(bytes.get(start), Some(b'-' | b'+' | b'*')) {
        return (start + 1).min(end);
    }
    let mut at = start;
    while at < end && bytes[at].is_ascii_digit() {
        at += 1;
    }
    if at < end && matches!(bytes[at], b'.' | b')') {
        at + 1
    } else {
        start
    }
}
fn is_link_definition(line: &str) -> bool {
    line.starts_with('[') && line.contains("]:")
}
fn next_line(source: &str, start: usize, cap: usize) -> usize {
    source[start..cap]
        .find('\n')
        .map(|at| start + at + 1)
        .unwrap_or(cap)
}
fn trim_newline(source: &str, start: usize, end: usize) -> usize {
    start + source[start..end].trim_end_matches(['\r', '\n']).len()
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
