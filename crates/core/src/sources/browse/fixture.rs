use super::{Browse, Catalog, Request, Sort};
use crate::base::error::Error;
use crate::rimworld::workshop::Details;
use crate::rimworld::workshopindex::{Entry, Index};
use crate::rimworld::workshopsearch::{Counts, Page};
use crate::rimworld::workshopsweep::{Record, Sweep};
use std::collections::BTreeMap;

pub const HARMONY: &str = "2009463077";
pub const COLLECTION: &str = "3000000001";

pub struct Fixture {
    index: Index,
}

fn entry(id: &str, title: &str, author: Option<&str>, tags: &[&str], requires: &[&str]) -> Entry {
    Entry {
        id: id.into(),
        title: title.into(),
        author: author.map(Into::into),
        package_id: Some(format!("fixture.{id}")),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        requires: requires.iter().map(|r| r.to_string()).collect(),
        dlc: Vec::new(),
    }
}

pub(super) fn entries() -> Vec<Entry> {
    let mut royal = entry(
        "3100000004",
        "Fixture Royal Armour",
        Some("Fixture Smith"),
        &["Mod", "1.6"],
        &[HARMONY],
    );
    royal.dlc = vec!["1149640".into()];
    vec![
        entry(
            HARMONY,
            "Fixture Harmony",
            Some("Fixture Pardeike"),
            &["Mod", "1.5", "1.6"],
            &[],
        ),
        entry(
            "3100000001",
            "Fixture Better Storage",
            Some("Fixture Smith"),
            &["Mod", "1.5", "1.6"],
            &[HARMONY],
        ),
        entry(
            "3100000002",
            "Fixture Storage Extras",
            Some("Fixture Smith"),
            &["Mod", "1.6"],
            &["3100000001", HARMONY, "3199999999"],
        ),
        entry(
            "3100000003",
            "Fixture Tidy Hauling",
            None,
            &["Mod", "1.5"],
            &[HARMONY],
        ),
        royal,
        entry(
            "3100000005",
            "Fixture Deutsch",
            Some("Fixture Linguist"),
            &["Translation", "1.6"],
            &[],
        ),
        entry(
            "3100000006",
            "Fixture Lost Tribe",
            Some("Fixture Linguist"),
            &["Scenario", "1.6"],
            &[],
        ),
    ]
}

fn made_up_sweep() -> Sweep {
    let records = entries().into_iter().enumerate().map(|(i, e)| {
        let record = Record {
            title: e.title,
            tags: e.tags,
            subscriptions: ((i as u64 * 7) % 11 + 1) * 1000,
            created: 1_700_000_000 + i as u64 * 1000,
            updated: 1_760_000_000 + ((i as u64 * 5) % 7) * 100_000,
            app_id: 294_100,
            ..Record::default()
        };
        (e.id.parse().unwrap_or(0), record)
    });
    Sweep {
        completed: 1_760_000_000,
        resume_after: 0,
        records: records.collect(),
    }
}

pub(super) fn index() -> Index {
    Index::new(entries())
        .with_sweep(&made_up_sweep())
        .with_apps(BTreeMap::from([
            ("1149640".to_string(), "Fixture Expansion One".to_string()),
            ("1826140".to_string(), "Fixture Expansion Two".to_string()),
        ]))
}

impl Default for Fixture {
    fn default() -> Self {
        Fixture { index: index() }
    }
}

impl Fixture {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Browse for Fixture {
    fn search(&self, request: &Request) -> Result<Page, Error> {
        let Request {
            query,
            filters,
            sort,
            offset,
            count,
            ..
        } = request;
        let order = super::order_of(*sort);
        Ok(self.index.search(query, filters, order, *offset, *count))
    }

    fn sorts(&self) -> Vec<Sort> {
        super::cache_sorts(&self.index)
    }

    fn counts(&self, request: &Request) -> Option<Counts> {
        Some(self.index.counts(&request.query, &request.filters))
    }

    fn catalog(&self) -> Catalog {
        Catalog::of_versions_with(&self.index, 1)
    }

    fn details(&self, ids: &[String]) -> Result<Vec<Details>, Error> {
        let one = |id: &String| {
            let entry = self.index.get(id)?;
            let number: u64 = id.parse().unwrap_or(0);
            Some(Details {
                id: id.clone(),
                title: entry.title.clone(),
                updated: 1_760_000_000 + number % 1000,
                created: 1_700_000_000 + number % 1000,
                size: 1_048_576 + number % 4096,
                preview_url: String::new(),
                description: format!(
                    "[b]{}[/b]\nA made-up item.\n[img]https://images.steamusercontent.com/ugc/{id}/A/[/img]\nAnd a second paragraph.",
                    entry.title
                ),
                views: 5000 + number % 7000,
                subscriptions: 1000 + number % 5000,
                favorited: 100 + number % 500,
                banned: false,
                creator: "76561198000000000".into(),
                app_id: 294_100,
                collection: false,
                ..Details::default()
            })
        };
        Ok(ids.iter().filter_map(one).collect())
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
        let mut all: Vec<String> = Vec::new();
        for id in ids {
            let these = match id.as_str() {
                COLLECTION => vec!["3100000001".to_string(), "3100000003".to_string()],
                _ => vec![id.clone()],
            };
            for item in these {
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
    use crate::rimworld::workshopsearch::{Filters, Kind, Query};

    fn ids(page: Result<Page, Error>) -> Vec<String> {
        page.unwrap().ids
    }

    fn ask(text: &str, filters: Filters) -> Vec<String> {
        let request = Request {
            query: Query::parse(text),
            filters,
            count: 50,
            ..Request::default()
        };
        ids(Fixture::new().search(&request))
    }

    #[test]
    fn it_finds_each_kind_of_item_it_makes_up() {
        let kind = |kind: Kind| Filters {
            include_tags: vec![kind.steam_tag().unwrap().into()],
            ..Filters::default()
        };
        assert_eq!(ask("", kind(Kind::Translation)), ["3100000005"]);
        assert_eq!(ask("", kind(Kind::Scenario)), ["3100000006"]);
        let version = Filters {
            include_tags: vec!["1.5".into()],
            ..Filters::default()
        };
        assert_eq!(ask("tidy", version), ["3100000003"]);
        assert_eq!(ask("d:1149640", Filters::default()), ["3100000004"]);
    }

    #[test]
    fn expansions_requirements_details_and_collections_are_answered_from_the_made_up_items() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.app_name("1149640").as_deref(),
            Some("Fixture Expansion One")
        );
        assert_eq!(fixture.app_name("294100"), None);
        assert!(fixture.needed_by(HARMONY).unwrap().len() >= 3);
        assert_eq!(fixture.requires("3100000002").unwrap().len(), 2);
        assert_eq!(fixture.entry("3100000002").unwrap().requires.len(), 3);
        assert!(fixture.entry("3100000003").unwrap().author.is_none());
        let found = fixture.details(&[HARMONY.into(), "1".into()]).unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].preview_url.is_empty());
        let items = fixture.collection_items(&[COLLECTION.into()]).unwrap();
        assert_eq!(items, ["3100000001", "3100000003"]);
        assert_eq!(fixture.collection_items(&["5".into()]).unwrap(), ["5"]);
    }
}
