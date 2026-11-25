use std::time::Duration;

pub const HOLD_AFTER: Duration = Duration::from_millis(180);
pub const START_DIP: f64 = 20.0;
const CANCEL_DIP: f64 = 14.0;
const HYSTERESIS: f64 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
    North,
    NorthEast,
}

impl Direction {
    const CLOCKWISE: [Self; 8] = [
        Self::East,
        Self::SouthEast,
        Self::South,
        Self::SouthWest,
        Self::West,
        Self::NorthWest,
        Self::North,
        Self::NorthEast,
    ];

    pub const GRID: [Self; 8] = [
        Self::NorthWest,
        Self::North,
        Self::NorthEast,
        Self::West,
        Self::East,
        Self::SouthWest,
        Self::South,
        Self::SouthEast,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        ["右", "右下", "下", "左下", "左", "左上", "上", "右上"][self.index()]
    }

    pub fn icon_path(self) -> &'static str {
        [
            "M 2 7 H 12 M 8 3 L 12 7 L 8 11",
            "M 3 3 L 11 11 M 11 5 V 11 H 5",
            "M 7 2 V 12 M 3 8 L 7 12 L 11 8",
            "M 11 3 L 3 11 M 3 5 V 11 H 9",
            "M 12 7 H 2 M 6 3 L 2 7 L 6 11",
            "M 11 11 L 3 3 M 3 9 V 3 H 9",
            "M 7 12 V 2 M 3 6 L 7 2 L 11 6",
            "M 3 11 L 11 3 M 11 9 V 3 H 5",
        ][self.index()]
    }

    fn select(x: f64, y: f64, previous: Option<Self>) -> Self {
        let angle = y.atan2(x).to_degrees();
        if let Some(direction) = previous {
            let center = direction.index() as f64 * 45.0;
            let difference = (angle - center + 180.0).rem_euclid(360.0) - 180.0;
            if difference.abs() <= 22.5 + HYSTERESIS {
                return direction;
            }
        }
        Self::CLOCKWISE[((angle + 22.5).rem_euclid(360.0) / 45.0) as usize % 8]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Pending,
    Center,
    Selecting(Direction),
}

impl Phase {
    pub fn advance(self, age: Duration, peak: f64, x: f64, y: f64) -> Self {
        match self {
            Self::Pending if age < HOLD_AFTER && peak < START_DIP => Self::Pending,
            Self::Selecting(_) if x.hypot(y) <= CANCEL_DIP => Self::Center,
            Self::Selecting(previous) => Self::Selecting(Direction::select(x, y, Some(previous))),
            _ if x.hypot(y) >= START_DIP => Self::Selecting(Direction::select(x, y, None)),
            _ => Self::Center,
        }
    }

    pub fn release(self, age: Duration, peak: f64, x: f64, y: f64) -> Release {
        match self {
            Self::Pending if age < HOLD_AFTER && peak < START_DIP => Release::Panel,
            Self::Selecting(_) => match self.advance(age, peak, x, y) {
                Self::Selecting(direction) => Release::Direction(direction),
                _ => Release::Cancel,
            },
            // An up packet can contain motion by itself. It can't start a gesture.
            _ => Release::Cancel,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Release {
    Panel,
    Direction(Direction),
    Cancel,
}

#[cfg(test)]
mod tests {
    use super::{Direction::*, *};

    #[test]
    fn time_peak_and_current_position_have_distinct_roles() {
        let quick = Duration::from_millis(179);
        let pending = Phase::Pending;
        assert_eq!(pending.release(quick, 19.9, 0.0, 0.0), Release::Panel);
        assert_eq!(pending.release(HOLD_AFTER, 0.0, 0.0, 0.0), Release::Cancel);
        assert_eq!(pending.release(quick, 22.0, 22.0, 0.0), Release::Cancel);
        let center = pending.advance(quick, 24.0, 0.0, 0.0);
        assert_eq!(center, Phase::Center);
        assert_eq!(center.release(quick, 24.0, 18.0, 0.0), Release::Cancel);
        let selecting = pending.advance(quick, 20.0, 20.0, 0.0);
        assert_eq!(selecting, Phase::Selecting(East));
        assert_eq!(selecting.release(quick, 20.0, 14.0, 0.0), Release::Cancel);
        assert_eq!(
            selecting.release(quick, 20.0, 14.1, 0.0),
            Release::Direction(East)
        );
    }

    #[test]
    fn release_resolves_latest_direction_with_wrapped_hysteresis() {
        for (x, y, direction) in [
            (30.0, 0.0, East),
            (30.0, 30.0, SouthEast),
            (0.0, 30.0, South),
            (-30.0, 30.0, SouthWest),
            (-30.0, 0.0, West),
            (-30.0, -30.0, NorthWest),
            (0.0, -30.0, North),
            (30.0, -30.0, NorthEast),
        ] {
            let phase = Phase::Pending.advance(Duration::ZERO, 42.0, x, y);
            assert_eq!(phase, Phase::Selecting(direction));
        }
        let west = Phase::Selecting(West);
        assert_eq!(
            west.release(HOLD_AFTER, 64.0, -50.0, -28.0),
            Release::Direction(West)
        );
        assert_eq!(
            west.release(HOLD_AFTER, 64.0, -50.0, -32.0),
            Release::Direction(NorthWest)
        );
        assert_eq!(
            west.release(HOLD_AFTER, 64.0, 50.0, 0.0),
            Release::Direction(East)
        );
    }
}
