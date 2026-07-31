//! OKF projection derived exclusively from revision-scoped shell snapshots.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use waml_syntax::{
    write_green_to, GreenNode, MarkdownLinkKind, MarkdownStructureMap, MarkdownSyntaxSnapshot,
    OkfMarkdownLanguage, TextRange,
};

use crate::{
    analysis::{AnalysisError, AnalysisStage, DocumentCatalog, DocumentVersion, MarkdownSyntaxSet},
    frontmatter::{parse_closed_syntax, Frontmatter},
    source::SourceSlice,
};

use super::{
    resolve_href, Bundle, BundleError, Citation, Concept, Directory, DirectoryAddress, Index, Link,
    Log,
};

const KNOWN_KEYS: &[&str] = &[
    "type",
    "title",
    "description",
    "resource",
    "tags",
    "timestamp",
];

struct ShellDocument<'a> {
    document: &'a Arc<DocumentVersion>,
    snapshot: &'a Arc<MarkdownSyntaxSnapshot>,
    frontmatter: Frontmatter,
    body: SourceSlice,
    body_range: TextRange,
}

struct AuthoredIndex {
    index: Index,
    authored_order: Vec<String>,
}

pub(crate) fn derive(
    catalog: &Arc<DocumentCatalog>,
    markdown: &MarkdownSyntaxSet,
) -> Result<Bundle, AnalysisError> {
    let documents = validate(catalog, markdown)?;
    project(documents).map_err(AnalysisError::Okf)
}

fn validate<'a>(
    catalog: &'a Arc<DocumentCatalog>,
    markdown: &'a MarkdownSyntaxSet,
) -> Result<Vec<ShellDocument<'a>>, AnalysisError> {
    if !Arc::ptr_eq(catalog, markdown.catalog()) {
        return invariant("shell catalog is not the candidate catalog");
    }
    if catalog.documents().len() != markdown.documents().len() {
        return invariant("catalog and Markdown snapshot widths differ");
    }

    let mut validated = Vec::with_capacity(catalog.documents().len());
    for (id, document) in catalog.documents() {
        let snapshot = markdown.document(*id).ok_or_else(|| {
            structural(format!("missing Markdown snapshot for {}", document.path()))
        })?;
        let structure = snapshot.structure();
        if snapshot.revision() != document.revision() {
            return invariant(format!(
                "Markdown snapshot revision differs for {}",
                document.path()
            ));
        }
        let source = document.text().shared();
        if !exact_tree_source(snapshot.tree().root_green(), source.as_str())
            || snapshot.tree().root().range().end().to_usize() != source.len()
        {
            return invariant(format!(
                "shell tree text differs from candidate source for {}",
                document.path()
            ));
        }
        validate_structure(document, structure)?;
        let (frontmatter, body_start) = shell_fields(document, snapshot.tree())?;
        let body_range = text_range(body_start, source.len(), document)?;
        let body = SourceSlice::from_shared_range(source.clone(), body_start..source.len())
            .map_err(|_| structural(format!("invalid body range for {}", document.path())))?;
        validated.push(ShellDocument {
            document,
            snapshot,
            frontmatter,
            body,
            body_range,
        });
    }
    Ok(validated)
}

/// Validates lossless green output against source without materializing the
/// document.  A short write, mismatch, or trailing source bytes is false.
fn exact_tree_source(tree: &GreenNode<OkfMarkdownLanguage>, source: &str) -> bool {
    struct Comparator<'a> {
        source: &'a str,
        cursor: usize,
        matched: bool,
    }
    impl fmt::Write for Comparator<'_> {
        fn write_str(&mut self, fragment: &str) -> fmt::Result {
            let Some(end) = self.cursor.checked_add(fragment.len()) else {
                self.matched = false;
                return Err(fmt::Error);
            };
            if source_fragment(self.source, self.cursor, end) != Some(fragment) {
                self.matched = false;
                return Err(fmt::Error);
            }
            self.cursor = end;
            Ok(())
        }
    }
    let mut comparator = Comparator {
        source,
        cursor: 0,
        matched: true,
    };
    write_green_to(tree, &mut comparator).is_ok()
        && comparator.matched
        && comparator.cursor == source.len()
}

fn source_fragment(source: &str, start: usize, end: usize) -> Option<&str> {
    (start <= end
        && end <= source.len()
        && source.is_char_boundary(start)
        && source.is_char_boundary(end))
    .then(|| &source[start..end])
}

fn validate_structure(
    document: &DocumentVersion,
    structure: &MarkdownStructureMap,
) -> Result<(), AnalysisError> {
    let len = document.text().len();
    let valid = structure.headings.iter().all(|heading| {
        heading.range.end() <= len
            && heading.text_range.end() <= len
            && heading.range.start() <= heading.text_range.start()
    }) && structure
        .protected_ranges
        .iter()
        .all(|range| range.end() <= len);
    if valid {
        Ok(())
    } else {
        invariant(format!(
            "structure map exceeds candidate source for {}",
            document.path()
        ))
    }
}

fn shell_fields(
    document: &DocumentVersion,
    tree: &waml_syntax::SyntaxTree<OkfMarkdownLanguage>,
) -> Result<(Frontmatter, usize), AnalysisError> {
    let mut frontmatter = Frontmatter::default();
    let mut body_start = 0;
    for element in tree.root().children() {
        let Some(node) = element.into_node() else {
            continue;
        };
        let Some(parsed) = parse_closed_syntax(&node) else {
            if node.kind() != waml_syntax::OkfMarkdownSyntaxKind::Frontmatter {
                continue;
            }
            return Ok((Frontmatter::default(), 0));
        };
        body_start = node.range().end().to_usize();
        frontmatter = parsed;
        break;
    }
    if body_start <= document.text().len().to_usize() {
        Ok((frontmatter, body_start))
    } else {
        invariant(format!(
            "frontmatter exceeds source for {}",
            document.path()
        ))
    }
}

fn project(documents: Vec<ShellDocument<'_>>) -> Result<Bundle, BundleError> {
    let mut addresses = BTreeSet::from([DirectoryAddress("/".into())]);
    for shell in &documents {
        let mut address = DirectoryAddress::from_source_path(shell.document.path().as_str());
        loop {
            addresses.insert(address.clone());
            let Some(parent) = address.parent() else {
                break;
            };
            address = parent;
        }
    }

    let mut concepts = Vec::new();
    let mut concept_paths = BTreeMap::new();
    let mut authored_indexes = BTreeMap::new();
    let mut logs = Vec::new();
    for shell in &documents {
        let path = shell.document.path().as_str();
        let directory = DirectoryAddress::from_source_path(path);
        match reserved_filename(path) {
            Some("index.md") => {
                authored_indexes.insert(directory.clone(), parse_authored_index(shell, directory));
            }
            Some("log.md") => logs.push(Log {
                directory,
                body: shell.body.clone(),
            }),
            _ => {
                let concept = project_concept(shell);
                if let Some(first_path) = concept_paths.insert(concept.id.clone(), path.to_owned())
                {
                    return Err(BundleError::DuplicateConceptId {
                        id: concept.id,
                        first_path,
                        second_path: path.to_owned(),
                    });
                }
                concepts.push(concept);
            }
        }
    }
    concepts.sort_by(|left, right| left.id.cmp(&right.id));
    logs.sort_by(|left, right| left.directory.cmp(&right.directory));

    let mut directories: Vec<Directory> = addresses
        .iter()
        .map(|address| {
            let mut child_directories: Vec<_> = addresses
                .iter()
                .filter(|candidate| candidate.parent().as_ref() == Some(address))
                .cloned()
                .collect();
            child_directories.sort();
            let mut direct_concepts: Vec<_> = concepts
                .iter()
                .filter(|concept| {
                    DirectoryAddress::concept_parent(&concept.id).as_ref() == Ok(address)
                })
                .map(|concept| concept.id.clone())
                .collect();
            direct_concepts.sort();
            Directory {
                address: address.clone(),
                parent: address.parent(),
                child_directories,
                concepts: direct_concepts,
            }
        })
        .collect();
    directories.sort_by(|left, right| left.address.cmp(&right.address));

    let mut indexes = Vec::new();
    for directory in &directories {
        let default_order = default_member_order(directory, &concepts);
        match authored_indexes.remove(&directory.address) {
            Some(mut authored) => {
                let mut members = Vec::new();
                for member in authored.authored_order {
                    if default_order.contains(&member) && !members.contains(&member) {
                        members.push(member);
                    }
                }
                for member in default_order {
                    if !members.contains(&member) {
                        members.push(member);
                    }
                }
                authored.index.members = members;
                indexes.push(authored.index);
            }
            None => indexes.push(Index {
                directory: directory.address.clone(),
                title: None,
                description: None,
                members: default_order,
                body: None,
                authored: false,
            }),
        }
    }
    Ok(Bundle {
        concepts,
        indexes,
        logs,
        directories,
    })
}

fn project_concept(shell: &ShellDocument<'_>) -> Concept {
    let fm = &shell.frontmatter;
    let title = fm
        .get_str("title")
        .map(String::from)
        .or_else(|| first_h1(shell));
    let description = fm.get_str("description").map(String::from);
    let resource = fm.get_str("resource").map(String::from);
    let timestamp = fm.get_str("timestamp").map(String::from);
    let tags = fm.get_string_list("tags");
    let ty = fm.get_str("type").unwrap_or("").to_string();
    let extra = Frontmatter {
        entries: fm
            .entries
            .iter()
            .filter(|(key, _)| !KNOWN_KEYS.contains(&key.as_str()))
            .cloned()
            .collect(),
    };
    let citation_start = citation_start(shell).unwrap_or(shell.body_range.end());
    let prose_range = TextRange::new(shell.body_range.start(), citation_start)
        .expect("citation heading follows the document body start");
    let citation_range = TextRange::new(citation_start, shell.body_range.end())
        .expect("citation heading is inside the document body");
    Concept {
        id: shell
            .document
            .path()
            .concept_id()
            .expect("validated source paths end in .md")
            .to_owned(),
        ty,
        title,
        description,
        resource,
        tags,
        timestamp,
        body: shell.body.clone(),
        links: extract_links(shell, prose_range),
        citations: extract_citations(shell, citation_range),
        extra,
    }
}

fn first_h1(shell: &ShellDocument<'_>) -> Option<String> {
    shell
        .snapshot
        .queries()
        .headings()
        .find(|heading| heading.level == 1 && heading.range.start() >= shell.body_range.start())
        .and_then(|heading| shell.document.text().slice(heading.content_range).ok())
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_owned)
}

fn citation_start(shell: &ShellDocument<'_>) -> Option<waml_syntax::TextSize> {
    shell.snapshot.queries().headings().find_map(|heading| {
        (heading.level == 1
            && heading.range.start() >= shell.body_range.start()
            && shell
                .document
                .text()
                .slice(heading.content_range)
                .is_ok_and(|text| text.trim().eq_ignore_ascii_case("citations")))
        .then_some(heading.range.start())
    })
}

fn link_is_inside(link: &waml_syntax::MarkdownLink, range: TextRange) -> bool {
    range.start() <= link.source_range.start() && link.source_range.end() <= range.end()
}

fn is_image_link(shell: &ShellDocument<'_>, link: &waml_syntax::MarkdownLink) -> bool {
    shell
        .snapshot
        .queries()
        .images()
        .any(|image| image.owner == link.owner)
}

fn exact_link_text(shell: &ShellDocument<'_>, link: &waml_syntax::MarkdownLink) -> String {
    shell
        .document
        .text()
        .slice(link.content_range)
        .expect("Markdown query link content is inside the document")
        .to_owned()
}

fn extract_links(shell: &ShellDocument<'_>, range: TextRange) -> Vec<Link> {
    shell
        .snapshot
        .queries()
        .links()
        .filter(|link| {
            matches!(
                link.kind,
                MarkdownLinkKind::Inline | MarkdownLinkKind::Reference
            ) && link_is_inside(link, range)
                && !is_image_link(shell, link)
        })
        .map(|link| Link {
            text: exact_link_text(shell, link),
            href: link.destination.to_string(),
        })
        .collect()
}

fn extract_citations(shell: &ShellDocument<'_>, range: TextRange) -> Vec<Citation> {
    shell
        .snapshot
        .queries()
        .links()
        .filter(|link| {
            matches!(
                link.kind,
                MarkdownLinkKind::Inline | MarkdownLinkKind::Reference
            ) && link_is_inside(link, range)
                && !is_image_link(shell, link)
        })
        .map(|link| Citation {
            text: exact_link_text(shell, link),
            href: link.destination.to_string(),
        })
        .collect()
}

fn parse_authored_index(shell: &ShellDocument<'_>, directory: DirectoryAddress) -> AuthoredIndex {
    let title_from_frontmatter = shell
        .frontmatter
        .get_str("title")
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_owned);
    let heading =
        shell.snapshot.queries().headings().find(|heading| {
            heading.level == 1 && heading.range.start() >= shell.body_range.start()
        });
    let title = heading
        .and_then(|heading| shell.document.text().slice(heading.content_range).ok())
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .unwrap_or_default()
        .to_owned();
    let list_items = shell
        .snapshot
        .queries()
        .lists()
        .filter(|list| {
            list.is_item
                && shell.body_range.start() <= list.range.start()
                && list.range.end() <= shell.body_range.end()
        })
        .map(|list| (list.owner, list.range))
        .collect::<Vec<_>>();
    let intro_start = heading
        .map(|heading| heading.range.end())
        .unwrap_or(shell.body_range.start());
    let intro_end = list_items
        .iter()
        .map(|(_, range)| range.start())
        .min()
        .unwrap_or(shell.body_range.end());
    let description = TextRange::new(intro_start, intro_end)
        .ok()
        .and_then(|range| shell.document.text().slice(range).ok())
        .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|text| !text.is_empty());
    let mut authored_order = Vec::new();
    for (owner, item_range) in &list_items {
        let is_bullet = shell
            .snapshot
            .queries()
            .list(*owner)
            .is_some_and(|list| matches!(list.kind, waml_syntax::MarkdownListKind::Bullet));
        if !is_bullet {
            continue;
        }
        let link = shell.snapshot.queries().links().find(|link| {
            matches!(
                link.kind,
                MarkdownLinkKind::Inline | MarkdownLinkKind::Reference
            ) && link_is_inside(link, *item_range)
                && !is_image_link(shell, link)
                && list_items
                    .iter()
                    .filter(|(_, candidate)| link_is_inside(link, *candidate))
                    .min_by_key(|(_, candidate)| {
                        candidate.end().to_usize() - candidate.start().to_usize()
                    })
                    .is_some_and(|(candidate_owner, _)| candidate_owner == owner)
        });
        if let Some(link) = link {
            let url = link.destination.as_ref();
            let member = if let Some(subdirectory) = url.strip_suffix('/') {
                let name = subdirectory.trim_start_matches("./").trim_end_matches('/');
                directory
                    .join_directory(name)
                    .map(|address| address.to_string())
                    .unwrap_or_default()
            } else {
                resolve_href(shell.document.path().as_ref(), url)
            };
            if !member.is_empty() {
                authored_order.push(member);
            }
        }
    }
    AuthoredIndex {
        index: Index {
            directory,
            title: title_from_frontmatter.or((!title.is_empty()).then_some(title)),
            description,
            members: Vec::new(),
            body: Some(shell.body.clone()),
            authored: true,
        },
        authored_order,
    }
}

fn default_member_order(directory: &Directory, concepts: &[Concept]) -> Vec<String> {
    let mut members = Vec::new();
    for child in &directory.child_directories {
        let label = child.as_str().rsplit('/').next().unwrap_or(child.as_str());
        members.push((label.to_lowercase(), child.to_string()));
    }
    for id in &directory.concepts {
        let concept = concepts
            .iter()
            .find(|concept| concept.id == *id)
            .expect("directory concept exists");
        let label = concept
            .title
            .as_deref()
            .unwrap_or_else(|| id.rsplit('/').next().unwrap_or(id));
        members.push((label.to_lowercase(), id.clone()));
    }
    members.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    members.into_iter().map(|(_, id)| id).collect()
}

fn reserved_filename(path: &str) -> Option<&str> {
    match path.rsplit('/').next().unwrap_or(path) {
        "index.md" => Some("index.md"),
        "log.md" => Some("log.md"),
        _ => None,
    }
}

fn text_range(
    start: usize,
    end: usize,
    document: &DocumentVersion,
) -> Result<TextRange, AnalysisError> {
    let start = waml_syntax::TextSize::try_from_usize(start)
        .map_err(|_| structural(format!("source width overflow for {}", document.path())))?;
    let end = waml_syntax::TextSize::try_from_usize(end)
        .map_err(|_| structural(format!("source width overflow for {}", document.path())))?;
    TextRange::new(start, end)
        .map_err(|_| structural(format!("reversed body range for {}", document.path())))
}

fn invariant<T>(reason: impl Into<Arc<str>>) -> Result<T, AnalysisError> {
    Err(AnalysisError::StructuralInvariant {
        stage: AnalysisStage::Shell,
        reason: reason.into(),
    })
}

fn structural(reason: impl Into<Arc<str>>) -> AnalysisError {
    AnalysisError::StructuralInvariant {
        stage: AnalysisStage::Shell,
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::exact_tree_source;
    use std::sync::Arc;
    use waml_syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText};

    #[test]
    fn streaming_tree_source_comparator_covers_exact_mismatch_short_and_trailing() {
        let source = "# Café\nbody\n";
        let parsed = parse_markdown(
            DocumentRevision::INITIAL,
            SourceText::from_shared(Arc::new(source.to_owned())).unwrap(),
            MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        assert!(exact_tree_source(parsed.tree().root_green(), source));
        assert!(!exact_tree_source(
            parsed.tree().root_green(),
            "# Cafe\nbody\n"
        ));
        assert!(!exact_tree_source(
            parsed.tree().root_green(),
            "# Café\nbody"
        ));
        assert!(!exact_tree_source(
            parsed.tree().root_green(),
            "# Café\nbody\ntrailing"
        ));
    }
}
