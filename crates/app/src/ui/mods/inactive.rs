use super::Mods;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{Btn, Chip, btn_width, chip_width, label, measure};
use crate::words::{count, put};
use crate::workspace::load_order::Side;
use gpui::{Context, FontWeight, ParentElement, Styled, Window, div, prelude::FluentBuilder, px};
use rimstack_core::say;

impl Mods {
    pub(super) fn inactive_pane(
        &mut self,
        theme: &Theme,
        width: gpui::Pixels,
        drawer: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let title = put(&say!("Inactive"));
        let counted = count(self.list.read(cx).count(Side::Inactive));
        let order = put(&self.orders.inactive_said);
        let hide = match drawer {
            true => put(&say!("Pin")),
            false => put(&say!("Hide")),
        };
        let fresh = (self.reads.fresh > 0).then(|| put(&say!("{} new", count(self.reads.fresh))));
        let named = measure(window, theme, &title, theme.heading(), FontWeight::BOLD)
            + theme::GAP * 2.
            + measure(window, theme, &counted, theme.body(), FontWeight::NORMAL)
            + fresh.as_deref().map_or(px(0.), |said| {
                chip_width(window, theme, said, false) + theme::GAP
            });
        let acts = btn_width(window, theme, &order, true, true)
            + theme::GAP
            + btn_width(window, theme, &hide, false, false);
        let room = width - theme::GAP_CARD - theme::GAP_LARGE;
        let stacked = named + acts + theme::GAP > room;
        let buttons = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(theme::GAP)
            .child({
                let me = cx.entity().downgrade();
                let button = Btn::new("inactive-order", order)
                    .icon("arrow-up-down")
                    .tip(put(&say!("Order the inactive mods")))
                    .spoken(put(&say!("Order the inactive mods")))
                    .opens(move |below, window, cx| {
                        let _ = me.update(cx, |mods, cx| mods.inactive_order_menu(below, window, cx));
                    });
                super::columns::placed(button, self.layout.menu_buttons.order.clone())
            })
            .child(match drawer {
                true => Btn::new("pin-inactive", hide)
                    .tip(put(&say!("Beside the load order, always open")))
                    .on_click(cx.listener(|mods, _, _, cx| mods.set_hidden(false, cx))),
                false => Btn::new("hide-inactive", hide)
                    .tip(put(&say!(
                        "Folds the list into a bar at the left edge; its button opens the list over the load order"
                    )))
                    .on_click(cx.listener(|mods, _, _, cx| mods.set_hidden(true, cx))),
            });
        let mirror = drawer.then(|| {
            div()
                .px(theme::GAP_LARGE)
                .pb(theme::GAP)
                .child(self.filter_field(
                    &self.mirror.clone(),
                    "drawer-filter",
                    width - theme::GAP_LARGE * 2.,
                    theme,
                    window,
                    cx,
                ))
        });
        let only_new = self.reads.fresh > 0 && self.filter_text(cx) == "is:new";
        let activate = only_new.then(|| {
            div()
                .flex()
                .px(theme::GAP_LARGE)
                .pb(theme::GAP)
                .child(
                    Btn::new(
                        "activate-fresh",
                        put(&say!("Activate all {}", count(self.reads.fresh))),
                    )
                    .primary()
                    .enabled(!self.list.read(cx).previewing())
                    .tip(put(&say!(
                        "Each goes where its rules put it in the load order. One Undo takes them all back."
                    )))
                    .on_click(cx.listener(|mods, _, _, cx| mods.activate_fresh(cx))),
                )
        });
        let shows = !super::columns::chosen(&self.view, &self.orders, Side::Inactive).is_empty();
        let headings = shows.then(|| {
            let columns = super::columns::columns(
                theme,
                &self.orders,
                &self.view,
                Side::Inactive,
                width,
                window,
            );
            self.heads(theme, Side::Inactive, &columns, window, cx)
        });
        if !shows {
            self.layout.shown[Side::Inactive as usize].clear();
        }
        let c = &theme.colours;
        div()
            .flex()
            .flex_col()
            .h_full()
            .bg(c.panel)
            .child(
                div()
                    .flex()
                    .when(stacked, |d| d.flex_col().items_end())
                    .when(!stacked, |d| d.items_center())
                    .pl(theme::GAP_CARD)
                    .pr(theme::GAP_LARGE)
                    .gap(theme::GAP)
                    .when(stacked, |d| d.gap_0())
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .w_full()
                            .items_center()
                            .gap(theme::GAP)
                            .h(theme.header())
                            .child(
                                label(theme, theme.heading(), c.ink, title)
                                    .font_weight(FontWeight::BOLD),
                            )
                            .child(label(theme, theme.body(), c.ink3, counted))
                            .when_some(fresh, |d, said| {
                                d.child(
                                    Chip::new(said)
                                        .tone(c.ok)
                                        .tip(put(&say!("Installed since the game was last started from RimStack. Click to show only these.")))
                                        .on_click(
                                            "fresh-chip",
                                            cx.listener(|mods, _, _, cx| {
                                                let only = mods.filter_text(cx) == "is:new";
                                                mods.filter_by(if only { "" } else { "is:new" }, cx);
                                            }),
                                        ),
                                )
                            }),
                    )
                    .child(buttons.when(stacked, |d| d.mb(theme::GAP))),
            )
            .children(mirror)
            .children(activate)
            .children(headings)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(crate::ui::cached(self.list(Side::Inactive).clone())),
            )
    }
}
