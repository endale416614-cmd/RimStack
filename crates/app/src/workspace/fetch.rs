use super::App;
use rimstack_core::base::error::{Error, Result};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::ModId;
use rimstack_core::sources::{git, redownload};
use rimstack_core::{said, say};
use std::path::PathBuf;

pub struct Done {
    pub note: Result<Sentence>,
    pub wrote: bool,
}

impl Done {
    pub(super) fn told(note: Result<Sentence>) -> Self {
        Self { note, wrote: false }
    }

    fn of_a_write(note: Result<Sentence>) -> Self {
        Self {
            wrote: note.is_ok(),
            note,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteamAsk {
    Subscribe,
    Unsubscribe,
    Download,
}

impl SteamAsk {
    fn word(self) -> &'static str {
        match self {
            SteamAsk::Subscribe => "subscribe",
            SteamAsk::Unsubscribe => "unsubscribe",
            SteamAsk::Download => "download",
        }
    }
}

impl App {
    fn mod_folder(&self, id: ModId) -> Result<(PathBuf, String)> {
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let m = library.get(id).ok_or_else(|| {
            Error::refused(said!(
                "That mod has moved since it was read; read the folders again"
            ))
        })?;
        Ok((m.path.clone(), m.display_name().to_string()))
    }

    /// Through the program started again: Steam shows the game as running
    /// while its library is in use.
    pub fn ask_steam(&self, what: SteamAsk, ids: &[String]) -> Done {
        let request = format!("--steamworks={}:{}", what.word(), ids.join(","));
        // On Linux through /proc: the path of a program rebuilt or
        // updated while it runs no longer leads to it.
        let own = std::path::Path::new("/proc/self/exe");
        let program = match own.exists() {
            true => Ok(own.to_path_buf()),
            false => std::env::current_exe(),
        };
        let ran = program.and_then(|program| {
            rimstack_core::base::program::run(
                rimstack_core::base::program::command(program).arg(&request),
                rimstack_core::base::program::NETWORK,
            )
        });
        Done::told(match ran {
            Ok(out) if out.status.success() && what == SteamAsk::Download => {
                Ok(say!("Steam was asked to update {}", ids.len()))
            }
            Ok(out) if out.status.success() => Ok(say!("Steam was asked about {}", ids.len())),
            Ok(out) => Err(Error::program(said!(
                "Steam did not do all it was asked: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ))),
            Err(problem) => Err(Error::program(said!("Steam was not asked: {}", problem))),
        })
    }

    pub fn unsubscribe(&self, id: ModId) -> Done {
        let Some(workshop) = self.workshop_id(id) else {
            return Done::told(Err(Error::refused(said!("Not a Workshop mod"))));
        };
        self.take_back_own_files(id);
        self.ask_steam(SteamAsk::Unsubscribe, &[workshop])
    }

    /// The DDS files RimStack made are deleted and the textures it set aside put back, so the
    /// folder holds only what the mod shipped.
    fn take_back_own_files(&self, id: ModId) {
        let never = std::sync::atomic::AtomicBool::new(false);
        self.convert_textures(&[id], true, &never, &|_| {});
        if let Err(problem) = self.unhide_mod(id) {
            rimstack_core::problem!("{problem}");
        }
    }

    pub fn update_checkout(&self, id: ModId) -> Done {
        let (folder, name) = match self.mod_folder(id) {
            Ok(found) => found,
            Err(problem) => return Done::told(Err(problem)),
        };
        let updated = git::update(&folder);
        match &updated {
            Ok(_) => self.checkout_updated(&folder),
            Err(problem) => self.checkout_refused(&folder, problem.sentence()),
        }
        self.publish(crate::changes::Changes::UPDATES);
        Done::of_a_write(match updated {
            Ok(commit) => Ok(say!("Updated {}: {}", name, commit)),
            Err(problem) => Err(Error::new(
                problem.kind(),
                said!("Not updated: {}", problem.within()),
            )),
        })
    }

    pub fn clone_repository(&self, link: &str) -> Done {
        let Some(mods) = self.paths().local_mods() else {
            return Done::told(Err(Error::refused(said!("The game's folder is not known"))));
        };
        Done::of_a_write(match git::clone(link, &mods) {
            Ok(into) => Ok(say!("Cloned {} into {}", link, into.display())),
            Err(problem) => Err(Error::new(
                problem.kind(),
                said!("Not cloned: {}", problem.within()),
            )),
        })
    }

    pub fn install_steamcmd(&self) -> Done {
        Done::told(match redownload::install(self.data_dir()) {
            Ok(_) => {
                self.look_for_steamcmd();
                self.publish(crate::changes::Changes::DOWNLOAD_LIST);
                Ok(say!("SteamCMD installed"))
            }
            Err(problem) => Err(Error::new(
                problem.kind(),
                said!("SteamCMD not installed: {}", problem.within()),
            )),
        })
    }

    pub fn download_listed(&self) -> Done {
        if self.listed_ids().is_empty() {
            return Done::told(Err(Error::refused(said!("The download list is empty"))));
        }
        if let Err(problem) = self.steamcmd_and_local() {
            return Done::told(Err(problem));
        }
        self.clear_list_failures();
        let ids = self.vet_list();
        if ids.is_empty() {
            return Done::told(Err(Self::nothing_to_ask()));
        }
        let outcomes = match self.fetch_into_local(&ids) {
            Ok(outcomes) => outcomes,
            Err(problem) => return Done::told(Err(problem)),
        };
        self.apply_downloads(&outcomes);
        let n = outcomes.len();
        super::copies::done_of(&outcomes, say!("{} downloaded", n), |failed, first| {
            said!("{} of {} not downloaded: {}", failed, n, first.within())
        })
    }

    fn nothing_to_ask() -> Error {
        Error::refused(said!(
            "Steam cannot be asked about anything on the list; each row says why"
        ))
    }

    pub fn subscribe_listed(&self) -> Done {
        self.clear_list_failures();
        let ids = self.vet_list();
        if ids.is_empty() {
            return Done::told(Err(Self::nothing_to_ask()));
        }
        let done = self.ask_steam(SteamAsk::Subscribe, &ids);
        if done.note.is_ok() {
            self.forget_listed(&ids);
        }
        done
    }

    /// Slow when a crash left a copy to delete: off the window's thread, before the library is
    /// read.
    pub fn settle_downloads(&self) {
        let Some(local) = self.paths().local_mods() else {
            return;
        };
        for problem in redownload::settle_within(&local) {
            rimstack_core::problem!("{problem}");
        }
    }

    pub fn redownload(&self, id: ModId) -> Done {
        let found = self.mod_folder(id).and_then(|(folder, _)| {
            let workshop = self
                .workshop_id(id)
                .ok_or_else(|| Error::refused(said!("Not a Workshop mod")))?;
            Ok((folder, workshop))
        });
        let scratch = self.data_dir().join("steamcmd");
        let done = found.and_then(|(folder, workshop)| {
            let steamcmd = (*self.steamcmd())
                .clone()
                .ok_or_else(|| Error::refused(said!("SteamCMD is not installed")))?;
            let fetched =
                redownload::fetch_all(&steamcmd, std::slice::from_ref(&workshop), &scratch);
            redownload::replace(&folder, fetched.first()?)
        });
        Done::of_a_write(match done {
            Ok(()) => Ok(say!("Downloaded again")),
            Err(problem) => Err(Error::new(
                problem.kind(),
                said!("Not downloaded again: {}", problem.within()),
            )),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::workspace::tests::installed;
    use rimstack_core::base::error::Kind;
    use rimstack_core::mods::hidden::Hidden;
    use rimstack_core::mods::library::ModId;
    use std::fs;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn a_local_mod_is_not_downloaded_again_and_a_gone_one_is_not_updated() {
        let (_tmp, app) = installed();
        let loaded = app.library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();
        let roads = library.ids().find(|(_, m)| m.folder == "roads").unwrap().0;
        drop(loaded);
        let done = app.redownload(roads);
        assert_eq!(done.note.unwrap_err().kind(), Kind::Refused);
        assert!(!done.wrote);
        let gone = app.update_checkout(ModId::new(0, 99));
        assert_eq!(gone.note.unwrap_err().kind(), Kind::Refused);
        let not_git = app.update_checkout(roads);
        assert!(not_git.note.is_err() && !not_git.wrote);
    }

    #[test]
    fn before_unsubscribing_only_what_rimstack_added_is_taken_back() {
        let (tmp, app) = installed();
        let textures = tmp.path().join("game/Mods/roads/Textures");
        fs::create_dir_all(&textures).unwrap();
        image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]))
            .save(textures.join("road.png"))
            .unwrap();
        fs::write(textures.join("own.dds"), "the author's own").unwrap();
        fs::write(textures.join("rock.png"), "set aside").unwrap();
        let mut hidden = Hidden::read(&app.hidden_file());
        hidden.hide(&textures.join("rock.png")).unwrap();
        hidden.write(&app.hidden_file()).unwrap();
        let id = app.texture_rows().rows[0].id;
        let stop = AtomicBool::new(false);
        app.convert_textures(&[id], false, &stop, &|_| {});
        assert!(textures.join("road.dds").exists());

        app.take_back_own_files(id);

        let mut left: Vec<String> = fs::read_dir(&textures)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["own.dds", "road.png", "rock.png"]);
    }

    #[test]
    fn a_mod_left_aside_by_a_crash_is_back_when_the_library_is_read() {
        let (tmp, app) = installed();
        let mods = tmp.path().join("game/Mods");
        fs::rename(mods.join("roads"), mods.join("roads.rimstack-old")).unwrap();
        fs::create_dir_all(mods.join("roads.rimstack-new/About")).unwrap();
        app.settle_downloads();
        let loaded = app.library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();
        let folders: Vec<&str> = library.ids().map(|(_, m)| m.folder.as_str()).collect();
        assert!(folders.contains(&"roads"), "{folders:?}");
        assert!(!mods.join("roads.rimstack-old").exists());
        assert!(!mods.join("roads.rimstack-new").exists());
    }

    /// A tiny Workshop mod (719 bytes when chosen); SteamCMD is installed into the scratch folder.
    #[test]
    #[ignore = "needs the network, Linux, a scratch HOME and RIMSTACK_SCRATCH: an empty folder"]
    fn the_list_is_downloaded_by_steamcmd_into_the_games_mods() {
        use crate::workspace::App;
        use rimstack_core::rimworld::paths;
        const TINY: &str = "3223946831";
        let scratch = std::path::PathBuf::from(std::env::var_os("RIMSTACK_SCRATCH").unwrap());
        assert_eq!(
            std::env::var_os("HOME").map(Into::into),
            Some(scratch.clone())
        );
        let core = scratch.join("game/Data/Core/About");
        fs::create_dir_all(&core).unwrap();
        fs::write(
            core.join("About.xml"),
            "<ModMetaData><packageId>Ludeon.RimWorld</packageId></ModMetaData>",
        )
        .unwrap();
        fs::create_dir_all(scratch.join("game/Mods")).unwrap();
        fs::write(scratch.join("game/Version.txt"), "1.6.4871 rev598").unwrap();
        fs::create_dir_all(scratch.join("config/Config")).unwrap();
        let given = paths::Overrides {
            game: Some(scratch.join("game")),
            config: Some(scratch.join("config")),
            workshop: Some(scratch.join("workshop")),
        };
        let app = App::new(
            scratch.join("home"),
            &scratch.join("settings"),
            &scratch.join("data"),
            given,
        );
        let installed = app.install_steamcmd();
        println!("install: {:?}", installed.note);
        installed.note.unwrap();
        assert!(app.steamcmd().is_some());
        let added = app.add_to_list(TINY, false);
        println!("added: {:?}", added.added);
        assert_eq!(app.listed_ids(), [TINY]);
        let done = app.download_listed();
        println!("download: {:?}", done.note);
        done.note.unwrap();
        let mod_folder = scratch.join("game/Mods").join(TINY);
        assert!(mod_folder.join("About/About.xml").is_file());
        let noted = fs::read_to_string(mod_folder.join("About/PublishedFileId.txt")).unwrap();
        assert_eq!(noted.trim(), TINY);
        assert!(app.listed_ids().is_empty());
        let recorded = app.copies().get(TINY).unwrap().have;
        assert!(
            recorded.manifest != 0 && recorded.updated != 0,
            "{recorded:?}"
        );
    }
}
