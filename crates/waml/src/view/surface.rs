//! `SurfaceId`: the name table an extension's editor half contributes rows
//! and containers into. One name table with middleware — not a second
//! namespace.

use crate::diagnostic::{DiagCode, Diagnostic};
use crate::okf::Bundle;
use crate::view::row::RowTarget;

/// A surface name contributed by an extension's editor half.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SurfaceId(pub String);

impl SurfaceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn markdown() -> Self {
        SurfaceId("markdown".into())
    }

    pub fn source() -> Self {
        SurfaceId("source".into())
    }

    pub fn canvas() -> Self {
        SurfaceId("canvas".into())
    }

    pub fn folder() -> Self {
        SurfaceId("folder".into())
    }
}

/// Document-type default surfaces, by convention with `crate::model::ElementType`:
/// a `uml.*` concept type (UML classifier or behavior) defaults to `"canvas"`;
/// any other concept type defaults to `"markdown"`; a folder target defaults to
/// `"folder"`. Total on real targets: a `Concept` whose id is not in `bundle`
/// still resolves (to `"markdown"`) rather than panicking — the row exists in
/// the projection even if the concept lookup that would refine its surface
/// fails.
pub fn default_surface(target: &RowTarget, bundle: &Bundle) -> SurfaceId {
    match target {
        RowTarget::Concept(id) => {
            let is_diagram_like = bundle
                .concept(id)
                .map(|concept| {
                    matches!(
                        crate::model::ElementType::parse(&concept.ty),
                        crate::model::ElementType::Uml(_)
                            | crate::model::ElementType::Behavior(_)
                            | crate::model::ElementType::Diagram
                    )
                })
                .unwrap_or(false);
            SurfaceId(if is_diagram_like {
                "canvas".to_string()
            } else {
                "markdown".to_string()
            })
        }
        RowTarget::Folder(_) => SurfaceId("folder".to_string()),
        RowTarget::Virtual => SurfaceId("markdown".to_string()),
    }
}

/// Resolves a requested surface against the set an editor build actually
/// registered. A `None` request (no explicit `surface:` override) or a known
/// id resolves silently. An unknown id **degrades to `default` and emits an
/// `UnknownSurface` warning diagnostic** — never a blank tab, never a panic.
///
/// `default` is supplied by the caller rather than derived here: the editor
/// resolves a target's default from the UML analysis' claim set
/// (`waml_editor::documents::default_surface_for`), which this crate cannot
/// consult; a caller with only a `Bundle` passes [`default_surface`].
pub fn resolve_surface(
    requested: Option<&str>,
    default: SurfaceId,
    known: &[&str],
    file: &str,
    line: usize,
) -> (SurfaceId, Option<Diagnostic>) {
    match requested {
        None => (default, None),
        Some(id) if known.contains(&id) => (SurfaceId(id.to_string()), None),
        Some(id) => {
            let diagnostic = Diagnostic::new(
                DiagCode::UnknownSurface,
                format!("unknown surface `{id}`, falling back to `{}`", default.0),
                file,
                line,
            );
            (default, Some(diagnostic))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle_with(id: &str, ty: &str) -> Bundle {
        let concept =
            crate::okf::project(&format!("{id}.md"), &format!("---\ntype: {ty}\n---\n# X\n"))
                .unwrap();
        Bundle::from_parts(vec![concept], vec![], vec![], vec![])
    }

    #[test]
    fn surface_resolution_is_total_for_real_targets() {
        let bundle = bundle_with("order", "uml.Class");
        assert_eq!(
            default_surface(&RowTarget::Concept("order".to_string()), &bundle).0,
            "canvas"
        );

        let markdown_bundle = bundle_with("notes", "note");
        assert_eq!(
            default_surface(&RowTarget::Concept("notes".to_string()), &markdown_bundle).0,
            "markdown"
        );

        let empty = Bundle::default();
        assert_eq!(
            default_surface(&RowTarget::Concept("missing".to_string()), &empty).0,
            "markdown"
        );

        assert_eq!(
            default_surface(&RowTarget::Folder("/archive".to_string()), &empty).0,
            "folder"
        );
    }

    #[test]
    fn an_unknown_surface_id_degrades_to_the_type_default_with_a_diagnostic() {
        let bundle = bundle_with("order", "uml.Class");
        let (surface, diagnostic) = resolve_surface(
            Some("no-such-surface"),
            default_surface(&RowTarget::Concept("order".to_string()), &bundle),
            &["markdown", "canvas", "source", "folder"],
            "index.waml",
            3,
        );
        assert_eq!(surface.0, "canvas");
        let diagnostic = diagnostic.expect("unknown surface must produce a diagnostic");
        assert_eq!(diagnostic.code, DiagCode::UnknownSurface);
        assert_eq!(diagnostic.severity, crate::diagnostic::Severity::Warning);
    }

    #[test]
    fn a_known_requested_surface_resolves_silently() {
        let bundle = bundle_with("notes", "note");
        let (surface, diagnostic) = resolve_surface(
            Some("source"),
            default_surface(&RowTarget::Concept("notes".to_string()), &bundle),
            &["markdown", "canvas", "source", "folder"],
            "index.waml",
            1,
        );
        assert_eq!(surface.0, "source");
        assert!(diagnostic.is_none());
    }
}
