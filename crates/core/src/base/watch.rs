use serde::Serialize;
use std::path::Path;

/// Milliseconds since 1970, or 0 when nothing is there. A folder's time moves
/// when an entry is added or removed.
pub fn changed_at(path: &Path) -> u64 {
    let written = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let since = written.and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok());
    since.map_or(0, |d| d.as_millis() as u64)
}

pub fn newest<'a>(paths: impl IntoIterator<Item = &'a Path>) -> u64 {
    paths.into_iter().map(changed_at).max().unwrap_or(0)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stamps {
    pub mods: u64,
    pub saves: u64,
    pub log: u64,
    /// 0 while it holds the load order last saved or read here.
    pub mods_config: u64,
    pub theme: u64,
    pub language: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Noticed {
    pub mods: bool,
    pub saves: bool,
    pub log: bool,
    pub mods_config: bool,
    pub theme: bool,
    pub language: bool,
}

#[derive(Debug, Default)]
pub struct Watcher {
    before: Option<Stamps>,
    mods_pending: bool,
}

impl Watcher {
    pub fn look(&mut self, mut now: Stamps, game_running: bool, in_the_way: bool) -> Noticed {
        let Some(before) = self.before else {
            self.before = Some(now);
            return Noticed::default();
        };
        let mut noticed = Noticed::default();
        // Steam keeps writing its record while it downloads: wait until a look finds it quiet.
        if now.mods != before.mods {
            self.mods_pending = true;
        } else if self.mods_pending && !in_the_way {
            self.mods_pending = false;
            noticed.mods = true;
        }
        noticed.saves = now.saves != before.saves;
        noticed.theme = now.theme != before.theme;
        noticed.language = now.language != before.language;
        // The game writes its log while it runs and ModsConfig.xml as it starts:
        // both wait until it stops.
        if now.log != before.log {
            match game_running {
                true => now.log = before.log,
                false => noticed.log = true,
            }
        }
        if now.mods_config != before.mods_config && now.mods_config != 0 {
            match game_running {
                true => now.mods_config = before.mods_config,
                false => noticed.mods_config = true,
            }
        }
        self.before = Some(now);
        noticed
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn at(mods: u64, log: u64, mods_config: u64) -> Stamps {
        Stamps {
            mods,
            log,
            mods_config,
            ..Stamps::default()
        }
    }

    #[test]
    fn the_first_look_only_takes_note_and_an_unchanged_one_finds_nothing() {
        let mut watcher = Watcher::default();
        assert_eq!(watcher.look(at(1, 1, 0), false, false), Noticed::default());
        assert_eq!(watcher.look(at(1, 1, 0), false, false), Noticed::default());
    }

    #[test]
    fn changed_mods_are_read_once_quiet_for_a_look_and_nothing_is_in_the_way() {
        let mut watcher = Watcher::default();
        watcher.look(at(1, 0, 0), false, false);
        assert!(!watcher.look(at(2, 0, 0), false, false).mods);
        assert!(!watcher.look(at(3, 0, 0), false, false).mods);
        assert!(!watcher.look(at(3, 0, 0), false, true).mods);
        assert!(watcher.look(at(3, 0, 0), false, false).mods);
        assert!(!watcher.look(at(3, 0, 0), false, false).mods);
    }

    #[test]
    fn the_log_and_the_list_written_while_the_game_runs_wait_until_it_stops() {
        let mut watcher = Watcher::default();
        watcher.look(at(0, 10, 0), false, false);
        assert!(!watcher.look(at(0, 11, 5), true, false).log);
        assert!(!watcher.look(at(0, 12, 5), true, false).mods_config);
        let stopped = watcher.look(at(0, 12, 5), false, false);
        assert!(stopped.log && stopped.mods_config);
        assert_eq!(watcher.look(at(0, 12, 5), false, false), Noticed::default());
    }

    #[test]
    fn a_list_back_to_the_saved_one_is_no_change_to_offer() {
        let mut watcher = Watcher::default();
        watcher.look(at(0, 0, 5), false, false);
        assert!(!watcher.look(at(0, 0, 0), false, false).mods_config);
        assert!(watcher.look(at(0, 0, 9), false, false).mods_config);
    }

    #[test]
    fn saves_the_theme_and_the_language_are_noticed_at_once() {
        let mut watcher = Watcher::default();
        watcher.look(Stamps::default(), true, true);
        let changed = Stamps {
            saves: 1,
            theme: 2,
            language: 3,
            ..Stamps::default()
        };
        let noticed = watcher.look(changed, true, true);
        assert!(noticed.saves && noticed.theme && noticed.language);
    }

    #[test]
    fn a_missing_file_has_no_time_and_a_new_entry_changes_its_folder() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(changed_at(&tmp.path().join("none")), 0);
        let before = changed_at(tmp.path());
        assert!(before > 0);
        // File times can be coarse: the sleep makes the new entry's time later.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::create_dir(tmp.path().join("mod")).unwrap();
        let after = changed_at(tmp.path());
        assert!(after > before);
        assert_eq!(
            newest([tmp.path(), tmp.path().join("none").as_path()]),
            after
        );
        assert_eq!(newest([]), 0);
    }
}
