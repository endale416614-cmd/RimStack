use crate::changes::Changes;
use gpui::{Context, EventEmitter};
use rimstack_core::base::watch::{Stamps, Watcher};
use std::time::Duration;

pub enum Told {
    /// The game emptied its list on a new version; the last launch from RimStack had these.
    ResetByGame(Vec<String>),
    /// ModsConfig.xml was written outside RimStack and now lists these.
    WrittenOutside(Vec<String>),
    NotStarted(String),
}

pub struct Game {
    running: bool,
    starting: bool,
    watcher: Watcher,
    demo: bool,
    hidden: bool,
    /// What is under way on the load order: a change seen on disk waits for it.
    held: Box<dyn Fn(&gpui::App) -> bool>,
}

impl EventEmitter<Told> for Game {}

/// How often the disk is looked at; `None` never, as for the demo and for pictures.
fn every(demo: bool, still: bool) -> Option<Duration> {
    (!demo && !still).then_some(Duration::from_secs(2))
}

/// While the window shows nothing, one look in this many. Not none: gpui-fast reports a window
/// wholly behind another as it reports a minimised one, and behind the running game RimStack
/// must still see it start and stop.
pub(crate) const HIDDEN_LOOKS_ONE_IN: u32 = 5;

impl Game {
    pub fn new(
        demo: bool,
        still: bool,
        held: impl Fn(&gpui::App) -> bool + 'static,
        cx: &mut Context<Self>,
    ) -> Self {
        let game = Self {
            running: false,
            starting: false,
            watcher: Watcher::default(),
            demo,
            hidden: false,
            held: Box::new(held),
        };
        if let Some(every) = every(demo, still) {
            game.watch(every, cx);
        }
        game
    }

    pub fn running(&self) -> bool {
        self.running
    }

    pub fn starting(&self) -> bool {
        self.starting
    }

    pub fn shown(&mut self, visible: bool) {
        self.hidden = !visible;
    }

    fn watch(&self, every: Duration, cx: &mut Context<Self>) {
        let app = super::app(cx);
        let at_start = app.clone();
        crate::jobs::run(
            "emptied list",
            cx,
            move || at_start.list_reset_by_game(),
            |_, ids, cx| {
                if let Some(ids) = ids {
                    cx.emit(Told::ResetByGame(ids));
                }
            },
        );
        cx.spawn(async move |game, cx| {
            let mut skipped = 0;
            loop {
                cx.background_executor().timer(every).await;
                let Ok(hidden) = game.read_with(cx, |game, _| game.hidden) else {
                    return;
                };
                if hidden && skipped + 1 < HIDDEN_LOOKS_ONE_IN {
                    skipped += 1;
                    continue;
                }
                skipped = 0;
                let app = app.clone();
                let looked =
                    crate::jobs::off_thread("watch", move || (app.game_running(), app.stamps()));
                let Ok((running, stamps)) = looked.await else {
                    return;
                };
                if game
                    .update(cx, |game, cx| game.looked(running, stamps, cx))
                    .is_err()
                {
                    return;
                }
            }
        })
        .detach();
    }

    fn looked(&mut self, running: bool, stamps: Stamps, cx: &mut Context<Self>) {
        let app = super::app(cx);
        if running != self.running {
            self.running = running;
            if running {
                let app = app.clone();
                crate::jobs::spawn("versions at start", move || app.record_versions());
            }
            cx.notify();
        }
        let held = (self.held)(cx);
        let noticed = self.watcher.look(stamps, running, held);
        if noticed.mods {
            app.rescan_soon();
        }
        let mut changes = Changes::NONE;
        for (seen, change) in [
            (noticed.saves, Changes::SAVES),
            (noticed.log, Changes::LOGS),
            (noticed.theme, Changes::THEME),
            (noticed.language, Changes::LANGUAGE),
        ] {
            if seen {
                changes |= change;
            }
        }
        if !changes.is_empty() {
            app.publish(changes);
        }
        // Never taken unasked: the game empties the file when it first starts on a new version.
        if noticed.mods_config {
            crate::jobs::run(
                "mods config",
                cx,
                move || app.mods_config_list(),
                |_, ids, cx| {
                    if let Some(ids) = ids {
                        cx.emit(Told::WrittenOutside(ids));
                    }
                },
            );
        }
    }

    /// Starts the game; `round` for a round of the search for a broken mod, which is not recorded.
    pub fn play(&mut self, round: bool, cx: &mut Context<Self>) {
        if self.starting || self.running {
            return;
        }
        self.starting = true;
        let app = super::app(cx);
        let record = !self.demo && !round;
        crate::jobs::run(
            "game start",
            cx,
            move || app.launch(record),
            move |game, started, cx| {
                game.starting = false;
                match started {
                    Ok(address) => {
                        if let Some(address) = address {
                            cx.open_url(&address);
                        }
                        if record {
                            super::app(cx).publish(Changes::LAUNCHES);
                        }
                    }
                    Err(problem) => {
                        rimstack_core::problem!("game not started: {problem}");
                        cx.emit(Told::NotStarted(crate::words::said_of(&problem)));
                    }
                }
                cx.notify();
            },
        );
        cx.notify();
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use gpui::{AppContext, HeadlessAppContext};
    use std::path::Path;
    use std::sync::Arc;

    fn listed(app: &crate::App, folder: &Path) -> bool {
        crate::jobs::waiting_here(|| match &*app.library() {
            Ok((library, _)) => library.installed.iter().any(|m| m.path == folder),
            Err(_) => false,
        })
    }

    fn look(cx: &mut HeadlessAppContext) {
        cx.advance_clock(Duration::from_secs(2));
        cx.run_until_parked();
    }

    #[test]
    fn a_mod_folder_removed_outside_is_read_away_once_the_folder_is_quiet() {
        let (tmp, app) = crate::ui::made_up::made_up();
        let app = Arc::new(app);
        let mut cx = HeadlessAppContext::with_asset_source(
            crate::ui::made_up::text_system(),
            Arc::new(crate::ui::assets::Assets),
        );
        cx.update(|cx| crate::ui::install(app.clone(), &crate::ui::Options::default(), cx));
        let mods = tmp.path().join("fx/game/Mods");
        let gone = std::fs::read_dir(&mods)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert!(listed(&app, &gone));
        let _game = cx.new(|cx| Game::new(false, false, |_| false, cx));
        cx.run_until_parked();
        look(&mut cx);
        std::fs::remove_dir_all(&gone).unwrap();
        for _ in 0..4 {
            look(&mut cx);
        }
        assert!(!listed(&app, &gone));
    }
}
