use super::step;

use crate::ui::logs::Logs;
use crate::ui::menu::{self, Entry};
use crate::ui::rows::{bar, rows};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, breakable, label, measure_in, with_alpha};
use crate::words::put;

use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    ParentElement, Pixels, Role, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use rimstack_core::say;
use std::rc::Rc;

impl Logs {
    pub(in crate::ui::logs) fn log_tops(&mut self, theme: &Theme, window: &Window) -> Rc<[Pixels]> {
        let width = Self::middle_width(theme, window);
        if let Some((at, tops)) = &self.log_tops
            && *at == width
        {
            return tops.clone();
        }
        let line = theme.line(theme.small());
        let row = |rows: usize| theme.sized(20.).max(line * rows as f32 + px(4.));
        let room = width - Self::log_text_x(theme) - theme::GAP_WIDE;
        let advance = measure_in(
            window,
            theme,
            theme.mono.clone(),
            "n",
            theme.small(),
            FontWeight::NORMAL,
        );
        let per_row = ((room / advance).floor() as usize).max(1);
        let mut at = theme::GAP_TINY;
        let mut tops = Vec::with_capacity(self.lines.len() + 1);
        let text = self.report.as_ref().map(|r| &r.text);
        for &line_at in self.lines.iter() {
            tops.push(at);
            let chars = text.map_or(0, |t| t[line_at].chars().count());
            let rows = if self.wrap_log {
                chars.div_ceil(per_row).max(1)
            } else {
                1
            };
            at += row(rows);
        }
        tops.push(at);
        let tops: Rc<[Pixels]> = tops.into();
        self.log_tops = Some((width, tops.clone()));
        tops
    }

    fn log_text_x(theme: &Theme) -> Pixels {
        theme::GAP + theme.sized(52.) + theme::GAP_SMALL + theme.sized(16.) + theme::GAP
    }

    pub(super) fn log_view(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = theme.colours;
        let tops = self.log_tops(theme, window);
        let lines = self.lines.clone();
        if lines.is_empty() && !self.busy {
            let said = if self.filter_text.is_empty() {
                put(&say!("No log"))
            } else {
                put(&say!("No line matches"))
            };
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(label(theme, theme.body(), c.ink2, said))
                .into_any_element();
        }
        let report = self.report.clone();
        let (marked, wrap, me) = (self.marked, self.wrap_log, cx.entity().downgrade());
        let theme_owned = theme.clone();
        let draw_tops = tops.clone();
        let list = rows(
            "log-rows",
            self.log_scroll.clone(),
            tops.clone(),
            theme.sized(40.),
            cx.entity_id(),
            move |range, width, _, _| {
                let theme = &theme_owned;
                let Some(report) = &report else {
                    return Vec::new();
                };
                range
                    .filter_map(|at| {
                        let line_at = *lines.get(at)?;
                        let number = line_at + 1;
                        let text = report.text[line_at].clone();
                        let level = report.levels[line_at];
                        let bad = level == Some(rimstack_core::rimworld::logs::Level::Error);
                        let warning = level == Some(rimstack_core::rimworld::logs::Level::Warning);
                        let x = Self::log_text_x(theme);
                        let (me, copied) = (me.clone(), text.clone());
                        let mark_me = me.clone();
                        Some(
                            div()
                                .id(("log-line", at))
                                .role(Role::ListItem)
                                .aria_label(match (bad, warning) {
                                    (true, _) => format!("{}: {}", put(&say!("Error")), text),
                                    (_, true) => format!("{}: {}", put(&say!("Warning")), text),
                                    _ => text.clone(),
                                })
                                .relative()
                                .w(width)
                                .h(draw_tops[at + 1] - draw_tops[at])
                                .when(number == marked, |d| d.bg(with_alpha(c.accent, 0.25)))
                                .child(
                                    div()
                                        .absolute()
                                        .left(theme::GAP)
                                        .top_0()
                                        .bottom_0()
                                        .w(theme.sized(52.))
                                        .flex()
                                        .items_center()
                                        .justify_end()
                                        .child(
                                            div()
                                                .sized_text(theme, theme.small())
                                                .font_family(theme.mono.clone())
                                                .text_color(c.ink3)
                                                .child(number.to_string()),
                                        ),
                                )
                                .when(bad || warning, |d| {
                                    d.child(
                                        div()
                                            .absolute()
                                            .left(theme::GAP + theme.sized(52.) + theme::GAP_SMALL)
                                            .top_0()
                                            .bottom_0()
                                            .flex()
                                            .items_center()
                                            .child(
                                                div()
                                                    .size(theme.sized(16.))
                                                    .rounded(theme::CORNER)
                                                    .when(bad, |d| d.bg(c.danger_bg))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        div()
                                                            .sized_text(theme, theme.small())
                                                            .font_family(theme.mono.clone())
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(if bad {
                                                                c.err
                                                            } else {
                                                                c.warn
                                                            })
                                                            .child(if bad {
                                                                put(&say!("E"))
                                                            } else {
                                                                put(&say!("W"))
                                                            }),
                                                    ),
                                            ),
                                    )
                                })
                                .child(
                                    div()
                                        .absolute()
                                        .left(x)
                                        .right(theme::GAP_WIDE)
                                        .top_0()
                                        .bottom_0()
                                        .flex()
                                        .items_center()
                                        .child(
                                            div()
                                                .w_full()
                                                .sized_text(theme, theme.small())
                                                .font_family(theme.mono.clone())
                                                .text_color(if bad { c.err } else { c.ink })
                                                .when(!wrap, |d| {
                                                    d.overflow_hidden()
                                                        .whitespace_nowrap()
                                                        .text_ellipsis()
                                                })
                                                .child(if wrap { breakable(&text) } else { text }),
                                        ),
                                )
                                .on_click(move |_, _, cx| {
                                    let _ = mark_me.update(cx, |logs, cx| {
                                        logs.marked = number;
                                        cx.notify();
                                    });
                                })
                                .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                                    let copied = copied.clone();
                                    let entries: Vec<Entry> = vec![
                                        Entry::item(put(&say!("Copy line")), move |_, cx| {
                                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                                copied.clone(),
                                            ))
                                        })
                                        .icon("copy")
                                        .into(),
                                    ];
                                    menu::open_at(event.position, entries, window, cx);
                                })
                                .into_any_element(),
                        )
                    })
                    .collect()
            },
        );
        let count = self.lines.len();
        let current = self
            .lines
            .iter()
            .position(|l| l + 1 == self.marked)
            .unwrap_or(0);
        let (keys_lines, keys_tops) = (self.lines.clone(), tops);
        div()
            .id("log")
            .track_focus(&self.focus.log)
            .role(Role::List)
            .aria_label(put(&say!("Log")))
            .relative()
            .flex_1()
            .min_h_0()
            .on_key_down(cx.listener(move |logs, event: &KeyDownEvent, _, cx| {
                if let Some(to) = step(event, current, count) {
                    logs.marked = keys_lines[to] + 1;
                    logs.log_scroll
                        .borrow_mut()
                        .contain(keys_tops[to], keys_tops[to + 1]);
                    cx.notify();
                }
            }))
            .child(list)
            .child(bar(theme, self.log_scroll.clone(), cx.entity_id()))
            .into_any_element()
    }
}
