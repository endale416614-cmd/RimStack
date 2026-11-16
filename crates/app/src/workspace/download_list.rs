use super::App;
use crate::changes::Changes;
use rimstack_core::base::error::Result;
use rimstack_core::base::locks::Held;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::base::sizes;
use rimstack_core::kept::download_list::{self, DownloadList, Row, Why};
use rimstack_core::mods::describe::{self, Fix};
use rimstack_core::rimworld::paths::Os;
use rimstack_core::rimworld::{about, id};
use rimstack_core::sources::browse::{Browse, Refusal, add_with_requirements};
use rimstack_core::{say, say_n};
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListRow {
    pub id: String,
    pub title: String,
    pub size: Option<u64>,
    pub size_said: Option<Sentence>,
    pub reason: Sentence,
    pub failed: Option<Sentence>,
}

#[derive(Serialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListAction {
    pub download: bool,
    pub install_steamcmd: bool,
    pub subscribe: bool,
    pub enabled: bool,
    pub subscribe_enabled: bool,
    pub label: Sentence,
    pub stuck: Option<Sentence>,
}

#[derive(Serialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListShown {
    pub rows: Vec<ListRow>,
    pub count: usize,
    pub size_said: Option<Sentence>,
    pub failures: bool,
    pub action: ListAction,
}

#[derive(Serialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AddShown {
    pub added: Vec<String>,
    pub said: Option<Sentence>,
    pub refusal: Option<Sentence>,
    pub unknown: Vec<String>,
    pub unknown_said: Option<Sentence>,
}

impl AddShown {
    pub fn note(&self) -> Option<(Sentence, bool)> {
        let parts: Vec<Sentence> = [&self.said, &self.refusal, &self.unknown_said]
            .into_iter()
            .flatten()
            .cloned()
            .collect();
        match parts.len() {
            0 => None,
            1 => Some((parts[0].clone(), self.added.is_empty())),
            _ => Some((Sentence::joined(". ", parts), self.added.is_empty())),
        }
    }
}

impl ListShown {
    pub fn empty() -> Self {
        Self {
            rows: Vec::new(),
            count: 0,
            size_said: None,
            failures: false,
            action: action(Os::running(), true, true, true),
        }
    }
}

pub fn action(os: Os, steam: bool, steamcmd: bool, empty: bool) -> ListAction {
    let runs = os != Os::Mac;
    let (download, install_steamcmd) = (runs && steamcmd, runs && !steamcmd);
    ListAction {
        download,
        install_steamcmd,
        subscribe: steam,
        enabled: (download && !empty) || install_steamcmd,
        subscribe_enabled: steam && !empty,
        label: if install_steamcmd {
            say!("Install SteamCMD")
        } else {
            say!("Download with SteamCMD…")
        },
        stuck: (!runs && !steam).then(|| {
            say!("RimStack cannot download on this system without Steam: open each mod's Workshop page to get it")
        }),
    }
}

pub fn vet(
    ids: &[String],
    answer: &Result<Vec<rimstack_core::rimworld::workshop::Details>>,
    in_database: &dyn Fn(&str) -> bool,
) -> (Vec<String>, Vec<(String, Sentence)>) {
    let (mut passed, mut refused) = (Vec::new(), Vec::new());
    for id in ids {
        let found = answer
            .as_ref()
            .ok()
            .and_then(|all| all.iter().find(|d| d.id == *id));
        let why = match (found, answer) {
            (Some(d), _) => d.refusal(),
            (None, Ok(_)) => Some(say!("Steam does not know this item")),
            (None, Err(_)) if in_database(id) => None,
            (None, Err(_)) => Some(say!("Steam could not be asked about this item")),
        };
        match why {
            Some(why) => refused.push((id.clone(), why)),
            None => passed.push(id.clone()),
        }
    }
    (passed, refused)
}

fn alone(
    list: &mut DownloadList,
    source: &dyn Browse,
    installed: &HashSet<String>,
    id: &str,
    why: Why,
) -> (Vec<String>, Option<Refusal>) {
    if installed.contains(id) {
        return (Vec::new(), Some(Refusal::Installed));
    }
    let title = source.entry(id).map_or(id.to_string(), |e| e.title);
    match list.add(Row::new(id, title, why)) {
        true => (vec![id.to_string()], None),
        false => (Vec::new(), Some(Refusal::OnList)),
    }
}

impl App {
    fn list(&self) -> &Mutex<DownloadList> {
        self.download_list
            .get_or_init(|| Mutex::new(DownloadList::read(&download_list::file_in(&self.data_dir))))
    }

    fn change_list<T>(&self, change: impl FnOnce(&mut DownloadList) -> T) -> T {
        let mut list = self.list().held();
        let before = list.clone();
        let out = change(&mut list);
        let changed = *list != before;
        if changed && let Err(problem) = list.write(&download_list::file_in(&self.data_dir)) {
            rimstack_core::problem!("{problem}");
        }
        drop(list);
        if changed {
            self.publish(Changes::DOWNLOAD_LIST);
        }
        out
    }

    pub fn listed_ids(&self) -> Vec<String> {
        self.list().held().ids()
    }

    pub fn list_shown(&self) -> ListShown {
        let list = self.list().held().clone();
        let titled = |id: &str| {
            let on_list = list.rows().iter().find(|row| row.id == id);
            on_list
                .map(|row| row.title.clone())
                .or_else(|| self.browse_source().entry(id).map(|e| e.title))
                .unwrap_or_else(|| id.to_string())
        };
        let rows: Vec<ListRow> = list
            .rows()
            .iter()
            .map(|row| ListRow {
                id: row.id.clone(),
                title: row.title.clone(),
                size: row.size,
                size_said: row.size.map(sizes::of_bytes),
                reason: match &row.why {
                    Why::Added => say!("Added by you"),
                    Why::Missing => say!("Missing from your list"),
                    Why::NeededBy(by) => say!("Needed by {}", titled(by)),
                },
                failed: row.failed.clone(),
            })
            .collect();
        let total: u64 = rows.iter().filter_map(|row| row.size).sum();
        ListShown {
            count: rows.len(),
            size_said: (total > 0).then(|| sizes::of_bytes(total)),
            failures: rows.iter().any(|row| row.failed.is_some()),
            action: action(
                Os::running(),
                self.paths().steam,
                self.steamcmd().is_some(),
                rows.is_empty(),
            ),
            rows,
        }
    }

    fn added_shown(
        &self,
        source: &dyn Browse,
        id: &str,
        added: Vec<String>,
        refused: Option<Refusal>,
        unknown: Vec<String>,
    ) -> AddShown {
        let title = source.entry(id).map_or(id.to_string(), |e| e.title);
        AddShown {
            said: (!added.is_empty()).then(|| {
                say_n!(
                    added.len(),
                    "Added {} mod to the list",
                    "Added {} mods to the list",
                    added.len()
                )
            }),
            refusal: refused.map(|why| match why {
                Refusal::Installed => say!("{} is already installed", title),
                Refusal::OnList => say!("{} is already on the list", title),
            }),
            unknown_said: (!unknown.is_empty()).then(|| {
                say_n!(
                    unknown.len(),
                    "{} requirement is not in the database, so it is not added",
                    "{} requirements are not in the database, so they are not added",
                    unknown.len()
                )
            }),
            added,
            unknown,
        }
    }

    pub fn add_to_list(&self, id: &str, with_requirements: bool) -> AddShown {
        let source = self.browse_source();
        let installed = self.installed_workshop();
        let items = self.items_of(&*source, id);
        let (added, refused, unknown) = self.change_list(|list| {
            let (mut added, mut refused, mut unknown) = (Vec::new(), None, Vec::new());
            for item in &items {
                let (more, no, lost) = match with_requirements {
                    true => {
                        let done = add_with_requirements(
                            list,
                            &*source,
                            &|id| installed.contains(id),
                            item,
                        );
                        (done.added, done.refused, done.unknown)
                    }
                    false => {
                        let (more, no) = alone(list, &*source, &installed, item, Why::Added);
                        (more, no, Vec::new())
                    }
                };
                added.extend(more);
                refused = refused.or(no);
                unknown.extend(lost);
            }
            (added, refused, unknown)
        });
        self.added_shown(&*source, id, added, refused, unknown)
    }

    fn items_of(&self, source: &dyn Browse, id: &str) -> Vec<String> {
        let alone = vec![id.to_string()];
        if !self.key_mode() {
            return alone;
        }
        source.collection_items(&alone).unwrap_or(alone)
    }

    pub fn add_missing_to_list(&self, workshop_id: &str) -> AddShown {
        let source = self.browse_source();
        if source.entry(workshop_id).is_none() {
            let unknown = vec![workshop_id.to_string()];
            return self.added_shown(&*source, workshop_id, Vec::new(), None, unknown);
        }
        let installed = self.installed_workshop();
        let (added, refused) =
            self.change_list(|list| alone(list, &*source, &installed, workshop_id, Why::Missing));
        self.added_shown(&*source, workshop_id, added, refused, Vec::new())
    }

    pub fn add_wanted_to_list(&self, wanted: &[(String, String)]) {
        self.change_list(|list| {
            for (id, title) in wanted {
                list.add(Row::new(id, title, Why::Added));
            }
        });
    }

    pub fn remove_from_list(&self, id: &str) {
        self.change_list(|list| list.remove(id));
    }

    pub fn clear_list_failures(&self) {
        self.change_list(DownloadList::clear_failures);
    }

    pub fn empty_list(&self) {
        self.change_list(|list| list.remove_downloaded(&list.ids()));
    }

    pub fn list_size_known(&self, id: &str, size: u64) {
        let known = self
            .list()
            .held()
            .rows()
            .iter()
            .any(|row| row.id == id && row.size != Some(size));
        if known {
            self.change_list(|list| list.set_size(id, size));
        }
    }

    pub fn vet_list(&self) -> Vec<String> {
        let ids = self.listed_ids();
        let source = self.browse_source();
        let answer = self.listed_details(&*source, &ids);
        if let Ok(found) = &answer {
            self.change_list(|list| {
                found
                    .iter()
                    .filter(|d| d.size > 0)
                    .for_each(|d| list.set_size(&d.id, d.size))
            });
        }
        let (passed, refused) = vet(&ids, &answer, &|id| source.entry(id).is_some());
        self.change_list(|list| {
            for (id, why) in refused {
                list.mark_failed(&id, why);
            }
        });
        passed
    }

    fn listed_details(
        &self,
        source: &dyn rimstack_core::sources::browse::Browse,
        ids: &[String],
    ) -> Result<Vec<rimstack_core::rimworld::workshop::Details>> {
        self.remember_details(source, ids);
        let kept = self.browse_details.held();
        let now = std::time::Instant::now();
        if let Some(problem) = ids.iter().find_map(|id| kept.failure(id, now)) {
            return Err(problem);
        }
        Ok(ids
            .iter()
            .filter_map(|id| kept.get(id).map(|d| (*d).clone()))
            .collect())
    }

    pub fn forget_listed(&self, ids: &[String]) {
        self.change_list(|list| list.remove_downloaded(ids));
    }

    pub fn apply_downloads(&self, outcomes: &[(String, Result<PathBuf>)]) {
        let downloaded: Vec<(&String, &Path)> = outcomes
            .iter()
            .filter_map(|(id, outcome)| Some((id, outcome.as_deref().ok()?)))
            .collect();
        let needed = self.requirements_of(&downloaded);
        self.change_list(|list| {
            let done: Vec<String> = downloaded.iter().map(|(id, _)| (*id).clone()).collect();
            list.remove_downloaded(&done);
            for (id, outcome) in outcomes {
                if let Err(problem) = outcome {
                    list.mark_failed(id, problem.sentence().clone());
                }
            }
            for row in needed {
                list.add(row);
            }
        });
    }

    fn requirements_of(&self, downloaded: &[(&String, &Path)]) -> Vec<Row> {
        let version = self.paths().game_version.clone().unwrap_or_default();
        let abouts: Vec<(&String, about::About)> = downloaded
            .iter()
            .filter_map(|(id, path)| {
                let bytes = std::fs::read(about::locate(path)?).ok()?;
                Some((*id, about::parse(&bytes).ok()?))
            })
            .collect();
        let arrived: HashSet<String> = abouts
            .iter()
            .map(|(_, a)| id::fold(&a.package_id))
            .collect();
        let loaded = self.library();
        let known = |key: &str| {
            let library = loaded.as_ref().as_ref().ok().map(|(library, _)| library);
            library.is_some_and(|library| library.name_of(key).is_some())
        };
        let names = self.workshop_names();
        let installed = self.installed_workshop();
        let arrived_ids: HashSet<&String> = downloaded.iter().map(|(id, _)| *id).collect();
        let mut rows: Vec<Row> = Vec::new();
        for (by, about) in &abouts {
            for dependency in about.dependencies.for_version(&version) {
                let key = id::fold(dependency);
                if known(&key) || arrived.contains(&key) {
                    continue;
                }
                let (name, fix) = describe::missing(&names, dependency);
                let Fix::Subscribe(workshop_id) = fix else {
                    continue;
                };
                let wanted = self.browse_source().entry(&workshop_id).is_some()
                    && !installed.contains(&workshop_id)
                    && !arrived_ids.contains(&workshop_id)
                    && !rows.iter().any(|row| row.id == workshop_id);
                if wanted {
                    rows.push(Row::new(workshop_id, name, Why::NeededBy((*by).clone())));
                }
            }
        }
        rows
    }
}
