mod left;
mod middle;
mod side;

use super::rows::Scrolling;
use super::text_field::{Edited, TextField};
use super::theme::Theme;
use crate::changes::Changes;
use crate::workspace::logs::{self, LogsReport};
use gpui::{AppContext, Context, Entity, FocusHandle, Window};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Errors,
    Mods,
    Log,
}

const READS: Changes = Changes::LOAD_ORDER
    .or(Changes::LIBRARY)
    .or(Changes::LOGS)
    .or(Changes::LAUNCHES)
    .or(Changes::LANGUAGE);

pub struct Logs {
    unseen: crate::ui::section::Unseen,
    demo: bool,
    report: Option<Rc<LogsReport>>,
    problem: Option<String>,
    busy: bool,
    /// Read again once the read under way ends.
    stale: bool,
    told_launches: bool,
    /// The game's log before the last, or one RimStack kept, instead of the last.
    previous: bool,
    kept: Option<String>,
    /// A recorded launch shown in place of a log, by its place among the launches.
    recorded: Option<usize>,
    view: View,
    selected: usize,
    culprit: usize,
    /// The log line marked, by its number; 0 for none.
    marked: usize,
    open_runs: HashSet<usize>,
    filter: Entity<TextField>,
    filter_text: String,
    lines: Rc<Vec<usize>>,
    errors_scroll: Scrolling,
    culprits_scroll: Scrolling,
    log_scroll: Scrolling,
    launches_paging: super::page::Paging,
    side_paging: super::page::Paging,
    focus: Lists,
    /// `wrapLog` and `wrapTrace` of `[Appearance]`: long lines wrapped in the log and the trace.
    wrap_log: bool,
    wrap_trace: bool,
    hunting: bool,
    /// The log's row tops, for the width and the wrapping they were measured at.
    log_tops: Option<(gpui::Pixels, Rc<[gpui::Pixels]>)>,
}

struct Lists {
    launches: FocusHandle,
    errors: FocusHandle,
    culprits: FocusHandle,
    log: FocusHandle,
}

impl Logs {
    pub fn new(demo: bool, view: usize, cx: &mut Context<Self>) -> Self {
        let theme = cx.global::<Theme>().clone();
        let filter = cx.new(|cx| TextField::new(theme.body(), cx));
        cx.subscribe(&filter, |logs, field, _: &Edited, cx| {
            logs.filter_text = field.read(cx).text().trim().to_lowercase();
            logs.selected = 0;
            logs.list();
            cx.notify();
        })
        .detach();
        let scrolling = || {
            let scroll: Scrolling = Scrolling::default();
            scroll
                .borrow_mut()
                .set_reduce_motion(theme.appearance.reduce_motion);
            scroll
        };
        let mut logs = Self {
            unseen: Default::default(),
            demo,
            report: None,
            problem: None,
            busy: false,
            stale: true,
            told_launches: false,
            previous: false,
            kept: None,
            recorded: None,
            view: [View::Errors, View::Mods, View::Log][view.min(2)],
            selected: 0,
            culprit: 0,
            marked: 0,
            open_runs: HashSet::new(),
            filter,
            filter_text: String::new(),
            lines: Rc::default(),
            errors_scroll: scrolling(),
            culprits_scroll: scrolling(),
            log_scroll: scrolling(),
            launches_paging: super::page::Paging::new(theme.appearance.reduce_motion),
            side_paging: super::page::Paging::new(theme.appearance.reduce_motion),
            focus: Lists {
                launches: cx.focus_handle(),
                errors: cx.focus_handle(),
                culprits: cx.focus_handle(),
                log: cx.focus_handle(),
            },
            wrap_log: super::app(cx)
                .prefs()
                .flag("Appearance", "wrapLog")
                .unwrap_or(false),
            wrap_trace: super::app(cx)
                .prefs()
                .flag("Appearance", "wrapTrace")
                .unwrap_or(true),
            log_tops: None,
            hunting: !demo && super::app(cx).hunt().active,
        };
        if !demo {
            logs.hear(cx);
            logs.read(cx);
        }
        logs
    }

    pub fn show_view(&mut self, view: usize, cx: &mut Context<Self>) {
        self.view = [View::Errors, View::Mods, View::Log][view.min(2)];
        cx.notify();
    }

    pub fn set_hunting(&mut self, hunting: bool, cx: &mut Context<Self>) {
        self.hunting = hunting;
        cx.notify();
    }

    fn hear(&self, cx: &mut Context<Self>) {
        super::hear(cx, READS, Self::heard);
    }

    fn heard(&mut self, changes: Changes, cx: &mut Context<Self>) {
        let Some(changes) = self.unseen.pass(changes) else {
            return;
        };
        if changes.contains(Changes::LOAD_ORDER) {
            self.hunting = super::app(cx).hunt().active;
        }
        let mut read_again = Changes::LIBRARY | Changes::LOAD_ORDER | Changes::LOGS;
        if !std::mem::take(&mut self.told_launches) {
            read_again |= Changes::LAUNCHES;
        }
        if changes.contains(read_again) {
            self.stale = true;
            self.read(cx);
        } else if changes.contains(Changes::LANGUAGE) {
            cx.notify();
        }
    }

    /// The log chosen read again, off the window's thread; one read at a time.
    /// Read again now, as the command list asks.
    pub fn reread(&mut self, cx: &mut Context<Self>) {
        self.stale = true;
        self.read(cx);
    }

    fn read(&mut self, cx: &mut Context<Self>) {
        if !self.stale || self.busy || self.demo {
            return;
        }
        self.stale = false;
        self.busy = true;
        let app = super::app(cx);
        let (previous, kept) = (self.previous, self.kept.clone().map(PathBuf::from));
        crate::jobs::run(
            "log",
            cx,
            move || app.logs_report(previous, kept.as_deref()),
            |logs, read, cx| {
                match read {
                    Ok((report, changed)) => {
                        if changed {
                            logs.told_launches = true;
                            super::app(cx).publish(Changes::LAUNCHES);
                        }
                        logs.report = Some(Rc::new(report));
                        logs.problem = None;
                    }
                    Err(problem) => {
                        logs.report = None;
                        logs.problem = Some(crate::words::put(problem.sentence()));
                    }
                }
                logs.busy = false;
                logs.list();
                logs.log_tops = None;
                logs.read(cx);
                cx.notify();
            },
        );
        cx.notify();
    }

    fn list(&mut self) {
        let Some(report) = &self.report else {
            self.lines = Rc::default();
            return;
        };
        let filter = &self.filter_text;
        let kept = report
            .text
            .iter()
            .enumerate()
            .filter(|(_, text)| filter.is_empty() || text.to_lowercase().contains(filter))
            .map(|(at, _)| at);
        self.lines = Rc::new(kept.collect());
        self.log_tops = None;
    }

    /// The last log, the one before it, or one kept, by its path.
    fn show(&mut self, previous: bool, kept: Option<String>, cx: &mut Context<Self>) {
        self.recorded = None;
        self.selected = 0;
        self.previous = previous;
        self.kept = kept;
        self.stale = true;
        self.open_runs.clear();
        self.read(cx);
        cx.notify();
    }

    fn errors<'a>(&self, report: &'a LogsReport) -> Vec<logs::ErrorShown<'a>> {
        logs::listed(&report.errors, &self.filter_text, &self.open_runs)
    }

    fn toggle_run(&mut self, run: usize, cx: &mut Context<Self>) {
        if !self.open_runs.remove(&run) {
            self.open_runs.insert(run);
        }
        cx.notify();
    }

    fn show_line(&mut self, number: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.filter.update(cx, |field, cx| field.set_text("", cx));
        self.filter_text.clear();
        self.list();
        self.view = View::Log;
        self.marked = number;
        let at = self
            .lines
            .iter()
            .position(|line| line + 1 == number)
            .unwrap_or(0);
        let tops = self.log_tops(&cx.global::<Theme>().clone(), window);
        if let (Some(top), Some(bottom)) = (tops.get(at), tops.get(at + 1)) {
            self.log_scroll.borrow_mut().centre(*top, *bottom);
        }
        window.focus(&self.focus.log, cx);
        cx.notify();
    }

    fn open_culprit(&mut self, name: &str, cx: &mut Context<Self>) {
        let text = if name.is_empty() {
            rimstack_core::rimworld::logs::NO_MOD
        } else {
            name
        };
        self.view = View::Errors;
        self.filter.update(cx, |field, cx| field.set_text(text, cx));
        cx.notify();
    }

    fn keep_wrap(&mut self, key: &'static str, on: bool, cx: &mut Context<Self>) {
        super::keep_appearance(key, if on { "true" } else { "false" }, cx);
        match key {
            "wrapLog" => self.wrap_log = on,
            _ => self.wrap_trace = on,
        }
        self.log_tops = None;
        cx.notify();
    }
}

impl gpui::Render for Logs {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        use gpui::{ParentElement, Styled};
        let theme = cx.global::<Theme>().clone();
        let record = self
            .recorded
            .and_then(|at| self.launch_record(at, &theme, window, cx));
        gpui::div()
            .relative()
            .flex()
            .size_full()
            .child(self.launches_pane(&theme, window, cx))
            .child(self.middle(&theme, window, cx))
            .child(self.side(&theme, window, cx))
            .children(record)
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests;

impl crate::ui::section::Shown for Logs {
    fn set_shown(&mut self, shown: bool, cx: &mut Context<Self>) {
        if let Some(missed) = self.unseen.shown(shown) {
            self.heard(missed, cx);
        }
    }
}
