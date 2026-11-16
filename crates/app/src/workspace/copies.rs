use super::App;
use super::fetch::Done;
use rimstack_core::base::error::{Error, Result};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::kept::copies::Copies;
use rimstack_core::mods::library::Library;
use rimstack_core::sources::downloads::{self, OnDisk, Origin};
use rimstack_core::sources::{redownload, rimsort_steamcmd};
use rimstack_core::{said, say_n};
use std::collections::HashMap;
use std::path::PathBuf;

pub type Outcomes = Vec<(String, Result<PathBuf>)>;

const BATCH: usize = 25;

/// Something was written while any one succeeded; the first failure is told.
pub(super) fn done_of(
    outcomes: &Outcomes,
    done: Sentence,
    not_done: impl FnOnce(usize, &Error) -> Sentence,
) -> Done {
    let failed: Vec<&Error> = outcomes
        .iter()
        .filter_map(|(_, o)| o.as_ref().err())
        .collect();
    Done {
        wrote: failed.len() < outcomes.len(),
        note: match failed.first() {
            None => Ok(done),
            Some(first) => Err(Error::new(first.kind(), not_done(failed.len(), first))),
        },
    }
}

impl App {
    fn copies_file(&self) -> PathBuf {
        self.data_dir().join("steamcmd-copies.json")
    }

    pub fn copies(&self) -> Copies {
        Copies::read(&self.copies_file())
    }

    fn keep_copies(&self, copies: &Copies) {
        if let Err(problem) = copies.write(&self.copies_file()) {
            rimstack_core::problem!("{problem}");
        }
    }

    /// The copies in the game's Mods folder whose version is known, by Workshop id; RimSort's
    /// record is read only for those RimStack did not download.
    pub(super) fn copies_known(&self, library: &Library, kept: &Copies) -> OnDisk {
        let copies: Vec<&String> = library
            .loadable()
            .filter(|(_, m)| downloads::is_copy(m))
            .map(|(_, m)| &m.folder)
            .collect();
        let rimsort = match self.paths().local_mods() {
            Some(local) if copies.iter().any(|id| kept.get(id).is_none()) => {
                rimsort_steamcmd::versions(&self.rimsort_dir(), &local)
            }
            _ => HashMap::new(),
        };
        let known = |id: &str| match kept.get(id) {
            Some(copy) => Some((copy.have, Origin::SteamCmd)),
            None => Some((*rimsort.get(id)?, Origin::Linked)),
        };
        copies
            .into_iter()
            .filter_map(|id| Some((id.clone(), known(id)?)))
            .collect()
    }

    pub(super) fn steamcmd_and_local(&self) -> Result<(PathBuf, PathBuf)> {
        let steamcmd = (*self.steamcmd()).clone();
        match (steamcmd, self.paths().local_mods()) {
            (Some(steamcmd), Some(local)) => Ok((steamcmd, local)),
            (None, _) => Err(Error::refused(said!("SteamCMD is not installed"))),
            (_, None) => Err(Error::refused(said!("The game's folder is not known"))),
        }
    }

    /// The one way SteamCMD puts a mod in the game's Mods folder: what it downloaded is
    /// recorded, so Check now follows it, and a copy that failed is not tried again by itself
    /// until Steam offers another version.
    pub(super) fn fetch_into_local(&self, ids: &[String]) -> Result<Outcomes> {
        let (steamcmd, local) = self.steamcmd_and_local()?;
        let scratch = self.data_dir().join("steamcmd-downloads");
        let offered = self.last_check().offered();
        let mut copies = self.copies();
        let mut outcomes = Outcomes::new();
        // One run is stopped at `program::DOWNLOAD`, and what it fetched is placed only once it
        // ends: a run of a few hundred mods could lose them all.
        for batch in ids.chunks(BATCH) {
            let fetched = redownload::fetch_all(&steamcmd, batch, &scratch);
            let versions = fetched.versions();
            for (id, fresh) in &fetched.items {
                let fresh = fresh.as_ref().map_err(Clone::clone);
                let put = fresh.and_then(|fresh| redownload::put_in_local(fresh, id, &local));
                match (&put, versions.get(id), offered.get(id)) {
                    (Ok(_), Some(have), _) => copies.downloaded(id, *have),
                    (Err(problem), _, Some(offered)) => {
                        let why = problem.sentence().to_kept().to_string();
                        copies.refuse(id, why, *offered);
                    }
                    _ => {}
                }
                outcomes.push((id.clone(), put));
            }
            self.keep_copies(&copies);
        }
        Ok(outcomes)
    }

    pub fn update_copies(&self, ids: &[String]) -> Done {
        let outcomes = match self.fetch_into_local(ids) {
            Ok(outcomes) => outcomes,
            Err(problem) => return Done::told(Err(problem)),
        };
        self.publish(crate::changes::Changes::UPDATES);
        let n = outcomes.len();
        done_of(
            &outcomes,
            say_n!(n, "{} mod updated", "{} mods updated"),
            |failed, first| said!("{} of {} not updated: {}", failed, n, first.within()),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::workspace::tests::installed;
    use rimstack_core::kept::updates::LastCheck;
    use rimstack_core::rimworld::workshop::Version;
    use rimstack_core::sources::downloads::Origin;
    use std::collections::{BTreeMap, BTreeSet};

    fn at(manifest: u64, updated: u64) -> Version {
        Version { manifest, updated }
    }

    #[test]
    fn a_copy_rimstack_downloaded_is_checked_by_its_manifest_and_others_are_listed() {
        let (tmp, app) = installed();
        for (id, name) in [
            ("3200000001", "Epsilon"),
            ("3200000002", "Zeta"),
            ("3200000003", "Eta"),
        ] {
            let about = tmp.path().join("game/Mods").join(id).join("About");
            std::fs::create_dir_all(&about).unwrap();
            let xml = format!(
                "<ModMetaData><packageId>f.{name}</packageId><name>{name}</name></ModMetaData>"
            );
            std::fs::write(about.join("About.xml"), xml).unwrap();
        }
        let mut copies = app.copies();
        copies.downloaded("3200000001", at(1, 10));
        app.keep_copies(&copies);
        app.rescan();
        let kept = LastCheck {
            checked: Some(20),
            now: BTreeMap::from([("3200000001".into(), 10), ("3200000002".into(), 10)]),
            manifests: BTreeMap::from([("3200000001".into(), 2)]),
            unanswered: BTreeSet::from(["3200000003".to_string()]),
            ..LastCheck::default()
        };
        let loaded = app.library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();
        let report = app.updates_of(library, &kept);
        let behind: Vec<(&str, Origin)> = report
            .items
            .iter()
            .map(|r| (r.id.as_str(), r.origin))
            .collect();
        assert_eq!(
            behind,
            [("3200000001", Origin::SteamCmd)],
            "a new upload at the same time"
        );
        assert_eq!(report.copies_by_itself().count(), 1);
        let untracked: Vec<&str> = report.untracked.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(untracked, ["Zeta"], "Eta is no longer on the Workshop");
        let unanswered: Vec<&str> = report.unanswered.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(unanswered, ["Eta"]);

        let mut copies = app.copies();
        copies.refuse("3200000001", "Timeout".into(), at(2, 10));
        app.keep_copies(&copies);
        let report = app.updates_of(library, &kept);
        assert_eq!(report.refused["3200000001"].to_string(), "Timeout");
        assert_eq!(
            report.copies_by_itself().count(),
            0,
            "not tried again by itself"
        );
        assert_eq!(report.by_steamcmd().count(), 1, "still offered on request");
    }
}
