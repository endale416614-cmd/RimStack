use super::{App, now_secs};
use crate::changes::Changes;
use rimstack_core::problem;
use rimstack_core::rimworld::workshopsweep::{self, Sweep};
use rimstack_core::sources::sweep::{self, Fetch, Outcome, Pace, Web};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Sweeper {
    fetch: Arc<dyn Fetch>,
    pace: Pace,
    decided: AtomicBool,
}

impl Default for Sweeper {
    fn default() -> Self {
        Sweeper {
            fetch: Arc::new(Web::default()),
            pace: Pace::GENTLE,
            decided: AtomicBool::new(false),
        }
    }
}

impl App {
    pub fn with_sweeper(self, fetch: Arc<dyn Fetch>, pace: Pace) -> Self {
        Self {
            sweeper: Sweeper {
                fetch,
                pace,
                decided: AtomicBool::new(false),
            },
            ..self
        }
    }

    pub fn sweep_if_due(self: &Arc<Self>) {
        let opened = self.browse_open.load(Ordering::Relaxed);
        if self.fixture
            || !opened
            || !self.database_there()
            || self.sweeper.decided.swap(true, Ordering::SeqCst)
        {
            return;
        }
        let file = workshopsweep::file_in(&self.data_dir);
        let app = self.clone();
        crate::jobs::spawn("workshop sweep", move || {
            let now = now_secs();
            if !Sweep::due_in(&file, now) {
                return;
            }
            let ids = app.workshop_index().ids().to_vec();
            let Sweeper { fetch, pace, .. } = &app.sweeper;
            match sweep::run(&ids, &file, &**fetch, pace, now) {
                Ok(Outcome::Finished) => {}
                Ok(Outcome::Stopped { reached, of }) => {
                    problem!("the Workshop sweep stopped after {reached} of {of} requests")
                }
                Err(error) => return problem!("the Workshop sweep failed: {error}"),
            }
            let _ = app.workshop_index();
            app.publish(Changes::COMMUNITY);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstack_core::base::error::Error;
    use rimstack_core::rimworld::workshop::Details;
    use rimstack_core::rimworld::workshopindex;
    use rimstack_core::rimworld::workshopindex::Entry;
    use rimstack_core::rimworld::workshopsearch::{Filters, Order, Query};
    use std::sync::Mutex;
    use std::sync::mpsc;
    use std::time::Duration;

    struct Counting(Mutex<usize>);

    impl Fetch for Counting {
        fn batch(&self, ids: &[String]) -> Result<Vec<Details>, Error> {
            *self.0.lock().unwrap() += 1;
            Ok(ids
                .iter()
                .filter(|id| *id != "300")
                .map(|id| Details {
                    id: id.clone(),
                    title: format!("Steam {id}"),
                    subscriptions: id.parse::<u64>().unwrap_or(0) * 7,
                    tags: vec!["Mod".into(), "1.7".into()],
                    ..Details::default()
                })
                .collect())
        }
    }

    fn entry(id: &str) -> Entry {
        Entry {
            id: id.into(),
            title: format!("Old {id}"),
            author: None,
            package_id: None,
            tags: vec!["Mod".into(), "1.6".into()],
            requires: Vec::new(),
            dlc: Vec::new(),
        }
    }

    fn app_with(fetch: Arc<Counting>) -> (tempfile::TempDir, Arc<App>) {
        let tmp = tempfile::tempdir().unwrap();
        let pace = Pace {
            workers: 2,
            pause: Duration::ZERO,
            backoff: Duration::ZERO,
        };
        let app = App::new(
            tmp.path().join("home"),
            &tmp.path().join("config"),
            &tmp.path().join("data"),
            Default::default(),
        )
        .with_sweeper(fetch, pace);
        std::fs::create_dir_all(&app.data_dir).unwrap();
        (tmp, Arc::new(app))
    }

    fn index_of(app: &App, ids: &[&str]) {
        let entries = ids.iter().map(|id| entry(id)).collect();
        let index = rimstack_core::rimworld::workshopindex::Index::new(entries);
        index.store(&workshopindex::file_in(&app.data_dir)).unwrap();
    }

    #[test]
    fn a_pass_starts_once_browse_is_open_and_the_database_is_there_and_is_laid_over_it() {
        let fetch = Arc::new(Counting(Mutex::new(0)));
        let (_tmp, app) = app_with(fetch.clone());
        let (told, heard) = mpsc::channel();
        app.listen(move |changes| {
            if changes.contains(Changes::COMMUNITY) {
                let _ = told.send(());
            }
        });
        app.sweep_if_due();
        index_of(&app, &["100", "200", "300"]);
        app.sweep_if_due();
        assert!(heard.try_recv().is_err(), "Browse is not open yet");
        app.browse_opened();
        app.sweep_if_due();
        heard.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(*fetch.0.lock().unwrap(), 1);
        let merged = app.workshop_index();
        assert!(merged.swept());
        assert_eq!(merged.get("100").unwrap().tags, ["Mod", "1.7"]);
        let shown = merged.search(
            &Query::default(),
            &Filters::default(),
            Order::Subscribed,
            0,
            5,
        );
        assert_eq!(shown.ids, ["200", "100"], "300 is removed and hidden");
        app.sweep_if_due();
        assert_eq!(*fetch.0.lock().unwrap(), 1, "once a session");
    }

    #[test]
    fn a_fresh_pass_is_left_alone_and_a_made_up_install_never_asks() {
        let fetch = Arc::new(Counting(Mutex::new(0)));
        let (_tmp, app) = app_with(fetch.clone());
        index_of(&app, &["100"]);
        Sweep {
            completed: now_secs(),
            ..Sweep::default()
        }
        .store(&workshopsweep::file_in(&app.data_dir))
        .unwrap();
        app.browse_opened();
        app.sweep_if_due();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(*fetch.0.lock().unwrap(), 0);

        let fetch = Arc::new(Counting(Mutex::new(0)));
        let (_tmp, app) = app_with(fetch.clone());
        let app = Arc::new(Arc::into_inner(app).unwrap().browsing_fixture(true));
        index_of(&app, &["100"]);
        app.browse_opened();
        app.sweep_if_due();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(*fetch.0.lock().unwrap(), 0);
    }
}
