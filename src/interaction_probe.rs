//! Opt-in component integration checks; synthetic events are not native IME proof.

use std::cell::Cell;
use std::rc::Rc;

use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};

use crate::{AppWindow, confirmations};

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

fn click(app: &AppWindow, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
    for event in [
        WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        },
        WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        },
    ] {
        app.window().dispatch_event(event);
    }
}

pub fn run(app: &AppWindow, confirmations: &confirmations::Controller) -> Result<(), String> {
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
        // The home heading puts the first tile at content y=124.
        // Allow two logical pixels while checking the wrapped row is visible.
        app.get_selected_id() == first && app.get_home_scroll_y() >= -126.0,
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
    let results = app.get_result_items();
    let last = results.row_data(results.row_count() - 1).unwrap().id;
    press(app, Key::UpArrow);
    check(
        app.get_selected_id() == last && app.get_result_scroll_y() < -200.0,
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
    press(app, "zz-no-matches");
    check(
        app.get_input_text() == "zz-no-matches" && app.get_result_items().row_count() == 0,
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
    press(app, "jsb");
    check(
        app.get_selected_id() == "notes",
        "pinyin_initials_use_search_kernel",
    )?;
    app.invoke_clear_input();
    press(app, "\u{3000} CODE \u{a0}");
    check(
        app.get_selected_id() == "code" && app.get_result_items().row_count() == 2,
        "unicode_query_normalization",
    )?;
    check_confirmation(app, confirmations)?;
    app.invoke_clear_input();
    Ok(())
}

fn check_confirmation(
    app: &AppWindow,
    controller: &confirmations::Controller,
) -> Result<(), String> {
    let query = app.get_input_text();
    let selection = app.get_selected_id();
    let mut dialog = confirmations::question(
        "取消固定应用？",
        "取消固定后，应用将从首页移除。",
        "取消固定",
        "pinned",
    );
    dialog.subject = "取色卡".into();
    let answer = Rc::new(Cell::new(None));
    let reply = answer.clone();
    check(
        controller.show(dialog.clone(), false, move |accepted| {
            reply.set(Some(accepted))
        }),
        "confirmation_opens",
    )?;
    check(
        !controller.show(dialog.clone(), false, |_| {}),
        "confirmation_rejects_stacking",
    )?;
    click(app, 300.0, 40.0);
    check(
        app.get_confirmation_open() && !app.get_input_has_focus(),
        "confirmation_blocks_background_click",
    )?;
    press(app, Key::Return);
    check(
        !app.get_confirmation_open()
            && !app.get_native_dialog_active()
            && app.get_input_has_focus()
            && app.get_input_text() == query
            && app.get_selected_id() == selection
            && answer.get() == Some(false),
        "confirmation_defaults_to_cancel_and_restores_search",
    )?;

    let reply = answer.clone();
    controller.show(dialog, false, move |accepted| reply.set(Some(accepted)));
    press(app, Key::Tab);
    press(app, Key::Backtab);
    press(app, Key::Tab);
    press(app, Key::Return);
    check(
        answer.get() == Some(true),
        "confirmation_keeps_tab_focus_on_actions",
    )?;

    // Keep the existing Recycle Bin keyboard flow. The callback here records
    // only the answer, so no system action can execute.
    let mut dialog =
        confirmations::question("清空回收站？", "此操作无法撤销。", "清空回收站", "delete");
    dialog.default_accept = true;
    dialog.destructive = true;
    let reply = answer.clone();
    app.set_native_dialog_active(true);
    controller.show(dialog.clone(), true, move |accepted| {
        reply.set(Some(accepted))
    });
    app.window().dispatch_event(WindowEvent::KeyPressRepeated {
        text: Key::Return.into(),
    });
    check(
        app.get_confirmation_open(),
        "confirmation_ignores_held_enter",
    )?;
    press(app, Key::Return);
    check(
        !app.get_confirmation_open()
            && app.get_native_dialog_active()
            && answer.get() == Some(true),
        "recycle_confirmation_enter_keeps_launch_lock",
    )?;
    let reply = answer.clone();
    controller.show(dialog, true, move |accepted| reply.set(Some(accepted)));
    press(app, Key::Escape);
    check(
        !app.get_confirmation_open() && answer.get() == Some(false),
        "confirmation_escape_cancels",
    )?;
    app.set_native_dialog_active(false);
    app.invoke_focus_input();
    Ok(())
}
