use std::collections::HashSet;
use std::sync::Arc;

use super::scalar::{scan_quoted_scalar, QuotedScalarScan};
use crate::{
    shell::{ParseError, ParsedShellWindow, ShellParse, ShellWindow, ShellWindowKind},
    GreenElement, GreenFactory, GreenText, GreenTrivia, MarkdownDialect, OkfMarkdownLanguage,
    OkfMarkdownSyntaxKind, OkfSyntaxDiagnosticCode, SourceText, SyntaxIdentity, SyntaxSeverity,
    SyntaxTree, TextRange, TextSize, TreeDiagnostic, TriviaKind,
};

pub(crate) fn parse(text: SourceText, dialect: MarkdownDialect) -> Result<ShellParse, ParseError> {
    let structure = match crate::markdown::shell_map(&text, dialect) {
        Ok(structure) => Arc::new(structure),
        Err(ParseError::InvalidRange { .. }) => Arc::new(crate::markdown::ShellStructure {
            headings: [].into(),
            nested_headings: [].into(),
            protected_ranges: [].into(),
            list_item_lines: [].into(),
            tab_indented_item_lines: [].into(),
            opaque_ranges: [].into(),
            dialect,
        }),
        Err(error) => return Err(error),
    };
    parse_with_structure(text, dialect, structure)
}

pub(crate) fn parse_with_structure(
    text: SourceText,
    dialect: MarkdownDialect,
    structure: Arc<crate::markdown::ShellStructure>,
) -> Result<ShellParse, ParseError> {
    let factory = GreenFactory::<OkfMarkdownLanguage>::new();
    let source = text.shared();
    let mut children = Vec::new();
    let mut diagnostics = Vec::new();
    let mut at = 0;

    if let Some(class) = classify_frontmatter(&text, &structure)? {
        let (node, end) = frontmatter(&factory, &text, source, class, &mut diagnostics)?;
        children.push(GreenElement::Node(node));
        at = end;
    }
    let eof_trivia_start = trailing_eof_whitespace_start(source, at);
    if at < eof_trivia_start {
        let blocks = crate::markdown::block::parse(&text, dialect, at, eof_trivia_start)?;
        children.extend(crate::markdown::block::wrap_waml_sections(
            &factory,
            source,
            dialect,
            at,
            blocks.root.children().to_vec(),
        )?);
        diagnostics.extend(blocks.diagnostics.iter().cloned());
    }
    let eof_leading = trivia(&factory, &text, eof_trivia_start, source.len())?;
    children.push(GreenElement::Token(
        factory
            .missing_token_with_leading(OkfMarkdownSyntaxKind::EndOfFileToken, eof_leading)
            .map_err(|_| ParseError::WidthOverflow)?,
    ));
    let root = factory
        .node(OkfMarkdownSyntaxKind::Root, children)
        .map_err(|_| ParseError::WidthOverflow)?;
    let tree = Arc::new(SyntaxTree::new(root, diagnostics.into(), dialect));
    let structure = Arc::new(crate::markdown::from_tree(&tree, source)?);
    Ok(ShellParse { tree, structure })
}

#[derive(Clone, Copy)]
struct FrontmatterClass {
    range: TextRange,
    open: Line,
    close: Option<Line>,
    entries_end: usize,
    recovered: bool,
}

pub(crate) fn frontmatter_range(
    text: &SourceText,
    structure: &crate::markdown::ShellStructure,
) -> Result<Option<TextRange>, ParseError> {
    Ok(classify_frontmatter(text, structure)?.map(|class| class.range))
}

fn classify_frontmatter(
    text: &SourceText,
    structure: &crate::markdown::ShellStructure,
) -> Result<Option<FrontmatterClass>, ParseError> {
    if !structure.dialect.waml_frontmatter() {
        return Ok(None);
    }
    let source = text.shared();
    let bom = usize::from(source.starts_with('\u{feff}')) * 3;
    let open = line_at(source, 0, source.len());
    if open.start != 0 || source[bom..open.significant_end] != *"---" {
        return Ok(None);
    }

    let boundary = structure
        .headings
        .iter()
        .find(|heading| heading.level <= 2 && heading.range.start().to_usize() >= open.end)
        .map(|heading| heading.range.start().to_usize())
        .unwrap_or(source.len());
    let close = frontmatter_close_fence_line(source, open.end, boundary);
    let recovered = close.is_none();
    let entries_end = close.map_or(boundary, |line| line.start);
    if recovered && !plausible_unclosed_frontmatter(source, open.end, entries_end) {
        return Ok(None);
    }
    let end = close.map_or_else(
        || {
            lines(source, open.end, entries_end)
                .map(|(start, end)| structured_end(line_at(source, start, end)))
                .last()
                .unwrap_or(open.end)
        },
        structured_end,
    );
    Ok(Some(FrontmatterClass {
        range: TextRange::new(size(open.start)?, size(end)?).map_err(|_| {
            ParseError::StructuralInvariant {
                reason: "frontmatter classifier produced a reversed range".into(),
            }
        })?,
        open,
        close,
        entries_end,
        recovered,
    }))
}

fn frontmatter(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    class: FrontmatterClass,
    diagnostics: &mut Vec<TreeDiagnostic<OkfSyntaxDiagnosticCode>>,
) -> Result<(crate::GreenNode<OkfMarkdownLanguage>, usize), ParseError> {
    let FrontmatterClass {
        open,
        close,
        entries_end,
        recovered,
        range,
    } = class;

    let mut children = line_tokens(
        factory,
        text,
        source,
        open,
        OkfMarkdownSyntaxKind::FrontmatterOpenFence,
    )?;
    let mut clean = !recovered;
    let (mapping, mapping_clean, entries_consumed_end) =
        build_frontmatter_mapping(factory, text, source, open.end, entries_end, diagnostics)?;
    clean = clean && mapping_clean;
    children.push(GreenElement::Node(mapping));
    if let Some(close) = close {
        children.extend(line_tokens(
            factory,
            text,
            source,
            close,
            OkfMarkdownSyntaxKind::FrontmatterCloseFence,
        )?);
    } else {
        children.push(GreenElement::Token(
            factory.missing_token(OkfMarkdownSyntaxKind::FrontmatterCloseFence),
        ));
        diagnostics.push(diagnostic(
            OkfSyntaxDiagnosticCode::MissingFrontmatterFence,
            entries_consumed_end,
            entries_consumed_end,
            "missing frontmatter close fence",
        ));
    }
    if !clean {
        diagnostics.push(diagnostic(
            OkfSyntaxDiagnosticCode::FrontmatterNotClean,
            open.end,
            entries_consumed_end,
            "frontmatter required recovery",
        ));
    }
    Ok((
        identified_node(factory, OkfMarkdownSyntaxKind::Frontmatter, children)?,
        range.end().to_usize(),
    ))
}

pub(crate) fn parse_window(
    text: &SourceText,
    structure: &crate::markdown::ShellStructure,
    window: ShellWindow,
) -> Result<ParsedShellWindow, ParseError> {
    let source = text.shared();
    let start = window.range.start().to_usize();
    let end = window.range.end().to_usize();
    if start > end || end > source.len() {
        return Err(ParseError::InvalidRange {
            range: window.range,
        });
    }
    // A full parse peels trailing spaces and tabs at the end of the document into
    // end-of-file trivia. Only the tail window emits that token, so a block window
    // reaching the document end over such whitespace cannot reproduce the full parse.
    if matches!(
        window.kind,
        ShellWindowKind::Heading | ShellWindowKind::MarkdownRegion
    ) && end == source.len()
        && trailing_eof_whitespace_start(source, start) < end
    {
        return Err(window_not_consumed());
    }
    let factory = GreenFactory::<OkfMarkdownLanguage>::new();
    let mut diagnostics = Vec::new();
    let elements = match window.kind {
        ShellWindowKind::Frontmatter => {
            let Some(class) = classify_frontmatter(text, structure)? else {
                return Err(window_not_consumed());
            };
            if class.range != window.range {
                return Err(window_not_consumed());
            }
            let (node, _) = frontmatter(&factory, text, source, class, &mut diagnostics)?;
            vec![GreenElement::Node(node)]
        }
        ShellWindowKind::Heading => {
            let mut candidates = structure
                .headings
                .iter()
                .chain(structure.nested_headings.iter())
                .filter(|heading| heading.range == window.range);
            let Some(_) = candidates.next() else {
                return Err(window_not_consumed());
            };
            if candidates.next().is_some() {
                return Err(window_not_consumed());
            }
            let elements =
                parse_window_blocks(text, structure.dialect, start, end, &mut diagnostics)?;
            if elements.len() != 1
                || !matches!(
                    &elements[0],
                    GreenElement::Node(node)
                        if matches!(
                            node.kind(),
                            OkfMarkdownSyntaxKind::AtxHeading
                                | OkfMarkdownSyntaxKind::SetextHeading
                        )
                )
            {
                return Err(window_not_consumed());
            }
            elements
        }
        ShellWindowKind::MarkdownRegion => {
            parse_window_blocks(text, structure.dialect, start, end, &mut diagnostics)?
        }
        ShellWindowKind::Tail => {
            if end != source.len() {
                return Err(window_not_consumed());
            }
            let eof_trivia_start = trailing_eof_whitespace_start(source, start);
            let mut elements = Vec::new();
            if start < eof_trivia_start {
                let blocks = crate::markdown::block::parse(
                    text,
                    structure.dialect,
                    start,
                    eof_trivia_start,
                )?;
                diagnostics.extend(blocks.diagnostics.iter().cloned());
                elements.extend(blocks.root.children().iter().cloned());
            }
            let eof_leading = trivia(&factory, text, eof_trivia_start, source.len())?;
            elements.push(GreenElement::Token(
                factory
                    .missing_token_with_leading(OkfMarkdownSyntaxKind::EndOfFileToken, eof_leading)
                    .map_err(|_| ParseError::WidthOverflow)?,
            ));
            elements
        }
    };
    let consumed = elements
        .iter()
        .map(element_width)
        .try_fold(size(0)?, |sum, width| {
            sum.checked_add(width)
                .map_err(|_| ParseError::WidthOverflow)
        })?;
    if consumed != window.range.len() {
        return Err(window_not_consumed());
    }
    Ok(ParsedShellWindow {
        elements: elements.into(),
        diagnostics: diagnostics.into(),
    })
}

/// Parses the blocks of a window that still has document text after it.
///
/// [`crate::markdown::block::parse`] is handed `source[start..end]` and reads
/// `end` as the end of input — and end of input is not a neutral context. It
/// closes every container still open, which hands the innermost one whatever
/// blank space trails the window; a real following line closes those
/// containers first and leaves that space to an ancestor. Either way the
/// window's own bytes are unchanged, so the difference is invisible to every
/// check that compares text.
///
/// Parse one line past the window instead, so the parser sees the line that
/// actually follows, and keep the elements that land inside the window. The
/// caller's width check then rejects the parse whenever they no longer tile
/// it, which is the honest answer for a following line that reshapes the
/// window's own last block rather than merely closing it.
fn parse_window_blocks(
    text: &SourceText,
    dialect: MarkdownDialect,
    start: usize,
    end: usize,
    diagnostics: &mut Vec<TreeDiagnostic<OkfSyntaxDiagnosticCode>>,
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, ParseError> {
    let source = text.shared();
    let lookahead = source[end..]
        .find('\n')
        .map_or(source.len(), |offset| end + offset + 1);
    let blocks = crate::markdown::block::parse(text, dialect, start, lookahead)?;
    let mut elements = Vec::new();
    let mut at = start;
    for child in blocks.root.children() {
        let next = at + element_width(child).to_usize();
        if next > end {
            break;
        }
        elements.push(child.clone());
        at = next;
    }
    diagnostics.extend(
        blocks
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.range.end().to_usize() <= end)
            .cloned(),
    );
    Ok(elements)
}

fn element_width(element: &GreenElement<OkfMarkdownLanguage>) -> TextSize {
    match element {
        GreenElement::Node(node) => node.width(),
        GreenElement::Token(token) => token.width(),
    }
}

fn window_not_consumed() -> ParseError {
    ParseError::StructuralInvariant {
        reason: "shell window parser did not consume the selected range".into(),
    }
}

fn plausible_unclosed_frontmatter(source: &str, from: usize, to: usize) -> bool {
    let mut entries = 0;
    for (start, end) in lines(source, from, to) {
        let line = line_at(source, start, end);
        if line.start == line.significant_end {
            continue;
        }
        let (indent, indent_end, _has_tab) =
            leading_indent(source, line.start, line.significant_end);
        // Comments, sequence items, and indented continuation lines never
        // disqualify a candidate — only a column-0 `key:` line counts.
        if source.as_bytes().get(indent_end) == Some(&b'#')
            || is_dash_at(source, indent_end, line.significant_end)
        {
            continue;
        }
        if indent > 0 {
            continue;
        }
        let content = &source[line.start..line.significant_end];
        let Some(colon) = content.find(':') else {
            return false;
        };
        if content[..colon].trim().is_empty() {
            return false;
        }
        entries += 1;
    }
    entries > 0
}

/// Which syntax kind a stack frame's finished children get wrapped in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FmContainerKind {
    Mapping,
    Sequence,
}

impl FmContainerKind {
    fn node_kind(self) -> OkfMarkdownSyntaxKind {
        match self {
            FmContainerKind::Mapping => OkfMarkdownSyntaxKind::FrontmatterMapping,
            FmContainerKind::Sequence => OkfMarkdownSyntaxKind::FrontmatterSequence,
        }
    }
}

/// An entry or sequence-item that has been opened (key+colon, or a dash) but
/// whose value is not yet known — a deeper-indented line still to come
/// decides whether it becomes a nested mapping, a nested sequence, or (on
/// same/lesser indent, or EOF) an explicit `FrontmatterValue`/Null.
struct FmPending {
    is_sequence_item: bool,
    /// Already-built prefix children (key/dash, colon, optional comment).
    children: Vec<GreenElement<OkfMarkdownLanguage>>,
    /// The newline token for the opening line, appended AFTER the value.
    trailer: Option<GreenElement<OkfMarkdownLanguage>>,
    /// Blank or comment lines encountered while this entry is still open
    /// (between the "key:" / "-" line and the deeper-indented content that
    /// resolves it) — these bytes fall INSIDE the entry's own source span,
    /// so they must be replayed after `trailer` and before the resolved
    /// value/node, not appended as a sibling in the owning frame.
    deferred: Vec<GreenElement<OkfMarkdownLanguage>>,
    /// Index in the owning frame's `children` this entry must land at once
    /// resolved — recorded when the pending was opened.
    insert_at: usize,
}

struct FmFrame {
    kind: FmContainerKind,
    indent: usize,
    children: Vec<GreenElement<OkfMarkdownLanguage>>,
    /// DECODED keys seen in this frame, for the duplicate-key diagnostic —
    /// decoded (not raw source text) so `'a': 1` and `a: 2` are recognized
    /// as the same key, matching the model's last-wins collapse. A set, not
    /// a list: a linear scan per entry made frontmatter parsing quadratic in
    /// key count, which a hostile document can drive.
    seen_keys: HashSet<String>,
    pending: Option<FmPending>,
}

impl FmFrame {
    fn new(kind: FmContainerKind, indent: usize) -> Self {
        Self {
            kind,
            indent,
            children: Vec::new(),
            seen_keys: HashSet::new(),
            pending: None,
        }
    }

    /// Records `key` and reports whether this frame already held it.
    fn insert_key(&mut self, key: String) -> bool {
        !self.seen_keys.insert(key)
    }
}

enum ValueKind {
    Bare,
    Quoted,
}

struct ValueScan {
    kind: ValueKind,
    value_end: usize,
    comment_start: Option<usize>,
    unterminated: bool,
    malformed: bool,
    /// Span of the first unknown/malformed `\x` escape inside a
    /// double-quoted scalar, if any (single-quoted has no escapes to check).
    invalid_escape: Option<(usize, usize)>,
}

/// Scans a value position (after `key: ` or after `- `), quote-aware. Bare
/// scalars stop at a ` #` trailing comment; a bare scalar containing `: `
/// (colon-space) is flagged malformed per YAML's block-mapping grammar.
fn scan_value(source: &str, start: usize, limit: usize) -> ValueScan {
    let bytes = source.as_bytes();
    if start >= limit {
        return ValueScan {
            kind: ValueKind::Bare,
            value_end: start,
            comment_start: None,
            unterminated: false,
            malformed: false,
            invalid_escape: None,
        };
    }
    if let Some(scan) = scan_quoted_scalar(source, start, limit) {
        let QuotedScalarScan {
            end: value_end,
            unterminated,
            invalid_escape,
        } = scan;
        if unterminated {
            // Already consumed to `limit` — nothing trails to scan.
            return ValueScan {
                kind: ValueKind::Quoted,
                value_end,
                comment_start: None,
                unterminated: true,
                malformed: false,
                invalid_escape,
            };
        }
        // A closed quote can still be followed by a trailing comment, or by
        // garbage YAML would error on — fold garbage into the token's own
        // span (rather than silently dropping it) and flag it malformed.
        let mut trail = value_end;
        while trail < limit && bytes[trail] == b' ' {
            trail += 1;
        }
        if trail < limit && bytes[trail] == b'#' {
            return ValueScan {
                kind: ValueKind::Quoted,
                value_end,
                comment_start: Some(trail),
                unterminated: false,
                malformed: false,
                invalid_escape,
            };
        }
        return ValueScan {
            kind: ValueKind::Quoted,
            value_end: if trail < limit { limit } else { value_end },
            comment_start: None,
            unterminated: false,
            malformed: trail < limit,
            invalid_escape,
        };
    }
    // Bare run, but flow-aware: a `[...]` run is a flow sequence whose items
    // may be quoted, and a ` #` or `: ` INSIDE such a quoted item is content,
    // not a comment cutoff or a mapping indicator. The item-start rule matches
    // `split_flow_items` in the model crate, which reads the token back: a
    // quote only opens a quoted item at an item start (string start, after a
    // comma, or right after an opening bracket). Where the scalar then ENDS is
    // `scan_quoted_scalar`'s answer, the same one the model gets.
    let mut at = start;
    let mut comment_start = None;
    let mut malformed = false;
    let mut depth: i32 = 0;
    let mut at_item_start = false;
    while at < limit {
        let byte = bytes[at];
        if byte == b' ' {
            at += 1;
            continue;
        }
        if depth > 0 && at_item_start {
            if let Some(scan) = scan_quoted_scalar(source, at, limit) {
                at = scan.end;
                at_item_start = false;
                continue;
            }
        }
        if depth == 0 && byte == b'#' && at > start && bytes[at - 1] == b' ' {
            comment_start = Some(at);
            break;
        }
        match byte {
            b'[' | b'{' => {
                depth += 1;
                at_item_start = true;
                at += 1;
            }
            b']' | b'}' => {
                depth -= 1;
                at_item_start = false;
                at += 1;
            }
            b',' if depth > 0 => {
                at_item_start = true;
                at += 1;
            }
            b':' => {
                if depth == 0 && at + 1 < limit && bytes[at + 1] == b' ' {
                    malformed = true;
                }
                // Inside a flow collection a `: ` opens a mapping VALUE, which
                // may itself be a quoted scalar (`{ id: "a, b" }`) — that is
                // an item start for the quote bookkeeping above, exactly like
                // the position after a `,` or an opening bracket. The trailing
                // space is required, per YAML: a `:` not followed by one
                // (`:'`) is an ordinary character of a plain scalar.
                at_item_start = depth > 0 && at + 1 < limit && bytes[at + 1] == b' ';
                at += 1;
            }
            _ => {
                at_item_start = false;
                at += 1;
            }
        }
    }
    let raw_end = comment_start.unwrap_or(at);
    let value_end = trim_horizontal_end(source, start, raw_end);
    ValueScan {
        kind: ValueKind::Bare,
        value_end,
        comment_start,
        unterminated: false,
        malformed,
        invalid_escape: None,
    }
}

struct FmKeyMatch {
    key_start: usize,
    key_end: usize,
    colon: usize,
    key_kind: OkfMarkdownSyntaxKind,
}

/// Recognizes `key:` (bare or quoted) at a value position, per the YAML rule
/// that the colon must be followed by a space or end-of-line.
fn parse_mapping_key(source: &str, start: usize, limit: usize) -> Option<FmKeyMatch> {
    let bytes = source.as_bytes();
    if start >= limit {
        return None;
    }
    if let Some(scan) = scan_quoted_scalar(source, start, limit) {
        // An unterminated key runs to `limit`, so the colon test below fails
        // and this is not a key — the same verdict the value scanner reaches.
        let key_end = scan.end;
        if key_end < limit
            && bytes[key_end] == b':'
            && (key_end + 1 == limit || bytes[key_end + 1] == b' ')
        {
            return Some(FmKeyMatch {
                key_start: start,
                key_end,
                colon: key_end,
                key_kind: OkfMarkdownSyntaxKind::FrontmatterQuotedValueToken,
            });
        }
        return None;
    }
    // A `[` opens a flow sequence and a `{` a flow mapping — never a plain
    // key (YAML forbids an indicator there). Scanning on would find a `:`
    // inside the flow value and misread `- [": "]` as a `- key: value` entry,
    // or split `- { id: a, title: b }` into a key `{ id` and the rest.
    if bytes[start] == b'[' || bytes[start] == b'{' {
        return None;
    }
    let mut at = start;
    while at < limit {
        if bytes[at] == b':' && (at + 1 == limit || bytes[at + 1] == b' ') {
            let key_end = trim_horizontal_end(source, start, at);
            if key_end == start {
                return None;
            }
            return Some(FmKeyMatch {
                key_start: start,
                key_end,
                colon: at,
                key_kind: OkfMarkdownSyntaxKind::FrontmatterKey,
            });
        }
        at += 1;
    }
    None
}

fn is_dash_at(source: &str, at: usize, limit: usize) -> bool {
    let bytes = source.as_bytes();
    at < limit && bytes[at] == b'-' && (at + 1 == limit || bytes[at + 1] == b' ')
}

/// Leading-whitespace run of a line: (column count, end offset, saw a tab).
fn leading_indent(source: &str, start: usize, limit: usize) -> (usize, usize, bool) {
    let bytes = source.as_bytes();
    let mut at = start;
    let mut has_tab = false;
    while at < limit && matches!(bytes[at], b' ' | b'\t') {
        has_tab |= bytes[at] == b'\t';
        at += 1;
    }
    (at - start, at, has_tab)
}

fn blank_or_comment_entry(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    line: Line,
    comment_start: Option<usize>,
) -> Result<crate::GreenNode<OkfMarkdownLanguage>, ParseError> {
    let mut children = Vec::new();
    if let Some(comment_start) = comment_start {
        children.push(GreenElement::Token(token_with_leading(
            factory,
            text,
            line.start,
            comment_start,
            line.significant_end,
            OkfMarkdownSyntaxKind::FrontmatterCommentToken,
        )?));
    }
    if line.newline_start < line.end {
        children.push(GreenElement::Token(newline_token(factory, text, line)?));
    }
    identified_node(factory, OkfMarkdownSyntaxKind::FrontmatterEntry, children)
}

fn bad_line_entry(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    line: Line,
    content_start: usize,
    code: OkfSyntaxDiagnosticCode,
) -> Result<crate::GreenNode<OkfMarkdownLanguage>, ParseError> {
    let leading = trivia(factory, text, line.start, content_start)?;
    let mut children = vec![GreenElement::Token(
        factory
            .bad_token_with_leading(
                OkfMarkdownSyntaxKind::BadToken,
                slice(text, content_start, line.significant_end)?,
                leading,
                code,
            )
            .map_err(|_| ParseError::WidthOverflow)?,
    )];
    let _ = source;
    if line.newline_start < line.end {
        children.push(GreenElement::Token(newline_token(factory, text, line)?));
    }
    identified_node(factory, OkfMarkdownSyntaxKind::FrontmatterEntry, children)
}

/// Gives up on a frontmatter line: keeps its bytes as one bad-token entry on
/// the open block, so the line still reaches the tree spelled exactly as it
/// was written.  The caller has already said why in a diagnostic -- there are
/// several reasons a line ends up here and only the caller knows which.
fn push_bad_line(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    line: Line,
    content_start: usize,
    code: OkfSyntaxDiagnosticCode,
    stack: &mut [FmFrame],
) -> Result<(), ParseError> {
    let entry = bad_line_entry(factory, text, source, line, content_start, code)?;
    stack
        .last_mut()
        .expect("stack is never empty")
        .children
        .push(GreenElement::Node(entry));
    Ok(())
}

/// If `frame` has an open pending entry, resolve it to an explicit `Null`
/// value (a missing `FrontmatterValue` token) and splice it into the
/// frame's own children at its recorded `insert_at`. Shared by
/// `finalize_pending_with_null` (top-of-stack) and `pop_frame` (a frame
/// about to be consumed into its parent's node) — a frame is turned into a
/// green node from `frame.children` alone, so a still-open pending on that
/// frame must be resolved first or its key/dash tokens are silently lost.
fn finalize_frame_pending(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    frame: &mut FmFrame,
) -> Result<(), ParseError> {
    let Some(pending) = frame.pending.take() else {
        return Ok(());
    };
    let mut children = pending.children;
    if let Some(trailer) = pending.trailer {
        children.push(trailer);
    }
    children.extend(pending.deferred);
    children.push(GreenElement::Token(
        factory.missing_token(OkfMarkdownSyntaxKind::FrontmatterValue),
    ));
    let wrap_kind = if pending.is_sequence_item {
        OkfMarkdownSyntaxKind::FrontmatterSequenceItem
    } else {
        OkfMarkdownSyntaxKind::FrontmatterEntry
    };
    let insert_at = pending.insert_at.min(frame.children.len());
    frame.children.insert(
        insert_at,
        GreenElement::Node(identified_node(factory, wrap_kind, children)?),
    );
    Ok(())
}

fn pop_frame(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    stack: &mut Vec<FmFrame>,
) -> Result<(), ParseError> {
    let mut frame = stack.pop().expect("pop_frame requires a frame to pop");
    // A dedent can close this frame while one of its OWN entries is still
    // pending (e.g. `tags:` opened a nested block that never arrived because
    // the next line dedented past it) — resolve that to Null first, or its
    // key/dash tokens never make it into the frame's node.
    finalize_frame_pending(factory, &mut frame)?;
    let node = identified_node(factory, frame.kind.node_kind(), frame.children)?;
    let parent = stack.last_mut().expect("root frame is never popped");
    if let Some(pending) = parent.pending.take() {
        let mut children = pending.children;
        // The trailer is the newline that ended the opening ("key:" / "-")
        // line — it precedes the nested block's own bytes in source order.
        if let Some(trailer) = pending.trailer {
            children.push(trailer);
        }
        // Blank/comment lines seen while this entry was still open sit
        // between the trailer and the nested block in source order.
        children.extend(pending.deferred);
        children.push(GreenElement::Node(node));
        let wrap_kind = if pending.is_sequence_item {
            OkfMarkdownSyntaxKind::FrontmatterSequenceItem
        } else {
            OkfMarkdownSyntaxKind::FrontmatterEntry
        };
        let insert_at = pending.insert_at.min(parent.children.len());
        parent.children.insert(
            insert_at,
            GreenElement::Node(identified_node(factory, wrap_kind, children)?),
        );
    } else {
        // Defensive: a frame is only ever opened to satisfy a pending value
        // slot, so this should not happen in practice.
        parent.children.push(GreenElement::Node(node));
    }
    Ok(())
}

fn finalize_pending_with_null(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    stack: &mut [FmFrame],
) -> Result<(), ParseError> {
    let frame = stack.last_mut().expect("stack is never empty");
    finalize_frame_pending(factory, frame)
}

/// The outcome of pushing one mapping entry / sequence item: whether it
/// needed recovery, and — when it opened a block scalar and consumed extra
/// lines directly off the shared iterator — the new consumed-end offset the
/// caller's loop must adopt in place of the single line it saw.
struct EntryOutcome {
    malformed: bool,
    consumed_end: Option<usize>,
}

impl EntryOutcome {
    fn clean(malformed: bool) -> Self {
        Self {
            malformed,
            consumed_end: None,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn push_mapping_entry<I: Iterator<Item = (usize, usize)>>(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    line: Line,
    key_leading_start: usize,
    key: FmKeyMatch,
    stack: &mut [FmFrame],
    it: &mut std::iter::Peekable<I>,
    diagnostics: &mut Vec<TreeDiagnostic<OkfSyntaxDiagnosticCode>>,
) -> Result<EntryOutcome, ParseError> {
    let mut malformed = false;
    let mut children = vec![
        GreenElement::Token(token_with_leading(
            factory,
            text,
            key_leading_start,
            key.key_start,
            key.key_end,
            key.key_kind,
        )?),
        GreenElement::Token(token_with_leading(
            factory,
            text,
            key.key_end,
            key.colon,
            key.colon + 1,
            OkfMarkdownSyntaxKind::ColonToken,
        )?),
    ];
    // Compare DECODED key text: `'a': 1` and `a: 2` name the same key, and
    // the model's reader collapses them last-wins — so the diagnostic has to
    // fire, or an entry vanishes silently.
    let key_text = super::scalar::decode_quoted_scalar(&source[key.key_start..key.key_end]);
    let frame = stack.last_mut().expect("stack is never empty");
    let dup = frame.insert_key(key_text);
    if dup {
        malformed = true;
        diagnostics.push(diagnostic(
            OkfSyntaxDiagnosticCode::DuplicateFrontmatterKey,
            key.key_start,
            key.key_end,
            "duplicate frontmatter key",
        ));
    }

    let value_start = skip_horizontal(source, key.colon + 1, line.significant_end);
    if let Some(header_len) = block_scalar_header_len(&source[value_start..line.significant_end]) {
        let parent_indent = key.key_start - line.start;
        return open_block_scalar(
            factory,
            text,
            source,
            line,
            parent_indent,
            key.colon + 1,
            value_start,
            header_len,
            children,
            false,
            it,
            stack,
            diagnostics,
        );
    }
    let starts_comment =
        value_start < line.significant_end && source.as_bytes()[value_start] == b'#';
    if value_start == line.significant_end || starts_comment {
        if starts_comment {
            children.push(GreenElement::Token(token_with_leading(
                factory,
                text,
                key.colon + 1,
                value_start,
                line.significant_end,
                OkfMarkdownSyntaxKind::FrontmatterCommentToken,
            )?));
        }
        let trailer = if line.newline_start < line.end {
            Some(GreenElement::Token(newline_token(factory, text, line)?))
        } else {
            None
        };
        let insert_at = frame.children.len();
        frame.pending = Some(FmPending {
            is_sequence_item: false,
            children,
            trailer,
            deferred: Vec::new(),
            insert_at,
        });
        return Ok(EntryOutcome::clean(malformed));
    }

    let scan = scan_value(source, value_start, line.significant_end);
    match scan.kind {
        ValueKind::Quoted => {
            children.push(GreenElement::Token(token_with_leading(
                factory,
                text,
                key.colon + 1,
                value_start,
                scan.value_end,
                OkfMarkdownSyntaxKind::FrontmatterQuotedValueToken,
            )?));
            if scan.unterminated {
                malformed = true;
                diagnostics.push(diagnostic(
                    OkfSyntaxDiagnosticCode::UnterminatedQuotedScalar,
                    value_start,
                    scan.value_end,
                    "unterminated quoted scalar",
                ));
            } else if scan.malformed {
                malformed = true;
                diagnostics.push(diagnostic(
                    OkfSyntaxDiagnosticCode::MalformedFrontmatterEntry,
                    value_start,
                    scan.value_end,
                    "malformed frontmatter entry",
                ));
            }
            if let Some((escape_start, escape_end)) = scan.invalid_escape {
                malformed = true;
                diagnostics.push(diagnostic(
                    OkfSyntaxDiagnosticCode::InvalidEscapeSequence,
                    escape_start,
                    escape_end,
                    "unknown escape sequence in quoted scalar",
                ));
            }
        }
        ValueKind::Bare => {
            children.push(GreenElement::Token(token_with_leading(
                factory,
                text,
                key.colon + 1,
                value_start,
                scan.value_end,
                OkfMarkdownSyntaxKind::FrontmatterValue,
            )?));
            if scan.malformed {
                malformed = true;
                diagnostics.push(diagnostic(
                    OkfSyntaxDiagnosticCode::MalformedFrontmatterEntry,
                    value_start,
                    line.significant_end,
                    "malformed frontmatter entry",
                ));
            }
        }
    }
    if let Some(comment_start) = scan.comment_start {
        children.push(GreenElement::Token(token_with_leading(
            factory,
            text,
            scan.value_end,
            comment_start,
            line.significant_end,
            OkfMarkdownSyntaxKind::FrontmatterCommentToken,
        )?));
    }
    if line.newline_start < line.end {
        children.push(GreenElement::Token(newline_token(factory, text, line)?));
    }
    frame.children.push(GreenElement::Node(identified_node(
        factory,
        OkfMarkdownSyntaxKind::FrontmatterEntry,
        children,
    )?));
    Ok(EntryOutcome::clean(malformed))
}

/// Consumes a value-position block scalar header (`|`/`>` + modifiers) and
/// every content line indented deeper than `parent_indent` (or blank)
/// directly off the shared line iterator, then wraps the whole entry.
/// Shared by `push_mapping_entry` and `push_sequence_item` so a `- |` item
/// and a `key: |` entry behave identically.
#[allow(clippy::too_many_arguments)]
fn open_block_scalar<I: Iterator<Item = (usize, usize)>>(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    line: Line,
    parent_indent: usize,
    header_leading_start: usize,
    value_start: usize,
    header_len: usize,
    mut children: Vec<GreenElement<OkfMarkdownLanguage>>,
    is_sequence_item: bool,
    it: &mut std::iter::Peekable<I>,
    stack: &mut [FmFrame],
    diagnostics: &mut Vec<TreeDiagnostic<OkfSyntaxDiagnosticCode>>,
) -> Result<EntryOutcome, ParseError> {
    let header_end = value_start + header_len;
    children.push(GreenElement::Token(token_with_leading(
        factory,
        text,
        header_leading_start,
        value_start,
        header_end,
        OkfMarkdownSyntaxKind::FrontmatterBlockScalarHeaderToken,
    )?));
    let after_header = skip_horizontal(source, header_end, line.significant_end);
    let mut malformed = false;
    if after_header < line.significant_end {
        if source.as_bytes()[after_header] == b'#' {
            children.push(GreenElement::Token(token_with_leading(
                factory,
                text,
                header_end,
                after_header,
                line.significant_end,
                OkfMarkdownSyntaxKind::FrontmatterCommentToken,
            )?));
        } else {
            // Anything else trailing the header (chomping/indentation
            // indicators already consumed by `header_len`) is malformed —
            // capture it as a BadToken instead of silently dropping it, or
            // the tree's rendered text stops matching the source.
            malformed = true;
            diagnostics.push(diagnostic(
                OkfSyntaxDiagnosticCode::MalformedFrontmatterEntry,
                after_header,
                line.significant_end,
                "unexpected content after block scalar header",
            ));
            children.push(GreenElement::Token(token_with_leading(
                factory,
                text,
                header_end,
                after_header,
                line.significant_end,
                OkfMarkdownSyntaxKind::BadToken,
            )?));
        }
    }
    if line.newline_start < line.end {
        children.push(GreenElement::Token(newline_token(factory, text, line)?));
    }
    let mut consumed_end = structured_end(line);
    while let Some(&(cstart, cend)) = it.peek() {
        let cline = line_at(source, cstart, cend);
        let blank = cline.start == cline.significant_end;
        if !blank {
            let (cindent, _, _) = leading_indent(source, cline.start, cline.significant_end);
            if cindent <= parent_indent {
                break;
            }
        }
        it.next();
        children.push(GreenElement::Token(token_with_leading(
            factory,
            text,
            cline.start,
            cline.start,
            cline.significant_end,
            OkfMarkdownSyntaxKind::FrontmatterValue,
        )?));
        if cline.newline_start < cline.end {
            children.push(GreenElement::Token(newline_token(factory, text, cline)?));
        }
        consumed_end = structured_end(cline);
    }
    let wrap_kind = if is_sequence_item {
        OkfMarkdownSyntaxKind::FrontmatterSequenceItem
    } else {
        OkfMarkdownSyntaxKind::FrontmatterEntry
    };
    stack
        .last_mut()
        .expect("stack is never empty")
        .children
        .push(GreenElement::Node(identified_node(
            factory, wrap_kind, children,
        )?));
    Ok(EntryOutcome {
        malformed,
        consumed_end: Some(consumed_end),
    })
}

#[allow(clippy::too_many_arguments)]
fn push_sequence_item<I: Iterator<Item = (usize, usize)>>(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    line: Line,
    indent_end: usize,
    stack: &mut Vec<FmFrame>,
    it: &mut std::iter::Peekable<I>,
    diagnostics: &mut Vec<TreeDiagnostic<OkfSyntaxDiagnosticCode>>,
) -> Result<EntryOutcome, ParseError> {
    let dash = GreenElement::Token(token_with_leading(
        factory,
        text,
        line.start,
        indent_end,
        indent_end + 1,
        OkfMarkdownSyntaxKind::FrontmatterDashToken,
    )?);
    let after_dash = skip_horizontal(source, indent_end + 1, line.significant_end);
    if let Some(header_len) = block_scalar_header_len(&source[after_dash..line.significant_end]) {
        let parent_indent = indent_end - line.start;
        return open_block_scalar(
            factory,
            text,
            source,
            line,
            parent_indent,
            indent_end + 1,
            after_dash,
            header_len,
            vec![dash],
            true,
            it,
            stack,
            diagnostics,
        );
    }
    if after_dash == line.significant_end {
        let mut children = vec![
            dash,
            GreenElement::Token(
                factory
                    .missing_token_with_leading(
                        OkfMarkdownSyntaxKind::FrontmatterValue,
                        trivia(factory, text, indent_end + 1, after_dash)?,
                    )
                    .map_err(|_| ParseError::WidthOverflow)?,
            ),
        ];
        if line.newline_start < line.end {
            children.push(GreenElement::Token(newline_token(factory, text, line)?));
        }
        stack
            .last_mut()
            .expect("stack is never empty")
            .children
            .push(GreenElement::Node(identified_node(
                factory,
                OkfMarkdownSyntaxKind::FrontmatterSequenceItem,
                children,
            )?));
        return Ok(EntryOutcome::clean(false));
    }
    if let Some(key) = parse_mapping_key(source, after_dash, line.significant_end) {
        let parent = stack.last_mut().expect("stack is never empty");
        let insert_at = parent.children.len();
        parent.pending = Some(FmPending {
            is_sequence_item: true,
            children: vec![dash],
            trailer: None,
            deferred: Vec::new(),
            insert_at,
        });
        stack.push(FmFrame::new(
            FmContainerKind::Mapping,
            after_dash - line.start,
        ));
        return push_mapping_entry(
            factory,
            text,
            source,
            line,
            indent_end + 1,
            key,
            stack,
            it,
            diagnostics,
        );
    }

    let mut malformed = false;
    let mut children = vec![dash];
    let scan = scan_value(source, after_dash, line.significant_end);
    match scan.kind {
        ValueKind::Quoted => {
            children.push(GreenElement::Token(token_with_leading(
                factory,
                text,
                indent_end + 1,
                after_dash,
                scan.value_end,
                OkfMarkdownSyntaxKind::FrontmatterQuotedValueToken,
            )?));
            if scan.unterminated {
                malformed = true;
                diagnostics.push(diagnostic(
                    OkfSyntaxDiagnosticCode::UnterminatedQuotedScalar,
                    after_dash,
                    scan.value_end,
                    "unterminated quoted scalar",
                ));
            }
            if let Some((escape_start, escape_end)) = scan.invalid_escape {
                malformed = true;
                diagnostics.push(diagnostic(
                    OkfSyntaxDiagnosticCode::InvalidEscapeSequence,
                    escape_start,
                    escape_end,
                    "unknown escape sequence in quoted scalar",
                ));
            }
        }
        ValueKind::Bare => {
            children.push(GreenElement::Token(token_with_leading(
                factory,
                text,
                indent_end + 1,
                after_dash,
                scan.value_end,
                OkfMarkdownSyntaxKind::FrontmatterValue,
            )?));
            if scan.malformed {
                malformed = true;
                diagnostics.push(diagnostic(
                    OkfSyntaxDiagnosticCode::MalformedFrontmatterEntry,
                    after_dash,
                    line.significant_end,
                    "malformed frontmatter entry",
                ));
            }
        }
    }
    if let Some(comment_start) = scan.comment_start {
        children.push(GreenElement::Token(token_with_leading(
            factory,
            text,
            scan.value_end,
            comment_start,
            line.significant_end,
            OkfMarkdownSyntaxKind::FrontmatterCommentToken,
        )?));
    }
    if line.newline_start < line.end {
        children.push(GreenElement::Token(newline_token(factory, text, line)?));
    }
    stack
        .last_mut()
        .expect("stack is never empty")
        .children
        .push(GreenElement::Node(identified_node(
            factory,
            OkfMarkdownSyntaxKind::FrontmatterSequenceItem,
            children,
        )?));
    Ok(EntryOutcome::clean(malformed))
}

/// Builds the single `FrontmatterMapping` node that sits between the fences,
/// via an explicit indent-stack over the significant lines `[from, to)`.
/// Returns the node, whether it stayed clean (no recovery needed), and the
/// consumed-end offset of the last significant line (mirrors `structured_end`).
/// Route a blank or comment line's node to the top frame's pending entry
/// (when one is open) instead of the frame's own children: the line's bytes
/// fall INSIDE the still-open entry's source span (between its "key:" / "-"
/// line and the deeper-indented content that will resolve it), so appending
/// it as a frame sibling would reorder bytes ahead of the pending's node
/// once the entry resolves and is spliced in at `insert_at`.
fn push_blank_or_comment(stack: &mut [FmFrame], entry: crate::GreenNode<OkfMarkdownLanguage>) {
    let frame = stack.last_mut().expect("stack is never empty");
    if let Some(pending) = frame.pending.as_mut() {
        pending.deferred.push(GreenElement::Node(entry));
    } else {
        frame.children.push(GreenElement::Node(entry));
    }
}

/// Whether a frontmatter line's indentation found a block to belong to.
#[derive(Clone, Copy, Eq, PartialEq)]
enum IndentFit {
    /// The top of the stack is now the block this line belongs to.
    Open,
    /// The indentation matches no open block, or opening one would nest
    /// past the cap.  The line has been reported and kept as a bad line.
    Rejected,
}

/// Opens, closes, or keeps the open frames so that the top of the stack is
/// the block the line at this indentation belongs to.  A YAML block sequence
/// may sit at its own key's indentation, so `Open` does not imply the stack
/// is unchanged.
#[allow(clippy::too_many_arguments)]
fn fit_indent_to_open_blocks(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    line: Line,
    indent: usize,
    indent_end: usize,
    stack: &mut Vec<FmFrame>,
    diagnostics: &mut Vec<TreeDiagnostic<OkfSyntaxDiagnosticCode>>,
) -> Result<IndentFit, ParseError> {
    let top_indent = stack.last().expect("stack is never empty").indent;
    if indent > top_indent {
        if stack
            .last()
            .expect("stack is never empty")
            .pending
            .is_none()
        {
            diagnostics.push(diagnostic(
                OkfSyntaxDiagnosticCode::InvalidFrontmatterIndent,
                line.start,
                indent_end,
                "frontmatter indentation does not match an open block",
            ));
            push_bad_line(
                factory,
                text,
                source,
                line,
                indent_end,
                OkfSyntaxDiagnosticCode::InvalidFrontmatterIndent,
                stack,
            )?;
            return Ok(IndentFit::Rejected);
        }
        // Nesting cap, mirroring `MD_MAX_CONTAINER_DEPTH` for markdown
        // containers and published as `FRONTMATTER_MAX_NESTING_DEPTH` so
        // tree consumers cap at the same depth: every frame becomes one
        // more level of the green
        // tree, and reading or dropping that tree recurses. Without the
        // cap a document of progressively indented `k:` lines — cheap to
        // write, cheaper still to hit on wasm's 1MB stack — overflows the
        // stack instead of producing a diagnostic.
        if stack.len() >= super::FRONTMATTER_MAX_NESTING_DEPTH {
            diagnostics.push(diagnostic(
                OkfSyntaxDiagnosticCode::InvalidFrontmatterIndent,
                line.start,
                indent_end,
                "frontmatter nesting is too deep",
            ));
            finalize_pending_with_null(factory, stack)?;
            push_bad_line(
                factory,
                text,
                source,
                line,
                indent_end,
                OkfSyntaxDiagnosticCode::InvalidFrontmatterIndent,
                stack,
            )?;
            return Ok(IndentFit::Rejected);
        }
        let kind = if is_dash_at(source, indent_end, line.significant_end) {
            FmContainerKind::Sequence
        } else {
            FmContainerKind::Mapping
        };
        stack.push(FmFrame::new(kind, indent));
    } else if indent < top_indent {
        while stack.len() > 1 && indent < stack.last().expect("checked len").indent {
            pop_frame(factory, stack)?;
        }
        if indent != stack.last().expect("stack is never empty").indent {
            diagnostics.push(diagnostic(
                OkfSyntaxDiagnosticCode::InvalidFrontmatterIndent,
                line.start,
                indent_end,
                "frontmatter indentation does not match an open block",
            ));
            push_bad_line(
                factory,
                text,
                source,
                line,
                indent_end,
                OkfSyntaxDiagnosticCode::InvalidFrontmatterIndent,
                stack,
            )?;
            return Ok(IndentFit::Rejected);
        }
        finalize_pending_with_null(factory, stack)?;
    } else {
        let top = stack.last().expect("stack is never empty");
        let dash = is_dash_at(source, indent_end, line.significant_end);
        if top.kind == FmContainerKind::Mapping && dash && top.pending.is_some() {
            // YAML lets a block sequence sit at its own key's indentation:
            //
            //     tags:
            //     - a
            //
            // The key's value slot is still open, so the dash opens a
            // sequence at THIS indent rather than being a dash with no
            // container. Capped like the indent-increase branch.
            if stack.len() >= super::FRONTMATTER_MAX_NESTING_DEPTH {
                diagnostics.push(diagnostic(
                    OkfSyntaxDiagnosticCode::InvalidFrontmatterIndent,
                    line.start,
                    indent_end,
                    "frontmatter nesting is too deep",
                ));
                finalize_pending_with_null(factory, stack)?;
                push_bad_line(
                    factory,
                    text,
                    source,
                    line,
                    indent_end,
                    OkfSyntaxDiagnosticCode::InvalidFrontmatterIndent,
                    stack,
                )?;
                return Ok(IndentFit::Rejected);
            }
            stack.push(FmFrame::new(FmContainerKind::Sequence, indent));
        } else if top.kind == FmContainerKind::Sequence && !dash {
            // A non-dash line at the sequence's own indent ends it — the
            // sequence was opened at its key's indent, so the enclosing
            // mapping continues on this very line.
            pop_frame(factory, stack)?;
            finalize_pending_with_null(factory, stack)?;
        } else {
            finalize_pending_with_null(factory, stack)?;
        }
    }
    Ok(IndentFit::Open)
}

fn build_frontmatter_mapping(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    from: usize,
    to: usize,
    diagnostics: &mut Vec<TreeDiagnostic<OkfSyntaxDiagnosticCode>>,
) -> Result<(crate::GreenNode<OkfMarkdownLanguage>, bool, usize), ParseError> {
    let mut clean = true;
    let mut entries_consumed_end = from;
    let mut stack: Vec<FmFrame> = vec![FmFrame::new(FmContainerKind::Mapping, 0)];

    let mut it = lines(source, from, to).peekable();
    while let Some((start, end)) = it.next() {
        let line = line_at(source, start, end);
        entries_consumed_end = structured_end(line);

        if line.start == line.significant_end {
            let entry = blank_or_comment_entry(factory, text, line, None)?;
            push_blank_or_comment(&mut stack, entry);
            continue;
        }

        let (indent, indent_end, has_tab) =
            leading_indent(source, line.start, line.significant_end);

        if has_tab {
            clean = false;
            diagnostics.push(diagnostic(
                OkfSyntaxDiagnosticCode::TabInFrontmatterIndent,
                line.start,
                indent_end,
                "tab used in frontmatter indentation",
            ));
            finalize_pending_with_null(factory, &mut stack)?;
            push_bad_line(
                factory,
                text,
                source,
                line,
                indent_end,
                OkfSyntaxDiagnosticCode::TabInFrontmatterIndent,
                &mut stack,
            )?;
            continue;
        }

        if source.as_bytes().get(indent_end) == Some(&b'#') {
            let entry = blank_or_comment_entry(factory, text, line, Some(indent_end))?;
            push_blank_or_comment(&mut stack, entry);
            continue;
        }

        let fit = fit_indent_to_open_blocks(
            factory,
            text,
            source,
            line,
            indent,
            indent_end,
            &mut stack,
            diagnostics,
        )?;
        if fit == IndentFit::Rejected {
            clean = false;
            continue;
        }

        // A dash only opens a sequence item when the block it lands in IS a
        // sequence — a sequence block is only ever opened by the indent-
        // increase branch above. At a MAPPING's own indent the dash has no
        // reading: the model reader (`map_entries_from_mapping`) skips
        // sequence items, so accepting it would drop the line silently.
        // Keep it malformed, as it was before block sequences existed.
        if is_dash_at(source, indent_end, line.significant_end) {
            if stack.last().expect("stack is never empty").kind == FmContainerKind::Mapping {
                clean = false;
                diagnostics.push(diagnostic(
                    OkfSyntaxDiagnosticCode::MalformedFrontmatterEntry,
                    line.start,
                    line.significant_end,
                    "sequence item outside a sequence",
                ));
                push_bad_line(
                    factory,
                    text,
                    source,
                    line,
                    indent_end,
                    OkfSyntaxDiagnosticCode::MalformedFrontmatterEntry,
                    &mut stack,
                )?;
                continue;
            }
            let outcome = push_sequence_item(
                factory,
                text,
                source,
                line,
                indent_end,
                &mut stack,
                &mut it,
                diagnostics,
            )?;
            clean = clean && !outcome.malformed;
            if let Some(end) = outcome.consumed_end {
                entries_consumed_end = end;
            }
            continue;
        }

        if let Some(key) = parse_mapping_key(source, indent_end, line.significant_end) {
            let outcome = push_mapping_entry(
                factory,
                text,
                source,
                line,
                line.start,
                key,
                &mut stack,
                &mut it,
                diagnostics,
            )?;
            clean = clean && !outcome.malformed;
            if let Some(end) = outcome.consumed_end {
                entries_consumed_end = end;
            }
        } else {
            clean = false;
            diagnostics.push(diagnostic(
                OkfSyntaxDiagnosticCode::MalformedFrontmatterEntry,
                line.start,
                line.significant_end,
                "malformed frontmatter entry",
            ));
            push_bad_line(
                factory,
                text,
                source,
                line,
                indent_end,
                OkfSyntaxDiagnosticCode::MalformedFrontmatterEntry,
                &mut stack,
            )?;
        }
    }

    while stack.len() > 1 {
        pop_frame(factory, &mut stack)?;
    }
    finalize_pending_with_null(factory, &mut stack)?;
    let root = stack.pop().expect("root frame always exists");
    let node = identified_node(
        factory,
        OkfMarkdownSyntaxKind::FrontmatterMapping,
        root.children,
    )?;
    Ok((node, clean, entries_consumed_end))
}

fn identified_node(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    kind: OkfMarkdownSyntaxKind,
    children: Vec<GreenElement<OkfMarkdownLanguage>>,
) -> Result<crate::GreenNode<OkfMarkdownLanguage>, ParseError> {
    let identity = SyntaxIdentity::fresh()?;
    factory
        .node_with_annotations(kind, children, vec![identity.annotation()].into())
        .map_err(|_| ParseError::WidthOverflow)
}

fn line_tokens(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    source: &str,
    line: Line,
    kind: OkfMarkdownSyntaxKind,
) -> Result<Vec<GreenElement<OkfMarkdownLanguage>>, ParseError> {
    let token_start = skip_horizontal(source, line.start, line.significant_end);
    let mut children = vec![GreenElement::Token(token_with_leading(
        factory,
        text,
        line.start,
        token_start,
        line.significant_end,
        kind,
    )?)];
    if line.newline_start < line.end {
        children.push(GreenElement::Token(newline_token(factory, text, line)?));
    }
    Ok(children)
}

fn newline_token(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    line: Line,
) -> Result<crate::GreenToken<OkfMarkdownLanguage>, ParseError> {
    token_with_leading(
        factory,
        text,
        line.significant_end,
        line.newline_start,
        line.end,
        OkfMarkdownSyntaxKind::NewlineToken,
    )
}

fn token_with_leading(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    leading_start: usize,
    text_start: usize,
    text_end: usize,
    kind: OkfMarkdownSyntaxKind,
) -> Result<crate::GreenToken<OkfMarkdownLanguage>, ParseError> {
    factory
        .token(
            kind,
            slice(text, text_start, text_end)?,
            trivia(factory, text, leading_start, text_start)?,
            [],
        )
        .map_err(|_| ParseError::WidthOverflow)
}

fn trivia(
    factory: &GreenFactory<OkfMarkdownLanguage>,
    text: &SourceText,
    start: usize,
    end: usize,
) -> Result<Vec<GreenTrivia>, ParseError> {
    if start == end {
        return Ok(Vec::new());
    }
    Ok(vec![factory
        .trivia(TriviaKind::Whitespace, slice(text, start, end)?)
        .map_err(|_| ParseError::WidthOverflow)?])
}

fn slice(text: &SourceText, start: usize, end: usize) -> Result<GreenText, ParseError> {
    let range = TextRange::new(size(start)?, size(end)?).map_err(|_| ParseError::InvalidRange {
        range: TextRange::new(size(0).unwrap(), size(0).unwrap()).unwrap(),
    })?;
    text.slice(range).map_err(|error| match error {
        crate::TextError::NonUtf8Boundary { .. } => ParseError::StructuralInvariant {
            reason: "parser produced a non-UTF-8 boundary".into(),
        },
        _ => ParseError::InvalidRange { range },
    })?;
    Ok(GreenText::SourceSlice {
        source: text.clone(),
        range,
    })
}

#[derive(Clone, Copy)]
pub(crate) struct Line {
    pub(crate) start: usize,
    pub(crate) significant_end: usize,
    pub(crate) newline_start: usize,
    pub(crate) end: usize,
}

fn line_at(source: &str, start: usize, cap: usize) -> Line {
    let end = source[start..cap]
        .find('\n')
        .map(|offset| start + offset + 1)
        .unwrap_or(cap);
    let newline_start = if end > start && source.as_bytes()[end - 1] == b'\n' {
        if end > start + 1 && source.as_bytes()[end - 2] == b'\r' {
            end - 2
        } else {
            end - 1
        }
    } else {
        end
    };
    let significant_end = trim_horizontal_end(source, start, newline_start);
    Line {
        start,
        significant_end,
        newline_start,
        end,
    }
}

fn lines(source: &str, from: usize, to: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
    let mut at = from;
    std::iter::from_fn(move || {
        if at >= to {
            return None;
        }
        let start = at;
        let end = source[at..to]
            .find('\n')
            .map(|offset| at + offset + 1)
            .unwrap_or(to);
        at = end;
        Some((start, end))
    })
}

fn is_fence_line(source: &str, line: Line) -> bool {
    matches!(
        source[line.start..line.significant_end].trim(),
        "---" | "..."
    )
}

/// If `content` (the text at a value position, right after `key:`/`- `) opens
/// a YAML block scalar, returns the length of the header run — the `|`/`>`
/// indicator plus an optional chomping modifier (`+`/`-`) and an optional
/// explicit indentation digit — not including any trailing whitespace or
/// comment. Anything else after the header (past whitespace) means the line
/// is a plain scalar, not a block scalar header. Shared by the classifier's close-fence scan and the builder's
/// value-position handling so they cannot drift.
fn block_scalar_header_len(content: &str) -> Option<usize> {
    let bytes = content.as_bytes();
    match bytes.first() {
        Some(b'|') | Some(b'>') => {}
        _ => return None,
    }
    let mut at = 1;
    if matches!(bytes.get(at), Some(b'+') | Some(b'-')) {
        at += 1;
    }
    if bytes.get(at).is_some_and(u8::is_ascii_digit) {
        at += 1;
    }
    let mut rest = at;
    while matches!(bytes.get(rest), Some(b' ') | Some(b'\t')) {
        rest += 1;
    }
    match bytes.get(rest) {
        None | Some(b'#') => Some(at),
        _ => None,
    }
}

/// A cheap line-shape scan (no tree needed): does a value position hold a
/// block scalar header?
fn line_opens_block_scalar(content: &str) -> bool {
    block_scalar_header_len(content).is_some()
}

/// The block-scalar-aware close-fence search, shared by `classify_frontmatter`
/// and `incremental.rs`'s `frontmatter_fences` so an edit inside a literal
/// block cannot compare fences that were found by two different scans. A
/// `---`/`...` line more indented than the parent of an open block scalar is
/// content, never a fence.
pub(crate) fn frontmatter_close_fence_line(
    source: &str,
    open_end: usize,
    boundary: usize,
) -> Option<Line> {
    let mut skip_deeper_than: Option<usize> = None;
    for (start, end) in lines(source, open_end, boundary) {
        let line = line_at(source, start, end);
        let blank = line.start == line.significant_end;
        let (indent, indent_end, _has_tab) =
            leading_indent(source, line.start, line.significant_end);
        if let Some(skip_indent) = skip_deeper_than {
            if blank || indent > skip_indent {
                continue;
            }
            skip_deeper_than = None;
        }
        if blank {
            continue;
        }
        if is_fence_line(source, line) {
            return Some(line);
        }
        if let Some(key) = parse_mapping_key(source, indent_end, line.significant_end) {
            let value_start = skip_horizontal(source, key.colon + 1, line.significant_end);
            if line_opens_block_scalar(&source[value_start..line.significant_end]) {
                skip_deeper_than = Some(indent);
            }
        } else if is_dash_at(source, indent_end, line.significant_end) {
            let after_dash = skip_horizontal(source, indent_end + 1, line.significant_end);
            if line_opens_block_scalar(&source[after_dash..line.significant_end]) {
                skip_deeper_than = Some(indent);
            }
        }
    }
    None
}
fn skip_horizontal(source: &str, mut at: usize, end: usize) -> usize {
    while at < end && matches!(source.as_bytes()[at], b' ' | b'\t') {
        at += 1;
    }
    at
}
fn trim_horizontal_end(source: &str, start: usize, mut end: usize) -> usize {
    while end > start && matches!(source.as_bytes()[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    end
}
fn trailing_eof_whitespace_start(source: &str, minimum: usize) -> usize {
    let mut at = source.len();
    while at > minimum && matches!(source.as_bytes()[at - 1], b' ' | b'\t') {
        at -= 1;
    }
    at
}
fn structured_end(line: Line) -> usize {
    if line.newline_start < line.end {
        line.end
    } else {
        line.significant_end
    }
}
fn size(value: usize) -> Result<TextSize, ParseError> {
    TextSize::try_from_usize(value).map_err(|_| ParseError::SourceTooLarge { bytes: value })
}
fn diagnostic(
    code: OkfSyntaxDiagnosticCode,
    start: usize,
    end: usize,
    message: &'static str,
) -> TreeDiagnostic<OkfSyntaxDiagnosticCode> {
    TreeDiagnostic {
        code,
        severity: SyntaxSeverity::Error,
        message: message.into(),
        range: TextRange::new(size(start).unwrap(), size(end).unwrap()).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(value: &str) -> SourceText {
        SourceText::from_shared(Arc::new(value.into())).unwrap()
    }

    fn size(value: usize) -> TextSize {
        super::size(value).unwrap()
    }

    fn range(start: usize, end: usize) -> TextRange {
        TextRange::new(size(start), size(end)).unwrap()
    }

    fn element_width(element: &GreenElement<OkfMarkdownLanguage>) -> TextSize {
        match element {
            GreenElement::Node(node) => node.width(),
            GreenElement::Token(token) => token.width(),
        }
    }

    #[test]
    fn supplied_structure_is_only_an_internal_synchronization_map() {
        let text = source("# Class\n## Attributes\nvalue\n");
        let structure =
            Arc::new(crate::markdown::shell_map(&text, MarkdownDialect::WAML_DEFAULT).unwrap());
        let parsed =
            parse_with_structure(text, MarkdownDialect::WAML_DEFAULT, structure.clone()).unwrap();

        assert_eq!(parsed.structure.islands.len(), 1);
    }

    #[test]
    fn frontmatter_classifier_drives_full_and_window_consumption() {
        let text = source("---\ntype: uml.Class\n---\n# Class\n");
        let structure =
            Arc::new(crate::markdown::shell_map(&text, MarkdownDialect::WAML_DEFAULT).unwrap());
        let frontmatter = frontmatter_range(&text, &structure).unwrap().unwrap();
        let full = parse_with_structure(
            text.clone(),
            MarkdownDialect::WAML_DEFAULT,
            structure.clone(),
        )
        .unwrap();
        let parsed = parse_window(
            &text,
            &structure,
            ShellWindow {
                kind: ShellWindowKind::Frontmatter,
                range: frontmatter,
            },
        )
        .unwrap();

        assert_eq!(full.tree.write_to_string(), text.shared().as_str());
        assert_eq!(parsed.elements.len(), 1);
        assert_eq!(element_width(&parsed.elements[0]), frontmatter.len());
    }

    #[test]
    fn nested_frontmatter_mapping_has_recursive_tree_shape() {
        let text = source("---\na:\n  b: 1\n---\n");
        let shell = parse(text.clone(), MarkdownDialect::WAML_DEFAULT).unwrap();
        assert_eq!(shell.tree.write_to_string(), text.shared().as_str());

        let root = shell.tree.root();
        let frontmatter = root
            .children()
            .filter_map(crate::SyntaxElement::into_node)
            .find(|n| n.kind() == OkfMarkdownSyntaxKind::Frontmatter)
            .expect("frontmatter node");
        let outer_mapping = frontmatter
            .children()
            .filter_map(crate::SyntaxElement::into_node)
            .find(|n| n.kind() == OkfMarkdownSyntaxKind::FrontmatterMapping)
            .expect("outer mapping");
        let entry_a = outer_mapping
            .children()
            .filter_map(crate::SyntaxElement::into_node)
            .find(|n| n.kind() == OkfMarkdownSyntaxKind::FrontmatterEntry)
            .expect("entry a");
        let key_a = entry_a
            .children()
            .filter_map(crate::SyntaxElement::into_token)
            .find(|t| t.kind() == OkfMarkdownSyntaxKind::FrontmatterKey)
            .expect("key a");
        assert_eq!(key_a.text().write_to_string(), "a");
        let inner_mapping = entry_a
            .children()
            .filter_map(crate::SyntaxElement::into_node)
            .find(|n| n.kind() == OkfMarkdownSyntaxKind::FrontmatterMapping)
            .expect("inner mapping");
        let entry_b = inner_mapping
            .children()
            .filter_map(crate::SyntaxElement::into_node)
            .find(|n| n.kind() == OkfMarkdownSyntaxKind::FrontmatterEntry)
            .expect("entry b");
        let key_b = entry_b
            .children()
            .filter_map(crate::SyntaxElement::into_token)
            .find(|t| t.kind() == OkfMarkdownSyntaxKind::FrontmatterKey)
            .expect("key b");
        assert_eq!(key_b.text().write_to_string(), "b");
        let value_b = entry_b
            .children()
            .filter_map(crate::SyntaxElement::into_token)
            .find(|t| t.kind() == OkfMarkdownSyntaxKind::FrontmatterValue)
            .expect("value b");
        assert_eq!(value_b.text().write_to_string(), "1");
    }

    #[test]
    fn tail_window_reclassifies_raw_text_and_source_backed_eof_trivia_together() {
        let text = source("body   ");
        let structure =
            Arc::new(crate::markdown::shell_map(&text, MarkdownDialect::WAML_DEFAULT).unwrap());
        let parsed = parse_window(
            &text,
            &structure,
            ShellWindow {
                kind: ShellWindowKind::Tail,
                range: range(0, text.shared().len()),
            },
        )
        .unwrap();

        let width = parsed
            .elements
            .iter()
            .map(element_width)
            .try_fold(size(0), |sum, width| sum.checked_add(width))
            .unwrap();
        assert_eq!(width, size(text.shared().len()));
        assert!(matches!(
            parsed.elements.last(),
            Some(GreenElement::Token(token)) if token.kind() == OkfMarkdownSyntaxKind::EndOfFileToken
        ));
    }
}
