use std::sync::Arc;

use super::{UmlLanguage, UmlSyntaxDiagnosticCode, UmlSyntaxKind};
use waml_syntax::{
    GreenElement, GreenFactory, GreenText, MarkdownStructureMap, SourceText, SyntaxSeverity,
    SyntaxTree, TextRange, TextSize, TreeDiagnostic, TriviaKind,
};

pub fn parse(text: SourceText, structure: &MarkdownStructureMap) -> Arc<SyntaxTree<UmlLanguage>> {
    let factory = GreenFactory::<UmlLanguage>::new();
    let source = text.shared();
    let mut children = Vec::new();
    let mut diagnostics = Vec::new();
    let mut at = 0;
    for (index, heading) in structure.headings.iter().enumerate() {
        if heading.level != 2 || !is_attributes_heading(source, heading.text_range) {
            continue;
        }
        let start = heading.range.start().to_usize();
        let end = structure
            .headings
            .get(index + 1)
            .map(|next| next.range.start().to_usize())
            .unwrap_or(source.len());
        if at < start {
            children.push(raw(&factory, &text, at, start));
        }
        let heading_end = line_end(source, start, end);
        let mut section = vec![raw(&factory, &text, start, heading_end)];
        for (line_start, line_end) in lines_between(source, heading_end, end) {
            if protected_non_list_line(structure, source, line_start) {
                section.push(raw(&factory, &text, line_start, line_end));
            } else if let Some(attribute) = attribute(
                &factory,
                &text,
                source,
                line_start,
                line_end,
                &mut diagnostics,
            ) {
                section.push(GreenElement::Node(attribute));
            } else {
                section.push(raw(&factory, &text, line_start, line_end));
            }
        }
        children.push(GreenElement::Node(
            factory
                .node(UmlSyntaxKind::AttributesSection, section)
                .unwrap(),
        ));
        at = end;
    }
    if at < source.len() {
        children.push(raw(&factory, &text, at, source.len()));
    }
    children.push(GreenElement::Token(
        factory.missing_token(UmlSyntaxKind::EndOfFileToken),
    ));
    let root = factory.node(UmlSyntaxKind::Root, children).unwrap();
    Arc::new(SyntaxTree::new(root, diagnostics.into(), structure.dialect))
}

fn attribute(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> Option<waml_syntax::GreenNode<UmlLanguage>> {
    let line = &source[start..end];
    let newline = line.find('\n').map(|i| start + i).unwrap_or(end);
    let sig = source[start..newline].trim_end_matches(['\r', ' ', '\t']);
    let content_end = start + sig.len();
    let lead = start + line[..newline - start].len()
        - line[..newline - start]
            .trim_start_matches([' ', '\t'])
            .len();
    if !source[lead..content_end].starts_with('-') {
        return None;
    }
    let mut c = Vec::new();
    c.push(token(
        f,
        text,
        start,
        lead,
        lead + 1,
        UmlSyntaxKind::BulletToken,
    ));
    let mut p = lead + 1;
    let after_bullet = p;
    p = skip_ws(source, p, content_end);
    let mut vis = None;
    if p < content_end {
        if crate::model::Visibility::from_marker(source[p..].chars().next().unwrap()).is_some() {
            vis = Some(p);
            p += 1;
        }
    }
    if let Some(v) = vis {
        c.push(token(
            f,
            text,
            lead + 1,
            v,
            v + 1,
            UmlSyntaxKind::VisibilityToken,
        ));
    }
    let name_leading = if vis.is_some() { p } else { after_bullet };
    let name_start = skip_ws(source, p, content_end);
    let name_end = scan_name(source, name_start, content_end);
    c.push(if name_start == name_end {
        missing_token(
            f,
            text,
            name_leading,
            name_start,
            UmlSyntaxKind::IdentifierToken,
        )
    } else {
        token(
            f,
            text,
            name_leading,
            name_start,
            name_end,
            UmlSyntaxKind::IdentifierToken,
        )
    });
    p = name_end;
    let colon = source[p..content_end]
        .find(':')
        .map(|i| p + i)
        .filter(|i| *i == p || source[p..*i].trim().is_empty());
    if let Some(colon) = colon {
        c.push(token(
            f,
            text,
            p,
            colon,
            colon + 1,
            UmlSyntaxKind::ColonToken,
        ));
        p = colon + 1;
    } else {
        c.push(GreenElement::Token(
            f.missing_token(UmlSyntaxKind::ColonToken),
        ));
        diags.push(diag(
            UmlSyntaxDiagnosticCode::MissingColon,
            start,
            content_end,
            "missing ':' in attribute",
        ));
    }
    let type_start = skip_ws(source, p, content_end);
    let type_end = source[type_start..content_end]
        .find('[')
        .map(|i| type_start + i)
        .unwrap_or(content_end)
        .trim_end_matches_index(source, type_start);
    if type_start < type_end {
        let ty = f
            .node(
                UmlSyntaxKind::TypeReference,
                [token(
                    f,
                    text,
                    p,
                    type_start,
                    type_end,
                    UmlSyntaxKind::TypeToken,
                )],
            )
            .unwrap();
        c.push(GreenElement::Node(ty));
        p = type_end;
    } else if colon.is_some() {
        diags.push(diag(
            UmlSyntaxDiagnosticCode::MissingType,
            start,
            content_end,
            "missing attribute type",
        ));
    }
    let mstart = skip_ws(source, p, content_end);
    if mstart < content_end && source[mstart..].starts_with('[') {
        if let Some(close) = source[mstart + 1..content_end]
            .find(']')
            .map(|i| mstart + 1 + i)
        {
            let value = &source[mstart + 1..close];
            let valid = crate::multiplicity::Multiplicity::parse(value).is_some();
            let mc = vec![
                token(
                    f,
                    text,
                    p,
                    mstart,
                    mstart + 1,
                    UmlSyntaxKind::OpenBracketToken,
                ),
                token(
                    f,
                    text,
                    mstart + 1,
                    mstart + 1,
                    close,
                    UmlSyntaxKind::IdentifierToken,
                ),
                token(
                    f,
                    text,
                    close,
                    close,
                    close + 1,
                    UmlSyntaxKind::CloseBracketToken,
                ),
            ];
            if !valid {
                diags.push(diag(
                    UmlSyntaxDiagnosticCode::InvalidMultiplicity,
                    mstart,
                    close + 1,
                    "invalid multiplicity",
                ));
            }
            c.push(GreenElement::Node(
                f.node(UmlSyntaxKind::Multiplicity, mc).unwrap(),
            ));
            p = close + 1;
        } else {
            let value_start = mstart + 1;
            c.push(GreenElement::Node(
                f.node(
                    UmlSyntaxKind::Multiplicity,
                    [
                        token(
                            f,
                            text,
                            p,
                            mstart,
                            mstart + 1,
                            UmlSyntaxKind::OpenBracketToken,
                        ),
                        token(
                            f,
                            text,
                            value_start,
                            value_start,
                            content_end,
                            UmlSyntaxKind::IdentifierToken,
                        ),
                        GreenElement::Token(f.missing_token(UmlSyntaxKind::CloseBracketToken)),
                    ],
                )
                .unwrap(),
            ));
            diags.push(diag(
                UmlSyntaxDiagnosticCode::InvalidMultiplicity,
                mstart,
                content_end,
                "unterminated multiplicity",
            ));
            p = content_end;
        }
    }
    if p < content_end {
        c.push(GreenElement::Node(
            f.node(
                UmlSyntaxKind::SkippedTokensSyntax,
                [GreenElement::Token(
                    f.bad_token(
                        UmlSyntaxKind::BadToken,
                        slice(text, p, content_end),
                        UmlSyntaxDiagnosticCode::UnexpectedToken,
                    )
                    .unwrap(),
                )],
            )
            .unwrap(),
        ));
        diags.push(diag(
            UmlSyntaxDiagnosticCode::UnexpectedToken,
            p,
            content_end,
            "unexpected attribute content",
        ));
    }
    if content_end < end {
        c.push(token(
            f,
            text,
            content_end,
            content_end,
            end,
            UmlSyntaxKind::NewlineToken,
        ));
    }
    Some(f.node(UmlSyntaxKind::Attribute, c).unwrap())
}
fn raw(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    start: usize,
    end: usize,
) -> GreenElement<UmlLanguage> {
    GreenElement::Node(
        f.node(
            UmlSyntaxKind::MarkdownRegion,
            [token(
                f,
                text,
                start,
                start,
                end,
                UmlSyntaxKind::RawMarkdownToken,
            )],
        )
        .unwrap(),
    )
}
fn token(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    leading: usize,
    start: usize,
    end: usize,
    kind: UmlSyntaxKind,
) -> GreenElement<UmlLanguage> {
    let trivia = if leading < start {
        vec![f
            .trivia(TriviaKind::Whitespace, slice(text, leading, start))
            .unwrap()]
    } else {
        vec![]
    };
    GreenElement::Token(f.token(kind, slice(text, start, end), trivia, []).unwrap())
}
fn missing_token(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    leading: usize,
    start: usize,
    kind: UmlSyntaxKind,
) -> GreenElement<UmlLanguage> {
    let trivia = if leading < start {
        vec![f
            .trivia(TriviaKind::Whitespace, slice(text, leading, start))
            .unwrap()]
    } else {
        vec![]
    };
    GreenElement::Token(f.missing_token_with_leading(kind, trivia).unwrap())
}
fn slice(text: &SourceText, start: usize, end: usize) -> GreenText {
    GreenText::SourceSlice {
        source: text.clone(),
        range: TextRange::new(
            TextSize::try_from_usize(start).unwrap(),
            TextSize::try_from_usize(end).unwrap(),
        )
        .unwrap(),
    }
}
fn lines_between(s: &str, from: usize, to: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
    let mut p = from;
    std::iter::from_fn(move || {
        if p >= to {
            return None;
        }
        let a = p;
        p = s[p..to].find('\n').map(|n| p + n + 1).unwrap_or(to);
        Some((a, p))
    })
}
fn line_end(source: &str, start: usize, limit: usize) -> usize {
    source[start..limit]
        .find('\n')
        .map(|offset| start + offset + 1)
        .unwrap_or(limit)
}
fn is_attributes_heading(source: &str, range: TextRange) -> bool {
    source[range.start().to_usize()..range.end().to_usize()]
        .trim()
        .trim_end_matches('#')
        .trim()
        .eq_ignore_ascii_case("Attributes")
}
fn protected_non_list_line(
    structure: &MarkdownStructureMap,
    source: &str,
    line_start: usize,
) -> bool {
    structure.protected_ranges.iter().any(|range| {
        let start = range.start().to_usize();
        let end = range.end().to_usize();
        if !(start <= line_start && line_start < end) {
            return false;
        }
        !source[start..line_end(source, start, end)]
            .trim_start()
            .starts_with('-')
    })
}
fn skip_ws(s: &str, mut p: usize, end: usize) -> usize {
    while p < end && matches!(s.as_bytes()[p], b' ' | b'\t') {
        p += 1
    }
    p
}
fn scan_name(s: &str, mut p: usize, end: usize) -> usize {
    while p < end && !matches!(s.as_bytes()[p], b':' | b' ' | b'\t' | b'[') {
        p += 1
    }
    p
}
trait TrimEndIndex {
    fn trim_end_matches_index(self, s: &str, start: usize) -> usize;
}
impl TrimEndIndex for usize {
    fn trim_end_matches_index(self, s: &str, start: usize) -> usize {
        let mut p = self;
        while p > start && matches!(s.as_bytes()[p - 1], b' ' | b'\t') {
            p -= 1
        }
        p
    }
}
fn diag(
    code: UmlSyntaxDiagnosticCode,
    start: usize,
    end: usize,
    message: &'static str,
) -> TreeDiagnostic<UmlSyntaxDiagnosticCode> {
    TreeDiagnostic {
        code,
        severity: SyntaxSeverity::Error,
        message: message.into(),
        range: TextRange::new(
            TextSize::try_from_usize(start).unwrap(),
            TextSize::try_from_usize(end).unwrap(),
        )
        .unwrap(),
    }
}
