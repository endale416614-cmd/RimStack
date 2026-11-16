use super::{Logs, View};
use crate::ui::button::Btn;
use crate::ui::page::page;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, label};
use crate::words::{count, put};
use crate::workspace::logs::{errors_of, help_text};
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, Pixels, Styled, Window, div,
    prelude::FluentBuilder,
};
use rimstack_core::{say, say_n};

impl Logs {
    pub(super) fn side_width(theme: &Theme, window: &Window) -> Pixels {
        theme.sized(360.).min(window.viewport_size().width * 0.32)
    }

    fn group(theme: &Theme, title: String) -> AnyElement {
        label(theme, theme.small(), theme.colours.ink2, title)
            .font_weight(theme.semibold)
            .pt(theme::GAP_WIDE)
            .pb(theme::GAP_SMALL)
            .into_any_element()
    }

    fn mod_side(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let c = &theme.colours;
        let report = self.report.clone()?;
        let culprit = report.culprits.get(self.culprit)?.clone();
        let name = if culprit.name.is_empty() {
            put(&say!("No mod recognised"))
        } else {
            culprit.name.clone()
        };
        let theirs: Vec<AnyElement> = errors_of(&report, &culprit.name)
            .into_iter()
            .map(|error| {
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .sized_text(theme, theme.body())
                            .text_color(c.ink)
                            .child(error.head.clone()),
                    )
                    .when(error.count > 1, |d| {
                        d.child(label(
                            theme,
                            theme.small(),
                            c.ink3,
                            put(&say_n!(error.count, "logged {} time", "logged {} times")),
                        ))
                    })
                    .into_any_element()
            })
            .collect();
        let open = culprit.name.clone();
        let mod_id = culprit.mod_id;
        Some(
            div()
                .flex()
                .flex_col()
                .gap(theme::GAP)
                .child(
                    div()
                        .sized_text(theme, theme.heading())
                        .font_weight(theme.semibold)
                        .text_color(c.ink)
                        .child(name),
                )
                .child(
                    div()
                        .sized_text(theme, theme.body())
                        .text_color(c.ink2)
                        .child(format!(
                            "{}, {}",
                            put(&say_n!(culprit.kinds, "{} error", "{} errors")),
                            put(&say_n!(culprit.times, "logged {} time", "logged {} times"))
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(theme::GAP_SMALL)
                        .child(
                            Btn::new("show-its-errors", put(&say!("Show its errors")))
                                .tip(put(&say!("Errors, narrowed to this mod")))
                                .on_click(
                                    cx.listener(move |logs, _, _, cx| logs.open_culprit(&open, cx)),
                                ),
                        )
                        .when_some(mod_id, |d, mod_id| {
                            d.child(Btn::new("go-to-culprit", put(&say!("Go to mod"))).on_click(
                                cx.listener(move |_, _, _, cx| {
                                    crate::ui::navigate::go(
                                        crate::ui::navigate::Go::ShowMod(mod_id),
                                        cx,
                                    )
                                }),
                            ))
                        }),
                )
                .child(Self::group(theme, put(&say!("Errors"))))
                .children(theirs)
                .into_any_element(),
        )
    }

    fn error_side(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let c = &theme.colours;
        let report = self.report.clone()?;
        let shown = self.errors(&report);
        let error = shown.get(self.selected)?;
        let pointed = error.error;
        let line = pointed.line;
        let help = help_text(&report, pointed);
        let mods: Vec<AnyElement> = pointed
            .mods
            .iter()
            .enumerate()
            .map(|(at, named)| {
                let mod_id = named.mod_id;
                div()
                    .flex()
                    .items_center()
                    .gap(theme::GAP)
                    .h(theme.sized(31.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .sized_text(theme, theme.body())
                            .text_color(c.ink)
                            .child(named.name.clone()),
                    )
                    .child(
                        Btn::new(("go-to-mod", at), put(&say!("Go to mod"))).on_click(cx.listener(
                            move |_, _, _, cx| {
                                crate::ui::navigate::go(
                                    crate::ui::navigate::Go::ShowMod(mod_id),
                                    cx,
                                )
                            },
                        )),
                    )
                    .into_any_element()
            })
            .collect();
        let none = pointed.mods.is_empty();
        let trace = (!pointed.trace.is_empty()).then(|| error.trace_text.clone());
        let wrap = self.wrap_trace;
        Some(
            div()
                .flex()
                .flex_col()
                .child(div().sized_text(theme, theme.heading()).font_weight(theme.semibold).text_color(c.ink).child(error.kind.to_string()))
                .child(div().pt(theme::GAP_HAIR).sized_text(theme, theme.body()).text_color(c.ink2).child(put(&say_n!(pointed.count, "{} time, first at line {}", "{} times, first at line {}", count(line)))))
                .child(div().h(theme::GAP_MEDIUM))
                .child(
                    div()
                        .flex()
                        .gap(theme::GAP_SMALL)
                        .child(Btn::new("show-in-log", put(&say!("Show in log"))).on_click(cx.listener(move |logs, _, window, cx| logs.show_line(line, window, cx))))
                        .child(
                            Btn::new("copy-report", put(&say!("Copy report")))
                                .icon("copy")
                                .tip(put(&say!("The error, its stack trace, the game version and the likely mod, as text.")))
                                .on_click(move |_, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(help.clone()))),
                        ),
                )
                .child(Self::group(theme, put(&say!("Mods"))))
                .when(none, |d| {
                    let none = crate::ui::tip::tipped(
                        gpui::InteractiveElement::id(label(theme, theme.body(), c.ink2, put(&say!("None recognised"))), "none-recognised"),
                        put(&say!("No active mod's code is in the stack trace, and the error names none.")).into(),
                    );
                    d.child(none)
                })
                .children(mods)
                .child(Self::group(theme, put(&say!("Logged"))))
                .child(div().sized_text(theme, theme.body()).text_color(c.ink).child(pointed.head.clone()))
                .when_some(trace, |d, trace| {
                    d.child(
                        div()
                            .flex()
                            .items_end()
                            .flex_wrap()
                            .justify_between()
                            .child(Self::group(theme, put(&say!("Stack trace"))))
                            .child(div().pb(theme::GAP_TINY).child(self.wrap_switch("wrap-trace", "wrapTrace", wrap, put(&say!("Wrap long lines in the stack trace")), theme, cx))),
                    )
                    .child(
                        div()
                            .p(theme::GAP_MEDIUM)
                            .rounded(theme::CORNER_LARGE)
                            .bg(c.well)
                            .overflow_hidden()
                            .child(
                                div()
                                    .sized_text(theme, theme.small())
                                    .font_family(theme.mono.clone())
                                    .text_color(c.ink2)
                                    .when(!wrap, |d| d.whitespace_nowrap())
                                    .child(if wrap { trace } else { trace.replace('\u{200b}', "") }),
                            ),
                    )
                })
                .into_any_element(),
        )
    }

    pub(super) fn side(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let content = match self.view {
            View::Mods => self.mod_side(theme, cx),
            _ => self.error_side(theme, cx),
        };
        let inner = div()
            .p(theme::GAP_CARD)
            .pr(theme::GAP_CARD + crate::ui::page::room(theme))
            .children(content);
        div()
            .flex_none()
            .w(Self::side_width(theme, window))
            .h_full()
            .bg(c.panel)
            .border_l_1()
            .border_color(c.edge)
            .child(page(
                "log-side",
                &self.side_paging,
                theme,
                cx.entity_id(),
                inner,
            ))
            .into_any_element()
    }
}
