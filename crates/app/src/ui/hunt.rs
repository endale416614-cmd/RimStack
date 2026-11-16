//! The search for the mod that breaks the game, by halves. A round's list is only
//! put on the Mods tab; the game gets it on Save.

use super::button::Btn;
use super::shell::Shell;
use super::theme::{self, Theme};
use super::widgets::{TextSized, icon};
use crate::words::put;
use gpui::{
    AnyElement, Context, FontWeight, HighlightStyle, InteractiveElement, IntoElement,
    ParentElement, Styled, StyledText, div, prelude::FluentBuilder, px,
};
use rimstack_core::{say, say_n};

impl Shell {
    fn try_round(&mut self, cx: &mut Context<Self>) {
        self.tell_hunting(cx);
        if self.hunt.asking {
            let ids = self.hunt.list.clone();
            self.sections
                .mods
                .update(cx, |mods, cx| mods.use_list_quietly(ids, cx));
        }
        cx.notify();
    }

    pub(crate) fn start_hunt(&mut self, cx: &mut Context<Self>) {
        if self.demo || self.hunt.active || self.hunt_busy {
            return;
        }
        self.before_play = false;
        self.go(crate::ui::section::Section::Mods, cx);
        let app = super::app(cx);
        self.hunt_busy = true;
        crate::jobs::run(
            "search started",
            cx,
            move || app.start_hunt(true),
            |shell, hunt, cx| {
                shell.hunt_busy = false;
                shell.hunt = hunt;
                shell.try_round(cx);
            },
        );
    }

    pub(crate) fn answer_hunt(&mut self, broken: bool, cx: &mut Context<Self>) {
        if self.hunt_busy {
            return;
        }
        let app = super::app(cx);
        self.hunt_busy = true;
        crate::jobs::run(
            "search answered",
            cx,
            move || app.answer_hunt(broken),
            |shell, hunt, cx| {
                shell.hunt_busy = false;
                shell.hunt = hunt;
                shell.try_round(cx);
            },
        );
    }

    pub(crate) fn end_hunt(&mut self, without_found: bool, cx: &mut Context<Self>) {
        if self.hunt_busy {
            return;
        }
        let app = super::app(cx);
        self.hunt_busy = true;
        crate::jobs::run(
            "search ended",
            cx,
            move || app.end_hunt(without_found),
            |shell, ids, cx| {
                shell.hunt_busy = false;
                shell.hunt = Default::default();
                shell.tell_hunting(cx);
                shell
                    .sections
                    .mods
                    .update(cx, |mods, cx| mods.use_list_quietly(ids, cx));
                cx.notify();
            },
        );
    }

    pub(crate) fn tell_hunting(&mut self, cx: &mut Context<Self>) {
        let active = self.hunt.active;
        self.sections
            .mods
            .update(cx, |mods, cx| mods.set_hunting(active, cx));
        if let Some(logs) = self.sections.made::<super::logs::Logs>() {
            logs.update(cx, |logs, cx| logs.set_hunting(active, cx));
        }
    }

    fn save_and_play_round(&mut self, cx: &mut Context<Self>) {
        let game = self.game.downgrade();
        self.sections.mods.update(cx, |mods, cx| {
            mods.save(
                Some(Box::new(move |_, cx| {
                    let _ = game.update(cx, |game, cx| game.play(true, cx));
                })),
                cx,
            )
        });
    }

    pub(crate) fn hunt_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let hunt = &self.hunt;
        if !hunt.active {
            return None;
        }
        let theme = cx.global::<Theme>().clone();
        let c = theme.colours;
        let bold = HighlightStyle {
            font_weight: Some(FontWeight::BOLD),
            ..HighlightStyle::default()
        };
        let (text, marks) = if !hunt.found.is_empty() {
            let said = put(&say!(
                "{} is the mod that breaks the game.",
                hunt.found.clone()
            ));
            let at = said.find(&hunt.found).unwrap_or(0);
            let range = at..at + hunt.found.len();
            (said, vec![(range, bold)])
        } else if !hunt.stuck.is_empty() {
            let names = hunt.stuck.join(", ");
            let said = put(&say!(
                "One of these, which need each other: {}",
                names.clone()
            ));
            let at = said.find(&names).unwrap_or(0);
            let range = at..at + names.len();
            (said, vec![(range, bold)])
        } else {
            let head = put(&say!("Finding the broken mod."));
            let rest = put(&say_n!(
                hunt.rounds as usize,
                "About {} round left, {} suspects.",
                "About {} rounds left, {} suspects.",
                hunt.suspects
            ));
            let then = put(&say!("Save, play, then say how it went."));
            let range = 0..head.len();
            (format!("{head} {rest} {then}"), vec![(range, bold)])
        };
        let dirty = super::around::around(cx).read(cx).dirty;
        let asking = hunt.asking;
        let found = !hunt.found.is_empty();
        Some(
            div()
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .gap(theme::GAP)
                .min_h(theme.sized(44.))
                .py(theme::GAP_MEDIUM)
                .pl(theme::GAP_CARD)
                .pr(theme::GAP_LARGE)
                .bg(c.banner)
                .child(icon("search", theme.icon(), c.moved))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .sized_text(&theme, theme.body())
                        .text_color(c.ink)
                        .child(StyledText::new(text).with_highlights(marks)),
                )
                .when(asking, |d| {
                    let info = div().id("hunt-how").child(icon("info", theme.icon(), c.ink2));
                    d.child(super::tip::tipped(
                        info,
                        put(&say!("Half of the suspects are deactivated in the list now. Do not load or save a colony during a round; the main menu or a new test colony shows whether the game breaks.")).into(),
                    ))
                    .child(
                        Btn::new("hunt-play", put(&say!("Save and play")))
                            .icon("play")
                            .primary_if(dirty)
                            .tip(put(&say!("Writes this round's list to ModsConfig.xml and starts the game. The newest save was copied to RimStack's data folder, save-before-search, when the search began.")))
                            .on_click(cx.listener(|shell, _, _, cx| shell.save_and_play_round(cx))),
                    )
                    .child(
                        Btn::new("hunt-broken", put(&say!("Still broken")))
                            .on_click(cx.listener(|shell, _, _, cx| shell.answer_hunt(true, cx))),
                    )
                    .child(
                        Btn::new("hunt-works", put(&say!("Works now")))
                            .on_click(cx.listener(|shell, _, _, cx| shell.answer_hunt(false, cx))),
                    )
                })
                .when(found, |d| {
                    d.child(
                        Btn::new("hunt-without", put(&say!("My list without it")))
                            .primary()
                            .tip(put(&say!("The list the search began with, minus this mod. Nothing is written before Save.")))
                            .on_click(cx.listener(|shell, _, _, cx| shell.end_hunt(true, cx))),
                    )
                })
                .child(
                    Btn::new(
                        "hunt-stop",
                        match found {
                            true => put(&say!("My list as it was")),
                            false => put(&say!("Stop")),
                        },
                    )
                    .tip(put(&say!("The list the search began with. Nothing is written before Save.")))
                    .on_click(cx.listener(|shell, _, _, cx| shell.end_hunt(false, cx))),
                )
                .child(div().absolute().bottom_0().left_0().right_0().h(px(1.)).bg(c.edge))
                .into_any_element(),
        )
    }
}
