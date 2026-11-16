use super::Mods;
use super::chosen::DetailsView;
use super::menus::Naming;
use crate::ui::button::Btn;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, elide_middle, icon, label};
use crate::words::{count, put};
use crate::workspace::load_order::{Level, SelectionSummary};
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, Pixels, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use rimstack_core::mods::library::ModId;
use rimstack_core::{say, say_n};

const MOST: usize = 12;

impl Mods {
    fn chosen_ids(&self, cx: &gpui::App) -> Vec<ModId> {
        self.list.read(cx).selection.clone()
    }

    pub(super) fn bulk_bar(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        use gpui::AnimationExt;
        let actions = self.bulk_actions(theme, cx);
        let height = match actions.is_some() {
            true => theme.header(),
            false => px(0.),
        };
        div()
            .flex_none()
            .overflow_hidden()
            .children(actions)
            .with_spring(
                "bulk-height",
                crate::ui::motion::towards(height, 0.5, crate::ui::motion::QUICK),
                |d, height| d.h(height),
            )
            .into_any_element()
    }

    fn bulk_actions(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let list = self.list.read(cx);
        if list.selection.len() < 2 || list.order.previewing() || self.comparing.is_some() {
            return None;
        }
        let c = &theme.colours;
        let active = list.selected_active;
        let warned = list.order.version_warned(&list.selection).len();
        let in_order = active && self.orders.active_key.is_empty();
        let ask = |naming: Naming| {
            cx.listener(
                move |mods: &mut Self,
                      _: &gpui::ClickEvent,
                      window: &mut Window,
                      cx: &mut Context<Self>| {
                    let ids = mods.chosen_ids(cx);
                    mods.ask_name(ids, naming, window, cx)
                },
            )
        };
        Some(
            div()
                .relative()
                .flex()
                .flex_none()
                .justify_center()
                .items_center()
                .h(theme.header())
                .bg(c.bar)
                .child(div().absolute().top_0().left_0().right_0().h(px(1.)).bg(c.edge))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(theme::GAP_SMALL)
                        .child(
                            label(theme, theme.body(), c.ink, put(&say!("{} selected", list.selection.len())))
                                .font_weight(theme.semibold)
                                .mr(px(6.)),
                        )
                        .child(
                            Btn::new("bulk-across", match active {
                                true => put(&say!("Deactivate")),
                                false => put(&say!("Activate")),
                            })
                            .icon(if active { "minus" } else { "plus" })
                            .primary()
                            .on_click(cx.listener(|m, _, _, cx| m.move_selected(cx))),
                        )
                        .child(
                            Btn::new("bulk-tag", put(&say!("Tag…")))
                                .icon("tag")
                                .tip(put(&say!("Adds a tag to every selected mod")))
                                .on_click(ask(Naming::Tag)),
                        )
                        .child(
                            Btn::new("bulk-group", put(&say!("Group…")))
                                .tip(put(&say!("Makes a tag the first of every selected mod: inside a category they are kept side by side under it")))
                                .on_click(ask(Naming::Group)),
                        )
                        .when(warned > 0, |d| {
                            d.child(
                                Btn::new("bulk-trust", put(&say!("Ignore version warning  {}", warned)))
                                    .icon("eye-off")
                                    .tip(put(&say!("No warning about the game version for the selected mods that have one")))
                                    .on_click(cx.listener(|m, _, _, cx| {
                                        let ids = m.list.read(cx).order.version_warned(&m.chosen_ids(cx));
                                        m.trust(ids, true, cx)
                                    })),
                            )
                        })
                        .when(in_order, |d| {
                            d.child(div().w(px(1.)).h(theme.sized(16.)).bg(c.line))
                                .child(
                                    Btn::new("bulk-top", put(&say!("To top")))
                                        .icon("arrow-up")
                                        .tip(put(&say!("To the top of their category")))
                                        .on_click(cx.listener(|m, _, _, cx| m.to_edge(true, cx))),
                                )
                                .child(
                                    Btn::new("bulk-bottom", put(&say!("To bottom")))
                                        .icon("arrow-down")
                                        .tip(put(&say!("To the bottom of their category")))
                                        .on_click(cx.listener(|m, _, _, cx| m.to_edge(false, cx))),
                                )
                        })
                        .child(div().w(theme::GAP_WIDE))
                        .child(
                            Btn::new("bulk-invert", put(&say!("Invert")))
                                .tip(put(&say!("Selects the other mods the list shows instead (Ctrl + I)")))
                                .on_click(cx.listener(move |m, _, _, cx| m.invert_selection(active, cx))),
                        )
                        .child(
                            Btn::new("bulk-clear", put(&say!("Clear selection")))
                                .tip(put(&say!("Clear the selection (Esc)")))
                                .on_click(cx.listener(|m, _, _, cx| m.select(None, true, cx))),
                        ),
                )
                .into_any_element(),
        )
    }

    pub(super) fn selection_summary(
        &self,
        cx: &gpui::App,
    ) -> Option<std::rc::Rc<SelectionSummary>> {
        let list = self.list.read(cx);
        (list.selection.len() > 1).then(|| {
            std::rc::Rc::new(
                list.order
                    .selection_summary(list.selected_active, &list.selection),
            )
        })
    }
}

impl DetailsView {
    pub(super) fn selection_pane(
        &self,
        theme: &Theme,
        summary: &SelectionSummary,
        width: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let shown = match self.summary_open {
            true => summary.rows.len(),
            false => summary.rows.len().min(MOST),
        };
        let count_chosen = self.shown.selected;
        let rows = summary.rows[..shown].iter().map(|one| {
            let colour = theme.category_colour(one.category as usize);
            let mark = match one.trouble {
                Level::Error => Some(("circle-alert", c.err)),
                Level::Warning => Some(("triangle-alert", c.ink3)),
                Level::None => None,
            };
            let room = width
                - px(7.)
                - theme::GAP * 3.
                - if mark.is_some() {
                    theme.sized(12.)
                } else {
                    px(0.)
                }
                - if one.load.is_empty() {
                    px(0.)
                } else {
                    theme.sized(48.)
                };
            div()
                .flex()
                .items_center()
                .gap(theme::GAP)
                .h(theme.sized(24.))
                .child(div().flex_none().size(px(7.)).rounded(px(3.5)).bg(colour))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .sized_text(theme, theme.body())
                        .text_color(c.ink)
                        .child(elide_middle(window, theme, &one.name, theme.body(), room)),
                )
                .when_some(mark, |d, (name, colour)| {
                    d.child(icon(name, theme.sized(12.), colour))
                })
                .when(!one.load.is_empty(), |d| {
                    d.child(label(theme, theme.small(), c.ink3, one.load.clone()))
                })
        });
        let more = summary.rows.len() > MOST;
        let troubled = put(&summary.troubled);
        let fact = |name: String, value: String| {
            div()
                .flex()
                .gap(theme::GAP_LARGE)
                .child(label(theme, theme.body(), c.ink2, name))
                .child(label(theme, theme.body(), c.ink, value))
        };
        div()
            .flex()
            .flex_col()
            .child(
                label(
                    theme,
                    theme.heading(),
                    c.ink,
                    put(&say_n!(count_chosen, "{} mod selected", "{} mods selected")),
                )
                .font_weight(theme.semibold),
            )
            .child(
                div()
                    .pt(theme::GAP_HAIR)
                    .sized_text(theme, theme.body())
                    .text_color(c.ink2)
                    .child(put(&summary.place)),
            )
            .child(crate::ui::card::group_title(theme, put(&say!("Selected"))))
            .children(rows)
            .when(more, |d| {
                let text = match self.summary_open {
                    true => put(&say!("Show {}", MOST)),
                    false => put(&say!("{} more", summary.rows.len() - MOST)),
                };
                d.child(div().h(theme::GAP_SMALL)).child(div().flex().child(
                    Btn::new("summary-more", text).on_click(cx.listener(|view, _, _, cx| {
                        view.summary_open = !view.summary_open;
                        cx.notify();
                    })),
                ))
            })
            .when(!troubled.is_empty() || summary.not_in_view > 0, |d| {
                d.child(crate::ui::card::group_title(theme, put(&say!("Together"))))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(theme::GAP_TINY)
                            .when(!troubled.is_empty(), |d| {
                                d.child(fact(put(&say!("With a problem")), troubled.clone()))
                            })
                            .when(summary.not_in_view > 0, |d| {
                                d.child(fact(put(&say!("Not in view")), count(summary.not_in_view)))
                            }),
                    )
            })
            .into_any_element()
    }
}
