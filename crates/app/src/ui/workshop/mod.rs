mod details;
mod screen;
mod side;

use super::notices::{self, Tone};
use super::rows::Scrolling;
use super::text_field::{Entered, TextField};
use super::theme::Theme;
use crate::changes::Changes;
use crate::jobs::Lane;
use crate::words::put;
use crate::workspace::browse::{
    Asked, Followed, Related, Results, Row, Shown, Sidebar, Sort, SortOffer, chosen, effective,
    follow, offers, sort_of,
};
use gpui::{AppContext, Context, Entity, FocusHandle};
use rimstack_core::base::error::Error;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::rimworld::workshopsearch::Query;
use rimstack_core::say;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

const SCREENSHOTS: usize = 8;

#[derive(Clone)]
enum View {
    Search(Asked),
    Related(Related, String),
}

type Found = Result<(Results, String), Error>;

#[derive(Default)]
struct Opened {
    shown: Option<Rc<Shown>>,
    busy: bool,
    preview: (String, Option<PathBuf>),
    shots: (Vec<String>, Vec<PathBuf>),
    images: HashMap<String, String>,
}

const READS: Changes = Changes::COMMUNITY
    .or(Changes::SOURCE)
    .or(Changes::KEY)
    .or(Changes::LANGUAGE)
    .or(Changes::DOWNLOAD_LIST)
    .or(Changes::LIBRARY)
    .or(Changes::SETTINGS);

pub struct Workshop {
    unseen: crate::ui::section::Unseen,
    demo: bool,
    list_view: Entity<super::downloads::list::DownloadList>,
    field: Entity<TextField>,
    total: usize,
    more: bool,
    searching: bool,
    problem: String,
    ready: bool,
    key_mode: bool,
    offered: Vec<Sort>,
    wanted_sort: String,
    sorts: Vec<SortOffer>,
    sort_label: String,
    sidebar: Rc<Sidebar>,
    collections: bool,
    notice: Option<Sentence>,
    asked: Option<Asked>,
    related: Option<(Related, String, String)>,
    search_loaded: usize,
    reached: usize,
    rows: Rc<Vec<Row>>,
    previews: HashMap<String, PathBuf>,
    stars: HashMap<String, Vec<bool>>,
    settled: HashSet<String>,
    selected: String,
    opened: Opened,
    filters: bool,
    list: bool,
    enlarged: Option<PathBuf>,
    /// The opened item's pictures, let go once another is opened; the one enlarged, once closed.
    pictures: crate::ui::pictures::Pictures,
    enlarged_pictures: crate::ui::pictures::Pictures,
    thumbs: crate::ui::thumbs::Shared,
    community_busy: bool,
    seeking: Lane,
    previewing: Lane,
    opening: Lane,
    detail_previewing: Lane,
    shooting: Lane,
    imaging: Lane,
    badging: Lane,
    defaulting: Lane,
    scroll: Scrolling,
    side_paging: super::page::Paging,
    details_paging: super::page::Paging,
    focus: FocusHandle,
    description_picks: std::cell::RefCell<Vec<super::rich::Selection>>,
}

impl Workshop {
    pub fn new(demo: bool, cx: &mut Context<Self>) -> Self {
        let theme = cx.global::<Theme>().clone();
        let reduce = theme.appearance.reduce_motion;
        let field = cx.new(|cx| TextField::new(theme.body(), cx));
        cx.subscribe(&field, |workshop, field, _: &Entered, cx| {
            let text = field.read(cx).text().to_string();
            workshop.search(text, cx);
        })
        .detach();
        let scroll = Scrolling::default();
        scroll.borrow_mut().set_reduce_motion(reduce);
        let mut workshop = Self {
            unseen: Default::default(),
            demo,
            list_view: cx.new(|cx| super::downloads::list::DownloadList::new(true, cx)),
            field,
            total: 0,
            more: false,
            searching: false,
            problem: String::new(),
            ready: false,
            key_mode: false,
            offered: Vec::new(),
            wanted_sort: String::new(),
            sorts: Vec::new(),
            sort_label: String::new(),
            sidebar: Rc::new(Sidebar::empty()),
            collections: false,
            notice: None,
            asked: None,
            related: None,
            search_loaded: 0,
            reached: 0,
            rows: Rc::default(),
            previews: HashMap::new(),
            stars: HashMap::new(),
            settled: HashSet::new(),
            selected: String::new(),
            opened: Opened::default(),
            description_picks: Default::default(),
            filters: false,
            list: false,
            enlarged: None,
            pictures: Default::default(),
            enlarged_pictures: Default::default(),
            thumbs: Default::default(),
            community_busy: false,
            seeking: Lane::default(),
            previewing: Lane::default(),
            opening: Lane::default(),
            detail_previewing: Lane::default(),
            shooting: Lane::default(),
            imaging: Lane::default(),
            badging: Lane::default(),
            defaulting: Lane::default(),
            scroll,
            side_paging: super::page::Paging::new(reduce),
            details_paging: super::page::Paging::new(reduce),
            focus: cx.focus_handle(),
        };
        if !demo {
            workshop.hear(cx);
            workshop.heard(Changes::COMMUNITY | Changes::DOWNLOAD_LIST, cx);
            workshop.open(cx);
        }
        workshop
    }

    fn hear(&self, cx: &mut Context<Self>) {
        super::hear(cx, READS, Self::heard);
    }

    fn heard(&mut self, changes: Changes, cx: &mut Context<Self>) {
        let Some(changes) = self.unseen.pass(changes) else {
            return;
        };
        if changes.contains(Changes::COMMUNITY | Changes::SOURCE | Changes::KEY) {
            self.read_defaults(cx);
        }
        if changes.contains(Changes::KEY) {
            self.note_refusal(cx);
        }
        if changes.contains(Changes::LANGUAGE) {
            self.settle_sort();
        }
        if changes.contains(Changes::COMMUNITY | Changes::SOURCE | Changes::LANGUAGE) {
            self.reload(cx);
        } else if changes.contains(Changes::DOWNLOAD_LIST | Changes::LIBRARY) {
            self.rebadge(cx);
        }
        let details = Changes::DOWNLOAD_LIST
            | Changes::LIBRARY
            | Changes::COMMUNITY
            | Changes::SETTINGS
            | Changes::SOURCE
            | Changes::LANGUAGE;
        if changes.contains(details) && !self.selected.is_empty() {
            let id = self.selected.clone();
            self.open_details(id, cx);
        }
        cx.notify();
    }

    fn read_defaults(&mut self, cx: &mut Context<Self>) {
        let app = super::app(cx);
        self.defaulting.run(
            "workshop defaults",
            cx,
            move |_| {
                let defaults = app.browse_defaults();
                app.sweep_if_due();
                defaults
            },
            |workshop, defaults, cx| {
                workshop.ready = defaults.ready;
                workshop.offered = defaults.sorts;
                workshop.key_mode = defaults.key_mode;
                if !defaults.key_mode {
                    workshop.collections = false;
                }
                workshop.settle_sort();
                cx.notify();
            },
        );
    }

    fn note_refusal(&mut self, cx: &mut Context<Self>) {
        let app = super::app(cx);
        crate::jobs::run(
            "steam key state for browse",
            cx,
            move || app.key_state(),
            |workshop, state, cx| {
                if let Some(note) = state.note.filter(|_| state.refused) {
                    workshop.notice = Some(note);
                    cx.notify();
                }
            },
        );
    }

    fn words(&self) -> bool {
        let text = self.asked.as_ref().map_or("", |asked| asked.text.as_str());
        !Query::parse(text).words.is_empty()
    }

    fn settle_sort(&mut self) {
        let words = self.words();
        let offered = offers(&self.offered, words, self.key_mode);
        let id = effective(&self.wanted_sort, &offered, words);
        self.sort_label = offered
            .iter()
            .find(|offer| offer.id == id)
            .map(|offer| put(&offer.label))
            .unwrap_or_default();
        self.sorts = chosen(&offered, id);
        let collections = self.collections;
        if let Some(asked) = self.asked.as_mut() {
            asked.sort = sort_of(id);
            asked.collections = collections;
        }
    }

    fn current_view(&self) -> Option<View> {
        match &self.related {
            Some((which, id, _)) => Some(View::Related(*which, id.clone())),
            None => self.asked.clone().map(View::Search),
        }
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.current_view() else {
            return;
        };
        let count = self.rows.len().max(super::app(cx).browse_page());
        self.load(view, 0, count, false, cx);
    }

    fn rebadge(&mut self, cx: &mut Context<Self>) {
        let app = super::app(cx);
        self.badging.run(
            "workshop badges",
            cx,
            move |_| app.marks(),
            |workshop, marks, cx| {
                let rows = Rc::make_mut(&mut workshop.rows);
                rows.iter_mut().for_each(|row| marks.mark(row));
                cx.notify();
            },
        );
    }

    fn load(
        &mut self,
        view: View,
        offset: usize,
        count: usize,
        append: bool,
        cx: &mut Context<Self>,
    ) {
        self.searching = true;
        let app = super::app(cx);
        self.seeking.run(
            "workshop results",
            cx,
            move |_| -> Found {
                match view {
                    View::Search(asked) => app
                        .browse_search(&asked, offset, count)
                        .map(|results| (results, String::new())),
                    View::Related(which, id) => app
                        .browse_related(&id, which)
                        .map(|results| (results, app.browse_title(&id))),
                }
            },
            move |workshop, found, cx| workshop.arrived(found, append, offset + count, cx),
        );
        cx.notify();
    }

    fn arrived(&mut self, found: Found, append: bool, reached: usize, cx: &mut Context<Self>) {
        match found {
            Ok((mut results, title)) => {
                self.problem.clear();
                if let Some(sidebar) = results.sidebar.take() {
                    self.sidebar = Rc::new(sidebar);
                }
                if let Some(notice) = results.notice.take() {
                    self.notice = Some(notice);
                    self.read_defaults(cx);
                }
                (self.total, self.more, self.reached) = (results.total, results.more, reached);
                let rows = Rc::make_mut(&mut self.rows);
                if append {
                    let present: HashSet<String> = rows.iter().map(|r| r.id.clone()).collect();
                    rows.extend(
                        results
                            .rows
                            .into_iter()
                            .filter(|r| !present.contains(&r.id)),
                    );
                } else {
                    *rows = results.rows;
                }
                if let Some((_, _, shown)) = self.related.as_mut() {
                    *shown = title;
                }
            }
            Err(problem) => {
                self.rows = Rc::default();
                self.problem = crate::words::said_of(&problem);
                (self.total, self.more) = (0, false);
            }
        }
        if !append {
            self.scroll.borrow_mut().set_top(gpui::px(0.));
        }
        self.searching = false;
        self.fetch_previews(cx);
        cx.notify();
    }

    fn fetch_previews(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<String> = self
            .rows
            .iter()
            .filter(|row| !self.settled.contains(&row.id))
            .map(|row| row.id.clone())
            .collect();
        if ids.is_empty() {
            return;
        }
        let app = super::app(cx);
        let (tell, mut told) = futures::channel::mpsc::unbounded();
        self.previewing.run(
            "workshop previews",
            cx,
            move |turn| {
                app.browse_previews(&ids, &|id, path, stars| {
                    turn.current() && tell.unbounded_send((id.to_string(), path, stars)).is_ok()
                });
            },
            |_, (), _| {},
        );
        cx.spawn(async move |workshop, cx| {
            use futures::StreamExt;
            while let Some((id, path, stars)) = told.next().await {
                let going = workshop.update(cx, |workshop, cx| {
                    workshop.preview_came(id, path, stars);
                    cx.notify();
                });
                if going.is_err() {
                    return;
                }
            }
        })
        .detach();
    }

    fn preview_came(&mut self, id: String, path: Option<PathBuf>, stars: Vec<bool>) {
        self.settled.insert(id.clone());
        if !stars.is_empty() {
            self.stars.insert(id.clone(), stars);
        }
        if let Some(path) = path {
            self.previews.insert(id, path);
        }
    }

    fn ask(&mut self, change: impl FnOnce(&mut Asked), cx: &mut Context<Self>) {
        let mut asked = self.asked.clone().unwrap_or_default();
        change(&mut asked);
        let text = asked.text.clone();
        if self.field.read(cx).text() != text {
            self.field.update(cx, |field, cx| field.set_text(&text, cx));
        }
        self.notice = None;
        self.asked = Some(asked);
        self.related = None;
        self.settle_sort();
        let app = super::app(cx);
        if app.browse_opened() {
            self.read_defaults(cx);
        }
        let asked = self.asked.clone().unwrap_or_default();
        self.load(View::Search(asked), 0, app.browse_page(), false, cx);
    }

    pub fn search(&mut self, text: String, cx: &mut Context<Self>) {
        self.ask(|asked| asked.text = text, cx);
    }

    pub fn browse(&mut self, text: String, cx: &mut Context<Self>) {
        self.select(String::new(), cx);
        self.search(text, cx);
    }

    fn open(&mut self, cx: &mut Context<Self>) {
        if self.asked.is_none() {
            self.ask(|_| {}, cx);
        }
    }

    fn toggle_tag(&mut self, id: String, include: bool, cx: &mut Context<Self>) {
        self.ask(|asked| asked.picked.tag(&id, include), cx);
    }

    fn toggle_dlc(&mut self, id: String, include: bool, cx: &mut Context<Self>) {
        self.ask(|asked| asked.picked.dlc(&id, include), cx);
    }

    fn clear_filter(&mut self, id: String, cx: &mut Context<Self>) {
        self.ask(|asked| asked.picked.clear(&id), cx);
    }

    fn clear_all(&mut self, cx: &mut Context<Self>) {
        self.ask(|asked| asked.picked = Default::default(), cx);
    }

    fn choose_sort(&mut self, id: &str, cx: &mut Context<Self>) {
        self.wanted_sort = id.to_string();
        self.ask(|_| {}, cx);
    }

    fn show_collections(&mut self, on: bool, cx: &mut Context<Self>) {
        self.collections = on;
        let Some(mut asked) = self.asked.clone() else {
            return;
        };
        asked.collections = on;
        self.asked = Some(asked.clone());
        self.related = None;
        let page = super::app(cx).browse_page();
        self.load(View::Search(asked), 0, page, false, cx);
    }

    fn want_more(&mut self, cx: &mut Context<Self>) {
        let Some(asked) = self
            .asked
            .clone()
            .filter(|_| self.more && !self.searching && self.related.is_none())
        else {
            return;
        };
        let (offset, page) = (self.reached, super::app(cx).browse_page());
        self.load(View::Search(asked), offset, page, true, cx);
    }

    fn show_related(&mut self, id: String, which: Related, cx: &mut Context<Self>) {
        if self.related.is_none() {
            self.search_loaded = self.rows.len();
        }
        self.related = Some((which, id.clone(), String::new()));
        let page = super::app(cx).browse_page();
        self.load(View::Related(which, id), 0, page, false, cx);
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        if self.related.take().is_none() {
            return;
        }
        let count = self.search_loaded.max(super::app(cx).browse_page());
        let asked = self.asked.get_or_insert_with(Asked::default).clone();
        self.load(View::Search(asked), 0, count, false, cx);
    }

    pub fn select(&mut self, id: String, cx: &mut Context<Self>) {
        self.selected = id.clone();
        self.enlarged = None;
        if id.is_empty() {
            self.opened.shown = None;
            self.opened.busy = false;
            self.opening.cancel();
            self.imaging.cancel();
            self.detail_previewing.cancel();
        } else {
            self.opened.busy = true;
            self.open_details(id, cx);
        }
        cx.notify();
    }

    /// A link in a description: an item of the Workshop is chosen here, a page opens outside.
    fn followed(&mut self, link: &str, cx: &mut Context<Self>) {
        match follow(link) {
            Followed::Item(id) => self.select(id, cx),
            Followed::Outside(url) => cx.open_url(&url),
            Followed::Nothing => {}
        }
    }

    pub fn add(&mut self, id: String, with_requirements: bool, cx: &mut Context<Self>) {
        let app = super::app(cx);
        crate::jobs::run(
            "add to the download list",
            cx,
            move || app.add_to_list(&id, with_requirements).note(),
            |_, note, cx| tell(note, cx),
        );
    }

    fn row_of(&self, row: &Row) -> Row {
        Row {
            stars: self.stars.get(&row.id).cloned().unwrap_or_default(),
            ..row.clone()
        }
    }

    pub fn show_asked(&mut self, asked: &str, cx: &mut Context<Self>) {
        match asked {
            "list" => self.list = true,
            "filters" => self.filters = true,
            "details" => {
                let id = "3100000002".to_string();
                self.add(id.clone(), true, cx);
                self.select(id, cx);
            }
            _ => {}
        }
        cx.notify();
    }

    fn opened(&self) -> bool {
        self.opened.shown.is_some()
    }

    fn found(&self) -> bool {
        self.ready && self.problem.is_empty()
    }
}

fn tell(note: Option<(Sentence, bool)>, cx: &mut gpui::App) {
    if let Some((said, failed)) = note {
        let tone = if failed { Tone::Err } else { Tone::Ok };
        notices::say(put(&said), tone, cx);
    }
}

fn related_said(which: Related, id: &str, title: &str) -> Sentence {
    let name = if title.is_empty() { id } else { title };
    match which {
        Related::Requires => say!("Requirements of {}", name),
        Related::NeededBy => say!("Mods that need {}", name),
        Related::SameAuthor => say!("More by the author of {}", name),
        Related::Collection => say!("In the collection {}", name),
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests;

impl crate::ui::section::Shown for Workshop {
    fn set_shown(&mut self, shown: bool, cx: &mut Context<Self>) {
        if let Some(missed) = self.unseen.shown(shown) {
            self.heard(missed, cx);
        }
    }
}
