use std::{
    ops::Range,
    path::{Path, PathBuf},
};

use tower_lsp_server::ls_types as lsp;
use waml::{
    analysis::{DocumentId, DocumentVersion, MarkdownTokenRole, TextSize, WamlCodeRole},
    okf,
    source::BundlePath,
    uml::{completions, ActionContext, CompletionKind},
};

use crate::lsp::{
    bundle::LspAnalysisState,
    map::{from_lsp_position, to_lsp_range},
};

const MARKER_TOKEN: u32 = 0;
const HEADING_TOKEN: u32 = 1;
const LINK_TOKEN: u32 = 2;
const CODE_TOKEN: u32 = 3;
const STRING_TOKEN: u32 = 4;
const COMMENT_TOKEN: u32 = 5;
const KEYWORD_TOKEN: u32 = 6;
const TYPE_TOKEN: u32 = 7;
const PROPERTY_TOKEN: u32 = 8;
const NUMBER_TOKEN: u32 = 9;
const INVALID_TOKEN: u32 = 10;

#[derive(Clone, Debug, Eq, PartialEq)]
struct RawToken {
    range: Range<usize>,
    token_type: u32,
}

pub fn semantic_token_legend() -> lsp::SemanticTokensLegend {
    lsp::SemanticTokensLegend {
        token_types: vec![
            lsp::SemanticTokenType::new("marker"),
            lsp::SemanticTokenType::new("heading"),
            lsp::SemanticTokenType::new("link"),
            lsp::SemanticTokenType::new("code"),
            lsp::SemanticTokenType::STRING,
            lsp::SemanticTokenType::COMMENT,
            lsp::SemanticTokenType::KEYWORD,
            lsp::SemanticTokenType::TYPE,
            lsp::SemanticTokenType::PROPERTY,
            lsp::SemanticTokenType::NUMBER,
            lsp::SemanticTokenType::new("invalid"),
        ],
        token_modifiers: Vec::new(),
    }
}

impl LspAnalysisState {
    pub fn document_symbols(&self, physical: &Path) -> Option<Vec<lsp::DocumentSymbol>> {
        let (_, document_id, document) = self.query_document(physical)?;
        let markdown = self.okf.markdown_snapshot(document_id)?;
        let text = document.text().shared().as_str();
        let symbols = markdown
            .queries()
            .headings()
            .filter_map(|heading| {
                let name = text
                    .get(
                        heading.content_range.start().to_usize()
                            ..heading.content_range.end().to_usize(),
                    )?
                    .trim()
                    .trim_end_matches('#')
                    .trim()
                    .to_owned();
                if name.is_empty() {
                    return None;
                }
                let range = to_lsp_range(
                    heading.range.start().to_usize()..heading.range.end().to_usize(),
                    document,
                )
                .ok()?;
                let selection_range = to_lsp_range(
                    heading.content_range.start().to_usize()
                        ..heading.content_range.end().to_usize(),
                    document,
                )
                .ok()?;
                let is_waml_section = markdown
                    .structure()
                    .islands
                    .iter()
                    .any(|island| island.heading_range == heading.range);
                #[allow(deprecated)]
                Some(lsp::DocumentSymbol {
                    name,
                    detail: None,
                    kind: if is_waml_section {
                        lsp::SymbolKind::FIELD
                    } else {
                        lsp::SymbolKind::NAMESPACE
                    },
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range,
                    children: None,
                })
            })
            .collect();
        Some(symbols)
    }

    pub fn document_links(&self, physical: &Path) -> Option<Vec<lsp::DocumentLink>> {
        let (logical, document_id, document) = self.query_document(physical)?;
        let markdown = self.okf.markdown_snapshot(document_id)?;
        let links = markdown
            .queries()
            .links()
            .filter_map(|link| {
                let range = to_lsp_range(
                    link.source_range.start().to_usize()..link.source_range.end().to_usize(),
                    document,
                )
                .ok()?;
                Some(lsp::DocumentLink {
                    range,
                    target: self.link_target(&logical, &link.destination),
                    tooltip: link.title.as_ref().map(ToString::to_string),
                    data: None,
                })
            })
            .chain(markdown.queries().images().filter_map(|image| {
                let range = to_lsp_range(
                    image.source_range.start().to_usize()..image.source_range.end().to_usize(),
                    document,
                )
                .ok()?;
                Some(lsp::DocumentLink {
                    range,
                    target: self.link_target(&logical, &image.source),
                    tooltip: image.title.as_ref().map(ToString::to_string),
                    data: None,
                })
            }))
            .collect();
        Some(links)
    }

    pub fn definition(&self, physical: &Path, position: lsp::Position) -> Option<lsp::Location> {
        let (logical, document_id, document) = self.query_document(physical)?;
        let markdown = self.okf.markdown_snapshot(document_id)?;
        let offset = from_lsp_position(position, document).ok()?;
        let link = markdown.queries().links().find(|link| {
            link.source_range.start().to_usize() <= offset
                && offset < link.source_range.end().to_usize()
        })?;
        let (target_logical, target_physical, fragment) =
            self.internal_link_target(&logical, &link.destination)?;
        let target_id = self.okf.catalog.id_for_path(&target_logical)?;
        let target_document = self.okf.catalog.document(target_id)?;
        let target_markdown = self.okf.markdown_snapshot(target_id)?;
        let heading = match fragment.as_deref() {
            Some(fragment) => target_markdown.queries().headings().find(|heading| {
                let text = target_document
                    .text()
                    .shared()
                    .get(
                        heading.content_range.start().to_usize()
                            ..heading.content_range.end().to_usize(),
                    )
                    .unwrap_or("");
                heading_slug(text) == heading_slug(fragment)
            }),
            None => target_markdown.queries().headings().next(),
        }?;
        let range = to_lsp_range(
            heading.range.start().to_usize()..heading.range.end().to_usize(),
            target_document,
        )
        .ok()?;
        Some(lsp::Location::new(
            lsp::Uri::from_file_path(target_physical)?,
            range,
        ))
    }

    /// Candidates at `position`. `None` means "not a document I know"; an
    /// empty `Vec` means "nothing is expected here", which is a normal result
    /// and must stay distinguishable from the former.
    pub fn completion(
        &self,
        physical: &Path,
        position: lsp::Position,
    ) -> Option<Vec<lsp::CompletionItem>> {
        let (_, document_id, document) = self.query_document(physical)?;
        let offset = from_lsp_position(position, document).ok()?;
        let offset = TextSize::try_from_usize(offset).ok()?;
        let context = ActionContext::new(&self.okf, &self.uml, self.revision).ok()?;
        let candidates = completions(context, document_id, offset).ok()?;
        let items = candidates
            .into_iter()
            .filter_map(|candidate| {
                let range = to_lsp_range(
                    candidate.replace.start().to_usize()..candidate.replace.end().to_usize(),
                    document,
                )
                .ok()?;
                Some(lsp::CompletionItem {
                    label: candidate.label.to_string(),
                    kind: Some(completion_item_kind(candidate.kind)),
                    detail: candidate.detail.map(|detail| detail.to_string()),
                    text_edit: Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
                        range,
                        new_text: candidate.insert.to_string(),
                    })),
                    ..Default::default()
                })
            })
            .collect();
        Some(items)
    }

    pub fn semantic_tokens(&self, physical: &Path) -> Option<lsp::SemanticTokens> {
        let (_, document_id, document) = self.query_document(physical)?;
        let markdown = self.okf.markdown_snapshot(document_id)?;
        let mut occupied = markdown
            .queries()
            .links()
            .map(|link| link.source_range.start().to_usize()..link.source_range.end().to_usize())
            .chain(markdown.queries().images().map(|image| {
                image.source_range.start().to_usize()..image.source_range.end().to_usize()
            }))
            .collect::<Vec<_>>();
        let mut waml_tokens = Vec::new();
        for span in self.okf.document_code_spans(document_id)?.iter() {
            let token_type = match span.role {
                WamlCodeRole::Keyword => KEYWORD_TOKEN,
                WamlCodeRole::Type => TYPE_TOKEN,
                WamlCodeRole::Property => PROPERTY_TOKEN,
                WamlCodeRole::String => STRING_TOKEN,
                WamlCodeRole::Number => NUMBER_TOKEN,
                WamlCodeRole::Comment => COMMENT_TOKEN,
                WamlCodeRole::Punctuation => MARKER_TOKEN,
                WamlCodeRole::Invalid => INVALID_TOKEN,
            };
            let range = span.range.start().to_usize()..span.range.end().to_usize();
            occupied.push(range.clone());
            waml_tokens.push(RawToken { range, token_type });
        }

        let mut raw = self
            .okf
            .markdown_token_spans(document_id)?
            .iter()
            .filter_map(|span| {
                let token_type = match span.role {
                    MarkdownTokenRole::Marker => MARKER_TOKEN,
                    MarkdownTokenRole::Heading => return None,
                    MarkdownTokenRole::Link => return None,
                    MarkdownTokenRole::Code => CODE_TOKEN,
                    MarkdownTokenRole::Invalid => INVALID_TOKEN,
                };
                let range = span.range.start().to_usize()..span.range.end().to_usize();
                (!occupied.iter().any(|other| overlaps(&range, other)))
                    .then_some(RawToken { range, token_type })
            })
            .collect::<Vec<_>>();
        raw.extend(markdown.queries().headings().map(|heading| RawToken {
            range: heading.content_range.start().to_usize()..heading.content_range.end().to_usize(),
            token_type: HEADING_TOKEN,
        }));
        raw.extend(markdown.queries().links().map(|link| RawToken {
            range: link.source_range.start().to_usize()..link.source_range.end().to_usize(),
            token_type: LINK_TOKEN,
        }));
        raw.extend(markdown.queries().images().map(|image| RawToken {
            range: image.source_range.start().to_usize()..image.source_range.end().to_usize(),
            token_type: LINK_TOKEN,
        }));
        raw.extend(waml_tokens);
        raw.sort_by_key(|token| (token.range.start, token.range.end, token.token_type));
        raw.dedup();
        raw = non_overlapping(raw);

        let mut absolute = raw
            .into_iter()
            .flat_map(|token| split_token(token, document))
            .collect::<Vec<_>>();
        absolute.sort_by_key(|(range, token_type)| {
            (
                range.start.line,
                range.start.character,
                range.end.character,
                *token_type,
            )
        });
        absolute.dedup();
        let mut previous_line = 0;
        let mut previous_character = 0;
        let data = absolute
            .into_iter()
            .map(|(range, token_type)| {
                let delta_line = range.start.line - previous_line;
                let delta_start = if delta_line == 0 {
                    range.start.character - previous_character
                } else {
                    range.start.character
                };
                previous_line = range.start.line;
                previous_character = range.start.character;
                lsp::SemanticToken {
                    delta_line,
                    delta_start,
                    length: range.end.character - range.start.character,
                    token_type,
                    token_modifiers_bitset: 0,
                }
            })
            .collect();
        Some(lsp::SemanticTokens {
            result_id: Some(format!("{}:{}", self.revision, document.revision().get())),
            data,
        })
    }

    fn query_document(
        &self,
        physical: &Path,
    ) -> Option<(BundlePath, DocumentId, &DocumentVersion)> {
        let physical = normalized_physical(physical);
        let logical = self
            .host
            .open_by_physical
            .get(&physical)
            .map(|open| open.logical.clone())
            .or_else(|| {
                self.host
                    .disk_by_physical
                    .get(&physical)
                    .map(|document| document.path().clone())
            })?;
        let document_id = self.okf.catalog.id_for_path(&logical)?;
        let document = self.okf.catalog.document(document_id)?;
        Some((logical, document_id, document))
    }

    fn link_target(&self, source: &BundlePath, destination: &str) -> Option<lsp::Uri> {
        // A `fluent_uri::Uri` parse only succeeds for an absolute URI with a
        // scheme; bundle-relative destinations fall through to the file path.
        if let Ok(url) = destination.parse::<lsp::Uri>() {
            return Some(url);
        }
        let (_, physical, fragment) = self.internal_link_target(source, destination)?;
        let url = lsp::Uri::from_file_path(physical)?;
        match fragment {
            Some(fragment) => format!("{}#{fragment}", url.as_str()).parse().ok(),
            None => Some(url),
        }
    }

    fn internal_link_target(
        &self,
        source: &BundlePath,
        destination: &str,
    ) -> Option<(BundlePath, PathBuf, Option<String>)> {
        if destination.split_once(':').is_some() {
            return None;
        }
        let fragment = destination
            .split_once('#')
            .map(|(_, fragment)| fragment.to_owned());
        let target_id = okf::resolve_href(source.as_str(), destination);
        let target = self.source.document_by_concept_id(&target_id)?;
        let logical = target.path().clone();
        let physical = self.physical_for_logical(&logical)?;
        Some((logical, physical, fragment))
    }

    fn physical_for_logical(&self, logical: &BundlePath) -> Option<PathBuf> {
        self.host
            .open_by_physical
            .iter()
            .find_map(|(physical, open)| (&open.logical == logical).then(|| physical.clone()))
            .or_else(|| {
                self.host
                    .disk_by_physical
                    .iter()
                    .find_map(|(physical, document)| {
                        (document.path() == logical).then(|| physical.clone())
                    })
            })
    }
}

fn split_token(
    token: RawToken,
    document: &DocumentVersion,
) -> impl Iterator<Item = (lsp::Range, u32)> {
    let text = document.text().shared().as_str();
    let mut cursor = token.range.start;
    let mut ranges = Vec::new();
    while cursor < token.range.end {
        let Some(remaining) = text.get(cursor..token.range.end) else {
            break;
        };
        let line_break = remaining.find('\n').map(|relative| cursor + relative);
        let raw_end = line_break.unwrap_or(token.range.end);
        let content_end = raw_end
            .checked_sub(1)
            .filter(|before| text.as_bytes().get(*before) == Some(&b'\r'))
            .unwrap_or(raw_end);
        if cursor < content_end {
            if let Ok(range) = to_lsp_range(cursor..content_end, document) {
                ranges.push((range, token.token_type));
            }
        }
        cursor = line_break.map_or(token.range.end, |line_break| line_break + 1);
    }
    ranges.into_iter()
}

fn non_overlapping(tokens: Vec<RawToken>) -> Vec<RawToken> {
    let mut output: Vec<RawToken> = Vec::new();
    for token in tokens {
        if output
            .last()
            .is_some_and(|previous| previous.range.end > token.range.start)
        {
            continue;
        }
        output.push(token);
    }
    output
}

fn overlaps(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

fn normalized_physical(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().replace('\\', "/"))
}

fn heading_slug(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('#')
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

fn completion_item_kind(kind: CompletionKind) -> lsp::CompletionItemKind {
    match kind {
        CompletionKind::Keyword => lsp::CompletionItemKind::KEYWORD,
        CompletionKind::Reference => lsp::CompletionItemKind::REFERENCE,
        CompletionKind::Field => lsp::CompletionItemKind::FIELD,
        CompletionKind::Value => lsp::CompletionItemKind::ENUM_MEMBER,
        CompletionKind::Link => lsp::CompletionItemKind::FILE,
        CompletionKind::Name => lsp::CompletionItemKind::VARIABLE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiline_tokens_split_before_utf16_delta_encoding() {
        let physical = PathBuf::from("C:/outside/multiline.md");
        let state = LspAnalysisState::empty()
            .unwrap()
            .open(physical.clone(), 1, "a\r\n😀b".into())
            .unwrap();
        let (_, _, document) = state.query_document(&physical).unwrap();
        let split = split_token(
            RawToken {
                range: 0..8,
                token_type: CODE_TOKEN,
            },
            document,
        )
        .collect::<Vec<_>>();

        assert_eq!(
            split,
            [
                (
                    lsp::Range::new(lsp::Position::new(0, 0), lsp::Position::new(0, 1)),
                    CODE_TOKEN,
                ),
                (
                    lsp::Range::new(lsp::Position::new(1, 0), lsp::Position::new(1, 3)),
                    CODE_TOKEN,
                ),
            ]
        );
    }
}
