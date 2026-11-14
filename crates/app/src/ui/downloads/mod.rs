mod changed;
mod copies;
pub mod list;
mod pane;
mod screen;
mod side;
mod tasks;
#[cfg(test)]
mod tests;

use super::page::Paging;
use super::text_field::TextField;
use super::theme::Theme;
use crate::changes::Changes;
use crate::jobs::Lane;
use crate::reports::SettingsReport;
use crate::workspace::changed::{ChangeNotes, NotesState, Only, SinceLaunch};
use crate::workspace::download_list::ListShown;
use crate::workspace::downloads::{Queue, Updates};
use gpui::{AppContext, Context, Entity};
use rimstack_core::base::error::Error;
use rimstack_core::kept::settings::Switch;
use rimstack_core::mods::library::ModId;
use rimstack_core::sources::downloads::Replaceable;
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;
use std::time::Duration;

const TICK: Duration = Duration::from_secs(60);

/// A task waiting for the one before it: two would write the same SteamCMD folder.
struct Waiting {
    doing: String,
    updating: Option<ModId>,
    work: Box<dyn FnOnce() -> crate::workspace::fetch::Done + Send>,
}

#[derive(Default)]
struct Tasks {
    running: bool,
    note: String,
    updating: Option<ModId>,
    cloning: Option<usize>,
    waiting: VecDeque<Waiting>,
}

struct Read {
    report: SettingsReport,
    replacements: Vec<Replaceable>,
    steamcmd: bool,
    list: ListShown,
}

const READS: Changes = Changes::SETTINGS
    .or(Changes::LANGUAGE)
    .or(Changes::FOLDERS)
    .or(Changes::LIBRARY)
    .or(Changes::COMMUNITY)
    .or(Changes::DOWNLOAD_LIST)
    .or(Changes::UPDATES)
    .or(Changes::LAUNCHES)
    .or(Changes::LOGS);

pub struct Downloads {
    demo: bool,
    /// The load order's mods its rules ask for that are not installed, as the Mods tab last told.
    needed: Rc<Vec<crate::workspace::load_order::ProblemSaid>>,
    report: Rc<SettingsReport>,
    updates: Updates,
    updates_busy: bool,
    asked_of_steam: HashSet<String>,
    steam_waits: bool,
    tried: u64,
    looked_up: Result<Queue, Error>,
    queue_busy: bool,
    pasted: Entity<TextField>,
    changed: SinceLaunch,
    fetched: Rc<ChangeNotes>,
    since_only: Only,
    since_reading: Lane,
    reading: Lane,
    notes_busy: bool,
    notes_state: Option<NotesState>,
    replacements: Vec<Replaceable>,
    steamcmd: bool,
    list: Rc<ListShown>,
    list_view: Entity<list::DownloadList>,
    tasks: Tasks,
    inactive_open: bool,
    since_open: bool,
    installed_open: bool,
    /// The cards folded against how they start, by their key.
    folded: HashSet<&'static str>,
    paging: Paging,
    side_paging: Paging,
    side: gpui::Pixels,
    side_kept: Lane,
    side_keys: gpui::FocusHandle,
    /// The changed mods' list, reached by Tab: the row its arrows are on, and where each row was
    /// last drawn, to scroll the one reached into view.
    changed_keys: gpui::FocusHandle,
    changed_at: usize,
    changed_drawn: Rc<std::cell::RefCell<Vec<gpui::Bounds<gpui::Pixels>>>>,
}

/// The button under a list cut at `first`: the rest, or back to `first` once `open`.
fn more(
    id: impl Into<gpui::ElementId>,
    open: bool,
    total: usize,
    first: usize,
    cx: &mut Context<Downloads>,
    toggle: impl Fn(&mut Downloads, &mut Context<Downloads>) + 'static,
) -> gpui::AnyElement {
    use gpui::{IntoElement, ParentElement, Styled};
    let said = match open {
        true => crate::words::put(&rimstack_core::say!("Show {}", first)),
        false => crate::words::put(&rimstack_core::say!("{} more", total - first)),
    };
    let button =
        super::widgets::Btn::new(id, said).on_click(cx.listener(move |d, _, _, cx| toggle(d, cx)));
    gpui::div().flex().child(button).into_any_element()
}

fn to_get(
    overview: &Option<Rc<crate::workspace::load_order::Overview>>,
) -> Vec<crate::workspace::load_order::ProblemSaid> {
    overview
        .as_ref()
        .map(|o| o.to_get.clone())
        .unwrap_or_default()
}

impl Downloads {
    fn tell_listed(&self, cx: &mut Context<Self>) {
        let fresh = super::listed::Listed {
            list: self.list.clone(),
            local_mods: self.report.local_mods.clone().unwrap_or_default(),
            steamcmd: self.steamcmd,
            notes: self.fetched.clone(),
            notes_busy: self.notes_busy,
            notes_label: self.ask_steam_label(),
        };
        super::listed::change(cx, fresh);
    }
}

impl Downloads {
    pub fn new(demo: bool, cx: &mut Context<Self>) -> Self {
        let theme = cx.global::<Theme>().clone();
        let pasted = cx.new(|cx| TextField::new(theme.body(), cx));
        cx.observe(&pasted, |_, _, cx| cx.notify()).detach();
        cx.subscribe(
            &pasted,
            |downloads, _, _: &super::text_field::Entered, cx| downloads.look_up(cx),
        )
        .detach();
        let order = super::order::order(cx);
        cx.observe(&order, |downloads, order, cx| {
            let needed = to_get(&order.read(cx).overview);
            if *downloads.needed != needed {
                downloads.needed = Rc::new(needed);
                cx.notify();
            }
        })
        .detach();
        cx.observe_self(|downloads, cx| downloads.tell_listed(cx))
            .detach();
        let reduce = theme.appearance.reduce_motion;
        let side = match demo {
            true => None,
            false => super::app(cx).downloads_side(),
        };
        let mut downloads = Self {
            demo,
            needed: Rc::new(to_get(&super::order::read(cx).overview)),
            report: Rc::default(),
            updates: Updates::default(),
            updates_busy: false,
            asked_of_steam: HashSet::new(),
            steam_waits: false,
            tried: 0,
            looked_up: Ok(Queue::default()),
            queue_busy: false,
            pasted,
            changed: SinceLaunch::default(),
            fetched: Rc::default(),
            since_only: Only::All,
            since_reading: Lane::default(),
            notes_busy: false,
            notes_state: None,
            replacements: Vec::new(),
            steamcmd: false,
            list: Rc::default(),
            list_view: cx.new(|cx| list::DownloadList::new(false, cx)),
            reading: Lane::default(),
            tasks: Tasks::default(),
            inactive_open: false,
            since_open: false,
            installed_open: false,
            folded: HashSet::new(),
            paging: Paging::new(reduce),
            side_paging: Paging::new(reduce),
            side: side.map_or_else(|| pane::first_side(&theme), gpui::px),
            side_kept: Lane::default(),
            side_keys: cx.focus_handle().tab_stop(true),
            changed_keys: cx.focus_handle().tab_stop(true),
            changed_at: 0,
            changed_drawn: Rc::default(),
        };
        downloads.read(cx);
        if !demo {
            downloads.hear(cx);
            downloads.heard(Changes::LIBRARY | Changes::SETTINGS, cx);
            downloads.keep_time(cx);
        }
        downloads
    }

    fn hear(&self, cx: &mut Context<Self>) {
        super::hear(cx, READS, Self::heard);
    }

    /// What Downloads shows of the `App` beside the updates and the changes, read off the
    /// window's thread: the library may still be being read. The demo shows no list and no
    /// SteamCMD.
    fn read(&mut self, cx: &mut Context<Self>) {
        let app = super::app(cx);
        self.reading.run(
            "downloads read",
            cx,
            move |_| Read {
                report: app.settings_report(),
                replacements: app.replaceable(),
                steamcmd: app.steamcmd().is_some(),
                list: app.list_shown(),
            },
            |downloads, read, cx| {
                downloads.report = Rc::new(read.report);
                downloads.replacements = read.replacements;
                downloads.steamcmd = read.steamcmd;
                downloads.list = Rc::new(read.list);
                cx.notify();
            },
        );
    }

    fn heard(&mut self, changes: Changes, cx: &mut Context<Self>) {
        if changes.contains(Changes::SETTINGS) {
            self.tick(cx);
        }
        let read = Changes::SETTINGS
            | Changes::LANGUAGE
            | Changes::FOLDERS
            | Changes::LIBRARY
            | Changes::COMMUNITY
            | Changes::DOWNLOAD_LIST;
        if changes.contains(read) {
            self.read(cx);
        }
        if changes.contains(Changes::LIBRARY | Changes::UPDATES) {
            self.restore_updates(cx);
        }
        if changes.contains(Changes::LIBRARY | Changes::UPDATES | Changes::LAUNCHES | Changes::LOGS)
        {
            self.read_since(cx);
        }
    }

    fn keep_time(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |downloads, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if downloads
                    .update(cx, |downloads, cx| downloads.tick(cx))
                    .is_err()
                {
                    return;
                }
            }
        })
        .detach();
    }

    fn by_itself(&self, cx: &Context<Self>) -> bool {
        super::app(cx).settings().switch(Switch::AutoUpdates)
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        if !self.by_itself(cx) || self.updates_busy {
            return;
        }
        let every = super::app(cx).settings().update_every_minutes() * 60;
        let last = self.updates.checked.unwrap_or(0).max(self.tried);
        if crate::workspace::now_secs().saturating_sub(last) >= every {
            self.tried = crate::workspace::now_secs();
            self.check_updates(cx);
        }
    }

    fn restore_updates(&mut self, cx: &mut Context<Self>) {
        let app = super::app(cx);
        crate::jobs::run(
            "updates restore",
            cx,
            move || app.kept_updates(),
            |downloads, kept, cx| {
                let Some(mut kept) = kept.filter(|_| !downloads.updates_busy) else {
                    return;
                };
                if kept.checked == downloads.updates.checked {
                    kept.problem = downloads.updates.problem.clone();
                }
                downloads.show_updates(kept, cx);
            },
        );
    }

    fn show_updates(&mut self, updates: Updates, cx: &mut Context<Self>) {
        let behind: HashSet<&String> = updates.items.iter().map(|row| &row.id).collect();
        self.asked_of_steam.retain(|id| behind.contains(id));
        let active = updates.items.iter().filter(|row| row.active).count();
        self.updates = updates;
        super::counts::change(cx, |counts| counts.behind = active);
        cx.notify();
    }

    pub fn check_updates(&mut self, cx: &mut Context<Self>) {
        if self.updates_busy || self.demo {
            return;
        }
        self.updates_busy = true;
        let app = super::app(cx);
        crate::jobs::run(
            "updates check",
            cx,
            move || {
                let checked = app.check_updates();
                app.publish(Changes::UPDATES);
                checked
            },
            |downloads, checked, cx| {
                downloads.updates_busy = false;
                match checked {
                    Ok(updates) => {
                        downloads.show_updates(updates, cx);
                        if downloads.by_itself(cx) {
                            downloads.apply(cx);
                        }
                    }
                    Err(problem) => {
                        let failed = Updates {
                            problem: Some(problem.sentence().clone()),
                            ..Updates::default()
                        };
                        downloads.show_updates(failed, cx);
                    }
                }
            },
        );
        cx.notify();
    }

    pub fn look_up(&mut self, cx: &mut Context<Self>) {
        let pasted = self.pasted.read(cx).text().trim().to_string();
        if self.queue_busy || self.demo || pasted.is_empty() {
            return;
        }
        self.queue_busy = true;
        let app = super::app(cx);
        crate::jobs::run(
            "look up",
            cx,
            move || app.look_up(&pasted),
            |downloads, looked_up, cx| {
                downloads.looked_up = looked_up;
                downloads.queue_busy = false;
                cx.notify();
            },
        );
        cx.notify();
    }

    fn read_since(&mut self, cx: &mut Context<Self>) {
        let app = super::app(cx);
        self.since_reading.run(
            "changed since launch",
            cx,
            move |_| app.since_launch(),
            |downloads, changed, cx| {
                let count = changed.items.len();
                downloads.changed = changed;
                super::counts::change(cx, |counts| counts.changed_since = count);
                cx.notify();
            },
        );
    }

    pub fn keep_setting(&mut self, name: &str, value: &str, cx: &mut Context<Self>) {
        if let Err(problem) = super::app(cx).keep_setting(name, value) {
            rimstack_core::problem!("{problem}");
        }
    }

    pub fn ignore_replacement(&mut self, id: Option<ModId>, cx: &mut Context<Self>) {
        if let Err(problem) = super::app(cx).ignore_replacement(id) {
            rimstack_core::problem!("{problem}");
        }
    }

    fn fold(&mut self, key: &'static str, cx: &mut Context<Self>) {
        if !self.folded.remove(key) {
            self.folded.insert(key);
        }
        cx.notify();
    }

    fn is_folded(&self, key: &'static str, starts_folded: bool) -> bool {
        self.folded.contains(key) != starts_folded
    }

    fn needed(&self) -> Rc<Vec<crate::workspace::load_order::ProblemSaid>> {
        self.needed.clone()
    }
}
