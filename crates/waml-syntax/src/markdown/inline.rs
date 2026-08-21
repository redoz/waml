use std::{collections::HashMap, num::NonZeroU64, sync::Arc};

use super::scan::{scan_is_inline_html, scan_text_entities};

use crate::{
    GreenElement, GreenFactory, GreenNode, GreenText, MarkdownDialect, OkfMarkdownLanguage,
    OkfMarkdownSyntaxKind as Kind, OkfSyntaxDiagnosticCode as Diagnostic, ParseError, SourceText,
    SyntaxAnnotation, SyntaxIdentity, TextRange, TextSize, TreeDiagnostic,
};

use super::reference::{
    decode_destination, normalize_label, MarkdownReferenceDefinition, MarkdownReferenceMap,
};

const DESTINATION_ANNOTATION: &str = "waml.markdown.link.destination";
const DESTINATION_RANGE_ANNOTATION: &str = "waml.markdown.link.destination_range";
const TITLE_ANNOTATION: &str = "waml.markdown.link.title";
const LINK_KIND_ANNOTATION: &str = "waml.markdown.link.kind";
const REFERENCE_LABEL_ANNOTATION: &str = "waml.markdown.link.reference_label";
const OWNER_ANNOTATION: &str = "waml.markdown.link.owner";
const ENTITY_VALUE_ANNOTATION: &str = "waml.markdown.entity.value";

const DESTINATION_ID: u64 = u64::MAX - 1;
const DESTINATION_RANGE_ID: u64 = u64::MAX - 2;
const TITLE_ID: u64 = u64::MAX - 3;
const LINK_KIND_ID: u64 = u64::MAX - 4;
const REFERENCE_LABEL_ID: u64 = u64::MAX - 5;
const OWNER_ID: u64 = u64::MAX - 6;
const ENTITY_VALUE_ID: u64 = u64::MAX - 7;

pub(crate) struct InlineParse {
    pub root: GreenNode<OkfMarkdownLanguage>,
    pub inline_roots: Arc<[GreenNode<OkfMarkdownLanguage>]>,
    pub references: MarkdownReferenceMap,
    pub diagnostics: Arc<[TreeDiagnostic<Diagnostic>]>,
}

pub(crate) fn apply(
    text: &SourceText,
    root: &GreenNode<OkfMarkdownLanguage>,
    references: MarkdownReferenceMap,
    start: usize,
    dialect: MarkdownDialect,
) -> Result<InlineParse, ParseError> {
    let (root, inline_roots, backlinks, diagnostics) = {
        let mut context = InlineContext {
            text,
            references: &references,
            inline_roots: Vec::new(),
            backlinks: HashMap::new(),
            dialect,
            diagnostics: Vec::new(),
            depth_diagnostic_emitted: false,
        };
        let mut at = start;
        let root = rebuild(&mut context, root, &mut at)?;
        (
            root,
            context.inline_roots,
            context.backlinks,
            context.diagnostics,
        )
    };
    let references = references.with_backlinks(backlinks);
    Ok(InlineParse {
        root,
        inline_roots: inline_roots.into(),
        references,
        diagnostics: diagnostics.into(),
    })
}

struct InlineContext<'a> {
    text: &'a SourceText,
    references: &'a MarkdownReferenceMap,
    inline_roots: Vec<GreenNode<OkfMarkdownLanguage>>,
    backlinks: HashMap<Arc<str>, Vec<SyntaxIdentity>>,
    dialect: MarkdownDialect,
    diagnostics: Vec<TreeDiagnostic<Diagnostic>>,
    /// Set the first time `parse_inlines` hits `MD_MAX_INLINE_DEPTH`, so only
    /// one diagnostic is emitted per document even if the cap bites many
    /// times (e.g. 10,000 nested emphasis markers).
    depth_diagnostic_emitted: bool,
}

fn rebuild(
    context: &mut InlineContext<'_>,
    node: &GreenNode<OkfMarkdownLanguage>,
    at: &mut usize,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    let start = *at;
    let end = start + node.width().to_usize();
    if node.kind() == Kind::Paragraph {
        *at = end;
        let owner = SyntaxIdentity::fresh()?;
        let children = parse_inlines(context, start, end, owner, true)?;
        let rebuilt = semantic_with_identity(Kind::Paragraph, children, owner, Vec::new())?;
        context.inline_roots.push(rebuilt.clone());
        return Ok(rebuilt);
    }
    if node.kind() == Kind::ListItem
        && node.children().iter().any(
            |child| matches!(child, GreenElement::Token(token) if token.kind() == Kind::TextToken),
        )
    {
        let owner = identity(node).ok_or_else(|| ParseError::StructuralInvariant {
            reason: "semantic Markdown list item has no unique valid identity".into(),
        })?;
        let mut children = Vec::new();
        for child in node.children() {
            match child {
                GreenElement::Token(token) if token.kind() == Kind::TextToken => {
                    let token_start = *at;
                    let token_end = token_start + token.width().to_usize();
                    if !has_inline_syntax(&context.text.shared()[token_start..token_end]) {
                        *at = token_end;
                        children.push(GreenElement::Token(token.clone()));
                        continue;
                    }
                    let inline_start = token_start
                        + context.text.shared()[token_start..token_end]
                            .bytes()
                            .take_while(|byte| matches!(byte, b' ' | b'\t'))
                            .count();
                    if token_start < inline_start {
                        children.push(tok(
                            context.text,
                            token_start,
                            inline_start,
                            Kind::WhitespaceToken,
                        )?);
                    }
                    children.extend(parse_inlines(
                        context,
                        inline_start,
                        token_end,
                        owner,
                        true,
                    )?);
                    *at = token_end;
                }
                GreenElement::Token(token) => {
                    *at += token.width().to_usize();
                    children.push(GreenElement::Token(token.clone()));
                }
                GreenElement::Node(child) => {
                    children.push(GreenElement::Node(rebuild(context, child, at)?));
                }
            }
        }
        return GreenFactory::new()
            .node_with_annotations(node.kind(), children, node.annotations().into())
            .map_err(|_| ParseError::WidthOverflow);
    }
    if matches!(
        node.kind(),
        Kind::AtxHeading | Kind::SetextHeading | Kind::TableCell
    ) && has_inline_syntax(&context.text.shared()[start..end])
    {
        let owner = SyntaxIdentity::fresh()?;
        let mut annotations = Vec::new();
        if node.kind() == Kind::TableCell {
            if let Some(data) = node
                .annotations()
                .iter()
                .find(|annotation| annotation.kind() == super::gfm::TABLE_ALIGNMENT)
                .and_then(SyntaxAnnotation::data)
            {
                let data = match data {
                    "left" => "left",
                    "center" => "center",
                    "right" => "right",
                    _ => "none",
                };
                annotations.push(owner.metadata_annotation(super::gfm::TABLE_ALIGNMENT, data));
            }
        }
        let mut children = Vec::new();
        for child in node.children() {
            match child {
                GreenElement::Token(token) if token.kind() == Kind::TextToken => {
                    let token_start = *at;
                    let token_end = token_start + token.width().to_usize();
                    children.extend(parse_inlines(context, token_start, token_end, owner, true)?);
                    *at = token_end;
                }
                GreenElement::Token(token) => {
                    *at += token.width().to_usize();
                    children.push(GreenElement::Token(token.clone()));
                }
                GreenElement::Node(child) => {
                    children.push(GreenElement::Node(rebuild(context, child, at)?));
                }
            }
        }
        let rebuilt = semantic_with_identity(node.kind(), children, owner, annotations)?;
        context.inline_roots.push(rebuilt.clone());
        return Ok(rebuilt);
    }
    let mut children = Vec::new();
    for child in node.children() {
        match child {
            GreenElement::Node(child) => {
                children.push(GreenElement::Node(rebuild(context, child, at)?));
            }
            GreenElement::Token(token) => {
                *at += token.width().to_usize();
                children.push(GreenElement::Token(token.clone()));
            }
        }
    }
    GreenFactory::new()
        .node_with_annotations(node.kind(), children, node.annotations().into())
        .map_err(|_| ParseError::WidthOverflow)
}

fn identity(node: &GreenNode<OkfMarkdownLanguage>) -> Option<SyntaxIdentity> {
    let mut annotations = node
        .annotations()
        .iter()
        .filter(|annotation| annotation.kind() == "waml.markdown.identity");
    let annotation = annotations.next()?;
    if annotations.next().is_some() {
        return None;
    }
    annotation
        .data()
        .and_then(SyntaxIdentity::from_annotation_data)
}

fn has_inline_syntax(source: &str) -> bool {
    source
        .bytes()
        .any(|byte| matches!(byte, b'\\' | b'&' | b'`' | b'*' | b'_' | b'[' | b'!' | b'<'))
}

#[derive(Clone, Copy, Debug)]
struct EmphasisPair {
    open: usize,
    close: usize,
    width: usize,
    kind: Kind,
}

#[derive(Clone, Copy, Debug)]
struct Delimiter {
    marker: u8,
    start: usize,
    length: usize,
    remaining: usize,
    can_open: bool,
    can_close: bool,
}

#[derive(Clone, Copy, Debug)]
struct BracketOpener {
    start: usize,
    open: usize,
    image: bool,
    active: bool,
}

/// What a matched `[...]` turned out to be, decided once by
/// [`bracket_match_end`] and kept.
///
/// The scan cannot match a bracket pair without resolving it -- an unresolved
/// `[...]` is not a link at all -- so the resolution is a by-product of
/// matching, and recording it is free. Everything downstream reads it instead
/// of reading the source again: two readings of one grammar can disagree, one
/// reading cannot.
#[derive(Clone, Debug)]
enum BracketResolution {
    /// `[label](destination "title")` -- the destination was written in the
    /// parentheses that follow the label.
    Inline(InlineDestination),
    /// `[label]`, `[label][]` or `[label][other]` -- resolved through a
    /// reference definition, which is carried here rather than looked up
    /// again.
    Reference {
        normalized: Arc<str>,
        definition: MarkdownReferenceDefinition,
    },
}

#[derive(Clone, Debug)]
struct BracketMatch {
    start: usize,
    open: usize,
    label_end: usize,
    end: usize,
    image: bool,
    resolution: BracketResolution,
}

/// The earliest delimiter the inline depth cap demotes to plain text, if any.
///
/// Used to span the nesting-depth diagnostic over the delimiter that was
/// actually demoted — a deep region containing no delimiters loses nothing and
/// so must not be reported at all.
fn first_demoted_delimiter(
    brackets: &[BracketMatch],
    emphasis: &[EmphasisPair],
    strikethrough: &[EmphasisPair],
) -> Option<std::ops::Range<usize>> {
    brackets
        .iter()
        .map(|bracket| bracket.start..bracket.end)
        .chain(
            emphasis
                .iter()
                .chain(strikethrough)
                .map(|pair| pair.open..pair.open + pair.width),
        )
        .min_by_key(|range| range.start)
}

/// Inline-parse `[start, end)` from the top: scans the protected code and
/// angle spans once for the whole range and hands them to the recursion, which
/// only ever narrows them to a sub-range.
fn parse_inlines(
    context: &mut InlineContext<'_>,
    start: usize,
    end: usize,
    owner: SyntaxIdentity,
    allow_links: bool,
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, ParseError> {
    let protected = {
        let source = context.text.shared();
        let mut protected = code_spans(source, start, end);
        protected.extend(angle_spans(source, start, end));
        protected
    };
    parse_inlines_within(context, start, end, owner, allow_links, 0, &protected)
}

/// The protected spans of `protected` that overlap `[start, end)`.
///
/// A code or angle span never straddles a delimiter the recursion descends
/// through — the delimiter would itself be protected and so never matched — so
/// narrowing the top-level scan is equivalent to rescanning the sub-range, at a
/// cost in the number of spans rather than in the length of the range.
fn protected_within(
    protected: &[std::ops::Range<usize>],
    start: usize,
    end: usize,
) -> Vec<std::ops::Range<usize>> {
    protected
        .iter()
        .filter(|range| range.start < end && range.end > start)
        .cloned()
        .collect()
}

fn parse_inlines_within(
    context: &mut InlineContext<'_>,
    start: usize,
    end: usize,
    owner: SyntaxIdentity,
    allow_links: bool,
    depth: usize,
    protected: &[std::ops::Range<usize>],
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, ParseError> {
    let source = context.text.shared();
    // Beyond `MD_MAX_INLINE_DEPTH`, stop matching strikethrough, emphasis,
    // and link delimiters: their arms below recurse into `parse_inlines`, so
    // clearing them here demotes the delimiters to plain text instead of
    // growing the recursion further. The candidates are still computed at the
    // cap (they do not recurse) so the diagnostic can be emitted only when a
    // delimiter is really demoted, spanning that delimiter.
    let capped = depth >= super::MD_MAX_INLINE_DEPTH;
    let mut brackets = bracket_matches(context, start, end, allow_links);
    let crosses_link_label = |brackets: &[BracketMatch], pair: &EmphasisPair| {
        brackets.iter().any(|bracket| {
            let open_inside = pair.open > bracket.open && pair.open < bracket.label_end;
            let close_inside = pair.close > bracket.open && pair.close < bracket.label_end;
            open_inside != close_inside
        })
    };
    // The protected spans were scanned once for the whole inline range by
    // `parse_inlines`; narrow that set to this level instead of rescanning.
    let protected = protected_within(protected, start, end);
    let mut emphasis: Vec<_> = emphasis_pairs(source, start, end, &protected)
        .into_iter()
        .filter(|pair| !crosses_link_label(&brackets, pair))
        .collect();
    let mut strikethrough: Vec<EmphasisPair> = if context.dialect.strikethrough() {
        strikethrough_pairs(source, start, end, &protected)
            .into_iter()
            .filter(|pair| !crosses_link_label(&brackets, pair))
            .collect()
    } else {
        Vec::new()
    };
    if capped {
        demote_delimiters_to_text(context, &mut brackets, &mut emphasis, &mut strikethrough)?;
    }
    // `brackets` is pushed in close-scan order (a LIFO pop on `]`), not
    // sorted by `start`, so a monotone cursor cannot be used for it; index by
    // start instead. `emphasis`/`strikethrough` are sorted ascending by
    // `open` (see their sort_by_key calls) and the loop below only ever
    // queries with a non-decreasing `at`, so both use a monotone cursor that
    // advances past entries whose `open` has already been passed.
    let bracket_by_start: HashMap<usize, usize> = brackets
        .iter()
        .enumerate()
        .map(|(index, matched)| (matched.start, index))
        .collect();
    let mut strikethrough_cursor = 0usize;
    let mut emphasis_cursor = 0usize;
    debug_assert!(
        emphasis.windows(2).all(|pair| pair[0].open <= pair[1].open),
        "emphasis pairs must be sorted ascending by `open` for the monotone cursor"
    );
    debug_assert!(
        strikethrough
            .windows(2)
            .all(|pair| pair[0].open <= pair[1].open),
        "strikethrough pairs must be sorted ascending by `open` for the monotone cursor"
    );
    let mut out = Vec::new();
    let mut at = start;
    let mut plain = start;
    let mut previous_at = start;
    while at < end {
        debug_assert!(
            at >= previous_at,
            "the inline cursor must not move backwards: the monotone \
             emphasis/strikethrough cursors would silently skip a pair"
        );
        previous_at = at;
        while strikethrough
            .get(strikethrough_cursor)
            .is_some_and(|pair| pair.open < at)
        {
            strikethrough_cursor += 1;
        }
        if let Some(pair) = strikethrough
            .get(strikethrough_cursor)
            .filter(|pair| pair.open == at)
        {
            flush(context.text, plain, at, &mut out)?;
            out.push(delimited_run(
                context,
                pair,
                Kind::StrikethroughDelimiterToken,
                owner,
                allow_links,
                depth,
                &protected,
            )?);
            at = pair.close + pair.width;
            plain = at;
            continue;
        }
        while emphasis
            .get(emphasis_cursor)
            .is_some_and(|pair| pair.open < at)
        {
            emphasis_cursor += 1;
        }
        if let Some(pair) = emphasis.get(emphasis_cursor).filter(|pair| pair.open == at) {
            flush(context.text, plain, at, &mut out)?;
            out.push(delimited_run(
                context,
                pair,
                Kind::EmphasisDelimiterToken,
                owner,
                allow_links,
                depth,
                &protected,
            )?);
            at = pair.close + pair.width;
            plain = at;
            continue;
        }

        let rest = &source[at..end];
        if rest.starts_with("\\\r\n") || rest.starts_with("\\\n") {
            let newline_start = at + 1;
            let newline_end = newline_start + newline_width(&source[newline_start..end]);
            if newline_end == end {
                flush(context.text, plain, newline_start, &mut out)?;
                out.push(tok(
                    context.text,
                    newline_start,
                    newline_end,
                    Kind::NewlineToken,
                )?);
                at = newline_end;
                plain = at;
                continue;
            }
            flush(context.text, plain, at, &mut out)?;
            out.push(node(
                Kind::HardLineBreak,
                vec![
                    tok(context.text, at, at + 1, Kind::BackslashToken)?,
                    tok(context.text, newline_start, newline_end, Kind::NewlineToken)?,
                ],
                Vec::new(),
            )?);
            at = newline_end;
            plain = at;
            continue;
        }
        if rest.as_bytes().first() == Some(&b' ') {
            let whitespace_end = at + rest.bytes().take_while(|byte| *byte == b' ').count();
            if whitespace_end - at >= 2
                && matches!(
                    source[whitespace_end..end].as_bytes().first(),
                    Some(b'\r' | b'\n')
                )
            {
                let newline_end = whitespace_end + newline_width(&source[whitespace_end..end]);
                if newline_end == end {
                    flush(context.text, plain, at, &mut out)?;
                    out.push(tok(
                        context.text,
                        at,
                        whitespace_end,
                        Kind::WhitespaceToken,
                    )?);
                    out.push(tok(
                        context.text,
                        whitespace_end,
                        newline_end,
                        Kind::NewlineToken,
                    )?);
                    at = newline_end;
                    plain = at;
                    continue;
                }
                flush(context.text, plain, at, &mut out)?;
                out.push(node(
                    Kind::HardLineBreak,
                    vec![
                        tok(context.text, at, whitespace_end, Kind::WhitespaceToken)?,
                        tok(
                            context.text,
                            whitespace_end,
                            newline_end,
                            Kind::NewlineToken,
                        )?,
                    ],
                    Vec::new(),
                )?);
                at = newline_end;
                plain = at;
                continue;
            }
        }
        if rest.starts_with('`') {
            let run = rest.bytes().take_while(|byte| *byte == b'`').count();
            if let Some(close) = code_span_close(source, at + run, end, run) {
                flush(context.text, plain, at, &mut out)?;
                out.push(node(
                    Kind::CodeSpan,
                    vec![
                        tok(context.text, at, at + run, Kind::CodeDelimiterToken)?,
                        tok(context.text, at + run, close, Kind::TextToken)?,
                        tok(context.text, close, close + run, Kind::CodeDelimiterToken)?,
                    ],
                    Vec::new(),
                )?);
                at = close + run;
                plain = at;
                continue;
            }
            at += run;
            continue;
        }
        if rest.starts_with('\\') {
            let next_start = at + 1;
            if let Some(next) = source[next_start..end].chars().next() {
                if next.is_ascii_punctuation() {
                    flush(context.text, plain, at, &mut out)?;
                    let next_end = next_start + next.len_utf8();
                    out.push(node(
                        Kind::Escape,
                        vec![
                            tok(context.text, at, next_start, Kind::BackslashToken)?,
                            tok(context.text, next_start, next_end, Kind::TextToken)?,
                        ],
                        Vec::new(),
                    )?);
                    at = next_end;
                    plain = at;
                    continue;
                }
            }
        }
        if rest.starts_with('&') {
            if let Some(relative) = rest.find(';').filter(|relative| *relative > 1) {
                let close = at + relative + 1;
                if let Some(decoded) = decode_entity(&source[at..close]) {
                    flush(context.text, plain, at, &mut out)?;
                    out.push(node(
                        Kind::Entity,
                        vec![tok(context.text, at, close, Kind::EntityToken)?],
                        vec![annotation(
                            ENTITY_VALUE_ID,
                            ENTITY_VALUE_ANNOTATION,
                            Some(decoded.into()),
                        )],
                    )?);
                    at = close;
                    plain = at;
                    continue;
                }
            }
        }
        if rest.starts_with('<') {
            if let Some(close) = push_angle_inline(context, at, end, owner, plain, &mut out)? {
                at = close;
                plain = at;
                continue;
            }
        }
        if let Some(matched) = bracket_by_start.get(&at).map(|&index| &brackets[index]) {
            let parsed = parse_link(context, matched, owner, allow_links, depth + 1, &protected)?;
            flush(context.text, plain, at, &mut out)?;
            out.push(GreenElement::Node(parsed.node));
            at = parsed.end;
            plain = at;
            continue;
        }
        if context.dialect.extended_autolinks() && allow_links {
            if let Some(candidate) = extended_autolink(source, start, at, end) {
                flush(context.text, plain, at, &mut out)?;
                let identity = SyntaxIdentity::fresh()?;
                out.push(GreenElement::Node(semantic_with_identity(
                    Kind::Autolink,
                    vec![tok(context.text, at, candidate.end, Kind::TextToken)?],
                    identity,
                    link_annotations(candidate.destination, at, candidate.end, owner),
                )?));
                at = candidate.end;
                plain = at;
                continue;
            }
        }
        if rest.starts_with('\r') || rest.starts_with('\n') {
            let newline_end = at + newline_width(rest);
            if newline_end == end {
                flush(context.text, plain, at, &mut out)?;
                out.push(tok(context.text, at, newline_end, Kind::NewlineToken)?);
                at = newline_end;
                plain = at;
                continue;
            }
            let content_end = plain + source[plain..at].trim_end_matches([' ', '\t']).len();
            flush(context.text, plain, content_end, &mut out)?;
            if content_end < at {
                out.push(tok(context.text, content_end, at, Kind::WhitespaceToken)?);
            }
            out.push(node(
                Kind::SoftLineBreak,
                vec![tok(context.text, at, newline_end, Kind::NewlineToken)?],
                Vec::new(),
            )?);
            at = newline_end;
            plain = at;
            continue;
        }
        at += source[at..end]
            .chars()
            .next()
            .expect("non-empty range")
            .len_utf8();
    }
    flush(context.text, plain, end, &mut out)?;
    Ok(out)
}

/// Past `MD_MAX_INLINE_DEPTH`, drops every delimiter whose arm would recurse,
/// so the delimiters are taken as plain text instead of growing the recursion
/// further.  Reports the first delimiter really demoted -- once per document,
/// spanning that delimiter.
fn demote_delimiters_to_text(
    context: &mut InlineContext<'_>,
    brackets: &mut Vec<BracketMatch>,
    emphasis: &mut Vec<EmphasisPair>,
    strikethrough: &mut Vec<EmphasisPair>,
) -> Result<(), ParseError> {
    if let Some(demoted) = first_demoted_delimiter(brackets, emphasis, strikethrough) {
        if !context.depth_diagnostic_emitted {
            context.depth_diagnostic_emitted = true;
            context.diagnostics.push(TreeDiagnostic {
                code: Diagnostic::NestingDepthExceeded,
                range: TextRange::new(
                    TextSize::try_from_usize(demoted.start).map_err(|_| {
                        ParseError::SourceTooLarge {
                            bytes: demoted.start,
                        }
                    })?,
                    TextSize::try_from_usize(demoted.end)
                        .map_err(|_| ParseError::SourceTooLarge { bytes: demoted.end })?,
                )
                .map_err(|_| ParseError::StructuralInvariant {
                    reason: "invalid inline nesting-depth range".into(),
                })?,
                severity: crate::SyntaxSeverity::Error,
                message: Arc::from(format!(
                    "Markdown inline nesting exceeds the supported depth of {}; deeper structure is treated as plain text.",
                    super::MD_MAX_INLINE_DEPTH
                )),
            });
        }
    }
    brackets.clear();
    emphasis.clear();
    strikethrough.clear();
    Ok(())
}

/// The content a matched delimiter pair wraps -- emphasis, strong, or
/// strikethrough -- as one node, with the opening and closing delimiters kept
/// as tokens of their own so the tree still spells the source.  The pair
/// already carries the node kind it opens and how wide its delimiters are.
fn delimited_run(
    context: &mut InlineContext<'_>,
    pair: &EmphasisPair,
    delimiter: Kind,
    owner: SyntaxIdentity,
    allow_links: bool,
    depth: usize,
    protected: &[std::ops::Range<usize>],
) -> Result<GreenElement<OkfMarkdownLanguage>, ParseError> {
    let mut children = vec![tok(
        context.text,
        pair.open,
        pair.open + pair.width,
        delimiter,
    )?];
    children.extend(parse_inlines_within(
        context,
        pair.open + pair.width,
        pair.close,
        owner,
        allow_links,
        depth + 1,
        protected,
    )?);
    children.push(tok(
        context.text,
        pair.close,
        pair.close + pair.width,
        delimiter,
    )?);
    node(pair.kind, children, Vec::new())
}

/// The inline forms an angle bracket can open -- an autolink or a run of raw
/// HTML -- flushed out with the pending plain text before them and the
/// annotations each carries.  Answers where the form ended, or `None` when the
/// bracket opens neither and its byte is left to be taken as text.
fn push_angle_inline(
    context: &mut InlineContext<'_>,
    at: usize,
    end: usize,
    owner: SyntaxIdentity,
    plain: usize,
    out: &mut Vec<GreenElement<OkfMarkdownLanguage>>,
) -> Result<Option<usize>, ParseError> {
    let source = context.text.shared();
    let Some(close) = html_close(source, at, end) else {
        return Ok(None);
    };
    let inside = &source[at + 1..close - 1];
    let kind = if is_autolink(inside) {
        Kind::Autolink
    } else if is_raw_html(&source[at..close]) {
        Kind::RawHtml
    } else {
        return Ok(None);
    };
    flush(context.text, plain, at, out)?;
    let identity = SyntaxIdentity::fresh()?;
    let mut annotations = if kind == Kind::Autolink {
        link_annotations(
            if inside.contains('@') && !inside.contains(':') {
                format!("mailto:{inside}")
            } else {
                inside.to_owned()
            },
            at + 1,
            close - 1,
            owner,
        )
    } else {
        Vec::new()
    };
    if kind == Kind::RawHtml && context.dialect.tag_filter() {
        let (tag_range, state) = super::gfm::classify_html(&source[at..close]);
        annotations.push(identity.metadata_annotation(super::gfm::HTML_TAG_FILTER, state.data()));
        if state == super::gfm::HtmlTagFilter::Disallowed {
            if let Some((tag_start, tag_end)) = tag_range {
                context.diagnostics.push(TreeDiagnostic {
                    code: Diagnostic::FilteredHtmlTag,
                    range: TextRange::new(
                        TextSize::try_from_usize(at + tag_start).map_err(|_| {
                            ParseError::SourceTooLarge {
                                bytes: at + tag_start,
                            }
                        })?,
                        TextSize::try_from_usize(at + tag_end).map_err(|_| {
                            ParseError::SourceTooLarge {
                                bytes: at + tag_end,
                            }
                        })?,
                    )
                    .map_err(|_| ParseError::StructuralInvariant {
                        reason: "invalid filtered HTML range".into(),
                    })?,
                    severity: crate::SyntaxSeverity::Error,
                    message: Arc::from("disallowed GFM HTML tag"),
                });
            }
        }
    }
    out.push(GreenElement::Node(semantic_with_identity(
        kind,
        vec![
            tok(context.text, at, at + 1, Kind::AutolinkOpenToken)?,
            tok(
                context.text,
                at + 1,
                close - 1,
                if kind == Kind::RawHtml {
                    Kind::HtmlToken
                } else {
                    Kind::TextToken
                },
            )?,
            tok(context.text, close - 1, close, Kind::AutolinkCloseToken)?,
        ],
        identity,
        annotations,
    )?));
    Ok(Some(close))
}

fn bracket_matches(
    context: &InlineContext<'_>,
    start: usize,
    end: usize,
    allow_links: bool,
) -> Vec<BracketMatch> {
    let source = context.text.shared();
    let mut openers = Vec::<BracketOpener>::new();
    let mut matches = Vec::new();
    let mut at = start;
    while at < end {
        let rest = &source[at..end];
        if rest.starts_with('\\') {
            at += 1;
            if at < end {
                at += source[at..end]
                    .chars()
                    .next()
                    .expect("non-empty escaped range")
                    .len_utf8();
            }
            continue;
        }
        if rest.starts_with('`') {
            let run = rest.bytes().take_while(|byte| *byte == b'`').count();
            if let Some(close) = code_span_close(source, at + run, end, run) {
                at = close + run;
                continue;
            }
        }
        if rest.starts_with('<') {
            if let Some(close) = html_close(source, at, end) {
                if is_autolink(&source[at + 1..close - 1]) || is_raw_html(&source[at..close]) {
                    at = close;
                    continue;
                }
            }
        }
        if rest.starts_with("![") {
            openers.push(BracketOpener {
                start: at,
                open: at + 1,
                image: true,
                active: true,
            });
            at += 2;
            continue;
        }
        if allow_links && rest.starts_with('[') {
            openers.push(BracketOpener {
                start: at,
                open: at,
                image: false,
                active: true,
            });
            at += 1;
            continue;
        }
        if rest.starts_with(']') {
            let Some(opener) = openers.pop() else {
                at += 1;
                continue;
            };
            if !opener.active {
                at += 1;
                continue;
            }
            let Some((close, resolution)) = bracket_match_end(context, opener.open, at, end) else {
                at += 1;
                continue;
            };
            matches.push(BracketMatch {
                start: opener.start,
                open: opener.open,
                label_end: at,
                end: close,
                image: opener.image,
                resolution,
            });
            if !opener.image {
                for earlier in &mut openers {
                    if !earlier.image {
                        earlier.active = false;
                    }
                }
            }
            at = close;
            continue;
        }
        at += rest
            .chars()
            .next()
            .expect("non-empty bracket scan range")
            .len_utf8();
    }
    matches
}

/// Whether the `]` at `label_end` closes a link, and if so where the link ends
/// and *what it is*.
///
/// This is the only place either question is answered. The resolution comes
/// back with the end offset because deciding the one decides the other:
/// separating them is what let two readings of the same bytes drift apart.
fn bracket_match_end(
    context: &InlineContext<'_>,
    open: usize,
    label_end: usize,
    end: usize,
) -> Option<(usize, BracketResolution)> {
    let source = context.text.shared();
    let label = &source[open + 1..label_end];
    let after_label = label_end + 1;
    if source[after_label..end].starts_with('(') {
        if let Some(parts) = inline_destination(source, after_label, end) {
            return Some((parts.close + 1, BracketResolution::Inline(parts)));
        }
    }
    let (reference_label, reference_end) = if source[after_label..end].starts_with('[') {
        let reference_end = find_unescaped(source, after_label + 1, end, ']')?;
        let explicit = &source[after_label + 1..reference_end];
        (
            if explicit.is_empty() { label } else { explicit },
            reference_end + 1,
        )
    } else {
        (label, after_label)
    };
    let normalized = normalize_label(reference_label)?;
    let definition = context.references.definitions.get(&normalized)?;
    Some((
        reference_end,
        BracketResolution::Reference {
            normalized,
            definition: definition.clone(),
        },
    ))
}

struct ParsedLink {
    node: GreenNode<OkfMarkdownLanguage>,
    end: usize,
}

/// Where a link points, and how the author said so.
struct LinkTarget {
    /// The destination the semantic layer reports, decoded.
    destination: String,
    /// The authored span the destination was read from -- inside this
    /// document for an inline link, inside the definition for a reference.
    range: std::ops::Range<usize>,
    /// The link's title, decoded, when it has one.
    title: Option<String>,
    /// The normalized label a reference link resolved through; `None` for an
    /// inline link.
    reference: Option<Arc<str>>,
    /// The authored spans of an inline `(destination "title")`, for emitting
    /// its tokens; `None` for a reference link.
    parts: Option<InlineDestination>,
    /// Which of the two forms this was, as the annotation spells it.
    kind: &'static str,
}

/// Reads a matched link's destination off the resolution the bracket scan
/// recorded.
///
/// Nothing is decided here. `bracket_match_end` already chose between the
/// inline and the reference form, already found the definition a reference
/// resolves through, and kept both; this only spells that choice the way the
/// tree and the annotations want it. So there is no second reading to
/// disagree with the first, and no failure to report: the function is total.
fn resolve_link_target(source: &str, matched: &BracketMatch) -> LinkTarget {
    match &matched.resolution {
        BracketResolution::Inline(parts) => LinkTarget {
            destination: decode_destination(&source[parts.destination.clone()]),
            range: parts.destination.clone(),
            title: parts
                .title
                .clone()
                .map(|range| decode_destination(&source[range.start + 1..range.end - 1])),
            reference: None,
            parts: Some(parts.clone()),
            kind: "inline",
        },
        BracketResolution::Reference {
            normalized,
            definition,
        } => LinkTarget {
            destination: definition.destination.to_string(),
            range: definition.destination_range.start().to_usize()
                ..definition.destination_range.end().to_usize(),
            title: definition.title.as_ref().map(ToString::to_string),
            reference: Some(Arc::clone(normalized)),
            parts: None,
            kind: "reference",
        },
    }
}

fn parse_link(
    context: &mut InlineContext<'_>,
    matched: &BracketMatch,
    owner: SyntaxIdentity,
    allow_links: bool,
    depth: usize,
    protected: &[std::ops::Range<usize>],
) -> Result<ParsedLink, ParseError> {
    let &BracketMatch {
        start,
        open,
        label_end,
        // Where the match ends is the scan's answer, not something re-read
        // here; it is also where the trailing `[...]` of a reference link
        // stops.
        end: close,
        image,
        ..
    } = matched;
    let after_label = label_end + 1;
    let LinkTarget {
        destination: semantic_destination,
        range: semantic_range,
        title: semantic_title,
        reference: normalized_reference,
        parts: destination_parts,
        kind: link_kind,
    } = resolve_link_target(context.text.shared(), matched);

    let mut children = Vec::new();
    if image {
        children.push(tok(context.text, start, open, Kind::ImageBangToken)?);
    }
    children.push(tok(context.text, open, open + 1, Kind::LinkLabelOpenToken)?);
    children.extend(parse_inlines_within(
        context,
        open + 1,
        label_end,
        owner,
        image && allow_links,
        depth,
        protected,
    )?);
    children.push(tok(
        context.text,
        label_end,
        label_end + 1,
        Kind::LinkLabelCloseToken,
    )?);
    if let Some(parts) = destination_parts {
        children.push(tok(
            context.text,
            after_label,
            after_label + 1,
            Kind::LinkDestinationOpenToken,
        )?);
        emit_link_gap(
            context.text,
            after_label + 1,
            parts.destination.start,
            &mut children,
        )?;
        children.push(tok(
            context.text,
            parts.destination.start,
            parts.destination.end,
            Kind::LinkDestinationToken,
        )?);
        if let Some(title) = parts.title {
            emit_link_gap(
                context.text,
                parts.destination.end,
                title.start,
                &mut children,
            )?;
            children.push(tok(
                context.text,
                title.start,
                title.end,
                Kind::LinkTitleToken,
            )?);
            emit_link_gap(context.text, title.end, parts.close, &mut children)?;
        } else {
            emit_link_gap(
                context.text,
                parts.destination.end,
                parts.close,
                &mut children,
            )?;
        }
        children.push(tok(
            context.text,
            parts.close,
            parts.close + 1,
            Kind::LinkDestinationCloseToken,
        )?);
    } else if after_label < close {
        children.push(tok(
            context.text,
            after_label,
            after_label + 1,
            Kind::LinkLabelOpenToken,
        )?);
        if after_label + 1 < close - 1 {
            children.push(tok(
                context.text,
                after_label + 1,
                close - 1,
                Kind::TextToken,
            )?);
        }
        children.push(tok(
            context.text,
            close - 1,
            close,
            Kind::LinkLabelCloseToken,
        )?);
    }

    let identity = SyntaxIdentity::fresh()?;
    let mut annotations = vec![
        annotation(
            DESTINATION_ID,
            DESTINATION_ANNOTATION,
            Some(semantic_destination.into()),
        ),
        annotation(
            DESTINATION_RANGE_ID,
            DESTINATION_RANGE_ANNOTATION,
            Some(format!("{}:{}", semantic_range.start, semantic_range.end).into()),
        ),
        annotation(
            LINK_KIND_ID,
            LINK_KIND_ANNOTATION,
            Some(Arc::from(link_kind)),
        ),
        annotation(
            OWNER_ID,
            OWNER_ANNOTATION,
            Some(Arc::from(owner.get().to_string())),
        ),
    ];
    if let Some(title) = semantic_title {
        annotations.push(annotation(TITLE_ID, TITLE_ANNOTATION, Some(title.into())));
    }
    if let Some(label) = normalized_reference {
        annotations.push(annotation(
            REFERENCE_LABEL_ID,
            REFERENCE_LABEL_ANNOTATION,
            Some(label.clone()),
        ));
        let owners = context.backlinks.entry(label).or_default();
        if !owners.contains(&owner) {
            owners.push(owner);
        }
    }
    Ok(ParsedLink {
        node: semantic_with_identity(
            if image { Kind::Image } else { Kind::Link },
            children,
            identity,
            annotations,
        )?,
        end: close,
    })
}

#[derive(Clone, Debug)]
struct InlineDestination {
    destination: std::ops::Range<usize>,
    title: Option<std::ops::Range<usize>>,
    close: usize,
}

fn link_annotations(
    destination: String,
    start: usize,
    end: usize,
    owner: SyntaxIdentity,
) -> Vec<SyntaxAnnotation> {
    vec![
        annotation(
            DESTINATION_ID,
            DESTINATION_ANNOTATION,
            Some(destination.into()),
        ),
        annotation(
            DESTINATION_RANGE_ID,
            DESTINATION_RANGE_ANNOTATION,
            Some(format!("{start}:{end}").into()),
        ),
        annotation(
            LINK_KIND_ID,
            LINK_KIND_ANNOTATION,
            Some(Arc::from("inline")),
        ),
        annotation(
            OWNER_ID,
            OWNER_ANNOTATION,
            Some(Arc::from(owner.get().to_string())),
        ),
    ]
}

fn inline_destination(source: &str, open: usize, end: usize) -> Option<InlineDestination> {
    let mut at = skip_whitespace(source, open + 1, end);
    let destination = if source[at..end].starts_with('<') {
        let close = find_unescaped(source, at + 1, end, '>')?;
        if source[at + 1..close].contains(['\r', '\n', '<']) {
            return None;
        }
        let result = at + 1..close;
        at = close + 1;
        result
    } else {
        let start = at;
        let mut depth = 0usize;
        while at < end {
            let ch = source[at..end].chars().next()?;
            if ch == '\\' {
                at += ch.len_utf8();
                if at < end {
                    at += source[at..end].chars().next()?.len_utf8();
                }
                continue;
            }
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            } else if is_link_whitespace(ch) && depth == 0 {
                break;
            }
            at += ch.len_utf8();
        }
        start..at
    };
    at = skip_whitespace(source, at, end);
    let title = if at < end && matches!(source.as_bytes()[at], b'"' | b'\'' | b'(') {
        let close_char = if source.as_bytes()[at] == b'(' {
            ')'
        } else {
            source[at..].chars().next()?
        };
        let close = find_unescaped(source, at + 1, end, close_char)?;
        let result = at..close + close_char.len_utf8();
        at = skip_whitespace(source, close + close_char.len_utf8(), end);
        Some(result)
    } else {
        None
    };
    (source[at..end].starts_with(')')).then_some(InlineDestination {
        destination,
        title,
        close: at,
    })
}

fn emit_link_gap(
    text: &SourceText,
    start: usize,
    end: usize,
    out: &mut Vec<GreenElement<OkfMarkdownLanguage>>,
) -> Result<(), ParseError> {
    if start < end {
        out.push(tok(text, start, end, Kind::WhitespaceToken)?);
    }
    Ok(())
}

fn emphasis_pairs(
    source: &str,
    start: usize,
    end: usize,
    protected: &[std::ops::Range<usize>],
) -> Vec<EmphasisPair> {
    let mut delimiters = Vec::<Delimiter>::new();
    let mut pairs = Vec::new();
    let mut at = start;
    while at < end {
        if let Some(range) = protected
            .iter()
            .find(|range| range.start <= at && at < range.end)
        {
            at = range.end;
            continue;
        }
        let byte = source.as_bytes()[at];
        if !matches!(byte, b'*' | b'_') || escaped(source, start, at) {
            at += source[at..end]
                .chars()
                .next()
                .expect("non-empty")
                .len_utf8();
            continue;
        }
        let length = source[at..end]
            .bytes()
            .take_while(|candidate| *candidate == byte)
            .count();
        let (can_open, can_close) = flanking(source, start, end, at, length, byte);
        let mut closer = Delimiter {
            marker: byte,
            start: at,
            length,
            remaining: length,
            can_open,
            can_close,
        };
        if can_close {
            while let Some(index) = delimiters.iter().rposition(|opener| {
                opener.marker == byte
                    && opener.can_open
                    && opener.remaining > 0
                    && !rule_of_three(*opener, closer)
            }) {
                delimiters.truncate(index + 1);
                let opener = &mut delimiters[index];
                let width = usize::from(opener.remaining >= 2 && closer.remaining >= 2) + 1;
                let open = opener.start + opener.remaining - width;
                let close = closer.start + closer.length - closer.remaining;
                pairs.push(EmphasisPair {
                    open,
                    close,
                    width,
                    kind: if width == 2 {
                        Kind::StrongEmphasis
                    } else {
                        Kind::Emphasis
                    },
                });
                opener.remaining -= width;
                closer.remaining -= width;
                if closer.remaining == 0 {
                    break;
                }
            }
        }
        if can_open && closer.remaining > 0 {
            delimiters.push(closer);
        }
        at += length;
    }
    pairs.sort_by_key(|pair| (pair.open, std::cmp::Reverse(pair.close)));
    pairs
}

fn strikethrough_pairs(
    source: &str,
    start: usize,
    end: usize,
    protected: &[std::ops::Range<usize>],
) -> Vec<EmphasisPair> {
    let mut openers = Vec::new();
    let mut pairs = Vec::new();
    let mut at = start;
    while at < end {
        if let Some(range) = protected
            .iter()
            .find(|range| range.start <= at && at < range.end)
        {
            at = range.end;
            continue;
        }
        if source.as_bytes()[at] != b'~' || escaped(source, start, at) {
            at += source[at..end]
                .chars()
                .next()
                .expect("non-empty")
                .len_utf8();
            continue;
        }
        let run = source[at..end]
            .bytes()
            .take_while(|byte| *byte == b'~')
            .count();
        if run != 2 {
            at += run;
            continue;
        }
        let previous = source[start..at].chars().next_back();
        let next = source[at + 2..end].chars().next();
        let previous_whitespace = previous.map_or(true, char::is_whitespace);
        let next_whitespace = next.map_or(true, char::is_whitespace);
        let previous_punctuation = previous.is_some_and(is_unicode_punctuation);
        let next_punctuation = next.is_some_and(is_unicode_punctuation);
        let can_open =
            !next_whitespace && (!next_punctuation || previous_whitespace || previous_punctuation);
        let can_close =
            !previous_whitespace && (!previous_punctuation || next_whitespace || next_punctuation);
        if can_close {
            if let Some(open) = openers.pop() {
                pairs.push(EmphasisPair {
                    open,
                    close: at,
                    width: 2,
                    kind: Kind::Strikethrough,
                });
                at += 2;
                continue;
            }
        }
        if can_open {
            openers.push(at);
        } else if !can_close {
            openers.clear();
        }
        at += 2;
    }
    pairs.sort_by_key(|pair| pair.open);
    pairs
}

fn flanking(
    source: &str,
    start: usize,
    end: usize,
    at: usize,
    length: usize,
    marker: u8,
) -> (bool, bool) {
    let previous = source[start..at].chars().next_back();
    let next = source[at + length..end].chars().next();
    let previous_whitespace = previous.map_or(true, char::is_whitespace);
    let next_whitespace = next.map_or(true, char::is_whitespace);
    let previous_punctuation = previous.is_some_and(is_unicode_punctuation);
    let next_punctuation = next.is_some_and(is_unicode_punctuation);
    let left =
        !next_whitespace && (!next_punctuation || previous_whitespace || previous_punctuation);
    let right =
        !previous_whitespace && (!previous_punctuation || next_whitespace || next_punctuation);
    if marker == b'_' {
        (
            left && (!right || previous_punctuation),
            right && (!left || next_punctuation),
        )
    } else {
        (left, right)
    }
}

fn is_unicode_punctuation(ch: char) -> bool {
    ch.is_ascii_punctuation() || (!ch.is_alphanumeric() && !ch.is_whitespace() && !ch.is_control())
}

fn rule_of_three(opener: Delimiter, closer: Delimiter) -> bool {
    (opener.can_close || closer.can_open)
        && (opener.length + closer.length) % 3 == 0
        && (opener.length % 3 != 0 || closer.length % 3 != 0)
}

fn code_spans(source: &str, start: usize, end: usize) -> Vec<std::ops::Range<usize>> {
    let mut spans = Vec::new();
    let mut at = start;
    while at < end {
        if source.as_bytes()[at] == b'`' {
            let run = source[at..end]
                .bytes()
                .take_while(|byte| *byte == b'`')
                .count();
            if let Some(close) = code_span_close(source, at + run, end, run) {
                spans.push(at..close + run);
                at = close + run;
                continue;
            }
            at += run;
        } else {
            at += source[at..end]
                .chars()
                .next()
                .expect("non-empty")
                .len_utf8();
        }
    }
    spans
}

fn angle_spans(source: &str, start: usize, end: usize) -> Vec<std::ops::Range<usize>> {
    let mut spans = Vec::new();
    let mut at = start;
    while at < end {
        let Some(relative) = source[at..end].find('<') else {
            break;
        };
        at += relative;
        let Some(close) = html_close(source, at, end) else {
            break;
        };
        if is_autolink(&source[at + 1..close - 1]) || is_raw_html(&source[at..close]) {
            spans.push(at..close);
        }
        at = close;
    }
    spans
}

fn html_close(source: &str, start: usize, end: usize) -> Option<usize> {
    let rest = &source[start..end];
    if rest.starts_with("<!-->") || rest.starts_with("<!--->") {
        return rest.find('>').map(|relative| start + relative + 1);
    }
    for (open, close) in [("<!--", "-->"), ("<![CDATA[", "]]>"), ("<?", "?>")] {
        if let Some(after_open) = rest.strip_prefix(open) {
            return after_open
                .find(close)
                .map(|relative| start + open.len() + relative + close.len());
        }
    }
    let mut quote = None;
    let mut at = start + 1;
    while at < end {
        let character = source[at..end].chars().next()?;
        match (quote, character) {
            (Some(open), close) if open == close => quote = None,
            (None, '"' | '\'') => quote = Some(character),
            (None, '>') => return Some(at + 1),
            _ => {}
        }
        at += character.len_utf8();
    }
    None
}

fn code_span_close(source: &str, mut at: usize, end: usize, wanted: usize) -> Option<usize> {
    while at < end {
        let relative = source[at..end].find('`')?;
        at += relative;
        let run = source[at..end]
            .bytes()
            .take_while(|byte| *byte == b'`')
            .count();
        if run == wanted {
            return Some(at);
        }
        at += run;
    }
    None
}

pub(crate) fn decode_entity(spelling: &str) -> Option<String> {
    let text = scan_text_entities(spelling);
    (text != spelling && !text.is_empty()).then_some(text)
}

fn is_autolink(value: &str) -> bool {
    let has_whitespace = value.chars().any(char::is_whitespace);
    let email = value.contains('@') && !value.contains('\\') && !has_whitespace;
    let uri = !has_whitespace
        && value.split_once(':').is_some_and(|(scheme, rest)| {
            !rest.is_empty()
                && scheme.len() >= 2
                && scheme.len() <= 32
                && scheme.bytes().enumerate().all(|(index, byte)| {
                    byte.is_ascii_alphanumeric()
                        || (index > 0 && matches!(byte, b'+' | b'-' | b'.'))
                })
        });
    email || uri
}

/// Whether `candidate` — the whole `<...>` slice, angle brackets included — is
/// raw inline HTML.
///
/// Taking the bracketed slice rather than its interior lets the scanner be
/// handed the source directly, with no per-candidate `String` on the parse
/// path.
fn is_raw_html(candidate: &str) -> bool {
    debug_assert!(candidate.starts_with('<') && candidate.ends_with('>'));
    let value = &candidate[1..candidate.len() - 1];
    if let Some(comment) = value.strip_prefix("!--") {
        return value.ends_with("--") && !comment.starts_with('>') && !comment.starts_with("->");
    }
    if (value.starts_with("![CDATA[") && value.ends_with("]]"))
        || (value.starts_with('?') && value.ends_with('?'))
        || value
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("!doctype"))
    {
        return true;
    }
    scan_is_inline_html(candidate)
}

struct ExtendedAutolink {
    end: usize,
    destination: String,
}

fn extended_autolink(
    source: &str,
    range_start: usize,
    at: usize,
    end: usize,
) -> Option<ExtendedAutolink> {
    let rest = &source[at..end];
    let scheme_len = if rest
        .get(..7)
        .is_some_and(|value| value.eq_ignore_ascii_case("http://"))
    {
        Some(7)
    } else if rest
        .get(..8)
        .is_some_and(|value| value.eq_ignore_ascii_case("https://"))
    {
        Some(8)
    } else if rest
        .get(..6)
        .is_some_and(|value| value.eq_ignore_ascii_case("ftp://"))
    {
        Some(6)
    } else {
        None
    };
    let url = scheme_len.is_some();
    let www = rest.starts_with("www.");
    let boundary = at == range_start
        || source[..at]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_whitespace() || matches!(ch, '(' | '*' | '_' | '~'));
    if (url || www) && boundary {
        let raw_len = rest
            .bytes()
            .take_while(|byte| {
                !byte.is_ascii_whitespace() && !matches!(byte, b'<' | b'>' | b'`' | b'[' | b']')
            })
            .count();
        let close = trim_extended_tail(source, at, at + raw_len);
        let value = &source[at..close];
        let authority = if let Some(scheme_len) = scheme_len {
            &value[scheme_len..]
        } else {
            value
        };
        let domain_and_port = authority.split(['/', '?', '#']).next().unwrap_or_default();
        let (domain, valid_port) =
            domain_and_port
                .rsplit_once(':')
                .map_or((domain_and_port, true), |(domain, port)| {
                    (
                        domain,
                        !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()),
                    )
                });
        if valid_port && valid_domain(domain.strip_prefix("www.").unwrap_or(domain)) {
            return Some(ExtendedAutolink {
                end: close,
                destination: if www {
                    format!("http://{value}")
                } else {
                    value.to_owned()
                },
            });
        }
        return None;
    }
    let first = rest.as_bytes().first().copied()?;
    let boundary = at == range_start
        || source[..at]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_whitespace() || matches!(ch, '*' | '_' | '~' | '('));
    if !first.is_ascii_alphanumeric() || !boundary {
        return None;
    }
    let local_len = rest
        .bytes()
        .take_while(|byte| email_local_byte(*byte))
        .count();
    if rest.as_bytes().get(local_len) != Some(&b'@') {
        return None;
    }
    let local = &rest[..local_len];
    let domain_start = local_len + 1;
    let mut domain_len = rest[domain_start..]
        .bytes()
        .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        .count();
    while domain_len > 0 && rest.as_bytes()[domain_start + domain_len - 1] == b'.' {
        domain_len -= 1;
    }
    if matches!(
        rest.as_bytes().get(domain_start + domain_len),
        Some(b'_' | b'-')
    ) {
        return None;
    }
    let domain = &rest[domain_start..domain_start + domain_len];
    if local.is_empty()
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
        || !valid_domain(domain)
    {
        return None;
    }
    let value = &rest[..domain_start + domain_len];
    Some(ExtendedAutolink {
        end: at + value.len(),
        destination: format!("mailto:{value}"),
    })
}

fn email_local_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+')
}

fn trim_extended_tail(source: &str, start: usize, mut end: usize) -> usize {
    if let Some(amp) = source[start..end].rfind('&').map(|offset| start + offset) {
        let tail = &source[amp + 1..end];
        if tail.ends_with(';')
            && tail[..tail.len() - 1]
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric())
        {
            end = amp;
        }
    }
    while end > start
        && matches!(
            source.as_bytes()[end - 1],
            b'?' | b'!' | b'.' | b',' | b':' | b'*' | b'_' | b'~'
        )
    {
        end -= 1;
    }
    while end > start && source.as_bytes()[end - 1] == b')' {
        let value = &source[start..end];
        if value.bytes().filter(|byte| *byte == b')').count()
            <= value.bytes().filter(|byte| *byte == b'(').count()
        {
            break;
        }
        end -= 1;
    }
    end
}

fn valid_domain(domain: &str) -> bool {
    if domain.len() > 253
        || domain
            .bytes()
            .any(|byte| !byte.is_ascii_alphanumeric() && byte != b'-' && byte != b'.')
    {
        return false;
    }
    let mut labels = domain.split('.');
    let Some(first) = labels.next() else {
        return false;
    };
    let rest: Vec<_> = labels.collect();
    if first.is_empty() || rest.is_empty() {
        return false;
    }
    let valid_label = |label: &str| {
        !label.is_empty()
            && label.len() <= 63
            && label
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            && label
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric)
    };
    valid_label(first)
        && rest.iter().all(|label| valid_label(label))
        && rest
            .last()
            .is_some_and(|label| label.bytes().all(|byte| byte.is_ascii_alphabetic()))
}

pub(super) fn find_unescaped(
    source: &str,
    mut at: usize,
    end: usize,
    wanted: char,
) -> Option<usize> {
    while at < end {
        let ch = source[at..end].chars().next()?;
        if ch == wanted && !escaped(source, 0, at) {
            return Some(at);
        }
        at += ch.len_utf8();
    }
    None
}

fn escaped(source: &str, start: usize, at: usize) -> bool {
    source[start..at]
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'\\')
        .count()
        % 2
        == 1
}

fn skip_whitespace(source: &str, mut at: usize, end: usize) -> usize {
    while at < end {
        let ch = source[at..end].chars().next().expect("non-empty");
        if !is_link_whitespace(ch) {
            break;
        }
        at += ch.len_utf8();
    }
    at
}

fn is_link_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\r' | '\n')
}

fn newline_width(value: &str) -> usize {
    if value.starts_with("\r\n") {
        2
    } else {
        1
    }
}

fn flush(
    text: &SourceText,
    start: usize,
    end: usize,
    out: &mut Vec<GreenElement<OkfMarkdownLanguage>>,
) -> Result<(), ParseError> {
    if start >= end {
        return Ok(());
    }
    let source = text.shared();
    let mut at = start;
    while at < end {
        let whitespace = source[at..end]
            .chars()
            .next()
            .expect("non-empty")
            .is_whitespace();
        let mut next = at;
        while next < end
            && source[next..end]
                .chars()
                .next()
                .expect("non-empty")
                .is_whitespace()
                == whitespace
        {
            next += source[next..end]
                .chars()
                .next()
                .expect("non-empty")
                .len_utf8();
        }
        out.push(node(
            Kind::Text,
            vec![tok(
                text,
                at,
                next,
                if whitespace {
                    Kind::WhitespaceToken
                } else {
                    Kind::TextToken
                },
            )?],
            Vec::new(),
        )?);
        at = next;
    }
    Ok(())
}

fn node(
    kind: Kind,
    children: Vec<GreenElement<OkfMarkdownLanguage>>,
    annotations: Vec<SyntaxAnnotation>,
) -> Result<GreenElement<OkfMarkdownLanguage>, ParseError> {
    let identity = SyntaxIdentity::fresh()?;
    Ok(GreenElement::Node(semantic_with_identity(
        kind,
        children,
        identity,
        annotations,
    )?))
}

fn semantic_with_identity(
    kind: Kind,
    children: Vec<GreenElement<OkfMarkdownLanguage>>,
    identity: SyntaxIdentity,
    mut annotations: Vec<SyntaxAnnotation>,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    annotations.insert(0, identity.annotation());
    GreenFactory::new()
        .node_with_annotations(kind, children, annotations.into())
        .map_err(|_| ParseError::WidthOverflow)
}

fn annotation(id: u64, kind: &'static str, data: Option<Arc<str>>) -> SyntaxAnnotation {
    SyntaxAnnotation::new(NonZeroU64::new(id).expect("non-zero"), kind, data)
}

fn tok(
    text: &SourceText,
    start: usize,
    end: usize,
    kind: Kind,
) -> Result<GreenElement<OkfMarkdownLanguage>, ParseError> {
    let range =
        TextRange::new(size(start)?, size(end)?).map_err(|_| ParseError::StructuralInvariant {
            reason: "reversed inline range".into(),
        })?;
    Ok(GreenElement::Token(
        GreenFactory::new()
            .token(
                kind,
                GreenText::SourceSlice {
                    source: text.clone(),
                    range,
                },
                [],
                [],
            )
            .map_err(|_| ParseError::WidthOverflow)?,
    ))
}

fn size(value: usize) -> Result<TextSize, ParseError> {
    TextSize::try_from_usize(value).map_err(|_| ParseError::SourceTooLarge { bytes: value })
}

pub(crate) fn link_annotation<'a>(
    annotations: &'a [SyntaxAnnotation],
    kind: &str,
) -> Option<&'a str> {
    annotations
        .iter()
        .find(|annotation| annotation.kind() == kind)
        .and_then(SyntaxAnnotation::data)
}

pub(crate) fn destination_annotation() -> &'static str {
    DESTINATION_ANNOTATION
}
pub(crate) fn destination_range_annotation() -> &'static str {
    DESTINATION_RANGE_ANNOTATION
}
pub(crate) fn title_annotation() -> &'static str {
    TITLE_ANNOTATION
}
pub(crate) fn link_kind_annotation() -> &'static str {
    LINK_KIND_ANNOTATION
}
pub(crate) fn reference_label_annotation() -> &'static str {
    REFERENCE_LABEL_ANNOTATION
}
pub(crate) fn owner_annotation() -> &'static str {
    OWNER_ANNOTATION
}
pub(crate) fn entity_value_annotation() -> &'static str {
    ENTITY_VALUE_ANNOTATION
}

#[cfg(test)]
mod tests {
    use crate::{parse_markdown, DocumentRevision, OkfSyntaxDiagnosticCode as Diagnostic};

    /// The protected set the recursion used to rescan for every sub-range.
    fn protected_scan(source: &str, start: usize, end: usize) -> Vec<std::ops::Range<usize>> {
        let mut protected = super::code_spans(source, start, end);
        protected.extend(super::angle_spans(source, start, end));
        protected.sort_by_key(|range| (range.start, range.end));
        protected
    }

    /// `protected_within` narrows the one top-level scan instead of
    /// rescanning each sub-range; pin that the narrowed set is what a rescan
    /// of that sub-range would have produced, over the sub-ranges the
    /// recursion actually descends into (emphasis and strikethrough
    /// interiors), including unpaired backticks and angle spans that sit
    /// beside or straddle an emphasis run.
    #[test]
    fn narrowed_protected_set_matches_a_rescan_of_the_sub_range() {
        for source in [
            "*a `code` b* c",
            "*a ` b* c",
            "a ` *b* ` *c*",
            "*a `` b ` c `` d*",
            "``a `b` c`` *d*",
            "*a <https://example.com/x> b*",
            "*a <b> c* <d>",
            "*a <not closed b* c",
            "~~a `b` c~~ *d `e` f*",
            "*a **b `c` d** e*",
            "`*a*` *b* `*c*`",
        ] {
            let top = protected_scan(source, 0, source.len());
            let mut interiors = Vec::new();
            for pair in super::emphasis_pairs(source, 0, source.len(), &top) {
                interiors.push(pair.open + pair.width..pair.close);
            }
            for pair in super::strikethrough_pairs(source, 0, source.len(), &top) {
                interiors.push(pair.open + 2..pair.close);
            }
            assert!(
                !interiors.is_empty(),
                "fixture {source:?} exercises no sub-range"
            );
            for interior in interiors {
                let mut narrowed = super::protected_within(&top, interior.start, interior.end);
                narrowed.sort_by_key(|range| (range.start, range.end));
                assert_eq!(
                    narrowed,
                    protected_scan(source, interior.start, interior.end),
                    "narrowing disagrees with a rescan of {interior:?} in {source:?}"
                );
            }
        }
    }

    /// A hostile paragraph of 10,000 `*` markers on each side of a single
    /// word peels off nested `Emphasis`/`StrongEmphasis` pairs from the
    /// single delimiter run, exactly like CommonMark's own nesting rule; the
    /// depth must not overflow the stack (bounded by `MD_MAX_INLINE_DEPTH`),
    /// must still round-trip the exact source, and must record exactly one
    /// nesting-depth diagnostic.
    #[test]
    fn deeply_nested_emphasis_is_capped_with_one_diagnostic() {
        let depth = 10_000;
        let source = "*".repeat(depth) + "x" + &"*".repeat(depth) + "\n";
        let snapshot = parse_markdown(
            DocumentRevision::INITIAL,
            crate::SourceText::new(&source).unwrap(),
            crate::MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        assert_eq!(snapshot.tree().write_to_string(), source);
        assert_eq!(
            snapshot
                .diagnostics()
                .iter()
                .filter(|diagnostic| diagnostic.code == Diagnostic::NestingDepthExceeded)
                .count(),
            1
        );
    }

    /// Same shape, but with strikethrough delimiters, which recurse through
    /// the same `parse_inlines` cap. Each `~~` must be its own delimiter run
    /// (strikethrough requires a run of exactly 2), so openers and closers
    /// are separated by a letter and built up on a stack via CommonMark's
    /// strikethrough flanking rules rather than by concatenating runs.
    #[test]
    fn deeply_nested_strikethrough_is_capped_with_one_diagnostic() {
        let depth = 10_000;
        let source = "y".to_string() + &" ~~a".repeat(depth) + "x" + &"a~~ ".repeat(depth) + "\n";
        let snapshot = parse_markdown(
            DocumentRevision::INITIAL,
            crate::SourceText::new(&source).unwrap(),
            crate::MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        assert_eq!(snapshot.tree().write_to_string(), source);
        assert_eq!(
            snapshot
                .diagnostics()
                .iter()
                .filter(|diagnostic| diagnostic.code == Diagnostic::NestingDepthExceeded)
                .count(),
            1
        );
    }

    /// A document nested to exactly one below the cap must be unaffected: no
    /// diagnostic, and the source still round-trips exactly. A single run of
    /// `2 * depth` emphasis markers peels off one `StrongEmphasis` pair per
    /// two markers consumed, so `depth` levels need `2 * depth` markers on
    /// each side.
    #[test]
    fn nested_emphasis_below_cap_is_unaffected() {
        let depth = super::super::MD_MAX_INLINE_DEPTH - 1;
        let source = "*".repeat(depth * 2) + "x" + &"*".repeat(depth * 2) + "\n";
        let snapshot = parse_markdown(
            DocumentRevision::INITIAL,
            crate::SourceText::new(&source).unwrap(),
            crate::MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        assert_eq!(snapshot.tree().write_to_string(), source);
        assert!(!snapshot
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == Diagnostic::NestingDepthExceeded));
    }

    /// A flat (non-nested) run of thousands of link-style bracket pairs
    /// exercises the per-byte `at` lookup against `brackets` directly — these
    /// pairs sit side by side rather than recursing, so the nesting-depth cap
    /// never engages and the whole cost lands on the bracket lookup inside
    /// `parse_inlines`'s main loop. Before the lookup was indexed, this shape
    /// was O(bytes × pairs); the test's real assertion is that it still
    /// completes and round-trips, not a timing measurement.
    #[test]
    fn many_flat_link_brackets_round_trip() {
        let count = 5_000;
        let source = "[a](u) ".repeat(count) + "\n";
        let snapshot = parse_markdown(
            DocumentRevision::INITIAL,
            crate::SourceText::new(&source).unwrap(),
            crate::MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        assert_eq!(snapshot.tree().write_to_string(), source);
        assert!(!snapshot
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == Diagnostic::NestingDepthExceeded));
    }

    /// Reaching the cap is not itself a defect: if the region at the cap holds
    /// no emphasis, link, or strikethrough delimiter, nothing is demoted to
    /// plain text and no diagnostic may be reported.
    #[test]
    fn nesting_at_cap_without_delimiters_reports_nothing() {
        let depth = super::super::MD_MAX_INLINE_DEPTH;
        let source = "*".repeat(depth * 2) + "x" + &"*".repeat(depth * 2) + "\n";
        let snapshot = parse_markdown(
            DocumentRevision::INITIAL,
            crate::SourceText::new(&source).unwrap(),
            crate::MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        assert_eq!(snapshot.tree().write_to_string(), source);
        assert!(!snapshot
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == Diagnostic::NestingDepthExceeded));
    }

    /// When a delimiter really is demoted, the diagnostic spans that
    /// delimiter, not the whole inner region.
    #[test]
    fn demoted_delimiter_diagnostic_spans_the_delimiter() {
        let depth = super::super::MD_MAX_INLINE_DEPTH;
        let inner = "a *b* a";
        let source = "*".repeat(depth * 2) + inner + &"*".repeat(depth * 2) + "\n";
        let snapshot = parse_markdown(
            DocumentRevision::INITIAL,
            crate::SourceText::new(&source).unwrap(),
            crate::MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        assert_eq!(snapshot.tree().write_to_string(), source);
        let diagnostics: Vec<_> = snapshot
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code == Diagnostic::NestingDepthExceeded)
            .collect();
        assert_eq!(diagnostics.len(), 1);
        let range = diagnostics[0].range;
        assert!(
            range.end().to_usize() - range.start().to_usize() < inner.len(),
            "diagnostic spans the whole inner region, not the demoted delimiter"
        );
    }
}
