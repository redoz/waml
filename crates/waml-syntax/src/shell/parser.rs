use std::sync::Arc;

use super::*;
use crate::{
    GreenElement, GreenFactory, GreenText, MarkdownDialect, SourceText, SyntaxSeverity, SyntaxTree,
    TextRange, TextSize, TreeDiagnostic,
};

pub(super) fn parse(text: SourceText, dialect: MarkdownDialect) -> Result<ShellParse, ParseError> {
    let structure = Arc::new(crate::markdown::map(&text, dialect)?);
    let factory = GreenFactory::<OkfMarkdownLanguage>::new();
    let source = text.shared();
    let mut children = Vec::new();
    let mut diagnostics = Vec::new();
    let mut at = 0usize;
    if let Some(frontmatter) = frontmatter(&factory, &text, source, &structure, &mut diagnostics)? {
        at = frontmatter.1;
        children.push(GreenElement::Node(frontmatter.0));
    }
    for heading in structure.headings.iter() {
        let start = heading.range.start().to_usize();
        let end = heading.range.end().to_usize();
        if start < at {
            continue;
        }
        if at < start {
            children.push(raw(&factory, &text, at, start)?);
        }
        let marker_end = heading.text_range.start().to_usize();
        children.push(GreenElement::Node(
            factory
                .node(
                    OkfMarkdownSyntaxKind::Heading,
                    [
                        GreenElement::Token(token(
                            &factory,
                            &text,
                            start,
                            marker_end,
                            OkfMarkdownSyntaxKind::HeadingMarkerToken,
                        )?),
                        GreenElement::Token(token(
                            &factory,
                            &text,
                            marker_end,
                            end,
                            OkfMarkdownSyntaxKind::HeadingText,
                        )?),
                    ],
                )
                .map_err(|_| ParseError::WidthOverflow)?,
        ));
        at = end;
    }
    if at < source.len() {
        children.push(raw(&factory, &text, at, source.len())?);
    }
    // Empty EOF token is intentional: EOF owns no authored byte, and no parser
    // token has trailing trivia.
    children.push(GreenElement::Token(
        factory.missing_token(OkfMarkdownSyntaxKind::EndOfFileToken),
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
    f: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    structure: &crate::MarkdownStructureMap,
    diagnostics: &mut Vec<TreeDiagnostic<OkfSyntaxDiagnosticCode>>,
) -> Result<Option<(crate::GreenNode<OkfMarkdownLanguage>, usize)>, ParseError> {
    let bom = if source.starts_with('\u{feff}') { 3 } else { 0 };
    let Some(open_end) = fence_end(source, bom) else {
        return Ok(None);
    };
    let first_heading = structure
        .headings
        .iter()
        .find(|h| h.level == 1)
        .map(|h| h.range.start().to_usize())
        .unwrap_or(source.len());
    let scan_end = first_heading.max(open_end);
    let close_start = lines(source, open_end, scan_end)
        .find_map(|(start, end)| is_fence(&source[start..end]).then_some((start, end)));
    let (close_start, close_end, recovered) = match close_start {
        Some(pair) => (pair.0, pair.1, false),
        None => (scan_end, scan_end, true),
    };
    // A thematic rule is not frontmatter merely because it is at byte zero.
    // For unclosed input require at least one flat key/value candidate before
    // the synchronization heading; otherwise preserve the whole document raw.
    if recovered
        && !lines(source, open_end, close_start).any(|(start, end)| {
            source[start..end]
                .trim_end_matches(['\r', '\n'])
                .contains(':')
        })
    {
        return Ok(None);
    }
    let mut kids = vec![GreenElement::Token(token(
        f,
        text,
        0,
        open_end,
        OkfMarkdownSyntaxKind::FrontmatterOpenFence,
    )?)];
    let mut clean = !recovered;
    for (start, end) in lines(source, open_end, close_start) {
        let line = &source[start..end];
        let content_end = line.trim_end_matches(['\r', '\n']).len() + start;
        if content_end == start {
            kids.push(GreenElement::Node(
                f.node(
                    OkfMarkdownSyntaxKind::FrontmatterEntry,
                    [GreenElement::Token(token(
                        f,
                        text,
                        start,
                        end,
                        OkfMarkdownSyntaxKind::NewlineToken,
                    )?)],
                )
                .map_err(|_| ParseError::WidthOverflow)?,
            ));
            continue;
        }
        let Some(colon) = source[start..content_end].find(':').map(|p| start + p) else {
            clean = false;
            kids.push(GreenElement::Node(
                f.node(
                    OkfMarkdownSyntaxKind::FrontmatterEntry,
                    [GreenElement::Token(bad(
                        f,
                        text,
                        start,
                        end,
                        OkfSyntaxDiagnosticCode::MalformedFrontmatterEntry,
                    )?)],
                )
                .map_err(|_| ParseError::WidthOverflow)?,
            ));
            continue;
        };
        kids.push(GreenElement::Node(
            f.node(
                OkfMarkdownSyntaxKind::FrontmatterEntry,
                [
                    GreenElement::Token(token(
                        f,
                        text,
                        start,
                        colon,
                        OkfMarkdownSyntaxKind::FrontmatterKey,
                    )?),
                    GreenElement::Token(token(
                        f,
                        text,
                        colon,
                        colon + 1,
                        OkfMarkdownSyntaxKind::ColonToken,
                    )?),
                    GreenElement::Token(token(
                        f,
                        text,
                        colon + 1,
                        end,
                        OkfMarkdownSyntaxKind::FrontmatterValue,
                    )?),
                ],
            )
            .map_err(|_| ParseError::WidthOverflow)?,
        ));
    }
    if recovered {
        kids.push(GreenElement::Token(
            f.missing_token(OkfMarkdownSyntaxKind::FrontmatterCloseFence),
        ));
        diagnostics.push(diag(
            OkfSyntaxDiagnosticCode::MissingFrontmatterFence,
            close_start,
            close_start,
        ));
    } else {
        kids.push(GreenElement::Token(token(
            f,
            text,
            close_start,
            close_end,
            OkfMarkdownSyntaxKind::FrontmatterCloseFence,
        )?));
    }
    if !clean {
        diagnostics.push(diag(
            OkfSyntaxDiagnosticCode::FrontmatterNotClean,
            open_end,
            close_start,
        ));
    }
    Ok(Some((
        f.node(OkfMarkdownSyntaxKind::Frontmatter, kids)
            .map_err(|_| ParseError::WidthOverflow)?,
        if recovered { close_start } else { close_end },
    )))
}

fn fence_end(source: &str, at: usize) -> Option<usize> {
    lines(source, at, source.len())
        .next()
        .and_then(|(start, end)| (start == at && is_fence(&source[start..end])).then_some(end))
}
fn is_fence(line: &str) -> bool {
    matches!(line.trim_end_matches(['\r', '\n']), "---" | "...")
}
fn lines(source: &str, from: usize, to: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
    let mut at = from;
    std::iter::from_fn(move || {
        if at >= to {
            return None;
        }
        let start = at;
        while at < to && source.as_bytes()[at] != b'\n' {
            at += 1;
        }
        if at < to {
            at += 1;
        }
        Some((start, at))
    })
}
fn diag(
    code: OkfSyntaxDiagnosticCode,
    start: usize,
    end: usize,
) -> TreeDiagnostic<OkfSyntaxDiagnosticCode> {
    TreeDiagnostic {
        code,
        severity: SyntaxSeverity::Error,
        message: Arc::from("recovered malformed frontmatter"),
        range: TextRange::new(
            TextSize::try_from_usize(start).unwrap(),
            TextSize::try_from_usize(end).unwrap(),
        )
        .unwrap(),
    }
}
fn raw(
    f: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    start: usize,
    end: usize,
) -> Result<GreenElement<OkfMarkdownLanguage>, ParseError> {
    Ok(GreenElement::Node(
        f.node(
            OkfMarkdownSyntaxKind::MarkdownRegion,
            [GreenElement::Token(token(
                f,
                text,
                start,
                end,
                OkfMarkdownSyntaxKind::RawTextToken,
            )?)],
        )
        .map_err(|_| ParseError::WidthOverflow)?,
    ))
}
fn token(
    f: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    start: usize,
    end: usize,
    kind: OkfMarkdownSyntaxKind,
) -> Result<crate::GreenToken<OkfMarkdownLanguage>, ParseError> {
    let range = TextRange::new(
        TextSize::try_from_usize(start).map_err(|_| ParseError::SourceTooLarge { bytes: start })?,
        TextSize::try_from_usize(end).map_err(|_| ParseError::SourceTooLarge { bytes: end })?,
    )
    .map_err(|_| ParseError::StructuralInvariant {
        reason: "reversed token range".into(),
    })?;
    f.token(
        kind,
        GreenText::SourceSlice {
            source: text.clone(),
            range,
        },
        [],
        [],
    )
    .map_err(|_| ParseError::WidthOverflow)
}
fn bad(
    f: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    start: usize,
    end: usize,
    code: OkfSyntaxDiagnosticCode,
) -> Result<crate::GreenToken<OkfMarkdownLanguage>, ParseError> {
    let range = TextRange::new(
        TextSize::try_from_usize(start).unwrap(),
        TextSize::try_from_usize(end).unwrap(),
    )
    .unwrap();
    f.bad_token(
        OkfMarkdownSyntaxKind::BadToken,
        GreenText::SourceSlice {
            source: text.clone(),
            range,
        },
        code,
    )
    .map_err(|_| ParseError::WidthOverflow)
}
