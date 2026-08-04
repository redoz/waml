use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::edit::{EditContext, EditError};
use crate::index_md::{render_index, render_members, IndexEntry};
use crate::source::{BundlePath, SourceBundle};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, OkfMarkdownSyntaxKind, ShellParse,
    SourceText, SyntaxElement,
};

pub(crate) struct OkfLoweringCursor<'a> {
    original: EditContext<'a>,
    candidate: SourceBundle,
    state: OkfLoweringState,
}

pub(crate) struct OkfLoweringState {
    touched_shell: BTreeMap<BundlePath, ShellParse>,
    structural_paths: BTreeSet<BundlePath>,
}

impl OkfLoweringState {
    pub(crate) fn from_source(source: &SourceBundle) -> Self {
        Self {
            touched_shell: BTreeMap::new(),
            structural_paths: source
                .documents()
                .iter()
                .map(|document| document.path().clone())
                .collect(),
        }
    }

    pub(crate) fn from_context(context: &EditContext<'_>) -> Self {
        Self::from_source(context.source)
    }

    pub(crate) fn invalidate_text(&mut self, path: &BundlePath) {
        self.touched_shell.remove(path);
    }

    pub(crate) fn inserted(&mut self, path: BundlePath) -> Result<(), EditError> {
        if !self.structural_paths.insert(path.clone()) {
            return Err(EditError::at(
                "okf.structure",
                format!("'{}' already exists", path.as_str()),
            ));
        }
        self.touched_shell.remove(&path);
        Ok(())
    }

    pub(crate) fn removed(&mut self, path: &BundlePath) {
        self.structural_paths.remove(path);
        self.touched_shell.remove(path);
    }

    pub(crate) fn renamed(&mut self, from: &BundlePath, to: BundlePath) -> Result<(), EditError> {
        if from != &to && self.structural_paths.contains(&to) {
            return Err(EditError::at(
                "okf.structure",
                format!("'{}' already exists", to.as_str()),
            ));
        }
        if !self.structural_paths.remove(from) {
            return Err(EditError::at(
                "okf.structure",
                format!("no document '{}'", from.as_str()),
            ));
        }
        self.structural_paths.insert(to.clone());
        self.touched_shell.remove(from);
        self.touched_shell.remove(&to);
        Ok(())
    }
}

impl crate::edit::InvalidationSink for OkfLoweringState {
    fn absorb(&mut self, event: &crate::edit::Invalidation) -> Result<(), EditError> {
        use crate::edit::Invalidation;
        match event {
            Invalidation::TextChanged(path) => self.invalidate_text(path),
            Invalidation::Inserted { path, .. } => self.inserted(path.clone())?,
            Invalidation::Removed { path, .. } => self.removed(path),
            Invalidation::Renamed { from, to, .. } => self.renamed(from, to.clone())?,
        }
        Ok(())
    }
}

impl OkfLoweringState {
    fn shell<'a>(
        &'a mut self,
        candidate: &SourceBundle,
        path: &BundlePath,
        op: &str,
    ) -> Result<&'a ShellParse, EditError> {
        if !self.touched_shell.contains_key(path) {
            let document = candidate
                .document(path)
                .ok_or_else(|| EditError::at(op, format!("no document '{}'", path.as_str())))?;
            let text = SourceText::from_shared(document.text_arc().clone())
                .map_err(|error| EditError::at(op, error.to_string()))?;
            let snapshot = parse_markdown(
                DocumentRevision::INITIAL,
                text,
                MarkdownDialect::WAML_DEFAULT,
            )
            .map_err(|error| EditError::at(op, error.to_string()))?;
            let parsed = ShellParse {
                tree: snapshot.tree().clone(),
                structure: snapshot.structure().clone(),
            };
            self.touched_shell.insert(path.clone(), parsed);
        }
        Ok(self
            .touched_shell
            .get(path)
            .expect("shell parse inserted for candidate path"))
    }

    fn reparse(
        &mut self,
        candidate: &SourceBundle,
        path: &BundlePath,
        op: &str,
    ) -> Result<(), EditError> {
        self.invalidate_text(path);
        self.shell(candidate, path, op).map(|_| ())
    }
}

impl<'a> OkfLoweringCursor<'a> {
    pub(crate) fn new(context: EditContext<'a>) -> Self {
        let candidate = context.source.clone();
        let state = OkfLoweringState::from_context(&context);
        Self {
            original: context,
            candidate,
            state,
        }
    }

    pub(crate) fn apply(&mut self, index: usize, op: &crate::okf::Op) -> Result<(), EditError> {
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
    state: &mut OkfLoweringState,
    index: usize,
    op: &crate::okf::Op,
) -> Result<(), EditError> {
    let before: BTreeSet<_> = candidate
        .documents()
        .iter()
        .map(|document| document.path().clone())
        .collect();
    crate::okf::ops::lower_one_with_state(candidate, state, op).map_err(|mut error| {
        error.index = index;
        error
    })?;
    let after: BTreeSet<_> = candidate
        .documents()
        .iter()
        .map(|document| document.path().clone())
        .collect();
    let removed: Vec<_> = before.difference(&after).cloned().collect();
    let inserted: Vec<_> = after.difference(&before).cloned().collect();
    if !removed.is_empty() && removed.len() == inserted.len() {
        for (from, to) in removed.iter().zip(inserted) {
            state.renamed(from, to).map_err(|mut error| {
                error.index = index;
                error
            })?;
        }
    } else {
        for path in &removed {
            state.removed(path);
        }
        for path in inserted {
            state.inserted(path).map_err(|mut error| {
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
    {
        return Err(EditError::at(
            "okf.context",
            "analysis revision does not match the requested session revision",
        ));
    }
    if !Arc::ptr_eq(catalog, context.okf_analysis.markdown.catalog())
        || !Arc::ptr_eq(catalog, context.uml.syntax.catalog())
        || catalog.documents().len() != context.source.len()
    {
        return Err(EditError::at(
            "okf.context",
            "analysis catalog does not match the source bundle",
        ));
    }
    for document in catalog.documents().values() {
        let source = context
            .source
            .document(document.path())
            .ok_or_else(|| EditError::at("okf.context", "catalog path is absent from source"))?;
        if !Arc::ptr_eq(document.text().shared(), source.text_arc()) {
            return Err(EditError::at(
                "okf.context",
                "catalog text does not match source identity",
            ));
        }
    }
    Ok(())
}

fn slug_of(path: &str) -> String {
    let segment = path.rsplit(['/', '\\']).next().unwrap_or(path);
    segment.strip_suffix(".md").unwrap_or(segment).to_string()
}

fn resolve_index(work: &SourceBundle, target: &str) -> Option<usize> {
    if let Some(index) = work
        .documents()
        .iter()
        .position(|document| crate::okf::id_of(document.path().as_str()) == target)
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

fn find_doc(work: &SourceBundle, target: &str, op: &str) -> Result<usize, EditError> {
    resolve_index(work, target).ok_or_else(|| EditError::at(op, format!("no document '{target}'")))
}

fn join(dir: &str, slug: &str) -> String {
    if dir.is_empty() {
        format!("{slug}.md")
    } else {
        format!("{dir}/{slug}.md")
    }
}

/// Move a concept/diagram doc to another package directory, keeping its
/// basename (key). Slug-based references are unaffected. Errors if the doc is
/// missing or a same-key doc already lives in `to_dir`.
pub(crate) fn op_pkg_move(
    work: &mut SourceBundle,
    slug: &str,
    to_dir: &str,
) -> Result<(), EditError> {
    let idx = find_doc(work, slug, "pkg.move")?;
    let dest = join(to_dir, &slug_of(slug));
    if work
        .documents()
        .iter()
        .enumerate()
        .any(|(i, document)| i != idx && document.path().as_str() == dest)
    {
        return Err(EditError::at(
            "pkg.move",
            format!("'{dest}' already exists"),
        ));
    }
    work.rename_document(idx, dest)
        .map_err(|error| EditError::at("pkg.move", error.to_string()))?;
    Ok(())
}

/// Rename a package directory: rewrite the `from/` path prefix of every doc
/// under it to `to/`. Slugs (keys) and slug-based references are unchanged.
/// Errors if `to` already exists as a directory prefix or `from` is empty/absent.
pub(crate) fn op_pkg_rename(
    work: &mut SourceBundle,
    from: &str,
    to: &str,
) -> Result<(), EditError> {
    if from.is_empty() {
        return Err(EditError::at(
            "pkg.rename",
            "cannot rename the root package",
        ));
    }
    let from_pfx = format!("{from}/");
    let to_pfx = format!("{to}/");
    if work
        .documents()
        .iter()
        .any(|document| document.path().as_str().starts_with(&to_pfx))
    {
        return Err(EditError::at(
            "pkg.rename",
            format!("directory '{to}' already exists"),
        ));
    }
    let renames: Vec<_> = work
        .documents()
        .iter()
        .enumerate()
        .filter_map(|(index, document)| {
            document
                .path()
                .as_str()
                .strip_prefix(&from_pfx)
                .map(|rest| (index, format!("{to_pfx}{rest}")))
        })
        .collect();
    if renames.is_empty() {
        return Err(EditError::at("pkg.rename", format!("no package '{from}'")));
    }
    for (index, path) in renames {
        work.rename_document(index, path)
            .map_err(|error| EditError::at("pkg.rename", error.to_string()))?;
    }
    Ok(())
}

fn parent_of(dir: &str) -> String {
    match dir.rfind('/') {
        Some(i) => dir[..i].to_string(),
        None => String::new(),
    }
}

/// Delete a package directory. `cascade=true` removes every doc under `path/`
/// (incl. its `index.md`). `cascade=false` = move-to-parent: strip the deleted
/// segment from every child path so children reparent one level up. Root cannot
/// be deleted.
pub(crate) fn op_pkg_delete(
    work: &mut SourceBundle,
    path: &str,
    cascade: bool,
) -> Result<(), EditError> {
    if path.is_empty() {
        return Err(EditError::at(
            "pkg.delete",
            "cannot delete the root package",
        ));
    }
    let pfx = format!("{path}/");
    if cascade {
        let before = work.len();
        work.retain_documents(|document| !document.path().as_str().starts_with(&pfx));
        if work.len() == before {
            return Err(EditError::at("pkg.delete", format!("no package '{path}'")));
        }
    } else {
        let parent = parent_of(path);
        let parent_pfx = if parent.is_empty() {
            String::new()
        } else {
            format!("{parent}/")
        };
        let renames: Vec<_> = work
            .documents()
            .iter()
            .enumerate()
            .filter_map(|(index, document)| {
                document
                    .path()
                    .as_str()
                    .strip_prefix(&pfx)
                    .map(|rest| (index, format!("{parent_pfx}{rest}")))
            })
            .collect();
        for (index, path) in renames {
            work.rename_document(index, path)
                .map_err(|error| EditError::at("pkg.delete", error.to_string()))?;
        }
    }
    Ok(())
}

/// Title/description now live on `concept` (single source). Look up a member's
/// display title across nodes, diagrams, and sub-packages.
/// How a rewritten index.md orders its members. `Sort` = A–Z by title; `Explicit`
/// = a caller-supplied order (unknown keys ignored, missing keys appended).
enum MemberOrder<'a> {
    Explicit(&'a [String]),
    Sort,
    /// Keep the package's current (reconciled) member order — used by retitle,
    /// which must not reshuffle the listing.
    Keep,
}

fn package_entries(
    work: &SourceBundle,
    state: &mut OkfLoweringState,
    directory: &str,
) -> Result<Vec<IndexEntry>, EditError> {
    let prefix = if directory.is_empty() {
        String::new()
    } else {
        format!("{directory}/")
    };
    let mut child_directories = BTreeSet::new();
    let mut concept_paths = Vec::new();
    for path in &state.structural_paths {
        let Some(relative) = path.as_str().strip_prefix(&prefix) else {
            continue;
        };
        if let Some((child, _)) = relative.split_once('/') {
            child_directories.insert(child.to_owned());
            continue;
        }
        if matches!(relative, "index.md" | "log.md") {
            continue;
        }
        concept_paths.push(path.clone());
    }

    let mut entries = Vec::new();
    for child in child_directories {
        let child_directory = if directory.is_empty() {
            child
        } else {
            format!("{directory}/{child}")
        };
        let index_path = BundlePath::parse(format!("{child_directory}/index.md"))
            .map_err(|error| EditError::at("pkg.index", error.to_string()))?;
        let title = if work.document(&index_path).is_some() {
            document_title(work, state, &index_path, "pkg.index")?
                .unwrap_or_else(|| child_directory.rsplit('/').next().unwrap().to_owned())
        } else {
            child_directory.rsplit('/').next().unwrap().to_owned()
        };
        entries.push(IndexEntry {
            key: format!("/{child_directory}"),
            title,
            blurb: None,
            is_package: true,
        });
    }
    for path in concept_paths {
        let key = path
            .as_str()
            .strip_suffix(".md")
            .expect("structural Markdown path has md suffix")
            .to_owned();
        let title = document_title(work, state, &path, "pkg.index")?
            .unwrap_or_else(|| slug_of(path.as_str()));
        let blurb = frontmatter_value(work, state, &path, "description", "pkg.index")?
            .map(|description| description.lines().next().unwrap_or("").to_owned());
        entries.push(IndexEntry {
            key,
            title,
            blurb,
            is_package: false,
        });
    }
    entries.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(entries)
}

fn document_title(
    work: &SourceBundle,
    state: &mut OkfLoweringState,
    path: &BundlePath,
    op: &str,
) -> Result<Option<String>, EditError> {
    if let Some(title) = frontmatter_value(work, state, path, "title", op)? {
        return Ok(Some(title));
    }
    let document = work
        .document(path)
        .ok_or_else(|| EditError::at(op, format!("no document '{}'", path.as_str())))?;
    let shell = state.shell(work, path, op)?;
    Ok(shell
        .structure
        .headings
        .iter()
        .find(|heading| heading.level == 1)
        .map(|heading| {
            document.text()
                [heading.text_range.start().to_usize()..heading.text_range.end().to_usize()]
                .trim()
                .to_owned()
        })
        .filter(|title| !title.is_empty()))
}

fn frontmatter_value(
    work: &SourceBundle,
    state: &mut OkfLoweringState,
    path: &BundlePath,
    wanted: &str,
    op: &str,
) -> Result<Option<String>, EditError> {
    let shell = state.shell(work, path, op)?;
    for child in shell
        .tree
        .root()
        .children()
        .filter_map(SyntaxElement::into_node)
    {
        if child.kind() != OkfMarkdownSyntaxKind::Frontmatter {
            continue;
        }
        let closed = child.children().any(|element| {
            element.into_token().is_some_and(|token| {
                token.kind() == OkfMarkdownSyntaxKind::FrontmatterCloseFence
                    && !token.flags().is_missing()
            })
        });
        if !closed {
            return Ok(None);
        }
        for entry in child.children().filter_map(SyntaxElement::into_node) {
            if entry.kind() != OkfMarkdownSyntaxKind::FrontmatterEntry {
                continue;
            }
            let mut key = None;
            let mut value = None;
            for token in entry.children().filter_map(SyntaxElement::into_token) {
                match token.kind() {
                    OkfMarkdownSyntaxKind::FrontmatterKey => {
                        key = Some(token.text().write_to_string())
                    }
                    OkfMarkdownSyntaxKind::FrontmatterValue if !token.flags().is_missing() => {
                        value = Some(token.text().write_to_string())
                    }
                    _ => {}
                }
            }
            if key.as_deref().is_some_and(|key| key.trim() == wanted) {
                return Ok(value.and_then(|value| {
                    match crate::frontmatter::parse_value(value.trim()) {
                        crate::frontmatter::FmValue::Str(value) => Some(value),
                        _ => None,
                    }
                }));
            }
        }
    }
    Ok(None)
}

fn authored_member_order(
    work: &SourceBundle,
    state: &mut OkfLoweringState,
    directory: &str,
    valid: &[String],
) -> Result<Vec<String>, EditError> {
    let path = BundlePath::parse(if directory.is_empty() {
        "index.md".to_owned()
    } else {
        format!("{directory}/index.md")
    })
    .map_err(|error| EditError::at("pkg.index", error.to_string()))?;
    let Some(document) = work.document(&path) else {
        return Ok(Vec::new());
    };
    let (ranges, _) = confirmed_member_block(work, state, &path, directory, valid)?;
    let mut ordered = Vec::new();
    for range in ranges {
        let line = &document.text()[range.clone()];
        let Some(key) = member_key(directory, line) else {
            continue;
        };
        if !ordered.contains(&key) {
            ordered.push(key);
        }
    }
    Ok(ordered)
}

fn confirmed_member_block(
    work: &SourceBundle,
    state: &mut OkfLoweringState,
    path: &BundlePath,
    directory: &str,
    valid: &[String],
) -> Result<(Vec<std::ops::Range<usize>>, usize), EditError> {
    let document = work
        .document(path)
        .ok_or_else(|| EditError::at("pkg.index", format!("no document '{}'", path.as_str())))?;
    let source = document.text();
    let shell = state.shell(work, path, "pkg.index")?;
    let first_h1 = shell
        .structure
        .headings
        .iter()
        .find(|heading| heading.level == 1)
        .map(|heading| heading.range.start());
    let preamble_end = shell
        .structure
        .headings
        .iter()
        .chain(shell.structure.nested_headings.iter())
        .filter(|heading| first_h1.map_or(true, |h1| heading.range.start() > h1))
        .map(|heading| heading.range.start())
        .min()
        .map(|start| start.to_usize())
        .unwrap_or(source.len());
    let ranges: Vec<_> = shell
        .structure
        .list_item_lines
        .iter()
        .filter(|range| range.start().to_usize() < preamble_end)
        .map(|range| range.start().to_usize()..range.end().to_usize())
        .collect();

    let mut blocks: Vec<Vec<std::ops::Range<usize>>> = Vec::new();
    for range in ranges {
        let continues = blocks
            .last()
            .and_then(|block| block.last())
            .is_some_and(|previous| source[previous.end..range.start].trim().is_empty());
        if continues {
            blocks
                .last_mut()
                .expect("continued block exists")
                .push(range);
        } else {
            blocks.push(vec![range]);
        }
    }
    let mut confirmed = blocks.into_iter().filter(|block| {
        block.iter().all(|range| {
            member_key(directory, &source[range.clone()]).is_some_and(|key| valid.contains(&key))
        })
    });
    let selected = confirmed.next().unwrap_or_default();
    if confirmed.next().is_some() {
        return Err(EditError::at(
            "pkg.index",
            "multiple shell-confirmed member-list blocks",
        ));
    }
    Ok((selected, preamble_end))
}

fn member_key(directory: &str, line: &str) -> Option<String> {
    let href = markdown_href(line)?;
    if href.ends_with('/') {
        let child = href.trim_end_matches('/').trim_start_matches("./");
        Some(if directory.is_empty() {
            format!("/{child}")
        } else {
            format!("/{directory}/{child}")
        })
    } else {
        let relative = href.trim_start_matches("./").trim_end_matches(".md");
        Some(if directory.is_empty() {
            relative.to_owned()
        } else {
            format!("{directory}/{relative}")
        })
    }
}

fn markdown_href(line: &str) -> Option<&str> {
    let start = line.find("](")? + 2;
    let end = line[start..].find(')')? + start;
    Some(&line[start..end])
}

fn update_authored_index(
    work: &mut SourceBundle,
    state: &mut OkfLoweringState,
    index_path: &BundlePath,
    directory: &str,
    title_override: Option<&str>,
    entries: &[IndexEntry],
) -> Result<(), EditError> {
    let document = work
        .document(index_path)
        .ok_or_else(|| EditError::at("pkg.index", "index disappeared during lowering"))?;
    let source = document.text();
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let valid: Vec<_> = entries.iter().map(|entry| entry.key.clone()).collect();
    let (member_ranges, preamble_end) =
        confirmed_member_block(work, state, index_path, directory, &valid)?;
    let shell = state.shell(work, index_path, "pkg.index")?;
    let h1 = shell
        .structure
        .headings
        .iter()
        .find(|heading| heading.level == 1)
        .map(|heading| {
            let start = heading.text_range.start().to_usize();
            let mut end = heading.text_range.end().to_usize();
            while end > start && source.as_bytes()[end - 1].is_ascii_whitespace() {
                end -= 1;
            }
            start..end
        });
    let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    if let Some(title) = title_override {
        match h1 {
            Some(range) => edits.push((range, title.to_owned())),
            None => edits.push((0..0, format!("# {title}{newline}{newline}"))),
        }
    }
    let listing = render_members(directory, entries, newline);
    if let (Some(first), Some(last)) = (member_ranges.first(), member_ranges.last()) {
        edits.push((first.start..last.end, listing));
    } else if !listing.is_empty() {
        let prefix = &source[..preamble_end];
        let separator = if prefix.is_empty() || prefix.ends_with("\n\n") {
            ""
        } else if prefix.ends_with('\n') {
            newline
        } else {
            if newline == "\r\n" {
                "\r\n\r\n"
            } else {
                "\n\n"
            }
        };
        let suffix = if preamble_end < source.len() {
            newline
        } else {
            ""
        };
        edits.push((
            preamble_end..preamble_end,
            format!("{separator}{listing}{suffix}"),
        ));
    }
    edits.sort_by_key(|(range, _)| (range.start, range.end));
    let text = work
        .document_mut(index_path)
        .expect("authored index remains in candidate")
        .text_mut();
    for (range, replacement) in edits.into_iter().rev() {
        text.replace_range(range, &replacement);
    }
    state.reparse(work, index_path, "pkg.index")
}

/// Write/replace `<path>/index.md` (root → `index.md`) with a listing in the
/// requested order, preserving intro prose + blurbs. The H1 title comes from
/// `title_override` when set, else the package's current title (root →
/// `model.path`, else `concept.title`), else the dir basename.
fn write_package_index(
    work: &mut SourceBundle,
    state: &mut OkfLoweringState,
    path: &str,
    order: MemberOrder<'_>,
    title_override: Option<&str>,
) -> Result<(), EditError> {
    let prefix = if path.is_empty() {
        String::new()
    } else {
        format!("{path}/")
    };
    if !path.is_empty()
        && !state
            .structural_paths
            .iter()
            .any(|candidate| candidate.as_str().starts_with(&prefix))
    {
        return Err(EditError::at("pkg.index", format!("no package '{path}'")));
    }
    let mut entries = package_entries(work, state, path)?;
    let mut keys: Vec<String> = entries.iter().map(|entry| entry.key.clone()).collect();
    keys.sort_by(|left, right| {
        let title = |key: &String| {
            entries
                .iter()
                .find(|entry| &entry.key == key)
                .map(|entry| entry.title.to_lowercase())
                .unwrap_or_default()
        };
        title(left).cmp(&title(right)).then(left.cmp(right))
    });
    keys = match order {
        MemberOrder::Explicit(o) => {
            let mut v: Vec<String> = o
                .iter()
                .filter_map(|key| {
                    if keys.contains(key) {
                        Some(key.clone())
                    } else {
                        let rooted = format!("/{key}");
                        keys.contains(&rooted).then_some(rooted)
                    }
                })
                .collect();
            for m in &keys {
                if !v.contains(m) {
                    v.push(m.clone());
                }
            }
            v
        }
        MemberOrder::Sort => {
            let mut v = keys;
            v.sort_by_key(|key| {
                entries
                    .iter()
                    .find(|entry| &entry.key == key)
                    .map(|entry| entry.title.to_lowercase())
                    .unwrap_or_default()
            });
            v
        }
        MemberOrder::Keep => {
            let authored = authored_member_order(work, state, path, &keys)?;
            authored
                .into_iter()
                .chain(keys)
                .fold(Vec::new(), |mut ordered, key| {
                    if !ordered.contains(&key) {
                        ordered.push(key);
                    }
                    ordered
                })
        }
    };
    entries.sort_by_key(|entry| {
        keys.iter()
            .position(|key| key == &entry.key)
            .unwrap_or(usize::MAX)
    });
    // Root special-case is ONLY the index-file path arithmetic.
    let idx_path = if path.is_empty() {
        "index.md".to_string()
    } else {
        format!("{path}/index.md")
    };
    let idx_path = BundlePath::parse(idx_path)
        .map_err(|error| EditError::at("pkg.index", error.to_string()))?;
    if work.document(&idx_path).is_some() {
        update_authored_index(work, state, &idx_path, path, title_override, &entries)?;
    } else {
        let title = title_override
            .or_else(|| (!path.is_empty()).then_some(path.rsplit('/').next().unwrap_or(path)));
        work.upsert(idx_path.clone(), render_index(path, title, None, &entries));
        state.reparse(work, &idx_path, "pkg.index")?;
    }
    Ok(())
}

pub(crate) fn op_pkg_reorder(
    work: &mut SourceBundle,
    state: &mut OkfLoweringState,
    path: &str,
    order: &[String],
) -> Result<(), EditError> {
    write_package_index(work, state, path, MemberOrder::Explicit(order), None)
}
pub(crate) fn op_pkg_sort(
    work: &mut SourceBundle,
    state: &mut OkfLoweringState,
    path: &str,
) -> Result<(), EditError> {
    write_package_index(work, state, path, MemberOrder::Sort, None)
}

/// Set a package's display title by writing its index.md H1, creating the file
/// (root → `index.md`, else `<path>/index.md`) when absent. Preserves the intro
/// prose and member listing. Empty/whitespace titles are rejected. Generic over
/// any package key; root ("") is just one instance.
pub(crate) fn op_pkg_retitle(
    work: &mut SourceBundle,
    state: &mut OkfLoweringState,
    path: &str,
    title: &str,
) -> Result<(), EditError> {
    if title.trim().is_empty() {
        return Err(EditError::at("pkg.retitle", "title cannot be empty"));
    }
    write_package_index(work, state, path, MemberOrder::Keep, Some(title))
}

/// Insert a package: re-root every doc in `docs` under `<parent_path>/<name>/`
/// (or `<name>/` at root) and append. The incoming top-level folder segment is
/// stripped so a template's baked folder is replaced by the target prefix;
/// `./`-relative links stay valid untouched. Identity is the full path, so
/// distinct same-basename docs across packages coexist. Errors if the target
/// package path already exists or `name` is empty.
pub(crate) fn op_pkg_insert(
    work: &mut SourceBundle,
    parent_path: &str,
    name: &str,
    docs: &[(String, String)],
) -> Result<(), EditError> {
    if name.is_empty() {
        return Err(EditError::at("pkg.insert", "package name is required"));
    }
    let prefix = if parent_path.is_empty() {
        format!("{name}/")
    } else {
        format!("{parent_path}/{name}/")
    };
    if work
        .documents()
        .iter()
        .any(|document| document.path().as_str().starts_with(&prefix))
    {
        return Err(EditError::at(
            "pkg.insert",
            format!("package '{}' already exists", prefix.trim_end_matches('/')),
        ));
    }
    for (path, text) in docs {
        let norm = path.replace('\\', "/");
        // strip the incoming top-level folder segment (if any)
        let rest = match norm.split_once('/') {
            Some((_, r)) => r,
            None => norm.as_str(),
        };
        work.push_document(format!("{prefix}{rest}"), text.clone())
            .map_err(|error| EditError::at("pkg.insert", error.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::ops::{apply, Op};
    #[test]
    fn move_changes_directory_keeps_basename() {
        let b = vec![(
            "sales/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        )];
        let out = apply(
            &b,
            &[Op::PkgMove {
                slug: "order".into(),
                to_dir: "billing".into(),
            }],
        )
        .unwrap();
        assert!(out.iter().any(|(p, _)| p == "billing/order.md"));
        assert!(out.iter().all(|(p, _)| p != "sales/order.md"));
    }
    #[test]
    fn move_to_root_uses_bare_filename() {
        let b = vec![(
            "sales/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        )];
        let out = apply(
            &b,
            &[Op::PkgMove {
                slug: "order".into(),
                to_dir: "".into(),
            }],
        )
        .unwrap();
        assert!(out.iter().any(|(p, _)| p == "order.md"));
    }

    #[test]
    fn rename_package_rewrites_child_paths_only() {
        let b = vec![
            ("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Customer](./customer.md)\n".to_string()),
            ("sales/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
        ];
        let out = apply(
            &b,
            &[Op::PkgRename {
                from: "sales".into(),
                to: "commerce".into(),
            }],
        )
        .unwrap();
        assert!(out.iter().any(|(p, _)| p == "commerce/order.md"));
        assert!(out.iter().any(|(p, _)| p == "commerce/customer.md"));
        assert!(out.iter().all(|(p, _)| !p.starts_with("sales/")));
        // slug-based references untouched
        let order = &out
            .iter()
            .find(|(p, _)| p == "commerce/order.md")
            .unwrap()
            .1;
        assert!(order.contains("(./customer.md)"));
    }

    #[test]
    fn delete_package_cascade_removes_subtree() {
        let b = vec![
            (
                "sales/order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            ),
            (
                "sales/orders/line.md".to_string(),
                "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n".to_string(),
            ),
            (
                "billing/invoice.md".to_string(),
                "---\ntype: uml.Class\ntitle: Invoice\n---\n# Invoice\n".to_string(),
            ),
        ];
        let out = apply(
            &b,
            &[Op::PkgDelete {
                path: "sales".into(),
                cascade: true,
            }],
        )
        .unwrap();
        assert!(out.iter().all(|(p, _)| !p.starts_with("sales")));
        assert!(out.iter().any(|(p, _)| p == "billing/invoice.md"));
    }
    #[test]
    fn delete_package_reparent_moves_children_up() {
        let b = vec![(
            "sales/orders/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        )];
        let out = apply(
            &b,
            &[Op::PkgDelete {
                path: "sales/orders".into(),
                cascade: false,
            }],
        )
        .unwrap();
        assert!(out.iter().any(|(p, _)| p == "sales/order.md"));
        assert!(out.iter().all(|(p, _)| !p.contains("orders")));
    }

    #[test]
    fn reorder_writes_index_md_in_requested_order() {
        let b = vec![
            (
                "sales/order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            ),
            (
                "sales/customer.md".to_string(),
                "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string(),
            ),
        ];
        let out = apply(
            &b,
            &[Op::PkgReorder {
                path: "sales".into(),
                order: vec!["sales/order".into(), "sales/customer".into()],
            }],
        )
        .unwrap();
        let idx = &out.iter().find(|(p, _)| p == "sales/index.md").unwrap().1;
        let oi = idx.find("order.md").unwrap();
        let ci = idx.find("customer.md").unwrap();
        assert!(oi < ci, "order must precede customer in index.md");
    }
    #[test]
    fn sort_writes_index_md_alphabetically() {
        let b = vec![
            (
                "sales/order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            ),
            (
                "sales/customer.md".to_string(),
                "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string(),
            ),
        ];
        let out = apply(
            &b,
            &[Op::PkgSort {
                path: "sales".into(),
            }],
        )
        .unwrap();
        let idx = &out.iter().find(|(p, _)| p == "sales/index.md").unwrap().1;
        assert!(idx.find("customer.md").unwrap() < idx.find("order.md").unwrap());
    }

    #[test]
    fn retitle_creates_root_index_when_absent() {
        let b = vec![(
            "order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        )];
        let out = apply(
            &b,
            &[Op::PkgRetitle {
                path: "".into(),
                title: "Acme".into(),
            }],
        )
        .unwrap();
        let idx = &out
            .iter()
            .find(|(p, _)| p == "index.md")
            .expect("root index.md created")
            .1;
        assert!(idx.starts_with("# Acme\n"), "root H1: {idx}");
        assert!(
            idx.contains("./order.md"),
            "member listing preserved: {idx}"
        );
    }

    #[test]
    fn retitle_preserves_intro_and_members_for_a_nested_package() {
        let b = vec![
            (
                "sales/index.md".to_string(),
                "# Old\n\nIntro prose.\n\n* [order](./order.md)\n".to_string(),
            ),
            (
                "sales/order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            ),
            (
                "sales/customer.md".to_string(),
                "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string(),
            ),
        ];
        let out = apply(
            &b,
            &[Op::PkgRetitle {
                path: "sales".into(),
                title: "Sales Domain".into(),
            }],
        )
        .unwrap();
        let idx = &out.iter().find(|(p, _)| p == "sales/index.md").unwrap().1;
        assert!(idx.starts_with("# Sales Domain\n"), "new H1: {idx}");
        assert!(idx.contains("Intro prose."), "intro preserved: {idx}");
        assert!(
            idx.contains("./order.md") && idx.contains("./customer.md"),
            "members preserved: {idx}"
        );
    }

    #[test]
    fn retitle_rejects_an_empty_title() {
        let b = vec![(
            "order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        )];
        let err = apply(
            &b,
            &[Op::PkgRetitle {
                path: "".into(),
                title: "   ".into(),
            }],
        )
        .unwrap_err();
        assert_eq!(err.op, "pkg.retitle");
        assert!(err.reason.contains("empty"), "reason: {}", err.reason);
    }

    #[test]
    fn insert_reroots_docs_under_parent_and_name() {
        let b: crate::ops::Bundle = vec![];
        let docs = vec![
            (
                "orders-domain-uml/order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            ),
            (
                "orders-domain-uml/customer.md".to_string(),
                "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string(),
            ),
        ];
        let out = apply(
            &b,
            &[Op::PkgInsert {
                parent_path: "sales".into(),
                name: "orders".into(),
                docs,
            }],
        )
        .unwrap();
        assert!(
            out.iter().any(|(p, _)| p == "sales/orders/order.md"),
            "{out:?}"
        );
        assert!(
            out.iter().any(|(p, _)| p == "sales/orders/customer.md"),
            "{out:?}"
        );
        assert!(
            out.iter()
                .all(|(p, _)| !p.starts_with("orders-domain-uml/")),
            "top folder stripped: {out:?}"
        );
    }

    #[test]
    fn insert_at_root_uses_name_as_top_segment() {
        let b: crate::ops::Bundle = vec![];
        let docs = vec![(
            "tmpl/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        )];
        let out = apply(
            &b,
            &[Op::PkgInsert {
                parent_path: "".into(),
                name: "orders".into(),
                docs,
            }],
        )
        .unwrap();
        assert!(out.iter().any(|(p, _)| p == "orders/order.md"), "{out:?}");
    }

    #[test]
    fn insert_preserves_same_directory_relative_links() {
        let b: crate::ops::Bundle = vec![];
        let docs = vec![
            ("t/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Customer](./customer.md)\n".to_string()),
            ("t/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
        ];
        let out = apply(
            &b,
            &[Op::PkgInsert {
                parent_path: "".into(),
                name: "orders".into(),
                docs,
            }],
        )
        .unwrap();
        let order = &out.iter().find(|(p, _)| p == "orders/order.md").unwrap().1;
        assert!(
            order.contains("(./customer.md)"),
            "relative link untouched: {order}"
        );
    }

    #[test]
    fn insert_keeps_distinct_same_basename_docs_across_packages() {
        // The old TS mergeBundles bug: a same-basename doc in a different package
        // must NOT be dropped. Full-path identity keeps both.
        let b: crate::ops::Bundle = vec![(
            "billing/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Invoice Order\n---\n# Invoice Order\n".to_string(),
        )];
        let docs = vec![(
            "t/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Sales Order\n---\n# Sales Order\n".to_string(),
        )];
        let out = apply(
            &b,
            &[Op::PkgInsert {
                parent_path: "".into(),
                name: "sales".into(),
                docs,
            }],
        )
        .unwrap();
        assert!(
            out.iter().any(|(p, _)| p == "billing/order.md"),
            "existing kept: {out:?}"
        );
        assert!(
            out.iter().any(|(p, _)| p == "sales/order.md"),
            "inserted kept: {out:?}"
        );
        assert_eq!(out.len(), 2, "neither dropped: {out:?}");
    }

    #[test]
    fn insert_errors_when_target_package_already_exists() {
        let b: crate::ops::Bundle = vec![(
            "sales/orders/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        )];
        let docs = vec![(
            "t/thing.md".to_string(),
            "---\ntype: uml.Class\ntitle: Thing\n---\n# Thing\n".to_string(),
        )];
        let err = apply(
            &b,
            &[Op::PkgInsert {
                parent_path: "sales".into(),
                name: "orders".into(),
                docs,
            }],
        )
        .unwrap_err();
        assert_eq!(err.op, "pkg.insert");
        assert!(err.reason.contains("already exists"), "got: {}", err.reason);
    }

    #[test]
    fn insert_errors_on_empty_name() {
        let b: crate::ops::Bundle = vec![];
        let docs = vec![(
            "t/x.md".to_string(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n".to_string(),
        )];
        let err = apply(
            &b,
            &[Op::PkgInsert {
                parent_path: "".into(),
                name: "".into(),
                docs,
            }],
        )
        .unwrap_err();
        assert!(err.reason.contains("name"), "got: {}", err.reason);
    }
}
