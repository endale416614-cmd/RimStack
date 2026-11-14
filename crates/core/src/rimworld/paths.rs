use crate::base::links;
use crate::base::sentence::Sentence;
use crate::base::vdf;
use crate::rimworld::engine::Engine;
use crate::say;
use std::path::{Path, PathBuf};

pub const APP_ID: &str = "294100";
const SETTINGS: &str = "Ludeon Studios/RimWorld by Ludeon Studios";
/// Flatpak's and Snap's Steam give the games they start these as their home.
const SANDBOXES: [&str; 2] = [".var/app/com.valvesoftware.Steam", "snap/steam/common"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    /// `<game>/Data`.
    Official,
    /// `<game>/Mods`.
    Local,
    Workshop,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModRoot {
    pub path: PathBuf,
    pub source: Source,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Paths {
    pub game: Option<PathBuf>,
    /// The folder holding `Config/` and `Saves/`.
    pub config: Option<PathBuf>,
    pub workshop: Option<PathBuf>,
    pub steam: bool,
    /// Short form, `1.6`, from `<game>/Version.txt`.
    pub game_version: Option<String>,
    /// As `<game>/Version.txt` gives it: `1.6.4871 rev598`.
    pub full_game_version: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Overrides {
    pub game: Option<PathBuf>,
    pub config: Option<PathBuf>,
    pub workshop: Option<PathBuf>,
}

impl Overrides {
    pub fn from_env() -> Self {
        let var = |name| std::env::var_os(name).map(PathBuf::from);
        Self {
            game: var("RIMSTACK_GAME"),
            config: var("RIMSTACK_CONFIG"),
            workshop: var("RIMSTACK_WORKSHOP"),
        }
    }
}

impl Paths {
    pub fn mods_config(&self) -> Option<PathBuf> {
        Some(self.config.as_ref()?.join("Config").join("ModsConfig.xml"))
    }

    pub fn local_mods(&self) -> Option<PathBuf> {
        Some(self.game.as_ref()?.join("Mods"))
    }

    /// `<library>/steamapps/workshop/appworkshop_294100.acf`, two folders above the items.
    pub fn steam_record(&self) -> Option<PathBuf> {
        let workshop = self.workshop.as_ref()?.parent()?.parent()?;
        Some(workshop.join(format!("appworkshop_{APP_ID}.acf")))
    }

    pub fn roots(&self) -> Vec<ModRoot> {
        let game = self.game.as_ref();
        [
            (game.map(|g| g.join("Data")), Source::Official),
            (self.local_mods(), Source::Local),
            (self.workshop.clone(), Source::Workshop),
        ]
        .into_iter()
        .filter_map(|(path, source)| {
            Some(ModRoot {
                path: path?,
                source,
            })
        })
        .filter(|root| root.path.is_dir())
        .fold(Vec::new(), |mut roots: Vec<ModRoot>, root| {
            // A Workshop folder typed by hand may be the game's own Mods (#6).
            let same =
                |a: &Path, b: &Path| dunce::canonicalize(a).ok() == dunce::canonicalize(b).ok();
            if !roots.iter().any(|kept| same(&kept.path, &root.path)) {
                roots.push(root);
            }
            roots
        })
    }
}

/// Steam's folder is often reached through several symlinks, so paths are canonicalised.
fn existing(candidates: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let tagged = existing_with(candidates.into_iter().map(|path| (path, ())));
    tagged.into_iter().map(|(path, ())| path).collect()
}

fn existing_with<T>(candidates: impl IntoIterator<Item = (PathBuf, T)>) -> Vec<(PathBuf, T)> {
    let mut out: Vec<(PathBuf, T)> = Vec::new();
    for (path, tag) in candidates {
        let Ok(path) = dunce::canonicalize(path) else {
            continue;
        };
        if path.is_dir() && !out.iter().any(|(kept, _)| *kept == path) {
            out.push((path, tag));
        }
    }
    out
}

fn home_seen_from(os: Os, home: &Path, root: &Path) -> PathBuf {
    let sandboxes = match os {
        Os::Linux => existing(SANDBOXES.map(|sandbox| home.join(sandbox))),
        _ => Vec::new(),
    };
    let holding = sandboxes
        .into_iter()
        .find(|sandbox| root.starts_with(sandbox));
    holding.unwrap_or_else(|| home.to_path_buf())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    Windows,
    Mac,
}

impl Os {
    pub fn running() -> Self {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Mac
        } else {
            Os::Linux
        }
    }
}

pub type Env<'a> = &'a dyn Fn(&str) -> Option<PathBuf>;

/// On Windows, Steam's install path is read from the registry under this name.
pub const STEAM_INSTALL: &str = "SteamInstallPath";

pub fn from_environment(name: &str) -> Option<PathBuf> {
    if name == STEAM_INSTALL {
        return steam_in_registry();
    }
    std::env::var_os(name).map(PathBuf::from)
}

/// Runs `reg`, which every Windows has.
fn steam_in_registry() -> Option<PathBuf> {
    if Os::running() != Os::Windows {
        return None;
    }
    let keys = [
        "HKLM\\SOFTWARE\\Wow6432Node\\Valve\\Steam",
        "HKLM\\SOFTWARE\\Valve\\Steam",
    ];
    keys.iter().find_map(|key| {
        let asked = crate::base::program::command("reg")
            .args(["query", key, "/v", "InstallPath"])
            .output()
            .ok()?;
        install_path(&String::from_utf8_lossy(&asked.stdout))
    })
}

/// The value of the `InstallPath` line that `reg query` prints: name, type, value.
fn install_path(printed: &str) -> Option<PathBuf> {
    let mut lines = printed.lines();
    let line = lines.find(|line| line.trim_start().starts_with("InstallPath"))?;
    let path = line.split("REG_SZ").nth(1)?.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

pub fn home(env: Env) -> PathBuf {
    env("HOME")
        .or_else(|| env("USERPROFILE"))
        .unwrap_or_default()
}

pub fn app_dirs(os: Os, home: &Path, env: Env) -> (PathBuf, PathBuf) {
    let set = |name: &str| env(name).filter(|path| path.is_absolute());
    match os {
        Os::Linux => (
            set("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")),
            set("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share")),
        ),
        Os::Windows => (
            set("APPDATA").unwrap_or_else(|| home.join("AppData/Roaming")),
            set("LOCALAPPDATA").unwrap_or_else(|| home.join("AppData/Local")),
        ),
        Os::Mac => {
            let support = home.join("Library/Application Support");
            (support.clone(), support)
        }
    }
}

pub fn steam_folders(home: &Path) -> Vec<PathBuf> {
    steam_roots(Os::running(), home, &from_environment)
}

fn steam_roots(os: Os, home: &Path, env: Env) -> Vec<PathBuf> {
    let places: Vec<PathBuf> = match os {
        Os::Linux => [
            ".local/share/Steam",
            ".steam/debian-installation",
            ".steam/steam",
            ".steam/root",
            ".var/app/com.valvesoftware.Steam/.local/share/Steam",
            "snap/steam/common/.local/share/Steam",
        ]
        .map(|p| home.join(p))
        .to_vec(),
        Os::Windows => {
            let programs = ["ProgramFiles(x86)", "ProgramFiles"];
            let usual = programs.iter().filter_map(|name| env(name));
            let usual = usual.map(|programs| programs.join("Steam"));
            env(STEAM_INSTALL).into_iter().chain(usual).collect()
        }
        Os::Mac => vec![home.join("Library/Application Support/Steam")],
    };
    existing(places)
}

/// Each folder may hold the game under any name.
fn other_shops(os: Os, home: &Path, env: Env) -> Vec<PathBuf> {
    let places = match os {
        Os::Linux => vec![home.join("GOG Games"), home.join("Games")],
        Os::Windows => {
            let drive = env("SystemDrive").unwrap_or_else(|| PathBuf::from("C:"));
            let on_drive = PathBuf::from(format!("{}\\GOG Games", drive.display()));
            vec![on_drive, home.join("GOG Games"), home.join("Games")]
        }
        Os::Mac => vec![PathBuf::from("/Applications"), home.join("Applications")],
    };
    existing(places)
}

/// Found by `Data/Core`, which every copy has.
fn game_in(os: Os, place: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(place).ok()?.flatten();
    let folders = entries
        .map(|entry| entry.path())
        .filter(|path| path.is_dir());
    let mut roots = folders.map(|folder| content_root(os, &folder));
    roots.find(|root| root.join("Data").join("Core").is_dir())
}

fn on_unity(game_version: Option<&str>) -> bool {
    game_version.and_then(Engine::read) == Some(Engine::Unity)
}

fn native_settings(os: Os, home: &Path, seen: &Path, game_version: Option<&str>) -> Vec<PathBuf> {
    match os {
        Os::Linux => {
            let sandboxes = SANDBOXES.map(|sandbox| home.join(sandbox));
            let homes = [seen.to_path_buf(), home.to_path_buf()].into_iter();
            let homes = homes.chain(sandboxes);
            homes
                .map(|home| home.join(".config/unity3d").join(SETTINGS))
                .collect()
        }
        Os::Windows => vec![home.join("AppData/LocalLow").join(SETTINGS)],
        Os::Mac => {
            let support = home.join("Library/Application Support");
            let waylith = [support.join("Rimworld"), support.join("RimWorld")];
            let unity = support.join("Ludeon Studios/RimWorld");
            if on_unity(game_version) {
                [unity].into_iter().chain(waylith).collect()
            } else {
                waylith.into_iter().chain([unity]).collect()
            }
        }
    }
}

/// On macOS, the `.app` bundle or the folder it lies in, whichever has `Data`.
fn content_root(os: Os, installed: &Path) -> PathBuf {
    if os != Os::Mac {
        return installed.to_path_buf();
    }
    let entries = std::fs::read_dir(installed).into_iter().flatten().flatten();
    let is_bundle = |path: &PathBuf| path.extension().is_some_and(|x| x == "app") && path.is_dir();
    let bundles = entries.map(|entry| entry.path()).filter(is_bundle);
    let mut candidates = bundles.chain([installed.to_path_buf()]);
    let with_data = candidates.find(|folder| folder.join("Data").is_dir());
    with_data.unwrap_or_else(|| installed.to_path_buf())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Folder {
    Game,
    Config,
    Workshop,
}

/// A settings folder given as its own `Saves` or `Config` is taken as the folder holding it.
fn settings_holder(place: PathBuf) -> PathBuf {
    let inner = ["Config", "Saves"];
    let named_inner = place
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| inner.iter().any(|d| d.eq_ignore_ascii_case(name)));
    let holds_inner = inner.iter().any(|d| place.join(d).is_dir());
    match place.parent() {
        Some(above) if named_inner && !holds_inner => above.to_path_buf(),
        _ => place,
    }
}

/// What a folder typed by hand would be used as: the sentence says either way.
pub fn judge(os: Os, kind: Folder, place: &Path) -> Result<Sentence, Sentence> {
    let place = match kind {
        Folder::Config => &settings_holder(links::through(place)),
        _ => &links::through(place),
    };
    if !place.is_dir() {
        return Err(say!("There is no such folder"));
    }
    match kind {
        Folder::Game => {
            let root = content_root(os, place);
            if !root.join("Data").is_dir() {
                return Err(say!(
                    "This folder has no Data folder, so it is not RimWorld"
                ));
            }
            let version = std::fs::read_to_string(root.join("Version.txt"))
                .ok()
                .and_then(|v| crate::rimworld::modsconfig::short_version(&v));
            Ok(match version {
                Some(version) => say!("RimWorld {} found", version),
                None => say!("RimWorld found"),
            })
        }
        Folder::Config => {
            if ["Config", "Saves"].iter().any(|d| place.join(d).is_dir()) {
                Ok(say!("Settings and saves found"))
            } else {
                Err(say!("This folder holds neither Config nor Saves"))
            }
        }
        Folder::Workshop => Ok(say!("Workshop folder found")),
    }
}

pub fn detect(home: &Path, overrides: &Overrides) -> Paths {
    detect_on(Os::running(), home, &from_environment, overrides)
}

pub fn detect_on(os: Os, home: &Path, env: Env, overrides: &Overrides) -> Paths {
    let roots = steam_roots(os, home, env);
    let seen_from: Vec<(&PathBuf, PathBuf)> = roots
        .iter()
        .map(|r| (r, home_seen_from(os, home, r)))
        .collect();
    let listed: Vec<(vdf::Library, &PathBuf)> = seen_from
        .iter()
        .filter_map(|(r, seen)| {
            // Steam keeps the list in both places; an install may lack one.
            let lists = ["steamapps/libraryfolders.vdf", "config/libraryfolders.vdf"];
            let text = lists
                .iter()
                .find_map(|list| std::fs::read_to_string(r.join(list)).ok())?;
            Some(vdf::libraries(&text).into_iter().map(move |l| (l, seen)))
        })
        .flatten()
        .collect();
    // Libraries that list the game come first, so a stale copy elsewhere
    // does not win.
    let (with_app, without): (Vec<_>, Vec<_>) = listed
        .iter()
        .partition(|(l, _)| l.apps.iter().any(|a| a == APP_ID));
    let libraries = existing_with(
        with_app
            .into_iter()
            .chain(without)
            .map(|(l, seen)| (l.path.clone(), *seen))
            .chain(seen_from.iter().map(|(r, seen)| ((*r).clone(), seen))),
    );

    let found_game = libraries
        .iter()
        .map(|l| (l, content_root(os, &l.0.join("steamapps/common/RimWorld"))))
        .find(|(_, game)| game.is_dir());
    let workshop_in = |(l, _): &(PathBuf, &PathBuf)| {
        Some(l.join("steamapps/workshop/content").join(APP_ID)).filter(|p| p.is_dir())
    };

    let given = |path: &Option<PathBuf>| path.as_deref().map(links::through);
    let game = given(&overrides.game)
        .or_else(|| found_game.as_ref().map(|(_, g)| g.clone()))
        .or_else(|| {
            let shops = other_shops(os, home, env);
            shops.iter().find_map(|place| game_in(os, place))
        });
    let workshop = given(&overrides.workshop).or_else(|| {
        // A game set by hand has no Workshop folder.
        if overrides.game.is_some() {
            return None;
        }
        found_game
            .as_ref()
            .and_then(|(l, _)| workshop_in(l))
            .or_else(|| libraries.iter().find_map(workshop_in))
    });
    let full_game_version = game
        .as_ref()
        .and_then(|g| std::fs::read_to_string(g.join("Version.txt")).ok())
        .and_then(|v| v.lines().next().map(|line| line.trim().to_string()))
        .filter(|v| !v.is_empty());
    let game_version = full_game_version
        .as_deref()
        .and_then(crate::rimworld::modsconfig::short_version);
    let config = given(&overrides.config).map(settings_holder).or_else(|| {
        let canonical = game.as_deref().and_then(|g| dunce::canonicalize(g).ok());
        let seen = canonical.map_or_else(
            || home.to_path_buf(),
            |game| match libraries.iter().find(|(l, _)| game.starts_with(l)) {
                Some((_, seen)) => (*seen).clone(),
                None => home_seen_from(os, home, &game),
            },
        );
        let native = native_settings(os, home, &seen, game_version.as_deref());
        // Under Proton the settings live inside the game's Windows prefix.
        let under_proton = libraries.iter().filter(|_| os == Os::Linux);
        let proton = under_proton.map(|(l, _)| {
            l.join("steamapps/compatdata")
                .join(APP_ID)
                .join("pfx/drive_c/users/steamuser/AppData/LocalLow")
                .join(SETTINGS)
        });
        let windows_build = game.as_ref().is_some_and(|g| {
            ["RimWorldWin64.exe", "RimWorld.exe"]
                .iter()
                .any(|exe| g.join(exe).is_file())
        });
        let ordered: Vec<PathBuf> = if windows_build {
            proton.chain(native).collect()
        } else {
            native.into_iter().chain(proton).collect()
        };
        ordered
            .into_iter()
            .map(|p| links::through(&p))
            .find(|p| p.is_dir())
    });

    Paths {
        game,
        config,
        workshop,
        steam: !roots.is_empty(),
        game_version,
        full_game_version,
    }
}

#[cfg(test)]
mod tests;
