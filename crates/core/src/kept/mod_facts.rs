//! `mod-facts.json`: `{"format":N,"game":"1.6","mods":{"<mod folder>":{"stamp":{…},"facts":{…}}}}`,
//! one record per mod, written whole. A file of another format or game version is read as empty.

use crate::base::atomic_file;
use crate::base::error::Error;
use crate::base::stamp::FolderStamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

pub const FORMAT: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Record<T> {
    stamp: FolderStamp,
    facts: Arc<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModFacts<T> {
    format: u32,
    game: String,
    mods: BTreeMap<String, Record<T>>,
}

impl<T> ModFacts<T> {
    pub fn empty(game: &str) -> Self {
        Self {
            format: FORMAT,
            game: game.to_string(),
            mods: BTreeMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.mods.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mods.is_empty()
    }

    pub fn get(&self, folder: &Path, stamp: FolderStamp) -> Option<Arc<T>> {
        let record = self.mods.get(&*folder.to_string_lossy())?;
        (record.stamp == stamp).then(|| record.facts.clone())
    }

    pub fn put(&mut self, folder: &Path, stamp: FolderStamp, facts: Arc<T>) {
        let key = folder.to_string_lossy().into_owned();
        self.mods.insert(key, Record { stamp, facts });
    }

    pub fn keep_only(&mut self, folders: &std::collections::HashSet<String>) {
        self.mods.retain(|folder, _| folders.contains(folder));
    }
}

impl<T: Serialize> ModFacts<T> {
    pub fn write(&self, file: &Path) -> Result<(), Error> {
        atomic_file::write_json(file, self)
    }
}

impl<T: for<'a> Deserialize<'a>> ModFacts<T> {
    pub fn read(file: &Path, game: &str) -> Self {
        let kept = std::fs::read(file)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Self>(&bytes).ok());
        kept.filter(|kept| kept.format == FORMAT && kept.game == game)
            .unwrap_or_else(|| Self::empty(game))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn stamp_of(dir: &Path) -> FolderStamp {
        FolderStamp::of(dir, |_| false)
    }

    #[test]
    fn a_record_is_given_back_while_the_stamp_holds_and_not_after_a_folder_changed() {
        let tmp = tempfile::tempdir().unwrap();
        let (folder, file) = (tmp.path().join("mod"), tmp.path().join("data/facts.json"));
        std::fs::create_dir_all(folder.join("Defs")).unwrap();
        let mut kept = ModFacts::<Vec<String>>::empty("1.6");
        kept.put(&folder, stamp_of(&folder), Arc::new(vec!["Gold".into()]));
        kept.write(&file).unwrap();
        let read = ModFacts::<Vec<String>>::read(&file, "1.6");
        assert_eq!(*read.get(&folder, stamp_of(&folder)).unwrap(), ["Gold"]);
        let later = SystemTime::now() + Duration::from_secs(500);
        let defs = std::fs::File::open(folder.join("Defs")).unwrap();
        defs.set_modified(later).unwrap();
        assert!(read.get(&folder, stamp_of(&folder)).is_none());
    }

    #[test]
    fn a_file_of_an_older_format_or_another_game_or_damaged_is_read_as_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("facts.json");
        let mut kept = ModFacts::<u8>::empty("1.6");
        kept.put(tmp.path(), stamp_of(tmp.path()), Arc::new(1));
        kept.write(&file).unwrap();
        assert_eq!(ModFacts::<u8>::read(&file, "1.6").len(), 1);
        assert!(ModFacts::<u8>::read(&file, "1.7").is_empty());
        let text = std::fs::read_to_string(&file).unwrap();
        let old = text.replacen(&format!("\"format\":{FORMAT}"), "\"format\":0", 1);
        std::fs::write(&file, old).unwrap();
        assert!(ModFacts::<u8>::read(&file, "1.6").is_empty());
        std::fs::write(&file, "damaged").unwrap();
        assert!(ModFacts::<u8>::read(&file, "1.6").is_empty());
    }
}
