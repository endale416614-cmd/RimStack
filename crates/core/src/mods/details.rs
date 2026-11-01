//! `at` is a place in the load order; a `mod_id` a place in `Library::installed`.

use crate::base::sentence::Sentence;
use crate::mods::conflicts::Overlaps;
use crate::mods::library::ModId;
use crate::mods::list::Mod;
use crate::mods::rules::{Kind, Rule};
use crate::say;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Named {
    pub name: String,
    pub note: Sentence,
    pub trouble: bool,
    pub mod_id: Option<ModId>,
}

pub fn required(requirements: Vec<(String, bool)>) -> Vec<Named> {
    let one = |(name, installed): (String, bool)| Named {
        name,
        note: match installed {
            true => Sentence::default(),
            false => say!("not installed"),
        },
        trouble: !installed,
        mod_id: None,
    };
    requirements.into_iter().map(one).collect()
}

/// Compared as numbers, so 1.10 is newer than 1.9.
pub fn newest(versions: &[String]) -> Option<&str> {
    let parts = |v: &str| -> Vec<u32> { v.split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    versions.iter().map(|v| v.trim()).max_by_key(|v| parts(v))
}

/// Nothing where either side says nothing.
pub fn made_for(supported: &[String], game_version: &str) -> Option<String> {
    let listed = supported.iter().any(|v| v.trim() == game_version);
    let told = !supported.is_empty() && !game_version.is_empty();
    (told && !listed).then(|| supported.join(", "))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Neighbours {
    pub after: Vec<Named>,
    pub before: Vec<Named>,
    pub bottom: bool,
}

fn said_by(rule: &Rule, at: usize) -> Sentence {
    match (rule.troubled(), rule.kind, rule.declared_by()) {
        (true, ..) => say!("out of order"),
        (_, Kind::Yours, _) => say!("your rule"),
        (_, Kind::Inferred | Kind::Hint, _) => say!("detected from files"),
        (_, _, None) => say!("community rule"),
        (_, Kind::Requires, Some(by)) if by == at => say!("required"),
        (_, Kind::Requires, _) => say!("requires this mod"),
        (_, _, Some(by)) if by == at => say!("this mod's rule"),
        _ => say!("that mod's rule"),
    }
}

pub fn neighbours(rules: &[Rule], at: usize, mods: &[Mod]) -> Neighbours {
    let mut found = Neighbours::default();
    let (mut above, mut below): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
    for rule in rules.iter().filter(|r| r.first == at || r.then == at) {
        if rule.kind == Kind::CommunityBottom {
            found.bottom |= rule.then == at;
            continue;
        }
        let (seen, lines, other) = match rule.then == at {
            true => (&mut above, &mut found.after, rule.first),
            false => (&mut below, &mut found.before, rule.then),
        };
        if seen.contains(&other) {
            continue;
        }
        seen.push(other);
        lines.push(Named {
            name: mods[other].name.clone(),
            note: said_by(rule, at),
            trouble: rule.troubled(),
            mod_id: mods[other].installed,
        });
    }
    found
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnRule {
    pub first: ModId,
    pub then: ModId,
    pub said: Sentence,
}

pub fn own_rules(yours: &[Rule], at: usize, mods: &[Mod]) -> Vec<OwnRule> {
    let about = yours.iter().filter(|r| r.first == at || r.then == at);
    about
        .filter_map(|rule| {
            let said = match rule.then == at {
                true => crate::say!("Loads after {}", mods[rule.first].name),
                false => crate::say!("Loads before {}", mods[rule.then].name),
            };
            Some(OwnRule {
                first: mods[rule.first].installed?,
                then: mods[rule.then].installed?,
                said,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Clash {
    pub mod_id: ModId,
    pub name: String,
    pub textures: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Clashes {
    pub loses: Vec<Clash>,
    pub wins: Vec<Clash>,
    pub marks: BTreeMap<ModId, Mark>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Mark {
    pub loses: bool,
}

/// Nothing for a mod no longer in the list.
pub fn clashes(
    members: &[ModId],
    overlaps: &Overlaps,
    mod_id: ModId,
    listed: &dyn Fn(ModId) -> Option<(usize, String)>,
) -> Clashes {
    let mut found = Clashes::default();
    let (Ok(place), Some((mine, _))) = (members.binary_search(&mod_id), listed(mod_id)) else {
        return found;
    };
    for (other, textures) in overlaps.with(place) {
        let other_id = members[other];
        let Some((theirs, name)) = listed(other_id) else {
            continue;
        };
        let clash = Clash {
            mod_id: other_id,
            name,
            textures,
        };
        let (side, loses) = match theirs > mine {
            true => (&mut found.loses, false),
            false => (&mut found.wins, true),
        };
        side.push(clash);
        found.marks.insert(other_id, Mark { loses });
    }
    found
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Slow {
    /// -1 for a mod that is not installed.
    pub mod_id: i64,
    pub name: String,
    pub took: String,
    /// Its part of the slowest mod's time, 0 to 1.
    pub part: f64,
}

/// Under a millisecond is not worth a line.
pub fn slowest(mods: &[Mod], most: usize) -> Vec<Slow> {
    let mut timed: Vec<(&Mod, u32)> = mods
        .iter()
        .filter_map(|m| Some((m, m.load_micros.filter(|micros| *micros >= 1000)?)))
        .collect();
    timed.sort_by_key(|(_, micros)| std::cmp::Reverse(*micros));
    let longest = timed.first().map_or(1, |(_, micros)| *micros);
    let one = |&(m, micros): &(&Mod, u32)| Slow {
        mod_id: m.installed.map_or(-1, |id| i64::from(id.to_qml())),
        name: m.name.clone(),
        took: crate::rimworld::startup::seconds(f64::from(micros) / 1000.0),
        part: f64::from(micros) / f64::from(longest),
    };
    timed.iter().take(most).map(one).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::list::Category;

    fn mods(names: &[&str]) -> Vec<Mod> {
        let one = |(at, name): (usize, &&str)| Mod {
            id: name.to_lowercase(),
            name: name.to_string(),
            author: String::new(),
            version: String::new(),
            category: Category::Content,
            installed: Some(ModId::new(0, at + 10)),
            tags: Vec::new(),
            local: false,
            workshop: false,
            git: false,
            code: false,
            patched: Default::default(),
            sub: String::new(),
            load_micros: None,
            filed: Category::Content,
            chosen: false,
            held_by: String::new(),
        };
        names.iter().enumerate().map(one).collect()
    }

    fn rule(first: usize, then: usize, kind: Kind) -> Rule {
        Rule { first, then, kind }
    }

    #[test]
    fn a_mod_s_neighbours_are_named_once_with_who_says_so() {
        let mods = mods(&["Core", "Lib", "Roads", "Hats"]);
        let rules = [
            rule(1, 2, Kind::Yours),
            rule(1, 2, Kind::CommunityAfter),
            rule(2, 3, Kind::CommunityBefore),
            rule(2, 0, Kind::Inferred),
            rule(0, 2, Kind::CommunityBottom),
        ];
        let found = neighbours(&rules, 2, &mods);
        let said = |lines: &[Named]| -> Vec<(String, String, bool)> {
            lines
                .iter()
                .map(|n| (n.name.clone(), n.note.to_string(), n.trouble))
                .collect()
        };
        assert_eq!(
            said(&found.after),
            [("Lib".into(), "your rule".into(), false)]
        );
        assert_eq!(
            said(&found.before),
            [
                ("Hats".into(), "community rule".into(), false),
                ("Core".into(), "out of order".into(), true)
            ]
        );
        assert!(found.bottom);
        assert_eq!(found.after[0].mod_id, Some(ModId::new(0, 11)));
        assert!(!neighbours(&rules, 1, &mods).bottom);
    }

    #[test]
    fn a_hint_out_of_order_is_no_trouble() {
        let mods = mods(&["Core", "Lib"]);
        let found = neighbours(&[rule(1, 0, Kind::Hint)], 0, &mods);
        assert_eq!(found.after[0].note.to_string(), "detected from files");
        assert!(!found.after[0].trouble);
    }

    #[test]
    fn your_rules_about_a_mod_name_the_other_one_and_which_way() {
        let mut mods = mods(&["Core", "Lib", "Roads"]);
        mods[0].installed = None;
        let yours = [
            rule(1, 2, Kind::Yours),
            rule(2, 0, Kind::Yours),
            rule(0, 1, Kind::Yours),
        ];
        let about_roads = own_rules(&yours, 2, &mods);
        assert_eq!(about_roads.len(), 1);
        assert_eq!(about_roads[0].said.to_string(), "Loads after Lib");
        let (lib, roads) = (mods[1].installed.unwrap(), mods[2].installed.unwrap());
        let about_lib = &own_rules(&yours, 1, &mods)[0];
        assert_eq!(about_lib.said.to_string(), "Loads before Roads");
        assert_eq!((about_lib.first, about_lib.then), (lib, roads));
    }

    #[test]
    fn a_mod_loses_to_those_loading_after_it_and_wins_over_those_before() {
        let provided = vec![
            vec!["a.png".to_string(), "b.png".to_string()],
            vec!["a.png".to_string()],
            vec!["a.png".to_string(), "b.png".to_string()],
            vec!["z.png".to_string()],
        ];
        let overlaps = crate::mods::conflicts::overlaps(&provided);
        let id = |n| ModId::new(0, n);
        let members = [id(20), id(30), id(40), id(50)];
        let listed = |of: ModId| {
            let at = [id(40), id(20), id(30)].iter().position(|m| *m == of)?;
            Some((at, format!("mod {}", of.index())))
        };
        let found = clashes(&members, &overlaps, id(20), &listed);
        assert_eq!(found.loses.len(), 1);
        assert_eq!(
            (found.loses[0].mod_id, found.loses[0].textures),
            (id(30), 1)
        );
        assert_eq!((found.wins[0].mod_id, found.wins[0].textures), (id(40), 2));
        let mark = |loses| Mark { loses };
        assert_eq!(
            found.marks,
            BTreeMap::from([(id(30), mark(false)), (id(40), mark(true))])
        );
        assert_eq!(
            clashes(&members, &overlaps, id(50), &listed),
            Clashes::default()
        );
        assert_eq!(
            clashes(&members, &overlaps, id(99), &listed),
            Clashes::default()
        );
    }

    #[test]
    fn the_slowest_mods_are_listed_against_the_slowest_of_all() {
        let mut mods = mods(&["Quick", "Slow", "Slower", "Unmeasured"]);
        mods[0].load_micros = Some(900);
        mods[1].load_micros = Some(50_000);
        mods[2].load_micros = Some(200_000);
        let listed = slowest(&mods, 5);
        let names: Vec<&str> = listed.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Slower", "Slow"]);
        assert_eq!((listed[1].part, listed[1].took.as_str()), (0.25, "50 ms"));
        assert_eq!(slowest(&mods, 1).len(), 1);
    }

    #[test]
    fn a_mod_is_made_for_other_versions_only_when_both_sides_say_which() {
        let versions = |v: &[&str]| -> Vec<String> { v.iter().map(|v| v.to_string()).collect() };
        assert_eq!(
            made_for(&versions(&["1.4", "1.5"]), "1.6").as_deref(),
            Some("1.4, 1.5")
        );
        assert_eq!(made_for(&versions(&["1.5", " 1.6 "]), "1.6"), None);
        assert_eq!(made_for(&[], "1.6"), None);
        assert_eq!(made_for(&versions(&["1.5"]), ""), None);
        assert_eq!(newest(&versions(&["1.10", "1.9", " 1.5 "])), Some("1.10"));
        assert_eq!(newest(&[]), None);
        let needs = required(vec![("Lib".into(), true), ("gone.lib".into(), false)]);
        assert_eq!(
            (needs[0].trouble, needs[1].note.to_string().as_str()),
            (false, "not installed")
        );
    }
}
