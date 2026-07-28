use std::sync::Arc;

use super::*;
use crate::{
    GreenElement, GreenFactory, GreenText, GreenTrivia, MarkdownDialect, SourceText,
    SyntaxSeverity, SyntaxTree, TextRange, TextSize, TreeDiagnostic, TriviaKind,
};

pub(super) fn parse(text: SourceText, dialect: MarkdownDialect) -> Result<ShellParse, ParseError> {
    let structure = Arc::new(crate::markdown::map(&text, dialect)?);
    let factory = GreenFactory::<OkfMarkdownLanguage>::new();
    let source = text.shared();
    let mut children = Vec::new();
    let mut diagnostics = Vec::new();
    let mut at = 0;

    if let Some((node, end)) = frontmatter(&factory, &text, source, &structure, &mut diagnostics)? {
        children.push(GreenElement::Node(node));
        at = end;
    }
    for heading in structure.headings.iter() {
        let start = heading.range.start().to_usize();
        if start < at {
            continue;
        }
        if at < start {
            children.push(raw(&factory, &text, at, start)?);
        }
        let line = line_at(source, start, source.len());
        children.push(GreenElement::Node(heading_node(
            &factory, &text, source, line,
        )?));
        at = line.end;
    }

    let eof_trivia_start = trailing_eof_whitespace_start(source, at);
    if at < eof_trivia_start {
        children.push(raw(&factory, &text, at, eof_trivia_start)?);
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
    Ok(ShellParse {
        tree: Arc::new(SyntaxTree::new(root, diagnostics.into(), dialect)),
        structure,
    })
}

fn frontmatter(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    structure: &crate::MarkdownStructureMap,
    diagnostics: &mut Vec<TreeDiagnostic<OkfSyntaxDiagnosticCode>>,
) -> Result<Option<(crate::GreenNode<OkfMarkdownLanguage>, usize)>, ParseError> {
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

    let mut children = line_tokens(
        factory,
        text,
        source,
        open,
        OkfMarkdownSyntaxKind::FrontmatterOpenFence,
    )?;
    let mut clean = !recovered;
    for (start, end) in lines(source, open.end, entries_end) {
        let line = line_at(source, start, end);
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
    let end = if let Some(close) = close {
        children.extend(line_tokens(
            factory,
            text,
            source,
            close,
            OkfMarkdownSyntaxKind::FrontmatterCloseFence,
        )?);
        close.end
    } else {
        children.push(GreenElement::Token(
            factory.missing_token(OkfMarkdownSyntaxKind::FrontmatterCloseFence),
        ));
        diagnostics.push(diagnostic(
            OkfSyntaxDiagnosticCode::MissingFrontmatterFence,
            boundary,
            boundary,
            "missing frontmatter close fence",
        ));
        boundary
    };
    if !clean {
        diagnostics.push(diagnostic(
            OkfSyntaxDiagnosticCode::FrontmatterNotClean,
            open.end,
            entries_end,
            "frontmatter required recovery",
        ));
    }
    Ok(Some((
        factory
            .node(OkfMarkdownSyntaxKind::Frontmatter, children)
            .map_err(|_| ParseError::WidthOverflow)?,
        end,
    )))
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

fn heading_node(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    line: Line,
) -> Result<crate::GreenNode<OkfMarkdownLanguage>, ParseError> {
    let marker_start = skip_horizontal(source, line.start, line.significant_end);
    let mut marker_end = marker_start;
    while marker_end < line.significant_end && source.as_bytes()[marker_end] == b'#' {
        marker_end += 1;
    }
    let heading_start = skip_horizontal(source, marker_end, line.significant_end);
    let mut children = vec![GreenElement::Token(token_with_leading(
        factory,
        text,
        line.start,
        marker_start,
        marker_end,
        OkfMarkdownSyntaxKind::HeadingMarkerToken,
    )?)];
    if heading_start == line.significant_end {
        children.push(GreenElement::Token(
            factory
                .missing_token_with_leading(
                    OkfMarkdownSyntaxKind::HeadingText,
                    trivia(factory, text, marker_end, heading_start)?,
                )
                .map_err(|_| ParseError::WidthOverflow)?,
        ));
    } else {
        children.push(GreenElement::Token(token_with_leading(
            factory,
            text,
            marker_end,
            heading_start,
            line.significant_end,
            OkfMarkdownSyntaxKind::HeadingText,
        )?));
    }
    if line.newline_start < line.end {
        children.push(GreenElement::Token(newline_token(factory, text, line)?));
    }
    factory
        .node(OkfMarkdownSyntaxKind::Heading, children)
        .map_err(|_| ParseError::WidthOverflow)
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
    Ok(vec![factory
        .trivia(TriviaKind::Whitespace, slice(text, start, end)?)
        .map_err(|_| ParseError::WidthOverflow)?])
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

fn raw(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    start: usize,
    end: usize,
) -> Result<GreenElement<OkfMarkdownLanguage>, ParseError> {
    Ok(GreenElement::Node(
        factory
            .node(
                OkfMarkdownSyntaxKind::MarkdownRegion,
                [GreenElement::Token(token_with_leading(
                    factory,
                    text,
                    start,
                    start,
                    end,
                    OkfMarkdownSyntaxKind::RawTextToken,
                )?)],
            )
            .map_err(|_| ParseError::WidthOverflow)?,
    ))
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
