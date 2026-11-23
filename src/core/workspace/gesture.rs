use std::time::Duration;

use super::radial::{Board, CENTER, HUB_RADIUS, Point, SlotId, Zone};

pub const HOLD_AFTER: Duration = Duration::from_millis(180);
pub const DRAG_DIP: f32 = 22.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Pending,
    Center,
    Selecting(Zone),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Release {
    Cancel,
    FullMenu,
    Submit(SlotId),
}

pub struct Gesture {
    phase: Phase,
    highlight: Option<SlotId>,
    armed: bool,
    left_hub: bool,
}

impl Gesture {
    pub fn new(origin: Point) -> Self {
        Self {
            phase: Phase::Pending,
            highlight: None,
            armed: origin.distance(CENTER) <= HUB_RADIUS,
            left_hub: false,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn highlight(&self) -> Option<SlotId> {
        self.highlight
    }

    pub fn advance(
        &mut self,
        age: Duration,
        travel_dip: f32,
        point: Point,
        board_scale: f32,
        board: &Board,
        enabled: impl Fn(SlotId) -> bool,
    ) {
        if self.phase == Phase::Pending {
            if age < HOLD_AFTER && travel_dip < DRAG_DIP {
                return;
            }
            self.phase = Phase::Center;
        }
        let within_hub = point.distance(CENTER) <= HUB_RADIUS;
        self.armed |= within_hub;
        if !self.armed {
            return;
        }
        if self.phase == Phase::Center {
            let dy = (point.y - CENTER.y) * board_scale;
            if dy.abs() >= DRAG_DIP {
                self.phase = Phase::Selecting(if dy < 0.0 { Zone::Upper } else { Zone::Lower });
            }
        }
        if let Phase::Selecting(zone) = self.phase {
            if within_hub && self.left_hub {
                self.phase = Phase::Center;
                self.highlight = None;
                self.left_hub = false;
                return;
            }
            self.left_hub |= !within_hub;
            // A move near the edge may keep the previous highlight. The final
            // release always uses the strict outline, including any empty gap.
            if self.highlight.is_some_and(|id| {
                enabled(id)
                    && board.slots.iter().any(|slot| {
                        slot.id == id && slot.outline.near_edge(point, 3.0 / board_scale)
                    })
            }) {
                return;
            }
            self.highlight = strict_hit(board, zone, point, enabled);
        }
    }

    pub fn release(
        self,
        age: Duration,
        travel_dip: f32,
        point: Point,
        board: &Board,
        enabled: impl Fn(SlotId) -> bool,
    ) -> Release {
        match self.phase {
            Phase::Pending if age < HOLD_AFTER && travel_dip < DRAG_DIP => Release::FullMenu,
            Phase::Selecting(zone) => strict_hit(board, zone, point, enabled)
                .filter(|id| Some(*id) == self.highlight)
                .map_or(Release::Cancel, Release::Submit),
            _ => Release::Cancel,
        }
    }
}

fn strict_hit(
    board: &Board,
    zone: Zone,
    point: Point,
    enabled: impl Fn(SlotId) -> bool,
) -> Option<SlotId> {
    if point.distance(CENTER) <= HUB_RADIUS {
        return None;
    }
    board
        .slots
        .iter()
        .find(|slot| slot.id.zone == zone && enabled(slot.id) && slot.outline.contains(point))
        .map(|slot| slot.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn press_thresholds_and_horizontal_swipe_are_exclusive() {
        let board = Board::default();
        for (age, travel, expect) in [
            (179, 21.0, Release::FullMenu),
            (180, 21.0, Release::Cancel),
            (181, 21.0, Release::Cancel),
            (179, 22.0, Release::Cancel),
            (179, 23.0, Release::Cancel),
        ] {
            let mut gesture = Gesture::new(CENTER);
            let p = Point {
                x: CENTER.x + travel,
                ..CENTER
            };
            gesture.advance(ms(age), travel, p, 1.0, &board, |_| true);
            assert_eq!(
                gesture.release(ms(age), travel, p, &board, |_| true),
                expect
            );
        }
    }

    #[test]
    fn motion_locks_one_zone_and_release_checks_enabled_shape() {
        let board = Board::default();
        let mut gesture = Gesture::new(CENTER);
        let upper = &board.slots[4];
        let lower = &board.slots[6];
        gesture.advance(ms(20), 94.0, upper.anchor, 1.0, &board, |_| true);
        assert_eq!(gesture.phase(), Phase::Selecting(Zone::Upper));
        assert_eq!(gesture.highlight(), Some(upper.id));
        gesture.advance(ms(40), 148.0, lower.anchor, 1.0, &board, |_| true);
        assert_eq!(gesture.phase(), Phase::Selecting(Zone::Upper));
        assert_eq!(gesture.highlight(), None);
        gesture.advance(ms(70), 148.0, CENTER, 1.0, &board, |_| true);
        assert_eq!(gesture.phase(), Phase::Center);
        gesture.advance(ms(90), 148.0, lower.anchor, 1.0, &board, |_| true);
        assert_eq!(gesture.highlight(), Some(lower.id));
        assert_eq!(
            gesture.release(ms(100), 148.0, lower.anchor, &board, |id| id != lower.id),
            Release::Cancel
        );
    }

    #[test]
    fn clamped_origin_needs_the_hub_before_a_target_can_be_armed() {
        let board = Board::default();
        let upper = &board.slots[4];
        let mut gesture = Gesture::new(Point { x: 20.0, y: 20.0 });
        gesture.advance(ms(60), 30.0, upper.anchor, 1.0, &board, |_| true);
        assert_eq!(gesture.phase(), Phase::Center);
        assert_eq!(gesture.highlight(), None);
        gesture.advance(ms(100), 100.0, CENTER, 1.0, &board, |_| true);
        gesture.advance(ms(170), 130.0, upper.anchor, 1.0, &board, |_| true);
        assert_eq!(
            gesture.release(ms(171), 130.0, upper.anchor, &board, |_| true),
            Release::Submit(upper.id)
        );
    }
}
