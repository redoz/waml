//! Project-tree observations: WHICH rows the tree is showing, and WHERE it
//! drew them.
//!
//! `ProjectTree::semantic_items` reports one `WamlProjectTreeRow` per
//! laid-out row -- in row order, with the rect clipped to the panel viewport
//! and `visible` false for a row that scrolled out of it. That makes two
//! claims assertable that nothing else in this harness can reach:
//!
//! * **The row list.** A projection that drops rows it should keep produces a
//!   view that still looks like a working tree, only with content missing
//!   (visual sign-off ledger V4: "the failure mode is invisible by
//!   construction"). A human reading pixels has to already know what was
//!   supposed to be there; an exact row-list assertion does not.
//! * **The row geometry.** A row can stay in the model, keep `visible:
//!   true`, and still stop being drawn -- the tree's own history has a header
//!   shrink that blanked every label without removing a row. Only the rect
//!   says so, which is why the list check below also enforces the layout
//!   invariant rather than leaving geometry to a separate operation nobody
//!   calls.
//!
//! Neither claim is about pixels: see the crate docs for the line between
//! what this harness can decide and what still needs eyes.

use crate::error::OperationFailure;
use makepad_test::{TestApp, WidgetSnapshot};

const ROW_TYPE: &str = "WamlProjectTreeRow";

pub(crate) fn expect_project_tree_rows(
    driver: &TestApp,
    expected: &[&str],
) -> Result<String, OperationFailure> {
    let widgets = snapshot(driver)?;
    observe_project_tree_rows(&widgets, expected)
}

pub(crate) fn expect_selected_row(
    driver: &TestApp,
    expected: &str,
) -> Result<String, OperationFailure> {
    let widgets = snapshot(driver)?;
    observe_selected_row(&widgets, expected)
}

fn observe_project_tree_rows(
    widgets: &[WidgetSnapshot],
    expected: &[&str],
) -> Result<String, OperationFailure> {
    let rows = visible_rows(widgets);
    let observed = row_titles(&rows);
    let expected_owned: Vec<String> = expected.iter().map(|title| title.to_string()).collect();
    if observed != expected_owned {
        return Err(OperationFailure {
            observed: format!("the tree shows {}", describe_titles(&observed)),
            detail: format!("expected {}", describe_titles(&expected_owned)),
        });
    }
    observe_rows_laid_out(&rows)?;
    Ok(format!("the tree shows {}", describe_titles(&observed)))
}

/// A visible row must occupy a real rect, and the rows must run strictly
/// down the panel without overlapping. Clipping to the viewport preserves
/// both properties, so a partially-scrolled first or last row still passes.
fn observe_rows_laid_out(rows: &[&WidgetSnapshot]) -> Result<(), OperationFailure> {
    for row in rows {
        if row.width <= 0 || row.height <= 0 {
            return Err(OperationFailure {
                observed: format!(
                    "the {} row is listed but drawn {}x{}",
                    row_title(row),
                    row.width,
                    row.height
                ),
                detail: "a visible tree row must occupy a non-zero rect".to_string(),
            });
        }
    }
    for pair in rows.windows(2) {
        let (above, below) = (pair[0], pair[1]);
        if above.y + above.height > below.y {
            return Err(OperationFailure {
                observed: format!(
                    "the {} row overlaps the {} row",
                    row_title(above),
                    row_title(below)
                ),
                detail: format!(
                    "{} ends at y={} but {} starts at y={}",
                    row_title(above),
                    above.y + above.height,
                    row_title(below),
                    below.y
                ),
            });
        }
    }
    Ok(())
}

fn observe_selected_row(
    widgets: &[WidgetSnapshot],
    expected: &str,
) -> Result<String, OperationFailure> {
    let rows = visible_rows(widgets);
    let selected: Vec<&WidgetSnapshot> = rows
        .iter()
        .copied()
        .filter(|row| row.checked == Some(true))
        .collect();
    match selected.as_slice() {
        [] => Err(OperationFailure {
            observed: "no visible tree row is selected".to_string(),
            detail: format!("expected the {expected} row to be selected and in view"),
        }),
        [row] if row_title(row) == expected => {
            Ok(format!("the {expected} row is selected and in view"))
        }
        [row] => Err(OperationFailure {
            observed: format!("the {} row is selected", row_title(row)),
            detail: format!("expected the {expected} row"),
        }),
        rows => Err(OperationFailure {
            observed: format!("{} visible tree rows are selected", rows.len()),
            detail: "the tree selection is ambiguous".to_string(),
        }),
    }
}

fn visible_rows(widgets: &[WidgetSnapshot]) -> Vec<&WidgetSnapshot> {
    widgets
        .iter()
        .filter(|widget| widget.visible && widget.widget_type == ROW_TYPE)
        .collect()
}

fn row_titles(rows: &[&WidgetSnapshot]) -> Vec<String> {
    rows.iter().copied().map(row_title).collect()
}

fn row_title(row: &WidgetSnapshot) -> String {
    row.text.clone().unwrap_or_else(|| "<untitled>".to_string())
}

fn describe_titles(titles: &[String]) -> String {
    if titles.is_empty() {
        return "no rows".to_string();
    }
    titles.join(", ")
}

fn snapshot(driver: &TestApp) -> Result<Vec<WidgetSnapshot>, OperationFailure> {
    driver
        .try_widget_snapshot()
        .map_err(|error| OperationFailure {
            observed: "the project tree could not be observed".to_string(),
            detail: error.message().to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::{observe_project_tree_rows, observe_selected_row, ROW_TYPE};
    use makepad_test::WidgetSnapshot;

    fn row(title: &str, y: i64, height: i64) -> WidgetSnapshot {
        WidgetSnapshot {
            id: format!("project-tree-row:{title}"),
            widget_type: ROW_TYPE.to_string(),
            window_id: "main".to_string(),
            window_index: 0,
            visible: true,
            enabled: true,
            x: 0,
            y,
            width: 220,
            height,
            text: Some(title.to_string()),
            value: Some(title.to_lowercase()),
            checked: Some(false),
            selected: None,
        }
    }

    fn tree(titles: &[&str]) -> Vec<WidgetSnapshot> {
        titles
            .iter()
            .enumerate()
            .map(|(index, title)| row(title, index as i64 * 24, 24))
            .collect()
    }

    #[test]
    fn the_row_list_is_the_visible_rows_in_draw_order() {
        let widgets = tree(&["Mini", "Order", "Orders"]);

        let observed = observe_project_tree_rows(&widgets, &["Mini", "Order", "Orders"]).unwrap();

        assert_eq!(observed, "the tree shows Mini, Order, Orders");
    }

    #[test]
    fn a_row_a_projection_dropped_is_named_in_the_failure() {
        let widgets = tree(&["Mini", "Orders"]);

        let failure =
            observe_project_tree_rows(&widgets, &["Mini", "Order", "Orders"]).unwrap_err();

        assert_eq!(failure.observed, "the tree shows Mini, Orders");
        assert_eq!(failure.detail, "expected Mini, Order, Orders");
    }

    #[test]
    fn a_scrolled_out_row_is_not_part_of_the_list() {
        let mut widgets = tree(&["Mini", "Order", "Orders"]);
        widgets[2].visible = false;

        observe_project_tree_rows(&widgets, &["Mini", "Order"]).unwrap();
    }

    #[test]
    fn a_row_that_kept_its_entry_but_lost_its_rect_fails_the_list_check() {
        let mut widgets = tree(&["Mini", "Order"]);
        widgets[1].height = 0;

        let failure = observe_project_tree_rows(&widgets, &["Mini", "Order"]).unwrap_err();

        assert_eq!(failure.observed, "the Order row is listed but drawn 220x0");
        assert_eq!(
            failure.detail,
            "a visible tree row must occupy a non-zero rect"
        );
    }

    #[test]
    fn overlapping_rows_fail_the_list_check() {
        let mut widgets = tree(&["Mini", "Order"]);
        widgets[1].y = 8;

        let failure = observe_project_tree_rows(&widgets, &["Mini", "Order"]).unwrap_err();

        assert_eq!(failure.observed, "the Mini row overlaps the Order row");
        assert!(failure.detail.contains("Mini ends at y=24"));
        assert!(failure.detail.contains("Order starts at y=8"));
    }

    #[test]
    fn a_clipped_first_row_still_passes_the_layout_invariant() {
        let mut widgets = tree(&["Mini", "Order"]);
        widgets[0].height = 9;

        observe_project_tree_rows(&widgets, &["Mini", "Order"]).unwrap();
    }

    #[test]
    fn the_selected_row_must_be_the_named_one() {
        let mut widgets = tree(&["Mini", "Order", "Orders"]);
        widgets[1].checked = Some(true);

        let observed = observe_selected_row(&widgets, "Order").unwrap();

        assert_eq!(observed, "the Order row is selected and in view");
        assert_eq!(
            observe_selected_row(&widgets, "Orders").unwrap_err().detail,
            "expected the Orders row"
        );
    }

    #[test]
    fn a_selection_scrolled_out_of_view_reads_as_no_selection() {
        let mut widgets = tree(&["Mini", "Order"]);
        widgets[1].checked = Some(true);
        widgets[1].visible = false;

        let failure = observe_selected_row(&widgets, "Order").unwrap_err();

        assert_eq!(failure.observed, "no visible tree row is selected");
    }

    #[test]
    fn two_selected_rows_are_ambiguous() {
        let mut widgets = tree(&["Mini", "Order"]);
        widgets[0].checked = Some(true);
        widgets[1].checked = Some(true);

        let failure = observe_selected_row(&widgets, "Order").unwrap_err();

        assert_eq!(failure.detail, "the tree selection is ambiguous");
    }
}
