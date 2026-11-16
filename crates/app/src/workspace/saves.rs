use super::App;
use rimstack_core::base::error::Error;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::rimworld::{rimsaves, saves};
use rimstack_core::{say, say_n};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Default)]
pub struct SavesReport {
    /// Newest first.
    pub rows: Vec<saves::Row>,
    pub folders: Vec<String>,
    pub current_folder: Option<String>,
    pub rimsaves: bool,
    /// `None` if none does.
    pub all_from: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveShown<'a> {
    #[serde(flatten)]
    pub row: &'a saves::Row,
    pub said: Sentence,
    pub about: Sentence,
    pub dir: String,
    pub not_installed: usize,
    pub not_active: usize,
    pub usable: bool,
    /// Each mod the save lacks counted once, with why it is lacking.
    pub missing_said: Sentence,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SavesShown<'a> {
    pub saves: Vec<SaveShown<'a>>,
    pub heading: Sentence,
    pub pictured: bool,
    /// More saves in the folder than a list is easy to scan.
    pub searchable: bool,
}

const FILTER_FROM: usize = 12;

fn missing_said(missing: usize, not_installed: usize) -> Sentence {
    if missing == 0 {
        return Sentence::default();
    }
    let head = say_n!(missing, "{} mod missing", "{} mods missing");
    let not_active = missing - not_installed;
    match (not_installed, not_active) {
        (_, 0) => Sentence::joined(", ", vec![head, say!("not installed")]),
        (0, _) => Sentence::joined(", ", vec![head, say!("not active")]),
        _ => say!(
            "{}: {} not installed, {} not active",
            head.inside(),
            not_installed,
            not_active
        ),
    }
}

fn said(row: &saves::Row) -> Sentence {
    match row.verdict {
        saves::Verdict::Unreadable => say!("Cannot be read"),
        saves::Verdict::Same => say!("Same mods as now"),
        saves::Verdict::Differs => say!("Loads with the current list"),
        saves::Verdict::Breaks => {
            let compared = row.compared.as_ref();
            let missing = compared.map_or(0, |c| c.missing.len());
            let mut parts = Vec::new();
            if missing > 0 {
                parts.push(say_n!(missing, "{} mod missing", "{} mods missing"));
            }
            if compared.is_some_and(|c| c.other_game) {
                parts.push(say!("other game version"));
            }
            Sentence::listed(parts)
        }
    }
}

/// Empty `folder` for every one; `filter` keeps the saves whose name has it.
pub fn shown<'a>(report: &'a SavesReport, folder: &str, filter: &str) -> SavesShown<'a> {
    let in_folder = || {
        report
            .rows
            .iter()
            .filter(|row| folder.is_empty() || row.folder == folder)
    };
    let searchable = in_folder().count() > FILTER_FROM;
    let wanted = filter.trim().to_lowercase();
    let saves: Vec<SaveShown> = in_folder()
        .filter(|row| wanted.is_empty() || row.name.to_lowercase().contains(&wanted))
        .map(|row| {
            let compared = row.compared.as_ref();
            let missing = compared.map_or(&[][..], |c| &c.missing[..]);
            let not_installed = missing.iter().filter(|m| !m.installed).count();
            let mut about = Vec::new();
            if let Some(compared) = compared {
                about.push(Sentence::Plain(format!(
                    "RimWorld {}",
                    compared.game_version
                )));
                about.push(say_n!(compared.mods, "{} mod", "{} mods"));
            }
            if report.rimsaves && folder.is_empty() {
                about.push(Sentence::Plain(row.folder.clone()));
            }
            let dir = std::path::Path::new(&row.path).parent();
            SaveShown {
                row,
                said: said(row),
                about: Sentence::joined("   ", about),
                dir: dir.map_or(String::new(), |d| d.to_string_lossy().into_owned()),
                not_installed,
                not_active: missing.len() - not_installed,
                missing_said: missing_said(missing.len(), not_installed),
                usable: !matches!(
                    row.verdict,
                    saves::Verdict::Unreadable | saves::Verdict::Same
                ),
            }
        })
        .collect();
    let count = saves.len();
    let mut heading = vec![match folder.is_empty() {
        true => Sentence::Number(count as i64),
        false => say!("{} in {}", count, folder),
    }];
    if let Some(version) = &report.all_from {
        heading.push(say!("all from RimWorld {}", version));
    }
    SavesShown {
        pictured: saves.iter().any(|s| s.row.preview.is_some()),
        searchable,
        heading: Sentence::listed(heading),
        saves,
    }
}

impl App {
    pub fn saves_dir(&self) -> Option<PathBuf> {
        self.paths().config.as_ref().map(|c| c.join("Saves"))
    }

    pub fn saves_report(&self) -> SavesReport {
        let config = self.paths().config.clone();
        let list = self.session().current().to_vec();
        let loaded = self.library();
        let rows: Vec<saves::Row> = match (&*loaded, self.saves_dir()) {
            (Ok((library, _)), Some(dir)) => saves::list(&dir)
                .iter()
                .map(|save| {
                    let config = config.clone().unwrap_or_default();
                    let compared = save.header.as_ref().ok().map(|header| {
                        let name_of = |key: &str| library.name_of(key);
                        saves::Compared::of(header, &list, &library.game_version, &name_of)
                    });
                    saves::Row::of(
                        save,
                        rimsaves::facts(&rimsaves::meta(&config, &save.name)),
                        rimsaves::preview(&config, &save.name),
                        compared,
                    )
                })
                .collect(),
            _ => Vec::new(),
        };
        let kept = config.as_deref().and_then(rimsaves::settings);
        let mut folders = kept.as_ref().map_or(Vec::new(), |s| s.folders.clone());
        for row in &rows {
            if !folders.contains(&row.folder) {
                folders.push(row.folder.clone());
            }
        }
        let versions: Vec<(&str, bool)> = rows
            .iter()
            .map(|row| match &row.compared {
                Some(compared) => (compared.game_version.as_str(), compared.other_game),
                None => ("", false),
            })
            .collect();
        SavesReport {
            all_from: saves::version_all_share(&versions),
            rimsaves: kept.is_some(),
            folders,
            current_folder: kept.map(|s| s.current),
            rows,
        }
    }

    pub fn back_up_newest_save(&self, into: &str, kept: usize) -> Result<(), Error> {
        let Some(newest) = self
            .saves_dir()
            .and_then(|dir| saves::list(&dir).into_iter().next())
        else {
            return Ok(());
        };
        let backups = self.data_dir().join(into);
        saves::back_up(&newest.path, &backups, super::now_secs(), kept).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::shown;
    use crate::workspace::tests::installed;

    const SAVE: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>
<savegame><meta><gameVersion>1.6.4871 rev600</gameVersion>
<modIds><li>ludeon.rimworld</li><li>ann.roads</li><li>gone.mod</li></modIds>
<modNames><li>Core</li><li>Ann.Roads</li><li>Gone</li></modNames></meta><game/></savegame>";

    #[test]
    fn a_save_is_compared_with_the_load_order_and_said_in_words() {
        let (tmp, app) = installed();
        let saves = tmp.path().join("config/Saves");
        std::fs::create_dir_all(&saves).unwrap();
        std::fs::write(saves.join("Colony.rws"), SAVE).unwrap();
        let report = app.saves_report();
        let all = crate::words::as_shown(&shown(&report, "", ""));
        let save = &all["saves"][0];
        assert_eq!(save["name"], "Colony");
        assert_eq!(save["verdict"]["breaks"], true);
        assert_eq!(save["said"], "2 mods missing");
        assert_eq!(save["about"], "RimWorld 1.6.4871 rev600   3 mods");
        assert_eq!(save["notInstalled"], 1);
        assert_eq!(save["notActive"], 1);
        assert_eq!(save["usable"], true);
        assert_eq!(
            save["missingSaid"],
            "2 mods missing: 1 not installed, 1 not active"
        );
        let none = crate::words::as_shown(&shown(&report, "", "zzz"));
        assert!(none["saves"].as_array().unwrap().is_empty());
        let some = crate::words::as_shown(&shown(&report, "", " COLO"));
        assert_eq!(some["saves"].as_array().unwrap().len(), 1);
        assert_eq!(save["dir"], saves.to_string_lossy().as_ref());
        assert_eq!(all["heading"], "1");
        let elsewhere = crate::words::as_shown(&shown(&report, "Other", ""));
        assert_eq!(elsewhere["saves"].as_array().unwrap().len(), 0);
        assert_eq!(elsewhere["heading"], "0 in Other");
    }

    #[test]
    fn the_newest_save_is_copied_and_none_is_no_failure() {
        let (tmp, app) = installed();
        app.back_up_newest_save("save-backups", 5).unwrap();
        let saves = tmp.path().join("config/Saves");
        std::fs::create_dir_all(&saves).unwrap();
        std::fs::write(saves.join("Colony.rws"), SAVE).unwrap();
        app.back_up_newest_save("save-backups", 5).unwrap();
        let kept = std::fs::read_dir(app.data_dir().join("save-backups"))
            .unwrap()
            .count();
        assert_eq!(kept, 1);
    }
}

#[cfg(test)]
mod missing_words {
    use super::missing_said;

    #[test]
    fn each_missing_mod_is_counted_once() {
        let english = |missing, absent| missing_said(missing, absent).to_string();
        assert_eq!(english(1, 1), "1 mod missing, not installed");
        assert_eq!(english(2, 0), "2 mods missing, not active");
        assert_eq!(
            english(3, 1),
            "3 mods missing: 1 not installed, 2 not active"
        );
        assert_eq!(english(0, 0), "");
    }
}
