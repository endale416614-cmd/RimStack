use super::workshop::ids_in;
use super::workshopindex::Index;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    Any,
    Mod,
    Translation,
    Scenario,
}

impl Kind {
    pub fn steam_tag(self) -> Option<&'static str> {
        match self {
            Kind::Any => None,
            Kind::Mod => Some("Mod"),
            Kind::Translation => Some("Translation"),
            Kind::Scenario => Some("Scenario"),
        }
    }
}

/// Tags and DLC are matched like Steam's own two-version search: every included one must be there
/// (two versions ticked finds the mods that carry both, checked on the site, 2026-11-10), and none
/// of the excluded ones may.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filters {
    /// Tags as Steam writes them (`Mod`, `1.6`); case is not significant.
    pub include_tags: Vec<String>,
    pub exclude_tags: Vec<String>,
    pub include_dlc: Vec<String>,
    pub exclude_dlc: Vec<String>,
    /// App ids of the DLC the player has: a mod needing another is left out. `None` leaves none out.
    pub owned_dlc: Option<Vec<String>>,
    pub show_incompatible: bool,
}

/// How hits are ordered. Without a sweep the dates and counts are unknown: every item ties, and
/// the highest id (the newest) comes first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Order {
    #[default]
    Relevance,
    Subscribed,
    Updated,
    Newest,
}

/// Words and values are lowercase.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    pub words: Vec<String>,
    /// `w:<id>` and pasted links: when there are any, only these mods are found.
    pub ids: Vec<String>,
    pub tags: Vec<String>,
    pub dlc: Vec<String>,
    pub requires: Vec<String>,
    pub authors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub ids: Vec<String>,
    pub total: usize,
}

fn pieces(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let (mut current, mut quoted) = (String::new(), false);
    for c in text.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => found.push(std::mem::take(&mut current)),
            c => current.push(c),
        }
    }
    found.push(current);
    found.retain(|piece| !piece.is_empty());
    found
}

impl Query {
    pub fn parse(text: &str) -> Query {
        let mut query = Query::default();
        for piece in pieces(text) {
            let keyed = piece.split_once(':').filter(|(_, v)| !v.is_empty());
            let digits = |v: &str| v.chars().all(|c| c.is_ascii_digit());
            match keyed.map(|(k, v)| (k.to_lowercase(), v)) {
                Some((k, v)) if k == "t" => query.tags.push(v.to_lowercase()),
                Some((k, v)) if k == "a" => query.authors.push(v.to_lowercase()),
                Some((k, v)) if k == "w" && digits(v) => query.ids.push(v.into()),
                Some((k, v)) if k == "d" && digits(v) => query.dlc.push(v.into()),
                Some((k, v)) if k == "m" && digits(v) => query.requires.push(v.into()),
                _ => {
                    let links = ids_in(&piece);
                    if links.is_empty() {
                        query.words.push(piece.to_lowercase());
                    }
                    for id in links {
                        if !query.ids.contains(&id) {
                            query.ids.push(id);
                        }
                    }
                }
            }
        }
        query
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Counts {
    pub total: usize,
    /// By tag, case folded into the first spelling met; only tags with results.
    pub tags: Vec<(String, usize)>,
    pub dlc: Vec<(String, usize)>,
}

#[derive(Clone, Copy)]
enum Held {
    IncludeTag,
    ExcludeTag(usize),
    IncludeDlc,
    ExcludeDlc(usize),
}

struct Plan<'a> {
    index: &'a Index,
    query: &'a Query,
    phrase: String,
    given: Vec<u64>,
    dlc: Vec<u64>,
    requires: Vec<u64>,
    tags: Vec<Vec<u32>>,
    include: Vec<Vec<u32>>,
    exclude: Vec<Vec<u32>>,
    include_dlc: Vec<u64>,
    exclude_dlc: Vec<u64>,
    owned: Option<Vec<u64>>,
    show_incompatible: bool,
}

struct Verdict {
    tier: u8,
    unmet: u8,
    last: Option<Held>,
}

impl<'a> Plan<'a> {
    fn new(index: &'a Index, query: &'a Query, filters: &Filters) -> Option<Plan<'a>> {
        let numbers = |texts: &[String]| -> Option<Vec<u64>> {
            texts.iter().map(|t| t.parse().ok()).collect()
        };
        let called = |names: &[String]| -> Vec<Vec<u32>> {
            names
                .iter()
                .map(|t| index.tags_called(&t.to_lowercase()))
                .collect()
        };
        Some(Plan {
            index,
            query,
            phrase: query.words.join(" "),
            given: numbers(&query.ids)?,
            dlc: numbers(&query.dlc)?,
            requires: numbers(&query.requires)?,
            tags: query.tags.iter().map(|t| index.tags_called(t)).collect(),
            include: called(&filters.include_tags),
            exclude: called(&filters.exclude_tags),
            include_dlc: numbers(&filters.include_dlc)?,
            exclude_dlc: numbers(&filters.exclude_dlc)?,
            owned: filters
                .owned_dlc
                .as_deref()
                .map(|o| o.iter().filter_map(|d| d.parse().ok()).collect()),
            show_incompatible: filters.show_incompatible,
        })
    }

    /// `None` when the query itself, or what the filters do not leave to the tags, rules it out.
    fn judge(&self, at: usize) -> Option<Verdict> {
        let (index, query) = (self.index, self.query);
        let (title, author) = (index.title_key(at), index.author_key(at));
        let by_ids = !query.ids.is_empty();
        let admitted = if by_ids {
            self.given.contains(&index.id_at(at))
        } else {
            self.owned
                .as_ref()
                .is_none_or(|owned| index.dlc_of(at).iter().all(|n| owned.contains(n)))
                && (self.show_incompatible || !index.incompatible(at))
        };
        let wanted = admitted
            && self.tags.iter().all(|t| index.has_any_tag(at, t))
            && self.dlc.iter().all(|d| index.dlc_of(at).contains(d))
            && self
                .requires
                .iter()
                .all(|m| index.requires_of(at).contains(m))
            && query.authors.iter().all(|a| author.contains(a.as_str()))
            && query.words.iter().all(|w| {
                [title, author, index.package_key(at)]
                    .iter()
                    .any(|s| s.contains(w.as_str()))
            });
        if !wanted {
            return None;
        }
        let tier = if query.words.is_empty() || title == self.phrase {
            0
        } else if title.starts_with(&self.phrase) {
            1
        } else if query.words.iter().all(|w| title.contains(w.as_str())) {
            2
        } else {
            3
        };
        let mut verdict = Verdict {
            tier,
            unmet: 0,
            last: None,
        };
        if by_ids {
            return Some(verdict);
        }
        let dlc = index.dlc_of(at);
        let mut unmet = |held, met: bool| {
            if !met {
                verdict.unmet = verdict.unmet.saturating_add(1);
                verdict.last = Some(held);
            }
        };
        for tags in &self.include {
            unmet(Held::IncludeTag, index.has_any_tag(at, tags));
        }
        for (i, tags) in self.exclude.iter().enumerate() {
            unmet(Held::ExcludeTag(i), !index.has_any_tag(at, tags));
        }
        for d in &self.include_dlc {
            unmet(Held::IncludeDlc, dlc.contains(d));
        }
        for (i, d) in self.exclude_dlc.iter().enumerate() {
            unmet(Held::ExcludeDlc(i), !dlc.contains(d));
        }
        Some(verdict)
    }
}

/// The `want` smallest by `key`, sorted: the rest is not ordered.
fn first_sorted<K: Ord>(hits: &mut Vec<(u8, usize)>, want: usize, key: impl Fn(&(u8, usize)) -> K) {
    if want < hits.len() {
        if want > 0 {
            hits.select_nth_unstable_by_key(want - 1, &key);
        }
        hits.truncate(want);
    }
    hits.sort_unstable_by_key(key);
}

impl Index {
    /// With no words and no other order, newest first: ids grow with time and the index keeps them in order.
    pub fn search(
        &self,
        query: &Query,
        filters: &Filters,
        order: Order,
        offset: usize,
        count: usize,
    ) -> Page {
        let Some(plan) = Plan::new(self, query, filters) else {
            return Page {
                ids: Vec::new(),
                total: 0,
            };
        };
        let mut hits: Vec<(u8, usize)> = (0..self.len())
            .filter_map(|at| {
                let verdict = plan.judge(at).filter(|v| v.unmet == 0)?;
                Some((verdict.tier, at))
            })
            .collect();
        let total = hits.len();
        let id = |at: usize| std::cmp::Reverse(self.id_at(at));
        let newest_first = query.words.is_empty() && order == Order::Relevance;
        let want = offset.saturating_add(count);
        type Hit = (u8, usize);
        match order {
            _ if newest_first => hits.reverse(),
            Order::Relevance => {
                let rank = |&(tier, at): &Hit| (tier, self.title_key(at), at);
                first_sorted(&mut hits, want, rank);
            }
            Order::Subscribed => first_sorted(&mut hits, want, |&(_, at): &Hit| {
                (std::cmp::Reverse(self.stat(at).subscriptions), id(at))
            }),
            Order::Updated => first_sorted(&mut hits, want, |&(_, at): &Hit| {
                (std::cmp::Reverse(self.stat(at).updated), id(at))
            }),
            Order::Newest => first_sorted(&mut hits, want, |&(_, at): &Hit| {
                (std::cmp::Reverse(self.stat(at).created), id(at))
            }),
        }
        let ids = hits
            .iter()
            .skip(offset)
            .take(count)
            .map(|&(_, at)| self.id_at(at).to_string())
            .collect();
        Page { ids, total }
    }

    /// The results of the query and filters, and under each tag and DLC how many a result has. A
    /// tag or DLC the filters include or exclude is counted as if that one filter were undone, so
    /// an excluded tag shows how many it hides and a ticked one how many it keeps. One pass.
    pub fn counts(&self, query: &Query, filters: &Filters) -> Counts {
        let Some(plan) = Plan::new(self, query, filters) else {
            return Counts::default();
        };
        let mut tags = vec![0usize; self.tag_count()];
        let mut dlc: HashMap<u64, usize> = HashMap::new();
        let mut total = 0;
        for at in 0..self.len() {
            let Some(verdict) = plan.judge(at) else {
                continue;
            };
            let own = self.tags_of(at);
            let firsts = own
                .iter()
                .enumerate()
                .filter(|&(i, t)| !own[..i].contains(t))
                .map(|(_, &t)| t);
            match (verdict.unmet, verdict.last) {
                (0, _) => {
                    total += 1;
                    firsts.for_each(|t| tags[t as usize] += 1);
                    for &d in self.dlc_of(at) {
                        *dlc.entry(d).or_default() += 1;
                    }
                }
                (1, Some(Held::ExcludeTag(i))) => {
                    if let Some(&t) = plan.exclude[i].iter().find(|t| own.contains(t)) {
                        tags[t as usize] += 1;
                    }
                }
                (1, Some(Held::ExcludeDlc(i))) => {
                    let wanted = plan.exclude_dlc[i];
                    if self.dlc_of(at).contains(&wanted) {
                        *dlc.entry(wanted).or_default() += 1;
                    }
                }
                _ => {}
            }
        }
        let mut folded: BTreeMap<String, (String, usize)> = BTreeMap::new();
        for (t, &n) in tags.iter().enumerate().filter(|&(_, &n)| n > 0) {
            let name = self.tag_name(t as u32);
            folded
                .entry(name.to_lowercase())
                .or_insert((name.to_string(), 0))
                .1 += n;
        }
        let mut dlc: Vec<(String, usize)> =
            dlc.into_iter().map(|(d, n)| (d.to_string(), n)).collect();
        dlc.sort();
        Counts {
            total,
            tags: folded.into_values().collect(),
            dlc,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::workshopindex::{Entry, entry};
    use super::*;
    use std::collections::BTreeMap;

    fn index() -> Index {
        let mut with_pkg = entry("600000", "Unrelated", Some("Someone"), &["Mod", "1.5"]);
        with_pkg.package_id = Some("vanillaexpanded.harmony".into());
        let mut needs = entry("700000", "Needs Stuff", Some("Ann Lee"), &["Mod", "1.6"]);
        needs.requires = vec!["100000".into()];
        needs.dlc = vec!["1826140".into()];
        let all: Vec<Entry> = vec![
            entry("100000", "Harmony", Some("Brrainz"), &["Mod", "1.6", "1.5"]),
            entry("200000", "Harmony Patches Extra", None, &["Mod", "1.6"]),
            entry("300000", "More Harmony", None, &["Translation", "1.6"]),
            entry("400000", "Zeta Harmony tweaks", None, &["Scenario"]),
            entry("500000", "Éclair", Some("Brrainz"), &["Mod"]),
            with_pkg,
            needs,
        ];
        Index::new(all)
    }

    fn found(text: &str, filters: &Filters) -> Vec<String> {
        index()
            .search(&Query::parse(text), filters, Order::Relevance, 0, 100)
            .ids
    }

    #[test]
    fn the_box_is_split_into_words_and_keys() {
        let q = Query::parse(
            r#"Big "Red Mod" t:Mod d:1826140 m:100000 A:"Ann Lee" w:42 https://steamcommunity.com/sharedfiles/filedetails/?id=2009463077 818773962 w:abc d:x"#,
        );
        assert_eq!(q.words, ["big", "red mod", "w:abc", "d:x"]);
        assert_eq!(
            (q.tags, q.dlc, q.requires),
            (
                vec!["mod".to_string()],
                vec!["1826140".to_string()],
                vec!["100000".to_string()]
            )
        );
        assert_eq!(q.authors, ["ann lee"]);
        assert_eq!(q.ids, ["42", "2009463077", "818773962"]);
        assert_eq!(Query::parse("  "), Query::default());
    }

    #[test]
    fn ranking_puts_the_title_first_then_its_start_then_inside_then_the_rest() {
        let got = found("harmony", &Filters::default());
        assert_eq!(got, ["100000", "200000", "300000", "400000", "600000"]);
        assert_eq!(found("harmony patches", &Filters::default()), ["200000"]);
        assert_eq!(found("HARMONY", &Filters::default())[0], "100000");
    }

    #[test]
    fn the_author_and_package_id_are_searched_after_the_title() {
        assert_eq!(found("brrainz", &Filters::default()), ["100000", "500000"]);
        assert_eq!(found("vanillaexpanded", &Filters::default()), ["600000"]);
        assert_eq!(found("éCLAIR", &Filters::default()), ["500000"]);
    }

    #[test]
    fn keys_narrow_the_search() {
        let any = Filters::default();
        assert_eq!(found("t:translation", &any), ["300000"]);
        assert_eq!(found("m:100000", &any), ["700000"]);
        assert_eq!(found("d:1826140 harmony", &any), Vec::<String>::new());
        assert_eq!(found("d:1826140", &any), ["700000"]);
        assert_eq!(found("a:ann", &any), ["700000"]);
        assert_eq!(found("w:300000 harmony", &any), ["300000"]);
        assert_eq!(
            found(
                "https://steamcommunity.com/sharedfiles/filedetails/?id=200000",
                &any
            ),
            ["200000"]
        );
    }

    #[test]
    fn filters_go_by_steams_tags_and_leave_given_ids_alone() {
        let v16 = Filters {
            include_tags: vec!["1.6".into(), "mod".into()],
            ..Default::default()
        };
        assert_eq!(found("harmony", &v16), ["100000", "200000"]);
        assert_eq!(found("w:300000", &v16), ["300000"]);
        let scenario = Filters {
            include_tags: vec![Kind::Scenario.steam_tag().unwrap().into()],
            ..Default::default()
        };
        assert_eq!(found("", &scenario), ["400000"]);
        let none_owned = Filters {
            owned_dlc: Some(vec![]),
            ..Default::default()
        };
        assert!(!found("", &none_owned).contains(&"700000".to_string()));
        let owned = Filters {
            owned_dlc: Some(vec!["1826140".into()]),
            ..Default::default()
        };
        assert!(found("", &owned).contains(&"700000".to_string()));
    }

    #[test]
    fn with_no_words_the_newest_come_first_and_with_words_the_best() {
        let all = index().search(
            &Query::default(),
            &Filters::default(),
            Order::Relevance,
            0,
            10,
        );
        assert_eq!(all.ids.first().map(String::as_str), Some("700000"));
        assert!(all.ids.windows(2).all(|w| w[0] > w[1]), "{:?}", all.ids);
    }

    #[test]
    fn a_page_is_a_slice_of_the_whole() {
        let page = index().search(
            &Query::parse("harmony"),
            &Filters::default(),
            Order::Relevance,
            1,
            2,
        );
        assert_eq!(
            (page.total, page.ids),
            (5, vec!["200000".to_string(), "300000".to_string()])
        );
        let past = index().search(
            &Query::default(),
            &Filters::default(),
            Order::Relevance,
            50,
            10,
        );
        assert_eq!((past.total, past.ids.len()), (7, 0));
    }

    #[test]
    fn a_title_padded_with_spaces_sorts_and_matches_by_its_text() {
        let index = Index::new(vec![
            entry("100000", "  Zebra", None, &["Mod"]),
            entry("200000", "Apple", None, &["Mod"]),
        ]);
        let all = index.search(
            &Query::parse("a"),
            &Filters::default(),
            Order::Relevance,
            0,
            10,
        );
        assert_eq!(all.ids, ["200000", "100000"]);
        assert_eq!(index.get("100000").unwrap().title, "Zebra");
    }

    fn swept() -> Index {
        use super::super::workshopsweep::{Record, Sweep};
        let record = |subs, created, updated, tags: &[&str]| Record {
            title: "Steam title".into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            subscriptions: subs,
            created,
            updated,
            ..Record::default()
        };
        let mut banned = record(1, 1, 1, &["Mod", "1.7"]);
        banned.banned = true;
        let sweep = Sweep {
            completed: 1,
            resume_after: 0,
            records: BTreeMap::from([
                (100000, record(50, 10, 900, &["Mod", "1.7"])),
                (200000, record(500, 30, 100, &["Mod", "1.7", "1.6"])),
                (300000, banned),
                (400000, Record::removed()),
            ]),
        };
        index().with_sweep(&sweep)
    }

    fn order(index: &Index, order: Order, filters: &Filters) -> Vec<String> {
        index.search(&Query::default(), filters, order, 0, 100).ids
    }

    #[test]
    fn the_sweep_wins_per_item_and_leaves_the_rest_to_the_database() {
        let index = swept();
        let one = index.get("100000").unwrap();
        assert_eq!(
            (one.title.as_str(), one.tags.clone()),
            ("Steam title", vec!["Mod".to_string(), "1.7".into()])
        );
        let kept = index.get("500000").unwrap();
        assert_eq!(
            (kept.title.as_str(), kept.tags.clone()),
            ("Éclair", vec!["Mod".to_string()])
        );
        assert_eq!(
            index.version_tags(1).last().map(String::as_str),
            Some("1.7")
        );
    }

    #[test]
    fn banned_and_removed_items_are_hidden_until_asked_for_but_not_when_named() {
        let index = swept();
        let shown = order(&index, Order::Relevance, &Filters::default());
        assert!(!shown.contains(&"300000".to_string()) && !shown.contains(&"400000".to_string()));
        let all = Filters {
            show_incompatible: true,
            ..Default::default()
        };
        let shown = order(&index, Order::Relevance, &all);
        assert!(shown.contains(&"300000".to_string()) && shown.contains(&"400000".to_string()));
        let named = index.search(
            &Query::parse("w:400000"),
            &Filters::default(),
            Order::Relevance,
            0,
            10,
        );
        assert_eq!(named.ids, ["400000"]);
    }

    #[test]
    fn the_sorts_follow_the_sweep_and_tie_to_the_highest_id() {
        let fresh = swept();
        let any = Filters::default();
        let top = |o| order(&fresh, o, &any)[..2].to_vec();
        assert_eq!(top(Order::Subscribed), ["200000", "100000"]);
        assert_eq!(top(Order::Updated), ["100000", "200000"]);
        assert_eq!(top(Order::Newest), ["200000", "100000"]);
        let unswept = order(&index(), Order::Subscribed, &any);
        assert_eq!(unswept, order(&index(), Order::Newest, &any));
        assert_eq!(unswept[0], "700000");
        assert!(!index().swept() && fresh.swept());
    }

    #[test]
    fn included_tags_must_all_be_there_and_excluded_ones_none() {
        let both = Filters {
            include_tags: vec!["1.6".into(), "1.5".into()],
            ..Default::default()
        };
        assert_eq!(found("", &both), ["100000"]);
        let one = Filters {
            include_tags: vec!["1.6".into()],
            exclude_tags: vec!["Translation".into(), "SCENARIO".into()],
            ..Default::default()
        };
        assert_eq!(found("", &one), ["700000", "200000", "100000"]);
        assert!(found("", &Filters::default()).len() == 7);
    }

    #[test]
    fn included_dlc_must_be_required_and_excluded_dlc_not() {
        let with = Filters {
            include_dlc: vec!["1826140".into()],
            ..Default::default()
        };
        assert_eq!(found("", &with), ["700000"]);
        let without = Filters {
            exclude_dlc: vec!["1826140".into()],
            ..Default::default()
        };
        assert_eq!(found("", &without).len(), 6);
        let nonsense = Filters {
            include_dlc: vec!["x".into()],
            ..Default::default()
        };
        assert!(found("", &nonsense).is_empty());
    }

    fn tag(counts: &Counts, name: &str) -> usize {
        counts
            .tags
            .iter()
            .find(|(t, _)| t == name)
            .map_or(0, |(_, n)| *n)
    }

    #[test]
    fn counts_are_per_tag_and_dlc_over_the_results() {
        let index = index();
        let all = index.counts(&Query::default(), &Filters::default());
        assert_eq!(
            (
                all.total,
                tag(&all, "Mod"),
                tag(&all, "Translation"),
                tag(&all, "1.6")
            ),
            (7, 5, 1, 4)
        );
        assert_eq!(all.dlc, [("1826140".to_string(), 1)]);
        let harmony = index.counts(&Query::parse("harmony"), &Filters::default());
        assert_eq!((harmony.total, tag(&harmony, "Mod")), (5, 3));
        let v15 = Filters {
            include_tags: vec!["1.5".into()],
            ..Default::default()
        };
        let counted = index.counts(&Query::default(), &v15);
        assert_eq!(
            (counted.total, tag(&counted, "1.6"), tag(&counted, "Mod")),
            (2, 1, 2)
        );
    }

    #[test]
    fn an_excluded_tag_counts_what_it_hides_and_a_ticked_one_what_it_keeps() {
        let index = index();
        let filters = Filters {
            include_tags: vec!["1.6".into()],
            exclude_tags: vec!["Translation".into()],
            exclude_dlc: vec!["1826140".into()],
            ..Default::default()
        };
        let counts = index.counts(&Query::default(), &filters);
        assert_eq!(counts.total, 2);
        assert_eq!((tag(&counts, "Translation"), tag(&counts, "1.6")), (1, 2));
        assert_eq!(counts.dlc, [("1826140".to_string(), 1)]);
        let page = index.search(&Query::default(), &filters, Order::Relevance, 0, 10);
        assert_eq!(page.total, counts.total);
    }

    #[test]
    fn counting_the_whole_index_is_quick() {
        let all: Vec<Entry> = (1..=60_000u64)
            .map(|n| {
                entry(
                    &n.to_string(),
                    &format!("mod {n}"),
                    None,
                    &["Mod", if n % 2 == 0 { "1.6" } else { "1.7" }, "1.5"],
                )
            })
            .collect();
        let index = Index::new(all);
        let filters = Filters {
            include_tags: vec!["1.5".into()],
            exclude_tags: vec!["Scenario".into()],
            ..Default::default()
        };
        let started = std::time::Instant::now();
        let counts = index.counts(&Query::parse("mod"), &filters);
        let took = started.elapsed();
        assert_eq!(counts.total, 60_000);
        eprintln!("counts over 60k: {took:?}");
        assert!(took.as_millis() < 200, "{took:?}");
    }
}
