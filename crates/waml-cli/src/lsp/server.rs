use std::{path::PathBuf, sync::Arc};

use tokio::sync::{Mutex, RwLock};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::lsp::bundle::{read_disk_documents, LspAnalysisState};

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

impl Backend {
    async fn snapshot(&self) -> Arc<LspAnalysisState> {
        self.current.read().await.clone()
    }

    async fn install_initial(&self, root: PathBuf) {
        match LspAnalysisState::from_documents(Some(root.clone()), read_disk_documents(&root)) {
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
        ordered_publish(
            self.publication_gate.clone(),
            self.current.clone(),
            move |snapshot| async move {
                for publication in snapshot.diagnostics() {
                    if let Ok(uri) = Url::from_file_path(&publication.physical) {
                        client
                            .publish_diagnostics(
                                uri,
                                publication.diagnostics,
                                publication.client_version,
                            )
                            .await;
                    }
                }
            },
        )
        .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        #[allow(deprecated)]
        if let Some(root) = params
            .workspace_folders
            .and_then(|folders| folders.into_iter().next())
            .and_then(|folder| folder.uri.to_file_path().ok())
        {
            self.install_initial(root).await;
        }
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.publish_all().await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let Ok(physical) = params.text_document.uri.to_file_path() else {
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
        let Ok(physical) = params.text_document.uri.to_file_path() else {
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
        let Ok(physical) = params.text_document.uri.to_file_path() else {
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

    async fn shutdown(&self) -> Result<()> {
        Ok(())
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

    #[tokio::test]
    async fn delayed_old_publication_cannot_arrive_after_newer_publication() {
        let physical = PathBuf::from("C:/outside/order.md");
        let opened = LspAnalysisState::empty()
            .unwrap()
            .open(physical.clone(), 1, "# One\n".into())
            .unwrap();
        let generation = opened.open_generation(&physical).unwrap();
        let current = Arc::new(RwLock::new(Arc::new(opened)));
        let gate = Arc::new(tokio::sync::Mutex::new(()));
        let old_started = Arc::new(Notify::new());
        let release_old = Arc::new(Notify::new());
        let sent = Arc::new(StdMutex::new(Vec::new()));

        let old = tokio::spawn(ordered_publish(gate.clone(), current.clone(), {
            let old_started = old_started.clone();
            let release_old = release_old.clone();
            let sent = sent.clone();
            move |snapshot| async move {
                old_started.notify_one();
                release_old.notified().await;
                sent.lock()
                    .unwrap()
                    .push(snapshot.client_version(&physical));
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
            move |snapshot| async move {
                sent.lock()
                    .unwrap()
                    .push(snapshot.client_version(Path::new("C:/outside/order.md")));
            }
        }));

        release_old.notify_one();
        old.await.unwrap();
        new.await.unwrap();
        assert_eq!(*sent.lock().unwrap(), [Some(1), Some(2)]);
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
