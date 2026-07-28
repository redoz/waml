use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use waml::parse::split_bundle;

/// Turn one file's text into `(path, content)` docs: split on `<!-- path -->`
/// markers if present, otherwise a single doc keyed by `display_path`.
pub fn expand_text(display_path: &str, text: &str) -> Vec<(String, String)> {
    if text.contains("<!--") {
        let parts = split_bundle(text);
        // split_bundle returns "pasted/doc.md" for unmarked text; only trust it if markers existed.
        if parts.len() > 1
            || parts
                .first()
                .map(|(p, _)| p != "pasted/doc.md")
                .unwrap_or(false)
        {
            return parts;
        }
    }
    vec![(display_path.to_string(), text.to_string())]
}

/// Recursively collect `.md` files under the given files/directories.
pub fn collect_md(paths: &[PathBuf]) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            for entry in fs::read_dir(p)? {
                let path = entry?.path();
                out.extend(collect_md(&[path])?);
            }
        } else if p.extension().and_then(|e| e.to_str()) == Some("md") {
            out.push(p.clone());
        }
    }
    out.sort();
    Ok(out)
}

fn path_key(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Read a bundle with every document keyed by its path *relative to the bundle
/// root* rather than as typed on the command line.
///
/// The distinction matters because a bundle's paths are its OKF ids
/// ([`waml::okf::id_of`]). Diagnostics-facing commands must echo the path the
/// user typed, but a shared bundle is a model: keying it off the invocation
/// path would make the same directory encode to a different model depending on
/// where it was shared from, would disagree with the editor (which reads paths
/// relative to the model directory), and would stamp the author's directory
/// layout into every link.
pub fn read_bundle_rooted(
    paths: &[PathBuf],
    stdin: bool,
) -> std::io::Result<Vec<(String, String)>> {
    if stdin {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        return Ok(expand_text("stdin.md", &buf));
    }
    let files = collect_md(paths)?;
    let root = bundle_root(paths, &files);
    let mut out = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file)?;
        let rel = root
            .as_ref()
            .and_then(|r| file.strip_prefix(r).ok())
            .unwrap_or(file);
        out.extend(expand_text(&path_key(rel), &text));
    }
    // Match the editor's `read_bundle`: sorted keys make the encoding
    // deterministic, so the same model always yields the same link.
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// The directory paths should be made relative to: the directory itself when a
/// single one was given (the common case, and what the editor does), otherwise
/// the deepest directory containing every collected file. `None` when there is
/// no shared ancestor, in which case paths are left as-is.
fn bundle_root(paths: &[PathBuf], files: &[PathBuf]) -> Option<PathBuf> {
    if let [only] = paths {
        if only.is_dir() {
            return Some(only.clone());
        }
    }
    let mut root = files.first()?.parent()?.to_path_buf();
    for file in files.iter().skip(1) {
        let parent = file.parent()?;
        while !parent.starts_with(&root) {
            if !root.pop() {
                return None;
            }
        }
    }
    Some(root)
}

/// `fmt`: each physical `.md` file is a single document (no blob splitting).
pub fn read_files(paths: &[PathBuf]) -> std::io::Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for file in collect_md(paths)? {
        let text = fs::read_to_string(&file)?;
        out.push((path_key(&file), text));
    }
    Ok(out)
}

pub struct PhysicalBundle {
    pub root: PathBuf,
    pub files: Vec<(String, String)>,
}

/// Read physical Markdown files as one invocation-local bundle while retaining
/// the root needed to map validated bundle paths back to filesystem paths.
pub fn read_physical_bundle(paths: &[PathBuf]) -> std::io::Result<PhysicalBundle> {
    let physical = collect_md(paths)?;
    let root = bundle_root(paths, &physical)
        .or_else(|| {
            physical
                .first()
                .and_then(|path| path.parent())
                .map(Path::to_path_buf)
        })
        .unwrap_or_default();
    let mut files = Vec::with_capacity(physical.len());
    for path in physical {
        let relative = path.strip_prefix(&root).unwrap_or(&path);
        files.push((path_key(relative), fs::read_to_string(path)?));
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(PhysicalBundle { root, files })
}

/// Read an NDJSON op-log: `(line_number, trimmed_line)` per non-blank line.
/// `src == "-"` reads stdin.
pub fn read_ndjson(src: &str) -> std::io::Result<Vec<(usize, String)>> {
    let text = if src == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        fs::read_to_string(src)?
    };
    Ok(text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim().to_string()))
        .filter(|(_, l)| !l.is_empty())
        .collect())
}

/// Write only changed/added entries; delete entries dropped from the bundle.
/// Returns a human list of what happened.
pub fn write_back(
    root: &Path,
    old: &[(String, String)],
    new: &[(String, String)],
) -> std::io::Result<Vec<String>> {
    let om: BTreeMap<&str, &str> = old.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    let nm: BTreeMap<&str, &str> = new.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    let mut touched = Vec::new();
    for (p, c) in &nm {
        if om.get(p) != Some(c) {
            let destination = root.join(p);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(destination, c)?;
            touched.push(format!("wrote {p}"));
        }
    }
    for p in om.keys() {
        if !nm.contains_key(p) {
            fs::remove_file(root.join(p))?;
            touched.push(format!("deleted {p}"));
        }
    }
    Ok(touched)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../waml-editor/tests/fixtures")
            .join(name)
    }

    /// A bundle's paths are its OKF ids, so a shared model must key off the
    /// bundle root -- exactly like the editor's `load::read_bundle` -- and not
    /// off whatever prefix happened to be typed at the shell.
    #[test]
    fn rooted_read_keys_relative_to_the_bundle_dir() {
        let bundle = read_bundle_rooted(&[fixture("mini")], false).unwrap();
        let keys: Vec<&str> = bundle.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            keys,
            [
                "customer.md",
                "index.md",
                "order.md",
                "orders-diagram.md",
                "payment-gateway.md"
            ]
        );
    }

    /// The same model shared from a different working directory must encode
    /// identically; otherwise one model has as many identities as it has
    /// callers, and links leak the author's directory layout.
    #[test]
    fn rooted_read_is_independent_of_the_path_typed() {
        let absolute = read_bundle_rooted(&[fixture("mini")], false).unwrap();
        let indirect =
            read_bundle_rooted(&[fixture("groups").parent().unwrap().join("mini")], false).unwrap();
        assert_eq!(absolute, indirect);
    }

    /// Given loose files rather than a directory, the root is the deepest
    /// directory containing all of them.
    #[test]
    fn rooted_read_uses_the_common_ancestor_of_loose_files() {
        let dir = fixture("mini");
        let bundle =
            read_bundle_rooted(&[dir.join("order.md"), dir.join("customer.md")], false).unwrap();
        let keys: Vec<&str> = bundle.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(keys, ["customer.md", "order.md"]);
    }

    /// One-shot analysis requires validated bundle-relative paths even when a
    /// single physical file was typed.
    #[test]
    fn rooted_read_of_one_file_uses_its_basename() {
        let dir = fixture("mini");
        let bundle = read_bundle_rooted(&[dir.join("order.md")], false).unwrap();
        assert_eq!(bundle[0].0, "order.md");
    }

    #[test]
    fn expands_blob_text_into_docs() {
        let blob = "<!-- a/one.md -->\n# One\n\n<!-- a/two.md -->\n# Two\n";
        let docs = expand_text("stdin", blob);
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].0, "a/one.md");
    }

    #[test]
    fn plain_text_uses_its_own_path() {
        let docs = expand_text("shop/order.md", "# Order\n");
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].0, "shop/order.md");
    }

    #[test]
    fn stray_comment_doc_is_one_doc() {
        // A single .md doc that happens to contain a lone, non-marker HTML
        // comment (e.g. a review note) must not be split apart: it must
        // come back as exactly one doc, keyed by its real display path,
        // with the full content — including the unresolved relationship
        // section that follows the stray comment — intact.
        let text = "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n<!-- reviewed: needs follow-up -->\n\n## Relationships\n- depends [Ghost](./ghost.md)\n";
        let docs = expand_text("shop/order.md", text);
        assert_eq!(
            docs.len(),
            1,
            "a stray non-.md comment must not split the document"
        );
        assert_eq!(docs[0].0, "shop/order.md");
        assert_eq!(
            docs[0].1, text,
            "content must be kept intact, nothing discarded"
        );
    }

    #[test]
    fn read_ndjson_skips_blanks_and_numbers_lines() {
        // write a temp file
        let dir = std::env::temp_dir().join(format!("waml_ndjson_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("ops.ndjson");
        std::fs::write(&f, "{\"op\":\"a\"}\n\n{\"op\":\"b\"}\n").unwrap();
        let lines = read_ndjson(f.to_str().unwrap()).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].0, 1);
        assert_eq!(lines[1].0, 3, "blank line 2 skipped, numbering preserved");
    }
}
