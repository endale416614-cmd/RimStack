mod row;
mod side;

use super::Saves;

use crate::ui::field::Field;

use crate::ui::rows::{bar, rows};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, label};
use crate::words::put;

use crate::workspace::saves::{SaveShown, shown};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels,
    Render, Role, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};

use rimstack_core::say;
use std::rc::Rc;

use row::{chips_of, save_row};

impl Saves {
    fn side_width(theme: &Theme, window: &Window) -> Pixels {
        theme.sized(360.).min(window.viewport_size().width * 0.36)
    }

    fn row_height(theme: &Theme, save: &SaveShown<'_>, pictured: bool) -> Pixels {
        let chips = chips_of(save).next().is_some();
        let facts = theme.line(theme.title())
            + theme::GAP_TINY
            + theme.line(theme.small())
            + theme::GAP_TINY
            + if chips { theme.tag() } else { px(0.) };
        let picture = if pictured { theme.sized(62.) } else { px(0.) };
        facts.max(picture) + theme::GAP_MEDIUM * 2.
    }

    fn heading(&self, heading: String, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let c = &theme.colours;
        let report = &self.report;
        let mut folders = vec![String::new()];
        folders.extend(report.folders.iter().cloned());
        let current = report.current_folder.clone().unwrap_or_default();
        let tabs = (report.folders.len() > 1).then(|| {
            div()
                .ml(theme::GAP)
                .p(px(2.))
                .rounded(theme::CORNER_LARGE)
                .bg(c.seg)
                .flex()
                .gap(theme::GAP_LINE)
                .children(folders.into_iter().enumerate().map(|(at, folder)| {
                    let on = folder == self.folder;
                    let said = match folder.is_empty() {
                        true => put(&say!("All")),
                        false if folder == current => format!("{folder} ●"),
                        false => folder.clone(),
                    };
                    div()
                        .id(("saves-folder", at))
                        .role(Role::Tab)
                        .aria_label(if folder.is_empty() {
                            put(&say!("All folders"))
                        } else {
                            folder.clone()
                        })
                        .flex()
                        .items_center()
                        .h(theme.sized(24.))
                        .px(theme::GAP_MEDIUM)
                        .rounded(theme::CORNER)
                        .when(on, |d| d.bg(c.ctl_hi))
                        .cursor_pointer()
                        .child(
                            label(theme, theme.small(), if on { c.ink } else { c.ink2 }, said)
                                .font_weight(theme.medium),
                        )
                        .on_click(cx.listener(move |saves, _, _, cx| {
                            saves.folder = folder.clone();
                            saves.selected = 0;
                            cx.notify();
                        }))
                }))
        });
        div()
            .flex()
            .items_center()
            .gap(theme::GAP)
            .h(theme.header())
            .px(theme::GAP_CARD)
            .child(
                label(theme, theme.heading(), c.ink, put(&say!("Saves")))
                    .font_weight(theme.semibold),
            )
            .child(label(theme, theme.body(), c.ink3, heading))
            .children(tabs)
            .into_any_element()
    }

    fn list(&self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let c = theme.colours;
        let report = self.report.clone();
        let (folder, filter) = (self.folder.clone(), self.filter_text.clone());
        let on_show = shown(&report, &folder, &filter);
        let (saves, pictured) = (&on_show.saves, on_show.pictured);
        let all_from = report.all_from.is_some();
        if saves.is_empty() {
            let width = (window.viewport_size().width
                - Self::side_width(theme, window)
                - theme::GAP_PAGE * 4.)
                .min(theme.sized(460.));
            let title = if self.busy {
                put(&say!("Reading saves…"))
            } else {
                put(&say!("No saves"))
            };
            let words = if self.folder.is_empty() {
                String::new()
            } else {
                put(&say!("RimSaves folder {} is empty", self.folder.clone()))
            };
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
                            label(theme, theme.title(), c.ink, title).font_weight(theme.semibold),
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
        let mut at = theme::GAP_HAIR;
        let mut tops = Vec::with_capacity(saves.len() + 1);
        for save in saves.iter() {
            tops.push(at);
            at += Self::row_height(theme, save, pictured) + theme::GAP_HAIR;
        }
        tops.push(at - theme::GAP_HAIR + theme::GAP_MEDIUM);
        let tops: Rc<[Pixels]> = tops.into();
        let (selected, me, thumbs, view) = (
            self.selected,
            cx.entity().downgrade(),
            self.thumbs.clone(),
            cx.entity_id(),
        );
        let theme_owned = theme.clone();
        let (held, folder, filter) = (report.clone(), folder.clone(), filter.clone());
        let list = rows(
            "save-rows",
            self.scroll.clone(),
            tops.clone(),
            theme.sized(40.),
            cx.entity_id(),
            move |range, width, window, cx| {
                let on_show = shown(&held, &folder, &filter);
                range
                    .filter_map(|at| {
                        let save = on_show.saves.get(at)?;
                        let height = Self::row_height(&theme_owned, save, pictured);
                        Some(save_row(
                            &theme_owned,
                            at,
                            save,
                            at == selected,
                            (pictured, all_from),
                            width,
                            height,
                            &thumbs,
                            view,
                            me.clone(),
                            window,
                            cx,
                        ))
                    })
                    .collect()
            },
        );
        let count = saves.len();
        let usable: Vec<Option<Vec<String>>> = saves
            .iter()
            .map(|s| {
                s.usable
                    .then(|| s.row.compared.as_ref().map(|c| c.ids.clone()))
                    .flatten()
            })
            .collect();
        div()
            .id("saves")
            .track_focus(&self.focus)
            .role(Role::List)
            .aria_label(put(&say!("Saves")))
            .relative()
            .flex_1()
            .min_h_0()
            .on_key_down(cx.listener(move |saves, event: &KeyDownEvent, _, cx| {
                let to = match event.keystroke.key.as_str() {
                    "up" => saves.selected.saturating_sub(1),
                    "down" => (saves.selected + 1).min(count - 1),
                    "home" => 0,
                    "end" => count - 1,
                    "enter" => {
                        if let Some(Some(ids)) = usable.get(saves.selected) {
                            crate::ui::navigate::go(
                                crate::ui::navigate::Go::UseList(ids.clone()),
                                cx,
                            );
                        }
                        return;
                    }
                    _ => return,
                };
                saves.selected = to;
                saves.scroll.borrow_mut().contain(tops[to], tops[to + 1]);
                cx.notify();
            }))
            .child(list)
            .child(bar(theme, self.scroll.clone(), cx.entity_id()))
            .into_any_element()
    }
}

impl Render for Saves {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let report = self.report.clone();
        let on_show = shown(&report, &self.folder, &self.filter_text);
        let heading = put(&on_show.heading);
        let searchable = on_show.searchable;
        let chosen = on_show.saves.get(self.selected);
        let side = self.side(chosen, &theme, window, cx);
        let list = self.list(&theme, window, cx);
        let filter = searchable.then(|| {
            let field = Field {
                id: "saves-filter",
                text: &self.filter,
                icon: "funnel",
                placeholder: put(&say!("Filter saves")),
                found: None,
                keys: None,
            };
            div()
                .px(theme::GAP_CARD)
                .pb(theme::GAP_SMALL)
                .child(field.render(&theme, window, cx).h(theme.sized(30.)))
        });
        div()
            .flex()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.heading(heading, &theme, cx))
                    .children(filter)
                    .child(list),
            )
            .child(side)
    }
}
