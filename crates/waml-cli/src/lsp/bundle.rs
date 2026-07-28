//! In-memory workspace bundle overlay + recompute.
//!
//! A `Workspace` is a map from path to live text, seeded from disk and
//! overlaid with open-buffer edits. Diagnostics are recomputed over the whole
//! bundle on each query, so cross-document checks (unresolved targets,
//! duplicate slugs) stay correct as buffers change.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types as lsp;

use crate::lsp::map::{is_waml, to_lsp_diagnostic};

#[derive(Default)]
pub struct Workspace {
    docs: HashMap<String, String>,
    root: Option<PathBuf>,
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
    /// Files are keyed by ABSOLUTE path — the same key `did_open`/`did_change`
    /// derive from a document URI — so an open buffer overlays its disk copy
    /// under one key (no phantom duplicate-slug, edits reach cross-file checks).
    pub fn seed_from_glob(&mut self, root: &Path) {
        self.root = Some(root.to_path_buf());
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
                let key = f.to_string_lossy().replace('\\', "/");
                self.docs.entry(key).or_insert(text);
            }
        }
    }

    /// Per-file LSP diagnostics for the whole bundle. Non-WAML files get an
    /// empty vec (so the client clears any stale squiggles).
    pub fn diagnostics(&self) -> Vec<(String, Vec<lsp::Diagnostic>)> {
        let entries: Vec<(String, String, String)> = self
            .docs
            .iter()
            .map(|(physical, text)| {
                let logical = self
                    .root
                    .as_ref()
                    .and_then(|root| Path::new(physical).strip_prefix(root).ok())
                    .map(|path| path.to_string_lossy().replace('\\', "/"))
                    .or_else(|| {
                        waml::source::BundlePath::parse(physical.clone())
                            .ok()
                            .map(|path| path.to_string())
                    })
                    .or_else(|| {
                        Path::new(physical)
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                    })
                    .unwrap_or_else(|| physical.clone());
                (physical.clone(), logical, text.clone())
            })
            .collect();
        let source = match waml::source::SourceBundle::try_from_pairs(
            entries
                .iter()
                .map(|(_, logical, text)| (logical.clone(), text.clone())),
        ) {
            Ok(source) => source,
            Err(_) => {
                return entries
                    .into_iter()
                    .map(|(physical, _, _)| (physical, Vec::new()))
                    .collect();
            }
        };
        let all = waml::validate::validate_from_source(&source);
        let mut out: Vec<(String, Vec<lsp::Diagnostic>)> = Vec::new();
        for (physical, logical, text) in &entries {
            let mut ds = Vec::new();
            if is_waml(text) {
                let lines: Vec<&str> = text.lines().collect();
                for d in all.iter().filter(|d| d.file == *logical) {
                    let line_text = lines.get(d.line.saturating_sub(1)).copied().unwrap_or("");
                    ds.push(to_lsp_diagnostic(d, line_text));
                }
            }
            out.push((physical.clone(), ds));
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
        ws.overlay(
            "notes.md".into(),
            "# just notes\n\nnot waml at all\n".into(),
        );
        let diags = ws.diagnostics();
        assert!(diags
            .iter()
            .find(|(p, _)| p == "notes.md")
            .map(|(_, d)| d.is_empty())
            .unwrap_or(true));
    }

    #[test]
    fn arbitrary_and_missing_okf_types_receive_no_unknown_uml_diagnostic() {
        let mut ws = Workspace::new();
        ws.overlay(
            "playbook.md".into(),
            "---\ntype: Playbook\n---\n# Playbook\n".into(),
        );
        ws.overlay("notes.md".into(), "# Notes\n".into());
        for (_, diagnostics) in ws.diagnostics() {
            assert!(diagnostics.iter().all(|diagnostic| !matches!(
                &diagnostic.code,
                Some(lsp::NumberOrString::String(code)) if code == "unknown-type"
            )));
        }
    }

    #[test]
    fn seeded_disk_file_and_open_buffer_share_one_key() {
        // Reproduces the overlay-key mismatch: `seed_from_glob` keyed disk files
        // by workspace-relative path while `did_open`/`did_change` key by absolute
        // path. In a real session every opened on-disk file then existed under two
        // keys, yielding a spurious `duplicate-slug` and stale cross-file content.
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("waml_lsp_seed_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("order.md");
        let mut f = std::fs::File::create(&file).unwrap();
        write!(
            f,
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n"
        )
        .unwrap();
        drop(f);

        let mut ws = Workspace::new();
        ws.seed_from_glob(&dir);
        // The editor opens the same file: overlay by ABSOLUTE path, exactly as
        // `did_open` normalizes a `file://` URI.
        let abs = file.to_string_lossy().replace('\\', "/");
        ws.overlay(
            abs.clone(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- name: [String](./string.md)\n".into(),
        );

        let diags = ws.diagnostics();
        assert!(
            diags.iter().all(|(_, ds)| ds.iter().all(|d| !matches!(
                &d.code,
                Some(lsp::NumberOrString::String(s)) if s == "duplicate-slug"
            ))),
            "open buffer must overlay its seeded disk copy under ONE key (no duplicate-slug), got: {diags:?}"
        );
        let entries = diags
            .iter()
            .filter(|(p, _)| p.ends_with("order.md"))
            .count();
        assert_eq!(entries, 1, "exactly one bundle entry for the opened file");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cross_document_unresolved_target_is_reported() {
        let mut ws = Workspace::new();
        ws.overlay(
            "order.md".into(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Ghost](./ghost.md)\n".into(),
        );
        let (_, ds) = ws
            .diagnostics()
            .into_iter()
            .find(|(p, _)| p == "order.md")
            .unwrap();
        assert!(ds.iter().any(
            |d| matches!(&d.code, Some(lsp::NumberOrString::String(s)) if s == "unresolved-target")
        ));
    }
}
