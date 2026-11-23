use std::rc::Rc;

use slint::Timer;
use velo_slint::core::workspace::{
    gesture,
    radial::{Point, SlotId},
};
use velo_slint::platform::windows::{
    geometry::Layout,
    window::NativeWindow,
    workspace::{self, Invocation},
    workspace_input::{self, Sample},
};

use super::{Controller, Display, Foreground, Phase, radial};

pub(super) struct Hold {
    invocation: Invocation,
    layout: Layout,
    sample: Sample,
    gesture: gesture::Gesture,
}

impl Hold {
    pub(super) fn highlight(&self) -> Option<SlotId> {
        self.gesture.highlight()
    }

    fn point(layout: Layout, cursor: (i32, i32)) -> Point {
        let bounds = layout.bounds;
        workspace::point_on_board(
            f64::from(cursor.0 - bounds.x),
            f64::from(cursor.1 - bounds.y),
            bounds.width,
            bounds.height,
        )
    }

    fn motion(&self, sample: Sample) -> f32 {
        (sample.distance * 96.0 / f64::from(self.layout.dpi)) as f32
    }
}

impl Controller {
    pub(super) fn drain_mouse(self: &Rc<Self>) {
        let events = self
            .bindings
            .borrow()
            .mouse
            .as_ref()
            .map(|mouse| mouse.service.drain())
            .unwrap_or_default();
        for event in events {
            match event {
                workspace_input::Event::Pressed(invocation) => self.side_press(invocation),
                workspace_input::Event::Moved(sample) => self.side_move(sample),
                workspace_input::Event::Released(sample) => self.side_release(sample),
                workspace_input::Event::Cancelled(reason) => self.cancel_side_input(reason),
                workspace_input::Event::Stopped => {
                    self.cancel_side_input("mouse_stopped");
                    self.bindings.borrow_mut().mouse.take();
                    self.log("workspace_mouse_error", "reason=worker_stopped");
                }
            }
        }
    }

    fn side_press(self: &Rc<Self>, invocation: Invocation) {
        if self.hold.borrow().is_some() || !matches!(self.state.borrow().phase, Phase::Hidden) {
            self.hide("side_toggle", true);
            return;
        }
        let Some(launcher) = self.launcher.upgrade() else {
            return;
        };
        if launcher.get_native_dialog_active() || launcher.get_hotkey_recording() {
            return;
        }
        let Some((layout, cursor)) = self.invocation_layout(invocation, &launcher) else {
            return;
        };
        self.request_context(invocation);
        self.hold.replace(Some(Hold {
            invocation,
            layout,
            sample: Sample {
                at: invocation.at,
                point: cursor,
                distance: 0.0,
            },
            gesture: gesture::Gesture::new(Hold::point(layout, cursor)),
        }));
        self.log("workspace_side_down", "");
        let weak = Rc::downgrade(self);
        Timer::single_shot(
            gesture::HOLD_AFTER.saturating_sub(invocation.at.elapsed()),
            move || {
                if let Some(controller) = weak.upgrade() {
                    // A release can already be queued when the hold timer wakes.
                    controller.drain_mouse();
                    let sample = controller
                        .hold
                        .borrow()
                        .as_ref()
                        .filter(|hold| hold.invocation.at == invocation.at)
                        .map(|hold| Sample {
                            at: invocation.at + gesture::HOLD_AFTER,
                            ..hold.sample
                        });
                    if let Some(sample) = sample {
                        controller.side_move(sample);
                    }
                }
            },
        );
    }

    fn visible_point(&self, layout: Layout, cursor: (i32, i32)) -> Point {
        if self.state.borrow().visible_generation().is_some()
            && let Some(region) = self.region.get()
        {
            return region.screen_to_board(cursor);
        }
        Hold::point(layout, cursor)
    }

    fn side_move(self: &Rc<Self>, sample: Sample) {
        let (scene, phase, highlight, invocation, layout) = {
            let mut active = self.hold.borrow_mut();
            let Some(hold) = active.as_mut() else { return };
            // Keep the newest position in hook order. A timer may already
            // have advanced the hold age beyond an older native timestamp.
            hold.sample = Sample {
                at: sample.at.max(hold.sample.at),
                ..sample
            };
            let age = sample.at.saturating_duration_since(hold.invocation.at);
            let motion = hold.motion(sample);
            if hold.gesture.phase() == gesture::Phase::Pending
                && age < gesture::HOLD_AFTER
                && motion < gesture::DRAG_DIP
            {
                return;
            }
            let scene = self.prepare_scene(hold.invocation);
            hold.gesture.advance(
                age,
                motion,
                self.visible_point(hold.layout, sample.point),
                hold.layout.bounds.width as f32 * 96.0
                    / hold.layout.dpi as f32
                    / velo_slint::core::workspace::radial::WIDTH,
                &scene.board,
                |slot| scene.available(slot),
            );
            (
                scene,
                hold.gesture.phase(),
                hold.gesture.highlight(),
                hold.invocation,
                hold.layout,
            )
        };
        let display = match phase {
            gesture::Phase::Pending => return,
            gesture::Phase::Center => Display::Center,
            gesture::Phase::Selecting(zone) => Display::Zone(zone),
        };
        if matches!(self.state.borrow().phase, Phase::Hidden) {
            scene.set_display(display);
            self.open_radial(invocation, layout);
        } else if scene.display() != display {
            scene.set_display(display);
            if let Some(window) = self.window() {
                scene.show(&window);
            }
            if let Some(native) = self.native.get()
                && self.state.borrow().visible_generation().is_some()
            {
                self.region.set(None);
                if self.update_region(native).is_err() {
                    self.hide("region_error", false);
                    return;
                }
                self.log("workspace_radial_display", &format!("mode={display:?}"));
            }
        }
        if let Some(window) = self.window() {
            let previous = window.get_radial_hover();
            scene.highlight(&window, highlight);
            if previous != window.get_radial_hover() {
                self.log(
                    "workspace_side_highlight",
                    &format!(
                        "slot={} input_us={}",
                        window.get_radial_hover(),
                        sample.at.elapsed().as_micros()
                    ),
                );
            }
        }
    }

    fn side_release(self: &Rc<Self>, sample: Sample) {
        let final_motion = self.hold.borrow().as_ref().is_some_and(|hold| {
            hold.gesture.phase() == gesture::Phase::Pending
                && hold.motion(sample) >= gesture::DRAG_DIP
        });
        if final_motion {
            // Windows can coalesce the first move into the release packet.
            self.side_move(sample);
        }
        let Some(hold) = self.hold.borrow_mut().take() else {
            return;
        };
        let origin = hold.invocation;
        let layout = hold.layout;
        let point = self.visible_point(layout, sample.point);
        let travel = hold.motion(sample);
        let scene = self.prepare_scene(origin);
        let result = hold.gesture.release(
            sample.at.saturating_duration_since(origin.at),
            travel,
            point,
            &scene.board,
            |slot| scene.available(slot),
        );
        self.log(
            "workspace_side_release",
            &format!(
                "result={result:?} input_us={}",
                sample.at.elapsed().as_micros()
            ),
        );
        match result {
            gesture::Release::FullMenu => {
                scene.set_display(Display::Full);
                self.open_radial(origin, layout);
            }
            gesture::Release::Submit(slot) => self.submit_swipe(radial::id(slot), origin),
            _ => self.hide("side_release_cancel", true),
        }
    }

    fn submit_swipe(&self, item: i32, origin: Invocation) {
        if !origin.display_unchanged() {
            self.hide("display_changed", true);
            return;
        }
        let phase = self.state.borrow().phase;
        if phase.swipe_can_submit(
            origin.foreground,
            Foreground::current(),
            self.native.get().is_some_and(NativeWindow::active),
        ) {
            // Cold window creation can outlast the whole swipe. The selected
            // shape has already been hit-tested against the frozen layout.
            if matches!(phase, Phase::Opening) {
                self.log("workspace_fast_release", "window=opening");
            }
            self.run_radial_action(item, origin.foreground);
        } else {
            self.hide("side_release_cancel", true);
        }
    }

    fn cancel_side_input(&self, reason: &'static str) {
        if self.hold.borrow().is_some() {
            self.hide(reason, true);
        }
        self.log("workspace_mouse_cancelled", &format!("reason={reason}"));
    }

    fn open_radial(self: &Rc<Self>, invocation: Invocation, layout: Layout) {
        let Some(launcher) = self.launcher.upgrade() else {
            return;
        };
        if let Err(error) = self.open(invocation, layout, &launcher) {
            self.hide("open_error", false);
            self.log("workspace_error", &format!("operation=open error={error}"));
        }
    }
}
