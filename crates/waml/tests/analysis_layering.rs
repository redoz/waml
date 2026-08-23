//! The analysis pipeline's layering rule, enforced instead of asserted.
//!
//! Audit A12 called `analysis.rs` "a hub, not a layer", and prescribed lifting
//! the pipeline vocabulary into a crate below both `analysis` and `uml`. The
//! measurement in `docs/design/analysis-uml-layering.md` refused that split:
//! `analysis` and `uml` sit in an eight-module cycle, and deleting *both*
//! directions of the `analysis`/`uml` edge leaves that cycle at eight members,
//! so the split buys no acyclicity at all.
//!
//! What the split *would* have bought — and what these tests buy instead — is
//! the guarantee the audit actually cared about: the pipeline stays
//! specialization-agnostic, and a specialization never re-enters it. Both
//! properties hold today. Neither is visible to a reader of `analysis.rs`, and
//! the previous layering violation this repository shipped (`okf::Index`
//! embedding a view type) was invisible for exactly that reason, which is why
//! `tests/okf_layering.rs` exists and why this file follows it.

use std::fs;
use std::path::{Path, PathBuf};

/// Tiers `analysis.rs` may name freely: its own substrate and below.
const ALLOWED_ROOTS: [&str; 3] = ["diagnostic", "okf", "source"];

/// The only two names by which the composition root mounts a specialization.
///
/// Both are pure naming — a field type and one call. Nothing else about UML may
/// reach the pipeline: the moment `analysis.rs` needs a *third* UML item it has
/// stopped composing a specialization and started knowing about one, which is
/// the hub the audit named.
const SPECIALIZATION_MOUNT: [&str; 2] = ["uml::Analysis", "uml::analyze"];

/// Pipeline entry points. A specialization runs *inside* one of these; calling
/// one from `uml/` would mean the specialization re-entering the pipeline that
/// invoked it.
const PIPELINE_ENTRY_POINTS: [&str; 4] = [
    "prepare_candidate",
    "prepare_candidate_with_markdown_updates",
    "analyze_okf",
    "PreviousAnalyses",
];

/// Shipped code only.
///
/// Tests may reach up into the tiers above: they exercise a layer *through*
/// them. Same rule, and same truncation, as `tests/okf_layering.rs`.
fn production_source(path: &Path) -> String {
    let text = fs::read_to_string(path).expect("source is readable");
    let text = match text.find("\n#[cfg(test)]\n") {
        Some(at) => text[..at].to_string(),
        None => text,
    };
    text.lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every path written after a `crate::`, including the members of a
/// `crate::{..}` group, as they appear in the source.
fn crate_paths(source: &str) -> Vec<(usize, String)> {
    let bytes = source.as_bytes();
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(hit) = source[at..].find("crate::") {
        let start = at + hit;
        let mut cursor = start + "crate::".len();
        while bytes.get(cursor).is_some_and(|b| b.is_ascii_whitespace()) {
            cursor += 1;
        }
        let line = source[..start].lines().count();
        if bytes.get(cursor) == Some(&b'{') {
            let mut depth = 0usize;
            let mut member = String::new();
            while cursor < bytes.len() {
                let byte = bytes[cursor];
                match byte {
                    b'{' => {
                        depth += 1;
                        if depth > 1 {
                            member.push('{');
                        }
                    }
                    b'}' => {
                        if depth == 1 {
                            push_member(&mut found, line, &member);
                            cursor += 1;
                            break;
                        }
                        depth -= 1;
                        member.push('}');
                    }
                    b',' if depth == 1 => {
                        push_member(&mut found, line, &member);
                        member.clear();
                    }
                    _ if depth >= 1 => member.push(byte as char),
                    _ => {}
                }
                cursor += 1;
            }
        } else {
            let end = source[cursor..]
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == ':'))
                .map_or(source.len(), |offset| cursor + offset);
            push_member(&mut found, line, &source[cursor..end]);
            cursor = end;
        }
        at = cursor.max(start + 1);
    }
    found
}

fn push_member(into: &mut Vec<(usize, String)>, line: usize, member: &str) {
    let member = member
        .split('{')
        .next()
        .unwrap_or("")
        .trim()
        .trim_end_matches(':')
        .to_string();
    if !member.is_empty() {
        into.push((line, member));
    }
}

#[test]
fn the_pipeline_names_its_specialization_and_nothing_else_above_it() {
    let path = Path::new("src/analysis.rs");
    let source = production_source(path);
    let paths = crate_paths(&source);
    assert!(
        paths.len() >= 5,
        "expected to find the crate:: references in analysis.rs, found {}",
        paths.len()
    );

    let mut violations = Vec::new();
    for (line, item) in paths {
        let root = item.split("::").next().unwrap_or("");
        if ALLOWED_ROOTS.contains(&root) {
            continue;
        }
        let two = item.split("::").take(2).collect::<Vec<_>>().join("::");
        if SPECIALIZATION_MOUNT.contains(&two.as_str()) {
            continue;
        }
        violations.push(format!("src/analysis.rs:{line}: crate::{item}"));
    }

    assert!(
        violations.is_empty(),
        "the analysis pipeline may name only its substrate ({}) and the two \
         names that mount a specialization ({}). Anything else makes it a hub \
         that knows about the tier above it -- see \
         docs/design/analysis-uml-layering.md:\n{}",
        ALLOWED_ROOTS.join(", "),
        SPECIALIZATION_MOUNT.join(", "),
        violations.join("\n")
    );
}

#[test]
fn no_specialization_re_enters_the_pipeline() {
    let mut sources = vec![PathBuf::from("src/uml.rs")];
    collect_rust_files(Path::new("src/uml"), &mut sources);
    assert!(
        sources.len() > 10,
        "expected the uml specialization's sources to be found, got {}",
        sources.len()
    );

    let mut violations = Vec::new();
    for path in sources {
        let source = production_source(&path);
        for (number, line) in source.lines().enumerate() {
            for entry in PIPELINE_ENTRY_POINTS {
                if line.contains(entry) {
                    violations.push(format!(
                        "{}:{}: {}",
                        path.display().to_string().replace('\\', "/"),
                        number + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "a specialization runs inside the pipeline; it must not call back into \
         it. Shipped uml code may name the pipeline *vocabulary* \
         (DomainAnalysisContext, AnalysisError, DocumentVersion, ...) but not \
         its entry points -- see docs/design/analysis-uml-layering.md:\n{}",
        violations.join("\n")
    );
}

fn collect_rust_files(dir: &Path, into: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("directory exists") {
        let path = entry.expect("readable entry").path();
        if path.is_dir() {
            collect_rust_files(&path, into);
        } else if path.extension().is_some_and(|e| e == "rs") {
            into.push(path);
        }
    }
}
