//! The first start's five steps; nothing changes until Start but what is chosen
//! here, which is kept as it is chosen.

mod parts;
mod steps;

use super::button::Btn;

use super::theme::{self, Theme};
use super::widgets::{label, measure};
use crate::changes::Changes;
use crate::reports::{Database, SettingsReport};
use crate::words::put;
use gpui::{
    Context, EventEmitter, FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, img, prelude::FluentBuilder,
};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::say;
use std::rc::Rc;

const READS: Changes = Changes::SETTINGS
    .or(Changes::COMMUNITY)
    .or(Changes::LANGUAGE)
    .or(Changes::FOLDERS);

/// Asked of the window: the setup ends, the community data fetched and RimSort's rules read
/// when asked.
pub struct Started {
    pub with_data: bool,
    pub with_rimsort: bool,
}

pub struct FirstRun {
    step: usize,
    report: Rc<SettingsReport>,
    databases: Rc<Vec<Database>>,
    languages: Rc<Vec<String>>,
    with_rimsort: bool,
    mods: (usize, usize, String),
    reading: crate::jobs::Lane,
    /// The official mods' previews in the glance at the list, let go with the first run.
    pictures: super::pictures::Pictures,
}

impl EventEmitter<Started> for FirstRun {}

const STEPS: usize = 5;

impl FirstRun {
    pub fn new(step: usize, cx: &mut Context<Self>) -> Self {
        let mut first = Self {
            step: step.min(STEPS - 1),
            report: Rc::default(),
            databases: Rc::default(),
            languages: Rc::default(),
            with_rimsort: true,
            mods: (0, 0, String::new()),
            reading: Default::default(),
            pictures: Default::default(),
        };
        first.read(cx);
        super::hear(cx, READS, |first: &mut Self, _, cx| first.read(cx));
        first
    }

    fn read(&mut self, cx: &mut Context<Self>) {
        let app = super::app(cx);
        self.reading.run(
            "first run read",
            cx,
            move |_| (app.settings_report(), app.databases(), app.languages()),
            |first, (report, databases, languages), cx| {
                first.report = Rc::new(report);
                first.databases = Rc::new(databases);
                first.languages = Rc::new(languages);
                cx.notify();
            },
        );
    }

    pub fn set_mods(
        &mut self,
        active: usize,
        inactive: usize,
        problem: String,
        cx: &mut Context<Self>,
    ) {
        self.mods = (active, inactive, problem);
        cx.notify();
    }

    fn keep(&mut self, name: &str, value: &str, cx: &mut Context<Self>) {
        if let Err(problem) = super::app(cx).keep_setting(name, value) {
            super::notices::say(
                crate::words::said_of(&problem),
                super::notices::Tone::Err,
                cx,
            );
        }
        self.read(cx);
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn press_start(&mut self, cx: &mut Context<Self>) {
        self.start(cx);
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        let with_data = self.databases.iter().any(|d| d.on);
        let with_rimsort = self.with_rimsort && self.report.rimsort_rules;
        cx.emit(Started {
            with_data,
            with_rimsort,
        });
    }

    fn steps() -> [Sentence; STEPS] {
        [
            say!("Found"),
            say!("Community data"),
            say!("Sorting"),
            say!("Appearance"),
            say!("Start"),
        ]
    }
}

impl Render for FirstRun {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let c = theme.colours;
        let viewport = window.viewport_size();
        let width = theme.sized(940.).min(viewport.width - theme::GAP_PAGE * 4.);
        let height = theme
            .sized(580.)
            .min(viewport.height - theme::GAP_PAGE * 2.);
        let side = theme.sized(112.).max(measure(
            window,
            &theme,
            &put(&say!("A mod manager for RimWorld")),
            theme.title(),
            FontWeight::NORMAL,
        ));
        let content_width = width - side - theme::GAP_WIDE * 3.;
        let steps = Self::steps();
        let marks = steps.iter().enumerate().map(|(at, name)| {
            let (ground, ink) = match at.cmp(&self.step) {
                std::cmp::Ordering::Equal => (c.accent, c.accent_ink),
                std::cmp::Ordering::Less => (c.ok, c.accent_ink),
                std::cmp::Ordering::Greater => (c.ctl, c.ink2),
            };
            let mark = if at < self.step {
                "✓".to_string()
            } else {
                (at + 1).to_string()
            };
            let on = at == self.step;
            div()
                .id(("step", at))
                .flex()
                .items_center()
                .gap(theme::GAP)
                .cursor_pointer()
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(theme.sized(22.))
                        .rounded(theme.sized(11.))
                        .bg(ground)
                        .child(label(&theme, theme.small(), ink, mark).font_weight(theme.semibold)),
                )
                .child(
                    label(
                        &theme,
                        theme.body(),
                        if on { c.ink } else { c.ink2 },
                        put(name),
                    )
                    .when(on, |d| d.font_weight(theme.semibold)),
                )
                .on_click(cx.listener(move |first, _, _, cx| {
                    first.step = at;
                    cx.notify();
                }))
        });
        let meanings = [
            say!("Found automatically; changeable in Settings"),
            say!("Fetched from GitHub on Start, and again only on request"),
            say!("What Sort follows besides each mod's own rules"),
            say!("Changeable in Settings at any time"),
            say!("Nothing is written to the game before Save"),
        ];
        let last = self.step == STEPS - 1;
        let buttons =
            div()
                .flex()
                .justify_end()
                .gap(theme::GAP)
                .when(self.step > 0, |d| {
                    d.child(Btn::new("first-back", put(&say!("Back"))).large().on_click(
                        cx.listener(|first, _, _, cx| {
                            first.step -= 1;
                            cx.notify();
                        }),
                    ))
                })
                .when(!last, |d| {
                    d.child(
                        Btn::new("first-skip", put(&say!("Skip to Start")))
                            .large()
                            .on_click(cx.listener(|first, _, _, cx| first.start(cx))),
                    )
                    .child(
                        Btn::new("first-next", put(&say!("Next")))
                            .large()
                            .primary()
                            .on_click(cx.listener(|first, _, _, cx| {
                                first.step += 1;
                                cx.notify();
                            })),
                    )
                })
                .when(last, |d| {
                    d.child(
                        Btn::new("first-start", put(&say!("Start")))
                            .large()
                            .primary()
                            .icon("check")
                            .on_click(cx.listener(|first, _, _, cx| first.start(cx))),
                    )
                });
        div()
            .id("first-run")
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(c.win)
            .child(
                div()
                    .flex()
                    .gap(theme::GAP_WIDE * 3.)
                    .w(width)
                    .h(height)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_none()
                            .gap(theme::GAP_SMALL)
                            .child(img("rimstack.svg").size(theme.sized(112.)))
                            .child(
                                label(&theme, theme.display(), c.brand, put(&say!("RimStack")))
                                    .font_weight(theme.semibold)
                                    .pt(theme::GAP),
                            )
                            .child(label(
                                &theme,
                                theme.title(),
                                c.ink,
                                put(&say!("A mod manager for RimWorld")),
                            ))
                            .child(div().h(theme::GAP_PAGE))
                            .children(marks),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(theme::GAP_WIDE)
                            .child(
                                div()
                                    .id("first-content")
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                label(
                                                    &theme,
                                                    theme.heading(),
                                                    c.ink,
                                                    put(&steps[self.step]),
                                                )
                                                .font_weight(theme.semibold),
                                            )
                                            .child(
                                                label(
                                                    &theme,
                                                    theme.body(),
                                                    c.ink2,
                                                    put(&meanings[self.step]),
                                                )
                                                .pt(theme::GAP_HAIR),
                                            )
                                            .children(self.content(
                                                &theme,
                                                window,
                                                content_width,
                                                cx,
                                            )),
                                    ),
                            )
                            .child(buttons),
                    ),
            )
    }
}
