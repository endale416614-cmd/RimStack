use super::App;
use rimstack_core::base::elapsed::ago;
use rimstack_core::base::error::Error;
use rimstack_core::base::locks::Shared;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::kept::settings::Switch;
use rimstack_core::kept::updates::{self, LastCheck, Remote};
use rimstack_core::mods::library::{Installed, Library, ModId};
use rimstack_core::rimworld::paths::Source;
use rimstack_core::rimworld::workshop::{self, Version};
use rimstack_core::sources::downloads::{
    self, Asked, Behind, Named, OnDisk, Origin, Replaceable, Repository,
};
use rimstack_core::{say, say_n};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Checkout {
    pub mod_id: ModId,
    pub name: String,
    pub workshop: bool,
    pub checked: bool,
    pub behind: Option<u64>,
    pub problem: Option<Sentence>,
    pub refused: Option<Sentence>,
}

impl Checkout {
    pub(crate) fn due(&self) -> bool {
        !self.failed() && self.behind.is_some_and(|n| n > 0)
    }

    fn failed(&self) -> bool {
        self.problem.is_some() || self.refused.is_some()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutShown<'a> {
    #[serde(flatten)]
    pub row: &'a Checkout,
    pub said: Sentence,
    pub due: bool,
    /// The colour shown for it, by index: 0 not checked, 1 up to date, 2 behind, 3 failed.
    pub standing: u8,
    pub updatable: bool,
}

impl<'a> CheckoutShown<'a> {
    fn of(row: &'a Checkout) -> Self {
        let said = match (&row.refused, &row.problem, row.checked, row.behind) {
            (Some(refused), _, _, _) => say!("Not updated: {}", refused.inside()),
            (None, Some(problem), _, _) => say!("Not checked: {}", problem.inside()),
            (None, None, false, _) => say!("Not checked"),
            (None, None, true, Some(n)) if n > 0 => {
                let n = usize::try_from(n).unwrap_or(usize::MAX);
                say_n!(n, "{} commit behind", "{} commits behind")
            }
            (None, None, true, _) => say!("Up to date"),
        };
        CheckoutShown {
            row,
            said,
            due: row.due(),
            standing: match (row.failed(), row.due(), row.checked) {
                (true, _, _) => 3,
                (false, true, _) => 2,
                (false, false, true) => 1,
                (false, false, false) => 0,
            },
            updatable: !row.workshop && (row.due() || row.refused.is_some()),
        }
    }
}

#[derive(Default)]
pub struct Updates {
    pub checked: Option<u64>,
    pub asked: Option<u64>,
    pub answered: usize,
    pub items: Vec<Behind>,
    /// The copies RimStack downloaded whose last update failed, held back until Steam offers
    /// another version.
    pub refused: HashMap<String, Sentence>,
    pub untracked: Vec<Named>,
    pub unanswered: Vec<Named>,
    pub checkouts: Vec<Checkout>,
    pub problem: Option<Sentence>,
}

impl Updates {
    pub fn by_steam(&self) -> impl Iterator<Item = &Behind> {
        self.items.iter().filter(|row| row.origin == Origin::Steam)
    }

    pub fn by_steamcmd(&self) -> impl Iterator<Item = &Behind> {
        self.items.iter().filter(|row| row.origin != Origin::Steam)
    }

    /// The copies RimStack downloaded that may be updated without asking: those that did not
    /// just fail.
    pub fn copies_by_itself(&self) -> impl Iterator<Item = &Behind> {
        let ours = |row: &&Behind| row.origin == Origin::SteamCmd;
        let held = |row: &&Behind| self.refused.contains_key(&row.id);
        self.items.iter().filter(ours).filter(move |row| !held(row))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BehindShown<'a> {
    #[serde(flatten)]
    pub row: &'a Behind,
    pub asked: bool,
    pub refused: Option<&'a Sentence>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatesShown<'a> {
    pub checked: Option<u64>,
    pub checked_said: Sentence,
    pub problem: Option<&'a Sentence>,
    pub count: usize,
    pub active: Vec<BehindShown<'a>>,
    pub inactive: Vec<BehindShown<'a>>,
    pub untracked: &'a [Named],
    pub unanswered: &'a [Named],
    pub checkouts: Vec<CheckoutShown<'a>>,
    pub checkouts_failed: Vec<CheckoutShown<'a>>,
    pub checkouts_due: Vec<CheckoutShown<'a>>,
    pub checkouts_said: Sentence,
}

pub fn updates_shown<'a>(updates: &'a Updates, asked: &HashSet<String>) -> UpdatesShown<'a> {
    let shown = |active: bool| {
        let rows = updates.items.iter().filter(move |row| row.active == active);
        rows.map(|row| BehindShown {
            row,
            asked: row.origin == Origin::Steam && asked.contains(&row.id),
            refused: updates.refused.get(&row.id),
        })
        .collect()
    };
    let checkouts = |which: &dyn Fn(&Checkout) -> bool| {
        let rows = updates.checkouts.iter().filter(|c| which(c));
        rows.map(CheckoutShown::of).collect()
    };
    UpdatesShown {
        checked: updates.checked,
        checked_said: match updates.checked {
            Some(at) => say!(
                "Checked {}",
                ago(super::now_secs().saturating_sub(at)).inside()
            ),
            None => say!("Never checked"),
        },
        problem: updates.problem.as_ref(),
        count: updates.items.len(),
        active: shown(true),
        inactive: shown(false),
        untracked: &updates.untracked,
        unanswered: &updates.unanswered,
        checkouts: updates.checkouts.iter().map(CheckoutShown::of).collect(),
        checkouts_failed: checkouts(&Checkout::failed),
        checkouts_due: checkouts(&Checkout::due),
        checkouts_said: match updates.checkouts.iter().filter(|c| c.due()).count() {
            0 => say!("all up to date"),
            due => say!("{} behind", due),
        },
    }
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Queue {
    /// What is not installed yet, those Steam said nothing of among them.
    pub items: Vec<Asked>,
    pub installed: Vec<Asked>,
    pub collections: Vec<String>,
    pub repositories: Vec<Repository>,
    pub note: Option<Sentence>,
    pub wanted: Vec<String>,
}

impl App {
    fn checked_file(&self) -> PathBuf {
        self.data_dir().join("updates.json")
    }

    pub fn last_check(&self) -> LastCheck {
        LastCheck::read(&self.checked_file())
    }

    fn keep_check(&self, check: &LastCheck) {
        if let Err(problem) = check.write(&self.checked_file()) {
            rimstack_core::problem!("{problem}");
        }
    }

    /// Last time's answer stays when Steam is not reached.
    pub fn check_updates(&self) -> Result<Updates, Error> {
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let mut kept = self.last_check();
        let in_steam = library
            .workshop_mods()
            .into_keys()
            .filter(|f| workshop::is_id(f));
        let copies = library.loadable().filter(|(_, m)| downloads::is_copy(m));
        let ids: BTreeSet<&str> = in_steam
            .chain(copies.map(|(_, m)| m.folder.as_str()))
            .collect();
        let ids: Vec<String> = ids.into_iter().map(str::to_string).collect();
        let problem = match workshop::fetch_details(&ids) {
            Ok(now) => {
                kept.answered(&ids, &now, super::now_secs());
                None
            }
            Err(problem) => Some(problem.sentence().clone()),
        };
        let before = std::mem::replace(&mut kept.checkouts, self.checkouts_asked(library));
        kept.keep_refusals(&before);
        self.keep_check(&kept);
        let mut report = self.updates_of(library, &kept);
        report.problem = problem;
        Ok(report)
    }

    /// `None` if there was none.
    pub fn kept_updates(&self) -> Option<Updates> {
        let kept = self.last_check();
        if kept == LastCheck::default() {
            return None;
        }
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().ok()?;
        Some(self.updates_of(library, &kept))
    }

    pub fn checkout_updated(&self, folder: &Path) {
        let mut kept = self.last_check();
        let caught_up = Remote {
            behind: Some(0),
            ..Remote::default()
        };
        let folder = folder.to_string_lossy().into_owned();
        kept.checkouts.insert(folder, caught_up);
        self.keep_check(&kept);
    }

    pub fn checkout_refused(&self, folder: &Path, why: &Sentence) {
        let mut kept = self.last_check();
        let folder = folder.to_string_lossy().into_owned();
        let remote = kept.checkouts.entry(folder).or_default();
        remote.refused = Some(why.to_kept().to_string());
        self.keep_check(&kept);
    }

    pub(super) fn updates_of(&self, library: &Library, kept: &LastCheck) -> Updates {
        let record = self.steam_record();
        let in_steam = |(id, &updated): (&String, &u64)| {
            let manifest = record.manifests.get(id).copied().unwrap_or(0);
            (id.clone(), (Version { manifest, updated }, Origin::Steam))
        };
        let in_steam: OnDisk = record.times.iter().map(in_steam).collect();
        let kept_copies = self.copies();
        let known = self.copies_known(library, &kept_copies);
        let now = kept.offered();
        let active = self.active_now(library);
        let (items, copies) = (
            downloads::items(library, &active),
            downloads::copies(library, &active),
        );
        let behind = downloads::behind(&[(&items, &in_steam), (&copies, &known)], &now);
        if kept.checked.is_some() {
            *self.behind().writing() = Some(behind.iter().map(|row| row.id.clone()).collect());
        }
        let refused = behind
            .iter()
            .filter(|row| row.origin == Origin::SteamCmd)
            .filter_map(|row| {
                let why = kept_copies.held_back(&row.id, row.offered)?;
                Some((row.id.clone(), Sentence::from_kept(why)))
            })
            .collect();
        Updates {
            checked: kept.checked,
            asked: kept.asked,
            answered: now.len(),
            items: behind,
            refused,
            untracked: downloads::untracked(&copies, &known, &kept.unanswered),
            unanswered: downloads::unanswered([&items, &copies], &kept.unanswered),
            checkouts: self.checkouts_of(library, &kept.checkouts),
            problem: None,
        }
    }

    fn asked_of_git<'a>(&self, library: &'a Library) -> Vec<(ModId, &'a Installed)> {
        let git_for_workshop = self.settings().switch(Switch::GitForWorkshop);
        library
            .ids()
            .filter(|(_, m)| updates::asks_git(m.source, m.git, git_for_workshop))
            .collect()
    }

    fn checkouts_asked(&self, library: &Library) -> BTreeMap<String, Remote> {
        use rayon::prelude::*;
        let asked = self.asked_of_git(library);
        let answers: Vec<(String, Remote)> = asked
            .par_iter()
            .map(|(_, m)| {
                let said = match rimstack_core::sources::git::behind(&m.path) {
                    Ok(count) => Remote {
                        behind: Some(count as u64),
                        ..Remote::default()
                    },
                    // Kept as a sentence, put in words when it is shown.
                    Err(problem) => Remote {
                        problem: Some(problem.sentence().to_kept().to_string()),
                        ..Remote::default()
                    },
                };
                (m.path.to_string_lossy().into_owned(), said)
            })
            .collect();
        answers.into_iter().collect()
    }

    fn checkouts_of(&self, library: &Library, answers: &BTreeMap<String, Remote>) -> Vec<Checkout> {
        let asked = self.asked_of_git(library).into_iter();
        asked
            .map(|(id, m)| {
                let said = answers.get(m.path.to_string_lossy().as_ref());
                Checkout {
                    mod_id: id,
                    name: m.display_name().to_string(),
                    workshop: m.source == Source::Workshop,
                    checked: said.is_some(),
                    behind: said.and_then(|s| s.behind),
                    problem: said.and_then(|s| s.problem.as_deref().map(Sentence::from_kept)),
                    refused: said.and_then(|s| s.refused.as_deref().map(Sentence::from_kept)),
                }
            })
            .collect()
    }

    pub fn look_up(&self, pasted: &str) -> Result<Queue, Error> {
        let ids = workshop::ids_in(pasted);
        let local = self.paths().local_mods();
        let repositories = downloads::repositories(pasted, local.as_deref());
        if ids.is_empty() {
            let note = match repositories.is_empty() {
                true => say!("No Workshop link, mod id or Git link found"),
                false => Sentence::default(),
            };
            return Ok(Queue {
                repositories,
                note: Some(note),
                ..Queue::default()
            });
        }
        let expanded = workshop::expand_collections(&ids)?;
        let opened: Vec<String> = ids
            .into_iter()
            .filter(|id| !expanded.contains(id))
            .collect();
        let every: Vec<String> = expanded.iter().chain(&opened).cloned().collect();
        let mut answered = workshop::fetch_details(&every)?;
        let collections = opened
            .iter()
            .filter_map(|id| answered.iter().find(|d| &d.id == id))
            .map(|d| d.title.clone())
            .collect();
        answered.retain(|details| details.refusal().is_none());
        let ids = expanded;
        self.library().as_ref().as_ref().map_err(Clone::clone)?;
        let installed = self.installed_workshop();
        let items = downloads::asked(&ids, &answered, &|id| installed.contains(id));
        let found = || items.iter().filter(|i| i.title.is_some() && !i.installed);
        let wanted: Vec<String> = found().map(|item| item.id.clone()).collect();
        let titled: Vec<(String, String)> = found()
            .filter_map(|item| Some((item.id.clone(), item.title.clone()?)))
            .collect();
        self.add_wanted_to_list(&titled);
        let (installed, items) = items.into_iter().partition(|item| item.installed);
        Ok(Queue {
            items,
            installed,
            collections,
            repositories,
            note: None,
            wanted,
        })
    }

    pub fn replaceable(&self) -> Vec<Replaceable> {
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return Vec::new();
        };
        let active = self.active_now(library);
        let items = downloads::items(library, &active);
        downloads::replaceable(&items, &self.replacements(), &self.replacements_ignored())
    }

    /// With `None`, every one set aside is suggested again.
    pub fn ignore_replacement(&self, id: Option<ModId>) -> Result<(), Error> {
        let mut ignored: Vec<String> = match id {
            Some(_) => self.replacements_ignored().into_iter().collect(),
            None => Vec::new(),
        };
        if let Some(id) = id.and_then(|id| self.workshop_id(id)) {
            ignored.push(id);
        }
        ignored.sort();
        ignored.dedup();
        self.set_setting("replacementsIgnored", &ignored.join(","))?;
        self.publish(crate::changes::Changes::SETTINGS);
        Ok(())
    }

    /// A Workshop mod's id: the name of its folder.
    pub fn workshop_id(&self, id: ModId) -> Option<String> {
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().ok()?;
        let m = library.get(id).filter(|m| m.source == Source::Workshop)?;
        Some(m.folder.clone())
    }

    pub fn mod_path(&self, id: ModId) -> Option<String> {
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().ok()?;
        Some(library.get(id)?.path.to_string_lossy().into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::installed;

    #[test]
    fn a_local_checkout_is_listed_by_its_mod_with_what_its_remote_said_last() {
        let (tmp, app) = installed();
        let roads = tmp.path().join("game/Mods/roads");
        std::fs::create_dir(roads.join(".git")).unwrap();
        app.rescan();
        let loaded = app.library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();
        let id = library.ids().find(|(_, m)| m.folder == "roads").unwrap().0;

        let report = app.updates_of(library, &LastCheck::default());
        assert_eq!(report.checkouts.len(), 1);
        let row = &report.checkouts[0];
        assert_eq!((row.mod_id, row.checked, row.behind), (id, false, None));
        let shown = updates_shown(&report, &HashSet::new());
        let (failed, due) = (shown.checkouts_failed.len(), shown.checkouts_due.len());
        assert_eq!((failed, due), (0, 0), "not asked yet");
        assert_eq!(shown.checkouts[0].said.to_string(), "Not checked");

        let mut kept = LastCheck::default();
        let remote = Remote {
            behind: Some(2),
            ..Remote::default()
        };
        kept.checkouts
            .insert(roads.to_string_lossy().into_owned(), remote);
        let report = app.updates_of(library, &kept);
        let row = &report.checkouts[0];
        assert_eq!(
            (row.checked, row.behind, row.workshop),
            (true, Some(2), false)
        );
        let shown = updates_shown(&report, &HashSet::new());
        assert_eq!(shown.checkouts_due.len(), 1);
        assert_eq!(shown.checkouts_said.to_string(), "1 behind");
        assert_eq!(crate::words::as_shown(&row)["modId"], id.to_packed());
        drop(loaded);

        app.checkout_updated(&roads);
        let caught_up = app.kept_updates().unwrap();
        assert_eq!(caught_up.checkouts[0].behind, Some(0));
        let shown = updates_shown(&caught_up, &HashSet::new());
        assert!(shown.checkouts_due.is_empty() && shown.checkouts[0].standing == 1);
        assert_eq!(shown.checkouts_said.to_string(), "all up to date");
    }

    #[test]
    fn a_local_mod_has_no_workshop_id_and_ignoring_none_clears_the_list() {
        let (_tmp, app) = installed();
        let loaded = app.library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();
        let roads = library.ids().find(|(_, m)| m.folder == "roads").unwrap().0;
        drop(loaded);
        assert_eq!(app.workshop_id(roads), None);
        app.set_setting("replacementsIgnored", "1,2").unwrap();
        app.ignore_replacement(Some(roads)).unwrap();
        assert_eq!(app.replacements_ignored().len(), 2);
        app.ignore_replacement(None).unwrap();
        assert!(app.replacements_ignored().is_empty());
    }
}
