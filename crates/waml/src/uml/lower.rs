use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use super::selector::{render_selector, RelBy, Selector};
use super::syntax::{
    AttributeSyntax, FlowNodeSyntax, FlowTransitionSyntax, RelationshipSyntax, UmlLanguage,
    UmlSyntaxKind,
};
use super::{DiagramDisplaySet, FieldEdit, NameSpec, TraceEdit, TransitionSelector};
use crate::edit::{EditCode, EditContext, EditError};
use crate::layout::Direction;
use crate::model::{CardinalityVisibility, ElementType, RelEnd, RelationshipKind, Visibility};
use crate::multiplicity::Multiplicity;
use crate::okf;
use crate::source::{BundlePath, SourceBundle};
use waml_syntax::{
    parse_markdown, AstNode, DocumentRevision, MarkdownDialect, OkfMarkdownSyntaxKind, ShellParse,
    SourceText, SyntaxElement, SyntaxNode, SyntaxToken, SyntaxTree,
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
            return Err(EditError::new(
                EditCode::AlreadyExists,
                "uml.structure",
                format!("concept '{id}' or path '{}' already exists", path.as_str()),
            )
            .about(id));
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
            return Err(EditError::new(
                EditCode::AlreadyExists,
                "uml.structure",
                format!("concept '{to}' already exists"),
            )
            .about(to));
        }
        let old_path = self.current_paths.remove(from).ok_or_else(|| {
            EditError::new(
                EditCode::NotFound,
                "uml.structure",
                format!("no claimed concept '{from}'"),
            )
            .about(from)
        })?;
        if self
            .current_paths
            .iter()
            .any(|(other, existing)| other != &to && existing == &path)
        {
            self.current_paths.insert(from.to_owned(), old_path);
            return Err(EditError::new(
                EditCode::AlreadyExists,
                "uml.structure",
                format!("path '{}' already exists", path.as_str()),
            )
            .about(path.as_str()));
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
        super::unique_match(self.current_paths.iter(), |(_, path)| {
            slug_of(path.as_str()) == target
        })
        .map(|(id, _)| id.as_str())
    }

    fn tree(
        &mut self,
        candidate: &SourceBundle,
        target: &str,
        op: &str,
    ) -> Result<(BundlePath, Arc<SyntaxTree<UmlLanguage>>), EditError> {
        let path = self.path(target).cloned().ok_or_else(|| {
            EditError::new(
                EditCode::NotFound,
                op,
                format!("no claimed concept '{target}'"),
            )
            .about(target)
        })?;
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
        let document = candidate.document(path).ok_or_else(|| {
            EditError::new(
                EditCode::NotFound,
                op,
                format!("no document '{}'", path.as_str()),
            )
            .about(path.as_str())
        })?;
        let text = SourceText::from_shared(document.text_arc().clone())
            .map_err(|error| EditError::wrap(op, &error))?;
        let markdown = parse_markdown(
            DocumentRevision::INITIAL,
            text.clone(),
            MarkdownDialect::WAML_DEFAULT,
        )
        .map_err(|error| EditError::wrap(op, &error))?;
        self.touched_islands.insert(
            path.clone(),
            super::syntax::parse_full(text, markdown.structure()),
        );
        Ok(())
    }
}

impl crate::edit::InvalidationSink for UmlLoweringState {
    fn absorb(&mut self, event: &crate::edit::Invalidation) -> Result<(), EditError> {
        use crate::edit::Invalidation;
        match event {
            Invalidation::TextChanged(path) => self.invalidate_text(path),
            Invalidation::Inserted { id, path } => match id {
                Some(id) => self.inserted_concept(id.clone(), path.clone())?,
                None => self.invalidate_text(path),
            },
            Invalidation::Removed { id, path } => {
                if let Some(id) = id {
                    self.removed_concept(id);
                }
                self.invalidate_text(path);
            }
            Invalidation::Renamed {
                id_from,
                id_to,
                from,
                to,
            } => match (id_from, id_to) {
                (Some(from_id), Some(to_id)) => {
                    self.renamed_concept(from_id, to_id.clone(), to.clone())?;
                }
                (Some(from_id), None) => {
                    self.removed_concept(from_id);
                    self.invalidate_text(to);
                }
                (None, Some(to_id)) => {
                    self.inserted_concept(to_id.clone(), to.clone())?;
                }
                (None, None) => {
                    self.invalidate_text(from);
                    self.invalidate_text(to);
                }
            },
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_tree_error_names_requested_concept() {
        let source = SourceBundle::default();
        let mut state = UmlLoweringState {
            current_paths: BTreeMap::new(),
            touched_islands: BTreeMap::new(),
        };

        let error = match state.tree(&source, "missing-order", "attr.add") {
            Err(error) => error,
            Ok(_) => panic!("missing concept must fail"),
        };

        assert_eq!(error.op, "attr.add");
        assert_eq!(error.reason, "no claimed concept 'missing-order'");
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
            let directory = okf::ops::legacy_path(directory);
            let inserted_path = BundlePath::parse(if directory.is_empty() {
                format!("{slug}.md")
            } else {
                format!("{directory}/{slug}.md")
            })
            .map_err(|error| EditError::wrap("node.new", &error))?;
            let document = candidate.document(&inserted_path).ok_or_else(|| {
                EditError::new(
                    EditCode::Internal,
                    "node.new",
                    format!("inserted concept '{slug}' is absent"),
                )
                .about(slug)
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
                    let mut error = EditError::new(
                        EditCode::Internal,
                        "node.rename",
                        format!("renamed concept '{}' is absent", destination.as_str()),
                    )
                    .about(destination.as_str());
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
        || !Arc::ptr_eq(catalog, context.okf_analysis.markdown.catalog())
        || !Arc::ptr_eq(catalog, context.uml.syntax.catalog())
        || catalog.documents().len() != context.source.len()
    {
        return Err(EditError::new(
            EditCode::StaleContext,
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
        super::Op::EditTransitionTraces { .. } => "transition.traces.edit",
    }
}

pub(crate) fn op_transition_traces_edit(
    work: &mut SourceBundle,
    state: &mut UmlLoweringState,
    selector: &TransitionSelector,
    edit: &TraceEdit,
) -> Result<(), EditError> {
    const OP: &str = "transition.traces.edit";
    let (path, transition) = selected_transition(work, state, selector, OP)?;
    let traces = transition.traces().collect::<Vec<_>>();
    let traces_node = transition
        .syntax()
        .child_at(FlowTransitionSyntax::TRACES_SLOT)
        .and_then(SyntaxElement::into_node)
        .expect("flow transition has a fixed traces occurrence");
    let source = work
        .document(&path)
        .expect("claimed document")
        .text()
        .to_owned();
    match edit {
        TraceEdit::Insert { index, label, href } => {
            if *index > traces.len() {
                return Err(EditError::new(
                    EditCode::OutOfRange,
                    OP,
                    format!("trace index {index} is out of bounds"),
                )
                .about(index.to_string()));
            }
            let clause = render_transition_trace(label, href, OP)?;
            if traces.is_empty() {
                let at = node_range(&traces_node).start;
                replace_range(work, &path, at..at, &format!(" traces {clause}"), OP)
            } else if *index == 0 {
                let at = node_content_range(&source, traces[0].syntax()).start;
                replace_range(
                    work,
                    &path,
                    at..at,
                    &format!("traces {clause}{}  ", line_ending(&source)),
                    OP,
                )
            } else {
                let at = node_content_range(&source, traces[*index - 1].syntax()).end;
                replace_range(
                    work,
                    &path,
                    at..at,
                    &format!("{}  traces {clause}", line_ending(&source)),
                    OP,
                )
            }
        }
        TraceEdit::Update { index, label, href } => {
            let trace = traces.get(*index).ok_or_else(|| {
                EditError::new(
                    EditCode::OutOfRange,
                    OP,
                    format!("trace index {index} is out of bounds"),
                )
                .about(index.to_string())
            })?;
            validate_transition_trace(label, href, OP)?;
            let tokens = trace.link().and_then(|link| {
                let label = link
                    .children()
                    .find(|element| element.kind() == UmlSyntaxKind::LinkTextToken)?
                    .into_token()?;
                let href = link
                    .children()
                    .find(|element| element.kind() == UmlSyntaxKind::LinkTargetToken)?
                    .into_token()?;
                (!label.flags().is_missing() && !href.flags().is_missing()).then_some((label, href))
            });
            if let Some((label_token, href_token)) = tokens {
                let label_range =
                    label_token.range().start().to_usize()..label_token.range().end().to_usize();
                let href_range =
                    href_token.range().start().to_usize()..href_token.range().end().to_usize();
                if source[href_range.clone()] != *href {
                    replace_range(
                        work,
                        &path,
                        href_range,
                        &render_transition_trace_href(href),
                        OP,
                    )?;
                }
                if source[label_range.clone()] != *label {
                    replace_range(
                        work,
                        &path,
                        label_range,
                        &render_transition_trace_label(label),
                        OP,
                    )?;
                }
                Ok(())
            } else {
                let clause = render_transition_trace(label, href, OP)?;
                replace_range(
                    work,
                    &path,
                    node_content_range(&source, trace.syntax()),
                    &format!("traces {clause}"),
                    OP,
                )
            }
        }
        TraceEdit::Remove { index } => {
            if *index >= traces.len() {
                return Err(EditError::new(
                    EditCode::OutOfRange,
                    OP,
                    format!("trace index {index} is out of bounds"),
                )
                .about(index.to_string()));
            }
            let range = if traces.len() == 1 {
                node_range(&traces_node)
            } else if *index == 0 {
                node_range(&traces_node).start..node_range(traces[1].syntax()).start
            } else {
                node_range(traces[*index - 1].syntax()).end..node_range(traces[*index].syntax()).end
            };
            replace_range(work, &path, range, "", OP)
        }
        TraceEdit::Move { from, to } => {
            if *from >= traces.len() || *to >= traces.len() {
                return Err(EditError::new(
                    EditCode::OutOfRange,
                    OP,
                    "trace move index is out of bounds",
                ));
            }
            if from == to {
                return Ok(());
            }
            let mut clauses = traces
                .iter()
                .map(|trace| source[node_content_range(&source, trace.syntax())].to_owned())
                .collect::<Vec<_>>();
            let clause = clauses.remove(*from);
            clauses.insert(*to, clause);
            let collection_range = node_range(&traces_node);
            let collection = &source[collection_range.clone()];
            let mut replacement = String::with_capacity(collection.len());
            let mut cursor = collection_range.start;
            for (trace, clause) in traces.iter().zip(clauses) {
                let range = node_content_range(&source, trace.syntax());
                replacement.push_str(&source[cursor..range.start]);
                replacement.push_str(&clause);
                cursor = range.end;
            }
            replacement.push_str(&source[cursor..collection_range.end]);
            replace_range(work, &path, collection_range, &replacement, OP)
        }
    }
}

fn selected_transition(
    work: &SourceBundle,
    state: &mut UmlLoweringState,
    selector: &TransitionSelector,
    op: &str,
) -> Result<(BundlePath, FlowTransitionSyntax), EditError> {
    let (path, tree) = state.tree(work, &selector.behavior, op)?;
    let flow_node = nodes(&tree, UmlSyntaxKind::FlowNode)
        .into_iter()
        .filter_map(FlowNodeSyntax::cast)
        .find(|node| node.identity_token().text().write_to_string().trim() == selector.source_node)
        .ok_or_else(|| {
            EditError::new(
                EditCode::NotFound,
                op,
                format!(
                    "no flow node '{}' in behavior '{}'",
                    selector.source_node, selector.behavior
                ),
            )
            .about(selector.source_node.as_str())
        })?;
    let mut transitions = Vec::new();
    syntax_nodes(
        flow_node.syntax(),
        UmlSyntaxKind::FlowTransition,
        &mut transitions,
    );
    let transition = transitions
        .into_iter()
        .filter_map(FlowTransitionSyntax::cast)
        .nth(selector.occurrence)
        .ok_or_else(|| {
            EditError::new(
                EditCode::NotFound,
                op,
                format!(
                    "no transition occurrence {} from '{}' in behavior '{}'",
                    selector.occurrence, selector.source_node, selector.behavior
                ),
            )
            .about(selector.source_node.as_str())
        })?;
    Ok((path, transition))
}

fn render_transition_trace(label: &str, href: &str, op: &str) -> Result<String, EditError> {
    validate_transition_trace(label, href, op)?;
    Ok(format!(
        "[{}]({})",
        render_transition_trace_label(label),
        render_transition_trace_href(href)
    ))
}

fn validate_transition_trace(label: &str, href: &str, op: &str) -> Result<(), EditError> {
    if label.trim().is_empty() || href.trim().is_empty() {
        return Err(EditError::new(
            EditCode::InvalidArgument,
            op,
            "trace label and href must not be empty",
        ));
    }
    if label.contains(['\r', '\n']) || href.contains(['\r', '\n']) {
        return Err(EditError::new(
            EditCode::InvalidArgument,
            op,
            "trace label and href must fit on one line",
        ));
    }
    Ok(())
}

fn render_transition_trace_label(label: &str) -> String {
    label.replace('\\', "\\\\").replace(']', "\\]")
}

fn render_transition_trace_href(href: &str) -> String {
    href.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
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
    super::unique_match(work.documents().iter().enumerate(), |(_, document)| {
        slug_of(document.path().as_str()) == target
    })
    .map(|(index, _)| index)
}

pub(crate) fn find_doc(work: &SourceBundle, target: &str, op: &str) -> Result<usize, EditError> {
    resolve_index(work, target).ok_or_else(|| {
        EditError::new(EditCode::NotFound, op, format!("no document '{target}'")).about(target)
    })
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
        .map_err(|error| EditError::wrap(op, &error))?;
    parse_markdown(
        DocumentRevision::INITIAL,
        text,
        MarkdownDialect::WAML_DEFAULT,
    )
    .map(|snapshot| ShellParse {
        tree: snapshot.tree().clone(),
        structure: snapshot.structure().clone(),
    })
    .map_err(|error| EditError::wrap(op, &error))
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

fn token_content_range(token: &SyntaxToken<UmlLanguage>) -> Range<usize> {
    let start = token.range().start().to_usize()
        + token
            .leading_trivia()
            .iter()
            .map(|trivia| trivia.text.write_to_string().len())
            .sum::<usize>();
    start..start + token.text().write_to_string().len()
}

fn node_content_range(source: &str, node: &SyntaxNode<UmlLanguage>) -> Range<usize> {
    let range = node_range(node);
    let authored = &source[range.clone()];
    let leading = authored.len() - authored.trim_start_matches([' ', '\t']).len();
    let trailing = authored.len() - authored.trim_end_matches([' ', '\t']).len();
    range.start + leading..range.end - trailing
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
        .ok_or_else(|| {
            EditError::new(
                EditCode::NotFound,
                op,
                format!("no document '{}'", path.as_str()),
            )
            .about(path.as_str())
        })?;
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
        .ok_or_else(|| {
            EditError::new(
                EditCode::NotFound,
                op,
                format!("no document '{}'", path.as_str()),
            )
            .about(path.as_str())
        })?;
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
        let entry_parent = frontmatter
            .children()
            .filter_map(SyntaxElement::into_node)
            .find(|node| node.kind() == OkfMarkdownSyntaxKind::FrontmatterMapping);
        let entry_source: Box<dyn Iterator<Item = SyntaxElement<_>>> = match &entry_parent {
            Some(mapping) => Box::new(mapping.children()),
            None => Box::new(frontmatter.children()),
        };
        for node in entry_source
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
        || value.contains('\n')
        || value.contains('\r');
    if needs_quote {
        format!(
            "\"{}\"",
            value
                .replace("\r\n", "\n")
                .replace('\r', "\n")
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
        )
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
        .ok_or_else(|| {
            EditError::new(
                EditCode::MalformedDocument,
                op,
                "claimed document has no clean frontmatter",
            )
        })?;
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
        .ok_or_else(|| {
            EditError::new(
                EditCode::MalformedDocument,
                op,
                "claimed document has no title heading",
            )
        })?;
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

fn stored_id(work: &SourceBundle, target: &str) -> String {
    resolve_index(work, target)
        .and_then(|index| work.document_at(index))
        .map(|document| okf::id_of(document.path().as_str()))
        .unwrap_or_else(|| target.to_owned())
}

fn target_href(work: &SourceBundle, referring: &BundlePath, target: &str) -> String {
    resolve_index(work, target)
        .and_then(|index| work.document_at(index))
        .map(|document| okf::relative_href(referring.as_str(), document.path().as_str()))
        .unwrap_or_else(|| {
            if target.starts_with('.') || target.starts_with('/') {
                target.to_owned()
            } else if target.ends_with(".md") {
                format!("./{target}")
            } else {
                format!("./{target}.md")
            }
        })
}

fn type_text(work: &SourceBundle, referring: &BundlePath, token: &str) -> String {
    resolve_index(work, token)
        .and_then(|index| work.document_at(index))
        .map(|document| {
            format!(
                "[{}]({})",
                document_title(work, token, "attr.type"),
                okf::relative_href(referring.as_str(), document.path().as_str())
            )
        })
        .unwrap_or_else(|| token.to_owned())
}

fn attribute_nodes(
    tree: &SyntaxTree<UmlLanguage>,
) -> Vec<(SyntaxNode<UmlLanguage>, AttributeSyntax)> {
    nodes(tree, UmlSyntaxKind::Attribute)
        .into_iter()
        .filter_map(|node| {
            let parsed = AttributeSyntax::cast(node.clone())?;
            Some((node, parsed))
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
    if attribute_nodes(&tree)
        .iter()
        .any(|(_, attribute)| attribute.name_token().text().write_to_string() == name)
    {
        return Err(EditError::new(
            EditCode::AlreadyExists,
            "attr.add",
            format!("attribute '{name}' already exists in {node}"),
        )
        .about(name));
    }
    let visibility = visibility
        .map(|value| format!("{} ", value.marker()))
        .unwrap_or_default();
    let multiplicity = multiplicity
        .as_ref()
        .map(|value| format!(" {{{}}}", value.as_str()))
        .unwrap_or_default();
    let line = format!(
        "- {visibility}{name}: {}{multiplicity}",
        type_text(work, &path, ty_token)
    );
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
    let attributes = attribute_nodes(&tree);
    if let Some(new_name) = rename {
        if new_name != name
            && attributes
                .iter()
                .any(|(_, attribute)| attribute.name_token().text().write_to_string() == *new_name)
        {
            return Err(EditError::new(
                EditCode::AlreadyExists,
                "attr.set",
                format!("attribute '{new_name}' already exists in {node}"),
            )
            .about(new_name));
        }
    }
    let (_syntax, attribute) = attributes
        .into_iter()
        .find(|(_, attribute)| attribute.name_token().text().write_to_string() == name)
        .ok_or_else(|| {
            EditError::new(
                EditCode::NotFound,
                "attr.set",
                format!("no attribute '{name}' in {node}"),
            )
            .about(name)
        })?;
    let source = work.document(&path).expect("claimed document").text();
    let mut edits = Vec::<(Range<usize>, String)>::new();
    if let Some(token) = ty_token {
        let type_syntax = attribute.type_syntax().ok_or_else(|| {
            EditError::new(
                EditCode::MalformedDocument,
                "attr.set",
                "attribute has no type reference",
            )
        })?;
        edits.push((
            node_content_range(source, type_syntax.syntax()),
            type_text(work, &path, token),
        ));
    }
    match multiplicity {
        FieldEdit::Unchanged => {}
        FieldEdit::Clear => {
            if let Some(existing) = attribute.multiplicity() {
                edits.push((node_range(existing.syntax()), String::new()));
            }
        }
        FieldEdit::Set(value) => {
            let replacement = format!("{{{}}}", value.as_str());
            if let Some(existing) = attribute.multiplicity() {
                edits.push((node_content_range(source, existing.syntax()), replacement));
            } else {
                let type_syntax = attribute.type_syntax().ok_or_else(|| {
                    EditError::new(
                        EditCode::MalformedDocument,
                        "attr.set",
                        "attribute has no type reference",
                    )
                })?;
                let end = node_range(type_syntax.syntax()).end;
                edits.push((end..end, format!(" {replacement}")));
            }
        }
    }
    if let Some(value) = visibility {
        let replacement = value.marker().to_string();
        if let Some(existing) = attribute.visibility_token() {
            edits.push((token_content_range(&existing), replacement));
        } else {
            let start = token_content_range(&attribute.name_token()).start;
            edits.push((start..start, format!("{replacement} ")));
        }
    }
    if let Some(value) = rename {
        let token = attribute.name_token();
        edits.push((token_content_range(&token), value.clone()));
    }
    edits.sort_by(|left, right| {
        right
            .0
            .start
            .cmp(&left.0.start)
            .then_with(|| right.0.len().cmp(&left.0.len()))
    });
    for (range, replacement) in edits {
        replace_range(work, &path, range, &replacement, "attr.set")?;
    }
    Ok(())
}

pub(crate) fn op_attr_rm(
    work: &mut SourceBundle,
    state: &mut UmlLoweringState,
    node: &str,
    name: &str,
) -> Result<(), EditError> {
    let (path, tree) = state.tree(work, node, "attr.rm")?;
    let syntax = attribute_nodes(&tree)
        .into_iter()
        .find(|(_, attribute)| attribute.name_token().text().write_to_string() == name)
        .map(|(syntax, _)| syntax)
        .ok_or_else(|| {
            EditError::new(
                EditCode::NotFound,
                "attr.rm",
                format!("no attribute '{name}' in {node}"),
            )
            .about(name)
        })?;
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
        return Err(EditError::new(
            EditCode::AlreadyExists,
            "value.add",
            format!("value '{literal}' already in {node}"),
        )
        .about(literal));
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
        .ok_or_else(|| {
            EditError::new(
                EditCode::NotFound,
                "value.rm",
                format!("no value '{literal}' in {node}"),
            )
            .about(literal)
        })?;
    remove_owned_node(work, &path, &tree, "Values", &syntax, "value.rm")
}

#[derive(Clone)]
struct RelationshipView {
    kind: RelationshipKind,
    target_id: String,
    name: Option<String>,
}

fn relationship_kinds() -> &'static [RelationshipKind] {
    crate::uml::vocabulary::RELATIONSHIP_KINDS
}

fn relationship_view(
    path: &BundlePath,
    source: &str,
    syntax: &RelationshipSyntax,
) -> Option<RelationshipView> {
    let kind_text = syntax.kind_token()?.text().write_to_string();
    let kind = relationship_kinds()
        .iter()
        .copied()
        .find(|kind| kind.as_str() == kind_text)?;
    let href = syntax.target_token()?.text().write_to_string();
    let target_id = okf::resolve_href(path.as_str(), &href);
    let name = syntax
        .name_label_token()
        .map(|token| token.text().write_to_string())
        .or_else(|| {
            syntax
                .name_link()
                .map(|link| source[node_content_range(source, &link)].to_owned())
        });
    Some(RelationshipView {
        kind,
        target_id,
        name,
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

fn render_name(work: &SourceBundle, referring: &BundlePath, name: &NameSpec) -> String {
    match name {
        NameSpec::Label(label) => format!("\"{label}\""),
        NameSpec::Ref(target) => format!(
            "[{}]({})",
            document_title(work, target, "rel.name"),
            target_href(work, referring, target)
        ),
    }
}

fn relationship_matches(relationship: &RelationshipView, by: &RelBy) -> bool {
    match by {
        RelBy::Endpoint { kind, target } => {
            relationship.kind == *kind && relationship.target_id == *target
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
        return Err(EditError::new(
            EditCode::InvalidArgument,
            "rel.add",
            if kind.is_ended() {
                format!("relationship '{}' requires ends", kind.as_str())
            } else {
                format!("relationship '{}' does not take ends", kind.as_str())
            },
        )
        .about(kind.as_str()));
    }
    let (path, tree) = state.tree(work, source_id, "rel.add")?;
    let source = work.document(&path).expect("claimed document").text();
    if nodes(&tree, UmlSyntaxKind::Relationship)
        .iter()
        .filter_map(|syntax| RelationshipSyntax::cast(syntax.clone()))
        .filter_map(|syntax| relationship_view(&path, source, &syntax))
        .any(|relationship| {
            relationship.kind == kind && relationship.target_id == stored_id(work, target)
        })
    {
        return Err(EditError::new(
            EditCode::AlreadyExists,
            "rel.add",
            format!(
                "relationship '{} {target}' already exists in {source_id}",
                kind.as_str()
            ),
        )
        .about(format!("{} {target}", kind.as_str())));
    }
    let name = name
        .as_ref()
        .map(|name| format!(" as {}", render_name(work, &path, name)))
        .unwrap_or_default();
    let ends = ends
        .as_ref()
        .map(|(from, to)| format!(": {} to {}", render_end(from), render_end(to)))
        .unwrap_or_default();
    let line = format!(
        "- {} [{}]({}){name}{ends}",
        kind.as_str(),
        document_title(work, target, "rel.add"),
        target_href(work, &path, target),
    );
    append_line(work, &path, &tree, "Relationships", &line, "rel.add")
}

fn relationship_target<'a>(
    selector: &'a Selector,
    op: &str,
) -> Result<(&'a str, &'a RelBy), EditError> {
    match selector {
        Selector::Rel { source, by } => Ok((source, by)),
        _ => Err(EditError::new(
            EditCode::WrongTarget,
            op,
            format!(
                "selector '{}' does not address a relationship",
                render_selector(selector)
            ),
        )
        .about(render_selector(selector))
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
            target: stored_id(work, target),
        },
        RelBy::Named(name) => RelBy::Named(name.clone()),
    };
    let (path, tree) = state.tree(work, source_id, "rel.set")?;
    let source = work.document(&path).expect("claimed document").text();
    let (syntax, relationship) = nodes(&tree, UmlSyntaxKind::Relationship)
        .into_iter()
        .filter_map(|syntax| {
            let typed = RelationshipSyntax::cast(syntax.clone())?;
            relationship_view(&path, source, &typed).map(|value| (typed, value))
        })
        .find(|(_, relationship)| relationship_matches(relationship, &resolved))
        .ok_or_else(|| {
            let display = render_selector(selector);
            EditError::new(
                EditCode::NotFound,
                "rel.set",
                format!("no relationship '{display}'"),
            )
            .about(display.clone())
            .with_sel(display)
        })?;
    let mut edits = Vec::<(Range<usize>, String)>::new();
    if let Some((from, to)) = ends {
        if !relationship.kind.is_ended() {
            return Err(EditError::new(
                EditCode::InvalidArgument,
                "rel.set",
                format!("'{}' does not take ends", relationship.kind.as_str()),
            )
            .about(relationship.kind.as_str()));
        }
        let from_syntax = syntax.source_end().ok_or_else(|| {
            EditError::new(
                EditCode::MalformedDocument,
                "rel.set",
                "relationship has no source end",
            )
        })?;
        let to_syntax = syntax.to_end().ok_or_else(|| {
            EditError::new(
                EditCode::MalformedDocument,
                "rel.set",
                "relationship has no target end",
            )
        })?;
        edits.push((
            node_content_range(source, from_syntax.syntax()).start
                ..node_content_range(source, to_syntax.syntax()).end,
            format!("{} to {}", render_end(from), render_end(to)),
        ));
    }
    if let Some(name) = name {
        let replacement = render_name(work, &path, name);
        if let Some(existing) = syntax
            .syntax()
            .children()
            .find(|element| element.kind() == UmlSyntaxKind::RelationshipName)
            .and_then(SyntaxElement::into_node)
        {
            edits.push((node_content_range(source, &existing), replacement));
        } else {
            let target = syntax.link().ok_or_else(|| {
                EditError::new(
                    EditCode::MalformedDocument,
                    "rel.set",
                    "relationship has no target link",
                )
            })?;
            let end = node_range(&target).end;
            edits.push((end..end, format!(" as {replacement}")));
        }
    }
    edits.sort_by(|left, right| {
        right
            .0
            .start
            .cmp(&left.0.start)
            .then_with(|| right.0.len().cmp(&left.0.len()))
    });
    for (range, replacement) in edits {
        replace_range(work, &path, range, &replacement, "rel.set")?;
    }
    Ok(())
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
            target: stored_id(work, target),
        },
        RelBy::Named(name) => RelBy::Named(name.clone()),
    };
    let (path, tree) = state.tree(work, source_id, "rel.rm")?;
    let source = work.document(&path).expect("claimed document").text();
    let syntax = nodes(&tree, UmlSyntaxKind::Relationship)
        .into_iter()
        .find(|syntax| {
            RelationshipSyntax::cast(syntax.clone())
                .and_then(|typed| relationship_view(&path, source, &typed))
                .is_some_and(|relationship| relationship_matches(&relationship, &resolved))
        })
        .ok_or_else(|| {
            let display = render_selector(selector);
            EditError::new(
                EditCode::NotFound,
                "rel.rm",
                format!("no relationship '{display}'"),
            )
            .about(display.clone())
            .with_sel(display)
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
        return Err(EditError::new(
            EditCode::AlreadyExists,
            "node.new",
            format!("document '{slug}' already exists"),
        )
        .about(slug));
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
        .map_err(|error| EditError::wrap("node.new", &error))
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

/// Whether one `## Layout` statement is the placement of `subject` against
/// `reference`.
///
/// `subject` and `reference` are *hrefs* as [`target_href`] renders them, NOT
/// raw slugs. That distinction is the whole point: this used to build its own
/// needle as `./{slug}.md`, while the writer emits a href resolved relative to
/// the referring document. For a bare, same-directory slug the two agree by
/// accident; for a qualified slug (`shop/order`) or any cross-directory
/// reference they never do, so `place.set` never found the line it was meant
/// to replace and appended a duplicate on every call, and `place.rm` silently
/// removed nothing. Resolve once at the call site and both operations compare
/// the same string the writer wrote.
fn placement_matches(source: &str, subject: &str, reference: &str) -> bool {
    source.contains(subject) && source.contains(reference)
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
    let subject_href = target_href(work, &path, subject_slug);
    let reference_href = target_href(work, &path, reference_slug);
    let mut ranges: Vec<_> = nodes(&tree, UmlSyntaxKind::LayoutStatement)
        .into_iter()
        .map(|syntax| node_range(&syntax))
        .filter(|range| placement_matches(&source[range.clone()], &subject_href, &reference_href))
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
                "- [{subject_title}]({subject_href}) {} [{reference_title}]({reference_href})",
                direction_text(*direction),
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
    let subject_href = target_href(work, &path, subject_slug);
    let reference_href = target_href(work, &path, reference_slug);
    let mut matches: Vec<_> = nodes(&tree, UmlSyntaxKind::LayoutStatement)
        .into_iter()
        .filter(|syntax| {
            placement_matches(&source[node_range(syntax)], &subject_href, &reference_href)
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
    let path = state.path(id).cloned().ok_or_else(|| {
        EditError::new(EditCode::NotFound, "node.rm", format!("no document '{id}'")).about(id)
    })?;
    if !cascade {
        let references = referrers_source(work, id);
        if !references.is_empty() {
            return Err(EditError::new(
                EditCode::ReferencedElsewhere,
                "node.rm",
                format!(
                    "'{id}' referenced by: {} (use --cascade)",
                    references.join(", ")
                ),
            )
            .about(id));
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

#[cfg(test)]
pub(crate) fn referrers(work: &[(String, String)], slug: &str) -> Vec<String> {
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
        let tree = super::syntax::parse_full(text, &parsed.structure);
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
