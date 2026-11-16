use super::Logs;
use crate::ui::button::{Btn, Chip};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, chip_width, label, measure, with_alpha};
use crate::words::put;
use crate::workspace::logs::{LaunchShown, launches, moment};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Pixels, Role, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use rimstack_core::{say, say_n};

struct Entry {
    id: (&'static str, usize),
    title: String,
    outcome: String,
    tip: String,
    bad: bool,
    clean: bool,
    current: bool,
    choosable: bool,
}

impl Logs {
    pub(super) fn pane_width(theme: &Theme, window: &Window) -> Pixels {
        theme.sized(250.).min(window.viewport_size().width * 0.27)
    }

    fn entry(
        &self,
        entry: Entry,
        width: Pixels,
        theme: &Theme,
        window: &Window,
        choose: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let dot_x = theme::GAP_WIDE + theme::GAP_LINE;
        let dot = px(9.);
        let left = dot_x + dot + theme::GAP_MEDIUM;
        let room = width - left - theme::GAP_CARD;
        let chip =
            (!entry.outcome.is_empty()).then(|| chip_width(window, theme, &entry.outcome, false));
        let stacked = chip.is_some_and(|chip| {
            measure(
                window,
                theme,
                &entry.title,
                theme.body(),
                FontWeight::NORMAL,
            ) + theme::GAP
                + chip
                > room
        });
        let tone = if entry.bad {
            c.err
        } else if entry.clean {
            c.ok
        } else {
            c.ink3
        };
        let title_ink = if entry.current {
            c.on_selected
        } else if entry.choosable {
            c.ink
        } else {
            c.ink2
        };
        let (hover, current, choosable) =
            (with_alpha(c.hover, 0.04), entry.current, entry.choosable);
        div()
            .id(entry.id)
            .role(Role::ListItem)
            .aria_label(format!(
                "{}{}",
                entry.title,
                if entry.outcome.is_empty() {
                    String::new()
                } else {
                    format!(", {}", entry.outcome)
                }
            ))
            .relative()
            .w(width)
            .min_h(theme.sized(34.))
            .py(theme::GAP_SMALL)
            .flex()
            .items_center()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(theme::GAP_SMALL)
                    .right(theme::GAP_SMALL)
                    .rounded(theme::CORNER)
                    .when(current, |d| d.bg(c.chosen))
                    .when(!current && choosable, |d| d.hover(move |s| s.bg(hover))),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(dot_x + theme::GAP_TINY)
                    .w(px(1.))
                    .bg(if current { c.on_selected_dim } else { c.line }),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(dot_x)
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .size(dot)
                            .rounded(dot / 2.)
                            .bg(if current { c.on_selected } else { tone })
                            .border_2()
                            .border_color(if current { c.chosen } else { c.panel }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .when(stacked, |d| d.flex_col().items_start())
                    .when(!stacked, |d| d.items_center())
                    .gap(if stacked { theme::GAP_TINY } else { theme::GAP })
                    .ml(left)
                    .w(room)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .line_clamp(2)
                            .sized_text(theme, theme.body())
                            .text_color(title_ink)
                            .child(entry.title),
                    )
                    .when(!entry.outcome.is_empty(), |d| {
                        d.child(
                            Chip::new(entry.outcome)
                                .tone(tone)
                                .on_accent(current)
                                .max_width(room)
                                .tip(entry.tip),
                        )
                    }),
            )
            .when(choosable, |d| {
                d.on_click(cx.listener(move |logs, _, window, cx| choose(logs, window, cx)))
            })
            .into_any_element()
    }

    pub(super) fn launches_pane(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let width = Self::pane_width(theme, window);
        let report = self.report.clone();
        let on_last = !self.previous && self.kept.is_none();
        let on_before = self.previous && self.kept.is_none();
        let outside = on_last && report.as_ref().is_some_and(|r| r.outside);
        let errors = report.as_ref().map(|r| r.errors.len());
        let outcome = |shown: bool| match (shown, errors) {
            (true, Some(n)) => put(&crate::workspace::logs::outcome(Some(n))),
            _ => String::new(),
        };
        let tip = put(&say!(
            "Different errors in the log; one that repeats counts once"
        ));
        let last = Entry {
            id: ("launch", 0),
            title: if outside {
                put(&say!("Started outside RimStack"))
            } else {
                put(&say!("Last launch"))
            },
            outcome: outcome(on_last),
            tip: match outside {
                true => format!(
                    "{tip}. {}",
                    put(&say!(
                        "The game was started without Play: in a round of the search for the broken mod, or from Steam."
                    ))
                ),
                false => tip.clone(),
            },
            bad: on_last && errors.is_some_and(|n| n > 0),
            clean: on_last && errors == Some(0),
            current: on_last && self.recorded.is_none(),
            choosable: true,
        };
        let has_other = report.as_ref().is_some_and(|r| r.has_other);
        let before = (self.kept.is_some() || self.previous || has_other).then(|| Entry {
            id: ("launch", 1),
            title: put(&say!("Previous launch")),
            outcome: outcome(on_before),
            tip: tip.clone(),
            bad: on_before && errors.is_some_and(|n| n > 0),
            clean: on_before && errors == Some(0),
            current: on_before && self.recorded.is_none(),
            choosable: true,
        });
        let shown: Vec<LaunchShown<'_>> = report.as_deref().map(launches).unwrap_or_default();
        let recorded: Vec<AnyElement> = shown
            .iter()
            .enumerate()
            .map(|(at, launch)| {
                let log = launch.launch.log.clone();
                let current = self.recorded == Some(at)
                    || (self.kept.is_some() && self.kept == log && self.recorded.is_none());
                let entry = Entry {
                    id: ("recorded", at),
                    title: moment(launch.launch.time),
                    outcome: put(&launch.outcome),
                    tip: String::new(),
                    bad: launch.launch.errors.is_some_and(|n| n > 0),
                    clean: launch.launch.errors == Some(0),
                    current,
                    choosable: true,
                };
                self.entry(
                    entry,
                    width,
                    theme,
                    window,
                    move |logs, _, cx| match log.clone() {
                        Some(log) => logs.show(false, Some(log), cx),
                        None => {
                            logs.recorded = Some(at);
                            cx.notify();
                        }
                    },
                    cx,
                )
            })
            .collect();
        let has_recorded = !recorded.is_empty();
        let fixed = if before.is_some() { 2 } else { 1 };
        let count = fixed + shown.len();
        let current = match (self.recorded, &self.kept) {
            (Some(at), _) => fixed + at,
            (None, Some(kept)) => {
                fixed
                    + shown
                        .iter()
                        .position(|l| l.launch.log.as_ref() == Some(kept))
                        .unwrap_or(0)
            }
            (None, None) if on_before && before.is_some() => 1,
            _ => 0,
        };
        let logs_for_keys: Vec<Option<String>> =
            shown.iter().map(|l| l.launch.log.clone()).collect();
        div()
            .id("launches")
            .track_focus(&self.focus.launches)
            .role(Role::List)
            .aria_label(put(&say!("Launches")))
            .flex()
            .flex_col()
            .flex_none()
            .w(width)
            .h_full()
            .bg(c.panel)
            .border_r_1()
            .border_color(c.edge)
            .on_key_down(cx.listener(move |logs, event: &KeyDownEvent, _, cx| {
                let to = match event.keystroke.key.as_str() {
                    "up" => current.saturating_sub(1),
                    "down" => (current + 1).min(count.saturating_sub(1)),
                    "home" => 0,
                    "end" => count.saturating_sub(1),
                    _ => return,
                };
                match to.checked_sub(fixed) {
                    None => logs.show(to == 1, None, cx),
                    Some(at) => match logs_for_keys.get(at).cloned().flatten() {
                        Some(log) => logs.show(false, Some(log), cx),
                        None => {
                            logs.recorded = Some(at);
                            cx.notify();
                        }
                    },
                }
            }))
            .child(div().flex().items_center().h(theme.header()).pl(theme::GAP_CARD).child(label(theme, theme.heading(), c.ink, put(&say!("Launches"))).font_weight(theme.semibold)))
            .child(self.entry(
                    last, width, theme, window, |logs, _, cx| logs.show(false, None, cx), cx))
            .children(before.map(|before| self.entry(
                    before, width, theme, window, |logs, _, cx| logs.show(true, None, cx), cx)))
            .when(has_recorded, |d| {
                let title = put(&say!("Recorded"));
                let group = crate::ui::tip::tipped(
                    div().id("recorded-title").pl(theme::GAP_WIDE).pt(theme::GAP_WIDE).pb(theme::GAP_SMALL).child(label(theme, theme.small(), c.ink2, title).font_weight(theme.semibold)),
                    put(&say!("The game keeps the logs of its last two launches. RimStack records how each launch it starts went.")).into(),
                );
                d.child(group).child(div().flex_1().min_h_0().pt(theme::GAP_TINY).child(crate::ui::page::page("recorded-launches", &self.launches_paging, theme, cx.entity_id(), div().flex().flex_col().children(recorded))))
            })
            .into_any_element()
    }

    pub(super) fn launch_record(
        &self,
        at: usize,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let c = &theme.colours;
        let report = self.report.clone()?;
        let shown = launches(&report);
        let launch = shown.get(at)?;
        let record = launch.launch;
        let outcome = put(&launch.outcome);
        let differs: Vec<String> = [
            (record.removed > 0).then(|| {
                put(&say_n!(
                    record.removed,
                    "{} mod no longer in the list",
                    "{} mods no longer in the list"
                ))
            }),
            (record.added > 0).then(|| {
                put(&say_n!(
                    record.added,
                    "{} mod added since",
                    "{} mods added since"
                ))
            }),
            record.reordered.then(|| put(&say!("order changed"))),
        ]
        .into_iter()
        .flatten()
        .collect();
        let order = format!(
            "{}. {}",
            put(&say_n!(record.mods, "{} mod", "{} mods")),
            if differs.is_empty() {
                put(&say!("The same as the list now"))
            } else {
                differs.join(", ")
            }
        );
        let ids = record.ids.clone();
        let changed = record.added + record.removed > 0 || record.reordered;
        let width = theme.sized(620.).min(
            window.viewport_size().width - Self::pane_width(theme, window) - theme::GAP_WIDE * 2.,
        );
        Some(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(Self::pane_width(theme, window))
                .right_0()
                .bg(c.win)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(theme::GAP_SMALL)
                        .w(width)
                        .ml(theme::GAP_WIDE)
                        .mt(theme::GAP_CARD)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(theme::GAP)
                                .child(label(theme, theme.title(), c.ink, moment(record.time)).font_weight(theme.semibold))
                                .child(Chip::new(outcome).tone(if record.errors.is_some_and(|n| n > 0) { c.err } else { c.ink2 })),
                        )
                        .child(div().sized_text(theme, theme.body()).text_color(c.ink2).child(put(&say!("The game keeps the logs of its last two launches only. For this one, RimStack kept the load order."))))
                        .child(label(theme, theme.small(), c.ink2, put(&say!("Its load order"))).font_weight(theme.semibold).pt(theme::GAP_WIDE).pb(theme::GAP_SMALL))
                        .child(div().sized_text(theme, theme.body()).text_color(c.ink).child(order))
                        .child(
                            div()
                                .flex()
                                .gap(theme::GAP_SMALL)
                                .pt(theme::GAP)
                                .child(
                                    Btn::new("use-its-list", put(&say!("Use its list")))
                                        .tip(put(&say!("Uses this launch's load order as the list on the Mods screen. Save writes it.")))
                                        .enabled(changed)
                                        .on_click(cx.listener(move |_, _, _, cx| crate::ui::navigate::go(crate::ui::navigate::Go::UseList(ids.clone()), cx))),
                                )
                                .child(Btn::new("back-to-log", put(&say!("Back to the log"))).on_click(cx.listener(|logs, _, _, cx| {
                                    logs.recorded = None;
                                    cx.notify();
                                }))),
                        ),
                )
                .into_any_element(),
        )
    }
}
