use std::{fmt, sync::Arc};

use waml_syntax::{MarkdownStructureMap, TextRange, TextSize};

use crate::{
    action::{ActionBasis, ActionError, CodeAction, TextEdit, VersionedDocumentChange},
    analysis::{DocumentId, OkfAnalysis, PreparedCandidate},
    edit::EditError,
    uml,
};

pub struct ActionContext<'a> {
    okf: &'a OkfAnalysis,
    uml: &'a uml::Analysis,
    session_revision: u64,
}

impl<'a> ActionContext<'a> {
    pub fn new(
        okf: &'a OkfAnalysis,
        uml: &'a uml::Analysis,
        session_revision: u64,
    ) -> Result<Self, ActionError> {
        if !Arc::ptr_eq(&okf.catalog, uml.syntax.catalog()) {
            return Err(ActionError::MismatchedCatalog);
        }
        for revision in [
            okf.catalog.session_revision(),
            uml.syntax.catalog().session_revision(),
            uml.session_revision(),
        ] {
            if revision != session_revision {
                return Err(ActionError::MismatchedAnalysisRevision {
                    catalog: revision,
                    requested: session_revision,
                });
            }
        }
        Ok(Self {
            okf,
            uml,
            session_revision,
        })
    }

    pub fn from_prepared(candidate: &'a PreparedCandidate) -> Result<Self, ActionError> {
        Self::new(candidate.okf(), candidate.uml(), candidate.revision())
    }
    pub fn okf(&self) -> &'a OkfAnalysis {
        self.okf
    }
    pub fn uml(&self) -> &'a uml::Analysis {
        self.uml
    }
    pub fn session_revision(&self) -> u64 {
        self.session_revision
    }
}

pub struct Formatter;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormatError {
    Action(ActionError),
    UnknownDocument { document: DocumentId },
    NotClaimed { document: DocumentId },
    StructuralInvariant { reason: Arc<str> },
}
impl fmt::Display for FormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "UML format error: {self:?}")
    }
}
impl std::error::Error for FormatError {}
impl From<ActionError> for FormatError {
    fn from(error: ActionError) -> Self {
        Self::Action(error)
    }
}
impl From<FormatError> for EditError {
    fn from(error: FormatError) -> Self {
        EditError {
            index: 0,
            op: "uml.format".into(),
            selector: None,
            reason: error.to_string(),
        }
    }
}

impl Formatter {
    pub fn format(
        &self,
        context: ActionContext<'_>,
        document: DocumentId,
    ) -> Result<CodeAction, FormatError> {
        let version = context
            .okf
            .catalog
            .document(document)
            .ok_or(FormatError::UnknownDocument { document })?;
        let snapshot = context
            .uml
            .syntax
            .document(document)
            .ok_or(FormatError::NotClaimed { document })?;
        if !Arc::ptr_eq(version, snapshot.document()) {
            return Err(FormatError::StructuralInvariant {
                reason: "UML syntax snapshot does not share the catalog document".into(),
            });
        }
        let exact = snapshot.syntax().write_to_string();
        let has_recovery = !snapshot.syntax().diagnostics().is_empty();
        let edits = if has_recovery {
            Vec::new()
        } else {
            let structure = context
                .okf
                .markdown
                .document(document)
                .map(|snapshot| snapshot.structure())
                .ok_or_else(|| FormatError::StructuralInvariant {
                    reason: "claimed document has no Markdown structure map".into(),
                })?;
            let canonical = canonical_document(&exact, structure);
            protected_diff(&exact, &canonical, structure)?
        };
        Ok(CodeAction {
            title: "Format UML document".into(),
            basis: ActionBasis::Document {
                document,
                document_revision: version.revision(),
                session_revision: context.session_revision,
            },
            changes: Arc::from([VersionedDocumentChange {
                document,
                base_document_revision: version.revision(),
                edits: edits.into(),
            }]),
        })
    }
}

#[derive(Clone)]
struct Section<'a> {
    title: &'a str,
    raw: &'a str,
    order: Option<u8>,
    protected: bool,
}

fn canonical_document(source: &str, structure: &MarkdownStructureMap) -> String {
    let starts = top_level_section_starts(source, structure);
    let prefix_end = starts
        .first()
        .map(|(start, _)| *start)
        .unwrap_or(source.len());
    let mut prefix =
        if let Some((body_start, body_end)) = unowned_body_range(source, structure, &starts) {
            let mut prefix = canonical_prefix(&source[..body_start])
                .trim_end_matches(['\r', '\n'])
                .to_owned();
            prefix.push_str("\n\n");
            prefix.push_str(&source[body_start..body_end]);
            prefix
        } else {
            canonical_prefix(&source[..prefix_end])
                .trim_end_matches('\n')
                .to_owned()
        };
    if !prefix.ends_with('\n') {
        prefix.push('\n');
    }
    let mut sections = Vec::new();
    for (index, (start, title)) in starts.iter().enumerate() {
        let end = starts
            .get(index + 1)
            .map(|(next, _)| *next)
            .unwrap_or(source.len());
        sections.push(Section {
            title,
            raw: &source[*start..end],
            order: section_order(title),
            protected: structure
                .opaque_ranges
                .iter()
                .any(|range| range.start().to_usize() < end && *start < range.end().to_usize()),
        });
    }
    sections.sort_by_key(|section| section.order.unwrap_or(u8::MAX));
    for section in sections {
        if !prefix.ends_with('\n') {
            prefix.push('\n');
        }
        if !prefix.ends_with("\n\n") {
            prefix.push('\n');
        }
        if section.order.is_some() && !section.protected {
            prefix.push_str(&canonical_section(section.title, section.raw));
        } else {
            prefix.push_str(section.raw);
        }
        if !prefix.ends_with('\n') {
            prefix.push('\n');
        }
    }
    prefix
}

fn canonical_prefix(prefix: &str) -> String {
    let normalized = prefix.replace("\r\n", "\n");
    if !normalized.starts_with("---\n") {
        return normalized;
    }
    let close = ["\n---\n", "\n...\n"]
        .into_iter()
        .filter_map(|delimiter| {
            normalized[4..]
                .find(delimiter)
                .map(|offset| (offset, delimiter.len()))
        })
        .min_by_key(|(offset, _)| *offset);
    let Some((relative_close, delimiter_len)) = close else {
        return normalized;
    };
    let close_start = 4 + relative_close;
    let close_end = close_start + delimiter_len;
    let rest = normalized[close_end..].trim_start_matches('\n');
    // The fence is re-rendered through the shared reader and writer, so the
    // formatter speaks exactly the dialect the model reads back. A private
    // second spelling here drifts from `waml::frontmatter` and rewrites a
    // quoted `"null"` into a real null on the next format.
    let Some(parsed) = crate::frontmatter::parse_frontmatter_source(&normalized[..close_end])
    else {
        return normalized;
    };
    let mut out = String::from("---\n");
    let rendered = crate::frontmatter::render_frontmatter(&parsed);
    if !rendered.is_empty() {
        out.push_str(&rendered);
        out.push('\n');
    }
    out.push_str("---\n");
    out.push('\n');
    out.push_str(rest);
    out
}

fn top_level_section_starts<'a>(
    source: &'a str,
    structure: &MarkdownStructureMap,
) -> Vec<(usize, &'a str)> {
    structure
        .headings
        .iter()
        .filter(|heading| heading.level == 2)
        .map(|heading| {
            (
                heading.range.start().to_usize(),
                source[heading.text_range.start().to_usize()..heading.text_range.end().to_usize()]
                    .trim(),
            )
        })
        .collect()
}

fn section_order(title: &str) -> Option<u8> {
    Some(match title.to_ascii_lowercase().as_str() {
        "body" => 0,
        "attributes" => 1,
        "slots" => 2,
        "values" => 3,
        "relationships" => 4,
        "notes" => 5,
        "nodes" => 6,
        "lifelines" => 7,
        "gates" => 8,
        "messages" => 9,
        "members" => 10,
        "layout" => 11,
        _ => return None,
    })
}

fn canonical_section(title: &str, raw: &str) -> String {
    let canonical_title = match title.to_ascii_lowercase().as_str() {
        "body" => "Body",
        "attributes" => "Attributes",
        "slots" => "Slots",
        "values" => "Values",
        "relationships" => "Relationships",
        "notes" => "Notes",
        "nodes" => "Nodes",
        "lifelines" => "Lifelines",
        "gates" => "Gates",
        "messages" => "Messages",
        "members" => "Members",
        "layout" => "Layout",
        _ => title,
    };
    let mut lines = raw.lines();
    let _heading = lines.next();
    let body: Vec<_> = lines.collect();
    let mut out = format!("## {canonical_title}");
    if canonical_title == "Nodes" {
        for line in canonical_flow_lines(&body) {
            out.push('\n');
            out.push_str(&line);
        }
    } else if canonical_title == "Members" {
        for line in canonical_member_lines(&body) {
            out.push('\n');
            out.push_str(&line);
        }
    } else {
        for line in body.into_iter().filter(|line| !line.trim().is_empty()) {
            out.push('\n');
            out.push_str(&canonical_owned_line(canonical_title, line));
        }
    }
    out
}

fn canonical_flow_lines(lines: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in lines.iter().map(|line| line.trim_end()) {
        if line.trim().is_empty() {
            continue;
        }
        if line.trim_start().starts_with("### ") || line.trim_start().starts_with("#### ") {
            if out.last().map_or(true, |line| !line.is_empty()) {
                out.push(String::new());
            }
            out.push(canonical_heading(line));
        } else {
            out.push(canonical_owned_line("Nodes", line));
        }
    }
    out
}

fn canonical_member_lines(lines: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in lines.iter().map(|line| line.trim_end()) {
        if line.trim().is_empty() {
            continue;
        }
        if line.trim_start().starts_with("### ") {
            if out.last().map_or(true, |line| !line.is_empty()) {
                out.push(String::new());
            }
            out.push(canonical_heading(line));
        } else {
            out.push(canonical_owned_line("Members", line));
        }
    }
    out
}

fn canonical_heading(line: &str) -> String {
    let trimmed = line.trim();
    let markers = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    format!("{} {}", "#".repeat(markers), trimmed[markers..].trim())
}

fn canonical_owned_line(section: &str, line: &str) -> String {
    let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
    let indent = if section == "Messages" {
        " ".repeat((indent_len / 2) * 2)
    } else {
        String::new()
    };
    let trimmed = line.trim();
    if !trimmed.starts_with('-') {
        return format!("{indent}{trimmed}");
    }
    let mut body = collapse_unquoted_whitespace(trimmed[1..].trim());
    body = normalize_colon(&body);
    body = normalize_commas(&body);
    if section != "Members" {
        body = normalize_links(&body);
    }
    if section == "Attributes" {
        body = canonical_attribute_multiplicity(&body);
    }
    if section == "Layout" {
        body = canonical_layout_keywords(&body);
        if let Some((before, hints)) = body.split_once(" with ") {
            body = format!("{before} with {}", hints.replace(" and ", ", "));
        }
    }
    format!("{indent}- {body}")
}

fn normalize_commas(value: &str) -> String {
    let mut out = String::new();
    let mut quote = None;
    let mut link_depth = 0usize;
    let chars: Vec<_> = value.chars().collect();
    let mut index = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        if matches!(ch, '"' | '`') {
            quote = if quote == Some(ch) {
                None
            } else if quote.is_none() {
                Some(ch)
            } else {
                quote
            };
        } else if ch == '[' && quote.is_none() {
            link_depth += 1;
        } else if ch == ']' && quote.is_none() {
            link_depth = link_depth.saturating_sub(1);
        }
        if ch == ',' && quote.is_none() && link_depth == 0 {
            while out.ends_with(' ') {
                out.pop();
            }
            out.push(',');
            out.push(' ');
            index += 1;
            while chars.get(index) == Some(&' ') {
                index += 1;
            }
            continue;
        }
        out.push(ch);
        index += 1;
    }
    out.trim_end().to_owned()
}

fn canonical_layout_keywords(value: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "above",
        "aligned",
        "and",
        "as",
        "below",
        "bottom",
        "box",
        "center",
        "collapsed",
        "column",
        "emphasized",
        "frame",
        "large",
        "left",
        "margin",
        "margins",
        "medium",
        "no",
        "of",
        "right",
        "row",
        "shrink",
        "small",
        "top",
        "with",
    ];
    let mut out = String::new();
    let mut cursor = 0usize;
    let mut quote = None;
    while cursor < value.len() {
        let ch = value[cursor..].chars().next().expect("cursor is in bounds");
        if matches!(ch, '"' | '`') {
            quote = if quote == Some(ch) {
                None
            } else if quote.is_none() {
                Some(ch)
            } else {
                quote
            };
            out.push(ch);
            cursor += ch.len_utf8();
            continue;
        }
        if quote.is_none() && ch == '[' {
            if let Some(close) = value[cursor..].find(')') {
                out.push_str(&value[cursor..cursor + close + 1]);
                cursor += close + 1;
                continue;
            }
        }
        if quote.is_none() && ch.is_ascii_alphabetic() {
            let end = value[cursor..]
                .find(|next: char| !next.is_ascii_alphabetic())
                .map(|relative| cursor + relative)
                .unwrap_or(value.len());
            let word = &value[cursor..end];
            let lower = word.to_ascii_lowercase();
            if KEYWORDS.contains(&lower.as_str()) {
                out.push_str(if lower == "margins" { "margin" } else { &lower });
            } else {
                out.push_str(word);
            }
            cursor = end;
            continue;
        }
        out.push(ch);
        cursor += ch.len_utf8();
    }
    out
}

fn canonical_attribute_multiplicity(value: &str) -> String {
    for (delimiter, close) in [(" [", ']'), (" {", '}')] {
        let Some(open) = value.rfind(delimiter) else {
            continue;
        };
        let Some(inner) = value[open + 2..].strip_suffix(close) else {
            continue;
        };
        let Some(multiplicity) = crate::multiplicity::Multiplicity::parse(inner) else {
            continue;
        };
        return format!("{} {{{}}}", &value[..open], multiplicity.as_str());
    }
    value.to_owned()
}

fn normalize_links(value: &str) -> String {
    let mut out = String::new();
    let mut cursor = 0usize;
    while let Some(open_label_relative) = value[cursor..].find('[') {
        let open_label = cursor + open_label_relative;
        let Some(close_label_relative) = value[open_label + 1..].find("](") else {
            break;
        };
        let close_label = open_label + 1 + close_label_relative;
        let href_start = close_label + 2;
        let Some(close_href_relative) = value[href_start..].find(')') else {
            break;
        };
        let close_href = href_start + close_href_relative;
        out.push_str(&value[cursor..open_label]);
        let label = value[open_label + 1..close_label].trim();
        let href = value[href_start..close_href].trim();
        let href = if href.ends_with(".md") && !href.starts_with("./") && !href.starts_with("../") {
            format!("./{href}")
        } else {
            href.to_owned()
        };
        out.push('[');
        out.push_str(label);
        out.push_str("](");
        out.push_str(&href);
        out.push(')');
        cursor = close_href + 1;
    }
    out.push_str(&value[cursor..]);
    out
}

fn collapse_unquoted_whitespace(value: &str) -> String {
    let mut out = String::new();
    let mut quote = None;
    let mut pending_space = false;
    for ch in value.chars() {
        if matches!(ch, '"' | '`') {
            if quote == Some(ch) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(ch);
            }
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.push(ch);
        } else if ch.is_whitespace() && quote.is_none() {
            pending_space = true;
        } else {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.push(ch);
        }
    }
    out
}

fn normalize_colon(value: &str) -> String {
    let mut out = String::new();
    let mut quote = None;
    let chars: Vec<_> = value.chars().collect();
    let mut index = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        if matches!(ch, '"' | '`') {
            quote = if quote == Some(ch) {
                None
            } else if quote.is_none() {
                Some(ch)
            } else {
                quote
            };
        }
        if ch == ':' && quote.is_none() {
            while out.ends_with(' ') {
                out.pop();
            }
            out.push(':');
            out.push(' ');
            index += 1;
            while chars.get(index) == Some(&' ') {
                index += 1;
            }
            continue;
        }
        out.push(ch);
        index += 1;
    }
    out.trim_end().to_owned()
}

fn protected_diff(
    source: &str,
    target: &str,
    structure: &MarkdownStructureMap,
) -> Result<Vec<TextEdit>, FormatError> {
    if source == target {
        return Ok(Vec::new());
    }
    let starts = top_level_section_starts(source, structure);
    let mut protected: Vec<(usize, &str)> = starts
        .iter()
        .enumerate()
        .filter_map(|(index, (start, title))| {
            let end = starts
                .get(index + 1)
                .map(|(next, _)| *next)
                .unwrap_or(source.len());
            let has_protected = structure
                .opaque_ranges
                .iter()
                .any(|range| range.start().to_usize() < end && *start < range.end().to_usize());
            (section_order(title).is_none() || has_protected).then(|| {
                let end = starts
                    .get(index + 1)
                    .map(|(next, _)| *next)
                    .unwrap_or(source.len());
                (*start, &source[*start..end])
            })
        })
        .collect();
    if let Some((start, end)) = unowned_body_range(source, structure, &starts) {
        protected.push((start, &source[start..end]));
    }
    protected.sort_by_key(|(start, _)| *start);
    if protected.is_empty() {
        return Ok(vec![edit_for_gap(0, source, target)?]);
    }
    let mut edits = Vec::new();
    let mut source_cursor = 0usize;
    let mut target_cursor = 0usize;
    for (_, anchor) in protected {
        let source_relative = source[source_cursor..]
            .find(anchor)
            .ok_or_else(|| invariant("protected source anchor is missing"))?;
        let target_relative = target[target_cursor..]
            .find(anchor)
            .ok_or_else(|| invariant("formatter changed protected Markdown"))?;
        let source_anchor = source_cursor + source_relative;
        let target_anchor = target_cursor + target_relative;
        let source_gap = &source[source_cursor..source_anchor];
        let target_gap = &target[target_cursor..target_anchor];
        if source_gap != target_gap {
            edits.push(edit_for_gap(source_cursor, source_gap, target_gap)?);
        }
        source_cursor = source_anchor + anchor.len();
        target_cursor = target_anchor + anchor.len();
    }
    if source[source_cursor..] != target[target_cursor..] {
        edits.push(edit_for_gap(
            source_cursor,
            &source[source_cursor..],
            &target[target_cursor..],
        )?);
    }
    Ok(edits)
}

fn unowned_body_range(
    source: &str,
    structure: &MarkdownStructureMap,
    starts: &[(usize, &str)],
) -> Option<(usize, usize)> {
    let heading = structure
        .headings
        .iter()
        .find(|heading| heading.level == 1)?;
    let heading_start = heading.range.start().to_usize();
    let after_heading = source[heading_start..]
        .find('\n')
        .map(|relative| heading_start + relative + 1)
        .unwrap_or_else(|| heading.range.end().to_usize());
    let section_start = starts
        .first()
        .map(|(start, _)| *start)
        .unwrap_or(source.len());
    if after_heading >= section_start {
        return None;
    }
    let between = &source[after_heading..section_start];
    let leading = between.len() - between.trim_start_matches(['\r', '\n']).len();
    let content = between.trim_end_matches(['\r', '\n']);
    if leading >= content.len() {
        return None;
    }
    let start = after_heading + leading;
    let content_end = after_heading + content.len();
    let line_ending = if source[content_end..].starts_with("\r\n") {
        2
    } else if source[content_end..].starts_with('\n') {
        1
    } else {
        0
    };
    Some((start, content_end + line_ending))
}

fn edit_for_gap(base: usize, source: &str, target: &str) -> Result<TextEdit, FormatError> {
    let prefix = common_prefix_boundary(source, target);
    let source_tail = &source[prefix..];
    let target_tail = &target[prefix..];
    let suffix = common_suffix_boundary(source_tail, target_tail);
    Ok(TextEdit {
        range: TextRange::new(
            size(base + prefix)?,
            size(base + source.len().saturating_sub(suffix))?,
        )
        .map_err(|_| invariant("invalid formatter diff range"))?,
        replacement: Arc::from(&target[prefix..target.len().saturating_sub(suffix)]),
    })
}

fn common_prefix_boundary(left: &str, right: &str) -> usize {
    left.char_indices()
        .zip(right.char_indices())
        .take_while(|((_, left), (_, right))| left == right)
        .map(|((index, ch), _)| index + ch.len_utf8())
        .last()
        .unwrap_or(0)
}

fn common_suffix_boundary(left: &str, right: &str) -> usize {
    left.char_indices()
        .rev()
        .zip(right.char_indices().rev())
        .take_while(|((_, left), (_, right))| left == right)
        .map(|((index, _), _)| left.len() - index)
        .last()
        .unwrap_or(0)
}

fn size(value: usize) -> Result<TextSize, FormatError> {
    TextSize::try_from_usize(value).map_err(|_| invariant("offset exceeds TextSize"))
}
fn invariant(reason: &str) -> FormatError {
    FormatError::StructuralInvariant {
        reason: Arc::from(reason),
    }
}
