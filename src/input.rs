//! Launcher commands are separate from TextInput editing and native IME events.

use slint::platform::Key;
use slint::winit_030::winit::event::Ime;

use crate::navigation::Direction;

#[derive(Default)]
pub struct Composition {
    active: bool,
}

impl Composition {
    pub fn update(&mut self, event: &Ime) -> bool {
        // Windows winit 0.30.13 sends Enabled at WM_IME_STARTCOMPOSITION and
        // Disabled at WM_IME_ENDCOMPOSITION. Empty Preedit precedes Commit:
        // neither is permission to dispatch the key that confirms a candidate.
        match event {
            Ime::Enabled => self.active = true,
            Ime::Preedit(text, _) if !text.is_empty() => self.active = true,
            Ime::Disabled => self.active = false,
            _ => {}
        }
        self.active
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Navigate(Direction),
    Choose,
    OpenFile(bool),
    SwitchFileMode,
    ReturnToApps,
    FileMenu,
    CloseAppearance,
    CloseImport,
    ClearQuery,
    Hide,
    Consume,
}

pub struct Context {
    pub composing: bool,
    pub appearance_open: bool,
    pub import_open: bool,
    pub native_dialog: bool,
    pub input_focused: bool,
    pub home: bool,
    pub query_empty: bool,
    pub space_launch: bool,
    pub everything_enabled: bool,
    pub file_mode: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

#[cfg(test)]
pub fn command(text: &str, modified: bool, repeat: bool, context: &Context) -> Option<Command> {
    command_with_modifiers(
        text,
        Modifiers {
            control: modified,
            alt: modified,
            shift: modified,
            meta: modified,
        },
        repeat,
        context,
    )
}

pub fn command_with_modifiers(
    text: &str,
    modifiers: Modifiers,
    repeat: bool,
    context: &Context,
) -> Option<Command> {
    if context.composing || modifiers.alt || modifiers.meta || context.native_dialog {
        return None;
    }
    let key = |key: Key| text.chars().eq(std::iter::once(char::from(key)));
    if context.file_mode
        && modifiers.shift
        && !modifiers.control
        && key(Key::F10)
        && context.input_focused
        && !context.appearance_open
        && !context.import_open
    {
        return Some(if repeat {
            Command::Consume
        } else {
            Command::FileMenu
        });
    }
    if modifiers.shift || (modifiers.control && !(context.file_mode && key(Key::Return))) {
        return None;
    }
    if key(Key::Escape) {
        return Some(if context.appearance_open {
            Command::CloseAppearance
        } else if context.import_open {
            Command::CloseImport
        } else if !context.query_empty {
            Command::ClearQuery
        } else if context.file_mode {
            Command::ReturnToApps
        } else {
            Command::Hide
        });
    }
    if context.appearance_open || context.import_open || !context.input_focused {
        return None;
    }
    if key(Key::Tab) {
        return context.everything_enabled.then_some(if repeat {
            Command::Consume
        } else {
            Command::SwitchFileMode
        });
    }
    Some(if key(Key::DownArrow) {
        Command::Navigate(Direction::Down)
    } else if key(Key::UpArrow) {
        Command::Navigate(Direction::Up)
    } else if context.home && key(Key::LeftArrow) {
        Command::Navigate(Direction::Left)
    } else if context.home && key(Key::RightArrow) {
        Command::Navigate(Direction::Right)
    } else if key(Key::Return) && context.file_mode {
        if repeat {
            Command::Consume
        } else {
            Command::OpenFile(modifiers.control)
        }
    } else if key(Key::Return) || (text == " " && context.space_launch && !context.file_mode) {
        if repeat {
            Command::Consume
        } else {
            Command::Choose
        }
    } else {
        // Fixed fixtures leave space_launch disabled. Clipboard/selection,
        // deletion, cursor motion and other editing stay in native TextInput.
        return None;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: Key) -> String {
        char::from(key).to_string()
    }

    fn context() -> Context {
        Context {
            composing: false,
            appearance_open: false,
            import_open: false,
            native_dialog: false,
            input_focused: true,
            home: true,
            query_empty: true,
            space_launch: false,
            everything_enabled: false,
            file_mode: false,
        }
    }

    #[test]
    fn legacy_configuration_only_adds_the_off_by_default_setting() {
        use velo_slint::core::JsonDocument;
        let legacy = velo_slint::core::config::Config::decode(
            br#"{"version":1,"hotkey":"Alt+Space","future":{"keep":true}}"#,
        )
        .unwrap();
        assert!(!legacy.everything.enabled);
        assert!(legacy.everything.executable.is_empty());
        assert_eq!(legacy.unknown["future"]["keep"], true);
    }

    #[test]
    fn composition_stays_guarded_through_empty_preedit_and_commit() {
        let mut composition = Composition::default();
        for event in [
            Ime::Enabled,
            Ime::Preedit("ni".into(), Some((2, 2))),
            Ime::Preedit("".into(), None),
            Ime::Commit("你".into()),
        ] {
            assert!(composition.update(&event));
        }
        assert!(!composition.update(&Ime::Disabled));
        assert!(composition.update(&Ime::Preedit("zhong".into(), None)));
        assert!(!composition.update(&Ime::Disabled));
    }

    #[test]
    fn ime_owns_enter_space_arrows_escape_even_without_preedit_text() {
        let context = Context {
            composing: true,
            ..context()
        };
        for key in [
            Key::Return,
            Key::UpArrow,
            Key::DownArrow,
            Key::LeftArrow,
            Key::RightArrow,
            Key::Escape,
        ] {
            assert_eq!(
                command(&char::from(key).to_string(), false, false, &context),
                None
            );
        }
        assert_eq!(command(" ", false, false, &context), None);
    }

    #[test]
    fn text_editing_modifiers_panel_focus_and_repeat_do_not_launch() {
        let home = context();
        assert_eq!(
            command(&key(Key::Return), false, false, &home),
            Some(Command::Choose)
        );
        assert_eq!(
            command(&key(Key::Return), false, true, &home),
            Some(Command::Consume)
        );
        for text in [" ", "a", &key(Key::Backspace), &key(Key::Delete)] {
            assert_eq!(command(text, false, false, &home), None);
        }
        assert_eq!(command(&key(Key::LeftArrow), true, false, &home), None);
        let list = Context {
            home: false,
            ..context()
        };
        assert_eq!(command(&key(Key::LeftArrow), false, false, &list), None);
        assert_eq!(command(&key(Key::RightArrow), false, false, &list), None);
        assert_eq!(
            command(&key(Key::DownArrow), false, false, &list),
            Some(Command::Navigate(Direction::Down))
        );
        let panel = Context {
            appearance_open: true,
            ..context()
        };
        assert_eq!(command(&key(Key::Return), false, false, &panel), None);
        assert_eq!(
            command(&key(Key::Escape), false, false, &panel),
            Some(Command::CloseAppearance)
        );
        let button = Context {
            input_focused: false,
            ..context()
        };
        assert_eq!(command(&key(Key::Return), false, false, &button), None);
        let query = Context {
            query_empty: false,
            ..context()
        };
        assert_eq!(
            command(&key(Key::Escape), false, false, &query),
            Some(Command::ClearQuery)
        );
        assert_eq!(
            command(&key(Key::Escape), false, false, &home),
            Some(Command::Hide)
        );
    }

    #[test]
    fn file_mode_tab_space_enter_and_ime_keep_distinct_behavior() {
        let home = Context {
            everything_enabled: true,
            ..context()
        };
        assert_eq!(
            command_with_modifiers(&key(Key::Tab), Modifiers::default(), false, &home),
            Some(Command::SwitchFileMode)
        );
        assert_eq!(
            command_with_modifiers(
                &key(Key::Tab),
                Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
                false,
                &home
            ),
            None
        );
        assert_eq!(
            command_with_modifiers(
                &key(Key::Tab),
                Modifiers {
                    control: true,
                    ..Modifiers::default()
                },
                false,
                &home
            ),
            None
        );
        let file = Context {
            file_mode: true,
            space_launch: true,
            everything_enabled: true,
            query_empty: false,
            home: false,
            ..context()
        };
        assert_eq!(command(" ", false, false, &file), None);
        assert_eq!(
            command_with_modifiers(&key(Key::Return), Modifiers::default(), false, &file),
            Some(Command::OpenFile(false))
        );
        assert_eq!(
            command_with_modifiers(
                &key(Key::Return),
                Modifiers {
                    control: true,
                    ..Modifiers::default()
                },
                false,
                &file
            ),
            Some(Command::OpenFile(true))
        );
        assert_eq!(
            command(
                &key(Key::Escape),
                false,
                false,
                &Context {
                    query_empty: true,
                    ..file
                }
            ),
            Some(Command::ReturnToApps)
        );
        assert_eq!(
            command_with_modifiers(
                &key(Key::Tab),
                Modifiers::default(),
                false,
                &Context {
                    composing: true,
                    ..home
                }
            ),
            None
        );
    }

    #[test]
    fn import_escape_returns_to_query_and_modal_keys_are_never_launcher_commands() {
        let importing = Context {
            import_open: true,
            query_empty: false,
            ..context()
        };
        assert_eq!(
            command(&key(Key::Escape), false, false, &importing),
            Some(Command::CloseImport)
        );
        assert_eq!(command(&key(Key::Return), false, false, &importing), None);
        let modal = Context {
            native_dialog: true,
            ..importing
        };
        for text in [
            key(Key::Escape),
            key(Key::Return),
            key(Key::DownArrow),
            " ".into(),
        ] {
            assert_eq!(command(&text, false, false, &modal), None);
        }
    }

    #[test]
    fn configured_space_launch_respects_composition_and_repeat() {
        let context = Context {
            space_launch: true,
            ..context()
        };
        assert_eq!(command(" ", false, false, &context), Some(Command::Choose));
        assert_eq!(command(" ", false, true, &context), Some(Command::Consume));
        assert_eq!(
            command(
                " ",
                false,
                false,
                &Context {
                    composing: true,
                    ..context
                }
            ),
            None
        );
    }
}
