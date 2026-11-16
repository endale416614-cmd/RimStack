use super::super::Mods;
use super::category_icon;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{icon, label};
use crate::words::{count, put};
use crate::workspace::load_order::{RowView, Side};
use gpui::{
    AnyElement, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, WeakEntity, div, prelude::FluentBuilder, px,
};
use rimstack_core::say;

fn arrow(theme: &Theme, collapsed: bool, colour: Hsla) -> gpui::Div {
    label(
        theme,
        theme.caption(),
        colour,
        if collapsed { "▸" } else { "▾" },
    )
    .w(theme.sized(10.))
}

pub(super) fn tipped_label(id: String, text: gpui::Div, tip: String) -> AnyElement {
    crate::ui::tip::tipped(div().id(SharedString::from(id)).child(text), tip.into())
        .into_any_element()
}

pub(super) fn heading(theme: &Theme, row: &RowView, colour: Hsla) -> AnyElement {
    let c = &theme.colours;
    div()
        .absolute()
        .left(theme::GAP_WIDE)
        .bottom(theme::GAP_TINY)
        .flex()
        .items_center()
        .gap(theme::GAP_SMALL)
        .child(arrow(theme, row.collapsed, c.ink2))
        .child(icon(category_icon(row.category), theme.icon(), colour))
        .child(tipped_label(
            format!("heading-{}", row.category as usize),
            label(theme, theme.small(), c.ink, put(&row.name)).font_weight(theme.semibold),
            put(&match row.collapsed {
                true => say!("Click to show its mods"),
                false => say!("Click to hide its mods"),
            }),
        ))
        .child(label(theme, theme.small(), c.ink3, count(row.count)))
        .into_any_element()
}

pub(super) fn sub(
    theme: &Theme,
    row: &RowView,
    side: Side,
    at: usize,
    pointed: bool,
    mods: WeakEntity<Mods>,
) -> AnyElement {
    let c = &theme.colours;
    let counted = match row.group_size > row.count {
        true => put(&say!("{} of {}", count(row.count), count(row.group_size))),
        false => count(row.count),
    };
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .pl(theme::GAP_WIDE + theme.sized(28.))
        .gap(theme::GAP_SMALL)
        .child(arrow(theme, row.collapsed, c.ink3))
        .child(tipped_label(
            format!("group-{}", put(&row.name)),
            label(theme, theme.small(), c.ink2, put(&row.name)),
            put(&match row.collapsed {
                true => say!("Click to show its mods"),
                false => say!("The mods whose first tag this is. Click to hide them."),
            }),
        ))
        .child(label(theme, theme.small(), c.ink3, counted))
        .child(div().flex_1())
        .when(pointed, |d| d.child(crate::ui::tip::tipped(
            div()
                .id(SharedString::from(format!("untag-{}", put(&row.name))))
                .role(gpui::Role::Button)
                .aria_label(put(&say!("Remove the tag {} from these mods", put(&row.name))))
                .flex()
                .items_center()
                .justify_center()
                .size(theme.sized(24.))
                .mr(theme::GAP_CARD)
                .rounded(theme::CORNER)
                .bg(c.ctl)
                .hover(|s| s.bg(c.ctl_hi))
                .cursor_pointer()
                .child(icon("x", theme.sized(12.), c.ink))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    let _ = mods.update(cx, |mods, cx| mods.remove_sub(side, at, cx));
                }),
            put(&say!("Takes this tag off these mods; they stay where they are")).into(),
        )))
        .when(row.again, |d| {
            d.child(tipped_label(
                format!("group-rest-{}", put(&row.name)),
                label(theme, theme.small(), c.ink3, put(&say!("the rest of it"))),
                put(&say!(
                    "This group has mods further up as well: a rule keeps them apart, and Sort follows rules before groups."
                )),
            ))
        })
        .into_any_element()
}

pub(super) fn again(theme: &Theme, row: &RowView) -> AnyElement {
    let c = &theme.colours;
    let said = format!("{}  {}", put(&row.name), count(row.count));
    let tip = put(&say!(
        "This category has mods further up as well. Sort brings a category's mods together."
    ));
    let line = div()
        .id(SharedString::from(format!(
            "again-{}-{}",
            row.category as usize, row.number
        )))
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .px(theme::GAP_WIDE)
        .gap(theme::GAP_SMALL)
        .child(arrow(theme, row.collapsed, c.ink3))
        .child(label(theme, theme.caption(), c.ink3, said))
        .child(label(
            theme,
            theme.caption(),
            c.warn,
            put(&say!("out of category order")),
        ))
        .child(div().flex_1().ml(theme::GAP_SMALL).h(px(1.)).bg(c.line));
    crate::ui::tip::tipped(line, tip.into()).into_any_element()
}
