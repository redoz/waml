//! Opening a folder tab -- and the record of why the rest of navigation is
//! out of reach.
//!
//! [`super::surfaces`] can say which surface owns the centre, but only if
//! something drives a route across a surface boundary first. Clicking a
//! directory row in the project tree is that something, and it is the only
//! one this harness has.
//!
//! # The caption band does not lay out under the headless backend
//!
//! This is worth stating plainly, because it looks like a missing operation
//! and is not. The editor's chrome is a custom caption bar
//! (`main_window.caption_bar`), and under the headless backend it never
//! draws: `caption_col`, `title_row`, `doc_tabs`, the burger, the search
//! button and both history arrows all appear in the widget snapshot with
//! `visible: true` and a rect of `0x0`, while `center_column` starts at
//! `y = 0` -- the band occupies no height at all. A locator refuses a widget
//! with no rect, so every control mounted in the caption is unclickable
//! here, however visible it claims to be.
//!
//! Two consequences, both of them ledger rows:
//!
//! * **View history cannot be traversed** (V5). Back and forward have
//!   exactly two triggers -- the caption's arrow pair, and the mouse's
//!   fourth/fifth buttons handled app-wide in
//!   `App::handle_global_shortcuts`. The first is in the dead band; the
//!   second is a button class the driver cannot send. There is no third way
//!   in, so the traversal half of V5 needs a human.
//! * **The tab strip is invisible twice over** (V5, V15). `DocTabs` draws
//!   its tabs into its own `tab_rects` rather than mounting child widgets
//!   and exposes no `semantic_items`, so even in a laid-out caption there
//!   would be nothing for a selector to resolve. "Which tab is active" has
//!   no snapshot entry; the surface the centre is showing is the only
//!   observable for where a route landed.

use crate::domain::DocumentSurface;
use crate::error::OperationFailure;
use makepad_test::{Selector, TestApp, TestError};

const ROW_TYPE: &str = "WamlProjectTreeRow";

/// Click a project-tree row that names a directory, opening that directory's
/// own tab, and settle the centre on the folder listing.
///
/// A directory row reports `enabled: false` in the snapshot -- `openable` on
/// a tree node means "opens a DOCUMENT", and a directory does not -- but it
/// is still the click target that routes to the folder view:
/// `tree_panel::row_navigation` returns a `Directory` target for a directory
/// row before it ever consults `openable`. Clicking does not require
/// `enabled`, so this works; asserting on `enabled` here would not.
pub(crate) fn open_folder_tab(driver: &TestApp, title: &str) -> Result<String, OperationFailure> {
    let row = driver.locator(Selector::widget_type(ROW_TYPE).text_exact(title));
    row.try_click()
        .map_err(|error| row_driver_failure(title, error))?;
    super::surfaces::expect_active_surface(driver, DocumentSurface::Folder)?;
    Ok(format!(
        "the {title} folder tab is open on the folder listing"
    ))
}

fn row_driver_failure(title: &str, error: TestError) -> OperationFailure {
    OperationFailure {
        observed: format!("the {title} row could not be clicked"),
        detail: error.message().to_string(),
    }
}
