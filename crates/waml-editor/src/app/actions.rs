#![cfg_attr(not(test), allow(dead_code, unused_imports))]

use super::App;
use crate::document_host::DocumentCommand;
use crate::popup::root::{MenuOpen, PopupRoot, PopupSpec, RadialOpen};
use makepad_widgets::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ActionFlow {
    Continue,
    Consumed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObserverHandler {
    CaptionAndDocks,
    PopupResults,
    ConflictList,
}

const OBSERVER_ORDER: [ObserverHandler; 3] = [
    ObserverHandler::CaptionAndDocks,
    ObserverHandler::PopupResults,
    ObserverHandler::ConflictList,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PopupRelay {
    ElementPickerClosed,
    PlaceDialArmed,
    PlaceDialClosed,
}

const DOCUMENT_POPUP_RELAY_ORDER: [PopupRelay; 3] = [
    PopupRelay::ElementPickerClosed,
    PopupRelay::PlaceDialArmed,
    PopupRelay::PlaceDialClosed,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExclusiveHandler {
    NavigationScope,
    NavigationQuery,
    NavigationFilter,
    TreeContextMenu,
    TreeDocumentOpen,
    DiagramSwitcher,
    ConflictBadge,
    ActiveDocumentView,
    LogoMenu,
    StartScreen,
    ShortcutsOverlay,
    FontsOverlay,
    IconsOverlay,
    ColorsOverlay,
    DocumentTabs,
}

const EXCLUSIVE_ORDER: [ExclusiveHandler; 15] = [
    ExclusiveHandler::NavigationScope,
    ExclusiveHandler::NavigationQuery,
    ExclusiveHandler::NavigationFilter,
    ExclusiveHandler::TreeContextMenu,
    ExclusiveHandler::TreeDocumentOpen,
    ExclusiveHandler::DiagramSwitcher,
    ExclusiveHandler::ConflictBadge,
    ExclusiveHandler::ActiveDocumentView,
    ExclusiveHandler::LogoMenu,
    ExclusiveHandler::StartScreen,
    ExclusiveHandler::ShortcutsOverlay,
    ExclusiveHandler::FontsOverlay,
    ExclusiveHandler::IconsOverlay,
    ExclusiveHandler::ColorsOverlay,
    ExclusiveHandler::DocumentTabs,
];

impl App {
    pub(super) fn apply_session_ops(
        &mut self,
        cx: &mut Cx,
        ops: &[waml::ops::Op],
        error_label: &str,
    ) -> Option<crate::editor_session::SessionChange> {
        if ops.is_empty() {
            return None;
        }
        match self.session.apply_ops(ops) {
            Ok(change) => {
                self.documents
                    .after_session_change(cx, &self.ui, &self.session, change);
                if change.navigation_changed {
                    self.nav_kinds = crate::nav::kinds_in_model(self.session.model());
                    self.refresh_nav(cx, false);
                }
                if change.conflicts_changed {
                    self.sync_conflict_badge(cx);
                }
                self.mark_dirty(cx);
                Some(change)
            }
            Err(error) => {
                log!("{error_label}: {error:?}");
                None
            }
        }
    }

    fn present_view_popup(&mut self, cx: &mut Cx, request: crate::doc_view::PopupRequest) {
        let bounds = self.window_bounds(cx);
        if let Some(mut popup) = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow_mut::<PopupRoot>()
        {
            match request {
                crate::doc_view::PopupRequest::NodeContextMenu {
                    anchor,
                    key,
                    context,
                } => {
                    self.node_menu_key = Some(key);
                    popup.show_at(
                        cx,
                        PopupSpec::Menu {
                            tag: live_id!(node_menu),
                            anchor,
                            bounds,
                            items: crate::popup::node_menu::compose(
                                context,
                                crate::popup::node_menu::base_items(),
                            ),
                            open: MenuOpen::Popup {
                                open_marking: None,
                                max_height: None,
                            },
                        },
                    );
                }
                crate::doc_view::PopupRequest::ElementPicker {
                    anchor_rect,
                    min_width,
                    items,
                } => {
                    let anchor = dvec2(
                        anchor_rect.pos.x,
                        anchor_rect.pos.y + anchor_rect.size.y + crate::popup::select::SELECT_GAP,
                    );
                    popup.show_at(
                        cx,
                        PopupSpec::Select {
                            tag: live_id!(element_picker),
                            anchor,
                            min_width,
                            bounds,
                            items,
                        },
                    );
                }
                crate::doc_view::PopupRequest::PlaceDial { center, items } => {
                    popup.show_at(
                        cx,
                        PopupSpec::Radial {
                            tag: live_id!(place_dial),
                            center,
                            bounds,
                            items,
                            open: RadialOpen::Dial,
                        },
                    );
                }
                crate::doc_view::PopupRequest::Dismiss => popup.close(cx),
            }
        }
    }

    pub(super) fn apply_view_outcome(
        &mut self,
        cx: &mut Cx,
        outcome: crate::doc_view::ViewOutcome,
    ) -> ActionFlow {
        let mut flow = ActionFlow::Continue;
        self.apply_session_ops(cx, &outcome.ops, "place.set failed");

        if let Some(request) = outcome.popup {
            self.present_view_popup(cx, request);
            flow = ActionFlow::Consumed;
        }
        if let Some(key) = outcome.promote_subject {
            self.documents.transition(
                cx,
                &self.ui,
                &self.session,
                DocumentCommand::PromoteSubject(key),
            );
            self.sync_document_shell(cx);
            flow = ActionFlow::Consumed;
        }
        if outcome.close_active {
            let id = self.documents.active_id();
            self.documents
                .transition(cx, &self.ui, &self.session, DocumentCommand::Close(id));
            self.sync_document_shell(cx);
            flow = ActionFlow::Consumed;
        }
        if outcome.statusbar_dirty {
            self.sync_statusbar(cx);
        }
        flow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_exclusive_observers_keep_the_existing_order() {
        assert_eq!(
            OBSERVER_ORDER,
            [
                ObserverHandler::CaptionAndDocks,
                ObserverHandler::PopupResults,
                ObserverHandler::ConflictList,
            ]
        );
    }

    #[test]
    fn exclusive_handlers_keep_the_existing_priority() {
        assert_eq!(
            EXCLUSIVE_ORDER,
            [
                ExclusiveHandler::NavigationScope,
                ExclusiveHandler::NavigationQuery,
                ExclusiveHandler::NavigationFilter,
                ExclusiveHandler::TreeContextMenu,
                ExclusiveHandler::TreeDocumentOpen,
                ExclusiveHandler::DiagramSwitcher,
                ExclusiveHandler::ConflictBadge,
                ExclusiveHandler::ActiveDocumentView,
                ExclusiveHandler::LogoMenu,
                ExclusiveHandler::StartScreen,
                ExclusiveHandler::ShortcutsOverlay,
                ExclusiveHandler::FontsOverlay,
                ExclusiveHandler::IconsOverlay,
                ExclusiveHandler::ColorsOverlay,
                ExclusiveHandler::DocumentTabs,
            ]
        );
    }

    #[test]
    fn placement_dial_armed_is_relayed_before_closed() {
        let armed = DOCUMENT_POPUP_RELAY_ORDER
            .iter()
            .position(|handler| *handler == PopupRelay::PlaceDialArmed)
            .unwrap();
        let closed = DOCUMENT_POPUP_RELAY_ORDER
            .iter()
            .position(|handler| *handler == PopupRelay::PlaceDialClosed)
            .unwrap();
        assert!(armed < closed);
    }
}
