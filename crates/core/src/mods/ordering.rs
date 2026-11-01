use crate::mods::community::Community;
use crate::mods::infer::Inferred;
use crate::mods::library::Library;
use crate::mods::list::{self, Category, Mod};
use crate::mods::rules::{self, Kind, Rule};
use crate::mods::sort;
use crate::mods::userrules::UserRules;

#[derive(Clone, Copy)]
pub struct Sources<'a> {
    pub yours: &'a UserRules,
    /// Empty unless the community rules are downloaded and switched on.
    pub community: Option<&'a Community>,
    pub from_files: &'a [Inferred],
    /// Compiled code's order always counts: the game needs that order.
    pub hints: bool,
}

pub fn all_rules(library: &Library, mods: &[Mod], sources: Sources) -> Vec<Rule> {
    let abouts = library.abouts(mods);
    let mut found = rules::among(&abouts, &library.game_version);
    found.extend(sources.yours.rules(&abouts));
    let from_files = sources
        .from_files
        .iter()
        .filter(|d| d.code || sources.hints);
    found.extend(from_files.map(|d| Rule {
        first: d.first,
        then: d.then,
        kind: if d.code { Kind::Inferred } else { Kind::Hint },
    }));
    if let Some(community) = sources.community {
        found.extend(community.rules(&abouts));
    }
    found
}

pub fn place(rules: &[Rule], at: usize, count: usize) -> usize {
    let among_others = |m: usize| if m > at { m - 1 } else { m };
    let must_follow: Vec<usize> = rules
        .iter()
        .filter(|r| r.then == at)
        .map(|r| among_others(r.first))
        .collect();
    let must_precede: Vec<usize> = rules
        .iter()
        .filter(|r| r.first == at)
        .map(|r| among_others(r.then))
        .collect();
    sort::place(at, count.saturating_sub(1), &must_follow, &must_precede)
}

pub fn sorted_by_category(
    library: &Library,
    mut mods: Vec<Mod>,
    sources: Sources,
) -> (Vec<Mod>, sort::Sorted) {
    let found = all_rules(library, &mods, sources);
    for m in &mut mods {
        m.category = m.filed;
    }
    if let Some(community) = sources.community {
        let abouts = library.abouts(&mods);
        let top = abouts
            .iter()
            .map(|about| about.is_some_and(|a| community.loads_at_top(&a.package_id)));
        let top: Vec<bool> = top.collect();
        for (m, top) in mods.iter_mut().zip(top) {
            if top && !m.chosen && m.category > Category::Libraries {
                m.category = Category::Libraries;
            }
        }
    }
    let mut subs: Vec<&str> = Vec::new();
    for m in mods
        .iter()
        .filter(|m| !m.sub.is_empty() && m.category != Category::Last)
    {
        if !subs.contains(&m.sub.as_str()) {
            subs.push(&m.sub);
        }
    }
    let per_category = subs.len() + 1;
    let rank = |m: &Mod| subs.iter().position(|s| *s == m.sub).map_or(0, |at| at + 1);
    let own: Vec<usize> = mods
        .iter()
        .map(|m| m.category as usize * per_category + rank(m))
        .collect();
    let bottom = Category::Last as usize * per_category;
    let mut sorted = sort::sort_grouped(&own, bottom, &found);
    for group in &mut sorted.groups {
        *group /= per_category;
    }
    let before: Vec<Category> = mods.iter().map(|m| m.category).collect();
    for (m, &group) in mods.iter_mut().zip(&sorted.groups) {
        m.category = Category::ALL[group];
    }
    let held_by: Vec<String> = (0..mods.len())
        .map(|at| {
            let pulled = mods[at].category < before[at];
            let holder = sorted.kept.iter().find(|r| {
                r.first == at
                    && !matches!(r.kind, Kind::Hint | Kind::CommunityBottom)
                    && mods[r.then].category == mods[at].category
            });
            match (pulled, holder) {
                (true, Some(rule)) => mods[rule.then].name.clone(),
                _ => String::new(),
            }
        })
        .collect();
    for (m, holder) in mods.iter_mut().zip(held_by) {
        m.held_by = holder;
    }
    (mods, sorted)
}

pub fn tidy(mods: &[Mod]) -> bool {
    list::runs(mods) <= 2 * Category::ALL.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::library::{build, scan};
    use crate::rimworld::paths::{ModRoot, Source};
    use std::fs;

    fn four_mods() -> (tempfile::TempDir, Library) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Mods");
        for (id, folder) in [
            ("a", "Defs"),
            ("b", "Defs"),
            ("c", "Textures"),
            ("d", "Textures"),
        ] {
            fs::create_dir_all(root.join(id).join("About")).unwrap();
            fs::create_dir_all(root.join(id).join(folder)).unwrap();
            let about = format!(
                "<ModMetaData><packageId>t.{id}</packageId><name>{id}</name></ModMetaData>"
            );
            fs::write(root.join(id).join("About/About.xml"), about).unwrap();
        }
        let roots = [ModRoot {
            path: root,
            source: Source::Local,
        }];
        let active: Vec<String> = ["t.a", "t.b", "t.c", "t.d"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        (tmp, build(scan(&roots), &active, "1.6"))
    }

    fn yours(pairs: &[(&str, &str)]) -> UserRules {
        let mut rules = UserRules::default();
        for (first, then) in pairs {
            rules.add(first, then);
        }
        rules
    }

    fn inferred(first: usize, then: usize, code: bool) -> Inferred {
        Inferred {
            first,
            then,
            why: Default::default(),
            code,
        }
    }

    #[test]
    fn rules_come_from_you_and_from_files_and_hints_only_when_asked() {
        let (_tmp, library) = four_mods();
        let own = yours(&[("t.b", "t.a")]);
        let from_files = [inferred(2, 3, true), inferred(0, 3, false)];
        let mut sources = Sources {
            yours: &own,
            community: None,
            from_files: &from_files,
            hints: false,
        };
        let pairs = |sources| -> Vec<(usize, usize, Kind)> {
            let found = all_rules(&library, &library.active, sources);
            found
                .into_iter()
                .map(|r| (r.first, r.then, r.kind))
                .collect()
        };
        assert_eq!(
            pairs(sources),
            [(1, 0, Kind::Yours), (2, 3, Kind::Inferred)]
        );
        sources.hints = true;
        assert_eq!(pairs(sources).len(), 3);
    }

    #[test]
    fn a_mod_is_placed_after_what_it_must_follow() {
        let rules = [Rule {
            first: 2,
            then: 0,
            kind: Kind::Yours,
        }];
        assert_eq!(place(&rules, 0, 4), 2);
        assert_eq!(place(&[], 1, 4), 1);
    }

    #[test]
    fn sorting_by_category_follows_the_rules_and_keeps_the_old_order_of_the_rest() {
        let (_tmp, library) = four_mods();
        let own = yours(&[("t.b", "t.a")]);
        let sources = Sources {
            yours: &own,
            community: None,
            from_files: &[],
            hints: false,
        };
        let (mods, sorted) = sorted_by_category(&library, library.active.clone(), sources);
        let names: Vec<&str> = sorted
            .order
            .iter()
            .map(|&i| mods[i].name.as_str())
            .collect();
        assert_eq!(names, ["b", "a", "c", "d"]);
        assert!(tidy(&mods));
    }

    #[test]
    fn sorting_keeps_a_sub_category_of_yours_together_inside_its_category() {
        let (_tmp, library) = four_mods();
        let none = UserRules::default();
        let sources = Sources {
            yours: &none,
            community: None,
            from_files: &[],
            hints: false,
        };
        let mut mods = library.active.clone();
        mods[2].filed = Category::Content;
        mods[0].sub = "Genes".into();
        mods[2].sub = "Genes".into();
        let (mods, sorted) = sorted_by_category(&library, mods, sources);
        let names: Vec<&str> = sorted
            .order
            .iter()
            .map(|&i| mods[i].name.as_str())
            .collect();
        assert_eq!(names, ["b", "a", "c", "d"]);
        assert_eq!(mods[2].category, Category::Content);
        assert_eq!(sorted.groups[3], Category::Textures as usize);
    }

    #[test]
    fn a_mod_filed_by_hand_keeps_its_category_while_a_rule_holds_it_earlier() {
        let (_tmp, library) = four_mods();
        let held = yours(&[("t.a", "t.c")]);
        let none = UserRules::default();
        let sources = |yours| Sources {
            yours,
            community: None,
            from_files: &[],
            hints: false,
        };
        let mut mods = library.active.clone();
        mods[0].filed = Category::Interface;
        mods[0].chosen = true;
        mods[2].filed = Category::Libraries;
        let (mods, _) = sorted_by_category(&library, mods, sources(&held));
        let a = &mods[0];
        assert_eq!(
            (a.category, a.filed),
            (Category::Libraries, Category::Interface)
        );
        assert_eq!(a.held_by, "c");
        assert_eq!(mods[1].held_by, "");
        let (mods, _) = sorted_by_category(&library, mods, sources(&none));
        assert_eq!(
            (mods[0].category, mods[0].held_by.as_str()),
            (Category::Interface, "")
        );
    }
}
