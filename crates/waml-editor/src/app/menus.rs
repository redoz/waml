use makepad_widgets::*;

/// The logo (app) drop-down rows, top to bottom: Properties, About, Exit
/// (danger). No Cancel row -- a drop-down dismisses via Esc / outside-click.
/// Ids are what `MenuPopup` reports on commit; `logo_command_for` maps them back.
pub fn logo_menu_items() -> Vec<crate::popup::base::PopupItem> {
    use crate::icons::Icon;
    use crate::popup::base::PopupItem;
    vec![
        PopupItem {
            id: live_id!(properties),
            label: "Properties".into(),
            icon: Some(Icon::SlidersHorizontal),
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(about),
            label: "About".into(),
            icon: Some(Icon::Info),
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(fonts),
            label: "Fonts".into(),
            icon: Some(Icon::Paintbrush),
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(icons),
            label: "Icons".into(),
            icon: Some(Icon::SquareMenu),
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(colors),
            label: "Colors".into(),
            icon: Some(Icon::Squircle),
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(exit),
            label: "Exit".into(),
            icon: Some(Icon::CircleX),
            danger: true,
            enabled: true,
        },
    ]
}

/// The burger (caption `menu_btn`) drop-down rows: Create, Open model, Close
/// model. New/Open mirror the start screen's actions; Close returns to the
/// start screen. Routed through `popup_root`; the committed ids are handled
/// via the tag-filtered `closed` read in `handle_actions`.
pub fn burger_menu_items() -> Vec<crate::popup::base::PopupItem> {
    use crate::icons::Icon;
    use crate::popup::base::PopupItem;
    vec![
        PopupItem {
            id: live_id!(new_model),
            // No model-specific glyph exists, so keep it a generic "Create".
            label: "Create".into(),
            icon: Some(Icon::SquarePlus),
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(open_model),
            label: "Open model".into(),
            // The open-door glyph, pairing with Close model's door-closed.
            icon: Some(Icon::DoorOpen),
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(close_model),
            label: "Close model".into(),
            // The door-closed glyph, drawn directly from the catalog.
            icon: Some(Icon::DoorClosed),
            danger: false,
            enabled: true,
        },
    ]
}

pub(super) const DOC_SWITCHER_MAX_H: f64 = 360.0;

pub(super) fn doc_switcher_items(
    open: &[crate::doc_tabs::DocTab],
) -> Vec<crate::popup::base::PopupItem> {
    open.iter()
        .map(|tab| crate::popup::base::PopupItem {
            id: tab.id,
            label: tab.title.clone(),
            icon: Some(tab.presentation.icon),
            danger: false,
            enabled: true,
        })
        .collect()
}

/// The logo-radial commands `App` acts on. `Cancel` is intentionally absent:
/// committing the Cancel wedge just closes the radial (mapped to `None`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogoCommand {
    Properties,
    About,
    Fonts,
    Icons,
    Colors,
    Exit,
}

/// Map a radial-committed `LiveId` to a logo command. `None` = not one of ours
/// (Cancel / node ids / unknown).
pub fn logo_command_for(id: LiveId) -> Option<LogoCommand> {
    if id == live_id!(properties) {
        Some(LogoCommand::Properties)
    } else if id == live_id!(about) {
        Some(LogoCommand::About)
    } else if id == live_id!(fonts) {
        Some(LogoCommand::Fonts)
    } else if id == live_id!(icons) {
        Some(LogoCommand::Icons)
    } else if id == live_id!(colors) {
        Some(LogoCommand::Colors)
    } else if id == live_id!(exit) {
        Some(LogoCommand::Exit)
    } else {
        None
    }
}
