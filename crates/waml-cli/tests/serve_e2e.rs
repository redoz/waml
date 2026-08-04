use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_waml"))
}

fn tmp() -> std::path::PathBuf {
    // Same collision-proofing as `cli_e2e.rs`'s `tmp()`: mix pid, a monotonic
    // clock reading, and a per-process counter so concurrently-running tests
    // never collide, even across recycled pids.
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static N: AtomicUsize = AtomicUsize::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the unix epoch")
        .as_nanos();
    let d = std::env::temp_dir().join(format!(
        "waml_serve_e2e_{}_{}_{stamp}_{n}",
        std::process::id(),
        line!()
    ));
    if d.exists() {
        std::fs::remove_dir_all(&d).unwrap();
    }
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Parse the `waml serve  http://127.0.0.1:<port>/?api=/api&token=<token>   (serving <dir>)`
/// line for the URL, splitting out the token.
fn parse_boot_line(line: &str) -> (String, String) {
    let url = line
        .split_whitespace()
        .find(|tok| tok.starts_with("http://"))
        .unwrap_or_else(|| panic!("no URL in boot line: {line:?}"))
        .to_string();
    let token = url
        .split("token=")
        .nth(1)
        .unwrap_or_else(|| panic!("no token in boot line: {line:?}"))
        .to_string();
    (url, token)
}

#[test]
fn serve_prints_a_bootable_url_and_serves_the_directory() {
    let d = tmp();
    std::fs::write(
        d.join("order.md"),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
    )
    .unwrap();

    let mut child = bin()
        .args(["serve"])
        .arg(&d)
        .args(["--port", "0", "--no-open", "--api-only"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.starts_with("waml serve"),
        "unexpected first stdout line: {line:?}"
    );
    let (url, token) = parse_boot_line(&line);

    // Swap the boot URL's `/?api=...` path for `/api/model` to hit the API
    // directly, reusing the printed token.
    let base = url.split("/?api=").next().unwrap();
    let model_url = format!("{base}/api/model");

    let result = std::panic::catch_unwind(|| {
        let client = reqwest::blocking::Client::new();
        let resp = client
            .get(&model_url)
            .bearer_auth(&token)
            .send()
            .expect("request to the running server");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body: serde_json::Value = resp.json().unwrap();
        let keys: Vec<String> = body["model"]["nodes"]
            .as_array()
            .expect("model.nodes is an array")
            .iter()
            .filter_map(|node| node["key"].as_str().map(str::to_string))
            .collect();
        assert!(
            keys.iter().any(|key| key == "order"),
            "expected the fixture's `order` node in {keys:?}"
        );
    });

    let _ = child.kill();
    let _ = child.wait();
    result.expect("verification against the running server should not panic");
}
