use crate::error::OperationFailure;
use makepad_test::{KeyCode, KeyModifiers, Selector, TestApp, TestError, WidgetSnapshot};

/// `PalettePopup::semantic_items`' widget type: one item per titled section
/// header (`text` = title, `value` = row count).
const PALETTE_SECTION_TYPE: &str = "WamlPaletteSection";
/// `SearchResultsListView::semantic_items`' widget type: one item per
/// document group (`text` = full document path, `value` = row count).
const RESULT_GROUP_TYPE: &str = "WamlSearchGroupHeader";
/// `FindStrip`'s single always-present counter `Label`.
const FIND_COUNTER_ID: &str = "counter_label";
/// `FindStrip`'s query `TextInput`. The readiness gate for "the strip is
/// open" reads THIS, not [`FIND_COUNTER_ID`]: the counter's text is `""`
/// until a query exists (`FindModel::counter_text`), and a `Label` with no
/// text draws no quad, so its snapshot rect stays `0x0` -- and
/// `try_wait_visible` only counts a widget with a non-zero rect. The query
/// input is laid out at a fixed 200px the instant the strip draws.
const FIND_QUERY_INPUT_ID: &str = "query_input";
/// The results tab's surface `View`. Same reason as [`FIND_QUERY_INPUT_ID`]:
/// [`RESULT_GROUP_TYPE`] items come from `semantic_items`, which reports the
/// group list as pure data with a `Rect::default()` -- readable by a
/// snapshot observation, never "visible" to a geometry-gated wait. The
/// surface is hidden and `0x0` until the results tab opens.
const RESULT_SURFACE_ID: &str = "search_results_surface";

/// Ctrl+K (Cmd+K on macOS): open the search palette. Waits for at least one
/// titled section header to become visible -- `open_palette` always seeds
/// the empty-query RECENT section synchronously, so this is present the
/// instant the palette is open, even with an empty recents list (decision 6,
/// spec §Palette).
pub(crate) fn open_search_palette(driver: &TestApp) -> Result<String, OperationFailure> {
    driver
        .try_press_key_with_modifiers(KeyCode::KeyK, primary_modifiers())
        .map_err(|error| palette_driver_failure(driver, &error))?;
    driver
        .locator(Selector::widget_type(PALETTE_SECTION_TYPE))
        .try_wait_visible()
        .map_err(|error| palette_driver_failure(driver, &error))?;
    Ok("the search palette is open".to_string())
}

/// Type into the currently-open palette query (or find strip -- whichever
/// currently owns raw keyboard input). `PalettePopup` consumes `TextInput`
/// events directly while it is the open popup (it is not a real `TextInput`
/// widget), so this sends the same event class the app already handles
/// rather than targeting a widget id.
pub(crate) fn type_search_query(driver: &TestApp, query: &str) -> Result<String, OperationFailure> {
    driver
        .try_type_text(query)
        .map_err(|error| OperationFailure {
            observed: "the query text could not be typed".to_string(),
            detail: error.message().to_string(),
        })?;
    Ok(format!("the query is now \"{query}\""))
}

/// Observe the palette's currently-rendered titled sections against
/// `expected` (title, row count) pairs, in the blended list's own order
/// (CONCEPTS, DOCUMENTS, TEXT, STRUCTURE, RECENT).
pub(crate) fn expect_palette_sections(
    driver: &TestApp,
    expected: &[(&str, usize)],
) -> Result<String, OperationFailure> {
    let widgets = snapshot(driver, "palette section state could not be observed")?;
    observe_palette_sections(&widgets, expected)
}

/// Commit the palette's trailing escalate row, which opens the full results
/// tab (spec: "always the last row for a non-empty query"). One `ArrowUp`
/// from the initial `armed = Some(0)` wraps to the last activatable row
/// (`PalettePopup::move_armed`'s `rem_euclid`) -- since the escalate row's
/// section is always pushed last, its row is always the last entry in
/// `activatable_positions` regardless of how many concept/document/text/
/// structure rows came before it. `Enter` then commits it.
pub(crate) fn escalate_to_results_tab(driver: &TestApp) -> Result<String, OperationFailure> {
    driver
        .try_press_key(KeyCode::ArrowUp)
        .map_err(|error| results_driver_failure(driver, &error))?;
    driver
        .try_press_key(KeyCode::ReturnKey)
        .map_err(|error| results_driver_failure(driver, &error))?;
    driver
        .locator(Selector::id(RESULT_SURFACE_ID))
        .try_wait_visible()
        .map_err(|error| results_driver_failure(driver, &error))?;
    Ok("the results tab is active".to_string())
}

/// Observe the results tab's document groups against `expected` (full
/// document path, row count) pairs, in rank order.
pub(crate) fn expect_results_grouped_by_document(
    driver: &TestApp,
    expected: &[(&str, usize)],
) -> Result<String, OperationFailure> {
    let widgets = snapshot(driver, "search results state could not be observed")?;
    observe_results_grouped_by_document(&widgets, expected)
}

/// Ctrl+F (Cmd+F on macOS): open the find-in-document strip. Waits for its
/// query input to become visible -- the first laid-out thing the strip draws
/// once `FindStrip::open` flips its own `visible` flag (see
/// [`FIND_QUERY_INPUT_ID`] for why the counter label cannot serve as the
/// gate).
pub(crate) fn open_find_strip(driver: &TestApp) -> Result<String, OperationFailure> {
    driver
        .try_press_key_with_modifiers(KeyCode::KeyF, primary_modifiers())
        .map_err(|error| find_strip_driver_failure(&error))?;
    driver
        .locator(Selector::id(FIND_QUERY_INPUT_ID))
        .try_wait_visible()
        .map_err(|error| find_strip_driver_failure(&error))?;
    Ok("the find strip is open".to_string())
}

/// Observe the find strip's `"{n} of {total}"` counter reading.
pub(crate) fn expect_find_counter(
    driver: &TestApp,
    expected: &str,
) -> Result<String, OperationFailure> {
    let widgets = snapshot(driver, "find strip counter state could not be observed")?;
    observe_find_counter(&widgets, expected)
}

fn observe_palette_sections(
    widgets: &[WidgetSnapshot],
    expected: &[(&str, usize)],
) -> Result<String, OperationFailure> {
    let observed = palette_section_pairs(widgets);
    let expected_owned = owned_pairs(expected);
    if observed != expected_owned {
        return Err(OperationFailure {
            observed: format!("palette sections are {}", describe_pairs(&observed)),
            detail: format!(
                "expected palette sections {}",
                describe_pairs(&expected_owned)
            ),
        });
    }
    Ok(format!(
        "palette sections are {}",
        describe_pairs(&observed)
    ))
}

fn observe_results_grouped_by_document(
    widgets: &[WidgetSnapshot],
    expected: &[(&str, usize)],
) -> Result<String, OperationFailure> {
    let observed = result_group_pairs(widgets);
    let expected_owned = owned_pairs(expected);
    if observed != expected_owned {
        return Err(OperationFailure {
            observed: format!("results are grouped as {}", describe_pairs(&observed)),
            detail: format!(
                "expected results grouped as {}",
                describe_pairs(&expected_owned)
            ),
        });
    }
    Ok(format!(
        "results are grouped as {}",
        describe_pairs(&observed)
    ))
}

fn observe_find_counter(
    widgets: &[WidgetSnapshot],
    expected: &str,
) -> Result<String, OperationFailure> {
    let counters: Vec<_> = widgets
        .iter()
        .filter(|widget| widget.visible && widget.id == FIND_COUNTER_ID)
        .collect();
    let counter = match counters.as_slice() {
        [] => {
            return Err(OperationFailure {
                observed: "the find strip counter is not visible".to_string(),
                detail: "expected the find strip to be open".to_string(),
            })
        }
        [counter] => *counter,
        counters => {
            return Err(OperationFailure {
                observed: format!("{} visible find strip counters exist", counters.len()),
                detail: "the find strip counter is ambiguous".to_string(),
            })
        }
    };
    match counter.text.as_deref() {
        Some(actual) if actual == expected => Ok(format!("the find counter reads \"{expected}\"")),
        Some(actual) => Err(OperationFailure {
            observed: format!("the find counter reads \"{actual}\""),
            detail: format!("expected \"{expected}\""),
        }),
        None => Err(OperationFailure {
            observed: "the find strip counter has no text".to_string(),
            detail: format!("expected \"{expected}\""),
        }),
    }
}

fn palette_section_pairs(widgets: &[WidgetSnapshot]) -> Vec<(String, usize)> {
    semantic_pairs(widgets, PALETTE_SECTION_TYPE)
}

fn result_group_pairs(widgets: &[WidgetSnapshot]) -> Vec<(String, usize)> {
    semantic_pairs(widgets, RESULT_GROUP_TYPE)
}

/// Every visible `widget_type` entry's `(text, value-as-count)` pair, in
/// snapshot order -- the collector appends a widget's `semantic_items` right
/// after the widget's own entry (see `WidgetTree::snapshot`), so this
/// preserves the section/group's own build order.
fn semantic_pairs(widgets: &[WidgetSnapshot], widget_type: &str) -> Vec<(String, usize)> {
    widgets
        .iter()
        .filter(|widget| widget.visible && widget.widget_type == widget_type)
        .map(|widget| {
            let label = widget.text.clone().unwrap_or_default();
            let count = widget
                .value
                .as_deref()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0);
            (label, count)
        })
        .collect()
}

fn owned_pairs(pairs: &[(&str, usize)]) -> Vec<(String, usize)> {
    pairs
        .iter()
        .map(|(label, count)| (label.to_string(), *count))
        .collect()
}

fn describe_pairs(pairs: &[(String, usize)]) -> String {
    if pairs.is_empty() {
        return "<none>".to_string();
    }
    pairs
        .iter()
        .map(|(label, count)| format!("{label} \u{00B7} {count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn snapshot(
    driver: &TestApp,
    observed: impl Into<String>,
) -> Result<Vec<WidgetSnapshot>, OperationFailure> {
    driver
        .try_widget_snapshot()
        .map_err(|error| OperationFailure {
            observed: observed.into(),
            detail: error.message().to_string(),
        })
}

fn palette_driver_failure(driver: &TestApp, error: &TestError) -> OperationFailure {
    let observed = driver
        .try_widget_snapshot()
        .ok()
        .map(|widgets| {
            format!(
                "palette sections are {}",
                describe_pairs(&palette_section_pairs(&widgets))
            )
        })
        .unwrap_or_else(|| "palette state could not be observed".to_string());
    OperationFailure {
        observed,
        detail: error.message().to_string(),
    }
}

fn results_driver_failure(driver: &TestApp, error: &TestError) -> OperationFailure {
    let observed = driver
        .try_widget_snapshot()
        .ok()
        .map(|widgets| {
            format!(
                "results are grouped as {}",
                describe_pairs(&result_group_pairs(&widgets))
            )
        })
        .unwrap_or_else(|| "search results state could not be observed".to_string());
    OperationFailure {
        observed,
        detail: error.message().to_string(),
    }
}

fn find_strip_driver_failure(error: &TestError) -> OperationFailure {
    OperationFailure {
        observed: "find strip state could not be observed".to_string(),
        detail: error.message().to_string(),
    }
}

#[cfg(target_vendor = "apple")]
fn primary_modifiers() -> KeyModifiers {
    KeyModifiers {
        logo: true,
        ..Default::default()
    }
}

#[cfg(not(target_vendor = "apple"))]
fn primary_modifiers() -> KeyModifiers {
    KeyModifiers {
        control: true,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn semantic_item(
        id: &str,
        widget_type: &str,
        text: &str,
        value: &str,
        visible: bool,
    ) -> WidgetSnapshot {
        WidgetSnapshot {
            id: id.to_string(),
            widget_type: widget_type.to_string(),
            window_id: "main_window".to_string(),
            window_index: 0,
            visible,
            enabled: true,
            x: 0,
            y: 0,
            width: 300,
            height: 22,
            text: Some(text.to_string()),
            value: Some(value.to_string()),
            checked: None,
            selected: None,
        }
    }

    fn palette_section(title: &str, count: usize, visible: bool) -> WidgetSnapshot {
        semantic_item(
            &format!("palette-section:{title}"),
            PALETTE_SECTION_TYPE,
            title,
            &count.to_string(),
            visible,
        )
    }

    fn result_group(path: &str, count: usize, visible: bool) -> WidgetSnapshot {
        semantic_item(
            &format!("search-group-header:{path}"),
            RESULT_GROUP_TYPE,
            path,
            &count.to_string(),
            visible,
        )
    }

    fn counter(text: &str, visible: bool) -> WidgetSnapshot {
        let mut widget = semantic_item(FIND_COUNTER_ID, "Label", text, "", visible);
        widget.value = None;
        widget
    }

    #[test]
    fn palette_sections_match_titles_and_counts_in_order() {
        let widgets = vec![
            palette_section("CONCEPTS", 3, true),
            palette_section("TEXT", 2, true),
        ];

        let observed = observe_palette_sections(&widgets, &[("CONCEPTS", 3), ("TEXT", 2)]).unwrap();

        assert_eq!(
            observed,
            "palette sections are CONCEPTS \u{00B7} 3, TEXT \u{00B7} 2"
        );
    }

    #[test]
    fn palette_sections_ignore_hidden_entries() {
        let widgets = vec![
            palette_section("CONCEPTS", 3, true),
            palette_section("STALE", 9, false),
        ];

        let observed = observe_palette_sections(&widgets, &[("CONCEPTS", 3)]).unwrap();

        assert_eq!(observed, "palette sections are CONCEPTS \u{00B7} 3");
    }

    #[test]
    fn palette_sections_mismatch_reports_both_sides() {
        let widgets = vec![palette_section("CONCEPTS", 3, true)];

        let error = observe_palette_sections(&widgets, &[("CONCEPTS", 5)]).unwrap_err();

        assert_eq!(error.observed, "palette sections are CONCEPTS \u{00B7} 3");
        assert_eq!(
            error.detail,
            "expected palette sections CONCEPTS \u{00B7} 5"
        );
    }

    #[test]
    fn no_palette_sections_report_none() {
        let error = observe_palette_sections(&[], &[("CONCEPTS", 3)]).unwrap_err();

        assert_eq!(error.observed, "palette sections are <none>");
    }

    #[test]
    fn result_groups_match_document_paths_and_counts_in_order() {
        let widgets = vec![
            result_group("billing.waml", 1, true),
            result_group("guides/checkout.md", 2, true),
        ];

        let observed = observe_results_grouped_by_document(
            &widgets,
            &[("billing.waml", 1), ("guides/checkout.md", 2)],
        )
        .unwrap();

        assert_eq!(
            observed,
            "results are grouped as billing.waml \u{00B7} 1, guides/checkout.md \u{00B7} 2"
        );
    }

    #[test]
    fn result_groups_mismatch_reports_both_sides() {
        let widgets = vec![result_group("billing.waml", 1, true)];

        let error =
            observe_results_grouped_by_document(&widgets, &[("billing.waml", 2)]).unwrap_err();

        assert_eq!(
            error.observed,
            "results are grouped as billing.waml \u{00B7} 1"
        );
        assert_eq!(
            error.detail,
            "expected results grouped as billing.waml \u{00B7} 2"
        );
    }

    #[test]
    fn find_counter_matches_the_expected_reading() {
        let widgets = vec![counter("3 of 12", true)];

        let observed = observe_find_counter(&widgets, "3 of 12").unwrap();

        assert_eq!(observed, "the find counter reads \"3 of 12\"");
    }

    #[test]
    fn find_counter_reports_a_mismatch() {
        let widgets = vec![counter("1 of 12", true)];

        let error = observe_find_counter(&widgets, "3 of 12").unwrap_err();

        assert_eq!(error.observed, "the find counter reads \"1 of 12\"");
        assert_eq!(error.detail, "expected \"3 of 12\"");
    }

    #[test]
    fn find_counter_missing_is_reported() {
        let widgets = vec![counter("1 of 12", false)];

        let error = observe_find_counter(&widgets, "3 of 12").unwrap_err();

        assert_eq!(error.observed, "the find strip counter is not visible");
    }

    #[test]
    fn find_counter_ambiguous_is_reported() {
        let widgets = vec![counter("1 of 12", true), counter("2 of 12", true)];

        let error = observe_find_counter(&widgets, "3 of 12").unwrap_err();

        assert_eq!(error.observed, "2 visible find strip counters exist");
    }
}
