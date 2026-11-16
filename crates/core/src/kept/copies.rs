//! Kept in `steamcmd-copies.json`: the mods SteamCMD put in the game's Mods folder for RimStack,
//! by Workshop id, `{"<id>": {"manifest": 1, "updated": 2, "refused": {...}}}`.

use crate::base::error::Error;
use crate::rimworld::workshop::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Copies(BTreeMap<String, Copy>);

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Copy {
    #[serde(flatten)]
    pub have: Version,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refused: Option<Refused>,
}

/// An update that failed, not tried again by itself while Steam offers the same version.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Refused {
    /// As `Sentence::to_kept` writes it.
    pub why: String,
    pub offered: Version,
}

impl Copies {
    /// A missing or damaged file is no copy yet.
    pub fn read(file: &Path) -> Self {
        let kept = crate::base::atomic_file::read_json(file).map(serde_json::from_value);
        kept.and_then(Result::ok).unwrap_or_default()
    }

    pub fn write(&self, file: &Path) -> Result<(), Error> {
        crate::base::atomic_file::write_json(file, self)
    }

    pub fn get(&self, id: &str) -> Option<&Copy> {
        self.0.get(id)
    }

    pub fn downloaded(&mut self, id: &str, have: Version) {
        self.0.insert(
            id.to_string(),
            Copy {
                have,
                refused: None,
            },
        );
    }

    /// Only a copy already kept: one never downloaded has nothing to hold back.
    pub fn refuse(&mut self, id: &str, why: String, offered: Version) {
        if let Some(copy) = self.0.get_mut(id) {
            copy.refused = Some(Refused { why, offered });
        }
    }

    /// Why its update failed, while Steam still offers the version it failed at.
    pub fn held_back(&self, id: &str, offered: Version) -> Option<&str> {
        let refused = self.get(id)?.refused.as_ref()?;
        (refused.offered == offered).then_some(refused.why.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(manifest: u64, updated: u64) -> Version {
        Version { manifest, updated }
    }

    #[test]
    fn a_copy_is_kept_and_read_back_and_a_damaged_file_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("steamcmd-copies.json");
        assert_eq!(Copies::read(&file), Copies::default());
        let mut copies = Copies::default();
        copies.downloaded("123", at(7, 70));
        copies.refuse("123", "Timeout".into(), at(8, 80));
        copies.write(&file).unwrap();
        assert_eq!(Copies::read(&file), copies);
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(
            text.starts_with(r#"{"123":{"manifest":7,"updated":70,"#),
            "{text}"
        );
        std::fs::write(&file, "{").unwrap();
        assert_eq!(Copies::read(&file), Copies::default());
    }

    #[test]
    fn a_failed_update_waits_for_another_version_and_a_download_clears_it() {
        let mut copies = Copies::default();
        copies.refuse("9", "never downloaded".into(), at(1, 1));
        assert_eq!(copies.get("9"), None);
        copies.downloaded("123", at(7, 70));
        copies.refuse("123", "Timeout".into(), at(8, 80));
        assert_eq!(copies.held_back("123", at(8, 80)), Some("Timeout"));
        assert_eq!(copies.held_back("123", at(9, 90)), None);
        copies.downloaded("123", at(9, 90));
        assert_eq!(copies.held_back("123", at(9, 90)), None);
    }
}
