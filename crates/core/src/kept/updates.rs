use crate::rimworld::paths::Source;
use crate::rimworld::workshop::{Details, Version};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

/// Kept in `updates.json`; all empty before the first check.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LastCheck {
    /// Seconds since 1970 when Steam last answered.
    pub checked: Option<u64>,
    pub asked: Option<u64>,
    /// Seconds since 1970 when each Workshop item was last updated, by Workshop id.
    pub now: BTreeMap<String, u64>,
    /// Each one's `hcontent_file`, where Steam gave it; a check before RimStack kept them has none.
    pub manifests: BTreeMap<String, u64>,
    /// Asked and not answered: unlisted by their authors, which Steam answers only to a login.
    pub unanswered: BTreeSet<String>,
    pub checkouts: BTreeMap<String, Remote>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Remote {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behind: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
    /// Why the last update of the checkout did not apply, as `Sentence::to_kept` writes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refused: Option<String>,
}

impl LastCheck {
    pub fn answered(&mut self, asked: &[String], now: &[Details], at: u64) {
        self.checked = Some(at);
        self.asked = Some(asked.len() as u64);
        self.now = now.iter().map(|d| (d.id.clone(), d.updated)).collect();
        self.manifests = now
            .iter()
            .filter(|d| d.manifest != 0)
            .map(|d| (d.id.clone(), d.manifest))
            .collect();
        let answered: BTreeSet<&String> = now.iter().map(|d| &d.id).collect();
        let unanswered = asked.iter().filter(|id| !answered.contains(id));
        self.unanswered = unanswered.cloned().collect();
    }

    pub fn offered(&self) -> HashMap<String, Version> {
        let version = |(id, &updated): (&String, &u64)| {
            let manifest = self.manifests.get(id).copied().unwrap_or(0);
            (id.clone(), Version { manifest, updated })
        };
        self.now.iter().map(version).collect()
    }

    /// A missing or damaged file is no check yet.
    pub fn read(file: &Path) -> Self {
        let kept = crate::base::atomic_file::read_json(file).map(serde_json::from_value);
        kept.and_then(Result::ok).unwrap_or_default()
    }

    /// A refusal stays while the remote is as far ahead as when it was refused:
    /// an update tried again would meet it again.
    pub fn keep_refusals(&mut self, before: &BTreeMap<String, Remote>) {
        for (folder, now) in &mut self.checkouts {
            let Some(then) = before.get(folder) else {
                continue;
            };
            if then.refused.is_some() && then.behind == now.behind {
                now.refused.clone_from(&then.refused);
            }
        }
    }

    pub fn write(&self, file: &Path) -> Result<(), crate::base::error::Error> {
        crate::base::atomic_file::write_json(file, self)
    }
}

/// Workshop folders belong to Steam, so a checkout there is asked only on request.
pub fn asks_git(source: Source, checkout: bool, git_for_workshop: bool) -> bool {
    checkout
        && match source {
            Source::Local => true,
            Source::Workshop => git_for_workshop,
            Source::Official => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_is_kept_and_read_back_and_an_older_file_still_reads() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("updates.json");
        assert_eq!(LastCheck::read(&file), LastCheck::default());
        let check = LastCheck {
            checked: Some(10),
            asked: Some(2),
            now: BTreeMap::from([("100".to_string(), 7)]),
            manifests: BTreeMap::from([("100".to_string(), 70)]),
            unanswered: BTreeSet::from(["200".to_string()]),
            checkouts: BTreeMap::from([
                (
                    "/mods/a".to_string(),
                    Remote {
                        behind: Some(3),
                        problem: None,
                        refused: Some("local changes".into()),
                    },
                ),
                (
                    "/mods/b".to_string(),
                    Remote {
                        behind: None,
                        problem: Some("offline".into()),
                        refused: None,
                    },
                ),
            ]),
        };
        check.write(&file).unwrap();
        assert_eq!(LastCheck::read(&file), check);
        std::fs::write(
            &file,
            r#"{"checked":5,"asked":1,"now":{"9":4},"checkouts":{"/m":{"behind":0}}}"#,
        )
        .unwrap();
        let older = LastCheck::read(&file);
        assert_eq!(
            (older.checked, older.now["9"], older.checkouts["/m"].behind),
            (Some(5), 4, Some(0))
        );
        assert_eq!(
            older.offered()["9"],
            Version {
                manifest: 0,
                updated: 4
            }
        );
    }

    #[test]
    fn an_answer_keeps_each_version_and_what_steam_said_nothing_of() {
        let details = |id: &str, manifest| Details {
            id: id.to_string(),
            updated: 5,
            manifest,
            ..Details::default()
        };
        let asked = ["1".to_string(), "2".to_string(), "3".to_string()];
        let mut check = LastCheck::default();
        check.answered(&asked, &[details("1", 11), details("2", 0)], 99);
        assert_eq!((check.checked, check.asked), (Some(99), Some(3)));
        let offered = check.offered();
        assert_eq!(
            offered["1"],
            Version {
                manifest: 11,
                updated: 5
            }
        );
        assert_eq!(offered["2"].manifest, 0);
        assert_eq!(check.unanswered, BTreeSet::from(["3".to_string()]));
    }

    #[test]
    fn a_refusal_lasts_until_the_remote_moves() {
        let refused = |behind| Remote {
            behind: Some(behind),
            problem: None,
            refused: Some("local changes".into()),
        };
        let before = BTreeMap::from([
            ("/same".to_string(), refused(2)),
            ("/moved".to_string(), refused(2)),
        ]);
        let fresh = |behind| Remote {
            behind: Some(behind),
            ..Remote::default()
        };
        let mut check = LastCheck {
            checkouts: BTreeMap::from([
                ("/same".to_string(), fresh(2)),
                ("/moved".to_string(), fresh(3)),
                ("/new".to_string(), fresh(1)),
            ]),
            ..LastCheck::default()
        };
        check.keep_refusals(&before);
        assert_eq!(check.checkouts["/same"], refused(2));
        assert_eq!(check.checkouts["/moved"].refused, None);
        assert_eq!(check.checkouts["/new"].refused, None);
    }

    #[test]
    fn a_local_checkout_is_asked_and_a_workshop_one_only_on_request() {
        assert!(asks_git(Source::Local, true, false));
        assert!(!asks_git(Source::Workshop, true, false));
        assert!(asks_git(Source::Workshop, true, true));
    }

    #[test]
    fn nothing_but_a_checkout_of_a_mod_is_asked() {
        for on in [false, true] {
            assert!(!asks_git(Source::Local, false, on));
            assert!(!asks_git(Source::Workshop, false, on));
            assert!(!asks_git(Source::Official, true, on));
        }
    }
}
