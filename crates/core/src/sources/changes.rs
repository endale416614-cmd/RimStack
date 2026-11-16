use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    /// Seconds since 1970.
    pub time: u64,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtLaunch {
    pub time: u64,
    pub workshop: HashMap<String, u64>,
}

impl AtLaunch {
    pub fn read(file: &Path) -> Option<Self> {
        serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()
    }

    pub fn write(&self, file: &Path) -> Result<(), crate::base::error::Error> {
        let text = serde_json::to_string(self)
            .map_err(|e| crate::base::error::Error::format(e.to_string()))?;
        crate::base::atomic_file::write(file, text)
    }

    /// A mod that was not installed at the start is not an update.
    pub fn updated(&self, now: &HashMap<String, u64>) -> Vec<(String, u64)> {
        let changed = |(id, was): (&String, &u64)| {
            let newer = now.get(id).is_some_and(|time| time != was);
            newer.then(|| (id.clone(), *was))
        };
        let mut found: Vec<(String, u64)> = self.workshop.iter().filter_map(changed).collect();
        found.sort();
        found
    }
}

/// Gives the newest note even when none is newer: notes can lag behind the files.
pub fn worth_showing(all: &[Note], was: u64) -> &[Note] {
    let newer = all.iter().take_while(|note| note.time > was).count();
    &all[..newer.max(1).min(all.len())]
}

/// Without a record, a mod published after `last` counts, with `last` as its earlier time.
/// None when neither is known.
pub fn since(
    record: Option<&AtLaunch>,
    last: Option<u64>,
    now: &HashMap<String, u64>,
) -> Option<(u64, Vec<(String, u64)>)> {
    if let Some(record) = record {
        return Some((record.time, record.updated(now)));
    }
    let last = last?;
    let mut published: Vec<(String, u64)> = now
        .iter()
        .filter(|(_, at)| **at > last)
        .map(|(id, _)| (id.clone(), last))
        .collect();
    published.sort();
    Some((last, published))
}

fn plain(html: &str) -> String {
    let broken = html
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n");
    let mut text = String::new();
    let mut in_tag = false;
    for c in broken.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let entities = [
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#039;", "'"),
        ("&amp;", "&"),
    ];
    let text = entities
        .iter()
        .fold(text, |text, (from, to)| text.replace(from, to));
    text.trim().to_string()
}

/// Changes are counted from the newer of the last launch from here and the game's log,
/// which a start from Steam also writes.
pub fn counted_from(launched: Option<u64>, logged: Option<u64>) -> Option<u64> {
    launched.max(logged)
}

/// Format: each entry is a `<p id="…">` whose id is the update's time, newest first.
pub fn steam_notes(page: &str) -> Vec<Note> {
    let mut notes = Vec::new();
    let mut rest = page;
    while let Some(at) = rest.find("<p id=\"") {
        rest = &rest[at + 7..];
        let Some((id, after)) = rest.split_once("\">") else {
            break;
        };
        let Some((body, _)) = after.split_once("</p>") else {
            break;
        };
        if let Ok(time) = id.parse() {
            notes.push(Note {
                time,
                text: plain(body),
            });
        }
    }
    notes
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotesProblem {
    /// `wait` is Steam's Retry-After, in seconds.
    Refused {
        wait: Option<u64>,
    },
    Failed(String),
}

/// None once `refusals` passes four: then asking stops. Steam's own figure stands when it is
/// longer; without one the pause doubles from half a minute.
pub fn pause_after_refusal(refusals: u32, asked_by_steam: Option<u64>) -> Option<u64> {
    const MOST_REFUSALS: u32 = 4;
    const LONGEST: u64 = 300;
    if refusals == 0 || refusals > MOST_REFUSALS {
        return None;
    }
    let own = 30 << (refusals - 1);
    Some(own.max(asked_by_steam.unwrap_or(0)).min(LONGEST))
}

/// Only the id is sent.
pub fn fetch_steam_notes(workshop_id: &str) -> Result<Vec<Note>, NotesProblem> {
    let url = format!("https://steamcommunity.com/sharedfiles/filedetails/changelog/{workshop_id}");
    let failed = |e: ureq::Error| NotesProblem::Failed(format!("steamcommunity.com: {e}"));
    // A refusal is read as an answer, not an error: its Retry-After header is needed.
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut answer = agent.get(&url).call().map_err(failed)?;
    let status = answer.status().as_u16();
    if status == 429 {
        let wait = answer
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse().ok());
        return Err(NotesProblem::Refused { wait });
    }
    if !(200..300).contains(&status) {
        return Err(NotesProblem::Failed(format!(
            "steamcommunity.com: {status}"
        )));
    }
    let page = answer.body_mut().read_to_string().map_err(failed)?;
    Ok(steam_notes(&page))
}

pub fn git_notes(checkout: &Path, since: u64) -> Vec<Note> {
    match crate::sources::git::commits_since(checkout, since) {
        Ok(commits) => commits
            .into_iter()
            .map(|(time, text)| Note { time, text })
            .collect(),
        Err(problem) => {
            crate::problem!("{}: {problem}", checkout.display());
            Vec::new()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetching {
    Under {
        done: usize,
    },
    /// `resume` is seconds since 1970.
    Waiting {
        done: usize,
        resume: u64,
    },
    /// A failure comes as one note with time 0, its text the problem.
    Fetched {
        id: String,
        notes: Vec<Note>,
    },
    GaveUp {
        done: usize,
    },
}

/// One mod at a time with a pause between: Steam's site answers 429 to a handful of quick
/// requests. Each keeps its notes newer than the version it had, at most `most`.
pub fn fetch_notes(
    asked: &[(String, u64)],
    most: usize,
    fetch: &dyn Fn(&str) -> Result<Vec<Note>, NotesProblem>,
    pause: &dyn Fn(std::time::Duration),
    now: &dyn Fn() -> u64,
    tell: &mut dyn FnMut(Fetching),
) {
    const BETWEEN: std::time::Duration = std::time::Duration::from_millis(1500);
    let (mut done, mut refusals) = (0, 0);
    tell(Fetching::Under { done });
    while let Some((id, was)) = asked.get(done) {
        let notes = match fetch(id) {
            Ok(all) => {
                let newer = worth_showing(&all, *was);
                newer[..newer.len().min(most)].to_vec()
            }
            Err(NotesProblem::Refused { wait }) => {
                refusals += 1;
                let Some(secs) = pause_after_refusal(refusals, wait) else {
                    tell(Fetching::GaveUp { done });
                    return;
                };
                tell(Fetching::Waiting {
                    done,
                    resume: now() + secs,
                });
                pause(std::time::Duration::from_secs(secs));
                continue;
            }
            Err(NotesProblem::Failed(problem)) => vec![Note {
                time: 0,
                text: problem,
            }],
        };
        refusals = 0;
        done += 1;
        tell(Fetching::Fetched {
            id: id.clone(),
            notes,
        });
        tell(Fetching::Under { done });
        if done < asked.len() {
            pause(BETWEEN);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_fetched_one_at_a_time_waiting_after_a_refusal_and_giving_up_after_many() {
        use std::cell::{Cell, RefCell};
        use std::time::Duration;
        let note = |time| Note {
            time,
            text: format!("at {time}"),
        };
        let refused = Cell::new(1);
        let fetch = |id: &str| -> Result<Vec<Note>, NotesProblem> {
            match id {
                "1" if refused.get() > 0 => {
                    refused.set(refused.get() - 1);
                    Err(NotesProblem::Refused { wait: Some(45) })
                }
                "1" => Ok(vec![note(30), note(20), note(10)]),
                _ => Err(NotesProblem::Failed("offline".into())),
            }
        };
        let paused = RefCell::new(Vec::new());
        let pause = |d: Duration| paused.borrow_mut().push(d.as_secs_f64());
        let mut told = Vec::new();
        let asked = [("1".to_string(), 15), ("2".to_string(), 0)];
        fetch_notes(&asked, 4, &fetch, &pause, &|| 1_000, &mut |f| told.push(f));
        assert_eq!(
            told,
            [
                Fetching::Under { done: 0 },
                Fetching::Waiting {
                    done: 0,
                    resume: 1_045
                },
                Fetching::Fetched {
                    id: "1".into(),
                    notes: vec![note(30), note(20)]
                },
                Fetching::Under { done: 1 },
                Fetching::Fetched {
                    id: "2".into(),
                    notes: vec![Note {
                        time: 0,
                        text: "offline".into()
                    }]
                },
                Fetching::Under { done: 2 },
            ]
        );
        assert_eq!(*paused.borrow(), [45.0, 1.5]);

        let always = |_: &str| Err(NotesProblem::Refused { wait: None });
        let mut last = None;
        fetch_notes(&asked, 4, &always, &|_| {}, &|| 0, &mut |f| last = Some(f));
        assert_eq!(last, Some(Fetching::GaveUp { done: 0 }));
    }

    #[test]
    fn the_notes_after_the_version_on_disk_are_shown_or_else_the_newest() {
        let note = |time| Note {
            time,
            text: String::new(),
        };
        let all = [note(30), note(20), note(10)];
        assert_eq!(worth_showing(&all, 15), &all[..2]);
        assert_eq!(worth_showing(&all, 99), &all[..1]);
        assert_eq!(worth_showing(&[], 0), &[]);
    }

    #[test]
    fn changes_are_counted_from_the_record_or_failing_that_from_the_last_start() {
        let times = |pairs: &[(&str, u64)]| -> HashMap<String, u64> {
            pairs.iter().map(|(id, at)| (id.to_string(), *at)).collect()
        };
        let now = times(&[("1", 50), ("2", 90), ("3", 120)]);
        let record = AtLaunch {
            time: 100,
            workshop: times(&[("1", 50), ("2", 60)]),
        };
        assert_eq!(
            since(Some(&record), Some(999), &now),
            Some((100, vec![("2".to_string(), 60)]))
        );
        assert_eq!(
            since(None, Some(80), &now),
            Some((80, vec![("2".to_string(), 80), ("3".to_string(), 80)]))
        );
        assert_eq!(since(None, None, &now), None);
    }

    #[test]
    fn a_refusal_is_waited_out_longer_each_time_then_given_up_and_what_steam_asks_stands_to_five_minutes()
     {
        assert_eq!(pause_after_refusal(1, None), Some(30));
        assert_eq!(pause_after_refusal(2, None), Some(60));
        assert_eq!(pause_after_refusal(4, None), Some(240));
        assert_eq!(pause_after_refusal(5, None), None);
        assert_eq!(pause_after_refusal(0, None), None);
        assert_eq!(pause_after_refusal(1, Some(90)), Some(90));
        assert_eq!(pause_after_refusal(2, Some(5)), Some(60));
        assert_eq!(pause_after_refusal(1, Some(86_400)), Some(300));
    }

    const PAGE: &str = r#"<div class="changelog headline">Update: 12 Oct @ 10:17am</div>
<p id="1791825433">v3.0.0<br><br>Adds RimWorld 1.7 support &amp; <b>more</b>.</p>
<div class="commentsLink changeLog">Discuss</div>
<div class="changelog headline">Update: 3 Sep, 2025 @ 8:49am</div>
<p id="1756914561">v2.4.2 persists the tweak values</p>
<p id="not-a-time">left out</p>"#;

    #[test]
    fn a_change_notes_page_gives_its_entries_newest_first_as_plain_text() {
        let notes = steam_notes(PAGE);
        assert_eq!(notes.len(), 2);
        assert_eq!(
            notes[0],
            Note {
                time: 1791825433,
                text: "v3.0.0\n\nAdds RimWorld 1.7 support & more.".to_string()
            }
        );
        assert_eq!(notes[1].text, "v2.4.2 persists the tweak values");
        assert!(steam_notes("<html>no notes</html>").is_empty());
        assert!(steam_notes("<p id=\"12").is_empty());
    }

    #[test]
    fn a_mod_is_updated_when_its_version_on_disk_is_another_one() {
        let times = |pairs: &[(&str, u64)]| {
            pairs
                .iter()
                .map(|(id, t)| (id.to_string(), *t))
                .collect::<HashMap<_, _>>()
        };
        let before = AtLaunch {
            time: 100,
            workshop: times(&[("1", 10), ("2", 20), ("3", 30)]),
        };
        let now = times(&[("1", 10), ("2", 25), ("4", 40)]);
        assert_eq!(before.updated(&now), [("2".to_string(), 20)]);
    }

    #[test]
    fn the_record_of_a_launch_is_written_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("kept/at-launch.json");
        assert_eq!(AtLaunch::read(&file), None);
        let record = AtLaunch {
            time: 7,
            workshop: [("9".to_string(), 3)].into(),
        };
        record.write(&file).unwrap();
        assert_eq!(AtLaunch::read(&file), Some(record));
    }
}
