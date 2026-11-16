use super::step;
use crate::ui::button::Chip;

use crate::ui::logs::Logs;

use crate::ui::rows::{bar, rows};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, label, with_alpha};
use crate::words::{count, put};
use crate::workspace::logs::head;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels,
    Role, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use rimstack_core::{say, say_n};
use std::rc::Rc;

impl Logs {
    fn error_tops(&self, theme: &Theme) -> Rc<[Pixels]> {
        let Some(report) = &self.report else {
            return Rc::from(vec![theme::GAP_TINY]);
        };
        let mut at = theme::GAP_TINY;
        let mut tops = Vec::new();
        for shown in self.errors(report) {
            tops.push(at);
            let words = theme.line(theme.body())
                + if shown.detail.is_empty() {
                    px(0.)
                } else {
                    theme::GAP_THIN + theme.line(theme.small())
                };
            at += theme.sized(40.).max(words + theme.sized(18.)) + theme::GAP_HAIR;
        }
        tops.push(at);
        tops.into()
    }

    pub(super) fn errors_view(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = theme.colours;
        let report = self.report.clone();
        let count_rows = report.as_ref().map_or(0, |r| self.errors(r).len());
        if count_rows == 0 {
            let facts = report.as_deref().map(head);
            let filtered = !self.filter_text.is_empty();
            let title = match () {
                _ if self.busy => put(&say!("Reading the log…")),
                _ if self.problem.is_some() => put(&say!("Log could not be read")),
                _ if filtered => put(&say!("No error matches")),
                _ if facts.as_ref().is_some_and(|f| f.lines > 0) => put(&say!("No errors")),
                _ => put(&say!("No log")),
            };
            let words = match &self.problem {
                Some(problem) => problem.clone(),
                None if filtered => String::new(),
                None => match facts.as_ref().filter(|f| f.lines > 0) {
                    Some(f) => put(&say_n!(f.lines, "{} line", "{} lines")),
                    None => put(&say!("Written when the game starts")),
                },
            };
            let width =
                (Self::middle_width(theme, window) - theme::GAP_PAGE * 4.).min(theme.sized(480.));
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(theme::GAP_SMALL)
                        .w(width)
                        .child(
                            div()
                                .text_center()
                                .sized_text(theme, theme.title())
                                .font_weight(theme.semibold)
                                .text_color(if self.problem.is_some() { c.err } else { c.ink })
                                .child(title),
                        )
                        .when(!self.busy, |d| {
                            d.child(
                                div()
                                    .text_center()
                                    .spaced_text(theme, theme.body(), 1.3)
                                    .text_color(c.ink2)
                                    .child(words),
                            )
                        }),
                )
                .into_any_element();
        }
        let tops = self.error_tops(theme);
        let selected = self.selected;
        let me = cx.entity().downgrade();
        let draw_tops = tops.clone();
        let theme_owned = theme.clone();
        let list = rows(
            "error-rows",
            self.errors_scroll.clone(),
            tops.clone(),
            theme.sized(40.),
            cx.entity_id(),
            move |range, width, window, cx| {
                let Some(logs) = me.upgrade() else {
                    return Vec::new();
                };
                let report = logs.read(cx).report.clone();
                let Some(report) = report else {
                    return Vec::new();
                };
                let shown = logs.read(cx).errors(&report);
                range
                    .filter_map(|at| {
                        Some(error_row(
                            &theme_owned,
                            at,
                            shown.get(at)?,
                            at == selected,
                            width,
                            draw_tops[at + 1] - draw_tops[at] - theme::GAP_HAIR,
                            me.clone(),
                            window,
                            cx,
                        ))
                    })
                    .collect()
            },
        );
        let count = count_rows;
        let keys_tops = tops;
        div()
            .id("errors")
            .track_focus(&self.focus.errors)
            .role(Role::List)
            .aria_label(put(&say!("Errors")))
            .relative()
            .flex_1()
            .min_h_0()
            .on_key_down(cx.listener(move |logs, event: &KeyDownEvent, _, cx| {
                if let Some(to) = step(event, logs.selected, count) {
                    logs.selected = to;
                    logs.errors_scroll
                        .borrow_mut()
                        .contain(keys_tops[to], keys_tops[to + 1]);
                    cx.notify();
                }
            }))
            .child(list)
            .child(bar(theme, self.errors_scroll.clone(), cx.entity_id()))
            .into_any_element()
    }
}

#[allow(clippy::too_many_arguments)]
fn error_row(
    theme: &Theme,
    at: usize,
    shown: &crate::workspace::logs::ErrorShown<'_>,
    current: bool,
    width: Pixels,
    height: Pixels,
    me: gpui::WeakEntity<Logs>,
    window: &Window,
    cx: &mut gpui::App,
) -> AnyElement {
    let _ = (window, cx);
    let c = &theme.colours;
    let error = shown.error;
    let hover = with_alpha(c.hover, 0.05);
    let run = error.run;
    let mod_id = error.mod_id;
    let (pick, more_me) = (me.clone(), me);
    let times = format!("×{}", count(error.count));
    let mod_room = theme.sized(300.).min(width * 0.36);
    div()
        .id(("error", at))
        .role(Role::ListItem)
        .aria_label(error.head.clone())
        .relative()
        .w(width)
        .h(height)
        .flex()
        .items_center()
        .gap(theme::GAP_LARGE)
        .pl(theme::GAP_PAGE)
        .pr(theme::GAP_PAGE + theme::GAP_SMALL)
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(theme::GAP_LARGE)
                .right(theme::GAP_CARD)
                .rounded(theme::CORNER)
                .when(current, |d| d.bg(c.chosen))
                .when(!current, |d| d.hover(move |s| s.bg(hover))),
        )
        .when(!current, |d| {
            d.child(
                div()
                    .absolute()
                    .bottom(px(-1.))
                    .left(theme::GAP_PAGE)
                    .right(theme::GAP_PAGE)
                    .h(px(1.))
                    .bg(with_alpha(c.line, 0.6)),
            )
        })
        .when(error.count > 1, |d| {
            let counted = crate::ui::tip::tipped(
                div()
                    .id(("times", at))
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .min_w(theme.control())
                    .h(theme.control())
                    .px(theme::GAP_SMALL)
                    .rounded(theme::CORNER)
                    .bg(if current {
                        theme::hex("#38000000")
                    } else {
                        with_alpha(c.err, 0.16)
                    })
                    .child(
                        label(
                            theme,
                            theme.small(),
                            if current { c.on_selected } else { c.err },
                            times,
                        )
                        .font_weight(theme.semibold),
                    ),
                put(&say!("How many times the log shows it")).into(),
            );
            d.child(counted)
        })
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(theme::GAP_THIN)
                .child(
                    div()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .sized_text(theme, theme.body())
                        .font_weight(theme.medium)
                        .text_color(if current { c.on_selected } else { c.ink })
                        .child(shown.kind.to_string()),
                )
                .when(!shown.detail.is_empty(), |d| {
                    d.child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .sized_text(theme, theme.small())
                            .text_color(if current { c.on_selected_dim } else { c.ink2 })
                            .child(shown.detail.to_string()),
                    )
                }),
        )
        .when(shown.more > 0, |d| {
            d.child(
                Chip::new(put(&say!("{} more like it", shown.more)))
                    .on_accent(current)
                    .icon(if shown.opened {
                        "chevron-down"
                    } else {
                        "chevron-right"
                    })
                    .tip(put(&say!(
                        "Later errors that differ from this one only in their last word"
                    )))
                    .on_click(("more-like", at), move |_, _, cx| {
                        let _ = more_me.update(cx, |logs, cx| logs.toggle_run(run, cx));
                    }),
            )
        })
        .when_some(error.first.clone(), |d, name| {
            let tip = match mod_id.is_some() {
                true => put(&say!("The mod the error points at. Click to go to it.")),
                false => put(&say!("The mod the error points at")),
            };
            let chip = Chip::new(name)
                .max_width(mod_room)
                .on_accent(current)
                .tone(c.ink)
                .icon("package")
                .tip(tip);
            d.child(match mod_id {
                Some(mod_id) => chip.on_click(("error-mod", at), move |_, _, cx| {
                    crate::ui::navigate::go(crate::ui::navigate::Go::ShowMod(mod_id), cx);
                }),
                None => chip,
            })
        })
        .on_click(move |_, _, cx| {
            let _ = pick.update(cx, |logs, cx| {
                logs.selected = at;
                cx.notify();
            });
        })
        .into_any_element()
}
