//! The open project: where its bundle came from, what to call it, and the
//! debounce and last error of writing it back.
//!
//! The four moved together because opening, replacing and closing a project all
//! have to touch every one of them, and each of the three used to touch a
//! different subset -- `open_bundle` cancelled the debounce and reset the error
//! but not the name, `open_dir` set the directory and nothing else, and
//! `close_model` cancelled the debounce and cleared the directory but left the
//! closed project's NAME behind. Nothing read that stale name, which is exactly
//! why it survived: with the fields apart there was no single place where
//! "abandon the project" was written down.
//!
//! # Invariants
//!
//! * **`dir` is native-only, `name` is not.** A browser session has a bundle
//!   decoded from the URL fragment: a real name, no directory behind it. So
//!   these are two fields and not one `Option`, and `dir.is_none()` does NOT
//!   mean "nothing is open".
//! * **The debounce has exactly two verbs.** [`OpenProject::schedule_save`]
//!   arms it and [`OpenProject::cancel_save`] parks it; no caller touches the
//!   `Timer` directly. Arming RESTARTS rather than extends, so a burst of ops
//!   -- a drag that re-authors placement as it moves -- coalesces into a single
//!   save when it settles instead of one per frame.
//! * **Cancelling does not clear the handle.** A stopped timer cannot deliver
//!   an event, so the retained handle is inert and re-arming stops it again
//!   harmlessly.

use super::workspace::SaveFeedback;
use super::*;

/// How long the document has to sit unchanged before `mark_dirty` turns into a
/// `save`. Sized for a pause in editing, not for the tail of a single gesture:
/// a save is a full deflate of the bundle, so coalescing a run of related edits
/// into one is worth more than persisting each of them promptly.
const SAVE_DEBOUNCE_SECS: f64 = 3.0;

#[derive(Default)]
pub(super) struct OpenProject {
    dir: Option<PathBuf>,
    name: String,
    save_timer: Timer,
    feedback: SaveFeedback,
}

impl OpenProject {
    /// Filesystem root backing the bundle in native builds. `None` in the
    /// browser, and between a close and the next open.
    pub(super) fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// Record the directory this project was read from. Native only; the call
    /// site is the one place a root is known.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(super) fn set_dir(&mut self, dir: PathBuf) {
        self.dir = Some(dir);
    }

    /// Basename of the currently-open bundle directory. The bundle's display
    /// name falls back to this when the model carries no root name
    /// (`model.path` is empty -- no root `index.md` H1 / frontmatter title), so
    /// an unnamed bundle reads as its folder rather than a bare "bundle".
    /// Retained across a theme live-edit reload (`rehydrate`), which has no
    /// `dir` in hand.
    pub(super) fn name(&self) -> &str {
        &self.name
    }

    pub(super) fn set_name(&mut self, name: String) {
        self.name = name;
    }

    /// Abandon the open project: park the debounce and forget both the
    /// directory and the name, so nothing of the closed project can be read
    /// back through the start screen and into whatever opens next.
    pub(super) fn close(&mut self, cx: &mut Cx) {
        self.cancel_save(cx);
        self.dir = None;
        self.name.clear();
    }

    /// Arm the save debounce, restarting rather than extending it.
    pub(super) fn schedule_save(&mut self, cx: &mut Cx) {
        cx.stop_timer(self.save_timer);
        self.save_timer = cx.start_timeout(SAVE_DEBOUNCE_SECS);
    }

    /// Park the save debounce without arming it.
    pub(super) fn cancel_save(&mut self, cx: &mut Cx) {
        cx.stop_timer(self.save_timer);
    }

    /// Whether `event` is the debounce firing.
    pub(super) fn save_timer_fired(&self, event: &Event) -> bool {
        self.save_timer.is_event(event).is_some()
    }

    pub(super) fn finish_save(&mut self, result: &Result<(), String>) {
        self.feedback.finish_save(result);
    }

    pub(super) fn save_error(&self) -> Option<&str> {
        self.feedback.save_error()
    }

    /// A replacement bundle is now what the session holds, so the error the
    /// PREVIOUS one ended on is no longer about anything on screen.
    pub(super) fn opened_replacement_bundle(&mut self) {
        self.feedback.opened_replacement_bundle();
    }
}
