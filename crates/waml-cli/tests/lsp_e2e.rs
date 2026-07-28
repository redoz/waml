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
/// above but drives a `uml.Activity` doc with a malformed flow bullet.
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
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///C:/tmp/flow.md","languageId":"markdown","version":1,"text":"---\ntype: uml.Activity\ntitle: A\n---\n# A\n\n## Nodes\n\n### Ship\n- goes to Deliver\n"}}}"#;
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
        out.contains("unrecognized flow bullet"),
        "expected 'unrecognized flow bullet' message; got: {out}"
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
