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
}

pub fn command(text: &str, modified: bool, repeat: bool, context: &Context) -> Option<Command> {
    if context.composing || modified || context.native_dialog {
        return None;
    }
    let key = |key: Key| text.chars().eq(std::iter::once(char::from(key)));
    if key(Key::Escape) {
        return Some(if context.appearance_open {
            Command::CloseAppearance
        } else if context.import_open {
            Command::CloseImport
        } else if !context.query_empty {
            Command::ClearQuery
        } else {
            Command::Hide
        });
    }
    if context.appearance_open || context.import_open || !context.input_focused {
        return None;
    }
    Some(if key(Key::DownArrow) {
        Command::Navigate(Direction::Down)
    } else if key(Key::UpArrow) {
        Command::Navigate(Direction::Up)
    } else if context.home && key(Key::LeftArrow) {
        Command::Navigate(Direction::Left)
    } else if context.home && key(Key::RightArrow) {
        Command::Navigate(Direction::Right)
    } else if key(Key::Return) {
        if repeat {
            Command::Consume
        } else {
            Command::Choose
        }
    } else {
        // Space is ordinary text (the product's optional space_launch setting
        // is not part of this fixed-data prototype). Clipboard/selection,
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
        }
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
}
