mod canned;
mod web;

pub use canned::Canned;
pub use web::Web;

use super::{Browse, Catalog, Memory, Request, Sort};
use crate::base::error::Error;
use crate::rimworld::steamapi::{
    self, ApiKey, Call, Found, SEXUAL_DESCRIPTORS, Search, query_type,
};
use crate::rimworld::workshop::Details;
use crate::rimworld::workshopindex::{Entry, Index};
use crate::rimworld::workshopsearch::{Counts, Order, Page};
use serde_json::Value;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

const PAGE_LIMIT: usize = 100;
const DETAILS_CHUNK: usize = 100;
const AUTHOR_PAGES: usize = 3;

/// What a call that did not give an answer says: never the address, which a header-less log might
/// otherwise carry, and never the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Status(u16),
    Unreachable,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Status(code) => write!(f, "HTTP status {code}"),
            Failure::Unreachable => f.write_str("no answer"),
        }
    }
}

pub trait Transport: Send + Sync {
    fn call(&self, call: Call, args: &Value, key: &ApiKey) -> Result<String, Failure>;
}

fn failed(call: Call, failure: Failure) -> Error {
    crate::base::web::steam_failed(format!("{}: {failure}", call.name()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCheck {
    Valid,
    /// Steam refused it (401 or 403).
    Invalid,
    Unreachable,
}

pub fn check_key(transport: &dyn Transport, key: &ApiKey) -> KeyCheck {
    let args = steamapi::query_files(&Search {
        query_type: query_type::NEWEST,
        cursor: "*",
        per_page: 1,
        ..Search::default()
    });
    match transport.call(Call::QueryFiles, &args, key) {
        Ok(_) => KeyCheck::Valid,
        Err(Failure::Status(401 | 403)) => KeyCheck::Invalid,
        Err(_) => KeyCheck::Unreachable,
    }
}

#[derive(Default)]
struct State {
    memory: Memory,
    paging: Option<Paging>,
}

/// `cursors[k]` is what asks for page `k`; empty means there is no such page.
struct Paging {
    shape: String,
    cursors: Vec<String>,
}

struct Shape {
    text: String,
    tags: Vec<String>,
    sort: Sort,
    collections: bool,
    step: usize,
}

/// Server-side: words (title and description), authors as words, version, type and `t:` tags,
/// sorts, collections, mature content. Taken from the cache index when there is one, and left
/// alone when not: `d:` and owned expansions (post-filter, so a page may come up short), `m:`
/// (Steam finds collections holding a mod, not mods needing it) and needed-by.
pub struct Steam {
    key: ApiKey,
    transport: Arc<dyn Transport>,
    index: Option<Arc<Index>>,
    show_mature: bool,
    state: Mutex<State>,
}

fn capitalised(tag: &str) -> String {
    let mut chars = tag.chars();
    chars
        .next()
        .map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
}

fn ranking(sort: Sort, words: bool) -> (u32, Option<u8>) {
    match sort {
        Sort::Relevance if words => (query_type::TEXT, None),
        Sort::Relevance => (query_type::BY_VOTE, None),
        Sort::Trending { days } => (query_type::TREND, Some(days.clamp(1, 7))),
        Sort::MostSubscribed => (query_type::MOST_SUBSCRIBED, None),
        Sort::MostRecent => (query_type::NEWEST, None),
        Sort::LastUpdated => (query_type::LAST_UPDATED, None),
    }
}

impl Steam {
    pub fn new(
        key: ApiKey,
        transport: Arc<dyn Transport>,
        index: Option<Arc<Index>>,
        show_mature: bool,
    ) -> Steam {
        Steam {
            key,
            transport,
            index,
            show_mature,
            state: Mutex::default(),
        }
    }

    pub fn fixture() -> Steam {
        let key = ApiKey::new("fixture").expect("not blank");
        let index = Arc::new(canned::index());
        Steam::new(key, Arc::new(canned::Canned), Some(index), false)
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn ask(&self, call: Call, args: &Value) -> Result<String, Error> {
        self.transport
            .call(call, args, &self.key)
            .map_err(|failure| failed(call, failure))
    }

    fn excluded(&self) -> &'static [u32] {
        match self.show_mature {
            true => &[],
            false => &SEXUAL_DESCRIPTORS,
        }
    }

    fn learn(&self, files: &[Details]) {
        let ids: Vec<String> = files.iter().map(|d| d.id.clone()).collect();
        self.state()
            .memory
            .learn(&ids, Ok(files.to_vec()), Instant::now());
    }

    fn shape(&self, request: &Request) -> Shape {
        let Request {
            query,
            filters,
            sort,
            collections,
            count,
            ..
        } = request;
        let mut words = query.words.clone();
        words.extend(query.authors.iter().cloned());
        let mut tags = Vec::new();
        if !collections {
            tags.extend(filters.include_tags.iter().map(|t| capitalised(t)));
        }
        tags.extend(query.tags.iter().map(|t| capitalised(t)));
        Shape {
            text: words.join(" "),
            tags,
            sort: *sort,
            collections: *collections,
            step: (*count).clamp(1, PAGE_LIMIT),
        }
    }

    fn query(&self, shape: &Shape, cursor: &str) -> Result<Found, Error> {
        let (kind, days) = ranking(shape.sort, !shape.text.is_empty());
        let args = steamapi::query_files(&Search {
            query_type: kind,
            cursor,
            per_page: shape.step,
            text: &shape.text,
            required_tags: &shape.tags,
            collections: shape.collections,
            days,
            excluded_descriptors: self.excluded(),
        });
        let found = steamapi::parse_query(&self.ask(Call::QueryFiles, &args)?)?;
        self.learn(&found.files);
        Ok(found)
    }

    /// The pages are walked from the nearest cursor kept, because Steam has no way to skip.
    fn page(&self, shape: &Shape, k: usize) -> Result<Found, Error> {
        let signature = format!(
            "{:?}",
            (
                &shape.text,
                &shape.tags,
                shape.sort,
                shape.collections,
                shape.step,
                self.show_mature
            )
        );
        let mut at = {
            let mut state = self.state();
            if state.paging.as_ref().is_none_or(|p| p.shape != signature) {
                state.paging = Some(Paging {
                    shape: signature,
                    cursors: vec!["*".to_string()],
                });
            }
            let known = state.paging.as_ref().map_or(1, |p| p.cursors.len());
            k.min(known - 1)
        };
        loop {
            let cursor = {
                let state = self.state();
                state.paging.as_ref().map(|p| p.cursors[at].clone())
            };
            let Some(cursor) = cursor.filter(|c| !c.is_empty()) else {
                return Ok(Found::default());
            };
            let found = self.query(shape, &cursor)?;
            let next = found
                .next_cursor
                .clone()
                .filter(|n| *n != cursor && !found.files.is_empty())
                .unwrap_or_default();
            if let Some(paging) = self.state().paging.as_mut()
                && paging.cursors.len() == at + 1
            {
                paging.cursors.push(next);
            }
            if at == k {
                return Ok(found);
            }
            at += 1;
        }
    }

    fn allowed(&self, request: &Request, file: &Details) -> bool {
        let Some(index) = &self.index else {
            return true;
        };
        let dlc = index.get(&file.id).map(|e| e.dlc);
        let wanted = &request.query.dlc;
        if !wanted.is_empty()
            && !dlc
                .as_ref()
                .is_some_and(|d| wanted.iter().all(|w| d.contains(w)))
        {
            return false;
        }
        match (&request.filters.owned_dlc, dlc) {
            (Some(owned), Some(needed)) => needed.iter().all(|d| owned.contains(d)),
            _ => true,
        }
    }

    fn by_ids(&self, request: &Request) -> Result<Page, Error> {
        let found: Vec<String> = self
            .shared(&request.query.ids)?
            .iter()
            .map(|d| d.id.clone())
            .collect();
        let total = found.len();
        let ids = found
            .into_iter()
            .skip(request.offset)
            .take(request.count)
            .collect();
        Ok(Page { ids, total })
    }

    fn shared(&self, ids: &[String]) -> Result<Vec<Arc<Details>>, Error> {
        let todo = self.state().memory.unasked(ids, Instant::now());
        for chunk in todo.chunks(DETAILS_CHUNK) {
            let answer = self
                .ask(Call::GetDetails, &steamapi::get_details(chunk))
                .and_then(|text| steamapi::parse_get_details(&text));
            self.state().memory.learn(chunk, answer, Instant::now());
        }
        let state = self.state();
        let mut found = Vec::new();
        for id in ids {
            match state.memory.get(id) {
                Some(d) => found.push(d),
                None => {
                    if let Some(error) = state.memory.failure(id, Instant::now()) {
                        return Err(error);
                    }
                }
            }
        }
        Ok(found)
    }

    fn known(&self, id: &str) -> Result<Option<Arc<Details>>, Error> {
        Ok(self.shared(&[id.to_string()])?.into_iter().next())
    }
}

impl Browse for Steam {
    fn search(&self, request: &Request) -> Result<Page, Error> {
        if !request.query.ids.is_empty() {
            return self.by_ids(request);
        }
        if !request.query.requires.is_empty() {
            let (query, filters) = (&request.query, &request.filters);
            let page = self.index.as_ref().map(|i| {
                i.search(
                    query,
                    filters,
                    Order::Relevance,
                    request.offset,
                    request.count,
                )
            });
            return Ok(page.unwrap_or(Page {
                ids: Vec::new(),
                total: 0,
            }));
        }
        let shape = self.shape(request);
        let (k, skip) = (request.offset / shape.step, request.offset % shape.step);
        let first = self.page(&shape, k)?;
        let total = first.total;
        let mut files = first.files;
        if skip > 0 && files.len() < skip + request.count {
            files.extend(self.page(&shape, k + 1)?.files);
        }
        let ids = files
            .iter()
            .skip(skip)
            .take(request.count)
            .filter(|f| self.allowed(request, f))
            .map(|f| f.id.clone())
            .collect();
        Ok(Page { ids, total })
    }

    fn details(&self, ids: &[String]) -> Result<Vec<Details>, Error> {
        Ok(self.shared(ids)?.iter().map(|d| (**d).clone()).collect())
    }

    fn sorts(&self) -> Vec<Sort> {
        vec![
            Sort::Relevance,
            Sort::Trending { days: 1 },
            Sort::Trending { days: 7 },
            Sort::MostSubscribed,
            Sort::MostRecent,
            Sort::LastUpdated,
        ]
    }

    fn counts(&self, _: &Request) -> Option<Counts> {
        None
    }

    fn catalog(&self) -> Catalog {
        self.index
            .as_ref()
            .map_or_else(Catalog::default, |index| Catalog::of(index))
    }

    fn entry(&self, id: &str) -> Option<Entry> {
        if let Some(entry) = self.index.as_ref().and_then(|i| i.get(id)) {
            return Some(entry);
        }
        let details = self.state().memory.get(id)?;
        Some(Entry {
            id: details.id.clone(),
            title: details.title.clone(),
            author: None,
            package_id: None,
            tags: details.tags.clone(),
            requires: Vec::new(),
            dlc: Vec::new(),
        })
    }

    fn app_name(&self, app_id: &str) -> Option<String> {
        let index = self.index.as_ref()?;
        index.app_name(app_id).map(str::to_string)
    }

    fn requires(&self, id: &str) -> Result<Vec<String>, Error> {
        let known = self.index.as_ref().map(|i| i.requires(id));
        if let Some(known) = known.filter(|k| !k.is_empty()) {
            return Ok(known);
        }
        Ok(self
            .known(id)?
            .filter(|d| !d.collection)
            .map_or_else(Vec::new, |d| d.children.clone()))
    }

    fn needed_by(&self, id: &str) -> Result<Vec<String>, Error> {
        Ok(self
            .index
            .as_ref()
            .map_or_else(Vec::new, |i| i.needed_by(id)))
    }

    fn same_author(&self, id: &str) -> Result<Vec<String>, Error> {
        let Some(creator) = self
            .known(id)?
            .map(|d| d.creator.clone())
            .filter(|c| !c.is_empty())
        else {
            return Ok(Vec::new());
        };
        let mut ids: Vec<String> = Vec::new();
        for page in 1..=AUTHOR_PAGES {
            let args = steamapi::user_files(&creator, page, PAGE_LIMIT, self.excluded());
            let found = steamapi::parse_user_files(&self.ask(Call::GetUserFiles, &args)?)?;
            self.learn(&found.files);
            ids.extend(found.files.iter().map(|f| f.id.clone()));
            if found.files.len() < PAGE_LIMIT || ids.len() >= found.total {
                break;
            }
        }
        ids.retain(|other| other != id);
        Ok(ids)
    }

    fn collection_items(&self, ids: &[String]) -> Result<Vec<String>, Error> {
        let mut all: Vec<String> = Vec::new();
        for id in ids {
            let set = self.known(id)?.filter(|d| d.collection);
            let items = set.map_or_else(|| vec![id.clone()], |d| d.children.clone());
            for item in items {
                if !all.contains(&item) {
                    all.push(item);
                }
            }
        }
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rimworld::workshopsearch::{Filters, Query};
    use crate::sources::browse::fixture::{COLLECTION, Fixture, HARMONY};

    const SECRET: &str = "ABCDEF0123456789SECRET";

    fn key() -> ApiKey {
        ApiKey::new(SECRET).unwrap()
    }

    fn request(text: &str, sort: Sort) -> Request {
        Request {
            query: Query::parse(text),
            sort,
            count: 50,
            ..Request::default()
        }
    }

    fn ids(steam: &Steam, request: &Request) -> Vec<String> {
        steam.search(request).unwrap().ids
    }

    struct Failing(Failure);

    impl Transport for Failing {
        fn call(&self, _: Call, _: &Value, _: &ApiKey) -> Result<String, Failure> {
            Err(self.0)
        }
    }

    struct Recording(Mutex<Vec<Value>>);

    impl Transport for Recording {
        fn call(&self, call: Call, args: &Value, key: &ApiKey) -> Result<String, Failure> {
            self.0.lock().unwrap().push(args.clone());
            canned::Canned.call(call, args, key)
        }
    }

    #[test]
    fn a_failure_names_the_call_and_the_status_and_never_the_key() {
        for failure in [
            Failure::Status(403),
            Failure::Status(429),
            Failure::Unreachable,
        ] {
            let steam = Steam::new(key(), Arc::new(Failing(failure)), None, false);
            let errors = [
                steam.search(&request("x", Sort::Relevance)).unwrap_err(),
                steam.details(&["5".into()]).unwrap_err(),
                steam.same_author("5").unwrap_err(),
            ];
            for error in errors {
                for shown in [
                    error.to_string(),
                    format!("{error:?}"),
                    format!("{:?}", error.sentence()),
                ] {
                    assert!(
                        !shown.contains("SECRET") && !shown.contains("ABCDEF"),
                        "{shown}"
                    );
                }
            }
        }
        let error = failed(Call::QueryFiles, Failure::Status(403));
        assert!(error.to_string().contains("QueryFiles: HTTP status 403"));
        assert!(!format!("{:?}", Steam::fixture().key).contains("fixture"));
    }

    #[test]
    fn the_key_is_checked_with_one_cheap_call() {
        let check = |failure| check_key(&Failing(failure), &key());
        assert_eq!(check(Failure::Status(401)), KeyCheck::Invalid);
        assert_eq!(check(Failure::Status(403)), KeyCheck::Invalid);
        assert_eq!(check(Failure::Status(500)), KeyCheck::Unreachable);
        assert_eq!(check(Failure::Unreachable), KeyCheck::Unreachable);
        let recording = Recording(Mutex::default());
        assert_eq!(check_key(&recording, &key()), KeyCheck::Valid);
        let sent = recording.0.lock().unwrap();
        assert_eq!((sent.len(), sent[0]["numperpage"].clone()), (1, 1.into()));
        assert!(!sent[0].to_string().contains("SECRET"));
    }

    #[test]
    fn sorts_change_the_order() {
        let steam = Steam::fixture();
        let by = |sort| ids(&steam, &request("", sort));
        let orders = [
            by(Sort::Relevance),
            by(Sort::Trending { days: 3 }),
            by(Sort::MostSubscribed),
            by(Sort::MostRecent),
            by(Sort::LastUpdated),
        ];
        for (i, one) in orders.iter().enumerate() {
            assert_eq!(one.len(), 7, "mods only, no collection");
            for other in &orders[i + 1..] {
                assert_ne!(one, other);
            }
        }
        assert_eq!(orders[3][0], "3100000006");
    }

    #[test]
    fn words_version_and_type_go_to_steam_and_pages_follow_the_cursor() {
        let steam = Steam::fixture();
        assert_eq!(
            ids(&steam, &request("storage", Sort::Relevance)),
            ["3100000001", "3100000002"]
        );
        let mut scenario = request("", Sort::Relevance);
        scenario.filters = Filters {
            include_tags: vec!["Scenario".into(), "1.6".into()],
            ..Filters::default()
        };
        assert_eq!(ids(&steam, &scenario), ["3100000006"]);
        let all = ids(&steam, &request("", Sort::MostSubscribed));
        let mut paged = Vec::new();
        for offset in (0..8).step_by(3) {
            let mut asked = request("", Sort::MostSubscribed);
            (asked.offset, asked.count) = (offset, 3);
            let page = steam.search(&asked).unwrap();
            assert_eq!(page.total, 7);
            paged.extend(page.ids);
        }
        assert_eq!(paged, all);
        let mut late = request("", Sort::MostSubscribed);
        (late.offset, late.count) = (4, 3);
        assert_eq!(ids(&steam, &late), all[4..7]);
    }

    #[test]
    fn a_collection_search_finds_collections_and_their_items_come_from_the_children() {
        let steam = Steam::fixture();
        let mut asked = request("", Sort::Relevance);
        asked.collections = true;
        assert_eq!(ids(&steam, &asked), [COLLECTION]);
        let items = steam
            .collection_items(&[COLLECTION.into(), "5".into()])
            .unwrap();
        assert_eq!(items, ["3100000001", "3100000003", "5"]);
        let set = &steam.details(&[COLLECTION.into()]).unwrap()[0];
        assert!(set.collection);
    }

    #[test]
    fn details_have_votes_and_screenshots_and_lists_come_from_steam() {
        let steam = Steam::fixture();
        let found = steam.details(&[HARMONY.into(), "1".into()]).unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].votes.is_some());
        assert_eq!(found[0].screenshots.len(), 3);
        assert!(found[0].screenshots.iter().all(|url| url.contains(HARMONY)));
        assert_eq!(steam.requires("3100000002").unwrap().len(), 2);
        assert_eq!(steam.same_author("3100000001").unwrap().len(), 3);
        assert!(
            !steam
                .same_author("3100000001")
                .unwrap()
                .contains(&"3100000001".to_string())
        );
        assert!(steam.needed_by(HARMONY).unwrap().len() >= 3);
        let plain = Steam::new(key(), Arc::new(canned::Canned), None, false);
        assert_eq!(plain.requires("3100000001").unwrap(), [HARMONY]);
        plain.details(&["3100000001".into()]).unwrap();
        assert_eq!(
            plain.entry("3100000001").unwrap().title,
            "Fixture Better Storage"
        );
    }

    #[test]
    fn ids_and_expansions_are_answered_as_far_as_the_api_allows() {
        let steam = Steam::fixture();
        assert_eq!(
            ids(&steam, &request(&format!("w:{HARMONY}"), Sort::Relevance)),
            [HARMONY]
        );
        assert_eq!(
            ids(&steam, &request("d:1149640", Sort::Relevance)),
            ["3100000004"]
        );
        let mut owned = request("", Sort::Relevance);
        owned.filters.owned_dlc = Some(Vec::new());
        assert!(!ids(&steam, &owned).contains(&"3100000004".to_string()));
        assert_eq!(
            ids(&steam, &request(&format!("m:{HARMONY}"), Sort::Relevance)).len(),
            4
        );
    }

    #[test]
    fn sexual_content_is_left_out_unless_the_player_asks() {
        for (show, wanted) in [(false, true), (true, false)] {
            let recording = Arc::new(Recording(Mutex::default()));
            let steam = Steam::new(key(), recording.clone(), None, show);
            steam.search(&request("a", Sort::Relevance)).unwrap();
            steam.same_author("3100000001").unwrap();
            let sent = recording.0.lock().unwrap();
            assert!(sent.len() >= 2);
            for args in sent
                .iter()
                .filter(|a| a.get("query_type").is_some() || a.get("steamid").is_some())
            {
                let has =
                    args.get("excluded_content_descriptors") == Some(&serde_json::json!([1, 3, 4]));
                assert_eq!(has, wanted, "{args}");
            }
        }
    }

    #[test]
    fn the_cache_and_the_fixture_do_not_trend() {
        let fixture = Fixture::new();
        let by = |sort| fixture.search(&request("", sort)).unwrap().ids;
        assert_eq!(by(Sort::Relevance), by(Sort::MostRecent));
        assert_eq!(by(Sort::Relevance), by(Sort::Trending { days: 7 }));
        assert_ne!(by(Sort::Relevance), by(Sort::MostSubscribed));
    }
}
