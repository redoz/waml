use std::sync::Arc;

use crate::{
    GreenElement, GreenFactory, GreenText, GreenTrivia, MarkdownDialect, OkfMarkdownLanguage,
    OkfMarkdownSyntaxKind, OkfSyntaxDiagnosticCode, SourceText, SyntaxSeverity, SyntaxTree,
    TextRange, TextSize, TreeDiagnostic, TriviaKind,
    shell::{ParseError, ParsedShellWindow, ShellParse, ShellWindow, ShellWindowKind},
};

pub(crate) fn parse(text: SourceText, dialect: MarkdownDialect) -> Result<ShellParse, ParseError> {
    let structure = match crate::markdown::shell_map(&text, dialect) {
        Ok(structure) => Arc::new(structure),
        Err(ParseError::InvalidRange { .. }) => Arc::new(crate::markdown::ShellStructure {
            headings: [].into(),
            nested_headings: [].into(),
            protected_ranges: [].into(),
            list_item_lines: [].into(),
            tab_indented_item_lines: [].into(),
            opaque_ranges: [].into(),
            dialect,
        }),
        Err(error) => return Err(error),
    };
    parse_with_structure(text, dialect, structure)
}

pub(crate) fn parse_with_structure(
    text: SourceText,
    dialect: MarkdownDialect,
    structure: Arc<crate::markdown::ShellStructure>,
) -> Result<ShellParse, ParseError> {
    let factory = GreenFactory::<OkfMarkdownLanguage>::new();
    let source = text.shared();
    let mut children = Vec::new();
    let mut diagnostics = Vec::new();
    let mut at = 0;

    if let Some(class) = classify_frontmatter(&text, &structure)? {
        let (node, end) = frontmatter(&factory, &text, source, class, &mut diagnostics)?;
        children.push(GreenElement::Node(node));
        at = end;
    }
    let eof_trivia_start = trailing_eof_whitespace_start(source, at);
    if at < eof_trivia_start {
        let blocks = crate::markdown::block::parse(&text, dialect, at, eof_trivia_start)?;
        children.extend(crate::markdown::block::wrap_waml_sections(
            &factory,
            source,
            dialect,
            at,
            blocks.root.children().to_vec(),
        )?);
        diagnostics.extend(blocks.diagnostics.iter().cloned());
    }
    let eof_leading = trivia(&factory, &text, eof_trivia_start, source.len())?;
    children.push(GreenElement::Token(
        factory
            .missing_token_with_leading(OkfMarkdownSyntaxKind::EndOfFileToken, eof_leading)
            .map_err(|_| ParseError::WidthOverflow)?,
    ));
    let root = factory
        .node(OkfMarkdownSyntaxKind::Root, children)
        .map_err(|_| ParseError::WidthOverflow)?;
    let tree = Arc::new(SyntaxTree::new(root, diagnostics.into(), dialect));
    let structure = Arc::new(crate::markdown::from_tree(&tree, source)?);
    Ok(ShellParse { tree, structure })
}

#[derive(Clone, Copy)]
struct FrontmatterClass {
    range: TextRange,
    open: Line,
    close: Option<Line>,
    entries_end: usize,
    recovered: bool,
}

pub(crate) fn frontmatter_range(
    text: &SourceText,
    structure: &crate::markdown::ShellStructure,
) -> Result<Option<TextRange>, ParseError> {
    Ok(classify_frontmatter(text, structure)?.map(|class| class.range))
}

fn classify_frontmatter(
    text: &SourceText,
    structure: &crate::markdown::ShellStructure,
) -> Result<Option<FrontmatterClass>, ParseError> {
    if !structure.dialect.waml_frontmatter() {
        return Ok(None);
    }
    let source = text.shared();
    let bom = usize::from(source.starts_with('\u{feff}')) * 3;
    let open = line_at(source, 0, source.len());
    if open.start != 0 || source[bom..open.significant_end] != *"---" {
        return Ok(None);
    }

    let boundary = structure
        .headings
        .iter()
        .find(|heading| heading.level <= 2 && heading.range.start().to_usize() >= open.end)
        .map(|heading| heading.range.start().to_usize())
        .unwrap_or(source.len());
    let close = lines(source, open.end, boundary)
        .map(|(start, end)| line_at(source, start, end))
        .find(|line| is_fence_line(source, *line));
    let recovered = close.is_none();
    let entries_end = close.map_or(boundary, |line| line.start);
    if recovered && !plausible_unclosed_frontmatter(source, open.end, entries_end) {
        return Ok(None);
    }
    let end = close.map_or_else(
        || {
            lines(source, open.end, entries_end)
                .map(|(start, end)| structured_end(line_at(source, start, end)))
                .last()
                .unwrap_or(open.end)
        },
        structured_end,
    );
    Ok(Some(FrontmatterClass {
        range: TextRange::new(size(open.start)?, size(end)?).map_err(|_| {
            ParseError::StructuralInvariant {
                reason: "frontmatter classifier produced a reversed range".into(),
            }
        })?,
        open,
        close,
        entries_end,
        recovered,
    }))
}

fn frontmatter(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    class: FrontmatterClass,
    diagnostics: &mut Vec<TreeDiagnostic<OkfSyntaxDiagnosticCode>>,
) -> Result<(crate::GreenNode<OkfMarkdownLanguage>, usize), ParseError> {
    let FrontmatterClass {
        open,
        close,
        entries_end,
        recovered,
        range,
    } = class;

    let mut children = line_tokens(
        factory,
        text,
        source,
        open,
        OkfMarkdownSyntaxKind::FrontmatterOpenFence,
    )?;
    let mut clean = !recovered;
    let mut entries_consumed_end = open.end;
    for (start, end) in lines(source, open.end, entries_end) {
        let line = line_at(source, start, end);
        entries_consumed_end = structured_end(line);
        let (entry, malformed) = frontmatter_entry(factory, text, source, line)?;
        if malformed {
            clean = false;
            diagnostics.push(diagnostic(
                OkfSyntaxDiagnosticCode::MalformedFrontmatterEntry,
                line.start,
                line.significant_end,
                "malformed frontmatter entry",
            ));
        }
        children.push(GreenElement::Node(entry));
    }
    if let Some(close) = close {
        children.extend(line_tokens(
            factory,
            text,
            source,
            close,
            OkfMarkdownSyntaxKind::FrontmatterCloseFence,
        )?);
    } else {
        children.push(GreenElement::Token(
            factory.missing_token(OkfMarkdownSyntaxKind::FrontmatterCloseFence),
        ));
        diagnostics.push(diagnostic(
            OkfSyntaxDiagnosticCode::MissingFrontmatterFence,
            entries_consumed_end,
            entries_consumed_end,
            "missing frontmatter close fence",
        ));
    }
    if !clean {
        diagnostics.push(diagnostic(
            OkfSyntaxDiagnosticCode::FrontmatterNotClean,
            open.end,
            entries_consumed_end,
            "frontmatter required recovery",
        ));
    }
    Ok((
        factory
            .node(OkfMarkdownSyntaxKind::Frontmatter, children)
            .map_err(|_| ParseError::WidthOverflow)?,
        range.end().to_usize(),
    ))
}

pub(crate) fn parse_window(
    text: &SourceText,
    structure: &crate::markdown::ShellStructure,
    window: ShellWindow,
) -> Result<ParsedShellWindow, ParseError> {
    let source = text.shared();
    let start = window.range.start().to_usize();
    let end = window.range.end().to_usize();
    if start > end || end > source.len() {
        return Err(ParseError::InvalidRange {
            range: window.range,
        });
    }
    let factory = GreenFactory::<OkfMarkdownLanguage>::new();
    let mut diagnostics = Vec::new();
    let elements = match window.kind {
        ShellWindowKind::Frontmatter => {
            let Some(class) = classify_frontmatter(text, structure)? else {
                return Err(window_not_consumed());
            };
            if class.range != window.range {
                return Err(window_not_consumed());
            }
            let (node, _) = frontmatter(&factory, text, source, class, &mut diagnostics)?;
            vec![GreenElement::Node(node)]
        }
        ShellWindowKind::Heading => {
            let mut candidates = structure
                .headings
                .iter()
                .chain(structure.nested_headings.iter())
                .filter(|heading| heading.range == window.range);
            let Some(_) = candidates.next() else {
                return Err(window_not_consumed());
            };
            if candidates.next().is_some() {
                return Err(window_not_consumed());
            }
            let blocks = crate::markdown::block::parse(text, structure.dialect, start, end)?;
            diagnostics.extend(blocks.diagnostics.iter().cloned());
            let elements = blocks.root.children().to_vec();
            if elements.len() != 1
                || !matches!(
                    &elements[0],
                    GreenElement::Node(node)
                        if matches!(
                            node.kind(),
                            OkfMarkdownSyntaxKind::AtxHeading
                                | OkfMarkdownSyntaxKind::SetextHeading
                        )
                )
            {
                return Err(window_not_consumed());
            }
            elements
        }
        ShellWindowKind::MarkdownRegion => {
            let blocks = crate::markdown::block::parse(text, structure.dialect, start, end)?;
            diagnostics.extend(blocks.diagnostics.iter().cloned());
            blocks.root.children().to_vec()
        }
        ShellWindowKind::Tail => {
            if end != source.len() {
                return Err(window_not_consumed());
            }
            let eof_trivia_start = trailing_eof_whitespace_start(source, start);
            let mut elements = Vec::new();
            if start < eof_trivia_start {
                let blocks = crate::markdown::block::parse(
                    text,
                    structure.dialect,
                    start,
                    eof_trivia_start,
                )?;
                diagnostics.extend(blocks.diagnostics.iter().cloned());
                elements.extend(blocks.root.children().iter().cloned());
            }
            let eof_leading = trivia(&factory, text, eof_trivia_start, source.len())?;
            elements.push(GreenElement::Token(
                factory
                    .missing_token_with_leading(OkfMarkdownSyntaxKind::EndOfFileToken, eof_leading)
                    .map_err(|_| ParseError::WidthOverflow)?,
            ));
            elements
        }
    };
    let consumed = elements
        .iter()
        .map(element_width)
        .try_fold(size(0)?, |sum, width| {
            sum.checked_add(width)
                .map_err(|_| ParseError::WidthOverflow)
        })?;
    if consumed != window.range.len() {
        return Err(window_not_consumed());
    }
    Ok(ParsedShellWindow {
        elements: elements.into(),
        diagnostics: diagnostics.into(),
    })
}

fn element_width(element: &GreenElement<OkfMarkdownLanguage>) -> TextSize {
    match element {
        GreenElement::Node(node) => node.width(),
        GreenElement::Token(token) => token.width(),
    }
}

fn window_not_consumed() -> ParseError {
    ParseError::StructuralInvariant {
        reason: "shell window parser did not consume the selected range".into(),
    }
}

fn plausible_unclosed_frontmatter(source: &str, from: usize, to: usize) -> bool {
    let mut entries = 0;
    for (start, end) in lines(source, from, to) {
        let line = line_at(source, start, end);
        if line.start == line.significant_end {
            continue;
        }
        let content = &source[line.start..line.significant_end];
        let Some(colon) = content.find(':') else {
            return false;
        };
        if content[..colon].trim().is_empty() {
            return false;
        }
        entries += 1;
    }
    entries > 0
}

fn frontmatter_entry(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    line: Line,
) -> Result<(crate::GreenNode<OkfMarkdownLanguage>, bool), ParseError> {
    let mut children = Vec::new();
    if line.start == line.significant_end {
        if line.newline_start < line.end {
            children.push(GreenElement::Token(newline_token(factory, text, line)?));
        }
        return Ok((
            factory
                .node(OkfMarkdownSyntaxKind::FrontmatterEntry, children)
                .map_err(|_| ParseError::WidthOverflow)?,
            false,
        ));
    }
    let content = &source[line.start..line.significant_end];
    let Some(relative_colon) = content.find(':') else {
        let text_start = skip_horizontal(source, line.start, line.significant_end);
        let leading = trivia(factory, text, line.start, text_start)?;
        children.push(GreenElement::Token(
            factory
                .bad_token_with_leading(
                    OkfMarkdownSyntaxKind::BadToken,
                    slice(text, text_start, line.significant_end)?,
                    leading,
                    OkfSyntaxDiagnosticCode::MalformedFrontmatterEntry,
                )
                .map_err(|_| ParseError::WidthOverflow)?,
        ));
        if line.newline_start < line.end {
            children.push(GreenElement::Token(newline_token(factory, text, line)?));
        }
        return Ok((
            factory
                .node(OkfMarkdownSyntaxKind::FrontmatterEntry, children)
                .map_err(|_| ParseError::WidthOverflow)?,
            true,
        ));
    };
    let colon = line.start + relative_colon;
    let key_start = skip_horizontal(source, line.start, colon);
    let key_end = trim_horizontal_end(source, key_start, colon);
    if key_start == key_end {
        let leading = trivia(factory, text, line.start, colon)?;
        children.push(GreenElement::Token(
            factory
                .bad_token_with_leading(
                    OkfMarkdownSyntaxKind::BadToken,
                    slice(text, colon, line.significant_end)?,
                    leading,
                    OkfSyntaxDiagnosticCode::MalformedFrontmatterEntry,
                )
                .map_err(|_| ParseError::WidthOverflow)?,
        ));
        if line.newline_start < line.end {
            children.push(GreenElement::Token(newline_token(factory, text, line)?));
        }
        return Ok((
            factory
                .node(OkfMarkdownSyntaxKind::FrontmatterEntry, children)
                .map_err(|_| ParseError::WidthOverflow)?,
            true,
        ));
    }
    children.push(GreenElement::Token(token_with_leading(
        factory,
        text,
        line.start,
        key_start,
        key_end,
        OkfMarkdownSyntaxKind::FrontmatterKey,
    )?));
    children.push(GreenElement::Token(token_with_leading(
        factory,
        text,
        key_end,
        colon,
        colon + 1,
        OkfMarkdownSyntaxKind::ColonToken,
    )?));
    let value_start = skip_horizontal(source, colon + 1, line.significant_end);
    if value_start == line.significant_end {
        children.push(GreenElement::Token(
            factory
                .missing_token_with_leading(
                    OkfMarkdownSyntaxKind::FrontmatterValue,
                    trivia(factory, text, colon + 1, value_start)?,
                )
                .map_err(|_| ParseError::WidthOverflow)?,
        ));
    } else {
        children.push(GreenElement::Token(token_with_leading(
            factory,
            text,
            colon + 1,
            value_start,
            line.significant_end,
            OkfMarkdownSyntaxKind::FrontmatterValue,
        )?));
    }
    if line.newline_start < line.end {
        children.push(GreenElement::Token(newline_token(factory, text, line)?));
    }
    Ok((
        factory
            .node(OkfMarkdownSyntaxKind::FrontmatterEntry, children)
            .map_err(|_| ParseError::WidthOverflow)?,
        false,
    ))
}

fn line_tokens(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    line: Line,
    kind: OkfMarkdownSyntaxKind,
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, ParseError> {
    let token_start = skip_horizontal(source, line.start, line.significant_end);
    let mut children = vec![GreenElement::Token(token_with_leading(
        factory,
        text,
        line.start,
        token_start,
        line.significant_end,
        kind,
    )?)];
    if line.newline_start < line.end {
        children.push(GreenElement::Token(newline_token(factory, text, line)?));
    }
    Ok(children)
}

fn newline_token(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    line: Line,
) -> Result<crate::GreenToken<OkfMarkdownLanguage>, ParseError> {
    token_with_leading(
        factory,
        text,
        line.significant_end,
        line.newline_start,
        line.end,
        OkfMarkdownSyntaxKind::NewlineToken,
    )
}

fn token_with_leading(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    leading_start: usize,
    text_start: usize,
    text_end: usize,
    kind: OkfMarkdownSyntaxKind,
) -> Result<crate::GreenToken<OkfMarkdownLanguage>, ParseError> {
    factory
        .token(
            kind,
            slice(text, text_start, text_end)?,
            trivia(factory, text, leading_start, text_start)?,
            [],
        )
        .map_err(|_| ParseError::WidthOverflow)
}

fn trivia(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    start: usize,
    end: usize,
) -> Result<Vec<GreenTrivia>, ParseError> {
    if start == end {
        return Ok(Vec::new());
    }
    Ok(vec![
        factory
            .trivia(TriviaKind::Whitespace, slice(text, start, end)?)
            .map_err(|_| ParseError::WidthOverflow)?,
    ])
}

fn slice(text: &SourceText, start: usize, end: usize) -> Result<GreenText, ParseError> {
    let range = TextRange::new(size(start)?, size(end)?).map_err(|_| ParseError::InvalidRange {
        range: TextRange::new(size(0).unwrap(), size(0).unwrap()).unwrap(),
    })?;
    text.slice(range).map_err(|error| match error {
        crate::TextError::NonUtf8Boundary { .. } => ParseError::StructuralInvariant {
            reason: "parser produced a non-UTF-8 boundary".into(),
        },
        _ => ParseError::InvalidRange { range },
    })?;
    Ok(GreenText::SourceSlice {
        source: text.clone(),
        range,
    })
}

#[derive(Clone, Copy)]
struct Line {
    start: usize,
    significant_end: usize,
    newline_start: usize,
    end: usize,
}

fn line_at(source: &str, start: usize, cap: usize) -> Line {
    let end = source[start..cap]
        .find('\n')
        .map(|offset| start + offset + 1)
        .unwrap_or(cap);
    let newline_start = if end > start && source.as_bytes()[end - 1] == b'\n' {
        if end > start + 1 && source.as_bytes()[end - 2] == b'\r' {
            end - 2
        } else {
            end - 1
        }
    } else {
        end
    };
    let significant_end = trim_horizontal_end(source, start, newline_start);
    Line {
        start,
        significant_end,
        newline_start,
        end,
    }
}

fn lines(source: &str, from: usize, to: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
    let mut at = from;
    std::iter::from_fn(move || {
        if at >= to {
            return None;
        }
        let start = at;
        let end = source[at..to]
            .find('\n')
            .map(|offset| at + offset + 1)
            .unwrap_or(to);
        at = end;
        Some((start, end))
    })
}

fn is_fence_line(source: &str, line: Line) -> bool {
    matches!(
        source[line.start..line.significant_end].trim(),
        "---" | "..."
    )
}
fn skip_horizontal(source: &str, mut at: usize, end: usize) -> usize {
    while at < end && matches!(source.as_bytes()[at], b' ' | b'\t') {
        at += 1;
    }
    at
}
fn trim_horizontal_end(source: &str, start: usize, mut end: usize) -> usize {
    while end > start && matches!(source.as_bytes()[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    end
}
fn trailing_eof_whitespace_start(source: &str, minimum: usize) -> usize {
    let mut at = source.len();
    while at > minimum && matches!(source.as_bytes()[at - 1], b' ' | b'\t') {
        at -= 1;
    }
    at
}
fn structured_end(line: Line) -> usize {
    if line.newline_start < line.end {
        line.end
    } else {
        line.significant_end
    }
}
fn size(value: usize) -> Result<TextSize, ParseError> {
    TextSize::try_from_usize(value).map_err(|_| ParseError::SourceTooLarge { bytes: value })
}
fn diagnostic(
    code: OkfSyntaxDiagnosticCode,
    start: usize,
    end: usize,
    message: &'static str,
) -> TreeDiagnostic<OkfSyntaxDiagnosticCode> {
    TreeDiagnostic {
        code,
        severity: SyntaxSeverity::Error,
        message: message.into(),
        range: TextRange::new(size(start).unwrap(), size(end).unwrap()).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(value: &str) -> SourceText {
        SourceText::from_shared(Arc::new(value.into())).unwrap()
    }

    fn size(value: usize) -> TextSize {
        super::size(value).unwrap()
    }

    fn range(start: usize, end: usize) -> TextRange {
        TextRange::new(size(start), size(end)).unwrap()
    }

    fn element_width(element: &GreenElement<OkfMarkdownLanguage>) -> TextSize {
        match element {
            GreenElement::Node(node) => node.width(),
            GreenElement::Token(token) => token.width(),
        }
    }

    #[test]
    fn supplied_structure_is_only_an_internal_synchronization_map() {
        let text = source("# Class\n## Attributes\nvalue\n");
        let structure =
            Arc::new(crate::markdown::shell_map(&text, MarkdownDialect::WAML_DEFAULT).unwrap());
        let parsed =
            parse_with_structure(text, MarkdownDialect::WAML_DEFAULT, structure.clone()).unwrap();

        assert_eq!(parsed.structure.islands.len(), 1);
    }

    #[test]
    fn frontmatter_classifier_drives_full_and_window_consumption() {
        let text = source("---\ntype: uml.Class\n---\n# Class\n");
        let structure =
            Arc::new(crate::markdown::shell_map(&text, MarkdownDialect::WAML_DEFAULT).unwrap());
        let frontmatter = frontmatter_range(&text, &structure).unwrap().unwrap();
        let full = parse_with_structure(
            text.clone(),
            MarkdownDialect::WAML_DEFAULT,
            structure.clone(),
        )
        .unwrap();
        let parsed = parse_window(
            &text,
            &structure,
            ShellWindow {
                kind: ShellWindowKind::Frontmatter,
                range: frontmatter,
            },
        )
        .unwrap();

        assert_eq!(full.tree.write_to_string(), text.shared().as_str());
        assert_eq!(parsed.elements.len(), 1);
        assert_eq!(element_width(&parsed.elements[0]), frontmatter.len());
    }

    #[test]
    fn tail_window_reclassifies_raw_text_and_source_backed_eof_trivia_together() {
        let text = source("body   ");
        let structure =
            Arc::new(crate::markdown::shell_map(&text, MarkdownDialect::WAML_DEFAULT).unwrap());
        let parsed = parse_window(
            &text,
            &structure,
            ShellWindow {
                kind: ShellWindowKind::Tail,
                range: range(0, text.shared().len()),
            },
        )
        .unwrap();

        let width = parsed
            .elements
            .iter()
            .map(element_width)
            .try_fold(size(0), |sum, width| sum.checked_add(width))
            .unwrap();
        assert_eq!(width, size(text.shared().len()));
        assert!(matches!(
            parsed.elements.last(),
            Some(GreenElement::Token(token)) if token.kind() == OkfMarkdownSyntaxKind::EndOfFileToken
        ));
    }
}
