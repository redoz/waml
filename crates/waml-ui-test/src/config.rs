use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static RUN_COUNTER: AtomicU64 = AtomicU64::new(0);
const MAX_TITLE_BYTES: usize = 48;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RunIdentity {
    pub run_id: String,
    pub test_slug: String,
    pub title: String,
    pub run_root: PathBuf,
}

impl RunIdentity {
    pub(crate) fn new(workspace_root: &Path, test_name: &str) -> Self {
        let counter = RUN_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
        let run_id = format!("{}-{counter}", std::process::id());
        let test_slug = sanitize_component(test_name);
        let title_prefix = format!("ui-{run_id}-");
        let title_slug_len = MAX_TITLE_BYTES.saturating_sub(title_prefix.len());
        let mut title_slug = test_slug[..test_slug.len().min(title_slug_len)]
            .trim_end_matches('-')
            .to_string();
        if title_slug.is_empty() {
            title_slug.push_str("test");
        }
        let title = format!("{title_prefix}{title_slug}");
        let run_root = workspace_root
            .join("target")
            .join("waml-ui-test")
            .join(&run_id)
            .join(&test_slug);

        Self {
            run_id,
            test_slug,
            title,
            run_root,
        }
    }
}

fn sanitize_component(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    for byte in value.bytes() {
        let normalized = match byte {
            b'a'..=b'z' | b'0'..=b'9' => Some(byte as char),
            b'A'..=b'Z' => Some((byte + (b'a' - b'A')) as char),
            _ => None,
        };
        if let Some(ch) = normalized {
            sanitized.push(ch);
        } else if !sanitized.ends_with('-') {
            sanitized.push('-');
        }
    }
    let sanitized = sanitized.trim_matches('-');
    if sanitized.is_empty() {
        "test".to_string()
    } else {
        sanitized.to_string()
    }
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
    use super::{RunIdentity, ScenarioConfig, WorkspaceFixture};
    use std::path::Path;

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

    #[test]
    fn run_identity_is_unique_for_repeated_test_names() {
        let first = RunIdentity::new(Path::new("C:/workspace"), "ui::opens orders");
        let second = RunIdentity::new(Path::new("C:/workspace"), "ui::opens orders");

        assert_ne!(first.run_id, second.run_id);
        assert_ne!(first.run_root, second.run_root);
        assert_ne!(first.title, second.title);
    }

    #[test]
    fn run_title_is_short_safe_and_keeps_identity_and_slug() {
        let identity = RunIdentity::new(
            Path::new("C:/workspace"),
            "ui::opens Orders source view with punctuation?! and a very long suffix",
        );

        assert!(identity.title.starts_with("ui-"));
        assert!(identity.title.len() <= 48);
        assert!(identity
            .title
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'));
        assert!(identity.title.contains(&identity.run_id));
        assert!(identity.title.contains("ui-opens-orders"));
    }

    #[test]
    fn test_names_cannot_add_path_components() {
        let identity = RunIdentity::new(
            Path::new("C:/workspace"),
            "module::..\\outside/also-outside?!",
        );

        assert_eq!(identity.test_slug, "module-outside-also-outside");
        assert_eq!(
            identity.run_root,
            Path::new("C:/workspace")
                .join("target")
                .join("waml-ui-test")
                .join(&identity.run_id)
                .join("module-outside-also-outside")
        );
    }
}
