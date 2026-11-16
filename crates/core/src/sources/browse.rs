mod add;
mod fixture;
mod memory;
mod steam;

pub use add::{Added, Refusal, add_with_requirements};
pub use fixture::{COLLECTION, Fixture, HARMONY};
pub use memory::Memory;
pub use steam::{Canned, Failure, KeyCheck, Steam, Transport, Web, check_key};

use crate::base::error::Error;
use crate::rimworld::workshop::{self, Details};
use crate::rimworld::workshopindex::{Entry, Index, REAL_VERSION};
use crate::rimworld::workshopsearch::{Counts, Filters, Order, Page, Query};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Relevance,
    /// Steam keeps a window of 1 to 7 days.
    Trending {
        days: u8,
    },
    MostSubscribed,
    MostRecent,
    LastUpdated,
}

#[derive(Debug, Clone, Default)]
pub struct Request {
    pub query: Query,
    pub filters: Filters,
    pub sort: Sort,
    pub collections: bool,
    pub offset: usize,
    pub count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    /// Oldest first.
    pub versions: Vec<String>,
    /// Steam app id and name, by name.
    pub dlc: Vec<(String, String)>,
}

impl Catalog {
    pub fn of(index: &Index) -> Catalog {
        Catalog::of_versions_with(index, REAL_VERSION)
    }

    pub fn of_versions_with(index: &Index, at_least: usize) -> Catalog {
        Catalog {
            versions: index.version_tags(at_least),
            dlc: index.apps(),
        }
    }
}

pub trait Browse: Send + Sync {
    fn search(&self, request: &Request) -> Result<Page, Error>;

    /// Blocks on the network for the cache. A mod Steam does not know is left out.
    fn details(&self, ids: &[String]) -> Result<Vec<Details>, Error>;

    /// What the source knows of a mod without asking: `requires` is as published, known or not.
    fn entry(&self, id: &str) -> Option<Entry>;

    fn app_name(&self, app_id: &str) -> Option<String>;

    fn sorts(&self) -> Vec<Sort>;

    /// How many results the request has under each tag and required DLC; `None` where the source
    /// cannot say (Steam with the key answers in pages, not whole).
    fn counts(&self, request: &Request) -> Option<Counts>;

    fn catalog(&self) -> Catalog;

    fn requires(&self, id: &str) -> Result<Vec<String>, Error>;

    fn needed_by(&self, id: &str) -> Result<Vec<String>, Error>;

    fn same_author(&self, id: &str) -> Result<Vec<String>, Error>;

    /// A collection's items, or the id itself for a mod that is not a collection.
    fn collection_items(&self, ids: &[String]) -> Result<Vec<String>, Error>;
}

pub(super) fn cache_sorts(index: &Index) -> Vec<Sort> {
    let mut sorts = vec![Sort::Relevance, Sort::MostRecent];
    if index.swept() {
        sorts.extend([Sort::MostSubscribed, Sort::LastUpdated]);
    }
    sorts
}

pub(super) fn order_of(sort: Sort) -> Order {
    match sort {
        Sort::MostSubscribed => Order::Subscribed,
        Sort::LastUpdated => Order::Updated,
        Sort::MostRecent => Order::Newest,
        Sort::Relevance | Sort::Trending { .. } => Order::Relevance,
    }
}

pub struct Cache {
    index: Arc<Index>,
}

impl Cache {
    pub fn new(index: Arc<Index>) -> Self {
        Cache { index }
    }
}

impl Browse for Cache {
    fn search(&self, request: &Request) -> Result<Page, Error> {
        let Request {
            query,
            filters,
            sort,
            offset,
            count,
            ..
        } = request;
        let order = order_of(*sort);
        Ok(self.index.search(query, filters, order, *offset, *count))
    }

    fn sorts(&self) -> Vec<Sort> {
        cache_sorts(&self.index)
    }

    fn counts(&self, request: &Request) -> Option<Counts> {
        Some(self.index.counts(&request.query, &request.filters))
    }

    fn catalog(&self) -> Catalog {
        Catalog::of(&self.index)
    }

    fn details(&self, ids: &[String]) -> Result<Vec<Details>, Error> {
        workshop::fetch_details(ids)
    }

    fn entry(&self, id: &str) -> Option<Entry> {
        self.index.get(id)
    }

    fn app_name(&self, app_id: &str) -> Option<String> {
        self.index.app_name(app_id).map(str::to_string)
    }

    fn requires(&self, id: &str) -> Result<Vec<String>, Error> {
        Ok(self.index.requires(id))
    }

    fn needed_by(&self, id: &str) -> Result<Vec<String>, Error> {
        Ok(self.index.needed_by(id))
    }

    fn same_author(&self, id: &str) -> Result<Vec<String>, Error> {
        Ok(self.index.same_author(id))
    }

    fn collection_items(&self, ids: &[String]) -> Result<Vec<String>, Error> {
        workshop::expand_collections(ids)
    }
}
