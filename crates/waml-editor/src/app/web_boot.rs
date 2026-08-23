//! What a browser session remembers about reaching the network: the boot it is
//! waiting on, and the `waml serve` backend a boot can commit to.
//!
//! Every field here is written only by the wasm build. The type is
//! unconditional because [`WebBoot::index_pending`] is read by `open_bundle`,
//! which is shared with the native build (and always answers `false` there).
//!
//! # One boot, two channels
//!
//! A session boots from at most ONE place. `select_browser_boot` answers with a
//! single [`BrowserBootSource`](crate::browser_boot::BrowserBootSource), and
//! only the `Start` arm goes on to ask the site config — which can then name a
//! bundle, but never an `?api=` base. Held as two `Option`s, "at most one is
//! ever `Some`" was a fact about the call graph that nothing checked; held as
//! one [`PendingBoot`], arming either channel disarms the other by
//! construction.
//!
//! Both channels are claimed the same way, and for the same reason the
//! deferred-navigation slots are (see [`super::deferred`]): [`claim`] empties
//! the slot *before* it asks whether the response belongs to the channel that
//! armed it, so a response arriving for a superseded boot cannot be written as
//! a path that leaves the boot armed.
//!
//! # The index claim belongs to a bundle that opened
//!
//! The export-time search-index asset is fetched for one specific bundle, and
//! `open_bundle` reads [`index_pending`](WebBoot::index_pending) to know it may
//! skip the local index build (spec §Export-time index, decision 10). If that
//! open then FAILS, the claim has to be cancelled: the asset would otherwise
//! land, hash-match the bundle it was exported for, and install itself over a
//! session holding entirely different documents — a search index whose hits
//! point into documents that are not open. [`cancel_index`](WebBoot::cancel_index)
//! is the boot path's statement that the bundle it armed for never opened.
//!
//! # One save in flight
//!
//! `POST /api/documents` is a mutex, not a queue. A second POST issued while
//! the first is pending would overwrite the stashed ticket — so the first
//! response completes the *second* ticket and the second response finds an
//! empty slot — and would carry the same already-stale `revision`. The guard
//! and the arm therefore live in one function (`App::start_api_save`), with
//! [`arm_save`](WebBoot::arm_save) asserting the slot it is filling was empty;
//! a caller that wants to save while one is in flight goes back through the
//! existing debounce.

use super::*;

/// The backend a browser boot committed to at open time: a live `waml serve`
/// mounted at `base`, presenting `token` on every request, tracking the
/// revision the last successful read or write reported. Held once the `?api=`
/// boot fetch succeeds; consulted by the wasm `save_backend` so the save seam
/// stays a choice made once at open, not re-derived per save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ApiBackend {
    pub(super) base: String,
    pub(super) token: Option<String>,
    pub(super) revision: u64,
}

/// Where a `?api=` boot is fetching from, and what it will present to get
/// there.
///
/// This is [`ApiBackend`] without a revision, because that is exactly what it
/// becomes: the revision is the one thing only the response can supply, so
/// [`WebBoot::commit_backend`] is the only way to build a backend and it can
/// only be handed a boot that actually landed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PendingApiBoot {
    pub(super) base: String,
    pub(super) token: Option<String>,
}

/// The one boot a session is waiting on, and which channel it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
enum PendingBoot {
    /// A Bundle Envelope v1 file, named by `?bundle=` or by the site config.
    Bundle(String),
    /// A live `waml serve`, named by `?api=`.
    Api(PendingApiBoot),
}

/// Take the pending boot, keeping it only if it came from the channel whose
/// response this is.
///
/// The slot is emptied either way — see the module docs.
fn claim<T>(
    slot: &mut Option<PendingBoot>,
    channel: impl FnOnce(PendingBoot) -> Option<T>,
) -> Option<T> {
    slot.take().and_then(channel)
}

/// The network state of a browser session: the boot in flight, the index asset
/// that boot armed, the backend it committed to, and the save that backend has
/// outstanding.
#[derive(Default)]
pub(super) struct WebBoot {
    boot: Option<PendingBoot>,
    index: Option<u64>,
    backend: Option<ApiBackend>,
    save: Option<SaveTicket>,
}

impl WebBoot {
    /// Arm the boot-bundle channel with the URL being fetched, so the response
    /// can name it in an error.
    pub(super) fn arm_boot_bundle(&mut self, url: String) {
        self.boot = Some(PendingBoot::Bundle(url));
    }

    /// Arm the `?api=` channel with the base and token being presented.
    pub(super) fn arm_api_boot(&mut self, base: String, token: Option<String>) {
        self.boot = Some(PendingBoot::Api(PendingApiBoot { base, token }));
    }

    /// Claim the boot-bundle URL, if the pending boot is one.
    pub(super) fn claim_boot_bundle(&mut self) -> Option<String> {
        claim(&mut self.boot, |boot| match boot {
            PendingBoot::Bundle(url) => Some(url),
            PendingBoot::Api(_) => None,
        })
    }

    /// Claim the pending `?api=` boot, if the pending boot is one.
    pub(super) fn claim_api_boot(&mut self) -> Option<PendingApiBoot> {
        claim(&mut self.boot, |boot| match boot {
            PendingBoot::Api(api) => Some(api),
            PendingBoot::Bundle(_) => None,
        })
    }

    /// Arm the export-time index-asset fetch with the hash of the bundle it was
    /// exported for — the bundle's own `waml::search::asset::bundle_hash`,
    /// never the asset's.
    pub(super) fn arm_index(&mut self, expected_hash: u64) {
        self.index = Some(expected_hash);
    }

    /// Whether an index asset is in flight for the bundle being opened right
    /// now, so `open_bundle` can skip the local build the asset exists to
    /// replace. Always `false` on native, where nothing arms it.
    pub(super) fn index_pending(&self) -> bool {
        self.index.is_some()
    }

    /// Claim the expected hash of the in-flight index asset.
    pub(super) fn claim_index(&mut self) -> Option<u64> {
        self.index.take()
    }

    /// Give up on the in-flight index asset because the bundle it was armed for
    /// did not open. See the module docs: without this the asset installs a
    /// search index over an unrelated session.
    pub(super) fn cancel_index(&mut self) {
        self.index = None;
    }

    /// Commit `pending` as this session's backend, at the revision its response
    /// reported.
    pub(super) fn commit_backend(&mut self, pending: PendingApiBoot, revision: u64) {
        self.backend = Some(ApiBackend {
            base: pending.base,
            token: pending.token,
            revision,
        });
    }

    /// The committed backend, if this session booted from `?api=`.
    pub(super) fn backend(&self) -> Option<&ApiBackend> {
        self.backend.as_ref()
    }

    /// Adopt `revision` as the backend's current one. A no-op when there is no
    /// backend, which is every native session and every browser session that
    /// booted from a file.
    pub(super) fn set_revision(&mut self, revision: u64) {
        if let Some(backend) = self.backend.as_mut() {
            backend.revision = revision;
        }
    }

    /// Whether a `POST /api/documents` is outstanding. A caller that wants to
    /// save must defer rather than start a second one; see the module docs.
    pub(super) fn save_in_flight(&self) -> bool {
        self.save.is_some()
    }

    /// Stash the ticket the in-flight save was built from, so its response can
    /// complete it.
    ///
    /// Asserts the slot was empty: overwriting it silently completes the wrong
    /// ticket, which is why the only caller checks
    /// [`save_in_flight`](Self::save_in_flight) immediately above.
    pub(super) fn arm_save(&mut self, ticket: SaveTicket) {
        debug_assert!(
            self.save.is_none(),
            "a second save was started while one was still in flight",
        );
        self.save = Some(ticket);
    }

    /// Claim the ticket the resolved save was built from.
    pub(super) fn claim_save(&mut self) -> Option<SaveTicket> {
        self.save.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use waml::edit::ExactSourceEdit;
    use waml::source::SourceBundle;
    use waml_syntax::{TextChange, TextRange, TextSize};

    /// A real `SaveTicket`, which only a dirty session mints. Its contents do
    /// not matter here — the slot is a mutex, not a value under test.
    fn ticket() -> SaveTicket {
        let path = waml::source::BundlePath::parse("order.md").unwrap();
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(SourceBundle::try_from_pairs([("order.md", "# Order\n")]).unwrap())
            .unwrap();
        let before = session.snapshot();
        let document = before.okf_analysis.catalog.id_for_path(&path).unwrap();
        let syntax = before.markdown_snapshot(document).unwrap();
        let edited = "# Ordered\n";
        session
            .apply(ExactSourceEdit {
                document,
                base_revision: syntax.revision(),
                changes: Arc::from([TextChange {
                    old_range: TextRange::new(TextSize::new(0), syntax.text().len()).unwrap(),
                    replacement: Arc::from(edited),
                }]),
                expected_text: waml_syntax::SourceText::new(edited.to_string()).unwrap(),
            })
            .unwrap();
        session.save_ticket().unwrap()
    }

    #[test]
    fn arming_one_boot_channel_disarms_the_other() {
        let mut web = WebBoot::default();
        web.arm_boot_bundle("bundle.waml".to_string());
        web.arm_api_boot("http://127.0.0.1/api".to_string(), Some("t".to_string()));

        assert_eq!(
            web.claim_boot_bundle(),
            None,
            "the bundle boot was replaced"
        );
        let mut web = WebBoot::default();
        web.arm_api_boot("http://127.0.0.1/api".to_string(), None);
        web.arm_boot_bundle("bundle.waml".to_string());
        assert_eq!(web.claim_api_boot(), None, "the api boot was replaced");
    }

    #[test]
    fn a_claim_for_the_wrong_channel_still_empties_the_slot() {
        // The rule the deferred-navigation slots share: a response for a boot
        // that is no longer the pending one abandons it rather than leaving it
        // armed for some later, unrelated response.
        let mut web = WebBoot::default();
        web.arm_api_boot("http://127.0.0.1/api".to_string(), None);

        assert_eq!(web.claim_boot_bundle(), None);
        assert_eq!(
            web.claim_api_boot(),
            None,
            "the mismatched claim must have emptied the slot",
        );
    }

    #[test]
    fn a_boot_is_claimed_once() {
        let mut web = WebBoot::default();
        web.arm_boot_bundle("bundle.waml".to_string());
        assert_eq!(web.claim_boot_bundle().as_deref(), Some("bundle.waml"));
        assert_eq!(web.claim_boot_bundle(), None);
    }

    #[test]
    fn committing_a_backend_carries_the_boot_it_landed_for() {
        let mut web = WebBoot::default();
        web.arm_api_boot("http://host/api".to_string(), Some("secret".to_string()));
        let pending = web.claim_api_boot().expect("the api boot is pending");

        web.commit_backend(pending, 7);

        let backend = web.backend().expect("the boot committed a backend");
        assert_eq!(backend.base, "http://host/api");
        assert_eq!(backend.token.as_deref(), Some("secret"));
        assert_eq!(backend.revision, 7);
    }

    #[test]
    fn a_revision_without_a_backend_is_dropped_rather_than_panicking() {
        // Every native session and every browser session that booted from a
        // file takes this path.
        let mut web = WebBoot::default();
        web.set_revision(3);
        assert!(web.backend().is_none());
    }

    #[test]
    fn a_committed_backend_adopts_a_new_revision() {
        let mut web = WebBoot::default();
        web.arm_api_boot("http://host/api".to_string(), None);
        let pending = web.claim_api_boot().unwrap();
        web.commit_backend(pending, 1);

        web.set_revision(9);

        assert_eq!(web.backend().unwrap().revision, 9);
    }

    #[test]
    fn an_index_claim_is_pending_only_while_armed() {
        let mut web = WebBoot::default();
        assert!(!web.index_pending(), "native never arms one");

        web.arm_index(42);
        assert!(web.index_pending());
        assert_eq!(web.claim_index(), Some(42));
        assert!(!web.index_pending());
        assert_eq!(web.claim_index(), None);
    }

    #[test]
    fn cancelling_an_index_claim_stops_the_asset_installing() {
        // The bundle the asset was exported for failed to open, so the
        // response must find nothing to install over the unrelated session
        // that is still loaded.
        let mut web = WebBoot::default();
        web.arm_index(42);

        web.cancel_index();

        assert!(!web.index_pending());
        assert_eq!(web.claim_index(), None);
    }

    #[test]
    fn the_save_slot_is_a_mutex() {
        let mut web = WebBoot::default();
        assert!(!web.save_in_flight());

        web.arm_save(ticket());
        assert!(web.save_in_flight(), "a second POST must be refused");

        assert!(web.claim_save().is_some());
        assert!(!web.save_in_flight());
        assert!(web.claim_save().is_none());
    }

    #[test]
    #[should_panic(expected = "a second save was started while one was still in flight")]
    fn arming_a_second_save_over_a_live_one_is_a_bug() {
        // The overwrite would complete the WRONG ticket from the first
        // response and strand the second. Callers check `save_in_flight`.
        let mut web = WebBoot::default();
        web.arm_save(ticket());
        web.arm_save(ticket());
    }
}
