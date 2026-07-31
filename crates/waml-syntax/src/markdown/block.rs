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
    pub references: super::reference::MarkdownReferenceMap,
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
    metadata: Option<(&'static str, &'static str)>,
    table_alignments: Vec<super::gfm::TableAlignment>,
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
                    let metadata = if kind == Kind::TableCell {
                        let column = stack.last().map_or(0, |parent| {
                            parent
                                .children
                                .iter()
                                .filter(|child| child.kind == Kind::TableCell)
                                .count()
                        });
                        stack
                            .iter()
                            .rev()
                            .find(|frame| frame.kind == Kind::Table)
                            .and_then(|table| table.table_alignments.get(column))
                            .map(|alignment| (super::gfm::TABLE_ALIGNMENT, alignment.data()))
                    } else {
                        None
                    };
                    let table_alignments = match &tag {
                        Tag::Table(alignments) => alignments
                            .iter()
                            .copied()
                            .map(super::gfm::TableAlignment::from_pulldown)
                            .collect(),
                        _ => Vec::new(),
                    };
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
                        metadata,
                        table_alignments,
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
                    metadata: None,
                    table_alignments: Vec::new(),
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
            dialect,
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
    let references = super::reference::MarkdownReferenceMap::from_tree(source, &root, start)?;
    let inline = super::inline::apply(text, &root, references, start, dialect)?;
    diagnostics.extend(inline.diagnostics.iter().cloned());
    Ok(BlockParse {
        root: inline.root,
        diagnostics: diagnostics.into(),
        inline_roots: inline.inline_roots,
        definitions: definitions.into(),
        references: inline.references,
    })
}

pub(crate) fn wrap_waml_sections(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    source: &str,
    dialect: MarkdownDialect,
    start: usize,
    children: Vec<GreenElement<OkfMarkdownLanguage>>,
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, ParseError> {
    if !dialect.waml_sections() {
        return Ok(children);
    }
    let mut offsets = Vec::with_capacity(children.len());
    let mut at = start;
    for child in &children {
        offsets.push(at);
        at += match child {
            GreenElement::Node(node) => node.width().to_usize(),
            GreenElement::Token(token) => token.width().to_usize(),
        };
    }
    let mut out = Vec::with_capacity(children.len());
    let mut index = 0;
    while index < children.len() {
        let Some((level, text_range)) = heading_at(&children[index], source, offsets[index]) else {
            out.push(children[index].clone());
            index += 1;
            continue;
        };
        if level != 2 || super::projection::waml_kind(source, text_range).is_none() {
            out.push(children[index].clone());
            index += 1;
            continue;
        }
        let mut end = index + 1;
        while end < children.len() {
            if let Some((next_level, _)) = heading_at(&children[end], source, offsets[end]) {
                if next_level <= level {
                    break;
                }
            }
            end += 1;
        }
        let identity = SyntaxIdentity::fresh()?;
        let section = factory
            .node_with_annotations(
                Kind::WamlSection,
                children[index..end].to_vec(),
                vec![identity.annotation()].into(),
            )
            .map_err(|_| ParseError::WidthOverflow)?;
        out.push(GreenElement::Node(section));
        index = end;
    }
    Ok(out)
}

fn heading_at(
    element: &GreenElement<OkfMarkdownLanguage>,
    source: &str,
    start: usize,
) -> Option<(u8, TextRange)> {
    let GreenElement::Node(node) = element else {
        return None;
    };
    if !matches!(node.kind(), Kind::AtxHeading | Kind::SetextHeading) {
        return None;
    }
    let end = start + node.width().to_usize();
    let line_end = source[start..end]
        .find('\n')
        .map_or(end, |offset| start + offset + 1);
    let line = &source[start..line_end];
    let trimmed = line.trim_start();
    let hashes = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    let level = if hashes > 0 { hashes as u8 } else { 2 };
    let text_start = if hashes > 0 {
        start
            + (line.len() - trimmed.len())
            + hashes
            + usize::from(trimmed.as_bytes().get(hashes) == Some(&b' '))
    } else {
        start
    };
    let text_range = range(text_start, line_end).ok()?;
    Some((level, text_range))
}

fn build_frame(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    mut frame: BlockFrame,
    diagnostics: &mut Vec<TreeDiagnostic<Diagnostic>>,
    dialect: MarkdownDialect,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    let metadata = frame_metadata(source, &frame, dialect);
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
            dialect,
        )?;
        return semantic_node_with_metadata(factory, frame.kind, children, metadata);
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
            dialect,
        )?;
        frame.cursor = child.source_range.end;
        children.push(GreenElement::Node(build_frame(
            factory,
            text,
            source,
            child,
            diagnostics,
            dialect,
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
        dialect,
    )?;
    if frame.kind == Kind::Table {
        let mut grouped = Vec::new();
        let mut body = Vec::new();
        for child in children {
            if matches!(&child, GreenElement::Node(node) if node.kind() == Kind::TableRow) {
                body.push(child);
            } else {
                if !body.is_empty() {
                    grouped.push(GreenElement::Node(semantic_node(
                        factory,
                        Kind::TableBody,
                        std::mem::take(&mut body),
                    )?));
                }
                grouped.push(child);
            }
        }
        if !body.is_empty() {
            grouped.push(GreenElement::Node(semantic_node(
                factory,
                Kind::TableBody,
                body,
            )?));
        }
        children = grouped;
    }
    semantic_node_with_metadata(factory, frame.kind, children, metadata)
}

fn frame_metadata(
    source: &str,
    frame: &BlockFrame,
    dialect: MarkdownDialect,
) -> Option<(&'static str, &'static str)> {
    if let Some(metadata) = frame.metadata {
        return Some(metadata);
    }
    if frame.kind == Kind::ListItem && dialect.task_lists() {
        let marker_end = list_prefix_end(source, frame.source_range.start, frame.source_range.end);
        let task_start = marker_end + usize::from(source.as_bytes().get(marker_end) == Some(&b' '));
        return super::gfm::task_marker(source, task_start, frame.source_range.end)
            .map(|(_, state)| (super::gfm::TASK_STATE, state.data()));
    }
    if frame.kind == Kind::HtmlBlock && dialect.tag_filter() {
        let (_, state) = super::gfm::classify_html(&source[frame.source_range.clone()]);
        return Some((super::gfm::HTML_TAG_FILTER, state.data()));
    }
    None
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
    dialect: MarkdownDialect,
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, ParseError> {
    match kind {
        Kind::AtxHeading => heading_tokens(factory, text, source, range),
        Kind::SetextHeading => setext_tokens(factory, text, source, range),
        Kind::ThematicBreak => {
            line_marker_tokens(factory, text, source, range, Kind::ThematicBreakToken)
        }
        Kind::IndentedCodeBlock => indented_tokens(factory, text, source, range),
        Kind::FencedCodeBlock => fence_tokens(factory, text, source, range, diagnostics),
        Kind::HtmlBlock => {
            if dialect.tag_filter() {
                if let (Some((tag_start, tag_end)), super::gfm::HtmlTagFilter::Disallowed) =
                    super::gfm::classify_html(&source[range.clone()])
                {
                    diagnostics.push(diagnostic(
                        Diagnostic::FilteredHtmlTag,
                        range.start + tag_start,
                        range.start + tag_end,
                        "disallowed GFM HTML tag",
                    )?);
                }
            }
            Ok(vec![token(
                factory,
                text,
                range.start,
                range.end,
                Kind::HtmlToken,
            )?])
        }
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
            let task_start =
                marker_end + usize::from(source.as_bytes().get(marker_end) == Some(&b' '));
            if let Some((task_end, _)) = dialect
                .task_lists()
                .then(|| super::gfm::task_marker(source, task_start, range.end))
                .flatten()
            {
                if marker_end < task_start {
                    out.push(token(
                        factory,
                        text,
                        marker_end,
                        task_start,
                        Kind::WhitespaceToken,
                    )?);
                }
                out.push(token(
                    factory,
                    text,
                    task_start,
                    task_end,
                    Kind::TaskListMarkerToken,
                )?);
                if task_end < range.end {
                    out.push(token(factory, text, task_end, range.end, Kind::TextToken)?);
                }
            } else if marker_end < range.end {
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
    dialect: MarkdownDialect,
) -> Result<(), ParseError> {
    if start == end {
        return Ok(());
    }
    if parent == Kind::TableRow {
        let mut at = start;
        while at < end {
            if source.as_bytes()[at] == b'|' {
                out.push(token(factory, text, at, at + 1, Kind::TablePipeToken)?);
                at += 1;
            } else {
                let next = source[at..end]
                    .find('|')
                    .map(|offset| at + offset)
                    .unwrap_or(end);
                out.push(token(factory, text, at, next, Kind::TextToken)?);
                at = next;
            }
        }
        return Ok(());
    }
    if matches!(parent, Kind::TableHead | Kind::Table) {
        let mut at = start;
        while at < end {
            let kind = if source.as_bytes()[at] == b':' {
                Kind::TableAlignmentColonToken
            } else if source.as_bytes()[at] == b'|' {
                Kind::TablePipeToken
            } else {
                Kind::TextToken
            };
            let mut next = at + 1;
            while next < end
                && ((kind == Kind::TableAlignmentColonToken && source.as_bytes()[next] == b':')
                    || (kind == Kind::TablePipeToken && source.as_bytes()[next] == b'|')
                    || (kind == Kind::TextToken && !matches!(source.as_bytes()[next], b':' | b'|')))
            {
                next += 1;
            }
            out.push(token(factory, text, at, next, kind)?);
            at = next;
        }
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
        if parent == Kind::ListItem {
            let task_start =
                marker_end + usize::from(source.as_bytes().get(marker_end) == Some(&b' '));
            if let Some((task_end, _)) = dialect
                .task_lists()
                .then(|| super::gfm::task_marker(source, task_start, end))
                .flatten()
            {
                if marker_end < task_start {
                    out.push(token(
                        factory,
                        text,
                        marker_end,
                        task_start,
                        Kind::WhitespaceToken,
                    )?);
                }
                out.push(token(
                    factory,
                    text,
                    task_start,
                    task_end,
                    Kind::TaskListMarkerToken,
                )?);
                if task_end < end {
                    out.push(token(factory, text, task_end, end, Kind::TextToken)?);
                }
                return Ok(());
            }
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
    semantic_node_with_metadata(factory, kind, children, None)
}

fn semantic_node_with_metadata(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    kind: Kind,
    children: Vec<GreenElement<OkfMarkdownLanguage>>,
    metadata: Option<(&'static str, &'static str)>,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    let identity = SyntaxIdentity::fresh()?;
    let mut annotations = vec![identity.annotation()];
    if let Some((kind, data)) = metadata {
        annotations.push(identity.metadata_annotation(kind, data));
    }
    factory
        .node_with_annotations(kind, children, annotations.into())
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
    let mut at = start;
    while at < end && bytes[at] == b' ' && at - start < 3 {
        at += 1;
    }
    if matches!(bytes.get(at), Some(b'-' | b'+' | b'*')) {
        return (at + 1).min(end);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_phase_returns_owner_identities_and_reference_backlinks() {
        let source = "[x][id]\n\n# *heading*\n\n[id]: /one\n";
        let text = SourceText::new(source).unwrap();
        let parsed = parse(&text, MarkdownDialect::CommonMarkCurrent, 0, source.len()).unwrap();

        assert_eq!(parsed.inline_roots.len(), 2);
        let owners: Vec<_> = parsed
            .inline_roots
            .iter()
            .map(|node| {
                node.annotations()
                    .iter()
                    .find(|annotation| annotation.kind() == "waml.markdown.identity")
                    .and_then(|annotation| annotation.data())
                    .and_then(SyntaxIdentity::from_annotation_data)
                    .unwrap()
            })
            .collect();
        let label = super::super::reference::normalize_label("id").unwrap();
        assert_eq!(parsed.references.backlinks[&label].len(), 1);
        assert!(owners.contains(&parsed.references.backlinks[&label][0]));
    }
}
