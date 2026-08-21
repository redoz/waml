use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use tokio::sync::{Mutex, RwLock};
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

use crate::lsp::{
    bundle::{
        is_watched_source, read_disk_document, read_disk_documents, LspAnalysisState, WATCHED_GLOB,
    },
    query::semantic_token_legend,
};

/// Registration id for the watched-files registration, kept as a constant so an
/// `client/unregisterCapability` could ever name the same thing.
const WATCHED_FILES_REGISTRATION: &str = "waml-did-change-watched-files";

struct Backend {
    client: Client,
    current: Arc<RwLock<Arc<LspAnalysisState>>>,
    publication_gate: Arc<Mutex<()>>,
    /// Whether the client said, at `initialize`, that it can take a dynamic
    /// `workspace/didChangeWatchedFiles` registration. The protocol has no
    /// static form for it -- a server that does not register gets no file
    /// events at all -- so this decides between watching and not.
    watched_files_dynamic: AtomicBool,
}

async fn ordered_publish<F, Fut>(
    gate: Arc<Mutex<()>>,
    current: Arc<RwLock<Arc<LspAnalysisState>>>,
    publish: F,
) where
    F: FnOnce(Arc<LspAnalysisState>) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let _send_order = gate.lock().await;
    let snapshot = current.read().await.clone();
    publish(snapshot).await;
}

async fn retain_if_current<T>(
    current: &RwLock<Arc<LspAnalysisState>>,
    captured: &Arc<LspAnalysisState>,
    result: Option<T>,
) -> Option<T> {
    let installed = current.read().await;
    Arc::ptr_eq(&installed, captured)
        .then_some(result)
        .flatten()
}

async fn publish_if_current<F, Fut>(
    current: &RwLock<Arc<LspAnalysisState>>,
    captured: &Arc<LspAnalysisState>,
    publish: F,
) -> bool
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let installed = current.read().await;
    if !Arc::ptr_eq(&installed, captured) {
        return false;
    }
    publish().await;
    drop(installed);
    true
}

/// Run a read-only query against an analysis snapshot and turn a panic inside
/// it into "no answer".
///
/// `waml::analysis` guards the parse itself, but the query side walks the trees
/// that parse produced -- completion, semantic tokens, document symbols, goto
/// definition -- and those walkers read positions the client supplied. Without
/// this, one bad walk kills the server and every open buffer in the workspace
/// loses its diagnostics until the editor restarts it. With it the request
/// answers `null`, which is a response the protocol defines and every client
/// already handles, and the next request is served normally.
///
/// # Unwind safety
///
/// The closure gets `&LspAnalysisState` and nothing else. A snapshot is
/// immutable once installed -- `ingress` builds a whole new one and swaps the
/// `Arc` -- so a query has nothing to leave half-written, and an interrupted
/// one cannot poison the snapshot for the requests that follow. That is what
/// makes `AssertUnwindSafe` honest here rather than a way to silence the
/// compiler.
fn guard_query<T>(
    what: &str,
    physical: &Path,
    state: &LspAnalysisState,
    query: impl FnOnce(&LspAnalysisState) -> Option<T>,
) -> Option<T> {
    // The default panic hook has already printed the message and its source
    // location to the server's stderr, which is where the client's LSP log
    // shows it; this only stops the unwind.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| query(state))) {
        Ok(result) => result,
        Err(_) => {
            eprintln!(
                "WAML language server: {what} panicked on {}; answering with nothing",
                physical.display()
            );
            None
        }
    }
}

/// The same containment for the write side: build a new analysis from an open,
/// change or close, and turn a panic into the failure the caller already knows
/// how to report.
///
/// # Unwind safety
///
/// The operation reads the installed snapshot through `&` and returns a fresh
/// state; the swap happens in `ingress` afterwards and is skipped entirely on
/// this path. So a panic cannot half-install anything, and the server keeps
/// serving the last analysis that did build -- which is also what it does when
/// the operation returns `Err` for an ordinary reason.
fn guard_ingest(
    base: &LspAnalysisState,
    operation: &impl Fn(&LspAnalysisState) -> std::result::Result<Option<LspAnalysisState>, String>,
) -> std::result::Result<Option<LspAnalysisState>, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(base))).unwrap_or_else(
        |_| Err("WAML document ingest panicked; keeping the previous analysis".to_string()),
    )
}

fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                // `includeText: false` on purpose. Between `didOpen` and
                // `didClose` the client's content reaches the server on exactly
                // one channel -- the versioned `didChange` stream -- and
                // `DidSaveTextDocumentParams` carries no version. Accepting its
                // `text` would open a second, unordered content channel that
                // could overwrite a newer `didChange`, which is precisely the
                // class of bug this handler set exists to close.
                save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                    include_text: Some(false),
                })),
                ..Default::default()
            },
        )),
        document_symbol_provider: Some(OneOf::Left(true)),
        definition_provider: Some(OneOf::Left(true)),
        document_link_provider: Some(DocumentLinkOptions {
            resolve_provider: Some(false),
            work_done_progress_options: Default::default(),
        }),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                work_done_progress_options: Default::default(),
                legend: semantic_token_legend(),
                range: None,
                full: Some(SemanticTokensFullOptions::Bool(true)),
            },
        )),
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(false),
            // A space commits the previous word, which is exactly when an empty
            // operand slot appears; `(` opens a link target.
            trigger_characters: Some(vec![" ".to_string(), "(".to_string(), "[".to_string()]),
            ..Default::default()
        }),
        ..Default::default()
    }
}

impl Backend {
    async fn snapshot(&self) -> Arc<LspAnalysisState> {
        self.current.read().await.clone()
    }

    async fn current_query<T>(
        &self,
        what: &str,
        physical: &Path,
        query: impl FnOnce(&LspAnalysisState) -> Option<T>,
    ) -> Option<T> {
        let captured = self.snapshot().await;
        let result = guard_query(what, physical, &captured, query);
        retain_if_current(&self.current, &captured, result).await
    }

    async fn install_initial(&self, root: PathBuf) {
        let (documents, ingest_errors) = read_disk_documents(&root);
        for error in &ingest_errors {
            self.client
                .log_message(MessageType::WARNING, format!("WAML bundle ingest: {error}"))
                .await;
        }
        match LspAnalysisState::from_documents(Some(root.clone()), documents) {
            Ok(state) => *self.current.write().await = Arc::new(state),
            Err(error) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("WAML initialization failed: {error}"),
                    )
                    .await;
            }
        }
    }

    async fn ingress(
        &self,
        operation: impl Fn(&LspAnalysisState) -> std::result::Result<Option<LspAnalysisState>, String>,
    ) -> bool {
        loop {
            let base = self.snapshot().await;
            let candidate = match guard_ingest(&base, &operation) {
                Ok(Some(candidate)) => candidate,
                Ok(None) => return false,
                Err(error) => {
                    self.client.log_message(MessageType::WARNING, error).await;
                    return false;
                }
            };
            let mut current = self.current.write().await;
            if current.revision == base.revision {
                *current = Arc::new(candidate);
                return true;
            }
        }
    }

    /// Ask the client to watch the workspace for us.
    ///
    /// `workspace/didChangeWatchedFiles` has no static server capability -- the
    /// specification says so explicitly -- so a server that wants file events
    /// must send `client/registerCapability` after `initialized`, and only if
    /// the client advertised dynamic registration for it. A client that did not
    /// gets a log line rather than silence, because on such a client every
    /// external edit to a closed file stays invisible until the document is
    /// opened, and that is worth being able to see in the LSP log.
    async fn register_watched_files(&self) {
        if !self.watched_files_dynamic.load(Ordering::Relaxed) {
            self.client
                .log_message(
                    MessageType::INFO,
                    "WAML: client does not support dynamic didChangeWatchedFiles \
                     registration; external edits to closed documents will not be seen",
                )
                .await;
            return;
        }
        let options = DidChangeWatchedFilesRegistrationOptions {
            watchers: vec![FileSystemWatcher {
                glob_pattern: GlobPattern::String(WATCHED_GLOB.to_string()),
                // No `kind`, which the protocol reads as create|change|delete.
                // All three matter: a create adds a link target, a delete
                // removes one, a change rewrites the document.
                kind: None,
            }],
        };
        let registration = Registration {
            id: WATCHED_FILES_REGISTRATION.to_string(),
            method: "workspace/didChangeWatchedFiles".to_string(),
            register_options: serde_json::to_value(options).ok(),
        };
        if let Err(error) = self.client.register_capability(vec![registration]).await {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!("WAML: could not register file watchers: {error}"),
                )
                .await;
        }
    }

    /// Reconcile one path against what a read of it returned just now, and
    /// report whether the analysis moved. See the ownership rule on
    /// [`crate::lsp::bundle`].
    async fn reconcile_disk(&self, physical: PathBuf, disk: Option<String>) -> bool {
        self.ingress(move |base| {
            base.refresh_disk(physical.clone(), disk.clone())
                .map_err(|error| error.to_string())
        })
        .await
    }

    /// Retract the diagnostics a document left behind when it dropped out of
    /// the bundle.
    ///
    /// `publish_all` walks the documents that exist, so a deleted one simply
    /// stops appearing and the client keeps showing whatever it was last told.
    /// An empty set is how the protocol says "there is nothing here any more".
    async fn retract_diagnostics(&self, uri: Uri) {
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn publish_all(&self) {
        let client = self.client.clone();
        let current = self.current.clone();
        ordered_publish(
            self.publication_gate.clone(),
            self.current.clone(),
            move |snapshot| async move {
                for publication in snapshot.diagnostics() {
                    if let Some(uri) = Uri::from_file_path(&publication.physical) {
                        let client = client.clone();
                        if !publish_if_current(&current, &snapshot, move || async move {
                            client
                                .publish_diagnostics(
                                    uri,
                                    publication.diagnostics,
                                    publication.client_version,
                                )
                                .await;
                        })
                        .await
                        {
                            return;
                        }
                    }
                }
            },
        )
        .await;
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        self.watched_files_dynamic.store(
            params
                .capabilities
                .workspace
                .as_ref()
                .and_then(|workspace| workspace.did_change_watched_files.as_ref())
                .and_then(|watched| watched.dynamic_registration)
                .unwrap_or(false),
            Ordering::Relaxed,
        );
        #[allow(deprecated)]
        if let Some(root) = params
            .workspace_folders
            .and_then(|folders| folders.into_iter().next())
            .and_then(|folder| folder.uri.to_file_path().map(|p| p.into_owned()))
        {
            self.install_initial(root).await;
        }
        Ok(InitializeResult {
            capabilities: server_capabilities(),
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        // Watchers first: the diagnostics published below describe a bundle the
        // server has just promised to keep current, and any write that lands
        // between the two would otherwise never be noticed.
        self.register_watched_files().await;
        self.publish_all().await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let Some(physical) = params
            .text_document
            .uri
            .to_file_path()
            .map(|p| p.into_owned())
        else {
            return;
        };
        let text = params.text_document.text;
        let version = params.text_document.version;
        let expected_generation = self.snapshot().await.open_generation(&physical);
        if self
            .ingress(move |base| {
                base.open_expected(physical.clone(), expected_generation, version, text.clone())
                    .map(Some)
                    .map_err(|error| error.to_string())
            })
            .await
        {
            self.publish_all().await;
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let Some(physical) = params
            .text_document
            .uri
            .to_file_path()
            .map(|p| p.into_owned())
        else {
            return;
        };
        let Some(text) = params
            .content_changes
            .into_iter()
            .last()
            .map(|change| change.text)
        else {
            return;
        };
        let generation = self.snapshot().await.open_generation(&physical);
        let Some(generation) = generation else {
            self.client
                .log_message(MessageType::WARNING, "change for non-open document")
                .await;
            return;
        };
        let version = params.text_document.version;
        if self
            .ingress(move |base| {
                base.change(&physical, generation, version, text.clone())
                    .map(Some)
                    .map_err(|error| error.to_string())
            })
            .await
        {
            self.publish_all().await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let Some(physical) = params
            .text_document
            .uri
            .to_file_path()
            .map(|p| p.into_owned())
        else {
            return;
        };
        let Some(expected_generation) = self.snapshot().await.open_generation(&physical) else {
            return;
        };
        // Ownership passes back to the disk here, so the disk is what gets read
        // -- once, before the retry loop below, so a retry cannot smear two
        // different reads together. `None` means the file is gone (deleted or
        // renamed while it was open) and the document goes with it.
        let disk = read_disk_document(&physical);
        let vanished = disk.is_none();
        if self
            .ingress(move |base| {
                base.close_expected(&physical, expected_generation, disk.clone())
                    .map_err(|error| error.to_string())
            })
            .await
        {
            if vanished {
                self.retract_diagnostics(params.text_document.uri).await;
            }
            self.publish_all().await;
        }
    }

    /// A save does not transfer ownership: the client still owns the buffer, so
    /// the analysis content must not move here, and the server deliberately did
    /// not ask for the saved text (see `server_capabilities`).
    ///
    /// The call below is therefore a no-op in the ordinary case --
    /// `refresh_disk` returns `Ok(None)` for an open path. It is routed through
    /// the same reconciliation as everything else anyway, so that a `didSave`
    /// arriving for a path the server does not have open (a client racing a
    /// close against a save) still lands the current bytes instead of being
    /// dropped on the floor.
    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let Some(physical) = params
            .text_document
            .uri
            .to_file_path()
            .map(|p| p.into_owned())
        else {
            return;
        };
        let disk = read_disk_document(&physical);
        if self.reconcile_disk(physical, disk).await {
            self.publish_all().await;
        }
    }

    /// External edits: a branch switch, a formatter, another editor, a
    /// generator. Anything not currently open must be re-read; anything open is
    /// the client's and is left alone until it closes.
    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        let root = self.snapshot().await.host.root.clone();
        let mut moved = false;
        let mut vanished = Vec::new();
        for event in params.changes {
            let Some(physical) = event.uri.to_file_path().map(|p| p.into_owned()) else {
                continue;
            };
            if !is_watched_source(root.as_deref(), &physical) {
                continue;
            }
            // `event.typ` is deliberately not consulted. Watch events are
            // advisory: clients coalesce them, drop them, and deliver them out
            // of order, so a `CREATED` can arrive for a file that has been
            // deleted again since. A read settles it, and a failed read means
            // "no document here" for the same reasons a delete does.
            let disk = read_disk_document(&physical);
            let gone = disk.is_none();
            let changed = self.reconcile_disk(physical, disk).await;
            moved |= changed;
            if changed && gone {
                vanished.push(event.uri);
            }
        }
        if moved {
            // Retract before republishing: the retraction concerns a document
            // that no longer exists, and sending it first means the client is
            // never briefly told both that the file is gone and that it still
            // has problems.
            for uri in vanished {
                self.retract_diagnostics(uri).await;
            }
            self.publish_all().await;
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let Some(physical) = params
            .text_document
            .uri
            .to_file_path()
            .map(|p| p.into_owned())
        else {
            return Ok(None);
        };
        Ok(self
            .current_query("documentSymbol", &physical, |snapshot| {
                snapshot.document_symbols(&physical)
            })
            .await
            .map(DocumentSymbolResponse::Nested))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let Some(physical) = params
            .text_document_position_params
            .text_document
            .uri
            .to_file_path()
            .map(|p| p.into_owned())
        else {
            return Ok(None);
        };
        let position = params.text_document_position_params.position;
        Ok(self
            .current_query("gotoDefinition", &physical, |snapshot| {
                snapshot.definition(&physical, position)
            })
            .await
            .map(GotoDefinitionResponse::Scalar))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let Some(physical) = params
            .text_document_position
            .text_document
            .uri
            .to_file_path()
            .map(|p| p.into_owned())
        else {
            return Ok(None);
        };
        let position = params.text_document_position.position;
        Ok(self
            .current_query("completion", &physical, |snapshot| {
                snapshot.completion(&physical, position)
            })
            .await
            .map(CompletionResponse::Array))
    }

    async fn document_link(&self, params: DocumentLinkParams) -> Result<Option<Vec<DocumentLink>>> {
        let Some(physical) = params
            .text_document
            .uri
            .to_file_path()
            .map(|p| p.into_owned())
        else {
            return Ok(None);
        };
        Ok(self
            .current_query("documentLink", &physical, |snapshot| {
                snapshot.document_links(&physical)
            })
            .await)
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let Some(physical) = params
            .text_document
            .uri
            .to_file_path()
            .map(|p| p.into_owned())
        else {
            return Ok(None);
        };
        Ok(self
            .current_query("semanticTokens/full", &physical, |snapshot| {
                snapshot.semantic_tokens(&physical)
            })
            .await
            .map(SemanticTokensResult::Tokens))
    }
}

pub fn serve_stdio() {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let initial = Arc::new(LspAnalysisState::empty().expect("empty LSP analysis"));
        let (service, socket) = LspService::new(move |client| Backend {
            client,
            current: Arc::new(RwLock::new(initial.clone())),
            publication_gate: Arc::new(Mutex::new(())),
            watched_files_dynamic: AtomicBool::new(false),
        });
        Server::new(stdin, stdout, socket).serve(service).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, path::Path, sync::Mutex as StdMutex};
    use tokio::sync::Notify;

    /// A parser panic must cost the request, not the server.
    ///
    /// The panic is injected rather than provoked with a document: any input
    /// that panics the parser today is a bug to fix, so pinning one as a
    /// fixture would pin the bug. What this guards is the boundary -- that
    /// whatever goes wrong three crates down, the request answers and the next
    /// one is served normally.
    #[test]
    fn a_panicking_query_answers_with_nothing_and_leaves_the_snapshot_usable() {
        let physical = PathBuf::from("C:/outside/seq.md");
        let text = concat!(
            "---\ntype: uml.SequenceDiagram\ntitle: S\n---\n# S\n\n",
            "## Lifelines\n\n- [A](./a.md) as A\n"
        );
        let state = LspAnalysisState::empty()
            .unwrap()
            .open(physical.clone(), 1, text.into())
            .unwrap();

        let answered: Option<u32> = guard_query("completion", &physical, &state, |_| {
            panic!("island parser fell over")
        });
        assert_eq!(answered, None);

        // The snapshot is the same object and still answers, which is the part
        // that matters: one bad request must not cost the workspace its
        // analysis.
        assert!(state.document_symbols(&physical).is_some());
    }

    #[test]
    fn a_panicking_ingest_keeps_the_previous_analysis() {
        let state = LspAnalysisState::empty().unwrap();
        let outcome = guard_ingest(&state, &|_: &LspAnalysisState| {
            panic!("island parser fell over")
        });
        let Err(reason) = outcome else {
            panic!("a contained panic is reported as a failed ingest");
        };
        assert!(reason.contains("panicked"), "{reason}");
    }

    #[test]
    fn a_query_that_does_not_panic_is_untouched_by_the_guard() {
        let physical = PathBuf::from("C:/outside/seq.md");
        let state = LspAnalysisState::empty()
            .unwrap()
            .open(
                physical.clone(),
                1,
                "---\ntype: uml.Class\ntitle: Alpha\n---\n# Alpha\n".into(),
            )
            .unwrap();
        assert!(
            guard_query("documentSymbol", &physical, &state, |snapshot| {
                snapshot.document_symbols(&physical)
            })
            .is_some()
        );
    }

    #[test]
    fn capabilities_advertise_snapshot_queries_and_keep_full_sync() {
        let capabilities = server_capabilities();
        assert!(capabilities.document_symbol_provider.is_some());
        assert!(capabilities.definition_provider.is_some());
        assert!(capabilities.document_link_provider.is_some());
        let Some(SemanticTokensServerCapabilities::SemanticTokensOptions(options)) =
            capabilities.semantic_tokens_provider
        else {
            panic!("semantic token options");
        };
        assert_eq!(options.legend.token_types.len(), 11);
        assert_eq!(options.full, Some(SemanticTokensFullOptions::Bool(true)));
        // Reversed by docs/superpowers/specs/2026-08-10-completion-suggestions-design.md.
        // The previous assertion recorded a deliberate decision not to offer
        // completions; the spec supersedes it, so this is updated rather than
        // deleted.
        let completion = capabilities
            .completion_provider
            .expect("completion is advertised");
        assert_eq!(
            completion.trigger_characters,
            Some(vec![" ".to_string(), "(".to_string(), "[".to_string()])
        );
        assert_eq!(completion.resolve_provider, Some(false));
        let Some(TextDocumentSyncCapability::Options(sync)) = capabilities.text_document_sync
        else {
            panic!("full text sync options");
        };
        assert_eq!(sync.change, Some(TextDocumentSyncKind::FULL));
    }

    #[test]
    fn completion_offers_message_verbs_at_an_empty_verb_slot() {
        let physical = PathBuf::from("C:/outside/seq.md");
        let text = concat!(
            "---\ntype: uml.SequenceDiagram\ntitle: S\n---\n# S\n\n",
            "## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- A \n"
        );
        let state = LspAnalysisState::empty()
            .unwrap()
            .open(physical.clone(), 1, text.into())
            .unwrap();
        // The cursor sits at the end of the "- A " line.
        let line = text.lines().count() as u32 - 1;
        let items = state
            .completion(&physical, Position::new(line, 4))
            .expect("completion returns a list");
        let labels = items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>();
        assert!(labels.contains(&"calls"), "{labels:?}");
        assert!(labels.contains(&"returns"), "{labels:?}");
        assert!(items
            .iter()
            .all(|item| item.text_edit.is_some() || item.insert_text.is_some()));
    }

    #[test]
    fn a_link_candidate_filters_on_its_href_not_its_title() {
        // The library matches a candidate on its label OR its inserted text, so
        // a half-typed href still offers the document. The client then filters
        // the response again, against `filterText` when set and the label
        // otherwise -- and a link candidate's label is the target's *title*. If
        // the server leaves `filterText` unset, typing "./a" hides the very item
        // the author is typing the path of.
        let target = PathBuf::from("C:/outside/a.md");
        let physical = PathBuf::from("C:/outside/seq.md");
        let text = concat!(
            "---\ntype: uml.SequenceDiagram\ntitle: S\n---\n# S\n\n",
            "## Lifelines\n\n- [X](./a)\n"
        );
        let state = LspAnalysisState::empty()
            .unwrap()
            .open(
                target,
                1,
                "---\ntype: uml.Class\ntitle: Alpha Doc\n---\n# Alpha Doc\n".into(),
            )
            .unwrap()
            .open(physical.clone(), 1, text.into())
            .unwrap();
        // The cursor sits just before the ")", at the end of the half-typed href.
        let line = text.lines().count() as u32 - 1;
        let character = "- [X](./a".len() as u32;
        let items = state
            .completion(&physical, Position::new(line, character))
            .expect("completion returns a list");
        let link = items
            .iter()
            .find(|item| item.filter_text.as_deref() == Some("./a.md"))
            .unwrap_or_else(|| panic!("expected the a.md candidate, got {items:?}"));
        // The label is the title, which is precisely why the filter cannot be
        // left to default to it.
        assert_eq!(link.label, "Alpha Doc");
        assert!(
            items.iter().all(|item| item.filter_text.is_some()),
            "every item must carry a filter, got {items:?}"
        );
    }

    #[test]
    fn the_frontmatter_type_value_completes_before_the_document_has_a_type() {
        // The document has no `type:` yet, so it has no UML tree and nothing
        // downstream knows what it is. That is exactly when the author needs
        // to be told what a type: value can be, so this must not be silence.
        let physical = PathBuf::from("C:/outside/new.md");
        let text = "---\ntype: \n---\n# New\n";
        let state = LspAnalysisState::empty()
            .unwrap()
            .open(physical.clone(), 1, text.into())
            .unwrap();
        let items = state
            .completion(&physical, Position::new(1, 6))
            .expect("completion returns a list");
        let labels = items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>();
        for expected in ["uml.Class", "uml.SequenceDiagram", "uml.ClassDiagram"] {
            assert!(labels.contains(&expected), "{expected} missing: {labels:?}");
        }
    }

    #[test]
    fn completion_in_prose_is_an_empty_list_not_an_absent_response() {
        let physical = PathBuf::from("C:/outside/prose.md");
        let state = LspAnalysisState::empty()
            .unwrap()
            .open(physical.clone(), 1, "# Title\n\nJust prose here.\n".into())
            .unwrap();
        assert_eq!(
            state.completion(&physical, Position::new(2, 5)),
            Some(Vec::new())
        );
    }

    #[tokio::test]
    async fn captured_query_result_is_discarded_after_state_replacement() {
        let physical = PathBuf::from("C:/outside/current.md");
        let opened = Arc::new(
            LspAnalysisState::empty()
                .unwrap()
                .open(physical.clone(), 1, "# Old\n".into())
                .unwrap(),
        );
        let generation = opened.open_generation(&physical).unwrap();
        let current = Arc::new(RwLock::new(opened.clone()));
        let changed = opened
            .change(&physical, generation, 2, "# Current\n".into())
            .unwrap();
        *current.write().await = Arc::new(changed);

        assert_eq!(
            retain_if_current(&current, &opened, Some("old result")).await,
            None
        );
    }

    #[tokio::test]
    async fn delayed_old_publication_cannot_arrive_after_newer_publication() {
        let physical = PathBuf::from("C:/outside/order.md");
        let opened = LspAnalysisState::empty()
            .unwrap()
            .open(physical.clone(), 1, "# One\n".into())
            .unwrap();
        let generation = opened.open_generation(&physical).unwrap();
        let current = Arc::new(RwLock::new(Arc::new(opened)));
        let gate = Arc::new(Mutex::new(()));
        let old_started = Arc::new(Notify::new());
        let release_old = Arc::new(Notify::new());
        let sent = Arc::new(StdMutex::new(Vec::new()));

        let old = tokio::spawn(ordered_publish(gate.clone(), current.clone(), {
            let old_started = old_started.clone();
            let release_old = release_old.clone();
            let sent = sent.clone();
            let current = current.clone();
            move |snapshot| async move {
                old_started.notify_one();
                release_old.notified().await;
                if retain_if_current(&current, &snapshot, Some(()))
                    .await
                    .is_some()
                {
                    sent.lock()
                        .unwrap()
                        .push(snapshot.client_version(&physical));
                }
            }
        }));
        old_started.notified().await;

        let newer = current
            .read()
            .await
            .change(
                Path::new("C:/outside/order.md"),
                generation,
                2,
                "# Two\n".into(),
            )
            .unwrap();
        *current.write().await = Arc::new(newer);
        let new = tokio::spawn(ordered_publish(gate.clone(), current.clone(), {
            let sent = sent.clone();
            let current = current.clone();
            move |snapshot| async move {
                if retain_if_current(&current, &snapshot, Some(()))
                    .await
                    .is_some()
                {
                    sent.lock()
                        .unwrap()
                        .push(snapshot.client_version(Path::new("C:/outside/order.md")));
                }
            }
        }));

        release_old.notify_one();
        old.await.unwrap();
        new.await.unwrap();
        assert_eq!(*sent.lock().unwrap(), [Some(2)]);
    }

    #[tokio::test]
    async fn validated_publication_finishes_before_replacement_is_installed() {
        let physical = PathBuf::from("C:/outside/order.md");
        let opened = Arc::new(
            LspAnalysisState::empty()
                .unwrap()
                .open(physical.clone(), 1, "# One\n".into())
                .unwrap(),
        );
        let generation = opened.open_generation(&physical).unwrap();
        let replacement = Arc::new(
            opened
                .change(&physical, generation, 2, "# Two\n".into())
                .unwrap(),
        );
        let current = Arc::new(RwLock::new(opened.clone()));
        let validated = Arc::new(Notify::new());
        let replacement_waiting = Arc::new(Notify::new());
        let release_publication = Arc::new(Notify::new());
        let events = Arc::new(StdMutex::new(Vec::new()));

        let publication = tokio::spawn({
            let current = current.clone();
            let opened = opened.clone();
            let validated = validated.clone();
            let release_publication = release_publication.clone();
            let events = events.clone();
            async move {
                publish_if_current(&current, &opened, move || async move {
                    events.lock().unwrap().push("validated");
                    validated.notify_one();
                    release_publication.notified().await;
                    events.lock().unwrap().push("published");
                })
                .await
            }
        });
        validated.notified().await;

        let install = tokio::spawn({
            let current = current.clone();
            let replacement_waiting = replacement_waiting.clone();
            let events = events.clone();
            async move {
                events.lock().unwrap().push("replacement waiting");
                replacement_waiting.notify_one();
                *current.write().await = replacement;
                events.lock().unwrap().push("replacement installed");
            }
        });
        replacement_waiting.notified().await;

        release_publication.notify_one();
        assert!(publication.await.unwrap());
        install.await.unwrap();
        assert_eq!(
            *events.lock().unwrap(),
            [
                "validated",
                "replacement waiting",
                "published",
                "replacement installed"
            ]
        );
    }

    #[tokio::test]
    async fn cross_document_reanalysis_keeps_each_documents_client_version() {
        let a = PathBuf::from("C:/outside/a.md");
        let b = PathBuf::from("C:/outside/b.md");
        let disk = PathBuf::from("C:/workspace/disk.md");
        let state = LspAnalysisState::from_documents(
            Some(PathBuf::from("C:/workspace")),
            [(disk.clone(), "# Disk\n".into())],
        )
        .unwrap()
        .open(a.clone(), 7, "# A\n".into())
        .unwrap()
        .open(b.clone(), 20, "# B\n".into())
        .unwrap();
        let generation = state.open_generation(&b).unwrap();
        let changed = state
            .change(&b, generation, 21, "# B changed\n".into())
            .unwrap();
        let versions = changed
            .diagnostics()
            .into_iter()
            .map(|publication| (publication.physical, publication.client_version))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(versions.get(&a), Some(&Some(7)));
        assert_eq!(versions.get(&b), Some(&Some(21)));
        assert_eq!(versions.get(&disk), Some(&None));
    }
}
