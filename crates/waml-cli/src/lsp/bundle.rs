//! The analysis state behind the language server, and the ownership rule that
//! decides whose bytes it holds.
//!
//! # Document ownership
//!
//! Exactly one owner supplies the content for any physical path at any moment,
//! and the LSP specification -- not this server -- picks which:
//!
//! * Between `textDocument/didOpen` and `textDocument/didClose` the **client**
//!   owns the file. Its buffer can differ from disk in ways nothing on disk can
//!   show (unsaved edits), so the server must not read the file and must not
//!   let a disk event reach the analysis. [`LspHostIndex::open_by_physical`]
//!   records exactly which paths are in that window.
//! * Outside that window the **disk** owns the file. The server must serve the
//!   bytes that are on disk *now*, and must never fall back to bytes it read
//!   earlier. [`LspHostIndex::disk_by_physical`] is a *shadow* of the last read,
//!   never a fallback: it is only ever written from a read that just happened.
//!
//! The A20 defect was one line breaking the second rule. `close_expected`
//! restored the bytes ingested at `initialize`, so after any external write --
//! a branch switch, another editor, a formatter -- the server served content
//! that matched neither the disk nor anything the user had typed, silently,
//! until the editor was restarted.
//!
//! Every path that learns the disk changed ([`LspAnalysisState::close_expected`]
//! for a close, [`LspAnalysisState::refresh_disk`] for a save or a
//! `workspace/didChangeWatchedFiles` event) takes the freshly-read bytes as an
//! argument: the read happens in [`crate::lsp::server`], which is the layer
//! allowed to touch the filesystem, and this module only ever reconciles what
//! it is handed. If you add a handler that touches document content, decide
//! which of the two owners it speaks for before you write a line of it.

use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

use tower_lsp_server::ls_types as lsp;
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
    /// Shadow of what the last read of each path returned. Only ever written
    /// from a read that just happened -- see the module-level ownership rule.
    /// Never consulted for a path that is also in `open_by_physical`.
    pub disk_by_physical: BTreeMap<PathBuf, SourceDocument>,
    /// The paths the client currently owns, between `didOpen` and `didClose`.
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

    /// Open at whatever generation the host currently holds. Test seam: every
    /// production caller knows the generation it raced against and goes
    /// straight to [`Self::open_expected`].
    #[cfg(test)]
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

    /// Close at whatever generation the host currently holds. Test seam, the
    /// mirror of [`Self::open`]: production goes to `close_expected`.
    #[cfg(test)]
    pub fn close(&self, physical: &Path, disk: Option<String>) -> Result<Option<Self>, BoxError> {
        let physical = normalize_physical(physical.to_path_buf());
        let Some(generation) = self.open_generation(&physical) else {
            return Ok(None);
        };
        self.close_expected(&physical, generation, disk)
    }

    /// Hand the file back to the disk.
    ///
    /// `disk` is what a read of `physical` returned *just now*: `Some(bytes)`
    /// if the file is there and readable, `None` if it is gone. It is not
    /// optional and it is not a cache -- reusing an older read here is the A20
    /// defect, and the reason the caller has to do the read rather than this
    /// method reaching for a stored copy.
    pub fn close_expected(
        &self,
        physical: &Path,
        expected_generation: u64,
        disk: Option<String>,
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
        let source = match disk {
            Some(text) => {
                let document = SourceDocument::new(logical.clone(), text);
                next_host
                    .disk_by_physical
                    .insert(physical, document.clone());
                if self.source.document(&logical).is_some() {
                    host::replace_document(&self.source, document)?
                } else {
                    host::add_document(&self.source, document)?
                }
            }
            None => {
                next_host.disk_by_physical.remove(&physical);
                host::remove_document(&self.source, &logical)?
            }
        };
        self.prepare(next_host, source).map(Some)
    }

    /// Reconcile a path the server has just learned about from the filesystem:
    /// a `workspace/didChangeWatchedFiles` event, or a `textDocument/didSave`.
    ///
    /// `disk` carries the result of a read that just happened, with the same
    /// contract as [`Self::close_expected`]: `None` means the file is gone.
    ///
    /// `Ok(None)` means nothing the server serves would change, so the caller
    /// should not rebuild or republish. That happens in two cases:
    ///
    /// * The path is **open**. The client owns it, so disk events for it are
    ///   noise; `close_expected` re-reads when ownership comes back. Refreshing
    ///   the shadow now would buy nothing and would tempt the next reader into
    ///   treating the shadow as a fallback again.
    /// * The bytes match the shadow. Editors fire watch events for touches that
    ///   changed nothing, and a save right after a `didChange` is the common
    ///   case; re-analyzing the whole bundle for those is pure waste.
    pub fn refresh_disk(
        &self,
        physical: PathBuf,
        disk: Option<String>,
    ) -> Result<Option<Self>, BoxError> {
        let physical = normalize_physical(physical);
        if self.host.open_by_physical.contains_key(&physical) {
            return Ok(None);
        }
        let mut next_host = self.host.clone();
        let source = match disk {
            Some(text) => {
                if self
                    .host
                    .disk_by_physical
                    .get(&physical)
                    .is_some_and(|document| document.text() == text)
                {
                    return Ok(None);
                }
                let logical = logical_path(self.host.root.as_deref(), &physical)?;
                self.reject_collision(&physical, &logical)?;
                let document = SourceDocument::new(logical.clone(), text);
                next_host
                    .disk_by_physical
                    .insert(physical, document.clone());
                if self.source.document(&logical).is_some() {
                    host::replace_document(&self.source, document)?
                } else {
                    host::add_document(&self.source, document)?
                }
            }
            None => {
                let Some(gone) = next_host.disk_by_physical.remove(&physical) else {
                    return Ok(None);
                };
                host::remove_document(&self.source, gone.path())?
            }
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

/// Walk `root` for `.md` files via the shared hardened ingester, normalizing
/// each returned path. Returns the ingested documents and every
/// [`host::ingest::IngestError`] encountered so the caller can surface them
/// instead of silently dropping unreadable or non-UTF-8 content.
pub fn read_disk_documents(
    root: &Path,
) -> (Vec<(PathBuf, String)>, Vec<host::ingest::IngestError>) {
    let ingested = host::ingest::ingest_markdown(
        std::slice::from_ref(&root.to_path_buf()),
        &host::ingest::IngestOptions::default(),
    );
    let files = ingested
        .files
        .into_iter()
        .map(|(path, text)| (normalize_physical(path), text))
        .collect();
    (files, ingested.errors)
}

/// The glob the server asks the client to watch on its behalf.
///
/// It has to be a superset of what [`read_disk_documents`] would ingest --
/// missing a file means missing an edit -- and [`is_watched_source`] narrows the
/// events back down to that set.
pub const WATCHED_GLOB: &str = "**/*.md";

/// Read one file the way the startup walk would.
///
/// `None` covers every reason the bytes are unavailable -- absent, unreadable,
/// not UTF-8 -- because they all mean the same thing to the bundle: there is no
/// document here. Callers must not substitute an older read for `None`; that is
/// the A20 defect.
pub fn read_disk_document(physical: &Path) -> Option<String> {
    std::fs::read_to_string(physical).ok()
}

/// Whether a path the client reported as changed is one this server would have
/// ingested at startup.
///
/// The startup walk runs with [`host::ingest::IngestOptions::default`], which
/// skips dot-directories. A client watcher glob cannot express that, so the
/// filter is re-applied here: without it a write under `.git/` would pull a
/// document into the bundle that the next restart would silently drop again,
/// and the server's answers would depend on how long it had been running.
pub fn is_watched_source(root: Option<&Path>, physical: &Path) -> bool {
    match physical.extension() {
        Some(extension) if extension.eq_ignore_ascii_case("md") => {}
        _ => return false,
    }
    // No root means no watcher registration either, so an event for such a
    // path did not come from a watcher this server asked for.
    let Some(relative) = root.and_then(|root| physical.strip_prefix(root).ok()) else {
        return false;
    };
    // Only the directories are checked: `ingest_markdown` skips dot-*dirs*, and
    // reads a dotted *file* like `.notes.md` normally.
    !relative
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .any(|component| match component {
            Component::Normal(segment) => segment.to_string_lossy().starts_with('.'),
            _ => false,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An absolute workspace root for the running platform.
    ///
    /// Most fixtures here treat their path as an opaque key, so a literal
    /// `C:/workspace` is harmless for them. This one is different: it asserts on
    /// `Uri::from_file_path`, which requires a genuinely absolute path. On Linux
    /// `C:/workspace` is *relative* -- no leading slash -- so URL construction
    /// returned None, `definition` found no target, and the test unwrapped None.
    fn workspace_root() -> PathBuf {
        let root = if cfg!(windows) {
            PathBuf::from("C:/workspace")
        } else {
            PathBuf::from("/workspace")
        };
        assert!(
            root.is_absolute(),
            "fixture root must be absolute on this platform: {root:?}"
        );
        root
    }

    fn query_fixture() -> (LspAnalysisState, PathBuf, PathBuf) {
        let root = workspace_root();
        let order = root.join("order.md");
        let next = root.join("next.md");
        let order_text = "---\ntype: uml.Class\n---\n# 😀 Order\n\nSee [Next](./next.md).\n\n## Attributes\n- count: Number {0..42}\n";
        let next_text = "---\ntype: uml.Class\n---\n# Next\n";
        let state = LspAnalysisState::from_documents(
            Some(root),
            [
                (order.clone(), order_text.into()),
                (next.clone(), next_text.into()),
            ],
        )
        .unwrap();
        (state, order, next)
    }

    fn absolute_tokens(tokens: &lsp::SemanticTokens) -> Vec<(u32, u32, u32, u32)> {
        let mut line = 0;
        let mut character = 0;
        tokens
            .data
            .iter()
            .map(|token| {
                line += token.delta_line;
                character = if token.delta_line == 0 {
                    character + token.delta_start
                } else {
                    token.delta_start
                };
                (line, character, token.length, token.token_type)
            })
            .collect()
    }

    #[test]
    fn snapshot_queries_publish_headings_links_and_definitions() {
        let (state, order, next) = query_fixture();

        let symbols = state.document_symbols(&order).unwrap();
        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.name.as_str())
                .collect::<Vec<_>>(),
            ["😀 Order", "Attributes"]
        );
        assert_eq!(
            symbols[0].selection_range,
            lsp::Range::new(lsp::Position::new(3, 2), lsp::Position::new(3, 10))
        );

        let links = state.document_links(&order).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, lsp::Uri::from_file_path(&next));
        assert_eq!(links[0].range.start, lsp::Position::new(5, 4));

        let definition = state.definition(&order, lsp::Position::new(5, 6)).unwrap();
        assert_eq!(definition.uri, lsp::Uri::from_file_path(&next).unwrap());
        assert_eq!(definition.range.start, lsp::Position::new(3, 0));
        assert!(state
            .definition(&order, lsp::Position::new(5, 21))
            .is_none());
    }

    #[test]
    fn semantic_tokens_use_fixed_roles_and_exact_astral_utf16_columns() {
        let (state, order, _) = query_fixture();

        let tokens = state.semantic_tokens(&order).unwrap();
        let absolute = absolute_tokens(&tokens);
        assert!(
            absolute.contains(&(3, 0, 1, 0)),
            "heading marker: {absolute:?}"
        );
        assert!(
            absolute.contains(&(3, 2, 8, 1)),
            "astral heading: {absolute:?}"
        );
        assert!(
            absolute.contains(&(5, 4, 17, 2)),
            "Markdown link: {absolute:?}"
        );
        assert!(
            absolute.iter().any(|token| token.3 == 8),
            "embedded WAML property: {absolute:?}"
        );
        assert!(
            absolute.iter().any(|token| token.3 == 7),
            "embedded WAML type: {absolute:?}"
        );
        assert!(
            absolute.windows(2).all(|pair| {
                pair[0].0 < pair[1].0 || (pair[0].0 == pair[1].0 && pair[0].1 <= pair[1].1)
            }),
            "semantic tokens must be sorted: {absolute:?}"
        );
    }

    #[test]
    fn semantic_tokens_project_waml_roles_from_fenced_blocks() {
        let physical = PathBuf::from("C:/workspace/fenced.md");
        let state = LspAnalysisState::empty()
            .unwrap()
            .open(
                physical.clone(),
                1,
                "# Example\n\n```waml\n## Attributes\n- unknown: Number {0..42}\n```\n".into(),
            )
            .unwrap();

        let absolute = absolute_tokens(&state.semantic_tokens(&physical).unwrap());
        for expected in [(4, 2, 7, 8), (4, 11, 6, 7), (4, 19, 5, 9)] {
            assert!(
                absolute.contains(&expected),
                "fenced WAML token {expected:?}: {absolute:?}"
            );
        }
    }

    #[test]
    fn semantic_link_role_wins_when_a_link_is_the_heading_content() {
        let root = PathBuf::from("C:/workspace");
        let source = root.join("source.md");
        let target = root.join("target.md");
        let state = LspAnalysisState::from_documents(
            Some(root),
            [
                (source.clone(), "# [Target](./target.md)\n".into()),
                (target, "# Target\n".into()),
            ],
        )
        .unwrap();

        let absolute = absolute_tokens(&state.semantic_tokens(&source).unwrap());
        assert!(
            absolute.contains(&(0, 2, 21, 2)),
            "link token: {absolute:?}"
        );
        assert!(
            !absolute.iter().any(|token| token.0 == 0 && token.3 == 1),
            "overlapping heading token: {absolute:?}"
        );
    }

    #[test]
    fn semantic_tokens_keep_markdown_roles_when_an_island_has_no_code_projection() {
        let physical = PathBuf::from("C:/outside/notes.md");
        let state = LspAnalysisState::empty()
            .unwrap()
            .open(
                physical.clone(),
                1,
                "---\ntype: Notes\n---\n# Notes\n\n## Attributes\nplain text\n".into(),
            )
            .unwrap();

        let absolute = absolute_tokens(&state.semantic_tokens(&physical).unwrap());
        assert!(
            absolute.iter().any(|token| token.3 == 1),
            "Markdown headings remain available: {absolute:?}"
        );
    }

    #[test]
    fn definition_does_not_fall_back_when_the_authored_fragment_is_missing() {
        let root = PathBuf::from("C:/workspace");
        let source = root.join("source.md");
        let target = root.join("target.md");
        let state = LspAnalysisState::from_documents(
            Some(root),
            [
                (
                    source.clone(),
                    "# Source\n\n[Missing](./target.md#missing)\n".into(),
                ),
                (target, "# Target\n".into()),
            ],
        )
        .unwrap();

        assert!(state
            .definition(&source, lsp::Position::new(2, 2))
            .is_none());
    }

    #[test]
    fn queries_read_only_the_current_replacement_snapshot() {
        let physical = PathBuf::from("C:/outside/current.md");
        let opened = LspAnalysisState::empty()
            .unwrap()
            .open(physical.clone(), 1, "# Old\n".into())
            .unwrap();
        let generation = opened.open_generation(&physical).unwrap();
        let changed = opened
            .change(&physical, generation, 2, "# Current 😀\n".into())
            .unwrap();

        let names = changed
            .document_symbols(&physical)
            .unwrap()
            .into_iter()
            .map(|symbol| symbol.name)
            .collect::<Vec<_>>();
        assert_eq!(names, ["Current 😀"]);
    }

    #[test]
    fn atomic_snapshot_open_change_close_installs_fresh_disk_and_revision_alignment() {
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
        // A20: the bytes close installs are the ones the caller just read, not
        // the ones ingested at startup. They differ here on purpose -- that is
        // the whole difference between a fresh read and a stale snapshot.
        let now_on_disk = "---\ntype: uml.Class\n---\n# Rewritten\n";
        assert_ne!(now_on_disk, disk);
        let closed = changed
            .close(&physical, Some(now_on_disk.into()))
            .unwrap()
            .unwrap();
        assert_eq!(closed.revision, 3);
        assert_eq!(closed.source.documents()[0].text(), now_on_disk);
        assert_eq!(closed.okf.catalog.session_revision(), closed.revision);
        assert_eq!(closed.uml.session_revision(), closed.revision);
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
        assert_eq!(open.source.documents().len(), 1);
        let closed = open.close(&physical, None).unwrap().unwrap();
        assert!(closed.source.documents().is_empty());
        assert!(closed.close(&physical, None).unwrap().is_none());
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
            .source
            .document(&logical)
            .unwrap()
            .slice(0..opened.source.document(&logical).unwrap().text().len())
            .unwrap();
        let before_id = opened.okf.catalog.id_for_path(&logical).unwrap();
        let before_document_revision = opened.okf.catalog.document(before_id).unwrap().revision();

        for version in [4, 3] {
            let rejected = opened.change(&physical, generation, version, "# stale\n".into());
            assert!(rejected.is_err());
            assert_eq!(opened.revision, 1);
            assert_eq!(opened.source.documents()[0].text(), "# Four\n");
            assert_eq!(opened.okf.catalog.id_for_path(&logical), Some(before_id));
            assert_eq!(
                opened.okf.catalog.document(before_id).unwrap().revision(),
                before_document_revision
            );
            assert_eq!(
                before_text.as_str(),
                opened.source.document(&logical).unwrap().text()
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
        assert_eq!(final_state.revision, 2);
        assert_eq!(final_state.source.documents()[0].text(), "# Three\n");
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
        let closed = opened.close(&physical, None).unwrap().unwrap();
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
        assert_eq!(reopened.source.documents()[0].text(), "# Reopened\n");
        assert_eq!(reopened.revision, 3);
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
                let _candidate = base.close_expected(&physical, g1, None).unwrap().unwrap();
                g1_prepared.wait();
                g2_reopened.wait();
                current.read().unwrap().close_expected(&physical, g1, None)
            })
        };

        g1_prepared.wait();
        let base = current.read().unwrap().clone();
        let closed = base.close_expected(&physical, g1, None).unwrap().unwrap();
        let reopened = closed.open(physical.clone(), 1, "# G2\n".into()).unwrap();
        let g2 = reopened.open_generation(&physical).unwrap();
        assert_ne!(g1, g2);
        let logical = logical_path(None, &physical).unwrap();
        let document_id = reopened.okf.catalog.id_for_path(&logical).unwrap();
        let document_revision = reopened
            .okf
            .catalog
            .document(document_id)
            .unwrap()
            .revision();
        let allocation = reopened
            .source
            .document(&logical)
            .unwrap()
            .slice(0..5)
            .unwrap();
        *current.write().unwrap() = Arc::new(reopened);
        g2_reopened.wait();

        assert!(stale_close.join().unwrap().unwrap().is_none());
        let final_state = current.read().unwrap();
        assert_eq!(final_state.revision, 3);
        assert_eq!(final_state.client_version(&physical), Some(1));
        assert_eq!(final_state.source.documents()[0].text(), "# G2\n");
        assert_eq!(final_state.open_generation(&physical), Some(g2));
        assert_eq!(
            final_state.okf.catalog.id_for_path(&logical),
            Some(document_id)
        );
        assert_eq!(
            final_state
                .okf
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
        assert_eq!(opened.revision, 1);
        assert_eq!(opened.source.documents().len(), 1);
        assert_eq!(opened.source.documents()[0].text(), "# First\n");
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

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "waml-lsp-bundle-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn read_disk_documents_skips_discovered_dot_directories() {
        let temp = TempDir::new("dotdir");
        std::fs::write(temp.0.join("order.md"), "# Order\n").unwrap();
        std::fs::create_dir(temp.0.join(".waml")).unwrap();
        std::fs::write(temp.0.join(".waml/hidden.md"), "# Hidden\n").unwrap();

        let (documents, errors) = read_disk_documents(&temp.0);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].0.file_name().unwrap(), "order.md");
    }

    #[test]
    fn read_disk_documents_reports_non_utf8_instead_of_dropping_it() {
        let temp = TempDir::new("nonutf8");
        std::fs::write(temp.0.join("bad.md"), [0xff, 0xfe, 0x00, 0x01]).unwrap();

        let (documents, errors) = read_disk_documents(&temp.0);
        assert!(documents.is_empty());
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].kind, host::ingest::IngestErrorKind::NotUtf8);
    }

    /// The client half of the ownership rule: while a document is open, no
    /// amount of disk churn may reach the analysis -- and the moment it closes,
    /// the disk wins outright.
    #[test]
    fn disk_refresh_cannot_touch_a_document_the_client_owns() {
        let root = workspace_root();
        let physical = root.join("order.md");
        let state = LspAnalysisState::from_documents(
            Some(root),
            [(
                physical.clone(),
                "---\ntype: uml.Class\n---\n# AtStartup\n".to_string(),
            )],
        )
        .unwrap();
        let open = state
            .open(
                physical.clone(),
                1,
                "---\ntype: uml.Class\n---\n# WhileOpen\n".into(),
            )
            .unwrap();

        let churn = "---\ntype: uml.Class\n---\n# OnDisk\n";
        assert!(
            open.refresh_disk(physical.clone(), Some(churn.into()))
                .unwrap()
                .is_none(),
            "an open document is the client's; a disk event for it changes nothing"
        );
        assert_eq!(
            open.source.documents()[0].text(),
            "---\ntype: uml.Class\n---\n# WhileOpen\n"
        );

        let closed = open.close(&physical, Some(churn.into())).unwrap().unwrap();
        assert_eq!(closed.source.documents()[0].text(), churn);
    }

    /// The disk half: create, change and delete of a document the client never
    /// opened, plus the no-op short circuit that keeps watch-event noise from
    /// re-analyzing the bundle.
    #[test]
    fn disk_refresh_creates_changes_and_deletes_closed_documents() {
        let root = workspace_root();
        let physical = root.join("late.md");
        let state = LspAnalysisState::from_documents(Some(root), []).unwrap();
        assert!(state.source.documents().is_empty());

        let first = "---\ntype: uml.Class\n---\n# Late\n";
        let created = state
            .refresh_disk(physical.clone(), Some(first.into()))
            .unwrap()
            .unwrap();
        assert_eq!(created.source.documents()[0].text(), first);

        assert!(
            created
                .refresh_disk(physical.clone(), Some(first.into()))
                .unwrap()
                .is_none(),
            "identical bytes must not cost a re-analysis"
        );

        let second = "---\ntype: uml.Class\n---\n# Later\n";
        let changed = created
            .refresh_disk(physical.clone(), Some(second.into()))
            .unwrap()
            .unwrap();
        assert_eq!(changed.source.documents()[0].text(), second);

        let deleted = changed
            .refresh_disk(physical.clone(), None)
            .unwrap()
            .unwrap();
        assert!(deleted.source.documents().is_empty());
        assert!(
            deleted.refresh_disk(physical, None).unwrap().is_none(),
            "deleting what is already gone must not cost a re-analysis"
        );
    }

    /// A rename reaches the server as a delete plus a create, which has to land
    /// as one document moving rather than two documents existing.
    #[test]
    fn disk_refresh_carries_a_rename_across_as_delete_plus_create() {
        let root = workspace_root();
        let before = root.join("before.md");
        let after = root.join("after.md");
        let text = "---\ntype: uml.Class\n---\n# Moved\n";
        let state =
            LspAnalysisState::from_documents(Some(root), [(before.clone(), text.to_string())])
                .unwrap();

        let removed = state.refresh_disk(before, None).unwrap().unwrap();
        let added = removed
            .refresh_disk(after, Some(text.into()))
            .unwrap()
            .unwrap();
        assert_eq!(added.source.documents().len(), 1);
        assert_eq!(added.source.documents()[0].path().as_str(), "after.md");
    }

    #[test]
    fn watched_events_are_filtered_to_what_the_startup_walk_would_ingest() {
        let root = workspace_root();
        assert!(is_watched_source(Some(&root), &root.join("order.md")));
        assert!(is_watched_source(Some(&root), &root.join("docs/order.md")));
        // A dotted *file* is ingested; only dot-*directories* are skipped.
        assert!(is_watched_source(Some(&root), &root.join(".notes.md")));
        assert!(!is_watched_source(Some(&root), &root.join(".git/HEAD.md")));
        assert!(!is_watched_source(
            Some(&root),
            &root.join("docs/.waml/cache.md")
        ));
        assert!(!is_watched_source(Some(&root), &root.join("order.txt")));
        assert!(!is_watched_source(Some(&root), &root.join("order")));
        // Outside the root, or with no root at all, no watcher was registered.
        assert!(!is_watched_source(
            Some(&root),
            Path::new("/elsewhere/x.md")
        ));
        assert!(!is_watched_source(None, &root.join("order.md")));
    }
}
