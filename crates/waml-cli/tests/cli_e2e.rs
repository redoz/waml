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
