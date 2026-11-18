use crate::{AppWindow, LauncherItem, LicenseEntry, PreviewEntry, SettingsDraft};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::rc::Rc;

pub const SCENES: &[&str] = &[
    "home",
    "home-empty-pinned",
    "list",
    "grid",
    "empty",
    "long-name",
    "custom",
    "settings-0",
    "settings-1",
    "settings-2",
    "settings-3",
    "settings-4",
    "settings-5",
    "settings-6",
    "scanning",
    "error",
    "update",
    "home-two-rows",
    "home-bottom",
    "home-empty-frequent",
    "custom-bottom",
    "settings-4-bottom",
    "settings-5-bottom",
    "settings-6-bottom",
    "missing-icon",
    "recording",
    "home-space",
    "list-space",
    "grid-space",
];

fn items(range: std::ops::Range<usize>) -> ModelRc<LauncherItem> {
    Rc::new(VecModel::from(
        [
            "代码编辑器",
            "开发工具箱",
            "中文笔记",
            "计算器",
            "项目文件夹",
            "终端",
            "系统设置",
            "任务看板",
        ]
        .iter()
        .cycle()
        .enumerate()
        .take(range.end)
        .skip(range.start)
        .map(|(index, name)| LauncherItem {
            id: format!("visual-{index}").into(),
            name: (*name).into(),
            subtitle: format!(r"C:\VeloFixture\应用 {index}\launcher.exe").into(),
            icon_key: [
                "code",
                "board",
                "apps",
                "calculator",
                "folder",
                "terminal",
                "settings",
                "board",
            ][index % 8]
                .into(),
            fallback: name.chars().next().unwrap_or('?').to_string().into(),
            system: index >= 6,
            pinned: index < 2,
            icon: slint::Image::default(),
        })
        .collect::<Vec<_>>(),
    ))
    .into()
}

pub fn prepare(app: &AppWindow, scene: &str, dark: bool) {
    app.set_product_version(env!("CARGO_PKG_VERSION").into());
    app.on_hotkey_parts(|text| {
        Rc::new(VecModel::from(
            text.split('+')
                .map(slint::SharedString::from)
                .collect::<Vec<_>>(),
        ))
        .into()
    });
    app.invoke_show_appearance(false);
    app.invoke_show_import(false);
    app.set_dark_theme(dark);
    app.set_reduce_motion(true);
    app.set_viewport_width(640.0);
    app.set_viewport_height(660.0);
    app.set_business_enabled(true);
    app.set_is_home(true);
    app.set_grid_results(false);
    app.set_space_launch(scene.ends_with("-space"));
    app.set_input_text("".into());
    app.set_scanning(false);
    app.set_hotkey_recording(false);
    app.set_home_scroll_y(0.0);
    app.set_result_scroll_y(0.0);
    app.set_import_scroll_y(0.0);
    app.set_settings_scroll_y(0.0);
    app.set_preview_entries(ModelRc::default());
    app.set_import_notice("".into());
    app.set_platform_error("".into());
    app.set_business_status("索引已就绪 · 8 项合成应用".into());
    app.set_settings_notice("".into());
    app.set_selection_notice("".into());
    app.set_update_message("当前版本已是最新版本（合成状态）".into());
    app.set_update_notes("".into());
    app.set_update_available(false);
    app.set_update_busy(false);
    app.set_hotkey_current("Alt+Space".into());
    app.set_hotkey_draft("Alt+Space".into());
    app.set_item_count(8);
    app.set_system_total_count(3);
    app.set_data_directory(r"C:\VeloFixture\data".into());
    app.set_third_party_licenses(
        Rc::new(VecModel::from(vec![
            LicenseEntry {
                title: "Microsoft Fluent UI System Icons（MIT）".into(),
                text: include_str!("../../ui/assets/fluent/LICENSE.txt").into(),
            },
            LicenseEntry {
                title: "go-pinyin（MIT）".into(),
                text: include_str!("../../licenses/go-pinyin-MIT.txt").into(),
            },
            LicenseEntry {
                title: "Go Unicode tables（BSD）".into(),
                text: include_str!("../../licenses/Go-BSD.txt").into(),
            },
        ]))
        .into(),
    );
    app.set_settings_draft(SettingsDraft {
        startup: false,
        space: true,
        maximum: "8".into(),
        fullscreen: false,
        everything: false,
        everything_executable: "".into(),
        theme: if dark { 2 } else { 1 },
        layout: 0,
        fuzzy: true,
        weight: "1".into(),
        program_files: true,
        noise: true,
        directories: r"C:\VeloFixture\apps".into(),
        refresh: "30".into(),
        updates: false,
    });
    app.set_pinned_items(items(0..2));
    app.set_frequent_items(items(2..5));
    app.set_system_items(items(5..8));
    app.set_result_items(items(0..8));
    app.set_custom_items(items(0..2));
    app.set_selected_id("visual-0".into());
    match scene {
        "home-empty-pinned" => app.set_pinned_items(ModelRc::default()),
        "home-empty-frequent" => app.set_frequent_items(ModelRc::default()),
        "home-two-rows" | "home-bottom" => {
            app.set_pinned_items(items(0..8));
            app.set_frequent_items(items(8..20));
            app.set_system_items(items(20..32));
            app.set_item_count(32);
            if scene == "home-bottom" {
                app.set_selected_id("visual-31".into());
                app.invoke_ensure_selection_visible(31);
            }
        }
        "missing-icon" => {
            app.set_pinned_items(
                Rc::new(VecModel::from(vec![LauncherItem {
                    id: "visual-0".into(),
                    name: "缺失图标样例".into(),
                    fallback: "缺".into(),
                    ..Default::default()
                }]))
                .into(),
            );
        }
        "list" | "list-space" | "grid" | "grid-space" | "empty" | "long-name" => {
            app.set_is_home(false);
            app.set_input_text("应用".into());
            app.set_grid_results(scene.starts_with("grid"));
            app.set_viewport_height(if scene == "empty" {
                210.0
            } else if scene.starts_with("grid") {
                396.0
            } else {
                588.0
            });
            if scene == "empty" {
                app.set_result_items(ModelRc::default());
                app.set_selected_id("".into());
            } else if scene == "long-name" {
                app.set_result_items(Rc::new(VecModel::from(vec![LauncherItem {
                    id: "visual-0".into(),
                    name: "用于检查中文换行与省略的很长应用名称 · Developer Workspace Edition 2026".into(),
                    subtitle: r"C:\VeloFixture\很长的目录名称\Developer Workspace\用于检查路径省略的应用程序.exe --profile=合成样例".into(),
                    icon_key: "code".into(), fallback: "用".into(), ..Default::default()
                }])).into());
                app.set_viewport_height(210.0);
            }
        }
        "custom" | "custom-bottom" => {
            app.invoke_show_import(true);
            app.set_import_summary("1 项合成预览 · 未执行文件访问".into());
            app.set_import_path(r"C:\VeloFixture\apps\示例.exe".into());
            app.set_preview_entries(
                Rc::new(VecModel::from(vec![PreviewEntry {
                    key: "preview-0".into(),
                    name: "示例自定义应用".into(),
                    path: r"C:\VeloFixture\apps\示例.exe".into(),
                    status: "可添加（合成）".into(),
                    valid: true,
                }]))
                .into(),
            );
            if scene == "custom-bottom" {
                app.set_custom_items(items(0..12));
                app.set_import_scroll_y(-790.0);
            }
        }
        "scanning" => {
            app.set_scanning(true);
            app.set_business_status("正在扫描 · 合成状态，不访问磁盘".into());
        }
        "error" => {
            app.set_platform_error("无法读取合成索引目录，请检查权限后重试。".into());
            app.set_business_status("部分目录不可访问 · 保留上次快照".into());
        }
        "update" => {
            app.invoke_show_appearance(true);
            app.set_settings_tab(6);
            app.set_update_available(true);
            app.set_update_message("发现新版本 · 仅展示合成状态，不下载或安装".into());
            app.set_update_notes("合成版本说明\n修复示例问题；保留本机数据。".into());
        }
        _ => {
            if let Some(tab) = scene.strip_prefix("settings-") {
                app.invoke_show_appearance(true);
                app.set_settings_tab(tab.trim_end_matches("-bottom").parse().unwrap_or(0));
            } else if scene == "recording" {
                app.invoke_show_appearance(true);
                app.set_settings_tab(1);
                app.set_hotkey_recording(true);
            }
        }
    }
    app.window().set_size(slint::LogicalSize::new(
        app.get_viewport_width(),
        app.get_viewport_height(),
    ));
    app.invoke_focus_input();
}

// The tab's changed-handler resets scrolling and disclosure state on the next
// event-loop pass. Apply bottom states afterwards, before the later capture.
pub fn settle(app: &AppWindow, scene: &str) {
    match scene {
        "settings-4-bottom" => app.set_settings_scroll_y(-69.0),
        "settings-5-bottom" => app.set_settings_scroll_y(-29.0),
        "settings-6-bottom" => {
            app.invoke_show_licenses(true);
            app.set_settings_scroll_y(-950.0);
        }
        _ => {}
    }
}
