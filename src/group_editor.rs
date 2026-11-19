//! UI-thread draft adapter. Native file inspection and persistence are submitted
//! to the existing bounded service; the editor never performs disk I/O.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use slint::{ComponentHandle, VecModel};
use velo_slint::core::group_run::{Report, State};
use velo_slint::core::launch_groups::{Detection, Draft, DraftTicket, Group, Member, RunningRule};
use velo_slint::core::service::Snapshot;

use crate::{AppWindow, GroupMemberRow, LauncherItem};

pub enum Command {
    Save(Group),
    Delete(String),
    Detection(DraftTicket, PathBuf),
    Run {
        id: String,
        only: Vec<String>,
        force: Option<String>,
    },
    Cancel,
}

enum Confirm {
    Close,
    Delete(String),
    Force(String),
}

pub struct Controller {
    app: slint::Weak<AppWindow>,
    snapshot: RefCell<Option<Arc<Snapshot>>>,
    draft: RefCell<Draft>,
    submit: Box<dyn Fn(Command) -> bool>,
    confirm: RefCell<Option<Confirm>>,
    detection: RefCell<Option<String>>,
    pending_save: RefCell<Option<Group>>,
    report: RefCell<Option<Report>>,
}

impl Controller {
    pub fn connect(app: &AppWindow, submit: impl Fn(Command) -> bool + 'static) -> Rc<Self> {
        let this = Rc::new(Self {
            app: app.as_weak(),
            snapshot: RefCell::default(),
            draft: RefCell::default(),
            submit: Box::new(submit),
            confirm: RefCell::default(),
            detection: RefCell::default(),
            pending_save: RefCell::default(),
            report: RefCell::default(),
        });
        let weak = Rc::downgrade(&this);
        app.on_groups_requested(move || {
            if let Some(this) = weak.upgrade() {
                this.sources();
            }
        });
        let weak = Rc::downgrade(&this);
        app.on_group_new(move || {
            if let Some(this) = weak.upgrade() {
                this.edit("");
            }
        });
        let weak = Rc::downgrade(&this);
        app.on_group_edit(move |id| {
            if let Some(this) = weak.upgrade() {
                this.edit(&id);
            }
        });
        let weak = Rc::downgrade(&this);
        app.on_group_filter_changed(move || {
            if let Some(this) = weak.upgrade() {
                this.sources();
            }
        });
        let weak = Rc::downgrade(&this);
        app.on_group_add(move |id| {
            if let Some(this) = weak.upgrade() {
                this.add(&id);
            }
        });
        let weak = Rc::downgrade(&this);
        app.on_group_remove(move |id| {
            if let Some(this) = weak.upgrade() {
                this.draft.borrow_mut().remove(&id);
                this.members();
            }
        });
        let weak = Rc::downgrade(&this);
        app.on_group_save(move || {
            if let Some(this) = weak.upgrade() {
                this.save();
            }
        });
        let weak = Rc::downgrade(&this);
        app.on_group_close(move || {
            if let Some(this) = weak.upgrade() {
                this.close();
            }
        });
        let weak = Rc::downgrade(&this);
        app.on_group_delete(move |id| {
            if let Some(this) = weak.upgrade() {
                this.ask(
                    Confirm::Delete(id.into()),
                    "删除此启动组合？应用文件和其他组合不会被删除。",
                );
            }
        });
        let weak = Rc::downgrade(&this);
        app.on_group_answer(move |yes| {
            if let Some(this) = weak.upgrade() {
                this.answer(yes);
            }
        });
        let weak = Rc::downgrade(&this);
        app.on_group_detection_selected(move |path| {
            if let Some(this) = weak.upgrade() {
                this.select_detection(PathBuf::from(path.as_str()));
            }
        });
        let weak = Rc::downgrade(&this);
        app.on_group_detect(move |id| {
            if let Some(this) = weak.upgrade() {
                *this.detection.borrow_mut() = Some(id.into());
                if let Some(app) = this.app.upgrade() {
                    app.invoke_import_native_requested(
                        crate::platform::windows::interaction::BROWSE_EXECUTABLE,
                    );
                }
            }
        });
        let weak = Rc::downgrade(&this);
        app.on_group_picker_cancelled(move || {
            if let Some(this) = weak.upgrade() {
                this.detection.borrow_mut().take();
            }
        });
        let weak = Rc::downgrade(&this);
        app.on_group_retry(move || {
            if let Some(this) = weak.upgrade() {
                this.retry(None);
            }
        });
        let weak = Rc::downgrade(&this);
        app.on_group_force(move |id| {
            if let Some(this) = weak.upgrade() {
                this.ask(
                    Confirm::Force(id.into()),
                    "无法确认该应用是否已运行。仍然打开可能创建重复窗口，是否继续？",
                );
            }
        });
        let weak = Rc::downgrade(&this);
        app.on_group_cancel(move || {
            if let Some(this) = weak.upgrade() {
                (this.submit)(Command::Cancel);
            }
        });
        this
    }

    fn notice(&self, text: &str) {
        if let Some(app) = self.app.upgrade() {
            app.set_group_notice(text.into());
        }
    }

    pub fn snapshot(&self, snapshot: Arc<Snapshot>) {
        if let Some(app) = self.app.upgrade() {
            app.set_groups_available(snapshot.groups_available);
            app.set_group_items(
                Rc::new(VecModel::from(
                    snapshot
                        .groups
                        .groups
                        .iter()
                        .map(|group| LauncherItem {
                            id: group.id.as_str().into(),
                            name: group.name.as_str().into(),
                            subtitle: format!(
                                "{} 个应用 · {}",
                                group.members.len(),
                                group
                                    .members
                                    .iter()
                                    .map(|member| member.display_name.as_str())
                                    .collect::<Vec<_>>()
                                    .join("、")
                            )
                            .into(),
                            ..LauncherItem::default()
                        })
                        .collect::<Vec<_>>(),
                ))
                .into(),
            );
        }
        *self.snapshot.borrow_mut() = Some(snapshot);
        self.sources();
        self.members();
    }

    fn edit(&self, id: &str) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if app.get_group_busy() || app.get_group_run_busy() || !app.get_groups_available() {
            return;
        }
        let group = if id.is_empty() {
            let Some(id) = crate::platform::windows::interaction::new_group_id() else {
                self.notice("无法创建组合标识，请重试");
                return;
            };
            Group {
                id,
                ..Group::default()
            }
        } else {
            let Some(group) = self
                .snapshot
                .borrow()
                .as_ref()
                .and_then(|snapshot| snapshot.groups.groups.iter().find(|group| group.id == id))
                .cloned()
            else {
                return;
            };
            group
        };
        app.set_group_name(group.name.as_str().into());
        app.set_group_aliases(group.aliases.join("，").into());
        self.draft.borrow_mut().open(group);
        self.detection.borrow_mut().take();
        app.set_group_mode(1);
        app.set_group_filter("".into());
        self.notice("拖入应用，或点击 + 添加；至少选择两个应用后保存。");
        self.members();
        self.sources();
    }

    fn sources(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let Some(snapshot) = self.snapshot.borrow().clone() else {
            return;
        };
        let query = app.get_group_filter();
        let mut items: Vec<_> = if query.trim().is_empty() {
            snapshot.catalog.applications().cloned().collect()
        } else {
            snapshot
                .catalog
                .search_applications(
                    &query,
                    &snapshot.config,
                    &snapshot.history,
                    velo_slint::core::service::now(),
                )
                .unwrap_or_default()
                .into_iter()
                .filter(|result| result.item.source != "System")
                .map(|result| result.item)
                .collect()
        };
        items.retain(|item| Member::from_app(item).is_ok());
        if query.trim().is_empty() {
            items.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        }
        app.set_group_sources(
            Rc::new(VecModel::from(
                items
                    .iter()
                    .map(|item| LauncherItem {
                        id: item.id.as_str().into(),
                        name: item.name.as_str().into(),
                        fallback: item
                            .name
                            .chars()
                            .next()
                            .unwrap_or('?')
                            .to_uppercase()
                            .to_string()
                            .into(),
                        ..LauncherItem::default()
                    })
                    .collect::<Vec<_>>(),
            ))
            .into(),
        );
        app.invoke_icon_viewport_changed();
    }

    fn members(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let draft = self.draft.borrow();
        let snapshot = self.snapshot.borrow();
        let rows: Vec<GroupMemberRow> = draft
            .group()
            .map(|group| {
                group
                    .members
                    .iter()
                    .map(|member| {
                        let item = snapshot.as_ref().and_then(|snapshot| {
                            snapshot.catalog.item(&member.app_id).map(AsRef::as_ref)
                        });
                        let status = match item {
                            Some(item) if member.running_program(item).is_some() => {
                                "自动跳过已运行应用"
                            }
                            Some(_) => "需要选择实际运行程序",
                            None => "应用入口失效，请移除后重新添加",
                        };
                        GroupMemberRow {
                            id: member.app_id.as_str().into(),
                            name: member.display_name.as_str().into(),
                            status: status.into(),
                            review: item.is_some_and(|item| member.running_program(item).is_none()),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        app.set_group_members(Rc::new(VecModel::from(rows)).into());
    }

    fn add(&self, id: &str) {
        let Some(item) = self
            .snapshot
            .borrow()
            .as_ref()
            .and_then(|snapshot| snapshot.catalog.item(id))
            .cloned()
        else {
            return;
        };
        if let Ok(member) = Member::from_app(&item) {
            let ticket = self.draft.borrow().ticket();
            let merged = self.draft.borrow_mut().merge(ticket, vec![member]);
            self.notice(if merged.duplicate > 0 {
                "此应用已经在组合中"
            } else if merged.overflow > 0 {
                "每个组合最多 20 个应用"
            } else {
                "已加入组合草稿，保存后生效"
            });
            self.members();
        }
    }

    fn select_detection(&self, path: PathBuf) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if self.detection.borrow().is_none() || app.get_group_busy() {
            return;
        }
        let ticket = self.draft.borrow().ticket();
        if (self.submit)(Command::Detection(ticket, path)) {
            app.set_group_busy(true);
            self.notice("正在检查运行程序…");
        } else {
            self.detection.borrow_mut().take();
            self.notice("其他操作正在进行，请稍后重新选择运行程序");
        }
    }

    pub fn detection_ready(&self, ticket: DraftTicket, result: velo_slint::core::Result<String>) {
        if let Some(app) = self.app.upgrade() {
            app.set_group_busy(false);
        }
        let member_id = self.detection.borrow_mut().take();
        if ticket != self.draft.borrow().ticket() {
            return;
        }
        if let Some(id) = member_id {
            match result {
                Ok(path) => {
                    self.draft.borrow_mut().set_running_rule(
                        &id,
                        RunningRule {
                            kind: Detection::Executable,
                            path,
                            ..RunningRule::default()
                        },
                    );
                    self.notice("运行检测已设置，保存组合后生效");
                }
                Err(_) => self.notice("请选择可读取的实际 .exe 主程序"),
            }
        }
        self.members();
    }

    fn edited_group(&self) -> Option<Group> {
        let app = self.app.upgrade()?;
        let mut group = self.draft.borrow().group()?.clone();
        group.name = app.get_group_name().trim().to_owned();
        group.aliases = app
            .get_group_aliases()
            .split([',', '，'])
            .map(str::trim)
            .filter(|alias| !alias.is_empty())
            .map(str::to_owned)
            .collect();
        Some(group)
    }

    fn save(&self) {
        let Some(group) = self.edited_group() else {
            return;
        };
        let valid = self
            .snapshot
            .borrow()
            .as_ref()
            .is_some_and(|snapshot| snapshot.groups.with_group(group.clone()).is_ok());
        if !valid {
            self.notice("请检查名称、别名是否重复，以及成员数量（2–20 个）");
            return;
        }
        if (self.submit)(Command::Save(group.clone())) {
            *self.pending_save.borrow_mut() = Some(group);
            if let Some(app) = self.app.upgrade() {
                app.set_group_busy(true);
            }
        } else {
            self.notice("其他操作正在进行，请稍后保存");
        }
    }

    pub fn managed(&self, kind: &str, success: bool) {
        if let Some(app) = self.app.upgrade() {
            app.set_group_busy(false);
            if success {
                self.pending_save.borrow_mut().take();
                self.draft.borrow_mut().close();
                app.set_group_mode(0);
                self.notice(if kind == "group_save" {
                    "组合已保存，输入名称即可搜索启动"
                } else {
                    "组合已删除"
                });
            } else {
                self.pending_save.borrow_mut().take();
                self.notice("保存未完成，原文件与草稿已保留；请检查目录权限后重试");
            }
        }
    }

    fn close(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if app.get_group_confirm() {
            self.answer(false);
            return;
        }
        if app.get_group_busy() {
            self.notice("正在检查或保存，请稍候");
            return;
        }
        if app.get_group_mode() == 1 {
            let edited = self.edited_group();
            if self.draft.borrow().dirty() || edited.as_ref() != self.draft.borrow().group() {
                self.ask(Confirm::Close, "放弃尚未保存的组合修改？");
                return;
            }
            self.draft.borrow_mut().close();
            app.set_group_mode(0);
        } else {
            app.invoke_show_groups(false);
        }
    }

    fn ask(&self, action: Confirm, text: &str) {
        *self.confirm.borrow_mut() = Some(action);
        if let Some(app) = self.app.upgrade() {
            app.set_group_confirm_text(text.into());
            app.invoke_suspend_input();
            app.set_group_confirm(true);
        }
    }

    fn answer(&self, yes: bool) {
        let action = self.confirm.borrow_mut().take();
        let Some(app) = self.app.upgrade() else {
            return;
        };
        app.set_group_confirm(false);
        app.invoke_focus_input();
        if !yes {
            return;
        }
        match action {
            Some(Confirm::Close) => {
                self.draft.borrow_mut().close();
                app.set_group_mode(0);
            }
            Some(Confirm::Delete(id)) => {
                if (self.submit)(Command::Delete(id)) {
                    app.set_group_busy(true);
                }
            }
            Some(Confirm::Force(id)) => self.retry(Some(id)),
            None => {}
        }
    }

    pub fn report(&self, report: Report) {
        if self
            .report
            .borrow()
            .as_ref()
            .is_some_and(|old| old.run_id > report.run_id)
        {
            return;
        }
        if let Some(app) = self.app.upgrade() {
            app.set_group_run_busy(!report.done);
            app.set_group_report_id(report.group_id.as_str().into());
            app.set_group_summary(report.summary().into());
            app.set_group_retryable(report.done && !report.retry_ids().is_empty());
            app.set_group_results(
                Rc::new(VecModel::from(
                    report
                        .entries
                        .iter()
                        .map(|entry| GroupMemberRow {
                            id: entry.app_id.as_str().into(),
                            name: entry.name.as_str().into(),
                            status: format!(
                                "{}{}{}",
                                entry.state.label(),
                                if entry.message.is_empty() { "" } else { " · " },
                                entry.message
                            )
                            .into(),
                            review: entry.state == State::NeedsReview,
                        })
                        .collect::<Vec<_>>(),
                ))
                .into(),
            );
        }
        *self.report.borrow_mut() = Some(report);
    }

    fn retry(&self, force: Option<String>) {
        let Some(report) = self.report.borrow().clone() else {
            return;
        };
        let only = force
            .as_ref()
            .map_or_else(|| report.retry_ids(), |id| vec![id.clone()]);
        if only.is_empty() {
            return;
        }
        if !(self.submit)(Command::Run {
            id: report.group_id,
            only,
            force,
        }) {
            self.notice("当前还有操作正在进行，请稍后重试");
        }
    }
}
