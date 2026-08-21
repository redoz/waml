//! End-to-end test: drive the compiled `waml lsp --stdio` server over stdio
//! with a small bundle and assert a `publishDiagnostics` notification arrives.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn frame(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

fn open_document_and_wait(text: &str, marker: &str) -> String {
    let exe = env!("CARGO_BIN_EXE_waml");
    let mut child = Command::new(exe)
        .args(["lsp", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn waml lsp");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let mut buf = [0u8; 8192];
        loop {
            match stdout.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    out.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if tx.send(out.clone()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let wait_for = |marker: &str| {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut out = String::new();
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_secs(20)) {
                Ok(latest) => {
                    out = latest;
                    if out.contains(marker) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        out
    };
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    stdin.write_all(frame(init).as_bytes()).unwrap();
    stdin.flush().unwrap();
    assert!(wait_for("\"id\":1").contains("capabilities"));
    let inited = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let open = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///C:/tmp/parser-platform.md",
                "languageId": "markdown",
                "version": 1,
                "text": text,
            }
        }
    })
    .to_string();
    for message in [inited, open.as_str()] {
        stdin.write_all(frame(message).as_bytes()).unwrap();
    }
    stdin.flush().unwrap();
    let out = wait_for(marker);
    let _ = child.kill();
    let _ = child.wait();
    drop(rx);
    let _ = reader.join();
    out
}

fn framed_json(output: &str) -> Vec<serde_json::Value> {
    let mut values = Vec::new();
    let mut remaining = output.as_bytes();
    while let Some(start) = remaining
        .windows(b"Content-Length: ".len())
        .position(|window| window == b"Content-Length: ")
    {
        remaining = &remaining[start + b"Content-Length: ".len()..];
        let Some(header_end) = remaining
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
        else {
            break;
        };
        let length = std::str::from_utf8(&remaining[..header_end])
            .unwrap()
            .trim()
            .parse::<usize>()
            .unwrap();
        remaining = &remaining[header_end + 4..];
        if remaining.len() < length {
            break;
        }
        values.push(serde_json::from_slice(&remaining[..length]).unwrap());
        remaining = &remaining[length..];
    }
    values
}

// Scenario: LSP-001
#[test]
fn publishes_diagnostics_for_unresolved_target_over_stdio() {
    let exe = env!("CARGO_BIN_EXE_waml");
    let mut child = Command::new(exe)
        .args(["lsp", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn waml lsp");

    let mut stdin = child.stdin.take().unwrap();

    // Read stdout on a worker thread so a blocking pipe read can never hang the
    // test; it streams the accumulated output back over a channel.
    let mut stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let mut buf = [0u8; 8192];
        loop {
            match stdout.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    out.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if tx.send(out.clone()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Wait for a marker to appear in the streamed output, bounded by a deadline.
    let wait_for = |rx: &mpsc::Receiver<String>, marker: &str| -> String {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut out = String::new();
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_secs(20)) {
                Ok(latest) => {
                    out = latest;
                    if out.contains(marker) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        out
    };

    // Per the LSP spec, wait for the `initialize` response before sending any
    // further messages — tower-lsp drops notifications received before it.
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    stdin.write_all(frame(init).as_bytes()).unwrap();
    stdin.flush().unwrap();
    let after_init = wait_for(&rx, "\"id\":1");
    assert!(
        after_init.contains("capabilities"),
        "no initialize response; got: {after_init}"
    );

    // A drive-lettered URI so `Url::to_file_path()` succeeds on Windows too
    // (a bare `file:///tmp/...` has no drive letter and fails to convert there).
    let inited = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///C:/tmp/order.md","languageId":"markdown","version":1,"text":"---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Ghost](./ghost.md)\n"}}}"#;
    for msg in [inited, open] {
        stdin.write_all(frame(msg).as_bytes()).unwrap();
    }
    stdin.flush().unwrap();

    let out = wait_for(&rx, "unresolved-target");
    let _ = child.kill();
    let _ = child.wait();
    drop(rx);
    let _ = reader.join();

    assert!(
        out.contains("publishDiagnostics"),
        "no publishDiagnostics seen; got: {out}"
    );
    assert!(
        out.contains("unresolved-target"),
        "expected unresolved-target; got: {out}"
    );
}

/// Regression guard: `Backend::did_open`/`did_change` re-run whole-document
/// `validate()` regardless of `ElementType`, so a behavioral-substrate
/// (flow/activity) diagnostic needs zero LSP-specific wiring to surface —
/// this mirrors `publishes_diagnostics_for_unresolved_target_over_stdio`
/// above but drives a `uml.ActivityDiagram` doc with a malformed flow bullet.
#[test]
fn publishes_diagnostics_for_a_malformed_flow_bullet_with_no_extra_wiring() {
    let exe = env!("CARGO_BIN_EXE_waml");
    let mut child = Command::new(exe)
        .args(["lsp", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn waml lsp");

    let mut stdin = child.stdin.take().unwrap();

    let mut stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let mut buf = [0u8; 8192];
        loop {
            match stdout.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    out.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if tx.send(out.clone()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let wait_for = |rx: &mpsc::Receiver<String>, marker: &str| -> String {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut out = String::new();
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_secs(20)) {
                Ok(latest) => {
                    out = latest;
                    if out.contains(marker) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        out
    };

    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    stdin.write_all(frame(init).as_bytes()).unwrap();
    stdin.flush().unwrap();
    let after_init = wait_for(&rx, "\"id\":1");
    assert!(
        after_init.contains("capabilities"),
        "no initialize response; got: {after_init}"
    );

    let inited = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///C:/tmp/flow.md","languageId":"markdown","version":1,"text":"---\ntype: uml.ActivityDiagram\ntitle: A\n---\n# A\n\n## Nodes\n\n### Ship\n- goes to Deliver\n"}}}"#;
    for msg in [inited, open] {
        stdin.write_all(frame(msg).as_bytes()).unwrap();
    }
    stdin.flush().unwrap();

    let out = wait_for(&rx, "malformed-flow-bullet");
    let _ = child.kill();
    let _ = child.wait();
    drop(rx);
    let _ = reader.join();

    assert!(
        out.contains("publishDiagnostics"),
        "no publishDiagnostics seen; got: {out}"
    );
    assert!(
        out.contains("malformed transition"),
        "expected parser-platform recovery message; got: {out}"
    );
}

#[test]
fn parser_platform_baseline_maps_astral_unicode_span_to_exact_utf16_range() {
    let fixture =
        include_str!("../../waml/tests/fixtures/parser-platform/malformed-crlf-unicode.md");
    assert!(
        fixture.contains("\r\n"),
        "malformed-crlf-unicode.md CRLF bytes"
    );
    let output = open_document_and_wait(fixture, "malformed-attribute");
    let notification = framed_json(&output)
        .into_iter()
        .find(|value| value["method"] == "textDocument/publishDiagnostics")
        .expect("malformed-crlf-unicode.md publishDiagnostics");
    let diagnostic = notification["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|diagnostic| diagnostic["code"] == "malformed-attribute")
        .expect("malformed-crlf-unicode.md malformed-attribute");
    assert_eq!(
        diagnostic["range"],
        serde_json::json!({
            "start": {"line": 7, "character": 0},
            "end": {"line": 7, "character": 31}
        }),
        "malformed-crlf-unicode.md UTF-16 diagnostic range"
    );
}

// Scenario: LSP-002
// Scenario: LSP-003
// Scenario: LSP-004
// Scenario: LSP-005
#[test]
fn snapshot_queries_are_advertised_unicode_exact_and_revision_current_over_stdio() {
    let exe = env!("CARGO_BIN_EXE_waml");
    let mut child = Command::new(exe)
        .args(["lsp", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn waml lsp");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let mut buf = [0u8; 8192];
        loop {
            match stdout.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    out.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if tx.send(out.clone()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let wait_for = |marker: &str| {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut out = String::new();
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_secs(20)) {
                Ok(latest) => {
                    out = latest;
                    if out.contains(marker) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        out
    };
    let send = |stdin: &mut std::process::ChildStdin, value: serde_json::Value| {
        stdin
            .write_all(frame(&value.to_string()).as_bytes())
            .unwrap();
        stdin.flush().unwrap();
    };

    send(
        &mut stdin,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"capabilities": {}}
        }),
    );
    let initialized = wait_for("\"id\":1");
    let initialized = framed_json(&initialized)
        .into_iter()
        .find(|value| value["id"] == 1)
        .unwrap();
    let capabilities = &initialized["result"]["capabilities"];
    assert_eq!(capabilities["documentSymbolProvider"], true);
    assert_eq!(capabilities["definitionProvider"], true);
    assert!(capabilities["documentLinkProvider"].is_object());
    assert_eq!(
        capabilities["semanticTokensProvider"]["legend"]["tokenTypes"]
            .as_array()
            .unwrap()
            .len(),
        11
    );
    assert_eq!(capabilities["textDocumentSync"]["change"], 1);

    send(
        &mut stdin,
        serde_json::json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );
    let next_uri = "file:///C:/tmp/next.md";
    // `ls_types::Uri::from_file_path` percent-encodes the drive colon (as VS
    // Code does), so server-emitted URIs carry `C%3A` even though the client
    // opened with a bare `C:`.
    let next_uri_emitted = "file:///C%3A/tmp/next.md";
    let order_uri = "file:///C:/tmp/order-query.md";
    for (uri, text) in [
        (next_uri, "---\ntype: uml.Class\n---\n# Next\n"),
        (
            order_uri,
            "---\ntype: uml.Class\n---\n# 😀 Order\n\nSee [Next](./next.md).\n\n## Attributes\n- count: Number {0..42}\n",
        ),
    ] {
        send(
            &mut stdin,
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {"textDocument": {
                    "uri": uri,
                    "languageId": "markdown",
                    "version": 1,
                    "text": text
                }}
            }),
        );
    }
    for request in [
        serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/documentSymbol",
            "params": {"textDocument": {"uri": order_uri}}
        }),
        serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "textDocument/documentLink",
            "params": {"textDocument": {"uri": order_uri}}
        }),
        serde_json::json!({
            "jsonrpc": "2.0", "id": 4, "method": "textDocument/definition",
            "params": {"textDocument": {"uri": order_uri}, "position": {"line": 5, "character": 6}}
        }),
        serde_json::json!({
            "jsonrpc": "2.0", "id": 5, "method": "textDocument/semanticTokens/full",
            "params": {"textDocument": {"uri": order_uri}}
        }),
    ] {
        send(&mut stdin, request);
    }
    let initial_output = wait_for("\"id\":5");
    let initial_messages = framed_json(&initial_output);
    let response = |id| {
        initial_messages
            .iter()
            .find(|value| value["id"] == id)
            .unwrap()
    };
    assert_eq!(response(2)["result"][0]["name"], "😀 Order");
    assert_eq!(response(3)["result"][0]["target"], next_uri_emitted);
    assert_eq!(response(4)["result"]["uri"], next_uri_emitted);
    let data = response(5)["result"]["data"].as_array().unwrap();
    let mut line = 0;
    let mut character = 0;
    let mut decoded = Vec::new();
    for token in data.chunks_exact(5) {
        let delta_line = token[0].as_u64().unwrap() as u32;
        line += delta_line;
        character = if delta_line == 0 {
            character + token[1].as_u64().unwrap() as u32
        } else {
            token[1].as_u64().unwrap() as u32
        };
        decoded.push((
            line,
            character,
            token[2].as_u64().unwrap() as u32,
            token[3].as_u64().unwrap() as u32,
        ));
    }
    assert!(
        decoded.contains(&(3, 2, 8, 1)),
        "astral heading: {decoded:?}"
    );

    send(
        &mut stdin,
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": {"uri": order_uri, "version": 2},
                "contentChanges": [{"text": "# Current 😀\n"}]
            }
        }),
    );
    let _ = wait_for("\"version\":2");
    send(
        &mut stdin,
        serde_json::json!({
            "jsonrpc": "2.0", "id": 6, "method": "textDocument/documentSymbol",
            "params": {"textDocument": {"uri": order_uri}}
        }),
    );
    let final_output = wait_for("\"id\":6");
    let final_response = framed_json(&final_output)
        .into_iter()
        .find(|value| value["id"] == 6)
        .unwrap();
    assert_eq!(final_response["result"][0]["name"], "Current 😀");

    let _ = child.kill();
    let _ = child.wait();
    drop(rx);
    let _ = reader.join();
}

// Scenario: LSP-006
#[test]
fn completion_is_advertised_and_returns_items_over_stdio() {
    let exe = env!("CARGO_BIN_EXE_waml");
    let mut child = Command::new(exe)
        .args(["lsp", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn waml lsp");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let mut buf = [0u8; 8192];
        loop {
            match stdout.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    out.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if tx.send(out.clone()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let wait_for = |marker: &str| {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut out = String::new();
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_secs(20)) {
                Ok(latest) => {
                    out = latest;
                    if out.contains(marker) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        out
    };
    let send = |stdin: &mut std::process::ChildStdin, value: serde_json::Value| {
        stdin
            .write_all(frame(&value.to_string()).as_bytes())
            .unwrap();
        stdin.flush().unwrap();
    };

    send(
        &mut stdin,
        serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"capabilities": {}}
        }),
    );
    let initialized = framed_json(&wait_for("\"id\":1"))
        .into_iter()
        .find(|value| value["id"] == 1)
        .unwrap();
    assert!(
        initialized["result"]["capabilities"]["completionProvider"].is_object(),
        "{initialized}"
    );

    send(
        &mut stdin,
        serde_json::json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );
    let uri = "file:///C:/tmp/completion-seq.md";
    // Line 9 (0-based) is "- A ", so character 4 is the empty verb slot.
    let text = "---\ntype: uml.SequenceDiagram\ntitle: S\n---\n# S\n\n## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- A \n";
    send(
        &mut stdin,
        serde_json::json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": {"textDocument": {
                "uri": uri, "languageId": "markdown", "version": 1, "text": text
            }}
        }),
    );
    let line = text.lines().count() as u32 - 1;
    send(
        &mut stdin,
        serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/completion",
            "params": {"textDocument": {"uri": uri}, "position": {"line": line, "character": 4}}
        }),
    );
    let response = framed_json(&wait_for("\"id\":2"))
        .into_iter()
        .find(|value| value["id"] == 2)
        .unwrap();
    let labels = response["result"]
        .as_array()
        .unwrap_or_else(|| panic!("expected an array of items, got {response}"))
        .iter()
        .map(|item| item["label"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"calls".to_string()), "{labels:?}");

    let _ = child.kill();
    let _ = child.wait();
    drop(rx);
    let _ = reader.join();
}

// ---------------------------------------------------------------------------
// Document-ownership scenarios (A20).
//
// These drive a real workspace root on disk, because the whole question is what
// the server believes about disk versus what the client last sent it. The
// scenarios above open `file:///C:/tmp/...` paths that never exist, so they
// exercise only the in-memory half of the model.
// ---------------------------------------------------------------------------

/// A live `waml lsp --stdio` child plus the plumbing every scenario needs:
/// framed writes, a stdout reader on its own thread so a blocking pipe read can
/// never hang the test, and marker waits over the accumulated output.
struct LspSession {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    rx: Option<mpsc::Receiver<String>>,
    reader: Option<std::thread::JoinHandle<()>>,
}

impl LspSession {
    fn spawn() -> Self {
        let exe = env!("CARGO_BIN_EXE_waml");
        let mut child = Command::new(exe)
            .args(["lsp", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn waml lsp");
        let stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel::<String>();
        let reader = std::thread::spawn(move || {
            let mut out = String::new();
            let mut buf = [0u8; 8192];
            loop {
                match stdout.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        out.push_str(&String::from_utf8_lossy(&buf[..n]));
                        if tx.send(out.clone()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Self {
            child,
            stdin,
            rx: Some(rx),
            reader: Some(reader),
        }
    }

    fn send(&mut self, value: serde_json::Value) {
        self.stdin
            .write_all(frame(&value.to_string()).as_bytes())
            .unwrap();
        self.stdin.flush().unwrap();
    }

    fn notify(&mut self, method: &str, params: serde_json::Value) {
        self.send(serde_json::json!({
            "jsonrpc": "2.0", "method": method, "params": params
        }));
    }

    /// Accumulated stdout once `marker` shows up, or once the deadline expires.
    /// Returning rather than panicking on timeout lets the caller assert on the
    /// content it actually got, which is a far better failure message.
    fn wait_for(&self, marker: &str) -> String {
        self.wait_for_nth(marker, 1)
    }

    /// The same, for the `count`-th occurrence. Stdout accumulates for the
    /// whole session, so a marker that already fired once cannot gate the next
    /// round trip; counting can.
    fn wait_for_nth(&self, marker: &str, count: usize) -> String {
        let rx = self.rx.as_ref().unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut out = String::new();
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_secs(20)) {
                Ok(latest) => {
                    out = latest;
                    if out.matches(marker).count() >= count {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        out
    }

    /// Send a request, wait for its response, and return the response object.
    fn request(&mut self, id: i64, method: &str, params: serde_json::Value) -> serde_json::Value {
        self.send(serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params
        }));
        let out = self.wait_for(&format!("\"id\":{id}"));
        framed_json(&out)
            .into_iter()
            .find(|value| value["id"] == id)
            .unwrap_or_else(|| panic!("no response for {method} (id {id}); got: {out}"))
    }
}

impl Drop for LspSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        drop(self.rx.take());
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

/// `file://` URI for a real local path, percent-encoded enough to survive a
/// temp directory that contains spaces or non-ASCII.
fn file_uri(path: &std::path::Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut uri = String::from("file://");
    if !text.starts_with('/') {
        uri.push('/');
    }
    for ch in text.chars() {
        match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '.' | '_' | '~' | '/' | ':' => uri.push(ch),
            _ => {
                let mut buf = [0u8; 4];
                for byte in ch.encode_utf8(&mut buf).as_bytes() {
                    uri.push_str(&format!("%{byte:02X}"));
                }
            }
        }
    }
    uri
}

/// Initialize against a real workspace root, advertising the watched-files
/// dynamic registration a modern client offers, and answering the
/// `client/registerCapability` request the server sends back — as a real client
/// does, and must, since the server waits for it before publishing.
///
/// Returns the `initialize` response and the registration request.
fn initialize_at(
    session: &mut LspSession,
    root: &std::path::Path,
) -> (serde_json::Value, serde_json::Value) {
    let initialized = session.request(
        1,
        "initialize",
        serde_json::json!({
            "capabilities": {
                "workspace": {"didChangeWatchedFiles": {"dynamicRegistration": true}}
            },
            "workspaceFolders": [{"uri": file_uri(root), "name": "fixture"}]
        }),
    );
    session.notify("initialized", serde_json::json!({}));
    let out = session.wait_for("client/registerCapability");
    let registration = framed_json(&out)
        .into_iter()
        .find(|value| value["method"] == "client/registerCapability")
        .unwrap_or_else(|| panic!("server never registered file watchers; got: {out}"));
    session.send(serde_json::json!({
        "jsonrpc": "2.0", "id": registration["id"], "result": serde_json::Value::Null
    }));
    (initialized, registration)
}

fn heading_of(symbols: &serde_json::Value) -> String {
    symbols["result"][0]["name"]
        .as_str()
        .unwrap_or("<none>")
        .to_string()
}

/// The diagnostics from the most recent `publishDiagnostics` whose URI ends in
/// `suffix`. Matched by suffix because the server percent-encodes the drive
/// colon on its way out (`C%3A`) and the test writes it plainly.
fn last_diagnostics(output: &str, suffix: &str) -> Option<serde_json::Value> {
    framed_json(output)
        .into_iter()
        .filter(|value| value["method"] == "textDocument/publishDiagnostics")
        .filter(|value| {
            value["params"]["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with(suffix))
        })
        .map(|value| value["params"]["diagnostics"].clone())
        .next_back()
}

/// A20: after `didClose` the disk is authoritative again, so the server must
/// re-read it. The old server restored the bytes it ingested at `initialize`,
/// which after any external write matched neither the disk nor anything the
/// user ever typed.
#[test]
fn close_rereads_disk_rather_than_the_startup_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let order = root.path().join("order.md");
    std::fs::write(
        &order,
        "---\ntype: uml.Class\ntitle: Order\n---\n# AtStartup\n",
    )
    .unwrap();

    let mut session = LspSession::spawn();
    let (initialized, _) = initialize_at(&mut session, root.path());
    assert_eq!(
        initialized["result"]["capabilities"]["textDocumentSync"]["save"]["includeText"], false,
        "the server must ask for save notifications without the text"
    );
    let uri = file_uri(&order);

    session.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {
            "uri": uri, "languageId": "markdown", "version": 1,
            "text": "---\ntype: uml.Class\ntitle: Order\n---\n# WhileOpen\n"
        }}),
    );
    let open_symbols = session.request(
        2,
        "textDocument/documentSymbol",
        serde_json::json!({"textDocument": {"uri": uri}}),
    );
    assert_eq!(
        heading_of(&open_symbols),
        "WhileOpen",
        "the client owns an open document"
    );

    // Somebody else writes the file: a git checkout, a formatter, another
    // editor. The unresolved link is the marker that proves the new bytes,
    // and only the new bytes, reached the analysis.
    std::fs::write(
        &order,
        "---\ntype: uml.Class\ntitle: Order\n---\n# AfterCheckout\n\n## Relationships\n- depends [Ghost](./nowhere.md)\n",
    )
    .unwrap();

    session.notify(
        "textDocument/didClose",
        serde_json::json!({"textDocument": {"uri": uri}}),
    );
    // Sequencing barrier: the close has landed once its re-analysis publishes.
    // Not an assertion yet -- on a stale server this simply times out, and the
    // symbol query below is the failure worth reading.
    let after_close = session.wait_for("unresolved-target");

    let closed_symbols = session.request(
        3,
        "textDocument/documentSymbol",
        serde_json::json!({"textDocument": {"uri": uri}}),
    );
    assert_eq!(
        heading_of(&closed_symbols),
        "AfterCheckout",
        "a closed document must read as the disk does, not as the startup snapshot"
    );
    assert!(
        after_close.contains("unresolved-target"),
        "the re-read bytes did not reach the analysis; got: {after_close}"
    );
}

/// A20: the server registers for `workspace/didChangeWatchedFiles` and acts on
/// it, so a file the client never opened stops being frozen at whatever it said
/// when the server booted.
#[test]
fn watched_file_events_refresh_documents_the_client_never_opened() {
    let root = tempfile::tempdir().unwrap();
    let order = root.path().join("order.md");
    // `zz-` so this sorts after `order.md`: the server publishes the bundle in
    // path order, so a marker on the last document proves every earlier
    // document's publish is already in the buffer being asserted on.
    let linked = root.path().join("zz-linked.md");
    std::fs::write(
        &order,
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Linked](./zz-linked.md)\n",
    )
    .unwrap();

    let mut session = LspSession::spawn();
    let (_, registration) = initialize_at(&mut session, root.path());
    let watcher = &registration["params"]["registrations"][0];
    assert_eq!(watcher["method"], "workspace/didChangeWatchedFiles");
    assert_eq!(
        watcher["registerOptions"]["watchers"][0]["globPattern"], "**/*.md",
        "the registered glob must cover everything the startup walk ingests"
    );

    let before = session.wait_for("unresolved-target");
    assert!(
        before.contains("unresolved-target"),
        "expected a dangling link before the target exists; got: {before}"
    );

    // Created on disk by something that is not the editor, and never opened.
    // The malformed bullet is a marker only this file can produce.
    std::fs::write(
        &linked,
        "---\ntype: uml.ActivityDiagram\ntitle: Linked\n---\n# Linked\n\n## Nodes\n\n### Ship\n- goes to Deliver\n",
    )
    .unwrap();
    session.notify(
        "workspace/didChangeWatchedFiles",
        serde_json::json!({"changes": [{"uri": file_uri(&linked), "type": 1}]}),
    );
    let after_create = session.wait_for("malformed-flow-bullet");
    assert!(
        after_create.contains("malformed-flow-bullet"),
        "a watched create never reached the analysis; got: {after_create}"
    );
    assert_eq!(
        last_diagnostics(&after_create, "/order.md"),
        Some(serde_json::json!([])),
        "the link should resolve once its target exists"
    );

    // Deleted again: the document leaves the bundle, its diagnostics are
    // retracted rather than left standing, and the dangling link comes back.
    std::fs::remove_file(&linked).unwrap();
    session.notify(
        "workspace/didChangeWatchedFiles",
        serde_json::json!({"changes": [{"uri": file_uri(&linked), "type": 3}]}),
    );
    let after_delete = session.wait_for_nth("unresolved-target", 2);
    assert_eq!(
        last_diagnostics(&after_delete, "/order.md")
            .map(|diagnostics| diagnostics.as_array().unwrap().len()),
        Some(1),
        "deleting the target should dangle the link again; got: {after_delete}"
    );
    assert_eq!(
        last_diagnostics(&after_delete, "/zz-linked.md"),
        Some(serde_json::json!([])),
        "a deleted document must have its diagnostics retracted, not left standing"
    );
}
