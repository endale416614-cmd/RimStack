//! Kept as `{"package.id": seconds since 1970}`: since when each mod of the saved load order
//! has been in it, by its id folded (`id::fold`).

use crate::base::error::Error;
use crate::kept::launches::Launch;
use crate::rimworld::id::fold;
use std::collections::BTreeMap;
use std::path::Path;

pub type Since = BTreeMap<String, u64>;

/// `None` for a missing or damaged file.
pub fn read(file: &Path) -> Option<Since> {
    let kept = crate::base::atomic_file::read_json(file)?;
    serde_json::from_value(kept).ok()
}

pub fn write(file: &Path, since: &Since) -> Result<(), Error> {
    let text = serde_json::to_string(since).map_err(|e| Error::format(e.to_string()))?;
    crate::base::atomic_file::write(file, text)
}

/// Forgets what left the list and dates what joined it `at`; true when anything changed.
pub fn follow(since: &mut Since, active: &[String], at: u64) -> bool {
    let now: std::collections::HashSet<String> = active.iter().map(|id| fold(id)).collect();
    let before = since.len();
    since.retain(|id, _| now.contains(id));
    let mut changed = since.len() != before;
    for id in now {
        if let std::collections::btree_map::Entry::Vacant(slot) = since.entry(id) {
            slot.insert(at);
            changed = true;
        }
    }
    changed
}

/// For a first record, as nothing was kept before: each mod since the oldest of the game's
/// latest starts it was in without a break, else `at`.
pub fn first(active: &[String], launches: &[Launch], at: u64) -> Since {
    let mut since = Since::new();
    for id in active {
        let id = fold(id);
        let run = launches
            .iter()
            .rev()
            .take_while(|launch| launch.ids.iter().any(|other| fold(other) == id));
        let oldest = run.last().map_or(at, |launch| launch.time.min(at));
        since.insert(id, oldest);
    }
    since
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn a_mod_is_dated_when_it_joins_and_forgotten_when_it_leaves() {
        let mut since = Since::new();
        assert!(follow(&mut since, &ids(&["Ann.Roads", "core"]), 10));
        assert!(!follow(&mut since, &ids(&["ann.roads", "core"]), 20));
        assert_eq!(since["ann.roads"], 10);
        assert!(follow(&mut since, &ids(&["core"]), 30));
        assert!(follow(&mut since, &ids(&["core", "ann.roads"]), 40));
        assert_eq!(
            since,
            Since::from([("ann.roads".into(), 40), ("core".into(), 10)])
        );
    }

    #[test]
    fn a_first_record_goes_back_through_the_latest_starts_without_a_break() {
        let launch = |time, names: &[&str]| Launch {
            time,
            ids: ids(names),
            errors: None,
        };
        let launches = [
            launch(1, &["core", "roads"]),
            launch(2, &["core"]),
            launch(3, &["core", "roads"]),
        ];
        let since = first(&ids(&["Core", "roads", "hats"]), &launches, 9);
        assert_eq!(since["core"], 1);
        assert_eq!(since["roads"], 3);
        assert_eq!(since["hats"], 9);
    }

    #[test]
    fn a_missing_or_damaged_file_reads_as_none_and_a_written_one_back() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("activated.json");
        assert_eq!(read(&file), None);
        std::fs::write(&file, "not json").unwrap();
        assert_eq!(read(&file), None);
        let since = Since::from([("core".into(), 5)]);
        write(&file, &since).unwrap();
        assert_eq!(read(&file), Some(since));
    }
}
