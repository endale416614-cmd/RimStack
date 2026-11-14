use super::App;
use rimstack_core::base::error::Error;
use rimstack_core::kept::launches;
use rimstack_core::rimworld::{modsconfig, paths::Source};
use rimstack_core::said;

const APP_ID: u32 = 294_100;

const SAVE_BACKUPS: usize = 5;

/// The text as JavaScript's `encodeURIComponent` gives it: Steam reads
/// the launch arguments of a `steam://run` address that way.
fn uri_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => out.push(char::from(byte)),
            b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => {
                out.push(char::from(byte))
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub fn steam_address(arguments: &str) -> String {
    match arguments.trim() {
        "" => format!("steam://rungameid/{APP_ID}"),
        given => format!("steam://run/{APP_ID}//{}/", uri_component(given)),
    }
}

fn through_steam(sandboxed: bool, directly: bool) -> bool {
    sandboxed || !directly
}

impl App {
    pub fn game_running(&self) -> bool {
        let paths = self.paths();
        let game = paths.game.as_deref();
        game.is_some_and(|game| rimstack_core::sources::maintenance::game_running(game, &self.home))
    }

    /// Slow: copies the newest save. Never run it on the window's thread.
    pub fn back_up_for_launch(&self) {
        if let Err(problem) = self.back_up_newest_save("save-backups", SAVE_BACKUPS) {
            rimstack_core::problem!("save not backed up: {problem}");
        }
    }

    pub fn record_launch(&self) {
        let list = self.session().current().to_vec();
        if let Err(problem) = launches::record(&self.launches_file(), &list, super::now_secs()) {
            rimstack_core::problem!("{problem}");
        }
        self.mark_all_seen();
        self.record_versions();
    }

    /// Slow. A launch is recorded only once it is started; a failed start records nothing.
    pub fn launch(&self, record: bool) -> Result<Option<String>, Error> {
        if record {
            self.back_up_for_launch();
        }
        let started = self.start_game()?;
        if record {
            self.record_launch();
        }
        Ok(started)
    }

    /// Only the game is left to run. Gives its Steam address, or nothing when started here.
    pub fn start_game(&self) -> Result<Option<String>, Error> {
        let (directly, arguments) = {
            let settings = self.settings();
            let arguments = settings.get("launchArguments").unwrap_or_default();
            (settings.launch_directly(), arguments.to_string())
        };
        if through_steam(rimstack_core::base::sandbox::sandboxed(), directly) {
            return Ok(Some(steam_address(&arguments)));
        }
        let paths = self.paths();
        let game = paths
            .game
            .as_ref()
            .ok_or_else(|| Error::refused(said!("The game's folder is not known")))?;
        rimstack_core::sources::maintenance::start_game(game, &arguments)?;
        Ok(None)
    }

    pub fn mods_config_list(&self) -> Option<Vec<String>> {
        let file = self.paths().mods_config()?;
        modsconfig::read(&file).ok().map(|config| config.active)
    }

    /// `None` unless the game has since emptied `ModsConfig.xml`.
    pub fn list_reset_by_game(&self) -> Option<Vec<String>> {
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().ok()?;
        let last = launches::read(&self.launches_file()).pop()?;
        let saved = self.session().saved().to_vec();
        emptied_by_game(library, &saved, &last).then_some(last.ids)
    }
}

/// On its first start on a new version the game leaves only its own mods in `ModsConfig.xml`.
pub fn emptied_by_game(
    library: &rimstack_core::mods::library::Library,
    saved: &[String],
    last: &launches::Launch,
) -> bool {
    let official = |m: &rimstack_core::mods::list::Mod| {
        let installed = m.installed.and_then(|i| library.get(i));
        installed.is_some_and(|i| i.source == Source::Official)
    };
    last.ids.len() > saved.len() && library.arrange(saved).active.iter().all(official)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::installed;

    #[test]
    fn steam_is_given_the_arguments_as_a_browser_would_encode_them() {
        assert_eq!(steam_address("  "), "steam://rungameid/294100");
        assert_eq!(
            steam_address("-logfile /tmp/a b.txt é"),
            "steam://run/294100//-logfile%20%2Ftmp%2Fa%20b.txt%20%C3%A9/"
        );
    }

    #[test]
    fn steam_starts_the_game_in_a_flatpak_whatever_the_setting_says() {
        assert!(through_steam(false, false));
        assert!(!through_steam(false, true));
        assert!(through_steam(true, false));
        assert!(through_steam(true, true));
    }

    #[test]
    fn a_list_emptied_to_the_official_mods_offers_the_last_launch_s() {
        let both = vec!["ludeon.rimworld".to_string(), "ann.roads".to_string()];
        let (_tmp, app) = installed();
        assert_eq!(app.list_reset_by_game(), None);
        launches::record(&app.launches_file(), &both, 1).unwrap();
        assert_eq!(app.list_reset_by_game(), Some(both.clone()));
        assert_eq!(
            app.mods_config_list(),
            Some(vec!["ludeon.rimworld".to_string()])
        );

        let (tmp, app) = installed();
        std::fs::write(
            tmp.path().join("config/Config/ModsConfig.xml"),
            "<ModsConfigData><activeMods><li>ludeon.rimworld</li><li>ann.roads</li></activeMods></ModsConfigData>",
        )
        .unwrap();
        launches::record(&app.launches_file(), &both, 1).unwrap();
        assert_eq!(app.list_reset_by_game(), None);
    }

    #[test]
    fn a_direct_start_that_fails_records_no_launch_and_leaves_new_mods_unseen() {
        let (tmp, app) = installed();
        app.mark_all_seen();
        let about = tmp.path().join("game/Mods/hats/About");
        std::fs::create_dir_all(&about).unwrap();
        std::fs::write(
            about.join("About.xml"),
            "<ModMetaData><packageId>Ann.Hats</packageId><name>Hats</name></ModMetaData>",
        )
        .unwrap();
        app.rescan();
        let hats = tmp.path().join("game/Mods/hats");
        let hats = hats.to_string_lossy();
        assert!(app.seen().read().unwrap().is_new(&hats));

        app.set_setting("launch", "direct").unwrap();
        assert!(app.launch(true).is_err());
        assert!(launches::read(&app.launches_file()).is_empty());
        assert!(app.seen().read().unwrap().is_new(&hats));
    }

    #[test]
    fn a_launch_that_started_is_recorded_once() {
        let (_tmp, app) = installed();
        assert_eq!(
            app.launch(true).unwrap().as_deref(),
            Some("steam://rungameid/294100")
        );
        let record = launches::read(&app.launches_file());
        assert_eq!(record.len(), 1);
        assert_eq!(record[0].ids, app.session().current().to_vec());
        app.launch(false).unwrap();
        assert_eq!(launches::read(&app.launches_file()).len(), 1);
    }

    /// What the game watcher's look every 2 s costs: the stamps of every mod and the game's
    /// files, and whether the game runs.
    #[test]
    #[ignore = "reads this machine's own install; RIMSTACK_TIME_HOME names a scratch folder"]
    fn times_the_watchers_look_on_this_machines_install() {
        let Ok(scratch) = std::env::var("RIMSTACK_TIME_HOME") else {
            return;
        };
        let scratch = std::path::PathBuf::from(scratch);
        let home = std::path::PathBuf::from(std::env::var("HOME").unwrap());
        let app = App::new(
            home,
            &scratch.join("settings"),
            &scratch.join("data"),
            Default::default(),
        );
        let count = app
            .library()
            .as_ref()
            .as_ref()
            .map_or(0, |(l, _)| l.installed.len());
        app.stamps();
        let mut took: Vec<std::time::Duration> = (0..20)
            .map(|_| {
                let started = std::time::Instant::now();
                let _ = (app.game_running(), app.stamps());
                started.elapsed()
            })
            .collect();
        took.sort();
        eprintln!(
            "watcher_look mods={count} median={:?} worst={:?}",
            took[took.len() / 2],
            took[took.len() - 1]
        );
    }
}
