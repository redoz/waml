//! In-memory workspace bundle overlay + recompute.
//!
//! A `Workspace` is a map from path to live text, seeded from disk and
//! overlaid with open-buffer edits. Diagnostics are recomputed over the whole
//! bundle on each query, so cross-document checks (unresolved targets,
//! duplicate slugs) stay correct as buffers change.

use std::collections::HashMap;
use std::path::Path;

use tower_lsp::lsp_types as lsp;

use crate::lsp::map::{is_uaml, to_lsp_diagnostic};

#[derive(Default)]
pub struct Workspace {
    docs: HashMap<String, String>,
}

impl Workspace {
    pub fn new() -> Self {
        Workspace::default()
    }

    /// Insert or replace one file's live text.
    pub fn overlay(&mut self, path: String, text: String) {
        self.docs.insert(path, text);
    }

    /// Seed the bundle from every `*.md` under `root` (recursive `read_dir`,
    /// no extra crate). Existing entries (open buffers) are not overwritten.
    pub fn seed_from_glob(&mut self, root: &Path) {
        fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        walk(&p, out);
                    } else if p.extension().and_then(|x| x.to_str()) == Some("md") {
                        out.push(p);
                    }
                }
            }
        }
        let mut files = Vec::new();
        walk(root, &mut files);
        for f in files {
            if let Ok(text) = std::fs::read_to_string(&f) {
                let rel = f
                    .strip_prefix(root)
                    .unwrap_or(&f)
                    .to_string_lossy()
                    .replace('\\', "/");
                self.docs.entry(rel).or_insert(text);
            }
        }
    }

    /// Per-file LSP diagnostics for the whole bundle. Non-UAML files get an
    /// empty vec (so the client clears any stale squiggles).
    pub fn diagnostics(&self) -> Vec<(String, Vec<lsp::Diagnostic>)> {
        let bundle: Vec<(String, String)> =
            self.docs.iter().map(|(p, t)| (p.clone(), t.clone())).collect();
        let all = uaml::validate::validate(&bundle);
        let mut out: Vec<(String, Vec<lsp::Diagnostic>)> = Vec::new();
        for (path, text) in &bundle {
            let mut ds = Vec::new();
            if is_uaml(text) {
                let lines: Vec<&str> = text.lines().collect();
                for d in all.iter().filter(|d| d.file == *path) {
                    let line_text = lines.get(d.line.saturating_sub(1)).copied().unwrap_or("");
                    ds.push(to_lsp_diagnostic(d, line_text));
                }
            }
            out.push((path.clone(), ds));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_edit_updates_diagnostics() {
        let mut ws = Workspace::new();
        ws.overlay(
            "a.md".into(),
            "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Attributes\n- id: AId\n".into(),
        );
        let clean = ws.diagnostics();
        assert!(clean.iter().all(|(_, ds)| ds.is_empty()));

        ws.overlay(
            "a.md".into(),
            "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Attributes\n- broken line\n".into(),
        );
        let dirty = ws.diagnostics();
        let (_, ds) = dirty.iter().find(|(p, _)| p == "a.md").unwrap();
        assert!(ds.iter().any(|d| d.message.contains("attribute")));
    }

    #[test]
    fn plain_markdown_is_filtered_out() {
        let mut ws = Workspace::new();
        ws.overlay("notes.md".into(), "# just notes\n\nnot uaml at all\n".into());
        let diags = ws.diagnostics();
        assert!(diags
            .iter()
            .find(|(p, _)| p == "notes.md")
            .map(|(_, d)| d.is_empty())
            .unwrap_or(true));
    }

    #[test]
    fn cross_document_unresolved_target_is_reported() {
        let mut ws = Workspace::new();
        ws.overlay(
            "order.md".into(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Ghost](./ghost.md)\n".into(),
        );
        let (_, ds) = ws.diagnostics().into_iter().find(|(p, _)| p == "order.md").unwrap();
        assert!(ds
            .iter()
            .any(|d| matches!(&d.code, Some(lsp::NumberOrString::String(s)) if s == "unresolved-target")));
    }
}
