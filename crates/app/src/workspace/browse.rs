mod shown;
mod sidebar;
mod sorts;

use super::{App, now_secs, written};
use rimstack_core::base::error::Error;
use rimstack_core::base::locks::Held;
use rimstack_core::mods::library::Library;
use rimstack_core::rimworld::paths::Source;
use rimstack_core::rimworld::workshop::{self, Details};
use rimstack_core::rimworld::workshopindex::{self, Entry, Index};
use rimstack_core::rimworld::workshopsearch::Query;
use rimstack_core::rimworld::workshopsweep::{self, Sweep};
pub use rimstack_core::sources::browse::Sort;
use rimstack_core::sources::browse::{Browse, Cache, Canned, Fixture, Request};
use rimstack_core::sources::previews;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

pub use shown::{Marks, Part, Results, Row, Shown};
pub use sidebar::{Chip, Facet, Picked, Sidebar};
pub use sorts::{SortOffer, chosen, effective, offers, sort_of};

pub type Each<'a> = dyn Fn(&str, Option<PathBuf>, Vec<bool>) -> bool + Sync + 'a;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Related {
    Requires,
    NeededBy,
    SameAuthor,
    Collection,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Defaults {
    pub ready: bool,
    pub key_mode: bool,
    pub sorts: Vec<Sort>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Asked {
    pub text: String,
    pub picked: Picked,
    pub sort: Sort,
    pub collections: bool,
}

impl Default for Asked {
    fn default() -> Self {
        Asked {
            text: String::new(),
            picked: Picked::default(),
            sort: Sort::MostRecent,
            collections: false,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Followed {
    Item(String),
    Outside(String),
    Nothing,
}

pub fn follow(link: &str) -> Followed {
    let link = link.trim();
    let ours = [
        "steamcommunity.com/sharedfiles/filedetails",
        "steamcommunity.com/workshop/filedetails",
    ];
    if ours.iter().any(|page| link.contains(page))
        && let Some(id) = workshop::ids_in(link).into_iter().next()
    {
        return Followed::Item(id);
    }
    match link.starts_with("http://") || link.starts_with("https://") {
        true => Followed::Outside(link.to_string()),
        false => Followed::Nothing,
    }
}

fn is_workshop_folder(folder: &str) -> bool {
    !folder.is_empty() && folder.chars().all(|c| c.is_ascii_digit())
}

fn installed_in(library: &Library) -> impl Iterator<Item = String> + '_ {
    // SteamCMD puts a mod in the game's own folder under its Workshop id.
    let ours = |source: Source, folder: &str| match source {
        Source::Workshop => true,
        Source::Local => is_workshop_folder(folder),
        _ => false,
    };
    library
        .loadable()
        .map(|(_, m)| m)
        .filter(move |m| ours(m.source, &m.folder))
        .map(|m| m.folder.clone())
}

impl App {
    pub fn browse_page(&self) -> usize {
        match self.fixture {
            true => 3,
            false => 30,
        }
    }

    pub fn browsing_fixture(self, on: bool) -> Self {
        let app = Self {
            fixture: on,
            ..self
        };
        match on {
            true => app.with_transport(Arc::new(Canned)),
            false => app,
        }
    }

    pub fn workshop_index(&self) -> Arc<Index> {
        let file = workshopindex::file_in(&self.data_dir);
        let swept = workshopsweep::file_in(&self.data_dir);
        let key = (written(file.clone()), written(swept.clone()));
        self.workshop_index
            .get(key, || Index::read(&file).with_sweep(&Sweep::read(&swept)))
    }

    pub fn browse_source(&self) -> Arc<dyn Browse> {
        self.keyed_source().unwrap_or_else(|| self.kept_source())
    }

    fn kept_source(&self) -> Arc<dyn Browse> {
        if self.fixture {
            return self
                .fixture_source
                .get_or_init(|| Arc::new(Fixture::new()))
                .clone();
        }
        Arc::new(Cache::new(self.workshop_index()))
    }

    pub(crate) fn database_there(&self) -> bool {
        std::fs::metadata(workshopindex::file_in(&self.data_dir)).is_ok_and(|file| file.len() > 0)
    }

    pub fn browse_opened(&self) -> bool {
        !self.browse_open.swap(true, Ordering::Relaxed)
    }

    pub fn browse_defaults(&self) -> Defaults {
        let opened = self.fixture || self.browse_open.load(Ordering::Relaxed);
        let key_mode = self.key_mode();
        Defaults {
            ready: self.fixture || key_mode || self.database_there(),
            key_mode,
            sorts: match opened || key_mode {
                true => self.browse_source().sorts(),
                false => Vec::new(),
            },
        }
    }

    pub fn installed_workshop(&self) -> HashSet<String> {
        let mut ids: HashSet<String> = self.steam_record().times.keys().cloned().collect();
        if let Ok((library, _)) = self.library().as_ref().as_ref() {
            ids.extend(installed_in(library));
        }
        ids
    }

    pub(crate) fn marks(&self) -> Marks {
        Marks {
            installed: self.installed_workshop(),
            listed: self.listed_ids().into_iter().collect(),
        }
    }

    fn rows_of(&self, source: &dyn Browse, ids: &[String]) -> Vec<Row> {
        let marks = self.marks();
        ids.iter()
            .filter_map(|id| source.entry(id))
            .map(|entry| marks.row(&entry))
            .collect()
    }

    /// The database gives a package id's app as its Workshop id.
    pub fn owned_dlc(&self) -> Vec<String> {
        let names = self.workshop_names();
        let official = self.install().official();
        let mut owned: Vec<String> = official
            .iter()
            .filter_map(|o| names.get(&o.package_id))
            .map(|item| item.workshop_id.clone())
            .collect();
        owned.sort();
        owned
    }

    pub fn browse_search(
        &self,
        asked: &Asked,
        offset: usize,
        count: usize,
    ) -> Result<Results, Error> {
        let keyed = self.keyed_source();
        let source = keyed.clone().unwrap_or_else(|| self.kept_source());
        self.steam_unasked();
        let problem = match self.search_in(&*source, asked, offset, count) {
            Err(problem) => problem,
            found => return found,
        };
        let Some(failure) = keyed.and(self.steam_failure()) else {
            return Err(problem);
        };
        rimstack_core::problem!("{problem}");
        let notice = self.steam_refusal_told(failure);
        let mut results = self.search_in(&*self.kept_source(), asked, offset, count)?;
        results.notice = Some(notice);
        Ok(results)
    }

    fn search_in(
        &self,
        source: &dyn Browse,
        asked: &Asked,
        offset: usize,
        count: usize,
    ) -> Result<Results, Error> {
        let owned = asked.picked.hide_unowned.then(|| self.owned_dlc());
        let request = Request {
            query: Query::parse(&asked.text),
            filters: asked.picked.filters(owned),
            sort: asked.sort,
            collections: asked.collections,
            offset,
            count,
        };
        let page = source.search(&request)?;
        let sidebar = (offset == 0).then(|| {
            let counts = source.counts(&request);
            let searched = !asked.text.trim().is_empty();
            Sidebar::of(
                counts.as_ref(),
                &source.catalog(),
                &asked.picked,
                self.paths().game_version.as_deref(),
                searched,
                page.total,
            )
        });
        Ok(Results {
            more: offset.saturating_add(count) < page.total,
            total: page.total,
            rows: self.rows_of(source, &page.ids),
            notice: None,
            sidebar,
        })
    }

    pub fn browse_related(&self, id: &str, which: Related) -> Result<Results, Error> {
        let source = self.browse_source();
        let ids = match which {
            Related::Requires => source.requires(id)?,
            Related::NeededBy => source.needed_by(id)?,
            Related::SameAuthor => source.same_author(id)?,
            Related::Collection => source.collection_items(&[id.to_string()])?,
        };
        let rows = self.rows_of(&*source, &ids);
        Ok(Results {
            total: rows.len(),
            more: false,
            rows,
            notice: None,
            sidebar: None,
        })
    }

    /// Slow on the first ask of an id, when Steam is asked; no lock is held meanwhile.
    fn details_of(&self, source: &dyn Browse, id: &str) -> Result<Option<Arc<Details>>, Error> {
        self.remember_details(source, &[id.to_string()]);
        let kept = self.browse_details.held();
        match (kept.get(id), kept.failure(id, Instant::now())) {
            (Some(details), _) => Ok(Some(details)),
            (None, Some(problem)) => Err(problem),
            (None, None) => Ok(None),
        }
    }

    /// No lock is held meanwhile.
    pub fn remember_details(&self, source: &dyn Browse, ids: &[String]) {
        let unasked = self.browse_details.held().unasked(ids, Instant::now());
        if unasked.is_empty() {
            return;
        }
        let answer = source.details(&unasked);
        if let Err(problem) = &answer {
            rimstack_core::problem!("{problem}");
            if let Some(failure) = self.steam_failure() {
                self.steam_refusal_told(failure);
            }
        }
        self.browse_details
            .held()
            .learn(&unasked, answer, Instant::now());
    }

    /// `None` when neither the database nor Steam knows the id.
    pub fn browse_details(&self, id: &str) -> Option<Shown> {
        let source = self.browse_source();
        let (details, problem) = match self.details_of(&*source, id) {
            Ok(details) => (details, None),
            Err(problem) => (None, Some(problem.sentence().clone())),
        };
        let entry = source.entry(id).or_else(|| {
            details.as_ref().map(|d| Entry {
                id: d.id.clone(),
                title: d.title.clone(),
                author: None,
                package_id: None,
                tags: Vec::new(),
                requires: Vec::new(),
                dlc: Vec::new(),
            })
        })?;
        if let Some(details) = details.as_ref().filter(|d| d.size > 0) {
            self.list_size_known(id, details.size);
        }
        let requires = source.requires(id).unwrap_or_default();
        let marks = self.marks();
        Some(Shown::of(
            marks.row(&entry),
            &entry,
            &*source,
            details.as_deref(),
            problem,
            self.rows_of(&*source, &requires),
            now_secs(),
        ))
    }

    pub fn browse_title(&self, id: &str) -> String {
        let found = self.browse_source().entry(id);
        found.map_or_else(|| id.to_string(), |entry| entry.title)
    }

    /// `each` hears every id once, with `None` when it has no preview; it stops the rest by
    /// answering false.
    pub fn browse_previews(&self, ids: &[String], each: &Each<'_>) {
        let source = self.browse_source();
        self.remember_details(&*source, ids);
        let urls: Vec<(&String, String, Vec<bool>)> = {
            let kept = self.browse_details.held();
            ids.iter()
                .map(|id| {
                    let found = kept.get(id);
                    let url = found.as_ref().map(|d| d.preview_url.clone());
                    let stars = shown::stars_of(found.and_then(|d| d.votes));
                    (id, url.unwrap_or_default(), stars)
                })
                .collect()
        };
        use rayon::prelude::*;
        let _ = urls.par_iter().try_for_each(|(id, url, stars)| {
            let path = self.browse_preview(url).unwrap_or_else(|problem| {
                rimstack_core::problem!("{problem}");
                None
            });
            each(id, path, stars.clone()).then_some(())
        });
        self.previews_trimmed
            .call_once(|| previews::trim(&self.previews_dir(), previews::CACHE_CAP));
    }

    /// Each is `None` when it could not be taken. Blocks on the network.
    pub fn browse_images(&self, urls: &[String]) -> Vec<(String, Option<PathBuf>)> {
        use rayon::prelude::*;
        let taken: Vec<(String, Option<PathBuf>)> = urls
            .par_iter()
            .map(|url| {
                let folder = self.previews_dir();
                let path = match self.fixture {
                    true => previews::made_up_in_text(url, &folder),
                    false => previews::fetch_in_text(url, &folder),
                };
                let path = path.map_err(|problem| rimstack_core::problem!("{problem}"));
                (url.clone(), path.ok())
            })
            .collect();
        self.previews_trimmed
            .call_once(|| previews::trim(&self.previews_dir(), previews::CACHE_CAP));
        taken
    }

    pub fn previews_dir(&self) -> PathBuf {
        self.data_dir.join("previews")
    }

    /// `None` for a mod with no preview. Blocks on the network the first time.
    pub fn browse_preview(&self, url: &str) -> Result<Option<PathBuf>, Error> {
        if url.is_empty() {
            return Ok(None);
        }
        if self.fixture {
            return previews::made_up(url, &self.previews_dir()).map(Some);
        }
        previews::fetch(url, &self.previews_dir()).map(Some)
    }
}

#[cfg(test)]
mod key_tests;
#[cfg(test)]
mod tests;
