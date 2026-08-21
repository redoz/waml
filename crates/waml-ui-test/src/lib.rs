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
//! drawn on top of another", "is this surface the only one showing".
//!
//! That last one is [`WamlApp::expect_active_surface`], and it is worth
//! calling out because it asserts a SET rather than a widget. The centre is
//! seven mutually-exclusive surfaces and every `show_*` on it is "mine on,
//! my siblings off"; the siblings half has already failed silently, when
//! each `show_*` carried a hand-copied surface list, five of the copies
//! never learned about the behavior canvas, and it went on drawing
//! underneath whatever replaced it. An occluded surface is
//! behind the live one, so that class of bug is invisible to a screenshot as
//! well as to a human -- but it is trivially visible to a set assertion.
//!
//! Hand-drawn rows (`ProjectTree`, `PalettePopup`, `SearchResultsListView`)
//! reach the snapshot through `semantic_items`. Two traps follow, both
//! already paid for in blood: an item reported with a `Rect::default()` can
//! be *read* but never *waited on* (`try_wait_visible` requires a non-zero
//! rect), and an empty `Label` draws no quad, so it also stays `0x0`. Gate a
//! wait on a real laid-out widget; assert on the semantic item.
//!
//! # Four walls, and they are not about pixels
//!
//! Reaching a surface this harness has never touched sometimes fails for a
//! reason that has nothing to do with what a snapshot can express. Four of
//! those are known, all found by trying:
//!
//! * **The caption band does not lay out under the headless backend.**
//!   `caption_col`, `title_row`, `doc_tabs`, the burger, the search button,
//!   the `[T]` tree toggle and both history arrows all report
//!   `visible: true` with a `0x0` rect, and `center_column` starts at
//!   `y = 0` -- the band has no height at all. A locator refuses a widget
//!   with no rect, so nothing mounted in the caption can be clicked. View
//!   history is the casualty: its only two triggers are that arrow pair and
//!   the mouse's fourth/fifth buttons, which the driver cannot send.
//! * **`FlatList` pools its items out of the window.** A pooled item reaches
//!   the snapshot with an empty `window_id` and a `0x0` rect, so the results
//!   tab's rows are real widgets that still cannot be clicked.
//! * **Popup surfaces draw their rows and offer no keyboard path.**
//!   `MenuPopup` and the conflict list mount no children and expose no
//!   `semantic_items`. `PalettePopup` is the exception, and being
//!   keyboard-driven is exactly why search is the most automated area here.
//! * **There is no resize operation**, so narrow mode (under 640px) is
//!   unreachable.
//!
//! `docs/reviews/visual-signoff-ledger.md` records which of its rows sit
//! behind each of these, so nobody re-discovers them one scenario at a time.
//!
//! # The rendering gate, and how narrow it is
//!
//! One operation does look at pixels:
//! [`WamlApp::expect_canvas_matches_reference`], used by two scenarios --
//! one behavior canvas and one class canvas. It compares the diagram
//! canvas against a stored reference -- but by INK, not by value. Each pixel
//! reduces to "is this the background or not", and the comparison is between
//! those masks. `reference`'s module docs carry the full argument; the short
//! version is that the SDF antialias ramp, the JIT-compiled shaders' host
//! toolchain, per-zoom text rasterisation and the pen quantiser all move
//! pixel VALUES, and none of them moves whether a pixel has ink in it. A
//! value comparison would be a flake generator. A mask comparison settles
//! geometry.
//!
//! So it can now decide: where connectors run, how thick a stroke is once
//! quantised, where glyphs sit and how big they are. `regression_proof`
//! demonstrates that on the change it was built for -- `90ffcf0f`'s router
//! fix moved a back edge 8px and a self-loop 8px, and the gate fails on that
//! by six times its tolerance.
//!
//! Two hard limits on it, both structural:
//!
//! * **Linux is the platform of record**, because the fork's headless shader
//!   loader is unix-only. On Windows every shader compiles, none loads, and
//!   the capture is a flat rectangle -- which is also why a Windows run's
//!   "failure screenshot" has never been worth opening. The gate detects that
//!   and says so rather than passing quietly.
//! * **A reference is per platform.** With none for the current platform the
//!   gate records one and passes advisory, naming the file to commit.
//!
//! # What it still cannot decide
//!
//! * colour, and antialias quality -- an ink mask throws exactly those away
//!   (visual sign-off ledger V1's colour half, and any "does this read as
//!   crisp" question);
//! * whether text at a new scale is *legible*, only that its ink moved (V2);
//! * whether a connector's route is GOOD rather than merely different -- the
//!   gate notices the change and hands a human the two drawings; the taste
//!   call is still theirs (V10, V14);
//! * hit-testing against anything drawn rather than laid out as a widget -- a
//!   glyph trash can or a drawn pin has no snapshot entry, which is exactly
//!   the misalignment those rows are about (V8, V11);
//! * temporal feel: animation, gesture inertia, drag preview (V9);
//! * anything outside the canvas rect. The comparison is cropped to the
//!   diagram surface on purpose, so no chrome, tab, tree or panel is gated.
//!
//! CI runs the whole suite headless on Linux, which is the verification of
//! record. A Windows developer can run it too -- see the tests README for the
//! prebuild the driver needs first -- and gets every scenario except the
//! rendering gate, which reports itself as not run.
//!
//! When a ledger row is *state*, not pixels, it belongs here rather than in a
//! human's queue. [`WamlApp::expect_project_tree_rows`] exists for exactly
//! that reason: a projection that silently drops rows produces a view that
//! looks like it is working, which no amount of looking reliably catches.
//!
//! One thing a scenario can never do for such a row, though, and the ledger
//! says it too: every obligation there is "look at whether this shipped
//! correctly", and a test written today pins whatever shipped. A new
//! scenario moves a row from "could regress again unseen" to "cannot", never
//! from owed to signed off.

mod adapters;
mod app;
mod config;
mod domain;
mod error;
mod fixture;
mod reference;
mod regression_proof;
mod run;
mod trace;

pub use app::WamlApp;
pub use config::{ScenarioConfig, WorkspaceFixture};
pub use domain::{DiagramName, DocumentSurface, ViewKind};
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
