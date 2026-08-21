//! A semantic driver for the WAML editor's headless UI scenarios.
//!
//! A scenario names domain operations on [`WamlApp`] and nothing else -- no
//! widget ids, coordinates, sleeps or timeouts. Every operation is recorded
//! to a semantic trace as `operation / expected / observed`, so a failure
//! reads as a sentence about the editor rather than a selector that missed.
//! `crates/waml-editor/tests/README.md` is the how-to-run; this is the
//! what-it-is-for and, more usefully, the what-it-cannot-do.
//!
//! # Shape
//!
//! ```text
//! scenario  #[waml_ui_test]  ->  WamlApp  ->  adapters  ->  makepad_test
//!           (macro)              (verbs)      (observe)     (real editor
//!                                                            process)
//! ```
//!
//! `run` stages a run-owned copy of the fixture, allocates an isolated run
//! directory under `target/waml-ui-test/`, launches a real headless editor
//! against it, and preserves the whole directory (trace, logs, widget dump,
//! screenshot) when the scenario fails. `adapters` turn the driver's widget
//! snapshot into domain observations; each is a pure function of
//! `&[WidgetSnapshot]`, unit-tested offline, so a new observation can be
//! trusted before it is ever run against a live app.
//!
//! # What it costs
//!
//! About 2,600 lines of harness, plus about 1,700 more that are the
//! harness's own unit tests -- so a line count of this crate roughly doubles
//! what the harness itself weighs. None of it is per-scenario cost: it is
//! fixed, already paid, and a scenario is about ten lines. The expensive part
//! is not the harness but the *adapters* -- reaching a surface the harness
//! has never touched means working out what that surface exposes, and
//! sometimes teaching the editor to expose it. Which is why the count of
//! scenarios is a poor measure of whether this crate is earning its keep.
//!
//! If someone does come here to cut, `trace` is the place to look first and
//! the rest is not. It spends about 400 lines (code and tests together) on a
//! two-file atomic publish -- prepare, back up, publish, roll back, four
//! phases across a `.txt` and a `.json` that say the same thing -- re-run on
//! every step of every scenario, to make a *debug artifact* crash-durable.
//! Two plain writes would lose only the case where the process dies between
//! them, leaving a stale JSON beside a fresh text file. That is a reviewed
//! decision to make deliberately, not a drive-by: this is the failure-
//! evidence path, and this project's UI failures have historically been hard
//! to diagnose. The staging and cleanup paranoia in `fixture`, by contrast,
//! is guarding a recursive delete and should stay exactly as paranoid as it
//! is.
//!
//! # What it can decide
//!
//! Anything the editor reports through a widget snapshot: which rows, tabs
//! and surfaces exist, their text, value, enabled/checked state, and their
//! rects. Plus input: clicks, key chords, typed text. So it can settle
//! questions of *state* and *layout* -- "is the Orders diagram active", "does
//! this query hit these four sections", "did the tree lose a row", "is a row
//! drawn on top of another".
//!
//! Hand-drawn rows (`ProjectTree`, `PalettePopup`, `SearchResultsListView`)
//! reach the snapshot through `semantic_items`. Two traps follow, both
//! already paid for in blood: an item reported with a `Rect::default()` can
//! be *read* but never *waited on* (`try_wait_visible` requires a non-zero
//! rect), and an empty `Label` draws no quad, so it also stays `0x0`. Gate a
//! wait on a real laid-out widget; assert on the semantic item.
//!
//! # What it cannot decide
//!
//! Pixels. It can capture a screenshot on failure, but it has no baseline to
//! compare one against, and it should not grow one casually: the editor draws
//! through SDF shaders whose output is not bit-stable across a GL driver
//! change, so a naive golden set would be a flake generator. Concretely, this
//! harness cannot answer:
//!
//! * stroke weight, colour, glyph identity, or anything else about how a
//!   shape was drawn (visual sign-off ledger V1, V3, V10, V14);
//! * whether text at a new scale is *legible*, only whether it changed size
//!   (V2);
//! * hit-testing against anything that is drawn rather than laid out as a
//!   widget -- a glyph trash can or a drawn pin has no snapshot entry, which
//!   is exactly the misalignment those rows are about (V8, V11);
//! * temporal feel: animation, gesture inertia, drag preview (V9).
//!
//! CI runs it headless on Linux, which is the verification of record. A
//! Windows developer can run it too -- see the tests README for the prebuild
//! the driver needs first.
//!
//! When a ledger row is *state*, not pixels, it belongs here rather than in a
//! human's queue. [`WamlApp::expect_project_tree_rows`] exists for exactly
//! that reason: a projection that silently drops rows produces a view that
//! looks like it is working, which no amount of looking reliably catches.

mod adapters;
mod app;
mod config;
mod domain;
mod error;
mod fixture;
mod run;
mod trace;

pub use app::WamlApp;
pub use config::{ScenarioConfig, WorkspaceFixture};
pub use domain::{DiagramName, ViewKind};
pub use error::WamlUiError;
pub use waml_ui_test_macros::waml_ui_test;

#[doc(hidden)]
pub mod __private {
    pub fn run_catalog_test(
        manifest_dir: &'static str,
        package_name: &'static str,
        module_path: &'static str,
        test_name: &'static str,
        workspace: crate::WorkspaceFixture,
        test: impl FnOnce(crate::WamlApp),
    ) {
        let scenario = crate::ScenarioConfig {
            package_name,
            manifest_dir,
            module_path,
            test_name,
            workspace,
        };
        crate::run::run_scenario(scenario, test);
    }
}

#[cfg(test)]
mod tests {
    use super::{__private::run_catalog_test, WorkspaceFixture};

    type CatalogRunner = fn(
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        WorkspaceFixture,
        fn(super::WamlApp),
    );

    #[test]
    fn catalog_runner_is_available_to_macro_expansions() {
        let _runner: CatalogRunner = run_catalog_test;
        let _fixture = WorkspaceFixture::Mini;
    }
}
