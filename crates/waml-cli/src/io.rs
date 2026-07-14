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
        if parts.len() > 1 || parts.first().map(|(p, _)| p != "pasted/doc.md").unwrap_or(false) {
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

/// `check`: expand dirs to `*.md`, split blob files, read stdin as one blob.
pub fn read_bundle(paths: &[PathBuf], stdin: bool) -> std::io::Result<Vec<(String, String)>> {
    if stdin {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        return Ok(expand_text("stdin", &buf));
    }
    let mut out = Vec::new();
    for file in collect_md(paths)? {
        let text = fs::read_to_string(&file)?;
        out.extend(expand_text(&path_key(&file), &text));
    }
    Ok(out)
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
pub fn write_back(old: &[(String, String)], new: &[(String, String)]) -> std::io::Result<Vec<String>> {
    let om: BTreeMap<&str, &str> = old.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    let nm: BTreeMap<&str, &str> = new.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    let mut touched = Vec::new();
    for (p, c) in &nm {
        if om.get(p) != Some(c) {
            fs::write(p, c)?;
            touched.push(format!("wrote {p}"));
        }
    }
    for p in om.keys() {
        if !nm.contains_key(p) {
            fs::remove_file(p)?;
            touched.push(format!("deleted {p}"));
        }
    }
    Ok(touched)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(docs.len(), 1, "a stray non-.md comment must not split the document");
        assert_eq!(docs[0].0, "shop/order.md");
        assert_eq!(docs[0].1, text, "content must be kept intact, nothing discarded");
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
