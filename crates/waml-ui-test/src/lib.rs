mod adapters;
mod app;
mod config;
mod domain;
mod error;
mod fixture;
mod run;
mod trace;

pub use app::WamlApp;
pub use config::{ScenarioConfig, WorkspaceFixture};
pub use domain::{DiagramName, ViewKind};
pub use error::WamlUiError;
pub use waml_ui_test_macros::waml_ui_test;

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
        let scenario = crate::ScenarioConfig {
            package_name,
            manifest_dir,
            module_path,
            test_name,
            workspace,
        };
        crate::run::run_scenario(scenario, test);
    }
}

#[cfg(test)]
mod tests {
    use super::{__private::run_catalog_test, WorkspaceFixture};

    #[test]
    fn catalog_runner_is_available_to_macro_expansions() {
        let _runner: fn(
            &'static str,
            &'static str,
            &'static str,
            &'static str,
            WorkspaceFixture,
            fn(super::WamlApp),
        ) = run_catalog_test;
        let _fixture = WorkspaceFixture::Mini;
    }
}
