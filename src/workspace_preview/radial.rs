use std::cell::Cell;

use slint::{ModelRc, VecModel};
use velo_slint::core::workspace::{
    actions::WorkspaceAction,
    config::Layouts,
    radial::{Board, Outline, Point, Slot, SlotId, Zone},
};
use velo_slint::platform::windows::workspace_actions::Application;

use crate::{RadialTile, WorkspaceWindow};

pub(super) const HUB: i32 = -1;
pub(super) const EDIT: i32 = -2;
const NONE: i32 = -3;

pub(super) fn id(slot: SlotId) -> i32 {
    i32::from(slot.index) + if slot.zone == Zone::Upper { 0 } else { 8 }
}

fn decode(id: i32) -> Option<SlotId> {
    match id {
        0..=7 => Some(SlotId {
            zone: Zone::Upper,
            index: id as u8,
        }),
        8..=15 => Some(SlotId {
            zone: Zone::Lower,
            index: (id - 8) as u8,
        }),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Display {
    #[default]
    Full,
    Intro,
    Center,
    Zone(Zone),
}

pub(super) struct Scene {
    pub board: Board,
    pub application: Option<Application>,
    layouts: Layouts,
    editable: bool,
    display: Cell<Display>,
    point: Cell<Option<Point>>,
    pressed: Cell<Option<i32>>,
    pub reverse_tab: Cell<bool>,
}

impl Scene {
    pub fn new(layouts: Layouts, application: Option<Application>, editable: bool) -> Self {
        Self {
            board: Board::new(layouts.upper.count, layouts.lower.count),
            application,
            layouts,
            editable,
            display: Cell::default(),
            point: Cell::new(None),
            pressed: Cell::new(None),
            reverse_tab: Cell::new(false),
        }
    }

    pub fn display(&self) -> Display {
        self.display.get()
    }

    pub fn set_display(&self, display: Display) {
        self.display.set(display);
    }

    pub fn action(&self, item: i32) -> Option<WorkspaceAction> {
        let slot = decode(item)?;
        self.layouts.action(slot)
    }

    pub fn available(&self, slot: SlotId) -> bool {
        self.layouts.available(slot)
    }

    pub fn item_available(&self, item: i32) -> bool {
        decode(item).is_some_and(|slot| self.available(slot))
    }

    fn has_scope(&self) -> bool {
        matches!(self.display(), Display::Full | Display::Zone(_))
    }

    pub fn can_edit(&self) -> bool {
        self.editable && self.display() == Display::Full
    }

    fn slots(&self) -> impl Iterator<Item = &Slot> {
        self.board.slots.iter().filter(|slot| {
            self.layouts.action(slot.id).is_some()
                && match self.display() {
                    Display::Full => true,
                    Display::Intro | Display::Center => false,
                    Display::Zone(zone) => zone == slot.id.zone,
                }
        })
    }

    pub fn outlines(&self) -> impl Iterator<Item = &Outline> {
        [
            Some(&self.board.hub),
            self.has_scope().then_some(&self.board.scope),
            self.can_edit().then_some(&self.board.edit),
        ]
        .into_iter()
        .flatten()
        .chain(self.slots().map(|slot| &slot.outline))
    }

    pub fn show(&self, window: &WorkspaceWindow) {
        self.point.set(None);
        self.pressed.set(None);
        self.reverse_tab.set(false);
        window.set_radial_hover(NONE);
        window.set_radial_focus(NONE);
        window.set_radial_detail("".into());
        window.set_radial(true);
        self.render(window);
    }

    pub fn render(&self, window: &WorkspaceWindow) {
        window.set_radial_full(self.display() == Display::Full);
        window.set_radial_editable(self.can_edit());
        window.set_radial_scope(if self.has_scope() {
            self.layouts.scope.as_str().into()
        } else {
            "".into()
        });
        window.set_radial_scope_glyph(self.layouts.glyph.as_str().into());
        let tiles = self
            .slots()
            .filter_map(|slot| {
                let action = self.layouts.action(slot.id)?;
                let (min, max) = slot.face.bounds();
                let label = action.label();
                let label_anchor = slot.label_anchor(label);
                let center = label_anchor.unwrap_or(slot.anchor);
                Some(RadialTile {
                    slot: id(slot.id),
                    path: slot.face.commands().into(),
                    label: label.into(),
                    glyph: action.glyph().into(),
                    center_x: center.x,
                    center_y: center.y,
                    face_x: min.x - 1.0,
                    face_y: min.y - 1.0,
                    face_width: max.x - min.x + 2.0,
                    face_height: max.y - min.y + 2.0,
                    inline_label: label_anchor.is_some(),
                    enabled: self.available(slot.id),
                    ..Default::default()
                })
            })
            .collect::<Vec<_>>();
        window.set_radial_tiles(ModelRc::new(VecModel::from(tiles)));
    }

    fn hit(&self) -> Option<i32> {
        let point = self.point.get()?;
        if self.board.hub.contains(point) {
            return Some(HUB);
        }
        if self.can_edit() && self.board.edit.contains(point) {
            return Some(EDIT);
        }
        self.slots()
            .find(|slot| slot.outline.contains(point))
            .map(|slot| id(slot.id))
    }

    pub fn highlight(&self, window: &WorkspaceWindow, target: Option<SlotId>) {
        self.set_hover(window, target.map(id).unwrap_or(NONE));
    }

    fn set_hover(&self, window: &WorkspaceWindow, item: i32) {
        if item != window.get_radial_hover() || window.get_radial_focus() != NONE {
            window.set_radial_hover(item);
            window.set_radial_focus(NONE);
            self.update_detail(window, if item == HUB { NONE } else { item });
        }
    }

    fn update_detail(&self, window: &WorkspaceWindow, item: i32) {
        let detail = match item {
            EDIT => "编辑布局",
            HUB if self.display() == Display::Full => "收起工作台",
            _ => self.action(item).map_or("", WorkspaceAction::label),
        };
        window.set_radial_detail(detail.into());
    }

    pub fn move_pointer(&self, window: &WorkspaceWindow, point: Option<Point>) {
        self.point.set(point);
        self.set_hover(window, self.hit().unwrap_or(NONE));
    }

    pub fn click(&self, down: bool) -> Option<i32> {
        let hit = self.hit();
        if down {
            self.pressed.set(hit);
            None
        } else {
            self.pressed.take().filter(|old| Some(*old) == hit)
        }
    }

    pub fn tab(&self, window: &WorkspaceWindow) {
        let order = std::iter::once(HUB)
            .chain(self.slots().map(|slot| id(slot.id)))
            .chain(self.can_edit().then_some(EDIT))
            .collect::<Vec<_>>();
        let current = order.iter().position(|&id| id == window.get_radial_focus());
        let next = if self.reverse_tab.get() {
            current
                .and_then(|i| i.checked_sub(1))
                .unwrap_or(order.len() - 1)
        } else {
            current.map_or(0, |i| (i + 1) % order.len())
        };
        window.set_radial_focus(order[next]);
        self.update_detail(window, order[next]);
    }
}
