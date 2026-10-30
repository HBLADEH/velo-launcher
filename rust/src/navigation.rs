//! The row/column rules from frontend/src/navigation.ts, independent of the UI.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

pub fn choice_rows(section_lengths: &[usize], columns: usize) -> Vec<Vec<usize>> {
    assert!(columns > 0);
    let mut offset = 0;
    let mut rows = Vec::new();
    for &length in section_lengths {
        for start in (0..length).step_by(columns) {
            rows.push((offset + start..offset + (start + columns).min(length)).collect());
        }
        offset += length;
    }
    rows
}

pub fn navigate(
    rows: &[Vec<usize>],
    selected: Option<usize>,
    direction: Direction,
) -> Option<usize> {
    let choices: Vec<_> = rows.iter().flatten().copied().collect();
    let first = *choices.first()?;
    let Some((row, column)) = rows.iter().enumerate().find_map(|(row, items)| {
        items
            .iter()
            .position(|item| Some(*item) == selected)
            .map(|column| (row, column))
    }) else {
        return Some(first);
    };
    let forward = matches!(direction, Direction::Down | Direction::Right);
    let step = |position, length| (position + if forward { 1 } else { length - 1 }) % length;
    if matches!(direction, Direction::Down | Direction::Up) {
        let target = &rows[step(row, rows.len())];
        target.get(column.min(target.len() - 1)).copied()
    } else {
        let position = choices.iter().position(|item| Some(*item) == selected)?;
        Some(choices[step(position, choices.len())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_vue_cross_group_and_incomplete_row_contract() {
        let rows = choice_rows(&[8, 4], 6);
        assert_eq!(
            rows,
            [vec![0, 1, 2, 3, 4, 5], vec![6, 7], vec![8, 9, 10, 11]]
        );
        for (current, direction, expected) in [
            (4, Direction::Down, 7),
            (7, Direction::Down, 9),
            (9, Direction::Up, 7),
            (0, Direction::Up, 8),
            (11, Direction::Down, 3),
            (7, Direction::Right, 8),
            (8, Direction::Left, 7),
            (0, Direction::Left, 11),
        ] {
            assert_eq!(navigate(&rows, Some(current), direction), Some(expected));
        }
    }

    #[test]
    fn empty_groups_stale_selection_and_list_wrap_match_vue() {
        assert_eq!(navigate(&[], Some(0), Direction::Down), None);
        let rows = choice_rows(&[0, 3], 1);
        assert_eq!(rows, [vec![0], vec![1], vec![2]]);
        assert_eq!(navigate(&rows, Some(0), Direction::Up), Some(2));
        assert_eq!(navigate(&rows, Some(2), Direction::Down), Some(0));
        assert_eq!(navigate(&rows, Some(10), Direction::Down), Some(0));
        assert_eq!(navigate(&rows, None, Direction::Up), Some(0));
    }

    #[test]
    fn real_fixture_groups_keep_their_own_six_column_rows() {
        let rows = choice_rows(&[3, 12, 13], 6);
        assert_eq!(
            rows.iter().map(Vec::len).collect::<Vec<_>>(),
            [3, 6, 6, 6, 6, 1]
        );
        assert_eq!(navigate(&rows, Some(2), Direction::Down), Some(5));
        assert_eq!(navigate(&rows, Some(26), Direction::Down), Some(27));
        assert_eq!(navigate(&rows, Some(27), Direction::Down), Some(0));
        assert_eq!(navigate(&rows, Some(0), Direction::Up), Some(27));
        let no_pins = choice_rows(&[0, 12, 13], 6);
        assert_eq!(navigate(&no_pins, Some(5), Direction::Up), Some(24));
    }
}
