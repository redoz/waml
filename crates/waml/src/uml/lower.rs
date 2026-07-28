use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use super::selector::{render_selector, RelBy, Selector};
use super::syntax::{AttributeSyntax, UmlLanguage, UmlSyntaxKind};
use super::{DiagramDisplaySet, FieldEdit, NameSpec};
use crate::edit::{EditContext, EditError};
use crate::model::{CardinalityVisibility, ElementType, RelEnd, RelationshipKind, Visibility};
use crate::multiplicity::Multiplicity;
use crate::okf;
use crate::source::{BundlePath, SourceBundle};
use crate::syntax::Direction;
use waml_syntax::{
    parse_okf_markdown, AstNode, MarkdownDialect, OkfMarkdownSyntaxKind, ShellParse, SourceText,
    SyntaxElement, SyntaxNode, SyntaxTree,
};

pub(crate) struct UmlLoweringCursor<'a> {
    original: EditContext<'a>,
    candidate: SourceBundle,
    state: UmlLoweringState,
}

pub(crate) struct UmlLoweringState {
    current_paths: BTreeMap<String, BundlePath>,
    touched_islands: BTreeMap<BundlePath, Arc<SyntaxTree<UmlLanguage>>>,
}

impl UmlLoweringState {
    pub(crate) fn from_context(context: &EditContext<'_>) -> Self {
        let current_paths = context
            .uml
            .claims
            .iter()
            .filter_map(|id| {
                context
                    .source
                    .document_by_concept_id(id)
                    .map(|document| (id.to_owned(), document.path().clone()))
            })
            .collect();
        let touched_islands = context
            .uml
            .syntax
            .documents()
            .values()
            .map(|snapshot| {
                (
                    snapshot.document().path().clone(),
                    snapshot.syntax().clone(),
                )
            })
            .collect();
        Self {
            current_paths,
            touched_islands,
        }
    }

    pub(crate) fn invalidate_text(&mut self, path: &BundlePath) {
        self.touched_islands.remove(path);
    }

    pub(crate) fn inserted_concept(
        &mut self,
        id: String,
        path: BundlePath,
    ) -> Result<(), EditError> {
        if self.current_paths.contains_key(&id)
            || self
                .current_paths
                .iter()
                .any(|(other, existing)| other != &id && existing == &path)
        {
            return Err(EditError::at(
                "uml.structure",
                format!("concept '{id}' or path '{}' already exists", path.as_str()),
            ));
        }
        self.invalidate_text(&path);
        self.current_paths.insert(id, path);
        Ok(())
    }

    pub(crate) fn removed_concept(&mut self, id: &str) {
        if let Some(path) = self.current_paths.remove(id) {
            self.invalidate_text(&path);
        }
    }

    pub(crate) fn renamed_concept(
        &mut self,
        from: &str,
        to: String,
        path: BundlePath,
    ) -> Result<(), EditError> {
        if from != to && self.current_paths.contains_key(&to) {
            return Err(EditError::at(
                "uml.structure",
                format!("concept '{to}' already exists"),
            ));
        }
        let old_path = self.current_paths.remove(from).ok_or_else(|| {
            EditError::at("uml.structure", format!("no claimed concept '{from}'"))
        })?;
        if self
            .current_paths
            .iter()
            .any(|(other, existing)| other != &to && existing == &path)
        {
            self.current_paths.insert(from.to_owned(), old_path);
            return Err(EditError::at(
                "uml.structure",
                format!("path '{}' already exists", path.as_str()),
            ));
        }
        self.invalidate_text(&old_path);
        self.invalidate_text(&path);
        self.current_paths.insert(to, path);
        Ok(())
    }

    pub(crate) fn path(&self, target: &str) -> Option<&BundlePath> {
        self.resolve_id(target)
            .and_then(|id| self.current_paths.get(id))
    }

    pub(crate) fn claimed_paths(&self) -> impl Iterator<Item = &BundlePath> {
        self.current_paths.values()
    }

    fn resolve_id(&self, target: &str) -> Option<&str> {
        if let Some((id, _)) = self.current_paths.get_key_value(target) {
            return Some(id);
        }
        let mut matches = self
            .current_paths
            .iter()
            .filter(|(_, path)| slug_of(path.as_str()) == target);
        match (matches.next(), matches.next()) {
            (Some((id, _)), None) => Some(id),
            _ => None,
        }
    }

    fn tree(
        &mut self,
        candidate: &SourceBundle,
        target: &str,
        op: &str,
    ) -> Result<(BundlePath, Arc<SyntaxTree<UmlLanguage>>), EditError> {
        let path = self
            .path(target)
            .cloned()
            .ok_or_else(|| EditError::at(op, format!("no claimed concept '{target}'")))?;
        if !self.touched_islands.contains_key(&path) {
            self.reparse(candidate, &path, op)?;
        }
        Ok((
            path.clone(),
            self.touched_islands
                .get(&path)
                .expect("candidate island was parsed")
                .clone(),
        ))
    }

    fn reparse(
        &mut self,
        candidate: &SourceBundle,
        path: &BundlePath,
        op: &str,
    ) -> Result<(), EditError> {
        let document = candidate
            .document(path)
            .ok_or_else(|| EditError::at(op, format!("no document '{}'", path.as_str())))?;
        let text = SourceText::from_shared(document.text_arc().clone())
            .map_err(|error| EditError::at(op, error.to_string()))?;
        let shell = parse_okf_markdown(text.clone(), MarkdownDialect::CommonMarkCurrent)
            .map_err(|error| EditError::at(op, error.to_string()))?;
        self.touched_islands.insert(
            path.clone(),
            super::syntax::parser::parse(text, &shell.structure),
        );
        Ok(())
    }
}

impl<'a> UmlLoweringCursor<'a> {
    pub(crate) fn new(context: EditContext<'a>) -> Self {
        let candidate = context.source.clone();
        let state = UmlLoweringState::from_context(&context);
        Self {
            original: context,
            candidate,
            state,
        }
    }

    pub(crate) fn apply(&mut self, index: usize, op: &super::Op) -> Result<(), EditError> {
        if index == 0 {
            validate_context(&self.original)?;
        }
        apply_step(&mut self.candidate, &mut self.state, index, op)
    }

    pub(crate) fn finish(self) -> SourceBundle {
        self.candidate
    }
}

pub(crate) fn apply_step(
    candidate: &mut SourceBundle,
    state: &mut UmlLoweringState,
    index: usize,
    op: &super::Op,
) -> Result<(), EditError> {
    let before: BTreeMap<_, _> = candidate
        .documents()
        .iter()
        .map(|document| (document.path().clone(), document.text_arc().clone()))
        .collect();
    let rename = match op {
        super::Op::ClassifierRename { from, to } => state
            .resolve_id(from)
            .map(str::to_owned)
            .map(|id| {
                let source = state.path(&id).expect("resolved classifier path");
                super::rename::destination_path(source, to).map(|destination| (id, destination))
            })
            .transpose()
            .map_err(|mut error| {
                error.index = index;
                error
            })?,
        _ => None,
    };
    let remove_id = match op {
        super::Op::ClassifierRemove { id, .. } => state.resolve_id(id).map(str::to_owned),
        _ => None,
    };
    super::ops::lower_one_with_state(candidate, state, op).map_err(|mut error| {
        error.index = index;
        error
    })?;

    match op {
        super::Op::ClassifierNew {
            slug, directory, ..
        } => {
            let directory = crate::okf::ops::legacy_path(directory);
            let inserted_path = BundlePath::parse(if directory.is_empty() {
                format!("{slug}.md")
            } else {
                format!("{directory}/{slug}.md")
            })
            .map_err(|error| EditError::at("node.new", error.to_string()))?;
            let document = candidate.document(&inserted_path).ok_or_else(|| {
                EditError::at("node.new", format!("inserted concept '{slug}' is absent"))
            })?;
            state
                .inserted_concept(
                    document
                        .path()
                        .concept_id()
                        .expect("classifier path")
                        .to_owned(),
                    document.path().clone(),
                )
                .map_err(|mut error| {
                    error.index = index;
                    error
                })?;
        }
        super::Op::ClassifierRemove { .. } => {
            if let Some(id) = remove_id {
                state.removed_concept(&id);
            }
        }
        super::Op::ClassifierRename { .. } => {
            if let Some((from, destination)) = rename {
                let document = candidate.document(&destination).ok_or_else(|| {
                    let mut error = EditError::at(
                        "node.rename",
                        format!("renamed concept '{}' is absent", destination.as_str()),
                    );
                    error.index = index;
                    error
                })?;
                state
                    .renamed_concept(
                        &from,
                        document
                            .path()
                            .concept_id()
                            .expect("classifier path")
                            .to_owned(),
                        document.path().clone(),
                    )
                    .map_err(|mut error| {
                        error.index = index;
                        error
                    })?;
            }
        }
        _ => {}
    }

    let changed: Vec<_> = candidate
        .documents()
        .iter()
        .filter(|document| {
            before
                .get(document.path())
                .map_or(true, |text| !Arc::ptr_eq(text, document.text_arc()))
        })
        .map(|document| document.path().clone())
        .collect();
    for path in changed {
        state.invalidate_text(&path);
        if state.current_paths.values().any(|claimed| claimed == &path) {
            state
                .reparse(candidate, &path, op_name(op))
                .map_err(|mut error| {
                    error.index = index;
                    error
                })?;
        }
    }
    Ok(())
}

fn validate_context(context: &EditContext<'_>) -> Result<(), EditError> {
    let catalog = &context.okf_analysis.catalog;
    if catalog.session_revision() != context.session_revision
        || context.uml.session_revision() != context.session_revision
        || !Arc::ptr_eq(catalog, context.okf_analysis.shell.catalog())
        || !Arc::ptr_eq(catalog, context.uml.syntax.catalog())
        || catalog.documents().len() != context.source.len()
    {
        return Err(EditError::at(
            "uml.context",
            "analysis/catalog revision does not match source",
        ));
    }
    Ok(())
}

fn op_name(op: &super::Op) -> &'static str {
    match op {
        super::Op::AttributeAdd { .. } => "attr.add",
        super::Op::AttributeSet { .. } => "attr.set",
        super::Op::AttributeRemove { .. } => "attr.rm",
        super::Op::ValueAdd { .. } => "value.add",
        super::Op::ValueRemove { .. } => "value.rm",
        super::Op::RelationshipAdd { .. } => "rel.add",
        super::Op::RelationshipSet { .. } => "rel.set",
        super::Op::RelationshipRemove { .. } => "rel.rm",
        super::Op::ClassifierNew { .. } => "node.new",
        super::Op::ClassifierSet { .. } => "node.set",
        super::Op::ClassifierRemove { .. } => "node.rm",
        super::Op::ClassifierRename { .. } => "node.rename",
        super::Op::DiagramSet { .. } => "diagram.set",
        super::Op::PlacementSet { .. } => "place.set",
        super::Op::PlacementRemove { .. } => "place.rm",
    }
}

pub(crate) fn slug_of(path: &str) -> String {
    let segment = path.rsplit(['/', '\\']).next().unwrap_or(path);
    segment.strip_suffix(".md").unwrap_or(segment).to_owned()
}

pub(crate) fn resolve_index(work: &SourceBundle, target: &str) -> Option<usize> {
    if let Some(index) = work
        .documents()
        .iter()
        .position(|document| okf::id_of(document.path().as_str()) == target)
    {
        return Some(index);
    }
    let mut matches = work
        .documents()
        .iter()
        .enumerate()
        .filter(|(_, document)| slug_of(document.path().as_str()) == target);
    match (matches.next(), matches.next()) {
        (Some((index, _)), None) => Some(index),
        _ => None,
    }
}

pub(crate) fn find_doc(work: &SourceBundle, target: &str, op: &str) -> Result<usize, EditError> {
    resolve_index(work, target).ok_or_else(|| EditError::at(op, format!("no document '{target}'")))
}

fn line_ending(source: &str) -> &'static str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn shell(source: &str, op: &str) -> Result<ShellParse, EditError> {
    let text = SourceText::from_shared(Arc::new(source.to_owned()))
        .map_err(|error| EditError::at(op, error.to_string()))?;
    parse_okf_markdown(text, MarkdownDialect::CommonMarkCurrent)
        .map_err(|error| EditError::at(op, error.to_string()))
}

fn syntax_nodes(
    node: &SyntaxNode<UmlLanguage>,
    kind: UmlSyntaxKind,
    output: &mut Vec<SyntaxNode<UmlLanguage>>,
) {
    for child in node.children().filter_map(SyntaxElement::into_node) {
        if child.kind() == kind {
            output.push(child.clone());
        }
        syntax_nodes(&child, kind, output);
    }
}

fn nodes(tree: &SyntaxTree<UmlLanguage>, kind: UmlSyntaxKind) -> Vec<SyntaxNode<UmlLanguage>> {
    let mut output = Vec::new();
    syntax_nodes(&tree.root(), kind, &mut output);
    output
}

fn node_range(node: &SyntaxNode<UmlLanguage>) -> Range<usize> {
    node.range().start().to_usize()..node.range().end().to_usize()
}

fn replace_range(
    work: &mut SourceBundle,
    path: &BundlePath,
    range: Range<usize>,
    replacement: &str,
    op: &str,
) -> Result<(), EditError> {
    let index = work
        .documents()
        .iter()
        .position(|document| document.path() == path)
        .ok_or_else(|| EditError::at(op, format!("no document '{}'", path.as_str())))?;
    work.document_at_mut(index)
        .expect("resolved document")
        .text_mut()
        .replace_range(range, replacement);
    Ok(())
}

fn replace_document(
    work: &mut SourceBundle,
    path: &BundlePath,
    replacement: String,
    op: &str,
) -> Result<(), EditError> {
    let index = work
        .documents()
        .iter()
        .position(|document| document.path() == path)
        .ok_or_else(|| EditError::at(op, format!("no document '{}'", path.as_str())))?;
    *work
        .document_at_mut(index)
        .expect("resolved document")
        .text_mut() = replacement;
    Ok(())
}

fn section_kind(name: &str) -> UmlSyntaxKind {
    match name {
        "Attributes" => UmlSyntaxKind::AttributesSection,
        "Values" => UmlSyntaxKind::ValuesSection,
        "Relationships" => UmlSyntaxKind::RelationshipsSection,
        "Layout" => UmlSyntaxKind::LayoutSection,
        _ => unreachable!("known operation section"),
    }
}

fn append_line(
    work: &mut SourceBundle,
    path: &BundlePath,
    tree: &SyntaxTree<UmlLanguage>,
    section: &str,
    line: &str,
    op: &str,
) -> Result<(), EditError> {
    let source = work.document(path).expect("claimed document").text();
    let newline = line_ending(source);
    let sections = nodes(tree, section_kind(section));
    if let Some(section_node) = sections.first() {
        let offset = section_node.range().end().to_usize();
        let prefix = if offset > 0 && source[..offset].ends_with('\n') {
            ""
        } else {
            newline
        };
        replace_range(
            work,
            path,
            offset..offset,
            &format!("{prefix}{line}{newline}"),
            op,
        )
    } else {
        let mut replacement = String::new();
        if !source.ends_with('\n') {
            replacement.push_str(newline);
        }
        if !source.ends_with(&format!("{newline}{newline}")) {
            replacement.push_str(newline);
        }
        replacement.push_str("## ");
        replacement.push_str(section);
        replacement.push_str(newline);
        replacement.push_str(line);
        replacement.push_str(newline);
        replace_range(work, path, source.len()..source.len(), &replacement, op)
    }
}

fn remove_owned_node(
    work: &mut SourceBundle,
    path: &BundlePath,
    tree: &SyntaxTree<UmlLanguage>,
    section_name: &str,
    node: &SyntaxNode<UmlLanguage>,
    op: &str,
) -> Result<(), EditError> {
    let source = work.document(path).expect("claimed document").text();
    let target = node_range(node);
    if let Some(section) = nodes(tree, section_kind(section_name)).first() {
        let section_range = node_range(section);
        let section_text = &source[section_range.clone()];
        let relative = target.start - section_range.start..target.end - section_range.start;
        let mut remainder = section_text.to_owned();
        remainder.replace_range(relative, "");
        let body = remainder
            .find('\n')
            .map(|offset| &remainder[offset + 1..])
            .unwrap_or("");
        if body.trim().is_empty() {
            let mut start = section_range.start;
            while start > 0 && matches!(source.as_bytes()[start - 1], b'\r' | b'\n') {
                start -= 1;
            }
            let newline = line_ending(source);
            return replace_range(
                work,
                path,
                start..section_range.end,
                if start == 0 { "" } else { newline },
                op,
            );
        }
    }
    replace_range(work, path, target, "", op)
}

fn frontmatter_entries(source: &str, op: &str) -> Result<Vec<(String, Range<usize>)>, EditError> {
    let parsed = shell(source, op)?;
    let mut entries = Vec::new();
    for frontmatter in parsed
        .tree
        .root()
        .children()
        .filter_map(SyntaxElement::into_node)
        .filter(|node| node.kind() == OkfMarkdownSyntaxKind::Frontmatter)
    {
        for node in frontmatter
            .children()
            .filter_map(SyntaxElement::into_node)
            .filter(|node| node.kind() == OkfMarkdownSyntaxKind::FrontmatterEntry)
        {
            let key = node
                .children()
                .filter_map(SyntaxElement::into_token)
                .find(|token| token.kind() == OkfMarkdownSyntaxKind::FrontmatterKey)
                .map(|token| token.text().write_to_string().trim().to_owned())
                .unwrap_or_default();
            entries.push((
                key,
                node.range().start().to_usize()..node.range().end().to_usize(),
            ));
        }
    }
    Ok(entries)
}

fn frontmatter_value(source: &str, wanted: &str, op: &str) -> Result<Option<String>, EditError> {
    for (key, range) in frontmatter_entries(source, op)? {
        if key == wanted {
            let line = source[range].trim_end_matches(['\r', '\n']);
            return Ok(line
                .split_once(':')
                .map(|(_, value)| decode_scalar(value.trim())));
        }
    }
    Ok(None)
}

fn decode_scalar(value: &str) -> String {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        value[1..value.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    } else {
        value.to_owned()
    }
}

fn frontmatter_number(value: &str) -> bool {
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    !whole.is_empty()
        && whole.bytes().all(|byte| byte.is_ascii_digit())
        && fraction.map_or(true, |digits| {
            !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
        && parts.next().is_none()
}

fn scalar(value: &str) -> String {
    let needs_quote = value.is_empty()
        || value != value.trim()
        || matches!(value, "true" | "false")
        || frontmatter_number(value)
        || (value.starts_with('[') && value.ends_with(']'))
        || value.starts_with('"')
        || value.contains('"')
        || value.contains('\\')
        || value.contains('\n');
    if needs_quote {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_owned()
    }
}

fn string_list(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| scalar(value))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn set_frontmatter(
    source: &mut String,
    key: &str,
    value: Option<&str>,
    op: &str,
) -> Result<(), EditError> {
    if let Some((_, range)) = frontmatter_entries(source, op)?
        .into_iter()
        .find(|(existing, _)| existing == key)
    {
        let newline = line_ending(source);
        let replacement = value
            .map(|value| format!("{key}: {value}{newline}"))
            .unwrap_or_default();
        source.replace_range(range, &replacement);
        return Ok(());
    }
    let Some(value) = value else {
        return Ok(());
    };
    let parsed = shell(source, op)?;
    let close = parsed
        .tree
        .root()
        .children()
        .filter_map(SyntaxElement::into_node)
        .find(|node| node.kind() == OkfMarkdownSyntaxKind::Frontmatter)
        .and_then(|node| {
            node.children()
                .filter_map(SyntaxElement::into_token)
                .find(|token| {
                    token.kind() == OkfMarkdownSyntaxKind::FrontmatterCloseFence
                        && !token.flags().is_missing()
                })
        })
        .ok_or_else(|| EditError::at(op, "claimed document has no clean frontmatter"))?;
    let newline = line_ending(source);
    source.insert_str(
        close.range().start().to_usize(),
        &format!("{key}: {value}{newline}"),
    );
    Ok(())
}

fn set_h1(source: &mut String, title: &str, op: &str) -> Result<(), EditError> {
    let parsed = shell(source, op)?;
    let heading = parsed
        .structure
        .headings
        .iter()
        .find(|heading| heading.level == 1)
        .ok_or_else(|| EditError::at(op, "claimed document has no title heading"))?;
    let range = heading.text_range.start().to_usize()..heading.text_range.end().to_usize();
    let authored = &source[range.clone()];
    let leading = authored.len() - authored.trim_start().len();
    let trailing = authored.len() - authored.trim_end().len();
    source.replace_range(range.start + leading..range.end - trailing, title);
    Ok(())
}

fn document_title(work: &SourceBundle, target: &str, op: &str) -> String {
    resolve_index(work, target)
        .and_then(|index| work.document_at(index))
        .and_then(|document| {
            frontmatter_value(document.text(), "title", op)
                .ok()
                .flatten()
                .or_else(|| {
                    shell(document.text(), op).ok().and_then(|parsed| {
                        parsed
                            .structure
                            .headings
                            .iter()
                            .find(|heading| heading.level == 1)
                            .map(|heading| {
                                document.text()[heading.text_range.start().to_usize()
                                    ..heading.text_range.end().to_usize()]
                                    .trim()
                                    .to_owned()
                            })
                    })
                })
        })
        .unwrap_or_else(|| target.to_owned())
}

fn stored_slug(work: &SourceBundle, target: &str) -> String {
    resolve_index(work, target)
        .and_then(|index| work.document_at(index))
        .map(|document| slug_of(document.path().as_str()))
        .unwrap_or_else(|| target.to_owned())
}

fn type_text(work: &SourceBundle, token: &str) -> String {
    resolve_index(work, token)
        .and_then(|index| work.document_at(index))
        .map(|document| {
            format!(
                "[{}](./{}.md)",
                document_title(work, token, "attr.type"),
                slug_of(document.path().as_str())
            )
        })
        .unwrap_or_else(|| token.to_owned())
}

#[derive(Clone)]
struct AttributeLine {
    visibility: Option<Visibility>,
    name: String,
    ty: String,
    multiplicity: Option<String>,
}

fn parse_attribute(source: &str) -> Option<AttributeLine> {
    let line = source.trim_end_matches(['\r', '\n']).strip_prefix("- ")?;
    let (visibility, line) = match line.chars().next().and_then(Visibility::from_marker) {
        Some(visibility) => (Some(visibility), line[1..].trim_start()),
        None => (None, line),
    };
    let (name, rest) = line.split_once(':')?;
    let rest = rest.trim();
    let (ty, multiplicity) = if let Some(start) = rest.rfind(" {") {
        if rest.ends_with('}') {
            (
                rest[..start].trim().to_owned(),
                Some(rest[start + 2..rest.len() - 1].to_owned()),
            )
        } else {
            (rest.to_owned(), None)
        }
    } else {
        (rest.to_owned(), None)
    };
    Some(AttributeLine {
        visibility,
        name: name.trim().to_owned(),
        ty,
        multiplicity,
    })
}

fn render_attribute(attribute: &AttributeLine) -> String {
    let visibility = attribute
        .visibility
        .map(|value| format!("{} ", value.marker()))
        .unwrap_or_default();
    let multiplicity = attribute
        .multiplicity
        .as_ref()
        .map(|value| format!(" {{{value}}}"))
        .unwrap_or_default();
    format!(
        "- {visibility}{}: {}{multiplicity}",
        attribute.name, attribute.ty
    )
}

fn attribute_nodes(
    source: &str,
    tree: &SyntaxTree<UmlLanguage>,
) -> Vec<(SyntaxNode<UmlLanguage>, AttributeLine)> {
    nodes(tree, UmlSyntaxKind::Attribute)
        .into_iter()
        .filter_map(|node| {
            let parsed = AttributeSyntax::cast(node.clone())?;
            let mut value = parse_attribute(&source[node_range(&node)])?;
            value.name = parsed.name_token().text().write_to_string();
            Some((node, value))
        })
        .collect()
}

pub(crate) fn op_attr_add(
    work: &mut SourceBundle,
    state: &mut UmlLoweringState,
    node: &str,
    name: &str,
    ty_token: &str,
    multiplicity: &Option<Multiplicity>,
    visibility: Option<Visibility>,
) -> Result<(), EditError> {
    let (path, tree) = state.tree(work, node, "attr.add")?;
    let source = work.document(&path).expect("claimed document").text();
    if attribute_nodes(source, &tree)
        .iter()
        .any(|(_, attribute)| attribute.name == name)
    {
        return Err(EditError::at(
            "attr.add",
            format!("attribute '{name}' already exists in {node}"),
        ));
    }
    let line = render_attribute(&AttributeLine {
        visibility,
        name: name.to_owned(),
        ty: type_text(work, ty_token),
        multiplicity: multiplicity.as_ref().map(|value| value.as_str().to_owned()),
    });
    append_line(work, &path, &tree, "Attributes", &line, "attr.add")
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn op_attr_set(
    work: &mut SourceBundle,
    state: &mut UmlLoweringState,
    node: &str,
    name: &str,
    ty_token: &Option<String>,
    multiplicity: &FieldEdit<Multiplicity>,
    visibility: Option<Visibility>,
    rename: &Option<String>,
) -> Result<(), EditError> {
    let (path, tree) = state.tree(work, node, "attr.set")?;
    let source = work.document(&path).expect("claimed document").text();
    let attributes = attribute_nodes(source, &tree);
    if let Some(new_name) = rename {
        if new_name != name
            && attributes
                .iter()
                .any(|(_, attribute)| attribute.name == *new_name)
        {
            return Err(EditError::at(
                "attr.set",
                format!("attribute '{new_name}' already exists in {node}"),
            ));
        }
    }
    let (syntax, mut attribute) = attributes
        .into_iter()
        .find(|(_, attribute)| attribute.name == name)
        .ok_or_else(|| EditError::at("attr.set", format!("no attribute '{name}' in {node}")))?;
    if let Some(token) = ty_token {
        attribute.ty = type_text(work, token);
    }
    match multiplicity {
        FieldEdit::Unchanged => {}
        FieldEdit::Clear => attribute.multiplicity = None,
        FieldEdit::Set(value) => attribute.multiplicity = Some(value.as_str().to_owned()),
    }
    if let Some(value) = visibility {
        attribute.visibility = Some(value);
    }
    if let Some(value) = rename {
        attribute.name = value.clone();
    }
    let newline = line_ending(source);
    replace_range(
        work,
        &path,
        node_range(&syntax),
        &format!("{}{newline}", render_attribute(&attribute)),
        "attr.set",
    )
}

pub(crate) fn op_attr_rm(
    work: &mut SourceBundle,
    state: &mut UmlLoweringState,
    node: &str,
    name: &str,
) -> Result<(), EditError> {
    let (path, tree) = state.tree(work, node, "attr.rm")?;
    let source = work.document(&path).expect("claimed document").text();
    let syntax = attribute_nodes(source, &tree)
        .into_iter()
        .find(|(_, attribute)| attribute.name == name)
        .map(|(syntax, _)| syntax)
        .ok_or_else(|| EditError::at("attr.rm", format!("no attribute '{name}' in {node}")))?;
    remove_owned_node(work, &path, &tree, "Attributes", &syntax, "attr.rm")
}

fn value_literal(source: &str) -> Option<String> {
    source
        .trim_end_matches(['\r', '\n'])
        .strip_prefix("- ")
        .map(str::trim)
        .map(str::to_owned)
}

pub(crate) fn op_value_add(
    work: &mut SourceBundle,
    state: &mut UmlLoweringState,
    node: &str,
    literal: &str,
) -> Result<(), EditError> {
    let (path, tree) = state.tree(work, node, "value.add")?;
    let source = work.document(&path).expect("claimed document").text();
    if nodes(&tree, UmlSyntaxKind::Value)
        .iter()
        .filter_map(|syntax| value_literal(&source[node_range(syntax)]))
        .any(|value| value == literal)
    {
        return Err(EditError::at(
            "value.add",
            format!("value '{literal}' already in {node}"),
        ));
    }
    append_line(
        work,
        &path,
        &tree,
        "Values",
        &format!("- {literal}"),
        "value.add",
    )
}

pub(crate) fn op_value_rm(
    work: &mut SourceBundle,
    state: &mut UmlLoweringState,
    node: &str,
    literal: &str,
) -> Result<(), EditError> {
    let (path, tree) = state.tree(work, node, "value.rm")?;
    let source = work.document(&path).expect("claimed document").text();
    let syntax = nodes(&tree, UmlSyntaxKind::Value)
        .into_iter()
        .find(|syntax| value_literal(&source[node_range(syntax)]).as_deref() == Some(literal))
        .ok_or_else(|| EditError::at("value.rm", format!("no value '{literal}' in {node}")))?;
    remove_owned_node(work, &path, &tree, "Values", &syntax, "value.rm")
}

#[derive(Clone)]
struct RelationshipLine {
    kind: RelationshipKind,
    target: String,
    target_slug: String,
    name: Option<String>,
    ends: Option<String>,
}

fn relationship_kinds() -> &'static [RelationshipKind] {
    &[
        RelationshipKind::InstanceOf,
        RelationshipKind::Associates,
        RelationshipKind::Aggregates,
        RelationshipKind::Composes,
        RelationshipKind::Specializes,
        RelationshipKind::Implements,
        RelationshipKind::Depends,
        RelationshipKind::Annotates,
        RelationshipKind::Includes,
        RelationshipKind::Extends,
        RelationshipKind::Links,
    ]
}

fn parse_relationship(source: &str) -> Option<RelationshipLine> {
    let mut body = source.trim_end_matches(['\r', '\n']).strip_prefix("- ")?;
    let kind = relationship_kinds()
        .iter()
        .copied()
        .find(|kind| body.starts_with(&format!("{} ", kind.as_str())))?;
    body = body[kind.as_str().len()..].trim_start();
    let open = body.find('[')?;
    let middle = body[open..].find("](")? + open;
    let close = body[middle + 2..].find(')')? + middle + 2;
    let target = body[open + 1..middle].to_owned();
    let href = &body[middle + 2..close];
    let target_slug = href
        .trim_start_matches("./")
        .strip_suffix(".md")
        .unwrap_or(href)
        .rsplit('/')
        .next()
        .unwrap_or(href)
        .to_owned();
    let tail = body[close + 1..].trim();
    let (tail, ends) = tail
        .split_once(':')
        .map(|(name, ends)| (name.trim(), Some(ends.trim().to_owned())))
        .unwrap_or((tail, None));
    let name = tail.strip_prefix("as ").map(str::trim).map(str::to_owned);
    Some(RelationshipLine {
        kind,
        target,
        target_slug,
        name,
        ends,
    })
}

fn render_end(end: &RelEnd) -> String {
    let multiplicity = end
        .multiplicity
        .as_ref()
        .map(Multiplicity::as_str)
        .unwrap_or("1");
    end.role
        .as_ref()
        .map(|role| format!("{multiplicity} {role}"))
        .unwrap_or_else(|| multiplicity.to_owned())
}

fn render_name(work: &SourceBundle, name: &NameSpec) -> String {
    match name {
        NameSpec::Label(label) => format!("\"{label}\""),
        NameSpec::Ref(target) => format!(
            "[{}](./{}.md)",
            document_title(work, target, "rel.name"),
            stored_slug(work, target)
        ),
    }
}

fn render_relationship(relationship: &RelationshipLine) -> String {
    let name = relationship
        .name
        .as_ref()
        .map(|name| format!(" as {name}"))
        .unwrap_or_default();
    let ends = relationship
        .ends
        .as_ref()
        .map(|ends| format!(": {ends}"))
        .unwrap_or_default();
    format!(
        "- {} [{}](./{}.md){name}{ends}",
        relationship.kind.as_str(),
        relationship.target,
        relationship.target_slug
    )
}

fn relationship_matches(relationship: &RelationshipLine, by: &RelBy) -> bool {
    match by {
        RelBy::Endpoint { kind, target } => {
            relationship.kind == *kind && relationship.target_slug == *target
        }
        RelBy::Named(name) => relationship.name.as_deref().is_some_and(|authored| {
            authored.trim_matches('"') == name
                || authored
                    .strip_prefix('[')
                    .and_then(|value| value.split_once(']'))
                    .is_some_and(|(title, _)| title == name)
        }),
    }
}

pub(crate) fn op_rel_add(
    work: &mut SourceBundle,
    state: &mut UmlLoweringState,
    source_id: &str,
    kind: RelationshipKind,
    target: &str,
    name: &Option<NameSpec>,
    ends: &Option<(RelEnd, RelEnd)>,
) -> Result<(), EditError> {
    if kind.is_ended() != ends.is_some() {
        return Err(EditError::at(
            "rel.add",
            if kind.is_ended() {
                format!("relationship '{}' requires ends", kind.as_str())
            } else {
                format!("relationship '{}' does not take ends", kind.as_str())
            },
        ));
    }
    let (path, tree) = state.tree(work, source_id, "rel.add")?;
    let source = work.document(&path).expect("claimed document").text();
    let target_slug = stored_slug(work, target);
    if nodes(&tree, UmlSyntaxKind::Relationship)
        .iter()
        .filter_map(|syntax| parse_relationship(&source[node_range(syntax)]))
        .any(|relationship| relationship.kind == kind && relationship.target_slug == target_slug)
    {
        return Err(EditError::at(
            "rel.add",
            format!(
                "relationship '{} {target}' already exists in {source_id}",
                kind.as_str()
            ),
        ));
    }
    let relationship = RelationshipLine {
        kind,
        target: document_title(work, target, "rel.add"),
        target_slug,
        name: name.as_ref().map(|name| render_name(work, name)),
        ends: ends
            .as_ref()
            .map(|(from, to)| format!("{} to {}", render_end(from), render_end(to))),
    };
    append_line(
        work,
        &path,
        &tree,
        "Relationships",
        &render_relationship(&relationship),
        "rel.add",
    )
}

fn relationship_target<'a>(
    selector: &'a Selector,
    op: &str,
) -> Result<(&'a str, &'a RelBy), EditError> {
    match selector {
        Selector::Rel { source, by } => Ok((source, by)),
        _ => Err(EditError::at(
            op,
            format!(
                "selector '{}' does not address a relationship",
                render_selector(selector)
            ),
        )
        .with_sel(render_selector(selector))),
    }
}

pub(crate) fn op_rel_set(
    work: &mut SourceBundle,
    state: &mut UmlLoweringState,
    selector: &Selector,
    ends: &Option<(RelEnd, RelEnd)>,
    name: &Option<NameSpec>,
) -> Result<(), EditError> {
    let (source_id, by) = relationship_target(selector, "rel.set")?;
    let resolved = match by {
        RelBy::Endpoint { kind, target } => RelBy::Endpoint {
            kind: *kind,
            target: stored_slug(work, target),
        },
        RelBy::Named(name) => RelBy::Named(name.clone()),
    };
    let (path, tree) = state.tree(work, source_id, "rel.set")?;
    let source = work.document(&path).expect("claimed document").text();
    let (syntax, mut relationship) = nodes(&tree, UmlSyntaxKind::Relationship)
        .into_iter()
        .filter_map(|syntax| {
            parse_relationship(&source[node_range(&syntax)]).map(|value| (syntax, value))
        })
        .find(|(_, relationship)| relationship_matches(relationship, &resolved))
        .ok_or_else(|| {
            let display = render_selector(selector);
            EditError::at("rel.set", format!("no relationship '{display}'")).with_sel(display)
        })?;
    if let Some((from, to)) = ends {
        if !relationship.kind.is_ended() {
            return Err(EditError::at(
                "rel.set",
                format!("'{}' does not take ends", relationship.kind.as_str()),
            ));
        }
        relationship.ends = Some(format!("{} to {}", render_end(from), render_end(to)));
    }
    if let Some(name) = name {
        relationship.name = Some(render_name(work, name));
    }
    let newline = line_ending(source);
    replace_range(
        work,
        &path,
        node_range(&syntax),
        &format!("{}{newline}", render_relationship(&relationship)),
        "rel.set",
    )
}

pub(crate) fn op_rel_rm(
    work: &mut SourceBundle,
    state: &mut UmlLoweringState,
    selector: &Selector,
) -> Result<(), EditError> {
    let (source_id, by) = relationship_target(selector, "rel.rm")?;
    let resolved = match by {
        RelBy::Endpoint { kind, target } => RelBy::Endpoint {
            kind: *kind,
            target: stored_slug(work, target),
        },
        RelBy::Named(name) => RelBy::Named(name.clone()),
    };
    let (path, tree) = state.tree(work, source_id, "rel.rm")?;
    let source = work.document(&path).expect("claimed document").text();
    let syntax = nodes(&tree, UmlSyntaxKind::Relationship)
        .into_iter()
        .find(|syntax| {
            parse_relationship(&source[node_range(syntax)])
                .is_some_and(|relationship| relationship_matches(&relationship, &resolved))
        })
        .ok_or_else(|| {
            let display = render_selector(selector);
            EditError::at("rel.rm", format!("no relationship '{display}'")).with_sel(display)
        })?;
    remove_owned_node(work, &path, &tree, "Relationships", &syntax, "rel.rm")
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn op_node_new(
    work: &mut SourceBundle,
    slug: &str,
    directory: &str,
    ty: &ElementType,
    title: &str,
    stereotype: &[String],
    description: &Option<String>,
    abstract_: bool,
) -> Result<(), EditError> {
    let path = if directory.is_empty() {
        format!("{slug}.md")
    } else {
        format!("{directory}/{slug}.md")
    };
    if work
        .documents()
        .iter()
        .any(|document| okf::id_of(document.path().as_str()) == okf::id_of(&path))
    {
        return Err(EditError::at(
            "node.new",
            format!("document '{slug}' already exists"),
        ));
    }
    let mut source = format!("---\ntype: {}\n", scalar(&ty.as_str()));
    if !stereotype.is_empty() {
        source.push_str(&format!("stereotype: {}\n", string_list(stereotype)));
    }
    if abstract_ {
        source.push_str("abstract: true\n");
    }
    source.push_str(&format!("title: {}\n", scalar(title)));
    if let Some(description) = description {
        source.push_str(&format!("description: {}\n", scalar(description)));
    }
    source.push_str(&format!("---\n\n# {title}\n"));
    work.push_document(path, source)
        .map_err(|error| EditError::at("node.new", error.to_string()))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn op_node_set(
    work: &mut SourceBundle,
    state: &mut UmlLoweringState,
    id: &str,
    title: &Option<String>,
    description: &Option<String>,
    stereotype: &Option<Vec<String>>,
    abstract_: &Option<bool>,
    ty: &Option<ElementType>,
) -> Result<(), EditError> {
    let (path, _) = state.tree(work, id, "node.set")?;
    let mut source = work
        .document(&path)
        .expect("claimed document")
        .text()
        .to_owned();
    if let Some(value) = title {
        set_frontmatter(&mut source, "title", Some(&scalar(value)), "node.set")?;
        set_h1(&mut source, value, "node.set")?;
    }
    if let Some(value) = description {
        set_frontmatter(&mut source, "description", Some(&scalar(value)), "node.set")?;
    }
    if let Some(values) = stereotype {
        set_frontmatter(
            &mut source,
            "stereotype",
            Some(&string_list(values)),
            "node.set",
        )?;
    }
    if let Some(value) = abstract_ {
        set_frontmatter(
            &mut source,
            "abstract",
            Some(if *value { "true" } else { "false" }),
            "node.set",
        )?;
    }
    if let Some(value) = ty {
        set_frontmatter(
            &mut source,
            "type",
            Some(&scalar(&value.as_str())),
            "node.set",
        )?;
    }
    replace_document(work, &path, source, "node.set")
}

const DISPLAY_KEYS: &[&str] = &[
    "showAttributes",
    "showType",
    "attributeDetail",
    "showAttributeVisibility",
    "showAttributeMultiplicity",
    "cardinality",
    "maxAttributes",
    "showRoles",
    "showCardinality",
    "showLabels",
    "showStereotype",
    "stereotypeFilter",
    "stereotypeColors",
];

pub(crate) fn op_diagram_set(
    work: &mut SourceBundle,
    state: &mut UmlLoweringState,
    key: &str,
    title: &Option<String>,
    description: &Option<String>,
    clear_description: bool,
    display: &Option<DiagramDisplaySet>,
) -> Result<(), EditError> {
    if description
        .as_deref()
        .is_some_and(|value| value.contains('\n') || value.contains('\r'))
    {
        return Err(
            EditError::at("diagram.set", "description must be one line").with_sel(key.to_owned())
        );
    }
    let (path, _) = state.tree(work, key, "diagram.set")?;
    let mut source = work
        .document(&path)
        .expect("claimed document")
        .text()
        .to_owned();
    if let Some(value) = title {
        set_frontmatter(&mut source, "title", Some(&scalar(value)), "diagram.set")?;
        set_h1(&mut source, value, "diagram.set")?;
    }
    if clear_description {
        set_frontmatter(&mut source, "description", None, "diagram.set")?;
    } else if let Some(value) = description {
        set_frontmatter(
            &mut source,
            "description",
            Some(&scalar(value)),
            "diagram.set",
        )?;
    }
    if let Some(display) = display {
        for key in DISPLAY_KEYS {
            set_frontmatter(&mut source, key, None, "diagram.set")?;
        }
        let values = [
            ("showAttributes", display.show_attributes.to_string()),
            ("showType", display.show_type.to_string()),
            (
                "showAttributeVisibility",
                display.show_attribute_visibility.to_string(),
            ),
            (
                "showAttributeMultiplicity",
                display.cardinality.legacy_attribute_gate().to_string(),
            ),
            ("showRoles", display.show_roles.to_string()),
            ("showCardinality", display.show_cardinality.to_string()),
            (
                "cardinality",
                match display.cardinality {
                    CardinalityVisibility::Off => "off",
                    CardinalityVisibility::Explicit => "explicit",
                    CardinalityVisibility::All => "all",
                }
                .to_owned(),
            ),
            ("showLabels", display.show_labels.to_string()),
            ("showStereotype", display.show_stereotype.to_string()),
        ];
        for (key, value) in values {
            set_frontmatter(&mut source, key, Some(&value), "diagram.set")?;
        }
        if let Some(value) = display.max_attributes {
            set_frontmatter(
                &mut source,
                "maxAttributes",
                Some(&value.to_string()),
                "diagram.set",
            )?;
        }
        if let Some(values) = &display.stereotype_filter {
            set_frontmatter(
                &mut source,
                "stereotypeFilter",
                Some(&string_list(values)),
                "diagram.set",
            )?;
        }
        if !display.stereotype_colors.is_empty() {
            set_frontmatter(
                &mut source,
                "stereotypeColors",
                Some(&string_list(&display.stereotype_colors)),
                "diagram.set",
            )?;
        }
    }
    replace_document(work, &path, source, "diagram.set")
}

fn direction_text(direction: Direction) -> &'static str {
    match direction {
        Direction::LeftOf => "left of",
        Direction::RightOf => "right of",
        Direction::Above => "above",
        Direction::Below => "below",
        Direction::AboveLeft => "above left of",
        Direction::AboveRight => "above right of",
        Direction::BelowLeft => "below left of",
        Direction::BelowRight => "below right of",
    }
}

fn placement_matches(source: &str, subject: &str, reference: &str) -> bool {
    let subject = format!("./{subject}.md");
    let reference = format!("./{reference}.md");
    source.contains(&subject) && source.contains(&reference)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn op_place_set(
    work: &mut SourceBundle,
    state: &mut UmlLoweringState,
    diagram: &str,
    subject_title: &str,
    subject_slug: &str,
    reference_title: &str,
    reference_slug: &str,
    directions: &[Direction],
) -> Result<(), EditError> {
    let (path, tree) = state.tree(work, diagram, "place.set")?;
    let source = work
        .document(&path)
        .expect("claimed document")
        .text()
        .to_owned();
    let mut ranges: Vec<_> = nodes(&tree, UmlSyntaxKind::LayoutStatement)
        .into_iter()
        .map(|syntax| node_range(&syntax))
        .filter(|range| placement_matches(&source[range.clone()], subject_slug, reference_slug))
        .collect();
    ranges.sort_by_key(|range| std::cmp::Reverse(range.start));
    for range in ranges {
        replace_range(work, &path, range, "", "place.set")?;
    }
    state.invalidate_text(&path);
    state.reparse(work, &path, "place.set")?;
    let mut tree = state
        .touched_islands
        .get(&path)
        .expect("placement candidate reparsed")
        .clone();
    for direction in directions {
        append_line(
            work,
            &path,
            &tree,
            "Layout",
            &format!(
                "- [{subject_title}](./{subject_slug}.md) {} [{reference_title}](./{reference_slug}.md)",
                direction_text(*direction)
            ),
            "place.set",
        )?;
        state.invalidate_text(&path);
        state.reparse(work, &path, "place.set")?;
        tree = state
            .touched_islands
            .get(&path)
            .expect("placement candidate reparsed")
            .clone();
    }
    Ok(())
}

pub(crate) fn op_place_rm(
    work: &mut SourceBundle,
    state: &mut UmlLoweringState,
    diagram: &str,
    subject_slug: &str,
    reference_slug: &str,
) -> Result<(), EditError> {
    let (path, tree) = state.tree(work, diagram, "place.rm")?;
    let source = work
        .document(&path)
        .expect("claimed document")
        .text()
        .to_owned();
    let mut matches: Vec<_> = nodes(&tree, UmlSyntaxKind::LayoutStatement)
        .into_iter()
        .filter(|syntax| {
            placement_matches(&source[node_range(syntax)], subject_slug, reference_slug)
        })
        .collect();
    matches.sort_by_key(|syntax| std::cmp::Reverse(node_range(syntax).start));
    for syntax in matches {
        let current_tree = state.tree(work, diagram, "place.rm")?.1;
        remove_owned_node(work, &path, &current_tree, "Layout", &syntax, "place.rm")?;
        state.invalidate_text(&path);
        state.reparse(work, &path, "place.rm")?;
    }
    Ok(())
}

pub(crate) fn op_node_rm(
    work: &mut SourceBundle,
    state: &mut UmlLoweringState,
    id: &str,
    cascade: bool,
) -> Result<(), EditError> {
    let path = state
        .path(id)
        .cloned()
        .ok_or_else(|| EditError::at("node.rm", format!("no document '{id}'")))?;
    if !cascade {
        let references = referrers_source(work, id);
        if !references.is_empty() {
            return Err(EditError::at(
                "node.rm",
                format!(
                    "'{id}' referenced by: {} (use --cascade)",
                    references.join(", ")
                ),
            ));
        }
    }
    let index = work
        .documents()
        .iter()
        .position(|document| document.path() == &path)
        .expect("claimed path exists");
    work.remove_document(index);
    Ok(())
}

pub fn referrers(work: &[(String, String)], slug: &str) -> Vec<String> {
    let Ok(source) = SourceBundle::try_from_pairs(work.iter().cloned()) else {
        return Vec::new();
    };
    referrers_source(&source, slug)
}

pub fn referrers_source(work: &SourceBundle, slug: &str) -> Vec<String> {
    let target_index = resolve_index(work, slug);
    let target = target_index
        .and_then(|index| work.document_at(index))
        .map(|document| slug_of(document.path().as_str()))
        .unwrap_or_else(|| slug.to_owned());
    let href = format!("./{target}.md");
    let mut output = Vec::new();
    for (index, document) in work.documents().iter().enumerate() {
        if Some(index) == target_index {
            continue;
        }
        let Ok(parsed) = shell(document.text(), "refs") else {
            continue;
        };
        let text = match SourceText::from_shared(document.text_arc().clone()) {
            Ok(text) => text,
            Err(_) => continue,
        };
        let tree = super::syntax::parser::parse(text, &parsed.structure);
        let referenced = [
            UmlSyntaxKind::Attribute,
            UmlSyntaxKind::Relationship,
            UmlSyntaxKind::Member,
            UmlSyntaxKind::InlineInstance,
            UmlSyntaxKind::LayoutStatement,
        ]
        .into_iter()
        .flat_map(|kind| nodes(&tree, kind))
        .any(|syntax| {
            let authored = &document.text()[node_range(&syntax)];
            authored.contains(&href)
                || (syntax.kind() == UmlSyntaxKind::LayoutStatement
                    && authored
                        .split(|character: char| {
                            character.is_whitespace()
                                || matches!(character, '[' | ']' | '(' | ')' | ',')
                        })
                        .any(|word| word == target))
        });
        if referenced {
            output.push(slug_of(document.path().as_str()));
        }
    }
    output.sort();
    output.dedup();
    output
}
