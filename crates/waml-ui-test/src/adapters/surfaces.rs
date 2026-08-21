//! Which document surface owns the centre -- and, just as much, that only
//! one of them does.
//!
//! The editor's centre is a stack of seven mutually-exclusive `View`s
//! (`DocView::DOCUMENT_SURFACES`), and every `show_*` on it is "mine on, my
//! siblings off". That rule is only as good as its list, and the list has
//! already been wrong: each `show_*` used to carry a hand-copied copy of it,
//! five of the copies never learned about `behavior_canvas_wrap`, and
//! switching away from an activity/state-machine/sequence tab left the
//! behavior canvas drawing underneath whatever replaced it. Nothing failed.
//! The app was in a valid-looking state, the new surface was up, and the
//! stale one was simply also up, behind it.
//!
//! That is the failure this module exists to catch, and it is why
//! [`observe_active_surface`] asserts a *set* rather than a widget: not "is
//! the folder view showing" but "is the folder view the ONLY thing showing".
//! A leak of this kind cannot be seen from a passing surface check, and on a
//! headless capture it cannot be seen at all -- the occluded surface is
//! behind the live one.
//!
//! It is state, not pixels. What is on the centre, and what is not, is a
//! question the widget snapshot answers exactly; whether what is drawn there
//! reads well is not (see the crate docs for the standing line).
//!
//! `diagram_properties_wrap` is deliberately absent from the list, for the
//! same reason `DocView::DOCUMENT_SURFACES` leaves it out: it is a companion
//! panel of the class canvas, shown and hidden per mode by
//! `ClassDiagramView::sync`, not a sibling competing for the centre.

use crate::domain::DocumentSurface;
use crate::error::OperationFailure;
use makepad_test::{TestApp, WidgetSnapshot};

/// How long [`expect_active_surface`] keeps re-reading before it calls the
/// centre settled on the wrong thing. Matches `documents`' own per-view
/// budget, and for the same reason: a route across surfaces is driven by a
/// click, and the swap lands a frame or more later.
const SURFACE_SETTLE: std::time::Duration = std::time::Duration::from_secs(30);
const SURFACE_POLL: std::time::Duration = std::time::Duration::from_millis(50);

/// Every surface that competes for the centre, in the order
/// `DocView::DOCUMENT_SURFACES` lists them.
const SURFACES: [(DocumentSurface, &str); 7] = [
    (DocumentSurface::Canvas, "canvas_wrap"),
    (DocumentSurface::BehaviorCanvas, "behavior_canvas_wrap"),
    (DocumentSurface::Source, "markdown_surface"),
    (DocumentSurface::Reading, "markdown_viewer_surface"),
    (DocumentSurface::Folder, "folder_view_surface"),
    (DocumentSurface::Results, "search_results_surface"),
    (DocumentSurface::Book, "book_surface"),
];

pub(crate) fn surface_id(surface: DocumentSurface) -> &'static str {
    SURFACES
        .iter()
        .find(|(candidate, _)| *candidate == surface)
        .map(|(_, id)| *id)
        .expect("every DocumentSurface is listed in SURFACES")
}

/// Settle the centre on `expected`, then hold it to the exclusivity rule.
///
/// Polls rather than waiting on a locator because the claim is about the
/// whole set: a locator wait on the surface that should appear says nothing
/// about the one that should have gone away. Every snapshot query pumps the
/// UI, so the poll is also what advances the app.
pub(crate) fn expect_active_surface(
    driver: &TestApp,
    expected: DocumentSurface,
) -> Result<String, OperationFailure> {
    let deadline = std::time::Instant::now() + SURFACE_SETTLE;
    loop {
        let failure = match driver.try_widget_snapshot() {
            Ok(widgets) => match observe_active_surface(&widgets, expected) {
                Ok(observed) => return Ok(observed),
                Err(failure) => failure,
            },
            Err(error) => OperationFailure {
                observed: "the document surfaces could not be observed".to_string(),
                detail: error.message().to_string(),
            },
        };
        if std::time::Instant::now() >= deadline {
            return Err(OperationFailure {
                observed: failure.observed,
                detail: format!(
                    "{}; the centre did not settle on the {} surface within {SURFACE_SETTLE:?}",
                    failure.detail,
                    expected.description()
                ),
            });
        }
        std::thread::sleep(SURFACE_POLL);
    }
}

/// Pure half of [`expect_active_surface`]: one verdict on one snapshot.
fn observe_active_surface(
    widgets: &[WidgetSnapshot],
    expected: DocumentSurface,
) -> Result<String, OperationFailure> {
    let mut showing = Vec::new();
    for (surface, id) in SURFACES {
        if resolve_surface(widgets, surface, id)?.visible {
            showing.push(surface);
        }
    }
    match showing.as_slice() {
        [] => Err(OperationFailure {
            observed: "no document surface is showing".to_string(),
            detail: format!("expected the {} surface", expected.description()),
        }),
        [only] if *only == expected => {
            let surface = resolve_surface(widgets, expected, surface_id(expected))?;
            if surface.width <= 0 || surface.height <= 0 {
                return Err(OperationFailure {
                    observed: format!(
                        "the {} surface is showing but drawn {}x{}",
                        expected.description(),
                        surface.width,
                        surface.height
                    ),
                    detail: "the surface holding the centre must occupy a non-zero rect"
                        .to_string(),
                });
            }
            Ok(format!(
                "the {} surface has the centre to itself",
                expected.description()
            ))
        }
        [only] => Err(OperationFailure {
            observed: format!("the {} surface is showing", only.description()),
            detail: format!("expected the {} surface", expected.description()),
        }),
        showing => Err(OperationFailure {
            observed: format!("{} document surfaces are showing at once", showing.len()),
            detail: format!(
                "expected only the {} surface; found {}",
                expected.description(),
                describe_surfaces(showing)
            ),
        }),
    }
}

fn resolve_surface<'a>(
    widgets: &'a [WidgetSnapshot],
    surface: DocumentSurface,
    id: &str,
) -> Result<&'a WidgetSnapshot, OperationFailure> {
    let matches: Vec<_> = widgets.iter().filter(|widget| widget.id == id).collect();
    match matches.as_slice() {
        [] => Err(OperationFailure {
            observed: format!("the {} surface is missing", surface.description()),
            detail: format!("expected exactly one `{id}` in the widget tree"),
        }),
        [only] => Ok(only),
        matches => Err(OperationFailure {
            observed: format!(
                "{} widgets answer to the {} surface",
                matches.len(),
                surface.description()
            ),
            detail: format!("expected exactly one `{id}` in the widget tree"),
        }),
    }
}

fn describe_surfaces(surfaces: &[DocumentSurface]) -> String {
    surfaces
        .iter()
        .map(|surface| surface.description().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::{observe_active_surface, surface_id, SURFACES};
    use crate::domain::DocumentSurface;
    use makepad_test::WidgetSnapshot;

    fn surface(id: &str, visible: bool) -> WidgetSnapshot {
        WidgetSnapshot {
            id: id.to_string(),
            widget_type: "View".to_string(),
            window_id: "main".to_string(),
            window_index: 0,
            visible,
            enabled: true,
            x: 0,
            y: 0,
            width: 1200,
            height: 800,
            text: None,
            value: None,
            checked: None,
            selected: None,
        }
    }

    /// Every surface present, only `showing` visible.
    fn centre(showing: &[DocumentSurface]) -> Vec<WidgetSnapshot> {
        SURFACES
            .iter()
            .map(|(candidate, id)| surface(id, showing.contains(candidate)))
            .collect()
    }

    #[test]
    fn the_one_visible_surface_is_the_active_one() {
        let widgets = centre(&[DocumentSurface::Folder]);

        let observed = observe_active_surface(&widgets, DocumentSurface::Folder).unwrap();

        assert_eq!(
            observed,
            "the folder listing surface has the centre to itself"
        );
    }

    #[test]
    fn a_stale_surface_left_showing_underneath_fails_even_though_the_new_one_is_up() {
        let widgets = centre(&[DocumentSurface::BehaviorCanvas, DocumentSurface::Source]);

        let failure = observe_active_surface(&widgets, DocumentSurface::Source).unwrap_err();

        assert_eq!(failure.observed, "2 document surfaces are showing at once");
        assert!(failure.detail.contains("behavior canvas"));
        assert!(failure.detail.contains("raw source"));
    }

    #[test]
    fn the_wrong_surface_is_named_rather_than_counted() {
        let widgets = centre(&[DocumentSurface::Canvas]);

        let failure = observe_active_surface(&widgets, DocumentSurface::Folder).unwrap_err();

        assert_eq!(failure.observed, "the diagram canvas surface is showing");
        assert_eq!(failure.detail, "expected the folder listing surface");
    }

    #[test]
    fn an_empty_centre_is_not_a_surface() {
        let widgets = centre(&[]);

        let failure = observe_active_surface(&widgets, DocumentSurface::Canvas).unwrap_err();

        assert_eq!(failure.observed, "no document surface is showing");
    }

    #[test]
    fn a_surface_that_kept_its_entry_but_lost_its_rect_fails() {
        let mut widgets = centre(&[DocumentSurface::Book]);
        let book = surface_id(DocumentSurface::Book);
        let entry = widgets
            .iter_mut()
            .find(|widget| widget.id == book)
            .expect("the book surface is in the centre");
        entry.height = 0;

        let failure = observe_active_surface(&widgets, DocumentSurface::Book).unwrap_err();

        assert_eq!(
            failure.observed,
            "the book surface is showing but drawn 1200x0"
        );
    }

    /// A surface missing from the tree is a broken mount, not an absent
    /// surface -- reporting it as "not showing" would let a scenario pass
    /// against a centre that lost a sibling entirely.
    #[test]
    fn a_surface_missing_from_the_tree_is_reported_as_missing() {
        let book = surface_id(DocumentSurface::Book);
        let widgets: Vec<_> = centre(&[DocumentSurface::Canvas])
            .into_iter()
            .filter(|widget| widget.id != book)
            .collect();

        let failure = observe_active_surface(&widgets, DocumentSurface::Canvas).unwrap_err();

        assert_eq!(failure.observed, "the book surface is missing");
        assert!(failure.detail.contains("book_surface"));
    }

    #[test]
    fn every_surface_has_an_id() {
        for (surface, id) in SURFACES {
            assert_eq!(surface_id(surface), id);
        }
        assert_eq!(SURFACES.len(), 7);
    }
}
