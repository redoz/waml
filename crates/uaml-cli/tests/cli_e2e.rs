use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_uaml"))
}

fn tmp() -> std::path::PathBuf {
    // `line!()` alone would collide: every call site is *inside this function*,
    // so it always expands to the same line. Mix in a per-process counter so
    // concurrently-running tests each get their own directory.
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("uaml_e2e_{}_{}_{n}", std::process::id(), line!()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn attr_add_writes_the_file() {
    let d = tmp();
    std::fs::write(d.join("order.md"), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n").unwrap();
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
    std::fs::write(d.join("order.md"), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n").unwrap();
    let out = bin()
        .args(["attr", "add", "order", "total", "Money", "--emit", "--dir"])
        .arg(&d)
        .output()
        .unwrap();
    assert!(out.status.success());
    let line = String::from_utf8(out.stdout).unwrap();
    assert!(line.contains("\"op\":\"attr.add\""));
    // file untouched
    assert!(!std::fs::read_to_string(d.join("order.md")).unwrap().contains("total"));
}

#[test]
fn duplicate_attr_exits_1() {
    let d = tmp();
    std::fs::write(
        d.join("order.md"),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n",
    )
    .unwrap();
    let status = bin().args(["attr", "add", "order", "id", "X", "--dir"]).arg(&d).status().unwrap();
    assert_eq!(status.code(), Some(1));
}
