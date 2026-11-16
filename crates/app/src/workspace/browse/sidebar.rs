use rimstack_core::base::{sentence::Sentence, versions};
use rimstack_core::rimworld::workshopsearch::{Counts, Filters};
use rimstack_core::sources::browse::Catalog;
use rimstack_core::{say, say_n};
use serde::Serialize;

const TYPES: [&str; 3] = ["Mod", "Translation", "Scenario"];

const TAG: &str = "tag:";
const DLC: &str = "dlc:";
const OWNED: &str = "owned";
const INCOMPATIBLE: &str = "incompatible";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Picked {
    pub include_tags: Vec<String>,
    pub exclude_tags: Vec<String>,
    pub include_dlc: Vec<String>,
    pub exclude_dlc: Vec<String>,
    pub hide_unowned: bool,
    pub show_incompatible: bool,
}

fn toggled(into: &mut Vec<String>, other: &mut Vec<String>, value: &str) {
    other.retain(|v| !v.eq_ignore_ascii_case(value));
    match into.iter().position(|v| v.eq_ignore_ascii_case(value)) {
        Some(at) => {
            into.remove(at);
        }
        None => into.push(value.to_string()),
    }
}

impl Picked {
    pub fn tag(&mut self, tag: &str, include: bool) {
        match include {
            true => toggled(&mut self.include_tags, &mut self.exclude_tags, tag),
            false => toggled(&mut self.exclude_tags, &mut self.include_tags, tag),
        }
    }

    pub fn dlc(&mut self, app: &str, include: bool) {
        match include {
            true => toggled(&mut self.include_dlc, &mut self.exclude_dlc, app),
            false => toggled(&mut self.exclude_dlc, &mut self.include_dlc, app),
        }
    }

    /// A chip's or a row's id: `tag:Mod`, `dlc:1826140`, `owned` or `incompatible`.
    pub fn clear(&mut self, id: &str) {
        if let Some(tag) = id.strip_prefix(TAG) {
            self.include_tags.retain(|v| !v.eq_ignore_ascii_case(tag));
            self.exclude_tags.retain(|v| !v.eq_ignore_ascii_case(tag));
        } else if let Some(app) = id.strip_prefix(DLC) {
            self.include_dlc.retain(|v| v != app);
            self.exclude_dlc.retain(|v| v != app);
        } else if id == OWNED {
            self.hide_unowned = false;
        } else if id == INCOMPATIBLE {
            self.show_incompatible = false;
        }
    }

    pub fn filters(&self, owned_dlc: Option<Vec<String>>) -> Filters {
        Filters {
            include_tags: self.include_tags.clone(),
            exclude_tags: self.exclude_tags.clone(),
            include_dlc: self.include_dlc.clone(),
            exclude_dlc: self.exclude_dlc.clone(),
            owned_dlc: owned_dlc.filter(|_| self.hide_unowned),
            show_incompatible: self.show_incompatible,
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Facet {
    pub id: String,
    pub label: Sentence,
    /// None where the source cannot count (Steam with the key).
    pub count: Option<Sentence>,
    pub include: bool,
    pub exclude: bool,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Chip {
    pub id: String,
    pub label: Sentence,
    pub exclude: bool,
    pub tone: &'static str,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sidebar {
    pub types: Vec<Facet>,
    pub versions: Vec<Facet>,
    pub dlc: Vec<Facet>,
    pub chips: Vec<Chip>,
    pub found: Sentence,
    pub counted: bool,
    pub filtered: bool,
    pub searched: bool,
    pub hide_unowned: bool,
    pub show_incompatible: bool,
}

fn has(list: &[String], value: &str) -> bool {
    list.iter().any(|v| v.eq_ignore_ascii_case(value))
}

fn number(n: usize) -> Sentence {
    Sentence::Number(i64::try_from(n).unwrap_or(i64::MAX))
}

impl Sidebar {
    pub fn empty() -> Sidebar {
        Sidebar::of(
            None,
            &Catalog::default(),
            &Picked::default(),
            None,
            false,
            0,
        )
    }

    pub fn of(
        counts: Option<&Counts>,
        catalog: &Catalog,
        picked: &Picked,
        game: Option<&str>,
        searched: bool,
        total: usize,
    ) -> Sidebar {
        let in_tags = |tag: &str| {
            counts.map(|c| {
                c.tags
                    .iter()
                    .find(|(t, _)| t.eq_ignore_ascii_case(tag))
                    .map_or(0, |(_, n)| *n)
            })
        };
        let in_dlc =
            |app: &str| counts.map(|c| c.dlc.iter().find(|(a, _)| a == app).map_or(0, |(_, n)| *n));
        let tag_facet = |tag: &str, label: Sentence| Facet {
            id: tag.to_string(),
            label,
            count: in_tags(tag).map(number),
            include: has(&picked.include_tags, tag),
            exclude: has(&picked.exclude_tags, tag),
        };
        let types = TYPES.iter().map(|t| tag_facet(t, tag_label(t))).collect();
        let own = game.map(minor_of);
        let released = |tag: &&String| own.as_ref().is_none_or(|own| !versions::newer(own, tag));
        let mut tags: Vec<String> = catalog.versions.iter().filter(released).cloned().collect();
        let ticked = picked.include_tags.iter().chain(&picked.exclude_tags);
        for tag in game
            .map(minor_of)
            .iter()
            .chain(ticked)
            .filter(|t| is_version(t))
        {
            if !has(&tags, tag) {
                tags.push(tag.clone());
            }
        }
        tags.sort_by_key(|tag| versions::numbers(tag));
        let versions = tags
            .iter()
            .map(|tag| tag_facet(tag, Sentence::Plain(tag.clone())))
            .collect();
        let named = |app: &str| {
            catalog
                .dlc
                .iter()
                .find(|(id, _)| id == app)
                .map_or_else(|| app.to_string(), |(_, name)| name.clone())
        };
        let mut apps: Vec<String> = catalog.dlc.iter().map(|(id, _)| id.clone()).collect();
        let counted = counts
            .into_iter()
            .flat_map(|c| c.dlc.iter().map(|(a, _)| a));
        let ticked = picked.include_dlc.iter().chain(&picked.exclude_dlc);
        for app in counted.chain(ticked) {
            if !apps.contains(app) {
                apps.push(app.clone());
            }
        }
        let mut dlc: Vec<(String, String)> = apps.iter().map(|a| (a.clone(), named(a))).collect();
        dlc.sort_by_key(|(_, name)| name.to_lowercase());
        let dlc = dlc
            .into_iter()
            .map(|(app, name)| Facet {
                label: Sentence::Plain(name),
                count: in_dlc(&app).map(number),
                include: picked.include_dlc.contains(&app),
                exclude: picked.exclude_dlc.contains(&app),
                id: app,
            })
            .collect();
        let mut chips = Vec::new();
        for (tags, exclude) in [(&picked.include_tags, false), (&picked.exclude_tags, true)] {
            for tag in tags {
                let label = tag_label(tag);
                chips.push(chip(format!("{TAG}{tag}"), label, exclude));
            }
        }
        for (apps, exclude) in [(&picked.include_dlc, false), (&picked.exclude_dlc, true)] {
            for app in apps {
                let label = Sentence::Plain(named(app));
                chips.push(chip(format!("{DLC}{app}"), label, exclude));
            }
        }
        if picked.hide_unowned {
            let label = say!("Hiding mods that need an expansion I don't own");
            chips.push(chip(OWNED.into(), label, false));
        }
        if picked.show_incompatible {
            let label = say!("Showing incompatible items");
            chips.push(chip(INCOMPATIBLE.into(), label, false));
        }
        Sidebar {
            types,
            versions,
            dlc,
            filtered: !chips.is_empty(),
            chips,
            found: say_n!(
                total,
                "{} result matching your filters",
                "{} results matching your filters",
                total
            ),
            counted: counts.is_some(),
            searched,
            hide_unowned: picked.hide_unowned,
            show_incompatible: picked.show_incompatible,
        }
    }
}

fn minor_of(game: &str) -> String {
    let numbers = versions::numbers(game);
    match numbers.as_slice() {
        [major, minor, ..] => format!("{major}.{minor}"),
        _ => game.to_string(),
    }
}

fn is_version(tag: &str) -> bool {
    tag.starts_with(|c: char| c.is_ascii_digit())
}

fn tag_label(tag: &str) -> Sentence {
    match tag {
        "Mod" => say!("Mod"),
        "Translation" => say!("Translation"),
        "Scenario" => say!("Scenario"),
        _ => Sentence::Plain(tag.to_string()),
    }
}

fn chip(id: String, label: Sentence, exclude: bool) -> Chip {
    let label = match exclude {
        true => say!("Not {}", label.inside()),
        false => label,
    };
    let tone = if exclude { "err" } else { "accent" };
    Chip {
        id,
        label,
        exclude,
        tone,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts() -> Counts {
        Counts {
            total: 42_815,
            tags: vec![
                ("1.6".into(), 12_819),
                ("1.5".into(), 9),
                ("Mod".into(), 42_815),
            ],
            dlc: vec![("1826140".into(), 7)],
        }
    }

    fn catalog() -> Catalog {
        Catalog {
            versions: vec!["1.4".into(), "1.6".into(), "1.10".into()],
            dlc: vec![("1826140".into(), "Biotech".into())],
        }
    }

    #[test]
    fn ticking_a_row_twice_clears_it_and_the_other_sign_moves_it() {
        let mut picked = Picked::default();
        picked.tag("Mod", true);
        picked.tag("mod", true);
        assert_eq!(picked, Picked::default());
        picked.tag("1.5", true);
        picked.tag("1.5", false);
        assert_eq!(
            (picked.include_tags.len(), picked.exclude_tags.clone()),
            (0, vec!["1.5".to_string()])
        );
        picked.dlc("1", false);
        picked.clear("tag:1.5");
        picked.clear("dlc:1");
        assert_eq!(picked, Picked::default());
    }

    #[test]
    fn owned_expansions_only_filter_when_asked() {
        let mut picked = Picked::default();
        assert_eq!(picked.filters(Some(vec!["1".into()])).owned_dlc, None);
        picked.hide_unowned = true;
        assert_eq!(
            picked.filters(Some(vec!["1".into()])).owned_dlc,
            Some(vec!["1".into()])
        );
    }

    #[test]
    fn versions_are_listed_oldest_first_with_zero_where_nothing_is_counted() {
        let mut picked = Picked::default();
        picked.tag("1.6", true);
        picked.tag("1.5", false);
        let side = Sidebar::of(Some(&counts()), &catalog(), &picked, None, true, 12_819);
        let ids: Vec<&str> = side.versions.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["1.4", "1.5", "1.6", "1.10"]);
        let said: Vec<Option<String>> = side
            .versions
            .iter()
            .map(|f| f.count.as_ref().map(ToString::to_string))
            .collect();
        assert_eq!(said[0].as_deref(), Some("0"));
        assert_eq!(said[2].as_deref(), Some("12819"));
        assert!(side.versions[2].include && side.versions[1].exclude);
        assert_eq!(side.types[0].count.as_ref().unwrap().to_string(), "42815");
        assert_eq!(side.dlc[0].label.to_string(), "Biotech");
        assert_eq!(
            side.found.to_string(),
            "12819 results matching your filters"
        );
        let chips: Vec<String> = side.chips.iter().map(|c| c.label.to_string()).collect();
        assert_eq!(chips, ["1.6", "Not 1.5"]);
        assert!(side.counted && side.filtered && side.searched);
    }

    #[test]
    fn the_games_version_is_listed_and_no_newer_one_nor_one_only_counted() {
        let mut counts = counts();
        counts.tags.push(("0.8".into(), 1));
        let side = Sidebar::of(
            Some(&counts),
            &catalog(),
            &Picked::default(),
            Some("1.7.4900 rev1"),
            false,
            1,
        );
        let ids: Vec<&str> = side.versions.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["1.4", "1.6", "1.7"]);
    }

    #[test]
    fn without_counts_the_rows_have_none() {
        let side = Sidebar::of(None, &catalog(), &Picked::default(), None, false, 3);
        assert!(!side.counted && !side.filtered);
        assert!(side.types.iter().all(|f| f.count.is_none()));
        assert_eq!(side.found.to_string(), "3 results matching your filters");
    }
}
