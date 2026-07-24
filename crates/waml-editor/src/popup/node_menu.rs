//! Uniform per-subject node menu: the two base actions plus a `compose()` that
//! merges surface-contributed context items above them. Pure functions + a
//! command enum; not a widget, so nothing registers with the vm. Lands ahead of
//! its wiring (Tasks 3-5), so like `popup/base.rs` and `doc_view.rs` a bin
//! crate's dead-code lint would flag every item until then.
#![allow(dead_code)]

use makepad_widgets::*;

use crate::icons::Icon;
use crate::popup::base::PopupItem;

/// Base (per-subject) node commands. Uniform across every invocation site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeMenuCommand {
    ViewSource,
    FindInDiagrams,
}

/// The base items every node menu ends with, top to bottom. Ids are what
/// `MenuPopup` reports on commit; `command_for` maps them back (mirrors
/// `logo_command_for`).
pub fn base_items() -> Vec<PopupItem> {
    vec![
        PopupItem {
            id: live_id!(view_source),
            label: "View Source".into(),
            icon: Some(Icon::Braces),
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(find_in_diagrams),
            label: "Find in diagrams".into(),
            icon: Some(Icon::Search),
            danger: false,
            enabled: true,
        },
    ]
}

/// Map a menu-committed `LiveId` to a base command. `None` = not one of ours.
pub fn command_for(id: LiveId) -> Option<NodeMenuCommand> {
    if id == live_id!(view_source) {
        Some(NodeMenuCommand::ViewSource)
    } else if id == live_id!(find_in_diagrams) {
        Some(NodeMenuCommand::FindInDiagrams)
    } else {
        None
    }
}

/// Context items first, base items last (base is the stable bottom zone). With
/// an empty `context`, returns `base` unchanged.
pub fn compose(context: Vec<PopupItem>, base: Vec<PopupItem>) -> Vec<PopupItem> {
    let mut items = context;
    items.extend(base);
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_items_yields_the_two_base_entries_in_order() {
        let items = base_items();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, live_id!(view_source));
        assert_eq!(items[0].label, "View Source");
        assert_eq!(items[1].id, live_id!(find_in_diagrams));
        assert_eq!(items[1].label, "Find in diagrams");
    }

    #[test]
    fn command_for_maps_ids_and_rejects_others() {
        assert_eq!(
            command_for(live_id!(view_source)),
            Some(NodeMenuCommand::ViewSource)
        );
        assert_eq!(
            command_for(live_id!(find_in_diagrams)),
            Some(NodeMenuCommand::FindInDiagrams)
        );
        assert_eq!(command_for(live_id!(nope)), None);
    }

    #[test]
    fn compose_puts_context_first_base_last() {
        let ctx = vec![PopupItem {
            id: live_id!(ctx_a),
            label: "Ctx A".into(),
            icon: Some(Icon::Search),
            danger: false,
            enabled: true,
        }];
        let out = compose(ctx, base_items());
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].id, live_id!(ctx_a));
        assert_eq!(out[1].id, live_id!(view_source));
        assert_eq!(out[2].id, live_id!(find_in_diagrams));
    }

    #[test]
    fn compose_empty_context_returns_base_unchanged() {
        let out = compose(vec![], base_items());
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, live_id!(view_source));
        assert_eq!(out[1].id, live_id!(find_in_diagrams));
    }
}
