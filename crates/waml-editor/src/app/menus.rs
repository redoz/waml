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
            checked: None,
        },
        PopupItem {
            id: live_id!(about),
            label: "About".into(),
            icon: Some(Icon::Info),
            danger: false,
            enabled: true,
            checked: None,
        },
        PopupItem {
            id: live_id!(fonts),
            label: "Fonts".into(),
            icon: Some(Icon::Paintbrush),
            danger: false,
            enabled: true,
            checked: None,
        },
        PopupItem {
            id: live_id!(icons),
            label: "Icons".into(),
            icon: Some(Icon::SquareMenu),
            danger: false,
            enabled: true,
            checked: None,
        },
        PopupItem {
            id: live_id!(colors),
            label: "Colors".into(),
            icon: Some(Icon::Squircle),
            danger: false,
            enabled: true,
            checked: None,
        },
        PopupItem {
            id: live_id!(exit),
            label: "Exit".into(),
            icon: Some(Icon::CircleX),
            danger: true,
            enabled: true,
            checked: None,
        },
    ]
}

/// The burger (caption `menu_btn`) drop-down rows: Create, Open model, Export
/// WAML bundle, Close model. New/Open mirror the start screen's actions; Close
/// returns to the start screen. Routed through `popup_root`; the committed ids
/// are handled via the tag-filtered `closed` read in `handle_actions`.
///
/// `has_model` disables the rows that need a model to act on, rather than
/// hiding them: a menu whose rows move depending on state is harder to use
/// than one with a greyed row.
pub fn burger_menu_items(has_model: bool) -> Vec<crate::popup::base::PopupItem> {
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
            checked: None,
        },
        PopupItem {
            id: live_id!(open_model),
            label: "Open model".into(),
            // The open-door glyph, pairing with Close model's door-closed.
            icon: Some(Icon::DoorOpen),
            danger: false,
            enabled: true,
            checked: None,
        },
        PopupItem {
            id: live_id!(export_waml),
            label: "Export WAML bundle…".into(),
            // A folder going down: the whole model directory leaves the
            // editor as one file.
            icon: Some(Icon::FolderDown),
            danger: false,
            enabled: has_model,
            checked: None,
        },
        PopupItem {
            id: live_id!(close_model),
            label: "Close model".into(),
            // The door-closed glyph, drawn directly from the catalog.
            icon: Some(Icon::DoorClosed),
            danger: false,
            enabled: has_model,
            checked: None,
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
            checked: None,
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

/// What a projection popup row toggles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectionToggle {
    /// Every one of this extension's maskable names, moved together.
    Extension(Vec<String>),
    /// One stage.
    Stage(String),
}

/// A projection row's leading glyph, from the same three eyes the tree
/// toolbar's button reads in (`TreePanel::projection_icon`), so the checklist
/// and the glyph that opened it say the same thing: open for running, dashed
/// for an extension only partly running, closed for switched off.
///
/// `running` counts how many of the row's stages are on, out of `total` -- a
/// stage row is simply the one-of-one case.
fn projection_row_icon(running: usize, total: usize) -> crate::icons::Icon {
    if running == 0 {
        crate::icons::Icon::EyeClosed
    } else if running == total {
        crate::icons::Icon::Eye
    } else {
        crate::icons::Icon::EyeDashed
    }
}

fn extension_row_id(owner: &str) -> LiveId {
    LiveId::from_str(&format!("ext:{owner}"))
}

fn stage_row_id(name: &str) -> LiveId {
    LiveId::from_str(&format!("stage:{name}"))
}

/// The projection checklist: one row per extension owning maskable stages,
/// its stage rows nested beneath.
///
/// CHECKED MEANS RUNNING, not masked -- the popup answers "what is on", the
/// same question the toolbar glyph answers.
///
/// Built from `folder_projection::maskable_names`, which is built from the
/// registry. Never hand-write an extension list here: two construction sites
/// that disagree are invisible.
pub fn projection_menu_items(
    maskable: &[(&str, Vec<&str>)],
    mask: &waml::view::mask::ProjectionMask,
) -> Vec<crate::popup::base::PopupItem> {
    use crate::popup::base::PopupItem;
    let mut items = Vec::new();
    for (owner, names) in maskable {
        let running = names.iter().filter(|name| !mask.is_masked(name)).count();
        items.push(PopupItem {
            id: extension_row_id(owner),
            // The registry name is an identifier, not a caption -- see
            // `folder_projection::extension_label`.
            label: crate::folder_projection::extension_label(owner),
            // A checkable row's on/off state is drawn as its leading icon's
            // TINT (see `popup::menu::row_tint_role`), which is only drawn for
            // a row that HAS an icon -- an iconless row would render its state
            // nowhere. The eye glyph carries the same state a second way, and
            // resolves the one thing the checkmark cannot: an extension whose
            // stages are SPLIT reads dashed rather than simply checked.
            icon: Some(projection_row_icon(running, names.len())),
            danger: false,
            enabled: true,
            checked: Some(running > 0),
        });
        for name in names {
            let running = usize::from(!mask.is_masked(name));
            items.push(PopupItem {
                id: stage_row_id(name),
                // Two leading spaces read as nesting without a new indent
                // mechanism in the menu's row layout.
                label: format!("  {}", crate::folder_projection::stage_label(name)),
                // Same reason as the extension row above: no icon, no visible
                // checked state.
                icon: Some(projection_row_icon(running, 1)),
                danger: false,
                enabled: true,
                checked: Some(running == 1),
            });
        }
    }
    items
}

/// Map a committed row id back to what it toggles, by re-deriving the same ids
/// `projection_menu_items` minted. Never parses a label.
pub fn projection_toggle_target(
    id: LiveId,
    maskable: &[(&str, Vec<&str>)],
) -> Option<ProjectionToggle> {
    for (owner, names) in maskable {
        if id == extension_row_id(owner) {
            return Some(ProjectionToggle::Extension(
                names.iter().map(|n| (*n).to_string()).collect(),
            ));
        }
        for name in names {
            if id == stage_row_id(name) {
                return Some(ProjectionToggle::Stage((*name).to_string()));
            }
        }
    }
    None
}

/// Apply one projection popup row's toggle to a mask, returning the new mask.
///
/// The whole decision the popup makes, with no widget tree in the way, so the
/// seam between a committed row id and the session mask can be tested.
pub fn apply_projection_toggle(
    mask: &waml::view::mask::ProjectionMask,
    target: &ProjectionToggle,
) -> waml::view::mask::ProjectionMask {
    let mut mask = mask.clone();
    match target {
        // An extension row moves all of its names together: if any is still
        // running, the row switches the whole extension off; otherwise it
        // switches the whole extension back on.
        ProjectionToggle::Extension(names) => {
            let any_running = names.iter().any(|name| !mask.is_masked(name));
            for name in names {
                mask.set_masked(name, any_running);
            }
        }
        ProjectionToggle::Stage(name) => {
            let masked = mask.is_masked(name);
            mask.set_masked(name, !masked);
        }
    }
    mask
}

#[cfg(test)]
mod projection_tests {
    use super::*;
    use waml::view::mask::ProjectionMask;

    fn maskable() -> Vec<(&'static str, Vec<&'static str>)> {
        vec![("core", vec!["hide"]), ("uml", vec!["uml"])]
    }

    #[test]
    fn an_empty_mask_shows_every_row_checked() {
        let items = projection_menu_items(&maskable(), &ProjectionMask::default());
        assert_eq!(
            items.len(),
            4,
            "two extension rows and their two stage rows",
        );
        assert!(
            items.iter().all(|item| item.checked == Some(true)),
            "checked means running, and an empty mask runs everything",
        );
    }

    #[test]
    fn masking_a_stage_unchecks_it_and_its_extension() {
        let items = projection_menu_items(&maskable(), &ProjectionMask::from_names(["hide"]));
        let core_ext = items
            .iter()
            .find(|item| item.id == LiveId::from_str("ext:core"))
            .unwrap();
        let hide = items
            .iter()
            .find(|item| item.id == LiveId::from_str("stage:hide"))
            .unwrap();
        let uml = items
            .iter()
            .find(|item| item.id == LiveId::from_str("stage:uml"))
            .unwrap();
        assert_eq!(hide.checked, Some(false));
        assert_eq!(
            core_ext.checked,
            Some(false),
            "core's only maskable stage is off, so core reads off",
        );
        assert_eq!(uml.checked, Some(true), "another extension is untouched");
    }

    /// The checkable-row state is drawn as the leading icon's tint, and that
    /// draw is gated on the row HAVING an icon: an iconless projection row
    /// renders no on/off indication at all.
    #[test]
    fn every_projection_row_carries_an_icon_to_tint() {
        let items = projection_menu_items(&maskable(), &ProjectionMask::from_names(["hide"]));
        assert!(
            items.iter().all(|item| item.icon.is_some()),
            "a checkable row without an icon has nowhere to show its state",
        );
    }

    /// The toggle seam end to end: the id a row was built with resolves to a
    /// target, applying it flips the mask, and re-seeding from the new mask
    /// flips exactly that row's checkmark.
    #[test]
    fn toggling_a_stage_row_flips_the_mask_and_the_reseeded_rows() {
        let maskable = maskable();
        let mask = ProjectionMask::default();
        let row_id = projection_menu_items(&maskable, &mask)
            .into_iter()
            .find(|item| item.id == stage_row_id("hide"))
            .expect("the hide stage has a row")
            .id;
        let target = projection_toggle_target(row_id, &maskable).expect("the row resolves");
        let mask = apply_projection_toggle(&mask, &target);
        assert!(mask.is_masked("hide"), "the toggled stage is now masked");
        assert!(!mask.is_masked("uml"), "no other stage moved");

        let items = projection_menu_items(&maskable, &mask);
        // Keyed on the minted id, never on the label: labels are captions and
        // may be reworded without the seam noticing.
        let checked = |name: &str| {
            items
                .iter()
                .find(|item| item.id == stage_row_id(name))
                .unwrap()
                .checked
        };
        assert_eq!(checked("hide"), Some(false), "re-seeded row reads off");
        assert_eq!(checked("uml"), Some(true));

        // Toggling the same row again restores it: the seam is symmetric.
        let mask = apply_projection_toggle(&mask, &target);
        assert!(!mask.is_masked("hide"));
    }

    #[test]
    fn toggling_an_extension_row_moves_all_of_its_stages() {
        let maskable = vec![("core", vec!["hide", "hide-others"])];
        let target = projection_toggle_target(LiveId::from_str("ext:core"), &maskable)
            .expect("the row resolves");
        // Any stage still running switches the whole extension off.
        let off = apply_projection_toggle(&ProjectionMask::from_names(["hide"]), &target);
        assert!(off.is_masked("hide") && off.is_masked("hide-others"));
        assert!(projection_menu_items(&maskable, &off)
            .iter()
            .all(|item| item.checked == Some(false)),);
        // With nothing running, it switches the whole extension back on.
        let on = apply_projection_toggle(&off, &target);
        assert!(!on.is_masked("hide") && !on.is_masked("hide-others"));
    }

    #[test]
    fn index_never_appears_as_a_row() {
        let items = projection_menu_items(&maskable(), &ProjectionMask::default());
        assert!(
            !items.iter().any(|item| item.id == stage_row_id("index")),
            "the terminal stage is not maskable, so offering it would be a lie",
        );
    }

    /// The checklist reads as captions, not as the registry identifiers the
    /// mask is keyed on. A row labelled `hide` or `core` is a leaked internal.
    #[test]
    fn rows_read_as_captions_not_registry_names() {
        let items = projection_menu_items(&maskable(), &ProjectionMask::default());
        let labels: Vec<String> = items
            .iter()
            .map(|item| item.label.trim().to_string())
            .collect();
        assert!(
            !labels
                .iter()
                .any(|label| label == "core" || label == "uml" || label == "hide"),
            "raw registry names leaked into the menu: {labels:?}",
        );
        assert!(
            labels.iter().any(|label| label == "Hide marked items"),
            "the hide stage is captioned by what it does while running: {labels:?}",
        );
    }

    /// The glyph carries the same reading as the tint, out of the same three
    /// eyes the tree toolbar's button uses.
    #[test]
    fn a_switched_off_row_draws_the_closed_eye() {
        let items = projection_menu_items(&maskable(), &ProjectionMask::from_names(["hide"]));
        for item in &items {
            let expected = if item.checked == Some(true) {
                crate::icons::Icon::Eye
            } else {
                crate::icons::Icon::EyeClosed
            };
            assert_eq!(item.icon, Some(expected), "row {:?}", item.label);
        }
    }

    /// The one state a checkmark cannot show: an extension with some stages on
    /// and some off is checked either way, so the glyph has to carry it.
    #[test]
    fn a_split_extension_row_draws_the_dashed_eye() {
        let maskable = vec![("core", vec!["hide", "hide-others"])];
        let items = projection_menu_items(&maskable, &ProjectionMask::from_names(["hide"]));
        let ext = items
            .iter()
            .find(|item| item.id == extension_row_id("core"))
            .unwrap();
        assert_eq!(ext.icon, Some(crate::icons::Icon::EyeDashed));
        assert_eq!(
            ext.checked,
            Some(true),
            "something is still running, so the row stays checked",
        );
    }

    #[test]
    fn an_extension_row_resolves_to_all_of_its_names() {
        let target = projection_toggle_target(LiveId::from_str("ext:core"), &maskable());
        assert_eq!(
            target,
            Some(ProjectionToggle::Extension(vec!["hide".to_string()])),
        );
        let stage = projection_toggle_target(LiveId::from_str("stage:uml"), &maskable());
        assert_eq!(stage, Some(ProjectionToggle::Stage("uml".to_string())));
        assert_eq!(
            projection_toggle_target(LiveId::from_str("ext:nope"), &maskable()),
            None,
        );
    }
}
