use super::button::Btn;
use super::field::Field;
use super::menu::{self, Entry};
use super::notices;
use super::prompt::{self, Line, Question};
use super::text_field::{Entered, TextField};
use super::theme::{self, Theme};
use super::widgets::{TextSized, label};
use crate::changes::Changes;
use crate::words::put;
use crate::workspace::lists::{HistoryLine, Snapshot};
use crate::workspace::logs::{clock, moment};
use gpui::{
    AnyElement, AppContext, Bounds, Context, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Pixels, Render, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use rimstack_core::base::error::Error;
use rimstack_core::kept::lists::Saved;
use rimstack_core::{say, say_n};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Which {
    Lists,
    History,
}

const READS: Changes = Changes::LISTS.or(Changes::LANGUAGE);

pub struct Lists {
    saved: Vec<Saved>,
    snapshots: Vec<Snapshot>,
    history: Vec<HistoryLine>,
    busy: bool,
    problem: String,
    clipboard_problem: String,
    name: Entity<TextField>,
    open: Option<(Which, Bounds<Pixels>)>,
    turn: usize,
    /// Closed, and drawn only while it fades out.
    fading: Option<(Which, Bounds<Pixels>)>,
    focus: FocusHandle,
    before: Option<FocusHandle>,
    reading: crate::jobs::Lane,
}

impl Lists {
    pub fn new(demo: bool, cx: &mut Context<Self>) -> Self {
        let theme = cx.global::<Theme>().clone();
        let name = cx.new(|cx| TextField::new(theme.body(), cx));
        cx.subscribe(&name, |lists, _, _: &Entered, cx| lists.keep_typed(cx))
            .detach();
        cx.observe(&name, |_, _, cx| cx.notify()).detach();
        let mut lists = Self {
            saved: Vec::new(),
            snapshots: Vec::new(),
            history: Vec::new(),
            busy: false,
            problem: String::new(),
            clipboard_problem: String::new(),
            name,
            open: None,
            turn: 0,
            fading: None,
            focus: cx.focus_handle(),
            before: None,
            reading: Default::default(),
        };
        if !demo {
            lists.hear(cx);
            lists.read(cx);
        }
        lists
    }

    fn hear(&self, cx: &mut Context<Self>) {
        super::hear(cx, READS, |lists: &mut Self, _, cx| lists.read(cx));
    }

    /// Every kept list's file is read and parsed: off the window's thread.
    fn read(&mut self, cx: &mut Context<Self>) {
        let app = super::app(cx);
        self.reading.run(
            "lists",
            cx,
            move |_| (app.saved_lists(), app.recent_snapshots(), app.history()),
            |lists, (saved, snapshots, history), cx| {
                (lists.saved, lists.snapshots, lists.history) = (saved, snapshots, history);
                cx.notify();
            },
        );
    }

    pub fn toggle(
        &mut self,
        which: Which,
        below: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.open.is_some_and(|(open, _)| open == which) {
            return self.close(window, cx);
        }
        self.problem.clear();
        self.clipboard_problem.clear();
        self.open = Some((which, below));
        self.fading = None;
        self.turn = super::motion::opening();
        self.before = window.focused(cx);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn drawn(&self) -> bool {
        self.open.is_some() || self.fading.is_some()
    }

    pub fn is_open(&self, which: Which) -> bool {
        self.open.is_some_and(|(open, _)| open == which)
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let closed = self.open.take();
        if super::motion::fades_out(cx) {
            self.fading = closed;
            let turn = self.turn;
            super::motion::after_fade(cx, move |lists: &mut Self, cx| {
                if lists.turn == turn && lists.fading.take().is_some() {
                    cx.notify();
                }
            });
        }
        if let Some(before) = self.before.take() {
            window.focus(&before, cx);
        }
        cx.notify();
    }

    fn list_name(&self, cx: &gpui::App) -> String {
        super::around::around(cx).read(cx).list_name.clone()
    }

    /// The load order now goes by `name`; "" when it has none.
    fn go_by(&self, name: &str, cx: &mut Context<Self>) {
        match super::app(cx).keep_pref("Mods", "listName", name) {
            Ok(()) => super::around::change(cx, |around| around.list_name = name.to_string()),
            Err(problem) => notices::say(crate::words::said_of(&problem), notices::Tone::Err, cx),
        }
        cx.notify();
    }

    fn use_ids(
        &mut self,
        ids: Vec<String>,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        super::navigate::go(super::navigate::Go::TakeList(ids), cx);
        self.go_by(name, cx);
        self.close(window, cx);
    }

    /// Kept off the window's thread; `typed`: the name field is emptied once it is.
    fn keep(&mut self, name: &str, typed: bool, cx: &mut Context<Self>) {
        let (app, name) = (super::app(cx), name.trim().to_string());
        let asked = name.clone();
        crate::jobs::run(
            "list kept",
            cx,
            move || app.keep_list(&asked, None),
            move |lists, kept, cx| {
                lists.problem = crate::words::why_not(&kept);
                if kept.is_ok() {
                    lists.go_by(&name, cx);
                    if typed {
                        lists.name.update(cx, |field, cx| field.set_text("", cx));
                    }
                }
                cx.notify();
            },
        );
    }

    fn keep_typed(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).text().trim().to_string();
        if !name.is_empty() {
            self.keep(&name, true, cx);
        }
    }

    fn start_fresh(&mut self, with_expansions: bool, window: &mut Window, cx: &mut Context<Self>) {
        let ids = super::app(cx).fresh_list(with_expansions);
        self.use_ids(ids, "", window, cx);
    }

    pub(crate) fn use_copied(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let text = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default();
        self.clipboard_problem.clear();
        self.busy = true;
        let app = super::app(cx);
        crate::jobs::run(
            "copied list",
            cx,
            move || app.ids_in_copied(&text),
            |lists, read: Result<Vec<String>, Error>, cx| {
                lists.busy = false;
                match read {
                    Ok(ids) if ids.is_empty() => {
                        lists.clipboard_problem = put(&say!("No mod found on the clipboard"))
                    }
                    Ok(ids) => {
                        super::navigate::go(super::navigate::Go::TakeList(ids), cx);
                        lists.go_by("", cx);
                        lists.open = None;
                    }
                    Err(problem) => lists.clipboard_problem = crate::words::said_of(&problem),
                }
                cx.notify();
            },
        );
        cx.notify();
    }

    fn share(&mut self, cx: &mut Context<Self>) {
        let listing = super::app(cx).listing(&super::order::read(cx).active);
        if self.busy {
            return;
        }
        if listing.is_empty() {
            self.clipboard_problem = put(&say!("There is no load order to share"));
            return cx.notify();
        }
        self.clipboard_problem.clear();
        self.busy = true;
        let app = super::app(cx);
        crate::jobs::run(
            "rentry publish",
            cx,
            move || app.publish_list(&listing),
            |lists, published, cx| {
                lists.busy = false;
                match published {
                    Ok(page) => {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(page.url.clone()));
                        let note = say!(
                            "The load order is at {}, and the link is copied. Its edit code, which RimStack does not keep: {}",
                            page.url,
                            page.edit_code
                        );
                        notices::say(put(&note), notices::Tone::Ok, cx);
                    }
                    Err(problem) => lists.clipboard_problem = crate::words::said_of(&problem),
                }
                cx.notify();
            },
        );
        cx.notify();
    }

    fn ask_share(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let me = cx.entity().downgrade();
        prompt::ask(
            Question {
                title: put(&say!("Share on Rentry.co")),
                lines: vec![Line::Text(put(&say!(
                    "The load order is published at a public link on Rentry.co that anyone who has it can open. RimStack does not keep the code that edits the page; the message that follows shows it."
                )))],
                label: put(&say!("Publish")),
                danger: false,
                run: Box::new(move |_, cx| {
                    let _ = me.update(cx, |lists, cx| lists.share(cx));
                }),
            },
            window,
            cx,
        );
    }

    fn revert(&mut self, line: &HistoryLine, window: &mut Window, cx: &mut Context<Self>) {
        let app = super::app(cx);
        if !line.restore.is_empty() {
            if let Some(ids) = app.snapshot_named(&line.restore) {
                super::navigate::go(super::navigate::Go::TakeList(ids), cx);
            }
        } else if !line.bring_back.is_empty() {
            let done = app.unhide_folder(std::path::Path::new(&line.bring_back));
            match done {
                Ok(()) => notices::say(put(&say!("Textures brought back")), notices::Tone::Ok, cx),
                Err(problem) => {
                    notices::say(crate::words::said_of(&problem), notices::Tone::Err, cx)
                }
            }
        }
        self.close(window, cx);
    }

    fn title(theme: &Theme, text: String) -> gpui::Div {
        label(theme, theme.title(), theme.colours.ink, text).font_weight(theme.semibold)
    }

    fn lists_popup(
        &self,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let c = &theme.colours;
        let current = self.list_name(cx);
        let row = |d: gpui::Div| {
            d.flex()
                .items_center()
                .gap(theme::GAP_SMALL)
                .h(theme.sized(28.))
        };
        let mut shown = vec![
            div()
                .flex()
                .items_center()
                .gap(theme::GAP_SMALL)
                .child(div().flex_1().min_w_0().child(Self::title(theme, put(&say!("Saved lists")))))
                .child(
                    Btn::new("new-list", put(&say!("New")))
                        .icon("plus")
                        .tip(put(&say!("Start a load order from the game alone or with its expansions; one Undo brings the old one back")))
                        .pops({
                            let me = cx.entity().downgrade();
                            move |below, window, cx| {
                                let (only, both) = (me.clone(), me.clone());
                                let entries: Vec<Entry> = vec![
                                    Entry::item(put(&say!("Only the game")), move |window, cx| {
                                        let _ = only.update(cx, |lists, cx| lists.start_fresh(false, window, cx));
                                    })
                                    .into(),
                                    Entry::item(put(&say!("The game and its expansions")), move |window, cx| {
                                        let _ = both.update(cx, |lists, cx| lists.start_fresh(true, window, cx));
                                    })
                                    .into(),
                                ];
                                menu::open(below, entries, window, cx);
                            }
                        }),
                )
                .into_any_element(),
        ];
        if self.saved.is_empty() {
            shown.push(label(theme, theme.body(), c.ink2, put(&say!("None"))).into_any_element());
        }
        for (at, saved) in self.saved.iter().enumerate() {
            let name = saved.name.clone();
            let (kept, used, deleted) = (name.clone(), name.clone(), name.clone());
            let update = Btn::new(
                ("update-list", at),
                if current == name {
                    put(&say!("Update"))
                } else {
                    String::new()
                },
            )
            .icon("save")
            .tip(put(&say!(
                "Saves the current load order as {}, replacing what it held",
                name.clone()
            )))
            .on_click(cx.listener(move |lists, _, _, cx| {
                lists.keep(&kept, false, cx);
            }));
            shown.push(
                row(div())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .sized_text(theme, theme.body())
                            .text_color(c.ink)
                            .child(name.clone()),
                    )
                    .child(label(
                        theme,
                        theme.small(),
                        c.ink3,
                        put(&say_n!(saved.mods, "{} mod", "{} mods")),
                    ))
                    .child(update)
                    .child(
                        Btn::new(("use-list", at), put(&say!("Use"))).on_click(cx.listener(
                            move |lists, _, window, cx| {
                                if let Some(ids) = super::app(cx).list_named(&used) {
                                    lists.use_ids(ids, &used, window, cx);
                                } else {
                                    lists.close(window, cx);
                                }
                            },
                        )),
                    )
                    .child(
                        Btn::new(("delete-list", at), "")
                            .icon("trash-2")
                            .tip(put(&say!("Delete {}", name)))
                            .on_click(move |_, _, cx| super::app(cx).delete_list(&deleted)),
                    )
                    .into_any_element(),
            );
        }
        shown.push(div().h(px(1.)).bg(c.line).into_any_element());
        let typed = !self.name.read(cx).text().trim().is_empty();
        shown.push(
            div()
                .flex()
                .items_center()
                .gap(theme::GAP_SMALL)
                .child(
                    div().flex_1().min_w_0().child(
                        Field {
                            id: "list-name",
                            text: &self.name,
                            icon: "",
                            placeholder: put(&say!("Name for the current list")),
                            found: None,
                            keys: None,
                        }
                        .render(theme, window, cx),
                    ),
                )
                .child(
                    Btn::new("keep-list", put(&say!("Keep")))
                        .icon("save")
                        .enabled(typed)
                        .on_click(cx.listener(|lists, _, _, cx| lists.keep_typed(cx))),
                )
                .into_any_element(),
        );
        let wrapped = |text: String, colour| {
            div()
                .sized_text(theme, theme.body())
                .text_color(colour)
                .child(text)
                .into_any_element()
        };
        if !self.problem.is_empty() {
            shown.push(wrapped(self.problem.clone(), c.err));
        }
        if !self.snapshots.is_empty() {
            shown.push(
                Self::title(theme, put(&say!("Earlier")))
                    .pt(theme::GAP_SMALL)
                    .into_any_element(),
            );
        }
        for (at, snapshot) in self.snapshots.iter().enumerate() {
            let name = snapshot.name.clone();
            shown.push(
                row(div())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .sized_text(theme, theme.body())
                            .text_color(c.ink2)
                            .child(put(&snapshot.title)),
                    )
                    .child(
                        Btn::new(("use-earlier", at), put(&say!("Use"))).on_click(cx.listener(
                            move |lists, _, window, cx| {
                                if let Some(ids) = super::app(cx).snapshot_named(&name) {
                                    lists.use_ids(ids, "", window, cx);
                                } else {
                                    lists.close(window, cx);
                                }
                            },
                        )),
                    )
                    .into_any_element(),
            );
        }
        shown.push(
            Self::title(theme, put(&say!("Clipboard and Rentry.co")))
                .pt(theme::GAP_SMALL)
                .into_any_element(),
        );
        shown.push(
            div()
                .flex()
                .flex_wrap()
                .gap(theme::GAP_SMALL)
                .child(
                    Btn::new("copy-order", put(&say!("Copy the load order")))
                        .icon("copy")
                        .tip(put(&say!("The active mods as text, one a line")))
                        .on_click(cx.listener(|lists, _, window, cx| {
                            let listing =
                                super::app(cx).listing(&super::order::read(cx).active);
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(listing));
                            lists.close(window, cx);
                        })),
                )
                .child(
                    Btn::new("use-copied", put(&say!("Use the copied list")))
                        .enabled(!self.busy)
                        .tip(put(&say!("A list of mods copied as text or as links, or a link to a Rentry.co page, becomes the load order. Save writes it.")))
                        .on_click(cx.listener(|lists, _, _, cx| lists.use_copied(cx))),
                )
                .child(
                    Btn::new("share-rentry", put(&say!("Share on Rentry.co")))
                        .icon("share-2")
                        .enabled(!self.busy)
                        .tip(put(&say!("Publishes the load order as a page at Rentry.co and copies its link")))
                        .on_click(cx.listener(|lists, _, window, cx| lists.ask_share(window, cx))),
                )
                .into_any_element(),
        );
        if self.busy {
            shown.push(wrapped(put(&say!("Waiting for Rentry.co…")), c.ink2));
        }
        if !self.clipboard_problem.is_empty() {
            shown.push(wrapped(self.clipboard_problem.clone(), c.err));
        }
        shown
    }

    fn history_popup(&self, theme: &Theme, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let c = &theme.colours;
        let mut shown =
            vec![Self::title(theme, put(&say!("What RimStack changed"))).into_any_element()];
        if self.history.is_empty() {
            shown.push(
                label(theme, theme.body(), c.ink2, put(&say!("Nothing yet"))).into_any_element(),
            );
        }
        for (at, line) in self.history.iter().enumerate() {
            let said = put(&line.said);
            let restores = !line.restore.is_empty();
            let reverts = restores || !line.bring_back.is_empty();
            let held = line.clone();
            shown.push(
                div()
                    .flex()
                    .items_start()
                    .gap(theme::GAP)
                    .child(label(theme, theme.small(), c.ink3, moment(line.time)).w(theme.sized(112.)))
                    .child(div().flex_1().min_w_0().sized_text(theme, theme.body()).text_color(c.ink).child(said.clone()))
                    .when(reverts, |d| {
                        d.child(
                            Btn::new(("revert", at), match restores {
                                true => put(&say!("Restore")),
                                false => put(&say!("Bring back")),
                            })
                            .spoken(put(&say!("Restore the list from before: {}", said)))
                            .tip(match restores {
                                true => put(&say!("The load order as it was just before. Nothing is written before Save.")),
                                false => put(&say!("Renames the files back inside the mod")),
                            })
                            .on_click(cx.listener(move |lists, _, window, cx| lists.revert(&held, window, cx))),
                        )
                    })
                    .into_any_element(),
            );
        }
        let earlier = notices::earlier(cx);
        if !earlier.is_empty() {
            shown.push(
                Self::title(theme, put(&say!("Since RimStack started")))
                    .pt(theme::GAP)
                    .into_any_element(),
            );
        }
        for (text, err, time) in earlier {
            shown.push(
                div()
                    .flex()
                    .items_start()
                    .gap(theme::GAP)
                    .child(label(theme, theme.small(), c.ink3, clock(time)).w(theme.sized(92.)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .sized_text(theme, theme.body())
                            .text_color(if err { c.err } else { c.ink2 })
                            .child(text),
                    )
                    .into_any_element(),
            );
        }
        shown
    }
}

impl Render for Lists {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some((which, below)) = self.open.or(self.fading) else {
            return div().id("lists-closed");
        };
        let fading = self.open.is_none();
        let theme = cx.global::<Theme>().clone();
        let c = theme.colours;
        let viewport = window.viewport_size();
        let content = match which {
            Which::Lists => self.lists_popup(&theme, window, cx),
            Which::History => self.history_popup(&theme, cx),
        };
        let column = div()
            .flex()
            .flex_col()
            .gap(theme::GAP_SMALL)
            .children(content);
        let popup = div()
            .id("lists-popup")
            .absolute()
            .p(theme::GAP_LARGE)
            .rounded(theme::CORNER_LARGE)
            .bg(c.bar)
            .border_1()
            .border_color(c.line)
            .when(!fading, |d| d.occlude())
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation());
        let popup = match which {
            Which::Lists => popup
                .left(below.left())
                .top(below.bottom() + px(6.))
                .w(theme.sized(380.))
                .child(column),
            Which::History => popup
                .right(viewport.width - below.right())
                .bottom(viewport.height - below.top() + px(8.))
                .w(theme.sized(440.))
                .child(
                    div()
                        .id("history-lines")
                        .max_h(
                            theme.sized(520.).min(below.top() - theme::GAP_LARGE)
                                - theme::GAP_LARGE * 2.,
                        )
                        .overflow_y_scroll()
                        .child(column),
                ),
        };
        if fading {
            return div()
                .id("lists-scrim")
                .absolute()
                .inset_0()
                .child(super::motion::pop_out(
                    popup.child(super::motion::shield()),
                    self.turn,
                ));
        }
        div()
            .id("lists-scrim")
            .track_focus(&self.focus)
            .absolute()
            .inset_0()
            .on_key_down(cx.listener(|lists, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    lists.close(window, cx);
                }
            }))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|lists, _, window, cx| lists.close(window, cx)),
            )
            .child(super::motion::pop_in(popup, self.turn))
    }
}
