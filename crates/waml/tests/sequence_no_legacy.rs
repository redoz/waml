use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("waml crate below workspace root")
        .to_owned()
}

fn normalized_words(line: &str) -> Vec<String> {
    line.split(|character: char| !character.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|word| !word.is_empty())
        .collect()
}

fn legacy_sequence_line(path: &str, line: &str) -> Option<&'static str> {
    let trimmed = line
        .trim()
        .trim_start_matches("///")
        .trim_start_matches("//")
        .trim_start_matches('*')
        .trim();
    let words = normalized_words(trimmed);
    let has_removed_verb = words
        .iter()
        .any(|word| matches!(word.as_str(), "sends" | "replies"));
    let lower = trimmed.to_ascii_lowercase();

    if trimmed.starts_with("- ") && has_removed_verb && words.len() <= 8 {
        return Some("authored legacy message");
    }
    if lower.contains("msg-verb") && has_removed_verb {
        return Some("legacy grammar production");
    }
    if lower.contains("messageverb") && has_removed_verb {
        return Some("legacy message enum terminology");
    }
    if (lower.contains("stack") || lower.contains("lifo"))
        && lower.contains("call")
        && lower.contains("repl")
    {
        return Some("legacy stack-based reply solver description");
    }
    if line.trim_start().starts_with("//") && lower.contains("message") && has_removed_verb {
        return Some("legacy message comment");
    }
    if let Some((_, tail)) = lower.split_once(" calls ") {
        if tail
            .split_whitespace()
            .next()
            .is_some_and(|target| target.ends_with(':'))
        {
            return Some("colon-form call example");
        }
    }
    if path == "docs/uaml-spec.md" && has_removed_verb {
        return Some("active specification uses a removed message verb");
    }
    None
}

fn collect_files(root: &Path, path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_file() {
        files.push(path.strip_prefix(root).unwrap().to_owned());
        return;
    }
    for entry in fs::read_dir(path).expect("read sequence corpus directory") {
        let path = entry.expect("read sequence corpus entry").path();
        if path.is_dir()
            || matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("rs" | "md" | "txt")
            )
        {
            collect_files(root, &path, files);
        }
    }
}

fn active_sequence_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for relative in [
        "docs/uaml-spec.md",
        "docs/waml",
        "crates/waml/src",
        "crates/waml-editor/src",
        "crates/waml/tests/fixtures",
        "fuzz/seeds",
    ] {
        collect_files(root, &root.join(relative), &mut files);
    }
    files
}

#[test]
fn zero_legacy_guard_rejects_each_sequence_language_category() {
    for (category, path, source) in [
        ("authored", "fixture.md", "- a sends b `legacy`"),
        (
            "grammar",
            "docs/uaml-spec.md",
            "msg-verb ::= calls | replies",
        ),
        ("enum", "design.md", "MessageVerb (Calls, Sends, Replies)"),
        (
            "solver",
            "design.md",
            "pair calls with replies on a per-lifeline stack",
        ),
        ("colon call", "inspector.rs", "/// a calls b: start()"),
        (
            "comment",
            "canvas.rs",
            "// Open message head for sends/replies",
        ),
    ] {
        assert!(
            legacy_sequence_line(path, source).is_some(),
            "zero-legacy guard accepted the {category} seed"
        );
    }
}

#[test]
fn active_sequence_corpus_has_no_legacy_language() {
    let root = workspace_root();
    let mut violations = Vec::new();
    for relative in active_sequence_files(&root) {
        let source = fs::read_to_string(root.join(&relative)).expect("read sequence corpus file");
        let path = relative.to_string_lossy().replace('\\', "/");
        for (index, line) in source.lines().enumerate() {
            if let Some(category) = legacy_sequence_line(&path, line) {
                violations.push(format!("{path}:{}: {category}: {line}", index + 1));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "active sequence corpus contains legacy language:\n{}",
        violations.join("\n")
    );
}

#[test]
fn older_canvas_sequence_semantics_are_clearly_superseded() {
    let path = workspace_root()
        .join("docs/superpowers/specs/2026-07-30-native-behavior-canvases-design.md");
    let source = fs::read_to_string(path).expect("read native behavior canvas design");
    let header = source.lines().take(12).collect::<Vec<_>>().join("\n");
    assert!(
        header.to_ascii_lowercase().contains("superseded")
            && header.contains("2026-08-02-waml-sequence-language-completeness"),
        "the older canvas design must point readers to the current sequence semantics"
    );
}
