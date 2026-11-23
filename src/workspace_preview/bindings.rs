use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};

use slint::Weak;
use velo_slint::core::workspace::config::{AppKey, WorkspaceConfig};
use velo_slint::platform::windows::{
    hotkey::{self, Key},
    workspace::Invocation,
    workspace_input::{self, Admission, Button},
};

use crate::AppWindow;

pub(super) enum Message {
    Invoke(Invocation),
    Stopped,
}

pub(super) struct Keyboard {
    pub key: Key,
    pub active: Arc<AtomicBool>,
    events: Receiver<Message>,
    _service: hotkey::Service,
}

impl Keyboard {
    pub fn reserve(key: Key, app: Weak<AppWindow>) -> Result<Self, String> {
        // Keep the queue with the registration so disabled and replaced bindings
        // have no mailbox and cannot deliver old input to the current window.
        let (send, events) = mpsc::sync_channel(8);
        let active = Arc::new(AtomicBool::new(false));
        let generation = active.clone();
        let service = hotkey::Service::start(move |event| {
            if !generation.load(Ordering::Acquire) {
                return;
            }
            let message = match event {
                hotkey::Event::Pressed(_) => Message::Invoke(Invocation::capture()),
                hotkey::Event::Stopped => Message::Stopped,
            };
            if send.try_send(message).is_ok() {
                let _ = app.upgrade_in_event_loop(|app| app.invoke_workspace_event());
            }
        })
        .map_err(|error| error.to_string())?;
        service.bind(key)?;
        Ok(Self {
            key,
            active,
            events,
            _service: service,
        })
    }

    pub fn next(&self) -> Option<Message> {
        self.events.try_recv().ok()
    }
}

impl Drop for Keyboard {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}

pub(super) struct Mouse {
    pub button: Button,
    pub service: workspace_input::Service,
    pub admission: Arc<Admission>,
}

impl Mouse {
    pub fn reserve(button: Button, app: Weak<AppWindow>) -> Result<Self, String> {
        let admission = Arc::new(Admission::default());
        let service = workspace_input::Service::start(button, admission.clone(), move || {
            let _ = app.upgrade_in_event_loop(|app| app.invoke_workspace_mouse_event());
        })
        .map_err(|error| error.to_string())?;
        Ok(Self {
            button,
            service,
            admission,
        })
    }
}

impl Drop for Mouse {
    fn drop(&mut self) {
        self.admission.enabled.store(false, Ordering::Release);
    }
}

#[derive(Default)]
pub(super) struct Bindings {
    pub keyboard: Option<Keyboard>,
    pub mouse: Option<Mouse>,
}

pub(super) struct Prepared {
    pub enabled: bool,
    button: Option<Button>,
    excluded: Vec<AppKey>,
    new: Bindings,
}

impl Bindings {
    pub fn prepare(
        &self,
        config: &WorkspaceConfig,
        app: Weak<AppWindow>,
    ) -> Result<Prepared, String> {
        let key = Key::parse(&config.hotkey)?;
        let button = config
            .enabled
            .then_some(config.mouse)
            .flatten()
            .map(Button::from_preference);
        let mut new = Bindings::default();
        if config.enabled && self.keyboard.as_ref().is_none_or(|old| old.key != key) {
            new.keyboard = Some(Keyboard::reserve(key, app.clone())?);
        }
        if let Some(button) = button
            && self.mouse.as_ref().is_none_or(|old| old.button != button)
        {
            new.mouse = Some(
                Mouse::reserve(button, app).map_err(|error| format!("无法注册侧键：{error}"))?,
            );
        }
        Ok(Prepared {
            enabled: config.enabled,
            button,
            excluded: button.map_or_else(Vec::new, |_| config.excluded.clone()),
            new,
        })
    }

    pub fn commit(&mut self, mut prepared: Prepared) -> Bindings {
        let mut old = Bindings::default();
        if !prepared.enabled || prepared.new.keyboard.is_some() {
            old.keyboard = self.keyboard.take();
        }
        if prepared.button.is_none() || prepared.new.mouse.is_some() {
            old.mouse = self.mouse.take();
        }
        if let Some(keyboard) = &old.keyboard {
            keyboard.active.store(false, Ordering::Release);
        }
        if let Some(mouse) = &old.mouse {
            mouse.admission.enabled.store(false, Ordering::Release);
        }
        if let Some(keyboard) = prepared.new.keyboard.take() {
            self.keyboard = Some(keyboard);
        }
        if let Some(mouse) = prepared.new.mouse.take() {
            self.mouse = Some(mouse);
        }
        if let Some(keyboard) = &self.keyboard {
            keyboard.active.store(true, Ordering::Release);
        }
        if let Some(mouse) = &self.mouse {
            mouse.admission.set_excluded(prepared.excluded);
            mouse.admission.enabled.store(true, Ordering::Release);
        }
        old
    }
}
