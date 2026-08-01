//! Compiles Markdown syntax queries into an immutable presentation plan.
//!
//! The syntax queries are the only classifier. Source bytes are read only after
//! a query has supplied their exact range, and never to infer Markdown.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

use waml_syntax::{
    MarkdownSemanticRole, MarkdownSourceRole, MarkdownSyntaxSnapshot, MarkdownSyntaxSpan,
    SourceText, SyntaxIdentity, TextRange, TextSize,
};

use super::{
    style::PresentationStyles, BlockDecorationKind, EmbeddedBlockKind, PresentationError,
    PresentationItem, PresentationItemId, PresentationPlan, PresentationRole, PresentedLink,
    TextRole,
};

/// Compiles one syntax snapshot into a validated presentation plan.
pub fn compile_presentation(
    snapshot: &MarkdownSyntaxSnapshot,
    styles: &PresentationStyles,
) -> Result<Arc<PresentationPlan>, PresentationError> {
    let queries = snapshot.queries();
    let text = snapshot.text();
    let zero = TextSize::try_from_usize(0)?;
    let full = TextRange::new(zero, text.len())?;
    let spans = queries.spans(full).collect::<Vec<_>>();

    let mut builder = PlanBuilder::new(styles);
    let mut wrappers = WrapperStack::default();
    let headings = queries.headings().cloned().collect::<Vec<_>>();

    for span in &spans {
        if span.range.start() == span.range.end() {
            continue;
        }
        let role = if let Some(role) = marker_role(span, snapshot, text) {
            wrappers.observe_marker(span);
            role
        } else {
            content_role(span, snapshot, text, &headings, &wrappers)
        };
        builder.push_text(span.range, role, span.owner);
    }

    add_decorations(&mut builder, &spans, snapshot)?;

    let plan = PresentationPlan {
        revision: snapshot.revision(),
        source_len: text.len(),
        items: builder.items.into(),
        links: builder.links.into(),
    };
    plan.validate_source_partition()?;
    Ok(Arc::new(plan))
}

/// Renders a plan into the stable golden text format.
///
/// Syntax identities are process-global counters, so the golden renumbers each
/// owner by first appearance. Identity *relationships* stay visible; the exact
/// counter value does not leak into the golden.
pub fn render_plan_golden(plan: &PresentationPlan, text: &SourceText) -> String {
    let mut out = String::new();
    let mut owner_numbers: HashMap<SyntaxIdentity, u32> = HashMap::new();
    let mut next_owner = 1;
    let mut number = |owner: SyntaxIdentity, owners: &mut HashMap<SyntaxIdentity, u32>| -> u32 {
        *owners.entry(owner).or_insert_with(|| {
            let value = next_owner;
            next_owner += 1;
            value
        })
    };
    for item in plan.items.iter() {
        let range = item.source_range();
        let (start, end) = (range.start().to_usize(), range.end().to_usize());
        match item {
            PresentationItem::TextRun { id, role, .. } => out.push_str(&format!(
                "TEXT {start}..{end} owner={} ordinal={} role={role:?} source={}\n",
                number(id.owner, &mut owner_numbers),
                id.fragment_ordinal,
                escape(text.slice(range).unwrap_or("")),
            )),
            PresentationItem::BlockDecoration { id, kind, .. } => out.push_str(&format!(
                "BLOCK {start}..{end} owner={} ordinal={} kind={kind:?}\n",
                number(id.owner, &mut owner_numbers),
                id.fragment_ordinal,
            )),
            PresentationItem::EmbeddedBlock { id, kind, .. } => out.push_str(&format!(
                "EMBED {start}..{end} owner={} ordinal={} kind={kind:?}\n",
                number(id.owner, &mut owner_numbers),
                id.fragment_ordinal,
            )),
        }
    }
    for link in plan.links.iter() {
        out.push_str(&format!(
            "LINK {}..{} owner={} destination={}\n",
            link.source_range.start().to_usize(),
            link.source_range.end().to_usize(),
            number(link.owner, &mut owner_numbers),
            escape(&link.destination),
        ));
    }
    out
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Inline constructs whose markers bracket their content.
#[derive(Default)]
struct WrapperStack {
    open: Vec<(SyntaxIdentity, MarkdownSemanticRole)>,
}

impl WrapperStack {
    fn observe_marker(&mut self, span: &MarkdownSyntaxSpan) {
        let role = span.semantic_role;
        if !matches!(
            role,
            MarkdownSemanticRole::Emphasis
                | MarkdownSemanticRole::Strong
                | MarkdownSemanticRole::Strikethrough
                | MarkdownSemanticRole::CodeSpan
        ) {
            return;
        }
        if let Some(index) = self
            .open
            .iter()
            .position(|(owner, open_role)| *owner == span.owner && *open_role == role)
        {
            self.open.remove(index);
        } else {
            self.open.push((span.owner, role));
        }
    }

    /// The composed inline role for content inside the open wrappers.
    fn content_role(&self) -> Option<TextRole> {
        if self
            .open
            .iter()
            .any(|(_, role)| *role == MarkdownSemanticRole::CodeSpan)
        {
            return Some(TextRole::InlineCode);
        }
        let strong = self
            .open
            .iter()
            .any(|(_, role)| *role == MarkdownSemanticRole::Strong);
        let emphasis = self
            .open
            .iter()
            .any(|(_, role)| *role == MarkdownSemanticRole::Emphasis);
        let strike = self
            .open
            .iter()
            .any(|(_, role)| *role == MarkdownSemanticRole::Strikethrough);
        match (strong, emphasis, strike) {
            (true, true, _) => Some(TextRole::StrongEmphasis),
            (true, false, _) => Some(TextRole::Strong),
            (false, true, _) => Some(TextRole::Emphasis),
            (false, false, true) => Some(TextRole::Strikethrough),
            (false, false, false) => None,
        }
    }
}

/// The visible role of a syntax-marker span, or `None` when the span carries
/// content.
fn marker_role(
    span: &MarkdownSyntaxSpan,
    snapshot: &MarkdownSyntaxSnapshot,
    text: &SourceText,
) -> Option<TextRole> {
    if span.source_role != MarkdownSourceRole::SyntaxMarker {
        return None;
    }
    let role = match span.semantic_role {
        MarkdownSemanticRole::Recovery => TextRole::Recovery,
        MarkdownSemanticRole::FencedCode => TextRole::CodeFence,
        MarkdownSemanticRole::List | MarkdownSemanticRole::ListItem => TextRole::ListMarker,
        MarkdownSemanticRole::TaskMarker => TextRole::TaskMarker,
        MarkdownSemanticRole::BlockQuote => TextRole::QuoteMarker,
        MarkdownSemanticRole::Table
        | MarkdownSemanticRole::TableHead
        | MarkdownSemanticRole::TableBody
        | MarkdownSemanticRole::TableRow
        | MarkdownSemanticRole::TableCell => TextRole::TableDelimiter,
        MarkdownSemanticRole::SoftBreak | MarkdownSemanticRole::HardBreak => TextRole::LineBreak,
        MarkdownSemanticRole::Whitespace => whitespace_role(span.range, text),
        MarkdownSemanticRole::Link | MarkdownSemanticRole::Image => {
            if destination_range(span.owner, snapshot) == Some(span.range) {
                TextRole::LinkDestination
            } else {
                TextRole::SyntaxMarker
            }
        }
        MarkdownSemanticRole::Frontmatter => TextRole::Frontmatter,
        MarkdownSemanticRole::Document
        | MarkdownSemanticRole::Paragraph
        | MarkdownSemanticRole::Heading
        | MarkdownSemanticRole::ThematicBreak
        | MarkdownSemanticRole::IndentedCode
        | MarkdownSemanticRole::HtmlBlock
        | MarkdownSemanticRole::LinkDefinition
        | MarkdownSemanticRole::Text
        | MarkdownSemanticRole::Escape
        | MarkdownSemanticRole::Entity
        | MarkdownSemanticRole::CodeSpan
        | MarkdownSemanticRole::Emphasis
        | MarkdownSemanticRole::Strong
        | MarkdownSemanticRole::Strikethrough
        | MarkdownSemanticRole::Autolink
        | MarkdownSemanticRole::RawHtml
        | MarkdownSemanticRole::WamlSection => TextRole::SyntaxMarker,
    };
    Some(role)
}

fn content_role(
    span: &MarkdownSyntaxSpan,
    snapshot: &MarkdownSyntaxSnapshot,
    text: &SourceText,
    headings: &[waml_syntax::MarkdownHeading],
    wrappers: &WrapperStack,
) -> TextRole {
    match span.semantic_role {
        MarkdownSemanticRole::Recovery => return TextRole::Recovery,
        MarkdownSemanticRole::RawHtml | MarkdownSemanticRole::HtmlBlock => {
            return TextRole::RawHtml
        }
        MarkdownSemanticRole::Frontmatter => return TextRole::Frontmatter,
        MarkdownSemanticRole::FencedCode | MarkdownSemanticRole::IndentedCode => {
            return fenced_content_role(span, snapshot)
        }
        MarkdownSemanticRole::Whitespace => return whitespace_role(span.range, text),
        MarkdownSemanticRole::SoftBreak | MarkdownSemanticRole::HardBreak => {
            return TextRole::LineBreak
        }
        MarkdownSemanticRole::Document
        | MarkdownSemanticRole::BlockQuote
        | MarkdownSemanticRole::List
        | MarkdownSemanticRole::ListItem
        | MarkdownSemanticRole::Paragraph
        | MarkdownSemanticRole::Heading
        | MarkdownSemanticRole::ThematicBreak
        | MarkdownSemanticRole::LinkDefinition
        | MarkdownSemanticRole::Table
        | MarkdownSemanticRole::TableHead
        | MarkdownSemanticRole::TableBody
        | MarkdownSemanticRole::TableRow
        | MarkdownSemanticRole::TableCell
        | MarkdownSemanticRole::Text
        | MarkdownSemanticRole::Escape
        | MarkdownSemanticRole::Entity
        | MarkdownSemanticRole::CodeSpan
        | MarkdownSemanticRole::Emphasis
        | MarkdownSemanticRole::Strong
        | MarkdownSemanticRole::Strikethrough
        | MarkdownSemanticRole::Link
        | MarkdownSemanticRole::Image
        | MarkdownSemanticRole::Autolink
        | MarkdownSemanticRole::TaskMarker
        | MarkdownSemanticRole::WamlSection => {}
    }
    if is_link_label(span.range, snapshot) {
        return TextRole::LinkLabel;
    }
    if let Some(role) = wrappers.content_role() {
        return role;
    }
    if let Some(heading) = headings
        .iter()
        .find(|heading| contains(heading.content_range, span.range))
    {
        return TextRole::Heading(heading.level);
    }
    if text
        .slice(span.range)
        .is_ok_and(|slice| !slice.is_empty() && slice.chars().all(char::is_whitespace))
    {
        return whitespace_role(span.range, text);
    }
    TextRole::Body
}

fn fenced_content_role(span: &MarkdownSyntaxSpan, snapshot: &MarkdownSyntaxSnapshot) -> TextRole {
    let Some(fenced) = snapshot.queries().fenced_code(span.owner) else {
        return TextRole::CodeContent;
    };
    if fenced
        .info_range
        .is_some_and(|info| contains(info, span.range))
    {
        return TextRole::CodeInfo;
    }
    TextRole::CodeContent
}

fn whitespace_role(range: TextRange, text: &SourceText) -> TextRole {
    if text
        .slice(range)
        .is_ok_and(|slice| slice.contains('\n') || slice.contains('\r'))
    {
        TextRole::LineBreak
    } else {
        TextRole::Whitespace
    }
}

fn destination_range(
    owner: SyntaxIdentity,
    snapshot: &MarkdownSyntaxSnapshot,
) -> Option<TextRange> {
    let queries = snapshot.queries();
    queries
        .image(owner)
        .and_then(|image| image.source_definition_range)
        .or_else(|| queries.link(owner).and_then(|link| link.destination_range))
}

fn is_link_label(range: TextRange, snapshot: &MarkdownSyntaxSnapshot) -> bool {
    snapshot
        .queries()
        .links()
        .filter(|link| snapshot.queries().image(link.owner).is_none())
        .any(|link| contains(link.content_range, range))
}

fn contains(outer: TextRange, inner: TextRange) -> bool {
    outer.start() <= inner.start() && inner.end() <= outer.end()
}

fn add_decorations(
    builder: &mut PlanBuilder<'_>,
    spans: &[&MarkdownSyntaxSpan],
    snapshot: &MarkdownSyntaxSnapshot,
) -> Result<(), PresentationError> {
    let queries = snapshot.queries();
    // One decoration per queried owner, in first-appearance source order.
    let mut extents: BTreeMap<(u64, usize), (SyntaxIdentity, MarkdownSemanticRole, TextRange)> =
        BTreeMap::new();
    let mut head_pipes: HashMap<SyntaxIdentity, u32> = HashMap::new();
    for span in spans {
        if matches!(span.semantic_role, MarkdownSemanticRole::TableHead)
            && span.source_role == MarkdownSourceRole::SyntaxMarker
        {
            *head_pipes.entry(span.owner).or_default() += 1;
        }
        let entry = extents
            .entry((span.owner.get(), span.semantic_role as usize))
            .or_insert((span.owner, span.semantic_role, span.range));
        entry.2 = TextRange::new(
            entry.2.start().min(span.range.start()),
            entry.2.end().max(span.range.end()),
        )?;
    }

    let mut ordered = extents.into_values().collect::<Vec<_>>();
    ordered.sort_by_key(|(owner, role, range)| {
        (range.start().to_usize(), owner.get(), *role as usize)
    });

    let mut seen_quotes = HashSet::new();
    for (owner, role, range) in ordered {
        match role {
            MarkdownSemanticRole::BlockQuote => {
                if seen_quotes.insert(owner) {
                    builder.push_block(owner, range, BlockDecorationKind::QuoteRule);
                }
            }
            MarkdownSemanticRole::CodeSpan => {
                builder.push_block(owner, range, BlockDecorationKind::InlineCodeFill);
            }
            MarkdownSemanticRole::FencedCode => {
                let source_range = queries
                    .fenced_code(owner)
                    .map_or(range, |fenced| fenced.source_range);
                builder.push_block(owner, source_range, BlockDecorationKind::FencedCodeSurface);
            }
            MarkdownSemanticRole::ThematicBreak => {
                builder.push_block(owner, range, BlockDecorationKind::ThematicRule);
            }
            MarkdownSemanticRole::TaskMarker => {
                let checked = queries
                    .list(owner)
                    .and_then(|list| list.task)
                    .is_some_and(|task| matches!(task, waml_syntax::TaskListState::Checked));
                builder.push_block(owner, range, BlockDecorationKind::TaskCheckbox { checked });
            }
            MarkdownSemanticRole::TableHead => {
                let columns = head_pipes
                    .get(&owner)
                    .copied()
                    .unwrap_or(1)
                    .saturating_sub(1);
                builder.push_block(owner, range, BlockDecorationKind::TableGrid { columns });
                builder.push_block(owner, range, BlockDecorationKind::TableHeaderFill);
            }
            _ => {}
        }
    }

    for image in queries.images() {
        builder.push_embed(
            image.owner,
            image.source_range,
            EmbeddedBlockKind::Image {
                destination: image.source.clone(),
                alt: Arc::from(snapshot.text().slice(image.alt_range).unwrap_or_default()),
                title: image.title.clone(),
            },
        );
    }

    for link in queries.links() {
        if queries.image(link.owner).is_some() {
            // An image is an embedded block, not a navigable link.
            continue;
        }
        builder.links.push(PresentedLink {
            owner: link.owner,
            source_range: link.source_range,
            destination: link.destination.clone(),
            title: link.title.clone(),
        });
    }
    Ok(())
}

struct PlanBuilder<'a> {
    styles: &'a PresentationStyles,
    items: Vec<PresentationItem>,
    links: Vec<PresentedLink>,
    ordinals: HashMap<(SyntaxIdentity, PresentationRole), u32>,
}

impl<'a> PlanBuilder<'a> {
    fn new(styles: &'a PresentationStyles) -> Self {
        Self {
            styles,
            items: Vec::new(),
            links: Vec::new(),
            ordinals: HashMap::new(),
        }
    }

    fn next_ordinal(&mut self, owner: SyntaxIdentity, role: PresentationRole) -> u32 {
        let slot = self.ordinals.entry((owner, role)).or_default();
        let ordinal = *slot;
        *slot += 1;
        ordinal
    }

    fn push_text(&mut self, range: TextRange, role: TextRole, owner: SyntaxIdentity) {
        let presentation_role = PresentationRole::Text(role);
        let fragment_ordinal = self.next_ordinal(owner, presentation_role);
        self.items.push(PresentationItem::TextRun {
            id: PresentationItemId {
                owner,
                role: presentation_role,
                fragment_ordinal,
            },
            range,
            role,
            style: self.styles.text_style(role),
        });
    }

    fn push_block(
        &mut self,
        owner: SyntaxIdentity,
        source_range: TextRange,
        kind: BlockDecorationKind,
    ) {
        let role = PresentationRole::Block(kind.role());
        let fragment_ordinal = self.next_ordinal(owner, role);
        self.items.push(PresentationItem::BlockDecoration {
            id: PresentationItemId {
                owner,
                role,
                fragment_ordinal,
            },
            owner,
            source_range,
            kind,
        });
    }

    fn push_embed(
        &mut self,
        owner: SyntaxIdentity,
        source_range: TextRange,
        kind: EmbeddedBlockKind,
    ) {
        let role = PresentationRole::Embedded(kind.role());
        let fragment_ordinal = self.next_ordinal(owner, role);
        self.items.push(PresentationItem::EmbeddedBlock {
            id: PresentationItemId {
                owner,
                role,
                fragment_ordinal,
            },
            owner,
            source_range,
            kind,
        });
    }
}
