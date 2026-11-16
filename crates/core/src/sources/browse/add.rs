use super::Browse;
use crate::kept::download_list::{DownloadList, Row, Why};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Installed,
    OnList,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Added {
    /// In the order they joined the list, the mod first.
    pub added: Vec<String>,
    pub refused: Option<Refusal>,
    pub unknown: Vec<String>,
}

pub fn add_with_requirements(
    list: &mut DownloadList,
    source: &dyn Browse,
    installed: &dyn Fn(&str) -> bool,
    id: &str,
) -> Added {
    let mut summary = Added::default();
    let root = source.entry(id);
    summary.refused = if installed(id) {
        Some(Refusal::Installed)
    } else if list.contains(id) {
        Some(Refusal::OnList)
    } else {
        let title = root.as_ref().map_or(id, |entry| entry.title.as_str());
        list.add(Row::new(id, title, Why::Added));
        summary.added.push(id.to_string());
        None
    };
    let mut seen: HashSet<String> = HashSet::from([id.to_string()]);
    let mut pending = vec![id.to_string()];
    while let Some(parent) = pending.pop() {
        let Some(entry) = source.entry(&parent) else {
            continue;
        };
        for needed in entry.requires {
            if !seen.insert(needed.clone()) {
                continue;
            }
            let Some(known) = source.entry(&needed) else {
                summary.unknown.push(needed);
                continue;
            };
            pending.push(needed.clone());
            if !installed(&needed)
                && list.add(Row::new(
                    &needed,
                    known.title,
                    Why::NeededBy(parent.clone()),
                ))
            {
                summary.added.push(needed);
            }
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rimworld::workshopindex::{Entry, Index};
    use crate::sources::browse::Cache;
    use std::sync::Arc;

    fn entry(id: &str, requires: &[&str]) -> Entry {
        Entry {
            id: id.into(),
            title: format!("Mod {id}"),
            author: None,
            package_id: None,
            tags: Vec::new(),
            requires: requires.iter().map(|r| r.to_string()).collect(),
            dlc: vec!["1149640".into()],
        }
    }

    fn source(entries: Vec<Entry>) -> Cache {
        Cache::new(Arc::new(Index::new(entries)))
    }

    fn nothing(_: &str) -> bool {
        false
    }

    #[test]
    fn a_mod_brings_what_it_requires_marked_by_who_needs_it() {
        let cache = source(vec![
            entry("100000", &["200000"]),
            entry("200000", &["300000"]),
            entry("300000", &[]),
        ]);
        let mut list = DownloadList::default();
        let done = add_with_requirements(&mut list, &cache, &nothing, "100000");
        assert_eq!(done.added, ["100000", "200000", "300000"]);
        assert_eq!(list.rows()[0].why, Why::Added);
        assert_eq!(list.rows()[1].why, Why::NeededBy("100000".into()));
        assert_eq!(list.rows()[2].why, Why::NeededBy("200000".into()));
        assert_eq!(list.rows()[2].title, "Mod 300000");
    }

    #[test]
    fn installed_and_listed_ones_are_left_and_the_mod_says_why_it_was_refused() {
        let cache = source(vec![
            entry("100000", &["200000", "300000"]),
            entry("200000", &[]),
            entry("300000", &[]),
        ]);
        let mut list = DownloadList::default();
        list.add(Row::new("300000", "Mod 300000", Why::Missing));
        let installed = |id: &str| id == "200000";
        let done = add_with_requirements(&mut list, &cache, &installed, "100000");
        assert_eq!(done.added, ["100000"]);
        assert_eq!(done.refused, None);
        assert_eq!(list.rows()[0].why, Why::Missing);

        let again = add_with_requirements(&mut list, &cache, &installed, "100000");
        assert_eq!(
            (again.added.len(), again.refused),
            (0, Some(Refusal::OnList))
        );
        let gone = add_with_requirements(&mut list, &cache, &|_: &str| true, "100000");
        assert_eq!(gone.refused, Some(Refusal::Installed));
        assert_eq!(list.ids(), ["300000", "100000"]);
    }

    #[test]
    fn a_requirement_the_source_does_not_know_is_reported_not_added() {
        let cache = source(vec![entry("100000", &["999999"])]);
        let mut list = DownloadList::default();
        let done = add_with_requirements(&mut list, &cache, &nothing, "100000");
        assert_eq!(done.added, ["100000"]);
        assert_eq!(done.unknown, ["999999"]);
        assert_eq!(list.ids(), ["100000"]);
    }

    #[test]
    fn a_cycle_ends_and_adds_each_once() {
        let cache = source(vec![
            entry("100000", &["200000"]),
            entry("200000", &["100000"]),
        ]);
        let mut list = DownloadList::default();
        let done = add_with_requirements(&mut list, &cache, &nothing, "100000");
        assert_eq!(done.added, ["100000", "200000"]);
        assert_eq!(list.ids(), ["100000", "200000"]);
    }

    #[test]
    fn a_mod_the_source_does_not_know_is_added_under_its_id() {
        let cache = source(Vec::new());
        let mut list = DownloadList::default();
        add_with_requirements(&mut list, &cache, &nothing, "123456");
        assert_eq!(list.rows()[0].title, "123456");
    }
}
