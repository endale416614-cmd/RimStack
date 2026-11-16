mod errors;
mod head;
mod log;

use super::{Logs, View};

use crate::ui::rows::{bar, rows};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, label, with_alpha};
use crate::words::put;
use crate::workspace::logs::head;
use gpui::{
    AnyElement, ClickEvent, Context, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Pixels, Role, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use rimstack_core::{say, say_n};
use std::rc::Rc;

impl Logs {
    pub(super) fn middle_width(theme: &Theme, window: &Window) -> Pixels {
        window.viewport_size().width
            - Self::pane_width(theme, window)
            - Self::side_width(theme, window)
            - px(2.)
    }

    fn culprits_view(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let c = theme.colours;
        let culprits: Rc<Vec<rimstack_core::mods::blame::Blamed>> = Rc::new(
            self.report
                .as_ref()
                .map(|r| r.culprits.clone())
                .unwrap_or_default(),
        );
        let height = theme.sized(34.);
        let tops: Rc<[Pixels]> = (0..=culprits.len())
            .map(|at| theme::GAP_TINY + height * at as f32)
            .collect::<Vec<_>>()
            .into();
        let (current, me) = (self.culprit, cx.entity().downgrade());
        let theme_owned = theme.clone();
        let shown = culprits.clone();
        let list = rows(
            "culprit-rows",
            self.culprits_scroll.clone(),
            tops.clone(),
            height,
            cx.entity_id(),
            move |range, width, _, _| {
                let theme = &theme_owned;
                range
                    .filter_map(|at| {
                        let culprit = shown.get(at)?.clone();
                        let on = at == current;
                        let name = if culprit.name.is_empty() {
                            put(&say!("No mod recognised"))
                        } else {
                            culprit.name.clone()
                        };
                        let said = format!(
                            "{}, {}",
                            put(&say_n!(culprit.kinds, "{} error", "{} errors")),
                            put(&say_n!(culprit.times, "logged {} time", "logged {} times"))
                        );
                        let (me, hover) = (me.clone(), with_alpha(c.hover, 0.04));
                        Some(
                            div()
                                .id(("culprit", at))
                                .role(Role::ListItem)
                                .aria_label(put(&say_n!(
                                    culprit.kinds,
                                    "{} error, {}",
                                    "{} errors, {}",
                                    name.clone()
                                )))
                                .relative()
                                .w(width)
                                .h(height)
                                .flex()
                                .items_center()
                                .gap(theme::GAP_LARGE)
                                .px(theme::GAP_WIDE)
                                .cursor_pointer()
                                .child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .bottom_0()
                                        .left(theme::GAP_SMALL)
                                        .right(theme::GAP_SMALL)
                                        .rounded(theme::CORNER)
                                        .when(on, |d| d.bg(c.chosen))
                                        .when(!on, |d| d.hover(move |s| s.bg(hover))),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .sized_text(theme, theme.body())
                                        .text_color(if on {
                                            c.on_selected
                                        } else if culprit.name.is_empty() {
                                            c.ink2
                                        } else {
                                            c.ink
                                        })
                                        .child(name),
                                )
                                .child(label(
                                    theme,
                                    theme.body(),
                                    if on { c.on_selected_dim } else { c.ink2 },
                                    said,
                                ))
                                .on_click(move |event: &ClickEvent, _, cx| {
                                    let name = culprit.name.clone();
                                    let _ = me.update(cx, |logs, cx| {
                                        logs.culprit = at;
                                        if event.click_count() == 2 {
                                            logs.open_culprit(&name, cx);
                                        }
                                        cx.notify();
                                    });
                                })
                                .into_any_element(),
                        )
                    })
                    .collect()
            },
        );
        let count = culprits.len();
        div()
            .id("culprits")
            .track_focus(&self.focus.culprits)
            .role(Role::List)
            .aria_label(put(&say!("Mods the errors point at")))
            .relative()
            .flex_1()
            .min_h_0()
            .on_key_down(cx.listener(move |logs, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" {
                    let name = culprits
                        .get(logs.culprit)
                        .map(|c| c.name.clone())
                        .unwrap_or_default();
                    return logs.open_culprit(&name, cx);
                }
                if let Some(to) = step(event, logs.culprit, count) {
                    logs.culprit = to;
                    logs.culprits_scroll
                        .borrow_mut()
                        .contain(tops[to], tops[to + 1]);
                    cx.notify();
                }
            }))
            .child(list)
            .child(bar(theme, self.culprits_scroll.clone(), cx.entity_id()))
            .into_any_element()
    }

    pub(super) fn middle(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = theme.colours;
        let cut = self
            .report
            .as_deref()
            .map(|r| put(&head(r).cut))
            .unwrap_or_default();
        let view = match self.view {
            View::Errors => self.errors_view(theme, window, cx),
            View::Mods => self.culprits_view(theme, cx),
            View::Log => self.log_view(theme, window, cx),
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .child(self.head(theme, window, cx))
            .children(self.since_box(theme, cx))
            .children(self.filter_row(theme, window, cx))
            .child(view)
            .when(self.view == View::Log && !cut.is_empty(), |d| {
                d.child(
                    label(theme, theme.small(), c.ink3, cut)
                        .pl(theme::GAP_WIDE)
                        .pb(theme::GAP_SMALL),
                )
            })
            .into_any_element()
    }
}

pub(super) fn step(event: &KeyDownEvent, current: usize, count: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    match event.keystroke.key.as_str() {
        "up" => Some(current.saturating_sub(1)),
        "down" => Some((current + 1).min(count - 1)),
        "home" => Some(0),
        "end" => Some(count - 1),
        _ => None,
    }
}
