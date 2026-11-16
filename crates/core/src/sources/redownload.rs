use crate::base::aside::{self, NEW, OLD};
use crate::base::error::{Error, Kind, Result, Sentence};
use crate::said;

use std::path::{Path, PathBuf};

use crate::rimworld::paths::APP_ID as GAME;
use crate::rimworld::workshop::{Version, installed_versions, is_id};

/// None on macOS, where it is not run.
pub fn steamcmd(data_dir: &Path) -> Option<PathBuf> {
    on_this_system().ok()?;
    steamcmd_among(data_dir, std::env::var_os("PATH"))
}

pub fn refusal() -> Option<Sentence> {
    on_this_system().err().map(|e| e.sentence().clone())
}

fn on_this_system() -> Result<()> {
    match crate::rimworld::paths::Os::running() {
        crate::rimworld::paths::Os::Mac => Err(Error::refused(said!(
            "RimStack does not run SteamCMD on macOS yet: it could write over Steam's own files there"
        ))),
        _ => Ok(()),
    }
}

fn steamcmd_among(data_dir: &Path, path: Option<std::ffi::OsString>) -> Option<PathBuf> {
    let own = ["steamcmd.sh", "steamcmd.exe"].map(|name| data_dir.join("steamcmd").join(name));
    if let Some(own) = own.into_iter().find(|file| file.is_file()) {
        return Some(own);
    }
    let path = path?;
    let named = |dir: PathBuf| ["steamcmd", "steamcmd.exe"].map(|name| dir.join(name));
    let mut places = std::env::split_paths(&path).flat_map(named);
    places.find(|file| file.is_file())
}

/// Uses the network and the system's `tar`; SteamCMD fetches the rest of itself on its first run.
pub fn install(data_dir: &Path) -> Result<PathBuf> {
    on_this_system()?;
    // Windows' own tar also reads zip files.
    let (archive, program) = match std::env::consts::OS {
        "linux" => ("steamcmd_linux.tar.gz", "steamcmd.sh"),
        "windows" => ("steamcmd.zip", "steamcmd.exe"),
        other => {
            return Err(Error::refused(said!(
                "RimStack does not install SteamCMD on {}",
                other
            )));
        }
    };
    let dir = data_dir.join("steamcmd");
    std::fs::create_dir_all(&dir)
        .map_err(|e| Error::caused(Kind::File, said!("its folder could not be made"), e))?;
    let agent = crate::base::web::agent(120);
    let url = format!("https://steamcdn-a.akamaihd.net/client/installer/{archive}");
    let mut response = agent
        .get(&url)
        .call()
        .map_err(|e| Error::caused(Kind::Network, said!("the download failed"), e))?;
    let bytes = response
        .body_mut()
        .read_to_vec()
        .map_err(|e| Error::caused(Kind::Network, said!("the download failed"), e))?;
    let packed = dir.join(archive);
    std::fs::write(&packed, bytes)
        .map_err(|e| Error::caused(Kind::File, said!("the download could not be kept"), e))?;
    let listed = crate::base::program::run(
        crate::base::program::command("tar").arg("-tf").arg(&packed),
        crate::base::program::LOCAL,
    )
    .map_err(|e| Error::caused(Kind::Program, said!("tar could not be run"), e))?;
    if let Err(problem) = only_inside(&String::from_utf8_lossy(&listed.stdout), program) {
        let _ = std::fs::remove_file(&packed);
        return Err(problem);
    }
    let unpacked = crate::base::program::run(
        crate::base::program::command("tar")
            .arg("-xf")
            .arg(&packed)
            .arg("-C")
            .arg(&dir),
        crate::base::program::NETWORK,
    )
    .map_err(|e| Error::caused(Kind::Program, said!("tar could not be run"), e))?
    .status;
    let _ = std::fs::remove_file(&packed);
    let program = dir.join(program);
    match unpacked.success() && program.is_file() {
        true => Ok(program),
        false => Err(Error::program(said!(
            "the download did not unpack to SteamCMD"
        ))),
    }
}

/// Valve publishes no checksum for the download, so this check is all there is.
fn only_inside(listing: &str, program: &str) -> Result<()> {
    let outside = listing.lines().find(|entry| {
        let entry = Path::new(entry.trim());
        entry.has_root()
            || entry.components().any(|part| {
                !matches!(
                    part,
                    std::path::Component::Normal(_) | std::path::Component::CurDir
                )
            })
    });
    if let Some(entry) = outside {
        return Err(Error::refused(said!(
            "the download would unpack outside its folder: {}",
            entry
        )));
    }
    let holds = |entry: &str| entry.trim().trim_start_matches("./") == program;
    match listing.lines().any(holds) {
        true => Ok(()),
        false => Err(Error::program(said!(
            "the download did not unpack to SteamCMD"
        ))),
    }
}

pub(super) fn not_an_id(id: &str) -> Error {
    Error::refused(said!("{} is not a Workshop id", id))
}

/// The folders are deleted when this is dropped: keep what is needed by moving it out first.
pub struct Fetched {
    pub items: Vec<(String, Result<PathBuf>)>,
    attempt: Option<PathBuf>,
}

impl Fetched {
    pub fn first(&self) -> Result<&Path> {
        match self.items.first() {
            Some((_, outcome)) => outcome.as_deref().map_err(Clone::clone),
            None => Err(Error::refused(said!("nothing was asked"))),
        }
    }

    /// What SteamCMD recorded of each item it downloaded in this run.
    pub fn versions(&self) -> std::collections::HashMap<String, Version> {
        let record = self.attempt.as_ref().and_then(|attempt| {
            let acf = attempt.join(format!("steamapps/workshop/appworkshop_{GAME}.acf"));
            std::fs::read_to_string(acf).ok()
        });
        record.map_or_else(Default::default, |acf| installed_versions(&acf))
    }
}

impl Drop for Fetched {
    fn drop(&mut self) {
        if let Some(attempt) = &self.attempt {
            let _ = std::fs::remove_dir_all(attempt);
        }
    }
}

const ATTEMPT: &str = "attempt-";

/// One run at a time per `scratch`.
fn new_attempt(scratch: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(scratch).map_err(|e| Error::file(scratch, e))?;
    for entry in std::fs::read_dir(scratch).into_iter().flatten().flatten() {
        if entry.file_name().to_string_lossy().starts_with(ATTEMPT) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let attempt = scratch.join(format!("{ATTEMPT}{}-{nanos}", std::process::id()));
    std::fs::create_dir(&attempt).map_err(|e| Error::file(&attempt, e))?;
    Ok(attempt)
}

fn plain(output: &[u8]) -> String {
    let text = String::from_utf8_lossy(output);
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\x1b' => {
                let _ = chars.find(char::is_ascii_alphabetic);
            }
            c => plain.push(c),
        }
    }
    plain
}

fn said_downloaded(said: &str, id: &str) -> bool {
    said.contains(&format!("Success. Downloaded item {id} "))
}

fn why_not_downloaded<'a>(said: &'a str, id: &str) -> Option<&'a str> {
    let opening = format!("ERROR! Download item {id} failed (");
    let after = &said[said.find(&opening)? + opening.len()..];
    after.split(')').next()
}

/// As an anonymous user. Blocks on the network for as long as the mods take.
pub fn fetch_all(steamcmd: &Path, ids: &[String], scratch: &Path) -> Fetched {
    let every = |problem: Error| Fetched {
        items: ids
            .iter()
            .map(|id| (id.clone(), Err(problem.clone())))
            .collect(),
        attempt: None,
    };
    if let Err(problem) = on_this_system() {
        return every(problem);
    }
    let (asked, refused): (Vec<&String>, Vec<&String>) = ids.iter().partition(|id| is_id(id));
    let not_an_id = |id: &&String| ((*id).clone(), Err(not_an_id(id)));
    let mut items: Vec<_> = refused.iter().map(not_an_id).collect();
    if asked.is_empty() {
        return Fetched {
            items,
            attempt: None,
        };
    }
    let attempt = match new_attempt(scratch) {
        Ok(attempt) => attempt,
        Err(problem) => return every(problem),
    };
    // A home of its own: with the user's, SteamCMD writes over Steam's list of library folders.
    let home = scratch.join("home");
    let _ = std::fs::create_dir_all(&home);
    let mut command = crate::base::program::command(steamcmd);
    command
        .env("HOME", &home)
        .env_remove("XDG_DATA_HOME")
        .env_remove("XDG_CONFIG_HOME")
        .arg("+force_install_dir")
        .arg(&attempt)
        .args(["+login", "anonymous"]);
    for id in &asked {
        command.args(["+workshop_download_item", GAME, id, "validate"]);
    }
    let ran = crate::base::program::run(command.arg("+quit"), crate::base::program::DOWNLOAD)
        .map(|out| plain(&out.stdout))
        .map_err(|e| Error::program(said!("SteamCMD could not be run: {}", e)));
    let content = attempt.join("steamapps/workshop/content").join(GAME);
    // SteamCMD exits with 0 when an item fails, and may run messages together on one line:
    // look for an item's outcome in the whole output.
    let outcome = |id: &String| {
        let said = ran.as_ref().map_err(Clone::clone)?;
        let fresh = content.join(id);
        if said_downloaded(said, id) && fresh.join("About").is_dir() {
            return Ok(fresh);
        }
        let why = why_not_downloaded(said, id).or_else(|| {
            let last = said.lines().rev().find(|l| !l.trim().is_empty());
            last.map(str::trim)
        });
        Err(Error::program(match why {
            Some(why) => said!("SteamCMD did not download it: {}", why),
            None => said!("SteamCMD did not download it"),
        }))
    };
    items.extend(asked.iter().map(|id| ((*id).clone(), outcome(id))));
    Fetched {
        items,
        attempt: Some(attempt),
    }
}

pub fn put_in_local(fresh: &Path, workshop_id: &str, local_mods: &Path) -> Result<PathBuf> {
    if !is_id(workshop_id) {
        return Err(not_an_id(workshop_id));
    }
    note_workshop_id(fresh, workshop_id)?;
    let to = local_mods.join(workshop_id);
    replace(&to, fresh)?;
    Ok(to)
}

/// `About/PublishedFileId.txt`, where the game and other tools look for a local mod's Workshop
/// id; one the author shipped is kept.
pub(super) fn note_workshop_id(mod_root: &Path, workshop_id: &str) -> Result<()> {
    let about = mod_root.join("About");
    let noted = about.join("PublishedFileId.txt");
    if noted.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(&about)
        .and_then(|()| std::fs::write(&noted, workshop_id))
        .map_err(|e| Error::file(&noted, e))
}

/// `installed` need not be there. The new copy is made beside it and swapped in by renames, so
/// a crash at any point leaves only what `settle` turns back into one whole folder.
pub fn replace(installed: &Path, fresh: &Path) -> Result<()> {
    let failed = |what: Sentence, e: std::io::Error| Error::caused(Kind::File, what, e);
    settle(installed)?;
    let (old, new) = (aside::beside(installed, OLD), aside::beside(installed, NEW));
    let staged = std::fs::rename(fresh, &new).or_else(|_| crate::base::walk::copy(fresh, &new));
    if let Err(problem) = staged {
        let _ = remove(&new);
        return Err(failed(
            said!("the new copy could not be put in place"),
            problem,
        ));
    }
    let present = installed.is_dir();
    if present && let Err(problem) = std::fs::rename(installed, &old) {
        let _ = remove(&new);
        return Err(failed(
            said!("the mod's folder could not be moved aside"),
            problem,
        ));
    }
    if let Err(problem) = std::fs::rename(&new, installed) {
        let _ = remove(&new);
        let back = !present || std::fs::rename(&old, installed).is_ok();
        let what = match back {
            true => said!("the new copy could not be put in place"),
            false => said!(
                "the new copy could not be put in place, and the old one is now {}",
                old.display()
            ),
        };
        return Err(failed(what, problem));
    }
    remove(&old).map_err(|e| failed(said!("the old copy could not be deleted"), e))
}

/// A copy being made beside `installed` may be cut short, so it is dropped. The one moved aside
/// goes back unless `installed` is there, which it only ever is whole.
pub fn settle(installed: &Path) -> Result<()> {
    let (old, new) = (aside::beside(installed, OLD), aside::beside(installed, NEW));
    remove(&new).map_err(|e| Error::file(&new, e))?;
    if std::fs::symlink_metadata(&old).is_err() {
        return Ok(());
    }
    match installed.is_dir() {
        true => remove(&old)
            .map_err(|e| Error::caused(Kind::File, said!("the old copy could not be deleted"), e)),
        false => std::fs::rename(&old, installed).map_err(|e| Error::file(&old, e)),
    }
}

/// Only the folders RimStack sets aside are touched; each that could not be settled is told.
pub fn settle_within(local_mods: &Path) -> Vec<Error> {
    let entries = std::fs::read_dir(local_mods)
        .into_iter()
        .flatten()
        .flatten();
    let owners: std::collections::BTreeSet<String> = entries
        .filter_map(|e| aside::left_beside(&e.file_name()).map(String::from))
        .collect();
    owners
        .iter()
        .filter_map(|owner| settle(&local_mods.join(owner)).err())
        .collect()
}

fn remove(path: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
        Ok(m) if m.is_dir() => std::fs::remove_dir_all(path),
        Ok(_) => std::fs::remove_file(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_archive_is_unpacked_only_when_every_entry_stays_inside_and_it_holds_the_program() {
        assert!(only_inside("steamcmd.sh\nlinux32/\nlinux32/steamcmd\n", "steamcmd.sh").is_ok());
        assert!(only_inside("./steamcmd.sh\n./linux32/libstdc++.so.6\n", "steamcmd.sh").is_ok());
        let escaping = only_inside("steamcmd.sh\n../../.bashrc\n", "steamcmd.sh").unwrap_err();
        assert_eq!(escaping.kind(), Kind::Refused);
        assert!(only_inside("/etc/profile.d/x.sh\nsteamcmd.sh\n", "steamcmd.sh").is_err());
        assert_eq!(
            only_inside("readme.txt\n", "steamcmd.sh")
                .unwrap_err()
                .kind(),
            Kind::Program
        );
    }

    #[test]
    fn the_fresh_copy_takes_the_place_of_the_one_on_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let (installed, fresh) = (tmp.path().join("123"), tmp.path().join("new"));
        std::fs::create_dir_all(installed.join("About")).unwrap();
        std::fs::write(installed.join("stale.txt"), "old").unwrap();
        std::fs::create_dir_all(fresh.join("About")).unwrap();
        std::fs::write(fresh.join("About/About.xml"), "new").unwrap();
        replace(&installed, &fresh).unwrap();
        assert_eq!(
            std::fs::read_to_string(installed.join("About/About.xml")).unwrap(),
            "new"
        );
        assert!(!installed.join("stale.txt").exists());
        assert_eq!(folders(tmp.path()), ["123"]);
    }

    fn folders(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn folder_holding(path: &Path, text: &str) {
        std::fs::create_dir_all(path.join("About")).unwrap();
        std::fs::write(path.join("About/About.xml"), text).unwrap();
    }

    fn held(path: &Path) -> String {
        std::fs::read_to_string(path.join("About/About.xml")).unwrap()
    }

    #[test]
    fn a_copy_cut_short_beside_the_mod_is_dropped_and_the_mod_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let installed = tmp.path().join("123");
        folder_holding(&installed, "whole");
        std::fs::create_dir_all(tmp.path().join("123.rimstack-new/About")).unwrap();
        settle(&installed).unwrap();
        assert_eq!(folders(tmp.path()), ["123"]);
        assert_eq!(held(&installed), "whole");
    }

    #[test]
    fn a_mod_moved_aside_and_not_replaced_goes_back() {
        let tmp = tempfile::tempdir().unwrap();
        let installed = tmp.path().join("123");
        folder_holding(&tmp.path().join("123.rimstack-old"), "whole");
        settle(&installed).unwrap();
        assert_eq!(folders(tmp.path()), ["123"]);
        assert_eq!(held(&installed), "whole");
    }

    #[test]
    fn a_mod_moved_aside_with_its_new_copy_beside_goes_back() {
        let tmp = tempfile::tempdir().unwrap();
        let installed = tmp.path().join("123");
        folder_holding(&tmp.path().join("123.rimstack-old"), "old");
        folder_holding(&tmp.path().join("123.rimstack-new"), "new");
        settle(&installed).unwrap();
        assert_eq!(folders(tmp.path()), ["123"]);
        assert_eq!(held(&installed), "old");
    }

    #[test]
    fn a_mod_in_place_drops_the_copy_moved_aside() {
        let tmp = tempfile::tempdir().unwrap();
        let installed = tmp.path().join("123");
        folder_holding(&installed, "new");
        folder_holding(&tmp.path().join("123.rimstack-old"), "old");
        settle(&installed).unwrap();
        assert_eq!(folders(tmp.path()), ["123"]);
        assert_eq!(held(&installed), "new");
    }

    #[test]
    fn a_copy_cut_short_earlier_is_not_put_in_place() {
        let tmp = tempfile::tempdir().unwrap();
        let (installed, fresh) = (tmp.path().join("123"), tmp.path().join("fresh"));
        folder_holding(&installed, "old");
        folder_holding(&fresh, "new");
        std::fs::write(tmp.path().join("123.rimstack-new"), "cut short").unwrap();
        replace(&installed, &fresh).unwrap();
        assert_eq!(folders(tmp.path()), ["123"]);
        assert_eq!(held(&installed), "new");
    }

    #[test]
    fn every_leftover_in_the_mods_folder_is_settled_and_nothing_else_touched() {
        let tmp = tempfile::tempdir().unwrap();
        let mods = tmp.path();
        folder_holding(&mods.join("1.rimstack-old"), "one");
        folder_holding(&mods.join("2"), "two");
        folder_holding(&mods.join("2.rimstack-old"), "two before");
        folder_holding(&mods.join("3"), "three");
        folder_holding(&mods.join("3.rimstack-new"), "three cut short");
        folder_holding(&mods.join("rimstack-old"), "a mod of that name");
        assert!(settle_within(mods).is_empty());
        assert_eq!(folders(mods), ["1", "2", "3", "rimstack-old"]);
        assert_eq!(held(&mods.join("1")), "one");
        assert_eq!(held(&mods.join("2")), "two");
        assert_eq!(held(&mods.join("3")), "three");
    }

    #[test]
    fn a_download_takes_the_place_of_a_mod_left_aside_by_an_earlier_one() {
        let tmp = tempfile::tempdir().unwrap();
        let (fresh, local) = (tmp.path().join("scratch/123"), tmp.path().join("Mods"));
        folder_holding(&fresh, "new");
        folder_holding(&local.join("123.rimstack-old"), "old");
        let put = put_in_local(&fresh, "123", &local).unwrap();
        assert_eq!(folders(&local), ["123"]);
        assert_eq!(held(&put), "new");
        assert!(put.join("About/PublishedFileId.txt").is_file());
    }

    #[test]
    fn a_folder_with_a_dot_in_its_name_is_set_aside_under_that_name() {
        let tmp = tempfile::tempdir().unwrap();
        let (installed, other) = (
            tmp.path().join("Mod.v2"),
            tmp.path().join("Mod.rimstack-old"),
        );
        std::fs::create_dir_all(&installed).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("theirs.txt"), "kept").unwrap();
        let fresh = tmp.path().join("new");
        std::fs::create_dir_all(&fresh).unwrap();
        replace(&installed, &fresh).unwrap();
        assert!(other.join("theirs.txt").is_file());
    }

    #[test]
    fn the_copy_on_disk_stays_when_there_is_nothing_to_put_in_its_place() {
        let tmp = tempfile::tempdir().unwrap();
        let installed = tmp.path().join("123");
        std::fs::create_dir_all(&installed).unwrap();
        std::fs::write(installed.join("kept.txt"), "old").unwrap();
        assert!(replace(&installed, &tmp.path().join("missing")).is_err());
        assert_eq!(
            std::fs::read_to_string(installed.join("kept.txt")).unwrap(),
            "old"
        );
        assert_eq!(folders(tmp.path()), ["123"]);
    }

    #[test]
    fn a_folder_left_aside_earlier_does_not_stop_the_next_download() {
        let tmp = tempfile::tempdir().unwrap();
        let (installed, fresh) = (tmp.path().join("123"), tmp.path().join("new"));
        let left = tmp.path().join("123.rimstack-old");
        for folder in [&installed, &fresh, &left] {
            std::fs::create_dir_all(folder.join("About")).unwrap();
        }
        assert_eq!(replace(&installed, &fresh), Ok(()));
        assert!(!left.exists());
    }

    #[test]
    fn a_download_is_never_put_outside_the_mods_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let (fresh, mods) = (tmp.path().join("fresh"), tmp.path().join("game/Mods"));
        std::fs::create_dir_all(fresh.join("About")).unwrap();
        std::fs::create_dir_all(&mods).unwrap();
        let put = put_in_local(&fresh, "../../escaped", &mods);
        assert_eq!(put.unwrap_err().kind(), Kind::Refused);
        assert!(!tmp.path().join("escaped").exists());
    }

    #[test]
    fn only_a_number_is_asked_of_steamcmd() {
        let tmp = tempfile::tempdir().unwrap();
        let fetch =
            |id: &str| fetch_all(Path::new("/nonexistent/steamcmd"), &[id.into()], tmp.path());
        assert_eq!(
            fetch("12; rm -rf").first().unwrap_err().kind(),
            Kind::Refused
        );
        assert_eq!(fetch("123456").first().unwrap_err().kind(), Kind::Program);
    }

    #[cfg(unix)]
    #[test]
    fn steamcmd_is_run_with_a_home_of_its_own_never_the_user_s() {
        let tmp = tempfile::tempdir().unwrap();
        let seen = tmp.path().join("seen");
        let body = format!("echo \"$HOME|$XDG_DATA_HOME\" > '{}'", seen.display());
        let fake = fake_steamcmd(tmp.path(), &body);
        let scratch = tmp.path().join("scratch");
        let _ = fetch_all(&fake, &["123456".to_string()], &scratch);
        let given = std::fs::read_to_string(&seen).unwrap();
        assert_eq!(given.trim(), format!("{}|", scratch.join("home").display()));
        assert_ne!(
            Some(scratch.join("home")),
            std::env::var_os("HOME").map(PathBuf::from)
        );
    }

    #[cfg(unix)]
    fn fake_steamcmd(at: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let (written, fake) = (at.join("steamcmd.txt"), at.join("steamcmd.sh"));
        let script = format!(
            "#!/bin/sh\nwhile [ $# -gt 0 ]; do\n  [ \"$1\" = +force_install_dir ] && dir=$2\n  shift\ndone\n\
             content=\"$dir/steamapps/workshop/content/294100\"\n{body}\n"
        );
        std::fs::write(&written, script).unwrap();
        std::fs::set_permissions(&written, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Copied by `cp`: a file this process wrote is "Text file busy" to run while a child
        // forked meanwhile holds it open.
        let copied = std::process::Command::new("cp")
            .arg(&written)
            .arg(&fake)
            .status();
        assert!(copied.unwrap().success());
        fake
    }

    #[cfg(unix)]
    #[test]
    fn an_earlier_download_left_in_the_scratch_is_not_taken_for_a_failed_one() {
        let tmp = tempfile::tempdir().unwrap();
        let scratch = tmp.path().join("scratch");
        let old = scratch.join("steamapps/workshop/content/294100/123");
        std::fs::create_dir_all(old.join("About")).unwrap();
        let fake = fake_steamcmd(
            tmp.path(),
            "printf 'Downloading item 123 ...\\x1b[0m\\nERROR! Download item 123 failed (File Not Found).\\n'",
        );
        let all = fetch_all(&fake, &["123".to_string()], &scratch);
        let (_, outcome) = &all.items[0];
        let problem = outcome.as_ref().unwrap_err();
        assert_eq!(problem.kind(), Kind::Program);
        assert!(problem.to_string().contains("File Not Found"), "{problem}");
    }

    #[cfg(unix)]
    #[test]
    fn a_download_is_taken_from_this_run_s_own_folder_when_steamcmd_says_it_succeeded() {
        let tmp = tempfile::tempdir().unwrap();
        let scratch = tmp.path().join("scratch");
        let stale = scratch.join("attempt-1-1");
        std::fs::create_dir_all(&stale).unwrap();
        let fake = fake_steamcmd(
            tmp.path(),
            "mkdir -p \"$content/123/About\"\n\
             printf '\"WorkshopItemsInstalled\" { \"123\" { \"timeupdated\" \"44\" \"manifest\" \"55\" } }' \
             > \"$dir/steamapps/workshop/appworkshop_294100.acf\"\n\
             printf '\\x1b[0mSuccess. Downloaded item 123 to \"%s/123\" (8296511 bytes) Downloading item 4 ...\\n' \"$content\"",
        );
        let all = fetch_all(&fake, &["123".to_string()], &scratch);
        let fresh = all.items[0].1.as_ref().unwrap().clone();
        assert!(fresh.join("About").is_dir());
        assert_eq!(
            all.versions()["123"],
            Version {
                manifest: 55,
                updated: 44
            }
        );
        assert!(!fresh.starts_with(scratch.join("steamapps")));
        assert!(fresh.starts_with(&scratch));
        assert!(!stale.exists());
        drop(all);
        assert!(!fresh.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_steamcmd_does_not_say_it_downloaded_is_not_taken() {
        let tmp = tempfile::tempdir().unwrap();
        let fake = fake_steamcmd(
            tmp.path(),
            "mkdir -p \"$content/123/About\"\necho 'ERROR! Download item 123 failed (Timeout).'",
        );
        let all = fetch_all(&fake, &["123".to_string()], &tmp.path().join("scratch"));
        assert!(all.first().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn one_run_tells_each_item_s_own_outcome() {
        let tmp = tempfile::tempdir().unwrap();
        let scratch = tmp.path().join("scratch");
        let fake = fake_steamcmd(
            tmp.path(),
            "mkdir -p \"$content/12/About\" \"$content/123/About\"\n\
             printf 'Success. Downloaded item 12 to \"x\" (1 bytes)\\x1b[0m\
             ERROR! Download item 123 failed (Timeout).\\n'",
        );
        let all = fetch_all(&fake, &["12".to_string(), "123".to_string()], &scratch);
        let outcome = |id: &str| &all.items.iter().find(|(i, _)| i == id).unwrap().1;
        assert!(outcome("12").as_ref().unwrap().join("About").is_dir());
        let failed = outcome("123").as_ref().unwrap_err().to_string();
        assert!(failed.contains("Timeout"), "{failed}");
    }

    #[test]
    fn a_download_lands_in_the_local_mods_under_its_id() {
        let tmp = tempfile::tempdir().unwrap();
        let (fresh, local) = (tmp.path().join("scratch/123"), tmp.path().join("Mods"));
        std::fs::create_dir_all(fresh.join("About")).unwrap();
        std::fs::create_dir_all(&local).unwrap();
        let put = put_in_local(&fresh, "123", &local).unwrap();
        assert_eq!(
            std::fs::read_to_string(put.join("About/PublishedFileId.txt")).unwrap(),
            "123"
        );
        std::fs::create_dir_all(fresh.join("About")).unwrap();
        std::fs::write(fresh.join("new.txt"), "2").unwrap();
        put_in_local(&fresh, "123", &local).unwrap();
        assert!(local.join("123/new.txt").is_file());
    }

    #[test]
    fn rimstack_s_own_steamcmd_is_found_before_the_system_s() {
        let tmp = tempfile::tempdir().unwrap();
        let (data, system) = (tmp.path().join("data"), tmp.path().join("bin"));
        std::fs::create_dir_all(&system).unwrap();
        std::fs::write(system.join("steamcmd"), "").unwrap();
        let path = Some(std::env::join_paths([tmp.path().join("none"), system.clone()]).unwrap());
        assert_eq!(
            steamcmd_among(&data, path.clone()),
            Some(system.join("steamcmd"))
        );
        std::fs::create_dir_all(data.join("steamcmd")).unwrap();
        std::fs::write(data.join("steamcmd/steamcmd.sh"), "").unwrap();
        let own = Some(data.join("steamcmd/steamcmd.sh"));
        assert_eq!(steamcmd_among(&data, path), own);
        assert_eq!(steamcmd_among(&tmp.path().join("empty"), None), None);
    }

    #[test]
    #[ignore = "needs the network and RIMSTACK_SCRATCH: an empty folder"]
    fn real_steamcmd_is_installed_and_downloads_a_mod() {
        let scratch = PathBuf::from(std::env::var_os("RIMSTACK_SCRATCH").unwrap());
        let program = steamcmd(&scratch)
            .map_or_else(|| install(&scratch), Ok)
            .unwrap();
        println!("installed {}", program.display());
        let harmony = ["2009463077".to_string()];
        let fetched = fetch_all(&program, &harmony, &scratch.join("downloads"));
        let fresh = fetched.first().unwrap();
        println!("downloaded {}", fresh.display());
        let local = scratch.join("Mods");
        std::fs::create_dir_all(&local).unwrap();
        let put = put_in_local(fresh, "2009463077", &local).unwrap();
        assert!(put.join("About/About.xml").is_file());
    }
}
