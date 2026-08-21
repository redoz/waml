use crate::fixture::reserve_run_root;
use crate::DiagramName;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static RUN_COUNTER: AtomicU64 = AtomicU64::new(0);
const MAX_TITLE_BYTES: usize = 48;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceFixture {
    Mini,
    /// One state-machine document, chosen for its SHAPES rather than its
    /// content: `Active` carries a self-loop and a long back edge to `Idle`,
    /// which are the two connectors `90ffcf0f` moved. The rendering gate
    /// compares this canvas, so the fixture stays small on purpose -- every
    /// extra node is more ink for a comparison to be noisy about.
    Behavior,
}

#[doc(hidden)]
pub struct ScenarioConfig {
    pub(crate) package_name: &'static str,
    pub(crate) manifest_dir: &'static str,
    pub(crate) module_path: &'static str,
    pub(crate) test_name: &'static str,
    pub(crate) workspace: WorkspaceFixture,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FixtureDescriptor {
    pub(crate) relative_path: &'static str,
    pub(crate) workspace: WorkspaceBinding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceBinding {
    pub(crate) root: WorkspaceRootFingerprint,
    pub(crate) ready_diagram: DiagramName,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceRootFingerprint {
    pub(crate) title: &'static str,
    pub(crate) value: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RunIdentity {
    pub run_id: String,
    pub test_slug: String,
    pub title: String,
    pub allocation_root: PathBuf,
    pub run_root: PathBuf,
}

impl RunIdentity {
    pub(crate) fn allocate(workspace_root: &Path, test_name: &str) -> io::Result<Self> {
        Self::allocate_with_counter(workspace_root, test_name, std::process::id(), &RUN_COUNTER)
    }

    fn allocate_with_counter(
        workspace_root: &Path,
        test_name: &str,
        process_id: u32,
        counter: &AtomicU64,
    ) -> io::Result<Self> {
        let test_slug = sanitize_component(test_name);
        loop {
            let sequence = counter
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    value.checked_add(1)
                })
                .map(|previous| previous + 1)
                .map_err(|_| io::Error::other("run identity counter is exhausted"))?;
            let run_id = format!("{process_id}-{sequence}");
            let (allocation_root, run_root) =
                match reserve_run_root(workspace_root, &run_id, &test_slug) {
                    Ok(paths) => paths,
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => return Err(error),
                };
            let title_prefix = format!("ui-{run_id}-");
            let title_slug_len = MAX_TITLE_BYTES.saturating_sub(title_prefix.len());
            let mut title_slug = test_slug[..test_slug.len().min(title_slug_len)]
                .trim_end_matches('-')
                .to_string();
            if title_slug.is_empty() {
                title_slug.push_str("test");
            }
            let title = format!("{title_prefix}{title_slug}");

            return Ok(Self {
                run_id,
                test_slug,
                title,
                allocation_root,
                run_root,
            });
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
                workspace: WorkspaceBinding {
                    root: WorkspaceRootFingerprint {
                        title: "Mini",
                        value: "/",
                    },
                    ready_diagram: DiagramName::ORDERS,
                },
            },
            Self::Behavior => FixtureDescriptor {
                relative_path: "tests/fixtures/behavior",
                workspace: WorkspaceBinding {
                    root: WorkspaceRootFingerprint {
                        title: "Behavior",
                        value: "/",
                    },
                    ready_diagram: DiagramName::LIGHT_CYCLE,
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RunIdentity, ScenarioConfig, WorkspaceBinding, WorkspaceFixture, WorkspaceRootFingerprint,
    };
    use crate::DiagramName;
    use std::collections::HashSet;
    use std::fs;
    use std::sync::atomic::AtomicU64;
    use std::sync::Barrier;
    use std::thread;

    #[test]
    fn mini_fixture_binds_its_semantic_root_and_typed_ready_diagram() {
        let descriptor = WorkspaceFixture::Mini.descriptor();

        assert_eq!(descriptor.relative_path, "tests/fixtures/mini");
        assert_eq!(
            descriptor.workspace,
            WorkspaceBinding {
                root: WorkspaceRootFingerprint {
                    title: "Mini",
                    value: "/",
                },
                ready_diagram: DiagramName::ORDERS,
            }
        );
    }

    #[test]
    fn behavior_fixture_binds_the_light_cycle_state_machine() {
        let descriptor = WorkspaceFixture::Behavior.descriptor();

        assert_eq!(descriptor.relative_path, "tests/fixtures/behavior");
        assert_eq!(descriptor.workspace.ready_diagram, DiagramName::LIGHT_CYCLE);
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
        let temp = tempfile::tempdir().unwrap();
        let first = RunIdentity::allocate(temp.path(), "ui::opens orders").unwrap();
        let second = RunIdentity::allocate(temp.path(), "ui::opens orders").unwrap();

        assert_ne!(first.run_id, second.run_id);
        assert_ne!(first.run_root, second.run_root);
        assert_ne!(first.title, second.title);
    }

    #[test]
    fn allocation_retries_a_preserved_candidate_without_touching_its_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let counter = AtomicU64::new(0);
        let ownership_root = temp.path().join("target").join("waml-ui-test");
        let preserved = ownership_root.join("4242-1").join("another-test");
        fs::create_dir_all(&preserved).unwrap();
        fs::write(preserved.join("failure.txt"), "preserved failure").unwrap();

        let identity =
            RunIdentity::allocate_with_counter(temp.path(), "ui::opens orders", 4242, &counter)
                .unwrap();

        assert_eq!(identity.run_id, "4242-2");
        assert_eq!(
            fs::read_to_string(preserved.join("failure.txt")).unwrap(),
            "preserved failure"
        );
        assert!(identity.allocation_root.is_dir());
        assert!(identity.run_root.is_dir());
        assert!(identity.title.contains("4242-2"));
        assert!(identity.title.len() <= 48);
    }

    #[test]
    fn parallel_allocations_contend_on_the_parent_and_retry_to_unique_identities() {
        const WORKERS: usize = 8;

        let temp = tempfile::tempdir().unwrap();
        let start = Barrier::new(WORKERS);

        let identities = thread::scope(|scope| {
            let handles: Vec<_> = (0..WORKERS)
                .map(|_| {
                    scope.spawn(|| {
                        let counter = AtomicU64::new(0);
                        start.wait();
                        RunIdentity::allocate_with_counter(
                            temp.path(),
                            "ui::opens orders",
                            4242,
                            &counter,
                        )
                        .unwrap()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });

        let run_ids: HashSet<_> = identities
            .iter()
            .map(|identity| identity.run_id.as_str())
            .collect();
        let allocation_roots: HashSet<_> = identities
            .iter()
            .map(|identity| identity.allocation_root.as_path())
            .collect();
        let run_roots: HashSet<_> = identities
            .iter()
            .map(|identity| identity.run_root.as_path())
            .collect();
        assert_eq!(run_ids.len(), WORKERS);
        assert_eq!(allocation_roots.len(), WORKERS);
        assert_eq!(run_roots.len(), WORKERS);
        assert!(run_roots.iter().all(|run_root| run_root.is_dir()));
        assert!(identities.iter().all(|identity| {
            identity.run_root.parent() == Some(identity.allocation_root.as_path())
                && identity.allocation_root.is_dir()
        }));
    }

    #[test]
    fn run_title_is_short_safe_and_keeps_identity_and_slug() {
        let temp = tempfile::tempdir().unwrap();
        let identity = RunIdentity::allocate(
            temp.path(),
            "ui::opens Orders source view with punctuation?! and a very long suffix",
        )
        .unwrap();

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
        let temp = tempfile::tempdir().unwrap();
        let identity =
            RunIdentity::allocate(temp.path(), "module::..\\outside/also-outside?!").unwrap();

        assert_eq!(identity.test_slug, "module-outside-also-outside");
        assert_eq!(
            identity.run_root,
            temp.path()
                .join("target")
                .join("waml-ui-test")
                .join(&identity.run_id)
                .join("module-outside-also-outside")
        );
    }
}
