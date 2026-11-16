use super::GroupedList;
use crate::mods::library::ModId;

impl GroupedList {
    /// `to` is the place dropped on, the length for past the end. The mods keep their
    /// order and land after the mod at `to` when the first of them was above it, before
    /// it otherwise, as `move_mod` puts one. Whether the order changed.
    pub fn move_mods(&mut self, positions: &[usize], to: usize) -> bool {
        let len = self.mods.len();
        let mut taken: Vec<usize> = positions.iter().copied().filter(|&p| p < len).collect();
        taken.sort_unstable();
        taken.dedup();
        let (Some(&first), Some(&last)) = (taken.first(), taken.last()) else {
            return false;
        };
        let is_taken = |at: &usize| taken.binary_search(at).is_ok();
        let to = to.min(len);
        let past_it = first < to && to < len && !is_taken(&to);
        let at = (0..to).filter(|p| !is_taken(p)).count() + usize::from(past_it);
        if at == first && last - first + 1 == taken.len() {
            return false;
        }
        let mut kept = Vec::with_capacity(len);
        let mut moved = Vec::with_capacity(taken.len());
        let pairs = self.mods.drain(..).zip(self.haystacks.drain(..));
        for (position, pair) in pairs.enumerate() {
            match is_taken(&position) {
                true => moved.push(pair),
                false => kept.push(pair),
            }
        }
        kept.splice(at..at, moved);
        (self.mods, self.haystacks) = kept.into_iter().unzip();
        self.flags.clear();
        self.rebuild();
        true
    }

    /// Every installed mod the rows show but those given, in the order shown.
    pub fn shown_except(&self, ids: &[ModId]) -> Vec<ModId> {
        let shown = (0..self.rows.len()).filter_map(|row| self.installed_on(row));
        shown.filter(|id| !ids.contains(id)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{id, installed_sample};
    use super::GroupedList;

    fn names(list: &GroupedList) -> Vec<&str> {
        list.mods().iter().map(|m| m.name.as_str()).collect()
    }

    #[test]
    fn mods_moved_down_land_after_the_one_dropped_on_and_up_before_it() {
        let mut list = installed_sample();
        assert!(list.move_mods(&[1, 0], 4));
        let down = [
            "Royalty",
            "HugsLib",
            "Dubs Bad Hygiene",
            "Harmony",
            "Core",
            "Rimefeller",
        ];
        assert_eq!(names(&list), down);
        let mut list = installed_sample();
        assert!(list.move_mods(&[4, 5], 1));
        let up = [
            "Harmony",
            "Dubs Bad Hygiene",
            "Rimefeller",
            "Core",
            "Royalty",
            "HugsLib",
        ];
        assert_eq!(names(&list), up);
    }

    #[test]
    fn mods_dropped_on_one_of_their_own_close_up_around_it_and_at_the_end_go_last() {
        let mut list = installed_sample();
        assert!(list.move_mods(&[1, 3], 3));
        let closed = [
            "Harmony",
            "Royalty",
            "Core",
            "HugsLib",
            "Dubs Bad Hygiene",
            "Rimefeller",
        ];
        assert_eq!(names(&list), closed);
        assert!(list.move_mods(&[0, 1], 6));
        let last = [
            "Core",
            "HugsLib",
            "Dubs Bad Hygiene",
            "Rimefeller",
            "Harmony",
            "Royalty",
        ];
        assert_eq!(names(&list), last);
    }

    #[test]
    fn one_mod_lands_where_move_mod_puts_it() {
        for (from, to) in [(0, 3), (4, 1), (2, 5), (5, 0)] {
            let (mut one, mut many) = (installed_sample(), installed_sample());
            one.move_mod(from, to);
            assert!(many.move_mods(&[from], to));
            assert_eq!(names(&many), names(&one), "{from} to {to}");
        }
    }

    #[test]
    fn a_drop_that_leaves_the_order_as_it_was_changes_nothing() {
        let mut list = installed_sample();
        assert!(!list.move_mods(&[1, 2], 1));
        assert!(!list.move_mods(&[4, 5], 6));
        assert!(!list.move_mods(&[], 3));
        assert!(!list.move_mods(&[99], 0));
        assert_eq!(names(&list), names(&installed_sample()));
    }

    #[test]
    fn the_mods_shown_but_those_given_leave_out_the_hidden_and_the_missing() {
        let mut list = installed_sample();
        assert_eq!(
            list.shown_except(&[id(101), id(104)]),
            [id(100), id(103), id(105)]
        );
        list.set_filter("dubwise");
        assert_eq!(list.shown_except(&[id(104)]), [id(105)]);
    }
}
