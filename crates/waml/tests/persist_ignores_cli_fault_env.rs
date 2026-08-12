//! The `WAML_CLI_TEST_FAIL_*` fault-injection backdoor that `cli_e2e.rs`
//! drives belongs to the `waml` CLI binary (`waml-cli`'s `io::write_back`),
//! not to this library.
//!
//! `host::persist::write_back` is *also* the native editor's save path and
//! `serve`'s write path, and both are routinely built with debug assertions
//! on (`run.ps1`), so a library-side env-var backdoor would let a variable
//! exported for a CLI test silently sabotage an editor save. The library takes
//! its fault injection as an argument (`write_back_injecting_ops`); it never
//! reads the process environment.
//!
//! This lives in its own integration-test binary holding exactly one test:
//! mutating the environment is only sound while no other thread reads it.

use std::fs;
use std::path::PathBuf;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("waml-persist-fault-env-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cli_fault_injection_env_vars_do_not_reach_the_library_writer() {
    let temp = TempDir::new();
    fs::write(temp.0.join("a.md"), "before").unwrap();
    std::env::set_var("WAML_CLI_TEST_FAIL_RENAME_AT", "1");
    std::env::set_var("WAML_CLI_TEST_FAIL_CLEANUP", "1");

    let touched = waml::host::persist::write_back(
        &temp.0,
        &[("a.md".to_owned(), "before".to_owned())],
        &[("a.md".to_owned(), "after".to_owned())],
    )
    .expect("a CLI test's environment must not fail a library write");

    // A honoured `WAML_CLI_TEST_FAIL_RENAME_AT=1` would have failed the write
    // outright; a honoured `WAML_CLI_TEST_FAIL_CLEANUP` would have appended a
    // staging-cleanup warning to the report.
    assert_eq!(touched, ["wrote a.md"]);
    assert_eq!(fs::read_to_string(temp.0.join("a.md")).unwrap(), "after");
    assert_eq!(
        fs::read_dir(&temp.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        ["a.md"]
    );
}
