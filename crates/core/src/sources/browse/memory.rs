use crate::base::error::Error;
use crate::rimworld::workshop::Details;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

const RETRY_AFTER: Duration = Duration::from_secs(180);

/// Holds no lock and asks nothing: a caller takes `unasked`, asks, and hands the answer to `learn`.
#[derive(Default)]
pub struct Memory {
    found: HashMap<String, Arc<Details>>,
    unknown: HashSet<String>,
    failed: HashMap<String, (Instant, Error)>,
}

impl Memory {
    pub fn get(&self, id: &str) -> Option<Arc<Details>> {
        self.found.get(id).cloned()
    }

    pub fn failure(&self, id: &str, now: Instant) -> Option<Error> {
        let (at, error) = self.failed.get(id)?;
        (now.saturating_duration_since(*at) < RETRY_AFTER).then(|| error.clone())
    }

    pub fn unasked(&self, ids: &[String], now: Instant) -> Vec<String> {
        let mut seen = HashSet::new();
        ids.iter()
            .filter(|id| {
                !self.found.contains_key(*id)
                    && !self.unknown.contains(*id)
                    && self.failure(id, now).is_none()
                    && seen.insert(*id)
            })
            .cloned()
            .collect()
    }

    pub fn learn(&mut self, asked: &[String], answer: Result<Vec<Details>, Error>, now: Instant) {
        match answer {
            Ok(details) => {
                for id in asked {
                    self.failed.remove(id);
                    if !details.iter().any(|d| d.id == *id) {
                        self.unknown.insert(id.clone());
                    }
                }
                for d in details {
                    self.found.insert(d.id.clone(), Arc::new(d));
                }
            }
            Err(error) => {
                for id in asked {
                    self.failed.insert(id.clone(), (now, error.clone()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::said;

    fn details(id: &str) -> Details {
        Details {
            id: id.into(),
            title: id.into(),
            updated: 1,
            size: 1,
            preview_url: String::new(),
            description: String::new(),
            subscriptions: 0,
            favorited: 0,
            banned: false,
            creator: String::new(),
            app_id: 294100,
            collection: false,
            ..Details::default()
        }
    }

    fn ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    #[test]
    fn what_steam_knows_and_what_it_does_not_is_not_asked_again() {
        let (mut memory, now) = (Memory::default(), Instant::now());
        let asked = ids(&["1", "2"]);
        assert_eq!(memory.unasked(&asked, now), asked);
        memory.learn(&asked, Ok(vec![details("1")]), now);
        assert!(memory.get("1").is_some() && memory.get("2").is_none());
        let later = now + Duration::from_secs(86_400);
        assert!(memory.unasked(&ids(&["1", "2", "1"]), later).is_empty());
        assert_eq!(memory.unasked(&ids(&["3", "3", "1"]), later), ids(&["3"]));
    }

    #[test]
    fn a_failure_is_remembered_for_a_few_minutes_and_then_asked_again() {
        let (mut memory, now) = (Memory::default(), Instant::now());
        let asked = ids(&["1"]);
        memory.learn(&asked, Err(Error::program(said!("offline"))), now);
        let soon = now + Duration::from_secs(60);
        assert!(memory.unasked(&asked, soon).is_empty());
        assert!(memory.failure("1", soon).is_some());
        let after = now + RETRY_AFTER + Duration::from_secs(1);
        assert_eq!(memory.unasked(&asked, after), asked);
        assert!(memory.failure("1", after).is_none());
        memory.learn(&asked, Ok(vec![details("1")]), after);
        assert!(memory.get("1").is_some() && memory.failure("1", now).is_none());
    }
}
