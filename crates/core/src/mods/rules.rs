//! Mods are named by their position in the list the rules were built from.

use crate::rimworld::about::About;
use crate::rimworld::id;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    CommunityBottom,
    Inferred,
    /// Inferred from patches, which the game applies after reading every mod: only a tidy order.
    Hint,
    /// The game only checks that a required mod is active; RimStack also loads it first.
    Requires,
    LoadAfter,
    LoadBefore,
    ForceLoadAfter,
    ForceLoadBefore,
    CommunityAfter,
    CommunityBefore,
    Yours,
}

impl Kind {
    pub fn strength(self) -> u8 {
        match self {
            // A hint from files gives way to everything, even "at the bottom".
            Kind::Inferred | Kind::Hint => 0,
            Kind::CommunityBottom => 1,
            Kind::Requires => 2,
            Kind::LoadAfter | Kind::LoadBefore => 3,
            Kind::ForceLoadAfter | Kind::ForceLoadBefore => 4,
            Kind::CommunityAfter | Kind::CommunityBefore => 5,
            Kind::Yours => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rule {
    pub first: usize,
    pub then: usize,
    pub kind: Kind,
}

impl Rule {
    /// `None` for a community rule or one of your own.
    pub fn declared_by(&self) -> Option<usize> {
        match self.kind {
            Kind::LoadBefore | Kind::ForceLoadBefore => Some(self.first),
            Kind::Requires | Kind::LoadAfter | Kind::ForceLoadAfter => Some(self.then),
            Kind::CommunityAfter
            | Kind::CommunityBefore
            | Kind::CommunityBottom
            | Kind::Yours
            | Kind::Inferred
            | Kind::Hint => None,
        }
    }

    pub fn broken(&self) -> bool {
        self.first > self.then
    }

    pub fn troubled(&self) -> bool {
        self.broken() && self.kind != Kind::Hint
    }
}

/// `None` stands for a mod with no readable `About.xml`.
pub fn among(mods: &[Option<&About>], game_version: &str) -> Vec<Rule> {
    let position = positions(mods);

    let mut rules = Vec::new();
    for (me, about) in mods.iter().enumerate() {
        let Some(about) = about else { continue };
        let mut add = |targets: &[String], kind: Kind| {
            for target in targets {
                // A rule may name the Workshop copy ("id_steam") of a mod.
                let (key, _) = id::split_steam(target);
                let Some(&other) = position.get(&key) else {
                    continue;
                };
                if other == me {
                    continue;
                }
                let (first, then) = match kind {
                    Kind::LoadBefore | Kind::ForceLoadBefore => (me, other),
                    _ => (other, me),
                };
                rules.push(Rule { first, then, kind });
            }
        };
        let required: Vec<String> = about
            .dependencies
            .for_version(game_version)
            .iter()
            .filter_map(|dep| {
                let listed = |accepted: &&str| position.contains_key(&id::split_steam(accepted).0);
                about.accepted_as(dep).find(listed).map(str::to_string)
            })
            .collect();
        add(&required, Kind::Requires);
        add(about.load_after.for_version(game_version), Kind::LoadAfter);
        add(
            about.load_before.for_version(game_version),
            Kind::LoadBefore,
        );
        add(&about.force_load_after, Kind::ForceLoadAfter);
        add(&about.force_load_before, Kind::ForceLoadBefore);
    }
    rules
}

pub(crate) fn positions(mods: &[Option<&About>]) -> HashMap<String, usize> {
    let mut position = HashMap::new();
    for (i, about) in mods.iter().enumerate() {
        if let Some(about) = about {
            position.entry(id::fold(&about.package_id)).or_insert(i);
        }
    }
    position
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rimworld::about::Versioned;

    fn about(id: &str) -> About {
        About {
            package_id: id.to_string(),
            ..About::default()
        }
    }

    fn list(items: &[String]) -> Versioned {
        Versioned {
            base: items.to_vec(),
            ..Versioned::default()
        }
    }

    fn rules_of(abouts: &[About]) -> Vec<Rule> {
        let refs: Vec<Option<&About>> = abouts.iter().map(Some).collect();
        among(&refs, "1.6")
    }

    #[test]
    fn load_after_and_load_before_point_the_right_way() {
        let mut a = about("a.a");
        a.load_after = list(&["b.b".to_string()]);
        let mut c = about("c.c");
        c.load_before = list(&["a.a".to_string()]);
        let rules = rules_of(&[a, about("b.b"), c]);
        assert_eq!(
            rules,
            vec![
                Rule {
                    first: 1,
                    then: 0,
                    kind: Kind::LoadAfter
                },
                Rule {
                    first: 2,
                    then: 0,
                    kind: Kind::LoadBefore
                },
            ]
        );
        assert_eq!(rules[0].declared_by(), Some(0));
        assert_eq!(rules[1].declared_by(), Some(2));
    }

    #[test]
    fn ids_match_in_any_case_and_with_the_steam_suffix() {
        let mut a = about("Author.Mod");
        a.load_after = list(&["Brrainz.Harmony_steam".to_string()]);
        let rules = rules_of(&[a, about("brrainz.harmony")]);
        assert_eq!(rules.len(), 1);
        assert_eq!((rules[0].first, rules[0].then), (1, 0));
    }

    #[test]
    fn a_rule_about_an_absent_mod_or_the_mod_itself_is_left_out() {
        let mut a = about("a.a");
        a.load_after = list(&["not.here".to_string(), "a.a".to_string()]);
        assert!(rules_of(&[a]).is_empty());
    }

    #[test]
    fn a_by_version_list_replaces_the_base_list() {
        let mut a = about("a.a");
        a.load_after = list(&["b.b".to_string()]);
        a.load_after
            .by_version
            .insert("1.6".to_string(), vec!["c.c".to_string()]);
        let rules = rules_of(&[a, about("b.b"), about("c.c")]);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].first, 2);
    }

    #[test]
    fn requirements_and_forced_rules_are_rules_too() {
        let mut a = about("a.a");
        a.dependencies = list(&["b.b".to_string()]);
        a.force_load_before = vec!["b.b".to_string()];
        let rules = rules_of(&[a, about("b.b")]);
        assert_eq!(
            rules.iter().map(|r| r.kind).collect::<Vec<_>>(),
            vec![Kind::Requires, Kind::ForceLoadBefore]
        );
        assert!(rules[0].broken());
        assert!(!rules[1].broken());
    }

    #[test]
    fn a_requirement_met_by_an_alternative_orders_after_the_alternative() {
        let mut a = about("a.a");
        a.dependencies = list(&["main.mod".to_string()]);
        a.alternatives
            .insert("main.mod".into(), vec!["Main.Mod.Dev".into()]);
        let rules = rules_of(&[a, about("main.mod.dev")]);
        assert_eq!(
            rules,
            vec![Rule {
                first: 1,
                then: 0,
                kind: Kind::Requires
            }]
        );
    }

    #[test]
    fn a_mod_that_cannot_be_read_has_no_rules() {
        let mut a = about("a.a");
        a.load_after = list(&["b.b".to_string()]);
        let rules = among(&[None, Some(&a)], "1.6");
        assert!(rules.is_empty());
    }
}
