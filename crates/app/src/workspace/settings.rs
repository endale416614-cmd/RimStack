use super::App;
use crate::changes::Changes;
use crate::reports::{
    Database, FolderVerdict, KeptCount, LastLaunch, ReleaseSaid, SettingsReport, ShortContrast,
    Summary,
};
use rimstack_core::base::error::Error;
use rimstack_core::base::locks::Shared;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::base::translate::{LOCALE, detected};
use rimstack_core::kept::owntheme;
use rimstack_core::kept::settings::{Switch, UPDATE_EVERY};
use rimstack_core::mods::community;
use rimstack_core::rimworld::workshopdb;
use rimstack_core::sources::replacements;
use rimstack_core::{said, say, say_n};
use serde_json::{Map, Value, json};
use std::path::PathBuf;

const GLANCED: usize = 3;

/// An empty box leaves the folder to detection.
pub fn judge_folder(name: &str, typed: &str) -> FolderVerdict {
    use rimstack_core::rimworld::paths::{Folder, Os, judge};
    let kind = match name {
        "game" => Folder::Game,
        "config" => Folder::Config,
        _ => Folder::Workshop,
    };
    let typed = typed.trim();
    let judged = if typed.is_empty() {
        Ok(say!("The folder found automatically is used"))
    } else {
        judge(Os::running(), kind, std::path::Path::new(typed))
    };
    FolderVerdict {
        ok: judged.is_ok(),
        said: judged.unwrap_or_else(|said| said),
    }
}

const SHIPPED: &[(&str, &str)] = &[
    (
        "English",
        include_str!("../../assets/languages/English.json"),
    ),
    (
        "Deutsch",
        include_str!("../../assets/languages/Deutsch.json"),
    ),
    (
        "Español",
        include_str!("../../assets/languages/Español.json"),
    ),
    (
        "Français",
        include_str!("../../assets/languages/Français.json"),
    ),
    (
        "Italiano",
        include_str!("../../assets/languages/Italiano.json"),
    ),
    ("Polski", include_str!("../../assets/languages/Polski.json")),
    (
        "Português",
        include_str!("../../assets/languages/Português.json"),
    ),
    (
        "Русский",
        include_str!("../../assets/languages/Русский.json"),
    ),
    ("한국어", include_str!("../../assets/languages/한국어.json")),
];

const EVERY_TEXT: &str = include_str!("../../assets/words.json");

fn shipped(name: &str) -> Option<&'static str> {
    SHIPPED
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, words)| *words)
}

fn every(minutes: u64) -> Sentence {
    match minutes {
        1 => say!("Every minute"),
        60 => say!("Every hour"),
        n => say_n!(
            usize::try_from(n).unwrap_or(0),
            "Every {} minute",
            "Every {} minutes"
        ),
    }
}

pub fn effects(name: &str) -> Changes {
    Changes::SETTINGS
        | match name {
            "game" | "config" | "workshop" => Changes::FOLDERS,
            "language" => Changes::LANGUAGE,
            "gitForWorkshop" => Changes::UPDATES,
            "showMature" => Changes::SOURCE,
            "logsKept" => Changes::LOGS,
            "communityRules" | "fileHints" | "versionWarnings" | "versionAsideFor" => {
                Changes::PROBLEMS
            }
            "databasesOff" => Changes::COMMUNITY | Changes::PROBLEMS,
            _ => Changes::NONE,
        }
}

impl App {
    pub fn languages_dir(&self) -> PathBuf {
        self.settings_file().with_file_name("languages")
    }

    pub fn theme_file(&self) -> PathBuf {
        self.settings_file().with_file_name("theme.json")
    }

    fn rimsort_rules_file(&self) -> PathBuf {
        self.rimsort_dir().join("dbs").join("userRules.json")
    }

    pub fn keep_setting(&self, name: &str, value: &str) -> Result<(), Error> {
        self.set_setting(name, value)?;
        self.publish(effects(name));
        Ok(())
    }

    pub fn settings_report(&self) -> SettingsReport {
        let paths = self.paths();
        let shown = |p: &Option<PathBuf>| p.as_ref().map(|p| p.to_string_lossy().into_owned());
        let settings = self.settings();
        let owned = |name: &str| settings.get(name).map(String::from);
        let config = paths.mods_config();
        let backup = config
            .as_ref()
            .map(|c| c.with_file_name("ModsConfig.xml.rimstack-backup"));
        let logs = self.kept_logs_dir();
        SettingsReport {
            game: shown(&paths.game),
            no_mod_folders: paths.roots().is_empty(),
            config: shown(&paths.config),
            workshop: shown(&paths.workshop),
            game_version: paths.game_version.clone(),
            mods_config: shown(&config),
            backup: shown(&backup.filter(|b| b.exists())),
            set_game: owned("game"),
            set_config: owned("config"),
            set_workshop: owned("workshop"),
            launch_arguments: owned("launchArguments"),
            launch_directly: settings.launch_directly(),
            sandboxed: rimstack_core::base::sandbox::sandboxed(),
            language: owned("language"),
            english: self.language_in_use() == "English",
            language_in_use: self.language_in_use(),
            steamcmd: self.steamcmd().is_some(),
            interface_scale: settings.get("interfaceScale").unwrap_or("1").to_string(),
            font: owned("font"),
            community_rules: settings.switch(Switch::CommunityRules),
            version_warnings: settings.switch(Switch::VersionWarnings),
            version_aside_for: owned("versionAsideFor"),
            file_hints: settings.switch(Switch::FileHints),
            dim_left_out: settings.switch(Switch::DimLeftOut),
            git_for_workshop: settings.switch(Switch::GitForWorkshop),
            auto_updates: settings.switch(Switch::AutoUpdates),
            auto_updates_asked: settings.get(Switch::AutoUpdates.name()).is_some(),
            update_every: UPDATE_EVERY
                .iter()
                .map(|m| every(m.parse().unwrap_or(0)))
                .collect(),
            update_every_at: UPDATE_EVERY
                .iter()
                .position(|m| m.parse() == Ok(settings.update_every_minutes()))
                .unwrap_or(0),
            update_every_said: every(settings.update_every_minutes()),
            update_every_set: settings.get("updateEvery").is_some(),
            git: self.git().clone(),
            self_check: settings.switch(Switch::SelfCheck),
            show_mature: settings.switch(Switch::ShowMature),
            nightly_updates: settings.nightly_updates(),
            logs_kept: settings.logs_kept().unwrap_or(10),
            logs_kept_now: rimstack_core::rimworld::logs::kept(&logs).len(),
            replacements_ignored: settings
                .listed("replacementsIgnored")
                .map(String::from)
                .collect(),
            setup_done: settings.get("setupDone").is_some(),
            rimsort_rules: self.rimsort_rules_file().is_file(),
            official: self
                .install()
                .official()
                .iter()
                .take(GLANCED)
                .cloned()
                .collect(),
            data_folder: self.data_dir().to_string_lossy().into_owned(),
            local_mods: shown(&paths.local_mods()),
            config_folder: shown(&paths.config.as_ref().map(|c| c.join("Config"))),
            game_log: shown(&self.game_log()),
            steamcmd_folder: self
                .data_dir()
                .join("steamcmd")
                .to_string_lossy()
                .into_owned(),
            steamcmd_refused: rimstack_core::sources::redownload::refusal(),
            settings_file: self.settings_file().to_string_lossy().into_owned(),
            diagnostics_file: self.diagnostics_file().to_string_lossy().into_owned(),
            version: build_label(),
        }
    }

    pub fn summary(&self) -> Summary {
        let config = self.paths().config.clone();
        let saves = config
            .as_ref()
            .and_then(|config| std::fs::read_dir(config.join("Saves")).ok())
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("rws"))
            })
            .count();
        let kept = config
            .as_deref()
            .and_then(rimstack_core::rimworld::rimsaves::settings);
        let last = rimstack_core::kept::launches::read(&self.launches_file()).pop();
        let ignored = self.replacements_ignored();
        let replacements = match &*self.library() {
            Ok((library, _)) => {
                let known = self.replacements();
                let mods = library.workshop_mods();
                let offered = mods.keys().filter(|id| known.contains_key(**id));
                offered.filter(|id| !ignored.contains(**id)).count()
            }
            Err(_) => 0,
        };
        let folders = kept.map_or(1, |s| s.folders.len());
        let snapshots = rimstack_core::kept::lists::count(&self.snapshots_dir());
        let rules = self.user_rules().reading().len();
        let community = (*self.community()).as_ref().map_or(0, |c| c.len());
        let saved = match folders {
            0 | 1 => Sentence::Number(saves as i64),
            _ => say!("{} in {} RimSaves folders", saves, folders),
        };
        let counts = [
            (say!("Saves"), saved),
            (say!("Snapshots"), Sentence::Number(snapshots as i64)),
            (say!("Own rules"), Sentence::Number(rules as i64)),
            (
                say!("Community rules"),
                match community {
                    0 => say!("Not downloaded"),
                    n => say_n!(n, "{} mod", "{} mods"),
                },
            ),
        ];
        let noted = [
            Some(say_n!(saves, "{} save", "{} saves")),
            Some(say_n!(snapshots, "{} snapshot", "{} snapshots")),
            (community > 0).then(|| say_n!(community, "rules for {} mod", "rules for {} mods")),
        ];
        Summary {
            saves,
            folders,
            snapshots,
            replacements,
            rules,
            kept_note: Sentence::listed(noted.into_iter().flatten().collect()),
            kept: counts
                .into_iter()
                .filter(|(_, value)| !value.is_empty())
                .map(|(label, value)| KeptCount { label, value })
                .collect(),
            last_launch: last.map(|l| LastLaunch {
                time: l.time,
                errors: l.errors,
                mods: l.ids.len(),
            }),
        }
    }

    /// Shipped first, then the user's own, by name.
    pub fn languages(&self) -> Vec<String> {
        let files = std::fs::read_dir(self.languages_dir())
            .into_iter()
            .flatten();
        let mut own: Vec<String> = files
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|x| x == "json"))
            .filter_map(|path| Some(path.file_stem()?.to_string_lossy().into_owned()))
            .filter(|name| shipped(name).is_none())
            .collect();
        own.sort();
        SHIPPED
            .iter()
            .map(|(name, _)| name.to_string())
            .chain(own)
            .collect()
    }

    /// Settings-folder texts take precedence over shipped ones.
    pub fn language_table(&self, name: &str) -> Map<String, Value> {
        let file = self.languages_dir().join(format!("{name}.json"));
        let own = std::fs::read_to_string(file).unwrap_or_default();
        rimstack_core::base::translate::table(shipped(name).unwrap_or_default(), &own)
    }

    /// Else the first system language with a file, else English.
    pub fn language_in_use(&self) -> String {
        let chosen = self
            .settings()
            .get("language")
            .map(str::trim)
            .map(String::from);
        if let Some(chosen) = chosen.filter(|chosen| !chosen.is_empty()) {
            return chosen;
        }
        let dir = self.languages_dir();
        let detect = || {
            let names = self.languages();
            let tables: Vec<(&str, Map<String, Value>)> = names
                .iter()
                .map(|name| (name.as_str(), self.language_table(name)))
                .collect();
            let offered: Vec<(&str, &str)> = tables
                .iter()
                .map(|(name, table)| {
                    (
                        *name,
                        table
                            .get(LOCALE)
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    )
                })
                .collect();
            detected(&self.system_languages, &offered)
                .unwrap_or("English")
                .to_string()
        };
        let found = self
            .detected_language
            .get(rimstack_core::base::watch::changed_at(&dir), detect);
        found.as_ref().clone()
    }

    pub fn speak(&self) -> Map<String, Value> {
        self.language_table(&self.language_in_use())
    }

    pub fn start_language(&self, name: &str) -> Result<PathBuf, Error> {
        let name = name.trim();
        if name.is_empty() || name.contains(['/', '\\']) {
            return Err(Error::refused(said!(
                "A language needs a name without slashes"
            )));
        }
        let dir = self.languages_dir();
        let file = dir.join(format!("{name}.json"));
        if !file.exists() {
            let start = shipped(name).unwrap_or(EVERY_TEXT);
            std::fs::create_dir_all(&dir)
                .and_then(|()| std::fs::write(&file, start))
                .map_err(|problem| Error::file(&file, problem))?;
        }
        Ok(file)
    }

    pub fn own_colours(&self, theme: &str) -> Map<String, Value> {
        let text = std::fs::read_to_string(self.theme_file()).unwrap_or_default();
        let file = serde_json::from_str::<Value>(&text).unwrap_or(Value::Null);
        let known = self.settings().get("ownColoursFor").map(String::from);
        let flat_for = match known {
            Some(known) => known,
            None if owntheme::is_flat(&file) => {
                if let Err(problem) = self.set_setting("ownColoursFor", theme) {
                    rimstack_core::problem!("{problem}");
                }
                theme.to_string()
            }
            None => String::new(),
        };
        owntheme::colours_for(&file, theme, &flat_for)
    }

    pub fn start_own_theme(&self, theme: &str, colours: &Value) -> Result<PathBuf, Error> {
        let file = self.theme_file();
        let now = std::fs::read_to_string(&file)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok());
        let wanted = match now {
            None => Some(json!({ theme: colours })),
            Some(now) => owntheme::with_theme(&now, theme, colours),
        };
        if let Some(pretty) = wanted.and_then(|w| serde_json::to_string_pretty(&w).ok()) {
            rimstack_core::base::atomic_file::write(&file, pretty)?;
            self.publish(Changes::THEME);
        }
        Ok(file)
    }

    pub fn databases(&self) -> Vec<Database> {
        let data = self.data_dir();
        let version = self.paths().game_version.clone().unwrap_or_default();
        let all = [
            (
                "rules",
                "Community Rules Database",
                Sentence::named("Load-order rules, for Sort"),
                community::file_in(data),
            ),
            (
                "worksOn",
                "No Version Warning",
                Sentence::named("Mods that work on this game version without saying so"),
                community::works_on_file(data, &version),
            ),
            (
                "replacements",
                "Use This Instead",
                Sentence::named("Maintained replacements for abandoned mods"),
                replacements::file_in(data),
            ),
            (
                "names",
                "Steam Workshop Database",
                Sentence::named("Names of mods that are not installed"),
                workshopdb::file_in(data),
            ),
        ];
        all.into_iter()
            .map(|(id, name, used_for, file)| {
                let updated = rimstack_core::base::watch::changed_at(&file) / 1000;
                let on = self.database_on(id);
                let waiting = on && updated == 0;
                Database {
                    id,
                    name,
                    used_for: if waiting {
                        say!("Used once downloaded: {}", used_for.inside())
                    } else {
                        used_for
                    },
                    updated,
                    on,
                    waiting,
                }
            })
            .collect()
    }

    pub fn use_database(&self, id: &str, on: bool) -> Result<(), Error> {
        let off: Vec<&str> = self
            .databases()
            .iter()
            .filter(|database| match database.id == id {
                true => !on,
                false => !database.on,
            })
            .map(|database| database.id)
            .collect();
        self.keep_setting("databasesOff", &off.join(","))
    }

    /// Browse searches the index the same download writes, so it is fetched even where the names
    /// are switched off until the index exists.
    fn names_wanted(&self) -> bool {
        self.database_on("names")
            || !rimstack_core::rimworld::workshopindex::file_in(self.data_dir()).exists()
    }

    pub fn download_databases(&self) -> Result<(), Error> {
        let data = self.data_dir();
        let mut failed: Vec<Error> = Vec::new();
        if self.database_on("rules") {
            let file = community::file_in(data);
            match community::download(&file) {
                Ok(fresh) => self.community.put(super::written(file), Some(fresh)),
                Err(why) => failed.push(why),
            }
        }
        let paths = self.paths();
        let version = paths.game_version.as_deref();
        if let Some(version) = version.filter(|_| self.database_on("worksOn")) {
            let file = community::works_on_file(data, version);
            match community::download_works_on(version, &file) {
                Ok(fresh) => self.works_on.put(super::written(file), fresh),
                Err(why) => failed.push(why),
            }
        }
        if self.database_on("replacements") {
            failed.extend(replacements::download(&replacements::file_in(data)).err());
            let _ = self.replacements();
        }
        if self.names_wanted() {
            let file = workshopdb::file_in(data);
            match workshopdb::download(&file) {
                Ok(fresh) => self.workshop_names.put(super::written(file), fresh),
                Err(why) => failed.push(why),
            }
        }
        *self
            .community_problem
            .write()
            .unwrap_or_else(|p| p.into_inner()) = failed.first().map(|why| why.sentence().clone());
        self.publish(Changes::COMMUNITY | Changes::PROBLEMS);
        for why in &failed {
            rimstack_core::problem!("{why}");
        }
        failed.into_iter().next().map_or(Ok(()), Err)
    }

    /// Why the community data last failed to download, until one succeeds.
    pub fn community_problem(&self) -> Option<Sentence> {
        self.community_problem
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// 0: the game version, 1: the installed version, 2: never.
    pub fn version_warnings(&self, choice: i32) -> Result<(), Error> {
        let installed = self.paths().game_version.clone().unwrap_or_default();
        let aside_for = if choice == 1 { installed.as_str() } else { "" };
        self.keep_setting("versionWarnings", if choice == 2 { "off" } else { "on" })?;
        self.keep_setting("versionAsideFor", aside_for)
    }

    pub fn import_rimsort_rules(&self) -> Sentence {
        let path = self.rimsort_rules_file();
        let read = std::fs::read_to_string(&path).map_err(|e| Error::file(&path, e));
        let pairs = match read.and_then(|text| Ok(community::parse(&text)?.pairs())) {
            Ok(pairs) => pairs,
            Err(problem) => return problem.sentence().clone(),
        };
        let mut yours = self.user_rules().writing();
        let added = yours.import(&pairs);
        let kept = yours.clone();
        drop(yours);
        self.write_user_rules(kept);
        if pairs.is_empty() {
            return say!("RimSort has no rules of your own.");
        }
        self.publish(Changes::RULES | Changes::PROBLEMS);
        say_n!(
            pairs.len(),
            "RimSort's {} rule: {} new, now yours here.",
            "RimSort's {} rules: {} new, now yours here.",
            added
        )
    }

    pub fn set_mod_settings_aside(&self) -> Sentence {
        let paths = self.paths();
        let Some(config) = paths.config.as_ref() else {
            return say!("The game's settings folder is not known");
        };
        let aside = rimstack_core::sources::maintenance::free_folder(
            &self
                .data_dir()
                .join(format!("mod-settings-{}", super::now_secs())),
        );
        match rimstack_core::sources::maintenance::set_mod_settings_aside(config, &aside) {
            Ok(0) => say!("No mod has settings"),
            Ok(count) => say_n!(
                count,
                "{} file moved to {}",
                "{} files moved to {}",
                aside.display()
            ),
            Err(problem) => problem.sentence().clone(),
        }
    }
}

pub fn short_contrast(colours: &Map<String, Value>) -> Vec<ShortContrast> {
    owntheme::short_pairs(colours)
        .into_iter()
        .map(|(what, found, needed)| ShortContrast {
            what,
            found,
            needed,
        })
        .collect()
}

const COMMIT: &str = match option_env!("RIMSTACK_COMMIT") {
    Some(commit) => commit,
    None => "",
};

pub fn build_label() -> String {
    match COMMIT {
        "" => env!("CARGO_PKG_VERSION").to_string(),
        commit => format!("{} ({commit})", env!("CARGO_PKG_VERSION")),
    }
}

pub fn newest_release(nightly: bool) -> ReleaseSaid {
    use rimstack_core::sources::release::{Channel, Running, check, newer};
    let running = Running {
        commit: COMMIT,
        made: option_env!("RIMSTACK_COMMIT_TIME")
            .and_then(|made| made.parse().ok())
            .unwrap_or(0),
    };
    let channel = if nightly {
        Channel::Nightly
    } else {
        Channel::Stable
    };
    match check(channel, std::env::consts::OS, running) {
        Ok(Some(release)) if nightly || newer(env!("CARGO_PKG_VERSION"), &release.version) => {
            ReleaseSaid::Newer {
                version: release.version,
                page: release.page,
            }
        }
        Ok(_) => ReleaseSaid::Latest {},
        Err(problem) => ReleaseSaid::Failed {
            problem: problem.sentence().clone(),
        },
    }
}

/// The families to choose from: "" for the system's first, then each once, by name ignoring case.
pub fn font_choices(mut names: Vec<String>) -> Vec<String> {
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    names.insert(0, String::new());
    names
}

/// Where in `fonts` a name holds `wanted`, ignoring case; "" is the system's, named `system`.
pub fn fonts_matching(fonts: &[impl AsRef<str>], wanted: &str, system: &str) -> Vec<usize> {
    let wanted = wanted.to_lowercase();
    let named = |name: &str| match name.is_empty() {
        true => system.to_lowercase(),
        false => name.to_lowercase(),
    };
    (0..fonts.len())
        .filter(|&at| named(fonts[at].as_ref()).contains(&wanted))
        .collect()
}

#[cfg(test)]
mod tests;
