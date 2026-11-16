use super::{Installed, Library, ModId};
use crate::base::error::Error;
use crate::base::versions::{latest, newer};
use crate::rimworld::paths::Source;
use crate::said;
use std::collections::HashMap;
use std::path::Path;

/// A mod in the game's Mods folder that is also a Workshop subscription: the game loads the
/// local one, often a copy left from SteamCMD or "Copy to the game folder".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Copies {
    pub local: ModId,
    pub workshop: ModId,
    pub workshop_newer: bool,
}

impl Library {
    pub fn copies(&self, id: ModId) -> Option<Copies> {
        let m = self.get(id)?;
        let key = m.key()?;
        let other = match m.source {
            Source::Local => Source::Workshop,
            Source::Workshop => Source::Local,
            Source::Official => return None,
        };
        let (found, _) = self
            .ids()
            .find(|(_, x)| x.source == other && x.key().as_ref() == Some(&key))?;
        let (local, workshop) = match m.source {
            Source::Local => (id, found),
            _ => (found, id),
        };
        Some(Copies {
            local,
            workshop,
            workshop_newer: workshop_newer(self.get(local)?, self.get(workshop)?),
        })
    }
}

impl Library {
    /// Each of `ids` that may be copied into the game's Mods folder, or why not: a Workshop mod
    /// may, unless a mod of its package id is there already.
    pub fn to_copy(&self, ids: &[ModId]) -> Vec<(ModId, Result<&Installed, Error>)> {
        let local: HashMap<String, &Path> = self
            .ids()
            .filter(|(_, m)| m.source == Source::Local)
            .filter_map(|(_, m)| Some((m.key()?, m.path.as_path())))
            .collect();
        let judged = |id: ModId| match self.get(id) {
            None => Err(Error::refused(said!(
                "That mod has moved since it was read; read the folders again"
            ))),
            Some(m) if m.source != Source::Workshop => {
                Err(Error::refused(said!("Not a Workshop mod")))
            }
            Some(m) => match m.key().and_then(|key| local.get(&key)) {
                Some(there) => Err(Error::refused(said!(
                    "{} is there already",
                    there.display()
                ))),
                None => Ok(m),
            },
        };
        ids.iter().map(|&id| (id, judged(id))).collect()
    }
}

/// By `modVersion` when both give one and they differ, which few do; else by the latest game
/// version each supports, since an author may add a game's folder without raising it.
fn workshop_newer(local: &Installed, workshop: &Installed) -> bool {
    let (Ok(local), Ok(workshop)) = (&local.about, &workshop.about) else {
        return false;
    };
    if let (Some(was), Some(is)) = (&local.mod_version, &workshop.mod_version)
        && (newer(was, is) || newer(is, was))
    {
        return newer(was, is);
    }
    match (
        latest(&local.supported_versions),
        latest(&workshop.supported_versions),
    ) {
        (Some(was), Some(is)) => newer(was, is),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::library::{build, scan};
    use crate::rimworld::paths::ModRoot;
    use std::path::Path;

    fn add(root: &Path, folder: &str, id: &str, extra: &str) {
        let about = root.join(folder).join("About");
        std::fs::create_dir_all(&about).unwrap();
        let xml = format!("<ModMetaData><packageId>{id}</packageId>{extra}</ModMetaData>");
        std::fs::write(about.join("About.xml"), xml).unwrap();
    }

    fn library(local: &str, workshop: &str) -> (tempfile::TempDir, Library) {
        let tmp = tempfile::tempdir().unwrap();
        let roots = [
            (tmp.path().join("Mods"), Source::Local),
            (tmp.path().join("workshop"), Source::Workshop),
        ]
        .map(|(path, source)| ModRoot { path, source });
        add(&roots[0].path, "3535", "Ann.Progress", local);
        add(&roots[1].path, "3535", "ann.progress", workshop);
        add(&roots[1].path, "77", "Ann.Other", "");
        let library = build(scan(&roots), &["ann.progress".into()], "1.7");
        (tmp, library)
    }

    fn by(library: &Library, source: Source, folder: &str) -> ModId {
        let mut ids = library.ids();
        ids.find(|(_, m)| m.source == source && m.folder == folder)
            .unwrap()
            .0
    }

    fn versions(mod_version: &str, game: &[&str]) -> String {
        let game: String = game.iter().map(|v| format!("<li>{v}</li>")).collect();
        format!(
            "<modVersion>{mod_version}</modVersion><supportedVersions>{game}</supportedVersions>"
        )
    }

    #[test]
    fn a_local_copy_and_its_workshop_subscription_find_each_other() {
        let (_tmp, library) = library(
            &versions("0.18.1", &["1.6"]),
            &versions("0.20.0", &["1.6", "1.7"]),
        );
        let (local, workshop) = (
            by(&library, Source::Local, "3535"),
            by(&library, Source::Workshop, "3535"),
        );
        let copies = Copies {
            local,
            workshop,
            workshop_newer: true,
        };
        assert_eq!(library.copies(local), Some(copies));
        assert_eq!(library.copies(workshop), Some(copies));
        assert_eq!(library.copies(by(&library, Source::Workshop, "77")), None);
    }

    #[test]
    fn only_a_workshop_mod_without_a_local_copy_may_be_copied() {
        let (_tmp, library) = library("", "");
        let (local, workshop, other) = (
            by(&library, Source::Local, "3535"),
            by(&library, Source::Workshop, "3535"),
            by(&library, Source::Workshop, "77"),
        );
        let judged = library.to_copy(&[local, workshop, other, ModId::new(0, 99)]);
        let ok: Vec<bool> = judged.iter().map(|(_, m)| m.is_ok()).collect();
        assert_eq!(ok, [false, false, true, false]);
        assert_eq!(judged[2].1.as_ref().unwrap().folder, "77");
    }

    #[test]
    fn the_workshop_copy_is_newer_by_its_own_version_else_by_the_game_s() {
        let newer = |local: &str, workshop: &str| {
            let (_tmp, library) = library(local, workshop);
            let copies = library.copies(by(&library, Source::Local, "3535"));
            copies.unwrap().workshop_newer
        };
        assert!(newer(
            &versions("0.9", &["1.7"]),
            &versions("0.10", &["1.6"])
        ));
        assert!(!newer(
            &versions("0.20.1", &["1.6"]),
            &versions("0.20.0", &["1.7"])
        ));
        assert!(newer(
            &versions("0.20.0", &["1.6"]),
            &versions("0.20", &["1.6", "1.7"])
        ));
        let game = |versions: &[&str]| {
            self::versions("", versions).replace("<modVersion></modVersion>", "")
        };
        assert!(newer(&game(&["1.5", "1.6"]), &game(&["1.6", "1.7"])));
        assert!(!newer(&game(&["1.7"]), &game(&["1.7"])));
        assert!(!newer("", ""));
    }
}
