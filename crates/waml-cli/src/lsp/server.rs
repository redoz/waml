use std::{path::PathBuf, sync::Arc};

use tokio::sync::{Mutex, RwLock};
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

use crate::lsp::{
    bundle::{read_disk_documents, LspAnalysisState},
    query::semantic_token_legend,
};

struct Backend {
    client: Client,
    current: Arc<RwLock<Arc<LspAnalysisState>>>,
    publication_gate: Arc<Mutex<()>>,
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

fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
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
        ..Default::default()
    }
}

impl Backend {
    async fn snapshot(&self) -> Arc<LspAnalysisState> {
        self.current.read().await.clone()
    }

    async fn current_query<T>(
        &self,
        query: impl FnOnce(&LspAnalysisState) -> Option<T>,
    ) -> Option<T> {
        let captured = self.snapshot().await;
        let result = query(&captured);
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
            let candidate = match operation(&base) {
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
        if self
            .ingress(move |base| {
                base.close_expected(&physical, expected_generation)
                    .map_err(|error| error.to_string())
            })
            .await
        {
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
            .current_query(|snapshot| snapshot.document_symbols(&physical))
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
            .current_query(|snapshot| snapshot.definition(&physical, position))
            .await
            .map(GotoDefinitionResponse::Scalar))
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
            .current_query(|snapshot| snapshot.document_links(&physical))
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
            .current_query(|snapshot| snapshot.semantic_tokens(&physical))
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
        });
        Server::new(stdin, stdout, socket).serve(service).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, path::Path, sync::Mutex as StdMutex};
    use tokio::sync::Notify;

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
        assert!(capabilities.completion_provider.is_none());
        let Some(TextDocumentSyncCapability::Options(sync)) = capabilities.text_document_sync
        else {
            panic!("full text sync options");
        };
        assert_eq!(sync.change, Some(TextDocumentSyncKind::FULL));
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
