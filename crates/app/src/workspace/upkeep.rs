//! Folders are found from the mod's id; the window never names a path to act on.

use super::App;
use rimstack_core::base::error::Error;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::{Installed, ModId};
use rimstack_core::rimworld::paths::Source;
use rimstack_core::sources::maintenance;
use rimstack_core::{said, say, say_n};
use serde::Serialize;
use std::path::{Path, PathBuf};

impl App {
    fn installed_mod<T>(&self, id: ModId, with: impl FnOnce(&Installed) -> T) -> Result<T, Error> {
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let found = library.get(id).ok_or_else(|| {
            Error::refused(said!(
                "That mod has moved since it was read; read the folders again"
            ))
        })?;
        Ok(with(found))
    }

    pub fn bin_mod(&self, id: ModId) -> Result<(), Error> {
        let folder = self.bin_target(id)?;
        trash::delete(&folder).map_err(|problem| Error::file(&folder, problem))
    }

    /// The game's own mods and those in the load order being edited are refused, but for a
    /// local copy whose Workshop subscription then takes its place.
    fn bin_target(&self, id: ModId) -> Result<PathBuf, Error> {
        let active = self.session().current().to_vec();
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let replaced = library.copies(id).is_some_and(|c| c.local == id);
        self.installed_mod(id, |m| {
            let in_use = m
                .key()
                .is_some_and(|key| active.iter().any(|a| a.eq_ignore_ascii_case(&key)));
            match (m.source, in_use && !replaced) {
                (Source::Official, _) => Err(Error::refused(said!("The game's own mods stay"))),
                (_, true) => Err(Error::refused(said!("Deactivate it first"))),
                _ => Ok(m.path.clone()),
            }
        })?
    }

    /// The Workshop mods among `ids` that have no copy in the game's Mods folder yet, with the
    /// words to ask before copying them.
    pub fn copy_offer(&self, ids: &[ModId]) -> CopyOffer {
        let loaded = self.library();
        let (Ok((library, _)), Some(local)) = (loaded.as_ref(), self.paths().local_mods()) else {
            return CopyOffer::default();
        };
        let copyable: Vec<(ModId, &Installed)> = library
            .to_copy(ids)
            .into_iter()
            .filter_map(|(id, m)| Some((id, m.ok()?)))
            .collect();
        let into = Line::path(&local);
        let (title, lines) = match copyable.as_slice() {
            [(_, one)] => (
                say!("Copy {} to the game's Mods folder", one.display_name()),
                vec![
                    Line::Said(say!("From")),
                    Line::path(&one.path),
                    Line::Said(say!("Into")),
                    into,
                    Line::Said(say!(
                        "The Workshop copy stays where it is; Steam keeps updating that one, not this."
                    )),
                ],
            ),
            many => (
                say_n!(
                    many.len(),
                    "Copy {} mod to the game's Mods folder",
                    "Copy {} mods to the game's Mods folder"
                ),
                vec![
                    Line::Said(say!("Into")),
                    into,
                    Line::Said(say!(
                        "The Workshop copies stay where they are; Steam keeps updating those, not these."
                    )),
                ],
            ),
        };
        CopyOffer {
            ids: copyable.iter().map(|(id, _)| *id).collect(),
            title,
            lines,
        }
    }

    /// Each is tried, whatever became of the others.
    pub fn copy_to_local(&self, ids: &[ModId]) -> Result<(), Error> {
        let local = self
            .paths()
            .local_mods()
            .ok_or_else(|| Error::refused(said!("The game's folder is not known")))?;
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let failed: Vec<Error> = library
            .to_copy(ids)
            .into_iter()
            .filter_map(|(_, m)| {
                m.and_then(|m| maintenance::copy_to_local(&m.path, &m.folder, &local))
                    .err()
            })
            .collect();
        match failed.first() {
            None => Ok(()),
            Some(only) if ids.len() == 1 => Err(only.clone()),
            Some(first) => Err(Error::new(
                first.kind(),
                said!(
                    "{} of {} not copied: {}",
                    failed.len(),
                    ids.len(),
                    first.within()
                ),
            )),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CopyOffer {
    pub ids: Vec<ModId>,
    pub title: Sentence,
    /// A text or a path, as the window's question shows them.
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Line {
    Said(Sentence),
    Path { path: String },
}

impl Line {
    fn path(path: &Path) -> Self {
        Line::Path {
            path: path.to_string_lossy().into_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::workspace::tests::installed;
    use rimstack_core::base::error::Kind;
    use rimstack_core::mods::library::ModId;
    use rimstack_core::rimworld::paths::Source;

    fn by_folder(app: &crate::App, folder: &str) -> ModId {
        let loaded = app.library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();

        library.ids().find(|(_, m)| m.folder == folder).unwrap().0
    }

    #[test]
    fn the_game_s_own_mods_and_active_ones_are_never_binned() {
        let (tmp, app) = installed();
        let core = by_folder(&app, "Core");
        assert_eq!(app.bin_mod(core).unwrap_err().kind(), Kind::Refused);
        assert!(tmp.path().join("game/Data/Core").is_dir());
        app.session()
            .record(vec!["ludeon.rimworld".into(), "ann.roads".into()]);
        let roads = by_folder(&app, "roads");
        assert_eq!(app.bin_mod(roads).unwrap_err().kind(), Kind::Refused);
        assert!(tmp.path().join("game/Mods/roads").is_dir());
        assert_eq!(
            app.bin_mod(ModId::new(0, 99)).unwrap_err().kind(),
            Kind::Refused
        );
    }

    #[test]
    fn an_active_local_copy_may_be_binned_when_its_workshop_subscription_takes_its_place() {
        let (tmp, app) = installed();
        std::fs::create_dir_all(tmp.path().join("workshop/123/About")).unwrap();
        std::fs::write(
            tmp.path().join("workshop/123/About/About.xml"),
            "<ModMetaData><packageId>Ann.Roads</packageId></ModMetaData>",
        )
        .unwrap();
        app.rescan();
        app.session()
            .record(vec!["ludeon.rimworld".into(), "ann.roads".into()]);
        let roads = by_folder(&app, "roads");
        assert_eq!(
            app.bin_target(roads).unwrap(),
            tmp.path().join("game/Mods/roads")
        );
        let subscribed = by_folder(&app, "123");
        assert_eq!(
            app.bin_target(subscribed).unwrap_err().kind(),
            Kind::Refused
        );
    }

    fn subscribe(root: &std::path::Path, id: &str, package_id: &str) {
        let about = root.join("workshop").join(id).join("About");
        std::fs::create_dir_all(&about).unwrap();
        let xml = format!("<ModMetaData><packageId>{package_id}</packageId></ModMetaData>");
        std::fs::write(about.join("About.xml"), xml).unwrap();
    }

    fn subscribed(app: &crate::App, folder: &str) -> ModId {
        let loaded = app.library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();
        let mut ids = library.ids();
        ids.find(|(_, m)| m.source == Source::Workshop && m.folder == folder)
            .unwrap()
            .0
    }

    #[test]
    fn a_mod_is_copied_into_the_game_s_folder_once_with_its_workshop_id() {
        let (tmp, app) = installed();
        subscribe(tmp.path(), "123", "Ann.Hats");
        app.rescan();
        let hats = subscribed(&app, "123");
        app.copy_to_local(&[hats]).unwrap();
        let copy = tmp.path().join("game/Mods/123");
        assert_eq!(
            std::fs::read_to_string(copy.join("About/PublishedFileId.txt")).unwrap(),
            "123"
        );
        assert!(
            !tmp.path()
                .join("workshop/123/About/PublishedFileId.txt")
                .exists()
        );
        assert_eq!(
            app.copy_to_local(&[hats]).unwrap_err().kind(),
            Kind::Refused
        );
        app.rescan();
        assert!(app.copy_offer(&[subscribed(&app, "123")]).ids.is_empty());
    }

    #[test]
    fn of_a_selection_only_workshop_mods_without_a_copy_are_offered_and_each_is_tried() {
        let (tmp, app) = installed();
        subscribe(tmp.path(), "123", "Ann.Hats");
        subscribe(tmp.path(), "456", "Ann.Roads");
        subscribe(tmp.path(), "789", "Ann.Boots");
        app.rescan();
        let (hats, roads, boots) = (
            subscribed(&app, "123"),
            subscribed(&app, "456"),
            subscribed(&app, "789"),
        );
        let (core, local_roads) = (by_folder(&app, "Core"), by_folder(&app, "roads"));
        let offer = app.copy_offer(&[core, local_roads, hats, roads, boots]);
        assert_eq!(offer.ids, [hats, boots]);
        assert_eq!(
            offer.title.to_string(),
            "Copy 2 mods to the game's Mods folder"
        );
        assert_eq!(
            app.copy_offer(&[hats, roads]).title.to_string(),
            "Copy Ann.Hats to the game's Mods folder"
        );
        std::fs::create_dir_all(tmp.path().join("game/Mods/789")).unwrap();
        let problem = app.copy_to_local(&[hats, boots, roads, core]).unwrap_err();
        assert_eq!(problem.kind(), Kind::Refused);
        assert!(
            problem.to_string().starts_with("3 of 4 not copied: "),
            "{problem}"
        );
        assert!(tmp.path().join("game/Mods/123/About/About.xml").is_file());
        assert!(!tmp.path().join("game/Mods/456").exists());
    }
}
