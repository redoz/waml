#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceFixture {
    Mini,
}

pub struct ScenarioConfig {
    pub package_name: &'static str,
    pub manifest_dir: &'static str,
    pub module_path: &'static str,
    pub test_name: &'static str,
    pub workspace: WorkspaceFixture,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FixtureDescriptor {
    pub relative_path: &'static str,
    pub ready_diagram: &'static str,
}

impl WorkspaceFixture {
    pub(crate) const fn descriptor(self) -> FixtureDescriptor {
        match self {
            Self::Mini => FixtureDescriptor {
                relative_path: "tests/fixtures/mini",
                ready_diagram: "Orders",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ScenarioConfig, WorkspaceFixture};

    #[test]
    fn mini_fixture_has_its_catalog_metadata() {
        let descriptor = WorkspaceFixture::Mini.descriptor();

        assert_eq!(descriptor.relative_path, "tests/fixtures/mini");
        assert_eq!(descriptor.ready_diagram, "Orders");
    }

    #[test]
    fn scenario_config_keeps_catalog_call_site_metadata() {
        let scenario = ScenarioConfig {
            package_name: "waml-editor",
            manifest_dir: "C:/dev/waml",
            module_path: "ui",
            test_name: "navigation",
            workspace: WorkspaceFixture::Mini,
        };

        assert_eq!(scenario.package_name, "waml-editor");
        assert_eq!(scenario.test_name, "navigation");
    }
}
