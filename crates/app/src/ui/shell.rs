use super::Options;
use super::downloads::Downloads;
use super::navigate::Go;
use super::section::Section;
use super::theme::Theme;
use super::top_bar::top_bar;
use crate::changes::Changes;
use gpui::{
    AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement, ParentElement, Render,
    Styled, Window, div,
};

const READS: Changes = Changes::LOGS;

pub struct Shell {
    bar_keys: [gpui::FocusHandle; 2],
    pub(crate) demo: bool,
    pub(crate) sections: super::sections::Sections,
    pub(crate) tray_open: bool,
    pub(crate) tray_turn: usize,
    pub(crate) tray_fading: bool,
    /// The turn of the section shown last, which fades it in; none for the first.
    shown_turn: usize,
    /// The turning arrows beside the work under way, a view of its own so that only it is drawn
    /// again while it turns.
    pub(crate) spinner: Entity<super::status_bar::Spinner>,
    pub(crate) release: Option<(String, String)>,
    pub(crate) game: Entity<super::game::Game>,
    /// The search for the broken mod, as `bisect.json` keeps it between starts.
    pub(crate) hunt: crate::workspace::search::Hunt,
    pub(crate) hunt_busy: bool,
    pub(crate) lists: Entity<super::lists::Lists>,
    pub(crate) first_run: Option<Entity<super::first_run::FirstRun>>,
    pub(crate) commands: Option<Entity<super::commands::Commands>>,
    pub(crate) closing_commands: Option<Entity<super::commands::Commands>>,
    pub(crate) keys_open: bool,
    pub(crate) keys_focus: gpui::FocusHandle,
    pub(crate) before_play: bool,
    /// The status bar's note, drawn again alone as the hint changes.
    pub(crate) note: Entity<super::status_note::StatusNote>,
    prompt: Entity<super::prompt::Prompt>,
    menu: Entity<super::menu::Menu>,
    notices: Entity<super::notices::Notices>,
    drawn: bool,
}

impl Shell {
    pub fn new(options: &Options, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let demo = options.demo;
        let start = super::sections::Start {
            demo,
            part: options.part,
            view: options.view,
        };
        let sections = super::sections::Sections::new(start, cx);
        let mods = sections.mods.clone();
        let focus = mods.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        cx.subscribe_in(
            &super::navigate::router(cx),
            window,
            |shell, _, to: &Go, window, cx| shell.follow(to, window, cx),
        )
        .detach();
        let held_by = mods.downgrade();
        let game = cx.new(|cx| {
            super::game::Game::new(
                demo,
                options.still,
                move |cx| held_by.upgrade().is_some_and(|mods| mods.read(cx).held(cx)),
                cx,
            )
        });
        cx.subscribe(&game, |shell, _, told, cx| shell.heard_game(told, cx))
            .detach();
        let watched = game.downgrade();
        window
            .observe_window_visibility(move |visibility, _, cx| {
                let _ = watched.update(cx, |game, _| game.shown(visibility.is_visible()));
            })
            .detach();
        let section = options.section;
        let lists = cx.new(|cx| super::lists::Lists::new(demo, cx));
        let mut shell = Self {
            demo,
            sections,
            tray_open: false,
            tray_turn: 0,
            tray_fading: false,
            shown_turn: 0,
            spinner: cx.new(|_| super::status_bar::Spinner),
            release: None,
            game,
            lists,
            commands: None,
            closing_commands: None,
            first_run: None,
            keys_open: false,
            keys_focus: cx.focus_handle(),
            hunt: match demo {
                true => Default::default(),
                false => super::app(cx).hunt(),
            },
            hunt_busy: false,
            before_play: false,
            note: cx.new(|_| super::status_note::StatusNote { demo }),
            prompt: super::prompt::install(cx),
            menu: super::menu::install(cx),
            notices: super::notices::install(cx),
            drawn: false,
            bar_keys: [
                cx.focus_handle().tab_stop(true),
                cx.focus_handle().tab_stop(true),
            ],
        };
        if !demo {
            keep_logs(cx);
        }
        shell.go(section, cx);
        shell.shown_turn = 0;
        shell.tell_hunting(cx);
        let unset = !demo && !options.still && super::app(cx).settings().get("setupDone").is_none();
        if options.first || unset {
            shell.start_first_run(options.step, cx);
        }
        if !demo && !options.still {
            let left = shell.sections.mods.downgrade();
            window.on_window_should_close(cx, move |window, cx| {
                super::keep_window(window, cx);
                let _ = left.update(cx, |mods, cx| mods.keep_left_scroll(cx));
                super::app(cx).settle_writes();
                true
            });
            check_release(cx);
        }
        super::scenes::stage(&mut shell, options, window, cx);
        shell
    }

    #[cfg(test)]
    pub(crate) fn section(&self) -> Section {
        self.sections.shown()
    }

    pub(crate) fn downloads(&mut self, cx: &mut Context<Self>) -> Entity<Downloads> {
        self.sections.get(cx)
    }

    pub(crate) fn workshop(&mut self, cx: &mut Context<Self>) -> Entity<super::workshop::Workshop> {
        self.sections.get(cx)
    }

    pub(crate) fn find(&mut self, cx: &mut Context<Self>) -> Entity<super::find::Find> {
        self.sections.get(cx)
    }

    fn follow(&mut self, to: &Go, window: &mut Window, cx: &mut Context<Self>) {
        match to {
            Go::Problem(act, said) => {
                let act = *act;
                self.sections
                    .mods
                    .update(cx, |mods, cx| mods.act_on(act, said, cx));
            }
            Go::DownloadListed | Go::SubscribeListed | Go::InstallSteamcmd | Go::FetchNotes(_) => {
                let downloads = self.downloads(cx);
                downloads.update(cx, |downloads, cx| match to {
                    Go::DownloadListed => downloads.ask_download(window, cx),
                    Go::SubscribeListed => downloads.subscribe_wanted(cx),
                    Go::InstallSteamcmd => downloads.ask_install_steamcmd(window, cx),
                    Go::FetchNotes(id) => downloads.fetch_notes_of(*id, cx),
                    _ => {}
                });
            }
            Go::Section(section) => self.go(*section, cx),
            Go::ShowMod(mod_id) => {
                let mod_id = *mod_id;
                self.go(Section::Mods, cx);
                self.sections
                    .mods
                    .update(cx, |mods, cx| mods.show_mod(mod_id, cx));
            }
            Go::UseList(ids) => {
                let ids = ids.clone();
                self.sections
                    .mods
                    .update(cx, |mods, cx| mods.use_list(ids, cx));
                self.go(Section::Mods, cx);
            }
            Go::TakeList(ids) => {
                let ids = ids.clone();
                self.sections
                    .mods
                    .update(cx, |mods, cx| mods.use_list(ids, cx));
            }
            Go::Distrust(id) => {
                let id = *id;
                self.sections
                    .mods
                    .update(cx, |mods, cx| mods.trust(vec![id], false, cx));
            }
            Go::StartHints => self
                .sections
                .mods
                .update(cx, |mods, cx| mods.start_hints(cx)),
            Go::SearchedInMods => {
                self.sections
                    .mods
                    .update(cx, |mods, cx| mods.filter_by("in:search", cx));
                self.go(Section::Mods, cx);
            }
            Go::FindBroken => self.start_hunt(cx),
            Go::Browse(text) => {
                let (text, workshop) = (text.clone(), self.workshop(cx));
                workshop.update(cx, |workshop, cx| workshop.browse(text, cx));
            }
            Go::Subscribe(_)
            | Go::Redownload(_)
            | Go::Unsubscribe(_)
            | Go::UnsubscribeAll(_)
            | Go::UpdateFromGit(_) => {
                let downloads = self.downloads(cx);
                downloads.update(cx, |downloads, cx| match to {
                    Go::Subscribe(id) => downloads.subscribe(id.clone(), cx),
                    Go::Redownload(id) => downloads.redownload(*id, cx),
                    Go::Unsubscribe(id) => downloads.unsubscribe(*id, cx),
                    Go::UnsubscribeAll(ids) => downloads.unsubscribe_all(ids.clone(), cx),
                    Go::UpdateFromGit(id) => downloads.update_from_git(*id, cx),
                    _ => {}
                });
            }
        }
    }

    pub fn go(&mut self, section: Section, cx: &mut Context<Self>) {
        if section != self.sections.shown() {
            self.shown_turn = super::motion::opening();
        }
        self.sections.show(section, cx);
        cx.notify();
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(test)]
        RENDERS.with(|renders| renders.set(renders.get() + 1));
        if !self.drawn {
            self.drawn = true;
            window.on_next_frame(|_, _| {
                if let Some(started) = crate::STARTED.get() {
                    rimstack_core::note!("first_frame_ms={}", started.elapsed().as_millis());
                }
            });
        }
        let around = super::around::around(cx).read(cx);
        let in_the_way = (around.in_the_way.len(), around.errors);
        let (list_name, dirty) = (around.list_name.clone(), around.dirty);
        let theme = cx.global::<Theme>();
        let (win, ink, family, body) = (
            theme.colours.win,
            theme.colours.ink,
            theme.family.clone(),
            theme.body(),
        );
        if let Some(first) = self.first_run.clone() {
            let theme = cx.global::<Theme>();
            return div()
                .relative()
                .size_full()
                .bg(theme.colours.win)
                .text_color(theme.colours.ink)
                .font_family(theme.family.clone())
                .text_size(theme.body())
                .on_action(|_: &super::focus::Next, window, cx| window.focus_next(cx))
                .on_action(|_: &super::focus::Previous, window, cx| window.focus_prev(cx))
                .child(first)
                .child(self.notices.clone())
                .child(self.menu.clone())
                .child(self.prompt.clone());
        }
        super::keys::developer_tools(div())
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .on_action(|_: &super::focus::Next, window, cx| window.focus_next(cx))
            .on_action(|_: &super::focus::Previous, window, cx| window.focus_prev(cx))
            .on_action(cx.listener(|shell, _: &super::keys::ShowKeys, window, cx| {
                shell.open_keys(window, cx)
            }))
            .on_action(
                cx.listener(|shell, _: &super::keys::OpenCommands, window, cx| {
                    shell.open_commands(window, cx)
                }),
            )
            .on_action(
                cx.listener(|shell, _: &super::keys::Section1, _, cx| shell.go(Section::Mods, cx)),
            )
            .on_action(cx.listener(|shell, _: &super::keys::Section2, _, cx| {
                shell.go(Section::Downloads, cx)
            }))
            .on_action(cx.listener(|shell, _: &super::keys::Section3, _, cx| {
                shell.go(Section::Textures, cx)
            }))
            .on_action(cx.listener(|shell, _: &super::keys::Section4, _, cx| {
                shell.go(Section::Workshop, cx)
            }))
            .on_action(
                cx.listener(|shell, _: &super::keys::Section5, _, cx| shell.go(Section::Saves, cx)),
            )
            .on_action(
                cx.listener(|shell, _: &super::keys::Section6, _, cx| shell.go(Section::Logs, cx)),
            )
            .on_action(
                cx.listener(|shell, _: &super::keys::Section7, _, cx| shell.go(Section::Find, cx)),
            )
            .on_action(cx.listener(|shell, _: &super::keys::Section8, _, cx| {
                shell.go(Section::Settings, cx)
            }))
            .on_action(
                cx.listener(|shell, _: &super::find::FocusSearch, window, cx| {
                    shell.go(Section::Find, cx);
                    let find = shell.find(cx);
                    find.update(cx, |find, cx| find.focus_search(window, cx));
                }),
            )
            .capture_any_mouse_down(|_, _, cx| super::tip::forget_hint(cx))
            .capture_key_down(|_, _, cx| super::tip::forget_hint(cx))
            .child(
                gpui::canvas(
                    |_, _, _| (),
                    |_, _, window, _| {
                        window.on_mouse_event(|_: &gpui::ScrollEvent, phase, _, cx| {
                            if phase.capture() {
                                super::tip::forget_hint(cx);
                            }
                        })
                    },
                )
                .absolute()
                .size_0(),
            )
            .bg(win)
            .text_color(ink)
            .font_family(family)
            .text_size(body)
            .child(top_bar(
                super::top_bar::TopState {
                    section: self.sections.shown(),
                    demo: self.demo,
                    in_the_way,
                    list_name,
                    dirty,
                    behind: super::counts::read(cx).behind,
                    running: self.game.read(cx).running(),
                    lists_view: self.lists.clone(),
                    keys: self.bar_keys.clone(),
                },
                window,
                cx,
            ))
            .children(self.hunt_bar(cx))
            .child(self.shown_faded())
            .child(self.status_bar(cx))
            .children(self.before_play_popup(cx))
            .child(self.lists.clone())
            .children(self.keys_sheet(window, cx))
            .children(self.closing_commands.clone())
            .children(self.commands.clone())
            .child(self.notices.clone())
            .child(self.menu.clone())
            .child(self.prompt.clone())
    }
}

impl Shell {
    fn shown_faded(&self) -> gpui::AnyElement {
        let shown = div()
            .flex_1()
            .min_h_0()
            .children(self.sections.view_shown());
        match self.shown_turn {
            0 => shown.into_any_element(),
            turn => super::motion::appear(shown, ("appear", turn)).into_any_element(),
        }
    }
}

#[cfg(test)]
thread_local! {
    pub(crate) static RENDERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(all(test, target_os = "linux"))]
impl Shell {
    pub(crate) fn behind(&self, cx: &gpui::App) -> usize {
        super::counts::read(cx).behind
    }
}

fn keep_logs(cx: &mut Context<Shell>) {
    super::app(cx).keep_logs_soon();
    super::hear(cx, READS, |_: &mut Shell, _, cx| {
        super::app(cx).keep_logs_soon();
    });
}

fn check_release(cx: &mut Context<Shell>) {
    use super::notices;
    use crate::words::put;
    use rimstack_core::say;
    const SETTLED: std::time::Duration = std::time::Duration::from_secs(4);
    let app = super::app(cx);
    let self_check = app
        .settings()
        .switch(rimstack_core::kept::settings::Switch::SelfCheck);
    if !self_check {
        return;
    }
    cx.spawn(async move |shell, cx| {
        cx.background_executor().timer(SETTLED).await;
        let _ = shell.update(cx, |_, cx| {
            let nightly = app.settings().nightly_updates();
            crate::jobs::run(
                "release check",
                cx,
                move || crate::workspace::newest_release(nightly),
                |shell, found, cx| {
                    if let crate::reports::ReleaseSaid::Newer { version, page } = found {
                        shell.release = Some((version.clone(), page.clone()));
                        cx.notify();
                        let running = crate::workspace::build_label();
                        notices::offer(
                            put(&say!(
                                "RimStack {} is published; this is {}",
                                version,
                                running
                            )),
                            put(&say!("Open its page")),
                            move |_, cx| cx.open_url(&page),
                            cx,
                        );
                    }
                },
            );
        });
    })
    .detach();
}

impl Shell {
    fn start_first_run(&mut self, step: usize, cx: &mut Context<Self>) {
        let first = cx.new(|cx| super::first_run::FirstRun::new(step, cx));
        cx.subscribe(
            &first,
            |shell, _, started: &super::first_run::Started, cx| {
                shell.end_first_run(started.with_data, started.with_rimsort, cx)
            },
        )
        .detach();
        let list = self.sections.mods.read(cx).list.clone();
        cx.observe(&list, |shell, list, cx| {
            let Some(first) = shell.first_run.clone() else {
                return;
            };
            let (active, inactive, problem) = list.read(cx).read_as();
            first.update(cx, |first, cx| {
                first.set_mods(active, inactive, problem, cx)
            });
        })
        .detach();
        self.first_run = Some(first);
        cx.notify();
    }

    fn end_first_run(&mut self, with_data: bool, with_rimsort: bool, cx: &mut Context<Self>) {
        use super::notices;
        use crate::words::put;
        let app = super::app(cx);
        if let Err(problem) = app.keep_setting("setupDone", "yes") {
            notices::say(crate::words::said_of(&problem), notices::Tone::Err, cx);
        }
        let overview = self.sections.mods.read(cx).list.read(cx).overview();
        let missing = overview.is_none_or(|o| o.community_mods.is_none() || o.works_on == 0);
        if with_data && missing {
            let fetching = app.clone();
            crate::jobs::run(
                "community download",
                cx,
                move || fetching.download_databases(),
                |_, done, cx| {
                    if let Err(problem) = done {
                        notices::say(crate::words::said_of(&problem), notices::Tone::Err, cx);
                    }
                },
            );
        }
        if with_rimsort {
            let said = app.import_rimsort_rules();
            notices::say(put(&said), notices::Tone::Ok, cx);
        }
        self.first_run = None;
        self.sections
            .mods
            .update(cx, |mods, cx| mods.start_hints(cx));
        cx.notify();
    }
}
