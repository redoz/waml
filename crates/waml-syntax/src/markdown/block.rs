use std::{ops::Range, sync::Arc};

use super::inline::find_unescaped;
use super::scan::{scan_blocks, ScanEvent, ScanProfile, ScanTag, ScanTagKind};

use crate::{
    GreenElement, GreenFactory, GreenNode, GreenText, MarkdownDialect, OkfMarkdownLanguage,
    OkfMarkdownSyntaxKind as Kind, OkfSyntaxDiagnosticCode as Diagnostic, ParseError, SourceText,
    SyntaxIdentity, TextRange, TextSize, TreeDiagnostic,
};

pub(crate) struct BlockParse {
    pub root: GreenNode<OkfMarkdownLanguage>,
    pub diagnostics: Arc<[TreeDiagnostic<Diagnostic>]>,
    /// Inline sub-trees, one per block that carried inline content. Asserted by
    /// this file's tests; no reader outside them, since `parse_markdown` hands
    /// callers the assembled `root` rather than the pieces.
    #[allow(dead_code)] // read by this module's block tests
    pub inline_roots: Arc<[GreenNode<OkfMarkdownLanguage>]>,
    /// Link-reference definition ranges and the reference map built from them.
    /// Produced by the inline pass and carried here for the reference-resolving
    /// consumer that has not landed.
    #[allow(dead_code)] // consumer: link-reference resolution
    pub definitions: Arc<[TextRange]>,
    #[allow(dead_code)] // consumer: link-reference resolution
    pub references: super::reference::MarkdownReferenceMap,
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

/// A malformed range emitted by the CommonMark event stream.
///
/// This stays private because it is a recovery signal, not a public parse
/// failure. All other parser failures keep their original `ParseError`.
#[derive(Debug)]
enum BlockBuildError {
    MalformedEventRange,
    Parse(ParseError),
}

impl From<ParseError> for BlockBuildError {
    fn from(error: ParseError) -> Self {
        Self::Parse(error)
    }
}

pub(crate) fn parse(
    text: &SourceText,
    dialect: MarkdownDialect,
    start: usize,
    end: usize,
) -> Result<BlockParse, ParseError> {
    match parse_strict(text, dialect, start, end) {
        Ok(parsed) => Ok(parsed),
        Err(BlockBuildError::MalformedEventRange) => recover_raw_text(text, start, end),
        Err(BlockBuildError::Parse(error)) => Err(error),
    }
}

fn parse_strict(
    text: &SourceText,
    dialect: MarkdownDialect,
    start: usize,
    end: usize,
) -> Result<BlockParse, BlockBuildError> {
    let source = text.shared();
    validate_range(source, start, end)?;
    let event_start = if start == 0 && source.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        start
    };
    let mut stack = Vec::<BlockFrame>::new();
    let mut blocks = Vec::<BlockFrame>::new();
    // Container opens beyond `MD_MAX_CONTAINER_DEPTH` are suppressed, along
    // with everything they contain: no frame is pushed, so the matching close
    // must also be swallowed to keep the stack balanced. Counts every
    // suppressed start still awaiting its close. The suppressed range's bytes
    // still reach the
    // tree via `emit_uncovered`'s raw-text flush, so exact-source recovery
    // still holds.
    let mut suppressed_depth = 0usize;
    let mut suppressed_range: Option<Range<usize>> = None;
    // Counts only the *container* frames on `stack` (see `is_container_kind`).
    // Leaf frames — paragraphs, headings, code blocks — cannot recurse, so
    // letting them consume cap budget would make the effective limit depend
    // on the leaf a document happens to end in.
    let mut container_depth = 0usize;
    // Offsets are relative to the scanned slice; re-base them onto `source`.
    let scan = scan_blocks(&source[event_start..end], dialect, ScanProfile::Tree);
    // The seam filters inline and text events out, so only it can screen their
    // ranges; a malformed one anywhere condemns the whole stream, exactly as
    // when this loop still saw every event itself.
    if scan.malformed_range {
        return Err(BlockBuildError::MalformedEventRange);
    }
    let mut reference_spans = Vec::new();
    let definitions = scan
        .reference_definitions
        .as_deref()
        .expect("ScanProfile::Tree collects reference definitions");
    for definition in definitions {
        let span = (event_start + definition.start)..(event_start + definition.end);
        validate_event_range(source, event_start, end, &span)?;
        reference_spans.push(span);
    }
    reference_spans.sort_by_key(|definition| (definition.start, definition.end));
    for (event, offsets) in scan.events {
        let mut offsets = (event_start + offsets.start)..(event_start + offsets.end);
        validate_event_range(source, event_start, end, &offsets)?;
        match event {
            ScanEvent::Start(tag) => {
                if let Some(kind) = start_kind(&tag, source, &offsets) {
                    // Once a container is suppressed its whole subtree must be
                    // too: a lone `TableHead`/`TableCell` reparented onto the
                    // grandparent loses the alignment metadata its consumers
                    // require, and a lone `ListItem` loses its `List`.
                    if suppressed_depth > 0
                        || (is_container_kind(kind)
                            && container_depth >= super::MD_MAX_CONTAINER_DEPTH)
                    {
                        suppressed_depth += 1;
                        suppressed_range.get_or_insert_with(|| offsets.clone());
                        continue;
                    }
                    container_depth += usize::from(is_container_kind(kind));
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
                        ScanTag::Table { alignments } => alignments
                            .iter()
                            .copied()
                            .map(super::gfm::TableAlignment::from_scan)
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
            ScanEvent::End(kind) => {
                // `end_closes_block` mirrors `start_kind`, so the same
                // predicate that decides a start pushed a frame decides
                // whether its suppressed twin has to be swallowed here.
                if suppressed_depth > 0 && end_closes_block(kind) {
                    suppressed_depth -= 1;
                } else if end_closes_block(kind) {
                    let Some(mut frame) = stack.pop() else {
                        return Err(ParseError::StructuralInvariant {
                            reason: "CommonMark closed a block without an open frame".into(),
                        }
                        .into());
                    };
                    container_depth -= usize::from(is_container_kind(frame.kind));
                    frame.source_range.end = frame.source_range.end.max(offsets.end);
                    frame.cursor = frame.source_range.start;
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(frame);
                    } else {
                        blocks.push(frame);
                    }
                }
            }
            ScanEvent::Rule => {
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
    if let Some(range) = suppressed_range {
        diagnostics.push(diagnostic(
            Diagnostic::NestingDepthExceeded,
            range.start,
            range.end,
            format!(
                "Markdown nesting exceeds the supported depth of {} containers (a nested list level counts as two: the list and its item); deeper structure is treated as plain text.",
                super::MD_MAX_CONTAINER_DEPTH
            ),
        )?);
    }
    let mut definitions = Vec::new();
    let mut children = Vec::new();
    let mut cursor = start;
    for frame in blocks {
        if frame.source_range.start < cursor {
            return Err(BlockBuildError::MalformedEventRange);
        }
        emit_uncovered(
            &factory,
            text,
            source,
            cursor,
            frame.source_range.start,
            &mut children,
            &mut definitions,
            &reference_spans,
        )?;
        cursor = frame.source_range.end;
        let node = build_frame(
            &factory,
            text,
            source,
            frame,
            &mut diagnostics,
            dialect,
            &reference_spans,
            &mut definitions,
        )
        .map_err(classify_block_build_error)?;
        children.push(GreenElement::Node(node));
    }
    emit_uncovered(
        &factory,
        text,
        source,
        cursor,
        end,
        &mut children,
        &mut definitions,
        &reference_spans,
    )?;
    let root = factory
        .node(Kind::Root, children)
        .map_err(|_| ParseError::WidthOverflow)?;
    if root.width().to_usize() != end - start {
        return Err(ParseError::StructuralInvariant {
            reason: "block children do not cover their source range".into(),
        }
        .into());
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

fn classify_block_build_error(error: BlockBuildError) -> BlockBuildError {
    match error {
        BlockBuildError::Parse(ParseError::InvalidRange { .. }) => {
            BlockBuildError::MalformedEventRange
        }
        error => error,
    }
}

fn recover_raw_text(text: &SourceText, start: usize, end: usize) -> Result<BlockParse, ParseError> {
    validate_range(text.shared(), start, end)?;
    let factory = GreenFactory::<OkfMarkdownLanguage>::new();
    let children = if start == end {
        Vec::new()
    } else {
        vec![token(&factory, text, start, end, Kind::RawTextToken)?]
    };
    let root = factory
        .node(Kind::Root, children)
        .map_err(|_| ParseError::WidthOverflow)?;
    let diagnostics = [diagnostic(
        Diagnostic::MalformedBlock,
        start,
        end,
        "recovered Markdown after invalid block-event ranges",
    )?];
    Ok(BlockParse {
        root,
        diagnostics: diagnostics.into(),
        inline_roots: [].into(),
        definitions: [].into(),
        references: super::reference::MarkdownReferenceMap::default(),
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

#[allow(clippy::too_many_arguments)]
fn build_frame(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    mut frame: BlockFrame,
    diagnostics: &mut Vec<TreeDiagnostic<Diagnostic>>,
    dialect: MarkdownDialect,
    reference_spans: &[Range<usize>],
    definitions: &mut Vec<TextRange>,
) -> Result<GreenNode<OkfMarkdownLanguage>, BlockBuildError> {
    validate_event_range(source, 0, source.len(), &frame.source_range)?;
    let metadata = frame_metadata(source, &frame, dialect);
    frame
        .children
        .sort_by_key(|child| (child.source_range.start, child.source_range.end));
    let contains_definition = reference_spans.iter().any(|definition| {
        definition.start >= frame.source_range.start && definition.end <= frame.source_range.end
    });
    if frame.children.is_empty() && !contains_definition {
        let children = leaf_tokens(
            factory,
            text,
            source,
            frame.kind,
            frame.source_range.clone(),
            diagnostics,
            dialect,
        )?;
        return Ok(semantic_node_with_metadata(
            factory, frame.kind, children, metadata,
        )?);
    }
    let mut children = Vec::new();
    frame.cursor = frame.source_range.start;
    for child in frame.children {
        if child.source_range.start < frame.cursor
            || child.source_range.end > frame.source_range.end
        {
            return Err(BlockBuildError::MalformedEventRange);
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
            reference_spans,
            definitions,
        )?;
        frame.cursor = child.source_range.end;
        children.push(GreenElement::Node(build_frame(
            factory,
            text,
            source,
            child,
            diagnostics,
            dialect,
            reference_spans,
            definitions,
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
        reference_spans,
        definitions,
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
    Ok(semantic_node_with_metadata(
        factory, frame.kind, children, metadata,
    )?)
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

/// Container kinds whose nesting an attacker can drive arbitrarily deep
/// (`> > > …`, nested lists, nested tables). Leaf kinds (paragraphs,
/// headings, code blocks) cannot themselves recurse, so only these need the
/// `MD_MAX_CONTAINER_DEPTH` cap.
fn is_container_kind(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::BlockQuote | Kind::List | Kind::ListItem | Kind::Table
    )
}

fn start_kind(tag: &ScanTag, source: &str, range: &Range<usize>) -> Option<Kind> {
    Some(match tag {
        ScanTag::Paragraph => Kind::Paragraph,
        ScanTag::Heading { .. } => {
            if heading_is_setext(source, range) {
                Kind::SetextHeading
            } else {
                Kind::AtxHeading
            }
        }
        ScanTag::BlockQuote => Kind::BlockQuote,
        ScanTag::IndentedCodeBlock => Kind::IndentedCodeBlock,
        ScanTag::FencedCodeBlock => Kind::FencedCodeBlock,
        ScanTag::HtmlBlock => Kind::HtmlBlock,
        ScanTag::List => Kind::List,
        ScanTag::Item => Kind::ListItem,
        ScanTag::Table { .. } => Kind::Table,
        ScanTag::TableHead => Kind::TableHead,
        ScanTag::TableRow => Kind::TableRow,
        ScanTag::TableCell => Kind::TableCell,
        _ => return None,
    })
}

/// Whether this end closes a frame the tree builder opened.
///
/// Must mirror `start_kind`'s `None` cases exactly, or the frame stack unwinds
/// out of step.
fn end_closes_block(kind: ScanTagKind) -> bool {
    matches!(
        kind,
        ScanTagKind::Paragraph
            | ScanTagKind::Heading
            | ScanTagKind::BlockQuote
            | ScanTagKind::IndentedCodeBlock
            | ScanTagKind::FencedCodeBlock
            | ScanTagKind::HtmlBlock
            | ScanTagKind::List
            | ScanTagKind::Item
            | ScanTagKind::Table
            | ScanTagKind::TableHead
            | ScanTagKind::TableRow
            | ScanTagKind::TableCell
    )
}

fn leaf_tokens(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    kind: Kind,
    range: Range<usize>,
    diagnostics: &mut Vec<TreeDiagnostic<Diagnostic>>,
    dialect: MarkdownDialect,
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, BlockBuildError> {
    if kind == Kind::AtxHeading {
        return heading_tokens(factory, text, source, range);
    }
    Ok(match kind {
        Kind::AtxHeading => unreachable!(),
        Kind::SetextHeading => setext_tokens(factory, text, source, range),
        Kind::ThematicBreak => {
            line_marker_tokens(factory, text, source, range, Kind::ThematicBreakToken)
        }
        Kind::IndentedCodeBlock => indented_tokens(factory, text, source, range),
        Kind::FencedCodeBlock => fence_tokens(factory, text, source, range, diagnostics),
        Kind::HtmlBlock => {
            if dialect.tag_filter() {
                for (tag_start, tag_end) in
                    super::gfm::disallowed_html_tag_name_ranges(&source[range.clone()])
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
    }?)
}

#[allow(clippy::too_many_arguments)]
fn emit_frame_gap(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    parent: Kind,
    start: usize,
    end: usize,
    out: &mut Vec<GreenElement<OkfMarkdownLanguage>>,
    dialect: MarkdownDialect,
    reference_spans: &[Range<usize>],
    definitions: &mut Vec<TextRange>,
) -> Result<(), BlockBuildError> {
    if start == end {
        return Ok(());
    }
    if let Some(definition) = reference_spans
        .iter()
        .find(|definition| definition.start >= start && definition.end <= end)
    {
        emit_frame_gap(
            factory,
            text,
            source,
            parent,
            start,
            definition.start,
            out,
            dialect,
            &[],
            definitions,
        )?;
        definitions.push(range(definition.start, definition.end)?);
        out.push(GreenElement::Node(semantic_node(
            factory,
            Kind::LinkReferenceDefinition,
            definition_tokens(factory, text, source, definition.start, definition.end)?,
        )?));
        return emit_frame_gap(
            factory,
            text,
            source,
            parent,
            definition.end,
            end,
            out,
            dialect,
            reference_spans,
            definitions,
        );
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

#[allow(clippy::too_many_arguments)]
fn emit_uncovered(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    mut start: usize,
    end: usize,
    out: &mut Vec<GreenElement<OkfMarkdownLanguage>>,
    definitions: &mut Vec<TextRange>,
    reference_spans: &[Range<usize>],
) -> Result<(), BlockBuildError> {
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
        if let Some(definition) = reference_spans
            .iter()
            .find(|definition| definition.start >= start && definition.start < end)
        {
            if definition.end < definition.start || definition.end > end {
                return Err(BlockBuildError::MalformedEventRange);
            }
            if start < definition.start {
                out.push(token(
                    factory,
                    text,
                    start,
                    definition.start,
                    if source[start..definition.start]
                        .trim_matches([' ', '\t', '\r', '\n'])
                        .is_empty()
                    {
                        Kind::WhitespaceToken
                    } else {
                        Kind::BadToken
                    },
                )?);
                start = definition.start;
                continue;
            }
            let definition_end = definition.end;
            definitions.push(range(start, definition_end)?);
            out.push(GreenElement::Node(semantic_node(
                factory,
                Kind::LinkReferenceDefinition,
                definition_tokens(factory, text, source, start, definition_end)?,
            )?));
            start = definition_end;
            continue;
        }
        let line_end = next_line(source, start, end);
        validate_event_range(source, start, end, &(start..line_end))?;
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
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, BlockBuildError> {
    let content_end = trim_newline(source, range.start, range.end);
    let open_end = range.start
        + source[range.clone()]
            .bytes()
            .take_while(|b| *b == b'#')
            .count();
    let text_start = open_end + usize::from(source.as_bytes().get(open_end) == Some(&b' '));
    if text_start > content_end {
        return Err(BlockBuildError::MalformedEventRange);
    }
    let trimmed = source[text_start..content_end].trim_end_matches(' ');
    let candidate_end = text_start + trimmed.len();
    let closing_start = if source[text_start..candidate_end]
        .bytes()
        .all(|byte| byte == b'#')
    {
        Some(text_start)
    } else {
        source[text_start..candidate_end]
            .rfind(' ')
            .filter(|at| {
                source[text_start + at + 1..candidate_end]
                    .bytes()
                    .all(|b| b == b'#')
            })
            .map(|at| text_start + at + 1)
    };
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
    let content_line_end = trim_newline(source, range.start, underline_start);
    let text_end = range.start
        + source[range.start..content_line_end]
            .trim_end_matches([' ', '\t'])
            .len();
    let mut out = Vec::new();
    if range.start < text_end {
        out.push(token(
            factory,
            text,
            range.start,
            text_end,
            Kind::TextToken,
        )?);
    }
    if text_end < content_line_end {
        out.push(token(
            factory,
            text,
            text_end,
            content_line_end,
            Kind::WhitespaceToken,
        )?);
    }
    if content_line_end < underline_start {
        out.push(token(
            factory,
            text,
            content_line_end,
            underline_start,
            Kind::NewlineToken,
        )?);
    }
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
    let line_start = source[..range.start].rfind('\n').map_or(0, |at| at + 1);
    let container_indent = range.start - line_start;
    let closing_indent_limit = if container_indent <= 3 {
        3
    } else {
        container_indent + 3
    };
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
        if spaces <= closing_indent_limit {
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
    // Not every range that lands here is a scanner-confirmed definition: the
    // by-line fallback recognises one by shape alone, and it cuts at the first
    // newline, so a multi-line title's closing delimiter can sit outside
    // `content_end`. Run to the end of the range instead of failing -- the
    // tokens still have to spell the source back exactly.
    if let Some(close) = source.as_bytes().get(at).copied().and_then(title_close) {
        let title_end = find_unescaped(source, at + 1, content_end, close).unwrap_or(content_end);
        out.push(token(factory, text, at, at + 1, Kind::LinkTitleToken)?);
        out.push(token(factory, text, at + 1, title_end, Kind::TextToken)?);
        at = title_end;
        if title_end < content_end {
            out.push(token(
                factory,
                text,
                title_end,
                title_end + close.len_utf8(),
                Kind::LinkTitleToken,
            )?);
            at = title_end + close.len_utf8();
        }
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
) -> Result<(), BlockBuildError> {
    if event.start > event.end
        || event.start < start
        || event.end > end
        || !source.is_char_boundary(event.start)
        || !source.is_char_boundary(event.end)
    {
        return Err(BlockBuildError::MalformedEventRange);
    }
    Ok(())
}
fn heading_is_setext(source: &str, range: &Range<usize>) -> bool {
    source[range.clone()]
        .trim_end_matches(['\r', '\n'])
        .rsplit_once('\n')
        .map(|(_, line)| line)
        .is_some_and(|line| {
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
/// The delimiter that closes a CommonMark link title opened by `open`.
fn title_close(open: u8) -> Option<char> {
    match open {
        b'"' => Some('"'),
        b'\'' => Some('\''),
        b'(' => Some(')'),
        _ => None,
    }
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
    message: impl Into<Arc<str>>,
) -> Result<TreeDiagnostic<Diagnostic>, ParseError> {
    Ok(TreeDiagnostic {
        code,
        range: range(start, end)?,
        severity: crate::SyntaxSeverity::Error,
        message: message.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One representative tag per kind. The exhaustive match means a new
    /// `ScanTagKind` cannot be added without deciding what it maps to here.
    fn sample_tag(kind: ScanTagKind) -> ScanTag {
        match kind {
            ScanTagKind::Paragraph => ScanTag::Paragraph,
            ScanTagKind::Heading => ScanTag::Heading { level: 1 },
            ScanTagKind::BlockQuote => ScanTag::BlockQuote,
            ScanTagKind::IndentedCodeBlock => ScanTag::IndentedCodeBlock,
            ScanTagKind::FencedCodeBlock => ScanTag::FencedCodeBlock,
            ScanTagKind::HtmlBlock => ScanTag::HtmlBlock,
            ScanTagKind::List => ScanTag::List,
            ScanTagKind::Item => ScanTag::Item,
            ScanTagKind::Table => ScanTag::Table {
                alignments: Vec::new(),
            },
            ScanTagKind::TableHead => ScanTag::TableHead,
            ScanTagKind::TableRow => ScanTag::TableRow,
            ScanTagKind::TableCell => ScanTag::TableCell,
            ScanTagKind::FootnoteDefinition => ScanTag::FootnoteDefinition,
            ScanTagKind::DefinitionList => ScanTag::DefinitionList,
            ScanTagKind::DefinitionListDefinition => ScanTag::DefinitionListDefinition,
        }
    }

    /// The frame stack pushes on `start_kind` and pops on `end_closes_block`.
    /// If a kind ever opens without closing (or the reverse) the stack unwinds
    /// out of step, so the two must agree over the whole vocabulary.
    #[test]
    fn end_closes_block_mirrors_start_kind() {
        for &kind in ScanTagKind::ALL {
            let opens = start_kind(&sample_tag(kind), "", &(0..0)).is_some();
            assert_eq!(
                opens,
                end_closes_block(kind),
                "{kind:?} opens a frame but does not close one (or the reverse)"
            );
        }
    }

    #[test]
    fn malformed_scan_event_range_recovers_as_raw_text() {
        let source = "0\n\r\t\u{0800}";
        let text = SourceText::new(source).unwrap();
        match parse_strict(&text, MarkdownDialect::WAML_DEFAULT, 0, source.len()) {
            Err(BlockBuildError::MalformedEventRange) => {}
            Err(error) => panic!("unexpected strict error: {error:?}"),
            Ok(_) => panic!("strict parser unexpectedly accepted malformed event ranges"),
        }
        let parsed = parse(&text, MarkdownDialect::WAML_DEFAULT, 0, source.len()).unwrap();

        let tree = crate::SyntaxTree::new(
            parsed.root,
            parsed.diagnostics.clone(),
            MarkdownDialect::WAML_DEFAULT,
        );
        assert_eq!(tree.write_to_string(), source);
        assert!(parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == Diagnostic::MalformedBlock));
        assert!(parsed.inline_roots.is_empty());
        assert!(parsed.references.definitions.is_empty());
    }

    /// A hostile 10,000-deep blockquote must not overflow the stack (the
    /// tree builder is iterative), must still yield an exact-source tree,
    /// and must record exactly one nesting-depth diagnostic.
    #[test]
    fn deeply_nested_block_quote_is_capped_with_one_diagnostic() {
        let source = "> ".repeat(10_000) + "leaf\n";
        let text = SourceText::new(&source).unwrap();
        let parsed = parse(&text, MarkdownDialect::WAML_DEFAULT, 0, source.len()).unwrap();

        let tree = crate::SyntaxTree::new(
            parsed.root,
            parsed.diagnostics.clone(),
            MarkdownDialect::WAML_DEFAULT,
        );
        assert_eq!(tree.write_to_string(), source);
        assert_eq!(
            parsed
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == Diagnostic::NestingDepthExceeded)
                .count(),
            1
        );
    }

    /// A document nested to exactly one below the cap must be unaffected:
    /// no diagnostic, and every level materializes as real tree structure.
    #[test]
    fn block_quote_below_cap_is_unaffected() {
        let depth = super::super::MD_MAX_CONTAINER_DEPTH - 1;
        let source = "> ".repeat(depth) + "leaf\n";
        let text = SourceText::new(&source).unwrap();
        let parsed = parse(&text, MarkdownDialect::WAML_DEFAULT, 0, source.len()).unwrap();

        assert!(!parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == Diagnostic::NestingDepthExceeded));

        fn count_block_quotes(node: &crate::GreenNode<OkfMarkdownLanguage>) -> usize {
            let mut count = usize::from(node.kind() == Kind::BlockQuote);
            for child in node.children() {
                if let GreenElement::Node(child) = child {
                    count += count_block_quotes(child);
                }
            }
            count
        }
        assert_eq!(count_block_quotes(&parsed.root), depth);
    }

    /// A leaf block sitting under the containers must not consume cap budget:
    /// a block quote nested to one below the cap still materializes every
    /// level even though its paragraph adds a frame to `stack`.
    #[test]
    fn leaf_frames_do_not_consume_container_cap_budget() {
        let depth = super::super::MD_MAX_CONTAINER_DEPTH;
        let source = "> ".repeat(depth) + "leaf\n";
        let text = SourceText::new(&source).unwrap();
        let parsed = parse(&text, MarkdownDialect::WAML_DEFAULT, 0, source.len()).unwrap();

        assert!(!parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == Diagnostic::NestingDepthExceeded));
    }

    fn nested_bullet_list(levels: usize) -> String {
        (0..levels)
            .map(|level| format!("{}- x\n", "  ".repeat(level)))
            .collect()
    }

    fn count_kind(node: &crate::GreenNode<OkfMarkdownLanguage>, kind: Kind) -> usize {
        let mut count = usize::from(node.kind() == kind);
        for child in node.children() {
            if let GreenElement::Node(child) = child {
                count += count_kind(child, kind);
            }
        }
        count
    }

    /// A nested bullet list costs two container frames per visual level
    /// (`List` + `ListItem`), so the cap bites at roughly half the block-quote
    /// depth. Deep nesting must still round-trip exactly and record exactly
    /// one diagnostic.
    #[test]
    fn deeply_nested_list_is_capped_with_one_diagnostic() {
        let source = nested_bullet_list(super::super::MD_MAX_CONTAINER_DEPTH * 4);
        let text = SourceText::new(&source).unwrap();
        let parsed = parse(&text, MarkdownDialect::WAML_DEFAULT, 0, source.len()).unwrap();

        let tree = crate::SyntaxTree::new(
            parsed.root,
            parsed.diagnostics.clone(),
            MarkdownDialect::WAML_DEFAULT,
        );
        assert_eq!(tree.write_to_string(), source);
        assert_eq!(
            parsed
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == Diagnostic::NestingDepthExceeded)
                .count(),
            1
        );
    }

    /// The last list level that fits (`MD_MAX_CONTAINER_DEPTH / 2`, two
    /// frames each) must be untouched, and every level must materialize.
    #[test]
    fn nested_list_at_half_the_cap_is_unaffected() {
        let levels = super::super::MD_MAX_CONTAINER_DEPTH / 2;
        let source = nested_bullet_list(levels);
        let text = SourceText::new(&source).unwrap();
        let parsed = parse(&text, MarkdownDialect::WAML_DEFAULT, 0, source.len()).unwrap();

        assert!(!parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == Diagnostic::NestingDepthExceeded));
        assert_eq!(count_kind(&parsed.root, Kind::ListItem), levels);
    }

    /// Suppressing a `Table` start leaves its `TableHead`/`TableRow`/
    /// `TableCell` frames to attach under the grandparent without alignment
    /// metadata. That degraded shape is deliberate, but the stack must stay
    /// balanced, the source must round-trip exactly, and exactly one
    /// diagnostic must be recorded.
    #[test]
    fn suppressed_table_start_keeps_the_stack_balanced() {
        let prefix = "> ".repeat(super::super::MD_MAX_CONTAINER_DEPTH);
        let source = format!("{prefix}| a | b |\n{prefix}| --- | ---: |\n{prefix}| 1 | 2 |\n");
        let text = SourceText::new(&source).unwrap();
        let parsed = parse(&text, MarkdownDialect::WAML_DEFAULT, 0, source.len()).unwrap();

        let root = parsed.root.clone();
        let tree = crate::SyntaxTree::new(
            parsed.root,
            parsed.diagnostics.clone(),
            MarkdownDialect::WAML_DEFAULT,
        );
        assert_eq!(tree.write_to_string(), source);
        assert_eq!(count_kind(&root, Kind::Table), 0);
        assert_eq!(
            parsed
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == Diagnostic::NestingDepthExceeded)
                .count(),
            1
        );
        // No `TableCell` may carry alignment metadata harvested from a
        // suppressed `Table`: the lookup finds no table frame at all.
        fn no_orphan_alignment(node: &crate::GreenNode<OkfMarkdownLanguage>) -> bool {
            let ok = node.kind() != Kind::TableCell
                || !node
                    .annotations()
                    .iter()
                    .any(|annotation| annotation.kind() == super::super::gfm::TABLE_ALIGNMENT);
            ok && node.children().iter().all(|child| match child {
                GreenElement::Node(child) => no_orphan_alignment(child),
                GreenElement::Token(_) => true,
            })
        }
        assert!(no_orphan_alignment(&root));
    }

    #[test]
    fn inline_phase_returns_owner_identities_and_reference_backlinks() {
        let source = "[x][id]\n\n# *heading*\n\n[id]: /one\n";
        let text = SourceText::new(source).unwrap();
        let parsed = parse(&text, MarkdownDialect::WAML_DEFAULT, 0, source.len()).unwrap();

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
