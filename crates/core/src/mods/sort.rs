//! Not always the fewest moves. The same input always gives the same output.

use crate::mods::rules::{Kind, Rule};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sorted {
    pub order: Vec<usize>,
    pub kept: Vec<Rule>,
    pub dropped: Vec<Rule>,
    pub groups: Vec<usize>,
}

impl Sorted {
    pub fn moved(&self) -> Vec<bool> {
        moved(&self.order)
    }
}

pub(crate) fn moved(order: &[usize]) -> Vec<bool> {
    let mut tails: Vec<usize> = Vec::new(); // index into `order`, by run length
    let mut previous: Vec<Option<usize>> = vec![None; order.len()];
    for (i, &old) in order.iter().enumerate() {
        let at = tails.partition_point(|&t| order[t] < old);
        previous[i] = at.checked_sub(1).map(|p| tails[p]);
        if at == tails.len() {
            tails.push(i);
        } else {
            tails[at] = i;
        }
    }
    let mut moved = vec![true; order.len()];
    let mut at = tails.last().copied();
    while let Some(i) = at {
        moved[i] = false;
        at = previous[i];
    }
    moved
}

pub fn sort(count: usize, rules: &[Rule]) -> Sorted {
    let (kept, dropped) = without_loops(count, rules);
    Sorted {
        order: fewest_moves(count, &kept).expect("kept rules have no loop"),
        kept,
        dropped,
        groups: vec![0; count],
    }
}

/// Sorting the result again changes nothing.
pub fn sort_grouped(groups: &[usize], bottom_group: usize, rules: &[Rule]) -> Sorted {
    let count = groups.len();
    let (kept, mut dropped) = without_loops(count, rules);
    let own_place: Vec<usize> = (0..count).collect();
    let any_valid = ordered(count, &kept, &own_place).expect("kept rules have no loop");

    // Belonging at the bottom does not pull the others into its group, and neither does a hint.
    let mut after: Vec<Vec<usize>> = vec![Vec::new(); count];
    for rule in kept
        .iter()
        .filter(|r| !matches!(r.kind, Kind::CommunityBottom | Kind::Hint))
    {
        after[rule.first].push(rule.then);
    }
    let mut group = groups.to_vec();
    let mut at_bottom = vec![false; count];
    for rule in rules.iter().filter(|r| r.kind == Kind::CommunityBottom) {
        if rule.then < count {
            at_bottom[rule.then] = true;
        }
    }
    for &m in &any_valid {
        if at_bottom[m] {
            group[m] = bottom_group;
            for &later in &after[m] {
                at_bottom[later] = true;
            }
        }
    }
    for &m in any_valid.iter().rev() {
        let earliest = after[m].iter().map(|&later| group[later]).min();
        group[m] = group[m].min(earliest.unwrap_or(usize::MAX));
    }
    // A hint that would take a mod out of its group gives way, as it does in a loop.
    let (kept, against): (Vec<Rule>, Vec<Rule>) = kept
        .into_iter()
        .partition(|r| r.kind != Kind::Hint || group[r.first] <= group[r.then]);
    dropped.extend(against);
    let place: Vec<usize> = (0..count).map(|m| group[m] * count + m).collect();
    Sorted {
        order: ordered(count, &kept, &place).expect("kept rules have no loop"),
        kept,
        dropped,
        groups: group,
    }
}

/// Returns a place from 0 (first) to `count` (last).
pub fn place(
    preferred: usize,
    count: usize,
    must_follow: &[usize],
    must_precede: &[usize],
) -> usize {
    let earliest = must_follow.iter().map(|&m| m + 1).max().unwrap_or(0);
    let latest = must_precede.iter().copied().min().unwrap_or(count);
    let preferred = preferred.min(count);
    if earliest > latest {
        return earliest;
    }
    preferred.clamp(earliest, latest)
}

fn without_loops(count: usize, rules: &[Rule]) -> (Vec<Rule>, Vec<Rule>) {
    let valid: Vec<Rule> = rules
        .iter()
        .copied()
        .filter(|r| r.first < count && r.then < count && r.first != r.then)
        .collect();
    let own_place: Vec<usize> = (0..count).collect();
    if ordered(count, &valid, &own_place).is_some() {
        return (valid, Vec::new());
    }

    let mut ranked = valid;
    ranked.sort_by_key(|r| (Reverse(r.kind.strength()), r.broken()));
    let mut after: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut kept = Vec::with_capacity(ranked.len());
    let mut dropped = Vec::new();
    for rule in ranked {
        if reaches(&after, rule.then, rule.first) {
            dropped.push(rule);
        } else {
            after[rule.first].push(rule.then);
            kept.push(rule);
        }
    }
    (kept, dropped)
}

/// None when the rules loop.
fn fewest_moves(count: usize, rules: &[Rule]) -> Option<Vec<usize>> {
    let own_place: Vec<usize> = (0..count).collect();
    let pushed_down = ordered(count, rules, &own_place)?;

    let mut after: Vec<Vec<usize>> = vec![Vec::new(); count];
    for rule in rules {
        after[rule.first].push(rule.then);
    }
    let mut place = own_place;
    for &m in pushed_down.iter().rev() {
        let earliest_waiting = after[m].iter().map(|&later| place[later]).min();
        place[m] = place[m].min(earliest_waiting.unwrap_or(usize::MAX));
    }
    let pulled_up = ordered(count, rules, &place)?;

    let moves = |order: &[usize]| moved(order).into_iter().filter(|m| *m).count();
    Some(if moves(&pulled_up) < moves(&pushed_down) {
        pulled_up
    } else {
        pushed_down
    })
}

/// None when the rules loop. Ties go to the lowest `place`, then the earliest mod.
fn ordered(count: usize, rules: &[Rule], place: &[usize]) -> Option<Vec<usize>> {
    let mut after: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut waiting_on = vec![0usize; count];
    for rule in rules {
        after[rule.first].push(rule.then);
        waiting_on[rule.then] += 1;
    }
    let mut ready: BinaryHeap<Reverse<(usize, usize)>> = (0..count)
        .filter(|&i| waiting_on[i] == 0)
        .map(|i| Reverse((place[i], i)))
        .collect();
    let mut order = Vec::with_capacity(count);
    while let Some(Reverse((_, next))) = ready.pop() {
        order.push(next);
        for &later in &after[next] {
            waiting_on[later] -= 1;
            if waiting_on[later] == 0 {
                ready.push(Reverse((place[later], later)));
            }
        }
    }
    (order.len() == count).then_some(order)
}

fn reaches(after: &[Vec<usize>], from: usize, to: usize) -> bool {
    let mut seen = vec![false; after.len()];
    let mut stack = vec![from];
    while let Some(at) = stack.pop() {
        if at == to {
            return true;
        }
        if !std::mem::replace(&mut seen[at], true) {
            stack.extend(&after[at]);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(first: usize, then: usize) -> Rule {
        Rule {
            first,
            then,
            kind: Kind::LoadAfter,
        }
    }

    fn follows(order: &[usize], rule: &Rule) -> bool {
        let at = |m: usize| order.iter().position(|&x| x == m).unwrap();
        at(rule.first) < at(rule.then)
    }

    struct Random(u64);
    impl Random {
        fn below(&mut self, n: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 33) as usize) % n
        }
    }

    fn random_rules(random: &mut Random, count: usize, how_many: usize) -> Vec<Rule> {
        let kinds = [
            Kind::Hint,
            Kind::Requires,
            Kind::LoadAfter,
            Kind::ForceLoadBefore,
        ];
        (0..how_many)
            .map(|_| Rule {
                first: random.below(count),
                then: random.below(count),
                kind: kinds[random.below(kinds.len())],
            })
            .collect()
    }

    #[test]
    fn a_list_with_no_rules_stays_as_it_is() {
        let sorted = sort(4, &[]);
        assert_eq!(sorted.order, vec![0, 1, 2, 3]);
        assert_eq!(sorted.moved(), vec![false; 4]);
    }

    #[test]
    fn a_mod_moves_to_just_after_the_one_it_must_follow() {
        let sorted = sort(5, &[rule(3, 0)]);
        assert_eq!(sorted.order, vec![1, 2, 3, 0, 4]);
        assert!(sorted.dropped.is_empty());
    }

    #[test]
    fn only_the_mod_that_moved_is_marked() {
        let sorted = sort(5, &[rule(3, 0)]);
        assert_eq!(sorted.moved(), vec![false, false, false, true, false]);
    }

    fn moves(sorted: &Sorted) -> usize {
        sorted.moved().iter().filter(|m| **m).count()
    }

    #[test]
    fn a_library_below_the_mods_that_need_it_is_the_only_mod_that_moves() {
        let rules: Vec<Rule> = (5..=15).map(|m| rule(20, m)).collect();
        let sorted = sort(30, &rules);
        assert_eq!(moves(&sorted), 1);
        assert_eq!(sorted.order[5], 20);
    }

    #[test]
    fn a_patch_above_the_mods_it_follows_is_the_only_mod_that_moves() {
        let rules: Vec<Rule> = (5..=15).map(|m| rule(m, 0)).collect();
        let sorted = sort(30, &rules);
        assert_eq!(moves(&sorted), 1);
        assert_eq!(sorted.order[15], 0);
    }

    #[test]
    fn two_rules_that_contradict_drop_the_one_the_list_breaks() {
        let sorted = sort(2, &[rule(1, 0), rule(0, 1)]);
        assert_eq!(sorted.order, vec![0, 1]);
        assert_eq!(sorted.dropped, vec![rule(1, 0)]);
    }

    #[test]
    fn in_a_loop_the_weakest_rule_is_dropped() {
        let weak = Rule {
            first: 0,
            then: 1,
            kind: Kind::Requires,
        };
        let strong = Rule {
            first: 1,
            then: 0,
            kind: Kind::ForceLoadBefore,
        };
        let sorted = sort(2, &[weak, strong]);
        assert_eq!(sorted.order, vec![1, 0]);
        assert_eq!(sorted.dropped, vec![weak]);
    }

    #[test]
    fn rules_about_mods_outside_the_list_are_ignored() {
        let sorted = sort(2, &[rule(5, 0), rule(1, 1)]);
        assert_eq!(sorted.order, vec![0, 1]);
        assert!(sorted.dropped.is_empty());
    }

    #[test]
    fn every_kept_rule_holds_and_every_mod_appears_once() {
        let mut random = Random(7);
        for round in 0..300 {
            let count = 2 + random.below(40);
            let how_many = random.below(60);
            let rules = random_rules(&mut random, count, how_many);
            let sorted = sort(count, &rules);

            let mut seen = sorted.order.clone();
            seen.sort_unstable();
            assert_eq!(seen, (0..count).collect::<Vec<_>>(), "round {round}");
            for r in rules.iter().filter(|r| r.first != r.then) {
                assert!(
                    sorted.dropped.contains(r) || follows(&sorted.order, r),
                    "round {round}: {r:?} broken in {:?}",
                    sorted.order
                );
            }
        }
    }

    #[test]
    fn sorting_a_sorted_list_moves_nothing() {
        let mut random = Random(13);
        for round in 0..300 {
            let count = 2 + random.below(40);
            let how_many = random.below(60);
            let rules = random_rules(&mut random, count, how_many);
            let first = sort(count, &rules);

            let mut now_at = vec![0; count];
            for (position, &old) in first.order.iter().enumerate() {
                now_at[old] = position;
            }
            let renumbered: Vec<Rule> = rules
                .iter()
                .filter(|r| !first.dropped.contains(r))
                .map(|r| Rule {
                    first: now_at[r.first],
                    then: now_at[r.then],
                    kind: r.kind,
                })
                .collect();
            let again = sort(count, &renumbered);
            assert_eq!(again.order, (0..count).collect::<Vec<_>>(), "round {round}");
            assert!(again.moved().iter().all(|m| !m), "round {round}");
        }
    }

    #[test]
    fn grouped_mods_come_out_group_by_group_in_their_old_order() {
        let sorted = sort_grouped(&[2, 0, 1, 0, 2, 1], 9, &[]);
        assert_eq!(sorted.order, vec![1, 3, 2, 5, 0, 4]);
        assert_eq!(sorted.groups, vec![2, 0, 1, 0, 2, 1]);
    }

    #[test]
    fn a_mod_that_an_earlier_group_needs_joins_that_group() {
        let sorted = sort_grouped(&[0, 0, 3, 1], 9, &[rule(2, 0)]);
        assert_eq!(sorted.groups, vec![0, 0, 0, 1]);
        assert_eq!(sorted.order, vec![1, 2, 0, 3]);
    }

    #[test]
    fn a_hint_orders_mods_inside_a_group_but_takes_none_out_of_it() {
        let hint = |first, then| Rule {
            first,
            then,
            kind: Kind::Hint,
        };
        let across = sort_grouped(&[0, 0, 3, 1], 9, &[hint(2, 0)]);
        assert_eq!(across.groups, vec![0, 0, 3, 1]);
        assert_eq!(across.order, vec![0, 1, 3, 2]);
        assert_eq!(across.dropped, vec![hint(2, 0)]);
        let inside = sort_grouped(&[0, 3, 3], 9, &[hint(2, 1)]);
        assert_eq!(inside.order, vec![0, 2, 1]);
    }

    #[test]
    fn a_bottom_mod_and_what_follows_it_go_to_the_last_group() {
        let bottom = |first| Rule {
            first,
            then: 0,
            kind: Kind::CommunityBottom,
        };
        let rules = [bottom(1), bottom(2), rule(0, 3)];
        let sorted = sort_grouped(&[1, 2, 3, 1], 9, &rules);
        assert_eq!(sorted.groups, vec![9, 2, 3, 9]);
        assert_eq!(sorted.order, vec![1, 2, 0, 3]);
    }

    #[test]
    fn a_grouped_sort_holds_its_rules_and_sorting_again_moves_nothing() {
        let mut random = Random(19);
        for round in 0..300 {
            let count = 2 + random.below(40);
            let how_many = random.below(60);
            let rules = random_rules(&mut random, count, how_many);
            let groups: Vec<usize> = (0..count).map(|_| random.below(5)).collect();
            let first = sort_grouped(&groups, 9, &rules);
            for r in rules.iter().filter(|r| r.first != r.then) {
                assert!(
                    first.dropped.contains(r) || follows(&first.order, r),
                    "round {round}"
                );
            }
            let in_order: Vec<usize> = first.order.iter().map(|&m| first.groups[m]).collect();
            assert!(in_order.is_sorted(), "round {round}: {in_order:?}");

            let mut now_at = vec![0; count];
            for (position, &old) in first.order.iter().enumerate() {
                now_at[old] = position;
            }
            let renumbered: Vec<Rule> = rules
                .iter()
                .filter(|r| !first.dropped.contains(r))
                .map(|r| Rule {
                    first: now_at[r.first],
                    then: now_at[r.then],
                    kind: r.kind,
                })
                .collect();
            let again = sort_grouped(&in_order, 9, &renumbered);
            assert_eq!(again.order, (0..count).collect::<Vec<_>>(), "round {round}");
            assert_eq!(again.groups, in_order, "round {round}");
        }
    }

    #[test]
    fn a_new_mod_goes_where_asked_when_nothing_forbids_it() {
        assert_eq!(place(3, 10, &[], &[]), 3);
        assert_eq!(place(99, 10, &[], &[]), 10);
        assert_eq!(place(5, 10, &[1, 4], &[7, 9]), 5);
    }

    #[test]
    fn a_new_mod_goes_after_what_it_follows_and_before_what_follows_it() {
        assert_eq!(place(10, 10, &[], &[6, 4]), 4);
        assert_eq!(place(0, 10, &[2, 5], &[]), 6);
        assert_eq!(place(0, 10, &[3], &[4]), 4);
    }

    #[test]
    fn a_new_mod_with_contradicting_demands_goes_after_what_it_follows() {
        assert_eq!(place(10, 10, &[7], &[2]), 8);
    }

    #[test]
    fn two_thousand_mods_sort_quickly() {
        let mut random = Random(17);
        let rules = random_rules(&mut random, 2000, 6000);
        let started = std::time::Instant::now();
        let sorted = sort(2000, &rules);
        assert_eq!(sorted.order.len(), 2000);
        assert!(started.elapsed().as_secs() < 5);
    }
}
