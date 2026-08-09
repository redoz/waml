use std::sync::Arc;

use crate::uml::vocabulary;

use super::{UmlLanguage, UmlSyntaxDiagnosticCode, UmlSyntaxKind};
use waml_syntax::{
    GreenElement, GreenFactory, GreenText, MarkdownStructureMap, SourceText, SyntaxIdentity,
    SyntaxSeverity, SyntaxTree, TextRange, TextSize, TreeDiagnostic, TriviaKind, WamlSectionKind,
};

#[derive(Clone, Copy)]
pub(super) struct Island {
    pub kind: UmlSyntaxKind,
    pub range: TextRange,
    pub owner: Option<SyntaxIdentity>,
    pub content_range: TextRange,
}

pub(super) fn islands(
    source_len: TextSize,
    structure: &MarkdownStructureMap,
) -> Option<Vec<Island>> {
    let mut result = Vec::new();
    let mut at = TextSize::try_from_usize(0).ok()?;
    for island in structure.islands.iter() {
        let start = island.heading_range.start();
        let end = island.content_range.end();
        if start < at || end < start || end > source_len {
            return None;
        }
        if at < start {
            result.push(Island {
                kind: UmlSyntaxKind::MarkdownRegion,
                range: TextRange::new(at, start).ok()?,
                owner: None,
                content_range: TextRange::new(at, start).ok()?,
            });
        }
        result.push(Island {
            kind: uml_section_kind(island.kind),
            range: TextRange::new(start, end).ok()?,
            owner: Some(island.owner),
            content_range: island.content_range,
        });
        at = end;
    }
    if at < source_len {
        result.push(Island {
            kind: UmlSyntaxKind::MarkdownRegion,
            range: TextRange::new(at, source_len).ok()?,
            owner: None,
            content_range: TextRange::new(at, source_len).ok()?,
        });
    }
    Some(result)
}

fn uml_section_kind(kind: WamlSectionKind) -> UmlSyntaxKind {
    match kind {
        WamlSectionKind::Attributes => UmlSyntaxKind::AttributesSection,
        WamlSectionKind::Values => UmlSyntaxKind::ValuesSection,
        WamlSectionKind::Slots => UmlSyntaxKind::SlotsSection,
        WamlSectionKind::Relationships => UmlSyntaxKind::RelationshipsSection,
        WamlSectionKind::Members => UmlSyntaxKind::MembersSection,
        WamlSectionKind::Layout => UmlSyntaxKind::LayoutSection,
        WamlSectionKind::Nodes => UmlSyntaxKind::FlowSection,
        WamlSectionKind::Lifelines => UmlSyntaxKind::LifelinesSection,
        WamlSectionKind::Messages => UmlSyntaxKind::MessagesSection,
        WamlSectionKind::Gates => UmlSyntaxKind::GatesSection,
    }
}

pub(super) fn parse(
    text: SourceText,
    structure: &MarkdownStructureMap,
) -> Arc<SyntaxTree<UmlLanguage>> {
    let factory = GreenFactory::<UmlLanguage>::new();
    let source = text.shared();
    let descriptors =
        islands(text.len(), structure).expect("markdown structure has ordered ranges");
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
    let content_start = island.content_range.start().to_usize();
    let content_end = island.content_range.end().to_usize();
    let mut section = vec![raw(factory, text, start, content_start)];
    if island.kind == UmlSyntaxKind::FlowSection {
        section.push(GreenElement::Node(flow_block(
            factory,
            text,
            source,
            content_start,
            content_end,
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
            content_start,
            content_end,
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
            content_start,
            content_end,
            structure,
            diagnostics,
        ));
        return GreenElement::Node(factory.node(island.kind, section).unwrap());
    }
    for (line_start, line_end) in lines_between(source, content_start, content_end) {
        let item_line = confirmed_list_item_line(structure, line_start)
            || tab_indented_item_line(structure, line_start);
        if opaque_line(structure, line_start, line_end) && !item_line {
            section.push(raw(factory, text, line_start, line_end));
        } else if island.kind == UmlSyntaxKind::AttributesSection {
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
        } else if island.kind == UmlSyntaxKind::GatesSection {
            let line = source[line_start..line_end].trim_end_matches(['\r', '\n']);
            if line.trim().is_empty() {
                section.push(raw(factory, text, line_start, line_end));
            } else {
                section.push(gate_line(
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
    let markers = source.as_bytes()[start..line_end]
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
    let lines = lines_between(source, from, to).collect::<Vec<_>>();
    let mut line_index = 0;
    while let Some(&(start, end)) = lines.get(line_index) {
        let mut next_index = line_index + 1;
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
                line_index = next_index;
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
                line_index = next_index;
                continue;
            }
        }
        let trimmed = source[start..end].trim_end_matches(['\r', '\n']);
        if trimmed.trim().is_empty() {
            (if have_node { &mut current } else { &mut roots }).push(raw(f, text, start, end));
            line_index = next_index;
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
            line_index = next_index;
            continue;
        }
        if notes && trimmed.trim_start().starts_with("- ") {
            current.push(flow_value_line(f, text, source, start, end));
        } else if !notes {
            let mut trace_lines = Vec::new();
            if !flow_line_is_internal(source, start, end) {
                let transition_indent = behavior_bounds(source, start, end).0 - start;
                while let Some(&(trace_start, trace_end)) = lines.get(next_index) {
                    if !is_flow_trace_continuation(
                        source,
                        trace_start,
                        trace_end,
                        transition_indent,
                    ) {
                        break;
                    }
                    trace_lines.push((trace_start, trace_end));
                    next_index += 1;
                }
            }
            current.push(flow_line(f, text, source, start, end, &trace_lines, diags));
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
        line_index = next_index;
    }
    close(&mut current, &mut roots, &mut have_node);
    f.node(UmlSyntaxKind::FlowBlock, roots).unwrap()
}

#[derive(Clone, Copy)]
struct SequenceIndentation {
    bytes: usize,
    spaces: usize,
    malformed: bool,
}

fn sequence_indentation(line: &str) -> SequenceIndentation {
    let mut bytes = 0;
    let mut spaces = 0;
    let mut has_tab = false;
    for byte in line.bytes() {
        match byte {
            b' ' => {
                bytes += 1;
                spaces += 1;
            }
            b'\t' => {
                bytes += 1;
                has_tab = true;
            }
            _ => break,
        }
    }
    SequenceIndentation {
        bytes,
        spaces,
        malformed: has_tab || spaces % 2 != 0,
    }
}

fn sequence_body(line: &str, indentation: SequenceIndentation) -> &str {
    let content = &line[indentation.bytes..];
    content.strip_prefix("- ").unwrap_or(content)
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
    let mut fragment_indents = Vec::new();
    let lines = lines_between(source, from, to).collect::<Vec<_>>();
    let mut line_index = 0;
    while let Some(&(start, end)) = lines.get(line_index) {
        line_index += 1;
        let line = source[start..end].trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            items.push(raw(f, text, start, end));
            continue;
        }
        let indentation = sequence_indentation(line);
        let leading = indentation.spaces;
        let malformed_indent = indentation.malformed;
        let significant_start = start + indentation.bytes;
        let content_end = start + line.len();
        let body = sequence_body(line, indentation);
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
            continue;
        }
        if opaque_line(structure, start, end) {
            items.push(raw(f, text, start, end));
            continue;
        }
        while fragment_indents
            .last()
            .copied()
            .is_some_and(|indent| indent >= leading)
        {
            fragment_indents.pop();
        }
        let nested_under_fragment = fragment_indents
            .last()
            .copied()
            .is_some_and(|indent| leading > indent);
        let operand_owned = fragment_indents
            .last()
            .copied()
            .is_some_and(|indent| leading == indent + 2);
        let fragment_head = matches!(
            body,
            "alt" | "opt" | "loop" | "par" | "break" | "critical" | "assert" | "neg"
        );
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
        } else if body == "ref" || body.starts_with("ref ") {
            let mut bindings = Vec::new();
            while let Some(&(binding_start, binding_end)) = lines.get(line_index) {
                let binding_source =
                    source[binding_start..binding_end].trim_end_matches(['\r', '\n']);
                let binding_indentation = sequence_indentation(binding_source);
                let binding_body = sequence_body(binding_source, binding_indentation);
                let is_binding = binding_body == "bind" || binding_body.starts_with("bind ");
                if binding_indentation.malformed && is_binding {
                    bindings.push(recovery_line(
                        f,
                        text,
                        binding_start,
                        binding_end,
                        UmlSyntaxDiagnosticCode::MalformedIndentation,
                        "sequence indentation must use pairs of spaces",
                        diags,
                    ));
                    line_index += 1;
                    continue;
                }
                if opaque_line(structure, binding_start, binding_end) {
                    break;
                }
                if binding_indentation.spaces != leading + 2 || !is_binding {
                    break;
                }
                bindings.push(binding_line(
                    f,
                    text,
                    source,
                    binding_start,
                    binding_end,
                    diags,
                ));
                line_index += 1;
            }
            items.push(interaction_use(
                f, text, source, start, end, bindings, diags,
            ));
        } else if fragment_head {
            fragment_indents.push(leading);
            items.push(sequence_fragment(f, text, source, start, end, diags));
        } else if body == "when"
            || body.starts_with("when ")
            || body == "else"
            || body.starts_with("else ")
            || body == "branch"
            || body.starts_with("branch ")
        {
            items.push(sequence_operand(
                f,
                text,
                source,
                start,
                end,
                operand_owned,
                diags,
            ));
        } else if nested_under_fragment && !canonical_message_body(body) {
            items.push(recovery_line_at(
                f,
                text,
                start,
                end,
                significant_start,
                content_end,
                UmlSyntaxDiagnosticCode::MalformedMessage,
                "unknown sequence operand",
                diags,
            ));
        } else {
            items.push(sequence_message(f, text, source, start, end, diags));
        }
    }
    items
}

fn gate_line(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    let (lead, content_end, newline) = behavior_bounds(source, start, end);
    let has_bullet = source.get(lead..lead + 2) == Some("- ");
    let bullet_end = if has_bullet { lead + 1 } else { lead };
    let mut children = vec![if has_bullet {
        token(f, text, start, lead, bullet_end, UmlSyntaxKind::BulletToken)
    } else {
        missing_token(f, text, start, lead, UmlSyntaxKind::BulletToken)
    }];
    let name_start = skip_ws(source, bullet_end, content_end);
    let name_end = scan_endpoint(source, name_start, content_end);
    children.push(slot(
        f,
        UmlSyntaxKind::GateName,
        if name_start == name_end {
            missing_token(
                f,
                text,
                bullet_end,
                name_start,
                UmlSyntaxKind::IdentifierToken,
            )
        } else {
            token(
                f,
                text,
                bullet_end,
                name_start,
                name_end,
                UmlSyntaxKind::IdentifierToken,
            )
        },
    ));
    let recovery = (name_end < content_end).then(|| {
        skipped(
            f,
            text,
            name_end,
            content_end,
            UmlSyntaxDiagnosticCode::MalformedMessage,
        )
    });
    if !has_bullet || name_start == name_end || recovery.is_some() {
        diags.push(diag(
            UmlSyntaxDiagnosticCode::MalformedMessage,
            lead,
            content_end,
            "malformed gate",
        ));
    }
    children.push(behavior_recovery(f, recovery));
    push_behavior_newline(f, text, &mut children, content_end, newline, end);
    GreenElement::Node(f.node(UmlSyntaxKind::Gate, children).unwrap())
}

fn interaction_use(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    bindings: Vec<GreenElement<UmlLanguage>>,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    let (lead, content_end, newline) = behavior_bounds(source, start, end);
    let has_bullet = source.get(lead..lead + 2) == Some("- ");
    let bullet_end = if has_bullet { lead + 1 } else { lead };
    let ref_start = skip_ws(source, bullet_end, content_end);
    let ref_end = scan_word(source, ref_start, content_end);
    let mut children = vec![
        if has_bullet {
            token(f, text, start, lead, bullet_end, UmlSyntaxKind::BulletToken)
        } else {
            missing_token(f, text, start, lead, UmlSyntaxKind::BulletToken)
        },
        token(
            f,
            text,
            bullet_end,
            ref_start,
            ref_end,
            UmlSyntaxKind::RefToken,
        ),
    ];
    let link_start = skip_ws(source, ref_end, content_end);
    let diagnostic_count = diags.len();
    let (link, mut p) = behavior_link(
        f,
        text,
        source,
        link_start,
        content_end,
        ref_end,
        UmlSyntaxDiagnosticCode::MalformedMessage,
        "malformed interaction-use link",
        diags,
    );
    let mut valid = has_bullet && ref_start < ref_end && diags.len() == diagnostic_count;
    children.push(link);
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
        let alias_end = scan_endpoint(source, alias_start, content_end);
        children.push(slot(
            f,
            UmlSyntaxKind::InteractionUseAlias,
            if alias_start == alias_end {
                valid = false;
                missing_token(
                    f,
                    text,
                    as_start + 2,
                    alias_start,
                    UmlSyntaxKind::AliasToken,
                )
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
        children.push(missing_token(f, text, p, as_start, UmlSyntaxKind::AsToken));
        children.push(slot(
            f,
            UmlSyntaxKind::InteractionUseAlias,
            GreenElement::Token(f.missing_token(UmlSyntaxKind::AliasToken)),
        ));
        p = as_start;
        valid = false;
    }
    let recovery = (p < content_end).then(|| {
        valid = false;
        skipped(
            f,
            text,
            p,
            content_end,
            UmlSyntaxDiagnosticCode::MalformedMessage,
        )
    });
    if !valid {
        diags.push(diag(
            UmlSyntaxDiagnosticCode::MalformedMessage,
            lead,
            content_end,
            "malformed interaction use",
        ));
    }
    children.push(behavior_recovery(f, recovery));
    push_behavior_newline(f, text, &mut children, content_end, newline, end);
    children.push(GreenElement::Node(
        f.node(UmlSyntaxKind::InteractionBindings, bindings)
            .unwrap(),
    ));
    GreenElement::Node(f.node(UmlSyntaxKind::InteractionUse, children).unwrap())
}

fn binding_line(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    let (lead, content_end, newline) = behavior_bounds(source, start, end);
    let has_bullet = source.get(lead..lead + 2) == Some("- ");
    let bullet_end = if has_bullet { lead + 1 } else { lead };
    let bind_start = skip_ws(source, bullet_end, content_end);
    let bind_end = scan_word(source, bind_start, content_end);
    let mut children = vec![
        if has_bullet {
            token(f, text, start, lead, bullet_end, UmlSyntaxKind::BulletToken)
        } else {
            missing_token(f, text, start, lead, UmlSyntaxKind::BulletToken)
        },
        token(
            f,
            text,
            bullet_end,
            bind_start,
            bind_end,
            UmlSyntaxKind::BindToken,
        ),
    ];
    let local_start = skip_ws(source, bind_end, content_end);
    let local_end = scan_endpoint(source, local_start, content_end);
    children.push(slot(
        f,
        UmlSyntaxKind::BindingLocal,
        if local_start == local_end || &source[local_start..local_end] == "to" {
            missing_token(f, text, bind_end, local_start, UmlSyntaxKind::LocalToken)
        } else {
            token(
                f,
                text,
                bind_end,
                local_start,
                local_end,
                UmlSyntaxKind::LocalToken,
            )
        },
    ));
    let to_start = skip_ws(source, local_end, content_end);
    let has_to = keyword_at(source, to_start, content_end, "to");
    children.push(if has_to {
        token(
            f,
            text,
            local_end,
            to_start,
            to_start + 2,
            UmlSyntaxKind::ToToken,
        )
    } else {
        missing_token(f, text, local_end, to_start, UmlSyntaxKind::ToToken)
    });
    let target_leading = if has_to { to_start + 2 } else { to_start };
    let target_start = skip_ws(source, target_leading, content_end);
    let target_end = scan_endpoint(source, target_start, content_end);
    children.push(slot(
        f,
        UmlSyntaxKind::BindingTarget,
        if target_start == target_end {
            missing_token(
                f,
                text,
                target_leading,
                target_start,
                UmlSyntaxKind::TargetToken,
            )
        } else {
            token(
                f,
                text,
                target_leading,
                target_start,
                target_end,
                UmlSyntaxKind::TargetToken,
            )
        },
    ));
    let recovery = (target_end < content_end).then(|| {
        skipped(
            f,
            text,
            target_end,
            content_end,
            UmlSyntaxDiagnosticCode::MalformedMessage,
        )
    });
    if !has_bullet
        || bind_start == bind_end
        || &source[bind_start..bind_end] != "bind"
        || local_start == local_end
        || &source[local_start..local_end] == "to"
        || !has_to
        || target_start == target_end
        || recovery.is_some()
    {
        diags.push(diag(
            UmlSyntaxDiagnosticCode::MalformedMessage,
            lead,
            content_end,
            "malformed interaction binding",
        ));
    }
    children.push(behavior_recovery(f, recovery));
    push_behavior_newline(f, text, &mut children, content_end, newline, end);
    GreenElement::Node(f.node(UmlSyntaxKind::Binding, children).unwrap())
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
                // The alias is optional, but `as` promises one.  Mark the
                // keyword itself rather than the whole bullet: the link ahead
                // of it parsed fine.
                diags.push(diag(
                    UmlSyntaxDiagnosticCode::MalformedLifeline,
                    as_start,
                    as_start + 2,
                    "expected a lifeline alias after \"as\"",
                ));
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
        let leading: Vec<_> = (p < as_start)
            .then(|| {
                f.trivia(TriviaKind::Whitespace, slice(text, p, as_start))
                    .unwrap()
            })
            .into_iter()
            .collect();
        children.push(GreenElement::Token(
            f.missing_token_with_leading(UmlSyntaxKind::AsToken, leading)
                .unwrap(),
        ));
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

#[allow(clippy::too_many_arguments)]
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
    trace_lines: &[(usize, usize)],
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
        flow_transition(f, text, source, start, end, trace_lines, diags)
    }
}

fn flow_line_is_internal(source: &str, start: usize, end: usize) -> bool {
    let (_, content_end, _) = behavior_bounds(source, start, end);
    let lead = skip_ws(source, start, content_end);
    let body = skip_ws(source, (lead + 1).min(content_end), content_end);
    let word_end = scan_word(source, body, content_end);
    matches!(
        &source[body..word_end],
        "entry:" | "do:" | "exit:" | "refines" | "partition:"
    )
}

fn is_flow_trace_continuation(
    source: &str,
    start: usize,
    end: usize,
    transition_indent: usize,
) -> bool {
    let (lead, content_end, _) = behavior_bounds(source, start, end);
    lead > start + transition_indent && keyword_at(source, lead, content_end, "traces")
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
    trace_lines: &[(usize, usize)],
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
    let target_end = find_clause(source, p, content_end, &[" carries ", ": ", " traces "]);
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
        let link_end = find_clause(source, link_start, content_end, &[": ", " traces "]);
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
    p = skip_ws(source, p, content_end);
    let mut trace_children = Vec::new();
    while keyword_at(source, p, content_end, "traces") {
        let clause_end = find_clause(source, p + 6, content_end, &[" traces "]);
        let (trace, next, trace_valid) =
            flow_trace_clause(f, text, source, owned, p, clause_end, diags);
        trace_children.push(trace);
        owned = next;
        p = skip_ws(source, next, content_end);
        valid &= trace_valid;
    }
    if !trace_lines.is_empty() {
        push_behavior_newline(f, text, &mut trace_children, owned, newline, end);
        for &(trace_start, trace_end) in trace_lines {
            let (lead, trace_content_end, trace_newline) =
                behavior_bounds(source, trace_start, trace_end);
            let (trace, next, trace_valid) =
                flow_trace_clause(f, text, source, trace_start, lead, trace_content_end, diags);
            trace_children.push(trace);
            push_behavior_newline(f, text, &mut trace_children, next, trace_newline, trace_end);
            valid &= trace_valid;
        }
    }
    children.push(GreenElement::Node(
        f.node(UmlSyntaxKind::FlowTraces, trace_children).unwrap(),
    ));
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
    if trace_lines.is_empty() {
        push_behavior_newline(f, text, &mut children, owned, newline, end);
    } else {
        children.push(missing_token(
            f,
            text,
            trace_lines.last().map_or(end, |(_, end)| *end),
            trace_lines.last().map_or(end, |(_, end)| *end),
            UmlSyntaxKind::NewlineToken,
        ));
    }
    GreenElement::Node(f.node(UmlSyntaxKind::FlowTransition, children).unwrap())
}

fn flow_trace_clause(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    owned: usize,
    start: usize,
    end: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> (GreenElement<UmlLanguage>, usize, bool) {
    let keyword_end = (start + 6).min(end);
    let keyword = token(
        f,
        text,
        owned,
        start,
        keyword_end,
        UmlSyntaxKind::FlowKeywordToken,
    );
    let link_start = skip_ws(source, keyword_end, end);
    let (link, next) = behavior_link(
        f,
        text,
        source,
        link_start,
        end,
        keyword_end,
        UmlSyntaxDiagnosticCode::MalformedFlow,
        "malformed transition trace",
        diags,
    );
    let recovery =
        (next < end).then(|| skipped(f, text, next, end, UmlSyntaxDiagnosticCode::MalformedFlow));
    let valid = keyword_at(source, start, end, "traces")
        && link_start < end
        && source.as_bytes().get(link_start) == Some(&b'[')
        && next == end;
    (
        GreenElement::Node(
            f.node(
                UmlSyntaxKind::FlowTrace,
                [keyword, link, behavior_recovery(f, recovery)],
            )
            .unwrap(),
        ),
        end,
        valid,
    )
}

fn sequence_fragment(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    let (lead, content_end, newline) = behavior_bounds(source, start, end);
    let has_bullet = source.get(lead..lead + 2) == Some("- ");
    let body_start = if has_bullet { lead + 1 } else { lead };
    let kind_start = skip_ws(source, body_start, content_end);
    let mut children = vec![
        if has_bullet {
            token(f, text, start, lead, lead + 1, UmlSyntaxKind::BulletToken)
        } else {
            missing_token(f, text, start, lead, UmlSyntaxKind::BulletToken)
        },
        slot(
            f,
            UmlSyntaxKind::FragmentKind,
            token(
                f,
                text,
                body_start,
                kind_start,
                content_end,
                UmlSyntaxKind::FragmentKindToken,
            ),
        ),
    ];
    if !has_bullet {
        diags.push(diag(
            UmlSyntaxDiagnosticCode::MalformedMessage,
            lead,
            content_end,
            "missing sequence fragment bullet",
        ));
    }
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
    let has_bullet = source.get(lead..lead + 2) == Some("- ");
    let body_start = if has_bullet { lead + 1 } else { lead };
    let keyword = skip_ws(source, body_start, content_end);
    let keyword_end = scan_word(source, keyword, content_end);
    let mut children = vec![
        if has_bullet {
            token(f, text, start, lead, lead + 1, UmlSyntaxKind::BulletToken)
        } else {
            missing_token(f, text, start, lead, UmlSyntaxKind::BulletToken)
        },
        token(
            f,
            text,
            body_start,
            keyword,
            keyword_end,
            UmlSyntaxKind::OperandKeywordToken,
        ),
    ];
    let mut p = keyword_end;
    let mut valid = nested && has_bullet;
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
                missing_token(f, text, keyword_end, guard, UmlSyntaxKind::GuardToken),
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
    if &source[keyword..keyword_end] == "branch" {
        let label = skip_ws(source, keyword_end, content_end);
        if label == content_end {
            children.push(slot(
                f,
                UmlSyntaxKind::OperandBranchLabel,
                GreenElement::Token(f.missing_token(UmlSyntaxKind::BranchLabelToken)),
            ));
            p = label;
        } else if let Some(q) = scan_backtick(source, label, content_end) {
            children.push(slot(
                f,
                UmlSyntaxKind::OperandBranchLabel,
                token(
                    f,
                    text,
                    keyword_end,
                    label,
                    q,
                    UmlSyntaxKind::BranchLabelToken,
                ),
            ));
            p = q;
        } else {
            children.push(slot(
                f,
                UmlSyntaxKind::OperandBranchLabel,
                missing_token(f, text, keyword_end, label, UmlSyntaxKind::BranchLabelToken),
            ));
            p = label;
            valid = false;
        }
    } else {
        children.push(slot(
            f,
            UmlSyntaxKind::OperandBranchLabel,
            GreenElement::Token(f.missing_token(UmlSyntaxKind::BranchLabelToken)),
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
    let has_bullet = source.get(lead..lead + 1) == Some("-");
    let body_start = if has_bullet { lead + 1 } else { lead };
    let mut children = vec![if has_bullet {
        token(f, text, start, lead, lead + 1, UmlSyntaxKind::BulletToken)
    } else {
        missing_token(f, text, start, lead, UmlSyntaxKind::BulletToken)
    }];
    let source_start = skip_ws(source, body_start, content_end);
    let source_end = scan_word(source, source_start, content_end);
    if source_start == source_end {
        return malformed_message(f, text, source, start, end, diags);
    }
    if !sequence_endpoint_valid(&source[source_start..source_end]) {
        return malformed_message(f, text, source, start, end, diags);
    }
    children.push(slot(
        f,
        UmlSyntaxKind::MessageSource,
        token(
            f,
            text,
            body_start,
            source_start,
            source_end,
            UmlSyntaxKind::SourceToken,
        ),
    ));
    let verb_start = skip_ws(source, source_end, content_end);
    let verb_end = scan_word(source, verb_start, content_end);
    let verb = &source[verb_start..verb_end];
    if matches!(verb, "replies" | "sends") {
        return unsupported_message(f, text, source, start, end, diags);
    }
    if !vocabulary::MESSAGE_VERBS.contains(&verb) {
        return malformed_message(f, text, source, start, end, diags);
    }
    children.push(slot(
        f,
        UmlSyntaxKind::MessageVerb,
        token(
            f,
            text,
            source_end,
            verb_start,
            verb_end,
            UmlSyntaxKind::VerbToken,
        ),
    ));

    if matches!(verb, "calls" | "returns" | "signals")
        && contains_unquoted_colon(source, verb_end, content_end)
    {
        return unsupported_message(f, text, source, start, end, diags);
    }

    let tail = match verb {
        "calls" => parse_call_tail(f, text, source, verb_end, content_end, &mut children),
        "returns" => parse_return_tail(f, text, source, verb_end, content_end, &mut children),
        "signals" => parse_signal_tail(f, text, source, verb_end, content_end, &mut children),
        "creates" | "destroys" => {
            parse_other_message_tail(f, text, source, verb_end, content_end, &mut children)
        }
        "replies" | "sends" => return unsupported_message(f, text, source, start, end, diags),
        _ => return malformed_message(f, text, source, start, end, diags),
    };
    if !tail.valid {
        diags.push(diag(
            UmlSyntaxDiagnosticCode::MalformedMessage,
            lead,
            content_end,
            "malformed message",
        ));
    }
    children.push(behavior_recovery(f, tail.recovery));
    push_behavior_newline(f, text, &mut children, tail.end, newline, end);
    GreenElement::Node(f.node(UmlSyntaxKind::Message, children).unwrap())
}

struct MessageTail {
    end: usize,
    valid: bool,
    recovery: Option<GreenElement<UmlLanguage>>,
}

fn parse_call_tail(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    verb_end: usize,
    content_end: usize,
    children: &mut Vec<GreenElement<UmlLanguage>>,
) -> MessageTail {
    let target_start = skip_ws(source, verb_end, content_end);
    let target_end = scan_endpoint(source, target_start, content_end);
    let target = &source[target_start..target_end];
    let target_valid = target_start < target_end
        && !matches!(target, "async" | "as")
        && source.as_bytes()[target_start] != b'`'
        && sequence_endpoint_valid(target);
    children.push(slot(
        f,
        UmlSyntaxKind::MessageTarget,
        if target_valid {
            token(
                f,
                text,
                verb_end,
                target_start,
                target_end,
                UmlSyntaxKind::TargetToken,
            )
        } else {
            missing_token(f, text, verb_end, target_start, UmlSyntaxKind::TargetToken)
        },
    ));
    let mut owned = if target_valid {
        target_end
    } else {
        target_start
    };
    let mut valid = target_valid;

    let p = skip_ws(source, owned, content_end);
    if keyword_at(source, p, content_end, "async") {
        let q = p + "async".len();
        children.push(slot(
            f,
            UmlSyntaxKind::MessageAsync,
            token(f, text, owned, p, q, UmlSyntaxKind::AsyncToken),
        ));
        owned = q;
    } else {
        children.push(missing_message_slot(
            f,
            UmlSyntaxKind::MessageAsync,
            UmlSyntaxKind::AsyncToken,
        ));
    }
    children.push(GreenElement::Token(
        f.missing_token(UmlSyntaxKind::ColonToken),
    ));

    let p = skip_ws(source, owned, content_end);
    if p < content_end && source.as_bytes()[p] == b'`' {
        if let Some(q) = scan_backtick(source, p, content_end) {
            children.push(slot(
                f,
                UmlSyntaxKind::MessageValue,
                token(f, text, owned, p, q, UmlSyntaxKind::ValueToken),
            ));
            owned = q;
        } else {
            children.push(slot(
                f,
                UmlSyntaxKind::MessageValue,
                missing_token(f, text, owned, p, UmlSyntaxKind::ValueToken),
            ));
            owned = p;
            valid = false;
        }
    } else {
        children.push(missing_message_slot(
            f,
            UmlSyntaxKind::MessageValue,
            UmlSyntaxKind::ValueToken,
        ));
    }

    let p = skip_ws(source, owned, content_end);
    if keyword_at(source, p, content_end, "as") {
        let q = p + "as".len();
        children.push(token(f, text, owned, p, q, UmlSyntaxKind::AsToken));
        owned = q;
        let name_start = skip_ws(source, owned, content_end);
        let name_end = scan_word(source, name_start, content_end);
        children.push(slot(
            f,
            UmlSyntaxKind::MessageCallId,
            if name_start < name_end {
                token(
                    f,
                    text,
                    owned,
                    name_start,
                    name_end,
                    UmlSyntaxKind::CallIdToken,
                )
            } else {
                valid = false;
                missing_token(f, text, owned, name_start, UmlSyntaxKind::CallIdToken)
            },
        ));
        owned = name_end;
    } else {
        children.push(GreenElement::Token(f.missing_token(UmlSyntaxKind::AsToken)));
        children.push(missing_message_slot(
            f,
            UmlSyntaxKind::MessageCallId,
            UmlSyntaxKind::CallIdToken,
        ));
    }
    push_missing_return_slots(f, children);
    finish_message_tail(f, text, owned, content_end, valid)
}

fn parse_return_tail(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    verb_end: usize,
    content_end: usize,
    children: &mut Vec<GreenElement<UmlLanguage>>,
) -> MessageTail {
    children.push(missing_message_slot(
        f,
        UmlSyntaxKind::MessageTarget,
        UmlSyntaxKind::TargetToken,
    ));
    children.push(missing_message_slot(
        f,
        UmlSyntaxKind::MessageAsync,
        UmlSyntaxKind::AsyncToken,
    ));
    children.push(GreenElement::Token(
        f.missing_token(UmlSyntaxKind::ColonToken),
    ));
    let mut owned = verb_end;
    let mut valid = true;
    let p = skip_ws(source, owned, content_end);
    if p < content_end && source.as_bytes()[p] == b'`' {
        if let Some(q) = scan_backtick(source, p, content_end) {
            children.push(slot(
                f,
                UmlSyntaxKind::MessageValue,
                token(f, text, owned, p, q, UmlSyntaxKind::ValueToken),
            ));
            owned = q;
        } else {
            children.push(slot(
                f,
                UmlSyntaxKind::MessageValue,
                missing_token(f, text, owned, p, UmlSyntaxKind::ValueToken),
            ));
            owned = p;
            valid = false;
        }
    } else {
        children.push(missing_message_slot(
            f,
            UmlSyntaxKind::MessageValue,
            UmlSyntaxKind::ValueToken,
        ));
    }
    children.push(GreenElement::Token(f.missing_token(UmlSyntaxKind::AsToken)));
    children.push(missing_message_slot(
        f,
        UmlSyntaxKind::MessageCallId,
        UmlSyntaxKind::CallIdToken,
    ));

    let p = skip_ws(source, owned, content_end);
    if keyword_at(source, p, content_end, "to") {
        let q = p + "to".len();
        children.push(token(f, text, owned, p, q, UmlSyntaxKind::ToToken));
        owned = q;
        let target_start = skip_ws(source, owned, content_end);
        let target_end = scan_endpoint(source, target_start, content_end);
        let target = &source[target_start..target_end];
        let target_valid = target_start < target_end
            && target != "for"
            && target != "async"
            && source.as_bytes()[target_start] != b'`'
            && sequence_endpoint_valid(target);
        children.push(slot(
            f,
            UmlSyntaxKind::MessageReturnTarget,
            if target_valid {
                token(
                    f,
                    text,
                    owned,
                    target_start,
                    target_end,
                    UmlSyntaxKind::ReturnTargetToken,
                )
            } else {
                valid = false;
                missing_token(
                    f,
                    text,
                    owned,
                    target_start,
                    UmlSyntaxKind::ReturnTargetToken,
                )
            },
        ));
        owned = if target_valid {
            target_end
        } else {
            target_start
        };
    } else {
        children.push(GreenElement::Token(f.missing_token(UmlSyntaxKind::ToToken)));
        children.push(missing_message_slot(
            f,
            UmlSyntaxKind::MessageReturnTarget,
            UmlSyntaxKind::ReturnTargetToken,
        ));
    }

    let p = skip_ws(source, owned, content_end);
    if keyword_at(source, p, content_end, "for") {
        let q = p + "for".len();
        children.push(token(f, text, owned, p, q, UmlSyntaxKind::ForToken));
        owned = q;
        let call_start = skip_ws(source, owned, content_end);
        let call_end = scan_word(source, call_start, content_end);
        children.push(slot(
            f,
            UmlSyntaxKind::MessageReturnCall,
            if call_start < call_end {
                token(
                    f,
                    text,
                    owned,
                    call_start,
                    call_end,
                    UmlSyntaxKind::ReturnCallToken,
                )
            } else {
                valid = false;
                missing_token(f, text, owned, call_start, UmlSyntaxKind::ReturnCallToken)
            },
        ));
        owned = call_end;
    } else {
        children.push(GreenElement::Token(
            f.missing_token(UmlSyntaxKind::ForToken),
        ));
        children.push(missing_message_slot(
            f,
            UmlSyntaxKind::MessageReturnCall,
            UmlSyntaxKind::ReturnCallToken,
        ));
    }
    finish_message_tail(f, text, owned, content_end, valid)
}

fn parse_signal_tail(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    verb_end: usize,
    content_end: usize,
    children: &mut Vec<GreenElement<UmlLanguage>>,
) -> MessageTail {
    let target_start = skip_ws(source, verb_end, content_end);
    let target_end = scan_endpoint(source, target_start, content_end);
    let target = &source[target_start..target_end];
    let target_valid = target_start < target_end
        && target != "async"
        && source.as_bytes()[target_start] != b'`'
        && sequence_endpoint_valid(target);
    children.push(slot(
        f,
        UmlSyntaxKind::MessageTarget,
        if target_valid {
            token(
                f,
                text,
                verb_end,
                target_start,
                target_end,
                UmlSyntaxKind::TargetToken,
            )
        } else {
            missing_token(f, text, verb_end, target_start, UmlSyntaxKind::TargetToken)
        },
    ));
    children.push(missing_message_slot(
        f,
        UmlSyntaxKind::MessageAsync,
        UmlSyntaxKind::AsyncToken,
    ));
    children.push(GreenElement::Token(
        f.missing_token(UmlSyntaxKind::ColonToken),
    ));
    let mut owned = if target_valid {
        target_end
    } else {
        target_start
    };
    let mut valid = target_valid;
    let p = skip_ws(source, owned, content_end);
    if p < content_end && source.as_bytes()[p] == b'`' {
        if let Some(q) = scan_backtick(source, p, content_end) {
            children.push(slot(
                f,
                UmlSyntaxKind::MessageValue,
                token(f, text, owned, p, q, UmlSyntaxKind::ValueToken),
            ));
            owned = q;
        } else {
            children.push(slot(
                f,
                UmlSyntaxKind::MessageValue,
                missing_token(f, text, owned, p, UmlSyntaxKind::ValueToken),
            ));
            owned = p;
            valid = false;
        }
    } else {
        children.push(missing_message_slot(
            f,
            UmlSyntaxKind::MessageValue,
            UmlSyntaxKind::ValueToken,
        ));
    }
    push_missing_call_and_return_slots(f, children);
    finish_message_tail(f, text, owned, content_end, valid)
}

fn parse_other_message_tail(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    verb_end: usize,
    content_end: usize,
    children: &mut Vec<GreenElement<UmlLanguage>>,
) -> MessageTail {
    let target_start = skip_ws(source, verb_end, content_end);
    let target_end = scan_endpoint(source, target_start, content_end);
    let target = &source[target_start..target_end];
    let target_valid = target_start < target_end
        && target != "async"
        && source.as_bytes()[target_start] != b'`'
        && sequence_endpoint_valid(target);
    children.push(slot(
        f,
        UmlSyntaxKind::MessageTarget,
        if target_valid {
            token(
                f,
                text,
                verb_end,
                target_start,
                target_end,
                UmlSyntaxKind::TargetToken,
            )
        } else {
            missing_token(f, text, verb_end, target_start, UmlSyntaxKind::TargetToken)
        },
    ));
    children.push(missing_message_slot(
        f,
        UmlSyntaxKind::MessageAsync,
        UmlSyntaxKind::AsyncToken,
    ));
    let mut owned = if target_valid {
        target_end
    } else {
        target_start
    };
    let mut valid = target_valid;
    let p = skip_ws(source, owned, content_end);
    if p < content_end && source.as_bytes()[p] == b':' {
        let colon_end = p + 1;
        children.push(token(
            f,
            text,
            owned,
            p,
            colon_end,
            UmlSyntaxKind::ColonToken,
        ));
        owned = colon_end;
        let value_start = skip_ws(source, colon_end, content_end);
        if let Some(value_end) = scan_backtick(source, value_start, content_end) {
            children.push(slot(
                f,
                UmlSyntaxKind::MessageValue,
                token(
                    f,
                    text,
                    owned,
                    value_start,
                    value_end,
                    UmlSyntaxKind::ValueToken,
                ),
            ));
            owned = value_end;
        } else {
            children.push(slot(
                f,
                UmlSyntaxKind::MessageValue,
                missing_token(f, text, owned, value_start, UmlSyntaxKind::ValueToken),
            ));
            owned = value_start;
            valid = false;
        }
    } else {
        children.push(GreenElement::Token(
            f.missing_token(UmlSyntaxKind::ColonToken),
        ));
        children.push(missing_message_slot(
            f,
            UmlSyntaxKind::MessageValue,
            UmlSyntaxKind::ValueToken,
        ));
    }
    push_missing_call_and_return_slots(f, children);
    finish_message_tail(f, text, owned, content_end, valid)
}

fn missing_message_slot(
    f: &GreenFactory<UmlLanguage>,
    slot_kind: UmlSyntaxKind,
    token_kind: UmlSyntaxKind,
) -> GreenElement<UmlLanguage> {
    slot(
        f,
        slot_kind,
        GreenElement::Token(f.missing_token(token_kind)),
    )
}

fn push_missing_return_slots(
    f: &GreenFactory<UmlLanguage>,
    children: &mut Vec<GreenElement<UmlLanguage>>,
) {
    children.push(GreenElement::Token(f.missing_token(UmlSyntaxKind::ToToken)));
    children.push(missing_message_slot(
        f,
        UmlSyntaxKind::MessageReturnTarget,
        UmlSyntaxKind::ReturnTargetToken,
    ));
    children.push(GreenElement::Token(
        f.missing_token(UmlSyntaxKind::ForToken),
    ));
    children.push(missing_message_slot(
        f,
        UmlSyntaxKind::MessageReturnCall,
        UmlSyntaxKind::ReturnCallToken,
    ));
}

fn push_missing_call_and_return_slots(
    f: &GreenFactory<UmlLanguage>,
    children: &mut Vec<GreenElement<UmlLanguage>>,
) {
    children.push(GreenElement::Token(f.missing_token(UmlSyntaxKind::AsToken)));
    children.push(missing_message_slot(
        f,
        UmlSyntaxKind::MessageCallId,
        UmlSyntaxKind::CallIdToken,
    ));
    push_missing_return_slots(f, children);
}

fn finish_message_tail(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    owned: usize,
    content_end: usize,
    mut valid: bool,
) -> MessageTail {
    let recovery = if owned < content_end {
        valid = false;
        Some(skipped(
            f,
            text,
            owned,
            content_end,
            UmlSyntaxDiagnosticCode::MalformedMessage,
        ))
    } else {
        None
    };
    MessageTail {
        end: if recovery.is_some() {
            content_end
        } else {
            owned
        },
        valid,
        recovery,
    }
}

fn malformed_message(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    let (lead, content_end, newline) = behavior_bounds(source, start, end);
    let has_bullet = source.get(lead..lead + 1) == Some("-");
    let body_start = if has_bullet { lead + 1 } else { lead };
    let source_start = skip_ws(source, body_start, content_end);
    let source_end = scan_word(source, source_start, content_end);
    let verb_start = skip_ws(source, source_end, content_end);
    let mut children = vec![if has_bullet {
        token(f, text, start, lead, lead + 1, UmlSyntaxKind::BulletToken)
    } else {
        missing_token(f, text, start, lead, UmlSyntaxKind::BulletToken)
    }];
    children.push(slot(
        f,
        UmlSyntaxKind::MessageSource,
        if source_start < source_end {
            token(
                f,
                text,
                body_start,
                source_start,
                source_end,
                UmlSyntaxKind::SourceToken,
            )
        } else {
            missing_token(
                f,
                text,
                body_start,
                source_start,
                UmlSyntaxKind::SourceToken,
            )
        },
    ));
    children.push(slot(
        f,
        UmlSyntaxKind::MessageVerb,
        missing_token(f, text, source_end, verb_start, UmlSyntaxKind::VerbToken),
    ));
    children.push(missing_message_slot(
        f,
        UmlSyntaxKind::MessageTarget,
        UmlSyntaxKind::TargetToken,
    ));
    children.push(missing_message_slot(
        f,
        UmlSyntaxKind::MessageAsync,
        UmlSyntaxKind::AsyncToken,
    ));
    children.push(missing_message_slot(
        f,
        UmlSyntaxKind::MessageValue,
        UmlSyntaxKind::ValueToken,
    ));
    push_missing_call_and_return_slots(f, &mut children);
    children.push(GreenElement::Token(
        f.missing_token(UmlSyntaxKind::ColonToken),
    ));
    let recovery = (verb_start < content_end).then(|| {
        skipped(
            f,
            text,
            verb_start,
            content_end,
            UmlSyntaxDiagnosticCode::MalformedMessage,
        )
    });
    diags.push(diag(
        UmlSyntaxDiagnosticCode::MalformedMessage,
        lead,
        content_end,
        "malformed message",
    ));
    children.push(behavior_recovery(f, recovery));
    push_behavior_newline(f, text, &mut children, content_end, newline, end);
    GreenElement::Node(f.node(UmlSyntaxKind::Message, children).unwrap())
}

fn unsupported_message(
    f: &GreenFactory<UmlLanguage>,
    text: &SourceText,
    source: &str,
    start: usize,
    end: usize,
    diags: &mut Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>>,
) -> GreenElement<UmlLanguage> {
    let (lead, content_end, _) = behavior_bounds(source, start, end);
    recovery_line_at(
        f,
        text,
        start,
        end,
        lead,
        content_end,
        UmlSyntaxDiagnosticCode::UnsupportedSequenceForm,
        "unsupported sequence form",
        diags,
    )
}

fn scan_endpoint(source: &str, mut p: usize, end: usize) -> usize {
    while p < end && !source.as_bytes()[p].is_ascii_whitespace() && source.as_bytes()[p] != b':' {
        p += 1;
    }
    p
}

fn sequence_endpoint_valid(endpoint: &str) -> bool {
    let at_count = endpoint.bytes().filter(|byte| *byte == b'@').count();
    !endpoint.is_empty() && (at_count == 0 || (at_count == 1 && !endpoint.ends_with('@')))
}

fn contains_unquoted_colon(source: &str, from: usize, to: usize) -> bool {
    let mut in_code = false;
    for byte in source.as_bytes()[from..to].iter().copied() {
        if byte == b'`' {
            in_code = !in_code;
        } else if byte == b':' && !in_code {
            return true;
        }
    }
    false
}

fn unsupported_sequence_body(body: &str) -> bool {
    let body = body.trim();
    body == "strict"
        || body.starts_with("strict ")
        || body == "seq"
        || body.starts_with("seq ")
        || body == "ignore"
        || body.starts_with("ignore ")
        || body == "consider"
        || body.starts_with("consider ")
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

fn canonical_message_body(body: &str) -> bool {
    body.split_ascii_whitespace()
        .nth(1)
        .is_some_and(|verb| vocabulary::MESSAGE_VERBS.contains(&verb))
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
        children.push(missing_token(
            f,
            text,
            leading.min(newline),
            newline,
            UmlSyntaxKind::NewlineToken,
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
    if lead >= content_end || !source[lead..content_end].starts_with('-') {
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
    if kind == UmlSyntaxKind::Slot {
        match source[body..content_end].find(':').map(|i| body + i) {
            None => {
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
            // A slot names an attribute and gives it a value, and the colon
            // promises both halves.  The attribute line already reports the
            // same two shapes; mark the colon so the squiggle sits on the
            // punctuation that made the promise rather than the whole bullet.
            Some(colon) => {
                if source[body..colon].trim().is_empty() {
                    diags.push(diag(
                        UmlSyntaxDiagnosticCode::UnexpectedToken,
                        colon,
                        colon + 1,
                        "expected a slot name before \":\"",
                    ));
                } else if source[colon + 1..content_end].trim().is_empty() {
                    diags.push(diag(
                        UmlSyntaxDiagnosticCode::MissingType,
                        colon,
                        colon + 1,
                        "expected a slot value after \":\"",
                    ));
                }
            }
        }
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

#[allow(clippy::too_many_arguments)]
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
    // Authored byte span of each entry in `atom_words`, excluding the leading
    // whitespace the green token carries as trivia.  Recovery diagnostics point
    // at these spans so a squiggle starts on the word, not in front of it.
    let mut atom_spans: Vec<(usize, usize)> = Vec::new();
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
            // As with a lexed atom, the gap in front of the unlexable bytes is
            // leading trivia so the recovery node's range starts on them.
            let leading = (token_start < at)
                .then(|| {
                    f.trivia(TriviaKind::Whitespace, slice(text, token_start, at))
                        .unwrap()
                })
                .into_iter();
            children.push(GreenElement::Node(
                f.node(
                    UmlSyntaxKind::SkippedTokensSyntax,
                    [GreenElement::Token(
                        f.bad_token_with_leading(
                            UmlSyntaxKind::BadToken,
                            slice(text, at, next),
                            leading,
                            UmlSyntaxDiagnosticCode::UnexpectedToken,
                        )
                        .unwrap(),
                    )],
                )
                .unwrap(),
            ));
        } else {
            // The gap before the atom is leading trivia, not part of the atom.
            // Folding it into the token text would make every range taken from
            // a layout node start one space early.
            children.push(token(f, text, token_start, at, next, kind));
            atom_words.push(source[at..next].trim().to_ascii_lowercase());
            atom_spans.push((at, next));
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
            Err(error) => {
                let (span_start, span_end) =
                    malformed_layout_span(&error, &atom_spans, lead + 1, content_end);
                diags.push(diag(
                    UmlSyntaxDiagnosticCode::MalformedLayout,
                    span_start,
                    span_end,
                    error.expected.message(),
                ));
                append_layout_recovery(f, &mut children, atoms, error)
            }
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

/// Why a layout bullet failed to match the fixed grammar.  The shape parser is
/// the only place that knows which word had to come next, so it carries the
/// authored words the message needs rather than leaving the analysis layer to
/// re-derive the grammar from the recovery nodes.
#[derive(Clone, Debug, Eq, PartialEq)]
enum LayoutExpectation {
    /// A fixed keyword had to follow an authored word (`left` -> `of`).
    Keyword {
        after: String,
        keyword: &'static str,
    },
    /// A member reference -- a link, a quoted name, or a parenthesised group.
    Reference,
    /// `as` must name an explicit axis.
    Axis,
    /// A parenthesised group was left open.
    CloseParen,
    /// `with`, `and` or `,` must be followed by a hint.
    Hint,
    /// A hint word outside the hint vocabulary.
    UnknownHint(String),
    /// The statement parsed but words remain after it.
    TrailingWords,
    /// An edge anchor (`top of A`) used outside an alignment.
    EdgeOutsideAlignment,
}

impl LayoutExpectation {
    /// The end-user message.  These render inline at the end of the authored
    /// row, so each one names the missing word and stops.
    fn message(&self) -> String {
        match self {
            Self::Keyword { after, keyword } => format!("expected \"{keyword}\" after \"{after}\""),
            Self::Reference => "expected a diagram member here".to_string(),
            Self::Axis => "expected \"row\" or \"column\" after \"as\"".to_string(),
            Self::CloseParen => "expected \")\" to close the group".to_string(),
            Self::Hint => "expected a layout hint after \"with\"".to_string(),
            Self::UnknownHint(word) => format!("\"{word}\" is not a layout hint"),
            Self::TrailingWords => "unexpected extra words after the layout statement".to_string(),
            Self::EdgeOutsideAlignment => {
                "an edge anchor like \"top of\" needs \"aligned with\"".to_string()
            }
        }
    }
}

#[derive(Clone, Debug)]
struct LayoutShapeError {
    recovery_from: usize,
    missing_at: usize,
    missing: UmlSyntaxKind,
    expected: LayoutExpectation,
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
        expected: LayoutExpectation,
    ) -> LayoutShapeError {
        LayoutShapeError {
            recovery_from,
            missing_at,
            missing,
            expected,
        }
    }

    /// The word already consumed just before the cursor, for messages that name
    /// what the missing keyword had to follow.
    fn previous_word(&self) -> String {
        self.words
            .get(self.pos.wrapping_sub(1))
            .cloned()
            .unwrap_or_default()
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
                    LayoutExpectation::Axis,
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
                LayoutExpectation::Reference,
            ));
        };
        if word == "(" {
            self.pos += 1;
            self.operand()?;
            if !self.eat(")") {
                return Err(self.error(
                    start,
                    self.pos,
                    UmlSyntaxKind::LayoutCloseParenToken,
                    LayoutExpectation::CloseParen,
                ));
            }
            return Ok(());
        }
        if matches!(word, ")" | ",") {
            return Err(self.error(
                start,
                start,
                UmlSyntaxKind::LayoutWordToken,
                LayoutExpectation::Reference,
            ));
        }
        if vocabulary::LAYOUT_AXIS_WORDS.contains(&word) {
            let axis = word.to_string();
            self.pos += 1;
            if !self.eat("of") {
                return Err(self.error(
                    start,
                    self.pos,
                    UmlSyntaxKind::LayoutKeywordToken,
                    LayoutExpectation::Keyword {
                        after: axis,
                        keyword: "of",
                    },
                ));
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
            return Err(self.error(
                recovery_from,
                self.pos,
                UmlSyntaxKind::LayoutWordToken,
                LayoutExpectation::Hint,
            ));
        };
        match word {
            word if vocabulary::LAYOUT_SHAPE_HINTS.contains(&word) => {
                self.pos += 1;
                Ok(())
            }
            word if vocabulary::LAYOUT_MARGIN_SIZES.contains(&word) => {
                let size = word.to_string();
                self.pos += 1;
                if self.eat("margin") || self.eat("margins") {
                    Ok(())
                } else {
                    Err(self.error(
                        recovery_from,
                        self.pos,
                        UmlSyntaxKind::LayoutKeywordToken,
                        LayoutExpectation::Keyword {
                            after: size,
                            keyword: "margin",
                        },
                    ))
                }
            }
            _ => Err(self.error(
                recovery_from,
                self.pos,
                UmlSyntaxKind::LayoutWordToken,
                LayoutExpectation::UnknownHint(word.to_string()),
            )),
        }
    }

    fn anchored(&mut self) -> Result<(std::ops::Range<usize>, bool), LayoutShapeError> {
        let start = self.pos;
        let has_edge = self
            .word()
            .is_some_and(|word| vocabulary::LAYOUT_EDGE_WORDS.contains(&word))
            && self
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
            Some(word) if vocabulary::LAYOUT_DIRECTION_VERTICALS.contains(&word) => {
                self.pos += 1;
                if self
                    .word()
                    .is_some_and(|next| vocabulary::LAYOUT_DIRECTION_LATERALS.contains(&next))
                {
                    self.pos += 1;
                    if !self.eat("of") {
                        return Err(self.error(
                            start,
                            self.pos,
                            UmlSyntaxKind::LayoutKeywordToken,
                            LayoutExpectation::Keyword {
                                after: self.words[start..self.pos].join(" "),
                                keyword: "of",
                            },
                        ));
                    }
                }
                Ok(Some(start..self.pos))
            }
            Some(word) if vocabulary::LAYOUT_DIRECTION_LATERALS.contains(&word) => {
                self.pos += 1;
                if !self.eat("of") {
                    return Err(self.error(
                        start,
                        self.pos,
                        UmlSyntaxKind::LayoutKeywordToken,
                        LayoutExpectation::Keyword {
                            after: self.previous_word(),
                            keyword: "of",
                        },
                    ));
                }
                Ok(Some(start..self.pos))
            }
            _ => Ok(None),
        }
    }
}

/// The role the `## Layout` grammar expects at the position just past a prefix
/// of authored atoms. `uml::complete` selects a candidate family with this, so
/// the grammar stays the single authority on what may follow what.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::uml) enum LayoutRole {
    /// A member reference -- a name, a link, a quoted name or a group.
    Reference,
    /// A direction clause (`above`, `left of`, ...).
    Direction,
    /// A hint word after `with`, `and` or `,`.
    Hint,
}

/// What may follow `words`, by running the shape parser over them and reading
/// what it ran out of input expecting. A placement or standalone that parses as
/// a whole statement may be continued with a direction; an alignment may not be
/// continued at all (`parse_layout_shape` rejects anything after its right-hand
/// operand with `TrailingWords`), and anything else -- a keyword the grammar
/// demands, a malformed prefix, an error before the end -- is `None`, which is
/// how a position with no single answer offers nothing rather than a guess.
pub(in crate::uml) fn expected_layout_role(words: &[String]) -> Option<LayoutRole> {
    match parse_layout_shape(words) {
        Ok(LayoutShape::Alignment { .. }) => None,
        Ok(LayoutShape::Placement { .. } | LayoutShape::Standalone(_)) => {
            Some(LayoutRole::Direction)
        }
        Err(error) if error.missing_at == words.len() => match error.expected {
            LayoutExpectation::Reference => Some(LayoutRole::Reference),
            LayoutExpectation::Hint => Some(LayoutRole::Hint),
            _ => None,
        },
        Err(_) => None,
    }
}

fn parse_layout_shape(words: &[String]) -> Result<LayoutShape, LayoutShapeError> {
    let mut cursor = LayoutShapeCursor { words, pos: 0 };
    let (first, first_has_edge) = cursor.anchored()?;
    if cursor.eat("aligned") {
        let join_start = cursor.pos - 1;
        if !cursor.eat("with") {
            return Err(cursor.error(
                join_start,
                cursor.pos,
                UmlSyntaxKind::LayoutKeywordToken,
                LayoutExpectation::Keyword {
                    after: "aligned".to_string(),
                    keyword: "with",
                },
            ));
        }
        let (right, _) = cursor.anchored()?;
        if cursor.pos != words.len() {
            return Err(cursor.error(
                cursor.pos,
                cursor.pos,
                UmlSyntaxKind::EndOfFileToken,
                LayoutExpectation::TrailingWords,
            ));
        }
        return Ok(LayoutShape::Alignment {
            left: first,
            join: join_start..join_start + 2,
            right,
        });
    }
    if first_has_edge {
        return Err(cursor.error(
            0,
            cursor.pos,
            UmlSyntaxKind::LayoutKeywordToken,
            LayoutExpectation::EdgeOutsideAlignment,
        ));
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
        return Err(cursor.error(
            cursor.pos,
            cursor.pos,
            UmlSyntaxKind::EndOfFileToken,
            LayoutExpectation::TrailingWords,
        ));
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

/// The authored byte span a layout recovery diagnostic underlines.
///
/// The squiggle covers the construct the author got wrong -- from where
/// recovery starts through the atom the missing word was expected at -- rather
/// than the whole bullet, so `A left f B` marks `left f` and leaves both
/// members alone.  Trailing-word errors have nothing missing at a point, so
/// they run to the last atom instead.
fn malformed_layout_span(
    error: &LayoutShapeError,
    spans: &[(usize, usize)],
    fallback_start: usize,
    fallback_end: usize,
) -> (usize, usize) {
    let Some(last) = spans.len().checked_sub(1) else {
        return (fallback_start, fallback_end);
    };
    let from = error.recovery_from.min(last);
    let to = match error.expected {
        LayoutExpectation::TrailingWords => last,
        _ => error.missing_at.min(last).max(from),
    };
    (spans[from].0, spans[to].1)
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

#[allow(clippy::too_many_arguments)]
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
    // Authored span of `as`, when it is there at all.  A keyword the author did
    // write is what a missing operand after it gets marked on.
    let mut as_span = None;
    if source[p..content_end].starts_with("as") {
        let as_end = p + 2;
        c.push(token(f, text, q, p, as_end, UmlSyntaxKind::AsToken));
        as_span = Some((p, as_end));
        keyword_leading = as_end;
        p = skip_ws(source, as_end, content_end);
    } else {
        c.push(GreenElement::Token(f.missing_token(UmlSyntaxKind::AsToken)));
        keyword_leading = q;
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
        // `as` promised a name.  When `as` is missing too the diagnostic above
        // already covers the line, so only the authored keyword is reported.
        if let Some((as_start, as_end)) = as_span {
            diags.push(diag(
                UmlSyntaxDiagnosticCode::UnexpectedToken,
                as_start,
                as_end,
                "expected an instance name after \"as\"",
            ));
        }
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
    let mut with_span = None;
    if source[p..content_end].starts_with("with") {
        c.push(token(
            f,
            text,
            before_with,
            p,
            p + 4,
            UmlSyntaxKind::WithToken,
        ));
        with_span = Some((p, p + 4));
        keyword_leading = p + 4;
        p = skip_ws(source, p + 4, content_end);
    }
    let mut slots = 0usize;
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
        keyword_leading = name_end;
        p = skip_ws(source, name_end, content_end);
        let mut set_to_span = None;
        if source[p..content_end].starts_with("set to") {
            slot.push(token(
                f,
                text,
                name_end,
                p,
                p + 6,
                UmlSyntaxKind::SetToToken,
            ));
            set_to_span = Some((p, p + 6));
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
                // As with `as` above: `set to` promised a value, and when the
                // keyword itself is missing that is already reported.
                if let Some((set_to_start, set_to_end)) = set_to_span {
                    diags.push(diag(
                        UmlSyntaxDiagnosticCode::UnexpectedToken,
                        set_to_start,
                        set_to_end,
                        "expected a slot value after \"set to\"",
                    ));
                }
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
        slots += 1;
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
    // `with` promises at least one slot.  Trailing bytes that failed to scan as
    // a slot name are reported as skipped tokens below, so only an empty tail
    // reaches here unreported.
    if let Some((with_start, with_end)) = with_span {
        if slots == 0 && p >= content_end {
            diags.push(diag(
                UmlSyntaxDiagnosticCode::UnexpectedToken,
                with_start,
                with_end,
                "expected a slot after \"with\"",
            ));
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

#[allow(clippy::too_many_arguments)]
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
            .map_or(true, char::is_whitespace)
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
                .map_or(true, char::is_whitespace)
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
        let leading = (suffix_leading < p)
            .then(|| {
                f.trivia(TriviaKind::Whitespace, slice(text, suffix_leading, p))
                    .unwrap()
            })
            .into_iter();
        c.push(GreenElement::Node(
            f.node(
                UmlSyntaxKind::SkippedTokensSyntax,
                [GreenElement::Token(
                    f.bad_token_with_leading(
                        UmlSyntaxKind::BadToken,
                        slice(text, p, content_end),
                        leading,
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

#[allow(clippy::too_many_arguments)]
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
    if p < content_end
        && crate::model::Visibility::from_marker(source[p..].chars().next().unwrap()).is_some()
    {
        vis = Some(p);
        p += 1;
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
    // A nameless attribute (`- : OrderId`) built a missing identifier and said
    // nothing.  Report it only when there is a colon to point at -- without one
    // the missing-':' diagnostic above already covers the line.
    if name_start == name_end {
        if let Some(colon) = colon {
            diags.push(diag(
                UmlSyntaxDiagnosticCode::UnexpectedToken,
                colon,
                colon + 1,
                "expected an attribute name before \":\"",
            ));
        }
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
fn opaque_line(structure: &MarkdownStructureMap, line_start: usize, line_end: usize) -> bool {
    let line_start = TextSize::try_from_usize(line_start).ok();
    let line_end = TextSize::try_from_usize(line_end).ok();
    let (Some(line_start), Some(line_end)) = (line_start, line_end) else {
        return true;
    };
    let index = structure
        .opaque_ranges
        .partition_point(|range| range.end() <= line_start);
    structure
        .opaque_ranges
        .get(index)
        .is_some_and(|range| range.start() < line_end)
}
fn confirmed_list_item_line(structure: &MarkdownStructureMap, line_start: usize) -> bool {
    structure
        .list_item_lines
        .binary_search_by_key(&line_start, |range| range.start().to_usize())
        .is_ok()
}
fn tab_indented_item_line(structure: &MarkdownStructureMap, line_start: usize) -> bool {
    structure
        .tab_indented_item_lines
        .binary_search_by_key(&line_start, |range| range.start().to_usize())
        .is_ok()
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
    message: impl Into<std::sync::Arc<str>>,
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
