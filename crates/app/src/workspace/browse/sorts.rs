use rimstack_core::base::sentence::Sentence;
use rimstack_core::say;
use rimstack_core::sources::browse::Sort;
use serde::Serialize;

/// Steam keeps Trending for a window of 1 to 7 days.
const SORTS: [(&str, Sort); 6] = [
    ("relevance", Sort::Relevance),
    ("trending1", Sort::Trending { days: 1 }),
    ("trending7", Sort::Trending { days: 7 }),
    ("subscribed", Sort::MostSubscribed),
    ("recent", Sort::MostRecent),
    ("updated", Sort::LastUpdated),
];

pub fn sort_of(id: &str) -> Sort {
    SORTS
        .iter()
        .find(|(known, _)| *known == id)
        .map_or(Sort::Relevance, |(_, sort)| *sort)
}

#[cfg(test)]
pub fn id_of(sort: Sort) -> &'static str {
    SORTS
        .iter()
        .find(|(_, known)| *known == sort)
        .map_or("relevance", |(id, _)| id)
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SortOffer {
    pub id: &'static str,
    pub label: Sentence,
    pub chosen: bool,
}

fn label(sort: Sort, words: bool) -> Sentence {
    match sort {
        Sort::Relevance if words => say!("Best match"),
        Sort::Relevance => say!("Most popular"),
        Sort::Trending { days: 1 } => say!("Trending today"),
        Sort::Trending { .. } => say!("Trending this week"),
        Sort::MostSubscribed => say!("Most subscribed"),
        Sort::MostRecent => say!("Newest"),
        Sort::LastUpdated => say!("Recently updated"),
    }
}

pub fn offers(offered: &[Sort], words: bool, key_mode: bool) -> Vec<SortOffer> {
    SORTS
        .iter()
        .filter(|(_, sort)| offered.contains(sort))
        .filter(|(_, sort)| *sort != Sort::Relevance || words || key_mode)
        .map(|(id, sort)| SortOffer {
            id,
            label: label(*sort, words),
            chosen: false,
        })
        .collect()
}

pub fn effective(wanted: &str, offers: &[SortOffer], words: bool) -> &'static str {
    let offered = |id: &str| offers.iter().find(|offer| offer.id == id).map(|o| o.id);
    let default = if words { "relevance" } else { "recent" };
    offered(wanted)
        .or_else(|| offered(default))
        .or_else(|| offers.first().map(|offer| offer.id))
        .unwrap_or("relevance")
}

pub fn chosen(offers: &[SortOffer], id: &str) -> Vec<SortOffer> {
    offers
        .iter()
        .map(|offer| SortOffer {
            chosen: offer.id == id,
            ..offer.clone()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offered_sort_is_read_back_from_its_id_and_an_unknown_one_is_relevance() {
        let every: Vec<Sort> = SORTS.iter().map(|(_, sort)| *sort).collect();
        for offer in offers(&every, true, true) {
            assert_eq!(id_of(sort_of(offer.id)), offer.id);
        }
        assert_eq!(sort_of("nonsense"), Sort::Relevance);
        assert_eq!(sort_of("trending7"), Sort::Trending { days: 7 });
    }

    #[test]
    fn only_the_sorts_a_source_does_are_offered() {
        let cache = [Sort::Relevance, Sort::MostRecent, Sort::LastUpdated];
        let ids: Vec<&str> = offers(&cache, true, false).iter().map(|o| o.id).collect();
        assert_eq!(ids, ["relevance", "recent", "updated"]);
        let bare: Vec<&str> = offers(&cache, false, false).iter().map(|o| o.id).collect();
        assert_eq!(bare, ["recent", "updated"]);
        assert!(offers(&[], true, true).is_empty());
    }

    #[test]
    fn one_offer_is_chosen_by_its_id() {
        let every: Vec<Sort> = SORTS.iter().map(|(_, sort)| *sort).collect();
        let marked = chosen(&offers(&every, true, true), "subscribed");
        let ids: Vec<&str> = marked.iter().filter(|o| o.chosen).map(|o| o.id).collect();
        assert_eq!(ids, ["subscribed"]);
        assert!(
            chosen(&offers(&every, true, true), "nonsense")
                .iter()
                .all(|o| !o.chosen)
        );
    }

    #[test]
    fn the_sort_in_use_is_the_players_if_offered_else_the_newest_or_the_best_match() {
        let cache = [Sort::Relevance, Sort::MostRecent, Sort::LastUpdated];
        let words = offers(&cache, true, false);
        let bare = offers(&cache, false, false);
        assert_eq!(effective("", &words, true), "relevance");
        assert_eq!(effective("", &bare, false), "recent");
        assert_eq!(effective("updated", &bare, false), "updated");
        assert_eq!(effective("subscribed", &bare, false), "recent");
        assert_eq!(effective("relevance", &bare, false), "recent");
        assert_eq!(label(Sort::Relevance, false).to_string(), "Most popular");
    }
}
