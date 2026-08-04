//! The API router: one guard layer in front of the read table and the two
//! write surfaces `ServeState` implements. UI routes (`serve::ui`) merge in
//! alongside it when `run` mounts the embedded web editor; with no UI router
//! merged (or with `--api-only`), every non-`/api` path answers 404.

use std::sync::{Arc, Mutex};

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops_dto::OpDto;
use crate::serve::guard::{check, Deny, Guard, ReqFacts};
use crate::serve::state::{ApplyFailure, DocumentWrite, ServeState};

/// Shared server state: the loaded bundle behind a `std::sync::Mutex` (never
/// held across an `.await` — every handler does its CPU work synchronously
/// under the lock) plus the access-control guard.
#[derive(Clone)]
pub struct App {
    pub state: Arc<Mutex<ServeState>>,
    pub guard: Arc<Guard>,
}

/// The API router, optionally merged with the UI router (`serve::ui`). The
/// UI router answers via a catch-all fallback, so it must be merged in —
/// never made the base router — or it would shadow the `/api/*` routes.
pub fn router(app: App, ui: Option<Router>) -> Router {
    let api = Router::new()
        .route("/api/bundle", get(get_bundle))
        .route("/api/model", get(get_model))
        .route("/api/diagnostics", get(get_diagnostics))
        .route("/api/ops", post(post_ops))
        .route("/api/documents", post(post_documents))
        .with_state(app);
    match ui {
        Some(ui) => api.merge(ui),
        None => api,
    }
}

#[derive(Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

fn facts_of<'a>(headers: &'a HeaderMap, token_q: &'a TokenQuery, mutating: bool) -> ReqFacts<'a> {
    let bearer = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let origin = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|v| v.to_str().ok());
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok());
    let client_header = headers.get("X-Waml-Client").and_then(|v| v.to_str().ok());
    ReqFacts {
        bearer,
        query_token: token_q.token.as_deref(),
        origin,
        host,
        client_header,
        mutating,
    }
}

fn deny_response(deny: Deny) -> Response {
    match deny {
        Deny::Unauthorized => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "unauthorized"})),
        )
            .into_response(),
        Deny::Forbidden(reason) => {
            (StatusCode::FORBIDDEN, Json(json!({"error": reason}))).into_response()
        }
    }
}

fn admit(
    app: &App,
    headers: &HeaderMap,
    token_q: &TokenQuery,
    mutating: bool,
) -> Result<(), Box<Response>> {
    let facts = facts_of(headers, token_q, mutating);
    check(&app.guard, &facts).map_err(|deny| Box::new(deny_response(deny)))
}

/// Lock the serve state, recovering a poisoned mutex. A panic while a
/// previous request held the lock must cost that request only, not turn
/// every later `/api` call into a panic for the rest of the session; the
/// state's write paths only install a new `PreparedCandidate` as their last
/// step, so the recovered value is the last fully-applied one.
fn lock_state(app: &App) -> std::sync::MutexGuard<'_, ServeState> {
    app.state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Deserialize a request body only after `admit` has accepted the request
/// (guard.rs: access checks happen before any body work).
fn parse_body<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, Box<Response>> {
    serde_json::from_slice(body).map_err(|err| {
        Box::new(
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid request body", "reason": err.to_string()})),
            )
                .into_response(),
        )
    })
}

async fn get_bundle(
    State(app): State<App>,
    Query(token_q): Query<TokenQuery>,
    headers: HeaderMap,
) -> Response {
    if let Err(resp) = admit(&app, &headers, &token_q, false) {
        return *resp;
    }
    let state = lock_state(&app);
    match state.bundle_envelope() {
        Ok(body) => {
            let mut resp = body.into_response();
            resp.headers_mut().insert(
                "X-Waml-Revision",
                state
                    .revision()
                    .to_string()
                    .parse()
                    .expect("revision header value"),
            );
            resp
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err})),
        )
            .into_response(),
    }
}

async fn get_model(
    State(app): State<App>,
    Query(token_q): Query<TokenQuery>,
    headers: HeaderMap,
) -> Response {
    if let Err(resp) = admit(&app, &headers, &token_q, false) {
        return *resp;
    }
    let state = lock_state(&app);
    Json(json!({
        "revision": state.revision(),
        "model": state.model(),
    }))
    .into_response()
}

async fn get_diagnostics(
    State(app): State<App>,
    Query(token_q): Query<TokenQuery>,
    headers: HeaderMap,
) -> Response {
    if let Err(resp) = admit(&app, &headers, &token_q, false) {
        return *resp;
    }
    let state = lock_state(&app);
    Json(json!({
        "revision": state.revision(),
        "diagnostics": state.diagnostics(),
    }))
    .into_response()
}

#[derive(Deserialize)]
struct OpsRequest {
    revision: u64,
    ops: Vec<OpDto>,
}

#[derive(Serialize)]
struct OpsResponse {
    revision: u64,
    changed: Vec<(String, String)>,
}

async fn post_ops(
    State(app): State<App>,
    Query(token_q): Query<TokenQuery>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if let Err(resp) = admit(&app, &headers, &token_q, true) {
        return *resp;
    }
    let body: OpsRequest = match parse_body(&body) {
        Ok(body) => body,
        Err(resp) => return *resp,
    };
    let mut state = lock_state(&app);
    match state.apply_ops(body.revision, &body.ops) {
        Ok(changed) => Json(OpsResponse {
            revision: state.revision(),
            changed,
        })
        .into_response(),
        Err(failure) => apply_failure_response(failure),
    }
}

#[derive(Deserialize)]
struct DocumentsRequest {
    revision: u64,
    writes: Vec<DocumentWrite>,
}

#[derive(Serialize)]
struct DocumentsResponse {
    revision: u64,
}

async fn post_documents(
    State(app): State<App>,
    Query(token_q): Query<TokenQuery>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if let Err(resp) = admit(&app, &headers, &token_q, true) {
        return *resp;
    }
    let body: DocumentsRequest = match parse_body(&body) {
        Ok(body) => body,
        Err(resp) => return *resp,
    };
    let mut state = lock_state(&app);
    match state.apply_documents(body.revision, &body.writes) {
        Ok(()) => Json(DocumentsResponse {
            revision: state.revision(),
        })
        .into_response(),
        Err(failure) => apply_failure_response(failure),
    }
}

fn apply_failure_response(failure: ApplyFailure) -> Response {
    match failure {
        ApplyFailure::Stale { current } => (
            StatusCode::CONFLICT,
            Json(json!({"error": "stale revision", "current": current})),
        )
            .into_response(),
        ApplyFailure::Edit(message) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "edit rejected", "reason": message})),
        )
            .into_response(),
        ApplyFailure::Invalid(message) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "invalid candidate", "reason": message})),
        )
            .into_response(),
        ApplyFailure::Confinement(message) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "path rejected", "reason": message})),
        )
            .into_response(),
        ApplyFailure::Io(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "io failure", "reason": message})),
        )
            .into_response(),
    }
}

/// Bind `app`'s router onto an already-bound listener and serve it until
/// Ctrl-C. A separate function from `run` so tests can inject an ephemeral
/// (`127.0.0.1:0`) listener instead of a fixed port; tests never trigger the
/// shutdown signal, so the spawned server simply runs until the test task
/// that owns it is dropped.
pub async fn serve_on(
    listener: tokio::net::TcpListener,
    app: App,
    ui: Option<Router>,
) -> std::io::Result<()> {
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    axum::serve(listener, router(app, ui))
        .with_graceful_shutdown(shutdown)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serve::guard::Token;

    struct TestServer {
        dir: tempfile::TempDir,
        base: String,
    }

    async fn spawn() -> TestServer {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("order.md"),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        )
        .unwrap();
        let state = ServeState::load(dir.path()).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let guard = Guard {
            token: Token::from_raw("thetoken"),
            port,
            bind_all: false,
        };
        let app = App {
            state: Arc::new(Mutex::new(state)),
            guard: Arc::new(guard),
        };
        tokio::spawn(serve_on(listener, app, None));
        // Give the listener a moment to start accepting.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        TestServer {
            dir,
            base: format!("http://127.0.0.1:{port}"),
        }
    }

    #[tokio::test]
    async fn reads_require_a_token() {
        let server = spawn().await;
        let resp = reqwest::get(format!("{}/api/bundle", server.base))
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn every_read_carries_the_revision() {
        let server = spawn().await;
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{}/api/bundle", server.base))
            .bearer_auth("thetoken")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        assert_eq!(resp.headers().get("X-Waml-Revision").unwrap(), "0");

        let model: serde_json::Value = client
            .get(format!("{}/api/model", server.base))
            .bearer_auth("thetoken")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(model["revision"], 0);

        let diagnostics: serde_json::Value = client
            .get(format!("{}/api/diagnostics", server.base))
            .bearer_auth("thetoken")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(diagnostics["revision"], 0);
    }

    #[tokio::test]
    async fn an_op_post_mutates_the_disk_and_answers_changed_files() {
        let server = spawn().await;
        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "revision": 0,
            "ops": [OpDto::AttrAdd {
                v: 1,
                node: "order".to_string(),
                name: "total".to_string(),
                ty: "Money".to_string(),
                mult: None,
                vis: None,
            }],
        });
        let resp = client
            .post(format!("{}/api/ops", server.base))
            .bearer_auth("thetoken")
            .header("X-Waml-Client", "1")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let json: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(json["revision"], 1);
        assert_eq!(json["changed"].as_array().unwrap().len(), 1);

        let written = std::fs::read_to_string(server.dir.path().join("order.md")).unwrap();
        assert!(written.contains("- total: Money"));
    }

    #[tokio::test]
    async fn a_documents_post_mutates_the_disk() {
        let server = spawn().await;
        let client = reqwest::Client::new();
        let baseline = std::fs::read_to_string(server.dir.path().join("order.md")).unwrap();
        let desired = "---\ntype: uml.Class\ntitle: Order\n---\n# Order\nText\n";
        let body = serde_json::json!({
            "revision": 0,
            "writes": [{
                "path": "order.md",
                "baseline": baseline,
                "desired": desired,
            }],
        });
        let resp = client
            .post(format!("{}/api/documents", server.base))
            .bearer_auth("thetoken")
            .header("X-Waml-Client", "1")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let written = std::fs::read_to_string(server.dir.path().join("order.md")).unwrap();
        assert_eq!(written, desired);
    }

    #[tokio::test]
    async fn a_rejected_batch_is_422_with_the_edit_error_shape() {
        let server = spawn().await;
        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "revision": 0,
            "ops": [OpDto::AttrAdd {
                v: 1,
                node: "missing-node".to_string(),
                name: "x".to_string(),
                ty: "String".to_string(),
                mult: None,
                vis: None,
            }],
        });
        let resp = client
            .post(format!("{}/api/ops", server.base))
            .bearer_auth("thetoken")
            .header("X-Waml-Client", "1")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn an_invalid_documents_candidate_is_422_with_diagnostics() {
        let server = spawn().await;
        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "revision": 0,
            "writes": [
                {"path": "a\\b.md", "baseline": null, "desired": "# One\n"},
                {"path": "a/b.md", "baseline": null, "desired": "# Two\n"},
            ],
        });
        let resp = client
            .post(format!("{}/api/documents", server.base))
            .bearer_auth("thetoken")
            .header("X-Waml-Client", "1")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn an_escaping_documents_path_is_422_not_500() {
        // A client-supplied path that escapes the bundle root is hostile or
        // mistaken client input, not a server failure.
        let server = spawn().await;
        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "revision": 0,
            "writes": [{"path": "../x.md", "baseline": null, "desired": "# Escape\n"}],
        });
        let resp = client
            .post(format!("{}/api/documents", server.base))
            .bearer_auth("thetoken")
            .header("X-Waml-Client", "1")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn a_stale_revision_is_409() {
        let server = spawn().await;
        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "revision": 5,
            "ops": [OpDto::AttrAdd {
                v: 1,
                node: "order".to_string(),
                name: "total".to_string(),
                ty: "Money".to_string(),
                mult: None,
                vis: None,
            }],
        });
        let resp = client
            .post(format!("{}/api/ops", server.base))
            .bearer_auth("thetoken")
            .header("X-Waml-Client", "1")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn a_mutating_post_without_the_client_header_is_403() {
        let server = spawn().await;
        let client = reqwest::Client::new();
        let body = serde_json::json!({"revision": 0, "ops": []});
        let resp = client
            .post(format!("{}/api/ops", server.base))
            .bearer_auth("thetoken")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_foreign_origin_is_403() {
        let server = spawn().await;
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{}/api/bundle", server.base))
            .bearer_auth("thetoken")
            .header("Origin", "https://evil.example")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn an_unauthenticated_post_is_401_even_with_a_malformed_body() {
        // The guard must run before any body work: an unauthenticated caller
        // never gets its JSON parsed, so the answer is 401, not 400/422.
        let server = spawn().await;
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/api/ops", server.base))
            .header("Content-Type", "application/json")
            .body("{not json")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn an_admitted_post_with_a_malformed_body_is_400() {
        let server = spawn().await;
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/api/documents", server.base))
            .bearer_auth("thetoken")
            .header("X-Waml-Client", "1")
            .header("Content-Type", "application/json")
            .body("{not json")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
    }

    #[test]
    fn a_poisoned_state_mutex_is_recovered_not_repropagated() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("order.md"),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        )
        .unwrap();
        let state = ServeState::load(dir.path()).unwrap();
        let app = App {
            state: Arc::new(Mutex::new(state)),
            guard: Arc::new(Guard {
                token: Token::from_raw("thetoken"),
                port: 0,
                bind_all: false,
            }),
        };
        // Poison the mutex the way a handler panic would.
        let poisoner = app.state.clone();
        std::thread::spawn(move || {
            let _guard = poisoner.lock().unwrap();
            panic!("poison the serve state mutex");
        })
        .join()
        .unwrap_err();
        assert!(app.state.lock().is_err(), "mutex should be poisoned");
        // A later request must still get a working state, not a panic.
        let guard = lock_state(&app);
        assert_eq!(guard.revision(), 0);
    }

    #[tokio::test]
    async fn an_unknown_api_path_is_404() {
        let server = spawn().await;
        let resp = reqwest::get(format!("{}/api/nope", server.base))
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::NOT_FOUND);
    }
}
