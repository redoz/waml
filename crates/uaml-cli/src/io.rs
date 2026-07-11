use std::fs;
use std::io::Read;
use std::path::PathBuf;

use uaml::parse::split_bundle;

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

fn path_key(p: &PathBuf) -> String {
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
}
