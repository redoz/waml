use crate::app::{WamlApp, WorkspaceBinding};
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
    let identity = RunIdentity::new(&workspace_root, &test_name);
    let staged_workspace = stage_fixture(
        &workspace_root,
        config.workspace.descriptor(),
        &identity.run_root,
    )
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
    let workspace = WorkspaceBinding {
        fixture: config.workspace,
        staged_path: staged_workspace,
    };
    let result = makepad_test::run_with_config(driver_config, move |driver| {
        scenario(WamlApp::new(
            driver,
            workspace,
            app_test_name,
            app_run_root,
            trace,
        ));
    });
    let promotion = promote_driver_evidence(&run_root);

    match (result, promotion) {
        (Ok(()), Ok(())) => {
            cleanup_run(&ownership_root(&workspace_root), &run_root, true).unwrap_or_else(
                |error| {
                    panic!(
                        "scenario `{test_name}` passed but cleanup failed: {error}; run: {}",
                        run_root.display()
                    )
                },
            );
        }
        (Ok(()), Err(promotion_error)) => {
            panic!(
                "scenario `{test_name}` passed but driver evidence promotion failed: \
                 {promotion_error}; preserved run: {}",
                run_root.display()
            );
        }
        (Err(error), Ok(())) => {
            panic!("{}\nPreserved run: {}", error.message(), run_root.display());
        }
        (Err(error), Err(promotion_error)) => {
            panic!(
                "{}\nDriver evidence promotion also failed: {}\nPreserved run: {}",
                error.message(),
                promotion_error,
                run_root.display()
            );
        }
    }
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
    use super::{build_driver_config, promote_driver_evidence};
    use crate::config::{RunIdentity, ScenarioConfig, WorkspaceFixture};
    use std::fs;

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
        let identity = RunIdentity::new(temp.path(), "ui::documents::switch_view");

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
}
