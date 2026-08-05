use std::{collections::HashMap, fs, path::Path};

use waml_syntax::{
    parse_markdown, syntax_identity, write_green_to, DocumentRevision, FencedCodeInfo,
    MarkdownDialect, MarkdownLinkKind, MarkdownListKind, MarkdownSemanticRole, MarkdownSourceRole,
    MarkdownSyntaxQueries, SourceText, SyntaxElement, SyntaxNode, TableAlignment, TaskListState,
    TextRange,
};

#[derive(serde::Deserialize)]
struct CommonMarkExample {
    markdown: String,
    html: String,
    example: u32,
    section: String,
}

fn commonmark_examples() -> Vec<CommonMarkExample> {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/commonmark-0.31.2/spec.json");
    let source = fs::read_to_string(&fixture)
        .unwrap_or_else(|error| panic!("read CommonMark fixture {}: {error}", fixture.display()));
    serde_json::from_str(&source).expect("deserialize CommonMark fixture")
}

#[derive(Debug)]
struct GfmExample {
    example: u32,
    section: String,
    markdown: String,
    html: String,
}

#[derive(Clone, Debug)]
enum ConformanceMetadata {
    None,
    Heading {
        level: u8,
    },
    List {
        kind: MarkdownListKind,
        task: Option<TaskListState>,
    },
    Cell {
        alignment: TableAlignment,
    },
    Link {
        destination: String,
        title: Option<String>,
        kind: MarkdownLinkKind,
    },
    Image {
        source: String,
        title: Option<String>,
    },
    FencedCode {
        info: String,
        fence_range: TextRange,
        content_range: TextRange,
    },
    Entity {
        value: String,
    },
    RawHtml {
        filtered_ranges: Vec<TextRange>,
    },
    ContainerText,
}

#[derive(Clone, Debug)]
enum ConformanceEvent {
    Source(String),
    Start {
        role: MarkdownSemanticRole,
        range: TextRange,
        metadata: ConformanceMetadata,
    },
    End {
        role: MarkdownSemanticRole,
    },
}

#[test]
fn canonical_renderer_rejects_wrong_semantic_role() {
    let events = conformance_events("# heading\n", MarkdownDialect::COMMONMARK_0_31_2);
    let mut mutated = events.clone();
    let role = mutated
        .iter_mut()
        .find_map(|event| match event {
            ConformanceEvent::Start { role, .. } if *role == MarkdownSemanticRole::Heading => {
                Some(role)
            }
            _ => None,
        })
        .expect("heading role");
    *role = MarkdownSemanticRole::Paragraph;

    assert_ne!(
        canonical_html(&events),
        canonical_html(&mutated),
        "changing a production semantic role must change rendered HTML"
    );
}

#[test]
fn canonical_renderer_rejects_missing_semantic_roles() {
    let events = conformance_events("*strong*\n", MarkdownDialect::COMMONMARK_0_31_2);
    let mut mutated = events.clone();
    mutated.retain(|event| {
        !matches!(
            event,
            ConformanceEvent::Start { .. } | ConformanceEvent::End { .. }
        )
    });

    assert_ne!(
        canonical_html(&events),
        canonical_html(&mutated),
        "removing production semantic roles must change rendered HTML"
    );
}

#[test]
fn extended_autolink_roles_expose_false_negatives_and_false_positives() {
    let markdown = "www.example.com\n";
    let events = conformance_events(markdown, MarkdownDialect::WAML_DEFAULT);
    assert!(events.iter().any(|event| matches!(
        event,
        ConformanceEvent::Start {
            role: MarkdownSemanticRole::Autolink,
            metadata: ConformanceMetadata::Link {
                kind: MarkdownLinkKind::ExtendedAutolink,
                ..
            },
            ..
        }
    )));
    assert_eq!(
        canonical_html(&events),
        "<p><a href=\"http://www.example.com\">www.example.com</a></p>\n"
    );

    let mut missing_role = events.clone();
    missing_role.retain(|event| {
        !matches!(
            event,
            ConformanceEvent::Start {
                role: MarkdownSemanticRole::Autolink,
                ..
            } | ConformanceEvent::End {
                role: MarkdownSemanticRole::Autolink
            }
        )
    });
    assert_ne!(
        canonical_html(&events),
        canonical_html(&missing_role),
        "removing the extended-autolink role must expose a false negative"
    );

    for markdown in ["person@example.com_\n", "person@example.com-\n"] {
        let events = conformance_events(markdown, MarkdownDialect::WAML_DEFAULT);
        assert!(
            !events.iter().any(|event| matches!(
                event,
                ConformanceEvent::Start {
                    metadata: ConformanceMetadata::Link {
                        kind: MarkdownLinkKind::ExtendedAutolink,
                        ..
                    },
                    ..
                }
            )),
            "{markdown:?} must not gain extended-autolink metadata"
        );
        assert_eq!(
            canonical_html(&events),
            format!("<p>{}</p>\n", markdown.trim_end()),
            "{markdown:?} must render as plain text"
        );
    }
}

fn fixture(path: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn gfm_extension_examples() -> Vec<GfmExample> {
    const SECTIONS: [&str; 5] = [
        "Tables (extension)",
        "Task list items (extension)",
        "Strikethrough (extension)",
        "Autolinks (extension)",
        "Disallowed Raw HTML (extension)",
    ];
    let source =
        fs::read_to_string(fixture("tests/fixtures/gfm-0.29/spec.txt")).expect("read GFM fixture");
    let mut section = None;
    let mut examples = Vec::new();
    let mut example = 0_u32;
    let mut lines = source.lines().peekable();
    while let Some(line) = lines.next() {
        if let Some(heading) = line.strip_prefix("## ") {
            section = SECTIONS.contains(&heading).then(|| heading.to_owned());
            continue;
        }
        if !line.starts_with("````") || !line.contains(" example") {
            continue;
        }
        example += 1;
        if section.is_none() {
            continue;
        }
        let markdown = read_fixture_part(&mut lines);
        let html = read_until_fence(&mut lines);
        examples.push(GfmExample {
            example,
            section: section.clone().expect("active section"),
            markdown,
            html,
        });
    }
    examples
}

fn read_until_fence<'a>(lines: &mut std::iter::Peekable<std::str::Lines<'a>>) -> String {
    let mut part = String::new();
    while let Some(&line) = lines.peek() {
        lines.next();
        if line.starts_with("````````````````````````````````") {
            break;
        }
        part.push_str(line);
        part.push('\n');
    }
    part
}

fn read_fixture_part<'a>(lines: &mut std::iter::Peekable<std::str::Lines<'a>>) -> String {
    let mut part = String::new();
    while let Some(&line) = lines.peek() {
        lines.next();
        if line == "." {
            break;
        }
        part.push_str(line);
        part.push('\n');
    }
    part
}

fn conformance_events(markdown: &str, dialect: MarkdownDialect) -> Vec<ConformanceEvent> {
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(markdown).expect("fixture source is valid"),
        dialect,
    )
    .unwrap_or_else(|error| panic!("production syntax parser accepts {markdown:?}: {error:?}"));
    let mut recovered = String::new();
    write_green_to(snapshot.tree().root_green(), &mut recovered)
        .expect("recover syntax-tree source");
    assert_eq!(
        recovered, markdown,
        "production tree must recover fixture source"
    );

    let mut events = Vec::new();
    events.push(ConformanceEvent::Source(recovered));
    collect_events(&snapshot.tree().root(), snapshot.queries(), &mut events);
    events
}

fn collect_events(
    node: &SyntaxNode<waml_syntax::OkfMarkdownLanguage>,
    queries: &MarkdownSyntaxQueries,
    events: &mut Vec<ConformanceEvent>,
) {
    let range = node.range();
    let owner = syntax_identity(node);
    let role = role_for_kind(node.kind());
    let Some(role) = role else {
        for child in node.children() {
            if let SyntaxElement::Node(child) = child {
                collect_events(&child, queries, events);
            }
        }
        return;
    };
    let metadata = metadata(role, owner, range, queries);
    events.push(ConformanceEvent::Start {
        role,
        range,
        metadata,
    });
    for child in node.children() {
        match child {
            SyntaxElement::Node(child) => collect_events(&child, queries, events),
            SyntaxElement::Token(token) => {
                if !matches!(
                    role,
                    MarkdownSemanticRole::ListItem
                        | MarkdownSemanticRole::Heading
                        | MarkdownSemanticRole::TableCell
                ) {
                    continue;
                }
                let token_text = token.text().write_to_string();
                if !token_text.trim().is_empty() {
                    if let Some(span) = queries.spans(token.range()).find(|span| {
                        span.range == token.range()
                            && owner.is_some_and(|owner| span.owner == owner)
                            && span.semantic_role == MarkdownSemanticRole::Text
                            && span.source_role == MarkdownSourceRole::Content
                    }) {
                        events.push(ConformanceEvent::Start {
                            role: span.semantic_role,
                            range: span.range,
                            metadata: ConformanceMetadata::ContainerText,
                        });
                        events.push(ConformanceEvent::End {
                            role: span.semantic_role,
                        });
                    }
                }
            }
        }
    }
    events.push(ConformanceEvent::End { role });
}

fn role_for_kind(kind: waml_syntax::OkfMarkdownSyntaxKind) -> Option<MarkdownSemanticRole> {
    use waml_syntax::OkfMarkdownSyntaxKind as K;
    Some(match kind {
        K::Root => MarkdownSemanticRole::Document,
        K::Frontmatter | K::FrontmatterEntry => MarkdownSemanticRole::Frontmatter,
        K::BlockQuote => MarkdownSemanticRole::BlockQuote,
        K::List => MarkdownSemanticRole::List,
        K::ListItem => MarkdownSemanticRole::ListItem,
        K::Paragraph => MarkdownSemanticRole::Paragraph,
        K::AtxHeading | K::SetextHeading => MarkdownSemanticRole::Heading,
        K::ThematicBreak => MarkdownSemanticRole::ThematicBreak,
        K::IndentedCodeBlock => MarkdownSemanticRole::IndentedCode,
        K::FencedCodeBlock => MarkdownSemanticRole::FencedCode,
        K::HtmlBlock => MarkdownSemanticRole::HtmlBlock,
        K::LinkReferenceDefinition => MarkdownSemanticRole::LinkDefinition,
        K::Table => MarkdownSemanticRole::Table,
        K::TableHead => MarkdownSemanticRole::TableHead,
        K::TableBody => MarkdownSemanticRole::TableBody,
        K::TableRow => MarkdownSemanticRole::TableRow,
        K::TableCell => MarkdownSemanticRole::TableCell,
        K::Text => MarkdownSemanticRole::Text,
        K::Escape => MarkdownSemanticRole::Escape,
        K::Entity => MarkdownSemanticRole::Entity,
        K::CodeSpan => MarkdownSemanticRole::CodeSpan,
        K::Emphasis => MarkdownSemanticRole::Emphasis,
        K::StrongEmphasis => MarkdownSemanticRole::Strong,
        K::Strikethrough => MarkdownSemanticRole::Strikethrough,
        K::Link => MarkdownSemanticRole::Link,
        K::Image => MarkdownSemanticRole::Image,
        K::Autolink => MarkdownSemanticRole::Autolink,
        K::RawHtml => MarkdownSemanticRole::RawHtml,
        K::SoftLineBreak => MarkdownSemanticRole::SoftBreak,
        K::HardLineBreak => MarkdownSemanticRole::HardBreak,
        K::SkippedTokensSyntax => MarkdownSemanticRole::Recovery,
        K::WamlSection => MarkdownSemanticRole::WamlSection,
        _ => return None,
    })
}

fn metadata(
    role: MarkdownSemanticRole,
    owner: Option<waml_syntax::SyntaxIdentity>,
    range: TextRange,
    queries: &MarkdownSyntaxQueries,
) -> ConformanceMetadata {
    match role {
        MarkdownSemanticRole::Heading => queries
            .heading(owner.expect("heading owner"))
            .map(|heading| ConformanceMetadata::Heading {
                level: heading.level,
            })
            .expect("heading metadata"),
        MarkdownSemanticRole::List | MarkdownSemanticRole::ListItem => queries
            .list(owner.expect("list owner"))
            .map(|list| ConformanceMetadata::List {
                kind: list.kind,
                task: list.task,
            })
            .unwrap_or(ConformanceMetadata::None),
        MarkdownSemanticRole::TableCell => queries
            .table_cell(owner.expect("cell owner"))
            .map(|cell| ConformanceMetadata::Cell {
                alignment: cell.alignment,
            })
            .expect("table-cell metadata"),
        MarkdownSemanticRole::Link | MarkdownSemanticRole::Autolink => queries
            .link(owner.expect("link owner"))
            .map(|link| ConformanceMetadata::Link {
                destination: link.destination.to_string(),
                title: link.title.as_deref().map(str::to_owned),
                kind: link.kind,
            })
            .expect("link metadata"),
        MarkdownSemanticRole::Image => queries
            .image(owner.expect("image owner"))
            .map(|image| ConformanceMetadata::Image {
                source: image.source.to_string(),
                title: image.title.as_deref().map(str::to_owned),
            })
            .expect("image metadata"),
        MarkdownSemanticRole::FencedCode => queries
            .fenced_code(owner.expect("fenced-code owner"))
            .map(fenced_metadata)
            .expect("fenced-code metadata"),
        MarkdownSemanticRole::Entity => queries
            .entities()
            .find(|entity| entity.source_range == range)
            .map(|entity| ConformanceMetadata::Entity {
                value: entity.value.to_string(),
            })
            .expect("entity metadata"),
        MarkdownSemanticRole::HtmlBlock | MarkdownSemanticRole::RawHtml => queries
            .raw_html(owner.expect("raw HTML owner"))
            .map(|html| ConformanceMetadata::RawHtml {
                filtered_ranges: html.filtered_ranges.to_vec(),
            })
            .expect("raw HTML metadata"),
        _ => ConformanceMetadata::None,
    }
}

fn fenced_metadata(code: &FencedCodeInfo) -> ConformanceMetadata {
    ConformanceMetadata::FencedCode {
        info: code.info.to_string(),
        fence_range: code.fence_range,
        content_range: code.content_range,
    }
}

fn canonical_html(events: &[ConformanceEvent]) -> String {
    let source = events
        .iter()
        .find_map(|event| match event {
            ConformanceEvent::Source(source) => Some(source.as_str()),
            _ => None,
        })
        .expect("conformance events include recovered source");
    let mut renderer = Renderer::new(source);
    for event in events {
        renderer.event(event);
    }
    renderer.finish()
}

struct Renderer<'a> {
    source: &'a str,
    html: String,
    suppressed: usize,
    list_item_depth: usize,
    list_indents: Vec<usize>,
    initial_code_prefixes: Vec<usize>,
    block_quote_depth: usize,
    list_tags: Vec<&'static str>,
    heading_levels: Vec<u8>,
    cell_tags: Vec<&'static str>,
    cell_content_started: Vec<bool>,
    table_head_depth: usize,
    images: Vec<ImageCapture>,
    raw_html_quote: Option<char>,
}

struct ImageCapture {
    source: String,
    title: Option<String>,
    alt: String,
}

impl<'a> Renderer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            html: String::new(),
            suppressed: 0,
            list_item_depth: 0,
            list_indents: Vec::new(),
            initial_code_prefixes: Vec::new(),
            block_quote_depth: 0,
            list_tags: Vec::new(),
            heading_levels: Vec::new(),
            cell_tags: Vec::new(),
            cell_content_started: Vec::new(),
            table_head_depth: 0,
            images: Vec::new(),
            raw_html_quote: None,
        }
    }

    fn event(&mut self, event: &ConformanceEvent) {
        match event {
            ConformanceEvent::Source(_) => {}
            ConformanceEvent::Start {
                role,
                range,
                metadata,
            } => self.start(*role, *range, metadata),
            ConformanceEvent::End { role } => self.end(*role),
        }
    }

    fn start(
        &mut self,
        role: MarkdownSemanticRole,
        range: TextRange,
        metadata: &ConformanceMetadata,
    ) {
        if self.suppressed > 0 {
            self.suppressed += 1;
            return;
        }
        let text = &self.source[range.start().to_usize()..range.end().to_usize()];
        if let Some(image) = self.images.last_mut() {
            match role {
                MarkdownSemanticRole::Text
                    if !matches!(metadata, ConformanceMetadata::ContainerText) =>
                {
                    image
                        .alt
                        .push_str(&normalize_inline_text(text, self.block_quote_depth));
                }
                MarkdownSemanticRole::Escape => {
                    image.alt.push_str(text.strip_prefix('\\').unwrap_or(text))
                }
                MarkdownSemanticRole::Entity => {
                    if let ConformanceMetadata::Entity { value } = metadata {
                        image.alt.push_str(value);
                    }
                }
                MarkdownSemanticRole::CodeSpan => image.alt.push_str(&code_span(text)),
                MarkdownSemanticRole::SoftBreak | MarkdownSemanticRole::HardBreak => {
                    image.alt.push('\n')
                }
                MarkdownSemanticRole::RawHtml => image.alt.push_str(text),
                _ => {}
            }
            return;
        }
        match role {
            MarkdownSemanticRole::Document => {}
            MarkdownSemanticRole::BlockQuote => {
                self.ensure_block_line();
                self.block_quote_depth += 1;
                self.html.push_str("<blockquote>\n");
            }
            MarkdownSemanticRole::List => {
                if !self.html.is_empty() && !self.html.ends_with('\n') {
                    self.html.push('\n');
                }
                match metadata {
                    ConformanceMetadata::List {
                        kind: MarkdownListKind::Ordered { start },
                        ..
                    } => {
                        self.list_tags.push("ol");
                        if *start == 1 {
                            self.html.push_str("<ol>\n")
                        } else {
                            self.html.push_str(&format!("<ol start=\"{start}\">\n"))
                        }
                    }
                    _ => {
                        self.list_tags.push("ul");
                        self.html.push_str("<ul>\n");
                    }
                }
            }
            MarkdownSemanticRole::ListItem => {
                self.list_item_depth += 1;
                self.list_indents.push(list_item_indent(text));
                self.initial_code_prefixes
                    .push(list_initial_code_prefix(text));
                self.html.push_str("<li>");
                if let ConformanceMetadata::List {
                    task: Some(state), ..
                } = metadata
                {
                    if *state == TaskListState::Checked {
                        self.html
                            .push_str("<input checked=\"\" disabled=\"\" type=\"checkbox\"> ");
                    } else {
                        self.html
                            .push_str("<input disabled=\"\" type=\"checkbox\"> ");
                    }
                }
            }
            MarkdownSemanticRole::Paragraph => {
                if self.html.ends_with("<li>") {
                    self.html.push('\n');
                }
                self.html.push_str("<p>");
            }
            MarkdownSemanticRole::Heading => {
                if let ConformanceMetadata::Heading { level } = metadata {
                    self.ensure_block_line();
                    self.heading_levels.push(*level);
                    self.html.push_str(&format!("<h{level}>"));
                }
            }
            MarkdownSemanticRole::ThematicBreak => {
                if self.html.ends_with("<li>") {
                    self.html.push('\n');
                }
                self.html.push_str("<hr />\n");
                self.suppressed = 1;
            }
            MarkdownSemanticRole::IndentedCode => {
                if self.html.ends_with("<li>") {
                    self.html.push('\n');
                }
                self.html.push_str("<pre><code>");
                let mut code = indented_code(
                    text,
                    self.list_indents.iter().sum::<usize>() + self.block_quote_depth * 2,
                    self.block_quote_depth,
                    self.list_item_depth,
                );
                if let Some(prefix) = self.initial_code_prefixes.last_mut() {
                    code.insert_str(0, &" ".repeat(*prefix));
                    *prefix = 0;
                }
                self.html.push_str(&escape_html(&code, true));
                self.html.push_str("</code></pre>\n");
                self.suppressed = 1;
            }
            MarkdownSemanticRole::FencedCode => {
                if let ConformanceMetadata::FencedCode {
                    info,
                    fence_range,
                    content_range,
                } = metadata
                {
                    self.ensure_block_line();
                    self.html.push_str("<pre><code");
                    if let Some(language) = info
                        .split_ascii_whitespace()
                        .next()
                        .filter(|value| !value.is_empty())
                    {
                        self.html.push_str(" class=\"language-");
                        self.html.push_str(&escape_html(language, true));
                        self.html.push('"');
                    }
                    self.html.push('>');
                    let content = &self.source
                        [content_range.start().to_usize()..content_range.end().to_usize()];
                    let indent = fence_indent(
                        self.source,
                        fence_range.start().to_usize(),
                        self.block_quote_depth,
                        self.list_item_depth,
                    );
                    let content = fenced_content(
                        content,
                        self.block_quote_depth,
                        self.list_indents.iter().sum(),
                        indent,
                    );
                    self.html.push_str(&escape_html(&content, true));
                    self.html.push_str("</code></pre>\n");
                    self.suppressed = 1;
                }
            }
            MarkdownSemanticRole::HtmlBlock | MarkdownSemanticRole::RawHtml => {
                if let ConformanceMetadata::RawHtml { filtered_ranges } = metadata {
                    if role == MarkdownSemanticRole::HtmlBlock {
                        self.ensure_block_line();
                    }
                    let rendered = if role == MarkdownSemanticRole::HtmlBlock {
                        self.html.push_str(container_prefix(
                            self.source,
                            range.start().to_usize(),
                            self.block_quote_depth,
                            self.list_item_depth,
                        ));
                        fenced_content(
                            text,
                            self.block_quote_depth,
                            self.list_indents.iter().sum(),
                            0,
                        )
                    } else {
                        text.to_owned()
                    };
                    self.html.push_str(&filter_html_tags(
                        &rendered,
                        range.start().to_usize(),
                        filtered_ranges,
                    ));
                    if role == MarkdownSemanticRole::RawHtml {
                        for character in text
                            .chars()
                            .filter(|character| matches!(character, '"' | '\''))
                        {
                            match self.raw_html_quote {
                                Some(open) if open == character => self.raw_html_quote = None,
                                None => self.raw_html_quote = Some(character),
                                _ => {}
                            }
                        }
                    }
                    self.suppressed = 1;
                }
            }
            MarkdownSemanticRole::LinkDefinition => self.suppressed = 1,
            MarkdownSemanticRole::Table => {
                self.ensure_block_line();
                self.html.push_str("<table>\n");
            }
            MarkdownSemanticRole::TableHead => {
                self.table_head_depth += 1;
                self.html.push_str("<thead>\n<tr>\n");
            }
            MarkdownSemanticRole::TableBody => self.html.push_str("<tbody>\n"),
            MarkdownSemanticRole::TableRow => self.html.push_str("<tr>\n"),
            MarkdownSemanticRole::TableCell => {
                if let ConformanceMetadata::Cell { alignment } = metadata {
                    let tag = if self.table_head_depth > 0 {
                        "th"
                    } else {
                        "td"
                    };
                    self.cell_tags.push(tag);
                    self.cell_content_started.push(false);
                    self.html.push('<');
                    self.html.push_str(tag);
                    match alignment {
                        TableAlignment::Left => self.html.push_str(" align=\"left\""),
                        TableAlignment::Center => self.html.push_str(" align=\"center\""),
                        TableAlignment::Right => self.html.push_str(" align=\"right\""),
                        TableAlignment::None => {}
                    }
                    self.html.push('>');
                }
            }
            MarkdownSemanticRole::Text => {
                let mut text = if matches!(metadata, ConformanceMetadata::ContainerText) {
                    container_text(text, self.block_quote_depth, self.list_indents.iter().sum())
                        .trim()
                        .to_owned()
                } else {
                    normalize_inline_text(text, self.block_quote_depth)
                };
                if self.html.ends_with('\n') {
                    text = text.trim_start_matches([' ', '\t']).to_owned();
                }
                if let Some(started) = self.cell_content_started.last_mut() {
                    if !*started {
                        text = text.trim_start().to_owned();
                    }
                    *started = true;
                }
                let raw_continuation = text.contains("/>")
                    && self
                        .html
                        .rsplit("<p>")
                        .next()
                        .is_some_and(|fragment| fragment.contains('<'));
                if raw_continuation {
                    self.html.push_str(&text);
                } else {
                    self.html
                        .push_str(&escape_html(&text, self.raw_html_quote.is_none()));
                }
                self.suppressed = 1;
            }
            MarkdownSemanticRole::Escape => {
                self.html
                    .push_str(&escape_html(text.strip_prefix('\\').unwrap_or(text), true));
                self.suppressed = 1;
            }
            MarkdownSemanticRole::Entity => {
                if let ConformanceMetadata::Entity { value } = metadata {
                    self.html.push_str(&escape_html(value, true));
                    self.suppressed = 1;
                }
            }
            MarkdownSemanticRole::CodeSpan => {
                self.html.push_str("<code>");
                let mut code = code_span(text);
                if !self.cell_tags.is_empty() {
                    code = code.replace("\\|", "|");
                }
                self.html.push_str(&escape_html(&code, true));
                self.html.push_str("</code>");
                self.suppressed = 1;
            }
            MarkdownSemanticRole::Emphasis => self.html.push_str("<em>"),
            MarkdownSemanticRole::Strong => self.html.push_str("<strong>"),
            MarkdownSemanticRole::Strikethrough => self.html.push_str("<del>"),
            MarkdownSemanticRole::Link | MarkdownSemanticRole::Autolink => {
                if let ConformanceMetadata::Link {
                    destination,
                    title,
                    kind,
                } = metadata
                {
                    self.html.push_str("<a href=\"");
                    self.html
                        .push_str(&escape_html(&escape_url(destination), true));
                    self.html.push('"');
                    if let Some(title) = title {
                        self.html.push_str(" title=\"");
                        self.html.push_str(&escape_html(title, true));
                        self.html.push('"');
                    }
                    self.html.push('>');
                    if matches!(
                        kind,
                        MarkdownLinkKind::Autolink | MarkdownLinkKind::ExtendedAutolink
                    ) {
                        self.html
                            .push_str(&escape_html(text.trim_matches(['<', '>']), false));
                        self.html.push_str("</a>");
                        self.suppressed = 1;
                    }
                }
            }
            MarkdownSemanticRole::Image => {
                if let ConformanceMetadata::Image { source, title, .. } = metadata {
                    self.images.push(ImageCapture {
                        source: source.clone(),
                        title: title.clone(),
                        alt: String::new(),
                    });
                }
            }
            MarkdownSemanticRole::SoftBreak => {
                self.html.push('\n');
                self.suppressed = 1;
            }
            MarkdownSemanticRole::HardBreak => {
                self.html.push_str("<br />\n");
                self.suppressed = 1;
            }
            MarkdownSemanticRole::Whitespace
            | MarkdownSemanticRole::Recovery
            | MarkdownSemanticRole::Frontmatter
            | MarkdownSemanticRole::FrontmatterKey
            | MarkdownSemanticRole::FrontmatterPunctuation
            | MarkdownSemanticRole::FrontmatterFence
            | MarkdownSemanticRole::FrontmatterComment
            | MarkdownSemanticRole::FrontmatterScalar
            | MarkdownSemanticRole::FrontmatterBlockScalar
            | MarkdownSemanticRole::FrontmatterInvalid
            | MarkdownSemanticRole::TaskMarker
            | MarkdownSemanticRole::WamlSection => {}
        }
    }

    fn end(&mut self, role: MarkdownSemanticRole) {
        if !self.images.is_empty() {
            if role == MarkdownSemanticRole::Image {
                let image = self.images.pop().expect("checked image capture");
                self.html.push_str("<img src=\"");
                self.html
                    .push_str(&escape_html(&escape_url(&image.source), true));
                self.html.push_str("\" alt=\"");
                self.html
                    .push_str(&escape_html(&plain_alt(&image.alt), true));
                self.html.push('"');
                if let Some(title) = image.title {
                    self.html.push_str(" title=\"");
                    self.html.push_str(&escape_html(&title, true));
                    self.html.push('"');
                }
                self.html.push_str(" />");
            }
            return;
        }
        if self.suppressed > 0 {
            self.suppressed -= 1;
            return;
        }
        match role {
            MarkdownSemanticRole::BlockQuote => {
                self.html.push_str("</blockquote>\n");
                self.block_quote_depth = self.block_quote_depth.saturating_sub(1);
            }
            MarkdownSemanticRole::List => {
                if let Some(tag) = self.list_tags.pop() {
                    self.html.push_str(&format!("</{tag}>\n"));
                }
            }
            MarkdownSemanticRole::ListItem => {
                self.html.push_str("</li>\n");
                self.list_item_depth = self.list_item_depth.saturating_sub(1);
                self.list_indents.pop();
                self.initial_code_prefixes.pop();
            }
            MarkdownSemanticRole::Paragraph => self.html.push_str("</p>\n"),
            MarkdownSemanticRole::Heading => {
                if let Some(level) = self.heading_levels.pop() {
                    self.html.push_str(&format!("</h{level}>\n"));
                }
            }
            MarkdownSemanticRole::Table => self.html.push_str("</table>\n"),
            MarkdownSemanticRole::TableHead => {
                self.html.push_str("</tr>\n</thead>\n");
                self.table_head_depth = self.table_head_depth.saturating_sub(1);
            }
            MarkdownSemanticRole::TableBody => self.html.push_str("</tbody>\n"),
            MarkdownSemanticRole::TableRow => self.html.push_str("</tr>\n"),
            MarkdownSemanticRole::TableCell => {
                while self.html.ends_with([' ', '\t']) {
                    self.html.pop();
                }
                self.cell_content_started.pop();
                if let Some(tag) = self.cell_tags.pop() {
                    self.html.push_str(&format!("</{tag}>\n"));
                }
            }
            MarkdownSemanticRole::Emphasis => self.html.push_str("</em>"),
            MarkdownSemanticRole::Strong => self.html.push_str("</strong>"),
            MarkdownSemanticRole::Strikethrough => self.html.push_str("</del>"),
            MarkdownSemanticRole::Link => self.html.push_str("</a>"),
            _ => {}
        }
    }

    fn ensure_block_line(&mut self) {
        if self.list_item_depth > 0 && !self.html.ends_with('\n') {
            self.html.push('\n');
        }
    }

    fn finish(self) -> String {
        self.html
    }
}

fn escape_html(value: &str, attribute: bool) -> String {
    let mut escaped = value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    if attribute {
        escaped = escaped.replace('"', "&quot;");
    }
    escaped
}

fn filter_html_tags(value: &str, source_start: usize, filtered_ranges: &[TextRange]) -> String {
    let filtered_starts: Vec<_> = filtered_ranges
        .iter()
        .map(|range| range.start().to_usize())
        .collect();
    let mut rendered = String::with_capacity(value.len());
    for (offset, character) in value.char_indices() {
        if character == '<' && filtered_starts.contains(&(source_start + offset)) {
            rendered.push_str("&lt;");
        } else {
            rendered.push(character);
        }
    }
    rendered
}

fn escape_url(value: &str) -> String {
    let mut escaped = String::new();
    for byte in value.bytes() {
        if byte.is_ascii()
            && !matches!(
                byte,
                b'\\' | b' ' | b'"' | b'<' | b'>' | b'[' | b']' | b'`' | 0..=0x1f | 0x7f
            )
        {
            escaped.push(byte as char);
        } else {
            escaped.push_str(&format!("%{byte:02X}"));
        }
    }
    escaped
}

fn list_item_indent(value: &str) -> usize {
    let line = value.lines().next().unwrap_or(value);
    let bytes = line.as_bytes();
    let mut at = bytes
        .iter()
        .take(3)
        .take_while(|byte| **byte == b' ')
        .count();
    if matches!(bytes.get(at), Some(b'-' | b'+' | b'*')) {
        at += 1;
    } else {
        at += bytes[at..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        if matches!(bytes.get(at), Some(b'.' | b')')) {
            at += 1;
        }
    }
    let spaces = bytes[at..]
        .iter()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count();
    if bytes[at..at + spaces].contains(&b'\t') {
        return at + 1;
    }
    at + if (1..=4).contains(&spaces) { spaces } else { 1 }
}

fn list_initial_code_prefix(value: &str) -> usize {
    let line = value.lines().next().unwrap_or(value);
    let bytes = line.as_bytes();
    let mut at = bytes
        .iter()
        .take(3)
        .take_while(|byte| **byte == b' ')
        .count();
    if matches!(bytes.get(at), Some(b'-' | b'+' | b'*')) {
        at += 1;
    } else {
        at += bytes[at..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        at += usize::from(matches!(bytes.get(at), Some(b'.' | b')')));
    }
    bytes[at..]
        .iter()
        .take_while(|byte| **byte == b' ')
        .count()
        .saturating_sub(5)
}

fn container_text(value: &str, quote_depth: usize, list_indent: usize) -> String {
    fenced_content(value, quote_depth, 0, 0)
        .split_inclusive('\n')
        .enumerate()
        .map(|(index, line)| {
            if index == 0 {
                line
            } else {
                let indent = line.bytes().take_while(|byte| *byte == b' ').count();
                &line[indent.min(list_indent)..]
            }
        })
        .collect()
}

fn fence_indent(source: &str, fence_start: usize, quote_depth: usize, list_depth: usize) -> usize {
    container_prefix(source, fence_start, quote_depth, list_depth)
        .chars()
        .take_while(|character| *character == ' ')
        .count()
}

fn container_prefix(
    source: &str,
    content_start: usize,
    quote_depth: usize,
    list_depth: usize,
) -> &str {
    let line_start = source[..content_start].rfind('\n').map_or(0, |at| at + 1);
    let mut prefix = &source[line_start..content_start];
    for _ in 0..quote_depth {
        prefix = prefix.trim_start_matches(' ');
        if let Some(rest) = prefix.strip_prefix('>') {
            prefix = rest.strip_prefix(' ').unwrap_or(rest);
        }
    }
    for _ in 0..list_depth {
        let trimmed = prefix.trim_start_matches(' ');
        let marker_end = if trimmed.starts_with(['-', '+', '*']) {
            1
        } else {
            let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
            digits
                + usize::from(
                    trimmed
                        .as_bytes()
                        .get(digits)
                        .is_some_and(|byte| matches!(byte, b'.' | b')')),
                )
        };
        if marker_end > 0 {
            prefix = trimmed[marker_end..]
                .strip_prefix(' ')
                .unwrap_or(&trimmed[marker_end..]);
        }
    }
    prefix
}

fn fenced_content(value: &str, quote_depth: usize, list_indent: usize, indent: usize) -> String {
    value
        .split_inclusive('\n')
        .map(|line| {
            let mut line = line;
            for _ in 0..quote_depth {
                let trimmed = line.trim_start_matches(' ');
                if let Some(rest) = trimmed.strip_prefix('>') {
                    line = rest.strip_prefix(' ').unwrap_or(rest);
                }
            }
            let present = line.bytes().take_while(|byte| *byte == b' ').count();
            line = &line[present.min(list_indent)..];
            for _ in 0..indent {
                line = line.strip_prefix(' ').unwrap_or(line);
            }
            line
        })
        .collect()
}

fn indented_code(
    value: &str,
    container_columns: usize,
    quote_depth: usize,
    list_depth: usize,
) -> String {
    value
        .lines()
        .map(|line| {
            let mut line = line;
            for _ in 0..quote_depth {
                let trimmed = line.trim_start_matches(' ');
                if let Some(rest) = trimmed.strip_prefix('>') {
                    line = rest.strip_prefix(' ').unwrap_or(rest);
                }
            }
            for _ in 0..list_depth {
                let trimmed = line.trim_start_matches(' ');
                let marker_end = if trimmed.starts_with(['-', '+', '*']) {
                    1
                } else {
                    trimmed.bytes().take_while(u8::is_ascii_digit).count()
                        + usize::from(
                            trimmed
                                .as_bytes()
                                .get(trimmed.bytes().take_while(u8::is_ascii_digit).count())
                                .is_some_and(|byte| matches!(byte, b'.' | b')')),
                        )
                };
                if marker_end > 0 {
                    line = trimmed[marker_end..]
                        .strip_prefix(' ')
                        .unwrap_or(&trimmed[marker_end..]);
                }
            }
            let target = 4 + container_columns;
            let mut columns = 0;
            let mut bytes = 0;
            let mut remainder = 0;
            for character in line.chars() {
                match character {
                    ' ' if columns < target => {
                        columns += 1;
                        bytes += 1;
                    }
                    '\t' if columns < target => {
                        let next = columns + 4 - columns % 4;
                        if next > target {
                            remainder = next - target;
                        }
                        columns = next;
                        bytes += 1;
                    }
                    _ => break,
                }
                if columns >= target {
                    break;
                }
            }
            format!("{}{}", " ".repeat(remainder), &line[bytes..])
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn code_span(value: &str) -> String {
    let delimiter = value.bytes().take_while(|byte| *byte == b'`').count();
    let mut content =
        value[delimiter..value.len().saturating_sub(delimiter)].replace(['\r', '\n'], " ");
    if content.starts_with(' ')
        && content.ends_with(' ')
        && content.bytes().any(|byte| byte != b' ')
    {
        content.remove(0);
        content.pop();
    }
    content
}

fn plain_alt(value: &str) -> String {
    value.replace(['\n', '\r'], " ")
}

fn normalize_inline_text(value: &str, quote_depth: usize) -> String {
    let value = value.trim_end_matches(['\r', '\n']);
    let mut output = String::new();
    for (index, line) in value.split('\n').enumerate() {
        if index > 0 {
            output.push('\n');
        }
        let mut line = if index == 0 {
            line
        } else {
            line.trim_start_matches([' ', '\t'])
        };
        for _ in 0..quote_depth {
            if let Some(rest) = line.strip_prefix('>') {
                line = rest.strip_prefix(' ').unwrap_or(rest);
            }
        }
        output.push_str(line);
    }
    output
}

fn assert_conforms(markdown: &str, expected: &str, dialect: MarkdownDialect, label: &str) {
    let events = conformance_events(markdown, dialect);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, ConformanceEvent::Start { .. })),
        "{label}: production query API yielded no syntax roles"
    );
    assert_eq!(canonical_html(&events), expected, "{label}");
}

#[test]
fn commonmark_example_1() {
    let example = commonmark_examples()
        .into_iter()
        .find(|example| example.example == 1)
        .expect("CommonMark fixture must include example 1");
    assert_eq!(example.markdown, "\tfoo\tbaz\t\tbim\n");
    assert_eq!(example.section, "Tabs");
    assert_eq!(example.html, "<pre><code>foo\tbaz\t\tbim\n</code></pre>\n");
    assert_conforms(
        &example.markdown,
        &example.html,
        MarkdownDialect::COMMONMARK_0_31_2,
        "CommonMark example 1",
    );
}

#[test]
fn commonmark_conformance() {
    for example in commonmark_examples() {
        assert_conforms(
            &example.markdown,
            &example.html,
            MarkdownDialect::COMMONMARK_0_31_2,
            &format!(
                "CommonMark example {} ({})",
                example.example, example.section
            ),
        );
    }
}

#[test]
fn gfm_extension_conformance() {
    let examples = gfm_extension_examples();
    assert!(
        !examples.is_empty(),
        "GFM extension fixture loader found no examples"
    );
    let commonmark_by_source: HashMap<_, _> = commonmark_examples()
        .into_iter()
        .map(|example| (example.markdown, (example.example, example.html)))
        .collect();
    for example in &examples {
        let core = commonmark_by_source.get(&example.markdown);
        let expected = core.map_or(example.html.as_str(), |(_, html)| html.as_str());
        let precedence = core.map_or_else(String::new, |(number, _)| {
            format!("; CommonMark 0.31.2 example {number} takes precedence")
        });
        assert_conforms(
            &example.markdown,
            expected,
            MarkdownDialect::WAML_DEFAULT,
            &format!(
                "GFM 0.29 example {} ({}){}",
                example.example, example.section, precedence
            ),
        );
    }
}

/// The conformance suites are the oracle for every parser change. If a fixture
/// is trimmed or a loader silently stops matching, the suites keep passing
/// while covering less. Pin the counts so shrinkage is a test failure.
#[test]
fn conformance_corpus_is_complete() {
    assert_eq!(
        commonmark_examples().len(),
        652,
        "CommonMark 0.31.2 fixture must keep all 652 examples; a smaller corpus \
         means the conformance oracle shrank instead of the parser improving"
    );
    assert_eq!(
        gfm_extension_examples().len(),
        24,
        "GFM 0.29 loader must keep all 24 examples from its five extension \
         sections; a smaller corpus means the oracle shrank"
    );
}
