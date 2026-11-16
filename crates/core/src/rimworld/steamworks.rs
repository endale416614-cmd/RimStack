//! Steam shows the game as running while its library is loaded, so `act` runs
//! in a short-lived process of its own (`main` calls it for `--steamworks=`).

use crate::base::error::Error;
use crate::base::sentence::Sentence;
use crate::{said, say};

use std::ffi::{CStr, c_int, c_void};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::rimworld::paths::APP_ID as GAME;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Subscribe,
    Unsubscribe,
    Download,
}

const KNOWN: [&str; 9] = [
    "libsteam_api.so",
    "steam_api64.dll",
    "libsteam_api.dylib",
    "RimWorldLinux_Data/Plugins/libsteam_api.so",
    "RimWorldLinux_Data/Plugins/x86_64/libsteam_api.so",
    "RimWorldWin64_Data/Plugins/steam_api64.dll",
    "RimWorldWin64_Data/Plugins/x86_64/steam_api64.dll",
    "Contents/Plugins/steam_api.bundle/Contents/MacOS/libsteam_api.dylib",
    "RimWorldMac.app/Contents/Plugins/steam_api.bundle/Contents/MacOS/libsteam_api.dylib",
];

pub fn library(game_dir: &Path) -> Option<PathBuf> {
    if let Some(known) = KNOWN
        .iter()
        .map(|at| game_dir.join(at))
        .find(|at| at.is_file())
    {
        return Some(known);
    }
    const NAMES: [&str; 3] = ["libsteam_api.so", "steam_api64.dll", "libsteam_api.dylib"];
    // Deep enough for the plugin folder inside a Mac application bundle.
    const DEEPEST: usize = 4;
    let named = |entry: &crate::base::walk::Entry| {
        let name = entry.path.file_name();
        !entry.is_dir && name.is_some_and(|n| NAMES.iter().any(|name| n == *name))
    };
    let found = crate::base::walk::tree(game_dir, DEEPEST)
        .into_iter()
        .filter(named);
    found
        .min_by_key(|entry| entry.within.components().count())
        .map(|entry| entry.path)
}

pub fn parse(request: &str) -> Option<(Action, Vec<u64>)> {
    let (what, ids) = request.split_once(':')?;
    let action = match what {
        "subscribe" => Action::Subscribe,
        "unsubscribe" => Action::Unsubscribe,
        "download" => Action::Download,
        _ => return None,
    };
    let ids: Option<Vec<u64>> = ids.split(',').map(|id| id.trim().parse().ok()).collect();
    ids.filter(|ids| !ids.is_empty()).map(|ids| (action, ids))
}

/// Versioned name, `{prefix}0NN`; the newest found wins.
fn interface(
    steam: &libloading::Library,
    prefix: &str,
) -> Option<unsafe extern "C" fn() -> *mut c_void> {
    (10..=40).rev().find_map(|version| {
        let name = format!("{prefix}0{version}\0");
        // SAFETY: the symbol, when present, is the flat API's accessor,
        // which takes nothing and returns the interface.
        unsafe { steam.get(name.as_bytes()).ok().map(|found| *found) }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Start {
    Plain,
    Flat,
    Safe,
}

impl Start {
    // SDK 1.59 on made SteamAPI_Init an inline wrapper; the library exports
    // SteamAPI_InitFlat and SteamAPI_InitSafe instead.
    const ORDER: [Start; 3] = [Start::Plain, Start::Flat, Start::Safe];

    fn symbol(self) -> &'static [u8] {
        match self {
            Start::Plain => b"SteamAPI_Init\0",
            Start::Flat => b"SteamAPI_InitFlat\0",
            Start::Safe => b"SteamAPI_InitSafe\0",
        }
    }
}

fn start_with(has: impl Fn(&[u8]) -> bool) -> Option<Start> {
    Start::ORDER.into_iter().find(|start| has(start.symbol()))
}

fn start_of(steam: &libloading::Library) -> Option<Start> {
    // SAFETY: looked up, never called.
    start_with(|name| unsafe { steam.get::<unsafe extern "C" fn()>(name).is_ok() })
}

fn workshop_interface(
    steam: &libloading::Library,
) -> Option<unsafe extern "C" fn() -> *mut c_void> {
    interface(steam, "SteamAPI_SteamUGC_v")
}

/// Set before any thread starts: changing the environment while another
/// thread reads it is unsound.
pub fn environment() -> [(&'static str, String); 2] {
    [
        ("SteamAppId", GAME.to_string()),
        ("SteamGameId", GAME.to_string()),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Done,
    Refused(i32),
    Unconfirmed,
}

// RemoteStorage(Un)subscribePublishedFileResult_t. Steam packs its callback structs to 4
// bytes on Linux and macOS, to 8 on Windows, and GetAPICallResult refuses any other size.
#[cfg_attr(not(windows), repr(C, packed(4)))]
#[cfg_attr(windows, repr(C))]
#[derive(Default, Clone, Copy)]
struct Answer {
    result: i32,
    id: u64,
}

const SUBSCRIBE_ANSWER: i32 = 1313;
const UNSUBSCRIBE_ANSWER: i32 = 1315;
const RESULT_OK: i32 = 1;
const STATE_SUBSCRIBED: u32 = 1;
const LONGEST: Duration = Duration::from_secs(10);

fn answered(result: i32) -> Outcome {
    match result {
        RESULT_OK => Outcome::Done,
        refused => Outcome::Refused(refused),
    }
}

fn seen(action: Action, state: u32) -> Outcome {
    let subscribed = state & STATE_SUBSCRIBED != 0;
    match (action, subscribed) {
        (Action::Subscribe, true) | (Action::Unsubscribe, false) => Outcome::Done,
        _ => Outcome::Unconfirmed,
    }
}

fn result_name(result: i32) -> Option<Sentence> {
    Some(match result {
        2 => say!("failure"),
        9 => say!("file not found"),
        15 => say!("access denied"),
        16 => say!("timed out"),
        20 => say!("service unavailable"),
        25 => say!("limit exceeded"),
        _ => return None,
    })
}

fn reported(outcomes: &[(u64, Outcome)]) -> Result<(), Error> {
    let failed: Vec<Sentence> = outcomes
        .iter()
        .filter_map(|(id, outcome)| match outcome {
            Outcome::Done => None,
            Outcome::Refused(result) => Some(match result_name(*result) {
                Some(name) => say!("{}: Steam refused it, {} ({})", id, name.inside(), result),
                None => say!("{}: Steam refused it ({})", id, result),
            }),
            Outcome::Unconfirmed => Some(say!("{}: Steam did not confirm it", id)),
        })
        .collect();
    match failed.is_empty() {
        true => Ok(()),
        false => Err(Error::program(said!(
            "Steam did not do all it was asked: {}",
            Sentence::listed(failed).inside()
        ))),
    }
}

fn pump(run: impl Fn(), longest: Duration, mut settled: impl FnMut() -> bool) {
    let deadline = Instant::now() + longest;
    loop {
        run();
        if settled() || Instant::now() >= deadline {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn missing(what: &str) -> Error {
    Error::program(said!("the game's Steam library has no {}", what))
}

// SteamAPI_InitFlat(SteamErrMsg *) -> ESteamAPIInitResult: SteamErrMsg is
// char[1024], 0 is k_ESteamAPIInitResult_OK.
type Flat = unsafe extern "C" fn(*mut [u8; 1024]) -> c_int;

/// # Safety
/// `steam` is a Steamworks library and `start` was found in it.
unsafe fn started(steam: &libloading::Library, start: Start) -> Result<(), Error> {
    let refused = || said!("Steam is not running, or does not own the game");
    // SAFETY: each symbol is called as steam_api.h declares it.
    unsafe {
        match start {
            Start::Plain | Start::Safe => {
                let init: libloading::Symbol<unsafe extern "C" fn() -> bool> = steam
                    .get(start.symbol())
                    .map_err(|_| missing("SteamAPI_Init"))?;
                init()
                    .then_some(())
                    .ok_or_else(|| Error::program(refused()))
            }
            Start::Flat => {
                let init: libloading::Symbol<Flat> = steam
                    .get(start.symbol())
                    .map_err(|_| missing("SteamAPI_Init"))?;
                let mut message = [0u8; 1024];
                match init(&mut message) {
                    0 => Ok(()),
                    _ => Err(Error::program(match said_by_steam(&message) {
                        Some(message) => {
                            said!("Steam could not be started for the game: {}", message)
                        }
                        None => refused(),
                    })),
                }
            }
        }
    }
}

fn said_by_steam(message: &[u8]) -> Option<String> {
    let message = CStr::from_bytes_until_nul(message).ok()?.to_string_lossy();
    let message = message.trim();
    (!message.is_empty()).then(|| message.to_string())
}

/// The process's environment must hold `environment()`. Slow: waits up to ten
/// seconds for Steam's answers.
pub fn act(library: &Path, action: Action, ids: &[u64]) -> Result<(), Error> {
    type Item = unsafe extern "C" fn(*mut c_void, u64) -> u64;
    // ISteamUGC::DownloadItem(id, high priority): true when Steam took it.
    type Fetch = unsafe extern "C" fn(*mut c_void, u64, bool) -> bool;
    type State = unsafe extern "C" fn(*mut c_void, u64) -> u32;
    type Completed = unsafe extern "C" fn(*mut c_void, u64, *mut bool) -> bool;
    type Answered =
        unsafe extern "C" fn(*mut c_void, u64, *mut c_void, i32, i32, *mut bool) -> bool;
    enum Ask<'a> {
        Item(libloading::Symbol<'a, Item>),
        Fetch(libloading::Symbol<'a, Fetch>),
    }
    // SAFETY: the game's own Steamworks library, called as its flat C
    // interface is declared (steam_api.json): Init before anything else, the
    // Workshop and Utils interfaces only while initialised, Shutdown last.
    // Each call handle is one SubscribeItem/UnsubscribeItem returned, and
    // GetAPICallResult writes at most the size it is given into an `Answer`.
    unsafe {
        let steam = libloading::Library::new(library).map_err(|e| Error::file(library, e))?;
        let start = start_of(&steam).ok_or_else(|| missing("SteamAPI_Init"))?;
        let run: libloading::Symbol<unsafe extern "C" fn()> = steam
            .get(b"SteamAPI_RunCallbacks\0")
            .map_err(|_| missing("SteamAPI_RunCallbacks"))?;
        let shutdown: libloading::Symbol<unsafe extern "C" fn()> = steam
            .get(b"SteamAPI_Shutdown\0")
            .map_err(|_| missing("SteamAPI_Shutdown"))?;
        // Looked up before Init, so a library without it is left alone.
        let ask = match action {
            Action::Subscribe => steam
                .get(b"SteamAPI_ISteamUGC_SubscribeItem\0")
                .map(Ask::Item),
            Action::Unsubscribe => steam
                .get(b"SteamAPI_ISteamUGC_UnsubscribeItem\0")
                .map(Ask::Item),
            Action::Download => steam
                .get(b"SteamAPI_ISteamUGC_DownloadItem\0")
                .map(Ask::Fetch),
        }
        .map_err(|_| missing("way to do that"))?;
        let state: Option<libloading::Symbol<State>> =
            steam.get(b"SteamAPI_ISteamUGC_GetItemState\0").ok();
        let completed: Option<libloading::Symbol<Completed>> =
            steam.get(b"SteamAPI_ISteamUtils_IsAPICallCompleted\0").ok();
        let answer: Option<libloading::Symbol<Answered>> =
            steam.get(b"SteamAPI_ISteamUtils_GetAPICallResult\0").ok();
        let utils_interface = interface(&steam, "SteamAPI_SteamUtils_v");
        let interface = workshop_interface(&steam).ok_or_else(|| missing("Workshop interface"))?;
        started(&steam, start)?;
        let workshop = interface();
        if workshop.is_null() {
            shutdown();
            return Err(Error::program(said!("Steam gave no Workshop interface")));
        }
        let item = match ask {
            Ask::Item(item) => item,
            Ask::Fetch(fetch) => {
                let refused = ids.iter().filter(|id| !fetch(workshop, **id, true)).count();
                // Steam takes requests only while callbacks run.
                pump(|| run(), Duration::from_secs(3), || false);
                shutdown();
                return match refused {
                    0 => Ok(()),
                    _ => Err(Error::program(said!(
                        "Steam refused {} of {}",
                        refused,
                        ids.len()
                    ))),
                };
            }
        };
        let expected = match action {
            Action::Unsubscribe => UNSUBSCRIBE_ANSWER,
            _ => SUBSCRIBE_ANSWER,
        };
        let utils = utils_interface
            .map(|accessor| accessor())
            .filter(|utils| !utils.is_null());
        let calls: Vec<u64> = ids.iter().map(|id| item(workshop, *id)).collect();
        let mut answers: Vec<Option<Outcome>> = vec![None; ids.len()];
        let mut waiting: Vec<usize> = (0..ids.len()).filter(|i| calls[*i] != 0).collect();
        pump(
            || run(),
            LONGEST,
            || {
                let (Some(utils), Some(completed), Some(answer)) = (utils, &completed, &answer)
                else {
                    return false;
                };
                waiting.retain(|&i| {
                    let mut failed = false;
                    if !completed(utils, calls[i], &mut failed) {
                        return true;
                    }
                    let mut got = Answer::default();
                    let size = std::mem::size_of::<Answer>() as i32;
                    let read = answer(
                        utils,
                        calls[i],
                        (&raw mut got).cast(),
                        size,
                        expected,
                        &mut failed,
                    );
                    answers[i] = (read && !failed).then(|| answered(got.result));
                    false
                });
                waiting.is_empty()
            },
        );
        let outcomes: Vec<(u64, Outcome)> = ids
            .iter()
            .zip(answers)
            .map(|(id, answer)| {
                let outcome = answer.unwrap_or_else(|| match &state {
                    Some(state) => seen(action, state(workshop, *id)),
                    None => Outcome::Unconfirmed,
                });
                (*id, outcome)
            })
            .collect();
        shutdown();
        reported(&outcomes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_names_what_to_do_and_to_which_items() {
        assert_eq!(
            parse("subscribe:1,22, 333"),
            Some((Action::Subscribe, vec![1, 22, 333]))
        );
        assert_eq!(parse("unsubscribe:7"), Some((Action::Unsubscribe, vec![7])));
        assert_eq!(parse("download:7,8"), Some((Action::Download, vec![7, 8])));
        assert_eq!(parse("delete:7"), None);
        assert_eq!(parse("subscribe:"), None);
        assert_eq!(parse("subscribe:1,x"), None);
    }

    #[test]
    fn steam_s_answer_is_ok_only_for_its_first_result() {
        assert_eq!(answered(1), Outcome::Done);
        assert_eq!(answered(2), Outcome::Refused(2));
        assert_eq!(answered(15), Outcome::Refused(15));
    }

    #[test]
    fn an_item_s_state_confirms_what_was_asked() {
        assert_eq!(seen(Action::Subscribe, 1 | 4), Outcome::Done);
        assert_eq!(seen(Action::Subscribe, 4), Outcome::Unconfirmed);
        assert_eq!(seen(Action::Unsubscribe, 0), Outcome::Done);
        assert_eq!(seen(Action::Unsubscribe, 1), Outcome::Unconfirmed);
    }

    #[test]
    fn only_the_items_steam_did_not_do_are_reported() {
        assert!(reported(&[(1, Outcome::Done), (2, Outcome::Done)]).is_ok());
        let said = reported(&[
            (1, Outcome::Done),
            (22, Outcome::Refused(15)),
            (333, Outcome::Unconfirmed),
            (4444, Outcome::Refused(77)),
        ])
        .unwrap_err()
        .to_string();
        assert!(!said.contains("1:"), "{said}");
        assert!(
            said.contains("22: Steam refused it, access denied (15)"),
            "{said}"
        );
        assert!(said.contains("333: Steam did not confirm it"), "{said}");
        assert!(said.contains("4444: Steam refused it (77)"), "{said}");
    }

    #[test]
    fn steam_s_answer_has_the_size_of_its_platform() {
        let size = std::mem::size_of::<Answer>();
        assert_eq!(size, if cfg!(windows) { 16 } else { 12 });
    }

    #[test]
    fn the_library_is_found_in_the_game_s_plugins() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(library(tmp.path()), None);
        let plugins = tmp.path().join("RimWorldLinux_Data/Plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        std::fs::write(plugins.join("libsteam_api.so"), "").unwrap();
        assert_eq!(library(tmp.path()), Some(plugins.join("libsteam_api.so")));
    }

    #[test]
    fn the_library_is_found_inside_a_mac_application_deeper_than_the_walk_goes() {
        let tmp = tempfile::tempdir().unwrap();
        let inside = tmp
            .path()
            .join("RimWorldMac.app/Contents/Plugins/steam_api.bundle/Contents/MacOS");
        std::fs::create_dir_all(&inside).unwrap();
        std::fs::write(inside.join("libsteam_api.dylib"), "").unwrap();
        assert_eq!(library(tmp.path()), Some(inside.join("libsteam_api.dylib")));
    }

    #[test]
    fn steam_is_started_by_the_oldest_way_the_library_has() {
        let having = |names: &'static [&'static str]| {
            move |symbol: &[u8]| {
                names
                    .iter()
                    .any(|name| symbol == format!("{name}\0").as_bytes())
            }
        };
        let unity = having(&["SteamAPI_Init", "SteamAPI_InitSafe"]);
        assert_eq!(start_with(unity), Some(Start::Plain));
        let waylith = having(&[
            "SteamAPI_InitFlat",
            "SteamAPI_InitSafe",
            "SteamInternal_SteamAPI_Init",
        ]);
        assert_eq!(start_with(waylith), Some(Start::Flat));
        assert_eq!(
            start_with(having(&["SteamAPI_InitSafe"])),
            Some(Start::Safe)
        );
        assert_eq!(start_with(having(&["SteamInternal_SteamAPI_Init"])), None);
    }

    #[test]
    fn steam_s_message_is_read_up_to_its_end() {
        let mut message = [0u8; 1024];
        assert_eq!(said_by_steam(&message), None);
        message[..17].copy_from_slice(b"No appID found. \0");
        assert_eq!(said_by_steam(&message).as_deref(), Some("No appID found."));
        assert_eq!(said_by_steam(&[b'x'; 1024]), None);
    }

    #[test]
    #[ignore = "needs RimWorld installed; RIMSTACK_STEAM_LIBRARY names its Steam library"]
    fn real_library_has_what_is_called() {
        let file = match std::env::var_os("RIMSTACK_STEAM_LIBRARY") {
            Some(file) => std::path::PathBuf::from(file),
            None => {
                let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
                let paths = crate::rimworld::paths::detect(
                    &home,
                    &crate::rimworld::paths::Overrides::from_env(),
                );
                library(paths.game.as_ref().unwrap()).unwrap()
            }
        };
        // SAFETY: loading runs no Steam code; symbols are only looked up.
        let steam = unsafe { libloading::Library::new(&file).unwrap() };
        assert!(start_of(&steam).is_some());
        assert!(workshop_interface(&steam).is_some());
        assert!(interface(&steam, "SteamAPI_SteamUtils_v").is_some());
        for name in [
            &b"SteamAPI_RunCallbacks\0"[..],
            b"SteamAPI_Shutdown\0",
            b"SteamAPI_ISteamUGC_SubscribeItem\0",
            b"SteamAPI_ISteamUGC_UnsubscribeItem\0",
            b"SteamAPI_ISteamUGC_DownloadItem\0",
            b"SteamAPI_ISteamUGC_GetItemState\0",
            b"SteamAPI_ISteamUtils_IsAPICallCompleted\0",
            b"SteamAPI_ISteamUtils_GetAPICallResult\0",
        ] {
            // SAFETY: looked up, never called.
            assert!(unsafe { steam.get::<unsafe extern "C" fn()>(name).is_ok() });
        }
    }
}
