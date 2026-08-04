//! UI routes: the embedded web editor, served same-origin from `waml serve`.
//!
//! Assets are stored brotli-compressed in the binary and are served exactly
//! as stored -- `Content-Encoding: br`, never recompressed or decompressed
//! per request. The generated boot config (`waml-boot.txt`) is the one
//! exception: it is a few bytes built at startup and served identity-encoded.
//! These routes do not require the API token: they are static bytes, same
//! origin as the API they point at, and the token in the printed URL is for
//! the editor to present to `/api`, not a UI access control (see the spec's
//! Boot section).

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::site::{self, SiteError, SiteSource, BOOT_CONFIG_FILE};
use crate::web_artifact::EmbeddedAsset;

#[derive(Clone, Copy)]
struct Asset {
    brotli: &'static [u8],
    content_type: &'static str,
}

/// The embedded artifact, indexed by path and ready to serve.
#[derive(Clone)]
pub struct UiAssets {
    files: Arc<BTreeMap<String, Asset>>,
    boot_config: Arc<Vec<u8>>,
}

/// Validate the embedded manifest and index it for serving. Called once at
/// startup, not per request.
pub fn build(artifact: &'static [EmbeddedAsset]) -> Result<UiAssets, SiteError> {
    site::validate_manifest(artifact)?;

    let mut files = BTreeMap::new();
    for asset in artifact {
        files.insert(
            asset.path.to_string(),
            Asset {
                brotli: asset.brotli,
                content_type: content_type_of(asset.path),
            },
        );
    }
    let boot_config = site::boot_query(&SiteSource::Api).into_bytes();

    Ok(UiAssets {
        files: Arc::new(files),
        boot_config: Arc::new(boot_config),
    })
}

/// The UI router: `/` and every embedded asset path, plus the generated boot
/// config. Anything else is 404, never the shell -- a typo in an asset path
/// must not silently fall back to a blank editor.
pub fn router(assets: UiAssets) -> Router {
    Router::new().fallback(get(serve_path)).with_state(assets)
}

fn content_type_of(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
        Some("ico") => "image/x-icon",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

fn accepts_brotli(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|enc| enc.trim().starts_with("br")))
}

async fn serve_path(
    State(assets): State<UiAssets>,
    headers: axum::http::HeaderMap,
    uri: Uri,
) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if path == BOOT_CONFIG_FILE {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            assets.boot_config.as_slice().to_vec(),
        )
            .into_response();
    }

    let Some(asset) = assets.files.get(path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !accepts_brotli(&headers) {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    }
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, asset.content_type),
            (header::CONTENT_ENCODING, "br"),
        ],
        asset.brotli,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brotli_stored(bytes: &[u8]) -> &'static [u8] {
        let mut out = Vec::new();
        let mut input = bytes;
        brotli::BrotliCompress(
            &mut input,
            &mut out,
            &brotli::enc::BrotliEncoderParams::default(),
        )
        .unwrap();
        Box::leak(out.into_boxed_slice())
    }

    fn artifact(parts: &[(&'static str, &str)]) -> &'static [EmbeddedAsset] {
        let assets: Vec<EmbeddedAsset> = parts
            .iter()
            .map(|(path, body)| EmbeddedAsset {
                path,
                brotli: brotli_stored(body.as_bytes()),
            })
            .collect();
        Box::leak(assets.into_boxed_slice())
    }

    /// Spawn a bare UI router on an ephemeral port and return its base URL.
    async fn spawn(assets: UiAssets) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            axum::serve(listener, router(assets)).await.unwrap();
        });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        base
    }

    #[tokio::test]
    async fn the_shell_is_served_brotli_encoded_from_stored_bytes() {
        let raw = "<!doctype html><title>waml</title>";
        let artifact = artifact(&[("index.html", raw)]);
        let assets = build(artifact).unwrap();
        let stored_brotli = assets.files["index.html"].brotli.to_vec();
        let base = spawn(assets).await;

        let resp = reqwest::Client::new()
            .get(&base)
            .header(reqwest::header::ACCEPT_ENCODING, "br")
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        assert_eq!(resp.headers().get("content-encoding").unwrap(), "br");
        let body = resp.bytes().await.unwrap();
        assert_eq!(&body[..], &stored_brotli[..]);
    }

    #[tokio::test]
    async fn a_client_without_brotli_gets_406() {
        let artifact = artifact(&[("index.html", "<!doctype html>")]);
        let base = spawn(build(artifact).unwrap()).await;

        let resp = reqwest::get(&base).await.unwrap();

        assert_eq!(resp.status(), reqwest::StatusCode::NOT_ACCEPTABLE);
    }

    #[tokio::test]
    async fn an_unknown_asset_is_404_not_the_shell() {
        let artifact = artifact(&[("index.html", "<!doctype html>")]);
        let base = spawn(build(artifact).unwrap()).await;

        let resp = reqwest::Client::new()
            .get(format!("{base}/nope.js"))
            .header(reqwest::header::ACCEPT_ENCODING, "br")
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), reqwest::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn the_boot_config_names_the_api_and_carries_no_token() {
        let artifact = artifact(&[("index.html", "<!doctype html>")]);
        let base = spawn(build(artifact).unwrap()).await;

        let resp = reqwest::get(format!("{base}/{BOOT_CONFIG_FILE}"))
            .await
            .unwrap();

        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body = resp.bytes().await.unwrap();
        assert_eq!(&body[..], b"?api=/api");
    }

    #[tokio::test]
    async fn ui_routes_do_not_require_the_token() {
        let artifact = artifact(&[("index.html", "<!doctype html>")]);
        let base = spawn(build(artifact).unwrap()).await;

        let resp = reqwest::Client::new()
            .get(&base)
            .header(reqwest::header::ACCEPT_ENCODING, "br")
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), reqwest::StatusCode::OK);
    }
}
