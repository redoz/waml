use std::sync::Arc;

use super::{UmlLanguage, UmlSyntaxDiagnosticCode, UmlSyntaxKind};
use waml_syntax::{
    GreenElement, GreenFactory, GreenText, MarkdownStructureMap, SourceText, SyntaxSeverity,
    SyntaxTree, TextRange, TextSize, TreeDiagnostic, TriviaKind,
};

#[derive(Clone, Copy)]
pub(super) struct Island {
    pub kind: UmlSyntaxKind,
    pub range: TextRange,
    pub heading_end: Option<TextSize>,
}

pub(super) fn islands(source: &str, structure: &MarkdownStructureMap) -> Option<Vec<Island>> {
    let mut result = Vec::new();
    let mut at = 0;
    for (index, heading) in structure.headings.iter().enumerate() {
        if heading.level != 2 {
            continue;
        }
        let Some(kind) = section_kind(source, heading.text_range) else {
            continue;
        };
        let start = heading.range.start().to_usize();
        let end = structure
            .headings
            .iter()
            .skip(index + 1)
            .map(|next| next.range.start().to_usize())
            .next()
            .unwrap_or(source.len());
        if start < at || end < start || end > source.len() {
            return None;
        }
        if at < start {
            result.push(Island {
                kind: UmlSyntaxKind::MarkdownRegion,
                range: TextRange::new(
                    TextSize::try_from_usize(at).ok()?,
                    TextSize::try_from_usize(start).ok()?,
                )
                .ok()?,
                heading_end: None,
            });
        }
        result.push(Island {
            kind,
            range: TextRange::new(
                TextSize::try_from_usize(start).ok()?,
                TextSize::try_from_usize(end).ok()?,
            )
            .ok()?,
            heading_end: Some(TextSize::try_from_usize(line_end(source, start, end)).ok()?),
        });
        at = end;
    }
    if at < source.len() {
        result.push(Island {
            kind: UmlSyntaxKind::MarkdownRegion,
            range: TextRange::new(
                TextSize::try_from_usize(at).ok()?,
                TextSize::try_from_usize(source.len()).ok()?,
            )
            .ok()?,
            heading_end: None,
        });
    }
    Some(result)
}

pub(super) fn parse(
    text: SourceText,
    structure: &MarkdownStructureMap,
) -> Arc<SyntaxTree<UmlLanguage>> {
    let factory = GreenFactory::<UmlLanguage>::new();
    let source = text.shared();
    let descriptors = islands(source, structure).expect("markdown structure has ordered ranges");
    let mut children = Vec::with_capacity(descriptors.len() + 1);
    let mut diagnostics = Vec::new();
    for island in descriptors {
        children.push(parse_island_element(
            &factory,
            &text,
            source,
            structure,
            island,
            &mut diagnostics,
        ));
    }
    children.push(GreenElement::Token(
        factory.missing_token(UmlSyntaxKind::EndOfFileToken),
    ));
    let root = factory.node(UmlSyntaxKind::Root, children).unwrap();
    Arc::new(SyntaxTree::new(root, diagnostics.into(), structure.dialect))
}

pub(super) fn parse_island_element(
    factory: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    structure: &MarkdownStructureMap,
    island: Island,
    diagnostics: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    let start = island.range.start().to_usize();
    let end = island.range.end().to_usize();
    if island.kind == UmlSyntaxKind::MarkdownRegion {
        return raw(factory, text, start, end);
    }
    let heading_end = island
        .heading_end
        .expect("section islands have headings")
        .to_usize();
    let mut section = vec![raw(&factory, &text, start, heading_end)];
    if island.kind == UmlSyntaxKind::FlowSection {
        section.push(GreenElement::Node(flow_block(
            factory,
            text,
            source,
            heading_end,
            end,
            structure,
            diagnostics,
        )));
        return GreenElement::Node(factory.node(island.kind, section).unwrap());
    }
    if island.kind == UmlSyntaxKind::MessagesSection {
        section.extend(sequence_items(
            factory,
            text,
            source,
            heading_end,
            end,
            structure,
            diagnostics,
        ));
        return GreenElement::Node(factory.node(island.kind, section).unwrap());
    }
    if island.kind == UmlSyntaxKind::MembersSection {
        section.extend(member_items(
            factory,
            text,
            source,
            heading_end,
            end,
            structure,
            diagnostics,
        ));
        return GreenElement::Node(factory.node(island.kind, section).unwrap());
    }
    for (line_start, line_end) in lines_between(source, heading_end, end) {
        let item_line = confirmed_list_item_line(structure, line_start)
            || tab_indented_item_line(structure, line_start);
        if opaque_line(structure, line_start, line_end) && !item_line {
            section.push(raw(factory, text, line_start, line_end));
        } else {
            if island.kind == UmlSyntaxKind::AttributesSection {
                if let Some(attribute) =
                    attribute(factory, text, source, line_start, line_end, diagnostics)
                {
                    section.push(GreenElement::Node(attribute));
                } else {
                    section.push(raw(factory, text, line_start, line_end));
                }
            } else if island.kind == UmlSyntaxKind::LifelinesSection {
                let line = source[line_start..line_end].trim_end_matches(['\r', '\n']);
                if line.trim().is_empty() {
                    section.push(raw(factory, text, line_start, line_end));
                } else {
                    section.push(lifeline_line(
                        factory,
                        text,
                        source,
                        line_start,
                        line_end,
                        diagnostics,
                    ));
                }
            } else if let Some(item) = simple_item(
                factory,
                text,
                source,
                line_start,
                line_end,
                island.kind,
                diagnostics,
            ) {
                section.push(GreenElement::Node(item));
            } else {
                section.push(raw(factory, text, line_start, line_end));
            }
        }
    }
    GreenElement::Node(factory.node(island.kind, section).unwrap())
}

fn member_group_children(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
) -> Vec<GreenElement<UmlLanguage>> {
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
    vec![
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
    ]
}

fn member_items(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    from: usize,
    to: usize,
    structure: &MarkdownStructureMap,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> Vec<GreenElement<UmlLanguage>> {
    struct Pending {
        depth: u8,
        children: Vec<GreenElement<UmlLanguage>>,
    }
    fn close_one(
        f: &GreenFactory<UmlLanguage>,
        stack: &mut Vec<Pending>,
        roots: &mut Vec<GreenElement<UmlLanguage>>,
    ) {
        let group = stack.pop().expect("group stack");
        let node = GreenElement::Node(f.node(UmlSyntaxKind::MemberGroup, group.children).unwrap());
        if let Some(parent) = stack.last_mut() {
            parent.children.push(node)
        } else {
            roots.push(node)
        }
    }
    let mut roots = Vec::new();
    let mut stack: Vec<Pending> = Vec::new();
    for (start, end) in lines_between(source, from, to) {
        if let Some(heading) = structure
            .nested_headings
            .iter()
            .find(|h| h.range.start().to_usize() == start)
        {
            while stack.last().is_some_and(|g| g.depth >= heading.level) {
                close_one(f, &mut stack, &mut roots)
            }
            stack.push(Pending {
                depth: heading.level,
                children: member_group_children(f, text, source, start, end),
            });
            continue;
        }
        let item_line =
            confirmed_list_item_line(structure, start) || tab_indented_item_line(structure, start);
        let element = if opaque_line(structure, start, end) && !item_line {
            raw(f, text, start, end)
        } else if let Some(item) = simple_item(
            f,
            text,
            source,
            start,
            end,
            UmlSyntaxKind::MembersSection,
            diags,
        ) {
            GreenElement::Node(item)
        } else {
            raw(f, text, start, end)
        };
        if let Some(group) = stack.last_mut() {
            group.children.push(element)
        } else {
            roots.push(element)
        }
    }
    while !stack.is_empty() {
        close_one(f, &mut stack, &mut roots)
    }
    let first_group = roots.iter().position(|element| {
        matches!(
            element,
            GreenElement::Node(node) if node.kind() == UmlSyntaxKind::MemberGroup
        )
    });
    let root_end = first_group.unwrap_or(roots.len());
    let has_root_items = roots[..root_end].iter().any(|element| {
        matches!(
            element,
            GreenElement::Node(node)
                if matches!(
                    node.kind(),
                    UmlSyntaxKind::Member | UmlSyntaxKind::InlineInstance
                )
        )
    });
    if has_root_items || first_group.is_none() {
        let explicit_groups = roots.split_off(root_end);
        let implicit = GreenElement::Node(f.node(UmlSyntaxKind::MemberGroup, roots).unwrap());
        roots = vec![implicit];
        roots.extend(explicit_groups);
    }
    roots
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
    } else if name.eq_ignore_ascii_case("Layout") {
        Some(UmlSyntaxKind::LayoutSection)
    } else if name.eq_ignore_ascii_case("Nodes") {
        Some(UmlSyntaxKind::FlowSection)
    } else if name.eq_ignore_ascii_case("Lifelines") {
        Some(UmlSyntaxKind::LifelinesSection)
    } else if name.eq_ignore_ascii_case("Messages") {
        Some(UmlSyntaxKind::MessagesSection)
    } else {
        None
    }
}

fn flow_block(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    from: usize,
    to: usize,
    structure: &MarkdownStructureMap,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> waml_syntax::GreenNode<UmlLanguage> {
    let mut roots = Vec::new();
    let mut current = Vec::new();
    let mut have_node = false;
    let mut notes = false;
    let close = |current: &mut Vec<GreenElement<UmlLanguage>>,
                 roots: &mut Vec<GreenElement<UmlLanguage>>,
                 have_node: &mut bool| {
        if *have_node {
            roots.push(GreenElement::Node(
                f.node(UmlSyntaxKind::FlowNode, std::mem::take(current))
                    .unwrap(),
            ));
            *have_node = false;
        }
    };
    for (start, end) in lines_between(source, from, to) {
        let nested = structure
            .nested_headings
            .iter()
            .find(|heading| heading.range.start().to_usize() == start);
        if let Some(heading) = nested {
            if heading.level == 3 {
                close(&mut current, &mut roots, &mut have_node);
                current.extend(flow_heading(f, text, source, start, end, diags));
                have_node = true;
                notes = false;
                continue;
            }
            if heading.level == 4
                && have_node
                && source
                    [heading.text_range.start().to_usize()..heading.text_range.end().to_usize()]
                    .trim()
                    .eq_ignore_ascii_case("notes")
            {
                current.push(raw(f, text, start, end));
                notes = true;
                continue;
            }
        }
        let trimmed = source[start..end].trim_end_matches(['\r', '\n']);
        if trimmed.trim().is_empty() {
            (if have_node { &mut current } else { &mut roots }).push(raw(f, text, start, end));
            continue;
        }
        if !have_node {
            roots.push(recovery_line(
                f,
                text,
                start,
                end,
                UmlSyntaxDiagnosticCode::MalformedFlow,
                "flow content before first node heading",
                diags,
            ));
            continue;
        }
        if notes && trimmed.trim_start().starts_with("- ") {
            current.push(flow_value_line(f, text, source, start, end));
        } else if !notes {
            current.push(flow_line(f, text, source, start, end, diags));
        } else {
            let message = if notes {
                "malformed flow note"
            } else {
                "malformed flow bullet"
            };
            current.push(recovery_line(
                f,
                text,
                start,
                end,
                UmlSyntaxDiagnosticCode::MalformedFlow,
                message,
                diags,
            ));
        }
    }
    close(&mut current, &mut roots, &mut have_node);
    f.node(UmlSyntaxKind::FlowBlock, roots).unwrap()
}

fn sequence_items(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    from: usize,
    to: usize,
    structure: &MarkdownStructureMap,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> Vec<GreenElement<UmlLanguage>> {
    let mut items = Vec::new();
    for (start, end) in lines_between(source, from, to) {
        if opaque_line(structure, start, end) {
            items.push(raw(f, text, start, end));
            continue;
        }
        let line = source[start..end].trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            items.push(raw(f, text, start, end));
            continue;
        }
        let leading = line.len() - line.trim_start_matches(' ').len();
        let malformed_indent = leading % 2 != 0 || line.starts_with('\t');
        let significant_start = start + leading;
        let content_end = start + line.len();
        let body = line
            .trim_start()
            .strip_prefix("- ")
            .unwrap_or(line.trim_start());
        if unsupported_sequence_body(body) {
            items.push(recovery_line_at(
                f,
                text,
                start,
                end,
                significant_start,
                content_end,
                UmlSyntaxDiagnosticCode::UnsupportedSequenceForm,
                "unsupported sequence form",
                diags,
            ));
        } else if malformed_indent {
            items.push(recovery_line(
                f,
                text,
                start,
                end,
                UmlSyntaxDiagnosticCode::MalformedIndentation,
                "sequence indentation must use pairs of spaces",
                diags,
            ));
        } else if matches!(body, "alt" | "opt" | "loop") {
            items.push(sequence_fragment(f, text, source, start, end));
        } else if body.starts_with("when ") || body == "else" {
            items.push(sequence_operand(
                f,
                text,
                source,
                start,
                end,
                leading >= 2,
                diags,
            ));
        } else {
            let message = sequence_message(f, text, source, start, end, diags);
            if message_self_target(&message) {
                items.push(recovery_line_at(
                    f,
                    text,
                    start,
                    end,
                    significant_start,
                    content_end,
                    UmlSyntaxDiagnosticCode::UnsupportedSequenceForm,
                    "self messages are not supported",
                    diags,
                ));
            } else {
                items.push(message);
            }
        }
    }
    items
}

fn lifeline_line(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    let (lead, content_end, newline) = behavior_bounds(source, start, end);
    let has_bullet = source.get(lead..lead + 1) == Some("-");
    let bullet_end = if has_bullet { lead + 1 } else { lead };
    let mut children = vec![if has_bullet {
        token(f, text, start, lead, bullet_end, UmlSyntaxKind::BulletToken)
    } else {
        let leading: Vec<_> = (start < lead)
            .then(|| {
                f.trivia(TriviaKind::Whitespace, slice(text, start, lead))
                    .unwrap()
            })
            .into_iter()
            .collect();
        GreenElement::Token(
            f.missing_token_with_leading(UmlSyntaxKind::BulletToken, leading)
                .unwrap(),
        )
    }];
    let mut p = skip_ws(source, bullet_end, content_end);
    let (link, next) = behavior_link(
        f,
        text,
        source,
        p,
        content_end,
        bullet_end,
        UmlSyntaxDiagnosticCode::MalformedLifeline,
        "malformed lifeline link",
        diags,
    );
    children.push(link);
    p = next;
    if !has_bullet {
        diags.push(diag(
            UmlSyntaxDiagnosticCode::MalformedLifeline,
            lead,
            content_end,
            "missing lifeline bullet",
        ));
    }
    let as_start = skip_ws(source, p, content_end);
    if keyword_at(source, as_start, content_end, "as") {
        children.push(token(
            f,
            text,
            p,
            as_start,
            as_start + 2,
            UmlSyntaxKind::AsToken,
        ));
        let alias_start = skip_ws(source, as_start + 2, content_end);
        let alias_end = scan_word(source, alias_start, content_end);
        children.push(slot(
            f,
            UmlSyntaxKind::LifelineAlias,
            if alias_start == alias_end {
                GreenElement::Token(f.missing_token(UmlSyntaxKind::AliasToken))
            } else {
                token(
                    f,
                    text,
                    as_start + 2,
                    alias_start,
                    alias_end,
                    UmlSyntaxKind::AliasToken,
                )
            },
        ));
        p = alias_end;
    } else {
        children.push(GreenElement::Token(f.missing_token(UmlSyntaxKind::AsToken)));
        children.push(slot(
            f,
            UmlSyntaxKind::LifelineAlias,
            GreenElement::Token(f.missing_token(UmlSyntaxKind::AliasToken)),
        ));
        p = as_start;
    }
    let recovery = if p < content_end {
        let recovery = skipped(
            f,
            text,
            p,
            content_end,
            UmlSyntaxDiagnosticCode::MalformedLifeline,
        );
        diags.push(diag(
            UmlSyntaxDiagnosticCode::MalformedLifeline,
            lead,
            content_end,
            "malformed lifeline",
        ));
        p = content_end;
        Some(recovery)
    } else {
        None
    };
    children.push(behavior_recovery(f, recovery));
    push_behavior_newline(f, text, &mut children, p, newline, end);
    GreenElement::Node(f.node(UmlSyntaxKind::Lifeline, children).unwrap())
}

fn recovery_line(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    start: usize,
    end: usize,
    code: UmlSyntaxDiagnosticCode,
    message: &'static str,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    recovery_line_at(f, text, start, end, start, end, code, message, diags)
}

fn recovery_line_at(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    start: usize,
    end: usize,
    diagnostic_start: usize,
    diagnostic_end: usize,
    code: UmlSyntaxDiagnosticCode,
    message: &'static str,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    diags.push(diag(code, diagnostic_start, diagnostic_end, message));
    GreenElement::Node(
        f.node(
            UmlSyntaxKind::SkippedTokensSyntax,
            [GreenElement::Token(
                f.bad_token(UmlSyntaxKind::BadToken, slice(text, start, end), code)
                    .unwrap(),
            )],
        )
        .unwrap(),
    )
}

fn flow_heading(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> Vec<GreenElement<UmlLanguage>> {
    let (lead, content_end, newline) = behavior_bounds(source, start, end);
    let markers = source[lead..content_end]
        .bytes()
        .take_while(|byte| *byte == b'#')
        .count();
    let mut children = vec![token(
        f,
        text,
        start,
        lead,
        lead + markers,
        UmlSyntaxKind::HeadingMarkerToken,
    )];
    let body = skip_ws(source, lead + markers, content_end);
    let first_end = scan_word(source, body, content_end);
    let first = &source[body..first_end];
    let (kind, identity_start) = if crate::model::FlowNodeKind::from_keyword(first).is_some() {
        children.push(slot(
            f,
            UmlSyntaxKind::FlowNodeKindSlot,
            token(
                f,
                text,
                lead + markers,
                body,
                first_end,
                UmlSyntaxKind::NodeKindToken,
            ),
        ));
        (Some(first), skip_ws(source, first_end, content_end))
    } else {
        children.push(slot(
            f,
            UmlSyntaxKind::FlowNodeKindSlot,
            GreenElement::Token(f.missing_token(UmlSyntaxKind::NodeKindToken)),
        ));
        (None, body)
    };
    let mut p = identity_start;
    if kind == Some("object") && p < content_end && source.as_bytes()[p] == b'[' {
        let (link, next) = behavior_link(
            f,
            text,
            source,
            p,
            content_end,
            first_end,
            UmlSyntaxDiagnosticCode::MalformedFlow,
            "malformed object-node link",
            diags,
        );
        children.push(GreenElement::Node(
            f.node(UmlSyntaxKind::FlowIdentity, [link]).unwrap(),
        ));
        p = next;
    } else if p < content_end {
        children.push(GreenElement::Node(
            f.node(
                UmlSyntaxKind::FlowIdentity,
                [token(
                    f,
                    text,
                    if kind.is_some() {
                        first_end
                    } else {
                        lead + markers
                    },
                    p,
                    content_end,
                    UmlSyntaxKind::IdentityToken,
                )],
            )
            .unwrap(),
        ));
        p = content_end;
    } else {
        children.push(GreenElement::Node(
            f.node(
                UmlSyntaxKind::FlowIdentity,
                [GreenElement::Token(
                    f.missing_token(UmlSyntaxKind::IdentityToken),
                )],
            )
            .unwrap(),
        ));
        if !matches!(kind, Some("initial" | "final")) {
            diags.push(diag(
                UmlSyntaxDiagnosticCode::MalformedFlow,
                body,
                content_end,
                "missing flow node identity",
            ));
        }
    }
    let recovery = if p < content_end {
        let recovery = skipped(
            f,
            text,
            p,
            content_end,
            UmlSyntaxDiagnosticCode::MalformedFlow,
        );
        diags.push(diag(
            UmlSyntaxDiagnosticCode::MalformedFlow,
            p,
            content_end,
            "unexpected flow heading content",
        ));
        p = content_end;
        Some(recovery)
    } else {
        None
    };
    children.push(behavior_recovery(f, recovery));
    push_behavior_newline(f, text, &mut children, p, newline, end);
    children
}

fn flow_value_line(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
) -> GreenElement<UmlLanguage> {
    let (lead, content_end, newline) = behavior_bounds(source, start, end);
    let mut children = vec![token(
        f,
        text,
        start,
        lead,
        lead + 1,
        UmlSyntaxKind::BulletToken,
    )];
    let value_start = skip_ws(source, lead + 1, content_end);
    children.push(slot(
        f,
        UmlSyntaxKind::FlowNoteValue,
        if value_start < content_end {
            token(
                f,
                text,
                lead + 1,
                value_start,
                content_end,
                UmlSyntaxKind::IdentifierToken,
            )
        } else {
            GreenElement::Token(f.missing_token(UmlSyntaxKind::IdentifierToken))
        },
    ));
    push_behavior_newline(f, text, &mut children, content_end, newline, end);
    GreenElement::Node(f.node(UmlSyntaxKind::Value, children).unwrap())
}

fn flow_line(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    let (_, content_end, _) = behavior_bounds(source, start, end);
    let lead = skip_ws(source, start, content_end);
    let body = skip_ws(source, (lead + 1).min(content_end), content_end);
    let word_end = scan_word(source, body, content_end);
    let word = &source[body..word_end];
    if matches!(word, "entry:" | "do:" | "exit:" | "refines" | "partition:") {
        flow_internal(f, text, source, start, end, diags)
    } else {
        flow_transition(f, text, source, start, end, diags)
    }
}

fn flow_internal(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    let (lead, content_end, newline) = behavior_bounds(source, start, end);
    let mut children = vec![token(
        f,
        text,
        start,
        lead,
        lead + 1,
        UmlSyntaxKind::BulletToken,
    )];
    let p = skip_ws(source, lead + 1, content_end);
    let word_end = scan_word(source, p, content_end);
    let word = &source[p..word_end];
    let mut at = word_end;
    let mut valid = true;
    let keyword;
    let colon;
    let value;
    let link;
    match word {
        "entry:" | "do:" | "exit:" => {
            let colon_at = word_end - 1;
            keyword = token(
                f,
                text,
                lead + 1,
                p,
                colon_at,
                UmlSyntaxKind::InternalKeywordToken,
            );
            colon = token(
                f,
                text,
                colon_at,
                colon_at,
                word_end,
                UmlSyntaxKind::ColonToken,
            );
            let value_start = skip_ws(source, word_end, content_end);
            if let Some(expr_end) = scan_backtick(source, value_start, content_end) {
                value = slot(
                    f,
                    UmlSyntaxKind::FlowInternalValue,
                    token(
                        f,
                        text,
                        word_end,
                        value_start,
                        expr_end,
                        UmlSyntaxKind::ExpressionToken,
                    ),
                );
                at = expr_end;
            } else {
                value = slot(
                    f,
                    UmlSyntaxKind::FlowInternalValue,
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::ExpressionToken)),
                );
                at = value_start;
                valid = false;
            }
            link = missing_link(f);
        }
        "refines" => {
            keyword = token(
                f,
                text,
                lead + 1,
                p,
                word_end,
                UmlSyntaxKind::InternalKeywordToken,
            );
            colon = GreenElement::Token(f.missing_token(UmlSyntaxKind::ColonToken));
            value = slot(
                f,
                UmlSyntaxKind::FlowInternalValue,
                GreenElement::Token(f.missing_token(UmlSyntaxKind::ExpressionToken)),
            );
            let link_start = skip_ws(source, word_end, content_end);
            let (parsed_link, next) = behavior_link(
                f,
                text,
                source,
                link_start,
                content_end,
                word_end,
                UmlSyntaxDiagnosticCode::MalformedFlow,
                "malformed refinement link",
                diags,
            );
            link = parsed_link;
            at = next;
            valid = link_start < content_end && source.as_bytes()[link_start] == b'[';
        }
        "partition:" => {
            let colon_at = word_end - 1;
            keyword = token(
                f,
                text,
                lead + 1,
                p,
                colon_at,
                UmlSyntaxKind::InternalKeywordToken,
            );
            colon = token(
                f,
                text,
                colon_at,
                colon_at,
                word_end,
                UmlSyntaxKind::ColonToken,
            );
            let value_start = skip_ws(source, word_end, content_end);
            if value_start < content_end {
                value = slot(
                    f,
                    UmlSyntaxKind::FlowInternalValue,
                    token(
                        f,
                        text,
                        word_end,
                        value_start,
                        content_end,
                        UmlSyntaxKind::IdentifierToken,
                    ),
                );
                at = content_end;
            } else {
                value = slot(
                    f,
                    UmlSyntaxKind::FlowInternalValue,
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::IdentifierToken)),
                );
                valid = false;
            }
            link = missing_link(f);
        }
        _ => {
            keyword = GreenElement::Token(f.missing_token(UmlSyntaxKind::InternalKeywordToken));
            colon = GreenElement::Token(f.missing_token(UmlSyntaxKind::ColonToken));
            value = slot(
                f,
                UmlSyntaxKind::FlowInternalValue,
                GreenElement::Token(f.missing_token(UmlSyntaxKind::ExpressionToken)),
            );
            link = missing_link(f);
            at = p;
            valid = false;
        }
    }
    children.extend([keyword, colon, value, link]);
    let recovery = if at < content_end {
        let recovery = skipped(
            f,
            text,
            at,
            content_end,
            UmlSyntaxDiagnosticCode::MalformedFlow,
        );
        at = content_end;
        valid = false;
        Some(recovery)
    } else {
        None
    };
    if !valid {
        diags.push(diag(
            UmlSyntaxDiagnosticCode::MalformedFlow,
            lead,
            content_end,
            "malformed flow bullet",
        ));
    }
    children.push(behavior_recovery(f, recovery));
    push_behavior_newline(f, text, &mut children, at, newline, end);
    GreenElement::Node(f.node(UmlSyntaxKind::FlowInternal, children).unwrap())
}

fn flow_transition(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    let (lead, content_end, newline) = behavior_bounds(source, start, end);
    let mut children = vec![token(
        f,
        text,
        start,
        lead,
        lead + 1,
        UmlSyntaxKind::BulletToken,
    )];
    let mut owned = lead + 1;
    let mut p = skip_ws(source, owned, content_end);
    let mut valid = source.get(lead..lead + 1) == Some("-");
    if keyword_at(source, p, content_end, "on") {
        let keyword = token(f, text, lead + 1, p, p + 2, UmlSyntaxKind::FlowKeywordToken);
        owned = p + 2;
        let expr = skip_ws(source, p + 2, content_end);
        if let Some(q) = scan_backtick(source, expr, content_end) {
            children.push(GreenElement::Node(
                f.node(
                    UmlSyntaxKind::FlowTrigger,
                    [
                        keyword,
                        token(f, text, owned, expr, q, UmlSyntaxKind::TriggerToken),
                    ],
                )
                .unwrap(),
            ));
            owned = q;
            p = skip_ws(source, q, content_end);
        } else {
            children.push(GreenElement::Node(
                f.node(
                    UmlSyntaxKind::FlowTrigger,
                    [
                        keyword,
                        GreenElement::Token(f.missing_token(UmlSyntaxKind::TriggerToken)),
                    ],
                )
                .unwrap(),
            ));
            p = expr;
            valid = false;
        }
    } else {
        children.push(GreenElement::Node(
            f.node(
                UmlSyntaxKind::FlowTrigger,
                [
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::FlowKeywordToken)),
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::TriggerToken)),
                ],
            )
            .unwrap(),
        ));
    }
    if keyword_at(source, p, content_end, "when") {
        let keyword = token(f, text, owned, p, p + 4, UmlSyntaxKind::FlowKeywordToken);
        owned = p + 4;
        let expr = skip_ws(source, p + 4, content_end);
        if let Some(q) = scan_backtick(source, expr, content_end) {
            children.push(GreenElement::Node(
                f.node(
                    UmlSyntaxKind::FlowGuard,
                    [
                        keyword,
                        token(f, text, owned, expr, q, UmlSyntaxKind::GuardToken),
                    ],
                )
                .unwrap(),
            ));
            owned = q;
            p = skip_ws(source, q, content_end);
        } else {
            children.push(GreenElement::Node(
                f.node(
                    UmlSyntaxKind::FlowGuard,
                    [
                        keyword,
                        GreenElement::Token(f.missing_token(UmlSyntaxKind::GuardToken)),
                    ],
                )
                .unwrap(),
            ));
            p = expr;
            valid = false;
        }
    } else if keyword_at(source, p, content_end, "else") {
        children.push(GreenElement::Node(
            f.node(
                UmlSyntaxKind::FlowGuard,
                [
                    token(f, text, owned, p, p + 4, UmlSyntaxKind::ElseToken),
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::GuardToken)),
                ],
            )
            .unwrap(),
        ));
        owned = p + 4;
        p = skip_ws(source, p + 4, content_end);
    } else {
        children.push(GreenElement::Node(
            f.node(
                UmlSyntaxKind::FlowGuard,
                [
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::FlowKeywordToken)),
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::GuardToken)),
                ],
            )
            .unwrap(),
        ));
    }
    if keyword_at(source, p, content_end, "transitions") {
        children.push(token(
            f,
            text,
            owned,
            p,
            p + 11,
            UmlSyntaxKind::FlowKeywordToken,
        ));
        owned = p + 11;
        p = skip_ws(source, p + 11, content_end);
    } else {
        children.push(GreenElement::Token(
            f.missing_token(UmlSyntaxKind::FlowKeywordToken),
        ));
        valid = false;
    }
    if keyword_at(source, p, content_end, "to") {
        children.push(token(f, text, owned, p, p + 2, UmlSyntaxKind::ToToken));
        owned = p + 2;
        p = skip_ws(source, p + 2, content_end);
    } else {
        children.push(GreenElement::Token(f.missing_token(UmlSyntaxKind::ToToken)));
        valid = false;
    }
    let target_end = find_clause(source, p, content_end, &[" carries ", ": "]);
    let target_trimmed = trim_end_ws(source, p, target_end);
    if p < target_trimmed && source.as_bytes()[p] == b'[' {
        let (link, next) = behavior_link(
            f,
            text,
            source,
            p,
            target_trimmed,
            owned,
            UmlSyntaxDiagnosticCode::MalformedFlow,
            "malformed transition target",
            diags,
        );
        children.push(slot(f, UmlSyntaxKind::FlowTarget, link));
        owned = next;
        p = next;
    } else if p < target_trimmed {
        children.push(slot(
            f,
            UmlSyntaxKind::FlowTarget,
            token(
                f,
                text,
                owned,
                p,
                target_trimmed,
                UmlSyntaxKind::TargetToken,
            ),
        ));
        owned = target_trimmed;
        p = target_trimmed;
    } else {
        children.push(slot(
            f,
            UmlSyntaxKind::FlowTarget,
            GreenElement::Token(f.missing_token(UmlSyntaxKind::TargetToken)),
        ));
        valid = false;
    }
    p = skip_ws(source, p, content_end);
    if keyword_at(source, p, content_end, "carries") {
        let keyword = token(f, text, owned, p, p + 7, UmlSyntaxKind::FlowKeywordToken);
        owned = p + 7;
        let link_start = skip_ws(source, p + 7, content_end);
        let link_end = source[link_start..content_end]
            .find(": ")
            .map(|offset| link_start + offset)
            .unwrap_or(content_end);
        let (link, next) = behavior_link(
            f,
            text,
            source,
            link_start,
            link_end,
            owned,
            UmlSyntaxDiagnosticCode::MalformedFlow,
            "malformed carried-type link",
            diags,
        );
        children.push(GreenElement::Node(
            f.node(UmlSyntaxKind::FlowCarries, [keyword, link]).unwrap(),
        ));
        owned = next;
        valid &= link_start < link_end && source.as_bytes()[link_start] == b'[';
        p = skip_ws(source, next, content_end);
    } else {
        children.push(GreenElement::Node(
            f.node(
                UmlSyntaxKind::FlowCarries,
                [
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::FlowKeywordToken)),
                    missing_link(f),
                ],
            )
            .unwrap(),
        ));
    }
    if p < content_end && source.as_bytes()[p] == b':' {
        let colon = token(f, text, owned, p, p + 1, UmlSyntaxKind::ColonToken);
        owned = p + 1;
        let expr = skip_ws(source, p + 1, content_end);
        if let Some(q) = scan_backtick(source, expr, content_end) {
            children.push(GreenElement::Node(
                f.node(
                    UmlSyntaxKind::FlowEffect,
                    [
                        colon,
                        token(f, text, owned, expr, q, UmlSyntaxKind::EffectToken),
                    ],
                )
                .unwrap(),
            ));
            owned = q;
            p = q;
        } else {
            children.push(GreenElement::Node(
                f.node(
                    UmlSyntaxKind::FlowEffect,
                    [
                        colon,
                        GreenElement::Token(f.missing_token(UmlSyntaxKind::EffectToken)),
                    ],
                )
                .unwrap(),
            ));
            p = expr;
            valid = false;
        }
    } else {
        children.push(GreenElement::Node(
            f.node(
                UmlSyntaxKind::FlowEffect,
                [
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::ColonToken)),
                    GreenElement::Token(f.missing_token(UmlSyntaxKind::EffectToken)),
                ],
            )
            .unwrap(),
        ));
    }
    let recovery = if p < content_end {
        let recovery = skipped(
            f,
            text,
            owned,
            content_end,
            UmlSyntaxDiagnosticCode::MalformedFlow,
        );
        owned = content_end;
        valid = false;
        Some(recovery)
    } else {
        None
    };
    if !valid {
        diags.push(diag(
            UmlSyntaxDiagnosticCode::MalformedFlow,
            lead,
            content_end,
            "malformed transition",
        ));
    }
    children.push(behavior_recovery(f, recovery));
    push_behavior_newline(f, text, &mut children, owned, newline, end);
    GreenElement::Node(f.node(UmlSyntaxKind::FlowTransition, children).unwrap())
}

fn sequence_fragment(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
) -> GreenElement<UmlLanguage> {
    let (lead, content_end, newline) = behavior_bounds(source, start, end);
    let kind_start = skip_ws(source, lead + 1, content_end);
    let mut children = vec![
        token(f, text, start, lead, lead + 1, UmlSyntaxKind::BulletToken),
        slot(
            f,
            UmlSyntaxKind::FragmentKind,
            token(
                f,
                text,
                lead + 1,
                kind_start,
                content_end,
                UmlSyntaxKind::FragmentKindToken,
            ),
        ),
    ];
    children.push(behavior_recovery(f, None));
    push_behavior_newline(f, text, &mut children, content_end, newline, end);
    GreenElement::Node(f.node(UmlSyntaxKind::SequenceFragment, children).unwrap())
}

fn sequence_operand(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    nested: bool,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    let (lead, content_end, newline) = behavior_bounds(source, start, end);
    let keyword = skip_ws(source, lead + 1, content_end);
    let keyword_end = scan_word(source, keyword, content_end);
    let mut children = vec![
        token(f, text, start, lead, lead + 1, UmlSyntaxKind::BulletToken),
        token(
            f,
            text,
            lead + 1,
            keyword,
            keyword_end,
            UmlSyntaxKind::OperandKeywordToken,
        ),
    ];
    let mut p = keyword_end;
    let mut valid = nested;
    if &source[keyword..keyword_end] == "when" {
        let guard = skip_ws(source, keyword_end, content_end);
        if let Some(q) = scan_backtick(source, guard, content_end) {
            children.push(slot(
                f,
                UmlSyntaxKind::OperandGuard,
                token(f, text, keyword_end, guard, q, UmlSyntaxKind::GuardToken),
            ));
            p = q;
        } else {
            children.push(slot(
                f,
                UmlSyntaxKind::OperandGuard,
                GreenElement::Token(f.missing_token(UmlSyntaxKind::GuardToken)),
            ));
            p = guard;
            valid = false;
        }
    } else {
        children.push(slot(
            f,
            UmlSyntaxKind::OperandGuard,
            GreenElement::Token(f.missing_token(UmlSyntaxKind::GuardToken)),
        ));
    }
    let recovery = if p < content_end {
        let recovery = skipped(
            f,
            text,
            p,
            content_end,
            UmlSyntaxDiagnosticCode::MalformedMessage,
        );
        p = content_end;
        valid = false;
        Some(recovery)
    } else {
        None
    };
    if !valid {
        diags.push(diag(
            UmlSyntaxDiagnosticCode::MalformedMessage,
            lead,
            content_end,
            "malformed sequence operand",
        ));
    }
    children.push(behavior_recovery(f, recovery));
    push_behavior_newline(f, text, &mut children, p, newline, end);
    GreenElement::Node(f.node(UmlSyntaxKind::SequenceOperand, children).unwrap())
}

fn sequence_message(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    let (lead, content_end, newline) = behavior_bounds(source, start, end);
    let mut children = vec![token(
        f,
        text,
        start,
        lead,
        lead + 1,
        UmlSyntaxKind::BulletToken,
    )];
    let source_start = skip_ws(source, lead + 1, content_end);
    let source_end = scan_word(source, source_start, content_end);
    let mut valid = source_start < source_end;
    children.push(slot(
        f,
        UmlSyntaxKind::MessageSource,
        if valid {
            token(
                f,
                text,
                lead + 1,
                source_start,
                source_end,
                UmlSyntaxKind::SourceToken,
            )
        } else {
            GreenElement::Token(f.missing_token(UmlSyntaxKind::SourceToken))
        },
    ));
    let verb_start = skip_ws(source, source_end, content_end);
    let verb_end = scan_word(source, verb_start, content_end);
    let verb_valid = crate::model::MessageVerb::parse(&source[verb_start..verb_end]).is_some();
    children.push(slot(
        f,
        UmlSyntaxKind::MessageVerb,
        if verb_valid {
            token(
                f,
                text,
                source_end,
                verb_start,
                verb_end,
                UmlSyntaxKind::VerbToken,
            )
        } else {
            valid = false;
            GreenElement::Token(f.missing_token(UmlSyntaxKind::VerbToken))
        },
    ));
    let target_start = skip_ws(
        source,
        if verb_valid { verb_end } else { verb_start },
        content_end,
    );
    let target_end = source[target_start..content_end]
        .find(':')
        .map(|offset| target_start + offset)
        .unwrap_or_else(|| scan_word(source, target_start, content_end));
    let target_end = trim_end_ws(source, target_start, target_end);
    children.push(slot(
        f,
        UmlSyntaxKind::MessageTarget,
        if target_start < target_end {
            token(
                f,
                text,
                if verb_valid { verb_end } else { verb_start },
                target_start,
                target_end,
                UmlSyntaxKind::TargetToken,
            )
        } else {
            valid = false;
            GreenElement::Token(f.missing_token(UmlSyntaxKind::TargetToken))
        },
    ));
    let mut p = target_end;
    p = skip_ws(source, p, content_end);
    if p < content_end && source.as_bytes()[p] == b':' {
        children.push(token(f, text, p, p, p + 1, UmlSyntaxKind::ColonToken));
        let signature = skip_ws(source, p + 1, content_end);
        if let Some(q) = scan_backtick(source, signature, content_end) {
            children.push(slot(
                f,
                UmlSyntaxKind::MessageSignature,
                token(f, text, p + 1, signature, q, UmlSyntaxKind::SignatureToken),
            ));
            p = q;
        } else {
            children.push(slot(
                f,
                UmlSyntaxKind::MessageSignature,
                GreenElement::Token(f.missing_token(UmlSyntaxKind::SignatureToken)),
            ));
            p = signature;
            valid = false;
        }
    } else {
        children.push(GreenElement::Token(
            f.missing_token(UmlSyntaxKind::ColonToken),
        ));
        children.push(slot(
            f,
            UmlSyntaxKind::MessageSignature,
            GreenElement::Token(f.missing_token(UmlSyntaxKind::SignatureToken)),
        ));
    }
    let recovery = if p < content_end {
        let recovery = skipped(
            f,
            text,
            p,
            content_end,
            UmlSyntaxDiagnosticCode::MalformedMessage,
        );
        p = content_end;
        valid = false;
        Some(recovery)
    } else {
        None
    };
    if !valid {
        diags.push(diag(
            UmlSyntaxDiagnosticCode::MalformedMessage,
            lead,
            content_end,
            "malformed message",
        ));
    }
    children.push(behavior_recovery(f, recovery));
    push_behavior_newline(f, text, &mut children, p, newline, end);
    GreenElement::Node(f.node(UmlSyntaxKind::Message, children).unwrap())
}

fn message_self_target(element: &GreenElement<UmlLanguage>) -> bool {
    let GreenElement::Node(node) = element else {
        return false;
    };
    fn find(node: &waml_syntax::GreenNode<UmlLanguage>, kind: UmlSyntaxKind) -> Option<String> {
        node.children().iter().find_map(|child| match child {
            GreenElement::Token(token) if token.kind() == kind => {
                Some(token.text().write_to_string())
            }
            GreenElement::Node(child) => find(child, kind),
            GreenElement::Token(_) => None,
        })
    }
    let source = find(node, UmlSyntaxKind::SourceToken);
    let target = find(node, UmlSyntaxKind::TargetToken);
    source.is_some() && source == target
}

fn unsupported_sequence_body(body: &str) -> bool {
    let body = body.trim();
    body == "par"
        || body.starts_with("par ")
        || body == "coregion"
        || body.starts_with("coregion ")
        || body.contains(" coregion ")
        || body == "gate"
        || body.starts_with("gate ")
        || body.contains(" gate ")
        || body.starts_with("->")
        || body.ends_with("->")
        || body.starts_with("found ")
        || body.starts_with("lost ")
}

fn behavior_bounds(source: &str, start: usize, end: usize) -> (usize, usize, usize) {
    let newline = source[start..end]
        .find('\n')
        .map(|offset| start + offset)
        .unwrap_or(end);
    let content_end = start
        + source[start..newline]
            .trim_end_matches(['\r', ' ', '\t'])
            .len();
    (skip_ws(source, start, content_end), content_end, newline)
}

fn scan_word(source: &str, mut p: usize, end: usize) -> usize {
    while p < end && !source.as_bytes()[p].is_ascii_whitespace() {
        p += 1;
    }
    p
}

fn keyword_at(source: &str, p: usize, end: usize, keyword: &str) -> bool {
    source.get(p..p + keyword.len()) == Some(keyword)
        && p + keyword.len() <= end
        && (p + keyword.len() == end || source.as_bytes()[p + keyword.len()].is_ascii_whitespace())
}

fn scan_backtick(source: &str, p: usize, end: usize) -> Option<usize> {
    (p < end && source.as_bytes()[p] == b'`')
        .then(|| {
            source[p + 1..end]
                .find('`')
                .map(|offset| p + 1 + offset + 1)
        })
        .flatten()
}

fn find_clause(source: &str, p: usize, end: usize, clauses: &[&str]) -> usize {
    clauses
        .iter()
        .filter_map(|clause| source[p..end].find(clause).map(|offset| p + offset))
        .min()
        .unwrap_or(end)
}

fn trim_end_ws(source: &str, start: usize, mut end: usize) -> usize {
    while end > start && source.as_bytes()[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    end
}

fn skipped(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    start: usize,
    end: usize,
    code: UmlSyntaxDiagnosticCode,
) -> GreenElement<UmlLanguage> {
    GreenElement::Node(
        f.node(
            UmlSyntaxKind::SkippedTokensSyntax,
            [GreenElement::Token(
                f.bad_token(UmlSyntaxKind::BadToken, slice(text, start, end), code)
                    .unwrap(),
            )],
        )
        .unwrap(),
    )
}

fn slot(
    f: &GreenFactory<UmlLanguage>,
    kind: UmlSyntaxKind,
    child: GreenElement<UmlLanguage>,
) -> GreenElement<UmlLanguage> {
    GreenElement::Node(f.node(kind, [child]).unwrap())
}

fn missing_link(f: &GreenFactory<UmlLanguage>) -> GreenElement<UmlLanguage> {
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
            ],
        )
        .unwrap(),
    )
}

fn behavior_recovery(
    f: &GreenFactory<UmlLanguage>,
    recovery: Option<GreenElement<UmlLanguage>>,
) -> GreenElement<UmlLanguage> {
    slot(
        f,
        UmlSyntaxKind::BehaviorRecovery,
        recovery.unwrap_or_else(|| GreenElement::Token(f.missing_token(UmlSyntaxKind::BadToken))),
    )
}

fn push_behavior_newline(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    children: &mut Vec<GreenElement<UmlLanguage>>,
    leading: usize,
    newline: usize,
    end: usize,
) {
    if newline < end {
        children.push(token(
            f,
            text,
            leading.min(newline),
            newline,
            end,
            UmlSyntaxKind::NewlineToken,
        ));
    } else {
        children.push(GreenElement::Token(
            f.missing_token(UmlSyntaxKind::NewlineToken),
        ));
    }
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
        UmlSyntaxKind::LayoutSection => UmlSyntaxKind::LayoutStatement,
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
    if kind == UmlSyntaxKind::LayoutStatement {
        return Some(layout_statement(
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
    } else if kind == UmlSyntaxKind::Value {
        children.push(token(
            f,
            text,
            lead + 1,
            body,
            content_end,
            UmlSyntaxKind::IdentifierToken,
        ));
    } else if kind == UmlSyntaxKind::Member {
        let (link, next) = relationship_link(f, text, source, body, content_end, lead + 1, diags);
        children.push(link);
        let trailing = skip_ws(source, next, content_end);
        if trailing < content_end {
            children.push(GreenElement::Node(
                f.node(
                    UmlSyntaxKind::SkippedTokensSyntax,
                    [GreenElement::Token(
                        f.bad_token(
                            UmlSyntaxKind::BadToken,
                            slice(text, next, content_end),
                            UmlSyntaxDiagnosticCode::UnexpectedToken,
                        )
                        .unwrap(),
                    )],
                )
                .unwrap(),
            ));
            diags.push(diag(
                UmlSyntaxDiagnosticCode::UnexpectedToken,
                trailing,
                content_end,
                "unexpected member content",
            ));
        }
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

fn layout_statement(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    lead: usize,
    content_end: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> waml_syntax::GreenNode<UmlLanguage> {
    let mut children = vec![token(
        f,
        text,
        start,
        lead,
        lead + 1,
        UmlSyntaxKind::BulletToken,
    )];
    // One entry per successfully lexed atom.  The green elements retain the
    // exact authored bytes/trivia; the parallel spellings are used only to
    // choose their fixed grammar slots below.
    let mut atom_words: Vec<String> = Vec::new();
    let mut has_bad_atom = false;
    let mut at = lead + 1;
    while at < content_end {
        let token_start = at;
        at = skip_ws(source, at, content_end);
        if at == content_end {
            break;
        }
        let ch = source[at..].chars().next().expect("layout scalar");
        let (next, kind) = match ch {
            '(' => (at + 1, UmlSyntaxKind::LayoutOpenParenToken),
            ')' => (at + 1, UmlSyntaxKind::LayoutCloseParenToken),
            ',' => (at + 1, UmlSyntaxKind::LayoutCommaToken),
            '"' => match source[at + 1..content_end].find('"') {
                Some(n) => (at + n + 2, UmlSyntaxKind::LayoutQuoteToken),
                None => {
                    diags.push(diag(
                        UmlSyntaxDiagnosticCode::UnexpectedToken,
                        at,
                        content_end,
                        "unterminated layout quote",
                    ));
                    (content_end, UmlSyntaxKind::BadToken)
                }
            },
            '[' => match source[at..content_end].find(")") {
                Some(n)
                    if source[at..at + n + 1].contains("](")
                        && !source[at..at + n + 1].ends_with("]()") =>
                {
                    (at + n + 1, UmlSyntaxKind::LayoutLinkToken)
                }
                _ => {
                    let n = source[at..content_end]
                        .find(char::is_whitespace)
                        .map(|n| at + n)
                        .unwrap_or(content_end);
                    diags.push(diag(
                        UmlSyntaxDiagnosticCode::UnexpectedToken,
                        at,
                        n,
                        "malformed layout link",
                    ));
                    (n, UmlSyntaxKind::BadToken)
                }
            },
            _ => {
                let n = source[at..content_end]
                    .find(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | ',' | '[' | '"'))
                    .map(|n| at + n)
                    .unwrap_or(content_end);
                (n, UmlSyntaxKind::LayoutWordToken)
            }
        };
        if kind == UmlSyntaxKind::BadToken {
            has_bad_atom = true;
            children.push(GreenElement::Token(f.missing_token(match ch {
                '[' => UmlSyntaxKind::LayoutLinkToken,
                '"' => UmlSyntaxKind::LayoutQuoteToken,
                _ => UmlSyntaxKind::LayoutWordToken,
            })));
            children.push(GreenElement::Node(
                f.node(
                    UmlSyntaxKind::SkippedTokensSyntax,
                    [GreenElement::Token(
                        f.bad_token(
                            UmlSyntaxKind::BadToken,
                            slice(text, token_start, next),
                            UmlSyntaxDiagnosticCode::UnexpectedToken,
                        )
                        .unwrap(),
                    )],
                )
                .unwrap(),
            ));
        } else {
            children.push(token(f, text, token_start, token_start, next, kind));
            atom_words.push(source[at..next].trim().to_ascii_lowercase());
        }
        at = next.max(at + ch.len_utf8());
    }
    // Parse the authored atoms into fixed grammar slots. The shape parser
    // consumes atom indices only; the green elements below remain the sole
    // owners of source bytes.
    if children.len() > 1 && !has_bad_atom && !atom_words.is_empty() {
        let atoms = children.split_off(1);
        match parse_layout_shape(&atom_words) {
            Ok(LayoutShape::Alignment { left, join, right }) => {
                children.push(GreenElement::Node(
                    f.node(
                        UmlSyntaxKind::LayoutAlignment,
                        [
                            layout_anchored_node(
                                f,
                                atoms[left.clone()].to_vec(),
                                &atom_words[left],
                            ),
                            GreenElement::Node(
                                f.node(
                                    UmlSyntaxKind::DirectionClause,
                                    atoms[join.clone()].iter().cloned(),
                                )
                                .unwrap(),
                            ),
                            layout_anchored_node(
                                f,
                                atoms[right.clone()].to_vec(),
                                &atom_words[right],
                            ),
                        ],
                    )
                    .unwrap(),
                ));
            }
            Ok(LayoutShape::Placement {
                operands,
                directions,
            }) => {
                let mut slots = Vec::with_capacity(operands.len() + directions.len());
                for (index, operand) in operands.iter().enumerate() {
                    slots.push(layout_operand_node(
                        f,
                        atoms[operand.clone()].to_vec(),
                        &atom_words[operand.clone()],
                    ));
                    if let Some(direction) = directions.get(index) {
                        slots.push(GreenElement::Node(
                            f.node(
                                UmlSyntaxKind::DirectionClause,
                                atoms[direction.clone()].iter().cloned(),
                            )
                            .unwrap(),
                        ));
                    }
                }
                children.push(GreenElement::Node(
                    f.node(UmlSyntaxKind::LayoutPlacement, slots).unwrap(),
                ));
            }
            Ok(LayoutShape::Standalone(operand)) => {
                children.push(GreenElement::Node(
                    f.node(
                        UmlSyntaxKind::LayoutStandalone,
                        [layout_operand_node(
                            f,
                            atoms[operand.clone()].to_vec(),
                            &atom_words[operand],
                        )],
                    )
                    .unwrap(),
                ));
            }
            Err(error) => append_layout_recovery(f, &mut children, atoms, error),
        }
    }
    if children.len() == 1 {
        let bullet = children.pop().expect("layout bullet");
        children.push(GreenElement::Node(
            f.node(UmlSyntaxKind::SkippedTokensSyntax, [bullet])
                .unwrap(),
        ));
        children.push(GreenElement::Token(
            f.missing_token(UmlSyntaxKind::LayoutWordToken),
        ));
        diags.push(diag(
            UmlSyntaxDiagnosticCode::UnexpectedToken,
            lead,
            content_end,
            "missing layout statement",
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
    f.node(UmlSyntaxKind::LayoutStatement, children).unwrap()
}

#[derive(Clone, Debug)]
enum LayoutShape {
    Placement {
        operands: Vec<std::ops::Range<usize>>,
        directions: Vec<std::ops::Range<usize>>,
    },
    Alignment {
        left: std::ops::Range<usize>,
        join: std::ops::Range<usize>,
        right: std::ops::Range<usize>,
    },
    Standalone(std::ops::Range<usize>),
}

#[derive(Clone, Copy, Debug)]
struct LayoutShapeError {
    recovery_from: usize,
    missing_at: usize,
    missing: UmlSyntaxKind,
}

struct LayoutShapeCursor<'a> {
    words: &'a [String],
    pos: usize,
}

impl<'a> LayoutShapeCursor<'a> {
    fn word(&self) -> Option<&'a str> {
        self.words.get(self.pos).map(String::as_str)
    }

    fn eat(&mut self, expected: &str) -> bool {
        if self
            .word()
            .is_some_and(|word| word.eq_ignore_ascii_case(expected))
        {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn error(
        &self,
        recovery_from: usize,
        missing_at: usize,
        missing: UmlSyntaxKind,
    ) -> LayoutShapeError {
        LayoutShapeError {
            recovery_from,
            missing_at,
            missing,
        }
    }

    fn operand(&mut self) -> Result<std::ops::Range<usize>, LayoutShapeError> {
        let start = self.pos;
        self.reference()?;
        if self.eat("as") {
            let axis_at = self.pos;
            if !matches!(self.word(), Some("row") | Some("column")) {
                return Err(self.error(
                    start.max(axis_at.saturating_sub(1)),
                    axis_at,
                    UmlSyntaxKind::LayoutWordToken,
                ));
            }
            self.pos += 1;
        }
        if self.eat("with") {
            let with_at = self.pos - 1;
            self.hint(with_at)?;
            while matches!(self.word(), Some(",") | Some("and")) {
                let separator = self.pos;
                self.pos += 1;
                if let Err(mut error) = self.hint(separator) {
                    error.recovery_from = separator;
                    return Err(error);
                }
            }
        }
        Ok(start..self.pos)
    }

    fn reference(&mut self) -> Result<(), LayoutShapeError> {
        let start = self.pos;
        let Some(word) = self.word() else {
            return Err(self.error(
                start.saturating_sub(1),
                start,
                UmlSyntaxKind::LayoutWordToken,
            ));
        };
        if word == "(" {
            self.pos += 1;
            self.operand()?;
            if !self.eat(")") {
                return Err(self.error(start, self.pos, UmlSyntaxKind::LayoutCloseParenToken));
            }
            return Ok(());
        }
        if matches!(word, ")" | ",") {
            return Err(self.error(start, start, UmlSyntaxKind::LayoutWordToken));
        }
        if matches!(word, "row" | "column") {
            self.pos += 1;
            if !self.eat("of") {
                return Err(self.error(start, self.pos, UmlSyntaxKind::LayoutKeywordToken));
            }
            self.operand()?;
            while self.eat(",") {
                let separator = self.pos - 1;
                if let Err(mut error) = self.operand() {
                    error.recovery_from = separator;
                    return Err(error);
                }
            }
            return Ok(());
        }
        self.pos += 1;
        Ok(())
    }

    fn hint(&mut self, recovery_from: usize) -> Result<(), LayoutShapeError> {
        let Some(word) = self.word() else {
            return Err(self.error(recovery_from, self.pos, UmlSyntaxKind::LayoutWordToken));
        };
        match word {
            "frame" | "box" | "shrink" | "emphasized" | "collapsed" => {
                self.pos += 1;
                Ok(())
            }
            "no" | "small" | "medium" | "large" => {
                self.pos += 1;
                if self.eat("margin") || self.eat("margins") {
                    Ok(())
                } else {
                    Err(self.error(recovery_from, self.pos, UmlSyntaxKind::LayoutKeywordToken))
                }
            }
            _ => Err(self.error(recovery_from, self.pos, UmlSyntaxKind::LayoutWordToken)),
        }
    }

    fn anchored(&mut self) -> Result<(std::ops::Range<usize>, bool), LayoutShapeError> {
        let start = self.pos;
        let has_edge = matches!(
            self.word(),
            Some("top") | Some("bottom") | Some("left") | Some("right") | Some("center")
        ) && self
            .words
            .get(self.pos + 1)
            .is_some_and(|word| word == "of");
        if has_edge {
            self.pos += 2;
        }
        self.operand()?;
        Ok((start..self.pos, has_edge))
    }

    fn direction(&mut self) -> Result<Option<std::ops::Range<usize>>, LayoutShapeError> {
        let start = self.pos;
        match self.word() {
            Some("above") | Some("below") => {
                self.pos += 1;
                if matches!(self.word(), Some("left") | Some("right")) {
                    self.pos += 1;
                    if !self.eat("of") {
                        return Err(self.error(start, self.pos, UmlSyntaxKind::LayoutKeywordToken));
                    }
                }
                Ok(Some(start..self.pos))
            }
            Some("left") | Some("right") => {
                self.pos += 1;
                if !self.eat("of") {
                    return Err(self.error(start, self.pos, UmlSyntaxKind::LayoutKeywordToken));
                }
                Ok(Some(start..self.pos))
            }
            _ => Ok(None),
        }
    }
}

fn parse_layout_shape(words: &[String]) -> Result<LayoutShape, LayoutShapeError> {
    let mut cursor = LayoutShapeCursor { words, pos: 0 };
    let (first, first_has_edge) = cursor.anchored()?;
    if cursor.eat("aligned") {
        let join_start = cursor.pos - 1;
        if !cursor.eat("with") {
            return Err(cursor.error(join_start, cursor.pos, UmlSyntaxKind::LayoutKeywordToken));
        }
        let (right, _) = cursor.anchored()?;
        if cursor.pos != words.len() {
            return Err(cursor.error(cursor.pos, cursor.pos, UmlSyntaxKind::EndOfFileToken));
        }
        return Ok(LayoutShape::Alignment {
            left: first,
            join: join_start..join_start + 2,
            right,
        });
    }
    if first_has_edge {
        return Err(cursor.error(0, cursor.pos, UmlSyntaxKind::LayoutKeywordToken));
    }
    let mut operands = vec![first];
    let mut directions = Vec::new();
    while let Some(direction) = cursor.direction()? {
        let direction_start = direction.start;
        directions.push(direction);
        match cursor.operand() {
            Ok(operand) => operands.push(operand),
            Err(mut error) => {
                if error.missing_at == words.len() {
                    error.recovery_from = direction_start;
                }
                return Err(error);
            }
        }
    }
    if cursor.pos != words.len() {
        return Err(cursor.error(cursor.pos, cursor.pos, UmlSyntaxKind::EndOfFileToken));
    }
    if directions.is_empty() {
        Ok(LayoutShape::Standalone(operands.remove(0)))
    } else {
        Ok(LayoutShape::Placement {
            operands,
            directions,
        })
    }
}

fn append_layout_recovery(
    f: &GreenFactory<UmlLanguage>,
    children: &mut Vec<GreenElement<UmlLanguage>>,
    atoms: Vec<GreenElement<UmlLanguage>>,
    error: LayoutShapeError,
) {
    let from = error.recovery_from.min(atoms.len());
    let at = error.missing_at.clamp(from, atoms.len());
    children.extend(atoms[..from].iter().cloned());
    if from < at {
        children.push(GreenElement::Node(
            f.node(
                UmlSyntaxKind::SkippedTokensSyntax,
                atoms[from..at].iter().cloned(),
            )
            .unwrap(),
        ));
    }
    children.push(GreenElement::Token(f.missing_token(error.missing)));
    if at < atoms.len() {
        children.push(GreenElement::Node(
            f.node(
                UmlSyntaxKind::SkippedTokensSyntax,
                atoms[at..].iter().cloned(),
            )
            .unwrap(),
        ));
    }
}

fn layout_operand_node(
    f: &GreenFactory<UmlLanguage>,
    atoms: Vec<GreenElement<UmlLanguage>>,
    words: &[String],
) -> GreenElement<UmlLanguage> {
    let mut cursor = LayoutShapeCursor { words, pos: 0 };
    cursor.reference().expect("validated layout reference");
    let reference_end = cursor.pos;
    let axis_at = words
        .get(reference_end)
        .is_some_and(|word| word == "as")
        .then_some(reference_end);
    let hint_at = words
        .iter()
        .enumerate()
        .skip(reference_end)
        .find_map(|(index, word)| (word == "with").then_some(index));
    let mut atoms = atoms;
    let tail = atoms.split_off(reference_end);
    let mut children = vec![layout_ref_node(f, atoms, &words[..reference_end])];
    if let Some(axis_at) = axis_at {
        let mut tail = tail;
        let hint_offset = hint_at
            .map(|at| at.saturating_sub(axis_at))
            .unwrap_or(tail.len());
        let hints = tail.split_off(hint_offset.min(tail.len()));
        children.push(GreenElement::Node(
            f.node(UmlSyntaxKind::Axis, tail).unwrap(),
        ));
        if !hints.is_empty() {
            children.push(layout_hint_clause_node(
                f,
                hints,
                &words[axis_at + hint_offset.min(words.len() - axis_at)..],
            ));
        }
    } else if !tail.is_empty() {
        children.push(layout_hint_clause_node(
            f,
            tail,
            &words[hint_at.unwrap_or(words.len())..],
        ));
    }
    GreenElement::Node(f.node(UmlSyntaxKind::Operand, children).unwrap())
}

fn layout_ref_node(
    f: &GreenFactory<UmlLanguage>,
    atoms: Vec<GreenElement<UmlLanguage>>,
    words: &[String],
) -> GreenElement<UmlLanguage> {
    let children = if words.first().is_some_and(|word| word == "(")
        && words.last().is_some_and(|word| word == ")")
        && atoms.len() >= 2
    {
        vec![
            atoms[0].clone(),
            layout_operand_node(
                f,
                atoms[1..atoms.len() - 1].to_vec(),
                &words[1..words.len() - 1],
            ),
            atoms[atoms.len() - 1].clone(),
        ]
    } else if matches!(
        words.first().map(String::as_str),
        Some("row") | Some("column")
    ) && words.get(1).is_some_and(|word| word == "of")
        && atoms.len() >= 3
    {
        let mut children = vec![
            GreenElement::Node(f.node(UmlSyntaxKind::Axis, [atoms[0].clone()]).unwrap()),
            atoms[1].clone(),
        ];
        let mut cursor = LayoutShapeCursor { words, pos: 2 };
        while cursor.pos < words.len() {
            let item = cursor.operand().expect("validated inline group item");
            children.push(layout_operand_node(
                f,
                atoms[item.clone()].to_vec(),
                &words[item],
            ));
            if cursor.eat(",") {
                children.push(atoms[cursor.pos - 1].clone());
            } else {
                break;
            }
        }
        children
    } else {
        vec![GreenElement::Node(
            f.node(UmlSyntaxKind::NameRef, atoms).unwrap(),
        )]
    };
    GreenElement::Node(f.node(UmlSyntaxKind::OperandRef, children).unwrap())
}

fn layout_hint_clause_node(
    f: &GreenFactory<UmlLanguage>,
    atoms: Vec<GreenElement<UmlLanguage>>,
    words: &[String],
) -> GreenElement<UmlLanguage> {
    let mut children = Vec::new();
    let mut start = 0;
    for end in 0..=words.len() {
        let separator = end < words.len() && (words[end] == "and" || words[end] == ",");
        if !separator && end != words.len() {
            continue;
        }
        if start == 0 && words.first().is_some_and(|word| word == "with") {
            if let Some(with) = atoms.first().cloned() {
                children.push(with);
            }
            start = 1;
        }
        if start < end {
            let kind = match words.get(start).map(String::as_str) {
                Some("frame") | Some("box") | Some("shrink") => UmlSyntaxKind::Shape,
                Some("emphasized") | Some("collapsed") => UmlSyntaxKind::Flag,
                Some("no") | Some("small") | Some("medium") | Some("large") => {
                    UmlSyntaxKind::Margin
                }
                _ => UmlSyntaxKind::Hint,
            };
            children.push(GreenElement::Node(
                f.node(
                    UmlSyntaxKind::Hint,
                    [GreenElement::Node(
                        f.node(kind, atoms[start..end].iter().cloned()).unwrap(),
                    )],
                )
                .unwrap(),
            ));
        }
        if separator {
            children.push(atoms[end].clone());
            start = end + 1;
        }
    }
    GreenElement::Node(f.node(UmlSyntaxKind::HintClause, children).unwrap())
}

fn layout_anchored_node(
    f: &GreenFactory<UmlLanguage>,
    atoms: Vec<GreenElement<UmlLanguage>>,
    words: &[String],
) -> GreenElement<UmlLanguage> {
    let edge = matches!(
        words.first().map(String::as_str),
        Some("top") | Some("bottom") | Some("left") | Some("right") | Some("center")
    ) && words.get(1).is_some_and(|word| word == "of");
    let children = if edge {
        vec![
            GreenElement::Node(
                f.node(UmlSyntaxKind::Edge, atoms[..2].iter().cloned())
                    .unwrap(),
            ),
            layout_operand_node(f, atoms[2..].to_vec(), &words[2..]),
        ]
    } else {
        vec![layout_operand_node(f, atoms, words)]
    };
    GreenElement::Node(f.node(UmlSyntaxKind::Anchored, children).unwrap())
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

fn behavior_link(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    p: usize,
    end: usize,
    leading: usize,
    code: UmlSyntaxDiagnosticCode,
    message: &'static str,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> (GreenElement<UmlLanguage>, usize) {
    let before = diags.len();
    let result = relationship_link(f, text, source, p, end, leading, diags);
    let valid = p < end
        && source.as_bytes()[p] == b'['
        && source[p + 1..end].find("](").is_some()
        && source[p + 1..end].find(')').is_some();
    if !valid {
        diags.truncate(before);
        diags.push(diag(code, p, result.1, message));
    }
    result
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
    let q = scan_name(source, p, end).max(
        (p + source[p..end]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(0))
        .min(end),
    );
    diags.push(diag(
        UmlSyntaxDiagnosticCode::UnexpectedToken,
        p,
        q,
        "malformed relationship link",
    ));
    let leading_trivia = if leading < p {
        vec![f
            .trivia(TriviaKind::Whitespace, slice(text, leading, p))
            .unwrap()]
    } else {
        Vec::new()
    };
    let mut link = vec![
        GreenElement::Token(
            f.missing_token_with_leading(UmlSyntaxKind::OpenBracketToken, leading_trivia)
                .unwrap(),
        ),
        GreenElement::Token(f.missing_token(UmlSyntaxKind::LinkTextToken)),
        GreenElement::Token(f.missing_token(UmlSyntaxKind::CloseBracketToken)),
        GreenElement::Token(f.missing_token(UmlSyntaxKind::OpenBracketToken)),
        GreenElement::Token(f.missing_token(UmlSyntaxKind::LinkTargetToken)),
        GreenElement::Token(f.missing_token(UmlSyntaxKind::CloseBracketToken)),
    ];
    if p < q {
        link.push(GreenElement::Node(
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
        ));
    }
    (
        GreenElement::Node(f.node(UmlSyntaxKind::Link, link).unwrap()),
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
    if type_start < content_end && source.as_bytes()[type_start] == b'[' {
        let (link, next) = relationship_link(f, text, source, type_start, content_end, p, diags);
        c.push(GreenElement::Node(
            f.node(UmlSyntaxKind::TypeReference, [link]).unwrap(),
        ));
        p = next;
    } else {
        let type_end = source[type_start..content_end]
            .find(['[', '{'])
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
    }
    let mstart = skip_ws(source, p, content_end);
    if mstart < content_end && matches!(source.as_bytes()[mstart], b'[' | b'{') {
        let close_delimiter = if source.as_bytes()[mstart] == b'{' {
            '}'
        } else {
            ']'
        };
        if let Some(close) = source[mstart + 1..content_end]
            .find(close_delimiter)
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
