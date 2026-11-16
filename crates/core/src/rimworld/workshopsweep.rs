//! Kept as JSON lines. The first: `["sweep", 1, completed, resume_after]`, seconds since 1970 for
//! `completed` (0 before a pass has finished) and the Workshop id a stopped pass reached (0 for none).
//! Then one line per item, by id: `[id, title, [tags], size, subscriptions, favorited, time_created,
//! time_updated, banned, consumer app id]`, or `[id]` for an item Steam no longer returns.

use super::workshop::Details;
use crate::base::error::Error;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};

pub const DAY: u64 = 86_400;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record {
    pub title: String,
    pub tags: Vec<String>,
    pub size: u64,
    pub subscriptions: u64,
    pub favorited: u64,
    pub created: u64,
    pub updated: u64,
    pub banned: bool,
    pub app_id: u64,
    pub removed: bool,
}

impl Record {
    pub fn removed() -> Record {
        Record {
            removed: true,
            ..Record::default()
        }
    }

    pub fn of(details: &Details) -> Record {
        Record {
            title: details.title.clone(),
            tags: details.tags.clone(),
            size: details.size,
            subscriptions: details.subscriptions,
            favorited: details.favorited,
            created: details.created,
            updated: details.updated,
            banned: details.banned,
            app_id: details.app_id,
            removed: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sweep {
    pub completed: u64,
    pub resume_after: u64,
    pub records: BTreeMap<u64, Record>,
}

pub fn file_in(data_dir: &Path) -> PathBuf {
    data_dir.join("workshopSweep.jsonl")
}

impl Sweep {
    pub fn due(&self, now: u64) -> bool {
        self.resume_after != 0 || now.saturating_sub(self.completed) >= DAY
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn store(&self, to: &Path) -> Result<(), Error> {
        let header = json!(["sweep", 1, self.completed, self.resume_after]);
        let records = self.records.iter().map(|(id, r)| match r.removed {
            true => json!([id]),
            false => json!([
                id,
                r.title,
                r.tags,
                r.size,
                r.subscriptions,
                r.favorited,
                r.created,
                r.updated,
                r.banned,
                r.app_id
            ]),
        });
        crate::base::atomic_file::write_lines(to, std::iter::once(header).chain(records))
    }

    pub fn due_in(path: &Path, now: u64) -> bool {
        let first = std::fs::File::open(path)
            .ok()
            .and_then(|file| std::io::BufReader::new(file).lines().next()?.ok());
        let mut sweep = Sweep::default();
        if let Some(Value::Array(fields)) = first.and_then(|l| serde_json::from_str(&l).ok()) {
            let number = |at: usize| fields.get(at).and_then(Value::as_u64).unwrap_or(0);
            (sweep.completed, sweep.resume_after) = (number(2), number(3));
        }
        sweep.due(now)
    }

    /// Empty when the file is missing; a line that is not as written is skipped.
    pub fn read(path: &Path) -> Sweep {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let mut sweep = Sweep::default();
        for line in text.lines() {
            let Ok(Value::Array(fields)) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if fields.first().and_then(Value::as_str) == Some("sweep") {
                let number = |at: usize| fields.get(at).and_then(Value::as_u64).unwrap_or(0);
                (sweep.completed, sweep.resume_after) = (number(2), number(3));
            } else if let Some((id, record)) = record_of(&fields) {
                sweep.records.insert(id, record);
            }
        }
        sweep
    }
}

fn record_of(fields: &[Value]) -> Option<(u64, Record)> {
    let id = fields.first()?.as_u64()?;
    if fields.len() == 1 {
        return Some((id, Record::removed()));
    }
    let number = |at: usize| fields.get(at).and_then(Value::as_u64);
    let tags = fields.get(2)?.as_array()?;
    let record = Record {
        title: fields.get(1)?.as_str()?.to_string(),
        tags: tags
            .iter()
            .filter_map(|t| t.as_str().map(String::from))
            .collect(),
        size: number(3)?,
        subscriptions: number(4)?,
        favorited: number(5)?,
        created: number(6)?,
        updated: number(7)?,
        banned: fields.get(8)?.as_bool()?,
        app_id: number(9)?,
        removed: false,
    };
    Some((id, record))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(title: &str, subs: u64) -> Record {
        Record {
            title: title.into(),
            tags: vec!["Mod".into(), "1.7".into()],
            size: 10,
            subscriptions: subs,
            favorited: 2,
            created: 100,
            updated: 200,
            banned: true,
            app_id: 294100,
            removed: false,
        }
    }

    #[test]
    fn the_file_reads_back_as_stored_and_skips_what_is_not_a_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = file_in(dir.path());
        assert!(Sweep::read(&path).is_empty());
        let sweep = Sweep {
            completed: 5000,
            resume_after: 77,
            records: BTreeMap::from([
                (5, record("Tab\there \"q\" Ünï", 9)),
                (6, Record::removed()),
            ]),
        };
        sweep.store(&path).unwrap();
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str("garbage\n[7,\"short\"]\n");
        std::fs::write(&path, text).unwrap();
        assert_eq!(Sweep::read(&path), sweep);
    }

    #[test]
    fn a_pass_is_due_when_never_done_stopped_or_a_day_old() {
        let done = |completed, resume_after| Sweep {
            completed,
            resume_after,
            ..Sweep::default()
        };
        assert!(Sweep::default().due(DAY * 10));
        assert!(!done(1000, 0).due(1000 + DAY - 1));
        assert!(done(1000, 0).due(1000 + DAY));
        assert!(done(1000, 42).due(1001));
    }
}
