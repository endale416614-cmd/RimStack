//! Active mods come first in each.

use crate::mods::library::{Library, ModId};
use crate::sources::replacements::Replacement;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item<'a> {
    pub mod_id: ModId,
    pub name: &'a str,
    pub active: bool,
}

/// Keyed by Workshop id. `active` holds the mods in the load order.
pub fn items<'a>(library: &'a Library, active: &HashSet<ModId>) -> HashMap<&'a str, Item<'a>> {
    let named = |(id, mod_id): (&'a str, ModId)| {
        let item = Item {
            mod_id,
            name: library.get(mod_id)?.display_name(),
            active: active.contains(&mod_id),
        };
        Some((id, item))
    };
    library
        .workshop_mods()
        .into_iter()
        .filter_map(named)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Behind {
    pub id: String,
    pub mod_id: ModId,
    pub name: String,
    pub active: bool,
    pub have: u64,
    pub now: u64,
}

/// Active mods first, then the most recently updated.
pub fn behind(
    items: &HashMap<&str, Item>,
    on_disk: &HashMap<String, u64>,
    now: &HashMap<String, u64>,
) -> Vec<Behind> {
    let mut rows: Vec<Behind> = now
        .iter()
        .filter_map(|(id, &now)| {
            let have = *on_disk.get(id)?;
            let item = items.get(id.as_str()).filter(|_| now > have)?;
            Some(Behind {
                id: id.clone(),
                mod_id: item.mod_id,
                name: item.name.to_string(),
                active: item.active,
                have,
                now,
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        (!a.active, Reverse(a.now), &a.id).cmp(&(!b.active, Reverse(b.now), &b.id))
    });
    rows
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Replaceable {
    pub id: String,
    pub mod_id: ModId,
    pub name: String,
    pub active: bool,
    pub new_name: String,
    pub new_id: String,
    pub new_versions: String,
    pub have_new: bool,
}

pub fn replaceable(
    items: &HashMap<&str, Item>,
    known: &HashMap<String, Replacement>,
    ignored: &HashSet<String>,
) -> Vec<Replaceable> {
    let mut rows: Vec<Replaceable> = items
        .iter()
        .filter(|(id, _)| !ignored.contains(**id))
        .filter_map(|(id, item)| {
            let new = known.get(*id)?;
            Some(Replaceable {
                id: id.to_string(),
                mod_id: item.mod_id,
                name: item.name.to_string(),
                active: item.active,
                new_name: new.name.clone(),
                new_id: new.workshop_id.clone(),
                new_versions: new.versions.join(", "),
                have_new: items.contains_key(new.workshop_id.as_str()),
            })
        })
        .collect();
    rows.sort_by_cached_key(|row| (!row.active, row.name.to_lowercase(), row.id.clone()));
    rows
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Changed {
    /// A Git checkout's commits, not a Workshop item's update.
    pub git: bool,
    pub id: String,
    pub mod_id: ModId,
    pub name: String,
    pub active: bool,
    pub was: u64,
    pub now: Option<u64>,
    pub notes: Vec<crate::sources::changes::Note>,
}

pub fn changed(
    items: &HashMap<&str, Item>,
    updated: &[(String, u64)],
    now: &HashMap<String, u64>,
) -> Vec<Changed> {
    let one = |(id, was): &(String, u64)| {
        let item = items.get(id.as_str())?;
        Some(Changed {
            git: false,
            id: id.clone(),
            mod_id: item.mod_id,
            name: item.name.to_string(),
            active: item.active,
            was: *was,
            now: now.get(id).copied(),
            notes: Vec::new(),
        })
    };
    updated.iter().filter_map(one).collect()
}

pub fn newest_first(mut rows: Vec<Changed>) -> Vec<Changed> {
    rows.sort_by(|a, b| {
        (!a.active, Reverse(a.now), &a.id).cmp(&(!b.active, Reverse(b.now), &b.id))
    });
    rows
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Repository {
    pub link: String,
    pub name: String,
    /// None while the game's folder is not known.
    pub into: Option<String>,
    pub installed: bool,
}

pub fn repositories(pasted: &str, local_mods: Option<&std::path::Path>) -> Vec<Repository> {
    let one = |link: String| {
        let into = local_mods.map(|mods| crate::sources::git::folder_for(&link, mods));
        Repository {
            name: link.split('/').skip(3).collect::<Vec<_>>().join("/"),
            installed: false,
            into: into.map(|folder| folder.to_string_lossy().into_owned()),
            link,
        }
    };
    let mut found: Vec<Repository> = crate::sources::git::links_in(pasted)
        .into_iter()
        .map(one)
        .collect();
    recheck(&mut found);
    found
}

pub fn recheck(repositories: &mut [Repository]) {
    for repository in repositories {
        repository.installed = repository
            .into
            .as_ref()
            .is_some_and(|folder| std::path::Path::new(folder).exists());
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Asked {
    pub id: String,
    /// None when Steam said nothing of the item: removed, or private.
    pub title: Option<String>,
    pub updated: Option<u64>,
    pub megabytes: Option<f64>,
    pub installed: bool,
}

/// One line per id, in the order asked, even where Steam said nothing.
pub fn asked(
    ids: &[String],
    answered: &[crate::rimworld::workshop::Details],
    installed: &dyn Fn(&str) -> bool,
) -> Vec<Asked> {
    let one = |id: &String| {
        let said = answered.iter().find(|details| &details.id == id);
        Asked {
            id: id.clone(),
            title: said.map(|details| details.title.clone()),
            updated: said.map(|details| details.updated),
            megabytes: said.map(|details| details.size as f64 / 1_048_576.0),
            installed: installed(id),
        }
    };
    ids.iter().map(one).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> HashMap<&'static str, Item<'static>> {
        let item = |at, name, active| Item {
            mod_id: ModId::new(0, at),
            name,
            active,
        };
        HashMap::from([
            ("100", item(0, "Old Roads", false)),
            ("200", item(1, "Zebra", true)),
            ("300", item(2, "apples", true)),
            ("400", item(3, "New Roads", false)),
        ])
    }

    #[test]
    fn what_changed_lists_the_installed_mods_updated_since_newest_active_first() {
        let updated = [
            ("100".to_string(), 5),
            ("200".to_string(), 6),
            ("999".to_string(), 7),
        ];
        let now = times(&[("100", 80), ("200", 70)]);
        let rows = newest_first(changed(&sample(), &updated, &now));
        let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(ids, ["200", "100"]);
        assert_eq!(
            (rows[0].was, rows[0].now, rows[0].git),
            (6, Some(70), false)
        );
    }

    #[test]
    fn a_pasted_repository_says_where_it_would_go_and_whether_it_is_there() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("Roads")).unwrap();
        let pasted = "https://github.com/ann/Roads and https://github.com/ann/Hats.git";
        let found = repositories(pasted, Some(tmp.path()));
        assert_eq!(
            (found[0].name.as_str(), found[0].installed),
            ("ann/Roads", true)
        );
        assert_eq!(
            (found[1].name.as_str(), found[1].installed),
            ("ann/Hats", false)
        );
        assert!(found[1].into.as_deref().unwrap().ends_with("Hats"));
        let mut cloned = found.clone();
        std::fs::create_dir(tmp.path().join("Hats")).unwrap();
        recheck(&mut cloned);
        assert!(cloned[1].installed, "a clone since the look-up is seen");
        let unknown = repositories(pasted, None);
        assert_eq!(
            (unknown[0].into.as_deref(), unknown[0].installed),
            (None, false)
        );
    }

    #[test]
    fn every_id_asked_gets_a_line_even_where_steam_said_nothing() {
        let details = crate::rimworld::workshop::Details {
            id: "200".to_string(),
            title: "Zebra".to_string(),
            updated: 9,
            size: 3_145_728,
        };
        let ids = ["777".to_string(), "200".to_string()];
        let lines = asked(&ids, &[details], &|id| id == "200");
        assert_eq!(
            (lines[0].id.as_str(), &lines[0].title, lines[0].installed),
            ("777", &None, false)
        );
        assert_eq!(
            (lines[1].title.as_deref(), lines[1].megabytes),
            (Some("Zebra"), Some(3.0))
        );
        assert!(lines[1].installed);
    }

    fn times(pairs: &[(&str, u64)]) -> HashMap<String, u64> {
        pairs.iter().map(|(id, at)| (id.to_string(), *at)).collect()
    }

    #[test]
    fn a_mod_is_behind_when_the_workshop_s_copy_is_newer_than_the_one_on_disk() {
        let on_disk = times(&[("100", 50), ("200", 10), ("300", 10), ("400", 10)]);
        let now = times(&[
            ("100", 90),
            ("200", 20),
            ("300", 30),
            ("400", 10),
            ("999", 99),
            ("500", 5),
        ]);
        let rows = behind(&sample(), &on_disk, &now);
        let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(ids, ["300", "200", "100"]);
        assert_eq!(
            (rows[2].have, rows[2].now, rows[2].mod_id),
            (50, 90, ModId::new(0, 0))
        );
        assert!(behind(&sample(), &HashMap::new(), &now).is_empty());
    }

    #[test]
    fn a_replacement_is_listed_unless_set_aside_and_says_whether_it_is_installed() {
        let new = |id: &str, name: &str| Replacement {
            workshop_id: id.to_string(),
            name: name.to_string(),
            author: String::new(),
            versions: vec!["1.5".to_string(), "1.6".to_string()],
        };
        let known = HashMap::from([
            ("100".to_string(), new("400", "New Roads")),
            ("200".to_string(), new("777", "Zebra Continued")),
            ("300".to_string(), new("778", "Apples Continued")),
            ("999".to_string(), new("779", "Not installed")),
        ]);
        let rows = replaceable(&sample(), &known, &HashSet::from(["300".to_string()]));
        let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(ids, ["200", "100"]);
        assert!(rows[1].have_new && !rows[0].have_new);
        assert_eq!(rows[0].new_versions, "1.5, 1.6");
        let said = serde_json::to_value(&rows[1]).unwrap();
        assert_eq!(said["modId"], 0);
        assert_eq!(said["newId"], "400");
    }
}
