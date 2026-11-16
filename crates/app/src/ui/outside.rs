//! What the details and the status bar show of the game beside the mods: the saves and kept
//! lists counted, and the game's last start. Read off the window's thread when it changes; a view
//! that reads it while drawn is drawn again when it does.

use crate::changes::Changes;
use crate::jobs::Lane;
use crate::reports::Summary;
use crate::workspace::logs::LastStartSaid;
use gpui::{App as Cx, AppContext, Context, Entity, Global};
use std::rc::Rc;

const COUNTED: Changes = Changes::LIBRARY
    .or(Changes::LISTS)
    .or(Changes::LAUNCHES)
    .or(Changes::SAVES)
    .or(Changes::RULES)
    .or(Changes::UPDATES)
    .or(Changes::COMMUNITY)
    .or(Changes::LANGUAGE);
const STARTED: Changes = Changes::LAUNCHES
    .or(Changes::LOGS)
    .or(Changes::FOLDERS)
    .or(Changes::LANGUAGE);
const READS: Changes = COUNTED.or(STARTED);

#[derive(Default)]
pub struct Outside {
    pub summary: Option<Rc<Summary>>,
    pub last_start: Option<Rc<LastStartSaid>>,
    summary_reading: Lane,
    last_start_reading: Lane,
}

struct Held(Entity<Outside>);

impl Global for Held {}

pub fn install(demo: bool, cx: &mut Cx) {
    let outside = cx.new(|cx| Outside::new(demo, cx));
    cx.set_global(Held(outside));
}

pub fn outside(cx: &Cx) -> Entity<Outside> {
    cx.global::<Held>().0.clone()
}

impl Outside {
    fn new(demo: bool, cx: &mut Context<Self>) -> Self {
        let mut outside = Self::default();
        if !demo {
            super::hear(cx, READS, Self::read);
            outside.read(Changes::LAUNCHES | Changes::LISTS, cx);
        }
        outside
    }

    fn read(&mut self, changes: Changes, cx: &mut Context<Self>) {
        if changes.contains(COUNTED) {
            let app = super::app(cx);
            self.summary_reading.run(
                "summary",
                cx,
                move |_| app.summary(),
                |outside, summary, cx| {
                    outside.summary = Some(Rc::new(summary));
                    cx.notify();
                },
            );
        }
        if changes.contains(STARTED) {
            let app = super::app(cx);
            self.last_start_reading.run(
                "last start",
                cx,
                move |_| app.last_start_said(),
                |outside, said, cx| {
                    outside.last_start = said.map(Rc::new);
                    cx.notify();
                },
            );
        }
    }
}
