use std::collections::{HashMap, HashSet};

use crate::diagnostic::{DiagCode, Diagnostic};
use crate::model::{ElementType, RelationshipKind, UmlMetaclass};
use crate::slug::slugify;
use crate::syntax::{
    Direction, Document, LayoutStatement, Line, MemberGroup, NameRef, Operand, OperandRef, Section,
};

/// Last `/`-separated segment of a full-path node key (`"tables/order"` ->
/// `"order"`), used to match bare informal name references against
/// full-path `keyset` entries. Mirrors `solve::resolve::basename`.
fn basename(key: &str) -> &str {
    key.rsplit('/').next().unwrap_or(key)
}

/// Collect every group's heading name (recursively) into `names`.
fn collect_group_names(g: &MemberGroup, names: &mut HashSet<String>) {
    if !g.name.is_empty() {
        names.insert(g.name.clone());
    }
    for c in &g.children {
        collect_group_names(c, names);
    }
}

/// Walk an operand, reporting each `Name` ref that resolves to neither a
/// member key nor a declared group name.
fn check_operand_refs(
    op: &Operand,
    keyset: &HashSet<String>,
    group_names: &HashSet<String>,
    path: &str,
    line: usize,
    diags: &mut Vec<Diagnostic>,
) {
    match &op.ref_ {
        OperandRef::Name(name) => {
            let (label, resolved) = match name {
                NameRef::Link { slug, .. } => {
                    let resolved_id = crate::okf::resolve_href(path, slug);
                    (slug.clone(), keyset.contains(&resolved_id))
                }
                NameRef::Bare(s) => {
                    // Mirror `solve::resolve::resolve_ref`'s `NameRef::Bare` arm:
                    // group heading names match by raw name first; failing that,
                    // `keyset` is full-path node keys, so slugify `s` and try an
                    // exact (root-level) match, then a unique-basename match
                    // across all keys. An ambiguous basename is left unresolved.
                    let resolved = if group_names.contains(s) {
                        true
                    } else {
                        let slug = slugify(s, "");
                        if keyset.contains(&slug) {
                            true
                        } else {
                            let mut matches = keyset.iter().filter(|k| basename(k) == slug);
                            matches.next().is_some() && matches.next().is_none()
                        }
                    };
                    (s.clone(), resolved)
                }
            };
            if !resolved {
                diags.push(Diagnostic::warn(
                    DiagCode::UnresolvedLayoutRef,
                    format!("layout operand '{label}' resolves no member group"),
                    path,
                    line,
                ));
            }
        }
        OperandRef::InlineGroup { items, .. } => {
            for it in items {
                check_operand_refs(it, keyset, group_names, path, line, diags);
            }
        }
        OperandRef::Paren(inner) => {
            check_operand_refs(inner, keyset, group_names, path, line, diags)
        }
    }
}

/// A stable key for a named operand (its slug or bare name); `None` for an
/// anonymous inline group.
fn operand_key(op: &Operand) -> Option<String> {
    match &op.ref_ {
        OperandRef::Name(NameRef::Link { slug, .. }) => Some(slug.clone()),
        OperandRef::Name(NameRef::Bare(s)) => Some(s.clone()),
        OperandRef::Paren(inner) => operand_key(inner),
        OperandRef::InlineGroup { .. } => None,
    }
}

/// Depth-first cycle check over a directed adjacency map.
fn has_cycle(graph: &HashMap<String, Vec<String>>) -> bool {
    // 0 = unvisited, 1 = on stack, 2 = done
    fn dfs(
        node: &str,
        graph: &HashMap<String, Vec<String>>,
        state: &mut HashMap<String, u8>,
    ) -> bool {
        state.insert(node.to_string(), 1);
        if let Some(succs) = graph.get(node) {
            for s in succs {
                match state.get(s).copied().unwrap_or(0) {
                    1 => return true,
                    0 if dfs(s, graph, state) => {
                        return true;
                    }
                    _ => {}
                }
            }
        }
        state.insert(node.to_string(), 2);
        false
    }
    let mut state: HashMap<String, u8> = HashMap::new();
    for node in graph.keys() {
        if state.get(node).copied().unwrap_or(0) == 0 && dfs(node, graph, &mut state) {
            return true;
        }
    }
    false
}

/// Report each unresolved member bullet (recursively through sub-groups).
fn check_group_members(
    g: &MemberGroup,
    keyset: &HashSet<String>,
    path: &str,
    diags: &mut Vec<Diagnostic>,
) {
    use crate::syntax::MemberItem;
    for item in g.members.iter().filter_map(Line::parsed) {
        // Inline instances are not plain members — their classifier resolution
        // is validated separately (Task 6).
        let MemberItem::Member(m) = item else {
            continue;
        };
        let resolved = crate::okf::resolve_href(path, &m.slug);
        if !keyset.contains(&resolved) {
            let mut d = Diagnostic::warn(
                DiagCode::UnresolvedTarget,
                format!("diagram member './{}.md' resolves to no document", m.slug),
                path,
                m.line,
            );
            if let Some(span) = m.span {
                d = d.with_span(span);
            }
            diags.push(d);
        }
    }
    for c in &g.children {
        check_group_members(c, keyset, path, diags);
    }
}

/// Warn (never error) on an `instance of` target: unresolved → `InstanceOfUnresolved`;
/// resolved but not a classifier (incl. another instance) → `InstanceOfNonClassifier`.
#[allow(clippy::too_many_arguments)]
fn check_instance_of_target(
    target_slug: &str,
    resolved: &str,
    in_keyset: bool,
    target_ty: Option<&ElementType>,
    path: &str,
    line: usize,
    span: Option<(usize, usize)>,
    diags: &mut Vec<Diagnostic>,
) {
    let push = |code: DiagCode, msg: String, diags: &mut Vec<Diagnostic>| {
        let mut d = Diagnostic::warn(code, msg, path, line);
        if let Some(sp) = span {
            d = d.with_span(sp);
        }
        diags.push(d);
    };
    if !in_keyset {
        push(
            DiagCode::InstanceOfUnresolved,
            format!("'instance of' target './{target_slug}.md' resolves to no document"),
            diags,
        );
    } else if !target_ty.map(ElementType::is_classifier).unwrap_or(false) {
        push(
            DiagCode::InstanceOfNonClassifier,
            format!(
                "'instance of' target '{resolved}' is not a classifier — you do not instantiate an instance"
            ),
            diags,
        );
    }
}

/// The semantic (cross-document) pass over already-parsed documents: reports
/// `DuplicateSlug`, `UnresolvedTarget` (relationships + diagram members),
/// `UnresolvedLayoutRef`, and `LayoutCycle`, reusing the positions recorded in
/// each node during `parse`. Syntactic diagnostics are produced by `parse`.
pub fn link(docs: &[(String, ElementType, Document)]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut keyset: HashSet<String> = HashSet::new();
    let mut slug_count: HashMap<String, usize> = HashMap::new();
    for (path, ty, _doc) in docs {
        let slug = crate::okf::id_of(path);
        *slug_count.entry(slug.clone()).or_insert(0) += 1;
        if !ty.is_view() {
            keyset.insert(slug);
        }
    }
    let mut types: HashMap<String, ElementType> = HashMap::new();
    for (path, ty, _doc) in docs {
        types.insert(crate::okf::id_of(path), ty.clone());
    }
    let docs_by_key: HashMap<String, &Document> = docs
        .iter()
        .map(|(p, _, d)| (crate::okf::id_of(p), d))
        .collect();
    // Attribute names declared by a (classifier) document, for slot conformance.
    let attr_names_of = |key: &str| -> Option<HashSet<String>> {
        let doc = docs_by_key.get(key)?;
        let mut names = HashSet::new();
        for s in &doc.sections {
            if let Section::Attributes(a) = s {
                for at in a.iter().filter_map(Line::parsed) {
                    names.insert(at.name.clone());
                }
            }
        }
        Some(names)
    };

    for (path, ty, doc) in docs {
        let slug = crate::okf::id_of(path);
        if slug_count[&slug] > 1 {
            diags.push(Diagnostic::new(
                DiagCode::DuplicateSlug,
                format!("duplicate document slug '{slug}'"),
                path,
                1,
            ));
        }

        // Group heading names declared in this document's `## Members` section,
        // used to resolve bare layout operands.
        let mut group_names = HashSet::new();
        for s in &doc.sections {
            if let Section::Members(block) = s {
                for g in &block.groups {
                    collect_group_names(g, &mut group_names);
                }
            }
        }

        for s in &doc.sections {
            match s {
                Section::Relationships(rels) => {
                    for r in rels.iter().filter_map(Line::parsed) {
                        // Instance-authored edges get warn-only conformance
                        // checks below — never the hard UnresolvedTarget Error.
                        if matches!(
                            r.kind,
                            RelationshipKind::InstanceOf | RelationshipKind::Links
                        ) {
                            continue;
                        }
                        let resolved = crate::okf::resolve_href(path, &r.target_slug);
                        if !keyset.contains(&resolved) {
                            let mut d = Diagnostic::new(
                                DiagCode::UnresolvedTarget,
                                format!(
                                    "relationship target './{}.md' resolves to no document",
                                    r.target_slug
                                ),
                                path,
                                r.line,
                            );
                            if let Some(span) = r.span {
                                d = d.with_span(span);
                            }
                            diags.push(d);
                        }
                        // Context rule: an ends-less `associates` is a
                        // communication link — valid only when an actor or a
                        // use case participates. Between plain classifiers,
                        // ends are required (uaml-spec.md).
                        let is_comm_party = |t: Option<&ElementType>| {
                            matches!(
                                t,
                                Some(ElementType::Uml(UmlMetaclass::Actor))
                                    | Some(ElementType::Uml(UmlMetaclass::UseCase))
                            )
                        };
                        if r.kind == RelationshipKind::Associates
                            && r.from_end.multiplicity.is_none()
                            && keyset.contains(&resolved)
                            && !is_comm_party(Some(ty))
                            && !is_comm_party(types.get(&resolved))
                        {
                            let mut d = Diagnostic::new(
                                DiagCode::MalformedRelationship,
                                "'associates' between classifiers requires ': <near> to <far>' multiplicity ends (ends are optional only on an actor↔use-case communication link)",
                                path,
                                r.line,
                            );
                            if let Some(span) = r.span {
                                d = d.with_span(span);
                            }
                            diags.push(d);
                        }
                    }
                }
                Section::Members(block) => {
                    for g in &block.groups {
                        check_group_members(g, &keyset, path, &mut diags);
                    }
                }
                Section::Layout(items) => {
                    for it in items.iter().filter_map(Line::parsed) {
                        let ops: Vec<&Operand> = match &it.stmt {
                            LayoutStatement::Standalone(op) => vec![op],
                            LayoutStatement::Placement { operands, .. } => {
                                operands.iter().collect()
                            }
                            LayoutStatement::Alignment { left, right } => {
                                vec![&left.operand, &right.operand]
                            }
                        };
                        for op in ops {
                            check_operand_refs(
                                op,
                                &keyset,
                                &group_names,
                                path,
                                it.line,
                                &mut diags,
                            );
                        }
                    }

                    let mut horizontal: HashMap<String, Vec<String>> = HashMap::new();
                    let mut vertical: HashMap<String, Vec<String>> = HashMap::new();
                    let mut first_placement_line: Option<usize> = None;
                    for it in items.iter().filter_map(Line::parsed) {
                        let LayoutStatement::Placement {
                            operands,
                            directions,
                        } = &it.stmt
                        else {
                            continue;
                        };
                        first_placement_line.get_or_insert(it.line);
                        for (i, dir) in directions.iter().enumerate() {
                            let (a, b) = (operand_key(&operands[i]), operand_key(&operands[i + 1]));
                            let (Some(a), Some(b)) = (a, b) else { continue };
                            // Edge points from the operand that must come first to the one after it.
                            let (graph, from, to) = match dir {
                                Direction::LeftOf => (&mut horizontal, a, b),
                                Direction::RightOf => (&mut horizontal, b, a),
                                Direction::Above => (&mut vertical, a, b),
                                Direction::Below => (&mut vertical, b, a),
                            };
                            graph.entry(from).or_default().push(to);
                        }
                    }
                    if has_cycle(&horizontal) || has_cycle(&vertical) {
                        let line = first_placement_line
                            .or_else(|| {
                                items
                                    .iter()
                                    .filter_map(Line::parsed)
                                    .map(|it| it.line)
                                    .next()
                            })
                            .unwrap_or(1);
                        diags.push(Diagnostic::new(
                            DiagCode::LayoutCycle,
                            "layout placement constraints form a cycle (contradictory ordering)",
                            path,
                            line,
                        ));
                    }
                }
                Section::Nodes(block) => {
                    use crate::syntax::{FlowBullet, FlowTargetRef};
                    let mut counts: HashMap<&str, usize> = HashMap::new();
                    for n in &block.nodes {
                        *counts.entry(n.identity.as_str()).or_insert(0) += 1;
                    }
                    for n in &block.nodes {
                        if counts[n.identity.as_str()] > 1 {
                            diags.push(Diagnostic::new(
                                DiagCode::DuplicateFlowNode,
                                format!("duplicate node identity '{}' — transition targets resolve by identity", n.identity),
                                path,
                                n.line,
                            ));
                        }
                        for b in n.bullets.iter().filter_map(Line::parsed) {
                            let FlowBullet::Transition(t) = b else {
                                continue;
                            };
                            if let FlowTargetRef::Local(name) = &t.target {
                                if !counts.contains_key(name.as_str()) {
                                    diags.push(Diagnostic::warn(
                                        DiagCode::UnresolvedTarget,
                                        format!("transition target '{name}' matches no '###' node in this document"),
                                        path,
                                        t.line,
                                    ));
                                }
                            }
                        }
                    }
                }
                Section::Messages(block) => {
                    use crate::syntax::SeqItemSyntax;
                    // Participant tokens must match a declared lifeline
                    // (alias or title). Collect the declared names first.
                    let mut names: HashSet<String> = HashSet::new();
                    for sec in &doc.sections {
                        if let Section::Lifelines(lines) = sec {
                            for l in lines.iter().filter_map(Line::parsed) {
                                names.insert(l.link.title.clone());
                                if let Some(a) = &l.alias {
                                    names.insert(a.clone());
                                }
                            }
                        }
                    }
                    fn check_items(
                        items: &[Line<SeqItemSyntax>],
                        names: &HashSet<String>,
                        path: &str,
                        diags: &mut Vec<Diagnostic>,
                    ) {
                        for it in items.iter().filter_map(Line::parsed) {
                            match it {
                                SeqItemSyntax::Message(m) => {
                                    for token in [&m.from, &m.to] {
                                        let name = match crate::grammar::parse_link_ref(token) {
                                            Some(l) => l.title,
                                            None => token.clone(),
                                        };
                                        if !names.contains(&name) {
                                            diags.push(Diagnostic::warn(
                                                DiagCode::UnresolvedTarget,
                                                format!("message participant '{name}' matches no lifeline"),
                                                path,
                                                m.line,
                                            ));
                                        }
                                    }
                                }
                                SeqItemSyntax::Fragment { operands, .. } => {
                                    for op in operands {
                                        check_items(&op.items, names, path, diags);
                                    }
                                }
                            }
                        }
                    }
                    check_items(&block.items, &names, path, &mut diags);
                }
                _ => {}
            }
        }

        // ── Instance conformance (design spec §5), all warn-only ──────────
        if *ty == ElementType::Uml(UmlMetaclass::InstanceSpecification) {
            // Standalone instance doc: `instance of` targets + `## Slots`.
            let mut classifier: Option<String> = None;
            for s in &doc.sections {
                let Section::Relationships(rels) = s else {
                    continue;
                };
                for r in rels.iter().filter_map(Line::parsed) {
                    if r.kind != RelationshipKind::InstanceOf {
                        continue;
                    }
                    let resolved = crate::okf::resolve_href(path, &r.target_slug);
                    let in_keyset = keyset.contains(&resolved);
                    check_instance_of_target(
                        &r.target_slug,
                        &resolved,
                        in_keyset,
                        types.get(&resolved),
                        path,
                        r.line,
                        r.span,
                        &mut diags,
                    );
                    if in_keyset
                        && types
                            .get(&resolved)
                            .map(ElementType::is_classifier)
                            .unwrap_or(false)
                    {
                        classifier.get_or_insert(resolved);
                    }
                }
            }
            if let Some(ck) = &classifier {
                if let Some(names) = attr_names_of(ck) {
                    for s in &doc.sections {
                        let Section::Slots(slots) = s else {
                            continue;
                        };
                        for sl in slots.iter().filter_map(Line::parsed) {
                            if !names.contains(&sl.name) {
                                diags.push(Diagnostic::warn(
                                    DiagCode::SlotUnknownAttribute,
                                    format!(
                                        "slot '{}' is not an attribute of classifier '{ck}'",
                                        sl.name
                                    ),
                                    path,
                                    sl.line,
                                ));
                            }
                        }
                    }
                }
            }
        }
        if *ty == ElementType::Diagram {
            // Inline instances promoted from a diagram's `## Members`.
            use crate::syntax::MemberItem;
            #[allow(clippy::too_many_arguments)]
            fn walk_inline(
                g: &MemberGroup,
                path: &str,
                keyset: &HashSet<String>,
                types: &HashMap<String, ElementType>,
                attr_names_of: &dyn Fn(&str) -> Option<HashSet<String>>,
                diags: &mut Vec<Diagnostic>,
            ) {
                for item in g.members.iter().filter_map(Line::parsed) {
                    let MemberItem::Instance(inst) = item else {
                        continue;
                    };
                    let resolved = crate::okf::resolve_href(path, &inst.classifier.slug);
                    let in_keyset = keyset.contains(&resolved);
                    check_instance_of_target(
                        &inst.classifier.slug,
                        &resolved,
                        in_keyset,
                        types.get(&resolved),
                        path,
                        inst.line,
                        inst.span,
                        diags,
                    );
                    if in_keyset
                        && types
                            .get(&resolved)
                            .map(ElementType::is_classifier)
                            .unwrap_or(false)
                    {
                        if let Some(names) = attr_names_of(&resolved) {
                            for sl in &inst.slots {
                                if !names.contains(&sl.name) {
                                    diags.push(Diagnostic::warn(
                                        DiagCode::SlotUnknownAttribute,
                                        format!(
                                            "slot '{}' is not an attribute of classifier '{resolved}'",
                                            sl.name
                                        ),
                                        path,
                                        inst.line,
                                    ));
                                }
                            }
                        }
                    }
                }
                for c in &g.children {
                    walk_inline(c, path, keyset, types, attr_names_of, diags);
                }
            }
            for s in &doc.sections {
                if let Section::Members(block) = s {
                    for g in &block.groups {
                        walk_inline(g, path, &keyset, &types, &attr_names_of, &mut diags);
                    }
                }
            }
        }
    }
    diags
}

pub fn validate(bundle: &[(String, String)]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut docs = Vec::new();
    for (path, text) in bundle {
        let (doc, mut syn) = crate::parse::parse(text);
        for d in &mut syn {
            d.file = path.clone();
        }
        diags.append(&mut syn);
        let ty = ElementType::parse(doc.frontmatter.get_str("type").unwrap_or("uml.Class"));
        docs.push((path.clone(), ty, doc));
    }
    diags.extend(link(&docs));
    diags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Severity;

    #[test]
    fn unresolved_relationship_target_carries_a_span() {
        let b = vec![("a/order.md".into(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Ghost](./ghost.md)\n".into())];
        let d = validate(&b);
        let t = d
            .iter()
            .find(|x| x.code == DiagCode::UnresolvedTarget)
            .unwrap();
        assert_eq!(t.line, 8);
        let (s, e) = t.span.expect("unresolved target must span the link");
        assert!(s < e);
    }

    #[test]
    fn flags_unresolved_relationship_target() {
        let b = vec![("a/order.md".into(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Ghost](./ghost.md)\n".into())];
        let d = validate(&b);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, DiagCode::UnresolvedTarget);
        assert_eq!(d[0].line, 8);
    }

    #[test]
    fn relationship_target_resolves_against_referring_dir_not_a_same_basename_doc_elsewhere() {
        // `tables/index.md`'s `./order.md` must resolve to `tables/order`, not
        // to `shop/order.md` (a different doc that happens to share the
        // basename). If it mis-resolved to the wrong doc, the correct target
        // (`tables/order.md`) would be flagged as an unused/orphan doc but the
        // relationship itself would spuriously fail to resolve here — assert
        // it resolves cleanly with the correctly-directoried target present.
        let b = vec![
            ("tables/index.md".into(),
             "---\ntype: uml.Class\ntitle: Index\n---\n# Index\n\n## Relationships\n- depends [Order](./order.md)\n".into()),
            ("tables/order.md".into(),
             "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
            ("shop/order.md".into(),
             "---\ntype: uml.Class\ntitle: ShopOrder\n---\n# ShopOrder\n".into()),
        ];
        let d = validate(&b);
        assert!(
            d.iter().all(|x| x.code != DiagCode::UnresolvedTarget),
            "expected `./order.md` in tables/index.md to resolve to tables/order, got: {d:?}"
        );
    }

    #[test]
    fn relationship_target_does_not_fall_back_to_wrong_directory_same_basename_doc() {
        // Same shape as above but WITHOUT `tables/order.md` present — the
        // relationship must NOT resolve against `shop/order.md` (proving the
        // resolver is directory-relative, not a bare-basename fallback).
        let b = vec![
            ("tables/index.md".into(),
             "---\ntype: uml.Class\ntitle: Index\n---\n# Index\n\n## Relationships\n- depends [Order](./order.md)\n".into()),
            ("shop/order.md".into(),
             "---\ntype: uml.Class\ntitle: ShopOrder\n---\n# ShopOrder\n".into()),
        ];
        let d = validate(&b);
        assert!(d.iter().any(|x| x.code == DiagCode::UnresolvedTarget));
    }

    #[test]
    fn diagram_member_link_resolves_against_referring_diagrams_dir() {
        let b = vec![
            (
                "tables/dia.md".into(),
                "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Members\n- [Order](./order.md)\n"
                    .into(),
            ),
            (
                "tables/order.md".into(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into(),
            ),
            (
                "shop/order.md".into(),
                "---\ntype: uml.Class\ntitle: ShopOrder\n---\n# ShopOrder\n".into(),
            ),
        ];
        let d = validate(&b);
        assert!(
            d.iter().all(|x| x.code != DiagCode::UnresolvedTarget),
            "expected diagram member `./order.md` to resolve to tables/order, got: {d:?}"
        );
    }

    #[test]
    fn layout_link_ref_resolves_against_referring_diagrams_dir() {
        // A `## Layout` operand written as a link (not a bare name) must
        // resolve the same directory-relative way as Members/Relationships —
        // a diagram living in a subdirectory must not spuriously warn
        // UnresolvedLayoutRef against its own (full-path) member.
        let b = vec![("tables/dia.md".into(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Members\n- [Order](./order.md)\n- [Customer](./customer.md)\n\n## Layout\n- [Order](./order.md) left of [Customer](./customer.md)\n".into()),
            ("tables/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
            ("tables/customer.md".into(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".into()),
        ];
        let d = validate(&b);
        assert!(
            d.iter().all(|x| x.code != DiagCode::UnresolvedLayoutRef),
            "expected layout link refs to resolve against the diagram's own directory, got: {d:?}"
        );
    }

    #[test]
    fn flags_missing_ends_on_composition() {
        let b = vec![
            ("a/order.md".into(),
             "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- composes [OrderLine](./order-line.md)\n".into()),
            ("a/order-line.md".into(),
             "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".into()),
        ];
        let d = validate(&b);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, DiagCode::MalformedRelationship);
        assert!(d[0].message.contains("requires"));
    }

    #[test]
    fn flags_malformed_attribute() {
        let b = vec![(
            "a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Attributes\n- bad line without colon\n"
                .into(),
        )];
        let d = validate(&b);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, DiagCode::MalformedAttribute);
    }

    #[test]
    fn distinct_basenames_in_different_dirs_do_not_collide() {
        // Full-path keying: `a/order.md` and `b/order.md` share a basename but
        // key on their distinct full paths (`a/order`, `b/order`) — no
        // DuplicateSlug false positive.
        let b = vec![
            (
                "a/order.md".into(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into(),
            ),
            (
                "b/order.md".into(),
                "---\ntype: uml.Class\ntitle: Order2\n---\n# Order2\n".into(),
            ),
        ];
        let d = validate(&b);
        assert_eq!(
            d.iter()
                .filter(|x| x.code == DiagCode::DuplicateSlug)
                .count(),
            0
        );
    }

    #[test]
    fn flags_duplicate_slug() {
        // Two docs projecting to the *same* full id (identical bundle-relative
        // path) still collide.
        let b = vec![
            (
                "a/order.md".into(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into(),
            ),
            (
                "a/order.md".into(),
                "---\ntype: uml.Class\ntitle: Order2\n---\n# Order2\n".into(),
            ),
        ];
        let d = validate(&b);
        assert_eq!(
            d.iter()
                .filter(|x| x.code == DiagCode::DuplicateSlug)
                .count(),
            2
        );
    }

    #[test]
    fn unresolved_member_is_only_a_warning() {
        let b = vec![(
            "d/dia.md".into(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Members\n- [Ghost](./ghost.md)\n".into(),
        )];
        let d = validate(&b);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, DiagCode::UnresolvedTarget);
        assert_eq!(d[0].severity, Severity::Warning);
    }

    #[test]
    fn flags_frontmatter_that_is_not_a_metadata_block() {
        // A missing closing fence breaks metadata-block recognition (pulldown-cmark
        // 0.12.2 tolerates a leading blank line, but not an unterminated block).
        let b = vec![(
            "a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n# X\n".into(),
        )];
        let d = validate(&b);
        assert!(d.iter().any(|x| x.code == DiagCode::FrontmatterNotClean));
    }

    #[test]
    fn warns_on_unknown_type() {
        let b = vec![(
            "a/x.md".into(),
            "---\ntype: bpmn.Task\ntitle: X\n---\n# X\n".into(),
        )];
        let d = validate(&b);
        let w = d.iter().find(|x| x.code == DiagCode::UnknownType).unwrap();
        assert_eq!(w.severity, crate::diagnostic::Severity::Warning);
        assert_eq!(w.line, 2);
    }

    #[test]
    fn clean_document_has_no_diagnostics() {
        let b = vec![(
            "a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Attributes\n- id: XId\n".into(),
        )];
        assert!(validate(&b).is_empty());
    }

    #[test]
    fn flags_malformed_line_after_yaml_dots_close() {
        // Frontmatter opens with `---` but closes with `...` (both are valid
        // YAML-style metadata block closers). The body after it must still be
        // scanned — it must not be silently skipped.
        let b = vec![(
            "a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n...\n# X\n\n## Attributes\n- bad line without colon\n"
                .into(),
        )];
        let d = validate(&b);
        assert!(d.iter().any(|x| x.code == DiagCode::MalformedAttribute));
    }

    #[test]
    fn stray_comment_does_not_hide_unresolved_target() {
        // Reproduces the reported bug: an unresolved relationship target
        // appears BEFORE a later, unrelated HTML comment (e.g. a review
        // note). Pre-fix, `split_bundle` treats that stray comment as a
        // bundle marker and — because it only keeps the tail of the blob
        // starting at the marker — silently discards everything before it,
        // including the frontmatter and the Relationships section. That
        // makes `check` report no problems for a document that actually
        // has an Error-severity diagnostic.
        let text = "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Ghost](./ghost.md)\n\n<!-- reviewed: needs follow-up -->\n\nTrailing note.\n";
        let bundle = crate::parse::split_bundle(text);
        let d = validate(&bundle);
        assert!(
            d.iter().any(|x| x.code == DiagCode::UnresolvedTarget),
            "unresolved-target must still be reported (content before a stray comment must not be discarded), got: {d:?}"
        );
    }

    #[test]
    fn flags_prose_before_first_section() {
        // A non-blank prose line after the frontmatter and the H1 title but
        // before the first `## ` section is silently dropped by parse/serialize
        // today. It must be flagged as an Error so `fmt` skips the file.
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n\nSome stray prose here.\n\n## Attributes\n- id: XId\n".into())];
        let d = validate(&b);
        assert_eq!(d.len(), 1, "expected exactly one diagnostic, got: {d:?}");
        assert_eq!(d[0].code, DiagCode::DroppableContent);
        assert_eq!(d[0].severity, Severity::Error);
    }

    #[test]
    fn flags_non_bullet_line_in_values() {
        // A stray non-bullet line inside `## Values` was silently dropped by the
        // `filter_map(...ok())` path (no error node, no diagnostic) — so `fmt`
        // did not skip and `serialize` deleted it on round-trip. It must now be
        // preserved and flagged as droppable content.
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Enum\ntitle: X\n---\n# X\n\n## Values\n- DRAFT\nA stray comment line.\n".into())];
        let d = validate(&b);
        assert!(
            d.iter()
                .any(|x| x.code == DiagCode::DroppableContent && x.severity == Severity::Error),
            "expected a DroppableContent Error, got: {d:?}"
        );
    }

    #[test]
    fn flags_stray_line_in_members() {
        // A stray non-heading, non-member line inside `## Members` was silently
        // dropped by `parse_members_block` — data loss on `fmt` round-trip. It
        // must now be preserved and flagged as droppable content.
        let b = vec![("d/dia.md".into(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Members\n- [Customer](./customer.md)\nA stray note.\n".into())];
        let d = validate(&b);
        assert!(
            d.iter()
                .any(|x| x.code == DiagCode::DroppableContent && x.severity == Severity::Error),
            "expected a DroppableContent Error, got: {d:?}"
        );
    }

    #[test]
    fn unresolved_member_carries_line_and_span() {
        // The member bullet sits on line 8; its unresolved-target diagnostic must
        // point at that line (not 0) and carry a link span — the member walk fills
        // `MemberLine.line`/`.span`.
        let b = vec![(
            "d/dia.md".into(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Members\n- [Ghost](./ghost.md)\n".into(),
        )];
        let d = validate(&b);
        let t = d
            .iter()
            .find(|x| x.code == DiagCode::UnresolvedTarget)
            .unwrap();
        assert_eq!(t.line, 8, "member diagnostic must point at the bullet line");
        let (s, e) = t
            .span
            .expect("member unresolved-target must carry a link span");
        assert!(s < e);
    }

    #[test]
    fn flags_non_bullet_line_in_attributes() {
        // A stray non-bullet line inside a bullet section (e.g. `## Attributes`)
        // is neither preserved nor flagged today — `filter_map` just drops it.
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Attributes\n- id: XId\nA stray comment line.\n".into())];
        let d = validate(&b);
        assert!(
            d.iter().any(|x| x.code == DiagCode::DroppableContent),
            "expected a DroppableContent diagnostic, got: {d:?}"
        );
    }

    #[test]
    fn allows_prose_in_body_and_unknown_sections() {
        // Free prose in `## Body` and in an unrecognized `## Provenance` section
        // is preserved verbatim by serialize — it must never be flagged.
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Body\nSome free prose.\nMore prose.\n\n## Provenance\nHand-authored. Keep me.\n".into())];
        let d = validate(&b);
        assert!(
            !d.iter().any(|x| x.code == DiagCode::DroppableContent),
            "prose in Body/unknown sections must never be flagged, got: {d:?}"
        );
    }

    #[test]
    fn rel_error_message_ignores_colon_inside_link_title() {
        // A malformed `composes` line with no multiplicity ends, but whose
        // bracketed target title happens to contain a colon, must still get
        // the "requires ends" message — the colon inside `[Title]` must not
        // be misread as the ends separator by a whole-line colon scan.
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Relationships\n- composes [OrderLine: v2](./order-line.md)\n".into())];
        let d = validate(&b);
        let rel = d
            .iter()
            .find(|x| x.code == DiagCode::MalformedRelationship)
            .unwrap();
        assert!(rel.message.contains("requires"), "got: {}", rel.message);
    }

    #[test]
    fn malformed_layout_line_is_an_error() {
        let bundle = vec![(
            "d.md".to_string(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Layout\n- Users nonsense Orders\n"
                .to_string(),
        )];
        let diags = validate(&bundle);
        assert!(diags.iter().any(|d| d.code == DiagCode::MalformedLayout));
    }

    #[test]
    fn member_subheading_is_not_droppable() {
        let bundle = vec![(
            "d.md".to_string(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n".to_string(),
        )];
        let diags = validate(&bundle);
        assert!(
            !diags.iter().any(|d| d.code == DiagCode::DroppableContent),
            "### group heading must not be flagged droppable"
        );
    }

    #[test]
    fn unknown_layout_ref_is_a_warning() {
        let bundle = vec![
            ("customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
            ("d.md".to_string(),
             "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n\n## Layout\n- Users left of Ghosts\n".to_string()),
        ];
        let diags = validate(&bundle);
        // "Users" is a declared group, "Ghosts" resolves to nothing -> one warning.
        let refs: Vec<_> = diags
            .iter()
            .filter(|d| d.code == DiagCode::UnresolvedLayoutRef)
            .collect();
        assert_eq!(refs.len(), 1);
        assert!(refs[0].message.contains("Ghosts"));
    }

    #[test]
    fn bare_layout_ref_resolves_by_unique_basename_across_full_path_keys() {
        // `keyset` is full-path node keys (`tables/order`, not `order`) since
        // the full-path-keying migration. A bare informal layout ref
        // ("Order") carries no directory of its own, so it must still
        // resolve by unique basename match, same as `solve::resolve`.
        let bundle = vec![
            (
                "tables/dia.md".to_string(),
                "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Layout\n- Order\n".to_string(),
            ),
            (
                "tables/order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            ),
        ];
        let diags = validate(&bundle);
        assert!(
            diags
                .iter()
                .all(|d| d.code != DiagCode::UnresolvedLayoutRef),
            "expected bare layout ref to resolve by unique basename, got: {diags:?}"
        );
    }

    #[test]
    fn bare_layout_ref_with_ambiguous_basename_stays_unresolved() {
        // Two full-path keys share the basename `order` (`tables/order`,
        // `shop/order`) — the bare ref must NOT silently pick one.
        let bundle = vec![
            (
                "tables/dia.md".to_string(),
                "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Layout\n- Order\n".to_string(),
            ),
            (
                "tables/order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            ),
            (
                "shop/order.md".to_string(),
                "---\ntype: uml.Class\ntitle: ShopOrder\n---\n# ShopOrder\n".to_string(),
            ),
        ];
        let diags = validate(&bundle);
        assert!(diags
            .iter()
            .any(|d| d.code == DiagCode::UnresolvedLayoutRef));
    }

    #[test]
    fn contradictory_placement_is_a_cycle_error() {
        let bundle = vec![(
            "d.md".to_string(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Layout\n- A left of B\n- B left of A\n"
                .to_string(),
        )];
        let diags = validate(&bundle);
        assert!(diags.iter().any(|d| d.code == DiagCode::LayoutCycle));
    }

    #[test]
    fn consistent_placement_has_no_cycle() {
        let bundle = vec![(
            "d.md".to_string(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Layout\n- A left of B left of C\n- A above D\n".to_string(),
        )];
        let diags = validate(&bundle);
        assert!(!diags.iter().any(|d| d.code == DiagCode::LayoutCycle));
    }

    #[test]
    fn mismatched_fence_styles_do_not_hide_diagnostics() {
        // A `~~~`-fenced block containing a literal ``` line must not desync
        // the fence tracker: only a matching `~~~` should close it, and the
        // malformed line after the real close must still be flagged.
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Attributes\n~~~\nsome code\n```\nmore code\n~~~\n- bad line without colon\n".into())];
        let d = validate(&b);
        assert!(d.iter().any(|x| x.code == DiagCode::MalformedAttribute));
    }

    #[test]
    fn endless_associates_between_actor_and_use_case_is_clean() {
        let b = vec![
            ("u/place-order.md".into(),
             "---\ntype: uml.UseCase\ntitle: Place Order\n---\n# Place Order\n\n## Relationships\n- associates [Customer](./customer.md)\n- includes [Authenticate](./authenticate.md)\n".into()),
            ("u/customer.md".into(), "---\ntype: uml.Actor\ntitle: Customer\n---\n# Customer\n".into()),
            ("u/authenticate.md".into(), "---\ntype: uml.UseCase\ntitle: Authenticate\n---\n# Authenticate\n".into()),
        ];
        let d = validate(&b);
        assert!(d.is_empty(), "got: {d:?}");
    }

    #[test]
    fn endless_associates_between_classes_is_flagged() {
        let b = vec![
            ("c/order.md".into(),
             "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- associates [Customer](./customer.md)\n".into()),
            ("c/customer.md".into(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".into()),
        ];
        let d = validate(&b);
        let m = d
            .iter()
            .find(|x| x.code == DiagCode::MalformedRelationship)
            .expect("classifier↔classifier associates without ends must be flagged");
        assert_eq!(m.line, 8);
    }

    #[test]
    fn flow_doc_with_clean_graph_validates_clean() {
        let b = vec![("f/a.md".into(),
            "---\ntype: uml.Activity\ntitle: A\n---\n# A\n\n## Nodes\n\n### initial\n- transitions to Work\n\n### Work\n- transitions to final\n\n### final\n".into())];
        let d = validate(&b);
        assert!(d.is_empty(), "got: {d:?}");
    }

    #[test]
    fn flags_unresolved_local_transition_target() {
        let b = vec![("f/a.md".into(),
            "---\ntype: uml.Activity\ntitle: A\n---\n# A\n\n## Nodes\n\n### initial\n- transitions to Ghost\n\n### final\n".into())];
        let d = validate(&b);
        let t = d
            .iter()
            .find(|x| x.code == DiagCode::UnresolvedTarget)
            .unwrap();
        assert_eq!(t.severity, Severity::Warning);
        assert_eq!(t.line, 10);
    }

    #[test]
    fn flags_duplicate_flow_node_identity() {
        let b = vec![("f/a.md".into(),
            "---\ntype: uml.StateMachine\ntitle: A\n---\n# A\n\n## Nodes\n\n### Draft\n\n### Draft\n".into())];
        let d = validate(&b);
        assert!(d.iter().any(|x| x.code == DiagCode::DuplicateFlowNode));
    }

    #[test]
    fn flags_unknown_message_participant() {
        let b = vec![
            ("s/customer.md".into(), "---\ntype: uml.Actor\ntitle: Customer\n---\n# Customer\n".into()),
            ("s/seq.md".into(),
             "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n## Lifelines\n- [Customer](./customer.md)\n\n## Messages\n- Customer calls Ghost: `x()`\n".into()),
        ];
        let d = validate(&b);
        let w = d
            .iter()
            .find(|x| x.code == DiagCode::UnresolvedTarget && x.message.contains("Ghost"))
            .unwrap();
        assert_eq!(w.severity, Severity::Warning);
        assert_eq!(w.line, 11);
    }

    #[test]
    fn instance_of_unresolved_classifier_warns() {
        let b = vec![(
            "m/order42.md".into(),
            "---\ntype: uml.InstanceSpecification\ntitle: order42\n---\n# order42\n\n## Relationships\n- instance of [Gone](./gone.md)\n".into(),
        )];
        let d = crate::validate::validate(&b);
        let w = d
            .iter()
            .find(|x| x.code == DiagCode::InstanceOfUnresolved)
            .unwrap();
        assert_eq!(w.severity, crate::diagnostic::Severity::Warning);
        assert!(
            d.iter().all(|x| x.code != DiagCode::UnresolvedTarget),
            "instance-of must NOT surface as a hard UnresolvedTarget"
        );
    }

    #[test]
    fn instance_of_non_classifier_target_warns() {
        let b = vec![
            ("m/order42.md".into(), "---\ntype: uml.InstanceSpecification\ntitle: order42\n---\n# order42\n\n## Relationships\n- instance of [line42](./line42.md)\n".into()),
            ("m/line42.md".into(), "---\ntype: uml.InstanceSpecification\ntitle: line42\n---\n# line42\n".into()),
        ];
        let d = crate::validate::validate(&b);
        assert!(d.iter().any(|x| x.code == DiagCode::InstanceOfNonClassifier
            && x.severity == crate::diagnostic::Severity::Warning));
    }

    #[test]
    fn slot_unknown_attribute_warns() {
        let b = vec![
            ("m/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n".into()),
            ("m/order42.md".into(), "---\ntype: uml.InstanceSpecification\ntitle: order42\n---\n# order42\n\n## Relationships\n- instance of [Order](./order.md)\n\n## Slots\n- id: \"ORD-42\"\n- bogus: 3\n".into()),
        ];
        let d = crate::validate::validate(&b);
        let w: Vec<_> = d
            .iter()
            .filter(|x| x.code == DiagCode::SlotUnknownAttribute)
            .collect();
        assert_eq!(
            w.len(),
            1,
            "only the unknown slot 'bogus' warns; 'id' is a known attribute"
        );
        assert_eq!(w[0].severity, crate::diagnostic::Severity::Warning);
    }

    #[test]
    fn conformant_instance_produces_no_instance_warnings() {
        let b = vec![
            ("m/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n- status: Status {1}\n".into()),
            ("m/order42.md".into(), "---\ntype: uml.InstanceSpecification\ntitle: order42\n---\n# order42\n\n## Relationships\n- instance of [Order](./order.md)\n\n## Slots\n- id: \"ORD-42\"\n- status: PLACED\n".into()),
        ];
        let d = crate::validate::validate(&b);
        assert!(
            d.iter().all(|x| !matches!(
                x.code,
                DiagCode::SlotUnknownAttribute
                    | DiagCode::InstanceOfNonClassifier
                    | DiagCode::InstanceOfUnresolved
            )),
            "a conformant instance must emit no instance-conformance diagnostics: {d:?}"
        );
    }

    #[test]
    fn flags_unknown_message_participant_inside_fragment() {
        // Same check as `flags_unknown_message_participant`, but the offending
        // message sits inside an `alt` fragment's `when` operand (not
        // top-level) — this exercises `check_items`'s recursive call over
        // `Fragment { operands, .. }`.
        let b = vec![
            ("s/customer.md".into(), "---\ntype: uml.Actor\ntitle: Customer\n---\n# Customer\n".into()),
            ("s/seq.md".into(),
             "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n## Lifelines\n- [Customer](./customer.md)\n\n## Messages\n- alt\n  - when `guard`\n    - Customer calls Ghost: `x()`\n  - else\n    - Customer calls Customer: `y()`\n".into()),
        ];
        let d = validate(&b);
        let w = d
            .iter()
            .find(|x| x.code == DiagCode::UnresolvedTarget && x.message.contains("Ghost"))
            .unwrap();
        assert_eq!(w.severity, Severity::Warning);
        assert_eq!(w.line, 13);
    }
}
