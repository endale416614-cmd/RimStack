use crate::base::error::Error;
use crate::said;

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Saved {
    pub name: String,
    pub mods: usize,
    /// Seconds since 1970.
    pub modified: u64,
}

/// Characters a file name cannot hold become `_`: names differing only so share a file,
/// and `save` refuses the second.
fn file_for(dir: &Path, name: &str) -> PathBuf {
    let safe: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_.()".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    dir.join(format!("{safe}.json"))
}

pub fn save(dir: &Path, name: &str, ids: &[String]) -> Result<(), Error> {
    if name.trim().is_empty() {
        return Err(Error::refused(said!("Give the list a name.")));
    }
    let path = file_for(dir, name);
    if let Some((other, _)) = read(&path).filter(|(kept, _)| kept != name.trim()) {
        return Err(Error::refused(said!(
            "The list {} already uses the same file name. Give this one another name.",
            other
        )));
    }
    let text = json!({ "name": name.trim(), "ids": ids }).to_string();
    crate::base::atomic_file::write(&path, text)
}

fn read(path: &Path) -> Option<(String, Vec<String>)> {
    let root: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let ids = root.get("ids")?.as_array()?;
    let ids = ids
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    Some((root.get("name")?.as_str()?.to_string(), ids))
}

pub fn all(dir: &Path) -> Vec<Saved> {
    let mut found: Vec<Saved> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let (name, ids) = read(&entry.path())?;
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some(Saved {
                name,
                mods: ids.len(),
                modified: modified.duration_since(UNIX_EPOCH).ok()?.as_secs(),
            })
        })
        .collect();
    found.sort_by_key(|s| s.name.to_lowercase());
    found
}

/// Counts files without opening them: it runs on every change of the load order.
pub fn count(dir: &Path) -> usize {
    let files = std::fs::read_dir(dir).into_iter().flatten().flatten();
    let is_list = |entry: &std::fs::DirEntry| entry.path().extension().is_some_and(|x| x == "json");
    files.filter(is_list).count()
}

pub fn prune(dir: &Path, keep: usize) {
    let mut found = all(dir);
    found.sort_by_key(|s| std::cmp::Reverse(s.modified));
    for old in found.iter().skip(keep) {
        // Best effort: a snapshot that cannot be deleted is only clutter.
        let _ = delete(dir, &old.name);
    }
}

pub fn taken(dir: &Path, name: &str) -> bool {
    file_for(dir, name).exists()
}

pub fn load(dir: &Path, name: &str) -> Result<Vec<String>, Error> {
    let path = file_for(dir, name);
    read(&path)
        .map(|(_, ids)| ids)
        .ok_or_else(|| Error::format(said!("{} is not a saved list", path.display())))
}

pub fn delete(dir: &Path, name: &str) -> Result<(), Error> {
    let path = file_for(dir, name);
    std::fs::remove_file(&path).map_err(|e| Error::file(&path, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_are_counted_by_their_files() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(count(tmp.path()), 0);
        save(tmp.path(), "one", &["a".to_string()]).unwrap();
        save(tmp.path(), "two", &[]).unwrap();
        std::fs::write(tmp.path().join("notes.txt"), "").unwrap();
        assert_eq!((count(tmp.path()), all(tmp.path()).len()), (2, 2));
        assert_eq!(count(&tmp.path().join("absent")), 0);
    }

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_list_is_kept_listed_loaded_and_deleted_by_name() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("lists");
        assert!(all(&dir).is_empty());
        save(&dir, "Medieval run", &ids(&["a", "b"])).unwrap();
        save(&dir, "base", &ids(&["a"])).unwrap();
        let found = all(&dir);
        let names: Vec<&str> = found.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["base", "Medieval run"]);
        assert_eq!(found[1].mods, 2);
        assert_eq!(load(&dir, "Medieval run").unwrap(), ids(&["a", "b"]));
        delete(&dir, "base").unwrap();
        assert_eq!(all(&dir).len(), 1);
        assert!(load(&dir, "base").is_err());
    }

    #[test]
    fn saving_under_the_same_name_replaces_the_list() {
        let tmp = tempfile::tempdir().unwrap();
        save(tmp.path(), "run", &ids(&["a"])).unwrap();
        save(tmp.path(), " run ", &ids(&["b", "c"])).unwrap();
        assert_eq!(all(tmp.path()).len(), 1);
        assert_eq!(load(tmp.path(), "run").unwrap(), ids(&["b", "c"]));
    }

    #[test]
    fn a_name_cannot_leave_the_folder_or_be_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("lists");
        save(&dir, "../../escape/it", &ids(&["a"])).unwrap();
        assert!(!tmp.path().join("escape").exists());
        assert_eq!(all(&dir)[0].name, "../../escape/it");
        assert!(save(&dir, "  ", &ids(&["a"])).is_err());
    }

    #[test]
    fn pruning_keeps_the_given_number_of_lists() {
        let tmp = tempfile::tempdir().unwrap();
        for n in 0..5 {
            save(tmp.path(), &format!("snapshot {n}"), &ids(&["a"])).unwrap();
        }
        prune(tmp.path(), 3);
        assert_eq!(all(tmp.path()).len(), 3);
        prune(tmp.path(), 10);
        assert_eq!(all(tmp.path()).len(), 3);
    }

    #[test]
    fn other_files_in_the_folder_are_not_lists() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("notes.json"), "[1,2]").unwrap();
        std::fs::write(tmp.path().join("x.txt"), "hello").unwrap();
        assert!(all(tmp.path()).is_empty());
    }

    #[test]
    fn saving_a_list_never_replaces_another_whose_name_is_written_alike() {
        let tmp = tempfile::tempdir().unwrap();
        save(tmp.path(), "war_crimes", &ids(&["first.list"])).unwrap();
        let _ = save(tmp.path(), "war/crimes", &ids(&["second.list"]));
        assert_eq!(
            load(tmp.path(), "war_crimes").unwrap(),
            ids(&["first.list"])
        );
    }
}
