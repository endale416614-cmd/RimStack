use super::Find;
use super::layout::Layout;
use crate::ui::button::Btn;
use crate::ui::controls::Toggle;
use crate::ui::field::Field;
use crate::ui::rows::{bar, rows};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, breakable, elide_start_in, label, measure_in, with_alpha};
use crate::words::put;
use crate::workspace::find::PreviewShown;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Pixels, Role,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use rimstack_core::say;
use std::rc::Rc;

impl Find {
    fn preview_tops(&self, theme: &Theme, layout: Layout, window: &Window) -> Rc<[Pixels]> {
        let Some(shown) = &self.preview else {
            return Rc::from(vec![px(0.)]);
        };
        let line = theme.line(theme.small());
        let per_row = match self.wrap {
            false => usize::MAX,
            true => {
                let advance = measure_in(
                    window,
                    theme,
                    theme.mono.clone(),
                    "n",
                    theme.small(),
                    FontWeight::NORMAL,
                );
                let room = layout.preview_width - Self::code_x(layout) - theme::GAP_LARGE;
                ((room / advance).floor() as usize).max(1)
            }
        };
        let mut at = px(0.);
        let mut tops = Vec::with_capacity(shown.lines.len() + 1);
        for one in &shown.lines {
            tops.push(at);
            let rows = one.text.chars().count().div_ceil(per_row).max(1);
            at += line * rows as f32 + theme::GAP_HAIR;
        }
        tops.push(at);
        tops.into()
    }

    fn code_x(layout: Layout) -> Pixels {
        theme::GAP + layout.digits + theme::GAP
    }

    pub(super) fn centre_preview(&mut self, cx: &mut Context<Self>) {
        let Some(shown) = self.preview.clone() else {
            return;
        };
        let first = shown.lines.first().map_or(0, |l| l.number);
        let at = shown.line.saturating_sub(first);
        self.preview_scroll.borrow_mut().set_top(px(0.));
        self.centred = Some(at);
        cx.notify();
    }

    pub(super) fn preview_pane(
        &mut self,
        theme: &Theme,
        layout: Layout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let Some(shown) = self.preview.clone() else {
            return div().into_any_element();
        };
        let tops = self.preview_tops(theme, layout, window);
        if let Some(at) = self.centred.take()
            && at + 1 < tops.len()
        {
            self.preview_scroll
                .borrow_mut()
                .centre(tops[at], tops[at + 1]);
        }
        let inner = layout.preview_width - theme::GAP_LARGE * 2.;
        let path = elide_start_in(
            window,
            theme,
            theme.mono.clone(),
            &shown.path,
            theme.small(),
            inner,
        );
        let inside_found = self.viewing.found.len();
        let field = Field {
            id: "find-inside",
            text: &self.inside,
            icon: "search",
            placeholder: put(&say!("Search in this file")),
            found: self.viewing.said.as_ref().map(put),
            keys: None,
        };
        let buttons = div()
            .flex()
            .flex_wrap()
            .gap(theme::GAP_SMALL)
            .child(
                Btn::new("find-open-file", put(&say!("Open file")))
                    .icon("external-link")
                    .on_click(cx.listener(|find, _, _, cx| find.open_file(cx))),
            )
            .child(
                Btn::new("find-open-folder", put(&say!("Open folder")))
                    .icon("folder-open")
                    .on_click(cx.listener(|find, _, _, cx| find.open_folder(cx))),
            )
            .child(
                Btn::new("find-copy-path", put(&say!("Copy path")))
                    .icon("copy")
                    .on_click(cx.listener(|find, _, _, cx| find.copy_path(cx))),
            )
            .child(
                Btn::new("find-copy-line", put(&say!("Copy line")))
                    .icon("copy")
                    .on_click(cx.listener(|find, _, _, cx| find.copy_line(cx))),
            );
        let inside = div()
            .flex()
            .items_center()
            .gap(theme::GAP_SMALL)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(field.render(theme, window, cx)),
            )
            .child(
                Btn::new("find-inside-up", "")
                    .icon("chevron-up")
                    .tip(put(&say!("Previous match")))
                    .enabled(inside_found > 0)
                    .on_click(cx.listener(|find, _, _, cx| find.inside_step(-1, cx))),
            )
            .child(
                Btn::new("find-inside-down", "")
                    .icon("chevron-down")
                    .tip(put(&say!("Next match")))
                    .enabled(inside_found > 0)
                    .on_click(cx.listener(|find, _, _, cx| find.inside_step(1, cx))),
            );
        let wrap_words = put(&say!("Wrap lines"));
        let wrap = div()
            .flex()
            .items_center()
            .gap(theme::GAP)
            .child(
                Toggle::new("find-wrap", self.wrap, wrap_words.clone())
                    .silent()
                    .on_flip(crate::ui::act(cx, |find, on: bool, _, cx| {
                        find.wrap = on;
                        cx.notify();
                    })),
            )
            .child(label(theme, theme.small(), c.ink2, wrap_words));
        let problem = shown.problem.as_ref().map(put);
        div()
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .h_full()
            .w(layout.preview_width)
            .gap(theme::GAP_SMALL)
            .bg(c.panel)
            .child(
                div()
                    .pt(theme::GAP_LARGE)
                    .px(theme::GAP_LARGE)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .sized_text(theme, theme.small())
                    .font_family(theme.mono.clone())
                    .text_color(c.ink)
                    .child(path),
            )
            .child(div().px(theme::GAP_LARGE).child(buttons))
            .child(div().px(theme::GAP_LARGE).child(inside))
            .child(div().px(theme::GAP_LARGE).child(wrap))
            .when_some(problem, |d, problem| {
                d.child(
                    div()
                        .px(theme::GAP_LARGE)
                        .sized_text(theme, theme.small())
                        .text_color(c.err)
                        .child(problem),
                )
            })
            .child(self.preview_lines(shown, tops, theme, layout, cx))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .w(theme::GAP_LINE)
                    .bg(c.edge),
            )
            .into_any_element()
    }

    fn preview_lines(
        &self,
        shown: Rc<PreviewShown>,
        tops: Rc<[Pixels]>,
        theme: &Theme,
        layout: Layout,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (owned, wrap, draw_tops) = (theme.clone(), self.wrap, tops.clone());
        let list = rows(
            "find-preview",
            self.preview_scroll.clone(),
            tops,
            theme.sized(40.),
            cx.entity_id(),
            move |range, width, _, _| {
                let theme = &owned;
                let c = &theme.colours;
                let code_x = Self::code_x(layout);
                range
                    .filter_map(|at| {
                        let one = shown.lines.get(at)?;
                        let text = one.text.replace('\t', "    ");
                        let tint = match () {
                            _ if one.marked => Some(with_alpha(c.accent, 0.25)),
                            _ if one.here => Some(with_alpha(c.warn, 0.4)),
                            _ if one.found => Some(with_alpha(c.warn, 0.18)),
                            _ => None,
                        };
                        Some(
                            div()
                                .id(("find-preview-line", at))
                                .role(Role::ListItem)
                                .relative()
                                .w(width)
                                .h(draw_tops[at + 1] - draw_tops[at])
                                .when_some(tint, |d, tint| d.bg(tint))
                                .child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .left(theme::GAP)
                                        .w(layout.digits)
                                        .flex()
                                        .justify_end()
                                        .sized_text(theme, theme.small())
                                        .font_family(theme.mono.clone())
                                        .text_color(c.ink3)
                                        .child(one.number.to_string()),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .left(code_x)
                                        .right(theme::GAP_LARGE)
                                        .sized_text(theme, theme.small())
                                        .font_family(theme.mono.clone())
                                        .text_color(c.ink)
                                        .when(!wrap, |d| {
                                            d.overflow_hidden().whitespace_nowrap().text_ellipsis()
                                        })
                                        .child(if wrap { breakable(&text) } else { text }),
                                )
                                .into_any_element(),
                        )
                    })
                    .collect()
            },
        );
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(list)
            .child(bar(theme, self.preview_scroll.clone(), cx.entity_id()))
            .into_any_element()
    }
}
