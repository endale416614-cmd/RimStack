use crate::ui::button::Btn;
use crate::ui::controls::{Choice, Toggle};
use crate::ui::field::Field;
use crate::ui::logs::{Logs, View};

use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, btn_width, label, measure};
use crate::words::put;
use crate::workspace::logs::{head, moment, since};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, Styled,
    Window, div, prelude::FluentBuilder, px,
};
use rimstack_core::say;

impl Logs {
    pub(super) fn head(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let report = self.report.clone();
        let facts = report.as_deref().map(head);
        let path = facts
            .as_ref()
            .map(|f| f.path.to_string())
            .unwrap_or_default();
        let written = match facts.as_ref().filter(|f| f.written > 0) {
            Some(facts) => moment(facts.written),
            None if self.previous => put(&say!("Previous launch")),
            None => put(&say!("Last launch")),
        };
        let about = facts.as_ref().map(|f| put(&f.about)).unwrap_or_default();
        let room = Self::middle_width(theme, window) - theme::GAP_CARD - theme::GAP_LARGE;
        let (open, copy, find) = (
            put(&say!("Open file")),
            put(&say!("Copy path")),
            put(&say!("Find the broken mod")),
        );
        let views: Vec<SharedString> = [say!("Errors"), say!("Mods"), say!("Log")]
            .iter()
            .map(|o| put(o).into())
            .collect();
        let finds = !self.demo && !self.hunting;
        let full = |text: &str| btn_width(window, theme, text, true, false);
        let choice = Choice::width(window, theme, &views);
        let tight = room
            < full(&open)
                + full(&copy)
                + if finds { full(&find) } else { px(0.) }
                + choice
                + theme::GAP * 3.;
        let compact = theme.icon() + theme.sized(14.);
        let controls_width = if tight {
            compact * 2. + if finds { compact } else { px(0.) }
        } else {
            full(&open) + full(&copy) + if finds { full(&find) } else { px(0.) }
        } + choice
            + theme::GAP * 3.;
        let wrapped = room
            < measure(window, theme, &written, theme.title(), theme.semibold)
                + controls_width
                + theme::GAP_WIDE;
        let button = |id: &'static str, text: String, icon: &'static str| match tight {
            true => Btn::new(id, "").icon(icon).tip(text),
            false => Btn::new(id, text).icon(icon),
        };
        let (opened, copied) = (path.clone(), path.clone());
        let controls = div()
            .flex()
            .items_center()
            .gap(theme::GAP)
            .child(
                button("open-log", open, "external-link")
                    .tip(path.clone())
                    .enabled(!path.is_empty())
                    .on_click(move |_, _, cx| cx.open_with_system(std::path::Path::new(&opened))),
            )
            .child(button("copy-log-path", copy, "copy").enabled(!path.is_empty()).on_click(move |_, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(copied.clone()))))
            .when(finds, |d| {
                let tip = format!("{}. {}", find, put(&say!("Deactivates half of the mods; you play and say whether it still breaks. This repeats until one mod is left. Your list is kept and comes back at the end.")));
                d.child(button("find-broken", find.clone(), "search").tip(tip).on_click(cx.listener(|_, _, _, cx| crate::ui::navigate::go(crate::ui::navigate::Go::FindBroken, cx))))
            })
            .child(Choice::new("log-view", views, self.view as usize).title(put(&say!("View"))).on_pick(crate::ui::act(cx, |logs, at: usize, _, cx| {
                logs.view = [View::Errors, View::Mods, View::Log][at];
                cx.notify();
            })));
        div()
            .flex()
            .when(wrapped, |d| {
                d.flex_col()
                    .items_start()
                    .gap(theme::GAP_SMALL)
                    .py(theme::GAP_SMALL)
            })
            .when(!wrapped, |d| d.items_center().gap(theme::GAP))
            .min_h(theme.header())
            .pl(theme::GAP_CARD)
            .pr(theme::GAP_LARGE)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(label(theme, theme.title(), c.ink, written).font_weight(theme.semibold))
                    .child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .sized_text(theme, theme.small())
                            .text_color(c.ink3)
                            .child(about),
                    ),
            )
            .child(controls)
            .into_any_element()
    }

    pub(super) fn since_box(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let c = &theme.colours;
        let on_last = !self.previous && self.kept.is_none();
        let report = self.report.clone()?;
        let since = since(&report).filter(|_| on_last && self.view == View::Errors)?;
        let ids = since.ids.to_vec();
        let mut column = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(2.))
            .child(
                div()
                    .sized_text(theme, theme.body())
                    .font_weight(theme.semibold)
                    .text_color(c.ink)
                    .child(put(&say!("Since the last clean launch"))),
            )
            .child(label(theme, theme.small(), c.ink2, moment(since.time)));
        if since.same {
            column = column.child(label(
                theme,
                theme.body(),
                c.ink2,
                put(&say!("Same load order")),
            ));
        } else {
            if !since.added.is_empty() {
                column = column.child(
                    div()
                        .sized_text(theme, theme.body())
                        .text_color(c.ink)
                        .child(format!("{} {}", put(&say!("Added")), since.added)),
                );
            }
            if !since.removed.is_empty() {
                column = column.child(
                    div()
                        .sized_text(theme, theme.body())
                        .text_color(c.ink)
                        .child(format!("{} {}", put(&say!("Removed")), since.removed)),
                );
            }
            if since.reordered {
                column = column.child(label(
                    theme,
                    theme.body(),
                    c.ink2,
                    put(&say!("Order changed")),
                ));
            }
        }
        Some(
            div()
                .flex()
                .items_center()
                .gap(theme::GAP)
                .mx(theme::GAP_LARGE)
                .mb(theme::GAP)
                .py(theme::GAP_MEDIUM)
                .pl(theme::GAP_LARGE)
                .pr(theme::GAP_MEDIUM)
                .rounded(theme::CORNER_LARGE)
                .bg(c.card)
                .child(column)
                .when(!since.same, |d| {
                    d.child(
                        Btn::new("use-clean-list", put(&say!("Use")))
                            .tip(put(&say!("Uses the load order of the last clean launch as the list on the Mods screen. Save writes it.")))
                            .on_click(cx.listener(move |_, _, _, cx| crate::ui::navigate::go(crate::ui::navigate::Go::UseList(ids.clone()), cx))),
                    )
                })
                .into_any_element(),
        )
    }

    pub(super) fn filter_row(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.view == View::Mods {
            return None;
        }
        let placeholder = match self.view {
            View::Log => put(&say!("Search the log")),
            _ => put(&say!("Filter by text or mod")),
        };
        let field = Field {
            id: "log-filter",
            text: &self.filter,
            icon: "",
            placeholder,
            found: None,
            keys: None,
        };
        Some(
            div()
                .flex()
                .items_center()
                .gap(theme::GAP)
                .mx(theme::GAP_LARGE)
                .mb(theme::GAP)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(field.render(theme, window, cx)),
                )
                .when(self.view == View::Log, |d| {
                    d.child(self.wrap_switch(
                        "wrap-log",
                        "wrapLog",
                        self.wrap_log,
                        put(&say!("Wrap long lines in the log")),
                        theme,
                        cx,
                    ))
                })
                .into_any_element(),
        )
    }

    pub(in crate::ui::logs) fn wrap_switch(
        &self,
        id: &'static str,
        key: &'static str,
        on: bool,
        tip: String,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let title = put(&say!("Wrap long lines"));
        let switch = div()
            .id(id)
            .flex()
            .items_center()
            .gap(theme::GAP)
            .child(
                Toggle::new((id, 1usize), on, title.clone())
                    .silent()
                    .on_flip(crate::ui::act(cx, move |logs, on: bool, _, cx| {
                        logs.keep_wrap(key, on, cx)
                    })),
            )
            .child(label(theme, theme.small(), theme.colours.ink2, title));
        crate::ui::tip::tipped(switch, tip.into()).into_any_element()
    }
}
