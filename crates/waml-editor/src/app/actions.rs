#![cfg_attr(not(test), allow(dead_code, unused_imports))]

use super::App;
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
