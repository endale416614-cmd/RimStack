use super::rows::{Level, load_time_text, trouble_level};
use super::{LoadOrder, Side};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::ModId;
use rimstack_core::mods::list::{Category, Row};
use rimstack_core::say;
use std::collections::{HashMap, HashSet};

#[derive(PartialEq)]
pub struct Chosen {
    pub name: String,
    pub category: Category,
    pub load: String,
    pub trouble: Level,
}

#[derive(Default, PartialEq)]
pub struct SelectionSummary {
    pub place: Sentence,
    /// Those with a row in view, in the order of their list.
    pub rows: Vec<Chosen>,
    /// Empty when none has a problem.
    pub troubled: Sentence,
    pub not_in_view: usize,
}

impl LoadOrder {
    pub fn selection_summary(&self, active: bool, selected: &[ModId]) -> SelectionSummary {
        let side = Side::of(active);
        let list = self.list(side);
        let at: HashMap<ModId, usize> = list
            .mods()
            .iter()
            .enumerate()
            .filter_map(|(at, m)| Some((m.installed?, at)))
            .collect();
        let in_view: HashSet<usize> = list
            .rows()
            .iter()
            .filter_map(|row| match row {
                Row::Mod { index } => Some(*index),
                _ => None,
            })
            .collect();
        let mut found: Vec<usize> = selected
            .iter()
            .filter_map(|id| at.get(id).copied())
            .filter(|at| in_view.contains(at))
            .collect();
        found.sort_unstable();
        let own = |at: usize| match side {
            Side::Active => self.troubles.get(at).map(Vec::as_slice).unwrap_or_default(),
            Side::Inactive => &[],
        };
        let rows: Vec<Chosen> = found
            .iter()
            .map(|&at| {
                let m = &list.mods()[at];
                Chosen {
                    name: m.name.clone(),
                    category: m.category,
                    load: load_time_text(m),
                    trouble: trouble_level(own(at)),
                }
            })
            .collect();
        let place = match (active, found.first().zip(found.last())) {
            (false, _) => say!("Inactive"),
            (true, None) => say!("In the load order"),
            (true, Some((first, last))) if last - first + 1 == found.len() => {
                say!("In the load order, {} to {}", first + 1, last + 1)
            }
            (true, Some((first, last))) => {
                say!("In the load order, between {} and {}", first + 1, last + 1)
            }
        };
        let troubled = rows.iter().filter(|m| m.trouble > Level::None).count();
        let version = self.version_warned(selected).len();
        let troubled = match (troubled, version) {
            (0, _) => Sentence::default(),
            (n, 0) => Sentence::Number(n as i64),
            (n, v) if v == n => say!("{}, all for the game version", n),
            (n, v) => say!("{}, {} for the game version", n, v),
        };
        SelectionSummary {
            place,
            not_in_view: selected.len() - rows.len(),
            rows,
            troubled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::installed;

    fn four_active() -> (tempfile::TempDir, crate::App) {
        let (tmp, app) = installed();
        for (folder, id, versions) in [
            (
                "a",
                "Ann.A",
                "<supportedVersions><li>1.5</li></supportedVersions><incompatibleWith><li>Ann.C</li></incompatibleWith>",
            ),
            ("b", "Ann.B", ""),
            ("c", "Ann.C", ""),
        ] {
            let about = tmp.path().join("game/Mods").join(folder).join("About");
            std::fs::create_dir_all(&about).unwrap();
            let xml = format!(
                "<ModMetaData><packageId>{id}</packageId><name>{id}</name>{versions}</ModMetaData>"
            );
            std::fs::write(about.join("About.xml"), xml).unwrap();
        }
        let active = "<li>ludeon.rimworld</li><li>ann.a</li><li>ann.b</li><li>ann.c</li>";
        std::fs::write(
            tmp.path().join("config/Config/ModsConfig.xml"),
            format!("<ModsConfigData><activeMods>{active}</activeMods></ModsConfigData>"),
        )
        .unwrap();
        app.rescan();
        (tmp, app)
    }

    fn ids(order: &LoadOrder, side: Side, keys: &[&str]) -> Vec<ModId> {
        let mods = order.list(side).mods();
        let find = |key: &&str| {
            mods.iter()
                .find(|m| m.id == *key)
                .unwrap()
                .installed
                .unwrap()
        };
        keys.iter().map(find).collect()
    }

    fn said(sentence: &Sentence) -> String {
        sentence.to_string()
    }

    #[test]
    fn mods_chosen_say_where_the_run_starts_and_ends_or_between_which_places() {
        let (_tmp, app) = four_active();
        let order = LoadOrder::read(&app, true, None);
        let chosen = ids(&order, Side::Active, &["ann.c", "ann.b"]);
        let summary = order.selection_summary(true, &chosen);
        assert_eq!(said(&summary.place), "In the load order, 3 to 4");
        let names: Vec<&str> = summary.rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Ann.B", "Ann.C"]);
        assert_eq!(summary.not_in_view, 0);
        let apart = ids(&order, Side::Active, &["ludeon.rimworld", "ann.c"]);
        let summary = order.selection_summary(true, &apart);
        assert_eq!(said(&summary.place), "In the load order, between 1 and 4");
    }

    #[test]
    fn inactive_mods_are_only_called_inactive() {
        let (_tmp, app) = installed();
        let order = LoadOrder::read(&app, true, None);
        let roads = ids(&order, Side::Inactive, &["ann.roads"]);
        let summary = order.selection_summary(false, &roads);
        assert_eq!(said(&summary.place), "Inactive");
        assert_eq!(summary.rows.len(), 1);
    }

    #[test]
    fn a_mod_filtered_out_is_not_in_view() {
        let (_tmp, app) = four_active();
        let mut order = LoadOrder::read(&app, true, None);
        let chosen = ids(&order, Side::Active, &["ann.b", "ann.c"]);
        order.set_filter("Ann.B");
        let summary = order.selection_summary(true, &chosen);
        assert_eq!((summary.rows.len(), summary.not_in_view), (1, 1));
        assert_eq!(said(&summary.place), "In the load order, 3 to 3");
    }

    #[test]
    fn a_problem_for_the_game_version_is_counted_apart() {
        let (_tmp, app) = four_active();
        let mut order = LoadOrder::read(&app, true, None);
        order.settle(&app);
        if let Some(members) = order.files_to_read(&app) {
            app.read_active_files(members);
            order.recheck(&app);
        }
        let chosen = ids(&order, Side::Active, &["ann.a", "ann.b"]);
        let summary = order.selection_summary(true, &chosen);
        assert_eq!(said(&summary.troubled), "1, all for the game version");
        let both = ids(&order, Side::Active, &["ann.a", "ann.c"]);
        let summary = order.selection_summary(true, &both);
        assert_eq!(said(&summary.troubled), "2, 1 for the game version");
        let none = ids(&order, Side::Active, &["ann.b", "ludeon.rimworld"]);
        assert!(order.selection_summary(true, &none).troubled.is_empty());
    }
}
