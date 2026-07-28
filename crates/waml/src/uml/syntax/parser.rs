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
        let Some(section_kind) = section_kind(source, heading.text_range) else {
            continue;
        };
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
            if section_kind == UmlSyntaxKind::MembersSection
                && structure
                    .headings
                    .iter()
                    .any(|heading| heading.range.start().to_usize() == line_start)
                && is_member_group_heading(source, line_start, line_end)
            {
                section.push(GreenElement::Node(
                    factory
                        .node(
                            UmlSyntaxKind::MemberGroup,
                            [member_group(&factory, &text, source, line_start, line_end)],
                        )
                        .unwrap(),
                ));
                continue;
            }
            let item_line = confirmed_list_item_line(structure, line_start)
                || tab_indented_item_line(structure, line_start);
            if opaque_line(structure, line_start, line_end) && !item_line {
                section.push(raw(&factory, &text, line_start, line_end));
            } else {
                if section_kind == UmlSyntaxKind::AttributesSection {
                    if let Some(attribute) = attribute(
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
                } else if let Some(item) = simple_item(
                    &factory,
                    &text,
                    source,
                    line_start,
                    line_end,
                    section_kind,
                    &mut diagnostics,
                ) {
                    section.push(GreenElement::Node(item));
                } else {
                    section.push(raw(&factory, &text, line_start, line_end));
                }
            }
        }
        children.push(GreenElement::Node(
            factory.node(section_kind, section).unwrap(),
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

fn member_group(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
) -> GreenElement<UmlLanguage> {
    let line_end = source[start..end]
        .find('\n')
        .map(|n| start + n)
        .unwrap_or(end);
    let markers = source[start..line_end]
        .as_bytes()
        .iter()
        .take_while(|c| **c == b'#')
        .count();
    let name_start = skip_ws(source, start + markers, line_end);
    let name_end = source[start..line_end]
        .trim_end_matches(['\r', ' ', '\t'])
        .len()
        + start;
    GreenElement::Node(
        f.node(
            UmlSyntaxKind::MemberGroup,
            [
                token(
                    f,
                    text,
                    start,
                    start,
                    start + markers,
                    UmlSyntaxKind::HeadingMarkerToken,
                ),
                token(
                    f,
                    text,
                    start + markers,
                    name_start,
                    name_end,
                    UmlSyntaxKind::IdentifierToken,
                ),
                if line_end < end {
                    token(
                        f,
                        text,
                        name_end,
                        name_end,
                        end,
                        UmlSyntaxKind::NewlineToken,
                    )
                } else {
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::NewlineToken))
                },
            ],
        )
        .unwrap(),
    )
}

fn section_kind(source: &str, range: TextRange) -> Option<UmlSyntaxKind> {
    let name = source[range.start().to_usize()..range.end().to_usize()]
        .trim()
        .trim_end_matches('#')
        .trim();
    if name.eq_ignore_ascii_case("Attributes") {
        Some(UmlSyntaxKind::AttributesSection)
    } else if name.eq_ignore_ascii_case("Values") {
        Some(UmlSyntaxKind::ValuesSection)
    } else if name.eq_ignore_ascii_case("Slots") {
        Some(UmlSyntaxKind::SlotsSection)
    } else if name.eq_ignore_ascii_case("Relationships") {
        Some(UmlSyntaxKind::RelationshipsSection)
    } else if name.eq_ignore_ascii_case("Members") {
        Some(UmlSyntaxKind::MembersSection)
    } else {
        None
    }
}

fn is_member_group_heading(source: &str, start: usize, end: usize) -> bool {
    let line = source[start..end].trim();
    let hashes = line.as_bytes().iter().take_while(|c| **c == b'#').count();
    (3..=6).contains(&hashes) && line.as_bytes().get(hashes) == Some(&b' ')
}

fn simple_item(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    section: UmlSyntaxKind,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> Option<waml_syntax::GreenNode<UmlLanguage>> {
    let line = &source[start..end];
    let newline = line.find('\n').map(|i| start + i).unwrap_or(end);
    let content_end = start
        + source[start..newline]
            .trim_end_matches(['\r', ' ', '\t'])
            .len();
    let lead = start + source[start..newline].len()
        - source[start..newline].trim_start_matches([' ', '\t']).len();
    if !source[lead..content_end].starts_with('-') {
        return None;
    }
    let kind = match section {
        UmlSyntaxKind::ValuesSection => UmlSyntaxKind::Value,
        UmlSyntaxKind::SlotsSection => UmlSyntaxKind::Slot,
        UmlSyntaxKind::RelationshipsSection => UmlSyntaxKind::Relationship,
        UmlSyntaxKind::MembersSection => {
            if source[lead..content_end].contains("instance of") {
                UmlSyntaxKind::InlineInstance
            } else {
                UmlSyntaxKind::Member
            }
        }
        _ => return None,
    };
    if kind == UmlSyntaxKind::Relationship {
        return Some(relationship(
            f,
            text,
            source,
            start,
            end,
            lead,
            content_end,
            diags,
        ));
    }
    if kind == UmlSyntaxKind::InlineInstance {
        return Some(inline_instance(
            f,
            text,
            source,
            start,
            end,
            lead,
            content_end,
            diags,
        ));
    }
    let mut children = vec![token(
        f,
        text,
        start,
        lead,
        lead + 1,
        UmlSyntaxKind::BulletToken,
    )];
    let body = skip_ws(source, lead + 1, content_end);
    if body == content_end {
        children.push(GreenElement::Token(
            f.missing_token(UmlSyntaxKind::IdentifierToken),
        ));
        diags.push(diag(
            UmlSyntaxDiagnosticCode::UnexpectedToken,
            lead,
            content_end,
            "missing classifier item content",
        ));
    } else {
        children.extend(classifier_tokens(
            f,
            text,
            source,
            lead + 1,
            body,
            content_end,
            section,
        ));
    }
    if kind == UmlSyntaxKind::Slot && !source[body..content_end].contains(':') {
        children.push(GreenElement::Token(
            f.missing_token(UmlSyntaxKind::ColonToken),
        ));
        diags.push(diag(
            UmlSyntaxDiagnosticCode::MissingColon,
            lead,
            content_end,
            "missing ':' in slot",
        ));
    }
    if content_end < end {
        children.push(token(
            f,
            text,
            content_end,
            content_end,
            end,
            UmlSyntaxKind::NewlineToken,
        ));
    }
    Some(f.node(kind, children).unwrap())
}

fn inline_instance(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    lead: usize,
    content_end: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> waml_syntax::GreenNode<UmlLanguage> {
    let mut c = vec![token(
        f,
        text,
        start,
        lead,
        lead + 1,
        UmlSyntaxKind::BulletToken,
    )];
    let mut p = skip_ws(source, lead + 1, content_end);
    let mut keyword_leading = lead + 1;
    for word in ["instance", "of"] {
        let q = p + word.len();
        if source[p..content_end].starts_with(word) {
            c.push(token(
                f,
                text,
                keyword_leading,
                p,
                q,
                UmlSyntaxKind::FlowKeywordToken,
            ));
            keyword_leading = q;
            p = skip_ws(source, q, content_end);
        } else {
            c.push(missing_token(
                f,
                text,
                p,
                p,
                UmlSyntaxKind::FlowKeywordToken,
            ));
            diags.push(diag(
                UmlSyntaxDiagnosticCode::UnexpectedToken,
                p,
                p,
                "missing inline instance keyword",
            ));
        }
    }
    let (classifier, q) =
        relationship_link(f, text, source, p, content_end, keyword_leading, diags);
    c.push(classifier);
    p = skip_ws(source, q, content_end);
    if source[p..content_end].starts_with("as") {
        let as_end = p + 2;
        c.push(token(f, text, q, p, as_end, UmlSyntaxKind::AsToken));
        keyword_leading = as_end;
        p = skip_ws(source, as_end, content_end);
    } else {
        c.push(GreenElement::Token(f.missing_token(UmlSyntaxKind::AsToken)));
        diags.push(diag(
            UmlSyntaxDiagnosticCode::UnexpectedToken,
            p,
            p,
            "missing 'as' in inline instance",
        ));
    }
    let name_leading = keyword_leading;
    let name_end = scan_name(source, p, content_end);
    c.push(if p == name_end {
        missing_token(f, text, name_leading, p, UmlSyntaxKind::IdentifierToken)
    } else {
        token(
            f,
            text,
            name_leading,
            p,
            name_end,
            UmlSyntaxKind::IdentifierToken,
        )
    });
    p = name_end;
    let before_with = p;
    p = skip_ws(source, p, content_end);
    if source[p..content_end].starts_with("with") {
        c.push(token(
            f,
            text,
            before_with,
            p,
            p + 4,
            UmlSyntaxKind::WithToken,
        ));
        keyword_leading = p + 4;
        p = skip_ws(source, p + 4, content_end);
    }
    while p < content_end {
        let name_start = p;
        let name_end = scan_name(source, p, content_end);
        if name_start == name_end {
            break;
        }
        let mut slot = vec![token(
            f,
            text,
            keyword_leading,
            name_start,
            name_end,
            UmlSyntaxKind::IdentifierToken,
        )];
        p = skip_ws(source, name_end, content_end);
        if source[p..content_end].starts_with("set to") {
            slot.push(token(
                f,
                text,
                name_end,
                p,
                p + 6,
                UmlSyntaxKind::SetToToken,
            ));
            keyword_leading = p + 6;
            p = skip_ws(source, p + 6, content_end);
        } else {
            slot.push(GreenElement::Token(
                f.missing_token(UmlSyntaxKind::SetToToken),
            ));
            diags.push(diag(
                UmlSyntaxDiagnosticCode::UnexpectedToken,
                p,
                p,
                "missing 'set to' in inline slot",
            ));
        }
        let value_start = p;
        if p < content_end && source.as_bytes()[p] == b'"' {
            let q = source[p + 1..content_end]
                .find('"')
                .map(|n| p + n + 2)
                .unwrap_or(content_end);
            slot.push(token(
                f,
                text,
                keyword_leading,
                p,
                q,
                UmlSyntaxKind::TypeToken,
            ));
            p = q;
        } else if p < content_end && source.as_bytes()[p] == b'[' {
            let (link, q) =
                relationship_link(f, text, source, p, content_end, keyword_leading, diags);
            slot.push(link);
            p = q;
        } else {
            let q = scan_name(source, p, content_end);
            slot.push(if p == q {
                missing_token(f, text, keyword_leading, p, UmlSyntaxKind::IdentifierToken)
            } else {
                token(
                    f,
                    text,
                    keyword_leading,
                    p,
                    q,
                    UmlSyntaxKind::IdentifierToken,
                )
            });
            p = q;
        }
        c.push(GreenElement::Node(
            f.node(UmlSyntaxKind::InlineSlot, slot).unwrap(),
        ));
        let join_leading = p;
        p = skip_ws(source, p, content_end);
        if source[p..content_end].starts_with("and") {
            c.push(token(
                f,
                text,
                join_leading,
                p,
                p + 3,
                UmlSyntaxKind::IdentifierToken,
            ));
            keyword_leading = p + 3;
            p = skip_ws(source, p + 3, content_end);
        } else {
            break;
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
    f.node(UmlSyntaxKind::InlineInstance, c).unwrap()
}

fn relationship(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    lead: usize,
    content_end: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> waml_syntax::GreenNode<UmlLanguage> {
    let mut c = vec![token(
        f,
        text,
        start,
        lead,
        lead + 1,
        UmlSyntaxKind::BulletToken,
    )];
    let mut p = skip_ws(source, lead + 1, content_end);
    let kind_start = p;
    let kind_text = [
        "instance of",
        "associates",
        "aggregates",
        "composes",
        "specializes",
        "implements",
        "depends",
        "annotates",
        "includes",
        "extends",
        "links",
    ]
    .into_iter()
    .find(|k| source[p..content_end].starts_with(k));
    if let Some(k) = kind_text {
        p += k.len();
        c.push(token(
            f,
            text,
            lead + 1,
            kind_start,
            p,
            UmlSyntaxKind::RelationshipKindToken,
        ));
    } else {
        let q = scan_name(source, p, content_end);
        c.push(if p == q {
            missing_token(f, text, lead + 1, p, UmlSyntaxKind::RelationshipKindToken)
        } else {
            token(
                f,
                text,
                lead + 1,
                p,
                q,
                UmlSyntaxKind::RelationshipKindToken,
            )
        });
        diags.push(diag(
            UmlSyntaxDiagnosticCode::UnexpectedToken,
            kind_start,
            q,
            "invalid relationship kind",
        ));
        p = q;
    }
    let link_leading = p;
    p = skip_ws(source, p, content_end);
    let (target, next) = relationship_link(f, text, source, p, content_end, link_leading, diags);
    c.push(target);
    p = next;
    let mut suffix_leading = p;
    p = skip_ws(source, p, content_end);
    if source[p..content_end].starts_with("as")
        && source[p + 2..content_end]
            .chars()
            .next()
            .is_none_or(char::is_whitespace)
    {
        let as_end = p + 2;
        c.push(token(
            f,
            text,
            suffix_leading,
            p,
            as_end,
            UmlSyntaxKind::AsToken,
        ));
        p = skip_ws(source, as_end, content_end);
        let mut name = Vec::new();
        if p < content_end && source.as_bytes()[p] == b'"' {
            let q = source[p + 1..content_end]
                .find('"')
                .map(|n| p + n + 2)
                .unwrap_or(content_end);
            name.push(token(f, text, as_end, p, q, UmlSyntaxKind::TypeToken));
            p = q;
        } else {
            let (link, q) = relationship_link(f, text, source, p, content_end, as_end, diags);
            name.push(link);
            p = q;
        }
        c.push(GreenElement::Node(
            f.node(UmlSyntaxKind::RelationshipName, name).unwrap(),
        ));
        suffix_leading = p;
        p = skip_ws(source, p, content_end);
    }
    if p < content_end && source.as_bytes()[p] == b':' {
        c.push(token(
            f,
            text,
            suffix_leading,
            p,
            p + 1,
            UmlSyntaxKind::ColonToken,
        ));
        let from_leading = p + 1;
        p = skip_ws(source, p + 1, content_end);
        let (from, q) = relationship_end(f, text, source, p, content_end, from_leading, diags);
        c.push(GreenElement::Node(from));
        p = skip_ws(source, q, content_end);
        if source[p..content_end].starts_with("to")
            && source[p + 2..content_end]
                .chars()
                .next()
                .is_none_or(char::is_whitespace)
        {
            c.push(token(f, text, q, p, p + 2, UmlSyntaxKind::ToToken));
            let to_leading = p + 2;
            p = skip_ws(source, p + 2, content_end);
            let (to, q) = relationship_end(f, text, source, p, content_end, to_leading, diags);
            c.push(GreenElement::Node(to));
            p = q;
        } else {
            c.push(GreenElement::Token(f.missing_token(UmlSyntaxKind::ToToken)));
            diags.push(diag(
                UmlSyntaxDiagnosticCode::UnexpectedToken,
                p,
                content_end,
                "missing 'to' between relationship ends",
            ));
            let (to, q) = relationship_end(f, text, source, p, content_end, q, diags);
            c.push(GreenElement::Node(to));
            p = q;
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
            "unexpected relationship content",
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
    f.node(UmlSyntaxKind::Relationship, c).unwrap()
}

fn relationship_link(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    p: usize,
    end: usize,
    leading: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> (GreenElement<UmlLanguage>, usize) {
    if p < end && source.as_bytes()[p] == b'[' {
        if let Some(close) = source[p + 1..end].find(']').map(|n| p + 1 + n) {
            if source.get(close + 1..close + 2) == Some("(") {
                if let Some(q) = source[close + 2..end].find(')').map(|n| close + 2 + n) {
                    return (
                        GreenElement::Node(
                            f.node(
                                UmlSyntaxKind::Link,
                                [
                                    token(
                                        f,
                                        text,
                                        leading,
                                        p,
                                        p + 1,
                                        UmlSyntaxKind::OpenBracketToken,
                                    ),
                                    token(
                                        f,
                                        text,
                                        p + 1,
                                        p + 1,
                                        close,
                                        UmlSyntaxKind::LinkTextToken,
                                    ),
                                    token(
                                        f,
                                        text,
                                        close,
                                        close,
                                        close + 1,
                                        UmlSyntaxKind::CloseBracketToken,
                                    ),
                                    token(
                                        f,
                                        text,
                                        close + 1,
                                        close + 1,
                                        close + 2,
                                        UmlSyntaxKind::OpenBracketToken,
                                    ),
                                    token(
                                        f,
                                        text,
                                        close + 2,
                                        close + 2,
                                        q,
                                        UmlSyntaxKind::LinkTargetToken,
                                    ),
                                    token(f, text, q, q, q + 1, UmlSyntaxKind::CloseBracketToken),
                                ],
                            )
                            .unwrap(),
                        ),
                        q + 1,
                    );
                }
            }
        }
    }
    let q = scan_name(source, p, end);
    diags.push(diag(
        UmlSyntaxDiagnosticCode::UnexpectedToken,
        p,
        q,
        "malformed relationship link",
    ));
    (
        GreenElement::Node(
            f.node(
                UmlSyntaxKind::Link,
                [
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::OpenBracketToken)),
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::LinkTextToken)),
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::CloseBracketToken)),
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::OpenBracketToken)),
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::LinkTargetToken)),
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::CloseBracketToken)),
                    GreenElement::Node(
                        f.node(
                            UmlSyntaxKind::SkippedTokensSyntax,
                            [GreenElement::Token(
                                f.bad_token(
                                    UmlSyntaxKind::BadToken,
                                    slice(text, p, q),
                                    UmlSyntaxDiagnosticCode::UnexpectedToken,
                                )
                                .unwrap(),
                            )],
                        )
                        .unwrap(),
                    ),
                ],
            )
            .unwrap(),
        ),
        q,
    )
}

fn relationship_end(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    p: usize,
    end: usize,
    leading: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> (waml_syntax::GreenNode<UmlLanguage>, usize) {
    let q = scan_name(source, p, end);
    let mut c = Vec::new();
    if p == q {
        c.push(missing_token(
            f,
            text,
            leading,
            p,
            UmlSyntaxKind::IdentifierToken,
        ));
        diags.push(diag(
            UmlSyntaxDiagnosticCode::InvalidMultiplicity,
            p,
            p,
            "missing relationship end multiplicity",
        ));
        return (f.node(UmlSyntaxKind::RelationshipEnd, c).unwrap(), p);
    }
    let mult = &source[p..q];
    c.push(token(
        f,
        text,
        leading,
        p,
        q,
        UmlSyntaxKind::IdentifierToken,
    ));
    let at = skip_ws(source, q, end);
    let mut next = q;
    if crate::multiplicity::Multiplicity::parse(mult).is_none() {
        diags.push(diag(
            UmlSyntaxDiagnosticCode::InvalidMultiplicity,
            p,
            q,
            "invalid relationship end multiplicity",
        ));
    }
    if at < end && !source[at..end].starts_with("to") && source.as_bytes()[at] != b':' {
        let r = scan_name(source, at, end);
        c.push(token(f, text, q, at, r, UmlSyntaxKind::IdentifierToken));
        next = r;
    }
    (f.node(UmlSyntaxKind::RelationshipEnd, c).unwrap(), next)
}

/// Tokenize the small, currently-supported classifier line vocabulary.  This is
/// deliberately line-local: the shell has already established the list-item
/// boundary, so recovery cannot consume the following item or heading.
fn classifier_tokens(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    leading: usize,
    mut at: usize,
    end: usize,
    section: UmlSyntaxKind,
) -> Vec<GreenElement<UmlLanguage>> {
    let mut out = Vec::new();
    let mut first = true;
    let mut trivia_start = leading;
    while at < end {
        let start = at;
        let kind = if source[at..].starts_with('[') {
            if let Some(close) = source[at + 1..end].find(']').map(|n| at + 1 + n) {
                if source.get(close + 1..close + 2) == Some("(") {
                    if let Some(target_end) =
                        source[close + 2..end].find(')').map(|n| close + 2 + n)
                    {
                        let link = f
                            .node(
                                UmlSyntaxKind::Link,
                                [
                                    token(
                                        f,
                                        text,
                                        trivia_start,
                                        at,
                                        at + 1,
                                        UmlSyntaxKind::OpenBracketToken,
                                    ),
                                    token(
                                        f,
                                        text,
                                        at + 1,
                                        at + 1,
                                        close,
                                        UmlSyntaxKind::LinkTextToken,
                                    ),
                                    token(
                                        f,
                                        text,
                                        close,
                                        close,
                                        close + 1,
                                        UmlSyntaxKind::CloseBracketToken,
                                    ),
                                    token(
                                        f,
                                        text,
                                        close + 1,
                                        close + 1,
                                        close + 2,
                                        UmlSyntaxKind::OpenBracketToken,
                                    ),
                                    token(
                                        f,
                                        text,
                                        close + 2,
                                        close + 2,
                                        target_end,
                                        UmlSyntaxKind::LinkTargetToken,
                                    ),
                                    token(
                                        f,
                                        text,
                                        target_end,
                                        target_end,
                                        target_end + 1,
                                        UmlSyntaxKind::CloseBracketToken,
                                    ),
                                ],
                            )
                            .unwrap();
                        out.push(GreenElement::Node(link));
                        at = target_end + 1;
                        trivia_start = at;
                        first = false;
                        continue;
                    }
                }
            }
            at += 1;
            UmlSyntaxKind::BadToken
        } else if source.as_bytes()[at] == b':' {
            at += 1;
            UmlSyntaxKind::ColonToken
        } else if source.as_bytes()[at] == b'\"' {
            at += 1;
            while at < end && source.as_bytes()[at] != b'\"' {
                at += 1;
            }
            if at < end {
                at += 1;
            }
            UmlSyntaxKind::TypeToken
        } else if source.as_bytes()[at].is_ascii_whitespace() {
            trivia_start = at;
            at += 1;
            continue;
        } else {
            while at < end
                && !source.as_bytes()[at].is_ascii_whitespace()
                && !matches!(source.as_bytes()[at], b':' | b'[' | b'\"')
            {
                at += 1;
            }
            if first && section == UmlSyntaxKind::RelationshipsSection {
                UmlSyntaxKind::RelationshipKindToken
            } else {
                UmlSyntaxKind::IdentifierToken
            }
        };
        out.push(token(f, text, trivia_start, start, at, kind));
        trivia_start = at;
        first = false;
    }
    out
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
fn opaque_line(structure: &MarkdownStructureMap, line_start: usize, line_end: usize) -> bool {
    structure
        .opaque_ranges
        .iter()
        .any(|range| range.start().to_usize() < line_end && line_start < range.end().to_usize())
}
fn confirmed_list_item_line(structure: &MarkdownStructureMap, line_start: usize) -> bool {
    structure
        .list_item_lines
        .iter()
        .any(|range| range.start().to_usize() == line_start)
}
fn tab_indented_item_line(structure: &MarkdownStructureMap, line_start: usize) -> bool {
    structure
        .tab_indented_item_lines
        .iter()
        .any(|range| range.start().to_usize() == line_start)
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
