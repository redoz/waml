//! OKF projection derived exclusively from revision-scoped shell snapshots.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, LazyLock},
};

use regex::Regex;
use waml_syntax::{
    MarkdownStructureMap, OkfMarkdownLanguage, OkfMarkdownSyntaxKind, SyntaxElement, TextRange,
};

use crate::{
    analysis::{
        AnalysisError, AnalysisStage, DocumentCatalog, DocumentId, DocumentVersion, SyntaxSet,
    },
    frontmatter::{parse_value, Frontmatter},
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

static LINK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\(([^)]+)\)").unwrap());
static INDEX_BULLET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[*-]\s*\[[^\]]*\]\(([^)]+)\)(?:\s*-\s*(.*))?$").unwrap());

struct ShellDocument<'a> {
    document: &'a Arc<DocumentVersion>,
    structure: &'a Arc<MarkdownStructureMap>,
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
    shell: &SyntaxSet<OkfMarkdownLanguage>,
    structures: &Arc<BTreeMap<DocumentId, Arc<MarkdownStructureMap>>>,
) -> Result<Bundle, AnalysisError> {
    let documents = validate(catalog, shell, structures)?;
    project(documents).map_err(AnalysisError::Okf)
}

fn validate<'a>(
    catalog: &'a Arc<DocumentCatalog>,
    shell: &'a SyntaxSet<OkfMarkdownLanguage>,
    structures: &'a Arc<BTreeMap<DocumentId, Arc<MarkdownStructureMap>>>,
) -> Result<Vec<ShellDocument<'a>>, AnalysisError> {
    if !Arc::ptr_eq(catalog, shell.catalog()) {
        return invariant("shell catalog is not the candidate catalog");
    }
    if catalog.documents().len() != shell.documents().len()
        || catalog.documents().len() != structures.len()
    {
        return invariant("catalog, shell, and structure-map widths differ");
    }

    let mut validated = Vec::with_capacity(catalog.documents().len());
    for (id, document) in catalog.documents() {
        let snapshot = shell
            .document(*id)
            .ok_or_else(|| structural(format!("missing shell snapshot for {}", document.path())))?;
        let structure = structures
            .get(id)
            .ok_or_else(|| structural(format!("missing structure map for {}", document.path())))?;
        if !Arc::ptr_eq(snapshot.document(), document) {
            return invariant(format!(
                "shell snapshot provenance differs for {}",
                document.path()
            ));
        }
        let source = document.text().shared();
        let tree_text = snapshot.syntax().write_to_string();
        if tree_text.as_str() != source.as_str()
            || snapshot.syntax().root().range().end().to_usize() != source.len()
        {
            return invariant(format!(
                "shell tree text differs from candidate source for {}",
                document.path()
            ));
        }
        validate_structure(document, structure)?;
        let (frontmatter, body_start) = shell_fields(document, snapshot.syntax())?;
        let body_range = text_range(body_start, source.len(), document)?;
        let body = SourceSlice::from_shared_range(source.clone(), body_start..source.len())
            .map_err(|_| structural(format!("invalid body range for {}", document.path())))?;
        validated.push(ShellDocument {
            document,
            structure,
            frontmatter,
            body,
            body_range,
        });
    }
    Ok(validated)
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
        if node.kind() != OkfMarkdownSyntaxKind::Frontmatter {
            continue;
        }
        let closed = node.children().any(|element| {
            element.into_token().is_some_and(|token| {
                token.kind() == OkfMarkdownSyntaxKind::FrontmatterCloseFence
                    && !token.flags().is_missing()
            })
        });
        if !closed {
            return Ok((Frontmatter::default(), 0));
        }
        body_start = node.range().end().to_usize();
        for entry in node.children().filter_map(SyntaxElement::into_node) {
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
            if let (Some(key), Some(value)) = (key, value) {
                frontmatter.entries.push((key, parse_value(&value)));
            }
        }
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
    let (prose, citations_section) = split_citations(shell);
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
        links: extract_links(prose),
        citations: extract_citations(citations_section),
        extra,
    }
}

fn first_h1(shell: &ShellDocument<'_>) -> Option<String> {
    shell
        .structure
        .headings
        .iter()
        .find(|heading| heading.level == 1 && heading.range.start() >= shell.body_range.start())
        .and_then(|heading| shell.document.text().slice(heading.text_range).ok())
        .map(|title| title.trim().trim_end_matches('#').trim())
        .filter(|title| !title.is_empty())
        .map(str::to_owned)
}

fn split_citations<'a>(shell: &'a ShellDocument<'_>) -> (&'a str, &'a str) {
    let body = shell.body.as_str();
    let boundary = shell.structure.headings.iter().find(|heading| {
        heading.level == 1
            && heading.range.start() >= shell.body_range.start()
            && shell
                .document
                .text()
                .slice(heading.text_range)
                .is_ok_and(|text| {
                    text.trim()
                        .trim_end_matches('#')
                        .trim()
                        .eq_ignore_ascii_case("citations")
                })
    });
    if let Some(heading) = boundary {
        let offset = heading.range.start().to_usize() - shell.body_range.start().to_usize();
        (&body[..offset], &body[offset..])
    } else {
        (body, "")
    }
}

fn extract_links(text: &str) -> Vec<Link> {
    LINK_RE
        .captures_iter(text)
        .map(|capture| Link {
            text: capture[1].to_string(),
            href: capture[2].to_string(),
        })
        .collect()
}

fn extract_citations(text: &str) -> Vec<Citation> {
    LINK_RE
        .captures_iter(text)
        .map(|capture| Citation {
            text: capture[1].to_string(),
            href: capture[2].to_string(),
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
    let mut title = String::new();
    let mut intro_lines = Vec::new();
    let mut authored_order = Vec::new();
    let mut seen_bullet = false;
    for line in shell.body.lines() {
        if let Some(captures) = INDEX_BULLET_RE.captures(line) {
            seen_bullet = true;
            let url = captures.get(1).expect("captured index URL").as_str();
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
        } else if !seen_bullet {
            let trimmed = line.trim();
            if let Some(heading) = trimmed.strip_prefix("# ") {
                if title.is_empty() {
                    title = heading.trim().to_owned();
                }
            } else if !trimmed.is_empty() {
                intro_lines.push(trimmed);
            }
        }
    }
    AuthoredIndex {
        index: Index {
            directory,
            title: title_from_frontmatter.or((!title.is_empty()).then_some(title)),
            description: (!intro_lines.is_empty()).then(|| intro_lines.join(" ")),
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
