use super::App;
use rimstack_core::kept::launches;
use rimstack_core::mods::library::ModId;
use rimstack_core::sources::changes::{self, AtLaunch, Note};
use rimstack_core::sources::downloads::{self, Changed};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

pub const MOST_NOTES: usize = 4;

pub const MOST_ASKED: usize = 40;

pub type ChangeNotes = BTreeMap<String, Vec<Note>>;

#[derive(Default)]
pub struct SinceLaunch {
    pub time: u64,
    pub items: Vec<Changed>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Only {
    All,
    Active,
    Inactive,
}

impl Only {
    pub fn named(name: &str) -> Self {
        match name {
            "active" => Only::Active,
            "inactive" => Only::Inactive,
            _ => Only::All,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedShown<'a> {
    #[serde(flatten)]
    pub row: &'a Changed,
    pub said: &'a [Note],
    pub opens_day: bool,
    pub day_count: usize,
    pub same_day: bool,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SinceShown<'a> {
    pub time: u64,
    pub count: usize,
    pub active: usize,
    pub rows: Vec<ChangedShown<'a>>,
    pub workshop: usize,
}

fn day(seconds: u64) -> String {
    use chrono::TimeZone;
    let at = i64::try_from(seconds)
        .ok()
        .and_then(|s| chrono::Local.timestamp_opt(s, 0).single());
    at.map_or(String::new(), |at| at.format("%Y-%m-%d").to_string())
}

/// Newest first; active before inactive within a day.
pub fn since_shown<'a>(
    since: &'a SinceLaunch,
    only: Only,
    notes: &'a ChangeNotes,
) -> SinceShown<'a> {
    let mut rows: Vec<(&Changed, String)> = since
        .items
        .iter()
        .filter(|row| match only {
            Only::All => true,
            Only::Active => row.active,
            Only::Inactive => !row.active,
        })
        .map(|row| (row, day(row.now.unwrap_or(0))))
        .collect();
    rows.sort_by(|(a, a_day), (b, b_day)| {
        b_day
            .cmp(a_day)
            .then(b.active.cmp(&a.active))
            .then(b.now.cmp(&a.now))
    });
    let mut per_day: HashMap<&str, usize> = HashMap::new();
    for (_, on) in &rows {
        *per_day.entry(on).or_default() += 1;
    }
    let shown = rows
        .iter()
        .enumerate()
        .map(|(at, (row, on))| ChangedShown {
            row,
            said: match row.git {
                true => &row.notes,
                false => notes.get(&row.id).map_or(&[][..], Vec::as_slice),
            },
            opens_day: at == 0 || rows[at - 1].1 != *on,
            day_count: per_day[on.as_str()],
            same_day: day(row.was) == *on,
        })
        .collect();
    SinceShown {
        time: since.time,
        count: since.items.len(),
        active: since.items.iter().filter(|row| row.active).count(),
        workshop: since.items.iter().filter(|row| !row.git).count(),
        rows: shown,
    }
}

pub fn notes_asked(since: &SinceLaunch) -> Vec<(String, u64)> {
    let workshop = since.items.iter().filter(|row| !row.git);
    workshop.map(|row| (row.id.clone(), row.was)).collect()
}

impl App {
    fn at_launch_file(&self) -> PathBuf {
        self.data_dir().join("at-launch.json")
    }

    pub fn record_versions(&self) {
        let record = AtLaunch {
            time: super::now_secs(),
            workshop: self.steam_record().times.clone(),
        };
        if let Err(problem) = record.write(&self.at_launch_file()) {
            rimstack_core::problem!("{problem}");
        }
    }

    pub fn since_launch(&self) -> SinceLaunch {
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return SinceLaunch::default();
        };
        let now = &self.steam_record().times;
        let launched = launches::read(&self.launches_file()).pop().map(|l| l.time);
        let logged = Some(self.log_written() / 1000).filter(|&t| t > 0);
        let record = AtLaunch::read(&self.at_launch_file());
        let last = changes::counted_from(launched, logged);
        let Some((time, updated)) = changes::since(record.as_ref(), last, now) else {
            return SinceLaunch::default();
        };
        let active = self.active_now(library);
        let mut rows = downloads::changed(&downloads::items(library, &active), &updated, now);
        for (mod_id, m) in library.ids().filter(|(_, m)| m.git) {
            let commits = changes::git_notes(&m.path, time);
            if let Some(newest) = commits.first() {
                rows.push(Changed {
                    git: true,
                    id: m.path.to_string_lossy().into_owned(),
                    mod_id,
                    name: m.display_name().to_string(),
                    active: active.contains(&mod_id),
                    was: time,
                    now: Some(newest.time),
                    notes: commits.into_iter().take(MOST_NOTES).collect(),
                });
            }
        }
        SinceLaunch {
            time,
            items: downloads::newest_first(rows),
        }
    }

    /// The page of the repository a Git checkout was cloned from; empty without one.
    pub fn repository_of(&self, id: ModId) -> String {
        let loaded = self.library();
        let found = loaded.as_ref().as_ref().ok().and_then(|(l, _)| l.get(id));
        found
            .and_then(|m| rimstack_core::sources::git::page(&m.path))
            .unwrap_or_default()
    }

    pub fn commits_of(&self, id: ModId) -> Vec<Note> {
        let loaded = self.library();
        let Some(m) = loaded.as_ref().as_ref().ok().and_then(|(l, _)| l.get(id)) else {
            return Vec::new();
        };
        let mut commits = changes::git_notes(&m.path, 0);
        commits.truncate(MOST_NOTES);
        commits
    }
}

#[derive(Serialize)]
pub struct NotesState {
    pub done: usize,
    pub asked: usize,
    /// Seconds since 1970.
    pub resume: u64,
    pub refused: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changed(id: &str, active: bool, now: u64) -> Changed {
        Changed {
            git: false,
            id: id.into(),
            mod_id: ModId::new(0, 0),
            name: id.into(),
            active,
            was: now - 60,
            now: Some(now),
            notes: Vec::new(),
        }
    }

    #[test]
    fn changed_mods_go_by_day_newest_first_active_first_within_a_day() {
        let noon = 1_760_000_000 - 1_760_000_000 % 86_400 + 43_200;
        let since = SinceLaunch {
            time: 1,
            items: vec![
                changed("old", true, noon - 86_400),
                changed("off", false, noon + 60),
                changed("on", true, noon),
            ],
        };
        let mut notes = ChangeNotes::new();
        notes.insert(
            "on".into(),
            vec![Note {
                time: 5,
                text: "fixed".into(),
            }],
        );
        let shown = since_shown(&since, Only::All, &notes);
        let order: Vec<&str> = shown.rows.iter().map(|r| r.row.id.as_str()).collect();
        assert_eq!(order, ["on", "off", "old"]);
        assert!(shown.rows[0].opens_day && !shown.rows[1].opens_day && shown.rows[2].opens_day);
        assert_eq!((shown.rows[0].day_count, shown.rows[2].day_count), (2, 1));
        assert_eq!(shown.rows[0].said.len(), 1);
        assert!(shown.rows[0].same_day);
        assert_eq!((shown.count, shown.active, shown.workshop), (3, 2, 3));
        let off = since_shown(&since, Only::Inactive, &notes);
        assert_eq!(off.rows.len(), 1);
        assert_eq!(notes_asked(&since).len(), 3);
    }

    #[test]
    fn many_changed_mods_are_counted_once_per_day() {
        let noon = 1_760_000_000 - 1_760_000_000 % 86_400 + 43_200;
        let items: Vec<Changed> = (0..3000)
            .map(|n| changed(&format!("m{n}"), n % 2 == 0, noon - (n % 3) * 86_400 * 2))
            .collect();
        let since = SinceLaunch { time: 1, items };
        let notes = ChangeNotes::new();
        let shown = since_shown(&since, Only::All, &notes);
        assert_eq!(shown.rows.len(), 3000);
        assert!(shown.rows.iter().all(|r| r.day_count == 1000));
        assert_eq!(shown.rows.iter().filter(|r| r.opens_day).count(), 3);
    }
}
