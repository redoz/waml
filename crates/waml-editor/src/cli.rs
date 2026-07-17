//! CLI argument parsing and diagram selection for the viewer.

use std::path::PathBuf;
use waml::model::{Diagram, Model};

#[derive(Debug, Clone, PartialEq)]
pub struct Args {
    pub dir: PathBuf,
    pub diagram: Option<String>,
}

/// Parse `argv` (including argv[0]). Usage: `waml-editor <okf-dir> [--diagram <name>]`.
pub fn parse(argv: &[String]) -> Result<Args, String> {
    let mut dir: Option<PathBuf> = None;
    let mut diagram: Option<String> = None;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = Some(
                    argv.get(i).cloned().ok_or("--diagram requires a value")?,
                );
            }
            other if dir.is_none() => dir = Some(PathBuf::from(other)),
            other => return Err(format!("unexpected argument: {other}")),
        }
        i += 1;
    }
    Ok(Args {
        dir: dir.ok_or("usage: waml-editor <okf-dir> [--diagram <name>]")?,
        diagram,
    })
}

/// Pick a diagram by title or key; fall back to the first diagram.
pub fn select_diagram<'a>(model: &'a Model, wanted: Option<&str>) -> Option<&'a Diagram> {
    if let Some(w) = wanted {
        if let Some(d) = model.diagrams.iter().find(|d| d.title == w || d.key == w) {
            return Some(d);
        }
    }
    model.diagrams.first()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load;
    use std::path::Path;

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_dir_only() {
        let a = parse(&argv(&["waml-editor", "some/dir"])).unwrap();
        assert_eq!(a.dir, PathBuf::from("some/dir"));
        assert_eq!(a.diagram, None);
    }

    #[test]
    fn parses_dir_and_diagram_flag() {
        let a = parse(&argv(&["waml-editor", "some/dir", "--diagram", "Orders"])).unwrap();
        assert_eq!(a.diagram.as_deref(), Some("Orders"));
    }

    #[test]
    fn missing_dir_is_an_error() {
        assert!(parse(&argv(&["waml-editor"])).is_err());
    }

    #[test]
    fn select_matches_by_title_then_falls_back_to_first() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
        let model = load::load_model(&dir).unwrap();

        let by_title = select_diagram(&model, Some("Orders")).unwrap();
        assert_eq!(by_title.title, "Orders");

        // Unknown name falls back to the first diagram rather than None.
        let fallback = select_diagram(&model, Some("nope")).unwrap();
        assert_eq!(fallback.title, "Orders");

        let default = select_diagram(&model, None).unwrap();
        assert_eq!(default.title, "Orders");
    }
}
