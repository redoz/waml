//! `tower-lsp` server: lifecycle, didOpen/didChange, publish diagnostics.
//!
//! FULL text sync — each edit carries the whole document, which we overlay
//! onto the in-memory bundle and re-validate as a whole.

use std::sync::Arc;

use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::lsp::bundle::Workspace;

struct Backend {
    client: Client,
    ws: Arc<Mutex<Workspace>>,
}

impl Backend {
    async fn publish_all(&self) {
        let snapshot = { self.ws.lock().await.diagnostics() };
        for (path, diags) in snapshot {
            if let Ok(uri) = Url::from_file_path(std::path::Path::new(&path)) {
                self.client.publish_diagnostics(uri, diags, None).await;
            }
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        #[allow(deprecated)]
        let folders = params.workspace_folders;
        if let Some(folder) = folders.and_then(|f| f.into_iter().next()) {
            if let Ok(root) = folder.uri.to_file_path() {
                self.ws.lock().await.seed_from_glob(&root);
            }
        }
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.publish_all().await;
    }

    async fn did_open(&self, p: DidOpenTextDocumentParams) {
        let path = p
            .text_document
            .uri
            .to_file_path()
            .map(|x| x.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        self.ws.lock().await.overlay(path, p.text_document.text);
        self.publish_all().await;
    }

    async fn did_change(&self, p: DidChangeTextDocumentParams) {
        // FULL sync: the last content change is the whole document.
        if let Some(change) = p.content_changes.into_iter().last() {
            let path = p
                .text_document
                .uri
                .to_file_path()
                .map(|x| x.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            self.ws.lock().await.overlay(path, change.text);
            self.publish_all().await;
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

/// Build a tokio runtime and serve the language server over stdin/stdout.
pub fn serve_stdio() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let (service, socket) = LspService::new(|client| Backend {
            client,
            ws: Arc::new(Mutex::new(Workspace::new())),
        });
        Server::new(stdin, stdout, socket).serve(service).await;
    });
}
