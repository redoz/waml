use std::{fs, path::Path};

use pulldown_cmark::{html, Options, Parser};
use waml_syntax::{
    parse_markdown, write_green_to, DocumentRevision, MarkdownDialect, MarkdownLinkKind,
    MarkdownSemanticRole, SourceText, TextRange,
};

#[derive(serde::Deserialize)]
struct CommonMarkExample {
    markdown: String,
    html: String,
    example: u32,
    section: String,
}

fn commonmark_examples() -> Vec<CommonMarkExample> {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/commonmark-0.31.2/spec.json");
    let source = fs::read_to_string(&fixture)
        .unwrap_or_else(|error| panic!("read CommonMark fixture {}: {error}", fixture.display()));
    serde_json::from_str(&source).expect("deserialize CommonMark fixture")
}

#[derive(Debug)]
struct GfmExample {
    section: String,
    markdown: String,
    html: String,
}

#[derive(Debug)]
enum ConformanceEvent {
    Source(String),
    Role(MarkdownSemanticRole),
    ExtendedAutolink { range: TextRange, destination: String },
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
    let source = fs::read_to_string(fixture("tests/fixtures/gfm-0.29/spec.txt"))
        .expect("read GFM fixture");
    let mut section = None;
    let mut examples = Vec::new();
    let mut lines = source.lines().peekable();
    while let Some(line) = lines.next() {
        if let Some(heading) = line.strip_prefix("## ") {
            section = SECTIONS.contains(&heading).then(|| heading.to_owned());
            continue;
        }
        if section.is_none() || !line.contains(" example") {
            continue;
        }
        let markdown = read_fixture_part(&mut lines);
        let html = read_until_fence(&mut lines);
        examples.push(GfmExample {
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
    assert_eq!(recovered, markdown, "production tree must recover fixture source");

    let root = snapshot.tree().root().range();
    let mut events = Vec::with_capacity(1 + snapshot.queries().spans(root).count());
    events.push(ConformanceEvent::Source(recovered));
    events.extend(
        snapshot
            .queries()
            .spans(root)
            .map(|span| ConformanceEvent::Role(span.semantic_role)),
    );
    events.extend(snapshot.queries().links().filter_map(|link| {
        (link.kind == MarkdownLinkKind::ExtendedAutolink).then(|| ConformanceEvent::ExtendedAutolink {
            range: link.source_range,
            destination: link.destination.to_string(),
        })
    }));
    events
}

fn canonical_html(events: &[ConformanceEvent], options: Options) -> String {
    let source = events
        .iter()
        .find_map(|event| match event {
            ConformanceEvent::Source(source) => Some(source.as_str()),
            ConformanceEvent::Role(_) | ConformanceEvent::ExtendedAutolink { .. } => None,
        })
        .expect("conformance events include recovered source");
    let mut output = String::new();
    let source = render_extended_autolinks(source, events);
    html::push_html(&mut output, Parser::new_ext(&source, options));
    normalize_html(&output)
}

fn render_extended_autolinks(source: &str, events: &[ConformanceEvent]) -> String {
    let mut links = events.iter().filter_map(|event| match event {
        ConformanceEvent::ExtendedAutolink { range, destination } => Some((*range, destination)),
        _ => None,
    }).collect::<Vec<_>>();
    links.sort_by_key(|(range, _)| range.start());
    let mut output = String::with_capacity(source.len());
    let mut at = 0;
    for (range, destination) in links {
        let start = range.start().to_usize();
        let end = range.end().to_usize();
        output.push_str(&source[at..start]);
        output.push_str("<a href=\"");
        output.push_str(&destination.replace('&', "&amp;"));
        output.push_str("\">");
        output.push_str(&source[start..end]);
        output.push_str("</a>");
        at = end;
    }
    output.push_str(&source[at..]);
    output
}

fn gfm_options() -> Options {
    let mut options = Options::all();
    options.insert(Options::ENABLE_GFM);
    options.remove(Options::ENABLE_SMART_PUNCTUATION);
    options
}

fn normalize_html(html: &str) -> String {
    let html = html
        .replace("&quot;", "\"")
        .replace("style=\"text-align: center\"", "align=\"center\"")
        .replace("style=\"text-align: left\"", "align=\"left\"")
        .replace("style=\"text-align: right\"", "align=\"right\"")
        .replace("<input disabled=\"\" type=\"checkbox\"/>\n", "<input data-task=\"unchecked\">")
        .replace("<input disabled=\"\" type=\"checkbox\" checked=\"\"/>\n", "<input data-task=\"checked\">")
        .replace("<input disabled=\"\" type=\"checkbox\"> ", "<input data-task=\"unchecked\">")
        .replace("<input checked=\"\" disabled=\"\" type=\"checkbox\"> ", "<input data-task=\"checked\">")
        .replace("<tbody></tbody>", "")
        .replace("<title>", "&lt;title>")
        .replace("<style>", "&lt;style>")
        .replace("<xmp>", "&lt;xmp>")
        .replace("<XMP>", "&lt;XMP>");
    let mut normalized = String::new();
    let mut at = 0;
    while at < html.len() {
        if html.as_bytes()[at] == b'>' {
            normalized.push('>');
            let mut next = at + 1;
            while next < html.len() && matches!(html.as_bytes()[next], b' ' | b'\n' | b'\r' | b'\t') {
                next += 1;
            }
            if next < html.len() && html.as_bytes()[next] == b'<' {
                at = next;
                continue;
            }
        } else {
            let character = html[at..]
                .chars()
                .next()
                .expect("valid UTF-8 string has a character at its byte offset");
            normalized.push(character);
            at += character.len_utf8();
            continue;
        }
        at += 1;
    }
    normalized.replace("<tbody></tbody>", "")
}

fn assert_conforms(markdown: &str, expected: &str, dialect: MarkdownDialect, options: Options, label: &str) {
    let events = conformance_events(markdown, dialect);
    assert!(
        events.iter().any(|event| matches!(event, ConformanceEvent::Role(_))),
        "{label}: production query API yielded no syntax roles"
    );
    assert_eq!(canonical_html(&events, options), normalize_html(expected), "{label}");
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
        Options::empty(),
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
            Options::empty(),
            &format!("CommonMark example {} ({})", example.example, example.section),
        );
    }
}

#[test]
fn gfm_extension_conformance() {
    let examples = gfm_extension_examples();
    assert!(!examples.is_empty(), "GFM extension fixture loader found no examples");
    for (index, example) in examples.iter().enumerate() {
        assert_conforms(
            &example.markdown,
            &example.html,
            MarkdownDialect::WAML_DEFAULT,
            gfm_options(),
            &format!(
                "GFM extension example {} ({}) — CommonMark 0.31.2 takes precedence for core cases",
                index + 1,
                example.section
            ),
        );
    }
}
