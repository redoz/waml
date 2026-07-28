use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_waml"))
}

fn tmp() -> std::path::PathBuf {
    // `line!()` alone would collide: every call site is *inside this function*,
    // so it always expands to the same line. Mix in a per-process counter so
    // concurrently-running tests each get their own directory.
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("waml_e2e_{}_{}_{n}", std::process::id(), line!()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn attr_add_writes_the_file() {
    let d = tmp();
    std::fs::write(
        d.join("order.md"),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
    )
    .unwrap();
    let status = bin()
        .args(["attr", "add", "order", "total", "Money", "--dir"])
        .arg(&d)
        .status()
        .unwrap();
    assert!(status.success());
    let text = std::fs::read_to_string(d.join("order.md")).unwrap();
    assert!(text.contains("- total: Money"));
}

#[test]
fn emit_prints_an_op_line_without_writing() {
    let d = tmp();
    std::fs::write(
        d.join("order.md"),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
    )
    .unwrap();
    let out = bin()
        .args(["attr", "add", "order", "total", "Money", "--emit", "--dir"])
        .arg(&d)
        .output()
        .unwrap();
    assert!(out.status.success());
    let line = String::from_utf8(out.stdout).unwrap();
    assert!(line.contains("\"op\":\"attr.add\""));
    // file untouched
    assert!(!std::fs::read_to_string(d.join("order.md"))
        .unwrap()
        .contains("total"));
}

#[test]
fn duplicate_attr_exits_1() {
    let d = tmp();
    std::fs::write(
        d.join("order.md"),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n",
    )
    .unwrap();
    let status = bin()
        .args(["attr", "add", "order", "id", "X", "--dir"])
        .arg(&d)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(1));
}

#[test]
fn apply_rejects_traversal_and_duplicate_normalized_import_paths() {
    for (name, docs) in [
        ("traversal", r##"[["../escape.md","# Escape\n"]]"##),
        (
            "duplicate",
            r##"[["a\\b.md","# One\n"],["a/b.md","# Two\n"]]"##,
        ),
    ] {
        let d = tmp();
        std::fs::write(d.join("base.md"), "# Base\n").unwrap();
        let ops = d.join(format!("{name}.ndjson"));
        std::fs::write(
            &ops,
            format!(r#"{{"op":"pkg.insert","parent_path":"","name":"imported","docs":{docs}}}"#),
        )
        .unwrap();
        let output = bin()
            .arg("apply")
            .arg(&ops)
            .arg("--dir")
            .arg(&d)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{name}");
        assert!(!d.join("imported").exists(), "{name}");
    }
}

#[test]
fn apply_mixed_okf_and_uml_batch_succeeds() {
    let d = tmp();
    std::fs::create_dir_all(d.join("sales")).unwrap();
    std::fs::write(
        d.join("sales/order.md"),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
    )
    .unwrap();
    let ops = d.join("mixed.ndjson");
    std::fs::write(
        &ops,
        "{\"op\":\"pkg.retitle\",\"path\":\"sales\",\"title\":\"Commerce\"}\n\
         {\"op\":\"node.set\",\"slug\":\"sales/order\",\"title\":\"Purchase\"}\n",
    )
    .unwrap();
    let output = bin()
        .arg("apply")
        .arg(&ops)
        .arg("--dir")
        .arg(&d)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(std::fs::read_to_string(d.join("sales/index.md"))
        .unwrap()
        .contains("Commerce"));
    assert!(std::fs::read_to_string(d.join("sales/order.md"))
        .unwrap()
        .contains("title: Purchase"));
}

#[test]
fn apply_late_collision_rolls_back_earlier_okf_change() {
    let d = tmp();
    std::fs::create_dir_all(d.join("sales")).unwrap();
    std::fs::create_dir_all(d.join("archive")).unwrap();
    std::fs::write(
        d.join("sales/order.md"),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
    )
    .unwrap();
    std::fs::write(d.join("archive/order.md"), "# Existing\n").unwrap();
    let before = std::fs::read_to_string(d.join("sales/order.md")).unwrap();
    let ops = d.join("rollback.ndjson");
    std::fs::write(
        &ops,
        "{\"op\":\"pkg.retitle\",\"path\":\"sales\",\"title\":\"Changed\"}\n\
         {\"op\":\"pkg.move\",\"slug\":\"sales/order\",\"to_dir\":\"archive\"}\n",
    )
    .unwrap();
    let output = bin()
        .arg("apply")
        .arg(&ops)
        .arg("--dir")
        .arg(&d)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        std::fs::read_to_string(d.join("sales/order.md")).unwrap(),
        before
    );
    assert!(!d.join("sales/index.md").exists());
}

#[test]
fn check_accepts_generic_okf_without_uml_diagnostics() {
    let d = tmp();
    let generic = d.join("notes.md");
    std::fs::write(
        &generic,
        "---\ntype: notes.Decision\ntitle: Keep Authored Markdown\n---\n# Keep Authored Markdown\n\nArbitrary prose.\n",
    )
    .unwrap();

    let output = bin()
        .arg("check")
        .arg(&generic)
        .args(["--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "[]");
}

#[test]
fn check_reports_malformed_claimed_uml_from_parser_analysis() {
    let d = tmp();
    let malformed = d.join("order.md");
    std::fs::write(
        &malformed,
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id:\n",
    )
    .unwrap();

    let output = bin()
        .arg("check")
        .arg(&malformed)
        .args(["--format", "json"])
        .output()
        .unwrap();
    let diagnostics: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(diagnostics.as_array().is_some_and(|items| {
        items.iter().any(|diagnostic| {
            diagnostic["severity"] == "error"
                && diagnostic["file"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("order.md"))
                && diagnostic["span"].is_array()
        })
    }));
}

#[test]
fn fmt_stdout_preserves_generic_okf_exactly() {
    let d = tmp();
    let generic = d.join("notes.md");
    let authored =
        "---\ntype: notes.Decision\ntitle: Keep Me\n---\n# Keep Me\n\n  Deliberate spacing.\n";
    std::fs::write(&generic, authored).unwrap();

    let output = bin()
        .arg("fmt")
        .arg(&generic)
        .arg("--stdout")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), authored);
    assert_eq!(std::fs::read_to_string(generic).unwrap(), authored);
}

#[test]
fn fmt_canonical_output_is_idempotent() {
    let d = tmp();
    let class = d.join("order.md");
    std::fs::write(
        &class,
        "---\ntype: uml.Class\ntitle:   Order\n---\n# Order\n\n## Attributes\n\n-  id :  OrderId [1]\n",
    )
    .unwrap();

    let first = bin().arg("fmt").arg(&class).output().unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let once = std::fs::read_to_string(&class).unwrap();
    let second = bin().arg("fmt").arg(&class).output().unwrap();
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );

    assert_eq!(std::fs::read_to_string(class).unwrap(), once);
    assert!(second.stdout.is_empty());
}

#[test]
fn apply_late_multi_file_failure_writes_nothing() {
    let d = tmp();
    std::fs::write(
        d.join("alpha.md"),
        "---\ntype: uml.Class\ntitle: Alpha\n---\n# Alpha\n",
    )
    .unwrap();
    std::fs::write(
        d.join("beta.md"),
        "---\ntype: uml.Class\ntitle: Beta\n---\n# Beta\n",
    )
    .unwrap();
    let alpha_before = std::fs::read_to_string(d.join("alpha.md")).unwrap();
    let beta_before = std::fs::read_to_string(d.join("beta.md")).unwrap();
    let ops = d.join("late-failure.ndjson");
    std::fs::write(
        &ops,
        "{\"op\":\"node.set\",\"slug\":\"alpha\",\"title\":\"Changed Alpha\"}\n\
         {\"op\":\"node.rename\",\"from\":\"beta\",\"to\":\"alpha\"}\n",
    )
    .unwrap();

    let output = bin()
        .arg("apply")
        .arg(&ops)
        .arg("--dir")
        .arg(&d)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        std::fs::read_to_string(d.join("alpha.md")).unwrap(),
        alpha_before
    );
    assert_eq!(
        std::fs::read_to_string(d.join("beta.md")).unwrap(),
        beta_before
    );
}

#[test]
fn show_json_and_refs_share_prepared_referrer_results() {
    let d = tmp();
    std::fs::write(
        d.join("order.md"),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
    )
    .unwrap();
    std::fs::write(
        d.join("customer.md"),
        "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n\n## Attributes\n- order: [Order](./order.md)\n",
    )
    .unwrap();

    let shown = bin()
        .args(["show", "order", "--format", "json", "--dir"])
        .arg(&d)
        .output()
        .unwrap();
    let refs = bin()
        .args(["refs", "order", "--format", "json", "--dir"])
        .arg(&d)
        .output()
        .unwrap();
    assert!(shown.status.success());
    assert!(refs.status.success());
    let shown: serde_json::Value = serde_json::from_slice(&shown.stdout).unwrap();
    let refs: serde_json::Value = serde_json::from_slice(&refs.stdout).unwrap();

    assert_eq!(shown["referrers"], refs);
    assert_eq!(refs, serde_json::json!(["customer"]));
}

fn malformed_class() -> &'static str {
    "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id:\n"
}

fn diagnostic_file(output: &std::process::Output) -> String {
    let diagnostics: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    diagnostics[0]["file"].as_str().unwrap().to_owned()
}

#[test]
fn diagnostic_paths_preserve_absolute_and_relative_file_spellings() {
    let d = tmp();
    let absolute = d.join("absolute.md");
    std::fs::write(&absolute, malformed_class()).unwrap();
    std::fs::write(d.join("relative.md"), malformed_class()).unwrap();

    let absolute_output = bin()
        .arg("check")
        .arg(&absolute)
        .args(["--format", "json"])
        .output()
        .unwrap();
    let relative_output = {
        let mut command = bin();
        command
            .current_dir(&d)
            .args(["check", "relative.md", "--format", "json"]);
        command.output().unwrap()
    };

    assert_eq!(
        diagnostic_file(&absolute_output),
        absolute.to_string_lossy()
    );
    assert_eq!(diagnostic_file(&relative_output), "relative.md");
}

#[test]
fn diagnostic_paths_preserve_typed_directory_prefix() {
    let d = tmp();
    std::fs::create_dir(d.join("typed-bundle")).unwrap();
    std::fs::write(d.join("typed-bundle/order.md"), malformed_class()).unwrap();
    let mut command = bin();
    command
        .current_dir(&d)
        .args(["check", "typed-bundle", "--format", "json"]);

    let output = command.output().unwrap();

    assert_eq!(
        diagnostic_file(&output),
        std::path::Path::new("typed-bundle")
            .join("order.md")
            .to_string_lossy()
    );
}

#[test]
fn diagnostic_paths_render_stdin_exactly() {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = bin()
        .args(["check", "--stdin", "--format", "json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(malformed_class().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert_eq!(diagnostic_file(&output), "stdin");
}
