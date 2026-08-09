//! CLI argument parsing and diagram selection for the viewer.

use std::path::PathBuf;
use waml::model::{Diagram, Model};

#[derive(Debug, Clone, PartialEq)]
pub struct Args {
    pub dir: Option<PathBuf>,
    pub diagram: Option<String>,
    /// Badge text from `--title`. Identifies which agent launched this window;
    /// purely cosmetic, never interpreted.
    pub badge: Option<String>,
    /// sRGB components in `0.0..=1.0` from `--color`. Deliberately not a makepad
    /// `Vec4`, so this module stays makepad-free and its tests need no `Cx`.
    pub tint: Option<[f32; 3]>,
}

/// Parse `#rgb` / `#rrggbb` (leading `#` optional, case-insensitive) into sRGB
/// components in `0.0..=1.0`. Alpha forms are rejected: the blend factors are
/// fixed by the design, so a caller-supplied alpha would have no meaning.
fn parse_hex(s: &str) -> Option<[f32; 3]> {
    let h: Vec<char> = s.strip_prefix('#').unwrap_or(s).chars().collect();
    let nib = |c: char| c.to_digit(16).map(|d| d as u16);
    let rgb: [u16; 3] = match h.len() {
        // `f` -> `ff`: doubling the nibble, i.e. * 17.
        3 => [nib(h[0])? * 17, nib(h[1])? * 17, nib(h[2])? * 17],
        6 => [
            nib(h[0])? * 16 + nib(h[1])?,
            nib(h[2])? * 16 + nib(h[3])?,
            nib(h[4])? * 16 + nib(h[5])?,
        ],
        _ => return None,
    };
    Some([
        rgb[0] as f32 / 255.0,
        rgb[1] as f32 / 255.0,
        rgb[2] as f32 / 255.0,
    ])
}

/// Parse `argv` (including argv[0]).
/// Usage: `waml-editor [<okf-dir>] [--diagram <name>] [--title <text>] [--color <hex>]`.
pub fn parse(argv: &[String]) -> Result<Args, String> {
    let mut dir: Option<PathBuf> = None;
    let mut diagram: Option<String> = None;
    let mut badge: Option<String> = None;
    let mut tint: Option<[f32; 3]> = None;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = Some(argv.get(i).cloned().ok_or("--diagram requires a value")?);
            }
            "--title" => {
                i += 1;
                badge = Some(argv.get(i).cloned().ok_or("--title requires a value")?);
            }
            "--color" => {
                i += 1;
                let raw = argv.get(i).ok_or("--color requires a value")?;
                tint = Some(
                    parse_hex(raw).ok_or_else(|| format!("--color: not a hex colour: {raw}"))?,
                );
            }
            // makepad's studio/test runner appends these to every app it
            // launches; the platform layer consumes them, so skip them here
            // instead of mistaking them for the bundle dir.
            "--stdin-loop" => {}
            other if other.starts_with("--message-format=") => {}
            other if dir.is_none() => dir = Some(PathBuf::from(other)),
            other => return Err(format!("unexpected argument: {other}")),
        }
        i += 1;
    }
    Ok(Args {
        dir,
        diagram,
        badge,
        tint,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitialDocument<'a> {
    Diagram(&'a str),
    Concept(&'a str),
    None,
}

fn first_concept_in_directory<'a>(bundle: &'a waml::okf::Bundle, address: &str) -> Option<&'a str> {
    let index = bundle.index(address)?;
    for member in &index.members {
        if let Some(concept) = bundle.concept(member) {
            return Some(concept.id.as_str());
        }
        if bundle.directory(member).is_some() {
            if let Some(concept_id) = first_concept_in_directory(bundle, member) {
                return Some(concept_id);
            }
        }
    }
    None
}

pub fn select_initial_document<'a>(
    bundle: &'a waml::okf::Bundle,
    uml: &'a waml::uml::Projection,
    wanted_diagram: Option<&str>,
) -> InitialDocument<'a> {
    if let Some(diagram) = select_diagram(uml, wanted_diagram) {
        return InitialDocument::Diagram(diagram.key.as_str());
    }
    first_concept_in_directory(bundle, "/")
        .map(InitialDocument::Concept)
        .unwrap_or(InitialDocument::None)
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
        assert_eq!(a.dir, Some(PathBuf::from("some/dir")));
        assert_eq!(a.diagram, None);
    }

    #[test]
    fn parses_dir_and_diagram_flag() {
        let a = parse(&argv(&["waml-editor", "some/dir", "--diagram", "Orders"])).unwrap();
        assert_eq!(a.dir, Some(PathBuf::from("some/dir")));
        assert_eq!(a.diagram.as_deref(), Some("Orders"));
    }

    #[test]
    fn missing_dir_is_ok() {
        // No-arg launch is valid now (it opens the start screen); dir is None.
        let a = parse(&argv(&["waml-editor"])).unwrap();
        assert_eq!(a.dir, None);
        assert_eq!(a.diagram, None);
    }

    #[test]
    fn unknown_flag_is_still_an_error() {
        assert!(parse(&argv(&["waml-editor", "a", "b"])).is_err());
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

    fn fixture(name: &str) -> (waml::okf::Bundle, waml::uml::Projection) {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        let source = load::read_bundle(&dir).unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 0).unwrap();
        (
            prepared.okf().bundle.clone(),
            prepared.uml().projection.clone(),
        )
    }

    #[test]
    fn initial_document_prefers_requested_then_first_diagram() {
        let (bundle, uml) = fixture("mixed-okf");
        assert_eq!(
            select_initial_document(&bundle, &uml, Some("Orders")),
            InitialDocument::Diagram("orders-diagram")
        );
        assert_eq!(
            select_initial_document(&bundle, &uml, Some("missing")),
            InitialDocument::Diagram("orders-diagram")
        );
    }

    #[test]
    fn initial_document_falls_back_to_first_indexed_concept() {
        let (bundle, uml) = fixture("okf-only");
        assert_eq!(
            select_initial_document(&bundle, &uml, None),
            InitialDocument::Concept("notes")
        );
    }

    #[test]
    fn initial_document_is_none_for_empty_bundle() {
        let source = waml::source::SourceBundle::default();
        let prepared = waml::analysis::prepare_candidate(source, None, 0).unwrap();
        assert_eq!(
            select_initial_document(&prepared.okf().bundle, &prepared.uml().projection, None,),
            InitialDocument::None
        );
    }

    // Two-diagram coverage: the single-diagram fixture above can't distinguish a
    // title/key match from the fallback-to-first behavior, and never exercises
    // the `d.key == w` arm at all. `Model`/`Diagram` have all-`pub` fields and
    // `Model: Default`, so we build a tiny two-diagram model by hand rather than
    // adding a second OKF fixture.
    use waml::model::DiagramDisplay;

    fn diagram(key: &str, title: &str) -> Diagram {
        Diagram {
            key: key.to_string(),
            title: title.to_string(),
            profile: "erd".to_string(),
            description: None,
            groups: vec![],
            layout: vec![],
            display: DiagramDisplay::default(),
        }
    }

    fn two_diagram_model() -> Model {
        Model {
            diagrams: vec![
                diagram("orders-key", "Orders"),
                diagram("inventory-key", "Inventory"),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn select_by_title_returns_the_specific_non_first_diagram() {
        let model = two_diagram_model();
        let picked = select_diagram(&model, Some("Inventory")).unwrap();
        assert_eq!(picked.key, "inventory-key");
        assert_eq!(picked.title, "Inventory");
    }

    #[test]
    fn select_by_key_returns_the_specific_non_first_diagram() {
        let model = two_diagram_model();
        // Exercises the `d.key == w` arm: "inventory-key" is not a title.
        let picked = select_diagram(&model, Some("inventory-key")).unwrap();
        assert_eq!(picked.title, "Inventory");
        assert_eq!(picked.key, "inventory-key");
    }

    #[test]
    fn select_none_returns_the_first_diagram() {
        let model = two_diagram_model();
        let picked = select_diagram(&model, None).unwrap();
        assert_eq!(picked.key, "orders-key");
    }

    #[test]
    fn select_unknown_falls_back_to_the_first_diagram() {
        let model = two_diagram_model();
        let picked = select_diagram(&model, Some("nonexistent")).unwrap();
        assert_eq!(picked.key, "orders-key");
    }

    #[test]
    fn skips_makepad_runner_plumbing_args() {
        // Exact argv shape the makepad test driver launches the editor with.
        let a = parse(&argv(&[
            "waml-editor",
            "--message-format=json",
            "some/dir",
            "--title",
            "ui-test",
            "--stdin-loop",
        ]))
        .unwrap();
        assert_eq!(a.dir, Some(PathBuf::from("some/dir")));
        assert_eq!(a.badge.as_deref(), Some("ui-test"));
    }

    #[test]
    fn parses_title_flag() {
        let a = parse(&argv(&["waml-editor", "some/dir", "--title", "veil-fix"])).unwrap();
        assert_eq!(a.dir, Some(PathBuf::from("some/dir")));
        assert_eq!(a.badge.as_deref(), Some("veil-fix"));
        assert_eq!(a.tint, None);
    }

    #[test]
    fn parses_color_flag_six_digit_with_hash() {
        let a = parse(&argv(&["waml-editor", "--color", "#ff0080"])).unwrap();
        let t = a.tint.unwrap();
        assert!((t[0] - 1.0).abs() < 1e-6);
        assert!((t[1] - 0.0).abs() < 1e-6);
        assert!((t[2] - 128.0 / 255.0).abs() < 1e-6);
        assert_eq!(a.badge, None);
        assert_eq!(a.dir, None);
    }

    #[test]
    fn parses_color_flag_without_hash_and_mixed_case() {
        let a = parse(&argv(&["waml-editor", "--color", "FF0080"])).unwrap();
        assert!((a.tint.unwrap()[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn parses_three_digit_shorthand_by_doubling_nibbles() {
        // f0a -> ff00aa
        let short = parse(&argv(&["waml-editor", "--color", "f0a"]))
            .unwrap()
            .tint
            .unwrap();
        let long = parse(&argv(&["waml-editor", "--color", "#ff00aa"]))
            .unwrap()
            .tint
            .unwrap();
        for i in 0..3 {
            assert!((short[i] - long[i]).abs() < 1e-6, "component {i}");
        }
    }

    #[test]
    fn both_flags_compose_with_dir_and_diagram_in_any_order() {
        let a = parse(&argv(&[
            "waml-editor",
            "--title",
            "opus-3",
            "some/dir",
            "--color",
            "#2b8",
            "--diagram",
            "Orders",
        ]))
        .unwrap();
        assert_eq!(a.dir, Some(PathBuf::from("some/dir")));
        assert_eq!(a.diagram.as_deref(), Some("Orders"));
        assert_eq!(a.badge.as_deref(), Some("opus-3"));
        assert!(a.tint.is_some());
    }

    #[test]
    fn empty_title_is_accepted() {
        let a = parse(&argv(&["waml-editor", "--title", ""])).unwrap();
        assert_eq!(a.badge.as_deref(), Some(""));
    }

    #[test]
    fn missing_flag_values_are_errors() {
        assert!(parse(&argv(&["waml-editor", "--title"])).is_err());
        assert!(parse(&argv(&["waml-editor", "--color"])).is_err());
    }

    #[test]
    fn bad_hex_is_an_error_naming_the_value() {
        let e = parse(&argv(&["waml-editor", "--color", "zzz"])).unwrap_err();
        assert!(
            e.contains("zzz"),
            "error should name the bad value, got: {e}"
        );
        assert!(parse(&argv(&["waml-editor", "--color", "#ff00"])).is_err()); // wrong length
        assert!(parse(&argv(&["waml-editor", "--color", "#ff00800"])).is_err()); // wrong length
        assert!(parse(&argv(&["waml-editor", "--color", ""])).is_err());
    }

    #[test]
    fn absent_flags_leave_both_fields_none() {
        let a = parse(&argv(&["waml-editor", "some/dir"])).unwrap();
        assert_eq!(a.badge, None);
        assert_eq!(a.tint, None);
    }
}
