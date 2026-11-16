use super::navigate::{self, Go};
use super::section::Section;
use super::theme::{self, Theme};
use super::widgets::{TextSized, icon, label};
use crate::words::put;
use crate::workspace::logs::moment;
use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement,
    Styled, Window, div, prelude::FluentBuilder,
};
use rimstack_core::{say, say_n};

#[cfg(test)]
thread_local! {
    pub(crate) static DRAWN: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub struct StatusNote {
    pub demo: bool,
}

impl Render for StatusNote {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(test)]
        DRAWN.with(|drawn| drawn.set(drawn.get() + 1));
        let theme = cx.global::<Theme>().clone();
        let c = theme.colours;
        let demo = self.demo;
        let hint = super::tip::hint(cx);
        let hinted = !hint.is_empty() && hint != put(&say!("Opens Logs on this launch")).as_str();
        let pointed = hinted.then(|| hint.lines().next().unwrap_or_default().to_string());
        let unsaved = super::around::around(cx).read(cx).unsaved.clone();
        let last = super::outside::outside(cx)
            .read(cx)
            .last_start
            .as_ref()
            .map(|said| (said.start.time, said.start.errors));
        div()
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap(theme::GAP_WIDE)
            .when(demo, |d| {
                d.child(label(
                    &theme,
                    theme.small(),
                    c.ink2,
                    put(&say!("Demo data")),
                ))
            })
            .when_some(unsaved, |d, unsaved| {
                d.child(label(
                    &theme,
                    theme.small(),
                    c.warn,
                    put(&say!("Not saved: {}", unsaved)),
                ))
            })
            .when_some(pointed, |d, said| {
                let said: gpui::SharedString = said.into();
                d.child(super::widgets::watched(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .sized_text(&theme, theme.small())
                        .text_color(c.ink),
                    said.clone(),
                    said,
                    true,
                ))
            })
            .when_some(last.filter(|_| !hinted), |d, (time, errors)| {
                let started = put(&say!("Last start of the game {}", moment(time)));
                let said = errors.map(|n| match n {
                    0 => (put(&say!("no errors")), c.ink2),
                    n => (
                        put(&say_n!(n, "{} different error", "{} different errors")),
                        c.err,
                    ),
                });
                let tip = put(&say!("Opens Logs on this launch"));
                let last = div()
                    .id("last-start")
                    .flex()
                    .items_center()
                    .gap(theme::GAP_SMALL)
                    .h(theme.control())
                    .cursor_pointer()
                    .role(gpui::Role::Button)
                    .aria_label(started.clone())
                    .tab_index(0)
                    .capture_any_mouse_down(|_, window, _| window.prevent_default())
                    .focus_visible(|s| s.underline())
                    .child(label(&theme, theme.small(), c.ink2, started))
                    .when_some(said, |d, (said, tone)| {
                        d.child(label(&theme, theme.small(), tone, said))
                    })
                    .child(icon("chevron-right", theme.sized(11.), c.ink3))
                    .on_click(|_, _, cx| navigate::go(Go::Section(Section::Logs), cx));
                d.child(super::tip::tipped(last, tip.into()))
            })
    }
}
