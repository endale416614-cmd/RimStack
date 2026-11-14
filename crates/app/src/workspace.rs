use rimstack_core::base::derived::Derived;
use rimstack_core::base::locks::{Held, Shared};
use rimstack_core::base::reread::Reread;
use rimstack_core::sources::browse::{Transport, Web};
use rimstack_core::{
    kept::notes::Notes, kept::settings::Settings, mods::community, mods::library,
    mods::session::Session, mods::userrules::UserRules, rimworld::paths,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, RwLock, RwLockReadGuard};
use std::time::{Duration, Instant};

use rimstack_core::base::waits::slow;

pub mod browse;
pub mod changed;
mod copies;
pub mod download_list;
pub mod downloads;
pub mod fetch;
pub mod find;
mod game;
mod install;
mod key;
pub mod lists;
pub mod load_order;
pub mod logs;
mod mod_facts;
mod notes;
mod prefs;
mod records;
pub mod saves;
pub mod search;
pub mod secret;
mod settings;
pub mod sweep;
pub mod textures;
pub mod upkeep;
mod window;
pub use key::KeyState;
pub use prefs::ModsView;
pub use settings::{
    build_label, font_choices, fonts_matching, judge_folder, newest_release, short_contrast,
};

pub static STARTED: OnceLock<Instant> = OnceLock::new();

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub struct Conflicts {
    /// Ascending.
    pub members: Vec<library::ModId>,
    /// Indexed by place in `members`.
    pub overlaps: rimstack_core::mods::conflicts::Overlaps,
    pub scanned: HashMap<library::ModId, rimstack_core::mods::infer::Scanned>,
}

pub type Loaded = Result<(library::Library, Duration), rimstack_core::base::error::Error>;

/// Steam's `appworkshop_294100.acf`, by Workshop id.
#[derive(Default)]
pub struct SteamRecord {
    pub times: HashMap<String, u64>,
    pub sizes: HashMap<String, u64>,
    pub manifests: HashMap<String, u64>,
}

type Replacements = HashMap<String, rimstack_core::sources::replacements::Replacement>;

/// Written time in milliseconds since 1970.
type Written = (PathBuf, u64);

fn written(file: PathBuf) -> Written {
    let at = rimstack_core::base::watch::changed_at(&file);
    (file, at)
}

type Write = Box<dyn FnOnce() + Send>;

/// `value`, already kept in memory, written to `file` off this thread: once for however many
/// are asked while one is written, the newest, and a failure reported.
fn write_behind<T: Send + 'static>(
    writes: &crate::jobs::Single<T>,
    file: PathBuf,
    value: T,
    write: fn(&T, &Path) -> Result<(), rimstack_core::base::error::Error>,
) {
    let name = format!("{} written", file.display());
    writes.ask(&name, value, move |value| {
        if let Err(problem) = write(&value, &file) {
            rimstack_core::problem!("{problem}");
        }
    });
}

pub struct App {
    home: PathBuf,
    settings_dir: PathBuf,
    data_dir: PathBuf,
    /// Precedence: environment, then Settings, then detection.
    given: paths::Overrides,
    /// Most wanted first, as "fr-FR".
    system_languages: Vec<String>,
    detected_language: Derived<u64, String>,
    settings: RwLock<Settings>,
    prefs: Arc<Derived<Written, rimstack_core::kept::prefs::Prefs>>,
    /// The prefs kept and not yet written, with the turn of their keeping: read in place of the
    /// file until written.
    prefs_kept: Arc<Mutex<(u64, Option<rimstack_core::kept::prefs::Prefs>)>>,
    prefs_writes: crate::jobs::Single<(u64, rimstack_core::kept::prefs::Prefs)>,
    settings_writes: crate::jobs::Single<Settings>,
    notes_writes: crate::jobs::Single<Notes>,
    rules_writes: crate::jobs::Single<UserRules>,
    activated_writes: crate::jobs::Single<rimstack_core::kept::activated::Since>,
    /// What `activated.json` was last made to hold, read in its place while it is written.
    activated_kept: Mutex<Option<rimstack_core::kept::activated::Since>>,
    install: Reread<install::Install>,
    session: OnceLock<Mutex<Session>>,
    notes: OnceLock<RwLock<Notes>>,
    user_rules: OnceLock<RwLock<UserRules>>,
    seen: OnceLock<RwLock<rimstack_core::kept::new_mods::Seen>>,
    community: Derived<Written, Option<community::Community>>,
    works_on: Derived<Written, HashSet<String>>,
    replacements: Derived<Written, Replacements>,
    workshop_names: Derived<Written, rimstack_core::rimworld::workshopdb::Names>,
    workshop_index: Derived<(Written, Written), rimstack_core::rimworld::workshopindex::Index>,
    sweeper: sweep::Sweeper,
    browse_open: AtomicBool,
    fixture: bool,
    vault: Arc<dyn secret::Vault>,
    transport: Arc<dyn Transport>,
    watch: Arc<key::Watch>,
    keyed: Mutex<key::Chosen>,
    fixture_source: OnceLock<Arc<rimstack_core::sources::browse::Fixture>>,
    previews_trimmed: std::sync::Once,
    browse_details: Mutex<rimstack_core::sources::browse::Memory>,
    download_list: OnceLock<Mutex<rimstack_core::kept::download_list::DownloadList>>,
    texture_manifest: Derived<Written, rimstack_core::mods::textures::Manifest>,
    steamcmd: Reread<Option<PathBuf>>,
    conflicts: RwLock<Option<Conflicts>>,
    /// Changes only while `conflicts` is held for writing.
    conflicts_generation: AtomicU64,
    facts: RwLock<mod_facts::Read>,
    facts_runs: AtomicU64,
    facts_counted: std::sync::atomic::AtomicUsize,
    behind: RwLock<Option<HashSet<String>>>,
    /// Why the community data last failed to download; `None` once one succeeds.
    community_problem: RwLock<Option<rimstack_core::base::sentence::Sentence>>,
    list_on_disk: Mutex<(u64, bool)>,
    /// Read once: git installed later is not seen.
    git: OnceLock<Option<String>>,
    log_read: Mutex<Option<rimstack_core::kept::launches::LogRead>>,
    find: find::Find,
    rescans: crate::jobs::Single<()>,
    log_keeping: crate::jobs::Single<()>,
    /// Snapshots named and not yet written.
    snapshots_named: Arc<Mutex<HashSet<String>>>,
    /// The history's writes, snapshots and journal, to be done in the order asked.
    records_waiting: Arc<Mutex<std::collections::VecDeque<Write>>>,
    records_writes: crate::jobs::Single<()>,
    bus: Arc<crate::changes::Bus>,
}

impl App {
    pub fn from_environment() -> Self {
        let env = &paths::from_environment;
        let home = paths::home(env);
        let (settings, data) = paths::app_dirs(paths::Os::running(), &home, env);
        let made_up = std::env::var("RIMSTACK_FIXTURE").is_ok_and(|v| !v.is_empty() && v != "0");
        let checking = std::env::args().any(|a| a.starts_with("--shot") || a == "--demo");
        let app = Self::new(home, &settings, &data, paths::Overrides::from_env())
            .speaking(sys_locale::get_locales().collect())
            .browsing_fixture(made_up);
        match checking {
            true => app,
            false => app.with_vault(secret::system()),
        }
    }

    pub fn speaking(self, system_languages: Vec<String>) -> Self {
        Self {
            system_languages,
            ..self
        }
    }

    pub fn new(
        home: PathBuf,
        settings_root: &Path,
        data_root: &Path,
        given: paths::Overrides,
    ) -> Self {
        let settings_dir = settings_root.join("RimStack");
        let web: Arc<dyn Transport> = Arc::new(Web::default());
        Self {
            settings: RwLock::new(Settings::read(&settings_dir.join("settings.json"))),
            prefs: Arc::default(),
            prefs_kept: Arc::default(),
            prefs_writes: crate::jobs::Single::default(),
            settings_writes: crate::jobs::Single::default(),
            notes_writes: crate::jobs::Single::default(),
            rules_writes: crate::jobs::Single::default(),
            activated_writes: crate::jobs::Single::default(),
            activated_kept: Mutex::default(),
            home,
            settings_dir,
            data_dir: data_root.join("RimStack"),
            given,
            system_languages: Vec::new(),
            detected_language: Derived::default(),
            install: Reread::new(),
            session: OnceLock::new(),
            notes: OnceLock::new(),
            user_rules: OnceLock::new(),
            seen: OnceLock::new(),
            community: Derived::default(),
            works_on: Derived::default(),
            replacements: Derived::default(),
            workshop_names: Derived::default(),
            workshop_index: Derived::default(),
            sweeper: sweep::Sweeper::default(),
            browse_open: AtomicBool::new(false),
            fixture: false,
            vault: Arc::new(secret::Memory::default()),
            transport: web.clone(),
            watch: Arc::new(key::Watch::new(web)),
            keyed: Mutex::default(),
            fixture_source: OnceLock::new(),
            previews_trimmed: std::sync::Once::new(),
            browse_details: Mutex::new(Default::default()),
            download_list: OnceLock::new(),
            texture_manifest: Derived::default(),
            steamcmd: Reread::new(),
            conflicts: RwLock::new(None),
            conflicts_generation: AtomicU64::new(0),
            facts: RwLock::default(),
            facts_runs: AtomicU64::new(0),
            facts_counted: Default::default(),
            behind: RwLock::new(None),
            community_problem: RwLock::new(None),
            list_on_disk: Mutex::new((0, false)),
            git: OnceLock::new(),
            log_read: Mutex::new(None),
            find: find::Find::default(),
            rescans: crate::jobs::Single::default(),
            log_keeping: crate::jobs::Single::default(),
            snapshots_named: Arc::default(),
            records_waiting: Arc::default(),
            records_writes: crate::jobs::Single::default(),
            bus: Arc::default(),
        }
    }

    pub fn publish(&self, changes: crate::changes::Changes) {
        self.bus.publish(changes);
    }

    pub fn listen(&self, listener: impl Fn(crate::changes::Changes) + Send + Sync + 'static) {
        self.bus.listen(listener);
    }

    #[cfg(test)]
    pub(crate) fn listening(&self) -> usize {
        self.bus.listening()
    }

    pub fn listen_while(
        &self,
        listener: impl Fn(crate::changes::Changes) -> bool + Send + Sync + 'static,
    ) {
        self.bus.listen_while(listener);
    }

    /// Beside RimStack's own folder; inside a Flatpak that one is the sandbox's, and
    /// RimSort's is where it keeps it on the host.
    pub(crate) fn rimsort_dir(&self) -> PathBuf {
        match rimstack_core::base::sandbox::sandboxed() {
            true => self.home.join(".local/share/RimSort"),
            false => self.data_dir.with_file_name("RimSort"),
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// `$XDG_CONFIG_HOME/RimStack` on Linux; the system's place elsewhere.
    pub fn settings_file(&self) -> PathBuf {
        self.settings_dir.join("settings.json")
    }

    pub fn diagnostics_file(&self) -> PathBuf {
        self.data_dir.join("diagnostics.log")
    }

    pub fn texture_manifest_file(&self) -> PathBuf {
        self.data_dir.join("textures.json")
    }

    pub fn activated_file(&self) -> PathBuf {
        self.data_dir.join("activated.json")
    }

    pub fn launches_file(&self) -> PathBuf {
        self.data_dir.join("launches.json")
    }

    pub fn kept_logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    pub fn git(&self) -> &Option<String> {
        self.git.get_or_init(rimstack_core::sources::git::installed)
    }

    pub fn notes_file(&self) -> PathBuf {
        self.data_dir.join("notes.json")
    }

    pub fn user_rules_file(&self) -> PathBuf {
        self.data_dir.join("rules.json")
    }

    pub fn seen_file(&self) -> PathBuf {
        self.data_dir.join("seen.txt")
    }

    pub fn settings(&self) -> RwLockReadGuard<'_, Settings> {
        self.settings.reading()
    }

    /// Kept at once; the file is written off this thread, and a failure to write it reported.
    pub fn set_setting(
        &self,
        name: &str,
        value: &str,
    ) -> Result<(), rimstack_core::base::error::Error> {
        let kept = {
            let mut settings = self.settings.writing();
            settings.set(name, value)?;
            settings.clone()
        };
        write_behind(
            &self.settings_writes,
            self.settings_file(),
            kept,
            Settings::write,
        );
        Ok(())
    }

    /// Once every file kept from memory is written: before the program ends.
    pub fn settle_writes(&self) {
        while !(self.settings_writes.idle()
            && self.prefs_writes.idle()
            && self.notes_writes.idle()
            && self.rules_writes.idle()
            && self.activated_writes.idle()
            && self.records_writes.idle())
        {
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    pub(crate) fn write_notes(&self, notes: Notes) {
        write_behind(&self.notes_writes, self.notes_file(), notes, Notes::write);
    }

    pub(crate) fn write_user_rules(&self, rules: UserRules) {
        write_behind(
            &self.rules_writes,
            self.user_rules_file(),
            rules,
            UserRules::write,
        );
    }

    /// Ids: "rules", "worksOn", "replacements", "names"; on unless `databasesOff` lists them.
    pub fn database_on(&self, id: &str) -> bool {
        !self.settings().listed("databasesOff").any(|off| off == id)
    }

    pub fn replacements_ignored(&self) -> HashSet<String> {
        let settings = self.settings();
        let ids = settings.listed("replacementsIgnored");
        ids.map(String::from).collect()
    }

    fn find_paths(&self) -> paths::Paths {
        let mut given = self.given.clone();
        let settings = self.settings();
        let set = |name: &str| settings.get(name).map(PathBuf::from);
        given.game = given.game.or(set("game"));
        given.config = given.config.or(set("config"));
        given.workshop = given.workshop.or(set("workshop"));
        paths::detect(&self.home, &given)
    }

    fn install(&self) -> Arc<install::Install> {
        self.install
            .get(|| install::Install::of(Arc::new(self.find_paths())))
    }

    pub fn paths(&self) -> Arc<paths::Paths> {
        self.install().paths().clone()
    }

    /// Empty without Steam.
    pub fn steam_record(&self) -> Arc<SteamRecord> {
        self.install().steam_record()
    }

    /// `None` until Downloads has asked Steam.
    pub fn behind(&self) -> &RwLock<Option<HashSet<String>>> {
        &self.behind
    }

    pub fn notes(&self) -> &RwLock<Notes> {
        self.notes
            .get_or_init(|| RwLock::new(Notes::read(&self.notes_file())))
    }

    pub fn user_rules(&self) -> &RwLock<UserRules> {
        self.user_rules
            .get_or_init(|| RwLock::new(UserRules::read(&self.user_rules_file())))
    }

    fn facts(&self) -> &RwLock<mod_facts::Read> {
        &self.facts
    }

    fn facts_runs(&self) -> &AtomicU64 {
        &self.facts_runs
    }

    fn facts_counted(&self) -> &std::sync::atomic::AtomicUsize {
        &self.facts_counted
    }

    pub fn conflicts(&self) -> &RwLock<Option<Conflicts>> {
        &self.conflicts
    }

    /// Taken before the library is read, so a scan of an earlier library is told apart.
    pub fn conflicts_generation(&self) -> u64 {
        self.conflicts_generation.load(Ordering::Relaxed)
    }

    /// `None` until the mods are first read, rather than waiting for them: for what is drawn.
    pub fn session_if_read(&self) -> Option<MutexGuard<'_, Session>> {
        self.session.get().map(|session| session.held())
    }

    pub fn session(&self) -> MutexGuard<'_, Session> {
        self.session
            .get_or_init(|| Mutex::new(Session::new(self.install().on_disk())))
            .held()
    }

    pub fn active_now(&self, library: &library::Library) -> HashSet<library::ModId> {
        let list = self.session().current().to_vec();
        library.active_ids(&list)
    }

    pub fn library(&self) -> Arc<Loaded> {
        self.install().library()
    }

    /// The mods as read, `None` while the first reading runs, never waiting on it.
    pub fn library_if_read(&self) -> Option<Arc<Loaded>> {
        self.install.now()?.library_if_read()
    }

    pub fn texture_count(&self, mod_root: &Path) -> usize {
        self.install().texture_count(mod_root)
    }

    /// `rescan` off the caller's thread; asked again while one runs, it reads once more after.
    pub fn rescan_soon(self: &Arc<Self>) {
        let app = self.clone();
        self.rescans.ask("rescan", (), move |()| app.rescan());
    }

    /// Whoever still shows the earlier reading holds it until done.
    pub fn rescan(&self) {
        let fresh = install::Install::of(self.paths()).read_all();
        self.install.put(fresh);
        self.textures_changed();
        self.publish(crate::changes::Changes::LIBRARY);
    }

    pub fn textures_changed(&self) {
        let mut slot = self.conflicts.writing();
        self.conflicts_generation.fetch_add(1, Ordering::Relaxed);
        *slot = None;
        drop(slot);
        self.install().forget_texture_counts();
        self.publish(crate::changes::Changes::TEXTURES);
    }

    /// Everything is in place before anyone is told.
    pub fn reread_folders(&self) {
        let fresh = install::Install::of(Arc::new(self.find_paths())).read_all();
        let on_disk = fresh.on_disk();
        self.install.put(fresh);
        *self.session() = Session::new(on_disk);
        *self.behind.writing() = None;
        self.forget_list_on_disk();
        *self.log_read.held() = None;
        self.textures_changed();
        self.publish(crate::changes::Changes::LIBRARY);
    }

    pub fn forget_list_on_disk(&self) {
        *self.list_on_disk.held() = (0, false);
    }

    pub fn write_list(
        &self,
        file: &Path,
        ids: &[String],
        expected: &[String],
    ) -> rimstack_core::base::error::Result<()> {
        let mut last = self.list_on_disk.held();
        rimstack_core::rimworld::modsconfig::write(file, ids, expected)?;
        *last = (rimstack_core::base::watch::changed_at(file), false);
        Ok(())
    }

    /// `0` while it holds that one.
    fn list_changed_outside(&self) -> u64 {
        let Some(file) = self.paths().mods_config() else {
            return 0;
        };
        let written = rimstack_core::base::watch::changed_at(&file);
        let mut last = self.list_on_disk.held();
        if last.0 != written {
            let on_disk =
                rimstack_core::rimworld::modsconfig::read(&file).map(|config| config.active);
            let saved = self.session().saved().to_vec();
            let differs = on_disk.is_ok_and(|on_disk| {
                !rimstack_core::rimworld::modsconfig::same_order(&on_disk, &saved)
            });
            *last = (written, differs);
        }
        if last.1 { written } else { 0 }
    }

    /// Up to 1.6 in the settings folder; from 1.7 in `Logs` there.
    fn game_logs(&self) -> Vec<PathBuf> {
        let paths = self.paths();
        let Some(config) = paths.config.as_ref() else {
            return Vec::new();
        };
        vec![
            config.join("Player.log"),
            config.join("Logs").join("Player.log"),
        ]
    }

    pub fn log_written(&self) -> u64 {
        let logs = self.game_logs();
        rimstack_core::base::watch::newest(logs.iter().map(PathBuf::as_path))
    }

    pub fn game_log(&self) -> Option<PathBuf> {
        let logs = self.game_logs().into_iter();
        logs.filter(|log| log.is_file())
            .max_by_key(|log| rimstack_core::base::watch::changed_at(log))
    }

    pub fn stamps(&self) -> rimstack_core::base::watch::Stamps {
        use rimstack_core::base::watch::{changed_at, newest};
        let paths = self.paths();
        let (record, local) = (paths.steam_record(), paths.local_mods());
        let mods = [paths.workshop.clone(), record, local];
        // A checkout pulled outside RimStack moves HEAD; a mod changes in its folder or About.xml.
        let inside: Vec<PathBuf> = match &*self.library() {
            Ok((library, _)) => {
                let git = library.installed.iter().filter(|m| m.git);
                let heads = git.map(|m| m.path.join(".git").join("logs").join("HEAD"));
                let abouts = library
                    .installed
                    .iter()
                    .flat_map(|m| [m.path.clone(), m.path.join("About").join("About.xml")]);
                heads.chain(abouts).collect()
            }
            Err(_) => Vec::new(),
        };
        let saves = paths.config.as_ref().map(|c| c.join("Saves"));
        let language = Some(
            self.languages_dir()
                .join(format!("{}.json", self.language_in_use())),
        );
        let one = |path: Option<PathBuf>| path.map_or(0, |p| changed_at(&p));
        rimstack_core::base::watch::Stamps {
            mods: newest(mods.iter().flatten().chain(&inside).map(PathBuf::as_path)),
            saves: one(saves),
            log: self.log_written(),
            mods_config: self.list_changed_outside(),
            theme: one(Some(self.settings_file().with_file_name("theme.json"))),
            language: one(language),
        }
    }

    /// `None` if never downloaded.
    pub fn community(&self) -> Arc<Option<community::Community>> {
        let file = community::file_in(&self.data_dir);
        self.community.get(written(file.clone()), || {
            slow("the community rules");
            community::read(&file).unwrap_or_else(|problem| {
                rimstack_core::problem!("{problem}");
                None
            })
        })
    }

    fn works_on_file(&self) -> PathBuf {
        let version = self.paths().game_version.clone().unwrap_or_default();
        community::works_on_file(&self.data_dir, &version)
    }

    /// Empty if never downloaded.
    pub fn works_on(&self) -> Arc<HashSet<String>> {
        let file = self.works_on_file();
        self.works_on.get(written(file.clone()), || {
            slow("the mods known to work");
            community::read_works_on(&file)
        })
    }

    pub fn replacements(&self) -> Arc<Replacements> {
        let own = rimstack_core::sources::replacements::file_in(&self.data_dir);
        let rimsort = self
            .rimsort_dir()
            .join("dbs/UseThisInstead/replacements.json.gz");
        let file = if own.is_file() { own } else { rimsort };
        self.replacements.get(written(file.clone()), || {
            slow("the replacements");
            rimstack_core::sources::replacements::read(&file)
        })
    }

    pub fn workshop_names(&self) -> Arc<rimstack_core::rimworld::workshopdb::Names> {
        let file = rimstack_core::rimworld::workshopdb::file_in(&self.data_dir);
        self.workshop_names.get(written(file.clone()), || {
            slow("the Workshop names");
            rimstack_core::rimworld::workshopdb::read(&file)
        })
    }

    /// Re-read only once written.
    pub fn texture_manifest(&self) -> Arc<rimstack_core::mods::textures::Manifest> {
        let file = self.texture_manifest_file();
        self.texture_manifest.get(written(file.clone()), || {
            slow("the textures made");
            rimstack_core::mods::textures::Manifest::read(&file)
        })
    }

    pub fn steamcmd(&self) -> Arc<Option<PathBuf>> {
        self.steamcmd
            .get(|| rimstack_core::sources::redownload::steamcmd(&self.data_dir))
    }

    pub fn look_for_steamcmd(&self) {
        self.steamcmd
            .put(rimstack_core::sources::redownload::steamcmd(&self.data_dir));
    }

    pub fn startup(&self) -> Arc<rimstack_core::rimworld::startup::Measured> {
        self.install().startup()
    }

    /// `None` without a save.
    pub fn newest_save(&self) -> Arc<Option<HashSet<String>>> {
        self.install().newest_save()
    }

    fn all_seen(&self) -> rimstack_core::kept::new_mods::Seen {
        let loaded = self.library();
        let installed = loaded.as_ref().as_ref().ok();
        let installed = installed.map(|(library, _)| library.installed.as_slice());
        let folders = installed.unwrap_or_default().iter();
        let now = rimstack_core::kept::new_mods::Seen::of(
            folders.map(|m| m.path.to_string_lossy().into_owned()),
        );
        if let Err(problem) = now.write(&self.seen_file()) {
            rimstack_core::problem!("{problem}");
        }
        now
    }

    pub fn seen(&self) -> &RwLock<rimstack_core::kept::new_mods::Seen> {
        self.seen.get_or_init(|| {
            let kept = rimstack_core::kept::new_mods::Seen::read(&self.seen_file());
            RwLock::new(if kept.is_recorded() {
                kept
            } else {
                self.all_seen()
            })
        })
    }

    pub fn mark_all_seen(&self) {
        let now = self.all_seen();
        *self.seen().writing() = now;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::fs;

    pub(crate) fn installed() -> (tempfile::TempDir, App) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let add = |folder: &str, id: &str| {
            let about = root.join(folder).join("About");
            fs::create_dir_all(&about).unwrap();
            let xml =
                format!("<ModMetaData><packageId>{id}</packageId><name>{id}</name></ModMetaData>");
            fs::write(about.join("About.xml"), xml).unwrap();
        };
        add("game/Data/Core", "Ludeon.RimWorld");
        add("game/Mods/roads", "Ann.Roads");
        fs::write(root.join("game/Version.txt"), "1.6.4871 rev600").unwrap();
        fs::create_dir_all(root.join("config/Config")).unwrap();
        fs::write(
            root.join("config/Config/ModsConfig.xml"),
            "<ModsConfigData><activeMods><li>ludeon.rimworld</li></activeMods></ModsConfigData>",
        )
        .unwrap();
        let given = paths::Overrides {
            game: Some(root.join("game")),
            config: Some(root.join("config")),
            workshop: Some(root.join("workshop")),
        };
        let app = App::new(
            root.join("home"),
            &root.join("settings"),
            &root.join("data"),
            given,
        );
        (tmp, app)
    }

    #[test]
    fn a_setting_is_kept_on_disk_and_read_by_whoever_asks_next() {
        let (_tmp, app) = installed();
        assert!(app.database_on("rules"));
        app.set_setting("databasesOff", "rules,names").unwrap();
        assert!(!app.database_on("rules") && app.database_on("worksOn"));
        app.settle_writes();
        let kept = Settings::read(&app.settings_file());
        assert_eq!(kept.get("databasesOff"), Some("rules,names"));
    }

    #[test]
    fn the_mods_are_read_once_and_again_only_when_asked() {
        let (tmp, app) = installed();
        let first = app.library();
        let (library, _) = first.as_ref().as_ref().unwrap();
        assert_eq!((library.installed.len(), library.active.len()), (2, 1));
        assert_eq!(app.session().current(), ["ludeon.rimworld"]);
        assert_eq!(app.active_now(library).len(), 1);

        let about = tmp.path().join("game/Mods/hats/About");
        fs::create_dir_all(&about).unwrap();
        fs::write(
            about.join("About.xml"),
            "<ModMetaData><packageId>Ann.Hats</packageId></ModMetaData>",
        )
        .unwrap();
        assert!(Arc::ptr_eq(&first, &app.library()));
        app.rescan();
        let again = app.library();
        assert_eq!(again.as_ref().as_ref().unwrap().0.installed.len(), 3);
    }

    #[test]
    fn a_list_written_outside_is_noticed_and_one_of_the_same_order_is_not() {
        let (tmp, app) = installed();
        let file = tmp.path().join("config/Config/ModsConfig.xml");
        assert_eq!(app.stamps().mods_config, 0);
        let write = |ids: &str, at: u64| {
            fs::write(
                &file,
                format!("<ModsConfigData><activeMods>{ids}</activeMods></ModsConfigData>"),
            )
            .unwrap();
            let when = std::time::UNIX_EPOCH + Duration::from_secs(at);
            fs::File::options()
                .write(true)
                .open(&file)
                .unwrap()
                .set_modified(when)
                .unwrap();
        };
        // As the game writes it: the same mods, another case.
        write("<li>Ludeon.RimWorld</li>", 2_000_000_000);
        assert_eq!(app.stamps().mods_config, 0);
        write("<li>ludeon.rimworld</li><li>ann.roads</li>", 2_000_000_100);
        assert_ne!(app.stamps().mods_config, 0);
    }

    #[test]
    fn a_game_folder_given_by_the_environment_wins_over_one_set_in_settings() {
        let (tmp, app) = installed();
        let other = tmp.path().join("other-game");
        fs::create_dir_all(other.join("Data")).unwrap();
        app.set_setting("game", &other.to_string_lossy()).unwrap();
        app.reread_folders();
        assert_eq!(app.paths().game, Some(tmp.path().join("game")));
    }

    #[cfg(unix)]
    #[test]
    fn a_replacements_list_written_now_is_the_one_shown() {
        let (_tmp, app) = installed();
        assert!(app.replacements().is_empty());

        let file = rimstack_core::sources::replacements::file_in(app.data_dir());
        std::fs::create_dir_all(app.data_dir()).unwrap();
        let json = r#"{"rules":[{"oldWorkshopId":"111","newWorkshopId":"222","newName":"Roads Continued"}]}"#;
        let packed = std::process::Command::new("gzip")
            .arg("-c")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut gzip| {
                use std::io::Write;
                gzip.stdin.take().unwrap().write_all(json.as_bytes())?;
                gzip.wait_with_output()
            })
            .unwrap();
        std::fs::write(&file, packed.stdout).unwrap();
        assert!(
            rimstack_core::sources::replacements::read(&file).contains_key("111"),
            "control"
        );

        assert!(
            app.replacements().contains_key("111"),
            "a list downloaded now is shown at once"
        );
    }
}
