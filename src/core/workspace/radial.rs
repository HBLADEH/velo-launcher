use std::f32::consts::TAU;
use std::fmt::Write;

pub const WIDTH: f32 = 420.0;
pub const HEIGHT: f32 = 424.0;
pub const CENTER: Point = Point { x: 210.0, y: 210.0 };
pub const HUB_RADIUS: f32 = 32.0;
pub const SCOPE: Point = Point { x: 210.0, y: 272.0 };
pub const EDIT: Point = Point { x: 264.0, y: 226.0 };
pub const SCOPE_WIDTH: f32 = 134.0;
pub const SCOPE_HEIGHT: f32 = 26.0;
pub const EDIT_SIZE: f32 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn distance(self, other: Self) -> f32 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Upper,
    Lower,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotId {
    pub zone: Zone,
    pub index: u8,
}

pub struct Outline(Vec<Point>);

impl Outline {
    pub fn points(&self) -> &[Point] {
        &self.0
    }

    pub fn commands(&self) -> String {
        let mut path = String::new();
        for (index, p) in self.0.iter().enumerate() {
            let command = if index == 0 { 'M' } else { 'L' };
            let _ = write!(path, "{command}{:.3} {:.3} ", p.x, p.y);
        }
        path.push('Z');
        path
    }

    pub fn bounds(&self) -> (Point, Point) {
        let mut min = self.0[0];
        let mut max = min;
        for point in &self.0[1..] {
            min.x = min.x.min(point.x);
            min.y = min.y.min(point.y);
            max.x = max.x.max(point.x);
            max.y = max.y.max(point.y);
        }
        (min, max)
    }

    pub fn dashed_commands(&self) -> String {
        const DASH: f32 = 4.0;
        let mut path = String::new();
        let mut drawing = true;
        let mut remaining = DASH;
        for (start, end) in self.edges() {
            let length = start.distance(end);
            let mut used = 0.0;
            while length - used > 0.001 {
                let step = (length - used).min(remaining);
                if drawing {
                    let at = |part: f32| {
                        let u = part / length;
                        Point {
                            x: start.x + (end.x - start.x) * u,
                            y: start.y + (end.y - start.y) * u,
                        }
                    };
                    let a = at(used);
                    let b = at(used + step);
                    let _ = write!(path, "M{:.3} {:.3} L{:.3} {:.3} ", a.x, a.y, b.x, b.y);
                }
                used += step;
                remaining -= step;
                if remaining <= 0.001 {
                    remaining = DASH;
                    drawing = !drawing;
                }
            }
        }
        path
    }

    pub fn contains(&self, point: Point) -> bool {
        let mut inside = false;
        for (a, b) in self.edges() {
            if (a.y > point.y) != (b.y > point.y)
                && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
            {
                inside = !inside;
            }
        }
        inside
    }

    pub fn near_edge(&self, point: Point, distance: f32) -> bool {
        self.edges().any(|(a, b)| {
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let length_sq = dx * dx + dy * dy;
            if length_sq == 0.0 {
                return point.distance(a) <= distance;
            }
            let fraction =
                (((point.x - a.x) * dx + (point.y - a.y) * dy) / length_sq).clamp(0.0, 1.0);
            point.distance(Point {
                x: a.x + fraction * dx,
                y: a.y + fraction * dy,
            }) <= distance
        })
    }

    fn edges(&self) -> impl Iterator<Item = (Point, Point)> + '_ {
        self.0
            .iter()
            .copied()
            .zip(self.0.iter().copied().cycle().skip(1))
    }

    fn round_rect(center: Point, width: f32, height: f32, radius: f32) -> Self {
        let dx = width / 2.0 - radius;
        let dy = height / 2.0 - radius;
        let mut points = Vec::with_capacity(32);
        for (x, y, start) in [
            (dx, -dy, 0.0),
            (dx, dy, 90.0),
            (-dx, dy, 180.0),
            (-dx, -dy, 270.0),
        ] {
            arc(
                &mut points,
                Point {
                    x: center.x + x,
                    y: center.y + y,
                },
                radius,
                start,
                start + 90.0,
            );
        }
        Self(points)
    }
}

pub struct Slot {
    pub id: SlotId,
    pub anchor: Point,
    pub outline: Outline,
    pub face: Outline,
}

impl Slot {
    pub fn label_anchor(&self, label: &str) -> Option<Point> {
        // Allow one em per character at 12 DIP, plus padding. The icon and
        // horizontal label occupy different parts of the curved tile.
        let half_label = 6.0 * label.chars().count() as f32 + 4.0;
        let toward_edge = (self.anchor.x - CENTER.x).signum() * 4.0;
        [
            (0.0, 0.0),
            (0.0, -4.0),
            (0.0, 4.0),
            (toward_edge, -4.0),
            (toward_edge, 4.0),
            (-toward_edge / 2.0, 1.0),
        ]
        .into_iter()
        .find_map(|(dx, dy)| {
            let center = Point {
                x: self.anchor.x + dx,
                y: self.anchor.y + dy,
            };
            let fits = [
                (-12.0, -19.0),
                (12.0, -19.0),
                (-12.0, 3.0),
                (12.0, 3.0),
                (-half_label, 1.0),
                (half_label, 1.0),
                (-half_label, 19.0),
                (half_label, 19.0),
            ]
            .into_iter()
            .all(|(x, y)| {
                self.face.contains(Point {
                    x: center.x + x,
                    y: center.y + y,
                })
            });
            fits.then_some(center)
        })
    }
}

pub struct Board {
    pub hub: Outline,
    pub scope: Outline,
    pub edit: Outline,
    pub slots: Vec<Slot>,
}

impl Default for Board {
    fn default() -> Self {
        Self::new(6, 4)
    }
}

impl Board {
    pub fn new(upper: u8, lower: u8) -> Self {
        let hub = Outline(
            (0u8..64)
                .map(|step| {
                    let angle = f32::from(step) * TAU / 64.0;
                    Point {
                        x: CENTER.x + HUB_RADIUS * angle.sin(),
                        y: CENTER.y - HUB_RADIUS * angle.cos(),
                    }
                })
                .collect(),
        );
        let mut slots = Vec::with_capacity(usize::from(upper + lower));
        append_zone(&mut slots, Zone::Upper, upper);
        append_zone(&mut slots, Zone::Lower, lower);
        Self {
            hub,
            scope: Outline::round_rect(SCOPE, SCOPE_WIDTH, SCOPE_HEIGHT, 13.0),
            edit: Outline::round_rect(EDIT, EDIT_SIZE, EDIT_SIZE, 8.0),
            slots,
        }
    }
}

fn polar(center: Point, radius: f32, degrees: f32) -> Point {
    let (sin, cos) = degrees.to_radians().sin_cos();
    Point {
        x: center.x + radius * sin,
        y: center.y - radius * cos,
    }
}

fn arc(points: &mut Vec<Point>, center: Point, radius: f32, start: f32, end: f32) {
    // Four-degree arc segments keep the sagitta under 0.13 DIP at the outer edge.
    let steps = ((end - start).abs() / 4.0).ceil() as u8;
    for step in 0..=steps {
        let fraction = f32::from(step) / f32::from(steps.max(1));
        points.push(polar(center, radius, start + (end - start) * fraction));
    }
}

fn corner(points: &mut Vec<Point>, via: Point, end: Point) {
    let Some(&start) = points.last() else { return };
    for step in 1u8..=4 {
        let t = f32::from(step) / 4.0;
        let s = 1.0 - t;
        points.push(Point {
            x: s * s * start.x + 2.0 * s * t * via.x + t * t * end.x,
            y: s * s * start.y + 2.0 * s * t * via.y + t * t * end.y,
        });
    }
}

fn sector_outline(inner: f32, outer: f32, start: f32, end: f32) -> Outline {
    const ROUND: f32 = 7.0;
    let outer_inset = (ROUND / outer).to_degrees();
    let inner_inset = (ROUND / inner).to_degrees();
    let p = |radius, degrees| polar(CENTER, radius, degrees);
    let mut points = Vec::with_capacity(64);
    arc(
        &mut points,
        CENTER,
        outer,
        start + outer_inset,
        end - outer_inset,
    );
    corner(&mut points, p(outer, end), p(outer - ROUND, end));
    points.push(p(inner + ROUND, end));
    corner(&mut points, p(inner, end), p(inner, end - inner_inset));
    arc(
        &mut points,
        CENTER,
        inner,
        end - inner_inset,
        start + inner_inset,
    );
    corner(&mut points, p(inner, start), p(inner + ROUND, start));
    points.push(p(outer - ROUND, start));
    corner(&mut points, p(outer, start), p(outer, start + outer_inset));
    Outline(points)
}

fn sector(id: SlotId, inner: f32, outer: f32, start: f32, end: f32) -> Slot {
    let bearing = (start + end) / 2.0;
    let shift = if inner == 64.0 && bearing.abs() > 30.0 {
        8.0
    } else {
        0.0
    };
    let anchor = polar(CENTER, (inner + outer) / 2.0 + shift, bearing);
    let outline = sector_outline(inner, outer, start, end);
    // Win32 clips to whole pixels. Keep the painted stroke inside that clip
    // so the renderer can antialias it without changing the clickable shape.
    const INSET: f32 = 2.0;
    let inset_angle = (INSET / inner).to_degrees();
    let face = sector_outline(
        inner + INSET,
        outer - INSET,
        start + inset_angle,
        end - inset_angle,
    );
    Slot {
        id,
        anchor,
        outline,
        face,
    }
}

fn append_zone(slots: &mut Vec<Slot>, zone: Zone, count: u8) {
    const OUTER_THREE: [(f32, f32); 3] = [(-74.0, -26.0), (-22.0, 22.0), (26.0, 74.0)];
    const INNER_THREE: [(f32, f32); 3] = [(-72.0, -27.0), (-22.0, 22.0), (27.0, 72.0)];
    const FOUR: [(f32, f32); 4] = [(-75.0, -40.0), (-35.0, -3.0), (3.0, 35.0), (40.0, 75.0)];
    let mut index = 0;
    let mut append_ring = |inner, outer, spans: &[(f32, f32)]| {
        for &(left, right) in spans {
            let (start, end) = match zone {
                Zone::Upper => (left, right),
                Zone::Lower => (180.0 - right, 180.0 - left),
            };
            slots.push(sector(SlotId { zone, index }, inner, outer, start, end));
            index += 1;
        }
    };
    if count == 4 {
        match zone {
            Zone::Upper => append_ring(132.0, 198.0, &FOUR),
            Zone::Lower => append_ring(110.0, 184.0, &FOUR),
        }
        return;
    }
    let (outer, inner) = if count == 8 {
        (FOUR.as_slice(), FOUR.as_slice())
    } else {
        (OUTER_THREE.as_slice(), INNER_THREE.as_slice())
    };
    match zone {
        Zone::Upper => {
            append_ring(132.0, 198.0, outer);
            append_ring(64.0, 124.0, inner);
        }
        Zone::Lower => {
            append_ring(144.0, 198.0, outer);
            append_ring(80.0, 136.0, inner);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outlines_share_the_designed_centers_and_leave_click_through_gaps() {
        for count in [4, 6, 8] {
            let board = Board::new(count, count);
            assert_eq!(board.slots.len(), usize::from(count) * 2);
            assert!(board.hub.contains(CENTER));
            assert!(board.scope.contains(SCOPE));
            assert!(board.edit.contains(EDIT));
            for slot in &board.slots {
                assert!(slot.outline.contains(slot.anchor), "{:?}", slot.id);
                assert!(slot.outline.commands().starts_with('M'));
                assert!(
                    slot.face
                        .points()
                        .iter()
                        .all(|point| slot.outline.contains(*point))
                );
                assert!(!slot.outline.contains(CENTER));
                assert!(!slot.outline.contains(SCOPE));
                assert!(!slot.outline.contains(EDIT));
                assert!(
                    slot.outline
                        .points()
                        .iter()
                        .all(|p| { (0.0..WIDTH).contains(&p.x) && (0.0..HEIGHT).contains(&p.y) })
                );
            }
        }
        let board = Board::default();
        assert_eq!(board.slots.len(), 10);
        assert!(
            board.slots[..3]
                .iter()
                .all(|slot| slot.label_anchor("文件管理").is_some())
        );
        assert!(board.slots[4].label_anchor("启动器").is_some());
        assert!(board.slots[5].label_anchor("控制面板").is_some());
        assert!(Board::new(8, 8).slots[7].label_anchor("控制面板").is_none());
        for p in [
            Point { x: 210.0, y: 160.0 },
            Point { x: 210.0, y: 82.0 },
            Point { x: 20.0, y: 210.0 },
        ] {
            assert!(!board.hub.contains(p));
            assert!(board.slots.iter().all(|slot| !slot.outline.contains(p)));
        }
    }
}
