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
        if heading.level != 2 {
            continue;
        }
        let Some(section_kind) = section_kind(source, heading.text_range) else {
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
        if at < start {
            children.push(raw(&factory, &text, at, start));
        }
        let heading_end = line_end(source, start, end);
        let mut section = vec![raw(&factory, &text, start, heading_end)];
        if section_kind == UmlSyntaxKind::FlowSection {
            section.push(GreenElement::Node(flow_block(
                &factory,
                &text,
                source,
                heading_end,
                end,
                structure,
                &mut diagnostics,
            )));
            children.push(GreenElement::Node(
                factory.node(section_kind, section).unwrap(),
            ));
            at = end;
            continue;
        }
        if section_kind == UmlSyntaxKind::MessagesSection {
            section.extend(sequence_items(
                &factory,
                &text,
                source,
                heading_end,
                end,
                structure,
                &mut diagnostics,
            ));
            children.push(GreenElement::Node(
                factory.node(section_kind, section).unwrap(),
            ));
            at = end;
            continue;
        }
        if section_kind == UmlSyntaxKind::MembersSection {
            section.extend(member_items(
                &factory,
                &text,
                source,
                heading_end,
                end,
                structure,
                &mut diagnostics,
            ));
            children.push(GreenElement::Node(
                factory.node(section_kind, section).unwrap(),
            ));
            at = end;
            continue;
        }
        for (line_start, line_end) in lines_between(source, heading_end, end) {
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
                } else if section_kind == UmlSyntaxKind::LifelinesSection {
                    let line = source[line_start..line_end].trim_end_matches(['\r', '\n']);
                    if line.trim().is_empty() {
                        section.push(raw(&factory, &text, line_start, line_end));
                    } else {
                        section.push(behavior_item(
                            &factory,
                            &text,
                            source,
                            line_start,
                            line_end,
                            UmlSyntaxKind::Lifeline,
                            crate::grammar::parse_lifeline_line(line).is_ok(),
                            UmlSyntaxDiagnosticCode::MalformedLifeline,
                            "malformed lifeline",
                            &mut diagnostics,
                        ));
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
                current.extend(behavior_tokens(f, text, source, start, end, false));
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
                current.extend(behavior_tokens(f, text, source, start, end, false));
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
        let valid = if notes {
            trimmed.trim_start().starts_with("- ")
        } else {
            crate::grammar::parse_flow_bullet(trimmed).is_ok()
        };
        if valid {
            let kind = if !notes
                && matches!(
                    crate::grammar::parse_flow_bullet(trimmed),
                    Ok(crate::syntax::FlowBullet::Transition(_))
                ) {
                UmlSyntaxKind::FlowTransition
            } else {
                UmlSyntaxKind::Value
            };
            current.push(GreenElement::Node(
                f.node(kind, behavior_tokens(f, text, source, start, end, true))
                    .unwrap(),
            ));
        } else {
            let message = if notes {
                "malformed flow note"
            } else {
                "malformed flow bullet"
            };
            if !notes && trimmed.contains("transitions") {
                current.push(recovery_typed(
                    f,
                    text,
                    start,
                    end,
                    UmlSyntaxKind::FlowTransition,
                    UmlSyntaxDiagnosticCode::MalformedFlow,
                    message,
                    diags,
                ));
            } else {
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
        let body = line.trim_start();
        let (kind, valid, code, message) =
            if body == "- par" || body.contains(" coregion") || body.contains(" gate ") {
                (
                    UmlSyntaxKind::SkippedTokensSyntax,
                    false,
                    UmlSyntaxDiagnosticCode::UnsupportedSequenceForm,
                    "unsupported sequence form",
                )
            } else if body == "- alt" || body == "- opt" || body == "- loop" {
                (
                    UmlSyntaxKind::SequenceFragment,
                    true,
                    UmlSyntaxDiagnosticCode::MalformedMessage,
                    "",
                )
            } else if body.starts_with("- when ") || body == "- else" {
                (
                    UmlSyntaxKind::SequenceOperand,
                    leading >= 2,
                    UmlSyntaxDiagnosticCode::MalformedMessage,
                    "sequence operand outside a fragment",
                )
            } else {
                let parsed = crate::grammar::parse_message_line(body);
                let self_message = parsed
                    .as_ref()
                    .is_ok_and(|message| message.from == message.to);
                (
                    UmlSyntaxKind::Message,
                    parsed.is_ok() && !self_message,
                    if self_message {
                        UmlSyntaxDiagnosticCode::UnsupportedSequenceForm
                    } else {
                        UmlSyntaxDiagnosticCode::MalformedMessage
                    },
                    if self_message {
                        "self messages are not supported"
                    } else {
                        "malformed message"
                    },
                )
            };
        if malformed_indent {
            items.push(recovery_line(
                f,
                text,
                start,
                end,
                UmlSyntaxDiagnosticCode::MalformedIndentation,
                "sequence indentation must use pairs of spaces",
                diags,
            ));
        } else if valid {
            items.push(GreenElement::Node(
                f.node(kind, behavior_tokens(f, text, source, start, end, true))
                    .unwrap(),
            ));
        } else if kind == UmlSyntaxKind::Message {
            items.push(recovery_typed(
                f, text, start, end, kind, code, message, diags,
            ));
        } else {
            items.push(recovery_line(f, text, start, end, code, message, diags));
        }
    }
    items
}

fn behavior_item(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    kind: UmlSyntaxKind,
    valid: bool,
    code: UmlSyntaxDiagnosticCode,
    message: &'static str,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    if valid {
        GreenElement::Node(
            f.node(kind, behavior_tokens(f, text, source, start, end, true))
                .unwrap(),
        )
    } else {
        recovery_typed(f, text, start, end, kind, code, message, diags)
    }
}

fn recovery_typed(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    start: usize,
    end: usize,
    kind: UmlSyntaxKind,
    code: UmlSyntaxDiagnosticCode,
    message: &'static str,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    let recovery = recovery_line(f, text, start, end, code, message, diags);
    GreenElement::Node(f.node(kind, [recovery]).unwrap())
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
    diags.push(diag(code, start, end, message));
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

fn behavior_tokens(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    bullet: bool,
) -> Vec<GreenElement<UmlLanguage>> {
    let newline_start = source[start..end]
        .find('\n')
        .map(|offset| start + offset)
        .unwrap_or(end);
    let content_end = source[start..newline_start].trim_end_matches('\r').len() + start;
    let leading = skip_ws(source, start, content_end);
    let mut children = Vec::new();
    let mut at = leading;
    if bullet && at < content_end && source.as_bytes()[at] == b'-' {
        children.push(token(
            f,
            text,
            start,
            at,
            at + 1,
            UmlSyntaxKind::BulletToken,
        ));
        at += 1;
    }
    let mut word_index = 0usize;
    let mut previous = "";
    while at < content_end {
        let word = skip_ws(source, at, content_end);
        if word >= content_end {
            break;
        }
        let word_end = source[word..content_end]
            .find(char::is_whitespace)
            .map(|offset| word + offset)
            .unwrap_or(content_end);
        let spelling = &source[word..word_end];
        let message_verb = matches!(
            previous,
            "calls" | "sends" | "replies" | "creates" | "destroys"
        );
        let kind = if spelling.starts_with('[')
            || previous == "to"
            || message_verb
            || (bullet
                && word_index == 0
                && !matches!(
                    spelling,
                    "alt"
                        | "opt"
                        | "loop"
                        | "when"
                        | "else"
                        | "on"
                        | "transitions"
                        | "entry:"
                        | "do:"
                        | "exit:"
                        | "refines"
                        | "partition:"
                )) {
            UmlSyntaxKind::TargetToken
        } else if spelling.starts_with('`') {
            UmlSyntaxKind::ExpressionToken
        } else if matches!(
            spelling,
            "initial"
                | "final"
                | "decision"
                | "merge"
                | "fork"
                | "join"
                | "object"
                | "entry:"
                | "do:"
                | "exit:"
                | "refines"
                | "partition:"
                | "transitions"
        ) {
            UmlSyntaxKind::FlowKeywordToken
        } else if matches!(
            spelling,
            "calls"
                | "sends"
                | "replies"
                | "creates"
                | "destroys"
                | "alt"
                | "opt"
                | "loop"
                | "when"
                | "else"
        ) {
            UmlSyntaxKind::MessageKeywordToken
        } else {
            UmlSyntaxKind::IdentifierToken
        };
        children.push(token(f, text, at, word, word_end, kind));
        previous = spelling;
        word_index += 1;
        at = word_end;
    }
    if content_end < end {
        children.push(token(
            f,
            text,
            at,
            content_end,
            end,
            UmlSyntaxKind::NewlineToken,
        ));
    }
    children
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
                    if source[at..at + n + 1].contains("](./")
                        && source[at..at + n + 1].ends_with(".md)") =>
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
    let mut link = vec![
        GreenElement::Token(f.missing_token(UmlSyntaxKind::OpenBracketToken)),
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
