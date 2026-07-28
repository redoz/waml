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
    pub open_by_physical: BTreeMap<PathBuf, OpenDocument>,
    pub next_open_generation: u64,
}

#[derive(Clone)]
pub struct OpenDocument {
    pub logical: BundlePath,
    pub client_version: i32,
    pub generation: u64,
}

pub struct DiagnosticPublication {
    pub physical: PathBuf,
    pub diagnostics: Vec<lsp::Diagnostic>,
    pub client_version: Option<i32>,
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

    pub fn open(&self, physical: PathBuf, version: i32, text: String) -> Result<Self, BoxError> {
        let physical = normalize_physical(physical);
        let expected = self
            .host
            .open_by_physical
            .get(&physical)
            .map(|open| open.generation);
        self.open_expected(physical, expected, version, text)
    }

    pub fn open_expected(
        &self,
        physical: PathBuf,
        expected_generation: Option<u64>,
        version: i32,
        text: String,
    ) -> Result<Self, BoxError> {
        let physical = normalize_physical(physical);
        let current_generation = self
            .host
            .open_by_physical
            .get(&physical)
            .map(|open| open.generation);
        if current_generation != expected_generation {
            return Err(format!("stale didOpen generation {}", physical.display()).into());
        }
        if let Some(generation) = expected_generation {
            return self.change(&physical, generation, version, text);
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
        let generation = next_host
            .next_open_generation
            .checked_add(1)
            .ok_or("LSP open generation overflow")?;
        next_host.next_open_generation = generation;
        next_host.open_by_physical.insert(
            physical,
            OpenDocument {
                logical,
                client_version: version,
                generation,
            },
        );
        self.prepare(next_host, source)
    }

    pub fn change(
        &self,
        physical: &Path,
        generation: u64,
        version: i32,
        text: String,
    ) -> Result<Self, BoxError> {
        let physical = normalize_physical(physical.to_path_buf());
        let open = self
            .host
            .open_by_physical
            .get(&physical)
            .ok_or_else(|| format!("change for non-open document {}", physical.display()))?;
        if open.generation != generation {
            return Err(format!("change for stale open generation {}", physical.display()).into());
        }
        if version <= open.client_version {
            return Err(format!("stale document version {version}").into());
        }
        let logical = open.logical.clone();
        let source = host::replace_document(&self.source, SourceDocument::new(logical, text))?;
        let mut next_host = self.host.clone();
        next_host
            .open_by_physical
            .get_mut(&physical)
            .expect("open document checked")
            .client_version = version;
        self.prepare(next_host, source)
    }

    pub fn close(&self, physical: &Path) -> Result<Option<Self>, BoxError> {
        let physical = normalize_physical(physical.to_path_buf());
        let Some(generation) = self.open_generation(&physical) else {
            return Ok(None);
        };
        self.close_expected(&physical, generation)
    }

    pub fn close_expected(
        &self,
        physical: &Path,
        expected_generation: u64,
    ) -> Result<Option<Self>, BoxError> {
        let physical = normalize_physical(physical.to_path_buf());
        let Some(open) = self.host.open_by_physical.get(&physical) else {
            return Ok(None);
        };
        if open.generation != expected_generation {
            return Ok(None);
        }
        let logical = open.logical.clone();
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
            .any(|(owner, open)| owner != physical && &open.logical == logical);
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

    pub fn open_generation(&self, physical: &Path) -> Option<u64> {
        self.host
            .open_by_physical
            .get(&normalize_physical(physical.to_path_buf()))
            .map(|open| open.generation)
    }

    pub fn client_version(&self, physical: &Path) -> Option<i32> {
        self.host
            .open_by_physical
            .get(&normalize_physical(physical.to_path_buf()))
            .map(|open| open.client_version)
    }

    pub fn diagnostics(&self) -> Vec<DiagnosticPublication> {
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
            output.push(DiagnosticPublication {
                client_version: self.client_version(&physical),
                physical,
                diagnostics,
            });
        }
        output
    }

    fn physical_documents(&self) -> BTreeMap<PathBuf, BundlePath> {
        let mut documents = BTreeMap::new();
        for (physical, disk) in &self.host.disk_by_physical {
            documents.insert(physical.clone(), disk.path().clone());
        }
        for (physical, open) in &self.host.open_by_physical {
            documents.insert(physical.clone(), open.logical.clone());
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
                1,
                "---\ntype: uml.Class\n---\n# Open\n".into(),
            )
            .unwrap();
        let generation = open.open_generation(&physical).unwrap();
        let changed = open
            .change(
                &physical,
                generation,
                2,
                "---\ntype: uml.Class\n---\n# Changed\n".into(),
            )
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
            .open(
                physical.clone(),
                1,
                "---\ntype: Notes\n---\n# Notes\n".into(),
            )
            .unwrap();
        assert_eq!(open.source().documents().len(), 1);
        let closed = open.close(&physical).unwrap().unwrap();
        assert!(closed.source().documents().is_empty());
        assert!(closed.close(&physical).unwrap().is_none());
        assert!(open
            .change(&PathBuf::from("C:/missing.md"), 1, 2, String::new())
            .is_err());
    }

    #[test]
    fn duplicate_and_older_full_changes_are_rejected_without_revision_consumption() {
        let physical = PathBuf::from("C:/outside/order.md");
        let opened = LspAnalysisState::empty()
            .unwrap()
            .open(physical.clone(), 4, "# Four\n".into())
            .unwrap();
        let generation = opened.open_generation(&physical).unwrap();
        let logical = logical_path(None, &physical).unwrap();
        let before_text = opened
            .source()
            .document(&logical)
            .unwrap()
            .slice(0..opened.source().document(&logical).unwrap().text().len())
            .unwrap();
        let before_id = opened.okf().catalog.id_for_path(&logical).unwrap();
        let before_document_revision = opened.okf().catalog.document(before_id).unwrap().revision();

        for version in [4, 3] {
            let rejected = opened.change(&physical, generation, version, "# stale\n".into());
            assert!(rejected.is_err());
            assert_eq!(opened.revision(), 1);
            assert_eq!(opened.source().documents()[0].text(), "# Four\n");
            assert_eq!(opened.okf().catalog.id_for_path(&logical), Some(before_id));
            assert_eq!(
                opened.okf().catalog.document(before_id).unwrap().revision(),
                before_document_revision
            );
            assert_eq!(
                before_text.as_str(),
                opened.source().document(&logical).unwrap().text()
            );
        }
    }

    #[test]
    fn slow_v2_cannot_install_after_v3_wins_the_compare_and_swap() {
        use std::sync::{Arc, Barrier, RwLock};

        let physical = PathBuf::from("C:/outside/order.md");
        let opened = Arc::new(
            LspAnalysisState::empty()
                .unwrap()
                .open(physical.clone(), 1, "# One\n".into())
                .unwrap(),
        );
        let generation = opened.open_generation(&physical).unwrap();
        let current = Arc::new(RwLock::new(opened));
        let v2_prepared = Arc::new(Barrier::new(2));
        let v3_installed = Arc::new(Barrier::new(2));

        let stale_worker = {
            let current = current.clone();
            let physical = physical.clone();
            let v2_prepared = v2_prepared.clone();
            let v3_installed = v3_installed.clone();
            std::thread::spawn(move || {
                let base = current.read().unwrap().clone();
                let _candidate = base
                    .change(&physical, generation, 2, "# Two\n".into())
                    .unwrap();
                v2_prepared.wait();
                v3_installed.wait();
                current
                    .read()
                    .unwrap()
                    .change(&physical, generation, 2, "# Two\n".into())
            })
        };

        v2_prepared.wait();
        let base = current.read().unwrap().clone();
        let v3 = base
            .change(&physical, generation, 3, "# Three\n".into())
            .unwrap();
        *current.write().unwrap() = Arc::new(v3);
        v3_installed.wait();
        assert!(stale_worker.join().unwrap().is_err());
        let final_state = current.read().unwrap();
        assert_eq!(final_state.revision(), 2);
        assert_eq!(final_state.source().documents()[0].text(), "# Three\n");
        assert_eq!(final_state.client_version(&physical), Some(3));
    }

    #[test]
    fn in_flight_change_from_closed_generation_cannot_affect_reopen() {
        let physical = PathBuf::from("C:/outside/order.md");
        let opened = LspAnalysisState::empty()
            .unwrap()
            .open(physical.clone(), 10, "# First open\n".into())
            .unwrap();
        let old_generation = opened.open_generation(&physical).unwrap();
        let closed = opened.close(&physical).unwrap().unwrap();
        let reopened = closed
            .open(physical.clone(), 1, "# Reopened\n".into())
            .unwrap();

        assert_ne!(reopened.open_generation(&physical), Some(old_generation));
        assert!(reopened
            .change(&physical, old_generation, 11, "# Old work\n".into())
            .is_err());
        assert!(reopened
            .open_expected(
                physical.clone(),
                Some(old_generation),
                11,
                "# Old didOpen work\n".into(),
            )
            .is_err());
        assert_eq!(reopened.source().documents()[0].text(), "# Reopened\n");
        assert_eq!(reopened.revision(), 3);
    }

    #[test]
    fn delayed_g1_close_cannot_close_reopened_g2() {
        use std::sync::{Arc, Barrier, RwLock};

        let physical = PathBuf::from("C:/outside/order.md");
        let opened = Arc::new(
            LspAnalysisState::empty()
                .unwrap()
                .open(physical.clone(), 1, "# G1\n".into())
                .unwrap(),
        );
        let g1 = opened.open_generation(&physical).unwrap();
        let current = Arc::new(RwLock::new(opened));
        let g1_prepared = Arc::new(Barrier::new(2));
        let g2_reopened = Arc::new(Barrier::new(2));

        let stale_close = {
            let current = current.clone();
            let physical = physical.clone();
            let g1_prepared = g1_prepared.clone();
            let g2_reopened = g2_reopened.clone();
            std::thread::spawn(move || {
                let base = current.read().unwrap().clone();
                let _candidate = base.close_expected(&physical, g1).unwrap().unwrap();
                g1_prepared.wait();
                g2_reopened.wait();
                current.read().unwrap().close_expected(&physical, g1)
            })
        };

        g1_prepared.wait();
        let base = current.read().unwrap().clone();
        let closed = base.close_expected(&physical, g1).unwrap().unwrap();
        let reopened = closed.open(physical.clone(), 1, "# G2\n".into()).unwrap();
        let g2 = reopened.open_generation(&physical).unwrap();
        assert_ne!(g1, g2);
        let logical = logical_path(None, &physical).unwrap();
        let document_id = reopened.okf().catalog.id_for_path(&logical).unwrap();
        let document_revision = reopened
            .okf()
            .catalog
            .document(document_id)
            .unwrap()
            .revision();
        let allocation = reopened
            .source()
            .document(&logical)
            .unwrap()
            .slice(0..5)
            .unwrap();
        *current.write().unwrap() = Arc::new(reopened);
        g2_reopened.wait();

        assert!(stale_close.join().unwrap().unwrap().is_none());
        let final_state = current.read().unwrap();
        assert_eq!(final_state.revision(), 3);
        assert_eq!(final_state.client_version(&physical), Some(1));
        assert_eq!(final_state.source().documents()[0].text(), "# G2\n");
        assert_eq!(final_state.open_generation(&physical), Some(g2));
        assert_eq!(
            final_state.okf().catalog.id_for_path(&logical),
            Some(document_id)
        );
        assert_eq!(
            final_state
                .okf()
                .catalog
                .document(document_id)
                .unwrap()
                .revision(),
            document_revision
        );
        assert_eq!(allocation.as_str(), "# G2\n");
    }

    #[test]
    fn external_logical_collision_rejects_second_owner_atomically() {
        let first = PathBuf::from("C:/one/../two/order.md");
        let collision = PathBuf::from("C:/one/two/order.md");
        let opened = LspAnalysisState::empty()
            .unwrap()
            .open(first, 1, "# First\n".into())
            .unwrap();

        assert!(opened.open(collision, 1, "# Collision\n".into()).is_err());
        assert_eq!(opened.revision(), 1);
        assert_eq!(opened.source().documents().len(), 1);
        assert_eq!(opened.source().documents()[0].text(), "# First\n");
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
