use crate::base::error::Error;
use crate::rimworld::workshop::{self, Details};
use crate::rimworld::workshopsweep::{Record, Sweep};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

const BATCH: usize = 100;

pub trait Fetch: Send + Sync {
    fn batch(&self, ids: &[String]) -> Result<Vec<Details>, Error>;
}

pub struct Web {
    agent: ureq::Agent,
}

impl Default for Web {
    fn default() -> Self {
        Web {
            agent: crate::base::web::steam_agent(),
        }
    }
}

impl Fetch for Web {
    fn batch(&self, ids: &[String]) -> Result<Vec<Details>, Error> {
        workshop::fetch_batch(&self.agent, ids)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Pace {
    pub workers: usize,
    pub pause: Duration,
    pub backoff: Duration,
}

impl Pace {
    pub const GENTLE: Pace = Pace {
        workers: 4,
        pause: Duration::from_millis(250),
        backoff: Duration::from_secs(60),
    };
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Finished,
    Stopped { reached: usize, of: usize },
}

struct Shared {
    next: usize,
    done: Vec<bool>,
    got: Vec<(u64, Record)>,
    stopped: Option<Error>,
}

pub fn run(
    ids: &[u64],
    path: &Path,
    fetch: &dyn Fetch,
    pace: &Pace,
    now: u64,
) -> Result<Outcome, Error> {
    let mut sweep = Sweep::read(path);
    let from = ids.partition_point(|&id| id <= sweep.resume_after);
    let todo: Vec<&[u64]> = ids[from..].chunks(BATCH).collect();
    let shared = Mutex::new(Shared {
        next: 0,
        done: vec![false; todo.len()],
        got: Vec::new(),
        stopped: None,
    });
    let lock = || shared.lock().unwrap_or_else(|e| e.into_inner());
    std::thread::scope(|scope| {
        for _ in 0..pace.workers.clamp(1, todo.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let at = {
                        let mut state = lock();
                        if state.stopped.is_some() || state.next >= todo.len() {
                            return;
                        }
                        state.next += 1;
                        state.next - 1
                    };
                    let asked: Vec<String> = todo[at].iter().map(u64::to_string).collect();
                    let answer = fetch.batch(&asked).or_else(|_| {
                        std::thread::sleep(pace.backoff);
                        fetch.batch(&asked)
                    });
                    let mut state = lock();
                    match answer {
                        Ok(details) => {
                            let by_id: std::collections::HashMap<&str, &Details> =
                                details.iter().map(|d| (d.id.as_str(), d)).collect();
                            for &id in todo[at] {
                                let found = by_id.get(id.to_string().as_str());
                                state.got.push((
                                    id,
                                    found.map_or_else(Record::removed, |d| Record::of(d)),
                                ));
                            }
                            state.done[at] = true;
                        }
                        Err(error) => state.stopped = Some(error),
                    }
                    drop(state);
                    std::thread::sleep(pace.pause);
                }
            });
        }
    });
    let state = shared.into_inner().unwrap_or_else(|e| e.into_inner());
    let reached = state.done.iter().take_while(|done| **done).count();
    let finished = reached == todo.len();
    if let Some(error) = state.stopped.filter(|_| state.got.is_empty()) {
        return Err(error);
    }
    sweep.records.extend(state.got);
    if finished {
        sweep.completed = now;
        sweep.resume_after = 0;
        let known: std::collections::HashSet<u64> = ids.iter().copied().collect();
        sweep.records.retain(|id, _| known.contains(id));
    } else if let Some(last) = reached.checked_sub(1).and_then(|k| todo[k].last()) {
        sweep.resume_after = *last;
    }
    if finished || !sweep.is_empty() {
        sweep.store(path)?;
    }
    Ok(match finished {
        true => Outcome::Finished,
        false => Outcome::Stopped {
            reached: reached.min(todo.len()),
            of: todo.len(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rimworld::workshopsweep::file_in;
    use crate::said;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const QUICK: Pace = Pace {
        workers: 4,
        pause: Duration::ZERO,
        backoff: Duration::ZERO,
    };

    struct Fake {
        calls: AtomicUsize,
        sizes: Mutex<Vec<usize>>,
        fail_from: usize,
        fails: usize,
    }

    impl Fake {
        fn ok() -> Fake {
            Fake::failing(usize::MAX, 0)
        }

        fn failing(fail_from: usize, fails: usize) -> Fake {
            Fake {
                calls: AtomicUsize::new(0),
                sizes: Mutex::default(),
                fail_from,
                fails,
            }
        }
    }

    impl Fetch for Fake {
        fn batch(&self, ids: &[String]) -> Result<Vec<Details>, Error> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            self.sizes.lock().unwrap().push(ids.len());
            if call >= self.fail_from && call < self.fail_from + self.fails {
                return Err(Error::network(said!("HTTP status 429")));
            }
            let known = |id: &&String| id.parse::<u64>().unwrap() % 1000 != 0;
            Ok(ids
                .iter()
                .filter(known)
                .map(|id| Details {
                    id: id.clone(),
                    title: format!("t{id}"),
                    subscriptions: id.parse().unwrap(),
                    tags: vec!["Mod".into(), "1.7".into()],
                    banned: id == "7",
                    ..Details::default()
                })
                .collect())
        }
    }

    fn ids(n: u64) -> Vec<u64> {
        (1..=n).collect()
    }

    #[test]
    fn every_id_is_asked_in_batches_of_a_hundred_and_what_steam_drops_is_marked_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = file_in(dir.path());
        let fake = Fake::ok();
        let out = run(&ids(2050), &path, &fake, &QUICK, 5000).unwrap();
        assert_eq!(out, Outcome::Finished);
        let mut sizes = fake.sizes.lock().unwrap().clone();
        sizes.sort();
        assert_eq!((sizes.len(), sizes[0], sizes[20]), (21, 50, 100));
        let sweep = Sweep::read(&path);
        assert_eq!(
            (sweep.completed, sweep.resume_after, sweep.records.len()),
            (5000, 0, 2050)
        );
        assert!(sweep.records[&2000].removed && !sweep.records[&2001].removed);
        assert!(sweep.records[&7].banned);
        assert_eq!(sweep.records[&5].subscriptions, 5);
    }

    #[test]
    fn a_failure_backs_off_once_and_a_second_stops_the_pass_to_resume_after_what_was_reached() {
        let dir = tempfile::tempdir().unwrap();
        let path = file_in(dir.path());
        let one = Pace {
            workers: 1,
            ..QUICK
        };
        let fake = Fake::failing(3, 2);
        let out = run(&ids(1000), &path, &fake, &one, 5000).unwrap();
        assert_eq!(out, Outcome::Stopped { reached: 3, of: 10 });
        let kept = Sweep::read(&path);
        assert_eq!(
            (kept.completed, kept.resume_after, kept.records.len()),
            (0, 300, 300)
        );
        assert!(kept.due(6000));

        let later = Fake::ok();
        let out = run(&ids(1000), &path, &later, &one, 9000).unwrap();
        assert_eq!(out, Outcome::Finished);
        assert_eq!(later.sizes.lock().unwrap().len(), 7);
        let done = Sweep::read(&path);
        assert_eq!(
            (done.completed, done.resume_after, done.records.len()),
            (9000, 0, 1000)
        );
        assert!(!done.due(9001));
    }

    #[test]
    fn one_failed_request_that_works_on_retry_costs_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = file_in(dir.path());
        let fake = Fake::failing(1, 1);
        let out = run(&ids(500), &path, &fake, &QUICK, 1).unwrap();
        assert_eq!(out, Outcome::Finished);
        assert_eq!(Sweep::read(&path).records.len(), 500);
    }

    #[test]
    fn nothing_reached_is_an_error_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = file_in(dir.path());
        let fake = Fake::failing(0, 100);
        assert!(run(&ids(300), &path, &fake, &QUICK, 1).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn a_finished_pass_forgets_items_the_index_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let path = file_in(dir.path());
        run(&ids(300), &path, &Fake::ok(), &QUICK, 1).unwrap();
        run(&ids(150), &path, &Fake::ok(), &QUICK, 2).unwrap();
        assert_eq!(Sweep::read(&path).records.len(), 150);
    }
}
