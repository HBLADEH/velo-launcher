//! UI-thread adapters. Disk, Shell and persistence remain on bounded workers.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::fixtures::{self, FixtureItem, Group};
use crate::input::{self, Command, Context};
use crate::navigation;
use crate::{AppWindow, LauncherItem};

fn model(items: impl Iterator<Item = FixtureItem>) -> ModelRc<LauncherItem> {
    Rc::new(VecModel::from(
        items
            .map(|item| LauncherItem {
                id: item.id.into(),
                name: item.name.into(),
                subtitle: item.subtitle.into(),
                icon_key: item.icon_key.into(),
                fallback: item
                    .name
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string()
                    .into(),
                system: item.group == Group::System,
                pinned: item.pinned,
                icon: slint::Image::default(),
            })
            .collect::<Vec<_>>(),
    ))
    .into()
}

pub fn connect(
    app: &AppWindow,
    diagnostics: bool,
    snapshot: fixtures::Snapshot,
    measurement: Option<crate::measurement::Observer>,
) {
    for (group, setter) in [
        (
            Group::Pinned,
            AppWindow::set_pinned_items as fn(&AppWindow, ModelRc<LauncherItem>),
        ),
        (Group::Frequent, AppWindow::set_frequent_items),
        (Group::System, AppWindow::set_system_items),
    ] {
        setter(
            app,
            model(
                snapshot
                    .home
                    .iter()
                    .copied()
                    .filter(|item| item.group == group),
            ),
        );
    }
    let initial = snapshot.home.clone();
    app.set_item_count(fixtures::ITEMS.len() as i32);
    app.set_selected_id(initial.first().map_or("", |item| item.id).into());
    let section_lengths = [Group::Pinned, Group::Frequent, Group::System]
        .map(|group| initial.iter().filter(|item| item.group == group).count());
    let home_rows = navigation::choice_rows(&section_lengths, 6);
    let visible = Rc::new(RefCell::new(initial));
    let weak_app = app.as_weak();
    let query_visible = visible.clone();
    app.on_input_edited(move |text| {
        let Some(app) = weak_app.upgrade() else {
            return;
        };
        let home = velo_slint::core::normalize::normalize(&text).is_empty();
        let items = if home {
            snapshot.home.clone()
        } else {
            match snapshot.query(&text) {
                Ok(items) => items,
                Err(_) => {
                    eprintln!("event=search_error operation=fixture_query");
                    app.set_selection_notice("搜索失败，请重启后重试".into());
                    return;
                }
            }
        };
        app.set_is_home(home);
        app.set_selected_id(items.first().map_or("", |item| item.id).into());
        app.set_selection_notice("".into());
        app.set_result_items(model(items.iter().copied()));
        *query_visible.borrow_mut() = items;
        app.invoke_layout_changed();
        if let Some(observer) = &measurement {
            observer.input(&app, text.len());
        }
        if diagnostics {
            eprintln!(
                "event=input_edited utf8_bytes={} fixture_results={} home={home}",
                text.len(),
                query_visible.borrow().len()
            );
        }
    });
    let weak_app = app.as_weak();
    let chosen_visible = visible.clone();
    app.on_item_chosen(move |id| {
        let items = chosen_visible.borrow();
        let Some(item) = fixtures::selection(&items, &id) else {
            return;
        };
        if let Some(app) = weak_app.upgrade() {
            app.set_selected_id(id.clone());
            app.set_selection_notice(format!("已选择：{}", item.name).into());
            app.invoke_focus_input();
        }
        // Only fixed, synthetic IDs are logged. Never execute a fixture path.
        eprintln!("event=fixture_chosen id={} action=record_only", item.id);
    });
    connect_commands(app, move |app, home, direction| {
        let items = visible.borrow();
        let list_rows;
        let rows = if home {
            &home_rows
        } else {
            list_rows = navigation::choice_rows(&[items.len()], 1);
            &list_rows
        };
        let selected = app.get_selected_id();
        let current = items.iter().position(|item| item.id == selected.as_str());
        if let Some(index) = navigation::navigate(rows, current, direction) {
            app.set_selected_id(items[index].id.into());
            app.set_selection_notice("".into());
            app.invoke_ensure_selection_visible(index as i32);
            if diagnostics {
                eprintln!(
                    "event=keyboard_navigation direction={direction:?} index={index} home={home}"
                );
            }
        }
    });
}

pub fn connect_commands(
    app: &AppWindow,
    navigate: impl Fn(&AppWindow, bool, navigation::Direction) + 'static,
) {
    let weak_app = app.as_weak();
    app.on_key_requested(move |text, modified, repeat| {
        let Some(app) = weak_app.upgrade() else {
            return false;
        };
        let context = Context {
            composing: app.get_native_composing() || app.get_has_preedit(),
            appearance_open: app.get_appearance_open(),
            import_open: app.get_import_open(),
            native_dialog: app.get_native_dialog_active(),
            input_focused: app.get_input_has_focus(),
            home: app.get_is_home() || app.get_grid_results(),
            query_empty: app.get_input_text().is_empty(),
            space_launch: app.get_space_launch(),
        };
        let Some(command) = input::command(&text, modified, repeat, &context) else {
            return false;
        };
        match command {
            Command::Navigate(direction) => navigate(&app, context.home, direction),
            Command::Choose => {
                if !app.get_selected_id().is_empty() {
                    app.invoke_item_chosen(app.get_selected_id());
                }
            }
            Command::CloseAppearance => app.invoke_show_appearance(false),
            Command::CloseImport => app.invoke_show_import(false),
            Command::ClearQuery => app.invoke_clear_input(),
            Command::Hide => app.invoke_hide_requested(),
            Command::Consume => {}
        }
        true
    });
}
