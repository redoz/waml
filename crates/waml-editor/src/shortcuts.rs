use makepad_widgets::{KeyCode, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HistoryCommand {
    Undo,
    Redo,
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
}
