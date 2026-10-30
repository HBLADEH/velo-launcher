//! Opt-in component integration checks; synthetic events are not native IME proof.

use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, Model};

use crate::AppWindow;

fn press(app: &AppWindow, text: impl Into<slint::SharedString>) {
    let text = text.into();
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

fn check(passed: bool, name: &str) -> Result<(), String> {
    eprintln!("event=input_smoke_assert name={name} passed={passed}");
    if passed { Ok(()) } else { Err(name.into()) }
}

pub fn run(app: &AppWindow) -> Result<(), String> {
    app.set_reduce_motion(true);
    app.invoke_clear_input();
    let first = app.get_selected_id();
    check(app.get_input_has_focus(), "initial_component_focus")?;
    press(app, Key::UpArrow);
    check(
        app.get_selected_id() == "environment" && app.get_home_scroll_y() < -300.0,
        "home_wrap_and_scroll",
    )?;
    press(app, Key::DownArrow);
    check(
        app.get_selected_id() == first && app.get_home_scroll_y() >= -116.0,
        "home_wrap_back",
    )?;
    press(app, Key::Return);
    check(
        !app.get_selection_notice().is_empty(),
        "enter_records_fixture",
    )?;

    press(app, "code");
    check(
        !app.get_is_home()
            && app.get_result_items().row_count() == 2
            && app.get_selected_id() == "code",
        "text_filters_and_resets_selection",
    )?;
    press(app, Key::DownArrow);
    check(app.get_selected_id() == "editor", "list_down")?;
    press(app, Key::LeftArrow);
    press(app, Key::RightArrow);
    check(
        app.get_selected_id() == "editor",
        "list_horizontal_keys_edit_text",
    )?;
    press(app, Key::Return);
    check(
        app.get_selection_notice().contains("Code Editor Preview"),
        "list_enter_records_selection",
    )?;

    let selected = app.get_selected_id();
    let notice = app.get_selection_notice();
    app.set_native_composing(true);
    for key in [Key::Return, Key::UpArrow, Key::DownArrow, Key::Escape] {
        press(app, key);
    }
    check(
        app.get_selected_id() == selected
            && app.get_selection_notice() == notice
            && app.get_input_text() == "code",
        "native_composition_blocks_launcher_commands",
    )?;
    app.set_native_composing(false);
    press(app, Key::Escape);
    check(
        app.get_is_home() && app.get_input_text().is_empty() && app.get_input_has_focus(),
        "escape_clears_and_restores_focus",
    )?;

    press(app, "系统");
    press(app, Key::UpArrow);
    check(
        app.get_selected_id() == "environment" && app.get_result_scroll_y() < -200.0,
        "list_wrap_and_scroll",
    )?;
    app.invoke_show_appearance(true);
    check(!app.get_input_has_focus(), "panel_takes_focus")?;
    press(app, Key::Escape);
    check(
        !app.get_appearance_open() && app.get_input_has_focus(),
        "panel_escape_restores_focus",
    )?;
    press(app, Key::Escape);
    press(app, "z");
    check(
        app.get_input_text() == "z" && app.get_result_items().row_count() == 0,
        "typing_after_panel_close_and_empty_results",
    )?;
    press(app, Key::Return);
    check(
        app.get_selection_notice().is_empty(),
        "empty_results_cannot_launch",
    )?;
    app.invoke_clear_input();
    press(app, "code");
    let saved_query = app.get_input_text();
    let saved_selection = app.get_selected_id();
    app.invoke_show_import(true);
    check(
        app.get_import_open() && !app.get_input_has_focus(),
        "import_takes_focus",
    )?;
    press(app, Key::Return);
    check(
        app.get_selected_id() == saved_selection && app.get_selection_notice().is_empty(),
        "import_does_not_launch_search_selection",
    )?;
    app.set_native_dialog_active(true);
    press(app, Key::Escape);
    check(app.get_import_open(), "modal_escape_is_not_launcher_escape")?;
    app.set_native_dialog_active(false);
    press(app, Key::Escape);
    check(
        !app.get_import_open() && app.get_input_has_focus() && app.get_input_text() == saved_query,
        "import_escape_restores_query_and_focus",
    )?;
    app.invoke_show_import(true);
    app.invoke_show_appearance(true);
    check(
        !app.get_import_open() && app.get_appearance_open(),
        "settings_ends_import_protection",
    )?;
    app.invoke_show_appearance(false);
    app.invoke_clear_input();
    Ok(())
}
