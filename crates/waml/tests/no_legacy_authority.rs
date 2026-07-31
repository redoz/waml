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

fn authority_violations(files: &[(PathBuf, String)]) -> Vec<String> {
    let mut violations = Vec::new();
    for (path, source) in files {
        let path = path.to_string_lossy().replace('\\', "/");
        let is_waml_authority_consumer = path.starts_with("crates/waml/src/")
            || path.starts_with("crates/waml-editor/src/")
            || path.starts_with("crates/waml-cli/src/lsp/");
        if is_waml_authority_consumer && source.contains("pulldown_cmark::Parser") {
            violations.push(format!("{path}: creates pulldown_cmark::Parser"));
        }
        if source.contains("MarkdownStructureMap {")
            && path != "crates/waml-syntax/src/markdown/projection.rs"
        {
            violations.push(format!(
                "{path}: constructs MarkdownStructureMap outside projection"
            ));
        }
        if path.starts_with("crates/waml-editor/src/")
            && source.contains(".as_markdown()")
            && source.contains(".set_text(cx,")
        {
            violations.push(format!(
                "{path}: feeds source through Makepad Markdown parsing"
            ));
        }
        if path.starts_with("crates/waml-syntax/src/markdown/")
            && source.contains("regex::Regex::new")
        {
            violations.push(format!("{path}: classifies Markdown with regex"));
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
fn authority_guard_rejects_an_in_memory_second_parser_seed() {
    let violations = authority_violations(&[(
        PathBuf::from("crates/waml/src/in_memory_seed.rs"),
        "// pulldown_cmark::Parser::new".into(),
    )]);

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("pulldown_cmark::Parser")),
        "the authority guard accepted an in-memory second parser seed: {violations:#?}"
    );
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
