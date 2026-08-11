use makepad_widgets::{KeyCode, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HistoryCommand {
    Undo,
    Redo,
}

/// Ctrl+K / Cmd+K palette; Ctrl+F / Cmd+F find strip; F3 / Shift+F3 hits
/// (Task 11, spec §Shortcuts).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SearchCommand {
    OpenPalette,
    OpenFindStrip,
    NextHit,
    PreviousHit,
}

/// Alt always disqualifies; F3 needs NO primary modifier. These four chords
/// are checked here, in one function, against everything the editor already
/// claims (Ctrl+Z/Y, V/N/C, ?, T) -- the collision audit the spec's risk
/// section asks for IS this function's test module.
pub(crate) fn search_command_for(
    key: KeyCode,
    modifiers: KeyModifiers,
    macos: bool,
) -> Option<SearchCommand> {
    if modifiers.alt {
        return None;
    }
    let primary = if macos {
        modifiers.logo && !modifiers.control
    } else {
        modifiers.control && !modifiers.logo
    };
    match key {
        KeyCode::KeyK if primary => Some(SearchCommand::OpenPalette),
        KeyCode::KeyF if primary => Some(SearchCommand::OpenFindStrip),
        KeyCode::F3 if !modifiers.control && !modifiers.logo => {
            if modifiers.shift {
                Some(SearchCommand::PreviousHit)
            } else {
                Some(SearchCommand::NextHit)
            }
        }
        _ => None,
    }
}

pub(crate) fn history_command_for(
    key: KeyCode,
    modifiers: KeyModifiers,
    macos: bool,
) -> Option<HistoryCommand> {
    if modifiers.alt {
        return None;
    }
    let primary = if macos {
        modifiers.logo && !modifiers.control
    } else {
        modifiers.control && !modifiers.logo
    };
    if !primary {
        return None;
    }
    match (key, modifiers.shift, macos) {
        (KeyCode::KeyZ, false, _) => Some(HistoryCommand::Undo),
        (KeyCode::KeyZ, true, _) => Some(HistoryCommand::Redo),
        (KeyCode::KeyY, false, false) => Some(HistoryCommand::Redo),
        _ => None,
    }
}

/// Ctrl/Cmd + '='/'+' | '-' | '0' (spec §Inputs). Alt always disqualifies,
/// matching the two functions above; shift is legal only on Equals because
/// '+' IS Shift+'='. Dispatched by `App::handle_global_shortcuts`, consumed
/// only while a zoomable view is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ZoomCommand {
    In,
    Out,
    Reset,
}

pub(crate) fn zoom_command_for(
    key: KeyCode,
    modifiers: KeyModifiers,
    macos: bool,
) -> Option<ZoomCommand> {
    if modifiers.alt {
        return None;
    }
    let primary = if macos {
        modifiers.logo && !modifiers.control
    } else {
        modifiers.control && !modifiers.logo
    };
    if !primary {
        return None;
    }
    match key {
        KeyCode::Equals => Some(ZoomCommand::In),
        KeyCode::Minus if !modifiers.shift => Some(ZoomCommand::Out),
        KeyCode::Key0 if !modifiers.shift => Some(ZoomCommand::Reset),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modifiers(control: bool, logo: bool, shift: bool) -> KeyModifiers {
        KeyModifiers {
            control,
            logo,
            shift,
            ..Default::default()
        }
    }

    #[test]
    fn macos_uses_command_z_and_command_shift_z() {
        assert_eq!(
            history_command_for(KeyCode::KeyZ, modifiers(false, true, false), true),
            Some(HistoryCommand::Undo)
        );
        assert_eq!(
            history_command_for(KeyCode::KeyZ, modifiers(false, true, true), true),
            Some(HistoryCommand::Redo)
        );
        assert_eq!(
            history_command_for(KeyCode::KeyY, modifiers(false, true, false), true),
            None
        );
    }

    #[test]
    fn other_platforms_support_control_z_shift_z_and_y() {
        assert_eq!(
            history_command_for(KeyCode::KeyZ, modifiers(true, false, false), false),
            Some(HistoryCommand::Undo)
        );
        assert_eq!(
            history_command_for(KeyCode::KeyZ, modifiers(true, false, true), false),
            Some(HistoryCommand::Redo)
        );
        assert_eq!(
            history_command_for(KeyCode::KeyY, modifiers(true, false, false), false),
            Some(HistoryCommand::Redo)
        );
    }

    #[test]
    fn wrong_platform_modifier_and_extra_alt_are_not_history_commands() {
        assert_eq!(
            history_command_for(KeyCode::KeyZ, modifiers(true, false, false), true),
            None
        );
        assert_eq!(
            history_command_for(KeyCode::KeyZ, modifiers(false, true, false), false),
            None
        );
        assert_eq!(
            history_command_for(
                KeyCode::KeyZ,
                KeyModifiers {
                    control: true,
                    alt: true,
                    ..Default::default()
                },
                false,
            ),
            None
        );
    }

    #[test]
    fn macos_uses_command_k_and_command_f() {
        assert_eq!(
            search_command_for(KeyCode::KeyK, modifiers(false, true, false), true),
            Some(SearchCommand::OpenPalette)
        );
        assert_eq!(
            search_command_for(KeyCode::KeyF, modifiers(false, true, false), true),
            Some(SearchCommand::OpenFindStrip)
        );
    }

    #[test]
    fn other_platforms_use_control_k_and_control_f() {
        assert_eq!(
            search_command_for(KeyCode::KeyK, modifiers(true, false, false), false),
            Some(SearchCommand::OpenPalette)
        );
        assert_eq!(
            search_command_for(KeyCode::KeyF, modifiers(true, false, false), false),
            Some(SearchCommand::OpenFindStrip)
        );
    }

    #[test]
    fn f3_needs_no_primary_modifier_and_shift_reverses_direction() {
        assert_eq!(
            search_command_for(KeyCode::F3, modifiers(false, false, false), false),
            Some(SearchCommand::NextHit)
        );
        assert_eq!(
            search_command_for(KeyCode::F3, modifiers(false, false, true), false),
            Some(SearchCommand::PreviousHit)
        );
        // A primary modifier held alongside F3 is not a search command --
        // some other chord's territory.
        assert_eq!(
            search_command_for(KeyCode::F3, modifiers(true, false, false), false),
            None
        );
        assert_eq!(
            search_command_for(KeyCode::F3, modifiers(false, true, false), true),
            None
        );
    }

    #[test]
    fn wrong_platform_modifier_and_extra_alt_are_not_search_commands() {
        assert_eq!(
            search_command_for(KeyCode::KeyK, modifiers(true, false, false), true),
            None
        );
        assert_eq!(
            search_command_for(KeyCode::KeyF, modifiers(false, true, false), false),
            None
        );
        assert_eq!(
            search_command_for(
                KeyCode::KeyK,
                KeyModifiers {
                    control: true,
                    alt: true,
                    ..Default::default()
                },
                false,
            ),
            None
        );
    }

    #[test]
    fn every_already_claimed_chord_is_not_a_search_command() {
        // Ctrl+Z / Ctrl+Y (history), plain V/N/C (tool dock), plain T (theme
        // toggle), plain `?` (shortcuts overlay) -- the collision audit the
        // spec's risk section asks for.
        assert_eq!(
            search_command_for(KeyCode::KeyZ, modifiers(true, false, false), false),
            None
        );
        assert_eq!(
            search_command_for(KeyCode::KeyY, modifiers(true, false, false), false),
            None
        );
        for key in [
            KeyCode::KeyV,
            KeyCode::KeyN,
            KeyCode::KeyC,
            KeyCode::KeyT,
            KeyCode::Slash,
        ] {
            assert_eq!(
                search_command_for(key, modifiers(false, false, false), false),
                None
            );
        }
    }

    #[test]
    fn zoom_chords_map_on_both_platforms() {
        // Ctrl+= / Ctrl+Shift+= (i.e. +) / Ctrl+- / Ctrl+0 on non-mac
        assert_eq!(
            zoom_command_for(KeyCode::Equals, modifiers(true, false, false), false),
            Some(ZoomCommand::In)
        );
        assert_eq!(
            zoom_command_for(KeyCode::Equals, modifiers(true, false, true), false),
            Some(ZoomCommand::In)
        );
        assert_eq!(
            zoom_command_for(KeyCode::Minus, modifiers(true, false, false), false),
            Some(ZoomCommand::Out)
        );
        assert_eq!(
            zoom_command_for(KeyCode::Key0, modifiers(true, false, false), false),
            Some(ZoomCommand::Reset)
        );
        // Cmd on mac
        assert_eq!(
            zoom_command_for(KeyCode::Equals, modifiers(false, true, false), true),
            Some(ZoomCommand::In)
        );
        assert_eq!(
            zoom_command_for(KeyCode::Minus, modifiers(false, true, false), true),
            Some(ZoomCommand::Out)
        );
        assert_eq!(
            zoom_command_for(KeyCode::Key0, modifiers(false, true, false), true),
            Some(ZoomCommand::Reset)
        );
    }

    #[test]
    fn wrong_platform_modifier_extra_alt_and_bare_keys_are_not_zoom_commands() {
        assert_eq!(
            zoom_command_for(KeyCode::Equals, modifiers(true, false, false), true),
            None
        );
        assert_eq!(
            zoom_command_for(KeyCode::Minus, modifiers(false, true, false), false),
            None
        );
        assert_eq!(
            zoom_command_for(KeyCode::Minus, modifiers(false, false, false), false),
            None
        );
        assert_eq!(
            zoom_command_for(
                KeyCode::Equals,
                KeyModifiers {
                    control: true,
                    alt: true,
                    ..Default::default()
                },
                false,
            ),
            None
        );
    }

    #[test]
    fn zoom_chords_collide_with_nothing_already_claimed() {
        // The new chords are not history or search commands, and the claimed
        // chords are not zoom commands -- extends the existing audit.
        for key in [KeyCode::Equals, KeyCode::Minus, KeyCode::Key0] {
            assert_eq!(
                search_command_for(key, modifiers(true, false, false), false),
                None
            );
            assert_eq!(
                history_command_for(key, modifiers(true, false, false), false),
                None
            );
        }
        for key in [KeyCode::KeyZ, KeyCode::KeyY, KeyCode::KeyK, KeyCode::KeyF] {
            assert_eq!(
                zoom_command_for(key, modifiers(true, false, false), false),
                None
            );
        }
        assert_eq!(
            zoom_command_for(KeyCode::F3, modifiers(false, false, false), false),
            None
        );
    }
}
