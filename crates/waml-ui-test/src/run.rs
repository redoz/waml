use crate::app::WamlApp;
use crate::config::{RunIdentity, ScenarioConfig};
use crate::fixture::{cleanup_run, ownership_root, resolve_workspace_root, stage_fixture};
use crate::trace::SemanticTrace;
use std::fs;
use std::io;
use std::path::Path;

const DRIVER_ARTIFACTS_DIR: &str = "driver-artifacts";
const DRIVER_EVIDENCE_FILES: &[&str] = &[
    "failure.txt",
    "failure-screenshot.png",
    "failure-screenshot-error.txt",
    "widget-tree.txt",
    "widget-tree-error.txt",
    "widget-snapshot.json",
    "widget-snapshot-error.txt",
    "logs.txt",
    "logs-error.txt",
];

pub(crate) fn run_scenario(config: ScenarioConfig, scenario: impl FnOnce(WamlApp)) {
    let editor_manifest_dir = Path::new(config.manifest_dir);
    let workspace_root = resolve_workspace_root(editor_manifest_dir)
        .unwrap_or_else(|error| panic!("failed to resolve the WAML workspace: {error}"));
    let test_name = full_test_name(&config);
    let identity = RunIdentity::allocate(&workspace_root, &test_name).unwrap_or_else(|error| {
        panic!("failed to allocate run identity for `{test_name}`: {error}")
    });
    let descriptor = config.workspace.descriptor();
    let workspace = descriptor.workspace;
    let staged_workspace = stage_fixture(&workspace_root, descriptor, &identity.run_root)
        .unwrap_or_else(|error| {
            panic!(
                "failed to stage fixture for `{test_name}`: {error}; preserved run: {}",
                identity.run_root.display()
            )
        });
    let trace = SemanticTrace::new(&identity.run_root).unwrap_or_else(|error| {
        panic!(
            "failed to create semantic trace for `{test_name}`: {error}; preserved run: {}",
            identity.run_root.display()
        )
    });
    let driver_config =
        build_driver_config(&config, &identity, &staged_workspace).unwrap_or_else(|error| {
            panic!(
                "failed to configure `{test_name}`: {}; preserved run: {}",
                error.message(),
                identity.run_root.display()
            )
        });
    let run_root = identity.run_root.clone();
    let app_test_name = test_name.clone();
    let app_run_root = run_root.clone();
    let result = makepad_test::run_with_config(driver_config, move |driver| {
        scenario(WamlApp::new(
            driver,
            app_test_name,
            app_run_root,
            workspace,
            trace,
        ));
    });
    let promotion = promote_driver_evidence(&run_root);
    let result = result.map_err(|error| error.message().to_string());
    if let Err(error) = finalize_run(
        &ownership_root(&workspace_root),
        &identity.allocation_root,
        &run_root,
        result,
        promotion,
    ) {
        panic!("scenario `{test_name}` failed:\n{error}");
    }
}

fn finalize_run(
    ownership_root: &Path,
    allocation_root: &Path,
    run_root: &Path,
    run_result: Result<(), String>,
    promotion_result: io::Result<()>,
) -> Result<(), String> {
    let succeeded = run_result.is_ok() && promotion_result.is_ok();
    let cleanup_result = cleanup_run(ownership_root, allocation_root, succeeded);
    finalize_run_with_cleanup_result(run_root, run_result, promotion_result, cleanup_result)
}

fn finalize_run_with_cleanup_result(
    run_root: &Path,
    run_result: Result<(), String>,
    promotion_result: io::Result<()>,
    cleanup_result: io::Result<()>,
) -> Result<(), String> {
    let cleanup_attempted = run_result.is_ok() && promotion_result.is_ok();
    if cleanup_attempted {
        return cleanup_result.map_err(|error| {
            format!(
                "cleanup failed; evidence may be partial: {}\nDetail: {error}",
                run_root.display()
            )
        });
    }

    let mut failures = Vec::new();

    if let Err(error) = run_result {
        failures.push(error);
    }
    if let Err(error) = promotion_result {
        failures.push(format!("driver evidence promotion also failed: {error}"));
    }
    if let Err(error) = cleanup_result {
        failures.push(format!("cleanup validation also failed: {error}"));
    }

    failures.push(format!("Preserved run: {}", run_root.display()));
    Err(failures.join("\n"))
}

fn build_driver_config(
    scenario: &ScenarioConfig,
    identity: &RunIdentity,
    staged_workspace: &Path,
) -> makepad_test::TestResult<makepad_test::TestConfig> {
    let mut driver = makepad_test::TestConfig::current_package(
        scenario.manifest_dir,
        scenario.package_name,
        full_test_name(scenario),
    )?;
    driver.artifacts_dir = identity.run_root.join(DRIVER_ARTIFACTS_DIR);
    driver.args = vec![
        staged_workspace.to_string_lossy().into_owned(),
        "--title".to_string(),
        identity.title.clone(),
    ];
    Ok(driver)
}

fn full_test_name(scenario: &ScenarioConfig) -> String {
    if scenario.module_path.is_empty() {
        scenario.test_name.to_string()
    } else {
        format!("{}::{}", scenario.module_path, scenario.test_name)
    }
}

fn promote_driver_evidence(run_root: &Path) -> io::Result<()> {
    let driver_artifacts = run_root.join(DRIVER_ARTIFACTS_DIR);
    if !driver_artifacts.exists() {
        return Ok(());
    }
    for file_name in DRIVER_EVIDENCE_FILES {
        let source = driver_artifacts.join(file_name);
        if !source.exists() {
            continue;
        }
        let metadata = fs::symlink_metadata(&source)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "driver evidence is not a regular file: {}",
                    source.display()
                ),
            ));
        }
        fs::copy(source, run_root.join(file_name))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        build_driver_config, finalize_run, finalize_run_with_cleanup_result,
        promote_driver_evidence,
    };
    use crate::config::{RunIdentity, ScenarioConfig, WorkspaceFixture};
    use std::fs;
    use std::io;
    use std::path::Path;

    #[test]
    fn driver_config_binds_owned_artifacts_workspace_and_unique_title() {
        let temp = tempfile::tempdir().unwrap();
        let manifest_dir = temp.path().join("crates").join("waml-editor");
        let staged_workspace = temp.path().join("staged workspace");
        let manifest = manifest_dir.to_string_lossy().into_owned();
        let scenario = ScenarioConfig {
            manifest_dir: Box::leak(manifest.into_boxed_str()),
            package_name: "waml-editor",
            module_path: "ui::documents",
            test_name: "switch_view",
            workspace: WorkspaceFixture::Mini,
        };
        let identity = RunIdentity::allocate(temp.path(), "ui::documents::switch_view").unwrap();

        let driver = build_driver_config(&scenario, &identity, &staged_workspace).unwrap();

        assert_eq!(driver.test_name, "ui::documents::switch_view");
        assert_eq!(
            driver.artifacts_dir,
            identity.run_root.join("driver-artifacts")
        );
        assert_eq!(
            driver.args,
            vec![
                staged_workspace.to_string_lossy().into_owned(),
                "--title".to_string(),
                identity.title,
            ]
        );
    }

    #[test]
    fn promotion_copies_known_driver_evidence_without_touching_waml_artifacts() {
        let temp = tempfile::tempdir().unwrap();
        let run_root = temp.path().join("run");
        let driver_artifacts = run_root.join("driver-artifacts");
        fs::create_dir_all(run_root.join("workspace")).unwrap();
        fs::create_dir_all(&driver_artifacts).unwrap();
        fs::write(run_root.join("semantic-trace.txt"), "waml trace").unwrap();
        fs::write(driver_artifacts.join("failure.txt"), "driver failure").unwrap();
        fs::write(driver_artifacts.join("logs.txt"), "driver logs").unwrap();
        fs::write(
            driver_artifacts.join("private-driver-state.bin"),
            b"private",
        )
        .unwrap();

        promote_driver_evidence(&run_root).unwrap();

        assert_eq!(
            fs::read_to_string(run_root.join("failure.txt")).unwrap(),
            "driver failure"
        );
        assert_eq!(
            fs::read_to_string(run_root.join("logs.txt")).unwrap(),
            "driver logs"
        );
        assert_eq!(
            fs::read_to_string(run_root.join("semantic-trace.txt")).unwrap(),
            "waml trace"
        );
        assert!(run_root.join("workspace").is_dir());
        assert!(!run_root.join("private-driver-state.bin").exists());
        assert!(driver_artifacts.join("private-driver-state.bin").is_file());
    }

    #[test]
    fn promotion_error_keeps_the_original_driver_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let run_root = temp.path().join("run");
        let source = run_root.join("driver-artifacts").join("failure.txt");
        fs::create_dir_all(&source).unwrap();

        assert!(promote_driver_evidence(&run_root).is_err());
        assert!(source.is_dir());
    }

    #[test]
    fn finalization_validates_a_failed_run_before_preserving_it() {
        let temp = tempfile::tempdir().unwrap();
        let ownership_root = temp.path().join("target").join("waml-ui-test");
        let actual_run = ownership_root.join("actual-run");
        let linked_run = ownership_root.join("linked-run");
        fs::create_dir_all(&actual_run).unwrap();
        create_directory_link(&actual_run, &linked_run).unwrap();

        let error = finalize_run(
            &ownership_root,
            &linked_run,
            &linked_run,
            Err("driver failed".to_string()),
            Ok(()),
        )
        .unwrap_err();

        assert!(error.contains("driver failed"));
        assert!(error.contains("cleanup validation also failed"));
        assert!(fs::symlink_metadata(&linked_run).is_ok());
        assert!(actual_run.is_dir());
    }

    #[test]
    fn successful_finalization_removes_the_reserved_identity_directory() {
        let temp = tempfile::tempdir().unwrap();
        let ownership_root = temp.path().join("target").join("waml-ui-test");
        let allocation_root = ownership_root.join("4242-1");
        let run_root = allocation_root.join("ui-opens-orders");
        fs::create_dir_all(run_root.join("workspace")).unwrap();

        finalize_run(&ownership_root, &allocation_root, &run_root, Ok(()), Ok(())).unwrap();

        assert!(!allocation_root.exists());
    }

    #[test]
    fn successful_cleanup_failure_reports_that_evidence_may_be_partial() {
        let run_root = Path::new("C:/workspace/target/waml-ui-test/4242-1/ui-opens-orders");

        let error = finalize_run_with_cleanup_result(
            run_root,
            Ok(()),
            Ok(()),
            Err(io::Error::other("injected cleanup failure")),
        )
        .unwrap_err();

        assert_eq!(
            error,
            format!(
                "cleanup failed; evidence may be partial: {}\nDetail: injected cleanup failure",
                run_root.display()
            )
        );
        assert!(!error.contains("Preserved run"));
    }

    #[test]
    fn failed_finalization_preserves_the_reserved_identity_and_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let ownership_root = temp.path().join("target").join("waml-ui-test");
        let allocation_root = ownership_root.join("4242-1");
        let run_root = allocation_root.join("ui-opens-orders");
        fs::create_dir_all(&run_root).unwrap();
        fs::write(run_root.join("failure.txt"), "failure evidence").unwrap();

        let error = finalize_run(
            &ownership_root,
            &allocation_root,
            &run_root,
            Err("driver failed".to_string()),
            Ok(()),
        )
        .unwrap_err();

        assert!(allocation_root.is_dir());
        assert_eq!(
            fs::read_to_string(run_root.join("failure.txt")).unwrap(),
            "failure evidence"
        );
        assert!(error.contains(&format!("Preserved run: {}", run_root.display())));
    }

    #[cfg(unix)]
    fn create_directory_link(source: &Path, link: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(source, link)
    }

    #[cfg(windows)]
    fn create_directory_link(source: &Path, link: &Path) -> io::Result<()> {
        std::os::windows::fs::symlink_dir(source, link)
    }
}
