use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

use tower_lsp::lsp_types as lsp;
use waml::{
    analysis::{prepare_candidate, OkfAnalysis, PreviousAnalyses},
    host,
    source::{BundlePath, SourceBundle, SourceDocument},
    uml,
};

use crate::lsp::map::to_lsp_diagnostic;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone, Default)]
pub struct LspHostIndex {
    pub root: Option<PathBuf>,
    pub disk_by_physical: BTreeMap<PathBuf, SourceDocument>,
    pub open_by_physical: BTreeMap<PathBuf, BundlePath>,
}

pub struct LspAnalysisState {
    pub host: LspHostIndex,
    pub source: SourceBundle,
    pub okf: OkfAnalysis,
    pub uml: uml::Analysis,
    pub revision: u64,
}

impl LspAnalysisState {
    pub fn empty() -> Result<Self, BoxError> {
        Self::from_documents(None, std::iter::empty::<(PathBuf, String)>())
    }

    pub fn from_documents(
        root: Option<PathBuf>,
        documents: impl IntoIterator<Item = (PathBuf, String)>,
    ) -> Result<Self, BoxError> {
        let mut host_index = LspHostIndex {
            root,
            ..Default::default()
        };
        let mut source = SourceBundle::default();
        for (physical, text) in documents {
            let physical = normalize_physical(physical);
            let logical = logical_path(host_index.root.as_deref(), &physical)?;
            if source.document(&logical).is_some() {
                return Err(format!("logical path collision at {logical}").into());
            }
            let document = SourceDocument::new(logical, text);
            source = host::add_document(&source, document.clone())?;
            host_index.disk_by_physical.insert(physical, document);
        }
        let prepared = prepare_candidate(source, None, 0)?;
        let (source, okf, uml, revision) = prepared.into_parts();
        Ok(Self {
            host: host_index,
            source,
            okf,
            uml,
            revision,
        })
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn source(&self) -> &SourceBundle {
        &self.source
    }

    pub fn okf(&self) -> &OkfAnalysis {
        &self.okf
    }

    pub fn uml(&self) -> &uml::Analysis {
        &self.uml
    }

    pub fn open(&self, physical: PathBuf, text: String) -> Result<Self, BoxError> {
        let physical = normalize_physical(physical);
        if self.host.open_by_physical.contains_key(&physical) {
            return self.change(&physical, text);
        }
        let logical = logical_path(self.host.root.as_deref(), &physical)?;
        self.reject_collision(&physical, &logical)?;
        let document = SourceDocument::new(logical.clone(), text);
        let source = if self.source.document(&logical).is_some() {
            host::replace_document(&self.source, document)?
        } else {
            host::add_document(&self.source, document)?
        };
        let mut next_host = self.host.clone();
        next_host.open_by_physical.insert(physical, logical);
        self.prepare(next_host, source)
    }

    pub fn change(&self, physical: &Path, text: String) -> Result<Self, BoxError> {
        let physical = normalize_physical(physical.to_path_buf());
        let logical = self
            .host
            .open_by_physical
            .get(&physical)
            .cloned()
            .ok_or_else(|| format!("change for non-open document {}", physical.display()))?;
        let source = host::replace_document(&self.source, SourceDocument::new(logical, text))?;
        self.prepare(self.host.clone(), source)
    }

    pub fn close(&self, physical: &Path) -> Result<Option<Self>, BoxError> {
        let physical = normalize_physical(physical.to_path_buf());
        let Some(logical) = self.host.open_by_physical.get(&physical).cloned() else {
            return Ok(None);
        };
        let mut next_host = self.host.clone();
        next_host.open_by_physical.remove(&physical);
        let source = if let Some(disk) = self.host.disk_by_physical.get(&physical) {
            host::replace_document(&self.source, disk.clone())?
        } else {
            host::remove_document(&self.source, &logical)?
        };
        self.prepare(next_host, source).map(Some)
    }

    fn reject_collision(&self, physical: &Path, logical: &BundlePath) -> Result<(), BoxError> {
        let disk_collision = self
            .host
            .disk_by_physical
            .iter()
            .any(|(owner, document)| owner != physical && document.path() == logical);
        let open_collision = self
            .host
            .open_by_physical
            .iter()
            .any(|(owner, path)| owner != physical && path == logical);
        if disk_collision || open_collision {
            return Err(format!("logical path collision at {logical}").into());
        }
        Ok(())
    }

    fn prepare(&self, host: LspHostIndex, source: SourceBundle) -> Result<Self, BoxError> {
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("LSP revision overflow")?;
        let prepared = prepare_candidate(
            source,
            Some(PreviousAnalyses {
                okf: &self.okf,
                uml: &self.uml,
            }),
            revision,
        )?;
        let (source, okf, uml, revision) = prepared.into_parts();
        Ok(Self {
            host,
            source,
            okf,
            uml,
            revision,
        })
    }

    pub fn diagnostics(&self) -> Vec<(PathBuf, Vec<lsp::Diagnostic>)> {
        let mut output = Vec::new();
        for (physical, logical) in self.physical_documents() {
            let Some(_document) = self.source.document(&logical) else {
                continue;
            };
            let Some(version) = self
                .okf
                .catalog
                .id_for_path(&logical)
                .and_then(|id| self.okf.catalog.document(id))
            else {
                continue;
            };
            let diagnostics = self
                .uml
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.file == logical.as_str())
                .map(|diagnostic| to_lsp_diagnostic(diagnostic, version))
                .collect();
            output.push((physical, diagnostics));
        }
        output
    }

    fn physical_documents(&self) -> BTreeMap<PathBuf, BundlePath> {
        let mut documents = BTreeMap::new();
        for (physical, disk) in &self.host.disk_by_physical {
            documents.insert(physical.clone(), disk.path().clone());
        }
        for (physical, logical) in &self.host.open_by_physical {
            documents.insert(physical.clone(), logical.clone());
        }
        documents
    }
}

pub fn logical_path(root: Option<&Path>, physical: &Path) -> Result<BundlePath, BoxError> {
    if let Some(relative) = root.and_then(|root| physical.strip_prefix(root).ok()) {
        return BundlePath::parse(relative.to_string_lossy().replace('\\', "/"))
            .map_err(Into::into);
    }
    let suffix = physical
        .components()
        .filter_map(|component| match component {
            Component::Prefix(prefix) => Some(
                prefix
                    .as_os_str()
                    .to_string_lossy()
                    .replace(':', "_")
                    .replace('\\', ""),
            ),
            Component::Normal(segment) => Some(segment.to_string_lossy().replace(':', "_")),
            Component::RootDir | Component::CurDir | Component::ParentDir => None,
        })
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    BundlePath::parse(format!("__external__/{suffix}")).map_err(Into::into)
}

fn normalize_physical(path: PathBuf) -> PathBuf {
    PathBuf::from(path.to_string_lossy().replace('\\', "/"))
}

pub fn read_disk_documents(root: &Path) -> Vec<(PathBuf, String)> {
    fn walk(directory: &Path, output: &mut Vec<(PathBuf, String)>) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, output);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("md") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    output.push((normalize_physical(path), text));
                }
            }
        }
    }
    let mut output = Vec::new();
    walk(root, &mut output);
    output.sort_by(|left, right| left.0.cmp(&right.0));
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_snapshot_open_change_close_restores_disk_and_revision_alignment() {
        let physical = PathBuf::from("C:/workspace/order.md");
        let root = PathBuf::from("C:/workspace");
        let disk = "---\ntype: uml.Class\n---\n# Disk\n";
        let state = LspAnalysisState::from_documents(Some(root), [(physical.clone(), disk.into())])
            .unwrap();
        let open = state
            .open(
                physical.clone(),
                "---\ntype: uml.Class\n---\n# Open\n".into(),
            )
            .unwrap();
        let changed = open
            .change(&physical, "---\ntype: uml.Class\n---\n# Changed\n".into())
            .unwrap();
        let closed = changed.close(&physical).unwrap().unwrap();
        assert_eq!(closed.revision(), 3);
        assert_eq!(closed.source().documents()[0].text(), disk);
        assert_eq!(closed.okf().catalog.session_revision(), closed.revision());
        assert_eq!(closed.uml().session_revision(), closed.revision());
    }

    #[test]
    fn overlay_only_close_removes_document_and_missing_close_is_noop() {
        let physical = PathBuf::from("C:/outside/notes.md");
        let state = LspAnalysisState::empty().unwrap();
        let open = state
            .open(physical.clone(), "---\ntype: Notes\n---\n# Notes\n".into())
            .unwrap();
        assert_eq!(open.source().documents().len(), 1);
        let closed = open.close(&physical).unwrap().unwrap();
        assert!(closed.source().documents().is_empty());
        assert!(closed.close(&physical).unwrap().is_none());
        assert!(open
            .change(&PathBuf::from("C:/missing.md"), String::new())
            .is_err());
    }

    #[test]
    fn external_logical_paths_are_normalized_and_validated() {
        assert_eq!(
            logical_path(None, Path::new("C:/one/../two/order.md"))
                .unwrap()
                .as_str(),
            "__external__/C_/one/two/order.md"
        );
    }
}
