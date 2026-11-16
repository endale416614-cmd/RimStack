//! Kept as `author,version,load` (the columns) and `name=0.35,author=0.12` (each a share of the
//! row's room for its cells, so the row keeps its proportions at any width).

use crate::mods::list::SortKey;
use std::collections::BTreeMap;

pub fn chosen(kept: &str) -> Vec<&str> {
    kept.split(',')
        .filter(|column| !column.is_empty())
        .collect()
}

pub fn toggled(kept: &str, column: &str) -> String {
    let mut now = chosen(kept);
    match now.iter().position(|c| *c == column) {
        Some(at) => drop(now.remove(at)),
        None => now.push(column),
    }
    now.join(",")
}

pub fn moved(kept: &str, column: &str, to: i32) -> String {
    let mut now: Vec<&str> = chosen(kept).into_iter().filter(|c| *c != column).collect();
    let to = usize::try_from(to).unwrap_or(0).min(now.len());
    now.insert(to, column);
    now.join(",")
}

pub fn shares(kept: &str) -> BTreeMap<&str, f64> {
    let pairs = kept.split(',').filter_map(|pair| pair.split_once('='));
    let share = |text: &str| text.parse().ok().filter(|s: &f64| *s > 0.0 && *s <= 1.0);
    pairs
        .filter_map(|(column, text)| Some((column, share(text)?)))
        .collect()
}

fn written(shares: &BTreeMap<&str, f64>) -> String {
    let pairs: Vec<String> = shares
        .iter()
        .map(|(column, share)| format!("{column}={}", (share * 10_000.0).round() / 10_000.0))
        .collect();
    pairs.join(",")
}

/// The edge right of `key` moved `by` (to the right is more): `key` changes width and the last
/// column, which fills the row, gives or takes it, so nothing else moves. `drawn` is the name's
/// and each column's but the last as drawn when the edge was taken, `room` what they and the last
/// one shared; each is kept as its share of it, so the row keeps its proportions from then on.
/// `key` stays between `least` and its width with all the last column could `give`.
pub fn resized(
    kept: &str,
    drawn: &[(&str, f64)],
    room: f64,
    key: &str,
    by: f64,
    least: f64,
    give: f64,
) -> String {
    let mut now = shares(kept);
    for &(column, wide) in drawn {
        let wide = match column == key {
            true => wide + by.clamp((least - wide).min(0.0), give.max(0.0)),
            false => wide,
        };
        now.insert(column, (wide / room).clamp(0.0001, 1.0));
    }
    written(&now)
}

/// `key` as wide as it would be had it never been sized: its contents, or for the name its longest.
pub fn natural(kept: &str, key: &str) -> String {
    let mut now = shares(kept);
    now.remove(key);
    written(&now)
}

/// A row's widths as drawn: the name's first, then each column's, and what the last column, which
/// fills the row, can give a column dragged wider.
#[derive(Debug, PartialEq)]
pub struct Laid {
    pub widths: Vec<f64>,
    pub give: f64,
}

/// The name and columns in `room`: each as its share of it, or as `natural` without one, and no
/// narrower than its `least`; the last fills what is left. Short of room, the last gives down to
/// its `natural`, then all the others alike down to their `least`, then the last down to its
/// `least`; still short, the row scrolls.
pub fn laid(room: f64, shares: &[Option<f64>], natural: &[f64], least: &[f64]) -> Laid {
    let last = shares.len() - 1;
    let mut widths: Vec<f64> = (0..last)
        .map(|at| {
            shares[at]
                .map_or(natural[at], |share| share * room)
                .max(least[at])
        })
        .collect();
    let floor = natural[last].max(least[last]);
    let short = floor - (room - widths.iter().sum::<f64>());
    let spare: f64 = (0..last).map(|at| (widths[at] - least[at]).max(0.0)).sum();
    if short > 0.0 && spare > 0.0 {
        let part = (short / spare).min(1.0);
        for at in 0..last {
            widths[at] -= (widths[at] - least[at]).max(0.0) * part;
        }
    }
    let left = room - widths.iter().sum::<f64>();
    widths.push(left.max(least[last]));
    Laid {
        widths,
        give: (left - floor).max(0.0),
    }
}

/// `None` is the list's own order: the load order, or the inactive mods by category.
/// A key starts the way `SortKey::starts_descending` says, turns round when asked again,
/// and goes back to `None` the third time; one that does not turn goes back the second time.
pub fn pressed(now: Option<(SortKey, bool)>, asked: Option<SortKey>) -> Option<(SortKey, bool)> {
    let asked = asked?;
    let first = asked.starts_descending();
    match now {
        Some((by, _)) if by == asked && !asked.turns() => None,
        Some((by, descending)) if by == asked => (descending == first).then_some((asked, !first)),
        _ => Some((asked, first)),
    }
}

/// Kept as "" for the list's own order, "name", or "name down".
pub fn kept(order: Option<(SortKey, bool)>) -> String {
    match order {
        None => String::new(),
        Some((key, false)) => key.key().to_string(),
        Some((key, true)) => format!("{} down", key.key()),
    }
}

pub fn from_kept(kept: &str) -> Option<(SortKey, bool)> {
    let (key, way) = kept.split_once(' ').unwrap_or((kept, ""));
    SortKey::from_key(key).map(|key| (key, way == "down"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_column_is_taken_out_or_added_last_and_moved_among_the_others() {
        assert_eq!(chosen(""), Vec::<&str>::new());
        assert_eq!(toggled("author,version,load", "version"), "author,load");
        assert_eq!(toggled("author,load", "version"), "author,load,version");
        assert_eq!(toggled("", "load"), "load");
        assert_eq!(
            moved("author,version,load", "load", 0),
            "load,author,version"
        );
        assert_eq!(
            moved("author,version,load", "author", 1),
            "version,author,load"
        );
        assert_eq!(moved("author,version", "author", 99), "version,author");
        assert_eq!(moved("author,version", "version", -3), "version,author");
        assert_eq!(moved("author", "load", 1), "author,load");
    }

    #[test]
    fn a_share_is_kept_per_column_and_nonsense_is_not() {
        assert_eq!(
            shares("name=0.35,author=0.123456,load=180,=,x,version=0,size=NaN"),
            BTreeMap::from([("name", 0.35), ("author", 0.123456)])
        );
        assert_eq!(natural("author=0.2,name=0.3", "name"), "author=0.2");
    }

    #[test]
    fn an_edge_sizes_the_column_left_of_it_and_the_last_one_alone() {
        let drawn = [("name", 300.0), ("author", 100.0)];
        assert_eq!(
            resized("", &drawn, 1000.0, "author", 50.0, 40.0, 200.0),
            "author=0.15,name=0.3",
            "the name and the others are kept as they were drawn"
        );
        assert_eq!(
            resized("", &drawn, 1000.0, "author", 500.0, 40.0, 200.0),
            "author=0.3,name=0.3",
            "no wider than the last column can give"
        );
        assert_eq!(
            resized("", &drawn, 1000.0, "author", -500.0, 40.0, 200.0),
            "author=0.04,name=0.3",
            "no narrower than its least"
        );
        assert_eq!(
            resized("", &drawn, 1000.0, "name", -100.0, 160.0, 0.0),
            "author=0.1,name=0.2"
        );
    }

    #[test]
    fn a_row_keeps_its_shares_and_the_last_column_fills_it() {
        let natural = [200.0, 80.0, 60.0, 90.0];
        let least = [160.0, 50.0, 50.0, 50.0];
        let shared = [Some(0.3), Some(0.2), None, None];
        assert_eq!(
            laid(1000.0, &shared, &natural, &least),
            Laid {
                widths: vec![300.0, 200.0, 60.0, 440.0],
                give: 350.0
            }
        );
        let half = laid(500.0, &shared, &natural, &least);
        assert_eq!(
            half.widths,
            [160.0, 100.0, 60.0, 180.0],
            "the name no narrower than its least"
        );
        let narrow = laid(340.0, &shared, &natural, &least);
        assert_eq!(
            narrow.widths,
            [160.0, 50.0, 50.0, 80.0],
            "short, the others give their room, then the last below its contents"
        );
        assert_eq!(narrow.give, 0.0);
        assert_eq!(
            laid(250.0, &shared, &natural, &least).widths,
            [160.0, 50.0, 50.0, 50.0],
            "past every least: it scrolls"
        );
        assert_eq!(
            laid(800.0, &[None], &[200.0], &[160.0]).widths,
            [800.0],
            "the name alone fills the row"
        );
    }

    #[test]
    fn a_heading_pressed_three_times_comes_back_to_the_load_order() {
        let name = Some(SortKey::Name);
        let first = pressed(None, name);
        let second = pressed(first, name);
        let third = pressed(second, name);
        assert_eq!(
            [first, second, third],
            [
                Some((SortKey::Name, false)),
                Some((SortKey::Name, true)),
                None
            ]
        );
        let by_name = Some((SortKey::Name, true));
        assert_eq!(
            pressed(by_name, Some(SortKey::Author)),
            Some((SortKey::Author, false))
        );
        assert_eq!(pressed(by_name, None), None);
        let group = Some(SortKey::Group);
        let gathered = pressed(by_name, group);
        assert_eq!(gathered, Some((SortKey::Group, false)));
        assert_eq!(pressed(gathered, group), None, "groups are shown one way");
    }

    #[test]
    fn the_newest_and_largest_come_first_then_last() {
        let updated = Some(SortKey::Updated);
        let first = pressed(Some((SortKey::Name, false)), updated);
        let second = pressed(first, updated);
        assert_eq!(
            [first, second, pressed(second, updated)],
            [
                Some((SortKey::Updated, true)),
                Some((SortKey::Updated, false)),
                None
            ]
        );
    }

    #[test]
    fn an_order_is_kept_as_its_key_and_way_and_read_back() {
        for order in [
            None,
            Some((SortKey::Size, true)),
            Some((SortKey::PackageId, false)),
            Some((SortKey::Group, false)),
        ] {
            assert_eq!(from_kept(&kept(order)), order);
        }
        assert_eq!(kept(Some((SortKey::Size, true))), "size down");
        assert_eq!(from_kept("colour down"), None);
    }
}
