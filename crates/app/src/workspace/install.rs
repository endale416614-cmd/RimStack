//! Replaced whole when the folders change: nothing read earlier outlives them.

use super::{Loaded, SteamRecord};
use rimstack_core::base::locks::Held;
use rimstack_core::mods::library;
use rimstack_core::rimworld::official::{self, Official};
use rimstack_core::rimworld::{paths::Paths, startup::Measured};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

pub struct Install {
    paths: Arc<Paths>,
    library: OnceLock<Arc<Loaded>>,
    steam_record: OnceLock<Arc<SteamRecord>>,
    startup: OnceLock<Arc<Measured>>,
    newest_save: OnceLock<Arc<Option<HashSet<String>>>>,
    official: OnceLock<Arc<Vec<Official>>>,
    /// Counted once.
    texture_counts: Mutex<BTreeMap<PathBuf, usize>>,
}

/// `cell`'s value, read now when nobody has read it: a wait that `waits` hears of.
fn once<'a, T>(cell: &'a OnceLock<T>, what: &str, read: impl FnOnce() -> T) -> &'a T {
    if cell.get().is_none() {
        rimstack_core::base::waits::slow(what);
    }
    cell.get_or_init(read)
}

impl Install {
    pub fn of(paths: Arc<Paths>) -> Self {
        Self {
            paths,
            library: OnceLock::new(),
            steam_record: OnceLock::new(),
            startup: OnceLock::new(),
            newest_save: OnceLock::new(),
            official: OnceLock::new(),
            texture_counts: Mutex::default(),
        }
    }

    /// Readers keep the reading in use meanwhile.
    pub fn read_all(self) -> Self {
        self.library();
        self.steam_record();
        self.startup();
        self.newest_save();
        self.official();
        self
    }

    pub fn paths(&self) -> &Arc<Paths> {
        &self.paths
    }

    pub fn library(&self) -> Arc<Loaded> {
        let load = || {
            let started = Instant::now();
            let loaded = library::load(&self.paths).map(|library| (library, started.elapsed()));
            if let Ok((_, took)) = &loaded {
                rimstack_core::note!("scan_ms={}", took.as_millis());
            }
            Arc::new(loaded)
        };
        once(&self.library, "the mod folders", load).clone()
    }

    pub fn library_if_read(&self) -> Option<Arc<Loaded>> {
        self.library.get().cloned()
    }

    pub fn on_disk(&self) -> Vec<String> {
        match &*self.library() {
            Ok((library, _)) => library.active.iter().map(|m| m.id.clone()).collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn steam_record(&self) -> Arc<SteamRecord> {
        let read = || {
            let file = self.paths.steam_record();
            let record = file.and_then(|file| std::fs::read_to_string(file).ok());
            let record = record.unwrap_or_default();
            use rimstack_core::rimworld::workshop;
            Arc::new(SteamRecord {
                times: workshop::installed_times(&record),
                sizes: workshop::installed_sizes(&record),
                manifests: workshop::installed_manifests(&record),
            })
        };
        once(&self.steam_record, "Steam's record of the mods", read).clone()
    }

    pub fn startup(&self) -> Arc<Measured> {
        once(&self.startup, "the game's load times", || {
            Arc::new(self.read_startup())
        })
        .clone()
    }

    fn read_startup(&self) -> Measured {
        let config = self.paths.config.as_deref();
        // Times measured before the game was last updated are another game's.
        let written = |file: PathBuf| std::fs::metadata(file).and_then(|m| m.modified()).ok();
        let measured = config.and_then(|c| written(c.join("StartupImpactData.xml")));
        let updated = self
            .paths
            .game
            .as_ref()
            .and_then(|g| written(g.join("Version.txt")));
        if matches!((measured, updated), (Some(measured), Some(updated)) if measured < updated) {
            return Measured::default();
        }
        config
            .map(rimstack_core::rimworld::startup::read)
            .unwrap_or_default()
    }

    pub fn newest_save(&self) -> Arc<Option<HashSet<String>>> {
        let read = || {
            let saves = self
                .paths
                .config
                .as_ref()
                .map(|config| rimstack_core::rimworld::saves::list(&config.join("Saves")));
            let header = saves.and_then(|saves| saves.into_iter().next()?.header.ok());
            let ids = header.map(|header| {
                let ids = header.mod_ids.iter();
                ids.map(|id| rimstack_core::rimworld::id::split_steam(id).0)
                    .collect()
            });
            Arc::new(ids)
        };
        once(&self.newest_save, "the newest save", read).clone()
    }

    pub fn official(&self) -> Arc<Vec<Official>> {
        let read = || {
            Arc::new(
                self.paths
                    .game
                    .as_deref()
                    .map(official::installed)
                    .unwrap_or_default(),
            )
        };
        once(&self.official, "the official content", read).clone()
    }

    pub fn texture_count(&self, mod_root: &Path) -> usize {
        if let Some(known) = self.texture_counts.held().get(mod_root) {
            return *known;
        }
        let counted = rimstack_core::mods::textures::pngs(mod_root).len();
        let mut counts = self.texture_counts.held();
        counts.insert(mod_root.to_path_buf(), counted);
        counted
    }

    pub fn forget_texture_counts(&self) {
        self.texture_counts.held().clear();
    }
}

#[cfg(test)]
mod tests {
    use crate::App;
    use crate::changes::Changes;
    use rimstack_core::base::locks::Held;
    use rimstack_core::rimworld::paths::Overrides;
    use std::fs;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    fn about(root: &Path, folder: &str, id: &str) {
        let dir = root.join(folder).join("About");
        fs::create_dir_all(&dir).unwrap();
        let xml =
            format!("<ModMetaData><packageId>{id}</packageId><name>{id}</name></ModMetaData>");
        fs::write(dir.join("About.xml"), xml).unwrap();
    }

    fn mods_config(dir: &Path, ids: &[&str]) {
        fs::create_dir_all(dir.join("Config")).unwrap();
        let items: String = ids.iter().map(|id| format!("<li>{id}</li>")).collect();
        let xml = format!("<ModsConfigData><activeMods>{items}</activeMods></ModsConfigData>");
        fs::write(dir.join("Config/ModsConfig.xml"), xml).unwrap();
    }

    fn two_installs() -> (tempfile::TempDir, App) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        for (game, version) in [("game15", "1.5.4104 rev435"), ("game16", "1.6.4871 rev600")] {
            about(root, &format!("{game}/Data/Core"), "Ludeon.RimWorld");
            about(root, &format!("{game}/Mods/roads"), "Ann.Roads");
            fs::write(root.join(game).join("Version.txt"), version).unwrap();
        }
        mods_config(&root.join("config15"), &["ludeon.rimworld"]);
        mods_config(&root.join("config16"), &["ludeon.rimworld", "ann.roads"]);
        let app = App::new(
            root.join("home"),
            &root.join("settings"),
            &root.join("data"),
            Overrides::default(),
        );
        let set = |name: &str, folder: &str| {
            app.set_setting(name, &root.join(folder).to_string_lossy())
                .unwrap()
        };
        set("game", "game15");
        set("config", "config15");
        (tmp, app)
    }

    fn point_at_16(tmp: &Path, app: &App) {
        for (name, folder) in [("game", "game16"), ("config", "config16")] {
            app.set_setting(name, &tmp.join(folder).to_string_lossy())
                .unwrap();
        }
    }

    #[test]
    fn when_the_folders_change_every_publish_sees_the_new_load_order() {
        let (tmp, app) = two_installs();
        let app = Arc::new(app);
        assert_eq!(app.session().current(), ["ludeon.rimworld"]);
        let seen: Arc<Mutex<Vec<Vec<String>>>> = Arc::default();
        let (heard, reader) = (seen.clone(), Arc::downgrade(&app));
        app.listen(move |changes| {
            if let Some(app) = reader
                .upgrade()
                .filter(|_| changes.contains(Changes::LIBRARY))
            {
                heard.held().push(app.session().current().to_vec());
            }
        });
        point_at_16(tmp.path(), &app);
        app.reread_folders();
        assert_eq!(app.session().current(), ["ludeon.rimworld", "ann.roads"]);
        let last = seen.held().last().cloned().unwrap_or_default();
        assert_eq!(
            last,
            ["ludeon.rimworld", "ann.roads"],
            "the last LIBRARY published saw the old load order"
        );
    }

    #[test]
    fn the_list_of_mods_known_to_work_follows_the_game_s_version() {
        let (tmp, app) = two_installs();
        let list = |ids: &[&str]| {
            let items: String = ids.iter().map(|id| format!("<li>{id}</li>")).collect();
            format!("<ModIdsToFix>{items}</ModIdsToFix>")
        };
        fs::create_dir_all(app.data_dir()).unwrap();
        let file = |version| rimstack_core::mods::community::works_on_file(app.data_dir(), version);
        fs::write(file("1.5"), list(&["old.only"])).unwrap();
        fs::write(file("1.6"), list(&["new.only"])).unwrap();
        assert_eq!(app.paths().game_version.as_deref(), Some("1.5"));
        let before = app.works_on();
        assert!(before.contains("old.only"), "control: {before:?}");

        point_at_16(tmp.path(), &app);
        app.reread_folders();
        assert_eq!(app.paths().game_version.as_deref(), Some("1.6"));
        let after = app.works_on();
        assert!(after.contains("new.only"), "still the 1.5 list: {after:?}");
    }
}
