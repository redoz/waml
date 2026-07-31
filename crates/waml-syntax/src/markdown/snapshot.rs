use std::{collections::HashMap, sync::Arc};

use crate::{
    reparse_okf_markdown_with_structure, DocumentRevision, FullReparseReason, MarkdownDialect,
    MarkdownStructureMap, OkfMarkdownLanguage, OkfSyntaxDiagnosticCode, ParseError, ReparseOutcome,
    SourceText, SyntaxElement, SyntaxTree, TextChange, TextRange, TreeDiagnostic,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkdownLinkKind {
    Inline,
    Reference,
}

#[derive(Clone, Debug)]
pub struct MarkdownLink {
    pub destination: Arc<str>,
    pub destination_range: Option<TextRange>,
    pub title: Option<Arc<str>>,
    pub kind: MarkdownLinkKind,
    pub identity: super::SyntaxIdentity,
    pub owner: super::SyntaxIdentity,
    pub source_range: TextRange,
}

#[derive(Clone, Debug)]
pub struct MarkdownEntity {
    pub value: Arc<str>,
    pub source_range: TextRange,
    pub identity: super::SyntaxIdentity,
}

#[derive(Default)]
pub struct MarkdownSyntaxQueries {
    links: Arc<[MarkdownLink]>,
    entities: Arc<[MarkdownEntity]>,
    backlinks: Arc<HashMap<Arc<str>, Arc<[super::SyntaxIdentity]>>>,
}
impl MarkdownSyntaxQueries {
    pub fn links(&self) -> impl Iterator<Item = &MarkdownLink> {
        self.links.iter()
    }
    pub fn entities(&self) -> impl Iterator<Item = &MarkdownEntity> {
        self.entities.iter()
    }
    pub fn reference_backlinks(&self, label: &str) -> Arc<[super::SyntaxIdentity]> {
        super::reference::normalize_label(label)
            .and_then(|label| self.backlinks.get(&label).cloned())
            .unwrap_or_else(|| Arc::from([]))
    }
}

pub struct MarkdownSyntaxSnapshot {
    revision: DocumentRevision,
    text: SourceText,
    tree: Arc<SyntaxTree<OkfMarkdownLanguage>>,
    structure: Arc<MarkdownStructureMap>,
    diagnostics: Arc<[TreeDiagnostic<OkfSyntaxDiagnosticCode>]>,
    queries: Arc<MarkdownSyntaxQueries>,
}

impl MarkdownSyntaxSnapshot {
    pub fn revision(&self) -> DocumentRevision {
        self.revision
    }
    pub fn text(&self) -> &SourceText {
        &self.text
    }
    pub fn tree(&self) -> &Arc<SyntaxTree<OkfMarkdownLanguage>> {
        &self.tree
    }
    pub fn structure(&self) -> &Arc<MarkdownStructureMap> {
        &self.structure
    }
    pub fn diagnostics(&self) -> &Arc<[TreeDiagnostic<OkfSyntaxDiagnosticCode>]> {
        &self.diagnostics
    }
    pub fn queries(&self) -> &Arc<MarkdownSyntaxQueries> {
        &self.queries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarkdownReparseOutcome {
    Incremental {
        shared_source_independent_green: usize,
        reparsed_range: Option<TextRange>,
    },
    Full {
        reason: FullReparseReason,
    },
}

#[derive(Clone)]
pub struct MarkdownSyntaxUpdate {
    pub snapshot: Arc<MarkdownSyntaxSnapshot>,
    pub affected_ranges: Arc<[TextRange]>,
    pub outcome: MarkdownReparseOutcome,
}

pub fn parse_markdown(
    revision: DocumentRevision,
    text: SourceText,
    dialect: MarkdownDialect,
) -> Result<Arc<MarkdownSyntaxSnapshot>, ParseError> {
    let parsed = crate::parse_okf_markdown(text.clone(), dialect)?;
    let diagnostics = Arc::from(parsed.tree.diagnostics());
    let queries = Arc::new(queries(&parsed.tree)?);
    Ok(Arc::new(MarkdownSyntaxSnapshot {
        revision,
        text,
        tree: parsed.tree,
        structure: parsed.structure,
        diagnostics,
        queries,
    }))
}

pub fn reparse_markdown(
    previous: &MarkdownSyntaxSnapshot,
    revision: DocumentRevision,
    new_text: SourceText,
    changes: &[TextChange],
) -> Result<MarkdownSyntaxUpdate, ParseError> {
    if revision <= previous.revision {
        return Err(ParseError::NonMonotonicRevision {
            previous: previous.revision,
            requested: revision,
        });
    }
    let (outcome, structure) =
        reparse_okf_markdown_with_structure(previous.tree.as_ref(), new_text.clone(), changes)?;
    let (tree, outcome, affected_ranges): (_, _, Arc<[TextRange]>) = match outcome {
        ReparseOutcome::Incremental {
            tree,
            shared_source_independent_green,
            reparsed_range,
        } => {
            if reparsed_range.end() > new_text.len() {
                return Err(ParseError::StructuralInvariant {
                    reason: "incremental reparse range exceeds the new Markdown snapshot".into(),
                });
            }
            (
                tree,
                MarkdownReparseOutcome::Incremental {
                    shared_source_independent_green,
                    reparsed_range: Some(reparsed_range),
                },
                Arc::from([reparsed_range]),
            )
        }
        ReparseOutcome::Full { tree, reason } => {
            (tree, MarkdownReparseOutcome::Full { reason }, Arc::from([]))
        }
    };
    let diagnostics = Arc::from(tree.diagnostics());
    let queries = Arc::new(queries(&tree)?);
    Ok(MarkdownSyntaxUpdate {
        snapshot: Arc::new(MarkdownSyntaxSnapshot {
            revision,
            text: new_text,
            tree,
            structure,
            diagnostics,
            queries,
        }),
        affected_ranges,
        outcome,
    })
}

fn queries(
    tree: &SyntaxTree<crate::OkfMarkdownLanguage>,
) -> Result<MarkdownSyntaxQueries, ParseError> {
    let mut links = Vec::new();
    let mut entities = Vec::new();
    let mut backlinks = HashMap::<Arc<str>, Vec<super::SyntaxIdentity>>::new();
    collect_queries(&tree.root(), &mut links, &mut entities, &mut backlinks)?;
    Ok(MarkdownSyntaxQueries {
        links: links.into(),
        entities: entities.into(),
        backlinks: backlinks
            .into_iter()
            .map(|(label, owners)| (label, owners.into()))
            .collect::<HashMap<_, _>>()
            .into(),
    })
}

fn collect_queries(
    node: &crate::SyntaxNode<crate::OkfMarkdownLanguage>,
    links: &mut Vec<MarkdownLink>,
    entities: &mut Vec<MarkdownEntity>,
    out: &mut HashMap<Arc<str>, Vec<super::SyntaxIdentity>>,
) -> Result<(), ParseError> {
    if matches!(
        node.kind(),
        crate::OkfMarkdownSyntaxKind::Link | crate::OkfMarkdownSyntaxKind::Image
    ) {
        let annotations = node.syntax_annotations();
        let destination = required_annotation(
            annotations,
            super::inline::destination_annotation(),
            "link destination",
        )?;
        let destination_range = required_annotation(
            annotations,
            super::inline::destination_range_annotation(),
            "link destination range",
        )
        .and_then(parse_annotation_range)?;
        let kind = match required_annotation(
            annotations,
            super::inline::link_kind_annotation(),
            "link kind",
        )?
        .as_ref()
        {
            "inline" => MarkdownLinkKind::Inline,
            "reference" => MarkdownLinkKind::Reference,
            _ => {
                return Err(ParseError::StructuralInvariant {
                    reason: "unknown semantic Markdown link kind".into(),
                })
            }
        };
        let identity = identity(node).ok_or_else(|| ParseError::StructuralInvariant {
            reason: "semantic Markdown link has no identity".into(),
        })?;
        let owner =
            required_annotation(annotations, super::inline::owner_annotation(), "link owner")?
                .parse::<u64>()
                .ok()
                .and_then(|value| super::SyntaxIdentity::from_annotation_data(&value.to_string()))
                .ok_or_else(|| ParseError::StructuralInvariant {
                    reason: "semantic Markdown link has invalid owner identity".into(),
                })?;
        let title = super::inline::link_annotation(annotations, super::inline::title_annotation())
            .map(Arc::from);
        if let Some(label) =
            super::inline::link_annotation(annotations, super::inline::reference_label_annotation())
                .map(Arc::from)
        {
            let owners = out.entry(label).or_default();
            if !owners.contains(&owner) {
                owners.push(owner);
            }
        }
        links.push(MarkdownLink {
            destination,
            destination_range: Some(destination_range),
            title,
            kind,
            identity,
            owner,
            source_range: node.range(),
        });
    } else if node.kind() == crate::OkfMarkdownSyntaxKind::Entity {
        let value = required_annotation(
            node.syntax_annotations(),
            super::inline::entity_value_annotation(),
            "entity value",
        )?;
        let identity = identity(node).ok_or_else(|| ParseError::StructuralInvariant {
            reason: "semantic Markdown entity has no identity".into(),
        })?;
        entities.push(MarkdownEntity {
            value,
            source_range: node.range(),
            identity,
        });
    }
    for child in node.children() {
        if let SyntaxElement::Node(child) = child {
            collect_queries(&child, links, entities, out)?;
        }
    }
    Ok(())
}

fn identity(node: &crate::SyntaxNode<crate::OkfMarkdownLanguage>) -> Option<super::SyntaxIdentity> {
    node.syntax_annotations()
        .iter()
        .find(|annotation| annotation.kind() == "waml.markdown.identity")
        .and_then(|annotation| annotation.data())
        .and_then(super::SyntaxIdentity::from_annotation_data)
}

fn required_annotation(
    annotations: &[crate::SyntaxAnnotation],
    kind: &str,
    description: &'static str,
) -> Result<Arc<str>, ParseError> {
    super::inline::link_annotation(annotations, kind)
        .map(Arc::from)
        .ok_or_else(|| ParseError::StructuralInvariant {
            reason: format!("semantic Markdown node has no {description}").into(),
        })
}

fn parse_annotation_range(value: Arc<str>) -> Result<TextRange, ParseError> {
    let Some((start, end)) = value.split_once(':') else {
        return Err(ParseError::StructuralInvariant {
            reason: "semantic Markdown link has invalid destination range".into(),
        });
    };
    let start = start
        .parse::<usize>()
        .map_err(|_| ParseError::StructuralInvariant {
            reason: "semantic Markdown link has invalid destination start".into(),
        })?;
    let end = end
        .parse::<usize>()
        .map_err(|_| ParseError::StructuralInvariant {
            reason: "semantic Markdown link has invalid destination end".into(),
        })?;
    TextRange::new(
        crate::TextSize::try_from_usize(start)
            .map_err(|_| ParseError::SourceTooLarge { bytes: start })?,
        crate::TextSize::try_from_usize(end)
            .map_err(|_| ParseError::SourceTooLarge { bytes: end })?,
    )
    .map_err(|_| ParseError::StructuralInvariant {
        reason: "semantic Markdown link has reversed destination range".into(),
    })
}
