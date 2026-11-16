use super::{Mods, Standing};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{Btn, Chip, TextSized, btn_width, icon, label, measure, notice};
use crate::words::{count, put};
use crate::workspace::load_order::Side;
use gpui::{
    Context, FontWeight, ParentElement, Pixels, Styled, Window, div, prelude::FluentBuilder, px,
};
use rimstack_core::{say, say_n};

impl Mods {
    pub(super) fn active_pane(
        &mut self,
        theme: &Theme,
        width: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let columns =
            super::columns::columns(theme, &self.orders, &self.view, Side::Active, width, window);
        let headings = self.heads(theme, Side::Active, &columns, window, cx);
        div()
            .flex()
            .flex_col()
            .h_full()
            .child(self.head(theme, width, window, cx))
            .children(self.notices(theme, width, window, cx))
            .children(self.bands(theme, cx))
            .child(
                div()
                    .flex_none()
                    .px(theme::GAP_LARGE)
                    .pb(theme::GAP)
                    .child(self.filter_field(
                        &self.filter.clone(),
                        "filter",
                        width - theme::GAP_LARGE * 2.,
                        theme,
                        window,
                        cx,
                    )),
            )
            .child(headings)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(crate::ui::cached(self.list(Side::Active).clone())),
            )
    }

    fn head(
        &self,
        theme: &Theme,
        width: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let c = &theme.colours;
        let list = self.list.read(cx);
        let (errors, warnings) = list.order.errors_and_warnings();
        let problems = list
            .order
            .troubles()
            .iter()
            .filter(|t| !t.is_empty())
            .count();
        let previewing = list.order.previewing();
        let (version_only, other_warnings) = list
            .overview
            .as_ref()
            .map_or((0, 0), |o| (o.version_only, o.other_warnings));
        let version_line = version_only >= 3 && !previewing && !self.demo;
        let warnings_shown = if version_line {
            other_warnings
        } else {
            warnings
        };
        let Standing {
            dirty,
            can_undo,
            can_redo,
            ..
        } = list.standing.clone();
        let title = put(&say!("Active"));
        let counted = count(list.count(Side::Active));
        let files_read = list.files_read;
        let sort = match (files_read, list.sort_pending) {
            (false, _) => put(&say!("Reading files…")),
            (true, 0) => put(&say!("Sort")),
            (true, n) => put(&say_n!(n, "Sort · {} move", "Sort · {} moves")),
        };
        let save = put(&say!("Save"));
        let full = |text: &str| {
            measure(window, theme, text, theme.small(), theme.medium)
                + theme.icon()
                + theme::GAP_SMALL
                + theme.sized(20.)
        };
        let head_wide = theme::GAP_CARD
            + theme::GAP_LARGE
            + measure(window, theme, &title, theme.heading(), FontWeight::BOLD)
            + measure(window, theme, &counted, theme.body(), FontWeight::NORMAL)
            + full(&sort)
            + full(&save)
            + theme::GAP * 4.;
        let warned = match version_line {
            true => put(&say_n!(
                warnings_shown,
                "{} more warning",
                "{} more warnings"
            )),
            false => put(&say_n!(warnings_shown, "{} warning", "{} warnings")),
        };
        let only_warnings = errors == 0;
        let problems_said = match only_warnings {
            true => put(&say_n!(problems, "{} warning", "{} warnings")),
            false => put(&say_n!(
                problems,
                "{} problem · {} with errors",
                "{} problems · {} with errors",
                count(errors)
            )),
        };
        let stepping = self.problem_place(cx);
        let problems_said = match stepping {
            Some(place) => put(&say!("Problem {} of {}", place, count(problems))),
            None => problems_said,
        };
        let problems_button = stepping.is_some()
            || !(only_warnings && warnings_shown > 0 && warnings_shown == problems);
        let chip_width = |text: &str| {
            theme.sized(11.)
                + theme::GAP_TINY
                + measure(window, theme, text, theme.caption(), theme.medium)
                + theme::GAP_CARD
        };
        let icon_button = theme.icon() + theme.sized(14.);
        let extras = if warnings_shown > 0 {
            chip_width(&warned) + theme::GAP
        } else {
            px(0.)
        } + if problems > 0 {
            let said = if problems_button {
                btn_width(window, theme, &problems_said, true, false) + theme::GAP_LINE
            } else {
                px(0.)
            };
            said + icon_button * 2. + theme::GAP_LINE + theme::GAP
        } else {
            px(0.)
        } + if self.demo {
            px(0.)
        } else {
            icon_button * 2. + theme::GAP_LINE + theme::GAP
        };
        let updates = put(&say_n!(
            crate::ui::counts::read(cx).behind,
            "{} update",
            "{} updates"
        ));
        let extras = extras
            + if crate::ui::counts::read(cx).behind > 0 {
                btn_width(window, theme, &updates, false, false) + theme::GAP
            } else {
                px(0.)
            };
        let tight = width < head_wide + extras;
        let problems_count = if problems > 0 {
            count(problems)
        } else {
            String::new()
        };
        let more_wide = btn_width(window, theme, &problems_count, true, false);
        let cramped = width < head_wide + more_wide + theme::GAP;
        let shown_text = |said: &str| match cramped {
            true => String::new(),
            false => said.to_string(),
        };
        let problem_tone = if errors > 0 { c.err } else { c.warn };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(theme::GAP)
            .h(theme.header())
            .pl(theme::GAP_CARD)
            .pr(theme::GAP_LARGE)
            .child(label(theme, theme.heading(), c.ink, title).font_weight(FontWeight::BOLD))
            .child(label(theme, theme.body(), c.ink3, counted))
            .when(warnings_shown > 0 && !tight, |d| {
                d.child(
                    Chip::new(warned)
                        .tone(c.warn)
                        .held(self.filtering.read(cx).terms().iter().any(|t| t == "is:warning"))
                        .icon("triangle-alert")
                        .tip(put(&say!("Mods with a warning: the game runs, but parts of these mods may not work. Click to show only these.")))
                        .on_click("warnings-chip", cx.listener(|m, _, _, cx| m.toggle_term("is:warning", cx))),
                )
            })
            .when(problems > 0 && !tight, |d| {
                d.child(
                    div()
                        .flex()
                        .gap(theme::GAP_LINE)
                        .when(problems_button, |d| {
                            d.child(super::hints::anchored_at(
                                Btn::new("problems", problems_said)
                                    .icon("triangle-alert")
                                    .tone(problem_tone)
                                    .tip(put(&say!(
                                        "Opens the list of every problem, with the fix for each"
                                    )))
                                    .on_click(cx.listener(|m, _, _, cx| m.open_problems(cx))),
                                &self.hints.anchors,
                    self.hints.anchors.problems.clone(),
                            ))
                        })
                        .child(
                            Btn::new("problem-up", "")
                                .icon("chevron-up")
                                .spoken(put(&say!("Previous problem")))
                                .tip(put(&say!("Previous problem")))
                                .on_click(cx.listener(|m, _, _, cx| m.show_problem(false, cx))),
                        )
                        .child(
                            Btn::new("problem-down", "")
                                .icon("chevron-down")
                                .spoken(put(&say!("Next problem")))
                                .tip(put(&say!("Next problem")))
                                .on_click(cx.listener(|m, _, _, cx| m.show_problem(true, cx))),
                        ),
                )
            })
            .child(div().flex_1())
            .when(crate::ui::counts::read(cx).behind > 0 && !tight, |d| {
                d.child(
                    Btn::new("updates", updates)
                        .tip(put(&say_n!(
                            crate::ui::counts::read(cx).behind,
                            "{} active Workshop mod is older than its Workshop version",
                            "{} active Workshop mods are older than their Workshop version"
                        )))
                        .on_click(cx.listener(|_, _, _, cx| crate::ui::navigate::go(crate::ui::navigate::Go::Section(crate::ui::section::Section::Downloads), cx))),
                )
            })
            .when(!self.demo && !tight, |d| {
                d.child(
                    div()
                        .flex()
                        .gap(theme::GAP_LINE)
                        .child(
                            Btn::new("undo", "")
                                .icon("undo-2")
                                .tip(put(&say!("Undo (Ctrl + Z)")))
                                .enabled(can_undo && !previewing)
                                .on_click(cx.listener(|m, _, _, cx| m.undo(cx))),
                        )
                        .child(
                            Btn::new("redo", "")
                                .icon("redo-2")
                                .tip(put(&say!("Redo (Ctrl + Shift + Z)")))
                                .enabled(can_redo && !previewing)
                                .on_click(cx.listener(|m, _, _, cx| m.redo(cx))),
                        ),
                )
            })
            .when(!self.demo, |d| {
                d.child(super::hints::anchored_at(
                    Btn::new("sort", shown_text(&sort))
                        .spoken(sort.clone())
                        .icon("arrow-up-down")
                        .tone(c.moved)
                        .tip(put(&say!("Shows the order the rules ask for, to keep or discard. Nothing is written before Save.")))
                        .enabled(!previewing && files_read)
                        .on_click(cx.listener(|m, _, _, cx| m.preview_sort(cx))),
                    &self.hints.anchors,
                    self.hints.anchors.sort.clone(),
                ))
                .child(super::hints::anchored_at(
                    Btn::new("save", shown_text(&save))
                        .spoken(save.clone())
                        .icon("save")
                        .tip(put(&say!("Writes the load order to ModsConfig.xml (Ctrl + S). The file as it was is copied beside it first.")))
                        .enabled(dirty)
                        .primary_if(dirty)
                        .on_click(cx.listener(|m, _, _, cx| m.save(None, cx))),
                    &self.hints.anchors,
                    self.hints.anchors.save.clone(),
                ))
            })
            .when(tight && !self.demo, |d| {
                let more = Btn::new(
                    "more",
                    if problems > 0 {
                        count(problems)
                    } else {
                        String::new()
                    },
                )
                .icon("ellipsis")
                .tip(put(&say!("Problems, Undo, Redo")))
                .opens({
                    let me = cx.entity().downgrade();
                    move |below, window, cx| {
                        let _ = me.update(cx, |mods, cx| mods.more_menu(below, window, cx));
                    }
                });
                d.child(if problems > 0 {
                    more.tone(problem_tone)
                } else {
                    more
                })
            })
    }

    fn notices(
        &self,
        theme: &Theme,
        width: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::Div> {
        let c = &theme.colours;
        let mut shown = Vec::new();
        if self.demo {
            return shown;
        }
        let list = self.list.read(cx);
        let version_only = list.overview.as_ref().map_or(0, |o| o.version_only);
        if version_only >= 3 && !list.order.previewing() {
            let version = list.game_version();
            let said = put(&say_n!(
                version_only,
                "{} mod does not list RimWorld {}",
                "{} mods do not list RimWorld {}",
                version.clone()
            ));
            let show = put(&say!("Show"));
            let ignore = put(&say!("Ignore for {}", version));
            let does = btn_width(window, theme, &show, false, false)
                + theme::GAP
                + btn_width(window, theme, &ignore, false, false);
            let room = width - theme::GAP_LARGE * 3. - theme::GAP_TINY;
            let stacked = theme.icon()
                + measure(window, theme, &said, theme.body(), FontWeight::NORMAL)
                + does
                + theme::GAP * 2.
                > room;
            let buttons = div()
                .flex()
                .flex_none()
                .justify_end()
                .gap(theme::GAP)
                .child(
                    Btn::new("show-warnings", show)
                        .tip(put(&say!("Only the mods with a warning")))
                        .on_click(cx.listener(|m, _, _, cx| m.toggle_term("is:warning", cx))),
                )
                .child(
                    Btn::new("ignore-version", ignore)
                        .tip(put(&say!("No warning about the game version while RimWorld stays on this version. The switch is in Settings, Sorting.")))
                        .on_click(cx.listener(move |m, _, _, cx| {
                            let version = m.list.read(cx).game_version();
                            m.set_version_aside(version, cx)
                        })),
                );
            shown.push(
                div().flex_none().px(theme::GAP_LARGE).pb(theme::GAP).child(
                    notice(c.warn).child(
                        div()
                            .flex()
                            .when(stacked, |d| d.flex_col().gap(theme::GAP_SMALL))
                            .when(!stacked, |d| d.items_center().gap(theme::GAP))
                            .pl(theme::GAP_LARGE)
                            .pr(theme::GAP_TINY)
                            .py(theme::GAP_SMALL)
                            .child(
                                div()
                                    .flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(theme::GAP)
                                    .child(icon("triangle-alert", theme.icon(), c.warn).mt(px(2.)))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .sized_text(theme, theme.body())
                                            .text_color(c.ink)
                                            .child(said),
                                    ),
                            )
                            .child(buttons),
                    ),
                ),
            );
        }
        shown
    }
}

impl Mods {
    fn more_menu(&self, below: gpui::Bounds<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        use crate::ui::menu::{self, Entry};
        let me = cx.entity().downgrade();
        let run = |act: fn(&mut Mods, &mut Context<Mods>)| {
            let me = me.clone();
            move |_: &mut Window, cx: &mut gpui::App| {
                let _ = me.update(cx, act);
            }
        };
        let mut entries: Vec<Entry> = Vec::new();
        let problems = self
            .list
            .read(cx)
            .order
            .troubles()
            .iter()
            .filter(|t| !t.is_empty())
            .count();
        if problems > 0 {
            entries.push(
                Entry::item(
                    put(&say!("Next problem")),
                    run(|m, cx| m.show_problem(true, cx)),
                )
                .icon("chevron-down")
                .into(),
            );
            entries.push(
                Entry::item(
                    put(&say!("Previous problem")),
                    run(|m, cx| m.show_problem(false, cx)),
                )
                .icon("chevron-up")
                .into(),
            );
            entries.push(
                Entry::item(put(&say!("All problems")), run(|m, cx| m.open_problems(cx)))
                    .icon("triangle-alert")
                    .into(),
            );
        }
        let list = self.list.read(cx);
        let (version_only, other) = list
            .overview
            .as_ref()
            .map_or((0, 0), |o| (o.version_only, o.other_warnings));
        let warnings = match version_only >= 3 && !list.order.previewing() && !self.demo {
            true => other,
            false => list.order.errors_and_warnings().1,
        };
        if warnings > 0 {
            entries.push(
                Entry::item(
                    put(&say_n!(warnings, "{} warning", "{} warnings")),
                    run(|m, cx| m.toggle_term("is:warning", cx)),
                )
                .icon("funnel")
                .into(),
            );
        }
        if self.reads.conflicts > 0 {
            entries.push(
                Entry::item(
                    put(&say!("{} share textures", count(self.reads.conflicts))),
                    run(|m, cx| m.toggle_term("is:conflict", cx)),
                )
                .icon("image")
                .into(),
            );
        }
        if crate::ui::counts::read(cx).behind > 0 {
            entries.push(
                Entry::item(
                    put(&say_n!(
                        crate::ui::counts::read(cx).behind,
                        "{} update",
                        "{} updates"
                    )),
                    run(|_, cx| {
                        crate::ui::navigate::go(
                            crate::ui::navigate::Go::Section(
                                crate::ui::section::Section::Downloads,
                            ),
                            cx,
                        )
                    }),
                )
                .icon("download")
                .into(),
            );
        }
        if !entries.is_empty() {
            entries.push(Entry::Line);
        }
        let list = self.list.read(cx);
        let (can_undo, can_redo) = (list.standing.can_undo, list.standing.can_redo);
        let previewing = list.order.previewing();
        entries.push(
            Entry::item(put(&say!("Undo (Ctrl + Z)")), run(|m, cx| m.undo(cx)))
                .icon("undo-2")
                .off(!can_undo || previewing)
                .into(),
        );
        entries.push(
            Entry::item(
                put(&say!("Redo (Ctrl + Shift + Z)")),
                run(|m, cx| m.redo(cx)),
            )
            .icon("redo-2")
            .off(!can_redo || previewing)
            .into(),
        );
        menu::open(below, entries, window, cx);
    }
}
