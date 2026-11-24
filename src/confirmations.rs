//! One modal answer at a time, owned by the Slint event loop.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, SharedString};

use crate::{AppWindow, Confirmation};

struct Pending {
    during_launch: bool,
    answer: Box<dyn FnOnce(bool)>,
}

pub struct Controller {
    app: slint::Weak<AppWindow>,
    pending: RefCell<Option<Pending>>,
}

impl Controller {
    pub fn connect(app: &AppWindow) -> Rc<Self> {
        let controller = Rc::new(Self {
            app: app.as_weak(),
            pending: RefCell::default(),
        });
        let weak = Rc::downgrade(&controller);
        app.on_confirmation_answered(move |accepted| {
            if let Some(controller) = weak.upgrade() {
                controller.finish(accepted);
            }
        });
        controller
    }

    pub fn show(
        &self,
        content: Confirmation,
        during_launch: bool,
        answer: impl FnOnce(bool) + 'static,
    ) -> bool {
        let Some(app) = self.app.upgrade() else {
            return false;
        };
        if !app.get_presented()
            || app.get_native_dialog_active() != during_launch
            || self.pending.borrow().is_some()
        {
            return false;
        }
        self.pending.replace(Some(Pending {
            during_launch,
            answer: Box::new(answer),
        }));
        app.invoke_suspend_input();
        app.set_native_dialog_active(true);
        app.set_drop_hovering(false);
        app.set_confirmation_layout_height(0.0);
        app.set_confirmation(content);
        app.set_confirmation_open(true);
        app.invoke_focus_confirmation();
        app.invoke_layout_changed();
        true
    }

    fn finish(&self, accepted: bool) {
        let Some(pending) = self.pending.borrow_mut().take() else {
            return;
        };
        let Some(app) = self.app.upgrade() else {
            return;
        };
        // Slint ignores focus-out events after a control has been disabled.
        // Move focus before hiding the dialog so reopening starts cleanly.
        app.invoke_suspend_input();
        app.set_confirmation_open(false);
        app.set_confirmation(Confirmation::default());
        // The launch completion still owns the input lock and visibility.
        if !pending.during_launch {
            app.set_native_dialog_active(false);
        }
        (pending.answer)(accepted);
        app.invoke_restore_confirmation_focus();
        if !app.get_native_dialog_active() {
            app.invoke_native_interaction_finished();
        }
    }
}

pub fn question(
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    confirm_label: &str,
    icon_key: &str,
) -> Confirmation {
    Confirmation {
        title: title.into(),
        detail: detail.into(),
        confirm_label: confirm_label.into(),
        cancel_label: "取消".into(),
        icon_key: icon_key.into(),
        ..Confirmation::default()
    }
}
