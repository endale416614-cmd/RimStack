use super::screen::text_fill;
use crate::ui::listed::Listed;
use crate::ui::navigate::{Go, go};
use crate::ui::page::Paging;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{Btn, TextSized, label};
use crate::words::put;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, Render, Styled, Window, div,
    prelude::FluentBuilder,
};
use rimstack_core::say;

pub struct DownloadList {
    pane: bool,
    paging: Paging,
}

impl DownloadList {
    pub fn new(pane: bool, cx: &mut Context<Self>) -> Self {
        let reduce = cx.global::<Theme>().appearance.reduce_motion;
        Self {
            pane,
            paging: Paging::new(reduce),
        }
    }

    fn items(&self, theme: &Theme, listed: &Listed, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pane = self.pane;
        let offers_install = pane;
        let c = &theme.colours;
        let list = &*listed.list;
        if list.count == 0 {
            let said = put(&say!("Nothing on the list yet. Add mods from the results."));
            return vec![
                div()
                    .sized_text(theme, theme.body())
                    .text_color(c.ink3)
                    .pb(theme::GAP)
                    .child(said)
                    .into_any_element(),
                div().when(pane, |d| d.flex_1()).into_any_element(),
            ];
        }
        let rows: Vec<AnyElement> = list
            .rows
            .iter()
            .enumerate()
            .map(|(at, row)| {
                let title = if row.title.is_empty() {
                    row.id.clone()
                } else {
                    row.title.clone()
                };
                let id = row.id.clone();
                div()
                    .flex()
                    .items_center()
                    .gap(theme::GAP)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(text_fill(theme, theme.body(), c.ink, title))
                            .child(text_fill(theme, theme.small(), c.ink3, put(&row.reason)))
                            .when_some(row.failed.as_ref(), |d, failed| {
                                d.child(
                                    div()
                                        .sized_text(theme, theme.small())
                                        .text_color(c.err)
                                        .child(put(failed)),
                                )
                            }),
                    )
                    .when_some(row.size_said.as_ref(), |d, size| {
                        d.child(label(theme, theme.small(), c.ink3, put(size)))
                    })
                    .child(
                        Btn::new(("unlist", at), "")
                            .tip(put(&say!("Take off the list")))
                            .icon("x")
                            .on_click(cx.listener(move |_, _, _, cx| {
                                crate::ui::app(cx).remove_from_list(&id)
                            })),
                    )
                    .into_any_element()
            })
            .collect();
        let failed = list.rows.iter().filter(|row| row.failed.is_some()).count();
        let tall = (theme.line(theme.body()) + theme.line(theme.small())) * list.rows.len() as f32
            + theme.line(theme.small()) * failed as f32
            + theme::GAP_TINY * list.rows.len().saturating_sub(1) as f32;
        let held = div()
            .flex()
            .flex_col()
            .gap(theme::GAP_TINY)
            .pr(crate::ui::page::room(theme))
            .children(rows);
        let mut shown = vec![
            div()
                .when(pane, |d| d.flex_1().min_h_0())
                .when(!pane, |d| d.h(tall.min(theme.sized(240.))))
                .child(crate::ui::page::page(
                    "download-list-rows",
                    &self.paging,
                    theme,
                    cx.entity_id(),
                    held,
                ))
                .into_any_element(),
        ];
        if let Some(stuck) = &list.action.stuck {
            shown.push(
                div()
                    .sized_text(theme, theme.small())
                    .text_color(c.ink3)
                    .child(put(stuck))
                    .into_any_element(),
            );
        }
        let action = &list.action;
        let asks_install = offers_install && action.install_steamcmd;
        shown.push(
            div()
                .flex()
                .flex_wrap()
                .gap(theme::GAP)
                .when(action.download, |d| {
                    d.child(
                        Btn::new("download-wanted", put(&action.label))
                            .tip(put(&say!("Into the game's own Mods folder, {}, without subscribing: Steam will not update these.", listed.local_mods.clone())))
                            .icon("download")
                            .primary()
                            .enabled(action.enabled)
                            .on_click(|_, _, cx| go(Go::DownloadListed, cx)),
                    )
                })
                .when(action.subscribe, |d| {
                    d.child(
                        Btn::new("subscribe-wanted", put(&say!("Subscribe in Steam")))
                            .tip(put(&say!("Steam must be running. It subscribes to each and downloads it, then keeps them up to date.")))
                            .icon("plus")
                            .primary_if(!action.download && !asks_install)
                            .enabled(action.subscribe_enabled)
                            .on_click(|_, _, cx| go(Go::SubscribeListed, cx)),
                    )
                })
                .when(asks_install, |d| {
                    d.child(
                        Btn::new("install-steamcmd-listed", put(&action.label))
                            .icon("download")
                            .primary()
                            .enabled(action.enabled)
                            .on_click(|_, _, cx| go(Go::InstallSteamcmd, cx)),
                    )
                })
                .when(list.failures, |d| {
                    d.child(
                        Btn::new("retry-list", put(&say!("Retry")))
                            .tip(put(&say!("Forgets what went wrong, so the next download tries these again.")))
                            .icon("refresh-cw")
                            .on_click(
                                cx.listener(|_, _, _, cx| crate::ui::app(cx).clear_list_failures()),
                            ),
                    )
                })
                .child(
                    Btn::new("empty-list", put(&say!("Empty the list")))
                        .on_click(cx.listener(|_, _, _, cx| crate::ui::app(cx).empty_list())),
                )
                .into_any_element(),
        );
        shown
    }
}

impl Render for DownloadList {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let listed = crate::ui::listed::read(cx);
        let items = self.items(&theme, &listed, cx);
        let gap = match self.pane {
            true => theme::GAP,
            false => theme::GAP_SMALL,
        };
        div()
            .flex()
            .flex_col()
            .gap(gap)
            .when(self.pane, |d| d.flex_1().min_h_0())
            .children(items)
    }
}
