//! Every thread RimStack starts for its own work goes through here, named for the diagnostics;
//! rayon's and gpui's pools are their own.
//!
//! Under test, a job asked for on the window's thread runs on the test's own executor instead:
//! gpui's test scheduler takes a wake from any other thread for a fault, and hung when one came
//! while a test's window was closing (2026-11-12).

use futures::channel::oneshot;
use gpui::{AsyncApp, Context, Task, WeakEntity};
use rimstack_core::base::locks::Held;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg(test)]
thread_local! {
    static TESTED: std::cell::RefCell<Option<gpui::BackgroundExecutor>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn run_on(executor: gpui::BackgroundExecutor) {
    TESTED.with(|tested| *tested.borrow_mut() = Some(executor));
}

thread_local! {
    static WINDOW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static IN_JOB: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static NOTED: std::cell::RefCell<std::collections::HashSet<String>> = Default::default();
}

/// This thread draws the window: a long wait on it outside a job fails a test, and is noted in
/// the diagnostics once for each kind otherwise.
pub fn keep_window_thread() {
    WINDOW.set(true);
    rimstack_core::base::waits::watch(waited);
}

fn waited(what: &str) {
    if !WINDOW.get() || IN_JOB.get() > 0 {
        return;
    }
    #[cfg(test)]
    panic!("the window's thread waited on {what}: read it in a job");
    #[cfg(not(test))]
    if NOTED.with_borrow_mut(|noted| noted.insert(what.to_string())) {
        rimstack_core::problem!("the window waited on {what}");
    }
}

/// `work` on this thread, though it may wait long: what must be done before the window shows
/// anything, or a test's own setup.
pub fn waiting_here<R>(work: impl FnOnce() -> R) -> R {
    struct Counted;
    impl Drop for Counted {
        fn drop(&mut self) {
            IN_JOB.set(IN_JOB.get() - 1);
        }
    }
    IN_JOB.set(IN_JOB.get() + 1);
    let _counted = Counted;
    work()
}

#[cfg(test)]
fn as_job(work: impl FnOnce()) {
    waiting_here(work);
}

/// Threads running jobs at most; a job asked beyond it waits for one of them, and the asker
/// never waits.
const MOST_THREADS: usize = 32;
/// Threads past `MOST_THREADS` for work a view waits on the answer of, so that downloads,
/// searches and SteamCMD filling every thread never hold the window's reads.
const QUICK_THREADS: usize = 8;

type Job = Box<dyn FnOnce() + Send>;

/// Threads running each kind of job, and the jobs waiting for one. Ordinary jobs never hold
/// more than `MOST_THREADS`, whichever thread runs them, so `QUICK_THREADS` stay for quick ones.
struct Pool {
    ordinary: usize,
    quick: usize,
    waiting: std::collections::VecDeque<(String, Job)>,
    waiting_quick: std::collections::VecDeque<(String, Job)>,
}

impl Pool {
    fn room(&self, quick: bool) -> bool {
        match quick {
            true => self.ordinary + self.quick < MOST_THREADS + QUICK_THREADS,
            false => self.ordinary < MOST_THREADS,
        }
    }

    fn count(&mut self, quick: bool) -> &mut usize {
        match quick {
            true => &mut self.quick,
            false => &mut self.ordinary,
        }
    }

    /// The job a thread that has just run one of kind `was` takes next, quick ones first.
    fn next(&mut self, was: bool) -> Option<(bool, Job)> {
        *self.count(was) -= 1;
        let taken = if !self.waiting_quick.is_empty() && self.room(true) {
            self.waiting_quick.pop_front().map(|(_, job)| (true, job))
        } else if self.room(false) {
            self.waiting.pop_front().map(|(_, job)| (false, job))
        } else {
            None
        };
        if let Some((quick, _)) = &taken {
            *self.count(*quick) += 1;
        }
        taken
    }
}

static RUNNING: Mutex<Pool> = Mutex::new(Pool {
    ordinary: 0,
    quick: 0,
    waiting: std::collections::VecDeque::new(),
    waiting_quick: std::collections::VecDeque::new(),
});

pub fn spawn(name: &str, work: impl FnOnce() + Send + 'static) {
    spawn_as(name, false, work);
}

/// `quick`: a view waits on its answer, as a `Lane`'s; it runs ahead of the rest.
fn spawn_as(name: &str, quick: bool, work: impl FnOnce() + Send + 'static) {
    #[cfg(test)]
    if let Some(executor) = TESTED.with(|tested| tested.borrow().clone()) {
        executor.spawn(async move { as_job(work) }).detach();
        return;
    }
    let mut pool = RUNNING.held();
    let job = Box::new(work) as Job;
    if !pool.room(quick) {
        let waiting = match quick {
            true => &mut pool.waiting_quick,
            false => &mut pool.waiting,
        };
        waiting.push_back((name.to_string(), job));
        return;
    }
    *pool.count(quick) += 1;
    drop(pool);
    start(name.to_string(), quick, job);
}

fn start(name: String, quick: bool, work: Job) {
    let started = std::thread::Builder::new()
        .name(name.clone())
        .spawn(move || {
            let (mut quick, mut work) = (quick, work);
            loop {
                work();
                match RUNNING.held().next(quick) {
                    Some(next) => (quick, work) = next,
                    None => return,
                }
            }
        });
    if let Err(problem) = started {
        *RUNNING.held().count(quick) -= 1;
        rimstack_core::problem!("{name} not started: {problem}");
    }
}

/// `work` runs off the window's thread; a view gone by then is not told.
pub fn run<V: 'static, R: Send + 'static>(
    name: &str,
    cx: &mut Context<V>,
    work: impl FnOnce() -> R + Send + 'static,
    done: impl FnOnce(&mut V, R, &mut Context<V>) + 'static,
) {
    let waiting = off_thread(name, work);
    cx.spawn(async move |view: WeakEntity<V>, cx: &mut AsyncApp| {
        if let Ok(found) = waiting.await {
            let _ = view.update(cx, |view, cx| done(view, found, cx));
        }
    })
    .detach();
}

/// `work`'s answer, to be awaited on the window's thread.
pub fn off_thread<R: Send + 'static>(
    name: &str,
    work: impl FnOnce() -> R + Send + 'static,
) -> oneshot::Receiver<R> {
    off_thread_as(name, false, work)
}

fn off_thread_as<R: Send + 'static>(
    name: &str,
    quick: bool,
    work: impl FnOnce() -> R + Send + 'static,
) -> oneshot::Receiver<R> {
    let (found, waiting) = oneshot::channel();
    spawn_as(name, quick, move || {
        let _ = found.send(work());
    });
    waiting
}

/// A wait on the window's executor: a test moves its clock on (`advance_clock`).
fn pause(wait: Duration, cx: &AsyncApp) -> Task<()> {
    match wait.is_zero() {
        true => Task::ready(()),
        false => cx.background_executor().timer(wait),
    }
}

#[derive(Default)]
pub struct Lane {
    asked: Arc<AtomicU64>,
    /// The wait for the newest answer: replacing it drops the one before, and its wait with it.
    waiting: std::cell::RefCell<Option<Task<()>>>,
}

#[derive(Clone)]
pub struct Turn {
    asked: Arc<AtomicU64>,
    mine: u64,
}

impl Turn {
    pub fn current(&self) -> bool {
        self.asked.load(Ordering::SeqCst) == self.mine
    }
}

impl Lane {
    fn next(&self) -> Turn {
        Turn {
            asked: self.asked.clone(),
            mine: self.asked.fetch_add(1, Ordering::SeqCst) + 1,
        }
    }

    pub fn cancel(&self) {
        self.asked.fetch_add(1, Ordering::SeqCst);
        drop(self.waiting.take());
    }

    fn keep(&self, task: Task<()>) {
        drop(self.waiting.replace(Some(task)));
    }

    /// `done` is not called once the lane was asked again; `work` may stop early.
    pub fn run<V: 'static, R: Send + 'static>(
        &self,
        name: &str,
        cx: &mut Context<V>,
        work: impl FnOnce(&Turn) -> R + Send + 'static,
        done: impl FnOnce(&mut V, R, &mut Context<V>) + 'static,
    ) {
        self.run_after(Duration::ZERO, name, cx, work, done);
    }

    /// As `run`, once `wait` has passed with nothing newer asked: the pause of a field that
    /// applies itself. No thread is held while waiting.
    pub fn run_after<V: 'static, R: Send + 'static>(
        &self,
        wait: Duration,
        name: &str,
        cx: &mut Context<V>,
        work: impl FnOnce(&Turn) -> R + Send + 'static,
        done: impl FnOnce(&mut V, R, &mut Context<V>) + 'static,
    ) {
        let turn = self.next();
        let name = name.to_string();
        self.keep(
            cx.spawn(async move |view: WeakEntity<V>, cx: &mut AsyncApp| {
                pause(wait, cx).await;
                if !turn.current() {
                    return;
                }
                let mine = turn.clone();
                let Ok(found) = off_thread_as(&name, true, move || work(&mine)).await else {
                    return;
                };
                // Checked on the window's thread: another run may be asked for meanwhile.
                if turn.current() {
                    let _ = view.update(cx, |view, cx| done(view, found, cx));
                }
            }),
        );
    }

    /// `done` on the window's thread once `wait` has passed with nothing newer asked.
    pub fn after<V: 'static>(
        &self,
        wait: Duration,
        cx: &mut Context<V>,
        done: impl FnOnce(&mut V, &mut Context<V>) + 'static,
    ) {
        let turn = self.next();
        self.keep(
            cx.spawn(async move |view: WeakEntity<V>, cx: &mut AsyncApp| {
                pause(wait, cx).await;
                if turn.current() {
                    let _ = view.update(cx, |view, cx| done(view, cx));
                }
            }),
        );
    }
}

/// Work of which one runs at a time: asked again while it runs, it runs once more after with
/// the newest of what was asked, however often that was.
pub struct Single<T> {
    state: Arc<Mutex<(bool, Option<T>)>>,
}

impl<T> Default for Single<T> {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new((false, None))),
        }
    }
}

impl<T: Send + 'static> Single<T> {
    pub fn idle(&self) -> bool {
        let state = self.state.held();
        !state.0 && state.1.is_none()
    }

    pub fn ask(&self, name: &str, given: T, work: impl Fn(T) + Send + 'static) {
        let mut state = self.state.held();
        state.1 = Some(given);
        if state.0 {
            return;
        }
        state.0 = true;
        drop(state);
        let state = self.state.clone();
        spawn(name, move || {
            loop {
                let next = {
                    let mut state = state.held();
                    match state.1.take() {
                        Some(next) => next,
                        None => {
                            state.0 = false;
                            return;
                        }
                    }
                };
                work(next);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pool is the process's own: tests that fill it take turns.
    static POOL: Mutex<()> = Mutex::new(());

    #[test]
    fn a_long_wait_on_the_windows_thread_fails_outside_a_job_alone() {
        keep_window_thread();
        as_job(|| rimstack_core::base::waits::slow("inside a job"));
        let outside = std::panic::catch_unwind(|| rimstack_core::base::waits::slow("outside"));
        assert!(outside.is_err());
        let elsewhere = std::thread::spawn(|| rimstack_core::base::waits::slow("another thread"));
        assert!(elsewhere.join().is_ok());
    }

    #[test]
    fn work_a_view_waits_on_runs_while_every_other_thread_is_held() {
        let _turn = POOL.held();
        let (release, held) = std::sync::mpsc::channel::<()>();
        let held = Arc::new(Mutex::new(held));
        let (started, starting) = std::sync::mpsc::channel();
        for _ in 0..MOST_THREADS {
            let (held, started) = (held.clone(), started.clone());
            spawn("held", move || {
                let _ = started.send(());
                let _ = held.held().recv();
            });
        }
        for _ in 0..MOST_THREADS {
            let _ = starting.recv_timeout(std::time::Duration::from_secs(5));
        }
        let (told, answer) = std::sync::mpsc::channel();
        spawn_as("quick", true, move || {
            let _ = told.send(7);
        });
        let answer = answer.recv_timeout(std::time::Duration::from_secs(5)).ok();
        for _ in 0..MOST_THREADS {
            let _ = release.send(());
        }
        assert_eq!(answer, Some(7));
    }

    #[test]
    fn quick_work_keeps_its_threads_while_ordinary_jobs_wait() {
        let _turn = POOL.held();
        let (release, held) = std::sync::mpsc::channel::<()>();
        let held = Arc::new(Mutex::new(held));
        let (started, starting) = std::sync::mpsc::channel();
        let asked = MOST_THREADS + 4 * QUICK_THREADS;
        for _ in 0..asked {
            let (held, started) = (held.clone(), started.clone());
            spawn("held", move || {
                let _ = started.send(());
                let _ = held.held().recv();
            });
        }
        for _ in 0..MOST_THREADS {
            let _ = starting.recv_timeout(std::time::Duration::from_secs(5));
        }
        let answered = (0..2 * QUICK_THREADS)
            .filter(|_| {
                let (told, answer) = std::sync::mpsc::channel();
                spawn_as("quick", true, move || {
                    let _ = told.send(());
                });
                answer
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .is_ok()
            })
            .count();
        for _ in 0..asked {
            let _ = release.send(());
        }
        assert_eq!(answered, 2 * QUICK_THREADS);
    }

    #[test]
    fn jobs_past_the_most_threads_wait_for_one_and_all_run() {
        let _turn = POOL.held();
        let (release, held) = std::sync::mpsc::channel::<()>();
        let held = Arc::new(Mutex::new(held));
        let (done, finished) = std::sync::mpsc::channel();
        let asked = MOST_THREADS + 8;
        for at in 0..asked {
            let (held, done) = (held.clone(), done.clone());
            spawn("held", move || {
                if at < MOST_THREADS {
                    let _ = held.held().recv();
                }
                let _ = done.send(at);
            });
        }
        {
            let pool = RUNNING.held();
            assert!(pool.ordinary <= MOST_THREADS);
            assert!(pool.ordinary + pool.quick <= MOST_THREADS + QUICK_THREADS);
        }
        for _ in 0..MOST_THREADS {
            release.send(()).unwrap();
        }
        let wait = std::time::Duration::from_secs(5);
        let ran = (0..asked)
            .filter(|_| finished.recv_timeout(wait).is_ok())
            .count();
        assert_eq!(ran, asked);
    }

    #[test]
    fn only_the_newest_turn_of_a_lane_is_current() {
        let lane = Lane::default();
        let first = lane.next();
        assert!(first.current());
        let second = lane.next();
        assert!(!first.current() && second.current());
        lane.cancel();
        assert!(!second.current());
    }

    #[derive(Default)]
    struct Probe {
        lane: Lane,
        done: Vec<&'static str>,
    }

    fn probed() -> (gpui::HeadlessAppContext, gpui::Entity<Probe>) {
        let mut cx = gpui::HeadlessAppContext::with_asset_source(
            crate::ui::made_up::text_system(),
            Arc::new(crate::ui::assets::Assets),
        );
        let probe = cx.update(|cx| {
            run_on(cx.background_executor().clone());
            gpui::AppContext::new(cx, |_| Probe::default())
        });
        (cx, probe)
    }

    fn ask_after(
        probe: &gpui::Entity<Probe>,
        said: &'static str,
        cx: &mut gpui::HeadlessAppContext,
    ) {
        cx.update(|cx| {
            probe.update(cx, |probe, cx| {
                probe.lane.run_after(
                    Duration::from_millis(100),
                    said,
                    cx,
                    move |_| said,
                    |probe, said, _| probe.done.push(said),
                )
            })
        });
    }

    fn done(probe: &gpui::Entity<Probe>, cx: &mut gpui::HeadlessAppContext) -> Vec<&'static str> {
        cx.run_until_parked();
        cx.update(|cx| probe.read(cx).done.clone())
    }

    #[test]
    fn a_wait_starts_again_when_the_lane_is_asked_again() {
        let (mut cx, probe) = probed();
        ask_after(&probe, "first", &mut cx);
        cx.advance_clock(Duration::from_millis(60));
        ask_after(&probe, "second", &mut cx);
        cx.advance_clock(Duration::from_millis(60));
        assert!(done(&probe, &mut cx).is_empty());
        cx.advance_clock(Duration::from_millis(50));
        assert_eq!(done(&probe, &mut cx), ["second"]);
    }

    #[test]
    fn an_answer_asked_again_meanwhile_is_dropped() {
        let (mut cx, probe) = probed();
        cx.update(|cx| {
            probe.update(cx, |probe, cx| {
                let asked = probe.lane.asked.clone();
                probe.lane.run(
                    "stale",
                    cx,
                    move |turn| {
                        asked.fetch_add(1, Ordering::SeqCst);
                        turn.current()
                    },
                    |probe, current, _| {
                        assert!(!current);
                        probe.done.push("stale");
                    },
                )
            })
        });
        assert!(done(&probe, &mut cx).is_empty());
    }

    #[test]
    fn quiet_waits_asked_in_a_burst_end_once_after_the_last() {
        let (mut cx, probe) = probed();
        for _ in 0..3 {
            cx.update(|cx| {
                probe.update(cx, |probe, cx| {
                    probe
                        .lane
                        .after(Duration::from_millis(150), cx, |probe, _| {
                            probe.done.push("quiet")
                        })
                })
            });
            cx.advance_clock(Duration::from_millis(50));
        }
        assert!(done(&probe, &mut cx).is_empty());
        cx.advance_clock(Duration::from_millis(100));
        assert_eq!(done(&probe, &mut cx), ["quiet"]);
    }

    #[test]
    fn work_asked_while_it_runs_runs_once_more_with_the_newest() {
        let single = Single::default();
        let (started, running) = std::sync::mpsc::channel::<()>();
        let (release, held) = std::sync::mpsc::channel::<()>();
        let (seen, heard) = std::sync::mpsc::channel();
        let held = Arc::new(Mutex::new(held));
        let work = move |given: u32| {
            if given == 1 {
                let _ = started.send(());
                let _ = held.held().recv();
            }
            let _ = seen.send(given);
        };
        single.ask("first", 1, work.clone());
        running.recv().unwrap();
        single.ask("second", 2, work.clone());
        single.ask("third", 3, work);
        release.send(()).unwrap();
        let got: Vec<u32> = (0..2).map(|_| heard.recv().unwrap()).collect();
        assert_eq!(got, [1, 3]);
        assert!(
            heard
                .recv_timeout(std::time::Duration::from_millis(50))
                .is_err()
        );
    }
}
