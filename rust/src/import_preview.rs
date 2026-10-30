//! Session-only import preview. Filesystem metadata is inspected on a bounded
//! worker; no file is executed, resolved through Shell, or written to the library.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::time::Duration;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::measurement::{self, Observer};
use crate::platform::windows::interaction;
use crate::{AppWindow, PreviewEntry};

const LIMIT: usize = 256;
type InspectionJob = (i32, Vec<PathBuf>);

#[derive(Default)]
struct Draft {
    generation: i32,
    pending: Vec<PathBuf>,
    entries: Vec<PreviewEntry>,
    in_flight: usize,
}

impl Draft {
    fn clear(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending.clear();
        self.entries.clear();
        self.in_flight = 0;
    }

    fn merge(&mut self, generation: i32, entries: impl Iterator<Item = PreviewEntry>) -> bool {
        if generation != self.generation {
            return false;
        }
        self.in_flight = self.in_flight.saturating_sub(1);
        for entry in entries {
            if self.entries.len() == LIMIT {
                break;
            }
            if !self.entries.iter().any(|old| old.key == entry.key) {
                self.entries.push(entry);
            }
        }
        true
    }
}

fn inspect(path: &Path) -> PreviewEntry {
    let display = path.to_string_lossy().into_owned();
    let normalized = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    let key = normalized.to_string_lossy().to_lowercase();
    let extension = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let status = if !path.is_absolute() {
        "需要完整文件路径"
    } else if !matches!(extension.as_str(), "exe" | "lnk") {
        "不支持的类型，仅支持 .exe / .lnk"
    } else {
        match std::fs::metadata(path) {
            Err(_) => "文件不存在或无法读取",
            Ok(metadata) if !metadata.is_file() => "请添加文件，而非目录",
            Ok(_) if extension == "lnk" => "快捷方式 · 文件存在，目标尚未解析",
            Ok(_) => "程序 · 文件存在",
        }
    };
    PreviewEntry {
        key: key.into(),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
            .into(),
        path: display.into(),
        status: status.into(),
        valid: status.starts_with("快捷方式") || status.starts_with("程序"),
    }
}

pub struct Controller {
    app: slint::Weak<AppWindow>,
    draft: RefCell<Draft>,
    queued: Cell<bool>,
    native_request: Cell<i32>,
    worker: RefCell<Option<SyncSender<InspectionJob>>>,
    observer: Option<Observer>,
    diagnostics: bool,
}

impl Controller {
    pub fn connect(app: &AppWindow, diagnostics: bool, observer: Option<Observer>) -> Rc<Self> {
        let controller = Rc::new(Self {
            app: app.as_weak(),
            draft: RefCell::default(),
            queued: Cell::new(false),
            native_request: Cell::new(0),
            worker: RefCell::new(None),
            observer,
            diagnostics,
        });
        app.set_import_elevated(interaction::elevated());
        controller.log(
            "import_permission",
            &format!("elevated={}", app.get_import_elevated()),
        );
        let weak = Rc::downgrade(&controller);
        app.on_import_path_requested(move |text| {
            if let Some(controller) = weak.upgrade() {
                let trimmed = text.trim();
                let trimmed = trimmed
                    .strip_prefix('"')
                    .and_then(|text| text.strip_suffix('"'))
                    .unwrap_or(trimmed);
                if !trimmed.is_empty() {
                    controller.add_paths(vec![PathBuf::from(trimmed)], "path");
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_import_preview_ready(move |generation, entries| {
            if let Some(controller) = weak.upgrade() {
                let accepted = controller
                    .draft
                    .borrow_mut()
                    .merge(generation, entries.iter());
                controller.log(
                    "import_preview_ready",
                    &format!(
                        "generation={generation} accepted={accepted} entries={}",
                        entries.row_count()
                    ),
                );
                if accepted {
                    controller.publish();
                    if let Some(app) = controller.app.upgrade() {
                        app.set_import_notice("检查完成；相同路径只保留一项。".into());
                    }
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_import_native_requested(move |request| {
            if let Some(controller) = weak.upgrade() {
                controller.native(request);
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_import_native_completed(move |action, paths, error| {
            if let Some(controller) = weak.upgrade() {
                controller.native_completed(action, paths, error);
            }
        });
        controller
    }

    fn log(&self, name: &str, fields: &str) {
        measurement::event(&self.observer, name, fields);
        if self.diagnostics {
            eprintln!("event={name} {fields}");
        }
    }

    pub fn add_paths(self: &Rc<Self>, paths: Vec<PathBuf>, source: &str) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        // A delayed drop/worker must never reopen a hidden window or a modal.
        if !app.get_presented() || app.get_native_dialog_active() {
            return;
        }
        app.invoke_show_import(true);
        app.set_drop_hovering(false);
        let count = paths.len();
        let mut draft = self.draft.borrow_mut();
        let available = LIMIT.saturating_sub(draft.pending.len() + draft.entries.len());
        let limited = count > available;
        draft.pending.extend(paths.into_iter().take(available));
        drop(draft);
        app.set_import_notice(
            if limited {
                "预览最多保留 256 项；超出部分未接收。"
            } else {
                "正在检查文件…"
            }
            .into(),
        );
        self.log(
            "import_paths_received",
            &format!("source={source} count={count} limited={limited}"),
        );
        if self.queued.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        // winit reports one DroppedFile per item; coalesce one Explorer gesture.
        slint::Timer::single_shot(Duration::from_millis(40), move || {
            if let Some(controller) = weak.upgrade() {
                controller.queued.set(false);
                controller.submit();
            }
        });
    }

    fn submit(&self) {
        let mut draft = self.draft.borrow_mut();
        if draft.pending.is_empty() {
            return;
        }
        if self.worker.borrow().is_none() {
            let (sender, receiver) = sync_channel::<InspectionJob>(2);
            let weak = self.app.clone();
            let worker = std::thread::Builder::new()
                .name("velo-import-preview".into())
                .spawn(move || {
                    while let Ok((generation, paths)) = receiver.recv() {
                        // Slint models/images are created only after returning to UI.
                        let entries: Vec<_> = paths
                            .iter()
                            .map(|path| {
                                let entry = inspect(path);
                                (
                                    entry.key.to_string(),
                                    entry.name.to_string(),
                                    entry.path.to_string(),
                                    entry.status.to_string(),
                                    entry.valid,
                                )
                            })
                            .collect();
                        let _ = weak.upgrade_in_event_loop(move |app| {
                            let entries: Vec<_> = entries
                                .into_iter()
                                .map(|(key, name, path, status, valid)| PreviewEntry {
                                    key: key.into(),
                                    name: name.into(),
                                    path: path.into(),
                                    status: status.into(),
                                    valid,
                                })
                                .collect();
                            app.invoke_import_preview_ready(
                                generation,
                                Rc::new(VecModel::from(entries)).into(),
                            );
                        });
                    }
                });
            if worker.is_err() {
                if let Some(app) = self.app.upgrade() {
                    app.set_import_notice("无法创建文件检查线程，请重试。".into());
                }
                draft.pending.clear();
                return;
            }
            // Closing the bounded sender ends the idle worker. Do not join a
            // potentially slow filesystem call on the UI thread during shutdown.
            *self.worker.borrow_mut() = Some(sender);
        }
        let request = (draft.generation, std::mem::take(&mut draft.pending));
        if self
            .worker
            .borrow()
            .as_ref()
            .unwrap()
            .try_send(request)
            .is_ok()
        {
            draft.in_flight += 1;
        } else if let Some(app) = self.app.upgrade() {
            app.set_import_notice("文件检查队列已满，请稍后重新添加。".into());
            self.log("import_queue_rejected", "capacity=2");
        }
        drop(draft);
        self.publish();
    }

    fn publish(&self) {
        if let Some(app) = self.app.upgrade() {
            let draft = self.draft.borrow();
            app.set_preview_entries(Rc::new(VecModel::from(draft.entries.clone())).into());
            app.set_import_busy(draft.in_flight > 0 || !draft.pending.is_empty());
            let valid = draft.entries.iter().filter(|entry| entry.valid).count();
            self.log(
                "import_preview_published",
                &format!(
                    "generation={} total={} supported={valid} invalid={} busy={}",
                    draft.generation,
                    draft.entries.len(),
                    draft.entries.len() - valid,
                    app.get_import_busy()
                ),
            );
            app.set_import_summary(
                format!(
                    "{} 项预览 · {valid} 个支持的文件 · {} 个无效项",
                    draft.entries.len(),
                    draft.entries.len() - valid
                )
                .into(),
            );
        }
    }

    fn native(self: &Rc<Self>, request: i32) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if !app.get_presented() || !app.get_import_open() || app.get_native_dialog_active() {
            return;
        }
        let owner = app
            .window()
            .with_winit_window(|window| match window.window_handle().ok()?.as_raw() {
                RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
                _ => None,
            })
            .flatten();
        let Some(owner) = owner else {
            return;
        };
        app.set_native_dialog_active(true);
        self.native_request.set(request);
        app.invoke_suspend_input();
        self.log("native_interaction_started", &format!("request={request}"));
        let weak = self.app.clone();
        let started = std::thread::Builder::new()
            .name("velo-native-interaction".into())
            .spawn(move || {
                let result = interaction::run(owner, request);
                let _ = weak.upgrade_in_event_loop(move |app| {
                    let paths: Vec<SharedString> = result
                        .paths
                        .into_iter()
                        .map(|path| path.to_string_lossy().into_owned().into())
                        .collect();
                    app.invoke_import_native_completed(
                        result.action,
                        Rc::new(VecModel::from(paths)).into(),
                        result.error.into(),
                    );
                });
            });
        if started.is_err() {
            app.invoke_import_native_completed(0, ModelRc::default(), "无法打开原生交互。".into());
        }
    }

    fn native_completed(
        self: &Rc<Self>,
        action: i32,
        paths: ModelRc<SharedString>,
        error: SharedString,
    ) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        app.set_native_dialog_active(false);
        let request = self.native_request.replace(0);
        self.log(
            "native_interaction_completed",
            &format!(
                "action={action} paths={} error={}",
                paths.row_count(),
                !error.is_empty()
            ),
        );
        if !error.is_empty() {
            app.set_import_notice(error);
        }
        if request == interaction::MENU
            && matches!(action, interaction::BROWSE | interaction::CLEAR)
        {
            self.native(action);
            return;
        }
        match action {
            interaction::BROWSE if paths.row_count() > 0 => self.add_paths(
                paths
                    .iter()
                    .map(|text| PathBuf::from(text.as_str()))
                    .collect(),
                "picker",
            ),
            interaction::CLEAR => {
                self.draft.borrow_mut().clear();
                self.publish();
                app.set_import_notice("预览已清空；原文件未修改。".into());
            }
            interaction::BACK => app.invoke_show_import(false),
            _ => {}
        }
        // Menu "clear" opens the confirmation as its own owned interaction.
        app.invoke_native_interaction_finished();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_inspects_files_without_launching_and_rejects_invalid_inputs() {
        let folder = std::env::temp_dir().join(format!("velo-rs07-preview-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let executable = folder.join("中文 程序.EXE");
        let shortcut = folder.join("应用 空格.lnk");
        std::fs::write(&executable, b"preview fixture, never executable").unwrap();
        std::fs::write(&shortcut, b"target resolution belongs to core migration").unwrap();
        assert!(inspect(&executable).valid);
        assert!(inspect(&shortcut).status.contains("尚未解析"));
        assert!(!inspect(&folder.join("notes.txt")).valid);
        assert!(!inspect(&folder.join("missing.exe")).valid);
        assert!(!inspect(Path::new("relative.exe")).valid);
        let directory = folder.join("directory.exe");
        std::fs::create_dir_all(&directory).unwrap();
        assert!(!inspect(&directory).valid);
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn clear_rejects_late_results_and_windows_case_duplicates_are_not_published_twice() {
        let mut draft = Draft::default();
        let first = inspect(Path::new("D:\\中文 目录\\App.exe"));
        let duplicate = inspect(Path::new("d:\\中文 目录\\APP.EXE"));
        assert!(draft.merge(0, vec![first.clone(), duplicate].into_iter()));
        assert_eq!(draft.entries.len(), 1);
        draft.clear();
        assert!(!draft.merge(0, vec![first.clone()].into_iter()));
        assert!(draft.entries.is_empty());
        assert!(draft.merge(draft.generation, vec![first].into_iter()));
        assert_eq!(draft.entries.len(), 1);
    }
}
