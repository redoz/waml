use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn compile_external(case: &str, source: &str) -> std::process::Output {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "waml-authority-{case}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("src")).expect("create external fixture");

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .to_string_lossy()
        .replace('\\', "/");
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\n\
             name = \"authority-api-{case}\"\n\
             version = \"0.0.0\"\n\
             edition = \"2021\"\n\n\
             [dependencies]\n\
             waml = {{ path = \"{manifest_dir}\" }}\n"
        ),
    )
    .expect("write external fixture manifest");
    fs::write(root.join("src/main.rs"), source).expect("write external fixture source");

    let output = Command::new(env!("CARGO"))
        .args(["check", "--quiet", "--offline", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .expect("run external cargo check");
    fs::remove_dir_all(&root).expect("remove external fixture");
    output
}

fn assert_privacy_failure(case: &str, source: &str, expected_item: &str) {
    let output = compile_external(case, source);
    assert!(!output.status.success(), "{case} unexpectedly compiled");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error[E0603]"),
        "{case} was not an E0603 privacy failure:\n{stderr}"
    );
    assert!(
        stderr.contains("private"),
        "{case} did not report privacy:\n{stderr}"
    );
    assert!(
        stderr.contains(expected_item),
        "{case} did not name `{expected_item}`:\n{stderr}"
    );
    for unrelated in [
        "failed to get",
        "no matching package",
        "failed to parse manifest",
        "could not find `Cargo.toml`",
    ] {
        assert!(
            !stderr.contains(unrelated),
            "{case} failed for an unrelated reason:\n{stderr}"
        );
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("waml crate below workspace root")
        .to_owned()
}

fn rust_tokens(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        if bytes[cursor..].starts_with(b"//") {
            cursor += 2;
            while cursor < bytes.len() && bytes[cursor] != b'\n' {
                cursor += 1;
            }
            continue;
        }
        if bytes[cursor..].starts_with(b"/*") {
            cursor += 2;
            let mut depth = 1_u32;
            while cursor < bytes.len() && depth != 0 {
                if bytes[cursor..].starts_with(b"/*") {
                    depth += 1;
                    cursor += 2;
                } else if bytes[cursor..].starts_with(b"*/") {
                    depth -= 1;
                    cursor += 2;
                } else {
                    cursor += 1;
                }
            }
            continue;
        }
        if bytes[cursor] == b'r' {
            let mut marker = cursor + 1;
            while marker < bytes.len() && bytes[marker] == b'#' {
                marker += 1;
            }
            if marker < bytes.len() && bytes[marker] == b'"' {
                let hashes = marker - cursor - 1;
                let mut end = marker + 1;
                while end < bytes.len() {
                    if bytes[end] == b'"'
                        && end + hashes < bytes.len()
                        && bytes[end + 1..=end + hashes]
                            .iter()
                            .all(|byte| *byte == b'#')
                    {
                        end += hashes + 1;
                        break;
                    }
                    end += 1;
                }
                tokens.push(source[cursor..end].to_owned());
                cursor = end;
                continue;
            }
        }
        if bytes[cursor] == b'"' || bytes[cursor] == b'\'' {
            let quote = bytes[cursor];
            let start = cursor;
            cursor += 1;
            while cursor < bytes.len() {
                if bytes[cursor] == b'\\' {
                    cursor = (cursor + 2).min(bytes.len());
                } else {
                    let end = bytes[cursor] == quote;
                    cursor += 1;
                    if end {
                        break;
                    }
                }
            }
            tokens.push(source[start..cursor].to_owned());
            continue;
        }
        if bytes[cursor].is_ascii_alphabetic() || bytes[cursor] == b'_' {
            let start = cursor;
            cursor += 1;
            while cursor < bytes.len()
                && (bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'_')
            {
                cursor += 1;
            }
            tokens.push(source[start..cursor].to_owned());
            continue;
        }
        let width = source[cursor..]
            .chars()
            .next()
            .expect("cursor is in source")
            .len_utf8();
        tokens.push(source[cursor..cursor + width].to_owned());
        cursor += width;
    }
    tokens
}

fn contains_tokens(tokens: &[String], expected: &[&str]) -> bool {
    tokens.windows(expected.len()).any(|window| {
        window
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied())
    })
}

fn constructs_structure_map(tokens: &[String]) -> bool {
    tokens.iter().enumerate().any(|(index, token)| {
        token == "MarkdownStructureMap"
            && matches!(tokens.get(index + 1).map(String::as_str), Some("{"))
            || token == "MarkdownStructureMap"
                && tokens.get(index + 1).map(String::as_str) == Some(":")
                && tokens.get(index + 2).map(String::as_str) == Some(":")
                && matches!(
                    tokens.get(index + 3).map(String::as_str),
                    Some("new" | "default")
                )
    })
}

fn literal_classifies_markdown(literal: &str) -> bool {
    let compact = literal.replace(' ', "");
    [
        "^#{1,6}",
        "(?m)^#{1,6}",
        "^```",
        "(?m)^```",
        "^~~~",
        "(?m)^~~~",
        "^\\s{0,3}>",
        "(?m)^\\s{0,3}>",
        "^\\s{0,3}#{1,6}",
        "(?m)^\\s{0,3}#{1,6}",
    ]
    .iter()
    .any(|marker| compact.contains(marker))
}

fn uses_regex_markdown_classifier(tokens: &[String]) -> bool {
    tokens.iter().enumerate().any(|(index, token)| {
        let regex_new = token == "Regex"
            && tokens.get(index + 1).map(String::as_str) == Some(":")
            && tokens.get(index + 2).map(String::as_str) == Some(":")
            && tokens.get(index + 3).map(String::as_str) == Some("new")
            || token == "regex"
                && tokens.get(index + 1).map(String::as_str) == Some(":")
                && tokens.get(index + 2).map(String::as_str) == Some(":")
                && tokens.get(index + 3).map(String::as_str) == Some("Regex")
                && tokens.get(index + 4).map(String::as_str) == Some(":")
                && tokens.get(index + 5).map(String::as_str) == Some(":")
                && tokens.get(index + 6).map(String::as_str) == Some("new");
        regex_new
            && tokens[index..]
                .iter()
                .take(12)
                .any(|candidate| literal_classifies_markdown(candidate))
    })
}

fn uses_okf_raw_markdown_classifier(tokens: &[String]) -> bool {
    contains_tokens(tokens, &["Regex", ":", ":", "new", "("])
        || contains_tokens(tokens, &["regex", ":", ":", "Regex"])
        || contains_tokens(tokens, &[".", "lines", "("])
        || contains_tokens(tokens, &[".", "captures", "("])
        || contains_tokens(tokens, &[".", "captures_iter", "("])
        || contains_tokens(tokens, &[".", "strip_prefix", "("])
}

fn makepad_markdown_widget_ids(files: &[(PathBuf, String)]) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for (path, source) in files {
        let path = path.to_string_lossy().replace('\\', "/");
        if !path.starts_with("crates/waml-editor/src/") {
            continue;
        }
        let tokens = rust_tokens(source);
        for window in tokens.windows(5) {
            if window[1..] == [":", "=", "Markdown", "{"] {
                ids.insert(window[0].clone());
            }
        }
    }
    ids
}

fn feeds_makepad_markdown(tokens: &[String], markdown_widget_ids: &BTreeSet<String>) -> bool {
    let mut aliases = BTreeSet::new();
    loop {
        let before = aliases.len();
        for statement in tokens.split(|token| token == ";") {
            let Some(let_index) = statement.iter().position(|token| token == "let") else {
                continue;
            };
            let Some(alias) = statement.get(let_index + 1) else {
                continue;
            };
            let Some(equal) = statement.iter().position(|token| token == "=") else {
                continue;
            };
            let expression = &statement[equal + 1..];
            let typed_markdown = contains_tokens(expression, &["as_markdown", "(", ")"])
                || contains_tokens(expression, &["Markdown", ":", ":"])
                || aliases.iter().any(|known| expression.contains(known));
            let selected_markdown_widget = expression.windows(5).any(|window| {
                window[0] == "ids"
                    && window[1] == "!"
                    && window[2] == "("
                    && window[4] == ")"
                    && markdown_widget_ids.contains(&window[3])
            });
            if typed_markdown || selected_markdown_widget {
                aliases.insert(alias.clone());
            }
        }
        if aliases.len() == before {
            break;
        }
    }

    contains_tokens(tokens, &["as_markdown", "(", ")", ".", "set_text", "("])
        || contains_tokens(tokens, &["Markdown", ":", ":", "set_text", "("])
        || aliases.iter().any(|alias| {
            tokens.windows(4).any(|window| {
                window[0] == *alias
                    && window[1] == "."
                    && window[2] == "set_text"
                    && window[3] == "("
            })
        })
}

fn manifest_dependency_violations(path: &str, source: &str) -> Vec<String> {
    const FORBIDDEN: &[&str] = &[
        "cmark",
        "comrak",
        "commonmark",
        "discount",
        "hoedown",
        "markdown",
        "markdown-it",
        "pulldown-cmark",
    ];

    fn dependency_name(line: &str) -> Option<&str> {
        let (key, value) = line.split_once('=')?;
        let key = key
            .trim()
            .split('.')
            .next()
            .unwrap_or_default()
            .trim_matches(['\'', '"']);
        if let Some(package) = value.split("package").nth(1) {
            let (_, package) = package.split_once('=')?;
            return package
                .trim_start()
                .strip_prefix(['\'', '"'])
                .and_then(|value| value.split(['\'', '"']).next());
        }
        Some(key)
    }

    let mut section = "";
    let mut violations = Vec::new();
    for raw_line in source.lines() {
        let line = raw_line.split('#').next().unwrap_or_default().trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line.trim_matches(['[', ']']).trim();
            continue;
        }
        let is_dependency_section = section == "dependencies" || section.ends_with(".dependencies");
        let is_dev = section == "dev-dependencies" || section.ends_with(".dev-dependencies");
        let is_workspace = section.starts_with("workspace.");
        if !is_dependency_section || is_dev || is_workspace || line.is_empty() {
            continue;
        }
        let Some(dependency) = dependency_name(line) else {
            continue;
        };
        let dependency = dependency.replace('_', "-");
        if FORBIDDEN.contains(&dependency.as_str())
            && !(path == "crates/waml-syntax/Cargo.toml" && dependency == "pulldown-cmark")
        {
            violations.push(format!(
                "{path}: production dependency `{dependency}` creates another Markdown authority"
            ));
        }
    }
    violations
}

fn authority_violations(files: &[(PathBuf, String)]) -> Vec<String> {
    let mut violations = Vec::new();
    let markdown_widget_ids = makepad_markdown_widget_ids(files);
    for (path, source) in files {
        let path = path.to_string_lossy().replace('\\', "/");
        if path.ends_with("Cargo.toml") {
            violations.extend(manifest_dependency_violations(&path, source));
            continue;
        }
        let tokens = rust_tokens(source);
        let is_waml_authority_consumer = path.starts_with("crates/waml/src/")
            || path.starts_with("crates/waml-editor/src/")
            || path.starts_with("crates/waml-cli/src/lsp/");
        if is_waml_authority_consumer
            && contains_tokens(&tokens, &["pulldown_cmark", ":", ":", "Parser"])
        {
            violations.push(format!("{path}: creates pulldown_cmark::Parser"));
        }
        if constructs_structure_map(&tokens)
            && path != "crates/waml-syntax/src/markdown/projection.rs"
        {
            violations.push(format!(
                "{path}: constructs MarkdownStructureMap outside projection"
            ));
        }
        if path.starts_with("crates/waml-editor/src/")
            && feeds_makepad_markdown(&tokens, &markdown_widget_ids)
        {
            violations.push(format!(
                "{path}: feeds source through Makepad Markdown parsing"
            ));
        }
        if uses_regex_markdown_classifier(&tokens) {
            violations.push(format!("{path}: classifies Markdown with regex"));
        }
        if path == "crates/waml/src/okf/shell.rs" && uses_okf_raw_markdown_classifier(&tokens) {
            violations.push(format!(
                "{path}: classifies raw Markdown instead of using snapshot queries"
            ));
        }
        if tokens.iter().any(|token| token == "CommonMarkCurrent") {
            violations.push(format!(
                "{path}: uses the vague CommonMarkCurrent dialect alias"
            ));
        }
    }
    violations
}

fn authority_sources(root: &Path) -> Vec<(PathBuf, String)> {
    fn collect(dir: &Path, root: &Path, files: &mut Vec<(PathBuf, String)>) {
        for entry in fs::read_dir(dir).expect("read authority guard directory") {
            let path = entry.expect("read authority guard entry").path();
            if path.is_dir() {
                collect(&path, root, files);
                continue;
            }
            let is_rust_source = path.extension().and_then(|value| value.to_str()) == Some("rs")
                && path
                    .components()
                    .any(|component| component.as_os_str() == "src");
            let is_manifest =
                path.file_name().and_then(|value| value.to_str()) == Some("Cargo.toml");
            if is_rust_source || is_manifest {
                files.push((
                    path.strip_prefix(root)
                        .expect("authority source below workspace root")
                        .to_owned(),
                    fs::read_to_string(&path).expect("read authority guard source"),
                ));
            }
        }
    }

    let mut files = Vec::new();
    collect(&root.join("crates"), root, &mut files);
    files
}

#[test]
fn authority_guard_rejects_every_in_memory_forbidden_seed() {
    let cases = [
        (
            "waml pulldown parser",
            "crates/waml/src/seed.rs",
            "let _ = pulldown_cmark :: Parser :: new(source);",
        ),
        (
            "editor pulldown parser",
            "crates/waml-editor/src/seed.rs",
            "let _ = pulldown_cmark :: Parser :: new(source);",
        ),
        (
            "LSP pulldown parser",
            "crates/waml-cli/src/lsp/seed.rs",
            "let _ = pulldown_cmark :: Parser :: new(source);",
        ),
        (
            "Makepad Markdown source parser",
            "crates/waml-editor/src/seed.rs",
            "widget . as_markdown ( ) . set_text (cx, source);",
        ),
        (
            "aliased Makepad Markdown source parser",
            "crates/waml-editor/src/seed.rs",
            "md := Markdown {}; let compatibility = surface.widget(cx, ids!(md)); compatibility.set_text(cx, markdown);",
        ),
        (
            "regex Markdown classifier",
            "crates/waml-cli/src/seed.rs",
            r##"let _ = regex :: Regex :: new(r"(?m)^#{1,6}\\s+");"##,
        ),
        (
            "OKF regex link scanner",
            "crates/waml/src/okf/shell.rs",
            r##"let links = Regex::new(r"\[([^\]]*)\]\(([^)]+)\)").unwrap();"##,
        ),
        (
            "OKF raw line classifier",
            "crates/waml/src/okf/shell.rs",
            "for line in shell.body.lines() { let heading = line.trim().strip_prefix(\"# \"); }",
        ),
        (
            "off-projection structure map",
            "crates/waml-syntax/src/markdown/seed.rs",
            "let _ = MarkdownStructureMap :: new(Default::default());",
        ),
        (
            "vague Markdown dialect alias",
            "crates/waml/src/seed.rs",
            "let dialect = MarkdownDialect::CommonMarkCurrent;",
        ),
        (
            "forbidden production manifest dependency",
            "crates/waml/Cargo.toml",
            "[dependencies]\npulldown-cmark.workspace = true\n",
        ),
    ];

    for (case, path, source) in cases {
        let violations = authority_violations(&[(PathBuf::from(path), source.into())]);
        assert!(
            !violations.is_empty(),
            "the authority guard accepted the in-memory {case} seed"
        );
    }
}

#[test]
fn production_sources_have_one_markdown_authority() {
    let violations = authority_violations(&authority_sources(&workspace_root()));
    assert!(
        violations.is_empty(),
        "Markdown authority guard found forbidden production authority:\n{}",
        violations.join("\n")
    );
}

#[test]
fn retired_legacy_files_and_public_surface_are_absent() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for retired in ["grammar.rs", "parse.rs", "syntax.rs", "serialize.rs"] {
        assert!(
            !manifest.join("src").join(retired).exists(),
            "retired legacy authority file still exists: {retired}"
        );
    }

    let lib = fs::read_to_string(manifest.join("src/lib.rs")).expect("read waml lib.rs");
    for export in [
        "pub mod grammar;",
        "pub mod parse;",
        "pub mod syntax;",
        "pub mod serialize;",
    ] {
        assert!(
            !lib.contains(export),
            "retired root export remains: {export}"
        );
    }

    let public_surface = [
        ("pub struct ", "Document"),
        ("pub struct ", "Section"),
        ("pub struct ", "Line"),
        ("pub struct ", "ErrorNode"),
        ("pub fn ", "parse_document"),
        ("pub fn ", "build_model"),
        ("pub fn ", "build_model_from_source"),
        ("pub fn ", "project_okf"),
        ("pub fn ", "serialize_document"),
    ];
    for entry in fs::read_dir(manifest.join("src")).expect("read waml src") {
        let path = entry.expect("read src entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read root Rust module");
        for (prefix, retired) in public_surface {
            let needle = format!("{prefix}{retired}");
            let remains = source.match_indices(&needle).any(|(start, _)| {
                match source[start + needle.len()..].chars().next() {
                    None => true,
                    Some(next) => !(next.is_ascii_alphanumeric() || next == '_'),
                }
            });
            assert!(
                !remains,
                "retired public surface `{retired}` remains in {}",
                path.display()
            );
        }
    }
}

#[test]
fn only_waml_non_dev_depends_on_waml_syntax() {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(workspace_root())
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse cargo metadata JSON");

    let workspace_members = metadata["workspace_members"]
        .as_array()
        .expect("workspace_members array")
        .iter()
        .map(|id| id.as_str().expect("workspace member package id"))
        .collect::<BTreeSet<_>>();
    let direct_users = metadata["packages"]
        .as_array()
        .expect("packages array")
        .iter()
        .filter(|package| {
            workspace_members.contains(package["id"].as_str().expect("workspace package id"))
        })
        .filter(|package| {
            package["dependencies"]
                .as_array()
                .expect("package dependencies array")
                .iter()
                .any(|dependency| {
                    dependency["name"].as_str() == Some("waml-syntax")
                        && matches!(dependency["kind"].as_str(), None | Some("build"))
                })
        })
        .map(|package| {
            package["name"]
                .as_str()
                .expect("workspace package name")
                .to_owned()
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(
        direct_users,
        BTreeSet::from(["waml".to_owned()]),
        "workspace packages with a direct non-dev waml-syntax dependency changed"
    );
}

#[test]
fn raw_parser_module_is_private_to_external_crates() {
    assert_privacy_failure(
        "raw-parser",
        "fn main() { let _ = waml::uml::syntax::parser::parse; }",
        "parser",
    );
}

#[test]
fn full_parse_facade_is_private_to_external_crates() {
    assert_privacy_failure(
        "full-parse-facade",
        "fn main() { let _ = waml::uml::syntax::parse_full; }",
        "parse_full",
    );
}
