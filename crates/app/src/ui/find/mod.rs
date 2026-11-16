mod layout;
mod preview;
mod screen;
mod side;

use super::rows::Scrolling;
use super::text_field::{Edited, Entered, TextField};
use super::theme::Theme;
use crate::changes::Changes;
use crate::jobs::Lane;
use crate::words::why_not;
use crate::workspace::find::{
    FoundMods, Op, Options, Prepared, PreviewShown, Recent, Request, RowRef, TypeOffer, type_offers,
};
use gpui::{AppContext, Context, Entity, FocusHandle, Pixels, Window};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::find::{Batch, Summary};
use rimstack_core::mods::library::ModId;
use rimstack_core::{say, say_n};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

gpui::actions!(find, [FocusSearch]);

const TYPES_PAUSE: Duration = Duration::from_millis(400);
const MODS_EVERY: Duration = Duration::from_millis(200);

enum Heard {
    Batch(Batch<Option<ModId>>),
    Done(Summary),
}

#[derive(Default)]
struct Viewing {
    file: PathBuf,
    hit: usize,
    found: Vec<usize>,
    at: Option<usize>,
    said: Option<Sentence>,
}

const READS: Changes = Changes::LIBRARY
    .or(Changes::LOAD_ORDER)
    .or(Changes::LANGUAGE);

pub struct Find {
    unseen: crate::ui::section::Unseen,
    selected: Option<ModId>,
    request: Request,
    search: Entity<TextField>,
    types: Entity<TextField>,
    filter: Entity<TextField>,
    inside: Entity<TextField>,
    running: bool,
    done: usize,
    total: usize,
    working: Option<Sentence>,
    said: Option<Sentence>,
    problem: String,
    capped: bool,
    count: usize,
    generation: u64,
    rows: Vec<RowRef>,
    /// Each row's top and, last, the whole height, for the row height they were measured at.
    tops: Option<(Pixels, Rc<[Pixels]>)>,
    current: Option<usize>,
    recent: Rc<Vec<Recent>>,
    options: Rc<Options>,
    found: Rc<FoundMods>,
    extensions: Vec<(String, usize)>,
    offer_at: usize,
    offers_shut: bool,
    viewing: Viewing,
    preview: Option<Rc<PreviewShown>>,
    /// The preview's line to bring to the middle once its rows are measured.
    centred: Option<usize>,
    wrap: bool,
    filters: bool,
    mods_shown: Option<Instant>,
    /// The row chosen once the search ends, with the file searched for its text, for pictures.
    pick_after: Option<usize>,
    previews: Lane,
    retyped: Lane,
    checks: Lane,
    prepares: Lane,
    counted: Lane,
    scroll: Scrolling,
    preview_scroll: Scrolling,
    found_scroll: Scrolling,
    options_paging: super::page::Paging,
    focus: FocusHandle,
}

impl Find {
    pub fn new(demo: bool, cx: &mut Context<Self>) -> Self {
        let theme = cx.global::<Theme>().clone();
        let field = |cx: &mut Context<Self>| cx.new(|cx| TextField::new(theme.body(), cx));
        let (search, types, filter, inside) = (field(cx), field(cx), field(cx), field(cx));
        types.update(cx, |field, cx| field.set_text("xml", cx));
        cx.subscribe(&search, |find, _, _: &Edited, cx| find.check(cx))
            .detach();
        cx.subscribe(&search, |find, _, _: &Entered, cx| find.begin(false, cx))
            .detach();
        cx.subscribe(&types, |find, _, _: &Edited, cx| find.types_typed(cx))
            .detach();
        cx.subscribe(&types, |find, _, _: &Entered, cx| find.complete_type(cx))
            .detach();
        cx.subscribe(&filter, |find, field, _: &Edited, cx| {
            let text = field.read(cx).text().to_string();
            find.filter_results(&text, cx);
        })
        .detach();
        cx.subscribe(&inside, |find, _, _: &Edited, cx| find.inside_typed(cx))
            .detach();
        cx.subscribe(&inside, |find, _, _: &Entered, cx| find.inside_step(1, cx))
            .detach();
        cx.observe(&super::order::order(cx), |find, order, cx| {
            let selected = order.read(cx).selected;
            if selected != find.selected {
                find.selected = selected;
                find.count_extensions(cx);
                find.show_options(cx);
            }
        })
        .detach();
        let scrolling = || {
            let scroll = Scrolling::default();
            scroll
                .borrow_mut()
                .set_reduce_motion(theme.appearance.reduce_motion);
            scroll
        };
        let selected = super::order::read(cx).selected;
        let mut find = Self {
            unseen: Default::default(),
            selected,
            request: Request::default(),
            search,
            types,
            filter,
            inside,
            running: false,
            done: 0,
            total: 0,
            working: None,
            said: None,
            problem: String::new(),
            capped: false,
            count: 0,
            generation: 0,
            rows: Vec::new(),
            tops: None,
            current: None,
            recent: Rc::default(),
            options: Rc::new(super::app(cx).find_options(&Request::default())),
            found: Rc::new(super::app(cx).find_results(|r| r.found_mods())),
            extensions: Vec::new(),
            offer_at: 0,
            offers_shut: false,
            viewing: Viewing::default(),
            preview: None,
            centred: None,
            wrap: false,
            filters: false,
            mods_shown: None,
            pick_after: None,
            previews: Lane::default(),
            retyped: Lane::default(),
            checks: Lane::default(),
            prepares: Lane::default(),
            counted: Lane::default(),
            scroll: scrolling(),
            preview_scroll: scrolling(),
            found_scroll: scrolling(),
            options_paging: super::page::Paging::new(theme.appearance.reduce_motion),
            focus: cx.focus_handle(),
        };
        find.show_options(cx);
        if !demo {
            find.hear(cx);
            find.count_extensions(cx);
            let app = super::app(cx);
            crate::jobs::run(
                "find recent",
                cx,
                move || app.find_recent(),
                |find, recent, cx| {
                    find.recent = Rc::new(recent);
                    cx.notify();
                },
            );
        }
        find
    }

    fn hear(&self, cx: &mut Context<Self>) {
        super::hear(cx, READS, Self::heard);
    }

    fn heard(&mut self, changes: Changes, cx: &mut Context<Self>) {
        let Some(changes) = self.unseen.pass(changes) else {
            return;
        };
        if changes.contains(Changes::LIBRARY | Changes::LOAD_ORDER) {
            self.count_extensions(cx);
        }
        if changes.contains(Changes::LANGUAGE) {
            self.show_options(cx);
        }
    }

    pub fn focus_search(&self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = gpui::Focusable::focus_handle(self.search.read(cx), cx);
        window.focus(&focus, cx);
    }

    fn request_now(&self, cx: &gpui::App) -> Request {
        Request {
            text: self.search.read(cx).text().to_string(),
            types: self.types.read(cx).text().to_string(),
            selected: self.selected,
            ..self.request.clone()
        }
    }

    fn show_options(&mut self, cx: &mut Context<Self>) {
        let request = self.request_now(cx);
        self.options = Rc::new(super::app(cx).find_options(&request));
        cx.notify();
    }

    fn check(&mut self, cx: &mut Context<Self>) {
        let app = super::app(cx);
        let (text, switches) = (
            self.search.read(cx).text().to_string(),
            self.request.switches,
        );
        self.checks.run(
            "find check",
            cx,
            move |_| app.find_check(&text, switches),
            |find, checked, cx| {
                find.problem = why_not(&checked);
                cx.notify();
            },
        );
        cx.notify();
    }

    fn count_extensions(&mut self, cx: &mut Context<Self>) {
        let (app, request) = (super::app(cx), self.request_now(cx));
        self.counted.run(
            "find extensions",
            cx,
            move |_| app.find_extensions(&request),
            |find, counts, cx| {
                find.extensions = counts;
                cx.notify();
            },
        );
    }

    fn offers(&self, cx: &gpui::App) -> Vec<TypeOffer> {
        type_offers(&self.extensions, self.types.read(cx).text())
    }

    fn complete_type(&mut self, cx: &mut Context<Self>) {
        self.complete_with(self.offer_at, cx);
    }

    fn complete_with(&mut self, at: usize, cx: &mut Context<Self>) {
        let text = self.types.read(cx).text().to_string();
        let Some(offer) = self.offers(cx).into_iter().nth(at) else {
            return;
        };
        let done = rimstack_core::mods::find::complete(&text, &offer.ext);
        self.types.update(cx, |field, cx| field.set_text(&done, cx));
    }

    fn step_offer(&mut self, by: isize, cx: &mut Context<Self>) {
        let count = self.offers(cx).len();
        if count > 0 {
            self.offer_at = (self.offer_at as isize + by).rem_euclid(count as isize) as usize;
            cx.notify();
        }
    }

    fn flip(&mut self, id: &str, cx: &mut Context<Self>) {
        let folders = self.request.flip(id);
        self.check(cx);
        self.options_moved(folders, cx);
    }

    fn choose(&mut self, id: &str, cx: &mut Context<Self>) {
        self.request.choose(id);
        self.options_moved(true, cx);
    }

    fn options_moved(&mut self, folders: bool, cx: &mut Context<Self>) {
        if folders {
            self.count_extensions(cx);
        }
        self.show_options(cx);
        self.rerun(cx);
    }

    fn types_typed(&mut self, cx: &mut Context<Self>) {
        let typed = self.types.read(cx).text().to_string();
        self.offers_shut = false;
        self.offer_at = 0;
        cx.notify();
        if typed == self.request.types {
            return;
        }
        self.request.types = typed;
        self.show_options(cx);
        self.retyped
            .after(TYPES_PAUSE, cx, |find, cx| find.rerun(cx));
    }

    fn rerun(&mut self, cx: &mut Context<Self>) {
        if !self.search.read(cx).text().trim().is_empty() {
            self.begin(true, cx);
        }
    }

    /// The search asked; `revised` when an option of the last one changed.
    fn begin(&mut self, revised: bool, cx: &mut Context<Self>) {
        if self.search.read(cx).text().trim().is_empty() {
            return;
        }
        self.retyped.cancel();
        self.generation = u64::MAX;
        self.running = true;
        let app = super::app(cx);
        let request = self.request_now(cx);
        let asked = request.clone();
        self.prepares.run(
            "find prepare",
            cx,
            move |_| app.find_prepare(&asked),
            move |find, prepared, cx| match prepared {
                Ok(prepared) => find.launch(request, revised, prepared, cx),
                Err(problem) => {
                    find.running = false;
                    if !revised {
                        find.problem = crate::words::said_of(&problem);
                    }
                    cx.notify();
                }
            },
        );
        cx.notify();
    }

    fn launch(
        &mut self,
        request: Request,
        revised: bool,
        prepared: Prepared,
        cx: &mut Context<Self>,
    ) {
        let app = super::app(cx);
        let started = app.find_launch(prepared);
        let generation = started.generation;
        self.problem.clear();
        self.renew(Vec::new());
        self.generation = generation;
        self.said = None;
        self.working = Some(say!("Searching"));
        self.preview = None;
        self.previews.cancel();
        self.viewing = Viewing::default();
        self.found = Rc::new(app.find_results(|r| r.found_mods()));
        self.mods_shown = Some(Instant::now());
        (self.capped, self.count, self.done, self.total) = (false, 0, 0, 0);
        self.running = true;
        let remember = app.clone();
        crate::jobs::run(
            "find remember",
            cx,
            move || {
                remember.find_remember(&request, revised);
                remember.find_recent()
            },
            |find, recent, cx| {
                find.recent = Rc::new(recent);
                cx.notify();
            },
        );
        let (tell, mut told) = futures::channel::mpsc::unbounded::<Heard>();
        crate::jobs::spawn("find", move || {
            let batches = tell.clone();
            let summary = app.find_search(&started, |batch| {
                let _ = batches.unbounded_send(Heard::Batch(batch));
            });
            let _ = tell.unbounded_send(Heard::Done(summary));
        });
        cx.spawn(async move |find, cx| {
            use futures::StreamExt;
            while let Some(heard) = told.next().await {
                let going = find.update(cx, |find, cx| match heard {
                    Heard::Batch(batch) => find.arrive(generation, batch, cx),
                    Heard::Done(summary) => find.finished(generation, summary, cx),
                });
                if going.is_err() {
                    return;
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn arrive(&mut self, generation: u64, batch: Batch<Option<ModId>>, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        let app = super::app(cx);
        let Some(arrived) = app.find_add(generation, batch) else {
            return;
        };
        for op in arrived.ops {
            if let Op::Insert { at, rows } = op
                && at <= self.rows.len()
            {
                let coming = rows.len();
                self.rows.splice(at..at, rows);
                if let Some(current) = self.current.as_mut()
                    && *current >= at
                {
                    *current += coming;
                }
                self.tops = None;
            }
        }
        (self.done, self.total) = (arrived.progress.done, arrived.progress.total);
        self.count = arrived.mods;
        self.capped = arrived.capped;
        if self.mods_shown.is_none_or(|at| at.elapsed() >= MODS_EVERY) {
            self.mods_shown = Some(Instant::now());
            self.found = Rc::new(app.find_results(|r| r.found_mods()));
        }
        self.working = Some(say!("Mods searched: {} of {}", self.done, self.total));
        cx.notify();
    }

    fn finished(&mut self, generation: u64, summary: Summary, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        let app = super::app(cx);
        self.said = app.find_finish(generation, summary);
        self.working = None;
        self.running = false;
        self.found = Rc::new(app.find_results(|r| r.found_mods()));
        if let Some(row) = self.pick_after.take() {
            let text = self.search.read(cx).text().to_string();
            self.inside
                .update(cx, |field, cx| field.set_text(&text, cx));
            self.select(Some(row), cx);
        }
        cx.notify();
    }

    pub fn search_for(&mut self, text: &str, then: Option<usize>, cx: &mut Context<Self>) {
        self.pick_after = then;
        self.search.update(cx, |field, cx| field.set_text(text, cx));
        self.begin(false, cx);
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        super::app(cx).find_stop();
    }

    fn renew(&mut self, rows: Vec<RowRef>) {
        let current = self.current.and_then(|at| self.rows.get(at).copied());
        self.current = current.and_then(|row| rows.iter().position(|&other| other == row));
        self.rows = rows;
        self.tops = None;
    }

    fn filter_results(&mut self, text: &str, cx: &mut Context<Self>) {
        let app = super::app(cx);
        let rows = app.find_change(|results| results.set_filter(text));
        self.renew(rows);
        self.found = Rc::new(app.find_results(|r| r.found_mods()));
        cx.notify();
    }

    fn pick_mod(&mut self, group: Option<usize>, cx: &mut Context<Self>) {
        let app = super::app(cx);
        let rows = app.find_change(|results| results.set_pick(group));
        self.renew(rows);
        self.found = Rc::new(app.find_results(|r| r.found_mods()));
        cx.notify();
    }

    fn toggle(&mut self, row: usize, cx: &mut Context<Self>) {
        let Some(&at) = self.rows.get(row) else {
            return;
        };
        let app = super::app(cx);
        let Some(folded) = app.find_change(|results| results.toggle(at)) else {
            return;
        };
        self.current = folded.apply(&mut self.rows, row, self.current);
        self.tops = None;
        cx.notify();
    }

    fn step(&mut self, by: i32, cx: &mut Context<Self>) {
        let moved = super::app(cx).find_results(|r| r.step(&self.rows, self.current, by));
        if let Some(row) = moved {
            self.select(Some(row), cx);
            self.reveal(row, cx);
        }
    }

    /// Folds the current row's file (`how` -1), unfolds it (1) or flips it (0), and chooses it.
    fn fold(&mut self, how: i32, cx: &mut Context<Self>) {
        let Some(row) = self.current else {
            return;
        };
        let Some(&at) = self.rows.get(row).filter(|at| !at.is_mod()) else {
            return;
        };
        let file_row = match at.is_file() {
            true => row,
            false => self.rows[..row]
                .iter()
                .rposition(|other| other.same_file(at) && other.is_file())
                .unwrap_or(row),
        };
        let folded = super::app(cx).find_results(|r| r.is_folded(at));
        if how != 0 && folded == (how < 0) {
            return;
        }
        self.toggle(file_row, cx);
        self.select(Some(file_row), cx);
        self.reveal(file_row, cx);
    }

    fn reveal(&mut self, row: usize, cx: &mut Context<Self>) {
        let theme = cx.global::<Theme>().clone();
        let tops = self.tops(&theme, cx);
        if row + 1 < tops.len() {
            self.scroll.borrow_mut().contain(tops[row], tops[row + 1]);
        }
    }

    fn row_height(theme: &Theme, at: RowRef) -> Pixels {
        match (at.is_mod(), at.is_file()) {
            (true, _) => theme.row() + super::theme::GAP_SMALL,
            (_, true) => theme.row(),
            _ => theme.line(theme.small()) + super::theme::GAP_HAIR,
        }
    }

    fn tops(&mut self, theme: &Theme, _: &gpui::App) -> Rc<[Pixels]> {
        if let Some((row, tops)) = &self.tops
            && *row == theme.row()
        {
            return tops.clone();
        }
        let mut at = Pixels::ZERO;
        let mut tops = Vec::with_capacity(self.rows.len() + 1);
        for &row in &self.rows {
            tops.push(at);
            at += Self::row_height(theme, row);
        }
        tops.push(at);
        let tops: Rc<[Pixels]> = tops.into();
        self.tops = Some((theme.row(), tops.clone()));
        tops
    }

    fn select(&mut self, row: Option<usize>, cx: &mut Context<Self>) {
        let app = super::app(cx);
        let located = row
            .and_then(|row| self.rows.get(row).copied())
            .filter(|at| !at.is_mod())
            .and_then(|at| app.find_located(at));
        let Some((file, hit)) = located else {
            self.previews.cancel();
            self.current = None;
            self.viewing = Viewing::default();
            self.preview = None;
            cx.notify();
            return;
        };
        self.current = row;
        self.viewing = Viewing {
            file,
            hit,
            ..Viewing::default()
        };
        self.look(0, false, cx);
        cx.notify();
    }

    /// The preview read again, `step` matches on inside the file; `again` keeps the lines
    /// found instead of searching the file for them.
    fn look(&mut self, step: i32, again: bool, cx: &mut Context<Self>) {
        use crate::workspace::find::Look;
        let app = super::app(cx);
        let (file, hit) = (self.viewing.file.clone(), self.viewing.hit);
        let look = Look {
            text: self.inside.read(cx).text().to_string(),
            found: again.then(|| self.viewing.found.clone()),
            at: self.viewing.at,
            step,
        };
        self.previews.run(
            "find preview",
            cx,
            move |_| app.find_preview(&file, hit, look),
            |find, looked, cx| {
                let text = find.inside.read(cx).text().to_string();
                find.viewing.said = said_inside(&text, looked.found.len(), looked.at);
                find.viewing.found = looked.found;
                find.viewing.at = looked.at;
                find.preview = Some(Rc::new(looked.shown));
                find.centre_preview(cx);
                cx.notify();
            },
        );
    }

    fn inside_typed(&mut self, cx: &mut Context<Self>) {
        if self.viewing.file.as_os_str().is_empty() {
            return;
        }
        self.viewing.at = None;
        self.look(0, false, cx);
    }

    fn inside_step(&mut self, by: i32, cx: &mut Context<Self>) {
        if self.viewing.file.as_os_str().is_empty() || self.viewing.found.is_empty() {
            return;
        }
        self.look(by, true, cx);
    }

    fn show_in_mods(&mut self, cx: &mut Context<Self>) {
        self.count = super::app(cx).find_show_in_mods();
        crate::ui::navigate::go(crate::ui::navigate::Go::SearchedInMods, cx);
    }

    fn use_recent(&mut self, at: usize, cx: &mut Context<Self>) {
        let Some(request) = super::app(cx).find_recall(at, self.selected) else {
            return;
        };
        let (text, types) = (request.text.clone(), request.types.clone());
        self.request = request;
        self.search
            .update(cx, |field, cx| field.set_text(&text, cx));
        self.types
            .update(cx, |field, cx| field.set_text(&types, cx));
        self.show_options(cx);
        self.begin(false, cx);
    }

    fn searched(&self) -> bool {
        self.running || self.said.is_some()
    }
}

fn said_inside(text: &str, found: usize, at: Option<usize>) -> Option<Sentence> {
    match (text.is_empty(), at) {
        (true, _) => None,
        (false, _) if found == 0 => Some(say!("No matches")),
        (false, Some(at)) => Some(say!("{} of {}", at + 1, found)),
        (false, None) => Some(say_n!(found, "{} match", "{} matches")),
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests;

impl crate::ui::section::Shown for Find {
    fn set_shown(&mut self, shown: bool, cx: &mut Context<Self>) {
        if let Some(missed) = self.unseen.shown(shown) {
            self.heard(missed, cx);
        }
    }
}
