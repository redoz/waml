mod config;

pub use config::{ScenarioConfig, WorkspaceFixture};
pub use waml_ui_test_macros::waml_ui_test;

#[derive(Default)]
pub struct WamlApp;

#[doc(hidden)]
pub mod __private {
    pub fn run_catalog_test(
        manifest_dir: &'static str,
        package_name: &'static str,
        module_path: &'static str,
        test_name: &'static str,
        workspace: crate::WorkspaceFixture,
        test: impl FnOnce(crate::WamlApp),
    ) {
        let _scenario = crate::ScenarioConfig {
            package_name,
            manifest_dir,
            module_path,
            test_name,
            workspace,
        };
        let _fixture = workspace.descriptor();

        test(crate::WamlApp);
    }
}

#[cfg(test)]
mod tests {
    use super::{__private::run_catalog_test, WorkspaceFixture};

    #[test]
    fn catalog_runner_calls_the_test_with_a_zero_sized_app() {
        let mut called = false;

        run_catalog_test(
            "C:/dev/waml",
            "waml-editor",
            "ui",
            "navigation",
            WorkspaceFixture::Mini,
            |_| called = true,
        );

        assert!(called);
    }
}
