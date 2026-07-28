use std::{path::PathBuf, sync::Arc};

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::lsp::bundle::{read_disk_documents, LspAnalysisState};

struct Backend {
    client: Client,
    current: RwLock<Arc<LspAnalysisState>>,
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
        let snapshot = self.snapshot().await;
        let revision = i32::try_from(snapshot.revision).ok();
        let diagnostics = snapshot.diagnostics();
        if self.current.read().await.revision != snapshot.revision {
            return;
        }
        for (path, diagnostics) in diagnostics {
            if self.current.read().await.revision != snapshot.revision {
                return;
            }
            if let Ok(uri) = Url::from_file_path(&path) {
                self.client
                    .publish_diagnostics(uri, diagnostics, revision)
                    .await;
            }
        }
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
        if self
            .ingress(move |base| {
                base.open(physical.clone(), text.clone())
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
        if self
            .ingress(move |base| {
                base.change(&physical, text.clone())
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
        if self
            .ingress(move |base| base.close(&physical).map_err(|error| error.to_string()))
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
            current: RwLock::new(initial.clone()),
        });
        Server::new(stdin, stdout, socket).serve(service).await;
    });
}
